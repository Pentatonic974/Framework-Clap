use dsp::commons::{
    AnalysisBus, DspEvent, DspNode, NodeConfig, NodeInputs, NodeOutputs, NodeParams, OutputWriter,
    ProcessCtx, one_pole_coef,
};
use graph_core::node_registry::{NodeRegistry, PortKind};
use graph_core::spec::GraphSpec;
use graph_core::validation::validate;
use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

pub const DEFAULT_BLOCK: usize = 64;
const MAX_EVENTS_PER_BLOCK: usize = 128;
const PARAM_SMOOTHING_MS: f32 = 1.0;
pub const SILENT_WIRE: usize = usize::MAX;

pub fn param_uid(node_id: &str, param_id: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for &b in format!("{node_id}/{param_id}").as_bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

#[derive(Debug, Clone)]
pub struct ParamMeta {
    pub uid: u32,
    pub node_id: String,
    pub param_id: String,
    pub name: String,
    pub default: f64,
    pub min: f64,
    pub max: f64,
}

pub struct ParamBus {
    cells: Vec<AtomicU32>,
    idx_by_uid: HashMap<u32, usize>,
    pub meta: Vec<ParamMeta>,
}

impl ParamBus {
    pub fn set_param(&self, uid: u32, value: f32) -> bool {
        match self.idx_by_uid.get(&uid) {
            Some(&i) => {
                self.cells[i].store(value.to_bits(), Ordering::Release);
                true
            }
            None => false,
        }
    }

    pub fn get_param(&self, uid: u32) -> Option<f32> {
        self.idx_by_uid
            .get(&uid)
            .map(|&i| f32::from_bits(self.cells[i].load(Ordering::Acquire)))
    }

    pub fn index_by_uid(&self, uid: u32) -> Option<usize> {
        self.idx_by_uid.get(&uid).copied()
    }

    pub fn cell_meta_by_index(&self, idx: usize) -> Option<&ParamMeta> {
        self.meta.get(idx)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GraphCommand {
    Note { pitch: f32, velocity: f32, on: bool },
}

pub struct Shared {
    pub bus: ParamBus,
    pub analysis: AnalysisBus,
}

/// Catile UI : écriture de paramètres + envoi de notes, sans jamais bloquer l'audio.
pub struct UiLink {
    pub shared: Arc<Shared>,
    commands: Producer<GraphCommand>,
}

impl UiLink {
    /// Clone du `Shared` (bus paramètres + bus analyse) pour les threads hors moteur.
    pub fn shared(&self) -> Arc<Shared> {
        self.shared.clone()
    }

    pub fn set_param(&self, node_id: &str, param_id: &str, value: f32) -> bool {
        self.shared.bus.set_param(param_uid(node_id, param_id), value)
    }

    pub fn get_param(&self, node_id: &str, param_id: &str) -> Option<f32> {
        self.shared.bus.get_param(param_uid(node_id, param_id))
    }

    pub fn read_meter(&self, node_index: usize, slot: usize) -> f32 {
        let i = node_index * 4 + slot;
        self.shared
            .analysis
            .meters
            .get(i)
            .map(|c| f32::from_bits(c.0.load(Ordering::Acquire)))
            .unwrap_or(0.0)
    }

    pub fn scope_snapshot(&self, scope_index: usize) -> Option<Vec<f32>> {
        self.shared
            .analysis
            .scopes
            .get(scope_index)
            .map(|s| s.snapshot())
    }

    pub fn note(&mut self, pitch: f32, velocity: f32, on: bool) -> bool {
        self.commands
            .push(GraphCommand::Note { pitch, velocity, on })
            .is_ok()
    }
}



/// Normalisation de nom pour la correspondance paramètre <-> port :
/// minuscules et caractères alphanumériques uniquement ("Gain dB" == "gain_db").
fn norm_name(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Mixeur précompilé : fusionne plusieurs câbles sources dans un câble destination.
#[derive(Clone)]
struct Mixer {
    dst_wire: usize,
    src_wires: Vec<usize>,
}

/// Paramètre lissé alimentant le câ de sortie d'un `clap_param`.
struct ParamEntry {
    atom_idx: usize,
    dst_wire: usize,
    smoothing_state: f32,
}

pub struct GraphRuntime {
    spec_json: String,
    sample_rate: f32,
    max_frames: usize,
    topo: Vec<usize>,
    nodes: Vec<Option<Box<dyn DspNode>>>,
    node_types: Vec<String>,              // node_type par index spec
    /// [node][port] -> wire (SILENT_WIRE si rien) ; aligné sur def.inputs.
    in_wires: Vec<Vec<usize>>,
    audio_out_wires: Vec<Vec<usize>>,
    mixers: Vec<Vec<Mixer>>,
    fanout: Vec<Vec<(usize, Vec<usize>)>>,
    event_nodes: Vec<usize>,
    in_taps: Vec<usize>,
    out_taps: Vec<usize>,
    wire_bufs: Vec<Vec<f32>>,
    param_entries: Vec<ParamEntry>,
    events_scratch: Vec<DspEvent>,
    input_scratch: Vec<f32>,
    max_input_ports: usize,
    shared: Arc<Shared>,
    consumer: Consumer<GraphCommand>,
}

struct WireWriter<'a> {
    wires: &'a [usize],
    bufs: &'a mut [Vec<f32>],
    frames: usize,
}

impl OutputWriter for WireWriter<'_> {
    fn slice_mut(&mut self, port: usize) -> &mut [f32] {
        match self.wires.get(port).copied() {
            Some(w) if w != SILENT_WIRE && w < self.bufs.len() => {
                &mut self.bufs[w][..self.frames]
            }
            _ => &mut [],
        }
    }
}

impl GraphRuntime {
    pub fn compile(
        spec: &GraphSpec,
        sample_rate: f32,
        max_frames: usize,
    ) -> Result<(Self, UiLink), String> {
        let report = validate(spec);
        if !report.is_ok() {
            return Err(report.errors.join("; "));
        }
        let reg = NodeRegistry::global();
        let spec_json = spec.to_json().map_err(|e| e.to_string())?;
        let topo = report.topo_order.clone();

        let id_to_idx: HashMap<&str, usize> = spec
            .nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), i))
            .collect();
        let n = spec.nodes.len();

        let mut all_out: Vec<Vec<Vec<usize>>> = vec![Vec::new(); n];
        let mut all_in: Vec<Vec<Vec<usize>>> = vec![Vec::new(); n];
        let mut wire_bufs: Vec<Vec<f32>> = Vec::with_capacity(spec.connections.len() + 8);
        for wire in &spec.connections {
            let fi = id_to_idx
                .get(wire.from_node.as_str())
                .copied()
                .ok_or_else(|| format!("câble orphelin source {}", wire.from_node))?;
            let ti = id_to_idx
                .get(wire.to_node.as_str())
                .copied()
                .ok_or_else(|| format!("câble orphelin cible {}", wire.to_node))?;
            let fdef = reg.get(&spec.nodes[fi].node_type).unwrap();
            let tdef = reg.get(&spec.nodes[ti].node_type).unwrap();
            if wire.from_port >= fdef.outputs.len() || wire.to_port >= tdef.inputs.len() {
                return Err("port hors bornes (validation incohérente)".into());
            }
            let wi = wire_bufs.len();
            wire_bufs.push(vec![0.0; max_frames]);
            while all_out[fi].len() <= wire.from_port {
                all_out[fi].push(Vec::new());
            }
            while all_in[ti].len() <= wire.to_port {
                all_in[ti].push(Vec::new());
            }
            match fdef.outputs[wire.from_port].kind {
                PortKind::Audio | PortKind::Data | PortKind::Control => {
                    all_out[fi][wire.from_port].push(wi)
                }
                PortKind::Event => {}
            }
            match tdef.inputs[wire.to_port].kind {
                PortKind::Audio | PortKind::Data | PortKind::Control => {
                    all_in[ti][wire.to_port].push(wi)
                }
                PortKind::Event => {}
            }
        }

        let mut mixers: Vec<Vec<Mixer>> = vec![Vec::new(); n];
        for ni in 0..n {
            for wires in all_in[ni].iter() {
                if wires.len() > 1 {
                    let wi = wire_bufs.len();
                    wire_bufs.push(vec![0.0; max_frames]);
                    mixers[ni].push(Mixer {
                        dst_wire: wi,
                        src_wires: wires.clone(),
                    });
                }
            }
        }
        let mut in_resolved: Vec<Vec<usize>> = vec![Vec::new(); n];
        for ni in 0..n {
            for wires in all_in[ni].iter() {
                let wire = if wires.len() > 1 {
                    mixers[ni]
                        .iter()
                        .position(|m| m.src_wires == *wires)
                        .and_then(|i| mixers[ni].get(i).map(|m| m.dst_wire))
                        .unwrap_or(wires[0])
                } else {
                    wires.first().copied().unwrap_or(SILENT_WIRE)
                };
                in_resolved[ni].push(wire);
            }
        }
        let mut in_wires: Vec<Vec<usize>> = vec![Vec::new(); n];
        for ni in 0..n {
            let def = reg.get(&spec.nodes[ni].node_type).unwrap();
            let mut row: Vec<usize> = in_resolved[ni].clone();
            // Ports sans câble du tout : complétion en SILENT_WIRE (alignement ports).
            while row.len() < def.inputs.len() {
                row.push(SILENT_WIRE);
            }
            in_wires[ni] = row;
            let _ = &spec;
        }
        let mut audio_out_wires: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut fanout: Vec<Vec<(usize, Vec<usize>)>> = vec![Vec::new(); n];
        for ni in 0..n {
            for (pi, wires) in all_out[ni].iter().enumerate() {
                match wires.first() {
                    Some(&w) => audio_out_wires[ni].push(w),
                    None => audio_out_wires[ni].push(SILENT_WIRE),
                }
                if wires.len() > 1 {
                    fanout[ni].push((pi, wires[1..].to_vec()));
                }
            }
        }

        let mut nodes: Vec<Option<Box<dyn DspNode>>> = Vec::with_capacity(n);
        let mut in_taps = Vec::new();
        let mut out_taps = Vec::new();
        let mut event_nodes = Vec::new();
        let mut param_entries: Vec<ParamEntry> = Vec::new();
        let mut param_meta: Vec<ParamMeta> = Vec::new();
        let mut cells: Vec<AtomicU32> = Vec::new();
        let mut atom_by_uid: HashMap<u32, usize> = HashMap::new();
        let mut max_input_ports = 2usize;

        for (ni, node) in spec.nodes.iter().enumerate() {
            let def = reg.get(&node.node_type).unwrap();
            max_input_ports = max_input_ports.max(def.inputs.len());
            let mut params = NodeParams::default();
            for p in &def.params {
                params.0.insert(p.param_id.clone(), p.default);
            }
            for (k, v) in &node.params {
                params.0.insert(k.clone(), *v);
            }

            // Params exposés de l'instance : chaque paramètre devient un CLAP param
            // avec un câble masqué lissé vers le port de contrôle correspondant.
            for pd in &node.exposed_params {
                let uid = param_uid(&node.id, &pd.param_id);
                param_meta.push(ParamMeta {
                    uid,
                    node_id: node.id.clone(),
                    param_id: pd.param_id.clone(),
                    name: pd.name.clone(),
                    default: pd.default,
                    min: pd.min,
                    max: pd.max,
                });
                let atom_idx = cells.len();
                cells.push(AtomicU32::new((pd.default as f32).to_bits()));
                atom_by_uid.insert(uid, atom_idx);
                if let Some((pi, _)) = def
                    .inputs
                    .iter()
                    .enumerate()
                    .find(|(_, p)| {
                        p.kind == PortKind::Control && norm_name(&p.name) == norm_name(&pd.param_id)
                    })
                {
                    while in_wires[ni].len() <= pi {
                        in_wires[ni].push(SILENT_WIRE);
                    }
                    if in_wires[ni][pi] == SILENT_WIRE {
                        let wi = wire_bufs.len();
                        wire_bufs.push(vec![0.0; max_frames]);
                        in_wires[ni][pi] = wi;
                        param_entries.push(ParamEntry {
                            atom_idx,
                            dst_wire: wi,
                            smoothing_state: pd.default as f32,
                        });
                    }
                }
            }

            if node.node_type == "clap_param" {
                let default = params.get("value", 0.5);
                let uid = param_uid(&node.id, "value");
                param_meta.push(ParamMeta {
                    uid,
                    node_id: node.id.clone(),
                    param_id: "value".into(),
                    name: node.id.clone(),
                    default: default as f64,
                    min: 0.0,
                    max: 1.0,
                });
                let atom_idx = cells.len();
                cells.push(AtomicU32::new((default as f32).to_bits()));
                atom_by_uid.insert(uid, atom_idx);
                if let Some(&wire) = audio_out_wires[ni].first() {
                    if wire != SILENT_WIRE {
                        param_entries.push(ParamEntry {
                            atom_idx,
                            dst_wire: wire,
                            smoothing_state: default as f32,
                        });
                    }
                }
                nodes.push(None);
                continue;
            }

            match dsp::create_node(&node.node_type, &params, sample_rate, max_frames) {
                Some(dsp) => nodes.push(Some(dsp)),
                None if node.node_type.starts_with("clap_") => nodes.push(None),
                None => {
                    return Err(format!(
                        "{} : type '{}' n'a pas encore d'implémentation DSP",
                        node.id, node.node_type
                    ))
                }
            }

            if def.handles_events {
                event_nodes.push(ni);
            }
            match node.node_type.as_str() {
                "clap_audio_in" => in_taps.push(ni),
                "clap_audio_out" => out_taps.push(ni),
                _ => {}
            }
        }

        let mut analysis = AnalysisBus::new(n);
        let mut scope_indices: Vec<usize> = nodes
            .iter()
            .map(|node| {
                node.as_deref()
                    .and_then(|d| d.wants_scope())
                    .map_or(usize::MAX, |_| usize::MAX)
            })
            .collect();
        for (ni, node) in nodes.iter().enumerate() {
            if let Some(cap) = node.as_deref().and_then(|d| d.wants_scope()) {
                let idx = analysis.add_scope(cap.max(1024));
                scope_indices[ni] = idx;
            }
        }
        for (ni, node_opt) in nodes.iter_mut().enumerate() {
            if let Some(dsp) = node_opt.as_deref_mut() {
                dsp.configure(NodeConfig {
                    meter_slot: ni,
                    scope_index: scope_indices[ni],
                });
            }
        }

        let bus = ParamBus { cells, idx_by_uid: atom_by_uid, meta: param_meta };
        let shared = Arc::new(Shared { bus, analysis });
        let (producer, consumer) = RingBuffer::new(1024);

        let runtime = GraphRuntime {
            spec_json,
            sample_rate,
            max_frames,
            topo,
            nodes,
            node_types: spec.nodes.iter().map(|node| node.node_type.clone()).collect(),
            in_wires,
            audio_out_wires,
            mixers,
            fanout,
            event_nodes,
            in_taps,
            out_taps,
            wire_bufs,
            param_entries,
            events_scratch: Vec::with_capacity(MAX_EVENTS_PER_BLOCK * 2 + 8),
            input_scratch: vec![0.0; (max_input_ports * 3).max(16) * max_frames.max(1)],
            max_input_ports,
            shared: shared.clone(),
            consumer,
        };
        let ui = UiLink { shared, commands: producer };
        Ok((runtime, ui))
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn max_frames(&self) -> usize {
        self.max_frames
    }

    pub fn spec_json(&self) -> &str {
        &self.spec_json
    }

    #[allow(clippy::too_many_arguments)]
    pub fn render(
        &mut self,
        host_l: &[f32],
        host_r: &[f32],
        out_l: &mut [f32],
        out_r: &mut [f32],
        frames: usize,
        external_events: &[DspEvent],
    ) {
        let mut frames = frames
            .min(self.max_frames)
            .min(out_l.len())
            .min(out_r.len())
            .max(1);
        frames = frames.min(out_l.len()).min(out_r.len());

        let GraphRuntime {
            topo,
            nodes,
            node_types,
            in_wires,
            audio_out_wires,
            wire_bufs,
            mixers,
            fanout,
            in_taps,
            out_taps,
            param_entries,
            events_scratch,
            input_scratch,
            consumer,
            shared,
            sample_rate,
            event_nodes,
            ..
        } = self;

        for w in wire_bufs.iter_mut() {
            w[..frames].fill(0.0);
        }

        if let Some(&si) = in_taps.first() {
            let outs = audio_out_wires[si].as_slice();
            if let Some(&w) = outs.first() {
                copy_host_into(wire_bufs, w, host_l, frames);
            }
            if let Some(&w) = outs.get(1) {
                copy_host_into(wire_bufs, w, host_r, frames);
            }
        }

        let coef = one_pole_coef(PARAM_SMOOTHING_MS, *sample_rate);
        {
            let cells = &shared.bus.cells;
            for p in param_entries.iter_mut() {
                let target = f32::from_bits(cells[p.atom_idx].load(Ordering::Relaxed));
                let buf = &mut wire_bufs[p.dst_wire][..frames];
                for s in buf.iter_mut() {
                    p.smoothing_state += (target - p.smoothing_state) * coef;
                    *s = p.smoothing_state;
                }
            }
        }

        events_scratch.clear();
        while let Ok(cmd) = consumer.pop() {
            if let GraphCommand::Note { pitch, velocity, on } = cmd {
                events_scratch.push(DspEvent::Note { pitch, velocity, on });
            }
            if events_scratch.len() >= MAX_EVENTS_PER_BLOCK {
                break;
            }
        }
        // Événements externes du host (clap in_events déjà normalisés par l'adapteur).
        for e in external_events.iter().copied() {
            events_scratch.push(e);
            if events_scratch.len() >= MAX_EVENTS_PER_BLOCK * 2 {
                break;
            }
        }
        let events: &[DspEvent] = events_scratch;

        for &ni in topo.iter() {
            // 5a) mixeurs d'entrée : somme disjointe via split_at_mut (zéro alloc)
            for m in mixers[ni].iter() {
                for &src in m.src_wires.iter() {
                    if src > m.dst_wire {
                        let (first, rest) = wire_bufs.split_at_mut(src);
                        let d = &mut first[m.dst_wire][..frames];
                        let s = &rest[0][..frames];
                        for (a, b) in d.iter_mut().zip(s) {
                            *a += *b;
                        }
                    } else {
                        let (first, rest) = wire_bufs.split_at_mut(m.dst_wire);
                        let s = &first[src][..frames];
                        let d = &mut rest[0][..frames];
                        for (a, b) in d.iter_mut().zip(s) {
                            *a += *b;
                        }
                    }
                }
            }

            let Some(dsp) = nodes[ni].as_mut() else {
                apply_fanout(fanout, &audio_out_wires[ni], wire_bufs, ni, frames);
                continue;
            };

            // 5b) copie des entrées -> scratch, ALIGNÉE PAR PORT du registre :
            // audio_slots[pi]/ctl_slots[pi] référencent le port `pi` exactement
            // (tranche vide pour un port d'une autre classe), aucune allocation.
            const EMPTY: &[f32] = &[];
            let n_ports = in_wires[ni].len().min(16);
            let mut audio_slots: [&[f32]; 16] = [EMPTY; 16];
            let mut ctl_slots: [&[f32]; 16] = [EMPTY; 16];
            let reg_local = NodeRegistry::global();
            let def = reg_local.get(&node_types[ni]).expect("type validé");
            let usable = (input_scratch.len() / frames.max(1)) * frames;
            let scratch = &mut input_scratch[..usable];
            for (chunk, pi) in scratch.chunks_mut(frames).zip(0..n_ports) {
                let wire = in_wires[ni][pi];
                if wire != SILENT_WIRE {
                    chunk.copy_from_slice(&wire_bufs[wire][..frames]);
                } else {
                    chunk.fill(0.0);
                }
                match def.inputs[pi].kind {
                    PortKind::Audio | PortKind::Data => audio_slots[pi] = chunk,
                    PortKind::Control | PortKind::Event => ctl_slots[pi] = chunk,
                }
            }

            let node_events: &[DspEvent] = if event_nodes.contains(&ni) { events } else { &[] };
            let inputs = NodeInputs {
                audio: &audio_slots[..n_ports],
                controls: &ctl_slots[..n_ports],
                events: node_events,
            };
            let ctx = ProcessCtx { sample_rate: *sample_rate, frames };
            {
                let mut writer = WireWriter {
                    wires: &audio_out_wires[ni],
                    bufs: wire_bufs,
                    frames,
                };
                let mut outputs = NodeOutputs::new(&mut writer, &shared.analysis);
                dsp.process(&ctx, &inputs, &mut outputs);
            }
            apply_fanout(fanout, &audio_out_wires[ni], wire_bufs, ni, frames);
        }

        if let Some(&si) = out_taps.first() {
            for (port, dst) in [(0usize, out_l), (1usize, out_r)] {
                if port >= in_wires[si].len() {
                    break;
                }
                let wire = in_wires[si][port];
                if wire == SILENT_WIRE {
                    dst[..frames].fill(0.0);
                } else {
                    dst[..frames].copy_from_slice(&wire_bufs[wire][..frames]);
                }
            }
        }
    }
}

