//! The shader readers on synthetic tags: test case (a) of
//! specs/halo_ce/shaders.md (the environment and transparent chicago
//! parses with their expected texture transforms, the periodic functions'
//! numeric cases and tables), then a synthetic tag per remaining class,
//! the class dispatch and the rejections.

use blam_formats::cache::{MapFile, TagStruct, class};
use blam_formats::shader::PeriodicTables;
use blam_formats::shader::{
    AnimationSource, BlendFunction, ChicagoFunction, ChicagoMap, DetailFunction, DetailLevel,
    EnvironmentReflectionType, EnvironmentShader, EnvironmentType, FadeMode, FirstMapType, GenericInput, GenericShader,
    InputMapping, MaterialType, OutputMapping, PeriodicFunction, Shader, ShaderType, StageOutput, no_object,
};

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

fn tags(shader_class: &str) -> Vec<(&'static str, &'static str)> {
    let shader_class: &'static str = Box::leak(shader_class.to_string().into_boxed_str());
    vec![
        ("scnr", "scenarios\\syn"),
        (shader_class, "shaders\\under test"),
        ("bitm", "bitmaps\\x"),
        ("bitm", "bitmaps\\y"),
        ("bitm", "bitmaps\\z"),
        ("lens", "effects\\flare"),
    ]
}
const SHADER: usize = 1;
const X: usize = 2;
const Y: usize = 3;
const Z: usize = 4;

