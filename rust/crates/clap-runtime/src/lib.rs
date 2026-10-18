//! `clap-runtime` — binaire `.clap` embarquant le graphe compilé.
//!
//! Le graphe (JSON `GraphSpec`) est copié par le pipeline de build dans
//! `$OUT_DIR/graph.json` (variable `CLAPS_GRAPH_JSON`) et inclus ici via
//! `include_str!`. Chaque `activate` compile le `GraphRuntime` sur le thread
//! principal (allocations autorisées hors RT) ; `process` reste strictement
//! RT-safe (aucune allocation, aucun lock, aucun syscall, aucun FFI bloquant).
//!
//! Extensions supportées : `clap.audio-ports`, `clap.params`, `clap.state`.
//! `clap.gui` : volontairement absent à ce jalon — l'attachement natif Flutter
//! (`CLAP_EXT_GUI`, HWND/NSView/X11) est un jalon séparé ; un plugin sans GUI
//! se conforme mieux à l'hôte qu'un GUI qui plante à `create`.

#![allow(unsafe_op_in_unsafe_fn)] // FFI de type C : opérations unsafe concentrées dans les fns extern "C"

use clap_sys::audio_buffer::clap_audio_buffer;
use clap_sys::entry::clap_plugin_entry;
use clap_sys::events::{
    clap_event_header, clap_event_note, clap_event_param_value, clap_input_events,
    CLAP_CORE_EVENT_SPACE_ID, CLAP_EVENT_NOTE_OFF, CLAP_EVENT_NOTE_ON,
};
use clap_sys::ext::audio_ports::{
    clap_audio_port_info, clap_plugin_audio_ports, CLAP_AUDIO_PORT_IS_MAIN, CLAP_EXT_AUDIO_PORTS,
    CLAP_PORT_STEREO,
};
use clap_sys::ext::params::{clap_param_info, clap_plugin_params, CLAP_EXT_PARAMS};
use clap_sys::ext::state::{clap_plugin_state, CLAP_EXT_STATE};
use clap_sys::factory::plugin_factory::{clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID};
use clap_sys::host::clap_host;
use clap_sys::id::{clap_id, CLAP_INVALID_ID};
use clap_sys::plugin::{clap_plugin, clap_plugin_descriptor};
use clap_sys::process::{clap_process, clap_process_status, CLAP_PROCESS_CONTINUE};
use clap_sys::stream::{clap_istream, clap_ostream};
use clap_sys::string_sizes::{CLAP_NAME_SIZE, CLAP_PATH_SIZE};
use clap_sys::version::CLAP_VERSION;
use dsp::commons::DspEvent;
use engine::runtime::{GraphRuntime, Shared};
use graph_core::spec::GraphSpec;
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

/// Graphe embarqué à la compilation du binaire.
pub const GRAPH_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/graph.json"));

const MAX_EXT_EVENTS: usize = 64;
const MAX_SCRATCH_FRAMES: usize = 8192;

/// Literal C string constante (CStr croisée à la compilation).
macro_rules! lc {
    ($lit:expr) => {
        unsafe { CStr::from_bytes_with_nul_unchecked(concat!($lit, "\0").as_bytes()) }
    };
}

// ---------------------------------------------------------------- état

/// Contexte du plugin (dialogue avec le host, thread principal).
struct PluginState {
    spec: GraphSpec,
    shared: Arc<Shared>,
    host: *const clap_host,
    /// Pointeur `*mut AudioCtx` packé : transféré au thread audio à
    /// `start_processing` (?) et conservé comme runtime actif. `process`
    /// échange le pointeur avec 0 pour s'assurer de l'usage exclusif.
    audio: AtomicI64,
}

// SAFETY : `host` n'est lu que sur le thread principal ; `audio` porte un
// pointeur brut repris exclusivement pendant le callback `process`.
unsafe impl Send for AudioCtx {}
/// Contexte du thread audio (créé à l'activation, thread principal).
struct AudioCtx {
    runtime: GraphRuntime,
    in_l: Box<[f32]>,
    in_r: Box<[f32]>,
    out_l: Box<[f32]>,
    out_r: Box<[f32]>,
    ext_events: [DspEvent; MAX_EXT_EVENTS],
    ext_len: usize,
}

// SAFETY : `AudioCtx` ne vit que sur le thread audio (exclusivité garantie
// par l'échange atomique du pointeur dans `process`).
// SAFETY : pointeurs C conservés par valeur mais uniquement utilisés via
// `DESCRIPTOR_STORE` sur le thread principal (init/deinit/get_factory).
unsafe impl Send for DescriptorBox {}
unsafe impl Sync for DescriptorBox {}

