//! Registre des nœuds — miroir exact du `NODE_REGISTRY` de la maquette.
//! Chaque définition décrit les ports typés et les paramètres par défaut
//! utilisés à la fois par l'éditeur Flutter et par l'instanciation DSP.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Type de signal transporté par un port.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortKind {
    /// Audio échantillonné (f32 par canal)
    Audio,
    /// Valeur de contrôle modulable
    Control,
    /// Événements CLAP (notes, gate, transport) — flux discret
    Event,
    /// Données d'analyse (FFT, waveform, phase) — flux de visualisation
    Data,
}

pub const PORT_AUDIO: PortKind = PortKind::Audio;
pub const PORT_CONTROL: PortKind = PortKind::Control;
pub const PORT_EVENT: PortKind = PortKind::Event;
pub const PORT_DATA: PortKind = PortKind::Data;

/// Définition d'un port (nom + type).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortDef {
    pub name: String,
    pub kind: PortKind,
}

/// Paramètre par défaut déclaré par un type de nœud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryParam {
    pub param_id: String,
    pub name: String,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    pub step: f64,
}

/// Définition complète d'un type de nœud.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDef {
    /// Identifiant machine du type, ex. `filter_svf`
    pub node_type: String,
    /// Nom lisible affiché dans la palette
    pub display_name: String,
    pub category: String,
    pub inputs: Vec<PortDef>,
    pub outputs: Vec<PortDef>,
    #[serde(default)]
    pub params: Vec<RegistryParam>,
    /// `true` si le nœud reçoit les événements CLAP (notes/gate)
    #[serde(default)]
    pub handles_events: bool,
}

impl NodeDef {
    pub fn io(&mut self, inputs: &[(&str, PortKind)], outputs: &[(&str, PortKind)]) -> &mut Self {
        self.inputs = inputs
            .iter()
            .map(|(n, k)| PortDef { name: n.to_string(), kind: *k })
            .collect();
        self.outputs = outputs
            .iter()
            .map(|(n, k)| PortDef { name: n.to_string(), kind: *k })
            .collect();
        self
    }

    pub fn param(&mut self, id: &str, name: &str, default: f64, min: f64, max: f64, step: f64) -> &mut Self {
        self.params.push(RegistryParam {
            param_id: id.into(),
            name: name.into(),
            default,
            min,
            max,
            step,
        });
        self
    }

    pub fn events(&mut self) -> &mut Self {
        self.handles_events = true;
        self
    }
}

