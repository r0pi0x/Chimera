//! The shader readers on the user's Halo disc image, against section (b)
//! of specs/halo_ce/shaders.md and its "Measured facts": `bloodgulch`'s
//! class counts and four named shaders, its level geometry's shaders, and
//! a sweep that reads every shader tag of the 13 multiplayer maps (enums in
//! range, every map reference resolving to a bitmap). Each test passes with
//! a note when there is no image.

use blam_formats::cache::{MapFile, RealRgb, TagRef, class};
use blam_formats::shader::{
    AnimationSource, BlendFunction, DetailFunction, DetailLevel, EnvironmentReflectionType, EnvironmentType, FadeMode,
    MaterialType, PeriodicFunction, Shader, ShaderType,
};

use super::maps::{MULTIPLAYER, load};

fn near(a: f32, b: f32, what: &str) {
    assert!((a - b).abs() <= 1e-3, "{what}: {a} vs {b}");
}

fn rgb(c: RealRgb) -> [f32; 3] {
    [c.r, c.g, c.b]
}

fn shader(map: &MapFile, c: &str, path: &str) -> Shader {
    let class = blam_formats::cache::ClassCode::from_text(c);
    let tag = map.find(class, path).unwrap_or_else(|| panic!("no {c} tag `{path}`"));
    Shader::read(map, tag).unwrap()
}

fn name(map: &MapFile, r: &TagRef) -> Option<String> {
    if r.is_null() {
        None
    } else {
        Some(
            map.tag(r.id)
                .unwrap_or_else(|| panic!("{r:?} resolves to no tag"))
                .path
                .clone(),
        )
    }
}

#[test]
fn bloodgulch_class_counts() {
    let Some(map) = load("bloodgulch") else { return };
    for (c, n) in [
        (class::SHADER_ENVIRONMENT, 14),
        (class::SHADER_MODEL, 106),
        (class::SHADER_TRANSPARENT_GENERIC, 26),
        (class::SHADER_TRANSPARENT_CHICAGO, 20),
        (class::SHADER_TRANSPARENT_GLASS, 6),
        (class::SHADER_TRANSPARENT_METER, 7),
        (class::SHADER_TRANSPARENT_PLASMA, 1),
        (class::SHADER_TRANSPARENT_WATER, 0),
    ] {
        assert_eq!(map.of_class(c).count(), n, "{c}");
        for t in map.of_class(c) {
            assert_eq!(Shader::read(&map, t).unwrap().class(), c);
        }
    }
    assert_eq!(map.of_class(class::SHADER).count(), 0, "no bare shdr tags");
}

