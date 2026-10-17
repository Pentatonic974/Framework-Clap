//! render-tool — rendu offline d'un graphe en WAV stéréo.
//!
//! Usage : `render-tool <graphe.json> <out.wav> [--input guitar|noise] [--seconds 10]`
//!
//! Simule un DAW minimal : compile le GraphSpec avec le GraphRuntime réel,
//! injecte une source de test (accords de guitare synthétisés), lance le
//! rendu bloc par bloc, écrit le résultat en PCM16 stéréo 44.1 kHz.
//! Sert à diagnostiquer un plugin AVANT de l'ouvrir dans REAPER.

use engine::runtime::GraphRuntime;
use graph_core::spec::GraphSpec;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

const SAMPLE_RATE: f32 = 44_100.0;
const BLOCK: usize = 512;

fn main() {
    let mut args = std::env::args().skip(1);
    let graph = args.next().expect("usage : render-tool <graphe.json> <out.wav> [--input guitar|noise] [--seconds N]");
    let out = args.next().expect("fichier de sortie manquant");
    let mut input = String::from("guitar");
    let mut seconds = 10.0f32;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--input" => input = args.next().unwrap_or_else(|| "guitar".into()),
            "--seconds" => seconds = args.next().and_then(|s| s.parse().ok()).unwrap_or(10.0),
            _ => {}
        }
    }

    let json = std::fs::read_to_string(&graph).expect("lecture graphe");
    let spec = GraphSpec::from_json(&json).expect("graphe invalide");

    let (mut rt, _ui) = GraphRuntime::compile(&spec, SAMPLE_RATE, BLOCK)
        .expect("compilation du runtime");

    let total = (seconds * SAMPLE_RATE) as usize;
    let mut out_l = vec![0.0f32; total];
    let mut out_r = vec![0.0f32; total];

    // Buffer d'entrée du « host ».
    let mut in_l = vec![0.0f32; total];
    let mut in_r = vec![0.0f32; total];
    match input.as_str() {
        "noise" => {
            let mut state = 0x2463_a547u32;
            for v in in_l.iter_mut() {
                state ^= state << 13; state ^= state >> 17; state ^= state << 5;
                *v = (state as f32 / u32::MAX as f32 - 0.5) * 0.4;
            }
            in_r.copy_from_slice(&in_l);
        }
        // Accords de guitare : Karplus-Strong simplifié (cordes pincées sur
        // Em puis C puis G, grave en faux strum toutes les 600 ms).
        _ => {
            let chords: [&[(f32, f32)]; 3] = [
                &[(82.41, 1.0), (123.47, 0.8), (164.81, 0.7), (246.94, 0.5)],
                &[(130.81, 1.0), (164.81, 0.8), (196.0, 0.7), (261.63, 0.5)],
                &[(98.0, 1.0), (146.83, 0.8), (196.0, 0.7), (392.0, 0.5)],
            ];
            let strum_period = (0.6 * SAMPLE_RATE) as usize;
            let mut rng = 12345u32;
            for i in 0..total {
                let mut sample = 0.0f32;
                for (ci, chord) in chords.iter().enumerate() {
                    for (ni, &(freq, amp)) in chord.iter().enumerate() {
                        // Chaque note re-déclencle au strum décalé de 12 ms.
                        let t0 = ci as usize * strum_period / 3 + ni * 512;
                        let local_t = i.saturating_sub(t0) as f64;
                        if local_t <= 0.0 {
                            continue;
                        }
                        let period = SAMPLE_RATE as f64 / freq as f64;
                        // Excitation impulsion -> décroissance de type corde.
                        let burst = if local_t < period { 1.0 } else { 0.0 };
                        let _ = rng; let _ = burst;
                        let decay = (-local_t / (2.2 * SAMPLE_RATE as f64)).exp();
                        let phase = 2.0 * std::f64::consts::PI * freq as f64 * local_t / SAMPLE_RATE as f64;
                        sample += (phase.sin() as f32) * 0.25 * amp * decay as f32
                            + (phase * 2.0).sin() as f32 * 0.08 * amp * decay as f32;
                    }
                }
                // Cycle entre les accords.
                let cycle = total as f64 / (3.0 * strum_period as f64);
                let _ = cycle;
                in_l[i] = sample * 0.5;
                in_r[i] = sample * 0.5;
            }
        }
    }

    // Rendu bloc par bloc avec le runtime réel.
    let mut cursor = 0usize;
    while cursor < total {
        let frames = BLOCK.min(total - cursor);
        let (_, l) = &in_l[cursor..cursor + frames].split_at(frames.min(frames));
        let _ = l;
        rt.render(
            &in_l[cursor..cursor + frames],
            &in_r[cursor..cursor + frames],
            &mut out_l[cursor..cursor + frames],
            &mut out_r[cursor..cursor + frames],
            frames.min(BLOCK),
            &[],
        );
        cursor += frames;
    }

    write_wav(&out, &out_l, &out_r, SAMPLE_RATE);
    println!("WAV écrit : {out} ({} s, stéréo 44.1 kHz, entrée '{input}')", seconds);
}

/// Écriture WAV PCM16 stéréo sans dépendance externe.
fn write_wav(path: &str, l: &[f32], r: &[f32], sr: f32) {
    assert_eq!(l.len(), r.len());
    let frames = l.len();
    let file = File::create(Path::new(path)).expect("création WAV");
    let mut w = BufWriter::new(file);
    let data_len = (frames * 4) as u32;
    w.write_all(b"RIFF").unwrap();
    w.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    w.write_all(b"WAVE").unwrap();
    w.write_all(b"fmt ").unwrap();
    w.write_all(&16u32.to_le_bytes()).unwrap();
    w.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    w.write_all(&2u16.to_le_bytes()).unwrap(); // stéréo
    w.write_all(&(sr as u32).to_le_bytes()).unwrap();
    w.write_all(&(sr as u32 * 4).to_le_bytes()).unwrap(); // byte rate
    w.write_all(&4u16.to_le_bytes()).unwrap(); // block align
    w.write_all(&16u16.to_le_bytes()).unwrap(); // bits
    w.write_all(b"data").unwrap();
    w.write_all(&data_len.to_le_bytes()).unwrap();
    for i in 0..frames {
        for v in [l[i], r[i]] {
            let clamped = v.clamp(-1.0, 1.0);
            w.write_all(&((clamped * 32767.0) as i16).to_le_bytes()).unwrap();
        }
    }
    w.flush().unwrap();
}