fn copy_host_into(wire_bufs: &mut [Vec<f32>], wire: usize, src: &[f32], frames: usize) {
    if wire == SILENT_WIRE {
        return;
    }
    let buf_len = wire_bufs[wire].len().min(frames);
    let buf = &mut wire_bufs[wire][..frames];
    let n = src.len().min(buf_len);
    buf[..n].copy_from_slice(&src[..n]);
    buf[n..].fill(0.0);
}

fn apply_fanout(
    fanout: &[Vec<(usize, Vec<usize>)>],
    primaries: &[usize],
    wire_bufs: &mut [Vec<f32>],
    node: usize,
    frames: usize,
) {
    for (port, dupes) in fanout[node].iter() {
        let Some(&primary) = primaries.get(*port) else { continue };
        if primary == SILENT_WIRE {
            continue;
        }
        for &w in dupes.iter() {
            if w == SILENT_WIRE {
                continue;
            }
            // split_at_mut : borrows disjoints garantis par le compilateur.
            if w > primary {
                let (first, rest) = wire_bufs.split_at_mut(w);
                rest[0][..frames].copy_from_slice(&first[primary][..frames]);
            } else {
                let (first, rest) = wire_bufs.split_at_mut(primary);
                first[w][..frames].copy_from_slice(&rest[0][..frames]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use graph_core::spec::*;

    fn spec_with(nodes: Vec<(&str, &str)>, wires: Vec<(&str, usize, &str, usize)>) -> GraphSpec {
        GraphSpec {
            meta: GraphMeta::default(),
            nodes: nodes
                .into_iter()
                .map(|(id, t)| NodeInstance {
                    id: id.into(),
                    node_type: t.into(),
                    x: 0.0,
                    y: 0.0,
                    params: Default::default(),
                    exposed_params: Vec::new(),
                })
                .collect(),
            connections: wires
                .into_iter()
                .map(|(f, p, t, tp)| Wire {
                    from_node: f.into(),
                    from_port: p,
                    to_node: t.into(),
                    to_port: tp,
                })
                .collect(),
            gui_widgets: Vec::new(),
        }
    }

    #[test]
    fn passthrough_render() {
        let spec = spec_with(
            vec![("in", "clap_audio_in"), ("out", "clap_audio_out")],
            vec![("in", 0, "out", 0), ("in", 1, "out", 1)],
        );
        let (mut rt, _ui) = GraphRuntime::compile(&spec, 48_000.0, 512).unwrap();
        let mut ol = [0.0f32; 64];
        let mut or_ = [0.0f32; 64];
        let hl = [0.25f32; 64];
        let hr = [-0.25f32; 64];
        rt.render(&hl, &hr, &mut ol, &mut or_, 64, &[]);
        assert!((ol[63] - 0.25).abs() < 1e-6);
        assert!((or_[63] + 0.25).abs() < 1e-6);
    }

    #[test]
    fn gain_applies() {
        let mut spec = spec_with(
            vec![("in", "clap_audio_in"), ("g", "gain_node"), ("out", "clap_audio_out")],
            vec![("in", 0, "g", 0), ("g", 0, "out", 0), ("in", 1, "out", 1)],
        );
        // Le nœud expose son gain via un paramètre CLAP (câble masqué).
        spec.nodes[1].exposed_params.push(ParamDef {
            param_id: "gain_db".into(),
            name: "Gain (dB)".into(),
            default: 0.0,
            min: -60.0,
            max: 24.0,
            step: 0.1,
        });
        let (mut rt, mut ui) = GraphRuntime::compile(&spec, 48_000.0, 512).unwrap();
        ui.set_param("g", "gain_db", -20.0);
        let mut ol = [0.0f32; 512];
        let mut or_ = [0.0f32; 512];
        let hl = [1.0f32; 512];
        let hr = [0.0f32; 512];
        for _ in 0..4 {
            rt.render(&hl, &hr, &mut ol, &mut or_, 512, &[]);
        }
        assert!(ol[511] > 0.10 && ol[511] < 0.17, "gain lissé = {}", ol[511]);
    }
}