#[test]
fn bloodgulch_named_shaders() {
    let Some(map) = load("bloodgulch") else { return };

    let Shader::Environment(e) = shader(&map, "senv", "levels\\test\\bloodgulch\\shaders\\blood ground") else {
        panic!()
    };
    let h = &e.header;
    assert_eq!(h.shader_type, ShaderType::Environment);
    assert_eq!(
        (h.material_type, h.detail_level, h.radiosity_flags),
        (MaterialType(1), DetailLevel::Low, 1)
    );
    assert_eq!((e.environment_type, e.flags), (EnvironmentType::Blended, 0));
    assert_eq!(
        name(&map, &e.base_map).as_deref(),
        Some("levels\\test\\bloodgulch\\bitmaps\\blood ground")
    );
    assert_eq!(e.detail_function, DetailFunction::DoubleBiasedMultiply);
    assert_eq!(e.primary_detail_scale, 100.0);
    assert_eq!(
        name(&map, &e.primary_detail_map).as_deref(),
        Some("levels\\b30\\bitmaps\\detail sand")
    );
    assert_eq!(e.secondary_detail_scale, 60.0);
    assert_eq!(
        name(&map, &e.secondary_detail_map).as_deref(),
        Some("levels\\a30\\bitmaps\\detail grass")
    );
    assert_eq!(e.micro_detail_function, DetailFunction::DoubleBiasedMultiply);
    assert_eq!(e.micro_detail_scale, 12.0);
    assert_eq!(
        name(&map, &e.micro_detail_map).as_deref(),
        Some("levels\\b30\\bitmaps\\detail ground")
    );
    assert!(e.bump_map.is_null());
    assert_eq!(e.runtime_bump_scale, [1.0, 1.0]);
    assert_eq!(rgb(e.material_color), [1.0, 1.0, 1.0]);
    assert_eq!(e.v_animation.scale, -1.0);
    assert_eq!(e.specular_brightness, 0.0);
    assert!(e.reflection_cube_map.is_null());

    let Shader::Environment(e) = shader(&map, "senv", "levels\\test\\bloodgulch\\shaders\\cap_cliff01b") else {
        panic!()
    };
    assert_eq!(
        (e.header.material_type, e.header.detail_level),
        (MaterialType(2), DetailLevel::Low)
    );
    assert_eq!((e.environment_type, e.flags), (EnvironmentType::Normal, 0));
    assert_eq!(
        name(&map, &e.base_map).as_deref(),
        Some("levels\\test\\bloodgulch\\bitmaps\\blood cliff rock")
    );
    assert_eq!(e.primary_detail_scale, 8.0);
    assert_eq!(
        name(&map, &e.primary_detail_map).as_deref(),
        Some("levels\\b30\\bitmaps\\detail cliff rock")
    );
    assert!(e.secondary_detail_map.is_null());
    assert_eq!(e.secondary_detail_scale, 1.0);
    assert_eq!(e.micro_detail_scale, 60.0);
    assert_eq!(
        name(&map, &e.micro_detail_map).as_deref(),
        Some("levels\\b30\\bitmaps\\detail cliff rock smooth")
    );
    assert_eq!(e.bump_map_scale, 40.0);
    assert_eq!(
        name(&map, &e.bump_map).as_deref(),
        Some("levels\\a30\\bitmaps\\cliff rock bump")
    );
    assert_eq!(e.runtime_bump_scale, [40.0, 40.0]);
    assert!(e.reflection_cube_map.is_null());

    let Shader::TransparentGeneric(g) = shader(&map, "sotr", "scenery\\teleporter_shield\\shaders\\teleporter_shield")
    else {
        panic!()
    };
    assert_eq!(g.header.power, 10.0);
    assert_eq!(g.common.flags, 4, "two-sided");
    assert_eq!(g.common.framebuffer_blend_function, BlendFunction::Add);
    assert_eq!(g.common.framebuffer_fade_mode, FadeMode::WhenParallel);
    assert_eq!(g.maps.len(), 4);
    let plasma = "levels\\a50\\devices\\prison door\\bitmaps\\energy plasma";
    for (i, (pu, pv)) in [(10.0, 6.0), (11.0, 7.0)].into_iter().enumerate() {
        let m = &g.maps[i];
        assert_eq!(name(&map, &m.map).as_deref(), Some(plasma), "map {i}");
        assert_eq!(m.animation.u.function, PeriodicFunction::Slide, "map {i}");
        assert_eq!(m.animation.v.function, PeriodicFunction::Slide, "map {i}");
        assert_eq!((m.animation.u.period, m.animation.v.period), (pu, pv), "map {i}");
    }
    assert_eq!(g.stages.len(), 7);
    let st = &g.stages[0];
    assert_eq!(st.flags, 4);
    let (lo, hi) = (st.constant_color0_lower, st.constant_color0_upper);
    near(lo.a, 0.4, "lower a");
    near(lo.r, 0.0, "lower r");
    near(lo.g, 0.0, "lower g");
    near(lo.b, 0.35, "lower b");
    assert_eq!([hi.a, hi.r, hi.g, hi.b], [1.0, 0.0, 0.0, 1.0]);

    let Shader::Environment(e) = shader(&map, "senv", "levels\\b30\\shaders\\metal flat generic") else {
        panic!()
    };
    assert_eq!(e.header.material_type, MaterialType(7));
    assert_eq!(e.environment_type, EnvironmentType::Normal);
    assert_eq!(e.micro_detail_scale, 16.0);
    assert_eq!(
        name(&map, &e.micro_detail_map).as_deref(),
        Some("levels\\b30\\bitmaps\\detail metal sratches")
    );
    assert_eq!(
        name(&map, &e.bump_map).as_deref(),
        Some("levels\\b30\\bitmaps\\metal flat generic bump")
    );
    assert_eq!(e.bump_map_scale, 1.0);
    assert_eq!(e.specular_brightness, 1.0);
    for (got, want) in rgb(e.specular_perpendicular_color)
        .into_iter()
        .zip([0.376, 0.482, 0.651])
    {
        near(got, want, "specular perpendicular");
    }
    for (got, want) in rgb(e.specular_parallel_color).into_iter().zip([0.227, 0.427, 0.302]) {
        near(got, want, "specular parallel");
    }
    near(e.perpendicular_brightness, 0.45, "perpendicular brightness");
    near(e.parallel_brightness, 0.35, "parallel brightness");
    assert_eq!(e.reflection_type, EnvironmentReflectionType::BumpedCubeMap);
    assert_eq!(
        name(&map, &e.reflection_cube_map).as_deref(),
        Some("levels\\b30\\bitmaps\\cube map forerunner interiors")
    );
}

