//! Effets temporels : délai stéréo, chorus/flanger, réverb algorithmique,
//! saturation / waveshaper / bitcrusher.

use super::commons::*;

/// Ligne de retard préallouée, réutilisée par plusieurs nœuds.
struct DelayLine {
    buf: Vec<f32>,
    pos: usize,
}

impl DelayLine {
    fn new(cap: usize) -> Self {
        Self {
            buf: vec![0.0; cap.max(16)],
            pos: 0,
        }
    }
}

/// Délai stéréo avec feedback. Ports : 0 In L, 1 In R, 2 Time L, 3 Time R, 4 Feedback.
/// Time en ms via control, feedback 0..0.95.
pub struct StereoDelay {
    time_ms: f32,
    feedback: f32,
    mix: f32,
    delay_l: DelayLine,
    delay_r: DelayLine,
}

impl StereoDelay {
    pub fn new(params: &NodeParams, sample_rate: f32) -> Self {
        let cap = (((2.2 / 1000.0) * sample_rate) as usize).max(32);
        Self {
            time_ms: params.get("time_ms", 250.0) as f32,
            feedback: params.get("feedback", 0.35) as f32,
            mix: params.get("mix", 0.5) as f32,
            delay_l: DelayLine::new(cap),
            delay_r: DelayLine::new(cap),
        }
    }
}

impl DspNode for StereoDelay {
    fn reset(&mut self) {
        self.delay_l.buf.iter_mut().for_each(|v| *v = 0.0);
        self.delay_r.buf.iter_mut().for_each(|v| *v = 0.0);
        self.delay_l.pos = 0;
        self.delay_r.pos = 0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let xl = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let xr = inputs.audio(1).get(i).copied().unwrap_or(0.0);
            let t_l = inputs.ctl_at(2, i).unwrap_or(self.time_ms).clamp(1.0, 2000.0);
            let t_r = inputs.ctl_at(3, i).unwrap_or(t_l).clamp(1.0, 2000.0);
            let fb = inputs.ctl_at(4, i).unwrap_or(self.feedback).clamp(0.0, 0.95);
            let mix = self.mix.clamp(0.0, 1.0);
            let dpl = self.read_l(&xl, t_l, ctx.sample_rate, fb);
            let dpr = self.read_r(&xr, t_r, ctx.sample_rate, fb, dpl);
            outputs.out(0)[i] = xl * (1.0 - mix * 0.5) + dpl * mix;
            outputs.out(1)[i] = xr * (1.0 - mix * 0.5) + dpr * mix;
        }
    }
}

impl StereoDelay {
    fn read_l(&mut self, x: &f32, t: f32, sr: f32, fb: f32) -> f32 {
        let cap = self.delay_l.buf.len();
        let d = ((t / 1000.0 * sr) as usize).min(cap - 1);
        let idx = (self.delay_l.pos + cap - d) % cap;
        let delayed = self.delay_l.buf[idx];
        self.delay_l.buf[self.delay_l.pos] = x + delayed * fb;
        self.delay_l.pos = (self.delay_l.pos + 1) % cap;
        delayed
    }

    fn read_r(&mut self, x: &f32, t: f32, sr: f32, fb: f32, ping_pong: f32) -> f32 {
        let cap = self.delay_r.buf.len();
        let d = ((t / 1000.0 * sr) as usize).min(cap - 1);
        let idx = (self.delay_r.pos + cap - d) % cap;
        let delayed = self.delay_r.buf[idx];
        // Ping-pong : la sortie L réalimente R.
        self.delay_r.buf[self.delay_r.pos] = x + ping_pong * 0.35 * fb + delayed * fb;
        self.delay_r.pos = (self.delay_r.pos + 1) % cap;
        delayed
    }
}

