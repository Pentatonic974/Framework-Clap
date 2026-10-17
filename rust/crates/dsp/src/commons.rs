//! Contrats communs aux nœuds DSP : contexte de rendu, typage des ports,
//! bus d'analyse lock-free (mètres + oscilloscope) et événements CLAP.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicI64, AtomicU32, Ordering};

/// Paramètres d'instanciation d'un nœud (issus du registre + overrides de l'instance).
#[derive(Debug, Clone, Default)]
pub struct NodeParams(pub BTreeMap<String, f64>);

impl NodeParams {
    #[inline]
    pub fn get(&self, key: &str, default: f32) -> f32 {
        self.0
            .get(key)
            .copied()
            .map(|v| v as f32)
            .unwrap_or(default)
    }
}

/// Contexte immuable passé à chaque bloc de rendu.
#[derive(Debug, Clone, Copy)]
pub struct ProcessCtx {
    pub sample_rate: f32,
    pub frames: usize,
}

/// Événement CLAP normalisé (notes/gate uniquement en v1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DspEvent {
    /// Note ON/OFF avec vélocité 0..1
    Note { pitch: f32, velocity: f32, on: bool },
}

/// Entrées d'un nœud.
pub struct NodeInputs<'a> {
    /// Audio : une tranche par port d'entrée audio.
    pub audio: &'a [&'a [f32]],
    /// Contrôle : une tranche par port de contrôle, longueur = frames
    /// (interpolation linéaire des atomics faite par le scheduler).
    pub controls: &'a [&'a [f32]],
    /// Événements CLAP ciblés pour ce nœud sur ce bloc.
    pub events: &'a [DspEvent],
}

impl<'a> NodeInputs<'a> {
    /// Valeur de contrôle à l'échantillon `i` (dégénéré à 0.0 si port absent).
    #[inline]
    pub fn ctl_at(&self, port: usize, i: usize) -> Option<f32> {
        self.controls.get(port).and_then(|s| s.get(i)).copied()
    }

    /// Valeur de contrôle constante (dernier échantillon du port).
    pub fn ctl(&self, port: usize) -> f32 {
        self.controls
            .get(port)
            .and_then(|s| s.last())
            .copied()
            .unwrap_or(0.0)
    }

    pub fn audio(&self, port: usize) -> &'a [f32] {
        self.audio.get(port).copied().unwrap_or(&[])
    }
}

/// Sorties d'un nœud : écriture audio différée via un `OutputWriter` (le moteur
/// implémente l'accès aux fils de câblage) + accès lecture au bus d'analyse.
pub struct NodeOutputs<'a> {
    writer: &'a mut dyn OutputWriter,
    /// Bus d'analyse (métriques visibles depuis l'UI via atomics).
    pub analysis: &'a AnalysisBus,
}

/// Accès écriture aux fils de sortie, fourni par le moteur.
pub trait OutputWriter {
    /// Tranche mutable du port donné (vide si port absent).
    fn slice_mut(&mut self, port: usize) -> &mut [f32];
}

impl<'a> NodeOutputs<'a> {
    /// Constructeur réservé au moteur (le champ `writer` est privé).
    pub fn new(writer: &'a mut dyn OutputWriter, analysis: &'a AnalysisBus) -> Self {
        Self { writer, analysis }
    }

    /// Tranche de sortie du port donné (vide si port absent).
    #[inline]
    pub fn out(&mut self, port: usize) -> &mut [f32] {
        self.writer.slice_mut(port)
    }
}

/// Cellule métrique lock-free : f32 packagé en AtomicU32 (bits).
/// Écriture : thread audio uniquement. Lecture : UI, sans attendre.
#[repr(transparent)]
pub struct RtMeterCell(pub AtomicU32);

impl RtMeterCell {
    pub const fn new() -> Self {
        Self(AtomicU32::new(0))
    }

    /// RT-safe : store release, jamais bloquant.
    pub fn store(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Release);
    }

    /// Lecture UI (never blocks).
    pub fn load(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Acquire))
    }
}

/// Anneau d'oscilloscope à seqlock : écriture exclusivement sur le thread audio,
/// lecture ré-essayée côté UI (aucun lock). `UnsafeCell` synchronisé par le
/// protocole seqlock, justifiant l'implémentation manuelle de `Sync`.
pub struct ScopeRing {
    buf: std::cell::UnsafeCell<Vec<f32>>, // préalloué hors RT, jamais poussé
    cursor: AtomicU32,
    seq: AtomicI64,
}

// SAFETY : une seule écriture à la fois (thread audio unique vivant dans
// GraphRuntime) ; les lecteurs revérifient la version seqlock après copie.
unsafe impl Send for ScopeRing {}
unsafe impl Sync for ScopeRing {}