#[test]
fn bloodgulch_level_geometry_shaders() {
    let Some(map) = load("bloodgulch") else { return };
    let rows = [
        (
            "senv",
            "levels\\test\\bloodgulch\\shaders\\cap_cliff01b",
            "levels\\test\\bloodgulch\\bitmaps\\blood cliff rock",
        ),
        (
            "senv",
            "levels\\test\\beavercreek\\shaders\\beavercreek boulder",
            "levels\\test\\beavercreek\\bitmaps\\beavercreek boulder",
        ),
        (
            "senv",
            "levels\\test\\bloodgulch\\shaders\\blood ground",
            "levels\\test\\bloodgulch\\bitmaps\\blood ground",
        ),
        (
            "senv",
            "levels\\b30\\shaders\\metal flat generic",
            "levels\\b30\\bitmaps\\metal flat generic",
        ),
        (
            "senv",
            "levels\\b30\\shaders\\metal panels generic ext clean",
            "levels\\b30\\bitmaps\\metal panels generic ext clean",
        ),
        (
            "senv",
            "levels\\b30\\shaders\\metal panels generic ext",
            "levels\\b30\\bitmaps\\metal panels generic ext",
        ),
        (
            "senv",
            "levels\\test\\bloodgulch\\shaders\\cap_metal",
            "levels\\test\\bloodgulch\\bitmaps\\cap_metal",
        ),
        (
            "senv",
            "levels\\b30\\shaders\\metal panels ext unearthed",
            "levels\\b30\\bitmaps\\metal panels ext unearthed",
        ),
        (
            "senv",
            "levels\\test\\bloodgulch\\shaders\\cap_ramp",
            "levels\\test\\bloodgulch\\bitmaps\\cap_ramp01",
        ),
        (
            "senv",
            "levels\\test\\bloodgulch\\shaders\\cap_ramp_blue",
            "levels\\test\\bloodgulch\\bitmaps\\cap_ramp02",
        ),
        (
            "sotr",
            "levels\\test\\bloodgulch\\shaders\\cap_moss01b",
            "levels\\test\\bloodgulch\\bitmaps\\cap_moss01b",
        ),
        (
            "sotr",
            "scenery\\teleporter_shield\\shaders\\teleporter_shield",
            "levels\\a50\\devices\\prison door\\bitmaps\\energy plasma",
        ),
        (
            "sotr",
            "levels\\test\\bloodgulch\\shaders\\greenlight",
            "levels\\c10\\bitmaps\\c10_inv",
        ),
        (
            "sotr",
            "levels\\test\\bloodgulch\\shaders\\bloodgulch light red",
            "levels\\test\\bloodgulch\\bitmaps\\mp lights small strips red",
        ),
        (
            "sotr",
            "levels\\test\\bloodgulch\\shaders\\bloodgulch light yellow",
            "levels\\b30\\bitmaps\\lights small strips",
        ),
        (
            "sotr",
            "levels\\test\\bloodgulch\\shaders\\bloodgulch light blue",
            "levels\\b30\\bitmaps\\lights small strips",
        ),
        (
            "sotr",
            "levels\\test\\bloodgulch\\shaders\\bloodgulch light black",
            "levels\\test\\prisoner\\bitmaps\\black light",
        ),
    ];
    for (c, p, base) in rows {
        let s = shader(&map, c, p);
        let first = match &s {
            Shader::Environment(e) => e.base_map,
            Shader::TransparentGeneric(g) => g.maps[0].map,
            _ => unreachable!(),
        };
        assert_eq!(name(&map, &first).as_deref(), Some(base), "{p}");
    }
}

/// Per-map shader counts of the spec's table: senv, soso, sotr, schi, sgla,
/// smet, spla, swat.
const COUNTS: [(&str, [usize; 8]); 13] = [
    ("beavercreek", [15, 98, 21, 19, 6, 7, 1, 1]),
    ("bloodgulch", [14, 106, 26, 20, 6, 7, 1, 0]),
    ("boardingaction", [14, 90, 19, 19, 6, 7, 1, 0]),
    ("carousel", [12, 90, 25, 19, 7, 7, 1, 0]),
    ("chillout", [18, 89, 19, 18, 7, 7, 1, 0]),
    ("damnation", [17, 92, 21, 23, 6, 7, 1, 1]),
    ("hangemhigh", [10, 89, 17, 19, 6, 7, 1, 0]),
    ("longest", [13, 89, 20, 18, 6, 7, 1, 0]),
    ("prisoner", [11, 89, 22, 19, 7, 7, 1, 0]),
    ("putput", [15, 89, 14, 17, 6, 7, 1, 0]),
    ("ratrace", [16, 91, 21, 17, 6, 7, 1, 0]),
    ("sidewinder", [15, 93, 23, 20, 6, 7, 1, 0]),
    ("wizard", [10, 91, 19, 19, 6, 7, 1, 0]),
];

