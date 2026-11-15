//! The shader tags of the disc (specs/halo_ce/shaders.md): one struct per
//! class (environment `senv`, model `soso`, transparent chicago `schi`,
//! transparent generic `sotr`, glass `sgla`, meter `smet`, plasma `spla`,
//! water `swat`) with every field the spec lays out, typed enumerations,
//! the texture animation block and map references as tag references, and
//! `Shader`, one enum over the classes whose `read(map, tag)` dispatches on
//! the tag's class. Plain data: no rendering and no material conversion.
//! `PeriodicTables` (submodule `periodic`) has the spec's periodic animation
//! functions.
//!
//! - `Shader::read(map, tag)`, `Shader::header`, `Shader::bitmap_references`
//!   (every map the shader names, for a converter or a resolution check).
//! - `TextureAnimation::transform`, `EnvironmentShader::uv_offset`,
//!   `ChicagoMap::texture_transform`, `GenericMap::texture_transform`: the
//!   spec's texture transform and animation offset.
//! - Flags are the raw words with named bit constants on each struct
//!   (`EnvironmentShader::ALPHA_TESTED`, ...).
//!
//! Known shortcuts:
//! - `scex` (chicago extended) is not on the disc and is not read.
//! - Material types are a checked number (0..=32; the names are
//!   `collision.md`'s), not an enum.
//! - A shader's header type number is stored, not checked against its
//!   class (the spec: they match on every disc shader).
//! - The four-stage chicago/generic limits (4 maps, 7 stages, 4 extra
//!   layers) are checked at read time.

// `tag_struct!`'s size assertion trips this lint on a one-byte field.
#![allow(clippy::int_plus_one)]

use std::io;

use crate::cache::{ClassCode, MapFile, RealArgb, RealRgb, TagEntry, TagRef, TagStruct, View, class};
use crate::scenario::{coded_enum, nested_field};

pub mod periodic;

pub use periodic::PeriodicTables;

coded_enum! {
    /// A periodic animation function (the spec's table; evaluated by
    /// `periodic`).
    pub enum PeriodicFunction {
        One = 0,
        Zero = 1,
        Cosine = 2,
        CosineVariablePeriod = 3,
        DiagonalWave = 4,
        DiagonalWaveVariablePeriod = 5,
        Slide = 6,
        SlideVariablePeriod = 7,
        Noise = 8,
        Jitter = 9,
        Wander = 10,
        Spark = 11,
    }
}

coded_enum! {
    /// Where an animated value comes from: none (1, or white for colours)
    /// or one of the owning object's four exported functions.
    pub enum AnimationSource {
        None = 0,
        A = 1,
        B = 2,
        C = 3,
        D = 4,
    }
}

coded_enum! {
    /// How a detail map combines with the value under it.
    pub enum DetailFunction {
        /// `clamp(2 a d)`.
        DoubleBiasedMultiply = 0,
        /// `a d`.
        Multiply = 1,
        /// `clamp(a + 2 d - 1)`.
        DoubleBiasedAdd = 2,
    }
}

coded_enum! {
    /// A transparent shader's framebuffer blend function.
    pub enum BlendFunction {
        AlphaBlend = 0,
        Multiply = 1,
        DoubleMultiply = 2,
        Add = 3,
        Subtract = 4,
        ComponentMin = 5,
        ComponentMax = 6,
        AlphaMultiplyAdd = 7,
    }
}

coded_enum! {
    /// A transparent shader's framebuffer fade mode.
    pub enum FadeMode {
        None = 0,
        WhenPerpendicular = 1,
        WhenParallel = 2,
    }
}

coded_enum! {
    /// The shader type number of the common header (filled at build time).
    pub enum ShaderType {
        Screen = 0,
        Effect = 1,
        Decal = 2,
        Environment = 3,
        Model = 4,
        TransparentGeneric = 5,
        TransparentChicago = 6,
        TransparentWater = 7,
        TransparentGlass = 8,
        TransparentMeter = 9,
        TransparentPlasma = 10,
    }
}

coded_enum! {
    /// The radiosity detail level (lightmap baking).
    pub enum DetailLevel {
        High = 0,
        Medium = 1,
        Low = 2,
        Turd = 3,
    }
}

coded_enum! {
    /// Environment shader type.
    pub enum EnvironmentType {
        Normal = 0,
        Blended = 1,
        BlendedBaseSpecular = 2,
    }
}

coded_enum! {
    /// Environment shader reflection type.
    pub enum EnvironmentReflectionType {
        BumpedCubeMap = 0,
        FlatCubeMap = 1,
        BumpedRadiosity = 2,
    }
}

coded_enum! {
    /// Glass reflection type.
    pub enum GlassReflectionType {
        BumpedCubeMap = 0,
        FlatCubeMap = 1,
        DynamicMirror = 2,
    }
}

