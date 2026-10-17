//! Modèle de graphe sérialisable (`GraphSpec`) — contrat JSON unique.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Métadonnées du plugin produites.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphMeta {
    pub name: String,
    /// Identifiant de bundle unique, ex. `com.developer.customclap`
    pub id: String,
    pub version: String,
    #[serde(default)]
    pub vendor: String,
    #[serde(default)]
    pub url: String,
    /// `1x1x1` : CLAP place_at 1, pan_amount 1, ... (par défaut 1x1x1)
    #[serde(default = "default_clap_feature")]
    pub feature: String,
}

fn default_clap_feature() -> String {
    "audio-effect".into()
}

/// Définition d'un paramètre CLAP (`CLAP_EXT_PARAMS`) exposé par un nœud.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParamDef {
    /// Identifiant stable du paramètre, ex. `gain_db`
    pub param_id: String,
    pub name: String,
    pub default: f64,
    pub min: f64,
    pub max: f64,
    #[serde(default = "default_step")]
    pub step: f64,
}

fn default_step() -> f64 {
    0.001
}

impl Default for ParamDef {
    fn default() -> Self {
        Self {
            param_id: "param".into(),
            name: "Param".into(),
            default: 0.0,
            min: -1.0,
            max: 1.0,
            step: 0.001,
        }
    }
}

/// Instance d'un nœud posé sur le canvas nodal.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct NodeInstance {
    pub id: String,
    /// Type du registre, ex. `filter_svf`
    pub node_type: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    /// Paramètres explicites de l'instance (les autres prennent la valeur du registre)
    #[serde(default)]
    pub params: BTreeMap<String, f64>,
    /// Paramètres activés créant un `clap_param` modulable (subseteq de ParamDef)
    #[serde(default)]
    pub exposed_params: Vec<ParamDef>,
}

/// Connexion : port `from` → port `to`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Wire {
    pub from_node: String,
    pub from_port: usize,
    pub to_node: String,
    pub to_port: usize,
}

/// Widget GUI posé sur la grille du designer (`CLAP_EXT_GUI`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GuiWidget {
    pub id: String,
    /// Type du registre GUI, ex. `knob`
    pub widget_type: String,
    pub label: String,
    /// Identifiant du paramètre CLAP piloté ("" si non applicable).
    pub param_id: String,
    #[serde(default)]
    pub col: u32,
    #[serde(default)]
    pub row: u32,
    #[serde(default = "default_w")]
    pub col_span: u32,
    #[serde(default = "default_h")]
    pub row_span: u32,
}

fn default_w() -> u32 {
    2
}

fn default_h() -> u32 {
    2
}

/// Type de widget GUI du registre.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GuiWidgetType {
    Knob,
    Slider,
    Toggle,
    Dropdown,
    XyPad,
    PianoKeyboard,
    CurveEditor,
    VuMeter,
    ValueDisplay,
    WaveformDisplay,
    PanelGroup,
    TabContainer,
    TextLogo,
}

/// Enveloppe complète du projet, sérialisée en JSON par l'éditeur Flutter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GraphSpec {
    pub meta: GraphMeta,
    pub nodes: Vec<NodeInstance>,
    pub connections: Vec<Wire>,
    #[serde(default)]
    pub gui_widgets: Vec<GuiWidget>,
}

impl Default for GraphSpec {
    fn default() -> Self {
        Self {
            meta: GraphMeta {
                name: "MyClapPlugin".into(),
                id: "com.example.myclapplugin".into(),
                version: "1.0.0".into(),
                vendor: String::new(),
                url: String::new(),
                feature: "audio-effect".into(),
            },
            nodes: Vec::new(),
            connections: Vec::new(),
            gui_widgets: Vec::new(),
        }
    }
}

impl GraphSpec {
    /// Désérialisation à partir du JSON exporté par l'éditeur.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn node(&self, id: &str) -> Option<&NodeInstance> {
        self.nodes.iter().find(|n| n.id == id)
    }
}