fn in_unit(c: RealRgb, what: &str) {
    for v in rgb(c) {
        assert!((0.0..=1.0).contains(&v), "{what}: colour component {v}");
    }
}

#[test]
fn every_shader_of_every_multiplayer_map_reads() {
    assert_eq!(COUNTS.map(|c| c.0), MULTIPLAYER);
    let classes = [
        class::SHADER_ENVIRONMENT,
        class::SHADER_MODEL,
        class::SHADER_TRANSPARENT_GENERIC,
        class::SHADER_TRANSPARENT_CHICAGO,
        class::SHADER_TRANSPARENT_GLASS,
        class::SHADER_TRANSPARENT_METER,
        class::SHADER_TRANSPARENT_PLASMA,
        class::SHADER_TRANSPARENT_WATER,
    ];
    let mut total = 0;
    for (name, counts) in COUNTS {
        let Some(map) = load(name) else { return };
        for (c, want) in classes.iter().zip(counts) {
            assert_eq!(map.of_class(*c).count(), want, "{name}: {c}");
        }
        for t in map.of_kind(class::SHADER) {
            total += 1;
            let s = Shader::read(&map, t).unwrap_or_else(|e| panic!("{name}: {e}"));
            let at = format!("{name}: {} `{}`", t.class, t.path);
            let h = s.header();
            let expect = match s.class() {
                class::SHADER_ENVIRONMENT => ShaderType::Environment,
                class::SHADER_MODEL => ShaderType::Model,
                class::SHADER_TRANSPARENT_GENERIC => ShaderType::TransparentGeneric,
                class::SHADER_TRANSPARENT_CHICAGO => ShaderType::TransparentChicago,
                class::SHADER_TRANSPARENT_GLASS => ShaderType::TransparentGlass,
                class::SHADER_TRANSPARENT_METER => ShaderType::TransparentMeter,
                class::SHADER_TRANSPARENT_PLASMA => ShaderType::TransparentPlasma,
                class::SHADER_TRANSPARENT_WATER => ShaderType::TransparentWater,
                other => panic!("{at}: {other}"),
            };
            assert_eq!(h.shader_type, expect, "{at}");
            in_unit(h.emitted_color, &at);
            in_unit(h.tint_color, &at);
            assert!((0..33).contains(&h.material_type.0), "{at}");
            // Every map reference is none or a bitmap, and says so itself.
            for r in s.bitmap_references() {
                if r.is_null() {
                    continue;
                }
                let target = map
                    .tag(r.id)
                    .unwrap_or_else(|| panic!("{at}: {r:?} resolves to no tag"));
                assert_eq!(target.class, class::BITMAP, "{at}: references {}", target.path);
                assert_eq!(r.class, class::BITMAP, "{at}: the reference's own class");
            }
            let lens = |r: &TagRef| {
                if !r.is_null() {
                    let t = map.tag(r.id).unwrap_or_else(|| panic!("{at}: lens flare unresolved"));
                    assert_eq!(t.class, class::LENS_FLARE, "{at}");
                }
            };
            match &s {
                Shader::Environment(e) => {
                    lens(&e.lens_flare);
                    assert!(e.runtime_bump_scale.iter().all(|&v| v >= 0.0), "{at}");
                    assert_eq!(e.detail_function, DetailFunction::DoubleBiasedMultiply, "{at}");
                    assert_eq!(e.micro_detail_function, DetailFunction::DoubleBiasedMultiply, "{at}");
                    assert_eq!(e.reflection_type, EnvironmentReflectionType::BumpedCubeMap, "{at}");
                    assert_ne!(e.environment_type, EnvironmentType::BlendedBaseSpecular, "{at}");
                    for c in [
                        e.material_color,
                        e.specular_perpendicular_color,
                        e.specular_parallel_color,
                        e.primary_self_illumination.on_color,
                        e.primary_self_illumination.off_color,
                        e.secondary_self_illumination.on_color,
                        e.plasma_self_illumination.off_color,
                    ] {
                        in_unit(c, &at);
                    }
                    for v in [
                        e.specular_brightness,
                        e.lightmap_brightness_scale,
                        e.perpendicular_brightness,
                        e.parallel_brightness,
                    ] {
                        assert!((0.0..=1.0).contains(&v), "{at}: {v}");
                    }
                    for v in [
                        e.primary_detail_scale,
                        e.secondary_detail_scale,
                        e.micro_detail_scale,
                        e.bump_map_scale,
                        e.self_illumination_map_scale,
                    ] {
                        assert!(v >= 0.0, "{at}");
                    }
                }
                Shader::Model(m) => {
                    in_unit(m.self_illumination_color_lower, &at);
                    in_unit(m.self_illumination_color_upper, &at);
                    in_unit(m.perpendicular_tint_color, &at);
                    in_unit(m.parallel_tint_color, &at);
                }
                Shader::TransparentChicago(c) => {
                    lens(&c.common.lens_flare);
                    assert!(c.common.extra_layers.is_empty(), "{at}");
                    assert_eq!(
                        c.common.first_map_type,
                        blam_formats::shader::FirstMapType::Map2d,
                        "{at}"
                    );
                    assert!(matches!(
                        c.common.framebuffer_blend_function,
                        BlendFunction::AlphaBlend | BlendFunction::Add
                    ));
                    assert!(!c.maps.is_empty() && c.maps.len() <= 4, "{at}");
                }
                Shader::TransparentGeneric(g) => {
                    lens(&g.common.lens_flare);
                    assert!(g.common.extra_layers.is_empty(), "{at}");
                    assert!(matches!(
                        g.common.framebuffer_blend_function,
                        BlendFunction::AlphaBlend | BlendFunction::Add
                    ));
                    assert!(g.maps.len() <= 4 && g.stages.len() <= 7, "{at}");
                    for st in &g.stages {
                        for c in [st.constant_color0_lower, st.constant_color0_upper, st.constant_color1] {
                            for v in [c.a, c.r, c.g, c.b] {
                                assert!((0.0..=1.0).contains(&v), "{at}: stage constant {v}");
                            }
                        }
                    }
                }
                Shader::Glass(g) => {
                    in_unit(g.background_tint_color, &at);
                    for c in [g.perpendicular_color, g.parallel_color] {
                        for v in [c.a, c.r, c.g, c.b] {
                            assert!((0.0..=1.0).contains(&v), "{at}: {v}");
                        }
                    }
                }
                Shader::Water(w) => {
                    assert_eq!(w.ripples.len(), 4, "{at}");
                    for c in [w.view_perpendicular_tint_color, w.view_parallel_tint_color] {
                        for v in [c.a, c.r, c.g, c.b] {
                            assert!((0.0..=1.0).contains(&v), "{at}: {v}");
                        }
                    }
                }
                Shader::Meter(_) | Shader::Plasma(_) => {}
            }
        }
    }
    assert_eq!(total, 2077);
}

