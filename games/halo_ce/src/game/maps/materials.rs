//! The level's shaders as the engine's mesh looks, by the minimum faithful
//! approximation of each class (specs/halo_ce/shaders.md), and their
//! bitmaps as the map's textures (`blam_formats::bitmap`), each decoded
//! once and shared between meshes.
//!
//! - Environment (`senv`, "Minimum faithful approximation
//!   (environment)"): the base map as the texture at the vertex's own
//!   coordinates; the primary detail map (or, for the normal type with
//!   only a secondary, the secondary) as `MapMesh::detail` at its detail
//!   scale (times base / detail size with "rescale detail maps"); the
//!   lightmap as the light (`lightmap`); opaque, or cut out where the
//!   **bump map's** alpha is at most 127/255 when the alpha-tested flag is
//!   set ("Alpha testing"). A mesh has one texture and the engine's mask
//!   reads the base texture's alpha, so the bump map's alpha is baked into
//!   the base texture handed over (`Cutout`: each base texel takes the
//!   bump texel under it at the base coordinate times the runtime bump
//!   scale, nearest, as 0 or 255) and the mask is 0.5; such a texture
//!   keeps no block-compressed copy. A material colour other than white
//!   is the mesh's tint (it multiplies the lightmap; the product is the
//!   same).
//!   With no primary detail map, the micro detail map is the detail with
//!   its own function and scale (41 tags carry one).
//! - Detail as the engine draws it: Source's mode 0 (`albedo x 2 x
//!   detail`, `DetailMode::Source(0)`) with the detail texture read as
//!   stored (`srgb` false, so the texel is the stored byte / 255): the
//!   game's clamp(2 x base x detail) in gamma space, neutral at the same
//!   detail value (1/2); see "Known shortcuts". Multiply is Source's mode
//!   8 (`albedo x detail`).
//! - Environment self-illumination ("Lightmap pass value L", 16 of the
//!   76 disc tags on the multiplayer maps): the engine's world
//!   self-illumination, `MapSelfIllum` without a mask, which draws
//!   mix(lit, tint x albedo, base alpha); the self-illumination map's
//!   weight is baked into the base texture's alpha (`Derive::SelfIllum`,
//!   `bake_self_illumination`: its scale is 1 on every disc tag) and the
//!   tint is the channels' colours (`self_illumination_colour`: primary and
//!   secondary at their function's mean, plasma's off colour). Exact in
//!   colour where one channel lights (all but 2 tags); the game's L + SI
//!   becomes L + SI - L x SI, the same in the dark.
//! - Environment reflection ("Reflection pass value R", 57 tags): the
//!   engine's world reflection, `MapEnvmap`, with the shader's cube map
//!   (`engine_cube`: Halo's faces, bitmaps.md "Cube maps", resampled into
//!   the engine's sampling frame, the reflection taken in Halo's world
//!   frame) and the specular colours' K^8 / K mix baked into its texels at
//!   their mean (`reflection_contrast`); the brightness ramp from the
//!   perpendicular to the parallel brightness as the engine's fresnel
//!   ramp (`reflection_brightness`: same ends, Schlick's curve between);
//!   masked by the base map's alpha (a channel texture, `Derive::Channel`)
//!   and by the lightmap (`MapEnvmap::light_mask`: the engine extension
//!   for "Reflection mask value M").
//! - Transparent generic (`sotr`) and chicago (`schi`): map 0 with its
//!   texture transform (scale, offset, rotation, the animation at time 0)
//!   as `MapMesh::base_transform`, its slide animations as the transform's
//!   steady scroll (`slide_scroll`: exact for whole-number scales; 16
//!   generic and 9 chicago tags scroll); blend function 0 (alpha blend)
//!   and 7 (alpha-multiply add) blend, 3 (add) adds (`MapAlpha::Blend`/
//!   `Add`); alpha-tested masks at 0.5; two-sided by flag bit 2; unlit. A
//!   generic shader's tint is the upper constant colour 0 of the first
//!   stage that multiplies it with map 0.
//! - Model (`soso`, "Minimum faithful approximation (model)"): the base
//!   map at the shader's map scales, the detail map as detail, alpha-tested
//!   at 0.5 unless "not alpha-tested" (blended for an alpha-blended decal),
//!   two-sided where flagged, lit (by the placement's light: `objects`).
//!   Self-illumination (`model_self_illumination`): the multipurpose map's
//!   green as the mask of the engine's model self-illumination, the colour
//!   at its function's mean. Reflection (`model_reflection`, on the
//!   level's scenery and vehicles: `maps::apply_looks_in`): the cube map
//!   plain, masked by the multipurpose map's red, tinted by the mean of
//!   the perpendicular and parallel tints x brightnesses. Change colour
//!   (8 tags): the multipurpose map's alpha as `MapMesh::tint_mask` (the
//!   engine extension: the prop shader's tint by a mask), so only the
//!   masked parts take the owner's colour.
//! - Glass (`sgla`): the diffuse map (at its map scale) blended, lit by
//!   its group's lightmap, the diffuse detail map as detail, two-sided by
//!   flag; its reflection (`glass_reflection`, layer 2) as the
//!   environment's, flat and unmasked, but blended with the diffuse
//!   layer's alpha instead of added (one pass). Water (`swat`, two tags,
//!   both on sky geometry): its base map blended, unlit, double-sided
//!   (the spec's minimum); its ripples and reflection are not drawn.
//!   Meter (`smet`, 7 tags: weapon and vehicle bars): the meter map
//!   blended by its alpha, unlit (the spec's minimum, added and tinted by
//!   the gradient minimum colour, lit up the whole quad of the plasma
//!   pistol's dial: the bar's shape is in Mt.a; the gradient, value cut
//!   and flash are not drawn). Plasma (`spla`, the
//!   one shield-hit tag) by its minimum: the primary noise map's first
//!   slice added, tinted by the perpendicular colour x brightness, unlit,
//!   double-sided; drawing it over the Spartan as the shield flash is the
//!   bodies' (a modifier, objects.md), not this module's.
//! - Textures: RGBA8 with the stored mip levels when the file stores the
//!   whole chain (else box-filtered ones), and the stored blocks as `MapBc`
//!   for DXT1/3/5 when they fit (`MapBc::fits`). A cube map or a volume
//!   gives its first face or slice. With `MapLoadOptions::textures` off,
//!   each texture is a placeholder of its size (no texels), each cube map
//!   one without faces.
//! - Every shader's header material type is the mesh's surface name
//!   (`surfaces`). A shader that is missing, unreadable, or of a class the
//!   level does not use gives a flat colour and a line in the map's
//!   warnings, as does an unreadable bitmap; the load never fails.
//!
//! Known shortcuts (against shaders.md):
//! - A chicago shader of two or more maps on whole-number scales is
//!   combined into one baked texture (`combined`: the spec's combine at
//!   nearest sampling, offsets, rotation and animation ignored: the sky's
//!   three-map base and cloud layer, whose clouds therefore stand still);
//!   other chicago shaders draw map 0 alone.
//! - The model shader's change colour is drawn only where its owner gives
//!   a colour (the Spartan's armour: `body::load`; scenery's object
//!   functions are white), its detail mask and translucency are
//!   not drawn, its reflection has no fresnel ramp, no distance fade and
//!   no object reflection tint, and its two-sided flag is honoured though
//!   the game ignores it (two-sided parts carry both windings). The
//!   Spartan's and the guns' meshes (`apply_looks`, dressed on any game's
//!   map) have no reflection.
//! - Detail is blended in linear light (`base_lin x 2 x d`), the game's in
//!   gamma (`(2 x base x d)` before the display), without the game's clamp
//!   ("Diffuse texture pass value D"). One detail map: the primary (or
//!   the secondary, normal type alone), else the micro detail map; the
//!   micro detail under a primary, the secondary under a primary and the
//!   detail blend types are not drawn.
//! - No bump shading: Halo's lightmaps are plain, and the bump term
//!   (1 - w) + w clamp(n . l) needs each lightmap vertex's incident
//!   direction and weight, which the meshes do not carry; reflections are
//!   flat (the vertex normal), not bumped. No specular lightmap (1 tag).
//! - Self-illumination and reflection colours are added in linear light,
//!   the game's in gamma; the plasma channel's animated band, the
//!   functions' animation (drawn at their mean), the reflection's mask by
//!   the detail maps' alphas and by the bump map ("bump map is specular
//!   mask") are not drawn; an alpha-tested surface's self-illumination is
//!   not drawn (its base alpha holds the cut-out).
//! - Texture animations other than slides (cosine, noise, spark, wander:
//!   5 chicago tags) are frozen at time 0; a transparent shader's maps
//!   after map 0, its stages and fade are not combined (generic: "need the
//!   full combiner").
//! - Blend functions multiply, double multiply, subtract, component min
//!   and max (none on the multiplayer maps) are drawn as alpha blend.
//! - The cut-out bake samples the bump map nearest, exact when the bump
//!   scale is a whole number of repeats.

