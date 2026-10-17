/// Registre Dart des nœuds et widgets GUI — miroir du `NodeRegistry` Rust
/// (crate graph-core). Sert au palette de l'éditeur, à la validation locale
/// et au rendu des node cards. Les deux registres doivent rester isomorphes :
/// une modification ici engage la liste Rust de [rust/crates/graph-core].

/// Type de signal d'un port (doit correspondre à `PortKind` Rust).
library;

enum PortKind { audio, control, event, data }

class Port {
  final String name;
  final PortKind kind;
  const Port(this.name, this.kind);
}

class RegistryParam {
  final String paramId, name;
  final double defaultValue, min, max, step;
  const RegistryParam(this.paramId, this.name, this.defaultValue, this.min, this.max, this.step);
}

class NodeDef {
  final String nodeType, displayName, category;
  final List<Port> inputs, outputs;
  final List<RegistryParam> params;
  final bool handlesEvents;

  const NodeDef({
    required this.nodeType,
    required this.displayName,
    required this.category,
    required this.inputs,
    required this.outputs,
    this.params = const [],
    this.handlesEvents = false,
  });
}

const _a = PortKind.audio;
const _c = PortKind.control;
const _e = PortKind.event;
const _d = PortKind.data;

NodeDef _n(
  String type,
  String name,
  String cat, {
  List<Port> inputs = const [],
  List<Port> outputs = const [],
  List<RegistryParam> params = const [],
  bool events = false,
}) =>
    NodeDef(
      nodeType: type,
      displayName: name,
      category: cat,
      inputs: inputs,
      outputs: outputs,
      params: params,
      handlesEvents: events,
    );

RegistryParam _p(String paramId, String name, double def, double min, double max, double step) =>
    RegistryParam(paramId, name, def, min, max, step);