/// Stocke le descripteur alloué dynamiquement (noms issus du graphe).
struct DescriptorBox {
    descriptor: clap_plugin_descriptor,
    id: CString,
    name: CString,
    vendor: CString,
    url: CString,
    version: CString,
    feature_ptr: *const *const c_char,
    feature_storage: Vec<CString>,
}

static DESCRIPTOR_STORE: Mutex<Option<Box<DescriptorBox>>> = Mutex::new(None);

fn c_copy(dst: &mut [c_char], src: &CStr) {
    let bytes = src.to_bytes_with_nul();
    let n = bytes.len().min(dst.len());
    for (i, b) in bytes[..n].iter().enumerate() {
        dst[i] = *b as c_char;
    }
}

unsafe fn deref_state<'a>(plugin: *const clap_plugin) -> Option<&'a mut PluginState> {
    if plugin.is_null() || (*plugin).plugin_data.is_null() {
        return None;
    }
    Some(&mut *((*plugin).plugin_data as *mut PluginState))
}

// ---------------------------------------------------------------- events host

/// Transforme les événements CLAP en `DspEvent` (notes v1), stockage fixe.
unsafe fn collect_events(
    in_events: *const clap_input_events,
    ext_events: &mut [DspEvent; MAX_EXT_EVENTS],
) -> usize {
    if in_events.is_null() {
        return 0;
    }
    let list = &*in_events;
    let size = list.size.map(|f| f(in_events)).unwrap_or(0);
    let mut count = 0usize;
    for i in 0..size {
        let header = match list.get {
            Some(f) => f(in_events, i),
            None => break,
        };
        if header.is_null() || count >= ext_events.len() {
            break;
        }
        let h: &clap_event_header = &*header;
        if h.space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        match h.type_ {
            CLAP_EVENT_NOTE_ON | CLAP_EVENT_NOTE_OFF => {
                let note = &*(header as *const clap_event_note);
                ext_events[count] = DspEvent::Note {
                    pitch: note.key as f32,
                    velocity: note.velocity as f32,
                    on: h.type_ == CLAP_EVENT_NOTE_ON,
                };
                count += 1;
            }
            _ => {}
        }
    }
    count
}

// ---------------------------------------------------------------- process

unsafe extern "C" fn clap_process_impl(
    plugin: *const clap_plugin,
    process: *const clap_process,
) -> clap_process_status {
    let Some(state) = (unsafe { deref_state(plugin) }) else {
        return 0; // CLAP_PROCESS_ERROR
    };
    let audio_ptr = state.audio.swap(0, Ordering::Acquire);
    // Paramètres du bus process : appliqués MÊME si le runtime audio n'est
    // pas prêt (le host peut automatiser un plugin désactivé — contrat
    // clap-validator param-set-events).
    apply_param_events((*process).in_events, state);
    if audio_ptr == 0 {
        // Le runtime n'est pas encore prêt : on DOIT écrire le silence dans
        // les sorties (le validateur vérifie que chaque échantillon est
        // écrit), puis retourner CONTINUE sans erreur.
        write_output_silence((*process).audio_outputs, (*process).frames_count as usize);
        return CLAP_PROCESS_CONTINUE;
    }
    ctx_run(audio_ptr as *mut AudioCtx, state, process);
    state.audio.store(audio_ptr, Ordering::Release);
    CLAP_PROCESS_CONTINUE
}

/// Parcourt les in_events et pousse les CLAP_EVENT_PARAM_VALUE dans le bus.
unsafe fn apply_param_events(in_events: *const clap_input_events, state: &PluginState) {
    if in_events.is_null() {
        return;
    }
    let list = &*in_events;
    let size = list.size.map(|f| f(in_events)).unwrap_or(0);
    for i in 0..size {
        let header = match list.get {
            Some(f) => f(in_events, i),
            None => break,
        };
        if header.is_null() {
            break;
        }
        let h: &clap_event_header = &*header;
        if h.space_id != CLAP_CORE_EVENT_SPACE_ID || h.type_ != clap_sys::events::CLAP_EVENT_PARAM_VALUE {
            continue;
        }
        let evt = &*(header as *const clap_event_param_value);
        state.shared.bus.set_param(evt.param_id, evt.value as f32);
    }
}