impl ScopeRing {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: std::cell::UnsafeCell::new(vec![0.0; capacity]), // hors RT
            cursor: AtomicU32::new(0),
            seq: AtomicI64::new(0),
        }
    }

    /// RT-safe : écrit un bloc (modulo de capacité).
    pub fn write(&self, data: &[f32]) {
        self.seq.fetch_add(1, Ordering::AcqRel); // version impaire = écriture
        let cap = unsafe { (*self.buf.get()).len() };
        if cap == 0 || data.is_empty() {
            self.seq.fetch_add(1, Ordering::AcqRel); // version paire
            return;
        }
        let mut pos = self.cursor.load(Ordering::Relaxed) as usize % cap;
        unsafe {
            let buf = &mut *self.buf.get();
            for &s in data {
                buf[pos] = s;
                pos = (pos + 1) % cap;
            }
        }
        self.cursor.store(pos as u32, Ordering::Relaxed);
        self.seq.fetch_add(1, Ordering::AcqRel); // version paire = stable
    }

    /// Copie linéarisée pour l'UI (retry si écriture concurrente).
    pub fn snapshot(&self) -> Vec<f32> {
        loop {
            let v0 = self.seq.load(Ordering::Acquire);
            if v0 % 2 == 0 {
                let out = unsafe { (*self.buf.get()).clone() }; // lecture hors audio
                if self.seq.load(Ordering::Acquire) == v0 {
                    return out;
                }
            }
            std::hint::spin_loop();
        }
    }
}

/// Bus d'analyse partagé entre les nœuds d'un graphe (préalloué hors-RT).
pub struct AnalysisBus {
    /// Une cellule par nœud ; les métriques multiples d'un nœud
    /// (peak/rms/lufs...) sont multiplexées avec un stride de 4.
    pub meters: Vec<RtMeterCell>,
    pub scopes: Vec<ScopeRing>,
}

impl AnalysisBus {
    pub fn new(n_nodes: usize) -> Self {
        Self {
            meters: (0..n_nodes * 4).map(|_| RtMeterCell::new()).collect(),
            scopes: Vec::new(),
        }
    }

    pub fn add_scope(&mut self, capacity: usize) -> usize {
        self.scopes.push(ScopeRing::new(capacity));
        self.scopes.len() - 1
    }
}

/// Configuration d'instanciation passée au nœud par le moteur (hors RT).
#[derive(Debug, Clone, Copy)]
pub struct NodeConfig {
    /// Index de la cellule mètres multiplexée (node_index * 4).
    pub meter_slot: usize,
    /// Index de l'anneau oscilloscope (usize::MAX si aucun).
    pub scope_index: usize,
}

impl NodeConfig {
    pub fn none() -> Self {
        Self { meter_slot: usize::MAX, scope_index: usize::MAX }
    }
}

/// Trait central : un nœud DSP du graphe compilé.
pub trait DspNode: Send {
    /// Réinitialise l'état (appelé hors-RT quand la topologie/rate change).
    fn reset(&mut self);

    /// Rendu d'un bloc. Doit respecter le contrat RT strict.
    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>);

    /// Déclare la capacité scope nécessaire (Some => bus alloué). Default : non.
    fn wants_scope(&self) -> Option<usize> {
        None
    }

    /// Nombre de cellules mètres utilisées (default 0, multiplexé x4).
    fn meter_slots(&self) -> usize {
        0
    }

    /// Affecte les slots du bus d'analyse (appelé hors RT par le moteur).
    fn configure(&mut self, _config: NodeConfig) {}
}

// ---------- helpers numériques ----------

#[inline]
pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

#[inline]
pub fn db_to_gain(db: f32) -> f32 {
    10f32.powf(db / 20.0)
}

#[inline]
pub fn gain_to_db(g: f32) -> f32 {
    20.0 * g.max(1e-8).log10()
}

#[inline]
pub fn midi_to_hz(n: f32) -> f32 {
    440.0 * 2f32.powf((n - 69.0) / 12.0)
}

/// Correction anti-alias polynomiale (PolyBLEP), `t` phase 0..1, `dt` incrément de phase.
#[inline]
pub fn polyblep(t: f32, dt: f32) -> f32 {
    if t < dt {
        let x = t / dt - 1.0;
        -x * x - 2.0 * x - 1.0
    } else if t > 1.0 - dt {
        let x = (t - 1.0) / dt + 1.0;
        x * x + 2.0 * x + 1.0
    } else {
        0.0
    }
}

/// Constante d'un lisseur un pôle : 50 % du chemin atteint en `time_ms`.
#[inline]
pub fn one_pole_coef(time_ms: f32, sample_rate: f32) -> f32 {
    if time_ms <= 0.0 {
        return 0.0;
    }
    (1.0 - (-1000.0 / (time_ms * sample_rate)).exp()).min(1.0)
}