coded_enum! {
    /// What the model shader's detail map is masked by.
    pub enum ModelDetailMask {
        None = 0,
        ReflectionMaskInverse = 1,
        ReflectionMask = 2,
        SelfIlluminationMaskInverse = 3,
        SelfIlluminationMask = 4,
        ChangeColorMaskInverse = 5,
        ChangeColorMask = 6,
        MultipurposeAlphaInverse = 7,
        MultipurposeAlpha = 8,
    }
}

coded_enum! {
    /// A transparent shader's first map type.
    pub enum FirstMapType {
        Map2d = 0,
        ReflectionCubeMap = 1,
        ObjectCenteredCubeMap = 2,
        ViewerCenteredCubeMap = 3,
    }
}

coded_enum! {
    /// How a chicago map's colour (or alpha) joins the next map's.
    pub enum ChicagoFunction {
        Current = 0,
        NextMap = 1,
        Multiply = 2,
        DoubleMultiply = 3,
        Add = 4,
        AddSignedCurrent = 5,
        AddSignedNextMap = 6,
        SubtractCurrent = 7,
        SubtractNextMap = 8,
        BlendCurrentAlpha = 9,
        BlendCurrentAlphaInverse = 10,
        BlendNextMapAlpha = 11,
        BlendNextMapAlphaInverse = 12,
    }
}

coded_enum! {
    /// A generic stage input. For colour inputs 15 to 24 are the
    /// replicated alpha of the register named by the code minus 10 (map,
    /// vertex, scratch, constant); for alpha inputs 5 to 14 read alpha and
    /// 15 to 24 read the blue channel (the `*Alt` names).
    pub enum GenericInput {
        Zero = 0,
        One = 1,
        Half = 2,
        NegativeOne = 3,
        NegativeHalf = 4,
        Map0 = 5,
        Map1 = 6,
        Map2 = 7,
        Map3 = 8,
        Vertex0 = 9,
        Vertex1 = 10,
        Scratch0 = 11,
        Scratch1 = 12,
        Constant0 = 13,
        Constant1 = 14,
        Map0Alt = 15,
        Map1Alt = 16,
        Map2Alt = 17,
        Map3Alt = 18,
        Vertex0Alt = 19,
        Vertex1Alt = 20,
        Scratch0Alt = 21,
        Scratch1Alt = 22,
        Constant0Alt = 23,
        Constant1Alt = 24,
    }
}

coded_enum! {
    /// How a generic stage remaps an input before the products.
    pub enum InputMapping {
        /// `clamp(x)`.
        Clamp = 0,
        /// `1 - clamp(x)`.
        OneMinusClamp = 1,
        /// `2 clamp(x) - 1`.
        ExpandSigned = 2,
        /// `1 - 2 clamp(x)`.
        OneMinusExpandSigned = 3,
        /// `clamp(x) - 1/2`.
        ClampMinusHalf = 4,
        /// `1/2 - clamp(x)`.
        HalfMinusClamp = 5,
        /// `x`.
        Identity = 6,
        /// `-x`.
        Negate = 7,
    }
}

coded_enum! {
    /// Where a generic stage writes a result.
    pub enum StageOutput {
        Discard = 0,
        Scratch0 = 1,
        Scratch1 = 2,
        Vertex0 = 3,
        Vertex1 = 4,
        Map0 = 5,
        Map1 = 6,
        Map2 = 7,
        Map3 = 8,
    }
}

coded_enum! {
    /// How a generic stage combines two inputs into a product.
    pub enum OutputFunction {
        Multiply = 0,
        DotProduct = 1,
    }
}

coded_enum! {
    /// The scale or bias applied to a generic stage's results.
    pub enum OutputMapping {
        Identity = 0,
        Half = 1,
        Double = 2,
        Quadruple = 3,
        BiasMinusHalf = 4,
        ExpandSigned = 5,
    }
}

/// A material type number (0..=32, `collision.md`'s codes).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialType(pub i16);

/// The most material types.
pub const MATERIAL_TYPES: i16 = 33;

impl crate::cache::Field for MaterialType {
    const SIZE: usize = 2;
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
        Ok(MaterialType(v.enum16_below(offset, MATERIAL_TYPES)?))
    }
}

/// The common header's size, in bytes.
pub const HEADER_SIZE: usize = 0x28;
/// The most maps a chicago or generic shader has.
pub const MAX_TRANSPARENT_MAPS: usize = 4;
/// The most stages a generic shader has.
pub const MAX_GENERIC_STAGES: usize = 7;
/// The most extra layers a transparent shader has.
pub const MAX_EXTRA_LAYERS: usize = 4;

crate::tag_struct! {
    /// The 40-byte header every shader class starts with.
    pub struct ShaderHeader: 0x28 {
        /// Bit 0 simple parameterization, bit 1 ignore normals, bit 2
        /// transparent lit.
        0x00 radiosity_flags: u16,
        0x02 detail_level: DetailLevel,
        /// Light the surface emits for the lightmap baker.
        0x04 power: f32,
        0x08 emitted_color: RealRgb,
        0x14 tint_color: RealRgb,
        0x20 physics_flags: u16,
        0x22 material_type: MaterialType,
        0x24 shader_type: ShaderType,
    }
}

