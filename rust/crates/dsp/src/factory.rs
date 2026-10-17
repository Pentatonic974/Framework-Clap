//! Fabrique : crée une instance de nœud DSP concrète à partir du type de registre.
//! Appelée hors thread audio lors de la compilation du graphe.

use super::commons::{NodeParams, ProcessCtx};
use super::*;

/// Crée un nœud d'après son `node_type`. `None` si le type existe dans le
/// registre mais n'a pas encore d'implémentation (l'utilisateur est averti).
pub fn create_node(
    node_type: &str,
    params: &NodeParams,
    sample_rate: f32,
    max_frames: usize,
) -> Option<Box<dyn DspNode>> {
    let ctx = ProcessCtx { sample_rate, frames: max_frames };
    let _ = ctx;
    Some(match node_type {
        // Sources
        "osc_va" => Box::new(generators::OscVa::new(params)),
        "noise_gen" => Box::new(generators::NoiseGen::new(params)),
        "additive_engine" => Box::new(generators::AdditiveEngine::new(params)),
        // Filtres
        "filter_svf" => Box::new(filters::SvfFilter::new(params)),
        "filter_ladder" => Box::new(filters::LadderFilter::new(params)),
        "eq_parametric" => Box::new(filters::ParamEq::new(params)),
        "filter_comb" => Box::new(filters::CombFilter::new(params, sample_rate, max_frames)),
        // Dynamique
        "gain_node" => Box::new(dynamics::GainNode::new(params)),
        "compressor" => Box::new(dynamics::Compressor::new(params)),
        "limiter" => Box::new(dynamics::Limiter::new(params)),
        "noise_gate" => Box::new(dynamics::NoiseGate::new(params)),
        // Effets
        "delay_stereo" => Box::new(effects::StereoDelay::new(params, sample_rate)),
        "chorus_flanger" => Box::new(effects::ChorusFlanger::new(params, sample_rate)),
        "reverb_algo" => Box::new(effects::ReverbAlgo::new(params, sample_rate)),
        "waveshaper" => Box::new(effects::Waveshaper::new(params)),
        "saturation_tape" => Box::new(effects::TapeSaturation::new(params)),
        "bitcrusher" => Box::new(effects::Bitcrusher::new(params)),
        // Modulation
        "env_adsr" => Box::new(rhythm::AdsrEnv::new(params)),
        "lfo_mod" => Box::new(rhythm::LfoMod::new(params)),
        "env_follower" => Box::new(rhythm::EnvFollower::new(params)),
        "sample_hold" => Box::new(rhythm::SampleHold::new(params)),
        // Math
        "math_op" => Box::new(math_nodes::MathOp::new(params)),
        "scale_offset" => Box::new(math_nodes::ScaleOffset::new(params)),
        "clamp_value" => Box::new(math_nodes::ClampNode::new(params)),
        "logic_op" => Box::new(math_nodes::LogicOp::new(params)),
        "smooth_slew" => Box::new(math_nodes::SmoothSlew::new(params)),
        "quantizer" => Box::new(math_nodes::Quantizer::new(params)),
        // Analyse
        "peak_rms_meter" => Box::new(analysers::PeakRmsMeter::new(params)),
        "oscilloscope" => Box::new(analysers::Oscilloscope::new(params)),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn factory_creates_all_core_nodes() {
        for t in ["osc_va", "filter_svf", "gain_node", "env_adsr", "delay_stereo", "peak_rms_meter"] {
            let node = create_node(t, &NodeParams::default(), 48_000.0, 512);
            assert!(node.is_some(), "fabrique incomplète: {t}");
        }
    }

    #[test]
    fn factory_rejects_io_nodes() {
        // Les E/S clap_audio_in/out ne sont pas des nœuds DSP : elles sont
        // traitées par le moteur lui-même (ports du plugin).
        for t in ["clap_audio_in", "clap_audio_out", "clap_param"] {
            assert!(create_node(t, &NodeParams::default(), 48_000.0, 512).is_none(), "{t} ne doit pas être instancié comme nœud DSP");
        }
    }
}
