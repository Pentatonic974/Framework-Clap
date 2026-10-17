//! Modulation : enveloppe ADSR, LFO multi-formes, suiveur d'enveloppe, sample & hold.

use super::commons::*;

/// Enveloppe ADSR déclenchée par un événement Note (gate).
/// Port 0 : Gate (event). Sortie 0 : Env Out (control, 0..1).
pub struct AdsrEnv {
    attack_ms: f32,
    decay_ms: f32,
    sustain: f32,
    release_ms: f32,
    stage: Stage,
    value: f32,
    gate_on: bool,
    release_start: f32,
}

#[derive(PartialEq, Clone, Copy)]
enum Stage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

impl AdsrEnv {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            attack_ms: params.get("attack_ms", 10.0) as f32,
            decay_ms: params.get("decay_ms", 100.0) as f32,
            sustain: params.get("sustain", 0.7) as f32,
            release_ms: params.get("release_ms", 200.0) as f32,
            stage: Stage::Idle,
            value: 0.0,
            gate_on: false,
            release_start: 0.0,
        }
    }
}

impl DspNode for AdsrEnv {
    fn reset(&mut self) {
        self.stage = Stage::Idle;
        self.value = 0.0;
        self.gate_on = false;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        // Événements : le moteur met le dernier Note à jour avant le bloc;
        // ici on consomme le premier événement Notefound.
        for e in inputs.events {
            if let DspEvent::Note { velocity, on, .. } = e {
                if *on && !self.gate_on {
                    self.gate_on = true;
                    self.stage = Stage::Attack;
                    self.release_start = 0.0;
                } else if !*on && self.gate_on {
                    self.gate_on = false;
                    self.stage = Stage::Release;
                    self.release_start = self.value;
                }
                let _ = velocity;
            }
        }

        let atk = one_pole_coef(self.attack_ms.max(1.0), ctx.sample_rate);
        let dec = one_pole_coef(self.decay_ms.max(1.0), ctx.sample_rate);
        let rel = one_pole_coef(self.release_ms.max(1.0), ctx.sample_rate);
        let sustain_target = self.sustain.clamp(0.0, 1.0);

        for i in 0..n {
            match self.stage {
                Stage::Attack => {
                    self.value = 1.0 - (1.0 - self.value) * atk;
                    if self.value > 0.999 {
                        self.stage = Stage::Decay;
                    }
                }
                Stage::Decay => {
                    self.value = sustain_target + (self.value - sustain_target) * dec;
                    if (self.value - sustain_target).abs() < 0.001 {
                        self.stage = Stage::Sustain;
                    }
                }
                Stage::Sustain => self.value = sustain_target,
                Stage::Release => {
                    self.value = self.release_start * (self.release_start / self.release_start.max(1e-9)).ln() * 0.0
                        + self.value * rel;
                    if self.value < 0.001 {
                        self.value = 0.0;
                        self.stage = Stage::Idle;
                    }
                }
                Stage::Idle => self.value = 0.0,
            }
            outputs.out(0)[i] = self.value;
        }
    }
}

/// LFO multi-formes : shape 0=sin, 1=tri, 2=saw, 3=square.
/// Ports : 0 Rate Hz, 1 Sync (expérimental v1), 2 Shape.
pub struct LfoMod {
    rate: f32,
    shape_default: f32,
    phase: f32,
}

impl LfoMod {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            rate: params.get("rate_hz", 5.0) as f32,
            shape_default: params.get("shape", 0.0) as f32,
            phase: 0.0,
        }
    }
}

impl DspNode for LfoMod {
    fn reset(&mut self) {
        self.phase = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let rate = inputs.ctl_at(0, i).unwrap_or(self.rate).clamp(0.05, 100.0);
            let shape = inputs.ctl_at(2, i).unwrap_or(self.shape_default);
            self.phase += rate / ctx.sample_rate;
            self.phase -= self.phase.floor();
            let p = self.phase;
            let v = match (shape.round() as i32).rem_euclid(4) {
                1 => 1.0 - (p * 4.0) % 2.0 - if p < 0.75 { 0.0 } else { 0.0 }, // triangle
                2 => 2.0 * p - 1.0,                                           // saw
                3 => if p < 0.5 { 1.0 } else { -1.0 },                        // square
                _ => (p * std::f32::consts::TAU).sin(),
            };
            outputs.out(0)[i] = v.clamp(-1.0, 1.0);
        }
    }
}

/// Suiveur d'enveloppe : détecteur de crête avec attaques/release lissés.
/// Ports : 0 Audio In, 1 Attack ms, 2 Release ms.
pub struct EnvFollower {
    env: f32,
}

impl EnvFollower {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            env: 0.0,
        }
    }
}

impl DspNode for EnvFollower {
    fn reset(&mut self) {
        self.env = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let x = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            let coef = if x.abs() > self.env {
                one_pole_coef(inputs.ctl_at(1, i).unwrap_or(10.0).clamp(0.5, 200.0), ctx.sample_rate)
            } else {
                one_pole_coef(inputs.ctl_at(2, i).unwrap_or(100.0).clamp(1.0, 1000.0), ctx.sample_rate)
            };
            self.env += coef * (x.abs() - self.env);
            outputs.out(0)[i] = self.env.min(1.0);
        }
    }
}

/// Sample & Hold : bloque la valeur à chaque front haut du clock (event).
pub struct SampleHold {
    held: f32,
}

impl SampleHold {
    pub fn new(_params: &NodeParams) -> Self {
        Self { held: 0.0 }
    }
}

impl DspNode for SampleHold {
    fn reset(&mut self) {
        self.held = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        // Clock (control par front) gate : nouvelle valeur aléatoire à chaque front haut.
        let mut prev = 0.0f32;
        for i in 0..ctx.frames {
            let gate = inputs.ctl_at(0, i).unwrap_or(0.0);
            if gate > 0.5 && prev <= 0.5 {
                // PRNG xorshift linéaire : state drift légèrement, pas d'allocation.
                self.held = ((self.held * 12_345.0 + 89_023.0) % 1.0) * 2.0 - 1.0;
            }
            prev = gate;
            outputs.out(0)[i] = self.held;
        }
    }
}