use std::collections::BTreeMap;

use blam_formats::{
    bitmap::{Bitmap, Image, ImageType},
    cache::{MapFile, TagId, TagRef},
    shader::{
        BlendFunction, ChicagoFunction, ChicagoMap, DetailFunction, EnvironmentShader, EnvironmentType, GenericInput,
        ModelShader, PeriodicFunction, PeriodicTables, Shader, ShaderHeader, TextureAnimation, TextureTransform,
        TransparentCommon, no_object,
    },
};

use super::surfaces;
use crate::map::{DetailMode, MapAlpha, MapBc, MapBcFormat, MapDetail, MapTexture, MapUvTransform};

/// The flat colour of a surface without a texture.
pub const FLAT: [u8; 3] = [128, 128, 128];

/// Bake the bump map's alpha into the base texture: the bump bitmap and
/// its scale (u, v) on the base coordinates, as bits (an ordered key).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cutout {
    pub bump: u32,
    pub scale: [u32; 2],
}

/// A texture the meshes want: a bitmap's first image, as colour (sRGB) or
/// as stored (`srgb` false: a detail map), maybe with a cut-out baked in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TextureRequest {
    pub bitmap: u32,
    pub srgb: bool,
    pub cutout: Option<Cutout>,
    /// Bake a transparent chicago shader's maps combined (`combined`): the
    /// shader's tag id; `bitmap` is its map 0.
    pub combine: Option<u32>,
    /// A texture made from the bitmap (`Derive`).
    pub derive: Derive,
}

impl TextureRequest {
    /// The bitmap as colour (sRGB).
    pub fn colour(bitmap: u32) -> Self {
        TextureRequest {
            bitmap,
            srgb: true,
            cutout: None,
            combine: None,
            derive: Derive::None,
        }
    }

    /// The bitmap as stored (a detail map).
    pub fn stored(bitmap: u32) -> Self {
        TextureRequest {
            srgb: false,
            ..Self::colour(bitmap)
        }
    }

    /// One channel of the bitmap, as stored, in all four (a mask).
    pub fn channel(bitmap: u32, channel: u8) -> Self {
        TextureRequest {
            derive: Derive::Channel(channel),
            ..Self::stored(bitmap)
        }
    }
}

/// A texture made from a bitmap rather than the bitmap itself.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Derive {
    #[default]
    None,
    /// One channel (0 red, 1 green, 2 blue, 3 alpha) copied into all four:
    /// the engine's reflection and self-illumination masks read a texture
    /// per rgb channel (`channel_mask`).
    Channel(u8),
    /// The base map with its alpha replaced by an environment shader's
    /// self-illumination weight (`bake_self_illumination`): the
    /// self-illumination map `map` (at the base coordinates: its scale is
    /// 1 on every disc tag) and its channels' weights (f32 bits).
    SelfIllum { map: u32, weights: [u32; 3] },
}

/// A reflection cube map the meshes want: Halo's cube bitmap turned into
/// the engine's sampling frame with its high-contrast mix baked in
/// (`engine_cube`); `contrast` the tint (f32 bits) choosing per channel
/// between K^8 (0) and K (1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct CubeRequest {
    pub bitmap: u32,
    pub contrast: [u32; 3],
}

/// A reflection as the shader asks for it, before its textures are indices.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReflectionRequest {
    pub cube: CubeRequest,
    /// What scales it per texel (a channel mask texture); None: nothing.
    pub mask: Option<TextureRequest>,
    /// `MapEnvmap::tint` (linear) and `fresnel`.
    pub tint: [f32; 3],
    pub fresnel: f32,
    /// `MapEnvmap::light_mask`.
    pub light_mask: Option<crate::map::EnvmapLightMask>,
}

/// Self-illumination as the shader asks for it: its colour (linear) and
/// mask (None: the base texture's alpha, where `Derive::SelfIllum` put it).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SelfIllumRequest {
    pub tint: [f32; 3],
    pub mask: Option<TextureRequest>,
}

/// A detail texture and its scale.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetailRequest {
    pub texture: TextureRequest,
    pub scale: [f32; 2],
    pub mode: DetailMode,
}

/// How a shader draws, before its textures are indices.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    pub base: Option<TextureRequest>,
    pub detail: Option<DetailRequest>,
    pub alpha: MapAlpha,
    pub double_sided: bool,
    pub unlit: bool,
    /// Linear colour multiplier.
    pub tint: Option<[f32; 3]>,
    pub transform: MapUvTransform,
    pub surface: Option<String>,
    /// Drawn in the transparent pass (not `senv`).
    pub transparent: bool,
    /// Why it is not drawn as the shader says, for the warnings.
    pub problem: Option<String>,
    pub reflection: Option<ReflectionRequest>,
    pub self_illumination: Option<SelfIllumRequest>,
    /// Where a tint (the change colour) applies: `MapMesh::tint_mask`.
    pub tint_mask: Option<TextureRequest>,
}

impl Default for Look {
    fn default() -> Self {
        Look {
            base: None,
            detail: None,
            reflection: None,
            self_illumination: None,
            tint_mask: None,
            alpha: MapAlpha::Opaque,
            double_sided: false,
            unlit: false,
            tint: None,
            transform: MapUvTransform::IDENTITY,
            surface: None,
            transparent: false,
            problem: None,
        }
    }
}

/// A colour component in 0..1 as stored (gamma) to linear.
pub fn linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn tag_id(r: &TagRef) -> Option<u32> {
    (!r.id.is_none()).then_some(r.id.0)
}

/// The first image of bitmap `id`, if it reads.
fn first_image(map: &MapFile, id: u32) -> Option<Image> {
    let tag = map.tag(TagId(id))?;
    Bitmap::read(map, tag).ok()?.images.into_iter().next()
}