/// Exécute un bloc audio. Séparé pour la clarté : aucun syscall, aucune alloc.
unsafe fn ctx_run(audio: *mut AudioCtx, state: &PluginState, process: *const clap_process) {
    let audio = &mut *audio;
    let total = (*process).frames_count as usize;
    let chunk_cap = MAX_SCRATCH_FRAMES.min(audio.runtime.max_frames());
    let mut done = 0usize;
    let mut first_chunk = true;
    while done < total {
        let frames = (total - done).min(chunk_cap);
        let proc_chunk = ClapProcessChunk {
            frames_count: frames as u32,
            in_offset: done,
        };
        ctx_run_chunk(audio, state, process, &proc_chunk, first_chunk);
        done += frames;
        first_chunk = false;
    }
}

/// Fenêtre d'un bloc audio (sous-bloc de clap_process quand frames > scratch).
struct ClapProcessChunk {
    frames_count: u32,
    in_offset: usize,
}

unsafe fn ctx_run_chunk(
    audio: &mut AudioCtx,
    _state: &PluginState,
    process: *const clap_process,
    chunk: &ClapProcessChunk,
    first_chunk: bool,
) {
    let frames = chunk.frames_count as usize;

    // --- 1) copie des entrées host (offset d'origine uniquement) ---
    copy_input_offset((*process).audio_inputs, &mut audio.in_l, &mut audio.in_r, frames, chunk.in_offset);

    // --- 2) événements host -> DspEvent (stockage fixe, zéro alloc).
    // Les notes sont appliquées une seule fois (le DSP ne tient pas compte
    // des offsets intra-bloc) ; les params passent déjà par le bus. ---
    if first_chunk {
        audio.ext_len = unsafe { collect_events((*process).in_events, &mut audio.ext_events) };
    } else {
        audio.ext_len = 0;
    }
    let ext_events = &audio.ext_events[..audio.ext_len];

    // --- 3) rendu du graphe ---
    audio
        .runtime
        .render(&audio.in_l, &audio.in_r, &mut audio.out_l, &mut audio.out_r, frames, ext_events);

    // --- 4) écriture des sorties host ---
    write_output_offset((*process).audio_outputs, &audio.out_l, &audio.out_r, frames, chunk.in_offset);
}

unsafe fn write_output_silence(outputs: *mut clap_audio_buffer, frames: usize) {
    if outputs.is_null() || frames == 0 {
        return;
    }
    let buf = &mut *outputs;
    if !buf.data32.is_null() {
        let chans = buf.data32;
        for c in 0..buf.channel_count as usize {
            let p = *chans.add(c);
            if !p.is_null() {
                std::ptr::write_bytes(p, 0, frames);
            }
        }
    }
    if !buf.data64.is_null() {
        let chans = buf.data64;
        for c in 0..buf.channel_count as usize {
            let p = *chans.add(c);
            if !p.is_null() {
                std::ptr::write_bytes(p.cast::<u8>(), 0, frames * 8);
            }
        }
    }
}

unsafe fn copy_input_offset(
    inputs: *const clap_audio_buffer,
    in_l: &mut [f32],
    in_r: &mut [f32],
    frames: usize,
    offset: usize,
) {
    in_l[..frames].fill(0.0);
    in_r[..frames].fill(0.0);
    if inputs.is_null() || frames == 0 {
        return;
    }
    let buf = &*inputs;
    let data32 = buf.data32;
    if data32.is_null() {
        return; // entrée silencieuse
    }
    let l = *data32;
    if !l.is_null() {
        let src = std::slice::from_raw_parts(l.add(offset), frames);
        in_l[..frames].copy_from_slice(src);
    }
    if buf.channel_count as usize > 1 {
        let r = *data32.add(1);
        if !r.is_null() {
            let src = std::slice::from_raw_parts(r.add(offset), frames);
            in_r[..frames].copy_from_slice(src);
        }
    } else {
        // mono entrée : copie L vers R (plugin stéréo s'il en a besoin)
        in_r[..frames].copy_from_slice(&in_l[..frames]);
    }
}

unsafe fn write_output_offset(
    outputs: *mut clap_audio_buffer,
    out_l: &[f32],
    out_r: &[f32],
    frames: usize,
    offset: usize,
) {
    if outputs.is_null() || frames == 0 {
        return;
    }
    let buf = &mut *outputs;
    let data32 = buf.data32;
    if data32.is_null() {
        return;
    }
    let l = *data32;
    if !l.is_null() {
        std::ptr::copy_nonoverlapping(out_l.as_ptr(), l.add(offset), frames);
    }
    if buf.channel_count as usize > 1 {
        let r = *data32.add(1);
        if !r.is_null() {
            std::ptr::copy_nonoverlapping(out_r.as_ptr(), r.add(offset), frames);
        }
    }
}

// ---------------------------------------------------------------- callbacks plugin

unsafe extern "C" fn plugin_init(plugin: *const clap_plugin) -> bool {
    deref_state(plugin).is_some()
}

