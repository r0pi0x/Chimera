//! The shader looks' pure pieces on synthetic inputs (`game::maps::
//! materials`, specs/halo_ce/shaders.md and bitmaps.md "Cube maps"): the
//! cube-map faces and their turn into the engine's sampling frame, the
//! reflection's contrast mix and brightness ramp, the environment
//! self-illumination's colour, weights and bake, the channel masks and the
//! periodic functions' means.

use blam_formats::shader::{PeriodicFunction, PeriodicTables};
use halo_ce::game::maps::materials::{
    bake_self_illumination, channel_mask, cube_direction, cube_texel, engine_cube, halo_direction, linear,
    periodic_mean, reflection_brightness, reflection_contrast, self_illumination_mix, slide_rate,
};

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

/// bitmaps.md's table: each face's centre looks along its axis, and a
/// texel's direction reads that texel back.
#[test]
fn cube_faces_follow_the_bitmap_spec() {
    let axes = [
        [1.0, 0.0, 0.0],
        [-1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, -1.0, 0.0],
        [0.0, 0.0, 1.0],
        [0.0, 0.0, -1.0],
    ];
    for (f, axis) in axes.iter().enumerate() {
        let d = cube_direction(f, 32, 32, 64);
        let len = (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt();
        for k in 0..3 {
            assert!(close(d[k] / len, axis[k], 0.03), "face {f}: {d:?}");
        }
        for (x, y) in [(0, 0), (63, 0), (0, 63), (17, 40)] {
            assert_eq!(cube_texel(cube_direction(f, x, y, 64), 64), (f, x, y));
        }
    }
    // Face 0 (+X): right goes toward -Z, down toward -Y.
    let d = cube_direction(0, 63, 63, 64);
    assert!(d[2] < -0.9 && d[1] < -0.9);
    // Face 2 (+Y): right toward +X, down toward +Z.
    let d = cube_direction(2, 63, 63, 64);
    assert!(d[0] > 0.9 && d[2] > 0.9);
}

/// The engine samples a cube at (x, -z, y) of an engine vector, and the
/// engine's (x, y, z) is Halo's (-y, z, -x): Halo's forward (+X, the
/// engine's -Z) is sampled at +Y, so the engine's +Y face shows Halo's
/// +X face; up stays up.
#[test]
fn the_engines_cube_holds_halos_faces_turned() {
    assert_eq!(halo_direction([0.0, 1.0, 0.0]), [1.0, 0.0, 0.0]);
    assert_eq!(halo_direction([0.0, 0.0, 1.0]), [0.0, 0.0, 1.0]);
    // A cube whose faces are solid colours: face f has red 40 f.
    let n = 8;
    let faces: Vec<Vec<u8>> = (0..6u8).map(|f| [40 * f, 0, 0, 255].repeat((n * n) as usize)).collect();
    let cube = engine_cube("test".into(), &faces, n, [1.0; 3]);
    assert_eq!(cube.size, n);
    assert_eq!(cube.mips, 4);
    let red_at_centre = |f: usize| cube.faces[f][((4 * n + 4) * 4) as usize];
    // Engine +X samples Halo -Y (face 3), +Y Halo +X (0), -X Halo +Y (2),
    // -Y Halo -X (1), +Z and -Z Halo's own.
    assert_eq!(red_at_centre(0), 120);
    assert_eq!(red_at_centre(1), 80);
    assert_eq!(red_at_centre(2), 0);
    assert_eq!(red_at_centre(3), 40);
    assert_eq!(red_at_centre(4), 160);
    assert_eq!(red_at_centre(5), 200);
    // Each face holds its whole mip chain (8, 4, 2, 1).
    assert_eq!(cube.faces[0].len(), ((64 + 16 + 4 + 1) * 4) as usize);
}

/// R = K^8 (1 - tint) + K tint: black tint keeps only the brightest.
#[test]
fn the_reflection_contrast_chooses_between_k8_and_k() {
    assert!(close(reflection_contrast(0.5, 1.0), 0.5, 1e-6));
    assert!(close(reflection_contrast(0.5, 0.0), 0.5f32.powi(8), 1e-6));
    assert!(close(reflection_contrast(1.0, 0.3), 1.0, 1e-6));
    assert!(close(reflection_contrast(0.8, 0.5), 0.5 * 0.8f32.powi(8) + 0.4, 1e-6));
    // Baked into the cube on the stored values.
    let faces: Vec<Vec<u8>> = (0..6).map(|_| [128u8, 255, 0, 255].repeat(4)).collect();
    let cube = engine_cube("c".into(), &faces, 2, [0.0, 0.5, 1.0]);
    let k = 128.0f32 / 255.0;
    assert_eq!(cube.faces[0][0], (k.powi(8) * 255.0).round() as u8);
    assert_eq!(cube.faces[0][1], 255);
}

/// The brightness ramps from the perpendicular (head on) to the parallel
/// (grazing) one: the engine's fresnel ramp, tint x (R0 head on, 1
/// grazing).
#[test]
fn the_reflection_brightness_is_a_fresnel_ramp() {
    let (tint, r0) = reflection_brightness(0.7, 0.2);
    assert!(close(tint, linear(0.2), 1e-6));
    assert!(close(tint * r0, linear(0.7), 1e-6));
    // Without a parallel brightness, the perpendicular one throughout.
    assert_eq!(reflection_brightness(0.5, 0.0), (linear(0.5), 1.0));
}

/// SI = I.r P + I.g Q + I.b C: one lit channel gives its colour and
/// all the weight; several share it by brightness.
#[test]
fn self_illumination_colour_and_weights() {
    let (tint, w) = self_illumination_mix([[1.0, 0.05, 0.05], [0.0; 3], [0.0; 3]]);
    assert_eq!(w, [1.0, 0.0, 0.0]);
    assert!(close(tint[0], 1.0, 1e-6) && close(tint[1], linear(0.05), 1e-6));
    let (tint, w) = self_illumination_mix([[0.5, 0.5, 0.0], [0.0; 3], [0.0, 0.0, 0.5]]);
    assert!(close(w[0], 2.0 / 3.0, 1e-6) && close(w[2], 1.0 / 3.0, 1e-6));
    assert!(close(tint[2], linear(0.5), 1e-6));
    assert_eq!(self_illumination_mix([[0.0; 3]; 3]), ([0.0; 3], [0.0; 3]));
}

/// The bake writes the weighted map into the base's alpha, nearest at the
/// base coordinates; the colour stays.
#[test]
fn self_illumination_is_baked_into_the_base_alpha() {
    let mut base = [10u8, 20, 30, 255].repeat(4);
    // A 1 x 2 map: top red 200, bottom green 100 and blue 50.
    let map = [200u8, 0, 0, 0, 0, 100, 50, 0];
    bake_self_illumination(&mut base, 2, 2, &map, 1, 2, [1.0, 0.5, 0.0]);
    assert_eq!(&base[..4], &[10, 20, 30, 200]);
    assert_eq!(base[7], 200);
    assert_eq!(base[11], 50);
    assert_eq!(base[15], 50);
}

#[test]
fn a_channel_mask_fills_all_four() {
    let mut t = [1u8, 2, 3, 4, 5, 6, 7, 8];
    channel_mask(&mut t, 0);
    assert_eq!(t, [1, 1, 1, 1, 5, 5, 5, 5]);
    let mut t = [1u8, 2, 3, 4];
    channel_mask(&mut t, 3);
    assert_eq!(t, [4; 4]);
}

/// A periodic function's mean over its cycle: one 1, zero 0, cosine and
/// the slide about one half.
#[test]
fn periodic_means() {
    let t = PeriodicTables::new();
    assert_eq!(periodic_mean(&t, PeriodicFunction::One), 1.0);
    assert!(close(periodic_mean(&t, PeriodicFunction::Zero), 0.0, 1e-6));
    assert!(close(periodic_mean(&t, PeriodicFunction::Cosine), 0.5, 0.02));
    assert!(close(periodic_mean(&t, PeriodicFunction::Slide), 0.5, 0.03));
}

/// A slide rises by its scale every period / 0.7 seconds (the periodic
/// tables' quirk): a steady scroll of scale x 0.7 / period; the slide's
/// own table agrees over a cycle.
#[test]
fn slides_scroll_steadily() {
    assert!(close(slide_rate(PeriodicFunction::Slide, 2.0, 1.0), 0.35, 1e-6));
    assert!(close(slide_rate(PeriodicFunction::Slide, 0.0, -2.0), -1.4, 1e-6));
    assert_eq!(slide_rate(PeriodicFunction::Cosine, 2.0, 1.0), 0.0);
    let t = PeriodicTables::new();
    // Half a cycle in (period 1: 1/1.4 s), the slide is about one half.
    assert!(close(
        t.value_at(PeriodicFunction::Slide, 0.5 / 0.7, 1.0, 0.0),
        0.5,
        0.01
    ));
}