fn uv_transform(t: TextureTransform) -> MapUvTransform {
    let [su, sv] = t.scale;
    let [c, s] = t.cos_sin;
    let [tu, tv] = t.translation;
    MapUvTransform {
        rows: [[c * su, -s * sv, tu], [s * su, c * sv, tv]],
        scroll: [0.0, 0.0],
    }
}

/// A texture animation's slides as a steady scroll (texture units a
/// second): a u or v channel with the slide function rises by its scale
/// over one cycle of 1/0.7 periods (shaders.md, "Quirk": the sawtooth
/// repeats every period / 0.7 seconds), so it moves at scale x 0.7 /
/// period, turned by the map's rotation (`cos_sin`, as its offsets are).
/// Other functions give no scroll (frozen at time 0). The game wraps the
/// offset at its scale, the engine at one repeat: the same picture for a
/// whole-number scale. A channel with a source (an object's function)
/// scrolls at the source's full value.
pub fn slide_scroll(a: &TextureAnimation, cos_sin: [f32; 2]) -> [f32; 2] {
    let (du, dv) = (
        slide_rate(a.u.function, a.u.period, a.u.scale),
        slide_rate(a.v.function, a.v.period, a.v.scale),
    );
    let [c, s] = cos_sin;
    [c * du - s * dv, s * du + c * dv]
}

/// One channel's scroll rate (`slide_scroll`): scale x 0.7 / period for
/// the slide (a period of 0 counts as 1), else 0.
pub fn slide_rate(function: PeriodicFunction, period: f32, scale: f32) -> f32 {
    if function != PeriodicFunction::Slide {
        return 0.0;
    }
    let period = if period == 0.0 { 1.0 } else { period };
    scale * 0.7 / period
}

/// A map's transform at time 0 with its slides scrolling (`slide_scroll`).
fn animated_transform(t: TextureTransform, a: &TextureAnimation) -> MapUvTransform {
    MapUvTransform {
        scroll: slide_scroll(a, t.cos_sin),
        ..uv_transform(t)
    }
}

fn scale_transform(s: f32) -> MapUvTransform {
    let s = if s == 0.0 { 1.0 } else { s };
    MapUvTransform {
        rows: [[s, 0.0, 0.0], [0.0, s, 0.0]],
        scroll: [0.0, 0.0],
    }
}

fn surface(h: &ShaderHeader) -> Option<String> {
    surfaces::surface_of_code(h.material_type.0)
}

fn transparent_alpha(common: &TransparentCommon) -> (MapAlpha, Option<String>) {
    if common.flags & TransparentCommon::ALPHA_TESTED != 0 {
        return (MapAlpha::Mask(0.5), None);
    }
    match common.framebuffer_blend_function {
        BlendFunction::AlphaBlend | BlendFunction::AlphaMultiplyAdd => (MapAlpha::Blend, None),
        BlendFunction::Add => (MapAlpha::Add, None),
        other => (
            MapAlpha::Blend,
            Some(format!("blend function {other:?} drawn as alpha blend")),
        ),
    }
}

/// The look of shader `shader` (a level material's), reading the bitmaps'
/// headers it needs (sizes for the detail scale).
pub fn look(map: &MapFile, shader: &TagRef, tables: &PeriodicTables) -> Look {
    let shader_id = shader.id.0;
    let Some(tag) = map.tag(shader.id) else {
        return Look {
            problem: Some("no shader".into()),
            ..Look::default()
        };
    };
    let shader = match Shader::read(map, tag) {
        Ok(s) => s,
        Err(e) => {
            return Look {
                problem: Some(format!("unreadable shader: {e}")),
                ..Look::default()
            };
        }
    };
    let colour = |r: Option<u32>| r.map(TextureRequest::colour);
    match &shader {
        Shader::Environment(s) => environment(map, s, tables),
        Shader::TransparentGeneric(s) => {
            let (alpha, problem) = transparent_alpha(&s.common);
            let map0 = s.maps.first();
            // Tint: constant colour 0 of the first stage multiplying it
            // with map 0.
            let tint = s.stages.iter().find_map(|st| {
                let i = &st.color_inputs;
                let pair = |a: GenericInput, b: GenericInput| {
                    matches!(
                        (a, b),
                        (GenericInput::Map0, GenericInput::Constant0) | (GenericInput::Constant0, GenericInput::Map0)
                    )
                };
                (pair(i.a.input, i.b.input) || pair(i.c.input, i.d.input)).then(|| {
                    let c = st.constant_color0_upper;
                    [linear(c.r), linear(c.g), linear(c.b)]
                })
            });
            // Else (shaders.md: "constant colour 0 of the first stage that
            // reads constant colour 0"; an energy shield's many-stage
            // combine, the Jackal's) the first stage whose colour reads
            // constant colour 0 (or its alpha), a fixed colour (no
            // animation source), not black.
            let tint = tint.or_else(|| {
                s.stages.iter().find_map(|st| {
                    let i = &st.color_inputs;
                    let reads = [i.a.input, i.b.input, i.c.input, i.d.input]
                        .iter()
                        .any(|x| matches!(x, GenericInput::Constant0 | GenericInput::Constant0Alt));
                    let c = st.constant_color0_upper;
                    (reads && st.constant_color0_source == blam_formats::shader::AnimationSource::None)
                        .then(|| [linear(c.r), linear(c.g), linear(c.b)])
                        .filter(|t| t.iter().any(|x| *x > 0.0))
                })
            });
            Look {
                base: colour(map0.and_then(|m| tag_id(&m.map))),
                alpha,
                double_sided: s.common.flags & TransparentCommon::TWO_SIDED != 0,
                unlit: true,
                tint,
                transform: map0.map_or(MapUvTransform::IDENTITY, |m| {
                    animated_transform(m.texture_transform(tables, 0.0, &no_object), &m.animation)
                }),
                surface: surface(&s.header),
                transparent: true,
                problem,
                ..Look::default()
            }
        }
        Shader::TransparentChicago(s) => {
            let (alpha, problem) = transparent_alpha(&s.common);
            let map0 = s.maps.first();
            // Two or more maps on whole-number scales: the combine baked
            // into one texture at the maps' scales (`combined`).
            let combine = combinable(&s.maps).then_some(shader_id);
            let mut base = colour(map0.and_then(|m| tag_id(&m.map)));
            if let (Some(b), Some(id)) = (base.as_mut(), combine) {
                b.combine = Some(id);
            }
            Look {
                base,
                alpha,
                double_sided: s.common.flags & TransparentCommon::TWO_SIDED != 0,
                unlit: true,
                transform: if combine.is_some() {
                    MapUvTransform::IDENTITY
                } else {
                    map0.map_or(MapUvTransform::IDENTITY, |m| {
                        animated_transform(m.texture_transform(tables, 0.0, &no_object), &m.animation)
                    })
                },
                surface: surface(&s.header),
                transparent: true,
                problem,
                ..Look::default()
            }
        }
        Shader::Model(s) => model_shader(s, tables),
        Shader::Glass(s) => {
            let base = colour(tag_id(&s.diffuse_map));
            let detail = tag_id(&s.diffuse_detail_map).map(|d| DetailRequest {
                texture: TextureRequest::stored(d),
                scale: [nonzero(s.diffuse_detail_map_scale); 2],
                mode: DetailMode::Source(0),
            });
            Look {
                base,
                detail,
                alpha: if s.flags & blam_formats::shader::GlassShader::ALPHA_TESTED != 0 {
                    MapAlpha::Mask(0.5)
                } else {
                    MapAlpha::Blend
                },
                double_sided: s.flags & blam_formats::shader::GlassShader::TWO_SIDED != 0,
                transform: scale_transform(s.diffuse_map_scale),
                reflection: glass_reflection(s),
                surface: surface(&s.header),
                transparent: true,
                ..Look::default()
            }
        }
        Shader::Water(s) => Look {
            base: colour(tag_id(&s.base_map)),
            alpha: MapAlpha::Blend,
            double_sided: true,
            unlit: true,
            surface: surface(&s.header),
            transparent: true,
            ..Look::default()
        },
        // The meter map blended by its alpha, unlit (the spec's minimum
        // adds it coloured by the gradient's minimum colour, but its own
        // alpha decides where the bar is: added, the whole quad lit up);
        // the plasma's primary noise map (its first slice) added, tinted
        // by the perpendicular colour x brightness, unlit, double-sided.
        Shader::Meter(s) => Look {
            base: colour(tag_id(&s.meter_map)),
            alpha: MapAlpha::Blend,
            double_sided: s.flags & blam_formats::shader::MeterShader::TWO_SIDED != 0,
            unlit: true,
            surface: surface(&s.header),
            transparent: true,
            ..Look::default()
        },
        Shader::Plasma(s) => {
            let (c, b) = (s.perpendicular_tint_color, s.perpendicular_brightness);
            Look {
                base: colour(tag_id(&s.primary_noise_map)),
                alpha: MapAlpha::Add,
                double_sided: true,
                unlit: true,
                tint: Some([linear(c.r * b), linear(c.g * b), linear(c.b * b)]),
                surface: surface(&s.header),
                transparent: true,
                ..Look::default()
            }
        }
    }
}

