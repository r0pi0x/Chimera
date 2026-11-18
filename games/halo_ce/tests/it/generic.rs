//! The transparent generic shader's stage combiner (`maps::generic`,
//! specs/halo_ce/shaders.md "Transparent generic") on synthetic stages:
//! no stages give map 0; products, sums, the mux by scratch alpha 0, the
//! input mappings, an alpha input of the 15..24 codes reading blue, the
//! output mappings with their clamp, and reads before writes.

use blam_formats::{
    cache::RealArgb,
    shader::{
        AnimationSource, GenericInput as I, GenericStage, InputMapping as M, OutputFunction, OutputMapping,
        PeriodicFunction, StageAlphaOutputs, StageColorOutputs, StageInput, StageInputs, StageOutput as O,
    },
};
use halo_ce::game::maps::generic::{Registers, combine};

fn argb(a: f32, r: f32, g: f32, b: f32) -> RealArgb {
    RealArgb { a, r, g, b }
}

fn inputs(x: [(I, M); 4]) -> StageInputs {
    let i = |(input, mapping): (I, M)| StageInput { input, mapping };
    StageInputs {
        a: i(x[0]),
        b: i(x[1]),
        c: i(x[2]),
        d: i(x[3]),
    }
}

/// A stage writing its sum (or mux) to scratch 0, colour and alpha.
fn stage(flags: u16, c0: RealArgb, colour: [(I, M); 4], alpha: [(I, M); 4], mapping: OutputMapping) -> GenericStage {
    GenericStage {
        flags,
        constant_color0_source: AnimationSource::None,
        constant_color0_function: PeriodicFunction::One,
        constant_color0_period: 1.0,
        constant_color0_lower: c0,
        constant_color0_upper: c0,
        constant_color1: argb(0.0, 0.0, 0.0, 0.0),
        color_inputs: inputs(colour),
        color_outputs: StageColorOutputs {
            ab: O::Discard,
            ab_function: OutputFunction::Multiply,
            cd: O::Discard,
            cd_function: OutputFunction::Multiply,
            mux_sum: O::Scratch0,
            mapping,
        },
        alpha_inputs: inputs(alpha),
        alpha_outputs: StageAlphaOutputs {
            ab: O::Discard,
            cd: O::Discard,
            mux_sum: O::Scratch0,
            mapping,
        },
    }
}

const ZERO: (I, M) = (I::Zero, M::Clamp);

fn regs(map0: [f32; 4]) -> Registers {
    Registers {
        maps: [map0, [0.0; 4], [0.0; 4], [0.0; 4]],
        vertex: [[1.0; 4]; 2],
        ..Default::default()
    }
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
}

#[test]
fn no_stages_give_map_0() {
    assert_eq!(combine(&[], regs([0.2, 0.4, 0.6, 0.8]), &|_| 1.0), [0.2, 0.4, 0.6, 0.8]);
}

#[test]
fn map_0_times_constant_0_plus_a_half() {
    // rgb = map0 x c0 + 1/2 x 1; alpha = map0.a x c0.a.
    let s = stage(
        0,
        argb(0.5, 1.0, 0.5, 0.0),
        [
            (I::Map0, M::Clamp),
            (I::Constant0, M::Clamp),
            (I::Half, M::Clamp),
            (I::One, M::Clamp),
        ],
        [(I::Map0, M::Clamp), (I::Constant0, M::Clamp), ZERO, ZERO],
        OutputMapping::Identity,
    );
    let out = combine(&[s], regs([0.4, 0.4, 0.4, 0.8]), &|_| 1.0);
    assert!(near(out, [0.9, 0.7, 0.5, 0.4]), "{out:?}");
}

#[test]
fn mappings_clamps_and_the_blue_alpha_input() {
    // rgb = (1 - map0) x 1, doubled and clamped to 1; alpha = map0's
    // blue (code 15 as an alpha input) x 1.
    let s = stage(
        0,
        argb(0.0, 0.0, 0.0, 0.0),
        [(I::Map0, M::OneMinusClamp), (I::One, M::Clamp), ZERO, ZERO],
        [(I::Map0Alt, M::Clamp), (I::One, M::Clamp), ZERO, ZERO],
        OutputMapping::Double,
    );
    let out = combine(&[s], regs([0.1, 0.6, 0.9, 0.3]), &|_| 1.0);
    assert!(near(out, [1.0, 0.8, 0.2, 1.0]), "{out:?}");
    // Expand signed of a half is 0; negate keeps the sign; the output
    // clamp is [-1, 1].
    let t = stage(
        0,
        argb(0.0, 0.0, 0.0, 0.0),
        [
            (I::Half, M::ExpandSigned),
            (I::One, M::Clamp),
            (I::One, M::Negate),
            (I::One, M::Clamp),
        ],
        [ZERO, ZERO, ZERO, ZERO],
        OutputMapping::Quadruple,
    );
    assert!(near(combine(&[t], regs([0.0; 4]), &|_| 1.0), [-1.0, -1.0, -1.0, 0.0]));
}

#[test]
fn the_mux_picks_cd_when_scratch_alpha_0_is_a_half_or_more() {
    // Stage 0 sets scratch alpha 0 from map0.a; stage 1 muxes AB (one) and
    // CD (one half).
    let first = stage(
        0,
        argb(0.0, 0.0, 0.0, 0.0),
        [ZERO, ZERO, ZERO, ZERO],
        [(I::Map0, M::Clamp), (I::One, M::Clamp), ZERO, ZERO],
        OutputMapping::Identity,
    );
    let mux = stage(
        GenericStage::COLOR_MUX,
        argb(0.0, 0.0, 0.0, 0.0),
        [
            (I::One, M::Clamp),
            (I::One, M::Clamp),
            (I::Half, M::Clamp),
            (I::One, M::Clamp),
        ],
        [(I::Scratch0, M::Clamp), (I::One, M::Clamp), ZERO, ZERO],
        OutputMapping::Identity,
    );
    let low = combine(&[first.clone(), mux.clone()], regs([0.0, 0.0, 0.0, 0.3]), &|_| 1.0);
    let high = combine(&[first, mux], regs([0.0, 0.0, 0.0, 0.5]), &|_| 1.0);
    assert!(near(low, [1.0, 1.0, 1.0, 0.3]), "{low:?}");
    assert!(near(high, [0.5, 0.5, 0.5, 0.5]), "{high:?}");
}

#[test]
fn a_stage_reads_before_it_writes() {
    // AB writes scratch 0 = 1 and the sum reads scratch 0 (still 0): the
    // sum is 1 x 1 + 0, not 2.
    let mut s = stage(
        0,
        argb(0.0, 0.0, 0.0, 0.0),
        [
            (I::One, M::Clamp),
            (I::One, M::Clamp),
            (I::Scratch0, M::Clamp),
            (I::One, M::Clamp),
        ],
        [ZERO, ZERO, ZERO, ZERO],
        OutputMapping::Identity,
    );
    s.color_outputs.ab = O::Scratch1;
    let out = combine(&[s], regs([0.0; 4]), &|_| 1.0);
    assert!(near(out, [1.0, 1.0, 1.0, 0.0]), "{out:?}");
}