impl ShaderHeader {
    pub const SIMPLE_PARAMETERIZATION: u16 = 1;
    pub const IGNORE_NORMALS: u16 = 2;
    pub const TRANSPARENT_LIT: u16 = 4;
}

nested_field!(ShaderHeader);

crate::tag_struct! {
    /// One channel (u, v or rotation) of a texture animation (16 bytes).
    pub struct AnimationChannel: 0x10 {
        0x00 source: AnimationSource,
        0x02 function: PeriodicFunction,
        /// Seconds; 0 counts as 1.
        0x04 period: f32,
        /// Seconds.
        0x08 phase: f32,
        /// Texture units (rotation: degrees).
        0x0C scale: f32,
    }
}

nested_field!(AnimationChannel);

impl AnimationChannel {
    /// `periodic(function, (phase + t) / period) x scale x source value`.
    pub fn value(&self, tables: &PeriodicTables, t: f32, source: &dyn Fn(AnimationSource) -> f32) -> f32 {
        tables.value_at(self.function, t, self.period, self.phase) * self.scale * source(self.source)
    }
}

crate::tag_struct! {
    /// The texture animation block (0x38 bytes) of the model shader and of
    /// chicago and generic maps.
    pub struct TextureAnimation: 0x38 {
        0x00 u: AnimationChannel,
        0x10 v: AnimationChannel,
        0x20 rotation: AnimationChannel,
        /// Rotation centre (u, v).
        0x30 rotation_center: [f32; 2],
    }
}

nested_field!(TextureAnimation);

/// A texture transform: `apply` maps (u, v) to (u', v') by the spec's
/// formula.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextureTransform {
    pub scale: [f32; 2],
    /// Cosine and sine of the total rotation.
    pub cos_sin: [f32; 2],
    /// The translation terms `c U - s V + cx` and `c V + s U + cy`.
    pub translation: [f32; 2],
}

impl TextureTransform {
    pub fn apply(&self, u: f32, v: f32) -> [f32; 2] {
        let [su, sv] = self.scale;
        let [c, s] = self.cos_sin;
        let [tu, tv] = self.translation;
        [c * su * u - s * sv * v + tu, s * su * u + c * sv * v + tv]
    }
}

impl TextureAnimation {
    /// The texture transform at time `t` seconds for a map with scale,
    /// offset and rotation (degrees) `map_scale`, `map_offset`,
    /// `map_rotation` (the owner's; 1, 1, 0, 0, 0 where it has none).
    /// `source` gives an animation source's value (the owning object's
    /// function; 1 for `None` and for level geometry).
    pub fn transform(
        &self,
        tables: &PeriodicTables,
        t: f32,
        map_scale: [f32; 2],
        map_offset: [f32; 2],
        map_rotation: f32,
        source: &dyn Fn(AnimationSource) -> f32,
    ) -> TextureTransform {
        let [cx, cy] = self.rotation_center;
        let uu = map_offset[0] - cx + self.u.value(tables, t, source);
        let vv = map_offset[1] - cy + self.v.value(tables, t, source);
        let theta = self.rotation.value(tables, t, source) + map_rotation;
        let (s, c) = if theta == 0.0 {
            (0.0, 1.0)
        } else {
            theta.to_radians().sin_cos()
        };
        TextureTransform {
            scale: map_scale,
            cos_sin: [c, s],
            translation: [c * uu - s * vv + cx, c * vv + s * uu + cy],
        }
    }
}

/// A source function that reads every source as none (1): level geometry,
/// which has no owning object.
pub fn no_object(_: AnimationSource) -> f32 {
    1.0
}

crate::tag_struct! {
    /// A shader reference of an extra layers block (16 bytes).
    pub struct ExtraLayer: 16 {
        /// Class `shdr` family.
        0x00 shader: TagRef,
    }
}

crate::tag_struct! {
    /// A level surface's u or v animation (12 bytes).
    pub struct EnvironmentUvAnimation: 12 {
        0x00 function: PeriodicFunction,
        /// Seconds (never 0).
        0x04 period: f32,
        /// Texture units.
        0x08 scale: f32,
    }
}

nested_field!(EnvironmentUvAnimation);

crate::tag_struct! {
    /// One self-illumination channel of an environment shader (0x24
    /// bytes): on and off colours animated by a periodic function.
    pub struct SelfIlluminationChannel: 0x24 {
        0x00 on_color: RealRgb,
        0x0C off_color: RealRgb,
        0x18 function: PeriodicFunction,
        0x1C period: f32,
        0x20 phase: f32,
    }
}

nested_field!(SelfIlluminationChannel);