/// The model shader's look (shaders.md, "Minimum faithful approximation
/// (model)"): the base map at the shader's own map scales (the model's
/// are in its texture coordinates), the detail map as a second texture
/// (double biased multiply or multiply), alpha-tested at 0.5 from the
/// base alpha unless "not alpha-tested", blended for an alpha-blended
/// decal, two-sided where flagged. Lit by the placement's light, not
/// unlit; change colour, self-illumination and reflection are not drawn.
fn model_shader(s: &ModelShader, tables: &PeriodicTables) -> Look {
    let alpha = if s.flags & ModelShader::ALPHA_BLENDED_DECAL != 0 {
        MapAlpha::Blend
    } else if s.flags & ModelShader::NOT_ALPHA_TESTED != 0 {
        MapAlpha::Opaque
    } else {
        MapAlpha::Mask(0.5)
    };
    let mode = match s.detail_function {
        DetailFunction::DoubleBiasedMultiply => Some(DetailMode::Source(0)),
        DetailFunction::Multiply => Some(DetailMode::Source(8)),
        DetailFunction::DoubleBiasedAdd => None,
    };
    let detail = tag_id(&s.detail_map).zip(mode).map(|(d, mode)| {
        let scale = nonzero(s.detail_map_scale);
        DetailRequest {
            texture: TextureRequest::stored(d),
            scale: [scale, scale * nonzero(s.detail_map_v_scale)],
            mode,
        }
    });
    let multipurpose = tag_id(&s.multipurpose_map);
    Look {
        base: tag_id(&s.base_map).map(TextureRequest::colour),
        detail,
        reflection: model_reflection(s, multipurpose),
        self_illumination: model_self_illumination(s, multipurpose, tables),
        // The change colour by the multipurpose map's alpha (tint = white
        // (1 - Mp.a) + change colour Mp.a); the colour itself is the
        // owner's (the body's skins).
        tint_mask: multipurpose
            .filter(|_| s.change_color_source != blam_formats::shader::AnimationSource::None)
            .map(|m| TextureRequest::channel(m, 3)),
        alpha,
        double_sided: s.flags & ModelShader::TWO_SIDED != 0,
        transform: MapUvTransform {
            rows: [[nonzero(s.map_u_scale), 0.0, 0.0], [0.0, nonzero(s.map_v_scale), 0.0]],
            scroll: [0.0, 0.0],
        },
        surface: surface(&s.header),
        ..Look::default()
    }
}

/// A glass shader's reflection (shaders.md, "Glass", layer 2): the
/// environment reflection's math with the colours' rgb as the contrast
/// tint (at their mean) and their alphas as the brightnesses; flat,
/// unmasked.
pub fn glass_reflection(s: &blam_formats::shader::GlassShader) -> Option<ReflectionRequest> {
    let cube = tag_id(&s.reflection_map)?;
    let (p, q) = (s.perpendicular_color, s.parallel_color);
    if p.a <= 0.0 && q.a <= 0.0 {
        return None;
    }
    let (tint, fresnel) = reflection_brightness(p.a, q.a);
    let contrast = [(p.r + q.r) / 2.0, (p.g + q.g) / 2.0, (p.b + q.b) / 2.0];
    Some(ReflectionRequest {
        cube: CubeRequest {
            bitmap: cube,
            contrast: contrast.map(f32::to_bits),
        },
        mask: None,
        tint: [tint; 3],
        fresnel,
        light_mask: None,
    })
}

/// A model shader's reflection (shaders.md, "What it draws"): K V1 x
/// Mp.r V1.a, the cube map plain (no K^8 mix), masked by the multipurpose
/// map's red, tinted by the perpendicular and parallel tints times their
/// brightnesses at their mean (the vertex fresnel term V1 is not stated;
/// the engine's model reflections have no ramp); none without a
/// multipurpose map (its mask) or a brightness.
pub fn model_reflection(s: &ModelShader, multipurpose: Option<u32>) -> Option<ReflectionRequest> {
    let cube = tag_id(&s.reflection_cube_map)?;
    let mask = multipurpose?;
    let (pb, qb) = (s.perpendicular_brightness, s.parallel_brightness);
    if pb <= 0.0 && qb <= 0.0 {
        return None;
    }
    let (p, q) = (s.perpendicular_tint_color, s.parallel_tint_color);
    // Gamma values: the prop shader takes its tint through Source's
    // parameter gamma table.
    let tint = [
        (pb * p.r + qb * q.r) / 2.0,
        (pb * p.g + qb * q.g) / 2.0,
        (pb * p.b + qb * q.b) / 2.0,
    ];
    Some(ReflectionRequest {
        cube: CubeRequest {
            bitmap: cube,
            contrast: [1.0f32.to_bits(); 3],
        },
        mask: Some(TextureRequest::channel(mask, 0)),
        tint,
        fresnel: 1.0,
        light_mask: None,
    })
}

