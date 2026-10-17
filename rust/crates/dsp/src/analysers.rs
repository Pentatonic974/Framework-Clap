//! Analyse & mesure : vumètre peak/RMS, oscilloscope (écriture lock-free).

use super::commons::*;

/// Mesureur crête / RMS. Port 0 : Audio In.
/// Slots mètres : 0 = peak, 1 = rms (multiplexés par nœud).
pub struct PeakRmsMeter {
    peak: f32,
    rms_env: f32,
    meter_slot: usize,
}

impl PeakRmsMeter {
    pub fn new(_params: &NodeParams) -> Self {
        Self { peak: 0.0, rms_env: 0.0, meter_slot: usize::MAX }
    }
}

impl DspNode for PeakRmsMeter {
    fn reset(&mut self) {
        self.peak = 0.0;
        self.rms_env = 0.0;
    }

    fn meter_slots(&self) -> usize {
        2
    }

    fn configure(&mut self, config: NodeConfig) {
        self.meter_slot = config.meter_slot;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let mut peak = 0.0f32;
        let mut energy = 0.0f32;
        for i in 0..n {
            let x = inputs.audio(0).get(i).copied().unwrap_or(0.0);
            peak = peak.max(x.abs());
            energy += x * x;
        }
        let rms_inst = (energy / n.max(1) as f32).sqrt();
        // Release ~600 ms pour un affichage stable.
        let rel = one_pole_coef(600.0, ctx.sample_rate);
        self.rms_env += rel * (rms_inst - self.rms_env);
        self.peak = peak.max(self.peak * 0.6);

        let bus = outputs.analysis;
        let base = self.meter_slot * 4;
        if base < bus.meters.len() {
            bus.meters[base].store(self.peak);
        }
        if base + 1 < bus.meters.len() {
            bus.meters[base + 1].store(self.rms_env);
        }
    }
}

/// Oscilloscope : écrit la forme d'onde dans un ScopeRing du bus (lock-free).
pub struct Oscilloscope {
    scope_index: usize,
    meter_slot: usize,
}

impl Oscilloscope {
    pub fn new(_params: &NodeParams) -> Self {
        Self { scope_index: usize::MAX, meter_slot: usize::MAX }
    }
}

impl DspNode for Oscilloscope {
    fn reset(&mut self) {}

    fn wants_scope(&self) -> Option<usize> {
        Some(2048)
    }

    fn meter_slots(&self) -> usize {
        1
    }

    fn configure(&mut self, config: NodeConfig) {
        self.scope_index = config.scope_index;
        self.meter_slot = config.meter_slot;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        if self.scope_index != usize::MAX {
            if let Some(ring) = outputs.analysis.scopes.get(self.scope_index) {
                ring.write(&inputs.audio(0)[..n.min(2048)]);
            }
        }
        if self.meter_slot != usize::MAX {
            let mut peak = 0.0f32;
            for i in 0..n {
                peak = peak.max(inputs.audio(0).get(i).copied().unwrap_or(0.0).abs());
            }
            let base = self.meter_slot * 4;
            if base < outputs.analysis.meters.len() {
                outputs.analysis.meters[base].store(peak);
            }
        }
    }
}
