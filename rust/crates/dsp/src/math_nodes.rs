//! Math, logique & lissage : opérateur, scale/offset, clamp, logique, lisseur anti-zipper.

use super::commons::*;

fn op_index(v: f32, count: usize) -> usize {
    (v.round() as usize).min(count - 1)
}

/// Opérateur math : 0=+, 1=-, 2=*, 3=/.
/// Ports : 0 A, 1 B (control par échantillon).
pub struct MathOp {
    op: f32,
}

impl MathOp {
    pub fn new(params: &NodeParams) -> Self {
        Self { op: params.get("op", 0.0) }
    }
}

impl DspNode for MathOp {
    fn reset(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        let default_op = self.op;
        for i in 0..n {
            let a = inputs.ctl_at(0, i).unwrap_or(0.0);
            let b = inputs.ctl_at(1, i).unwrap_or(0.0);
            let op_sel = inputs.ctl_at(2, i).filter(|v| *v >= 0.0).unwrap_or(default_op);
            let r = match op_index(op_sel, 4) {
                0 => a + b,
                1 => a - b,
                2 => a * b,
                _ => if b.abs() > 1e-9 { a / b } else { 0.0 },
            };
            outputs.out(0)[i] = r;
        }
    }
}

/// Scale & offset. Ports : 0 In, 1 Scale, 2 Offset.
pub struct ScaleOffset {
    scale: f32,
    offset: f32,
}

impl ScaleOffset {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            scale: params.get("scale", 1.0) as f32,
            offset: params.get("offset", 0.0) as f32,
        }
    }
}

impl DspNode for ScaleOffset {
    fn reset(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let x = inputs.ctl_at(0, i).unwrap_or(0.0);
            let s = inputs.ctl_at(1, i).unwrap_or(self.scale);
            let o = inputs.ctl_at(2, i).unwrap_or(self.offset);
            outputs.out(0)[i] = x * s + o;
        }
    }
}

/// Clamp. Ports : 0 In, 1 Min, 2 Max.
pub struct ClampNode {
    min: f32,
    max: f32,
}

impl ClampNode {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            min: params.get("min", 0.0) as f32,
            max: params.get("max", 1.0) as f32,
        }
    }
}

impl DspNode for ClampNode {
    fn reset(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let x = inputs.ctl_at(0, i).unwrap_or(0.0);
            let min = inputs.ctl_at(1, i).unwrap_or(self.min);
            let max = inputs.ctl_at(2, i).unwrap_or(self.max);
            outputs.out(0)[i] = x.min(max).max(min);
        }
    }
}

/// Logique binare : 0=AND, 1=OR, 2=NOT(a). Seuil >0.5 = true.
pub struct LogicOp {
    op: f32,
}

impl LogicOp {
    pub fn new(params: &NodeParams) -> Self {
        Self { op: params.get("op", 0.0) }
    }
}

impl DspNode for LogicOp {
    fn reset(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let a = inputs.ctl_at(0, i).unwrap_or(0.0) > 0.5;
            let b = inputs.ctl_at(1, i).unwrap_or(0.0) > 0.5;
            let op = op_index(inputs.ctl_at(2, i).unwrap_or(self.op), 3);
            let r = match op {
                0 => a && b,
                1 => a || b,
                _ => !a,
            };
            outputs.out(0)[i] = if r { 1.0 } else { 0.0 };
        }
    }
}

/// Lisseur anti-zipper un pôle. Ports : 0 In, 1 Time ms.
pub struct SmoothSlew {
    state: f32,
}

impl SmoothSlew {
    pub fn new(params: &NodeParams) -> Self {
        Self {
            state: 0.0,
        }
    }
}

impl DspNode for SmoothSlew {
    fn reset(&mut self) {
        self.state = 0.0;
    }

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let x = inputs.ctl_at(0, i).unwrap_or(0.0);
            let time_ms = inputs.ctl_at(1, i).unwrap_or(20.0).clamp(0.5, 1000.0);
            let k = one_pole_coef(time_ms, ctx.sample_rate);
            self.state = x + (self.state - x) * k;
            outputs.out(0)[i] = self.state;
        }
    }
}

/// Quantificateur de gamme chromatique (0 = chroma 12 pts, 1 = gamme majeure).
pub struct Quantizer {
    scale: f32,
}

impl Quantizer {
    pub fn new(params: &NodeParams) -> Self {
        Self { scale: params.get("scale_select", 0.0) }
    }
}

impl DspNode for Quantizer {
    fn reset(&mut self) {}

    fn process(&mut self, ctx: &ProcessCtx, inputs: &NodeInputs<'_>, outputs: &mut NodeOutputs<'_>) {
        let n = ctx.frames;
        for i in 0..n {
            let pitch = inputs.ctl_at(0, i).unwrap_or(60.0);
            let scale = inputs.ctl_at(1, i).unwrap_or(self.scale);
            if scale < 0.5 {
                outputs.out(0)[i] = pitch.round();
            } else {
                // Gamme majeure [0,2,4,5,7,9,11] autour de C.
                let oct = pitch.rem_euclid(12.0);
                let majors = [0.0f32, 2.0, 4.0, 5.0, 7.0, 9.0, 11.0];
                let mut best = majors[0];
                let mut best_d = f32::INFINITY;
                for c in majors {
                    let d = (oct - c).abs();
                    if d < best_d {
                        best_d = d;
                        best = c;
                    }
                }
                let quant = pitch - oct + best;
                outputs.out(0)[i] = if best_d.abs() < f32::EPSILON { pitch.round() } else {
                    if oct - best > 0.5 { quant } else { quant }
                };
            }
        }
    }
}