/// A model shader's self-illumination (shaders.md, "What it draws"):
/// lit = (V0 + Mp.g C_si) x tint, C_si = lower + (upper - lower) f with f
/// the animation function at its mean (each object has its own random
/// phase), the source colour white; masked by the multipurpose map's
/// green. None when C_si is black or there is no multipurpose map.
pub fn model_self_illumination(
    s: &ModelShader,
    multipurpose: Option<u32>,
    tables: &PeriodicTables,
) -> Option<SelfIllumRequest> {
    let mask = multipurpose?;
    let f = periodic_mean(tables, s.self_illumination_function);
    let (lo, hi) = (s.self_illumination_color_lower, s.self_illumination_color_upper);
    let c = [
        lo.r + (hi.r - lo.r) * f,
        lo.g + (hi.g - lo.g) * f,
        lo.b + (hi.b - lo.b) * f,
    ];
    if c.iter().all(|v| *v <= 0.0) {
        return None;
    }
    Some(SelfIllumRequest {
        tint: c.map(|v| linear(v.clamp(0.0, 1.0))),
        mask: Some(TextureRequest::channel(mask, 1)),
    })
}

fn nonzero(s: f32) -> f32 {
    if s == 0.0 { 1.0 } else { s }
}

/// The environment shader's look (module doc).
fn environment(map: &MapFile, s: &EnvironmentShader, tables: &PeriodicTables) -> Look {
    let base_id = tag_id(&s.base_map);
    let base_size = base_id
        .and_then(|b| first_image(map, b))
        .map(|i| (i.width as f32, i.height as f32));
    // The cut-out reads the bump map's alpha (flat alpha 1 without one, or
    // when the bump map is the specular mask: nothing cut).
    let mut alpha = MapAlpha::Opaque;
    let mut cutout = None;
    let mut problem = None;
    if s.flags & EnvironmentShader::ALPHA_TESTED != 0
        && s.flags & EnvironmentShader::BUMP_MAP_IS_SPECULAR_MASK == 0
        && let Some(bump) = tag_id(&s.bump_map)
    {
        if base_id.is_some() {
            let scale = if s.runtime_bump_scale == [0.0, 0.0] {
                [nonzero(s.bump_map_scale); 2]
            } else {
                s.runtime_bump_scale.map(nonzero)
            };
            cutout = Some(Cutout {
                bump,
                scale: scale.map(f32::to_bits),
            });
            alpha = MapAlpha::Mask(0.5);
        } else {
            problem = Some("alpha-tested without a base map: drawn opaque".into());
        }
    }
    // The primary detail map, or the secondary for the normal type when
    // it is the only one, else the micro detail map with its own function.
    let (detail_id, detail_scale, function) = match (
        tag_id(&s.primary_detail_map),
        tag_id(&s.secondary_detail_map),
        tag_id(&s.micro_detail_map),
    ) {
        (Some(p), _, _) => (Some(p), s.primary_detail_scale, s.detail_function),
        (None, Some(q), _) if s.environment_type == EnvironmentType::Normal => {
            (Some(q), s.secondary_detail_scale, s.detail_function)
        }
        (None, _, Some(u)) => (Some(u), s.micro_detail_scale, s.micro_detail_function),
        _ => (None, 0.0, s.detail_function),
    };
    let mode = match function {
        DetailFunction::DoubleBiasedMultiply => Some(DetailMode::Source(0)),
        DetailFunction::Multiply => Some(DetailMode::Source(8)),
        DetailFunction::DoubleBiasedAdd => None,
    };
    if detail_id.is_some() && mode.is_none() {
        problem = Some("detail function double biased add: detail not drawn".into());
    }
    let detail = detail_id.zip(mode).map(|(d, mode)| {
        let mut scale = [nonzero(detail_scale); 2];
        if s.diffuse_flags & EnvironmentShader::RESCALE_DETAIL_MAPS != 0
            && let (Some((bw, bh)), Some(di)) = (base_size, first_image(map, d))
            && di.width > 0
            && di.height > 0
        {
            scale[0] *= bw / di.width as f32;
            scale[1] *= bh / di.height as f32;
        }
        DetailRequest {
            texture: TextureRequest::stored(d),
            scale,
            mode,
        }
    });
    let c = s.material_color;
    let tint = ([c.r, c.g, c.b] != [1.0; 3]).then(|| [linear(c.r), linear(c.g), linear(c.b)]);
    // Self-illumination: its weight baked into the base alpha (which a
    // cut-out already holds: then not drawn).
    let mut derive = Derive::None;
    let mut self_illumination = None;
    if let (Some(map), Some(_)) = (tag_id(&s.self_illumination_map), base_id) {
        let (tint, weights) = self_illumination_colour(tables, s);
        if cutout.is_some() {
            problem = Some("self-illumination on an alpha-tested surface: not drawn".into());
        } else if weights != [0.0; 3] {
            derive = Derive::SelfIllum {
                map,
                weights: weights.map(f32::to_bits),
            };
            self_illumination = Some(SelfIllumRequest { tint, mask: None });
        }
    }
    let reflection = environment_reflection(s, base_id);
    Look {
        base: base_id.map(|bitmap| TextureRequest {
            cutout,
            derive,
            ..TextureRequest::colour(bitmap)
        }),
        detail,
        alpha,
        tint,
        surface: surface(&s.header),
        problem,
        reflection,
        self_illumination,
        ..Look::default()
    }
}

/// The mean of periodic function `f` over one of its cycles (1/0.7
/// periods: shaders.md, "Quirk"), 256 samples; 1 for "one".
pub fn periodic_mean(tables: &PeriodicTables, f: PeriodicFunction) -> f32 {
    if f == PeriodicFunction::One {
        return 1.0;
    }
    let n = 256;
    (0..n).map(|i| tables.value(f, i as f32 / n as f32 / 0.7)).sum::<f32>() / n as f32
}

/// An environment shader's self-illumination as one colour and the
/// weights of the map's red, green and blue (shaders.md, "Lightmap pass
/// value L"): SI = I.r P + I.g Q + I.b C with P and Q the primary and
/// secondary colours at their function's mean (off (1 - p) + on p) and C
/// the plasma off colour (its on colour lights only a narrow band the
/// animated value sweeps: not drawn). The colour is P + Q + C (each
/// channel at most 1, linear) and a channel's weight its colour's share
/// of the sum's brightness (r + g + b): exact where one channel lights.
pub fn self_illumination_colour(tables: &PeriodicTables, s: &EnvironmentShader) -> ([f32; 3], [f32; 3]) {
    let colour = |c: &blam_formats::shader::SelfIlluminationChannel, blend: bool| {
        let p = if blend { periodic_mean(tables, c.function) } else { 0.0 };
        let (on, off) = (c.on_color, c.off_color);
        [
            off.r * (1.0 - p) + on.r * p,
            off.g * (1.0 - p) + on.g * p,
            off.b * (1.0 - p) + on.b * p,
        ]
    };
    let parts = [
        colour(&s.primary_self_illumination, true),
        colour(&s.secondary_self_illumination, true),
        colour(&s.plasma_self_illumination, false),
    ];
    self_illumination_mix(parts)
}

/// `self_illumination_colour`'s mix of three colours (gamma, 0..1): the
/// clamped sum as linear, and each one's share of the brightness.
pub fn self_illumination_mix(parts: [[f32; 3]; 3]) -> ([f32; 3], [f32; 3]) {
    let bright = parts.map(|c| c.iter().map(|v| v.max(0.0)).sum::<f32>());
    let total: f32 = bright.iter().sum();
    if total <= 0.0 {
        return ([0.0; 3], [0.0; 3]);
    }
    let sum = [0, 1, 2].map(|k| parts.iter().map(|c| c[k].max(0.0)).sum::<f32>().min(1.0));
    (sum.map(linear), bright.map(|b| b / total))
}

