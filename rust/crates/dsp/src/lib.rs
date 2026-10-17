//! dsp — nœuds DSP temps réel pour l'architecture CLAP Plugin Builder.
//!
//! Contrat RT (immuable) : chaque nœud est instancié en dehors du thread audio
//! (`factory::create_node`), et `process` doit être :
//! - sans allocation (aucun `Vec::push`, aucun `Box`) ;
//! - sans lock (aucun `Mutex`, `RwLock`, ni syscall) ;
//! - sans FFI bloquant ;
//! - déterministe vis-à-vis de l'état interne.
//!
//! Les paramètres glissent pendant le rendu via `NodeInputs::controls`
//! (le moteur interpole les atomiques échantillon par échantillon) ;
//! aucun nœud ne lit directement d'atomic.

pub mod commons;
pub mod analysers;
pub mod dynamics;
pub mod effects;
pub mod factory;
pub mod filters;
pub mod generators;
pub mod math_nodes;
pub mod rhythm;

pub use commons::{
    AnalysisBus, DspEvent, DspNode, NodeConfig, NodeInputs, NodeOutputs, NodeParams,
    OutputWriter, ProcessCtx,
};
pub use factory::create_node;
