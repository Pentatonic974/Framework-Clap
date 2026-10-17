//! Validation structurelle du graphe avant instanciation par le moteur ou la compilation.

use crate::node_registry::{NodeRegistry, PortKind};
use crate::spec::GraphSpec;
use std::collections::HashMap;

#[derive(Debug, Default, Clone)]
pub struct ValidationReport {
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    /// Ordre topologique (indices dans `spec.nodes`) si le graphe est valide.
    pub topo_order: Vec<usize>,
}

impl ValidationReport {
    pub fn is_ok(&self) -> bool {
        self.errors.is_empty()
    }
}

/// Valide le graphe : ids uniques, types connus, cohérence des ports
/// (kinds identiques, index valides), absence de cycles, ports audio
/// de sortie non connectés, paramètres hors bornes.
pub fn validate(spec: &GraphSpec) -> ValidationReport {
    let reg = NodeRegistry::global();
    let mut r = ValidationReport::default();

    // --- ids uniques ---
    let mut seen = HashMap::new();
    for (i, n) in spec.nodes.iter().enumerate() {
        if n.id.is_empty() {
            r.errors.push(format!("nœud #{i} : id vide"));
            continue;
        }
        if seen.insert(n.id.clone(), i).is_some() {
            r.errors.push(format!("id dupliqué : '{}'", n.id));
        }
        if reg.get(&n.node_type).is_none() {
            r.errors.push(format!("'{}' : type de nœud inconnu '{}'", n.id, n.node_type));
        }
        // Paramètres exposés hors bornes
        for p in &n.exposed_params {
            if p.default < p.min || p.default > p.max {
                r.errors.push(format!("'{}' : défaut '{}' hors bornes [{}, {}]", n.id, p.param_id, p.min, p.max));
            }
        }
        for (k, v) in &n.params {
            if let Some(def) = reg.get(&n.node_type).and_then(|d| d.params.iter().find(|p| &p.param_id == k)) {
                if *v < def.min || *v > def.max {
                    r.warnings.push(format!("'{}' : paramètre '{}' = {} hors [{}, {}]", n.id, k, v, def.min, def.max));
                }
            }
        }
    }

    // --- connexions ---
    for w in &spec.connections {
        let (from_ok, to_ok) = (spec.node(&w.from_node), spec.node(&w.to_node));
        let Some(from) = from_ok else {
            r.errors.push(format!("câble : nœud source inconnu '{}'", w.from_node));
            continue;
        };
        let Some(to) = to_ok else {
            r.errors.push(format!("câble : nœud cible inconnu '{}'", w.to_node));
            continue;
        };
        let (from_def, to_def) = (reg.get(&from.node_type), reg.get(&to.node_type));
        let (Some(fdef), Some(tdef)) = (from_def, to_def) else {
            continue; // déjà signalé
        };
        if w.from_port >= fdef.outputs.len() {
            r.errors.push(format!(
                "câble {}:{} → {}:{} : port de sortie #{} inexistant sur {} ({} sorties)",
                w.from_node, w.from_port, w.to_node, w.to_port, w.from_port, fdef.node_type, fdef.outputs.len()
            ));
            continue;
        }
        if w.to_port >= tdef.inputs.len() {
            r.errors.push(format!(
                "câble {}:{} → {}:{} : port d'entrée #{} inexistant sur {} ({} entrées)",
                w.from_node, w.from_port, w.to_node, w.to_port, w.to_port, tdef.node_type, tdef.inputs.len()
            ));
            continue;
        }
        let fk = fdef.outputs[w.from_port].kind;
        let tk = tdef.inputs[w.to_port].kind;
        if fk != tk {
            r.warnings.push(format!(
                "câble {}:{} → {}:{} : types divergents ({:?} → {:?}), implicit cast inséré",
                w.from_node, w.from_port, w.to_node, w.to_port, fk, tk
            ));
        }
    }

    // --- cycles et ordre topologique ---
    if r.is_ok() {
        match topo_sort(spec) {
            Ok(order) => r.topo_order = order,
            Err(nodes) => {
                r.errors.push(format!("cycle détecté impliquant les nœuds : {nodes:?}"));
            }
        }
    }

    // --- port de sortie audio du plugin non alimenté ---
    let has_out = spec.nodes.iter().any(|n| n.node_type == "clap_audio_out");
    if !has_out {
        r.warnings.push("aucun nœud 'clap_audio_out' : le plugin n'émettra pas de signal".into());
    } else {
        let out_fed = spec.connections.iter().any(|w| {
            spec.node(&w.to_node).map(|n| n.node_type == "clap_audio_out").unwrap_or(false)
        });
        if !out_fed {
            r.warnings.push("le port d'entrée de 'clap_audio_out' n'est pas connecté".into());
        }
    }

    r
}