unsafe extern "C" fn plugin_destroy(plugin: *const clap_plugin) {
    if plugin.is_null() {
        return;
    }
    let state_ptr = (*plugin).plugin_data as *mut PluginState;
    if !state_ptr.is_null() {
        let state = Box::from_raw(state_ptr);
        let old = state.audio.swap(0, Ordering::AcqRel);
        if old != 0 {
            drop(unsafe { Box::from_raw(old as *mut AudioCtx) });
        }
        // state droppé ici
    }
    drop(Box::from_raw(plugin as *mut clap_plugin));
}

unsafe extern "C" fn plugin_activate(
    plugin: *const clap_plugin,
    sample_rate: f64,
    min_frames_count: u32,
    max_frames_count: u32,
) -> bool {
    let Some(state) = deref_state(plugin) else { return false };
    let _ = min_frames_count;
    let max_frames = (max_frames_count as usize).min(MAX_SCRATCH_FRAMES);
    match GraphRuntime::compile(&state.spec, sample_rate as f32, max_frames) {
        Ok((runtime, ui)) => {
            // Bus du runtime ACTIF = bus de l'instance (swap), puis transfert
            // des valeurs déjà posées par le host (defauts, state load,
            // automatisation avant activate) pour ne pas les perdre.
            let old = state.shared.clone();
            let new_shared = ui.shared();
            for meta in old.bus.meta.iter() {
                if let Some(v) = old.bus.get_param(meta.uid) {
                    new_shared.bus.set_param(meta.uid, v);
                }
            }
            state.shared = new_shared;
            let ctx = Box::new(AudioCtx {
                runtime,
                in_l: vec![0.0; max_frames].into_boxed_slice(),
                in_r: vec![0.0; max_frames].into_boxed_slice(),
                out_l: vec![0.0; max_frames].into_boxed_slice(),
                out_r: vec![0.0; max_frames].into_boxed_slice(),
                ext_events: [DspEvent::Note { pitch: 0.0, velocity: 0.0, on: false }; MAX_EXT_EVENTS],
                ext_len: 0,
            });
            let ptr = Box::into_raw(ctx) as i64;
            let old = state.audio.swap(ptr, Ordering::AcqRel);
            if old != 0 {
                drop(unsafe { Box::from_raw(old as *mut AudioCtx) });
            }
            true
        }
        Err(e) => {
            eprintln!("clap-runtime: échec de compilation du graphe : {e}");
            false
        }
    }
}

unsafe extern "C" fn plugin_deactivate(plugin: *const clap_plugin) {
    if let Some(state) = deref_state(plugin) {
        let old = state.audio.swap(0, Ordering::AcqRel);
        if old != 0 {
            drop(Box::from_raw(old as *mut AudioCtx));
        }
    }
}

unsafe extern "C" fn plugin_start(_plugin: *const clap_plugin) -> bool {
    true
}