/// Registre complet.
class ClapNodeRegistry {
  static final List<NodeDef> defs = [
    // --- E/S & CLAP EXTENSIONS ---
    _n('clap_audio_in', 'Audio Input Port', 'E/S & CLAP API', outputs: [Port('Out L', _a), Port('Out R', _a)]),
    _n('clap_audio_out', 'Audio Output Port', 'E/S & CLAP API', inputs: [Port('In L', _a), Port('In R', _a)]),
    _n('clap_note_in', 'Note Input (MIDI/MPE)', 'E/S & CLAP API',
        outputs: [Port('Note', _e), Port('Gate', _e), Port('Velocity', _e), Port('Expression', _e)], events: true),
    _n('clap_note_out', 'Note Output Port', 'E/S & CLAP API',
        inputs: [Port('Note', _e), Port('Gate', _e), Port('Velocity', _e)]),
    _n('clap_param', 'CLAP Parameter', 'E/S & CLAP API',
        inputs: [Port('Mod Offset', _c)], outputs: [Port('Value Out', _c)],
        params: [_p('value', 'Value', 0.5, 0, 1, 0.001)]),
    _n('clap_voice_alloc', 'Voice Allocator (Poly)', 'E/S & CLAP API', inputs: [Port('Note In', _e)], events: true),
    _n('clap_transport', 'Transport Info (BPM)', 'E/S & CLAP API',
        outputs: [Port('BPM', _c), Port('IsPlaying', _c), Port('TimeSig', _c)]),
    _n('clap_track_info', 'Track Info Ext', 'E/S & CLAP API',
        outputs: [Port('Track Name', _d), Port('Color', _d)]),
    _n('clap_latency_tail', 'Latency & Tail Reporter', 'E/S & CLAP API',
        inputs: [Port('Latency ms', _c), Port('Tail ms', _c)]),

    // --- SOURCES & GÉNÉRATEURS DSP ---
    _n('osc_va', 'Oscillateur VA (Analog)', 'DSP - Sources',
        inputs: [Port('Pitch', _c), Port('PWM', _c), Port('Sync', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('waveform', 'Waveform', 0, 0, 2, 1), _p('detune', 'Detune (cents)', 0, -100, 100, 1)]),
    _n('osc_wavetable', 'Oscillateur Wavetable', 'DSP - Sources',
        inputs: [Port('Pitch', _c), Port('WT Position', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('table', 'Wavetable', 0, 0, 3, 1)]),
    _n('noise_gen', 'Générateur de Bruit', 'DSP - Sources',
        inputs: [Port('Color', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('level', 'Level', 0.2, 0, 1, 0.001)]),
    _n('sample_player', "Lecteur d'Échantillons", 'DSP - Sources',
        inputs: [Port('Trigger', _e), Port('Pitch', _c)], outputs: [Port('Out L', _a), Port('Out R', _a)], events: true),
    _n('additive_engine', 'Moteur Additif', 'DSP - Sources',
        inputs: [Port('Pitch', _c), Port('Harmonics', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('harmonics', 'Harmonics', 8, 1, 64, 1)]),

    // --- FILTRES & EQ ---
    _n('filter_svf', 'Filtre SVF (LP/HP/BP)', 'DSP - Filtres & EQ',
        inputs: [Port('Audio In', _a), Port('Cutoff', _c), Port('Reso', _c)],
        outputs: [Port('LP Out', _a), Port('HP Out', _a), Port('BP Out', _a)],
        params: [_p('mode', 'Mode (LP/HP/BP)', 0, 0, 2, 1), _p('cutoff_hz', 'Cutoff (Hz)', 1000, 20, 20000, 1), _p('resonance', 'Resonance', 0.7, 0.1, 10, 0.01)]),
    _n('filter_ladder', 'Filtre Ladder (24dB)', 'DSP - Filtres & EQ',
        inputs: [Port('Audio In', _a), Port('Cutoff', _c), Port('Reso', _c), Port('Drive', _c)],
        outputs: [Port('Audio Out', _a)],
        params: [_p('cutoff_hz', 'Cutoff (Hz)', 1000, 20, 20000, 1), _p('resonance', 'Resonance', 0, 0, 1, 0.01)]),
    _n('eq_parametric', 'Égaliseur Paramétrique', 'DSP - Filtres & EQ',
        inputs: [Port('Audio In', _a), Port('Freq', _c), Port('Q', _c), Port('Gain', _c)],
        outputs: [Port('Audio Out', _a)],
        params: [_p('freq_hz', 'Freq (Hz)', 1000, 20, 20000, 1), _p('q', 'Q', 1, 0.1, 10, 0.01), _p('gain_db', 'Gain (dB)', 0, -24, 24, 0.1)]),
    _n('filter_comb', 'Filtre Peigne (Comb)', 'DSP - Filtres & EQ',
        inputs: [Port('Audio In', _a), Port('Time', _c), Port('Feedback', _c)],
        outputs: [Port('Audio Out', _a)],
        params: [_p('time_ms', 'Time (ms)', 20, 0.1, 100, 0.1), _p('feedback', 'Feedback', 0.5, 0, 0.99, 0.01)]),
    _n('filter_formant', 'Filtre Formant Vocal', 'DSP - Filtres & EQ',
        inputs: [Port('Audio In', _a), Port('Vowel', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('vowel', 'Vowel (a-ou)', 0, 0, 1, 0.01)]),

    // --- DYNAMIQUE & AMPLITUDE ---
    _n('gain_node', 'Gain / Atténuateur', 'DSP - Dynamique',
        inputs: [Port('Audio In', _a), Port('Gain dB', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('gain_db', 'Gain (dB)', 0, -60, 24, 0.1)]),
    _n('compressor', 'Compresseur Stéréo', 'DSP - Dynamique',
        inputs: [Port('In L', _a), Port('In R', _a), Port('Threshold', _c), Port('Ratio', _c),
                 Port('Attack', _c), Port('Release', _c), Port('Sidechain', _a)],
        outputs: [Port('Out L', _a), Port('Out R', _a)],
        params: [_p('threshold_db', 'Threshold (dB)', -18, -60, 0, 0.1), _p('ratio', 'Ratio', 4, 1, 20, 0.1),
                 _p('attack_ms', 'Attack (ms)', 10, 0.1, 200, 0.1), _p('release_ms', 'Release (ms)', 100, 1, 2000, 1)]),
    _n('limiter', 'Limiteur de Crête', 'DSP - Dynamique',
        inputs: [Port('Audio In', _a), Port('Ceiling', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('ceiling_db', 'Ceiling (dB)', -1, -12, 0, 0.1)]),
    _n('noise_gate', 'Expander / Noise Gate', 'DSP - Dynamique',
        inputs: [Port('Audio In', _a), Port('Threshold', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('threshold_db', 'Threshold (dB)', -40, -80, -10, 0.1)]),
    _n('transient_shaper', 'Transient Shaper', 'DSP - Dynamique',
        inputs: [Port('Audio In', _a), Port('Attack', _c), Port('Sustain', _c)],
        outputs: [Port('Audio Out', _a)],
        params: [_p('attack', 'Attack', 0.5, 0, 2, 0.01), _p('sustain', 'Sustain', 0.5, 0, 2, 0.01)]),

    // --- EFFETS TEMPORELS & MODULATION ---
    _n('delay_stereo', 'Délai Stéréo / Tape', 'DSP - Effets',
        inputs: [Port('In L', _a), Port('In R', _a), Port('Time L', _c), Port('Time R', _c), Port('Feedback', _c)],
        outputs: [Port('Out L', _a), Port('Out R', _a)],
        params: [_p('time_ms', 'Time (ms)', 250, 1, 2000, 1), _p('feedback', 'Feedback', 0.35, 0, 0.95, 0.01), _p('mix', 'Mix', 0.5, 0, 1, 0.01)]),
    _n('reverb_algo', 'Réverb Algorithmique', 'DSP - Effets',
        inputs: [Port('In L', _a), Port('In R', _a), Port('Decay', _c), Port('Damp', _c), Port('Size', _c), Port('Mix', _c)],
        outputs: [Port('Out L', _a), Port('Out R', _a)],
        params: [_p('decay', 'Decay', 0.5, 0, 1, 0.01), _p('damp', 'Damp', 0.5, 0, 1, 0.01), _p('size', 'Size', 0.5, 0.1, 1, 0.01), _p('mix', 'Mix (wet/dry)', 0.35, 0, 1, 0.01)]),
    _n('reverb_convolution', 'Réverb Convolution', 'DSP - Effets',
        inputs: [Port('In L', _a), Port('In R', _a), Port('IR Select', _c)],
        outputs: [Port('Out L', _a), Port('Out R', _a)],
        params: [_p('ir_select', 'IR Preset', 0, 0, 3, 1)]),
    _n('chorus_flanger', 'Chorus / Flanger / Phaser', 'DSP - Effets',
        inputs: [Port('Audio In', _a), Port('Rate', _c), Port('Depth', _c), Port('Feedback', _c)],
        outputs: [Port('Out L', _a), Port('Out R', _a)],
        params: [_p('rate_hz', 'Rate (Hz)', 1, 0.05, 10, 0.01), _p('depth', 'Depth', 0.5, 0, 1, 0.01), _p('feedback', 'Feedback', 0.3, 0, 0.9, 0.01)]),
    _n('pitch_shifter', 'Pitch Shifter', 'DSP - Effets',
        inputs: [Port('Audio In', _a), Port('Semitones', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('semitones', 'Semitones', 0, -24, 24, 1)]),

    // --- SATURATION & DISTORSION ---
    _n('saturation_tape', 'Saturation Tube / Tape', 'DSP - Saturation',
        inputs: [Port('Audio In', _a), Port('Drive', _c), Port('Warmth', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('drive', 'Drive', 1.5, 1, 10, 0.1), _p('warmth', 'Warmth', 0.5, 0, 1, 0.01)]),
    _n('waveshaper', 'Waveshaper Courbe', 'DSP - Saturation',
        inputs: [Port('Audio In', _a), Port('Drive', _c), Port('Curve', _c)], outputs: [Port('Audio Out', _a)],
        params: [_p('drive', 'Drive', 2, 1, 20, 0.1), _p('curve', 'Curve', 0, 0, 3, 1)]),
    _n('bitcrusher', 'Bitcrusher / Downsampler', 'DSP - Saturation',
        inputs: [Port('Audio In', _a), Port('Bits', _c), Port('Downsample', _c)],
        outputs: [Port('Audio Out', _a)],
        params: [_p('bits', 'Bits', 8, 1, 16, 1), _p('downsample', 'Downsample (div)', 1, 1, 16, 1)]),

    // --- ANALYSE & MESURE ---
    _n('fft_analyzer', 'Analyseur Spectre FFT', 'DSP - Analyse',
        inputs: [Port('Audio In', _a)], outputs: [Port('FFT Data', _d)]),
    _n('peak_rms_meter', 'Mesureur Crête / RMS', 'DSP - Analyse',
        inputs: [Port('Audio In', _a)], outputs: [Port('Peak', _d), Port('RMS', _d)]),
    _n('lufs_meter', 'Mesureur Sonie LUFS', 'DSP - Analyse',
        inputs: [Port('In L', _a), Port('In R', _a)],
        outputs: [Port('LUFS Int', _d), Port('LUFS Short', _d)]),
    _n('oscilloscope', 'Oscilloscope Waveform', 'DSP - Analyse',
        inputs: [Port('Audio In', _a)], outputs: [Port('Scope Data', _d)]),
    _n('goniometer', 'Goniomètre / Corrélation', 'DSP - Analyse',
        inputs: [Port('In L', _a), Port('In R', _a)], outputs: [Port('Phase Data', _d)]),

    // --- MODULATION & CONTRÔLE ---
    _n('env_adsr', 'Enveloppe ADSR', 'Modulation',
        inputs: [Port('Gate', _e), Port('Attack', _c), Port('Decay', _c), Port('Sustain', _c), Port('Release', _c)],
        outputs: [Port('Env Out', _c)], events: true,
        params: [_p('attack_ms', 'Attack (ms)', 10, 1, 1000, 1), _p('decay_ms', 'Decay (ms)', 100, 1, 2000, 1),
                 _p('sustain', 'Sustain', 0.7, 0, 1, 0.01), _p('release_ms', 'Release (ms)', 200, 1, 4000, 1)]),
    _n('env_mseg', 'Enveloppe MSEG Multi-pts', 'Modulation',
        inputs: [Port('Trigger', _e), Port('Rate', _c)], outputs: [Port('Curve Out', _c)], events: true),
    _n('lfo_mod', 'LFO Multi-formes', 'Modulation',
        inputs: [Port('Rate', _c), Port('Sync', _c), Port('Shape', _c)], outputs: [Port('LFO Out', _c)],
        params: [_p('rate_hz', 'Rate (Hz)', 5, 0.1, 30, 0.1), _p('shape', 'Shape', 0, 0, 3, 1)]),
    _n('sample_hold', 'Sample & Hold / Random', 'Modulation',
        inputs: [Port('Clock', _e)], outputs: [Port('Random Out', _c)], events: true),
    _n('env_follower', "Suiveur d'Enveloppe", 'Modulation',
        inputs: [Port('Audio In', _a), Port('Attack', _c), Port('Release', _c)],
        outputs: [Port('Env Out', _c)],
        params: [_p('attack_ms', 'Attack (ms)', 10, 1, 200, 1), _p('release_ms', 'Release (ms)', 100, 5, 1000, 1)]),

    // --- MATH, LOGIQUE & LISSAGE ---
    _n('math_op', 'Opérateur Math (Add/Mul)', 'Math & Logique',
        inputs: [Port('A', _c), Port('B', _c), Port('Op', _c)], outputs: [Port('Result', _c)],
        params: [_p('op', 'Op (add/sub/mul/div)', 0, 0, 3, 1)]),
    _n('scale_offset', 'Scale & Offset', 'Math & Logique',
        inputs: [Port('In', _c), Port('Scale', _c), Port('Offset', _c)], outputs: [Port('Out', _c)],
        params: [_p('scale', 'Scale', 1, -10, 10, 0.01), _p('offset', 'Offset', 0, -10, 10, 0.01)]),
    _n('clamp_value', 'Clamp [Min, Max]', 'Math & Logique',
        inputs: [Port('In', _c), Port('Min', _c), Port('Max', _c)], outputs: [Port('Out', _c)],
        params: [_p('min', 'Min', 0, -10, 10, 0.01), _p('max', 'Max', 1, -10, 10, 0.01)]),
    _n('logic_op', 'Logique (AND/OR/NOT)', 'Math & Logique',
        inputs: [Port('In A', _c), Port('In B', _c), Port('Op', _c)], outputs: [Port('Bool Out', _c)],
        params: [_p('op', 'Op (and/or/not)', 0, 0, 2, 1)]),
    _n('math_expr', 'Formule Math Custom', 'Math & Logique',
        inputs: [Port('X', _c), Port('Y', _c), Port('Z', _c)], outputs: [Port('Result', _c)]),
    _n('smooth_slew', 'Lisseur Anti-Zipper', 'Math & Logique',
        inputs: [Port('In', _c), Port('Time ms', _c)], outputs: [Port('Out', _c)],
        params: [_p('time_ms', 'Time (ms)', 20, 1, 1000, 1)]),
    _n('quantizer', 'Quantificateur de Gamme', 'Math & Logique',
        inputs: [Port('Pitch In', _c), Port('Scale Select', _c)], outputs: [Port('Pitch Out', _c)],
        params: [_p('scale_select', 'Scale', 0, 0, 1, 1)]),
  ];

  static NodeDef? byType(String type) {
    for (final d in defs) {
      if (d.nodeType == type) return d;
    }
    return null;
  }

  static List<String> categories() {
    final cats = <String>[];
    for (final d in defs) {
      if (!cats.contains(d.category)) cats.add(d.category);
    }
    return cats;
  }
}

/// Registre des widgets GUI (miroir du GUI_REGISTRY de la maquette).
class GuiRegistry {
  static const List<(String, String, String)> widgets = [
    // (type, nom, catégorie)
    ('knob', 'Potentiomètre (Knob)', 'Contrôles GUI'),
    ('slider', 'Fader Linéaire', 'Contrôles GUI'),
    ('toggle', 'Bouton Switch On/Off', 'Contrôles GUI'),
    ('dropdown', 'Menu Déroulant', 'Contrôles GUI'),
    ('xy_pad', 'XY Pad 2D Vector', 'Contrôles GUI'),
    ('piano_keyboard', 'Clavier Virtuel Piano', 'Contrôles GUI'),
    ('curve_editor', 'Canvas Courbe / MSEG', 'Contrôles GUI'),
    ('vu_meter', 'Vumètre LED Bar / RMS', 'Visualiseurs GUI'),
    ('value_display', 'Afficheur Numérique', 'Visualiseurs GUI'),
    ('waveform_display', "Visu Forme d'Onde / Spectre", 'Visualiseurs GUI'),
    ('panel_group', 'Cadre Groupe / Section', 'Structure GUI'),
    ('tab_container', 'Conteneur Onglets (Tabs)', 'Structure GUI'),
    ('text_logo', 'Texte / Logo Branding', 'Structure GUI'),
  ];

  /// Types pilotables par un paramètre CLAP.
  static bool paramCapable(String type) {
    const controls = ['knob', 'slider', 'toggle', 'dropdown', 'xy_pad'];
    return controls.contains(type);
  }

  /// Types visualisateurs (lisent un flux d'analyse).
  static bool isViewer(String type) {
    return ['vu_meter', 'value_display', 'waveform_display'].contains(type);
  }
}
