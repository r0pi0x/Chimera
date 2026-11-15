//! The displays on Halo's guns (`game::view::displays`; specs/halo_ce/
//! weapons.md "Test cases", "On-model displays", and shaders.md "Numeric
//! shaders and image choice"), on synthetic programs: the counter's
//! images for the rounds loaded, the numeric rule's caps and digit
//! positions, the compass's image for each heading and local north, the
//! meters' functions and cuts (the plasma rifle's heat with its floor, the
//! plasma pistol's charge), a baked meter's texels, and a program run
//! tick by tick from a weapon's state.

use blam_formats::object::function::{BoundsMode, FunctionOutput, ObjectFunction};
use halo_ce::game::{
    view::displays::{
        self, Display, Functions, GunProgram, METER_LEVELS, MeterColours, aim_forward, compass, meter_cut, meter_frame, numeric_image,
    },
    weapons::{ASSAULT_RIFLE, PLASMA_PISTOL, PLASMA_RIFLE, gun, tick::{HaloGun, TriggerMode}},
};

/// (tens, units) of the assault rifle's counter: limit 60, 10 images,
/// A = loaded / 60 in single precision.
fn counter(loaded: i16) -> (usize, usize) {
    let a = f32::from(loaded) / 60.0;
    (numeric_image(60, 10, 1, a, 0.0), numeric_image(60, 10, 0, a, 0.0))
}

#[test]
fn the_counter_shows_the_rounds_loaded() {
    let cases = [(0, (0, 0)), (1, (0, 1)), (9, (0, 9)), (10, (1, 0)), (32, (3, 2)), (60, (6, 0)), (59, (5, 9))];
    for (loaded, images) in cases {
        assert_eq!(counter(loaded), images, "{loaded} rounds");
    }
}

#[test]
fn the_numeric_rule_caps_and_counts_digits() {
    // Limit 99, two positions: 99/99 and anything above 1 read 99.
    for a in [1.0, 1.2] {
        assert_eq!((numeric_image(99, 10, 1, a, 0.0), numeric_image(99, 10, 0, a, 0.0)), (9, 9), "A {a}");
    }
    // A value above the limit shows the limit.
    let a = 99.0 / 60.0;
    assert_eq!((numeric_image(60, 10, 1, a, 0.0), numeric_image(60, 10, 0, a, 0.0)), (6, 0));
    // Limit 255: units 5, tens 5 (the hundreds has no part).
    assert_eq!((numeric_image(255, 10, 1, 1.0, 0.0), numeric_image(255, 10, 0, 1.0, 0.0)), (5, 5));
    // shaders.md: A = 32/60, D = 0.9; positions 0, 1, 2 read A.
    let (a, d) = (32.0f32 / 60.0, 0.9);
    let images: Vec<usize> = (0..3).map(|k| numeric_image(60, 10, k, a, d)).collect();
    assert_eq!(images, [2, 3, 0]);
    // 8 images read D: floor(60 x 0.9 + 0.5) = 54, 54 mod 8 = 6.
    assert_eq!(numeric_image(60, 8, 0, a, d), 6);
    // Limit 8, 8 images.
    for (d, image) in [(0.75, 6), (1.0, 0), (0.0, 0), (0.0624, 0), (0.0625, 1)] {
        assert_eq!(numeric_image(8, 8, 0, 0.0, d), image, "D {d}");
    }
}

/// The compass's image for forward `f` and local north `north`.
fn dial(f: [f32; 3], north: f32) -> usize {
    numeric_image(8, 8, 0, 0.0, compass(f, north).expect("not vertical"))
}

#[test]
fn the_compass_turns_with_the_heading() {
    use std::f32::consts::PI;
    let at = |deg: f32, north: f32| dial(aim_forward(deg.to_radians(), 0.0), north);
    let yaws = [0.0, 90.0, 180.0, 270.0];
    assert_eq!(yaws.map(|y| at(y, 0.0)), [6, 4, 2, 0], "local north 0");
    assert_eq!(yaws.map(|y| at(y, PI)), [2, 0, 6, 4], "local north pi (bloodgulch)");
    // Bearings either side of the half steps: images 4 / 5 / 4 / 3.
    let bearing = |deg: f32| {
        let r = deg.to_radians();
        dial([r.sin(), r.cos(), 0.0], 0.0)
    };
    assert_eq!([22.4, 22.5, -22.5, -22.6].map(bearing), [4, 5, 4, 3]);
    // Within about 5.7 degrees of vertical the input keeps its value.
    assert_eq!(compass([0.05, 0.0, 0.9987], 0.0), None);
}

/// A function "one, scaled by incoming `input` (1 to 4)", clipped to
/// `lower`..1.
fn one_by(input: i16, lower: f32) -> ObjectFunction {
    ObjectFunction {
        scale_function_by: input,
        bounds_mode: BoundsMode::Clip,
        lower_bound: lower,
        ..ObjectFunction::default()
    }
    .with_runtime()
}