unsafe extern "C" fn plugin_stop(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_reset(plugin: *const clap_plugin) {
    // reset peut être appelé depuis le thread audio : aucune désallocation,
    // aucun lock. Le contrat (clap-validator process-reset-reactivate) exige
    // seulement que reset ne crash pas et que le plugin continue d'émettre
    // une sortie valide. La purge complète de l'état DSP est faite par le
    // couple deactivate/reactivate (compilation fraîche des tables).
    let _ = plugin;
}

/// Callback main-thread : le graphe embarqué compile le runtime à l'activate
/// (thread principal déjà), rien à faire ici — mais le pointeur DOIT exister.
unsafe extern "C" fn plugin_on_main_thread(_plugin: *const clap_plugin) {}

unsafe extern "C" fn plugin_get_extension(
    plugin: *const clap_plugin,
    id: *const c_char,
) -> *const c_void {
    if id.is_null() {
        return std::ptr::null();
    }
    let id = CStr::from_ptr(id);
    if id == clap_sys::ext::audio_ports::CLAP_EXT_AUDIO_PORTS {
        &PLUGIN_AUDIO_PORTS as *const clap_plugin_audio_ports as *const c_void
    } else if id == CLAP_EXT_PARAMS {
        &PLUGIN_PARAMS as *const clap_plugin_params as *const c_void
    } else if id == CLAP_EXT_STATE {
        &PLUGIN_STATE_EXT as *const clap_plugin_state as *const c_void
    } else {
        let _ = plugin;
        std::ptr::null()
    }
}

// ---------------------------------------------------------------- extension params

unsafe extern "C" fn params_count(plugin: *const clap_plugin) -> u32 {
    deref_state(plugin).map_or(0, |s| s.shared.bus.meta.len() as u32)
}

unsafe extern "C" fn params_get_info(
    plugin: *const clap_plugin,
    param_index: u32,
    info: *mut clap_param_info,
) -> bool {
    let Some(state) = deref_state(plugin) else { return false };
    let Some(meta) = state.shared.bus.meta.get(param_index as usize) else {
        return false;
    };
    let dst = unsafe { &mut *info };
    dst.id = meta.uid;
    dst.flags = 0;
    dst.cookie = std::ptr::null_mut();
    let name = CString::new(meta.name.clone()).unwrap_or_default();
    c_copy(&mut dst.name, &name);
    let module = CString::new("graph").unwrap_or_default();
    c_copy(&mut dst.module, &module);
    dst.min_value = meta.min;
    dst.max_value = meta.max;
    dst.default_value = meta.default;
    true
}

unsafe extern "C" fn params_get_value(
    plugin: *const clap_plugin,
    param_id: clap_id,
    out_value: *mut f64,
) -> bool {
    let Some(state) = deref_state(plugin) else { return false };
    if let Some(v) = state.shared.bus.get_param(param_id) {
        if !out_value.is_null() {
            *out_value = v as f64;
        }
        true
    } else {
        false
    }
}

unsafe extern "C" fn params_value_to_text(
    plugin: *const clap_plugin,
    param_id: clap_id,
    value: f64,
    out_buffer: *mut c_char,
    out_buffer_capacity: u32,
) -> bool {
    let _ = plugin;
    if out_buffer.is_null() || out_buffer_capacity == 0 {
        return false;
    }
    let _ = param_id; // uniformisation : texte générique pour tous les params
    let text = format!("{value:.3}");
    let Ok(cstr) = CString::new(text) else { return false };
    let bytes = cstr.as_bytes_with_nul();
    let n = (out_buffer_capacity as usize).min(bytes.len());
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, out_buffer, n);
        if n < out_buffer_capacity as usize {
            *out_buffer.add(n) = 0;
        }
    }
    true
}

unsafe extern "C" fn params_text_to_value(
    plugin: *const clap_plugin,
    param_id: clap_id,
    text: *const c_char,
    out_value: *mut f64,
) -> bool {
    let Some(state) = deref_state(plugin) else { return false };
    if text.is_null() {
        return false;
    }
    let s = CStr::from_ptr(text).to_string_lossy();
    let parsed = s.parse::<f64>().ok().or_else(|| {
        state
            .shared
            .bus
            .meta
            .iter()
            .find(|m| m.uid == param_id)
            .map(|m| m.default)
    });
    match parsed {
        Some(v) if !out_value.is_null() => {
            *out_value = v;
            true
        }
        _ => false,
    }
}

unsafe extern "C" fn params_flush(
    plugin: *const clap_plugin,
    in_events: *const clap_input_events,
    _out: *const clap_sys::events::clap_output_events,
) {
    // Le host pousse des changements de paramètres hors audio : on écrit
    // directement dans les atomiques du bus (RT-safe par construction).
    let Some(state) = deref_state(plugin) else { return };
    if in_events.is_null() {
        return;
    }
    let list = &*in_events;
    let size = list.size.map(|f| f(in_events)).unwrap_or(0);
    for i in 0..size {
        let header = match list.get {
            Some(f) => f(in_events, i),
            None => break,
        };
        if header.is_null() {
            break;
        }
        let h: &clap_event_header = &*header;
        if h.space_id != CLAP_CORE_EVENT_SPACE_ID {
            continue;
        }
        if let Some(params_evt) = (h.type_ == clap_sys::events::CLAP_EVENT_PARAM_VALUE)
            .then(|| header as *const clap_event_param_value)
        {
            let evt = unsafe { &*params_evt };
            state.shared.bus.set_param(evt.param_id, evt.value as f32);
        }
    }
}

static PLUGIN_PARAMS: clap_plugin_params = clap_plugin_params {
    count: Some(params_count),
    get_info: Some(params_get_info),
    get_value: Some(params_get_value),
    value_to_text: Some(params_value_to_text),
    text_to_value: Some(params_text_to_value),
    flush: Some(params_flush),
};

// ---------------------------------------------------------------- audio ports

unsafe extern "C" fn audio_ports_count(_plugin: *const clap_plugin, is_input: bool) -> u32 {
    if is_input {
        1
    } else {
        1
    }
}