fn finish(td: TagData) -> MapFile {
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

fn shader_of(map: &MapFile) -> blam_formats::cache::TagEntry {
    map.tags()[SHADER].clone()
}

fn name_of(map: &MapFile, r: &blam_formats::cache::TagRef) -> Option<String> {
    map.tag(r.id).map(|t| t.path.clone())
}

/// The spec's synthetic environment shader (0x344 bytes, "(a) Synthetic:
/// environment shader parse").
fn spec_environment() -> MapFile {
    let mut td = TagData::new(&tags("senv"));
    let s = Struct::new(0x344)
        .u16(0x00, 1)
        .i16(0x02, 2)
        .i16(0x22, 2)
        .i16(0x24, 3)
        .u16(0x28, 1)
        .i16(0x2A, 1)
        .put(0x88, &td.tag_ref(X))
        .i16(0xB0, 0)
        .f32(0xB4, 8.0)
        .put(0xB8, &td.tag_ref(Y))
        .f32(0xC8, 60.0)
        .put(0xCC, &null_ref("bitm"))
        .f32(0xF8, 16.0)
        .f32s(0x10C, &[1.0, 1.0, 1.0])
        .f32(0x124, 40.0)
        .f32s(0x138, &[40.0, 40.0])
        .i16(0x150, 0)
        .f32(0x154, 1.0)
        .f32(0x158, 1.0)
        .i16(0x15C, 6)
        .f32(0x160, 2.0)
        .f32(0x164, 0.5)
        .f32s(0x19C, &[1.0, 0.5, 0.0])
        .i16(0x1B4, 2)
        .f32(0x1B8, 1.0)
        .u16(0x27C, 2)
        .f32(0x290, 0.5)
        .i16(0x2D2, 1)
        .f32(0x2D4, 0.25)
        .f32(0x2F4, 0.35)
        .f32(0x2F8, 0.25);
    td.set_struct(SHADER, &s.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    finish(td)
}

#[test]
fn spec_environment_parse() {
    let map = spec_environment();
    let tag = shader_of(&map);
    let Shader::Environment(e) = Shader::read(&map, &tag).unwrap() else {
        panic!("not an environment shader");
    };
    let h = &e.header;
    assert_eq!(h.radiosity_flags, 1);
    assert_eq!(h.detail_level, DetailLevel::Low);
    assert_eq!(h.material_type, MaterialType(2));
    assert_eq!(h.shader_type, ShaderType::Environment);
    assert_eq!((h.power, h.physics_flags), (0.0, 0));
    assert_eq!(e.flags, EnvironmentShader::ALPHA_TESTED);
    assert_eq!(e.environment_type, EnvironmentType::Blended);
    assert_eq!(name_of(&map, &e.base_map).as_deref(), Some("bitmaps\\x"));
    assert_eq!(e.detail_function, DetailFunction::DoubleBiasedMultiply);
    assert_eq!(e.primary_detail_scale, 8.0);
    assert_eq!(name_of(&map, &e.primary_detail_map).as_deref(), Some("bitmaps\\y"));
    assert_eq!(e.secondary_detail_scale, 60.0);
    assert!(e.secondary_detail_map.is_null());
    assert_eq!(e.micro_detail_scale, 16.0);
    assert_eq!(
        (e.material_color.r, e.material_color.g, e.material_color.b),
        (1.0, 1.0, 1.0)
    );
    assert_eq!(e.bump_map_scale, 40.0);
    assert_eq!(e.runtime_bump_scale, [40.0, 40.0]);
    assert_eq!(e.u_animation.function, PeriodicFunction::One);
    assert_eq!((e.u_animation.period, e.u_animation.scale), (1.0, 1.0));
    assert_eq!(e.v_animation.function, PeriodicFunction::Slide);
    assert_eq!((e.v_animation.period, e.v_animation.scale), (2.0, 0.5));
    let on = e.primary_self_illumination.on_color;
    assert_eq!((on.r, on.g, on.b), (1.0, 0.5, 0.0));
    assert_eq!(e.primary_self_illumination.function, PeriodicFunction::Cosine);
    assert_eq!(e.primary_self_illumination.period, 1.0);
    assert_eq!(e.specular_flags, EnvironmentShader::SPECULAR_EXTRA_SHINY);
    assert_eq!(e.specular_brightness, 0.5);
    assert_eq!(e.reflection_type, EnvironmentReflectionType::FlatCubeMap);
    assert_eq!(e.lightmap_brightness_scale, 0.25);
    assert_eq!((e.perpendicular_brightness, e.parallel_brightness), (0.35, 0.25));
    // Every other field is zero.
    assert_eq!(
        (e.lens_flare_spacing, e.diffuse_flags, e.self_illumination_flags),
        (0.0, 0, 0)
    );
    assert_eq!(e.micro_detail_function, DetailFunction::DoubleBiasedMultiply);
    assert_eq!((e.mirror_index_of_refraction, e.mirror_depth), (0.0, 0.0));
    assert_eq!(e.specular_perpendicular_color.r, 0.0);
    assert_eq!(e.secondary_self_illumination.period, 0.0);

    // The animation offset at t = 1 s: du = 1.0, dv = 0.5 x slide(0.5).
    let [du, dv] = e.uv_offset(&PeriodicTables::new(), 1.0);
    assert_eq!(du, 1.0);
    assert!((dv - 0.1737).abs() < 1.0 / 255.0, "dv {dv}");
    assert_eq!(e.bitmap_references_len(), 7);
}

trait Count {
    fn bitmap_references_len(&self) -> usize;
}
impl Count for EnvironmentShader {
    fn bitmap_references_len(&self) -> usize {
        Shader::Environment(self.clone()).bitmap_references().len()
    }
}

/// The spec's synthetic chicago shader: a 0x6C main buffer and two 0xDC map
/// elements.
fn spec_chicago(extra: impl FnOnce(&mut Struct)) -> MapFile {
    let mut td = TagData::new(&tags("schi"));
    let map0 = Struct::new(0xDC)
        .u16(0x00, 0x0C)
        .i16(0x2C, 2)
        .i16(0x2E, 11)
        .f32(0x54, 2.0)
        .f32(0x58, 1.0)
        .f32(0x64, 90.0)
        .put(0x6C, &td.tag_ref(Z))
        .f32(0xA4 + 0x2C, 360.0);
    // Rotation animation function "one" (code 0) is the zero default.
    let map1 = Struct::new(0xDC)
        .f32(0x54, 1.0)
        .f32(0x58, 1.0)
        .put(0x6C, &null_ref("bitm"));
    let maps = td.block(&[map0.into(), map1.into()]);
    let mut main = Struct::new(0x6C)
        .i16(0x24, 6)
        .u8(0x28, 0)
        .u8(0x29, 0x05)
        .i16(0x2A, 0)
        .i16(0x2C, 3)
        .i16(0x2E, 2)
        .i16(0x30, 1)
        .put(0x54, &maps)
        .u32(0x60, 1);
    extra(&mut main);
    td.set_struct(SHADER, &main.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    finish(td)
}

#[test]
fn spec_chicago_parse_and_transform() {
    let map = spec_chicago(|_| {});
    let Shader::TransparentChicago(s) = Shader::read(&map, &shader_of(&map)).unwrap() else {
        panic!("not a chicago shader");
    };
    assert_eq!(s.header.shader_type, ShaderType::TransparentChicago);
    let c = &s.common;
    assert_eq!(c.numeric_counter_limit, 0);
    assert_eq!(c.flags, 0x05);
    assert_eq!(c.first_map_type, FirstMapType::Map2d);
    assert_eq!(c.framebuffer_blend_function, BlendFunction::Add);
    assert_eq!(c.framebuffer_fade_mode, FadeMode::WhenParallel);
    assert_eq!(c.framebuffer_fade_source, AnimationSource::A);
    assert!(c.extra_layers.is_empty());
    assert_eq!(s.maps.len(), 2);
    assert_eq!(s.extra_flags, 1);

    let (m0, m1) = (&s.maps[0], &s.maps[1]);
    assert_eq!(m0.flags, ChicagoMap::U_CLAMPED | ChicagoMap::V_CLAMPED);
    assert_eq!(m0.color_function, ChicagoFunction::Multiply);
    assert_eq!(m0.alpha_function, ChicagoFunction::BlendNextMapAlpha);
    assert_eq!((m0.u_scale, m0.v_scale, m0.rotation), (2.0, 1.0, 90.0));
    assert_eq!(name_of(&map, &m0.map).as_deref(), Some("bitmaps\\z"));
    assert_eq!(m0.animation.rotation.scale, 360.0);
    assert_eq!(m0.animation.rotation.function, PeriodicFunction::One);
    assert_eq!(
        (m1.color_function, m1.alpha_function),
        (ChicagoFunction::Current, ChicagoFunction::Current)
    );
    assert_eq!((m1.u_scale, m1.v_scale), (1.0, 1.0));
    assert!(m1.map.is_null());

    // Texture transform of map 0 at t = 0: theta = 360 + 90 = 450 degrees,
    // c = 0, s = 1: (0.25, 0.5) maps to (-0.5, 0.5).
    let t = m0.texture_transform(&PeriodicTables::new(), 0.0, &no_object);
    let [u, v] = t.apply(0.25, 0.5);
    assert!((u + 0.5).abs() < 1e-5 && (v - 0.5).abs() < 1e-5, "({u}, {v})");
    // Map 1 at t = 0: the identity.
    assert_eq!(
        m1.texture_transform(&PeriodicTables::new(), 0.0, &no_object)
            .apply(0.25, 0.5),
        [0.25, 0.5]
    );

    // One reference per map, in map order.
    let refs = Shader::TransparentChicago(s).bitmap_references();
    assert_eq!(refs.len(), 2);
}

#[test]
fn transparent_limits_are_enforced() {
    // Five maps are over the maximum of four.
    let mut td = TagData::new(&tags("schi"));
    let elems: Vec<Vec<u8>> = (0..5).map(|_| Struct::new(0xDC).0).collect();
    let maps = td.block(&elems);
    td.set_struct(SHADER, &Struct::new(0x6C).put(0x54, &maps).0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let e = Shader::read(&map, &shader_of(&map)).unwrap_err().to_string();
    assert!(
        e.contains("maps: 5 elements") && e.contains("shaders\\under test"),
        "{e}"
    );

    // An enumeration out of range names the tag.
    let map = spec_chicago(|m| {
        m.put_mut(0x2C, &8i16.to_le_bytes());
    });
    let e = Shader::read(&map, &shader_of(&map)).unwrap_err().to_string();
    assert!(e.contains("BlendFunction"), "{e}");

    // A class that is not a shader class.
    let mut td = TagData::new(&tags("bitm"));
    td.set_struct(SHADER, &Struct::new(0x100).0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let e = Shader::read(&map, &shader_of(&map)).unwrap_err().to_string();
    assert!(e.contains("not a shader class"), "{e}");
}

#[test]
fn generic_shader_maps_and_stages() {
    let mut td = TagData::new(&tags("sotr"));
    let map0 = Struct::new(0x64)
        .u16(0, 1)
        .f32(4, 3.0)
        .f32(8, 4.0)
        .f32(0x0C, 0.5)
        .f32(0x10, 0.25)
        .f32(0x14, 45.0)
        .put(0x1C, &td.tag_ref(X))
        .i16(0x2C + 0x10, 3)
        .i16(0x2C + 0x12, 2)
        .f32(0x2C + 0x14, 4.0);
    let maps = td.block(&[map0.into()]);
    let stage = Struct::new(0x70)
        .u16(0, 4)
        .i16(0x04, 1)
        .i16(0x06, 6)
        .f32(0x08, 10.0)
        .f32s(0x0C, &[0.35, 0.4, 0.0, 0.0])
        .f32s(0x1C, &[1.0, 1.0, 0.0, 0.0])
        .f32s(0x2C, &[0.5, 0.1, 0.2, 0.3])
        .i16(0x3C, 5)
        .i16(0x3E, 6)
        .i16(0x40, 13)
        .i16(0x42, 7)
        .i16(0x44, 24)
        .i16(0x46, 2)
        .i16(0x48, 0)
        .i16(0x4A, 1)
        .i16(0x4C, 1)
        .i16(0x4E, 1)
        .i16(0x50, 5)
        .i16(0x52, 0)
        .i16(0x54, 8)
        .i16(0x56, 5)
        .i16(0x58, 15)
        .i16(0x5A, 3)
        .i16(0x68, 1)
        .i16(0x6A, 2)
        .i16(0x6C, 0)
        .i16(0x6E, 4);
    let stages = td.block(&[stage.into()]);
    let main = Struct::new(0x6C)
        .i16(0x24, 5)
        .u8(0x29, 0x04)
        .i16(0x2C, 3)
        .i16(0x2E, 2)
        .put(0x54, &maps)
        .put(0x60, &stages);
    td.set_struct(SHADER, &main.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let Shader::TransparentGeneric(s) = Shader::read(&map, &shader_of(&map)).unwrap() else {
        panic!("not a generic shader");
    };
    assert_eq!(s.common.flags & 4, 4);
    assert_eq!(s.maps.len(), 1);
    let m = &s.maps[0];
    assert_eq!(
        (m.flags, m.u_scale, m.v_scale, m.u_offset, m.v_offset, m.rotation),
        (1, 3.0, 4.0, 0.5, 0.25, 45.0)
    );
    assert_eq!(m.animation.rotation.function, PeriodicFunction::One);
    assert_eq!(
        (m.animation.rotation.source, m.animation.rotation.scale),
        (AnimationSource::None, 0.0)
    );
    assert_eq!(m.animation.v.source, AnimationSource::C);
    assert_eq!(m.animation.v.function, PeriodicFunction::Cosine);
    assert_eq!(s.stages.len(), 1);
    let st = &s.stages[0];
    assert_eq!(
        st.flags,
        blam_formats::shader::GenericStage::A_OUT_CONTROLS_CONSTANT_COLOR0_ANIMATION
    );
    assert_eq!(st.constant_color0_source, AnimationSource::A);
    assert_eq!(st.constant_color0_function, PeriodicFunction::Slide);
    assert_eq!(st.constant_color0_period, 10.0);
    assert_eq!(
        (
            st.constant_color0_lower.a,
            st.constant_color0_lower.r,
            st.constant_color0_lower.g
        ),
        (0.35, 0.4, 0.0)
    );
    assert_eq!(st.constant_color0_upper.a, 1.0);
    assert_eq!((st.constant_color1.a, st.constant_color1.b), (0.5, 0.3));
    assert_eq!(st.color_inputs.a.input, GenericInput::Map0);
    assert_eq!(st.color_inputs.a.mapping, InputMapping::Identity);
    assert_eq!(st.color_inputs.b.input, GenericInput::Constant0);
    assert_eq!(st.color_inputs.b.mapping, InputMapping::Negate);
    assert_eq!(st.color_inputs.c.input, GenericInput::Constant1Alt);
    assert_eq!(st.color_inputs.c.mapping, InputMapping::ExpandSigned);
    assert_eq!(st.color_inputs.d.input, GenericInput::Zero);
    assert_eq!(st.color_inputs.d.mapping, InputMapping::OneMinusClamp);
    assert_eq!(st.color_outputs.ab, StageOutput::Scratch0);
    assert_eq!(
        st.color_outputs.ab_function,
        blam_formats::shader::OutputFunction::DotProduct
    );
    assert_eq!(st.color_outputs.cd, StageOutput::Map0);
    assert_eq!(st.color_outputs.mux_sum, StageOutput::Map3);
    assert_eq!(st.color_outputs.mapping, OutputMapping::ExpandSigned);
    assert_eq!(st.alpha_inputs.a.input, GenericInput::Map0Alt);
    assert_eq!(st.alpha_inputs.a.mapping, InputMapping::OneMinusExpandSigned);
    assert_eq!(st.alpha_outputs.ab, StageOutput::Scratch0);
    assert_eq!(st.alpha_outputs.cd, StageOutput::Scratch1);
    assert_eq!(st.alpha_outputs.mapping, OutputMapping::BiasMinusHalf);
}

#[test]
fn other_classes_read_their_layouts() {
    // Glass.
    let mut td = TagData::new(&tags("sgla"));
    let s = Struct::new(0x1E0)
        .i16(0x24, 8)
        .u16(0x28, 4)
        .f32s(0x54, &[0.5, 0.6, 0.9])
        .i16(0x8A, 1)
        .f32s(0x8C, &[0.7, 0.1, 0.2, 0.3])
        .f32s(0x9C, &[0.4, 0.5, 0.6, 0.7])
        .put(0xAC, &td.tag_ref(X))
        .f32(0xBC, 3.0)
        .put(0xC0, &null_ref("bitm"))
        .f32(0x154, 2.0)
        .put(0x158, &td.tag_ref(Y))
        .f32(0x168, 5.0)
        .put(0x16C, &null_ref("bitm"));
    td.set_struct(SHADER, &s.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let Shader::Glass(g) = Shader::read(&map, &shader_of(&map)).unwrap() else {
        panic!()
    };
    assert_eq!(g.flags, blam_formats::shader::GlassShader::TWO_SIDED);
    assert_eq!(g.background_tint_color.b, 0.9);
    assert_eq!(
        g.reflection_type,
        blam_formats::shader::GlassReflectionType::FlatCubeMap
    );
    assert_eq!((g.perpendicular_color.a, g.perpendicular_color.r), (0.7, 0.1));
    assert_eq!((g.parallel_color.a, g.parallel_color.b), (0.4, 0.7));
    assert_eq!(name_of(&map, &g.reflection_map).as_deref(), Some("bitmaps\\x"));
    assert_eq!(
        (g.bump_map_scale, g.diffuse_map_scale, g.diffuse_detail_map_scale),
        (3.0, 2.0, 5.0)
    );
    assert_eq!(name_of(&map, &g.diffuse_map).as_deref(), Some("bitmaps\\y"));
    assert_eq!(Shader::Glass(g).bitmap_references().len(), 7);

    // Meter.
    let mut td = TagData::new(&tags("smet"));
    let s = Struct::new(0x104)
        .i16(0x24, 9)
        .u16(0x28, 0x09)
        .put(0x4C, &td.tag_ref(X))
        .f32s(0x7C, &[0.1, 0.2, 0.3])
        .f32s(0x88, &[0.4, 0.5, 0.6])
        .f32s(0x94, &[0.0, 0.0, 1.0])
        .f32s(0xA0, &[1.0, 1.0, 0.0])
        .f32s(0xAC, &[0.9, 0.9, 0.9])
        .f32(0xB8, 0.75)
        .f32(0xBC, 0.25)
        .i16(0xD8, 1)
        .i16(0xDA, 2)
        .i16(0xDC, 3)
        .i16(0xDE, 4)
        .i16(0xE0, 0);
    td.set_struct(SHADER, &s.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let Shader::Meter(m) = Shader::read(&map, &shader_of(&map)).unwrap() else {
        panic!()
    };
    assert_eq!(m.flags, 9);
    assert_eq!(
        (m.gradient_min_color.g, m.gradient_max_color.b, m.background_color.b),
        (0.2, 0.6, 1.0)
    );
    assert_eq!((m.meter_transparency, m.background_transparency), (0.75, 0.25));
    assert_eq!(
        [
            m.meter_brightness_source,
            m.flash_brightness_source,
            m.value_source,
            m.gradient_source,
            m.flash_extension_source
        ],
        [
            AnimationSource::A,
            AnimationSource::B,
            AnimationSource::C,
            AnimationSource::D,
            AnimationSource::None
        ]
    );
    assert_eq!(name_of(&map, &m.meter_map).as_deref(), Some("bitmaps\\x"));

    // Plasma.
    let mut td = TagData::new(&tags("spla"));
    let s = Struct::new(0x14C)
        .i16(0x24, 10)
        .i16(0x2C, 1)
        .f32(0x30, 2.0)
        .i16(0x34, 2)
        .f32(0x38, 0.1)
        .f32(0x3C, 3.0)
        .f32(0x60, 0.9)
        .f32s(0x64, &[0.1, 0.2, 0.3])
        .f32(0x70, 0.8)
        .i16(0x80, 4)
        .u32(0xBC, 0x600)
        .f32(0xC0, 7.0)
        .f32s(0xC4, &[1.0, 0.0, 0.5])
        .f32(0xD0, 4.0)
        .put(0xD4, &td.tag_ref(X))
        .f32(0x108, 9.0)
        .f32s(0x10C, &[0.0, 1.0, 0.0])
        .f32(0x118, 6.0)
        .put(0x11C, &td.tag_ref(Y));
    td.set_struct(SHADER, &s.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let Shader::Plasma(p) = Shader::read(&map, &shader_of(&map)).unwrap() else {
        panic!()
    };
    assert_eq!(
        (p.intensity_source, p.offset_source, p.tint_color_source),
        (AnimationSource::A, AnimationSource::B, AnimationSource::D)
    );
    assert_eq!(
        (p.intensity_exponent, p.offset_amount, p.offset_exponent),
        (2.0, 0.1, 3.0)
    );
    assert_eq!(
        (
            p.perpendicular_brightness,
            p.perpendicular_tint_color.b,
            p.parallel_brightness
        ),
        (0.9, 0.3, 0.8)
    );
    assert_eq!(
        (p.primary_noise_period, p.primary_noise_direction, p.primary_noise_scale),
        (7.0, [1.0, 0.0, 0.5], 4.0)
    );
    assert_eq!(
        (
            p.secondary_noise_period,
            p.secondary_noise_direction,
            p.secondary_noise_scale
        ),
        (9.0, [0.0, 1.0, 0.0], 6.0)
    );
    assert_eq!(name_of(&map, &p.secondary_noise_map).as_deref(), Some("bitmaps\\y"));

    // Water.
    let mut td = TagData::new(&tags("swat"));
    let ripple = |c: f32, map_index: i16| -> Vec<u8> {
        Struct::new(0x4C)
            .f32(0x04, c)
            .f32(0x28, 4.712)
            .f32(0x2C, 0.03)
            .f32s(0x30, &[0.25, 0.5])
            .i16(0x38, 2)
            .i16(0x3A, map_index)
            .into()
    };
    let ripples = td.block(&[ripple(0.3, 0), ripple(0.3, 1), ripple(0.3, 2), ripple(0.3, 3)]);
    let s = Struct::new(0x140)
        .i16(0x24, 7)
        .u16(0x28, 5)
        .put(0x4C, &td.tag_ref(X))
        .f32s(0x6C, &[0.5, 0.1, 0.2, 0.3])
        .f32s(0x7C, &[0.6, 0.4, 0.5, 0.6])
        .put(0x9C, &td.tag_ref(Y))
        .f32(0xBC, 4.712)
        .f32(0xC0, 0.03)
        .f32(0xC4, 220.0)
        .put(0xC8, &td.tag_ref(Z))
        .i16(0xD8, 4)
        .f32(0xDC, 0.95)
        .f32(0xE0, 0.6)
        .put(0x124, &ripples);
    td.set_struct(SHADER, &s.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let Shader::Water(w) = Shader::read(&map, &shader_of(&map)).unwrap() else {
        panic!()
    };
    assert_eq!(w.flags, 5);
    assert_eq!(
        (w.view_perpendicular_tint_color.a, w.view_parallel_tint_color.b),
        (0.5, 0.6)
    );
    assert_eq!(
        (w.ripple_animation_angle, w.ripple_scale, w.ripple_mipmap_levels),
        (4.712, 220.0, 4)
    );
    assert_eq!((w.ripple_mipmap_fade_factor, w.ripple_mipmap_detail_bias), (0.95, 0.6));
    assert_eq!(w.ripples.len(), 4);
    assert_eq!(w.ripples[3].map_index, 3);
    assert_eq!(
        (
            w.ripples[0].contribution_factor,
            w.ripples[0].map_offset,
            w.ripples[0].map_repeats
        ),
        (0.3, [0.25, 0.5], 2)
    );
    assert_eq!(Shader::Water(w).bitmap_references().len(), 3);

    // Model.
    let mut td = TagData::new(&tags("soso"));
    let s = Struct::new(0x1B8)
        .i16(0x24, 4)
        .u16(0x28, 2)
        .f32(0x38, 0.5)
        .i16(0x4C, 3)
        .i16(0x70, 1)
        .i16(0x72, 2)
        .f32(0x74, 3.0)
        .f32s(0x78, &[0.1, 0.2, 0.3])
        .f32(0x9C, 2.0)
        .f32(0xA0, 4.0)
        .put(0xA4, &td.tag_ref(X))
        .put(0xBC, &td.tag_ref(Y))
        .i16(0xD4, 2)
        .i16(0xD6, 8)
        .f32(0xD8, 16.0)
        .put(0xDC, &td.tag_ref(Z))
        .f32(0xEC, 0.5)
        .f32(0xFC + 0x04, 5.0)
        .f32(0x13C, 12.0)
        .f32(0x144, 0.35)
        .f32s(0x148, &[0.3, 0.4, 0.5])
        .put(0x164, &null_ref("bitm"));
    td.set_struct(SHADER, &s.0);
    td.set_struct(0, &Struct::new(0x5B0).0);
    let map = finish(td);
    let Shader::Model(m) = Shader::read(&map, &shader_of(&map)).unwrap() else {
        panic!()
    };
    assert_eq!(
        (m.flags, m.translucency, m.change_color_source),
        (2, 0.5, AnimationSource::C)
    );
    assert_eq!(m.self_illumination_function, PeriodicFunction::Cosine);
    assert_eq!(
        (m.self_illumination_period, m.self_illumination_color_lower.b),
        (3.0, 0.3)
    );
    assert_eq!((m.map_u_scale, m.map_v_scale), (2.0, 4.0));
    assert_eq!(m.detail_function, DetailFunction::DoubleBiasedAdd);
    assert_eq!(m.detail_mask, blam_formats::shader::ModelDetailMask::MultipurposeAlpha);
    assert_eq!((m.detail_map_scale, m.detail_map_v_scale), (16.0, 0.5));
    assert_eq!(m.texture_animation.u.period, 5.0);
    assert_eq!(
        (m.reflection_falloff_distance, m.perpendicular_brightness),
        (12.0, 0.35)
    );
    assert_eq!(Shader::Model(m).bitmap_references().len(), 4);
}

#[test]
fn class_dispatch_and_sizes() {
    let sizes = [
        (
            class::SHADER_ENVIRONMENT,
            <blam_formats::shader::EnvironmentShader as TagStruct>::SIZE,
            0x344,
        ),
        (
            class::SHADER_MODEL,
            <blam_formats::shader::ModelShader as TagStruct>::SIZE,
            0x1B8,
        ),
        (
            class::SHADER_TRANSPARENT_CHICAGO,
            <blam_formats::shader::ChicagoShader as TagStruct>::SIZE,
            0x6C,
        ),
        (
            class::SHADER_TRANSPARENT_GENERIC,
            <GenericShader as TagStruct>::SIZE,
            0x6C,
        ),
        (
            class::SHADER_TRANSPARENT_GLASS,
            <blam_formats::shader::GlassShader as TagStruct>::SIZE,
            0x1E0,
        ),
        (
            class::SHADER_TRANSPARENT_METER,
            <blam_formats::shader::MeterShader as TagStruct>::SIZE,
            0x104,
        ),
        (
            class::SHADER_TRANSPARENT_PLASMA,
            <blam_formats::shader::PlasmaShader as TagStruct>::SIZE,
            0x14C,
        ),
        (
            class::SHADER_TRANSPARENT_WATER,
            <blam_formats::shader::WaterShader as TagStruct>::SIZE,
            0x140,
        ),
    ];
    for (class, got, want) in sizes {
        assert_eq!(got, want, "{class}");
    }
    let map = spec_environment();
    let s = Shader::read(&map, &shader_of(&map)).unwrap();
    assert_eq!(s.class(), class::SHADER_ENVIRONMENT);
    assert_eq!(s.header().shader_type, ShaderType::Environment);
}

#[test]
fn periodic_numeric_cases() {
    use PeriodicFunction::*;
    let tables = PeriodicTables::new();
    let periodic = |f, s| tables.value(f, s);
    let at = [0.0f32, 0.1, 0.25, 0.5, 1.0, 1.25, 10.0];
    let tol = 1.0 / 255.0 + 1e-4;
    let rows: [(PeriodicFunction, [f32; 7]); 6] = [
        (One, [1.0; 7]),
        (Zero, [0.0; 7]),
        (Cosine, [1.0, 0.9489, 0.7255, 0.2063, 0.3435, 0.8510, 0.9961]),
        (DiagonalWave, [0.0, 0.1366, 0.3475, 0.6988, 0.5984, 0.2471, 0.0]),
        (Slide, [0.0, 0.0664, 0.1718, 0.3475, 0.6988, 0.8745, 0.9961]),
        (Jitter, [0.6471, 0.5264, 0.5357, 0.2000, 0.5561, 0.5804, 0.5098]),
    ];
    for (f, want) in rows {
        for (s, w) in at.iter().zip(want) {
            let got = periodic(f, *s);
            assert!((got - w).abs() <= tol, "{f:?} at {s}: {got} vs {w}");
        }
    }
    // Wander is the same table as jitter.
    for s in at {
        assert_eq!(periodic(Wander, s), periodic(Jitter, s));
    }
    // The 0.7 quirk: a full cosine cycle at s = 1/0.7, the trough at 0.714286.
    assert!((periodic(Cosine, 1.0 / 0.7) - 0.9961).abs() <= tol);
    assert!(periodic(Cosine, 0.714_286).abs() <= tol);
    // A period of 0 counts as 1; phase and time add.
    assert_eq!(tables.value_at(Slide, 0.5, 0.0, 0.0), periodic(Slide, 0.5));
    assert_eq!(tables.value_at(Slide, 0.25, 2.0, 0.75), periodic(Slide, 0.5));
    // Negative input wraps through the table.
    assert!((0.0..=1.0).contains(&periodic(Cosine, -0.3)));
    assert!(
        (periodic(Cosine, -40.0) - periodic(Cosine, 0.0)).abs() < 1e-3,
        "the table wraps after 40"
    );
}

#[test]
fn periodic_tables_and_variable_functions() {
    use PeriodicFunction::*;
    let tables = PeriodicTables::new();
    let periodic = |f, s| tables.value(f, s);
    let first = |f: PeriodicFunction| tables.table(f)[..6].to_vec();
    assert_eq!(first(Cosine), [255, 253, 247, 238, 226, 210]);
    assert_eq!(first(Slide), [0, 6, 13, 20, 27, 34]);
    assert_eq!(first(DiagonalWave), [0, 13, 27, 41, 55, 69]);
    assert_eq!(first(Jitter), [165, 163, 146, 125, 116, 123]);
    assert_eq!(first(Noise), [220, 20, 6, 250, 75, 185]);
    // Tables have the full range except the slides.
    for f in [
        Cosine,
        DiagonalWave,
        Jitter,
        Noise,
        CosineVariablePeriod,
        DiagonalWaveVariablePeriod,
        Spark,
    ] {
        let t = tables.table(f);
        assert_eq!((*t.iter().min().unwrap(), *t.iter().max().unwrap()), (0, 255), "{f:?}");
    }
    // Variable-period functions depend on float summation order: 2/255.
    let tol = 2.0 / 255.0 + 1e-4;
    for (f, s, want) in [
        (Spark, 0.5, 0.3608),
        (CosineVariablePeriod, 0.25, 0.2651),
        (SlideVariablePeriod, 0.5, 0.5914),
    ] {
        let got = periodic(f, s);
        assert!((got - want).abs() <= tol, "{f:?} at {s}: {got} vs {want}");
    }
    assert_eq!(PeriodicFunction::from_code(12), None);
    assert_eq!(PeriodicFunction::ALL.len(), 12);
}