/// The program's outgoing values after one tick of `state`.
fn run(program: &GunProgram, gun: &halo_ce::game::weapons::params::Gun, state: &HaloGun) -> [FunctionOutput; 4] {
    let mut f = Functions::default();
    displays::step(program, &|c| displays::export(c, gun, state), None, displays::tables(), &mut f);
    f.outgoing
}

#[test]
fn the_plasma_rifles_gauge_shows_its_heat_with_a_floor() {
    // Export A = heat; Fn B = one x in A, clipped to 0.1..1; the meter reads B.
    let program = GunProgram {
        exports: [1, 0, 0, 0],
        functions: vec![ObjectFunction::default(), one_by(1, 0.1)],
        parts: vec![(3, Display::Meter { source: 2 })],
        ..GunProgram::default()
    };
    let g = gun(PLASMA_RIFLE);
    for (heat, value) in [(0.0, 0.1), (0.05, 0.1), (0.1, 0.1), (0.5, 0.5), (1.0, 1.0)] {
        let state = HaloGun { heat, ..HaloGun::new(&g) };
        let out = run(&program, &g, &state);
        assert!((out[1].value - value).abs() < 1e-6, "heat {heat}: Fn B {}", out[1].value);
        assert_eq!(displays::frames(&program, &out), [(3, meter_frame(value))]);
    }
    assert_eq!(meter_frame(0.1), 2, "a sliver always lit");
    assert_eq!(meter_frame(1.0), METER_LEVELS);
}

#[test]
fn the_plasma_pistols_gauge_shows_its_charge() {
    // Export B = primary charged; Fn D = one x in B; the meter reads D.
    let program = GunProgram {
        exports: [0, 10, 0, 0],
        functions: vec![ObjectFunction::default(), ObjectFunction::default(), ObjectFunction::default(), one_by(2, 0.0)],
        parts: vec![(2, Display::Meter { source: 4 })],
        ..GunProgram::default()
    };
    let g = gun(PLASMA_PISTOL);
    let charging = HaloGun { trigger: TriggerMode::Charging, trigger_timer: 9, ..HaloGun::new(&g) };
    let charged = HaloGun { trigger: TriggerMode::Charged, ..HaloGun::new(&g) };
    for (state, value) in [(HaloGun::new(&g), 0.0), (charging, 0.5), (charged, 1.0)] {
        let out = run(&program, &g, &state);
        assert!((out[3].value - value).abs() < 1e-5, "Fn D {} (want {value})", out[3].value);
    }
}

#[test]
fn the_counter_follows_the_weapon_tick_by_tick() {
    // The assault rifle's program: export B = primary ammunition, Fn A =
    // one x in B, the tens part (mesh 4) and the units part (mesh 5).
    let program = GunProgram {
        exports: [0, 2, 0, 0],
        inputs: [0, 0, 0, 19],
        functions: vec![one_by(2, 0.0)],
        parts: vec![
            (4, Display::Numeric { limit: 60, position: 1, images: 10 }),
            (5, Display::Numeric { limit: 60, position: 0, images: 10 }),
        ],
    };
    let g = gun(ASSAULT_RIFLE);
    let mut state = HaloGun::new(&g);
    let mut f = Functions::default();
    let mut shown = |state: &HaloGun| {
        displays::step(&program, &|c| displays::export(c, &g, state), compass([1.0, 0.0, 0.0], 0.0), displays::tables(), &mut f);
        displays::frames(&program, &f.outgoing)
    };
    assert_eq!(shown(&state), [(4, 6), (5, 0)], "60");
    state.loaded = 59;
    assert_eq!(shown(&state), [(4, 5), (5, 9)], "59 on the tick of the shot");
    state.loaded = 0;
    assert_eq!(shown(&state), [(4, 0), (5, 0)], "00 when empty");
}

#[test]
fn a_meter_is_cut_at_its_value() {
    let s = MeterColours {
        minimum: [0.0, 1.0, 0.0],
        maximum: [1.0, 0.0, 0.0],
        background: [0.4, 0.4, 1.0],
        tint: [0.8, 1.0, 0.8],
        ..MeterColours::default()
    };
    // Blue (the position along the bar) 0, 64, 192; the last texel empty.
    let texels = [0, 0, 0, 255, 0, 0, 64, 255, 0, 0, 192, 255, 0, 0, 0, 0];
    let cut = meter_cut(&texels, &s, 0.5);
    assert_eq!(&cut[0..4], &[0, 255, 0, 255], "lit: the gradient's minimum");
    assert_eq!(&cut[4..8], &[64, 191, 0, 255], "lit: a quarter along the gradient");
    assert_eq!(&cut[8..12], &[102, 102, 255, 255], "beyond the value: the background");
    assert_eq!(&cut[12..16], &[0, 0, 0, 0], "no meter there");
}