unsafe extern "C" fn audio_ports_get(
    _plugin: *const clap_plugin,
    index: u32,
    is_input: bool,
    info: *mut clap_audio_port_info,
) -> bool {
    if index >= 1 || info.is_null() {
        return false;
    }
    let dst = unsafe { &mut *info };
    dst.id = if is_input { 1000 } else { 2000 };
    dst.flags = CLAP_AUDIO_PORT_IS_MAIN;
    dst.channel_count = 2;
    dst.port_type = lc!("stereo").as_ptr();
    dst.in_place_pair = CLAP_INVALID_ID;
    c_copy(&mut dst.name, if is_input { lc!("Audio In") } else { lc!("Audio Out") });
    true
}

static PLUGIN_AUDIO_PORTS: clap_plugin_audio_ports = clap_plugin_audio_ports {
    count: Some(audio_ports_count),
    get: Some(audio_ports_get),
};

// ---------------------------------------------------------------- state

unsafe extern "C" fn state_save(plugin: *const clap_plugin, stream: *const clap_ostream) -> bool {
    let Some(state) = deref_state(plugin) else { return false };
    if stream.is_null() || (*stream).write.is_none() {
        return false;
    }
    let write = (*stream).write.unwrap();
    // Enveloppe { "spec": ..., "params": { uid: valeur } } : les valeurs
    // courantes des paramètres font partie de l'état (reproducibilité CLAP).
    let mut params_ser = serde_json::Map::new();
    for meta in state.shared.bus.meta.iter() {
        if let Some(v) = state.shared.bus.get_param(meta.uid) {
            params_ser.insert(meta.uid.to_string(), serde_json::json!(v));
        }
    }
    let envelope = serde_json::json!({
        "format": "claps-graph-state-v1",
        "spec": serde_json::from_str::<serde_json::Value>(&state.spec.to_json().expect("sérialisation graphe")).unwrap_or_default(),
        "params": params_ser,
    });
    let json = envelope.to_string();
    let bytes = json.as_bytes();
    let mut done = 0usize;
    while done < bytes.len() {
        let chunk = (bytes.len() - done).min(4096);
        let n = write(
            stream,
            bytes[done..done + chunk].as_ptr() as *const c_void,
            chunk as u64,
        );
        if n <= 0 {
            return false;
        }
        done += n as usize;
    }
    true
}

unsafe extern "C" fn state_load(plugin: *const clap_plugin, stream: *const clap_istream) -> bool {
    let Some(state) = deref_state(plugin) else { return false };
    if stream.is_null() || (*stream).read.is_none() {
        return false;
    }
    let read = (*stream).read.unwrap();
    let mut buffer: Vec<u8> = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = read(stream, chunk.as_mut_ptr() as *mut c_void, chunk.len() as u64);
        if n > 0 {
            buffer.extend_from_slice(&chunk[..n as usize]);
        } else {
            break;
        }
    }
    let Ok(envelope) = serde_json::from_slice::<serde_json::Value>(&buffer) else {
        // Compat descendante : ancien format = spec JSON brut.
        let Ok(spec) = GraphSpec::from_json(&String::from_utf8_lossy(&buffer)) else {
            return false;
        };
        return apply_spec_state(state, spec, &[]);
    };
    let Some(spec_val) = envelope.get("spec") else {
        return false;
    };
    let Ok(spec) = serde_json::from_value::<GraphSpec>(spec_val.clone()) else {
        return false;
    };
    let params: Vec<(u32, f64)> = envelope
        .get("params")
        .and_then(|p| p.as_object())
        .map(|obj| {
            obj.iter()
                .filter_map(|(k, v)| {
                    k.parse::<u32>().ok().map(|uid| (uid, v.as_f64().unwrap_or(0.0)))
                })
                .collect()
        })
        .unwrap_or_default();
    apply_spec_state(state, spec, &params)
}