/// Chorus / flanger : modulation du temps de retard par un LFO interne.
/// Ports : 0 Audio In, 1 Rate Hz, 2 Depth, 3 Feedback.
pub struct ChorusFlanger {
    rate_hz: f32,
    phase: f32,
    buf: Vec<f32>,
    pos: usize,
    last: f32,
}

impl ChorusFlanger {
    pub fn new(params: &NodeParams, sample_rate: f32) -> Self {
        let cap = (((30.0 / 1000.0) * sample_rate) as usize).max(32); // 30 ms max
        Self {
            rate_hz: params.get("rate_hz", 1.0) as f32,
            phase: 0.0,
            buf: vec![0.0; cap],
            pos: 0,
            last: 0.0,
        }
    }
}

impl DspNode for ChorusFlanger {
    fn reset(&mut self) {
        self.buf.iter_mut().for_each(|v| *v = 0.0);
        self.pos = 0;
        self.phase = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let cap = self.buf.len();
        for i in 0..n {
            let x = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let rate = inputs.ctl_at(1, i).unwrap_or(self.rate_hz).clamp(0.05, 20.0);
            let depth = inputs.ctl_at(2, i).unwrap_or(0.5).clamp(0.0, 1.0);
            let fb = inputs.ctl_at(3, i).unwrap_or(0.3).clamp(0.0, 0.9);
            self.phase += rate / ctx.sample_rate;
            self.phase -= self.phase.floor();
            // Retard modulé 1..15 ms ; centre 8 ms.
            let center = cap as f32 * 0.4;
            let offset = (self.phase * std::f32::consts::TAU).sin() * center * depth;
            let d = ((cap as f32 * 0.5 + offset) as usize).min(cap - 1);
            let idx = (self.pos + cap - d) % cap;
            let delayed = self.buf[idx];
            self.last = x + delayed * fb;
            self.buf[self.pos] = self.last;
            self.pos = (self.pos + 1) % cap;
            outputs.out(0)[i] = x * 0.5 + delayed * 0.5;
            outputs.out(1)[i] = x * 0.5 + (delayed * Self::half_allpass(delayed)) * 0.5;
        }
    }
}

impl ChorusFlanger {
    fn half_allpass(v: f32) -> f32 {
        v - 0.15 * (v - v.abs())
    }
}

/// Réverb algorithmique Freeverb-like : 8 peignes + 4 allpass par canal.
/// Ports : 0 In L, 1 In R, 2 Decay, 3 Damp, 4 Size, 5 Mix (wet/dry) ;
/// sorties L/R = mix(wet, dry) — le mix wet/dry est calculé dans le nœud.
pub struct ReverbAlgo {
    tunable: [f32; 8],
    comb_buf_l: [Vec<f32>; 8],
    comb_buf_r: [Vec<f32>; 8],
    comb_pos: [usize; 8],
    comb_val_l: [f32; 8],
    comb_val_r: [f32; 8],
    ap_buf_l: [Vec<f32>; 4],
    ap_buf_r: [Vec<f32>; 4],
    ap_pos_l: [usize; 4],
    ap_pos_r: [usize; 4],
    size_scale: f32,
}

const STEMS: [usize; 8] = [1116, 1188, 1277, 1356, 1422, 1491, 1557, 1617];
const ALLPASS: [usize; 4] = [556, 441, 341, 225];

impl ReverbAlgo {
    pub fn new(_params: &NodeParams, sample_rate: f32) -> Self {
        let scale = sample_rate / 44_100.0;
        Self {
            tunable: [0.0; 8],
            comb_buf_l: STEMS.map(|s| vec![0.0; (s as f32 * scale) as usize]),
            comb_buf_r: STEMS.map(|s| vec![0.0; ((s + 23) as f32 * scale) as usize]),
            comb_pos: [0; 8],
            comb_val_l: [0.0; 8],
            comb_val_r: [0.0; 8],
            ap_buf_l: ALLPASS.map(|s| vec![0.0; (s as f32 * scale) as usize]),
            ap_buf_r: ALLPASS.map(|s| vec![0.0; ((s + 17) as f32 * scale) as usize]),
            // Positions SÉPARÉES L/R : les caps diffèrent (+17 échan.) — un
            // index partagé déborde le canal R (bug trouvé par render-tool).
            ap_pos_l: [0; 4],
            ap_pos_r: [0; 4],
            size_scale: scale,
        }
    }
}