/// Tri topologique (Kahn). En cas de cycle, renvoie les responsables.
fn topo_sort(spec: &GraphSpec) -> Result<Vec<usize>, Vec<String>> {
    let idx: HashMap<&str, usize> = spec.nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let n = spec.nodes.len();
    let mut out_deg: Vec<usize> = vec![0; n];
    let mut in_edges: Vec<Vec<usize>> = vec![Vec::new(); n];

    for w in &spec.connections {
        if let (Some(&a), Some(&b)) = (idx.get(w.from_node.as_str()), idx.get(w.to_node.as_str())) {
            out_deg[a] += 1;
            in_edges[b].push(a);
        }
    }

    let mut queue: Vec<usize> = (0..n).filter(|&i| out_deg[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    let mut consumed = out_deg.clone();

    // Kahn inversé : les sources qui alimentent les autres.
    while let Some(v) = queue.pop() {
        order.push(v);
        for &u in &in_edges[v] {
            consumed[u] -= 1;
            if consumed[u] == 0 {
                queue.push(u);
            }
        }
    }

    if order.len() == n {
        Ok(order)
    } else {
        let cyclic: Vec<String> = (0..n)
            .filter(|i| !order.contains(i))
            .map(|i| spec.nodes[i].id.clone())
            .collect();
        Err(cyclic)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::*;
    use std::collections::BTreeMap;

    fn node(id: &str, t: &str) -> NodeInstance {
        NodeInstance { id: id.into(), node_type: t.into(), x: 0.0, y: 0.0, params: BTreeMap::new(), exposed_params: Vec::new() }
    }

    fn in_path() -> GraphSpec {
        GraphSpec {
            meta: GraphMeta::default(),
            nodes: vec![node("n1", "clap_audio_in"), node("g1", "gain_node"), node("n2", "clap_audio_out")],
            connections: vec![
                Wire { from_node: "n1".into(), from_port: 0, to_node: "g1".into(), to_port: 0 },
                Wire { from_node: "g1".into(), from_port: 0, to_node: "n2".into(), to_port: 0 },
            ],
            gui_widgets: Vec::new(),
        }
    }

    #[test]
    fn valid_graph_passes() {
        let r = validate(&in_path());
        assert!(r.is_ok(), "{:?}", r.errors);
        assert_eq!(r.topo_order, vec![2, 1, 0]); // out <- gain <- in
    }

    #[test]
    fn unknown_type_is_error() {
        let mut g = in_path();
        g.nodes.push(node("bad", "pas_un_noeud"));
        let r = validate(&g);
        assert!(!r.is_ok());
    }

    #[test]
    fn duplicate_id_is_error() {
        let mut g = in_path();
        g.nodes.push(node("n1", "gain_node"));
        let r = validate(&g);
        assert!(r.errors.iter().any(|e| e.contains("dupliqué")));
    }

    #[test]
    fn mismatched_port_is_reported() {
        // SVF "Cutoff" (control 1) alimenté par une sortie audio
        let mut g = in_path();
        g.nodes.push(node("f", "filter_svf"));
        g.connections.push(Wire { from_node: "n1".into(), from_port: 0, to_node: "f".into(), to_port: 1 });
        let r = validate(&g);
        assert!(r.warnings.iter().any(|w| w.contains("types divergents")));
        assert!(r.is_ok());
    }

    #[test]
    fn cycle_detected() {
        let g = GraphSpec {
            meta: GraphMeta::default(),
            nodes: vec![node("a", "gain_node"), node("b", "gain_node")],
            connections: vec![
                Wire { from_node: "a".into(), from_port: 0, to_node: "b".into(), to_port: 0 },
                Wire { from_node: "b".into(), from_port: 0, to_node: "a".into(), to_port: 0 },
            ],
            gui_widgets: Vec::new(),
        };
        let r = validate(&g);
        assert!(r.errors.iter().any(|e| e.contains("cycle")));
    }
}