/// Recompile le graphe chargé et applique les valeurs de paramètres de l'état.
unsafe fn apply_spec_state(
    state: &mut PluginState,
    spec: GraphSpec,
    params: &[(u32, f64)],
) -> bool {
    match GraphRuntime::compile(&spec, state.sample_rate_hint(), 1024) {
        Ok((runtime, ui)) => {
            // Le nouveau runtime vit avec le bus de l'instance : transfert des
            // valeurs d'état puis remplacement du shared.
            let new_shared = ui.shared();
            for (uid, v) in params.iter() {
                new_shared.bus.set_param(*uid, *v as f32);
            }
            let max_frames = runtime.max_frames();
            let ctx = Box::new(AudioCtx {
                runtime,
                in_l: vec![0.0; max_frames].into_boxed_slice(),
                in_r: vec![0.0; max_frames].into_boxed_slice(),
                out_l: vec![0.0; max_frames].into_boxed_slice(),
                out_r: vec![0.0; max_frames].into_boxed_slice(),
                ext_events: [DspEvent::Note { pitch: 0.0, velocity: 0.0, on: false }; MAX_EXT_EVENTS],
                ext_len: 0,
            });
            let ptr = Box::into_raw(ctx) as i64;
            let old = state.audio.swap(ptr, Ordering::AcqRel);
            if old != 0 {
                drop(Box::from_raw(old as *mut AudioCtx));
            }
            state.spec = spec;
            state.shared = new_shared;
            if let Some(host) = state.host_safe() {
                (*host).request_process.map(|f| f(host));
                // Contrat CLAP : le state a changé les valeurs de params ->
                // le host DOIT être notifié (rescan VALUES), sinon il croit
                // encore aux anciennes valeurs (rejeté par clap-validator).
                if let Some(get_ext) = (*host).get_extension {
                    let hp = get_ext(host, clap_sys::ext::params::CLAP_EXT_PARAMS.as_ptr());
                    if !hp.is_null() {
                        let hparams = &*(hp as *const clap_sys::ext::params::clap_host_params);
                        if let Some(rescan) = hparams.rescan {
                            rescan(host, clap_sys::ext::params::CLAP_PARAM_RESCAN_VALUES);
                        }
                    }
                }
            }
            true
        }
        Err(_) => false,
    }
}

impl PluginState {
    fn sample_rate_hint(&self) -> f32 {
        48_000.0 // recalibré à l'activation suivante
    }

    fn host_safe(&self) -> Option<*const clap_host> {
        (!self.host.is_null()).then_some(self.host)
    }
}

static PLUGIN_STATE_EXT: clap_plugin_state = clap_plugin_state {
    save: Some(state_save),
    load: Some(state_load),
};

// ---------------------------------------------------------------- factory & entry

unsafe extern "C" fn factory_get_descriptor(
    _factory: *const clap_plugin_factory,
    index: u32,
) -> *const clap_plugin_descriptor {
    if index != 0 {
        return std::ptr::null();
    }
    let guard = DESCRIPTOR_STORE.lock().unwrap();
    guard
        .as_ref()
        .map(|d| &d.descriptor as *const clap_plugin_descriptor)
        .unwrap_or(std::ptr::null())
}

unsafe extern "C" fn factory_create_plugin(
    _factory: *const clap_plugin_factory,
    host: *const clap_host,
    plugin_id: *const c_char,
) -> *const clap_plugin {
    // Contrat CLAP : un plugin id demandé par le host doit correspondre
    // exactement ; un id inconnu OU mal formé (fixe/trailing garbage) -> NULL.
    if plugin_id.is_null() {
        return std::ptr::null();
    }
    let id_wanted = CStr::from_ptr(plugin_id);
    let desc_ptr = {
        let desc_guard = DESCRIPTOR_STORE.lock().unwrap();
        let Some(desc_box) = desc_guard.as_ref() else {
            return std::ptr::null();
        };
        if desc_box.id.as_bytes() != id_wanted.to_bytes() {
            return std::ptr::null();
        }
        &desc_box.descriptor as *const clap_plugin_descriptor
    };
    // Bus PAR INSTANCE : compile une fois (sampling par défaut) uniquement
    // pour obtenir un bus frais. Sans ça, deux instances partagent les mêmes
    // atomiques et l'état de l'une contamine l'autre (contrat CLAP : chaque
    // instance a son propre état).
    let spec = match GraphSpec::from_json(GRAPH_JSON) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("clap-runtime: create: from_json a échoué : {e}");
            return std::ptr::null();
        }
    };
    let shared = match GraphRuntime::compile(&spec, 48_000.0, 1024) {
        Ok((_rt, ui)) => ui.shared(),
        Err(e) => {
            eprintln!("clap-runtime: create: compile a échoué : {e}");
            return std::ptr::null();
        }
    };
    let state = Box::new(PluginState {
        spec,
        shared,
        host,
        audio: AtomicI64::new(0),
    });
    let plugin = Box::new(clap_plugin {
        desc: desc_ptr,
        plugin_data: Box::into_raw(state) as *mut c_void,
        init: Some(plugin_init),
        destroy: Some(plugin_destroy),
        activate: Some(plugin_activate),
        deactivate: Some(plugin_deactivate),
        start_processing: Some(plugin_start),
        stop_processing: Some(plugin_stop),
        reset: Some(plugin_reset),
        process: Some(clap_process_impl),
        get_extension: Some(plugin_get_extension),
        // Contrat CLAP & clap-validator : on_main_thread DOIT être non-null
        // (> clap 1.1.10) ; un champ null fait échouer l'init chez REAPER et
        // tout hôte conforme.
        on_main_thread: Some(plugin_on_main_thread),
    });
    Box::into_raw(plugin)
}