/// Bake an environment shader's self-illumination weight into `rgba`'s
/// alpha (`width` x `height`): each texel takes the self-illumination
/// texel under it (its centre's coordinate, wrapped, nearest) weighted by
/// `weights` on its red, green and blue, clamped to 0..255.
pub fn bake_self_illumination(
    rgba: &mut [u8],
    width: u32,
    height: u32,
    map: &[u8],
    mw: u32,
    mh: u32,
    weights: [f32; 3],
) {
    if mw == 0 || mh == 0 {
        return;
    }
    for y in 0..height {
        for x in 0..width {
            let sx = (((x as f32 + 0.5) / width as f32 * mw as f32) as u32).min(mw - 1);
            let sy = (((y as f32 + 0.5) / height as f32 * mh as f32) as u32).min(mh - 1);
            let o = ((sy * mw + sx) * 4) as usize;
            let w: f32 = (0..3).map(|k| map[o + k] as f32 * weights[k]).sum();
            rgba[((y * width + x) * 4 + 3) as usize] = w.round().clamp(0.0, 255.0) as u8;
        }
    }
}

/// The reflection mask weights of the lightmap (shaders.md, "Reflection
/// mask value M": (128, 176, 80) / 255).
pub const REFLECTION_LIGHT_WEIGHTS: [f32; 3] = [128.0 / 255.0, 176.0 / 255.0, 80.0 / 255.0];

/// An environment shader's cube-map reflection (shaders.md, "Reflection
/// pass value R"), when it has one (a cube map and a brightness):
/// brightness beta = parallel (1 - a) + perpendicular a with a the squared
/// fresnel cosine, as the engine's fresnel ramp from the perpendicular
/// (head on) to the parallel brightness (grazing), linear; the specular
/// colours' K^8 / K mix baked into the cube at their mean
/// (`CubeRequest::contrast`); masked by the base map's alpha (the normal
/// and blended-base-specular types' D.a, without the detail maps'
/// alphas; the blended type unmasked) and by the lightmap (M).
pub fn environment_reflection(s: &EnvironmentShader, base: Option<u32>) -> Option<ReflectionRequest> {
    let cube = tag_id(&s.reflection_cube_map)?;
    let (perp, par) = (s.perpendicular_brightness, s.parallel_brightness);
    if perp <= 0.0 && par <= 0.0 {
        return None;
    }
    let (p, q) = (s.specular_perpendicular_color, s.specular_parallel_color);
    let contrast = [(p.r + q.r) / 2.0, (p.g + q.g) / 2.0, (p.b + q.b) / 2.0];
    let (tint, fresnel) = reflection_brightness(perp, par);
    let mask = base
        .filter(|_| s.environment_type != EnvironmentType::Blended)
        .map(|b| TextureRequest::channel(b, 3));
    Some(ReflectionRequest {
        cube: CubeRequest {
            bitmap: cube,
            contrast: contrast.map(f32::to_bits),
        },
        mask,
        tint: [tint; 3],
        fresnel,
        light_mask: Some(crate::map::EnvmapLightMask {
            weights: REFLECTION_LIGHT_WEIGHTS,
            floor: s.lightmap_brightness_scale.clamp(0.0, 1.0),
        }),
    })
}

/// The engine's reflection tint and fresnel R0 for the perpendicular and
/// parallel brightnesses (gamma, 0..1): tint the parallel one (linear),
/// R0 the perpendicular's share of it; with no parallel brightness, the
/// perpendicular one throughout (no ramp).
pub fn reflection_brightness(perpendicular: f32, parallel: f32) -> (f32, f32) {
    let (p, q) = (linear(perpendicular.clamp(0.0, 1.0)), linear(parallel.clamp(0.0, 1.0)));
    if q <= 0.0 { (p, 1.0) } else { (q, p / q) }
}

/// The reflection's contrast mix on one stored cube texel value k (0..1)
/// with tint c: k^8 (1 - c) + k c (shaders.md, "Reflection pass value R").
pub fn reflection_contrast(k: f32, c: f32) -> f32 {
    k.powi(8) * (1.0 - c) + k * c
}