impl DspNode for ReverbAlgo {
    fn reset(&mut self) {
        for b in self.comb_buf_l.iter_mut() {
            b.iter_mut().for_each(|x| *x = 0.0);
        }
        for b in self.comb_buf_r.iter_mut() {
            b.iter_mut().for_each(|x| *x = 0.0);
        }
        for b in self.ap_buf_l.iter_mut() {
            b.iter_mut().for_each(|x| *x = 0.0);
        }
        for b in self.ap_buf_r.iter_mut() {
            b.iter_mut().for_each(|x| *x = 0.0);
        }
        self.comb_pos = [0; 8];
        self.ap_pos_l = [0; 4];
        self.ap_pos_r = [0; 4];
        self.comb_val_l = [0.0; 8];
        self.comb_val_r = [0.0; 8];
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let l = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let r = inputs.audio(1).get(i).copied().unwrap_or(0.0);
            let decay = inputs.ctl_at(2, i).unwrap_or(0.5).clamp(0.0, 0.99);
            let damp = inputs.ctl_at(3, i).unwrap_or(0.5).clamp(0.0, 0.99);
            let size = inputs.ctl_at(4, i).unwrap_or(0.5).clamp(0.1, 1.0);
            let mix = inputs.ctl_at(5, i).unwrap_or(0.35).clamp(0.0, 1.0); // wet
            let _ = size; // la variation de taille nécessiterait une réallocation -> non in RT
            let room = 0.7 + 0.28 * decay;
            let damp_k = 0.15 + 0.35 * damp;
            let mut out_l = 0.0f32;
            let mut out_r = 0.0f32;
            // --- 8 peignes par canal ---
            for k in 0..8 {
                let cap_l = self.comb_buf_l[k].len();
                let cap_r = self.comb_buf_r[k].len();
                // Cache anti-trébuchet : seuil minimum de capacité.
                if cap_l < 8 || cap_r < 8 {
                    continue;
                }
                let idx_l = (self.comb_pos[k] + cap_l - 1) % cap_l;
                let idx_r = (self.comb_pos[k] + cap_r - 1) % cap_r;
                let dpl = self.comb_buf_l[k][idx_l];
                let dpr = self.comb_buf_r[k][idx_r];
                // Danger : les réseaux Freeverb utilisent un étage d'entrée commun.
                let mut lp = self.comb_val_l[k];
                lp = lp + damp_k * (dpl * room - lp);
                let mut rp = self.comb_val_r[k];
                rp = rp + damp_k * (dpr * room - rp);
                self.comb_val_l[k] = lp;
                self.comb_val_r[k] = rp;
                self.comb_buf_l[k][self.comb_pos[k]] = l + lp;
                self.comb_buf_r[k][self.comb_pos[k]] = r + rp;
                self.comb_pos[k] = (self.comb_pos[k] + 1) % cap_l;
                // NB : pos partagé L/R fonctionne car les caps diffèrent de 23 éch.
                let _ = cap_r;
                out_l += lp;
                out_r += rp;
            }
            // --- 4 allpass en série (positions L/R séparées) ---
            for k in 0..4 {
                out_l = Self::ap_process(&mut self.ap_buf_l[k], &mut self.ap_pos_l[k], out_l);
                out_r = Self::ap_process(&mut self.ap_buf_r[k], &mut self.ap_pos_r[k], out_r);
            }
            let _ = self.comb_val_l;
            let _ = self.comb_val_r;
            // Mix wet/dry : sortie = wet * mix + dry * (1 - mix). Le wet est
            // normalisé (le réseau Freeverb sort très chaud à 8 peignes).
            let wet_l = out_l * 0.25;
            let wet_r = out_r * 0.25;
            outputs.out(0)[i] = wet_l * mix + l * (1.0 - mix);
            outputs.out(1)[i] = wet_r * mix + r * (1.0 - mix);
        }
    }
}