crate::tag_struct! {
    /// The environment shader (`senv`, 0x344 bytes), used on level
    /// surfaces.
    pub struct EnvironmentShader: 0x344 {
        0x000 header: ShaderHeader,
        /// Bit 0 alpha tested, bit 1 bump map is specular mask, bit 2 true
        /// atmospheric fog.
        0x028 flags: u16,
        0x02A environment_type: EnvironmentType,
        /// World units (0 = one per surface).
        0x02C lens_flare_spacing: f32,
        /// Class `lens`.
        0x030 lens_flare: TagRef,
        /// Bit 0 rescale detail maps, bit 1 rescale bump map.
        0x06C diffuse_flags: u16,
        0x088 base_map: TagRef,
        0x0B0 detail_function: DetailFunction,
        0x0B4 primary_detail_scale: f32,
        0x0B8 primary_detail_map: TagRef,
        0x0C8 secondary_detail_scale: f32,
        0x0CC secondary_detail_map: TagRef,
        0x0F4 micro_detail_function: DetailFunction,
        0x0F8 micro_detail_scale: f32,
        0x0FC micro_detail_map: TagRef,
        /// Multiplies the lightmap.
        0x10C material_color: RealRgb,
        0x124 bump_map_scale: f32,
        0x128 bump_map: TagRef,
        /// The scale the game uses (u, v).
        0x138 runtime_bump_scale: [f32; 2],
        0x150 u_animation: EnvironmentUvAnimation,
        0x15C v_animation: EnvironmentUvAnimation,
        /// Bit 0 unfiltered.
        0x180 self_illumination_flags: u16,
        0x19C primary_self_illumination: SelfIlluminationChannel,
        0x1D8 secondary_self_illumination: SelfIlluminationChannel,
        0x214 plasma_self_illumination: SelfIlluminationChannel,
        0x250 self_illumination_map_scale: f32,
        0x254 self_illumination_map: TagRef,
        /// Bit 0 overbright, bit 1 extra shiny, bit 2 lightmap is
        /// specular.
        0x27C specular_flags: u16,
        0x290 specular_brightness: f32,
        0x2A8 specular_perpendicular_color: RealRgb,
        0x2B4 specular_parallel_color: RealRgb,
        /// Bit 0 dynamic mirror.
        0x2D0 reflection_flags: u16,
        0x2D2 reflection_type: EnvironmentReflectionType,
        0x2D4 lightmap_brightness_scale: f32,
        0x2F4 perpendicular_brightness: f32,
        0x2F8 parallel_brightness: f32,
        0x30C mirror_index_of_refraction: f32,
        0x310 mirror_depth: f32,
        /// Class `bitm` (cube).
        0x324 reflection_cube_map: TagRef,
    }
}

impl EnvironmentShader {
    pub const ALPHA_TESTED: u16 = 1;
    pub const BUMP_MAP_IS_SPECULAR_MASK: u16 = 2;
    pub const TRUE_ATMOSPHERIC_FOG: u16 = 4;
    pub const RESCALE_DETAIL_MAPS: u16 = 1;
    pub const RESCALE_BUMP_MAP: u16 = 2;
    pub const SPECULAR_OVERBRIGHT: u16 = 1;
    pub const SPECULAR_EXTRA_SHINY: u16 = 2;
    pub const SPECULAR_LIGHTMAP_IS_SPECULAR: u16 = 4;

    /// The texture coordinate offset at time `t`: `periodic(function, t /
    /// period) x scale` for u and for v (no phase, no source).
    pub fn uv_offset(&self, tables: &PeriodicTables, t: f32) -> [f32; 2] {
        let one = |a: &EnvironmentUvAnimation| tables.value_at(a.function, t, a.period, 0.0) * a.scale;
        [one(&self.u_animation), one(&self.v_animation)]
    }
}

crate::tag_struct! {
    /// The model shader (`soso`, 0x1B8 bytes), used on objects.
    pub struct ModelShader: 0x1B8 {
        0x000 header: ShaderHeader,
        /// Bit 0 detail after reflection, 1 two-sided, 2 not alpha-tested,
        /// 3 alpha-blended decal, 4 true atmospheric fog, 5 disable
        /// two-sided culling.
        0x028 flags: u16,
        /// 0..1: light passing to the back side.
        0x038 translucency: f32,
        0x04C change_color_source: AnimationSource,
        /// Bit 0 no random phase.
        0x06C self_illumination_flags: u16,
        0x070 self_illumination_color_source: AnimationSource,
        0x072 self_illumination_function: PeriodicFunction,
        0x074 self_illumination_period: f32,
        0x078 self_illumination_color_lower: RealRgb,
        0x084 self_illumination_color_upper: RealRgb,
        0x09C map_u_scale: f32,
        0x0A0 map_v_scale: f32,
        0x0A4 base_map: TagRef,
        0x0BC multipurpose_map: TagRef,
        0x0D4 detail_function: DetailFunction,
        0x0D6 detail_mask: ModelDetailMask,
        0x0D8 detail_map_scale: f32,
        0x0DC detail_map: TagRef,
        /// Multiplies the detail scale for v.
        0x0EC detail_map_v_scale: f32,
        0x0FC texture_animation: TextureAnimation,
        0x13C reflection_falloff_distance: f32,
        /// 0: no distance fade.
        0x140 reflection_cutoff_distance: f32,
        0x144 perpendicular_brightness: f32,
        0x148 perpendicular_tint_color: RealRgb,
        0x154 parallel_brightness: f32,
        0x158 parallel_tint_color: RealRgb,
        /// Class `bitm` (cube).
        0x164 reflection_cube_map: TagRef,
    }
}

