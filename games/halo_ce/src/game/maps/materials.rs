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
//! - Detail as the engine draws it: Source's mode 0 (`albedo x 2 x
//!   detail`, `DetailMode::Source(0)`) with the detail texture read as
//!   stored (`srgb` false, so the texel is the stored byte / 255): the
//!   game's clamp(2 x base x detail) in gamma space, neutral at the same
//!   detail value (1/2); see "Known shortcuts". Multiply is Source's mode
//!   8 (`albedo x detail`).
//! - Transparent generic (`sotr`) and chicago (`schi`): map 0 with its
//!   texture transform (scale, offset, rotation, the animation at time 0)
//!   as `MapMesh::base_transform`; blend function 0 (alpha blend) and 7
//!   (alpha-multiply add) blend, 3 (add) adds (`MapAlpha::Blend`/`Add`);
//!   alpha-tested masks at 0.5; two-sided by flag bit 2; unlit. A
//!   generic shader's tint is the upper constant colour 0 of the first
//!   stage that multiplies it with map 0.
//! - Model (`soso`, "Minimum faithful approximation (model)"): the base
//!   map at the shader's map scales, the detail map as detail, alpha-tested
//!   at 0.5 unless "not alpha-tested" (blended for an alpha-blended decal),
//!   two-sided where flagged, lit (by the placement's light: `objects`).
//! - Glass (`sgla`): the diffuse map (at its map scale) blended, lit by
//!   its group's lightmap, the diffuse detail map as detail, two-sided by
//!   flag. Water (`swat`): its base map blended, unlit, double-sided.
//! - Textures: RGBA8 with the stored mip levels when the file stores the
//!   whole chain (else box-filtered ones), and the stored blocks as `MapBc`
//!   for DXT1/3/5 when they fit (`MapBc::fits`). A cube map or a volume
//!   gives its first face or slice. With `MapLoadOptions::textures` off,
//!   each texture is a placeholder of its size (no texels).
//! - Every shader's header material type is the mesh's surface name
//!   (`surfaces`). A shader that is missing, unreadable, or of a class the
//!   level does not use gives a flat colour and a line in the map's
//!   warnings, as does an unreadable bitmap; the load never fails.
//!
//! Known shortcuts (against shaders.md):
//! - A chicago shader of two or more maps on whole-number scales is
//!   combined into one baked texture (`combined`: the spec's combine at
//!   nearest sampling, offsets, rotation and animation ignored: the sky's
//!   three-map base and cloud layer); other chicago shaders draw map 0
//!   alone.
//! - The model shader's change colour, self-illumination, reflection and
//!   translucency are not drawn, and its two-sided flag is honoured though
//!   the game ignores it (two-sided parts carry both windings).
//! - Detail is blended in linear light (`base_lin x 2 x d`), the game's in
//!   gamma (`(2 x base x d)` before the display), without the game's clamp
//!   ("Diffuse texture pass value D"); the secondary and micro detail maps
//!   and the detail blend types are not drawn.
//! - No bump shading of the lightmap, no self-illumination, specular
//!   lightmap or cube-map reflection ("How a level surface is drawn").
//! - Texture animations (environment u/v, transparent maps) are frozen at
//!   time 0; a transparent shader's maps after map 0, its stages and fade
//!   are not combined (generic: "need the full combiner").
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
        ModelShader, PeriodicTables, Shader, ShaderHeader, TextureTransform, TransparentCommon, no_object,
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
}

impl Default for Look {
    fn default() -> Self {
        Look {
            base: None,
            detail: None,
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
fn linear(c: f32) -> f32 {
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
pub fn look(map: &MapFile, shader: &TagRef) -> Look {
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
    let tables = PeriodicTables::new();
    let colour = |r: Option<u32>| {
        r.map(|bitmap| TextureRequest {
            bitmap,
            srgb: true,
            cutout: None,
            combine: None,
        })
    };
    match &shader {
        Shader::Environment(s) => environment(map, s),
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
            Look {
                base: colour(map0.and_then(|m| tag_id(&m.map))),
                alpha,
                double_sided: s.common.flags & TransparentCommon::TWO_SIDED != 0,
                unlit: true,
                tint,
                transform: map0.map_or(MapUvTransform::IDENTITY, |m| {
                    uv_transform(m.texture_transform(&tables, 0.0, &no_object))
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
                        uv_transform(m.texture_transform(&tables, 0.0, &no_object))
                    })
                },
                surface: surface(&s.header),
                transparent: true,
                problem,
                ..Look::default()
            }
        }
        Shader::Model(s) => model_shader(s),
        Shader::Glass(s) => {
            let base = colour(tag_id(&s.diffuse_map));
            let detail = tag_id(&s.diffuse_detail_map).map(|d| DetailRequest {
                texture: TextureRequest {
                    bitmap: d,
                    srgb: false,
                    cutout: None,
                    combine: None,
                },
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
        other => Look {
            base: other
                .bitmap_references()
                .first()
                .and_then(tag_id)
                .map(|b| TextureRequest {
                    bitmap: b,
                    srgb: true,
                    cutout: None,
                    combine: None,
                }),
            surface: surface(other.header()),
            transparent: true,
            unlit: true,
            alpha: MapAlpha::Blend,
            problem: Some(format!(
                "shader class {} on level geometry: its first map, blended",
                other.class()
            )),
            ..Look::default()
        },
    }
}

/// The model shader's look (shaders.md, "Minimum faithful approximation
/// (model)"): the base map at the shader's own map scales (the model's
/// are in its texture coordinates), the detail map as a second texture
/// (double biased multiply or multiply), alpha-tested at 0.5 from the
/// base alpha unless "not alpha-tested", blended for an alpha-blended
/// decal, two-sided where flagged. Lit by the placement's light, not
/// unlit; change colour, self-illumination and reflection are not drawn.
fn model_shader(s: &ModelShader) -> Look {
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
            texture: TextureRequest {
                bitmap: d,
                srgb: false,
                cutout: None,
                combine: None,
            },
            scale: [scale, scale * nonzero(s.detail_map_v_scale)],
            mode,
        }
    });
    Look {
        base: tag_id(&s.base_map).map(|bitmap| TextureRequest {
            bitmap,
            srgb: true,
            cutout: None,
            combine: None,
        }),
        detail,
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

fn nonzero(s: f32) -> f32 {
    if s == 0.0 { 1.0 } else { s }
}

/// The environment shader's look (module doc).
fn environment(map: &MapFile, s: &EnvironmentShader) -> Look {
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
    // it is the only one.
    let (detail_id, detail_scale) = match (tag_id(&s.primary_detail_map), tag_id(&s.secondary_detail_map)) {
        (Some(p), _) => (Some(p), s.primary_detail_scale),
        (None, Some(q)) if s.environment_type == EnvironmentType::Normal => (Some(q), s.secondary_detail_scale),
        _ => (None, 0.0),
    };
    let mode = match s.detail_function {
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
            texture: TextureRequest {
                bitmap: d,
                srgb: false,
                cutout: None,
                combine: None,
            },
            scale,
            mode,
        }
    });
    let c = s.material_color;
    let tint = ([c.r, c.g, c.b] != [1.0; 3]).then(|| [linear(c.r), linear(c.g), linear(c.b)]);
    Look {
        base: base_id.map(|bitmap| TextureRequest {
            bitmap,
            srgb: true,
            cutout,
            combine: None,
        }),
        detail,
        alpha,
        tint,
        surface: surface(&s.header),
        problem,
        ..Look::default()
    }
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
