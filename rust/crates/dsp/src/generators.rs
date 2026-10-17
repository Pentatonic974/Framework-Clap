//! Générateurs / sources : oscillateur VA, bruit, moteur additif.

use super::commons::*;

/// Oscillateur VA poly-BLEP : waveform 0=saw, 1=pulse(PWM), 2=triangle.
/// Port 0 : Pitch (control, note MIDI). Port 1 : PWM (control, 0..1).
pub struct OscVa {
    waveform: f32,
    detune_cents: f32,
    phase: f32,
    pub freq_hz: f32,
}

impl OscVa {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            waveform: params.get("waveform", 0.0),
            detune_cents: params.get("detune", 0.0),
            phase: 0.0,
            freq_hz: 440.0,
        }
    }
}

impl DspNode for OscVa {
    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let out = outputs.out(0);
        let pwm = inputs.ctl(1).clamp(0.0, 1.0);
        let wf = (self.waveform.round() as i32).rem_euclid(3);
        for i in 0..n {
            let pitch = inputs.ctl_at(0, i).unwrap_or(69.0);
            let freq = midi_to_hz(pitch + self.detune_cents / 1200.0)
                .clamp(0.01, ctx.sample_rate * 0.45);
            self.freq_hz = freq;
            let dt = freq / ctx.sample_rate;
            self.phase += dt;
            self.phase -= self.phase.floor();

            let s = if wf == 1 {
                // Pulse : correction PolyBLEP sur les deux discontinuités.
                let duty = pwm.clamp(0.02, 0.98);
                let mut p = if self.phase < duty { 1.0 } else { -1.0 };
                p += polyblep(self.phase, dt);
                p -= polyblep((self.phase - duty).rem_euclid(1.0), dt);
                p
            } else {
                // Saw : correction PolyBLEP sur la discontinuité unique.
                let mut p = 2.0 * self.phase - 1.0;
                p -= polyblep(self.phase, dt);
                if wf == 2 {
                    // Triangle : intégration explicite du pulse 50 % antialiasé.
                    p = (self.phase - 0.25) * 4.0 - polyblep(
                        (self.phase - 0.25).rem_euclid(1.0),
                        dt,
                    );
                    p = p.clamp(-1.0, 1.0);
                }
                p
            };
            out[i] = s;
        }
    }
}

/// Bruit coloré : couleur 0 = blanc, >0 = filtre passe-bas à un pôle.
/// Port 0 : Color (control, 0..1).
pub struct NoiseGen {
    level: f32,
    lp_state: f32,
    rng: u64,
}

impl NoiseGen {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            level: params.get("level", 0.2),
            lp_state: 0.0,
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }
}

impl DspNode for NoiseGen {
    fn reset(&mut self) {
        self.lp_state = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let out = outputs.out(0);
        for i in 0..n {
            self.rng ^= self.rng << 13;
            self.rng ^= self.rng >> 7;
            self.rng ^= self.rng << 17;
            let white = ((self.rng & 0xFFFF) as f32 / 65535.0) * 2.0 - 1.0;
            let color = inputs.ctl_at(0, i).unwrap_or(0.0).clamp(0.0, 1.0);
            let value = if color <= 0.001 {
                white
            } else {
                let k = (-std::f32::consts::TAU * color * 4_000.0 / ctx.sample_rate).exp();
                self.lp_state = k * self.lp_state + (1.0 - k) * white;
                self.lp_state
            };
            out[i] = value * self.level;
        }
    }
}

/// Moteur additif : N partiels sinusoïdales, enveloppe 1/n.
/// Port 0 : Pitch (control). Port 1 : Harmonics (control, 0..1 -> fraction des partiels).
pub struct AdditiveEngine {
    harmonics: usize,
    phase: f32,
}

impl AdditiveEngine {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            harmonics: (params.get("harmonics", 8.0).max(1.0) as usize).min(64),
            phase: 0.0,
        }
    }
}

impl DspNode for AdditiveEngine {
    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let out = outputs.out(0);
        for i in 0..n {
            let pitch = inputs.ctl_at(0, i).unwrap_or(69.0);
            let h_frac = inputs.ctl_at(1, i).unwrap_or(1.0).clamp(0.0, 1.0);
            let h_max = ((self.harmonics as f32 * h_frac).round() as usize).max(1).min(self.harmonics);
            let freq = midi_to_hz(pitch).clamp(0.01, ctx.sample_rate * 0.4);
            self.phase += freq / ctx.sample_rate;
            self.phase -= self.phase.floor();
            let mut s = 0.0f32;
            for h in 1..=h_max {
                s += (self.phase * h as f32 * std::f32::consts::TAU).sin() / h as f32;
            }
            out[i] = s * 0.5;
        }
    }
}
