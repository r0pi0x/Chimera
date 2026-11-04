//! Test cases from specs/cs_source/shaders_phong.md on the CPU mirror of
//! prop.wesl's Phong path (`chimera_world::map::phong`), and the material
//! the renderer builds from a `MapPhong` (`lit_prop_material`). The spec's
//! vectors are Source's (Z up); `src` turns them into engine axes (Y up).

use bevy::math::{Vec3, Vec4};
use chimera_world::map::phong::*;

fn close(a: f32, b: f32, tol: f32) -> bool {
    (a - b).abs() <= tol
}

#[track_caller]
fn near(a: f32, b: f32) {
    assert!(close(a, b, 2e-4 * b.abs().max(1.0)), "{a} != {b}");
}

/// A Source (Z-up) direction in engine axes.
fn src(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3::new(x, z, -y)
}

fn byte(b: u8) -> f32 {
    b as f32 / 255.0
}

fn srgb_byte(linear: f32) -> u8 {
    (linear.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0).round() as u8
}

fn mat(ranges: [f32; 3], boost: f32, k: f32) -> MapPhong {
    MapPhong {
        exponent: exponent_rule(0.0, Some(k), false),
        fresnel_ranges: ranges,
        boost,
        ..Default::default()
    }
}

fn pixel<'a>(n: Vec3, v: Vec3, lights: &'a [(Vec3, Vec3)]) -> PhongPixel<'a> {
    PhongPixel {
        n,
        v,
        base: Vec4::ONE,
        normal_alpha: 1.0,
        exp: Vec4::ONE,
        modulation: Vec3::ONE,
        cube: [Vec3::ZERO; 6],
        lights,
        env: None,
        selfillum: None,
        lightwarp: None,
        phong_warp: None,
    }
}

#[test]
fn routing() {
    let s = PhongSupport::default();
    let bumped = PhongInputs {
        phong: true,
        bumpmap: true,
        ..Default::default()
    };
    assert_eq!(route(bumped, s), PhongRoute::Phong);
    // No normal map, lightwarp or base-alpha mask: plain VertexLitGeneric.
    let bare = PhongInputs {
        phong: true,
        base_has_alpha: true,
        ..Default::default()
    };
    assert_eq!(route(bare, s), PhongRoute::Plain);
    // A lightwarp alone: this shader, the cheap variant when the base has
    // no alpha.
    let warp = PhongInputs {
        phong: true,
        lightwarp: true,
        ..Default::default()
    };
    assert_eq!(route(warp, s), PhongRoute::Cheap);
    assert_eq!(
        route(
            PhongInputs {
                base_has_alpha: true,
                ..warp
            },
            s
        ),
        PhongRoute::Phong
    );
    // No ps2.0b, Phong off, or mat_bumpmap 0: plain.
    for support in [
        PhongSupport { ps20b: false, ..s },
        PhongSupport { phong: false, ..s },
        PhongSupport { bumpmap: false, ..s },
    ] {
        assert_eq!(route(bumped, support), PhongRoute::Plain);
    }
    assert_eq!(route(PhongInputs { phong: false, ..bumped }, s), PhongRoute::Plain);
}

#[test]
fn fresnel_ramps() {
    let ndv = [1.0, 0.5, 0.29289, 0.2, 0.0];
    for (r, want) in [
        ([0.1, 0.5, 1.0], [0.1, 0.3, 0.5, 0.64, 1.0]),
        ([0.1, 5.0, 1.0], [0.1, 2.55, 5.0, 3.88, 1.0]),
    ] {
        for (x, w) in ndv.iter().zip(want) {
            near(fresnel_ramp(r, *x), w);
        }
    }
    for (x, w) in [(0.5, 0.3), (0.2, 0.848), (0.0, 2.0)] {
        near(fresnel_ramp([0.2, 0.4, 2.0], x), w);
    }
    near(fresnel_ramp(FRESNEL_RANGES_DEFAULT, 0.5), 0.25);
}

