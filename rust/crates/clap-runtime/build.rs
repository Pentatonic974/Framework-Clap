use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=CLAPS_GRAPH_JSON");
    let out = PathBuf::from(env::var("OUT_DIR").unwrap());
    let src = env::var("CLAPS_GRAPH_JSON")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists());

    match src {
        Some(path) => {
            fs::copy(&path, out.join("graph.json")).expect("copie du graphe");
            println!("cargo:rerun-if-changed={}", path.display());
        }
        None => {
            // Graphe de démonstration : entrée -> gain -> sortie stéréo.
            let demo = r#"{
  "meta": { "name": "DemoGain", "id": "com.claps.demo-gain", "version": "1.0.0", "vendor": "", "url": "", "feature": "audio-effect" },
  "nodes": [
    { "id": "in", "nodeType": "clap_audio_in", "x": 0, "y": 0, "params": {}, "exposedParams": [] },
    { "id": "g", "nodeType": "gain_node", "x": 100, "y": 0, "params": {}, "exposedParams": [
        { "paramId": "gain_db", "name": "Gain (dB)", "default": 0.0, "min": -60.0, "max": 24.0, "step": 0.1 }
    ] },
    { "id": "out", "nodeType": "clap_audio_out", "x": 200, "y": 0, "params": {}, "exposedParams": [] }
  ],
  "connections": [
    { "fromNode": "in", "fromPort": 0, "toNode": "g", "toPort": 0 },
    { "fromNode": "g", "fromPort": 0, "toNode": "out", "toPort": 0 },
    { "fromNode": "in", "fromPort": 1, "toNode": "out", "toPort": 1 }
  ],
  "guiWidgets": [
    { "id": "w1", "widgetType": "knob", "label": "Gain (dB)", "paramId": "g/gain_db", "paramIdRef": "", "col": 0, "row": 0, "colSpan": 2, "rowSpan": 2 }
  ]
}"#;
            fs::write(out.join("graph.json"), demo).expect("écriture du graphe de démo");
        }
    }
}