impl ModelShader {
    pub const DETAIL_AFTER_REFLECTION: u16 = 1;
    pub const TWO_SIDED: u16 = 2;
    pub const NOT_ALPHA_TESTED: u16 = 4;
    pub const ALPHA_BLENDED_DECAL: u16 = 8;
    pub const TRUE_ATMOSPHERIC_FOG: u16 = 16;
    pub const DISABLE_TWO_SIDED_CULLING: u16 = 32;
}

crate::tag_struct! {
    /// The block shared by chicago and generic shaders after the header
    /// (0x38 bytes, read at 0x28 of the main struct). The maps block at its
    /// end is each class's own and is not read here.
    pub struct TransparentCommon: 0x38 {
        /// 0..255.
        0x00 numeric_counter_limit: u8,
        /// Bit 0 alpha-tested, 1 decal, 2 two-sided, 3 first map is in
        /// screen space, 4 draw before water, 5 ignore effect, 6 scale
        /// first map with distance, 7 numeric.
        0x01 flags: u8,
        0x02 first_map_type: FirstMapType,
        0x04 framebuffer_blend_function: BlendFunction,
        0x06 framebuffer_fade_mode: FadeMode,
        0x08 framebuffer_fade_source: AnimationSource,
        /// World units.
        0x0C lens_flare_spacing: f32,
        /// Class `lens`.
        0x10 lens_flare: TagRef,
        /// Other shaders drawn first; at most 4 (none on the disc).
        0x20 extra_layers: Vec<ExtraLayer>,
    }
}

nested_field!(TransparentCommon);

impl TransparentCommon {
    pub const ALPHA_TESTED: u8 = 1;
    pub const DECAL: u8 = 2;
    pub const TWO_SIDED: u8 = 4;
    pub const FIRST_MAP_IN_SCREEN_SPACE: u8 = 8;
    pub const DRAW_BEFORE_WATER: u8 = 16;
    pub const IGNORE_EFFECT: u8 = 32;
    pub const SCALE_FIRST_MAP_WITH_DISTANCE: u8 = 64;
    pub const NUMERIC: u8 = 128;
}

crate::tag_struct! {
    /// A map of a transparent chicago shader (0xDC bytes).
    pub struct ChicagoMap: 0xDC {
        /// Bit 0 unfiltered, 1 alpha replicate, 2 u-clamped, 3 v-clamped.
        0x00 flags: u16,
        /// How this map's colour joins the next map's (unused on the last
        /// map).
        0x2C color_function: ChicagoFunction,
        0x2E alpha_function: ChicagoFunction,
        0x54 u_scale: f32,
        0x58 v_scale: f32,
        0x5C u_offset: f32,
        0x60 v_offset: f32,
        /// Degrees.
        0x64 rotation: f32,
        0x68 mipmap_bias: f32,
        /// Class `bitm`.
        0x6C map: TagRef,
        0xA4 animation: TextureAnimation,
    }
}

impl ChicagoMap {
    pub const UNFILTERED: u16 = 1;
    pub const ALPHA_REPLICATE: u16 = 2;
    pub const U_CLAMPED: u16 = 4;
    pub const V_CLAMPED: u16 = 8;

    /// The map's texture transform at time `t` (the animation block with
    /// the map's own scale, offset and rotation).
    pub fn texture_transform(
        &self,
        tables: &PeriodicTables,
        t: f32,
        source: &dyn Fn(AnimationSource) -> f32,
    ) -> TextureTransform {
        self.animation.transform(
            tables,
            t,
            [self.u_scale, self.v_scale],
            [self.u_offset, self.v_offset],
            self.rotation,
            source,
        )
    }
}

crate::tag_struct! {
    /// The transparent chicago shader (`schi`, 0x6C bytes).
    pub struct ChicagoShader: 0x6C {
        0x00 header: ShaderHeader,
        0x28 common: TransparentCommon,
        /// At most 4.
        0x54 maps: Vec<ChicagoMap>,
        /// Bit 0 don't fade active camouflage, bit 1 numeric countdown
        /// timer.
        0x60 extra_flags: u32,
    }
}

impl ChicagoShader {
    pub const DONT_FADE_ACTIVE_CAMOUFLAGE: u32 = 1;
    pub const NUMERIC_COUNTDOWN_TIMER: u32 = 2;
}

crate::tag_struct! {
    /// A map of a transparent generic shader (0x64 bytes).
    pub struct GenericMap: 0x64 {
        /// Bit 0 unfiltered, 1 u-clamped, 2 v-clamped.
        0x00 flags: u16,
        0x04 u_scale: f32,
        0x08 v_scale: f32,
        0x0C u_offset: f32,
        0x10 v_offset: f32,
        /// Degrees.
        0x14 rotation: f32,
        0x18 mipmap_bias: f32,
        /// Class `bitm`.
        0x1C map: TagRef,
        0x2C animation: TextureAnimation,
    }
}