#[test]
fn exponent_selection() {
    let k = |rule: (f32, f32), r: f32| rule.0 + rule.1 * r;
    let r128 = byte(128);
    near(k(exponent_rule(0.0, Some(15.0), false), r128), 15.0);
    near(k(exponent_rule(0.0, None, false), r128), 75.792);
    near(k(exponent_rule(0.0, None, false), 1.0), 150.0);
    near(k(exponent_rule(100.0, None, false), r128), 51.196);
    near(k(exponent_rule(0.0, Some(0.0), false), 1.0), 150.0);
    near(k(exponent_rule(0.0, None, true), 1.0), 0.0);
}

#[test]
fn half_lambert_is_forced() {
    for (ndl, w) in [(1.0, 1.0), (0.5, 0.5625), (0.0, 0.25), (-0.5, 0.0625), (-1.0, 0.0)] {
        near(half_lambert(ndl), w);
    }
    // `$halflambert` isn't an input: the diffuse light is the same.
    let m = MapPhong::default();
    let lights = [(Vec3::Y, Vec3::ONE)];
    near(diffuse_light(&m, &[Vec3::ZERO; 6], Vec3::Y, &lights, None).x, 1.0);
}

#[test]
fn lightwarp() {
    let m = MapPhong {
        lightwarp: Some(0),
        ..Default::default()
    };
    let warp = |u: f32| {
        assert!(close(u, 0.5, 1e-6));
        Vec3::splat(byte(128))
    };
    let lights = [(Vec3::X, Vec3::ONE)];
    let d = diffuse_light(&m, &[Vec3::ZERO; 6], Vec3::Y, &lights, Some(&warp));
    near(d.x, 1.00392);
}

#[test]
fn specular_mirror_and_off_mirror() {
    let n = src(0.0, 0.0, 1.0);
    let v = src(1.0, 0.0, 1.0).normalize();
    let m = mat([0.1, 0.5, 1.0], 5.0, 15.0);
    let l = src(-1.0, 0.0, 1.0).normalize();
    let r = reflect_view(n, v);
    near(r.dot(l), 1.0);
    near(n.dot(l), 0.70711);
    near(fresnel_ramp(m.fresnel_ranges, n.dot(v)), 0.16863);
    let lights = [(l, Vec3::ONE)];
    near(shade(&m, &pixel(n, v, &lights)).specular.x, 0.59619);
    // 10 degrees off the mirror direction.
    let l = src(-0.81915, 0.0, 0.57358);
    near(r.dot(l), 0.98481);
    near(r.dot(l).powf(15.0), 0.79483);
    let lights = [(l, Vec3::ONE)];
    near(shade(&m, &pixel(n, v, &lights)).specular.x, 0.38438);
}

#[test]
fn specular_mask_and_light_behind() {
    let n = src(0.0, 0.0, 1.0);
    let v = src(1.0, 0.0, 1.0).normalize();
    let m = mat([0.1, 0.5, 1.0], 5.0, 15.0);
    let lights = [(src(-1.0, 0.0, 1.0).normalize(), Vec3::ONE)];
    let p = PhongPixel {
        normal_alpha: byte(128),
        ..pixel(n, v, &lights)
    };
    near(shade(&m, &p).specular.x, 0.29927);
    for rdl in [-1.0, 0.0, 0.5, 1.0] {
        assert_eq!(lobe(rdl, -0.2, 15.0), 0.0);
    }
    let lights = [(src(0.3, 0.0, -1.0).normalize(), Vec3::ONE)];
    assert_eq!(shade(&m, &pixel(n, v, &lights)).specular, Vec3::ZERO);
}

#[test]
fn full_pixel() {
    let n = src(0.0, 0.0, 1.0);
    let v = src(1.0, 0.0, 1.0).normalize();
    let m = mat([0.1, 0.5, 1.0], 5.0, 15.0);
    let lights = [(src(-1.0, 0.0, 1.0).normalize(), Vec3::ONE)];
    let base = byte(128).powf(2.2);
    near(base, 0.21952);
    let p = PhongPixel {
        base: Vec4::new(base, base, base, 1.0),
        cube: [Vec3::splat(0.05); 6],
        ..pixel(n, v, &lights)
    };
    near(diffuse_light(&m, &p.cube, n, &lights, None).x, 0.77855);
    let out = shade(&m, &p);
    near(out.diffuse.x, 0.17091);
    near(out.specular.x, 0.59619);
    near(out.rgb.x, 0.76710);
    assert_eq!(srgb_byte(out.rgb.x), 226);
    let out = shade(
        &m,
        &PhongPixel {
            normal_alpha: byte(128),
            ..p
        },
    );
    near(out.rgb.x, 0.47017);
    assert_eq!(srgb_byte(out.rgb.x), 181);
}

