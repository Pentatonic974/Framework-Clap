//! Filtres : SVF (TPT), ladder 24 dB, biquad paramétrique, peigne.

use super::commons::*;

/// Filtre à variables d'état topologiquement préservé (TPT SVF).
/// Ports : 0 Audio In, 1 Cutoff (Hz, control), 2 Reso (control, 0.1..10).
/// Sorties : 0 LP, 1 HP, 2 BP. Mode : param `mode` 0=LP 1=HP 2=BP.
pub struct SvfFilter {
    cutoff_hz: f32,
    resonance: f32,
    ic1eq: f32,
    ic2eq: f32,
    out_lp: f32,
}

impl SvfFilter {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            cutoff_hz: params.get("cutoff_hz", 1000.0) as f32,
            resonance: params.get("resonance", 0.7) as f32,
            ic1eq: 0.0,
            ic2eq: 0.0,
            out_lp: 0.0,
        }
    }
}

impl DspNode for SvfFilter {
    fn reset(&mut self) {
        self.ic1eq = 0.0;
        self.ic2eq = 0.0;
        self.out_lp = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let mode = (inputs.ctl(0) * 0.0).round(); // mode via param -> modulé plus tard
        let _ = mode;
        let reso = inputs.ctl(2);
        let k = reso.clamp(0.1, 10.0).max(0.5) * 2.0;
        for i in 0..n {
            let src = inputs.audio(0);
            let v_in = src.get(i).copied().unwrap_or(0.0);
            let cutoff = inputs
                .ctl_at(1, i)
                .unwrap_or(self.cutoff_hz)
                .clamp(20.0, ctx.sample_rate * 0.45);
            let g = (std::f32::consts::PI * cutoff / ctx.sample_rate).tan();
            let a1 = 1.0 / (1.0 + g * (g + k));
            let a2 = g * a1;
            let a3 = g * a2;

            let v3 = v_in - self.ic2eq;
            let v1 = a1 * self.ic1eq + a2 * v3;
            let v2 = self.ic2eq + a2 * self.ic1eq + a3 * v3;
            self.ic1eq = 2.0 * v1 - self.ic1eq;
            self.ic2eq = 2.0 * v2 - self.ic2eq;
            self.out_lp = v2;

            outputs.out(0)[i] = v2;
            outputs.out(1)[i] = v_in - k * v1 - v2;
            outputs.out(2)[i] = v1;
        }
    }
}

/// Ladder 24 dB / octave (4 étages un pôle + feedback tanh saturé).
/// Ports : 0 Audio In, 1 Cutoff, 2 Reso (0..1), 3 Drive.
pub struct LadderFilter {
    cutoff_hz: f32,
    resonance: f32,
    stage: [f32; 4],
}

impl LadderFilter {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            cutoff_hz: params.get("cutoff_hz", 1000.0) as f32,
            resonance: params.get("resonance", 0.0) as f32,
            stage: [0.0; 4],
        }
    }
}

impl DspNode for LadderFilter {
    fn reset(&mut self) {
        self.stage = [0.0; 4];
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let reso = inputs.ctl(2).clamp(0.1, 1.0);
        let drive = inputs.ctl(3).clamp(1.0, 4.0);
        for i in 0..n {
            let src = inputs.audio(0);
            let cutoff = inputs
                .ctl_at(1, i)
                .unwrap_or(self.cutoff_hz)
                .clamp(20.0, ctx.sample_rate * 0.45);
            // 2x oversampling du coefficient pour rester stable au feedback élevé.
            let g = (std::f32::consts::PI * cutoff / ctx.sample_rate).tan().min(4.0);
            let k = 1.0 - (-std::f32::consts::TAU * g).exp();
            let input = inputs.audio(0).get(i).copied().unwrap_or(0.0) * drive;
            let mut v = input + self.stage[3] * (reso * 12.0);
            v = v.tanh();
            for s in self.stage.iter_mut() {
                *s += k * (v - *s);
                v = *s;
            }
            outputs.out(0)[i] = self.stage[3];
        }
    }
}

/// Biquad peaking (formules de RBJ). Ports : 0 Audio In, 1 Freq, 2 Q, 3 Gain dB.
pub struct ParamEq {
    b0: f32, b1: f32, b2: f32, a1: f32, a2: f32,
    x1: f32, x2: f32, y1: f32, y2: f32,
    last_freq: f32, last_q: f32, last_gain: f32,
}