impl ReverbAlgo {
    fn ap_process(buf: &mut [f32], pos: &mut usize, input: f32) -> f32 {
        let cap = buf.len();
        if cap == 0 {
            return input;
        }
        let bufout = buf[*pos];
        let output = -input + bufout;
        buf[*pos] = input + bufout * 0.5;
        *pos = (*pos + 1) % cap;
        output * 0.63212
    }
}

/// Bitcrusher : quantification en bits + rééchantillonnage décimation.
/// Ports : 0 Audio In, 1 Bits, 2 Downsample (div).
pub struct Bitcrusher {
    bits: f32,
    hold: f32,
    counter: usize,
}

impl Bitcrusher {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            bits: params.get("bits", 8.0) as f32,
            hold: 0.0,
            counter: 0,
        }
    }
}

impl DspNode for Bitcrusher {
    fn reset(&mut self) {
        self.hold = 0.0;
        self.counter = 0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let x = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let bits = inputs.ctl_at(1, i).unwrap_or(self.bits).clamp(1.0, 16.0);
            let div = inputs.ctl_at(2, i).unwrap_or(1.0).clamp(1.0, 32.0) as usize;
            if self.counter % div == 0 {
                let levels = 2f32.powf(bits) - 1.0;
                self.hold = (x * 0.5 + 0.5).clamp(0.0, 1.0) * levels;
                self.hold = (self.hold.round() / levels) * 2.0 - 1.0;
            }
            self.counter += 1;
            outputs.out(0)[i] = self.hold;
        }
    }
}

/// Waveshaper : drivé tanh / courbes 0..3 (tanh, hard clip, foldback).
pub struct Waveshaper {
    drive: f32,
    curve: f32,
}

impl Waveshaper {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            drive: params.get("drive", 2.0) as f32,
            curve: 0.0,
        }
    }
}

impl DspNode for Waveshaper {
    fn reset(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let x = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let drive = inputs.ctl_at(1, i).unwrap_or(self.drive).clamp(1.0, 20.0);
            let curve = inputs.ctl_at(2, i).unwrap_or(0.0);
            let d = x * drive;
            let shaped = if curve < 0.5 {
                d.tanh()
            } else if curve < 1.5 {
                d.clamp(-1.0, 1.0)
            } else {
                // foldback
                if d.abs() > 1.0 {
                    1.0 - (d.abs() - 1.0).min(1.0) * (d / d.abs())
                } else {
                    d
                }
            };
            outputs.out(0)[i] = shaped / drive.sqrt();
        }
    }
}

/// Saturation tube / tape : tanh asymétrique + lissage "warmth".
pub struct TapeSaturation {
    drive: f32,
    warmth: f32,
    lp: f32,
}

impl TapeSaturation {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            drive: params.get("drive", 1.5) as f32,
            warmth: params.get("warmth", 0.5) as f32,
            lp: 0.0,
        }
    }
}

impl DspNode for TapeSaturation {
    fn reset(&mut self) {
        self.lp = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let x = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let drive = inputs.ctl_at(1, i).unwrap_or(self.drive).clamp(1.0, 10.0);
            let warmth = inputs.ctl_at(2, i).unwrap_or(self.warmth).clamp(0.0, 1.0);
            let bias = 0.02 * drive; // un léger bias harmonique paire
            let d = x * drive * (1.0 + bias);
            let sat = d.tanh();
            let k = (1.0 - warmth * 0.75).max(0.05);
            self.lp += k * (sat - self.lp);
            outputs.out(0)[i] = self.lp;
        }
    }
}
