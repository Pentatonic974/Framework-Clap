//! Dynamique & amplitude : gain, compresseur, limiteur, gate.

use super::commons::*;

/// Gain / atténuateur lissé. Ports : 0 Audio In, 1 Gain dB (control).
pub struct GainNode {
    current: f32,
    gain_db: f32,
}

impl GainNode {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            current: db_to_gain(params.get("gain_db", 0.0) as f32),
            gain_db: params.get("gain_db", 0.0) as f32,
        }
    }
}

impl DspNode for GainNode {
    fn reset(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let k = one_pole_coef(20.0, ctx.sample_rate); // lissage anti-zipper ~20 ms
        for i in 0..n {
            let src = inputs.audio(0);
            let target_db = inputs.ctl_at(1, i).unwrap_or(self.gain_db);
            let target = db_to_gain(target_db.clamp(-96.0, 24.0));
            self.current = target + (self.current - target) * k;
            outputs.out(0)[i] = src.get(i).copied().unwrap_or(0.0) * self.current;
        }
    }
}

/// Compresseur RMS simple, détection binature + knee doux.
/// Ports : 0 In L, 1 In R, 2 Threshold dB, 3 Ratio, 4 Attack ms, 5 Release ms, 6 Sidechain.
pub struct Compressor {
    env: f32,
    gain_smooth: f32,
}

impl Compressor {
    pub fn new(_params: &NodeParams) -> Self {
        Self { env: 0.0, gain_smooth: 1.0 }
    }
}

impl DspNode for Compressor {
    fn reset(&mut self) {
        self.env = 0.0;
        self.gain_smooth = 1.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let threshold = inputs.ctl(2).clamp(-60.0, 0.0);
        let ratio = inputs.ctl(3).clamp(1.0, 20.0);
        let attack = one_pole_coef(inputs.ctl(4).clamp(0.1, 200.0), ctx.sample_rate);
        let release = one_pole_coef(inputs.ctl(5).clamp(1.0, 2000.0), ctx.sample_rate);
        for i in 0..n {
            let l = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let r = inputs.audio(1).get(i).copied().unwrap_or(0.0);
            // Détection : sidechain port 6 sinon signal d'entrée.
            let sc = inputs.audio(6);
            let det = if !sc.is_empty() { sc[i.min(sc.len() - 1)] } else { (l + r) * 0.5 };
            let sq = det * det;
            // Enveloppe RMS un pôle asymétrique.
            let coef = if sq > self.env { attack } else { release };
            self.env = self.env + coef * (sq - self.env);
            let env_db = gain_to_db(self.env.sqrt());
            let mut g = 1.0;
            if env_db > threshold {
                let over = env_db - threshold;
                let target = over * (1.0 / ratio) - over;
                g = db_to_gain(target);
            }
            self.gain_smooth = g + (self.gain_smooth - g) * attack;
            outputs.out(0)[i] = l * self.gain_smooth;
            outputs.out(1)[i] = r * self.gain_smooth;
        }
    }
}

/// Limiteur de crête look-ahead zéro (gain rétroactif doux).
/// Ports : 0 Audio In, 1 Ceiling dB.
pub struct Limiter {
    envelope: f32,
}

impl Limiter {
    pub fn new(_params: &NodeParams) -> Self {
        Self { envelope: 0.0 }
    }
}

impl DspNode for Limiter {
    fn reset(&mut self) {
        self.envelope = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let ceiling = db_to_gain(inputs.ctl(1).clamp(-24.0, 6.0));
        for i in 0..n {
            let src = inputs.audio(0);
            let x = src.get(i).copied().unwrap_or(0.0);
            let abs = x.abs().max(1e-9);
            // Attaque brève (2 ms), release long (150 ms).
            let target_gain = if abs > ceiling { ceiling / abs } else { 1.0 };
            let coef = if target_gain < self.envelope {
                one_pole_coef(2.0, ctx.sample_rate)
            } else {
                one_pole_coef(150.0, ctx.sample_rate)
            };
            self.envelope = target_gain + (self.envelope - target_gain) * coef;
            outputs.out(0)[i] = x * self.envelope;
        }
    }
}

/// Expander / noise gate un pôle.
/// Ports : 0 Audio In, 1 Threshold dB.
pub struct NoiseGate {
    envelope: f32,
    open: bool,
}

impl NoiseGate {
    pub fn new(_params: &NodeParams) -> Self {
        Self { envelope: 0.0, open: false }
    }
}

impl DspNode for NoiseGate {
    fn reset(&mut self) {
        self.envelope = 0.0;
        self.open = false;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let th_gain = db_to_gain(inputs.ctl(1).clamp(-96.0, 0.0));
        for i in 0..n {
            let src = inputs.audio(0);
            let x = src.get(i).copied().unwrap_or(0.0);
            let coef = if x.abs() > self.envelope {
                one_pole_coef(1.0, ctx.sample_rate)
            } else {
                one_pole_coef(50.0, ctx.sample_rate)
            };
            self.envelope += coef * (x.abs() - self.envelope);
            let mut target = 1.0f32;
            if self.envelope < th_gain {
                self.open = false;
                target = 0.0;
            } else if self.envelope > th_gain * 1.15 {
                self.open = true;
            }
            if self.open {
                target = 1.0;
            }
            outputs.out(0)[i] = x * target;
        }
    }
}