impl GenericMap {
    pub const UNFILTERED: u16 = 1;
    pub const U_CLAMPED: u16 = 2;
    pub const V_CLAMPED: u16 = 4;

    /// The map's texture transform at time `t`.
    pub fn texture_transform(
        &self,
        tables: &PeriodicTables,
        t: f32,
        source: &dyn Fn(AnimationSource) -> f32,
    ) -> TextureTransform {
        self.animation.transform(
            tables,
            t,
            [self.u_scale, self.v_scale],
            [self.u_offset, self.v_offset],
            self.rotation,
            source,
        )
    }
}

crate::tag_struct! {
    /// A generic stage input and its mapping (4 bytes).
    pub struct StageInput: 4 {
        0x00 input: GenericInput,
        0x02 mapping: InputMapping,
    }
}

nested_field!(StageInput);

crate::tag_struct! {
    /// A stage's four inputs A, B, C, D.
    pub struct StageInputs: 16 {
        0x00 a: StageInput,
        0x04 b: StageInput,
        0x08 c: StageInput,
        0x0C d: StageInput,
    }
}

nested_field!(StageInputs);

crate::tag_struct! {
    /// A stage's colour outputs (12 bytes).
    pub struct StageColorOutputs: 12 {
        0x00 ab: StageOutput,
        0x02 ab_function: OutputFunction,
        0x04 cd: StageOutput,
        0x06 cd_function: OutputFunction,
        /// The AB/CD mux-sum output.
        0x08 mux_sum: StageOutput,
        0x0A mapping: OutputMapping,
    }
}

nested_field!(StageColorOutputs);

crate::tag_struct! {
    /// A stage's alpha outputs (8 bytes).
    pub struct StageAlphaOutputs: 8 {
        0x00 ab: StageOutput,
        0x02 cd: StageOutput,
        0x04 mux_sum: StageOutput,
        0x06 mapping: OutputMapping,
    }
}

nested_field!(StageAlphaOutputs);

crate::tag_struct! {
    /// A stage of a transparent generic shader (0x70 bytes).
    pub struct GenericStage: 0x70 {
        /// Bit 0 colour mux, bit 1 alpha mux, bit 2 A-out controls constant
        /// colour 0 animation.
        0x00 flags: u16,
        0x04 constant_color0_source: AnimationSource,
        0x06 constant_color0_function: PeriodicFunction,
        /// Seconds.
        0x08 constant_color0_period: f32,
        0x0C constant_color0_lower: RealArgb,
        0x1C constant_color0_upper: RealArgb,
        0x2C constant_color1: RealArgb,
        0x3C color_inputs: StageInputs,
        0x4C color_outputs: StageColorOutputs,
        0x58 alpha_inputs: StageInputs,
        0x68 alpha_outputs: StageAlphaOutputs,
    }
}

impl GenericStage {
    pub const COLOR_MUX: u16 = 1;
    pub const ALPHA_MUX: u16 = 2;
    pub const A_OUT_CONTROLS_CONSTANT_COLOR0_ANIMATION: u16 = 4;
}

crate::tag_struct! {
    /// The transparent generic shader (`sotr`, 0x6C bytes).
    pub struct GenericShader: 0x6C {
        0x00 header: ShaderHeader,
        0x28 common: TransparentCommon,
        /// At most 4.
        0x54 maps: Vec<GenericMap>,
        /// At most 7.
        0x60 stages: Vec<GenericStage>,
    }
}

crate::tag_struct! {
    /// The glass shader (`sgla`, 0x1E0 bytes).
    pub struct GlassShader: 0x1E0 {
        0x000 header: ShaderHeader,
        /// Bit 0 alpha-tested, 1 decal, 2 two-sided, 3 bump map is
        /// specular mask.
        0x028 flags: u16,
        0x054 background_tint_color: RealRgb,
        0x060 background_tint_map_scale: f32,
        0x064 background_tint_map: TagRef,
        0x088 reflection_flags: u16,
        0x08A reflection_type: GlassReflectionType,
        /// Alpha is the brightness.
        0x08C perpendicular_color: RealArgb,
        0x09C parallel_color: RealArgb,
        /// Class `bitm` (cube).
        0x0AC reflection_map: TagRef,
        0x0BC bump_map_scale: f32,
        0x0C0 bump_map: TagRef,
        0x150 diffuse_flags: u16,
        0x154 diffuse_map_scale: f32,
        0x158 diffuse_map: TagRef,
        0x168 diffuse_detail_map_scale: f32,
        0x16C diffuse_detail_map: TagRef,
        /// Not drawn by the Xbox; none on the disc.
        0x19C specular_map_scale: f32,
        0x1A0 specular_map: TagRef,
        0x1B0 specular_detail_map_scale: f32,
        0x1B4 specular_detail_map: TagRef,
    }
}