#[test]
fn damnation_water_shader() {
    let Some(map) = load("damnation") else { return };
    let Shader::Water(w) = shader(&map, "swat", "sky\\dusk\\shaders\\water dusk") else {
        panic!()
    };
    assert_eq!(w.ripples.len(), 4);
    for r in &w.ripples {
        near(r.contribution_factor, 0.3, "ripple contribution");
    }
    assert_eq!(w.ripple_scale, 220.0);
    near(w.ripple_animation_angle, 4.712, "ripple angle");
    near(w.ripple_animation_velocity, 0.03, "ripple velocity");
    assert_eq!(w.ripple_mipmap_levels, 4);
    near(w.ripple_mipmap_fade_factor, 0.95, "fade");
    near(w.ripple_mipmap_detail_bias, 0.6, "bias");
}

#[test]
fn animation_sources_of_the_disc_are_in_range() {
    // Model shaders' change colour sources: none 1139, A 18, C 39 on
    // bloodgulch's 1196 over the disc; here bloodgulch's own.
    let Some(map) = load("bloodgulch") else { return };
    let mut sources = std::collections::BTreeMap::new();
    for t in map.of_class(class::SHADER_MODEL) {
        if let Shader::Model(m) = Shader::read(&map, t).unwrap() {
            *sources.entry(m.change_color_source.code()).or_insert(0usize) += 1;
        }
    }
    assert!(sources.keys().all(|k| [0, 1, 3].contains(k)), "{sources:?}");
    assert!(sources.contains_key(&AnimationSource::None.code()));
}