/// Halo's cube-map faces (bitmaps.md, "Cube maps"): the axis each looks
/// along, and where right and down in its image go.
const FACES: [([f32; 3], [f32; 3], [f32; 3]); 6] = [
    ([1.0, 0.0, 0.0], [0.0, 0.0, -1.0], [0.0, -1.0, 0.0]),
    ([-1.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.0, -1.0, 0.0]),
    ([0.0, 1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]),
    ([0.0, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, -1.0]),
    ([0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
    ([0.0, 0.0, -1.0], [-1.0, 0.0, 0.0], [0.0, -1.0, 0.0]),
];

fn dot3(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// The direction of texel (x, y) of face `face` of an n x n cube map
/// (`FACES`; the engine's cube textures share the layout).
pub fn cube_direction(face: usize, x: u32, y: u32, n: u32) -> [f32; 3] {
    let (axis, right, down) = FACES[face];
    let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
    let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
    [0, 1, 2].map(|k| axis[k] + u * right[k] + v * down[k])
}

/// The face and texel of an n x n cube map that direction `d` reads
/// (nearest).
pub fn cube_texel(d: [f32; 3], n: u32) -> (usize, u32, u32) {
    let face = (0..6)
        .max_by(|&a, &b| dot3(FACES[a].0, d).total_cmp(&dot3(FACES[b].0, d)))
        .unwrap_or(0);
    let (axis, right, down) = FACES[face];
    let t = dot3(axis, d).max(1e-12);
    let at = |s: f32| (((s / t + 1.0) / 2.0 * n as f32) as i64).clamp(0, n as i64 - 1) as u32;
    (face, at(dot3(right, d)), at(dot3(down, d)))
}

/// Halo's direction for the engine's cube-sampling direction `s`: the
/// engine's materials sample a cube at (x, -z, y) of an engine vector
/// (Source's frame), and an engine vector is (-y, z, -x) of Halo's
/// (`space`), so s = (-h.y, h.x, h.z). The reflection is taken in Halo's
/// world frame (shaders.md, "Reflection pass value R": the eye vector
/// reflected about the world-space normal).
pub fn halo_direction(s: [f32; 3]) -> [f32; 3] {
    [s[1], -s[0], s[2]]
}

/// Halo's cube map (six square faces' top levels, RGBA8, n x n) as the
/// engine's (`MapCubemap`): resampled into the engine's frame
/// (`halo_direction`, nearest), each colour channel's reflection
/// contrast baked in (`reflection_contrast` on the stored values, tint
/// `contrast`), with box-filtered mips.
pub fn engine_cube(name: String, faces: &[Vec<u8>], n: u32, contrast: [f32; 3]) -> crate::map::MapCubemap {
    let mut levels: Vec<Vec<u8>> = (0..6)
        .map(|f| {
            let mut out = vec![0u8; (n * n * 4) as usize];
            for y in 0..n {
                for x in 0..n {
                    let (hf, hx, hy) = cube_texel(halo_direction(cube_direction(f, x, y, n)), n);
                    let i = ((hy * n + hx) * 4) as usize;
                    let o = ((y * n + x) * 4) as usize;
                    for k in 0..3 {
                        let v = faces[hf].get(i + k).copied().unwrap_or(0) as f32 / 255.0;
                        out[o + k] = (reflection_contrast(v, contrast[k]) * 255.0).round() as u8;
                    }
                    out[o + 3] = 255;
                }
            }
            out
        })
        .collect();
    let mips = n.max(1).ilog2() + 1;
    for face in &mut levels {
        let mut size = n;
        let mut last = face.clone();
        while size > 1 {
            let half = size / 2;
            let mut next = vec![0u8; (half * half * 4) as usize];
            for y in 0..half {
                for x in 0..half {
                    for k in 0..4 {
                        let at = |dx: u32, dy: u32| last[(((2 * y + dy) * size + 2 * x + dx) * 4 + k) as usize] as u32;
                        next[((y * half + x) * 4 + k) as usize] =
                            ((at(0, 0) + at(1, 0) + at(0, 1) + at(1, 1) + 2) / 4) as u8;
                    }
                }
            }
            face.extend_from_slice(&next);
            last = next;
            size = half;
        }
    }
    let faces: [Vec<u8>; 6] = std::array::from_fn(|i| std::mem::take(&mut levels[i]));
    crate::map::MapCubemap {
        name,
        size: n,
        mips,
        faces,
    }
}

/// A cube request decoded (`engine_cube`); `decode` off gives a cube of
/// the right size without texels.
pub fn cube(map: &MapFile, r: &CubeRequest, decode: bool) -> Result<crate::map::MapCubemap, String> {
    let name = map
        .tag(TagId(r.bitmap))
        .map_or_else(|| format!("bitmap {:#010x}", r.bitmap), |t| t.path.replace('\\', "/"));
    let image = first_image(map, r.bitmap).ok_or_else(|| format!("{name}: does not read"))?;
    if image.image_type != ImageType::CubeMap {
        return Err(format!("{name}: not a cube map"));
    }
    let n = image.width as u32;
    if !decode {
        return Ok(crate::map::MapCubemap {
            name,
            size: n,
            mips: 0,
            faces: Default::default(),
        });
    }
    let decoded = image.decode(map).map_err(|e| format!("{name}: {e}"))?;
    if decoded.faces.len() != 6 {
        return Err(format!("{name}: {} faces", decoded.faces.len()));
    }
    let tops: Vec<Vec<u8>> = decoded
        .faces
        .iter()
        .map(|levels| {
            levels
                .first()
                .map_or_else(Vec::new, |l| l.rgba[..(n * n * 4) as usize].to_vec())
        })
        .collect();
    Ok(engine_cube(name, &tops, n, r.contrast.map(f32::from_bits)))
}

/// A detail request as the mesh's `MapDetail` with texture index `texture`.
pub fn map_detail(d: &DetailRequest, texture: usize) -> MapDetail {
    MapDetail {
        texture,
        scale: d.scale,
        factor: 1.0,
        mode: d.mode,
        tint: [1.0; 3],
        rotation: 0.0,
    }
}

/// The texture's name: the bitmap's tag path (`/` separators), marked
/// when read as stored or cut out.
fn texture_name(map: &MapFile, r: &TextureRequest) -> String {
    let path = map
        .tag(TagId(r.bitmap))
        .map_or_else(|| format!("bitmap {:#010x}", r.bitmap), |t| t.path.replace('\\', "/"));
    if r.combine.is_some() {
        return format!("{path} (maps combined)");
    }
    match r.derive {
        Derive::None => {}
        Derive::Channel(c) => return format!("{path} (channel {c})"),
        Derive::SelfIllum { .. } if r.cutout.is_none() => return format!("{path} (self-illumination in alpha)"),
        Derive::SelfIllum { .. } => {}
    }
    match (r.srgb, r.cutout.is_some()) {
        (true, false) => path,
        (false, _) => format!("{path} (as stored)"),
        (true, true) => format!("{path} (cut out by its bump map)"),
    }
}

/// Every requested texture, decoded on threads in `requests`' order (the
/// map's textures, in that order); `decode` off gives placeholders of
/// the right size.
pub fn decode_all(map: &MapFile, requests: &[TextureRequest], decode: bool) -> Vec<Result<MapTexture, String>> {
    let items: Vec<TextureRequest> = requests.to_vec();
    crate::map::parallel::par_map(items, |r| texture(map, &r, decode))
}

/// One texture (module doc).
pub fn texture(map: &MapFile, r: &TextureRequest, decode: bool) -> Result<MapTexture, String> {
    let name = texture_name(map, r);
    if let Some(shader) = r.combine {
        return combined(map, shader, name, decode);
    }
    let tag = map.tag(TagId(r.bitmap)).ok_or_else(|| format!("{name}: no such tag"))?;
    let bitmap = Bitmap::read(map, tag).map_err(|e| format!("{name}: {e}"))?;
    let image = bitmap.images.first().ok_or_else(|| format!("{name}: no images"))?;
    let (width, height) = (image.width as u32, image.height as u32);
    if !decode {
        return Ok(MapTexture {
            name,
            srgb: r.srgb,
            mips: Vec::new(),
            width,
            height,
            rgba8: Vec::new(),
            bc: None,
        });
    }
    let decoded = image.decode(map).map_err(|e| format!("{name}: {e}"))?;
    let levels = decoded.faces.first().ok_or_else(|| format!("{name}: no face"))?;
    let plane = |l: &blam_formats::bitmap::DecodedLevel| l.rgba[..(l.width * l.height * 4) as usize].to_vec();
    let level0 = levels.first().ok_or_else(|| format!("{name}: no level"))?;
    let mut rgba8 = plane(level0);
    let full_chain = width.max(height).max(1).ilog2() as usize + 1;
    let mut texture = MapTexture {
        name,
        srgb: r.srgb,
        mips: Vec::new(),
        width,
        height,
        rgba8: Vec::new(),
        bc: None,
    };
    if let Some(c) = r.cutout {
        let scale = c.scale.map(f32::from_bits);
        let bump = first_image(map, c.bump)
            .ok_or_else(|| format!("{}: its bump map does not read", texture.name))?
            .decode_level(map, 0, 0)
            .map_err(|e| format!("{}: bump map: {e}", texture.name))?;
        bake_cutout(&mut rgba8, width, height, &bump.rgba, bump.width, bump.height, scale);
        texture.rgba8 = rgba8;
        texture.mips = texture.box_mips();
        return Ok(texture);
    }
    match r.derive {
        Derive::None => {}
        Derive::Channel(c) => {
            channel_mask(&mut rgba8, c);
            texture.rgba8 = rgba8;
            texture.mips = texture.box_mips();
            return Ok(texture);
        }
        Derive::SelfIllum { map: si, weights } => {
            let si = first_image(map, si)
                .ok_or_else(|| format!("{}: its self-illumination map does not read", texture.name))?
                .decode_level(map, 0, 0)
                .map_err(|e| format!("{}: self-illumination map: {e}", texture.name))?;
            let weights = weights.map(f32::from_bits);
            bake_self_illumination(&mut rgba8, width, height, &si.rgba, si.width, si.height, weights);
            texture.rgba8 = rgba8;
            texture.mips = texture.box_mips();
            return Ok(texture);
        }
    }
    texture.rgba8 = rgba8;
    texture.mips = if levels.len() == full_chain && image.image_type == ImageType::Texture2d {
        levels[1..].iter().map(plane).collect()
    } else {
        texture.box_mips()
    };
    if image.format.is_block_compressed()
        && image.image_type == ImageType::Texture2d
        && let Ok(Some(blocks)) = image.blocks(map)
        && let Some(face) = blocks.faces.first()
    {
        let format = match image.format {
            blam_formats::bitmap::PixelFormat::Dxt1 => MapBcFormat::Bc1,
            blam_formats::bitmap::PixelFormat::Dxt3 => MapBcFormat::Bc2,
            _ => MapBcFormat::Bc3,
        };
        let bc = MapBc {
            format,
            levels: face.iter().map(|l| l.data.to_vec()).collect(),
        };
        if bc.fits(width, height) {
            texture.bc = Some(bc);
        }
    }
    Ok(texture)
}

/// Whether a chicago shader's maps are combined into one texture: two or
/// more maps, each on whole-number scales (so one repeat of the
/// coordinates holds whole repeats of every map); offsets, rotation and
/// animation are ignored.
fn combinable(maps: &[ChicagoMap]) -> bool {
    let whole = |v: f32| (v - v.round()).abs() < 1e-3 && v.round() >= 1.0 && v.round() <= 64.0;
    maps.len() >= 2
        && maps
            .iter()
            .all(|m| whole(m.u_scale) && whole(m.v_scale) && !m.map.id.is_none())
}

/// One step of the chicago combine (shaders.md, "Combine"): a function
/// of the current value `c` and the next map's `n`; `ca` and `na` the
/// alphas for the blend codes.
fn chicago_function(f: ChicagoFunction, c: f32, n: f32, ca: f32, na: f32) -> f32 {
    use ChicagoFunction::*;
    match f {
        Current => c,
        NextMap => n,
        Multiply => c * n,
        DoubleMultiply => 2.0 * c * n,
        Add => c + n,
        AddSignedCurrent => n + 2.0 * c - 1.0,
        AddSignedNextMap => c + 2.0 * n - 1.0,
        SubtractCurrent => n - c,
        SubtractNextMap => c - n,
        BlendCurrentAlpha => n * ca + c * (1.0 - ca),
        BlendCurrentAlphaInverse => c * ca + n * (1.0 - ca),
        BlendNextMapAlpha => n * na + c * (1.0 - na),
        BlendNextMapAlphaInverse => c * na + n * (1.0 - na),
    }
    .clamp(-1.0, 1.0)
}

/// A chicago shader's maps combined into one texture (`combinable`): one
/// repeat of the coordinates, each map sampled nearest at its own scale,
/// the shaders.md combine in the stored (gamma) values. The size is the
/// largest map times its scale, rounded up to a power of two, at most
/// 1024; `decode` off gives a placeholder of that size.
fn combined(map: &MapFile, shader: u32, name: String, decode: bool) -> Result<MapTexture, String> {
    let tag = map
        .tag(TagId(shader))
        .ok_or_else(|| format!("{name}: no such shader"))?;
    let Shader::TransparentChicago(s) = Shader::read(map, tag).map_err(|e| format!("{name}: {e}"))? else {
        return Err(format!("{name}: not a chicago shader"));
    };
    let mut width = 4u32;
    let mut height = 4u32;
    let mut images = Vec::with_capacity(s.maps.len());
    for m in &s.maps {
        let image = first_image(map, m.map.id.0).ok_or_else(|| format!("{name}: a map does not read"))?;
        width = width.max((image.width as f32 * m.u_scale).round() as u32);
        height = height.max((image.height as f32 * m.v_scale).round() as u32);
        images.push(image);
    }
    let width = width.next_power_of_two().min(1024);
    let height = height.next_power_of_two().min(1024);
    let mut texture = MapTexture {
        name,
        srgb: true,
        mips: Vec::new(),
        width,
        height,
        rgba8: Vec::new(),
        bc: None,
    };
    if !decode {
        return Ok(texture);
    }
    let levels = images
        .iter()
        .map(|i| i.decode_level(map, 0, 0).map_err(|e| format!("{}: {e}", texture.name)))
        .collect::<Result<Vec<_>, _>>()?;
    let mut rgba = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let (u, v) = ((x as f32 + 0.5) / width as f32, (y as f32 + 0.5) / height as f32);
            let sample = |i: usize| -> [f32; 4] {
                let (l, m) = (&levels[i], &s.maps[i]);
                let su = (u * m.u_scale).rem_euclid(1.0);
                let sv = (v * m.v_scale).rem_euclid(1.0);
                let sx = ((su * l.width as f32) as u32).min(l.width - 1);
                let sy = ((sv * l.height as f32) as u32).min(l.height - 1);
                let o = ((sy * l.width + sx) * 4) as usize;
                [0, 1, 2, 3].map(|k| l.rgba[o + k] as f32 / 255.0)
            };
            let mut c = sample(0);
            for i in 0..s.maps.len() - 1 {
                let mut n = sample(i + 1);
                if s.maps[i].flags & ChicagoMap::ALPHA_REPLICATE != 0 {
                    n = [n[3]; 4];
                }
                let f = s.maps[i].color_function;
                let g = s.maps[i].alpha_function;
                let (ca, na) = (c[3], n[3]);
                let rgb = [0, 1, 2].map(|k| chicago_function(f, c[k], n[k], ca, na));
                c = [rgb[0], rgb[1], rgb[2], chicago_function(g, ca, na, ca, na)];
            }
            let o = ((y * width + x) * 4) as usize;
            for k in 0..4 {
                rgba[o + k] = (c[k].clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
    }
    texture.rgba8 = rgba;
    texture.mips = texture.box_mips();
    Ok(texture)
}

/// Bake a cut-out into `rgba` (`width` x `height`): each texel's alpha
/// becomes 255 where the bump texel under it (at its centre's coordinate
/// times `scale`, wrapped, nearest) has alpha above 127, else 0
/// (shaders.md, "Alpha testing").
pub fn bake_cutout(rgba: &mut [u8], width: u32, height: u32, bump: &[u8], bw: u32, bh: u32, scale: [f32; 2]) {
    if bw == 0 || bh == 0 {
        return;
    }
    for y in 0..height {
        for x in 0..width {
            let u = (x as f32 + 0.5) / width as f32 * scale[0];
            let v = (y as f32 + 0.5) / height as f32 * scale[1];
            let bx = ((u.rem_euclid(1.0) * bw as f32) as u32).min(bw - 1);
            let by = ((v.rem_euclid(1.0) * bh as f32) as u32).min(bh - 1);
            let a = bump[((by * bw + bx) * 4 + 3) as usize];
            rgba[((y * width + x) * 4 + 3) as usize] = if a > 127 { 255 } else { 0 };
        }
    }
}

/// Copy channel `c` (0 red .. 3 alpha) of every RGBA8 texel into all four.
pub fn channel_mask(rgba: &mut [u8], c: u8) {
    let c = usize::from(c.min(3));
    for t in rgba.as_chunks_mut::<4>().0 {
        let v = t[c];
        t.fill(v);
    }
}

/// The textures the looks want, each once: their order and each one's
/// index.
#[derive(Default)]
pub struct Requests {
    pub list: Vec<TextureRequest>,
    index: BTreeMap<TextureRequest, usize>,
}

impl Requests {
    /// The index of `r` among the requests (added when new).
    pub fn want(&mut self, r: TextureRequest) -> usize {
        *self.index.entry(r).or_insert_with(|| {
            self.list.push(r);
            self.list.len() - 1
        })
    }
}