impl GlassShader {
    pub const ALPHA_TESTED: u16 = 1;
    pub const DECAL: u16 = 2;
    pub const TWO_SIDED: u16 = 4;
    pub const BUMP_MAP_IS_SPECULAR_MASK: u16 = 8;
}

crate::tag_struct! {
    /// The meter shader (`smet`, 0x104 bytes).
    pub struct MeterShader: 0x104 {
        0x000 header: ShaderHeader,
        /// Bit 0 decal, 1 two-sided, 2 flash colour is negative, 3 tint
        /// mode 2, 4 unfiltered.
        0x028 flags: u16,
        0x04C meter_map: TagRef,
        0x07C gradient_min_color: RealRgb,
        0x088 gradient_max_color: RealRgb,
        0x094 background_color: RealRgb,
        0x0A0 flash_color: RealRgb,
        0x0AC tint_color: RealRgb,
        0x0B8 meter_transparency: f32,
        0x0BC background_transparency: f32,
        0x0D8 meter_brightness_source: AnimationSource,
        0x0DA flash_brightness_source: AnimationSource,
        0x0DC value_source: AnimationSource,
        0x0DE gradient_source: AnimationSource,
        0x0E0 flash_extension_source: AnimationSource,
    }
}

impl MeterShader {
    pub const DECAL: u16 = 1;
    pub const TWO_SIDED: u16 = 2;
    pub const FLASH_COLOR_IS_NEGATIVE: u16 = 4;
    pub const TINT_MODE_2: u16 = 8;
    pub const UNFILTERED: u16 = 16;
}

crate::tag_struct! {
    /// The plasma shader (`spla`, 0x14C bytes).
    pub struct PlasmaShader: 0x14C {
        0x000 header: ShaderHeader,
        0x02C intensity_source: AnimationSource,
        0x030 intensity_exponent: f32,
        0x034 offset_source: AnimationSource,
        /// World units.
        0x038 offset_amount: f32,
        0x03C offset_exponent: f32,
        0x060 perpendicular_brightness: f32,
        0x064 perpendicular_tint_color: RealRgb,
        0x070 parallel_brightness: f32,
        0x074 parallel_tint_color: RealRgb,
        0x080 tint_color_source: AnimationSource,
        0x0C0 primary_noise_period: f32,
        0x0C4 primary_noise_direction: [f32; 3],
        0x0D0 primary_noise_scale: f32,
        /// Class `bitm` (3D).
        0x0D4 primary_noise_map: TagRef,
        0x108 secondary_noise_period: f32,
        0x10C secondary_noise_direction: [f32; 3],
        0x118 secondary_noise_scale: f32,
        0x11C secondary_noise_map: TagRef,
    }
}

crate::tag_struct! {
    /// A water ripple layer (0x4C bytes).
    pub struct WaterRipple: 0x4C {
        0x04 contribution_factor: f32,
        /// Radians.
        0x28 animation_angle: f32,
        /// Texture units per second.
        0x2C animation_velocity: f32,
        0x30 map_offset: [f32; 2],
        0x38 map_repeats: i16,
        /// Image of the ripple maps.
        0x3A map_index: i16,
    }
}

crate::tag_struct! {
    /// The water shader (`swat`, 0x140 bytes).
    pub struct WaterShader: 0x140 {
        0x000 header: ShaderHeader,
        /// Bit 0 base map alpha modulates reflection, 1 base map colour
        /// modulates background, 2 atmospheric fog, 3 draw before fog.
        0x028 flags: u16,
        0x04C base_map: TagRef,
        0x06C view_perpendicular_tint_color: RealArgb,
        0x07C view_parallel_tint_color: RealArgb,
        /// Class `bitm` (cube).
        0x09C reflection_map: TagRef,
        /// Radians.
        0x0BC ripple_animation_angle: f32,
        /// Texture units per second.
        0x0C0 ripple_animation_velocity: f32,
        0x0C4 ripple_scale: f32,
        /// Several images.
        0x0C8 ripple_maps: TagRef,
        0x0D8 ripple_mipmap_levels: i16,
        0x0DC ripple_mipmap_fade_factor: f32,
        0x0E0 ripple_mipmap_detail_bias: f32,
        /// Four used.
        0x124 ripples: Vec<WaterRipple>,
    }
}

impl WaterShader {
    pub const BASE_MAP_ALPHA_MODULATES_REFLECTION: u16 = 1;
    pub const BASE_MAP_COLOR_MODULATES_BACKGROUND: u16 = 2;
    pub const ATMOSPHERIC_FOG: u16 = 4;
    pub const DRAW_BEFORE_FOG: u16 = 8;
}

/// A shader tag of any of the disc's eight classes.
#[derive(Clone, Debug, PartialEq)]
pub enum Shader {
    Environment(EnvironmentShader),
    Model(ModelShader),
    TransparentChicago(ChicagoShader),
    TransparentGeneric(GenericShader),
    Glass(GlassShader),
    Meter(MeterShader),
    Plasma(PlasmaShader),
    Water(WaterShader),
}