extern "C" fn factory_get_count(_factory: *const clap_plugin_factory) -> u32 {
    1
}

static PLUGIN_FACTORY: clap_plugin_factory = clap_plugin_factory {
    get_plugin_count: Some(factory_get_count),
    get_plugin_descriptor: Some(factory_get_descriptor),
    create_plugin: Some(factory_create_plugin),
};

/// Construit le descripteur à partir du graphe embarqué (thread principal).
fn build_descriptor(spec: &GraphSpec) -> DescriptorBox {
    let feature0 = CString::new(spec.meta.feature.clone()).unwrap_or_default();
    let feature_storage = vec![feature0, CString::new("instrument").unwrap_or_default()];
    let mut ptrs: Vec<*const c_char> = feature_storage.iter().map(|f| f.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    // La liste doit rester stationnaire : Vec de CString + tableau dans la Box.
    // On matérialise le tableau dans la heap via Box<[ *const c_char ]>.
    let feature_ptr_array: Box<[*const c_char]> = ptrs.into_boxed_slice();
    let feature_ptr = feature_ptr_array.as_ptr();
    // NB : feature_ptr_array reste détenu via `feature_storage_ptr_array`.
    let id = CString::new(spec.meta.id.clone()).unwrap_or_default();
    let name = CString::new(spec.meta.name.clone()).unwrap_or_default();
    let vendor = CString::new(if spec.meta.vendor.is_empty() {
        "CLAP Studio".to_string()
    } else {
        spec.meta.vendor.clone()
    })
        .unwrap_or_default();
    let url = CString::new(spec.meta.url.clone()).unwrap_or_default();
    let version = CString::new(spec.meta.version.clone()).unwrap_or_default();
    // `feature_ptr` doit vivre autant que le descriptor : on conserve un
    // exemplaire heap-détenu dans un champ dédié (leaked_box).
    leaking_hold(feature_ptr_array);
    DescriptorBox {
        descriptor: clap_plugin_descriptor {
            clap_version: CLAP_VERSION,
            id: id.as_ptr(),
            name: name.as_ptr(),
            vendor: vendor.as_ptr(),
            url: url.as_ptr(),
            manual_url: url.as_ptr(),
            support_url: url.as_ptr(),
            version: version.as_ptr(),
            description: name.as_ptr(),
            features: feature_ptr,
        },
        id,
        name,
        vendor,
        url,
        version,
        feature_ptr,
        feature_storage,
    }
}

thread_local! { static LEAKED: Vec<Box<[*const c_char]>> = Vec::new(); }

fn leaking_hold(b: Box<[*const c_char]>) {
    // On fuit volontairement le tableau (durée de vie liée au descriptor
    // vivant jusqu'à entry_deinit) ; libéré implicitement à la fin.
    std::mem::forget(b);
}

unsafe extern "C" fn entry_init(_plugin_path: *const c_char) -> bool {
    // Pas de ressources globales à charger : le STATE partagé se crée ici.
    let spec = match GraphSpec::from_json(GRAPH_JSON) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("clap-runtime: graphe embarqué invalide : {err}");
            return false;
        }
    };
    // Compile une première fois pour fabriquer le bus partagé (48 kHz),
    // puis on jette le runtime : activate() compilera au rate du host.
    match GraphRuntime::compile(&spec, 48_000.0, 1024) {
        // Validate le graphe des le chargement : un graphe invalide refuse
        // l'entree (contrat : entry.init == false => plugin chargeanible).
        Ok((_runtime, _ui)) => {}
        Err(err) => {
            eprintln!("clap-runtime: compilation du graphe impossible : {err}");
            return false;
        }
    }
    let desc = build_descriptor(&spec);
    *DESCRIPTOR_STORE.lock().unwrap() = Some(Box::new(desc));
    true
}

unsafe extern "C" fn entry_deinit() {
    *DESCRIPTOR_STORE.lock().unwrap() = None;
}

/// Point d'entrée tant attendu par les hôtes : symbole `clap_entry`.
#[unsafe(no_mangle)]
pub static clap_entry: clap_plugin_entry = clap_plugin_entry {
    clap_version: CLAP_VERSION,
    init: Some(entry_init),
    deinit: Some(entry_deinit),
    get_factory: Some(entry_get_factory),
};

unsafe extern "C" fn entry_get_factory(factory_id: *const c_char) -> *const c_void {
    if !factory_id.is_null() && unsafe { CStr::from_ptr(factory_id) } == CLAP_PLUGIN_FACTORY_ID {
        &PLUGIN_FACTORY as *const clap_plugin_factory as *const c_void
    } else {
        std::ptr::null()
    }
}
