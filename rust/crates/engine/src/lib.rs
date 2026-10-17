//! engine — moteur temps réel du graphe DSP.
//!
//! Architecture lock-free imposée :
//! - `param_patch` : `AtomicF32` (bits) par paramètre, écrit par l'UI,
//!   lu par le thread audio ; le lissage (anti-zipper) est échantillon-par-échantillon ;
//! - `command_queue` : SPSC rtrb selon la sémantique producteur = UI,
//!   consommateur = moteur (notes/mutations demandées) ;
//! - `analysis` : atomics partagées (mètres) et anneaux séquencés (oscilloscope) ;
//! - `GraphRuntime::render_into` n'effectue **aucune allocation**, **aucun lock**
//!   **aucun syscall**.
//!
//! Le threading véritable est orchestré par le runtime CLAP (`clap-runtime`) :
//! `GraphRuntime` est pindéré à un seul thread audio (pas `Sync`) et l'UI
//! communique uniquement par le patch-params at queue déclarées.

pub mod runtime;

pub use runtime::{GraphRuntime, GraphCommand, UiLink, ParamBus, DEFAULT_BLOCK};