fn new(node_type: &str, display_name: &str, category: &str) -> NodeDef {
    NodeDef {
        node_type: node_type.into(),
        display_name: display_name.into(),
        category: category.into(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        params: Vec::new(),
        handles_events: false,
    }
}

use PortKind::{Audio, Control, Data, Event};

/// Registre global construit une seule fois (thread-safe).
pub struct NodeRegistry {
    defs: Vec<NodeDef>,
    by_type: BTreeMap<String, usize>,
}

impl NodeRegistry {
    pub fn global() -> &'static Self {
        use std::sync::OnceLock;
        static REG: OnceLock<NodeRegistry> = OnceLock::new();
        REG.get_or_init(Self::build)
    }

    fn build() -> Self {
        // --- E/S & CLAP EXTENSIONS ---
        let defs = vec![
            new("clap_audio_in", "Audio Input Port", "E/S & CLAP API")
                .io(&[], &[("Out L", Audio), ("Out R", Audio)])
                .clone(),
            new("clap_audio_out", "Audio Output Port", "E/S & CLAP API")
                .io(&[("In L", Audio), ("In R", Audio)], &[])
                .clone(),
            new("clap_note_in", "Note Input (MIDI/MPE)", "E/S & CLAP API")
                .io(&[], &[("Note", Event), ("Gate", Event), ("Velocity", Event), ("Expression", Event)])
                .events()
                .clone(),
            new("clap_note_out", "Note Output Port", "E/S & CLAP API")
                .io(&[("Note", Event), ("Gate", Event), ("Velocity", Event)], &[])
                .clone(),
            new("clap_param", "CLAP Parameter", "E/S & CLAP API")
                .io(&[("Mod Offset", Control)], &[("Value Out", Control)])
                .param("value", "Value", 0.5, 0.0, 1.0, 0.001)
                .clone(),
            new("clap_voice_alloc", "Voice Allocator (Poly)", "E/S & CLAP API")
                .io(&[("Note In", Event)], &[("Voice 1..N", Event)])
                .events()
                .clone(),
            new("clap_transport", "Transport Info (BPM)", "E/S & CLAP API")
                .io(&[], &[("BPM", Control), ("IsPlaying", Control), ("TimeSig", Control)])
                .clone(),
            new("clap_track_info", "Track Info Ext", "E/S & CLAP API")
                .io(&[], &[("Track Name", Data), ("Color", Data)])
                .clone(),
            new("clap_latency_tail", "Latency & Tail Reporter", "E/S & CLAP API")
                .io(&[("Latency ms", Control), ("Tail ms", Control)], &[])
                .clone(),

            // --- SOURCES & GÉNÉRATEURS DSP ---
            new("osc_va", "Oscillateur VA (Analog)", "DSP - Sources")
                .io(&[("Pitch", Control), ("PWM", Control), ("Sync", Control)], &[("Audio Out", Audio)])
                .param("waveform", "Waveform", 0.0, 0.0, 2.0, 1.0)
                .param("detune", "Detune (cents)", 0.0, -100.0, 100.0, 1.0)
                .clone(),
            new("osc_wavetable", "Oscillateur Wavetable", "DSP - Sources")
                .io(&[("Pitch", Control), ("WT Position", Control)], &[("Audio Out", Audio)])
                .param("table", "Wavetable", 0.0, 0.0, 3.0, 1.0)
                .clone(),
            new("noise_gen", "Générateur de Bruit", "DSP - Sources")
                .io(&[("Color", Control)], &[("Audio Out", Audio)])
                .param("level", "Level", 0.2, 0.0, 1.0, 0.001)
                .clone(),
            new("sample_player", "Lecteur d'Échantillons", "DSP - Sources")
                .io(&[("Trigger", Event), ("Pitch", Control)], &[("Out L", Audio), ("Out R", Audio)])
                .events()
                .clone(),
            new("additive_engine", "Moteur Additif", "DSP - Sources")
                .io(&[("Pitch", Control), ("Harmonics", Control)], &[("Audio Out", Audio)])
                .param("harmonics", "Harmonics", 8.0, 1.0, 64.0, 1.0)
                .clone(),

            // --- FILTRES & EQ ---
            new("filter_svf", "Filtre SVF (LP/HP/BP)", "DSP - Filtres & EQ")
                .io(
                    &[("Audio In", Audio), ("Cutoff", Control), ("Reso", Control)],
                    &[("LP Out", Audio), ("HP Out", Audio), ("BP Out", Audio)],
                )
                .param("mode", "Mode (LP/HP/BP)", 0.0, 0.0, 2.0, 1.0)
                .param("cutoff_hz", "Cutoff (Hz)", 1000.0, 20.0, 20000.0, 1.0)
                .param("resonance", "Resonance", 0.7, 0.1, 10.0, 0.01)
                .clone(),
            new("filter_ladder", "Filtre Ladder (24dB)", "DSP - Filtres & EQ")
                .io(
                    &[("Audio In", Audio), ("Cutoff", Control), ("Reso", Control), ("Drive", Control)],
                    &[("Audio Out", Audio)],
                )
                .param("cutoff_hz", "Cutoff (Hz)", 1000.0, 20.0, 20000.0, 1.0)
                .param("resonance", "Resonance", 0.0, 0.0, 1.0, 0.01)
                .clone(),
            new("eq_parametric", "Égaliseur Paramétrique", "DSP - Filtres & EQ")
                .io(
                    &[("Audio In", Audio), ("Freq", Control), ("Q", Control), ("Gain", Control)],
                    &[("Audio Out", Audio)],
                )
                .param("freq_hz", "Freq (Hz)", 1000.0, 20.0, 20000.0, 1.0)
                .param("q", "Q", 1.0, 0.1, 10.0, 0.01)
                .param("gain_db", "Gain (dB)", 0.0, -24.0, 24.0, 0.1)
                .clone(),
            new("filter_comb", "Filtre Peigne (Comb)", "DSP - Filtres & EQ")
                .io(
                    &[("Audio In", Audio), ("Time", Control), ("Feedback", Control)],
                    &[("Audio Out", Audio)],
                )
                .param("time_ms", "Time (ms)", 20.0, 0.1, 100.0, 0.1)
                .param("feedback", "Feedback", 0.5, 0.0, 0.99, 0.01)
                .clone(),
            new("filter_formant", "Filtre Formant Vocal", "DSP - Filtres & EQ")
                .io(&[("Audio In", Audio), ("Vowel", Control)], &[("Audio Out", Audio)])
                .param("vowel", "Vowel (a-ou)", 0.0, 0.0, 1.0, 0.01)
                .clone(),

            // --- DYNAMIQUE & AMPLITUDE ---
            new("gain_node", "Gain / Atténuateur", "DSP - Dynamique")
                .io(&[("Audio In", Audio), ("Gain dB", Control)], &[("Audio Out", Audio)])
                .param("gain_db", "Gain (dB)", 0.0, -60.0, 24.0, 0.1)
                .clone(),
            new("compressor", "Compresseur Stéréo", "DSP - Dynamique")
                .io(
                    &[
                        ("In L", Audio), ("In R", Audio), ("Threshold", Control),
                        ("Ratio", Control), ("Attack", Control), ("Release", Control),
                        ("Sidechain", Audio),
                    ],
                    &[("Out L", Audio), ("Out R", Audio)],
                )
                .param("threshold_db", "Threshold (dB)", -18.0, -60.0, 0.0, 0.1)
                .param("ratio", "Ratio", 4.0, 1.0, 20.0, 0.1)
                .param("attack_ms", "Attack (ms)", 10.0, 0.1, 200.0, 0.1)
                .param("release_ms", "Release (ms)", 100.0, 1.0, 2000.0, 1.0)
                .clone(),
            new("limiter", "Limiteur de Crête", "DSP - Dynamique")
                .io(&[("Audio In", Audio), ("Ceiling", Control)], &[("Audio Out", Audio)])
                .param("ceiling_db", "Ceiling (dB)", -1.0, -12.0, 0.0, 0.1)
                .clone(),
            new("noise_gate", "Expander / Noise Gate", "DSP - Dynamique")
                .io(&[("Audio In", Audio), ("Threshold", Control)], &[("Audio Out", Audio)])
                .param("threshold_db", "Threshold (dB)", -40.0, -80.0, -10.0, 0.1)
                .clone(),
            new("transient_shaper", "Transient Shaper", "DSP - Dynamique")
                .io(
                    &[("Audio In", Audio), ("Attack", Control), ("Sustain", Control)],
                    &[("Audio Out", Audio)],
                )
                .param("attack", "Attack", 0.5, 0.0, 2.0, 0.01)
                .param("sustain", "Sustain", 0.5, 0.0, 2.0, 0.01)
                .clone(),

            // --- EFFETS TEMPORELS & MODULATION ---
            new("delay_stereo", "Délai Stéréo / Tape", "DSP - Effets")
                .io(
                    &[("In L", Audio), ("In R", Audio), ("Time L", Control), ("Time R", Control), ("Feedback", Control)],
                    &[("Out L", Audio), ("Out R", Audio)],
                )
                .param("time_ms", "Time (ms)", 250.0, 1.0, 2000.0, 1.0)
                .param("feedback", "Feedback", 0.35, 0.0, 0.95, 0.01)
                .param("mix", "Mix", 0.5, 0.0, 1.0, 0.01)
                .clone(),
            new("reverb_algo", "Réverb Algorithmique", "DSP - Effets")
                .io(
                    &[("In L", Audio), ("In R", Audio), ("Decay", Control), ("Damp", Control), ("Size", Control), ("Mix", Control)],
                    &[("Out L", Audio), ("Out R", Audio)],
                )
                .param("decay", "Decay", 0.5, 0.0, 1.0, 0.01)
                .param("damp", "Damp", 0.5, 0.0, 1.0, 0.01)
                .param("size", "Size", 0.5, 0.1, 1.0, 0.01)
                .param("mix", "Mix (wet/dry)", 0.35, 0.0, 1.0, 0.01)
                .clone(),
            new("reverb_convolution", "Réverb Convolution", "DSP - Effets")
                .io(&[("In L", Audio), ("In R", Audio), ("IR Select", Control)], &[("Out L", Audio), ("Out R", Audio)])
                .param("ir_select", "IR Preset", 0.0, 0.0, 3.0, 1.0)
                .clone(),
            new("chorus_flanger", "Chorus / Flanger / Phaser", "DSP - Effets")
                .io(
                    &[("Audio In", Audio), ("Rate", Control), ("Depth", Control), ("Feedback", Control)],
                    &[("Out L", Audio), ("Out R", Audio)],
                )
                .param("rate_hz", "Rate (Hz)", 1.0, 0.05, 10.0, 0.01)
                .param("depth", "Depth", 0.5, 0.0, 1.0, 0.01)
                .param("feedback", "Feedback", 0.3, 0.0, 0.9, 0.01)
                .clone(),
            new("pitch_shifter", "Pitch Shifter", "DSP - Effets")
                .io(&[("Audio In", Audio), ("Semitones", Control)], &[("Audio Out", Audio)])
                .param("semitones", "Semitones", 0.0, -24.0, 24.0, 1.0)
                .clone(),

            // --- SATURATION & DISTORSION ---
            new("saturation_tape", "Saturation Tube / Tape", "DSP - Saturation")
                .io(
                    &[("Audio In", Audio), ("Drive", Control), ("Warmth", Control)],
                    &[("Audio Out", Audio)],
                )
                .param("drive", "Drive", 1.5, 1.0, 10.0, 0.1)
                .param("warmth", "Warmth", 0.5, 0.0, 1.0, 0.01)
                .clone(),
            new("waveshaper", "Waveshaper Courbe", "DSP - Saturation")
                .io(
                    &[("Audio In", Audio), ("Drive", Control), ("Curve", Control)],
                    &[("Audio Out", Audio)],
                )
                .param("drive", "Drive", 2.0, 1.0, 20.0, 0.1)
                .param("curve", "Curve", 0.0, 0.0, 3.0, 1.0)
                .clone(),
            new("bitcrusher", "Bitcrusher / Downsampler", "DSP - Saturation")
                .io(
                    &[("Audio In", Audio), ("Bits", Control), ("Downsample", Control)],
                    &[("Audio Out", Audio)],
                )
                .param("bits", "Bits", 8.0, 1.0, 16.0, 1.0)
                .param("downsample", "Downsample (div)", 1.0, 1.0, 16.0, 1.0)
                .clone(),

            // --- ANALYSE & MESURE ---
            new("fft_analyzer", "Analyseur Spectre FFT", "DSP - Analyse")
                .io(&[("Audio In", Audio)], &[("FFT Data", Data)])
                .clone(),
            new("peak_rms_meter", "Mesureur Crête / RMS", "DSP - Analyse")
                .io(&[("Audio In", Audio)], &[("Peak", Data), ("RMS", Data)])
                .clone(),
            new("lufs_meter", "Mesureur Sonie LUFS", "DSP - Analyse")
                .io(&[("In L", Audio), ("In R", Audio)], &[("LUFS Int", Data), ("LUFS Short", Data)])
                .clone(),
            new("oscilloscope", "Oscilloscope Waveform", "DSP - Analyse")
                .io(&[("Audio In", Audio)], &[("Scope Data", Data)])
                .clone(),
            new("goniometer", "Goniomètre / Corrélation", "DSP - Analyse")
                .io(&[("In L", Audio), ("In R", Audio)], &[("Phase Data", Data)])
                .clone(),

            // --- MODULATION & CONTRÔLE ---
            new("env_adsr", "Enveloppe ADSR", "Modulation")
                .io(
                    &[
                        ("Gate", Event), ("Attack", Control), ("Decay", Control),
                        ("Sustain", Control), ("Release", Control),
                    ],
                    &[("Env Out", Control)],
                )
                .events()
                .param("attack_ms", "Attack (ms)", 10.0, 1.0, 1000.0, 1.0)
                .param("decay_ms", "Decay (ms)", 100.0, 1.0, 2000.0, 1.0)
                .param("sustain", "Sustain", 0.7, 0.0, 1.0, 0.01)
                .param("release_ms", "Release (ms)", 200.0, 1.0, 4000.0, 1.0)
                .clone(),
            new("env_mseg", "Enveloppe MSEG Multi-pts", "Modulation")
                .io(&[("Trigger", Event), ("Rate", Control)], &[("Curve Out", Control)])
                .events()
                .clone(),
            new("lfo_mod", "LFO Multi-formes", "Modulation")
                .io(&[("Rate", Control), ("Sync", Control), ("Shape", Control)], &[("LFO Out", Control)])
                .param("rate_hz", "Rate (Hz)", 5.0, 0.1, 30.0, 0.1)
                .param("shape", "Shape", 0.0, 0.0, 3.0, 1.0)
                .clone(),
            new("sample_hold", "Sample & Hold / Random", "Modulation")
                .io(&[("Clock", Event)], &[("Random Out", Control)])
                .events()
                .clone(),
            new("env_follower", "Suiveur d'Enveloppe", "Modulation")
                .io(
                    &[("Audio In", Audio), ("Attack", Control), ("Release", Control)],
                    &[("Env Out", Control)],
                )
                .param("attack_ms", "Attack (ms)", 10.0, 1.0, 200.0, 1.0)
                .param("release_ms", "Release (ms)", 100.0, 5.0, 1000.0, 1.0)
                .clone(),

            // --- MATH, LOGIQUE & LISSAGE ---
            new("math_op", "Opérateur Math (Add/Mul)", "Math & Logique")
                .io(&[("A", Control), ("B", Control), ("Op", Control)], &[("Result", Control)])
                .param("op", "Op (add/sub/mul/div)", 0.0, 0.0, 3.0, 1.0)
                .clone(),
            new("scale_offset", "Scale & Offset", "Math & Logique")
                .io(&[("In", Control), ("Scale", Control), ("Offset", Control)], &[("Out", Control)])
                .param("scale", "Scale", 1.0, -10.0, 10.0, 0.01)
                .param("offset", "Offset", 0.0, -10.0, 10.0, 0.01)
                .clone(),
            new("clamp_value", "Clamp [Min, Max]", "Math & Logique")
                .io(&[("In", Control), ("Min", Control), ("Max", Control)], &[("Out", Control)])
                .param("min", "Min", 0.0, -10.0, 10.0, 0.01)
                .param("max", "Max", 1.0, -10.0, 10.0, 0.01)
                .clone(),
            new("logic_op", "Logique (AND/OR/NOT)", "Math & Logique")
                .io(&[("In A", Control), ("In B", Control), ("Op", Control)], &[("Bool Out", Control)])
                .param("op", "Op (and/or/not)", 0.0, 0.0, 2.0, 1.0)
                .clone(),
            new("math_expr", "Formule Math Custom", "Math & Logique")
                .io(&[("X", Control), ("Y", Control), ("Z", Control)], &[("Result", Control)])
                .clone(),
            new("smooth_slew", "Lisseur Anti-Zipper", "Math & Logique")
                .io(&[("In", Control), ("Time ms", Control)], &[("Out", Control)])
                .param("time_ms", "Time (ms)", 20.0, 1.0, 1000.0, 1.0)
                .clone(),
            new("quantizer", "Quantificateur de Gamme", "Math & Logique")
                .io(&[("Pitch In", Control), ("Scale Select", Control)], &[("Pitch Out", Control)])
                .param("scale_select", "Scale", 0.0, 0.0, 1.0, 1.0)
                .clone(),
        ];

        let mut by_type = BTreeMap::new();
        for (i, d) in defs.iter().enumerate() {
            by_type.insert(d.node_type.clone(), i);
        }
        Self { defs, by_type }
    }

    pub fn defs(&self) -> &[NodeDef] {
        &self.defs
    }

    pub fn get(&self, node_type: &str) -> Option<&NodeDef> {
        self.by_type.get(node_type).map(|&i| &self.defs[i])
    }

    pub fn categories(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self.defs.iter().map(|d| d.category.as_str()).collect();
        cats.dedup();
        cats
    }

    /// Paramètres par défaut d'une instance : registre écrasé par les overrides.
    pub fn resolved_params(
        &self,
        node_type: &str,
        overrides: &BTreeMap<String, f64>,
    ) -> BTreeMap<String, f64> {
        self.get(node_type)
            .map(|def| {
                def.params
                    .iter()
                    .map(|p| (p.param_id.clone(), p.default))
                    .chain(overrides.iter().map(|(k, v)| (k.clone(), *v)))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_maquette_nodes() {
        let reg = NodeRegistry::global();
        for t in [
            "clap_audio_in", "clap_audio_out", "clap_note_in", "clap_param",
            "osc_va", "filter_svf", "gain_node", "env_adsr", "delay_stereo",
            "peak_rms_meter", "math_op", "smooth_slew",
        ] {
            assert!(reg.get(t).is_some(), "node manquant: {t}");
        }
    }

    #[test]
    fn port_kinds_are_typed() {
        let svf = NodeRegistry::global().get("filter_svf").unwrap();
        assert_eq!(svf.inputs[0].kind, PortKind::Audio);
        assert_eq!(svf.inputs[1].kind, PortKind::Control);
        assert_eq!(svf.outputs.len(), 3);
    }

    #[test]
    fn categories_no_duplicates() {
        let mut cats = NodeRegistry::global().categories();
        let n = cats.len();
        cats.sort_unstable();
        cats.dedup();
        assert_eq!(cats.len(), n, "catégories dupliquées détectées");
    }
}