#[test]
fn specular_tints() {
    let t = tint(tint_rule(None, true, true), Vec3::new(0.5, 0.25, 0.1), 0.5);
    assert!(t.abs_diff_eq(Vec3::new(0.75, 0.625, 0.55), 1e-6));
    assert_eq!(tint_rule(None, true, false), PhongTint::White);
    assert_eq!(tint(tint_rule(Some([0.5, 0.5, 1.0]), false, false), Vec3::ONE, 1.0), Vec3::new(0.5, 0.5, 1.0));
    // All zero counts as unset.
    assert_eq!(tint_rule(Some([0.0; 3]), false, true), PhongTint::White);
}

fn rim_pixel_cube() -> [Vec3; 6] {
    let mut cube = [Vec3::ZERO; 6];
    cube[0] = Vec3::splat(0.2); // +X
    cube[2] = Vec3::splat(0.1); // +Z in Source, +Y here
    cube
}

#[test]
fn rim_ambient_only() {
    let n = src(0.0, 0.0, 1.0);
    let v = src(1.0, 0.0, 0.1).normalize();
    near(n.dot(v), 0.099504);
    near((1.0 - n.dot(v)).powi(4), 0.65755);
    let cube = rim_pixel_cube();
    near(ambient(&cube, v).x, 0.19901);
    let rim = |boost: f32| MapPhong {
        rim: Some(MapRimLight {
            exponent: rim_exponent(None),
            boost,
            mask: false,
        }),
        ..Default::default()
    };
    let p = PhongPixel {
        cube,
        ..pixel(n, v, &[])
    };
    near(shade(&rim(1.0), &p).specular.x, 0.13086);
    near(shade(&rim(3.0), &p).specular.x, 0.39258);
    // On a wall (N up component 0): nothing.
    let wall = src(1.0, 0.0, 0.0);
    let p = PhongPixel {
        cube,
        ..pixel(wall, v, &[])
    };
    assert_eq!(shade(&rim(1.0), &p).specular, Vec3::ZERO);
    assert_eq!(rim_ambient(&cube, v, wall, 0.65755, 3.0), Vec3::ZERO);
}

#[test]
fn rim_merges_by_max() {
    let (rdl, ndl, fresnel, rho) = (0.9, 0.5, 0.81089, 0.65755);
    let s = lobe(rdl, ndl, 50.0) * fresnel;
    near(s, 0.0020896);
    let rm = lobe(rdl, ndl, 2.0) * rho;
    near(rm, 0.26631);
    near(s.max(rm), 0.26631);
    assert_eq!(rim_exponent(Some(0.5)), 1.0);
    assert_eq!(rim_exponent(None), 4.0);
}

#[test]
fn envmap_terms() {
    let m = MapPhong {
        fresnel_ranges: [0.1, 0.6, 1.0],
        envmap_fresnel: 1.0,
        ..Default::default()
    };
    let f = fresnel_ramp(m.fresnel_ranges, 0.5);
    near(env_term(&m, Vec3::splat(0.2), Vec3::ONE, f, 1.0).x, 0.07);
    // The default mask is the base alpha, not inverted.
    let plain = MapPhong::default();
    let me = env_mask(&plain, 0.25, 1.0);
    near(env_term(&plain, Vec3::splat(0.2), Vec3::ONE, 1.0, me).x, 0.05);
    // `$invertphongmask` inverts it, and leaves the specular alone.
    let inverted = MapPhong {
        invert_env_mask: true,
        ..mat([0.1, 0.5, 1.0], 5.0, 15.0)
    };
    let me = env_mask(&inverted, 0.25, 1.0);
    near(env_term(&inverted, Vec3::splat(0.2), Vec3::ONE, 1.0, me).x, 0.15);
    let n = src(0.0, 0.0, 1.0);
    let v = src(1.0, 0.0, 1.0).normalize();
    let lights = [(src(-1.0, 0.0, 1.0).normalize(), Vec3::ONE)];
    let p = |m: &MapPhong| {
        shade(
            m,
            &PhongPixel {
                base: Vec4::new(1.0, 1.0, 1.0, 0.25),
                env: Some((Vec3::splat(0.2), Vec3::ONE)),
                ..pixel(n, v, &lights)
            },
        )
    };
    let normal = mat([0.1, 0.5, 1.0], 5.0, 15.0);
    near(p(&inverted).specular.x, p(&normal).specular.x);
    near(p(&inverted).env.x, 0.15);
    near(p(&normal).env.x, 0.05);
}

