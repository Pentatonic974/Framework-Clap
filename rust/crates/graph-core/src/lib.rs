//! graph-core — modèle de graphe sérialisable partagé entre l'éditeur Flutter
//! (via JSON), le moteur Rust et le pipeline de compilation des plugins CLAP.
//!
//! Ce crate est le contrat de sérialisation unique : `GraphSpec` est ce que
//! l'éditeur exporte, ce que `build-service` consomme et ce que le plugin CLAP
//! embarque. Tout est sérialisé en camelCase pour rester identique au schéma
//! de référence de la maquette (`pluginGraphState`).

pub mod node_registry;
pub mod spec;
pub mod validation;

pub use node_registry::{NodeDef, NodeRegistry, PortDef, PortKind, PORT_AUDIO, PORT_CONTROL, PORT_EVENT, PORT_DATA};
pub use spec::{GraphMeta, GraphSpec, GuiWidget, GuiWidgetType, NodeInstance, ParamDef, Wire};
pub use validation::{ValidationReport, validate};