impl Shader {
    /// Read the shader tag `tag` of `map`, dispatching on its class. Any
    /// other class is an error.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Shader> {
        fn at<T: TagStruct>(map: &MapFile, tag: &TagEntry) -> io::Result<T> {
            map.read_tag::<T>(tag)
        }
        Ok(match tag.class {
            class::SHADER_ENVIRONMENT => Shader::Environment(at(map, tag)?),
            class::SHADER_MODEL => Shader::Model(at(map, tag)?),
            class::SHADER_TRANSPARENT_CHICAGO => {
                let v = map.tag_view(tag, ChicagoShader::SIZE)?;
                let s = <ChicagoShader as TagStruct>::read(&v)?;
                limits(&v, &s.common, s.maps.len(), None)?;
                Shader::TransparentChicago(s)
            }
            class::SHADER_TRANSPARENT_GENERIC => {
                let v = map.tag_view(tag, GenericShader::SIZE)?;
                let s = <GenericShader as TagStruct>::read(&v)?;
                limits(&v, &s.common, s.maps.len(), Some(s.stages.len()))?;
                Shader::TransparentGeneric(s)
            }
            class::SHADER_TRANSPARENT_GLASS => Shader::Glass(at(map, tag)?),
            class::SHADER_TRANSPARENT_METER => Shader::Meter(at(map, tag)?),
            class::SHADER_TRANSPARENT_PLASMA => Shader::Plasma(at(map, tag)?),
            class::SHADER_TRANSPARENT_WATER => Shader::Water(at(map, tag)?),
            other => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "tag `{}` is class {other}, not a shader class this reader knows",
                        tag.path
                    ),
                ));
            }
        })
    }

    /// The tag class of this shader.
    pub fn class(&self) -> ClassCode {
        match self {
            Shader::Environment(_) => class::SHADER_ENVIRONMENT,
            Shader::Model(_) => class::SHADER_MODEL,
            Shader::TransparentChicago(_) => class::SHADER_TRANSPARENT_CHICAGO,
            Shader::TransparentGeneric(_) => class::SHADER_TRANSPARENT_GENERIC,
            Shader::Glass(_) => class::SHADER_TRANSPARENT_GLASS,
            Shader::Meter(_) => class::SHADER_TRANSPARENT_METER,
            Shader::Plasma(_) => class::SHADER_TRANSPARENT_PLASMA,
            Shader::Water(_) => class::SHADER_TRANSPARENT_WATER,
        }
    }

    /// The common header.
    pub fn header(&self) -> &ShaderHeader {
        match self {
            Shader::Environment(s) => &s.header,
            Shader::Model(s) => &s.header,
            Shader::TransparentChicago(s) => &s.header,
            Shader::TransparentGeneric(s) => &s.header,
            Shader::Glass(s) => &s.header,
            Shader::Meter(s) => &s.header,
            Shader::Plasma(s) => &s.header,
            Shader::Water(s) => &s.header,
        }
    }

    /// Every reference the shader makes to a bitmap (base, detail, bump,
    /// cube, ripple, noise and the transparent classes' maps), null ones
    /// included, in field order.
    pub fn bitmap_references(&self) -> Vec<TagRef> {
        match self {
            Shader::Environment(s) => vec![
                s.base_map,
                s.primary_detail_map,
                s.secondary_detail_map,
                s.micro_detail_map,
                s.bump_map,
                s.self_illumination_map,
                s.reflection_cube_map,
            ],
            Shader::Model(s) => vec![s.base_map, s.multipurpose_map, s.detail_map, s.reflection_cube_map],
            Shader::TransparentChicago(s) => s.maps.iter().map(|m| m.map).collect(),
            Shader::TransparentGeneric(s) => s.maps.iter().map(|m| m.map).collect(),
            Shader::Glass(s) => vec![
                s.background_tint_map,
                s.reflection_map,
                s.bump_map,
                s.diffuse_map,
                s.diffuse_detail_map,
                s.specular_map,
                s.specular_detail_map,
            ],
            Shader::Meter(s) => vec![s.meter_map],
            Shader::Plasma(s) => vec![s.primary_noise_map, s.secondary_noise_map],
            Shader::Water(s) => vec![s.base_map, s.reflection_map, s.ripple_maps],
        }
    }
}

/// The transparent classes' block limits.
fn limits(v: &View<'_>, common: &TransparentCommon, maps: usize, stages: Option<usize>) -> io::Result<()> {
    let mut checks = vec![
        ("maps", maps, MAX_TRANSPARENT_MAPS, 0x54),
        ("extra layers", common.extra_layers.len(), MAX_EXTRA_LAYERS, 0x48),
    ];
    if let Some(n) = stages {
        checks.push(("stages", n, MAX_GENERIC_STAGES, 0x60));
    }
    for (what, count, max, at) in checks {
        if count > max {
            return Err(v.error(at, format_args!("{what}: {count} elements, over the maximum {max}")));
        }
    }
    Ok(())
}