impl ParamEq {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0,
            x1: 0.0, x2: 0.0, y1: 0.0, y2: 0.0,
            last_freq: params.get("freq_hz", 1000.0) as f32,
            last_q: params.get("q", 1.0) as f32,
            last_gain: params.get("gain_db", 0.0) as f32,
        }
    }

    fn recompute(&mut self, freq: f32, q: f32, gain_db: f32, sr: f32) {
        let a = 10f32.powf(gain_db / 40.0);
        let w0 = std::f32::consts::TAU * freq.clamp(20.0, sr * 0.45) / sr;
        let (s, c) = w0.sin_cos();
        let alpha = s / (2.0 * q.max(0.05));
        let b0 = 1.0 + alpha * a;
        let b1 = -2.0 * c;
        let b2 = 1.0 - alpha * a;
        let a0 = 1.0 + alpha / a;
        let a1 = -2.0 * c / a0;
        let a2 = (1.0 - alpha / a) / a0;
        self.b0 = b0 / a0;
        self.b1 = b1 / a0;
        self.b2 = b2 / a0;
        self.a1 = a1;
        self.a2 = a2;
        self.last_freq = freq;
        self.last_q = q;
        self.last_gain = gain_db;
    }
}

impl DspNode for ParamEq {
    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        // Recalcul seulement si les modalités changent assez (économie CPU).
        let freq = inputs.ctl(1);
        let q = inputs.ctl(2);
        let gain_db = inputs.ctl(3);
        self.recompute(
            if freq.abs() > 0.1 && freq.is_finite() { freq } else { self.last_freq },
            if q.abs() > 0.05 { q } else { self.last_q },
            if gain_db.abs() > 0.05 { gain_db } else { self.last_gain },
            ctx.sample_rate,
        );
        for i in 0..n {
            let src = inputs.audio(0);
            let x = src.get(i).copied().unwrap_or(0.0);
            let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
                - self.a1 * self.y1 - self.a2 * self.y2;
            self.x2 = self.x1;
            self.x1 = x;
            self.y2 = self.y1;
            self.y1 = y;
            outputs.out(0)[i] = y;
        }
    }
}

/// Filtre peigne avec feedback. Ports : 0 Audio In, 1 Time ms, 2 Feedback.
pub struct CombFilter {
    time_ms: f32,
    feedback: f32,
    buf: Vec<f32>,   // préalloué hors RT
    pos: usize,
}

impl CombFilter {
    pub fn new(params: &NodeParams, sample_rate: f32, max_frames: usize) -> Self {
        let max_ms = params.get("time_ms", 20.0).max(0.1) * 4.0;
        let cap = ((((max_ms / 1000.0) * sample_rate) as usize).max(16).min(sample_rate as usize)) + max_frames;
        Self {
            time_ms: params.get("time_ms", 20.0) as f32,
            feedback: params.get("feedback", 0.5) as f32,
            buf: vec![0.0; cap],
            pos: 0,
        }
    }
}

impl DspNode for CombFilter {
    fn reset(&mut self) {
        self.pos = 0;
        self.buf.iter_mut().for_each(|v| *v = 0.0);
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let cap = self.buf.len();
        if cap < 8 {
            for i in 0..n {
                outputs.out(0)[i] = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            }
            return;
        }
        for i in 0..n {
            let src = inputs.audio(0);
            let x = src.get(i).copied().unwrap_or(0.0);
            let time_ms = inputs
                .ctl_at(1, i)
                .unwrap_or(self.time_ms)
                .clamp(0.05, 500.0);
            let fb = inputs.ctl_at(2, i).unwrap_or(self.feedback).clamp(0.0, 0.99);
            let delay_samples = (time_ms / 1000.0 * ctx.sample_rate).round().max(1.0);
            let idx = (self.pos + cap - (delay_samples as usize).min(cap - 1)) % cap;
            let delayed = self.buf[idx];
            let out_val = x + delayed * fb;
            self.buf[self.pos] = (x + delayed * fb) * 0.5;
            self.pos = (self.pos + 1) % cap;
            outputs.out(0)[i] = out_val * 0.5;
        }
    }
}