#[test]
fn base_alpha_phong_mask() {
    let m = MapPhong {
        base_alpha_mask: true,
        ..Default::default()
    };
    let (t, b, nv) = (Vec3::X, Vec3::Z, Vec3::Y);
    let texel = Vec3::new(byte(200), byte(90), byte(220));
    assert!(shading_normal(t, b, nv, texel, true).abs_diff_eq(nv, 1e-6));
    assert!(!shading_normal(t, b, nv, texel, false).abs_diff_eq(nv, 1e-2));
    near(specular_mask(&m, 1.0, 0.5), 0.5);
}

#[test]
fn self_illum_and_alpha() {
    let m = MapPhong::default();
    // A = 0.5 and D = 0.2: a grey ambient of 0.2, no lights.
    let p = PhongPixel {
        base: Vec4::new(0.5, 0.5, 0.5, 1.0),
        cube: [Vec3::splat(0.2); 6],
        selfillum: Some((Vec3::new(2.0, 1.0, 1.0), 1.0)),
        ..pixel(Vec3::Y, Vec3::Y, &[])
    };
    assert!(shade(&m, &p).diffuse.abs_diff_eq(Vec3::new(1.0, 0.5, 0.5), 1e-6));
    near(alpha(&m, 0.5, 0.5, false), 0.25);
    near(alpha(&m, 0.5, 0.5, true), 0.5);
}

/// The material the renderer builds: the Phong path's parameters and
/// textures in `PropParams`/`PropMaterial`.
#[test]
fn prop_material_takes_the_phong_parameters() {
    use chimera_present::render::lit_prop_material;
    use chimera_world::map::{MapData, MapDebugView, MapMesh};
    let mut m = MapMesh {
        normal_map: Some(1),
        phong: Some(Box::new(MapPhong {
            exponent: (15.0, 0.0),
            exponent_texture: Some(2),
            fresnel_ranges: [0.1, 0.5, 1.0],
            boost: 5.0,
            tint: PhongTint::Albedo,
            rim: Some(MapRimLight {
                exponent: 2.0,
                boost: 3.0,
                mask: true,
            }),
            env_mask: PhongEnvMask::SpecularMask,
            invert_env_mask: true,
            envmap_fresnel: 1.0,
            ..Default::default()
        })),
        ..Default::default()
    };
    let textures: Vec<bevy::prelude::Handle<bevy::prelude::Image>> = (0..3).map(|_| Default::default()).collect();
    let data = MapData::default();
    let p = lit_prop_material(&m, &textures, &data, MapDebugView::Normal, false);
    assert_eq!(p.params.phong, 1.0);
    assert_eq!(p.params.phong_fresnel, Vec4::new(0.1, 0.5, 1.0, 5.0));
    assert_eq!(p.params.phong_exponent, Vec4::new(15.0, 0.0, 2.0, 3.0));
    assert_eq!(p.params.phong_tint.w, 1.0);
    assert_eq!(p.params.phong_flags, Vec4::new(1.0, 1.0, 0.0, 1.0));
    assert_eq!(p.params.phong_env, Vec4::new(1.0, 1.0, 1.0, 0.0));
    assert!(p.normal_map.is_some() && p.phong_exponent.is_some());
    m.phong.as_mut().unwrap().cheap = true;
    let p = lit_prop_material(&m, &textures, &data, MapDebugView::Normal, false);
    assert_eq!(p.params.phong, 2.0);
    m.phong = None;
    let p = lit_prop_material(&m, &textures, &data, MapDebugView::Normal, false);
    assert_eq!(p.params.phong, 0.0);
    assert!(p.normal_map.is_none());
}
