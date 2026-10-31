//! The map's data: what a game's map importer produces and the engine
//! spawns (`MapData` and everything it holds). Units are meters, Y up.

//!
//! Plain data: it uses `core` and the data types of `map`'s submodules
//! (what a map holds of each feature), never their systems.
//!
//! `MapData` is the map's world plus two sub-structs for what is not the
//! map's file: `kit` (`GameKit`: what the map's game brings to every map,
//! which a mashup replaces with the gameplay's) and `extras` (`MapExtras`,
//! typed extensions such as Source's effect lists, `SourceEffects`).

use std::sync::Arc;

use bevy::prelude::*;

use super::{
    DetailMode, MapEntity, MapExtras, MapRagdoll, MapSelfIllum, MapSounds, MapTextureAnim, MapUvTransform, MapViewModel, SourceEffects, anim, beams,
    bot_profiles, breakables, color_correction, decal, emitters, hud, lod, nav, particles, psys, radio, rope_sway,
    shells, vis, water,
};
use crate::core::{MapBrush, MapBrushTree, MapWaterVolume, Team};

/// An RGBA8 texture in sRGB, top row first.
#[derive(Clone, Debug)]
pub struct MapTexture {
    /// Source path, e.g. `materials/de_dust/sitebwall01.vtf`.
    pub name: String,
    /// Color data (sRGB) or data such as normal maps (linear).
    pub srgb: bool,
    /// Smaller mip levels shipped with the texture (RGBA8, halving each
    /// time). Empty: generate them.
    pub mips: Vec<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub rgba8: Vec<u8>,
    /// The same image as the source file stores it, when that is block
    /// compressed (DXT1/3/5): uploaded as is where the GPU takes BC
    /// (`render::to_image`), `rgba8` and `mips` being what the CPU reads
    /// and the fallback. None: no compressed copy (lean loads, other formats).
    pub bc: Option<MapBc>,
}

/// BC1/2/3 blocks (DXT1/3/5) of a texture, each level's rows from the top.
#[derive(Clone, Debug, PartialEq)]
pub struct MapBc {
    pub format: MapBcFormat,
    /// Level 0 is the texture's size; each next one halves it (at least
    /// 1 x 1, one block for anything under 4 x 4). The chain may stop above
    /// 1 x 1 (Lithtech's files stop at 4 x 4).
    pub levels: Vec<Vec<u8>>,
}

/// A block compression format (`MapTexture::srgb` chooses the colour
/// space).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapBcFormat {
    /// DXT1: 8 bytes a block, colour and a one-bit alpha.
    Bc1,
    /// DXT3: 16 bytes, explicit 4-bit alpha.
    Bc2,
    /// DXT5: 16 bytes, interpolated alpha.
    Bc3,
}

impl MapBc {
    /// The bytes of a level `w` x `h` texels in `format`.
    pub fn level_bytes(format: MapBcFormat, w: u32, h: u32) -> usize {
        let block = if format == MapBcFormat::Bc1 { 8 } else { 16 };
        w.div_ceil(4) as usize * h.div_ceil(4) as usize * block
    }
}

impl MapTexture {
    /// The mip levels below `rgba8`'s, box-filtered (each texel the mean
    /// of the 2 x 2 above it, edges clamped), down to 1 x 1: what the
    /// drawing generates for a texture without its own. A loader that
    /// fills `mips` with these on its own threads spares the main thread
    /// that work when the map spawns.
    pub fn box_mips(&self) -> Vec<Vec<u8>> {
        let (mut w, mut h) = (self.width as usize, self.height as usize);
        let mut out: Vec<Vec<u8>> = Vec::new();
        while w > 1 || h > 1 {
            let level = out.last().unwrap_or(&self.rgba8);
            let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
            let mut next = vec![0u8; nw * nh * 4];
            for y in 0..nh {
                for x in 0..nw {
                    for c in 0..4 {
                        let mut sum = 0u32;
                        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                            let sx = (x * 2 + dx).min(w - 1);
                            let sy = (y * 2 + dy).min(h - 1);
                            sum += level[(sy * w + sx) * 4 + c] as u32;
                        }
                        next[(y * nw + x) * 4 + c] = (sum / 4) as u8;
                    }
                }
            }
            out.push(next);
            (w, h) = (nw, nh);
        }
        out
    }
}

/// How a surface's alpha is used.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MapAlpha {
    #[default]
    Opaque,
    /// Cut out below this alpha.
    Mask(f32),
    Blend,
    /// Added to what's behind (Source `$additive`: light glows, beams).
    Add,
}

impl MapAlpha {
    /// The material shaders' `translucent` parameter: 0 opaque, 1 blend,
    /// 2 additive.
    pub fn shader_mode(self) -> f32 {
        match self {
            MapAlpha::Opaque | MapAlpha::Mask(_) => 0.0,
            MapAlpha::Blend => 1.0,
            MapAlpha::Add => 2.0,
        }
    }

    /// Blend state for the material shaders (they output alpha 0 when
    /// additive, so premultiplied blending adds).
    #[cfg(feature = "render")]
    pub fn shader_alpha_mode(self) -> AlphaMode {
        match self {
            MapAlpha::Opaque | MapAlpha::Mask(_) => AlphaMode::Opaque,
            MapAlpha::Blend => AlphaMode::Blend,
            MapAlpha::Add => AlphaMode::Add,
        }
    }
}

/// Triangles sharing one material.
#[derive(Clone, Debug, Default)]
pub struct MapMesh {
    /// The source game's material name, e.g. `de_dust/sitebwall01`.
    pub material: String,
    /// Part of the 3D skybox (drawn by the sky camera, see `MapSkyCamera`).
    pub skybox: bool,
    pub positions: Vec<[f32; 3]>,
    pub normals: Vec<[f32; 3]>,
    /// Texture coordinates in texture repeats (1.0 = one texture width).
    pub uvs: Vec<[f32; 2]>,
    /// Skinned meshes: up to four bones per vertex (indices into the
    /// model's `MapBone`s) and their weights (summing to 1). Empty when
    /// the mesh isn't skinned.
    pub joints: Vec<[u16; 4]>,
    pub joint_weights: Vec<[f32; 4]>,
    /// Counter-clockwise triangles when seen from the front.
    pub indices: Vec<u32>,
    /// Flat color, used when there's no texture.
    pub color: [u8; 3],
    /// Index into `MapData::textures`.
    pub texture: Option<usize>,
    pub alpha: MapAlpha,
    pub double_sided: bool,
    /// Tangent-space normal map (index into `MapData::textures`, linear).
    pub normal_map: Option<usize>,
    /// Per-vertex coordinates into `MapData::lightmap` (0..1); empty when
    /// the map has no baked lighting.
    pub lightmap_uvs: Vec<[f32; 2]>,
    /// A second texture blended in per vertex (Source's WorldVertexTransition
    /// on displacements).
    pub blend: Option<MapBlend>,
    /// Per-vertex blend weight: 0 = first texture, 1 = second. Empty when
    /// there's no blend.
    pub blend_weights: Vec<f32>,
    /// A second set of texture coordinates (texture repeats), where the
    /// second texture of `blend` is sampled when the source game gives it
    /// coordinates of its own (LithTech's dual-texture sections). Empty:
    /// the second texture uses the base coordinates (`MapBlend::transform`).
    pub uvs2: Vec<[f32; 2]>,
    /// A detail texture tiled over the base texture.
    pub detail: Option<MapDetail>,
    /// `$basetexturetransform` (or a TextureScroll proxy driving it): the
    /// base texture's coordinates, and the detail's before `$detailscale`.
    pub base_transform: MapUvTransform,
    /// `$selfillum`: glows with its own colour where the mask says.
    pub selfillum: Option<MapSelfIllum>,
    /// Drawn at the texture's own brightness, ignoring lighting (Source's
    /// UnlitGeneric).
    pub unlit: bool,
    /// How much of the lighting full brightness replaces, 0..1 (Source's
    /// ShatteredGlass `$unlitfactor`: a broken window's cracked panes keep
    /// most of their brightness in the shade); `unlit` wins.
    pub unlit_factor: f32,
    /// A map decal or overlay that multiplies the surface under it by 2 x
    /// its texel (Source's DecalModulate: 0.5 grey leaves it unchanged),
    /// unlit; with `alpha` Blend.
    pub modulate: bool,
    /// Surface property name (footsteps, impacts), lower-case.
    pub surface: Option<String>,
    /// A blended surface's second surface property (Source `$surfaceprop2`
    /// on WorldVertexTransition), lower-case: the triangles where the
    /// second texture shows (`blend_weights`) take it
    /// (`sound::SurfaceGrid`).
    pub surface2: Option<String>,
    /// Reflection of a baked cubemap (Source `$envmap`).
    pub envmap: Option<MapEnvmap>,
    /// A flat environment map over the base texture (LithTech's `envmap`
    /// link); None for every Source surface.
    pub flat_envmap: Option<MapFlatEnvmap>,
    /// Part of a mover brush entity (index into `MapData::entities`):
    /// positions are relative to that entity's origin and unrotated
    /// (engine axes), drawn under its `MapBrushEntity` node.
    pub entity: Option<usize>,
    /// Colour multiplier (linear) on the texture (Source `$color`/`$color2`).
    pub tint: Option<[f32; 3]>,
    /// A water surface: index into `MapData::water_materials` (drawn by
    /// `render::water::WaterMaterial` instead).
    pub water: Option<usize>,
    /// A model mesh that belongs to one choice of a body part with
    /// several (part, choice): drawn only while the prop's body picks it
    /// (`MapModel::body_parts`).
    pub body: Option<(u16, u16)>,
    /// Model meshes: each vertex's index in the model's own vertex list
    /// (Source `.vvd` order), for light baked per vertex of the model
    /// (`MapProp::vertex_light`). Empty otherwise.
    pub source_vertices: Vec<u32>,
    /// What part of a breakable window's look it is (`PaneLook`).
    pub pane_look: PaneLook,
    /// A brush entity's translucent render mode (Source rendermode and
    /// renderamt): blended or added, and its opacity (`RenderLook`).
    pub render: Option<(MapAlpha, f32)>,
    /// Its rendercolor (sRGB 0-255; white: none), multiplied into the
    /// material's colour in any render mode.
    pub render_color: Option<[u8; 3]>,
    /// Render mode 1 (kRenderTransColor): drawn as the flat `render_color`
    /// (the loader drops the texture and sets `color` white).
    pub flat_color: bool,
    /// Its base texture's frames (indices into `MapData::textures`), when a
    /// map entity can pick one (env_texturetoggle; `render::tint::TextureFrames`).
    pub frames: Vec<usize>,
    /// A window's broken looks (`PaneLook::Cracked` and `Edge`): the jagged
    /// edge masks, square tiles side by side in one texture (index into
    /// `MapData::textures`), whose alpha cuts each edge strip (the strip
    /// picks its tile and lays it out by its vertex colour, while the
    /// material keeps the face's coordinates). The cracked face carries it
    /// too, unused, so a window's broken looks share one material.
    pub pane_mask: Option<usize>,
    /// A model skin that is a looping sequence of textures (`texture` its
    /// first frame), each drawn copy on its own clock (`MapSkinFrames`).
    pub skin_frames: Option<MapSkinFrames>,
    /// A second texture on generated coordinates combined with the base
    /// (`MapLayer`: environment and rim maps).
    pub layer: Option<MapLayer>,
}

impl MapMesh {
    /// The colour multiplier of its entity's render mode (an added look
    /// scales the colour, a blended one the alpha).
    pub fn render_factor(&self) -> Vec4 {
        let [r, g, b] = self.render_color.unwrap_or([255; 3]);
        // The game multiplies in gamma space: the colour is sRGB.
        let c = Color::srgb_u8(r, g, b).to_linear();
        let rgb = Vec3::new(c.red, c.green, c.blue);
        match self.render {
            Some((MapAlpha::Add, a)) => (rgb * a).extend(1.0),
            Some((_, a)) => rgb.extend(a),
            None => rgb.extend(1.0),
        }
    }
}

/// A window's looks (meshes of a brush entity drawn as panes,
/// `breakables::BrushPanes`): its face as placed, then, once it breaks,
/// the face in its cracked material over the unbroken panes and jagged
/// edge pieces where they meet broken ones.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PaneLook {
    /// Drawn while whole (every mesh but a window's broken looks).
    #[default]
    Whole,
    /// The face in its broken material (Source `$crackmaterial`): the
    /// same triangles, drawn over the unbroken panes once broken.
    Cracked,
    /// An edge piece (one of several, numbered): the face's triangles in
    /// its broken material as a template, from which strips along the edges
    /// of unbroken panes facing broken ones are cut
    /// (`breakables::edge_strips`), each cut by its jagged mask
    /// (`MapMesh::pane_mask`).
    Edge(u8),
}

/// How a model mesh looks under one skin family: the material fields of
/// `MapMesh` that a skin changes.
#[derive(Clone, Debug, Default)]
pub struct MapMeshLook {
    pub material: String,
    pub texture: Option<usize>,
    pub alpha: MapAlpha,
    pub double_sided: bool,
    pub unlit: bool,
    /// `$bumpmap` (index into `MapData::textures`, linear): on models only
    /// its alpha is read, as a `$normalmapalphaenvmapmask` reflection mask.
    pub normal_map: Option<usize>,
    pub envmap: Option<MapEnvmap>,
    pub tint: Option<[f32; 3]>,
    pub detail: Option<MapDetail>,
    pub base_transform: MapUvTransform,
    pub selfillum: Option<MapSelfIllum>,
    pub skin_frames: Option<MapSkinFrames>,
    pub layer: Option<MapLayer>,
}

impl MapMeshLook {
    /// The mesh with this look.
    pub fn apply(&self, m: &MapMesh) -> MapMesh {
        MapMesh {
            material: self.material.clone(),
            texture: self.texture,
            alpha: self.alpha,
            double_sided: self.double_sided,
            unlit: self.unlit,
            normal_map: self.normal_map,
            envmap: self.envmap.clone(),
            tint: self.tint,
            detail: self.detail,
            base_transform: self.base_transform,
            selfillum: self.selfillum,
            skin_frames: self.skin_frames.clone(),
            layer: self.layer.clone(),
            ..m.clone()
        }
    }
}

/// A model's skeleton and what it can play (animated props).
#[derive(Clone, Debug)]
pub struct MapRig {
    /// Parents before children, in the source game's axes and units.
    pub bones: Vec<MapBone>,
    /// From the skeleton's space to the model's (engine axes, meters).
    pub root: Transform,
    pub animations: Arc<anim::AnimSet>,
}

/// A baked environment cubemap: six square RGBA8 sRGB faces in the
/// source game's face order and axes (Source: +X, -X, +Y, -Y, +Z, -Z in
/// its Z-up frame), sampled with directions in that frame. Each face holds
/// its mip chain, largest first (`mips` levels, each half the last).
#[derive(Clone, Debug)]
pub struct MapCubemap {
    pub name: String,
    pub size: u32,
    pub mips: u32,
    pub faces: [Vec<u8>; 6],
}

/// What scales a surface's reflection, per texel.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum EnvmapMask {
    #[default]
    None,
    /// The normal map's alpha.
    NormalAlpha,
    /// One minus the base texture's alpha (Source's inverted mask).
    BaseAlphaInverted,
    /// A mask texture's colour (index into `MapData::textures`).
    Texture(usize),
}

/// Reflection parameters (specs/cs_source/shaders.md, "$envmap"), after the
/// shader's fast-path rule has been applied.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapEnvmap {
    /// Index into `MapData::cubemaps`; None: the cubemap sample nearest
    /// to each object using the material (`MapData::cubemap_samples`).
    pub cubemap: Option<usize>,
    pub mask: EnvmapMask,
    pub tint: [f32; 3],
    pub contrast: f32,
    pub saturation: f32,
    /// Fresnel R0 (1: no fresnel).
    pub fresnel: f32,
    /// WindowImposter (a "fake sky" window): the surface shows the
    /// cubemap in the view direction, unlit and unfogged, instead of
    /// reflecting it.
    pub imposter: bool,
}

/// Source `$detail`: a texture tiled `scale` times per base texture repeat
/// and combined with the base color (specs/cs_source/shaders.md).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapDetail {
    /// Index into `MapData::textures` (sRGB-decoded or raw as the mode and
    /// shader want: shaders.md section 1).
    pub texture: usize,
    pub scale: [f32; 2],
    pub factor: f32,
    /// Source's `$detailblendmode`, or one of WorldTwoTextureBlend's own
    /// modes, which also light the surface their own way
    /// (specs/cs_source/shaders_two_texture_blend.md).
    pub mode: DetailMode,
    /// `$detailtint`, linear as the shader uses it (raw on world
    /// surfaces, gamma-converted on models).
    pub tint: [f32; 3],
    /// Counter-clockwise rotation of the detail coordinates, radians
    /// (`scale` then this: LithTech's detail rotation; 0 for Source).
    pub rotation: f32,
}

/// The second layer of a two-texture surface. Indices into
/// `MapData::textures`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MapBlend {
    pub texture: Option<usize>,
    pub normal_map: Option<usize>,
    /// Shapes the blend: green moves the transition point, red sets its
    /// softness (linear texture).
    pub mask: Option<usize>,
    /// `$basetexturetransform2`: the second texture's coordinates.
    pub transform: MapUvTransform,
    /// How the two textures combine.
    pub mode: BlendMode,
}

/// How a two-texture surface combines its textures, by the per-vertex
/// weight w (`MapMesh::blend_weights`, 1 = second texture).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BlendMode {
    /// Source's WorldVertexTransition: the colours mixed by w in linear
    /// space; alpha stays the first's.
    #[default]
    VertexTransition,
    /// LithTech's dual texture (specs/combat_arms/world_dat.md, "Second
    /// textures"): a = (1 - w) x the first texture's alpha, colour
    /// a x first + (1 - a) x second, alpha a; in gamma space, as the
    /// fixed-function blend runs (`material_fx::dual_texture`).
    BaseAlpha,
    /// The same with a fullbright second texture: first + a x second
    /// (clamped), alpha a.
    BaseAlphaAdd,
}

impl BlendMode {
    /// The world shader's `blend_mode` parameter (see world.wesl).
    pub fn shader_value(self) -> f32 {
        match self {
            Self::VertexTransition => 0.0,
            Self::BaseAlpha => 1.0,
            Self::BaseAlphaAdd => 2.0,
        }
    }
}

/// A flat (2D) environment map laid over a surface's base texture,
/// sampled by the view-space reflection vector r as
/// (r.x, r.y) x (-0.5 / `scale`) + 0.5 (LithTech's `envmap` and
/// `envmapalpha` links, specs/combat_arms/world_dat.md "Second textures";
/// `material_fx::flat_envmap_uv`), combined before lighting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MapFlatEnvmap {
    /// Index into `MapData::textures` (sRGB).
    pub texture: usize,
    /// `EnvScale`.
    pub scale: f32,
    /// Combined as the signed add base + env - 0.5 (clamped, gamma space);
    /// false: multiplied.
    pub signed_add: bool,
    /// Weighted by the base texture's alpha (`envmapalpha`).
    pub by_base_alpha: bool,
}

/// A model skin that is a looping sequence of textures (LithTech's sprite
/// skins, specs/combat_arms/models_ltb.md "Sprite skins"): each drawn
/// copy of the model starts its own clock at 0 when created, advances it
/// by each drawn frame's time in whole milliseconds and shows frame
/// floor(t / `frame_ms`) mod n, the whole texture swapped (no blending);
/// it always loops (`step`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapSkinFrames {
    /// Each frame's texture (indices into `MapData::textures`).
    pub frames: Vec<usize>,
    /// How long each frame shows, ms (at least 1).
    pub frame_ms: u32,
}

impl MapSkinFrames {
    /// The clock `t` (ms) advanced by `delta_ms`, wrapped to the
    /// sequence's length, and the frame it shows.
    pub fn step(&self, t: u32, delta_ms: u32) -> (u32, usize) {
        let n = self.frames.len().max(1) as u64;
        let p = u64::from(self.frame_ms.max(1));
        let t = (u64::from(t) + u64::from(delta_ms)) % (p * n);
        (t as u32, ((t / p) % n) as usize)
    }
}

/// Where a `MapLayer`'s coordinates come from: x and y of a vector in
/// camera space (x right, y up) or world space, used as u and v.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayerCoords {
    /// The view direction reflected about the normal, camera space.
    #[default]
    CameraReflection,
    /// The normal, camera space.
    CameraNormal,
    /// The camera-space reflection mapped by u = 0.5x + 0.5, v = 0.5y +
    /// 0.5 (a sphere map).
    SphereMap,
    /// The view direction reflected about the normal, world space.
    WorldReflection,
    /// The normal, world space.
    WorldNormal,
}

/// How coordinates outside 0..1 sample a `MapLayer`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayerAddress {
    Wrap,
    #[default]
    Clamp,
    Mirror,
}

/// How a `MapLayer`'s sample S combines with the base texture T (its
/// alpha T.a) and the source game's lit colour L (the drawn light times
/// `MapLayer::light`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum LayerCombine {
    /// T + T.a x S x L: the base unlit, its alpha masking a lit
    /// reflection.
    #[default]
    MaskedAdd,
    /// T x L + `factor` x S: a rim glow added.
    Add,
    /// `factor` x L x lerp(T, S, T.a): the reflection through the base's
    /// alpha, lit.
    AlphaLerp,
    /// (T x L + T.a x S) x (1 + T.a^2): the base's alpha a gloss mask.
    Gloss,
    /// LithTech's `pv_char_b`, its three passes in one (`pv_char_b`):
    /// the dot3 grey of the lit pass, plus the added pass, times the
    /// multiplied pass that reads S.
    PvCharB,
}

/// The fixed-function lighting material of a render-style pass (LithTech's:
/// the style file's material, specs/combat_arms/models_ltb.md Follow-up
/// (2026-10-31) 3). A pass with lighting on lights its vertices with it:
/// the lit colour is `lit(A, sigma)`, and a separate specular colour is
/// added to the pixel after all the texture stages (`specular`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayerLighting {
    pub emissive: Vec3,
    pub ambient: Vec3,
    pub diffuse: Vec3,
    pub specular: Vec3,
    pub power: f32,
}

impl LayerLighting {
    /// L = clamp(M_e + M_a x A + M_d x sigma), per channel, with `ambient`
    /// the ambient light the engine gives the model and `direct` the sum
    /// over lights of colour x max(0, N.D) x falloff.
    pub fn lit(&self, ambient: Vec3, direct: Vec3) -> Vec3 {
        (self.emissive + self.ambient * ambient + self.diffuse * direct).clamp(Vec3::ZERO, Vec3::ONE)
    }

    /// The specular colour S = M_s x sum_k C_k x max(0, N.H_k)^p over the
    /// lights `(colour, direction toward the light)` for a normal `n` seen
    /// from the unit direction `to_eye`; nothing when the material's
    /// specular colour is black or its power is 0.
    pub fn specular_of(&self, n: Vec3, to_eye: Vec3, lights: &[(Vec3, Vec3)]) -> Vec3 {
        if self.specular == Vec3::ZERO || self.power <= 0.0 {
            return Vec3::ZERO;
        }
        let sum: Vec3 = lights
            .iter()
            .map(|(c, d)| *c * n.dot((*d + to_eye).normalize_or_zero()).max(0.0).powf(self.power))
            .sum();
        self.specular * sum
    }
}

/// A second texture sampled on generated coordinates and combined with
/// the base (LithTech render styles' slot 1: environment and rim maps,
/// specs/combat_arms/models_ltb.md "Stock styles").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MapLayer {
    /// The source game's texture name, which its loader resolves into
    /// `texture`.
    pub material: String,
    /// Index into `MapData::textures`; None: not drawn.
    pub texture: Option<usize>,
    pub coords: LayerCoords,
    pub address: LayerAddress,
    pub combine: LayerCombine,
    pub factor: f32,
    /// The source game's lit colour as a fraction of the drawn light.
    pub light: f32,
    /// The pass's lighting material: the lit colour L comes from it (and a
    /// specular colour is added after the stages) on a probe-lit model,
    /// instead of `light`'s fraction.
    pub lighting: Option<LayerLighting>,
}

/// Baked lighting atlas: linear RGB, where 1.0 shows a texture at its own
/// brightness (above 1.0 is overbright).
#[derive(Clone, Debug, Default)]
pub struct MapLightmap {
    pub width: u32,
    pub height: u32,
    /// Row-major, top row first.
    pub rgb: Vec<[f32; 3]>,
    /// Directional lightmaps for normal-mapped surfaces (same layout), one
    /// per basis direction of radiosity normal mapping. Surfaces without
    /// them repeat `rgb`.
    pub bumped: Option<[Vec<[f32; 3]>; 3]>,
    /// Light styles that change: what each adds where it lights, so they
    /// can be turned on and off (`LightStyles`) or animate (`pattern`).
    /// `rgb` and `bumped` hold the lighting at map start (switchable
    /// styles with `on` and animated ones at brightness 1 included).
    pub styles: Vec<MapLightStyle>,
}

/// One light style's share of the lightmap atlas.
#[derive(Clone, Debug, Default)]
pub struct MapLightStyle {
    pub style: u8,
    /// Lit at map start.
    pub on: bool,
    /// Atlas texels (row-major index) it lights, and the light it adds
    /// there (same units as `MapLightmap::rgb`).
    pub texels: Vec<u32>,
    pub rgb: Vec<[f32; 3]>,
    /// What it adds to each directional page, per texel.
    pub bumped: Option<[Vec<[f32; 3]>; 3]>,
    /// An animated style's brightness per step (1 = as baked), at
    /// `LIGHT_STYLE_STEPS` steps a second, looping; empty for a style that
    /// only switches.
    pub pattern: Vec<f32>,
    /// The atlas rectangles it lights, one per face block (x, y, width,
    /// height, padding included), in the order `texels` lists them.
    pub rects: Vec<[u32; 4]>,
}

/// Steps a second of animated light styles' patterns (Quake's and
/// Source's light style rate: ten characters a second).
pub const LIGHT_STYLE_STEPS: f32 = 10.0;

impl MapLightStyle {
    /// Its brightness in `MapLightmap::rgb` (map start).
    pub fn start(&self) -> f32 {
        if !self.pattern.is_empty() || self.on { 1.0 } else { 0.0 }
    }

    /// An animated style's brightness `t` seconds in.
    pub fn at(&self, t: f32) -> f32 {
        if self.pattern.is_empty() {
            return self.start();
        }
        let step = (t.max(0.0) * LIGHT_STYLE_STEPS) as usize;
        self.pattern[step % self.pattern.len()]
    }
}

impl MapLightmap {
    /// The atlas (flat and directional pages) with each style at the
    /// brightness `level(style)` gives (switchable: 0 or 1).
    #[allow(clippy::type_complexity)]
    pub fn relit(&self, level: &dyn Fn(&MapLightStyle) -> f32) -> (Vec<[f32; 3]>, Option<[Vec<[f32; 3]>; 3]>) {
        let mut rgb = self.rgb.clone();
        let mut bumped = self.bumped.clone();
        for s in &self.styles {
            let sign = level(s) - s.start();
            if sign == 0.0 {
                continue;
            }
            let add = |dst: &mut [f32; 3], v: [f32; 3]| {
                for k in 0..3 {
                    dst[k] = (dst[k] + sign * v[k]).max(0.0);
                }
            };
            for (i, &t) in s.texels.iter().enumerate() {
                add(&mut rgb[t as usize], s.rgb[i]);
                if let (Some(pages), Some(src)) = (bumped.as_mut(), s.bumped.as_ref()) {
                    for k in 0..3 {
                        add(&mut pages[k][t as usize], src[k][i]);
                    }
                }
            }
        }
        (rgb, bumped)
    }
}

/// A reusable model (e.g. a window frame), in its own space: meters, Y up.
#[derive(Clone, Debug, Default)]
pub struct MapModel {
    pub meshes: Vec<MapMesh>,
    /// Collision box in model space (min, max), for box-solid props.
    pub bounds: (Vec3, Vec3),
    /// The model's own collision model (Source `.phy`), when it has one.
    pub collision: Option<MapCollision>,
    /// Surface property name (footsteps on the prop), lower-case.
    pub surfaceprop: Option<String>,
    /// Where the model is lit from, in model space (Source
    /// `$illumposition`), when the game gives one.
    pub illum: Option<Vec3>,
    /// Each skin family's look per mesh (same order as `meshes`), when
    /// the model has more than one; the meshes themselves show the skin
    /// they were loaded with.
    pub skins: Vec<Vec<MapMeshLook>>,
    /// Choices per body part (Source body groups); a prop's body number
    /// picks one per part (`body_choice`).
    pub body_parts: Vec<u16>,
    /// Skeleton and animations, for props that play sequences (meshes
    /// skinned to it).
    pub rig: Option<Arc<MapRig>>,
    /// What it breaks into, when it can break.
    pub breaks: Option<breakables::MapBreak>,
    /// Its lower levels of detail after `meshes` (LOD 0), coarsest last:
    /// drawn instead with distance and Video > Advanced's model detail
    /// (`lod`).
    pub lods: Vec<lod::MapLod>,
}

impl MapModel {
    /// Which choice of body part `part` a body number shows.
    pub fn body_choice(&self, body: i32, part: usize) -> u16 {
        body_choice(&self.body_parts, body, part)
    }
}

/// Which choice of body part `part` a body number shows, given each
/// part's choice count (Source: the body number in the mixed radix of
/// the parts' counts, the first part lowest).
pub fn body_choice(parts: &[u16], body: i32, part: usize) -> u16 {
    let base: i32 = parts[..part.min(parts.len())]
        .iter()
        .map(|n| (*n).max(1) as i32)
        .product();
    let n = parts.get(part).copied().unwrap_or(1).max(1) as i32;
    ((body.max(0) / base.max(1)) % n) as u16
}

/// A character body from the game: meshes in the character's local space
/// (feet at the origin, facing -Z) and its hitboxes in the same space.
#[derive(Clone, Debug)]
pub struct MapCharacterModel {
    /// The game whose body it is (`cs_source`); a character whose
    /// identity is another game's (`slots::GameIdentity`) never wears it.
    /// None: any game's.
    pub game: Option<&'static str>,
    /// The team it's for; None: anyone without a better match.
    pub team: Option<Team>,
    /// Set for a model only characters that name it use (`BodyName`, e.g.
    /// hostages); such models are never picked by team.
    pub name: Option<String>,
    /// The player class that wears it (`core::PlayerClass`; CS:S's
    /// `joinclass` number); such models are never picked by team: a
    /// team's own is its first class's.
    pub class: Option<u8>,
    pub model: MapModel,
    pub hitboxes: Vec<crate::core::Hitbox>,
    /// The skeleton the meshes are skinned to, parents before children.
    pub bones: Vec<MapBone>,
    /// From the skeleton's space (the source game's axes and units) to the
    /// character's local space.
    pub root: Transform,
    /// What the skeleton can play (None: it stays in the reference pose).
    pub animations: Option<Arc<anim::AnimSet>>,
    /// The hitboxes on their bones (same order as `hitboxes`), to follow
    /// the animated skeleton.
    pub boxes: Vec<BoneBox>,
    /// The body's physics when it dies (None: no ragdoll; the dead body
    /// is just hidden).
    pub ragdoll: Option<MapRagdoll>,
}

/// A hitbox in its bone's frame (the source game's axes and units).
#[derive(Clone, Debug)]
pub struct BoneBox {
    pub bone: usize,
    pub center: Vec3,
    pub half: Vec3,
    pub group: crate::core::Hitgroup,
}

/// A model characters hold (a weapon's world model), keyed by what
/// `Held` names; meshes in the frame of `bone` of the character skeleton
/// (its axes and units).
#[derive(Clone, Debug)]
pub struct MapHeldModel {
    pub key: String,
    pub model: MapModel,
    pub bone: String,
    /// Where it shoots from, in the bone's frame (muzzle flashes; +X
    /// forward).
    pub muzzle: Option<Transform>,
    /// Bone merge: parts that follow other bones of the skeleton (the
    /// dual pistols' left gun, on the left hand), each in its bone's
    /// frame; held in a hand, `model` then draws only what is left. Empty:
    /// all of it follows `bone`.
    pub merged: Vec<(String, MapModel)>,
    /// `model` without the `merged` parts (None: `merged` is empty).
    pub own: Option<MapModel>,
}

/// One bone of a character skeleton, in its reference pose relative to
/// its parent (the source game's axes and units).
#[derive(Clone, Debug)]
pub struct MapBone {
    pub name: String,
    pub parent: Option<usize>,
    pub position: Vec3,
    pub rotation: Quat,
}

/// A model's collision: convex pieces in model space, and the physics
/// parameters that came with them.
#[derive(Clone, Debug, Default)]
pub struct MapCollision {
    pub pieces: Vec<MapConvex>,
    /// kg.
    pub mass: f32,
    pub damping: f32,
    pub rotdamping: f32,
    /// Multiplier on the inertia tensor.
    pub inertia: f32,
    /// Rotational inertia per kilogram about the model's axes (engine
    /// axes, m²), when the collision model stores one.
    pub unit_inertia: Option<Vec3>,
    /// Surface property name (friction, elasticity, sounds).
    pub surfaceprop: String,
}

/// A convex piece: its corners and outward planes (n . p <= d inside).
#[derive(Clone, Debug, Default)]
pub struct MapConvex {
    pub points: Vec<Vec3>,
    pub planes: Vec<(Vec3, f32)>,
}

/// How players collide with a prop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PropSolid {
    None,
    /// The model's bounds as a box, rotated with the prop.
    Box,
    /// The box along the world's axes around the model's bounds as the
    /// prop turns them (Source's static props with the bbox solid:
    /// measured 2026-10-18, de_dust's windows turned 120°,
    /// specs/cs_source/physics_props.md Q9).
    WorldBox,
    /// The model's triangles.
    Mesh,
}

/// Lighting for something without a lightmap, sampled at one point: light
/// arriving from six axis directions (+X, -X, +Y, -Y, +Z, -Z) plus
/// directional lights (direction toward the light, color), all in lightmap
/// units. The same model Source uses for props and characters.
#[derive(Clone, Debug, Default)]
pub struct LightProbe {
    pub cube: [Vec3; 6],
    pub lights: Vec<(Vec3, Vec3)>,
}

impl LightProbe {
    /// Light on a surface with normal `n`.
    pub fn eval(&self, n: Vec3) -> Vec3 {
        let c = &self.cube;
        let pick = |v: f32, pos: Vec3, neg: Vec3| if v >= 0.0 { pos } else { neg };
        let ambient =
            pick(n.x, c[0], c[1]) * n.x * n.x + pick(n.y, c[2], c[3]) * n.y * n.y + pick(n.z, c[4], c[5]) * n.z * n.z;
        self.lights
            .iter()
            .fold(ambient, |acc, (dir, color)| acc + *color * n.dot(*dir).max(0.0))
    }
}

/// A placed model.
#[derive(Clone, Debug)]
pub struct MapProp {
    /// Index into `MapData::models`.
    pub model: usize,
    /// A fixed pose of its skeleton (each bone's local rotation and
    /// position, the model's frames and units): a map-placed ragdoll
    /// (prop_ragdoll) drawn as it lies. None: the reference pose, or its
    /// animations.
    pub pose: Option<Arc<Vec<(Quat, Vec3)>>>,
    /// A map-placed ragdoll's bodies and joints (its model's `.phy`): it is
    /// simulated from `pose` (`ragdoll::PlacedRagdoll`).
    pub ragdoll: Option<Arc<MapRagdoll>>,
    pub translation: Vec3,
    pub rotation: Quat,
    pub solid: PropSolid,
    /// Part of the 3D skybox.
    pub skybox: bool,
    /// Baked lighting; lit by the scene's lights when absent.
    pub lighting: Option<LightProbe>,
    /// Light baked per vertex for this placement (Source static props
    /// compiled with per-vertex lighting), by the model's own vertex index
    /// (`MapMesh::source_vertices`), linear lightmap units: replaces
    /// `lighting` for the meshes' colours.
    pub vertex_light: Option<Arc<Vec<[f32; 3]>>>,
    /// Casts a dynamic shadow onto the world (Source: entity props, not
    /// static props, whose shadows are baked into lightmaps).
    pub casts_shadow: bool,
    /// Simulated as a rigid body, when set.
    pub physics: Option<MapPhysics>,
    /// Fade distances from the camera (start, gone), meters, when the prop
    /// fades out with distance: dithered out between them (`vis::fade_band`),
    /// hidden beyond the far one.
    pub fade: Option<(f32, f32)>,
    /// The mover entity it's attached to (index into `MapData::entities`):
    /// it rides that entity's node (de_nuke's door handles) and isn't solid.
    pub parent: Option<usize>,
    /// The entity it was placed by (index into `MapData::entities`); its
    /// node then carries `PropEntity`.
    pub entity: Option<usize>,
    /// The skin family its meshes show (`MapModel::skins`).
    pub skin: i32,
    /// Its body number (`MapModel::body_choice`).
    pub body: i32,
    /// A detail model (the map's detail props: grass and bushes drawn as
    /// models), not one of its static props.
    pub detail: bool,
}

/// A physics prop's body (specs/cs_source/physics_props.md 3, 4).
#[derive(Clone, Debug)]
pub struct MapPhysics {
    /// kg.
    pub mass: f32,
    pub friction: f32,
    pub elasticity: f32,
    pub damping: f32,
    pub rotdamping: f32,
    /// Rotational inertia about the model's axes (engine axes, kg m²);
    /// None: computed from the collider's shape and the mass.
    pub inertia: Option<Vec3>,
    pub push: PushAway,
    /// Held in place (Source "motion disabled") until something enables
    /// its motion: it stays put like a static prop until then.
    pub frozen: bool,
    /// Starts asleep (Source spawnflag 1): it doesn't move until woken
    /// (`prop_physics::PropAwakened` then tells).
    pub asleep: bool,
}

/// How a physics prop and players interact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PushAway {
    /// Collides with players like a wall (pushed by their physics shadow).
    Collide,
    /// Players walk through it; it's shoved away from them, and pushes
    /// them back (multiplayer "solid" mode).
    Solid,
    /// Players walk through it; it's shoved away from them.
    NonSolid,
    /// Players walk through it and don't push it (multiplayer client-side
    /// props, with the default sv_pushaway_clientside 0).
    Ignore,
}

/// Dynamic prop shadows (Source `shadow_control`, specs/cs_source/shadows_sky.md):
/// one direction, colour and cast distance for the whole map.
#[derive(Clone, Debug)]
pub struct MapShadows {
    /// Unit vector from caster toward receiver, engine space.
    pub direction: Vec3,
    /// sRGB bytes.
    pub color: [u8; 3],
    /// How far past the caster a shadow reaches, meters.
    pub distance: f32,
}

/// Sound entries the announcer plays for rounds: a side's win, a draw,
/// and one picked at random when a round goes live; also the player's
/// own interface sounds (+use on nothing, the HUD's weapon selection).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct RoundSounds {
    pub attackers_win: Option<String>,
    pub defenders_win: Option<String>,
    pub draw: Option<String>,
    pub start: Vec<String>,
    pub use_deny: Option<String>,
    /// The weapon selection opening or its highlight moving.
    pub weapon_move: Option<String>,
    /// A weapon picked from the weapon selection.
    pub weapon_pick: Option<String>,
    /// A slot key on an empty slot.
    pub weapon_deny: Option<String>,
}

/// Indices into `core::MapBrushes` of brushes that stop players but not
/// line traces such as a fire's (player clips, grates, ladder-only
/// volumes; `MapData::trace_skip`).
#[derive(Resource, Clone, Debug, Default)]
pub struct MapTraceSkip(pub Vec<usize>);

/// Where `core::MapBrushes`' static props' brushes start: the map's own
/// brushes come first. A line trace starting inside a static prop stops
/// at once (Source's props are physics models), one starting inside a
/// brush passes out of it (a fire's drop, specs/source/fire.md Q5).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct MapPropBrushStart(pub usize);

#[derive(Clone, Debug, Default)]
pub struct MapData {
    pub name: String,
    /// SHA-256 of the map's file as read from the install (before any
    /// lump inflating), for the network handshake (`net`): every player
    /// must load the same file. None for maps built in code.
    pub file_hash: Option<[u8; 32]>,
    pub meshes: Vec<MapMesh>,
    pub textures: Vec<MapTexture>,
    /// Baked reflection cubemaps (`MapEnvmap::cubemap` indexes these).
    pub cubemaps: Vec<MapCubemap>,
    /// Where the map's cubemaps were baked: (position, index into
    /// `cubemaps`), for objects that take the nearest.
    pub cubemap_samples: Vec<(Vec3, usize)>,
    pub lightmap: Option<MapLightmap>,
    pub models: Vec<MapModel>,
    pub props: Vec<MapProp>,
    /// Collision triangles (surfaces with no solid volume, e.g. terrain).
    pub collision_positions: Vec<[f32; 3]>,
    pub collision_indices: Vec<[u32; 3]>,
    /// More collision triangles (into `collision_positions`) that only
    /// some things meet.
    pub collision_sets: Vec<MapCollisionSet>,
    /// Solid convex volumes, each as its corner points: what stops shots
    /// and physics bodies (player-only clips are in `collision_brushes`
    /// only).
    pub collision_hulls: Vec<Vec<[f32; 3]>>,
    /// Indices into `collision_hulls` of volumes light passes (glass,
    /// grates): their own collider, marked `SeeThrough`.
    pub see_through_hulls: Vec<usize>,
    /// Brush entities with nodes of their own whose volumes are all
    /// see-through (breakable glass): their nodes get `SeeThrough`.
    pub see_through_entities: Vec<usize>,
    /// The same volumes as planes, for exact swept-box movement collision.
    pub collision_brushes: Vec<MapBrush>,
    /// Indices into `collision_brushes` of the ones line traces pass
    /// (player clips, grates, ladder-only volumes): `MapTraceSkip`.
    pub trace_skip: Vec<usize>,
    /// The BSP tree over `collision_brushes` (Source maps), for the order
    /// traces meet coincident faces in.
    pub brush_tree: Option<MapBrushTree>,
    /// Feet positions.
    pub spawns: Vec<(Vec3, Option<Team>)>,
    /// Each spawn's facing as an `Intent` yaw (radians, 0 = -Z), same order
    /// as `spawns`; missing entries face -Z.
    pub spawn_yaws: Vec<f32>,
    /// Things the importer couldn't load (missing materials etc.).
    pub warnings: Vec<String>,
    /// How the source game presents the map, so it can be reproduced.
    pub look: MapLook,
    pub sky: Option<MapSky>,
    pub sky_camera: Option<MapSkyCamera>,
    /// The world's fog (not the 3D skybox's, which `sky_camera` has).
    pub fog: Option<MapFog>,
    pub sprites: Vec<MapSprite>,
    /// Sprite trails (env_spritetrail): `emitters`.
    pub trails: Vec<emitters::MapTrail>,
    /// env_muzzleflash entities (`emitters::MuzzleFlashEmitter`).
    pub muzzle_flashes: Vec<emitters::MapMuzzleFlash>,
    /// color_correction entities (`color_correction`).
    pub color_corrections: Vec<color_correction::MapColorCorrection>,
    /// Screen overlay materials maps name (env_screenoverlay): lower-case
    /// name and index into `textures`.
    pub screen_overlays: Vec<(String, usize)>,
    /// The monitor screen texture (Source `_rt_camera`, index into
    /// `textures`): drawn with what the active point_camera sees
    /// (`monitor`).
    pub camera_texture: Option<usize>,
    /// Beams (point_spotlight, env_laser, env_beam) and light glows
    /// (env_lightglow): `beams`.
    pub beams: Vec<beams::MapBeam>,
    pub glows: Vec<beams::MapGlow>,
    pub ropes: Vec<MapRope>,
    /// Which sky each part of the map can see (Source: BSP leaf flags). When
    /// set, the camera draws the sky only from places that see it, and
    /// clears to black inside solid.
    pub sky_vis: Option<MapSkyVis>,
    /// The playable world's sky faces as a triangle list (engine space,
    /// three positions per triangle). They are not drawn; with
    /// `chimera_skyocclude 1` they hide what lies behind them
    /// (`sky_occluder`).
    pub sky_surfaces: Vec<[f32; 3]>,
    /// Precomputed visibility (clusters and potentially visible sets),
    /// when the game's maps have it: parts the camera can't see are hidden.
    pub visibility: Option<Arc<vis::MapVisibility>>,
    /// Water and slime volumes.
    pub water: Vec<MapWaterVolume>,
    /// Water volumes of brush entities that can move (func_water_analog):
    /// (index into `water`, index into `entities`); they follow the
    /// entity's node (`water::move_water`).
    pub water_movers: Vec<(usize, usize)>,
    /// The map's entities (keyvalues, brush volumes) for the logic layer,
    /// in the map's own order.
    pub entities: Vec<MapEntity>,
    /// Meters per entity-space unit (`entities` module docs); 0 when the
    /// map has no entities.
    pub entity_scale: f32,
    /// How water surfaces look (`MapMesh::water` indexes these).
    pub water_materials: Vec<water::MapWaterMaterial>,
    /// Animated textures (Source's AnimatedTexture proxy): materials of
    /// meshes and models bind each one's first frame, and the renderer
    /// shows its frames in turn wherever that is bound
    /// (`render::texture_anim`).
    pub texture_anims: Vec<MapTextureAnim>,
    /// Dynamic prop shadows, when the game draws them.
    pub shadows: Option<MapShadows>,
    /// Gravity for physics bodies, m/s^2 (downward), when the game sets it.
    pub gravity: Option<f32>,
    /// The playable area (engine space, min and max), when the map has a 3D
    /// skybox outside it.
    pub playable: Option<(Vec3, Vec3)>,
    /// Sound entries, clips and surfaces the map uses.
    pub sounds: Arc<MapSounds>,
    /// The gameplay's surface names for the map's own (a map of another
    /// game in a mashup, `sources::SurfaceTable`): the map's lower-case
    /// surface property name to the gameplay's; empty when the map's
    /// names are the gameplay's. Spawned as `sources::SurfaceNames`.
    pub surface_names: std::collections::HashMap<String, String>,
    /// A top-down picture of the map (radar), when the game has one.
    pub overview: Option<hud::MapOverview>,
    /// Materials for particle effects (impacts).
    pub particles: particles::MapParticles,
    /// The map's game and whose impact looks it carries (`impact`).
    pub impacts: super::impact::MapImpacts,
    /// Data-driven particle systems and where the map runs them
    /// (info_particle_system): `psys`.
    pub particle_systems: Arc<psys::MapParticleSystems>,
    /// The light at any point, for moving models.
    pub light_field: Option<MapLightField>,
    /// Gib lists for breaking brushes, and how gibs move.
    pub gibs: Vec<breakables::MapGibSet>,
    pub gib_physics: Option<breakables::MapGibPhysics>,
    /// What the map's game brings to every map, not the map itself (the
    /// gameplay's kit): a mashup replaces it with the gameplay's own
    /// (`GameKit::stay`, `sources::MapSources::dress`).
    pub kit: GameKit,
    /// Features only some games' maps have (`MapExtras`), such as Source's
    /// effect lists (`MapData::source_fx`).
    pub extras: MapExtras,
    /// Seconds each loading stage took (`loading::LoadTimer`), in order.
    pub load_times: Vec<(&'static str, f32)>,
}

/// What a game brings to every map rather than the map's file defines: the
/// gameplay's kit. Character bodies and held models, view models, the HUD
/// look, the radio, bot names and the navigation mesh, the round sounds,
/// the muzzle flash, spent shells and impact decals. Its textures are in
/// `MapData::textures` (the indices below point there). A mashup's map of
/// another game has its own game's kit replaced (`GameKit::stay`) by the
/// gameplay's; every field added here is replaced with it, none to list.
#[derive(Clone, Debug, Default)]
pub struct GameKit {
    /// The announcer's round sounds (sound entries), when the game has them.
    pub round_sounds: RoundSounds,
    /// The team radio (commands, menus, chat format), when the game has one.
    pub radio: Option<radio::RadioCommands>,
    /// The game's bot personalities (bots' names), when it has them.
    pub bot_profiles: Option<bot_profiles::BotProfiles>,
    /// The bots' navigation mesh, if the game ships one for the map.
    pub nav: Option<Arc<nav::NavMesh>>,
    /// How characters look and where they can be hit, per team.
    pub characters: Vec<MapCharacterModel>,
    /// Models characters can hold (weapons' world models).
    pub held: Vec<MapHeldModel>,
    /// Runtime decals (bullet holes, slashes) by group.
    pub decals: decal::MapDecals,
    /// The game's own HUD look, when it has one.
    pub hud: Option<Arc<hud::GameHud>>,
    /// What characters see of what they hold (weapons' view models).
    pub view_models: Vec<MapViewModel>,
    /// What shots look like at the muzzle (flash sprites and light).
    pub muzzle_flash: Option<MapMuzzleFlash>,
    /// Spent shell types and how they fly.
    pub shells: Vec<shells::MapShell>,
    pub shell_physics: Option<shells::MapShellPhysics>,
}

impl GameKit {
    /// The part of this kit that stays on a map of another game: the held
    /// and view models whose keys start with one of `kept` (another game's
    /// weapons an app offers in the gameplay's slots), and with `players`
    /// what a player of the map's game wears and sees (its character
    /// bodies, hostages' aside, and its HUD look: an app whose players may
    /// be that game's, `slots::GameIdentity`), and its impact decals: they
    /// are part of the map game's impact looks, which stay with the map
    /// (`MapData::impacts`; `weapon::impact` routes that game's guns' and
    /// the map's impacts to them). Nothing else stays.
    pub fn stay(self, kept: &[&str], players: bool) -> GameKit {
        let keep = |key: &str| kept.iter().any(|k| key.starts_with(k));
        let Self {
            held,
            view_models,
            characters,
            hud,
            decals,
            ..
        } = self;
        GameKit {
            held: held.into_iter().filter(|h| keep(&h.key)).collect(),
            view_models: view_models.into_iter().filter(|v| keep(&v.key)).collect(),
            characters: characters.into_iter().filter(|c| players && c.name.is_none()).collect(),
            hud: hud.filter(|_| players),
            decals,
            ..GameKit::default()
        }
    }
}

impl MapData {
    /// Source's effect lists (empty on a map without them).
    pub fn source_fx(&self) -> &SourceEffects {
        static NONE: SourceEffects = SourceEffects::NONE;
        self.extras.get().unwrap_or(&NONE)
    }

    /// Source's effect lists to fill or change (added when absent).
    pub fn source_fx_mut(&mut self) -> &mut SourceEffects {
        self.extras.entry()
    }
}

/// Collision triangles that block only some things, for maps whose game
/// says so per surface (Combat Arms: collision blockers stop players
/// only, non-solid detail only traces). Each set is its own static
/// collider: on `core::PLAYER_CLIP_LAYER` (characters only),
/// `core::TRACE_ONLY_LAYER` (traces only) or the default layer (both).
#[derive(Clone, Debug, Default)]
pub struct MapCollisionSet {
    pub name: String,
    /// Triangles, indices into `MapData::collision_positions`.
    pub indices: Vec<[u32; 3]>,
    /// Characters' movement collides with them (`CHARACTER_FILTER`).
    pub characters: bool,
    /// Line traces and other gameplay queries (`SOLID_LAYERS`: shots,
    /// sight, explosion cover, thrown objects) meet them.
    pub traces: bool,
    /// Light passes them (glass): their collider is marked `SeeThrough`,
    /// as `see_through_hulls`' are.
    pub see_through: bool,
}

/// A muzzle flash: view-facing additive sprites strung out along the
/// muzzle's forward axis, shown for `life` seconds, and a brief point
/// light (`DynamicLight`) that lights the world, props and view models.
#[derive(Clone, Debug, Default)]
pub struct MapMuzzleFlash {
    /// Index into `MapData::textures`.
    pub texture: Option<usize>,
    /// Each sprite: distance ahead of the muzzle and full size (meters),
    /// before the random scale.
    pub sprites: Vec<(f32, f32)>,
    /// Random size factor range.
    pub scale: (f32, f32),
    /// sRGB colour multiplier of the sprites.
    pub color: [f32; 3],
    pub life: f32,
    pub light: Option<MapFlashLight>,
}

/// A muzzle flash's light: colour in lightmap units (1 = a fully lit
/// surface shows its texture), radius range (meters, picked at random) and
/// lifetime; its radius shrinks to 0 over the lifetime.
#[derive(Clone, Copy, Debug)]
pub struct MapFlashLight {
    pub color: Vec3,
    pub radius: (f32, f32),
    pub life: f32,
}

/// What the BSP leaf around a point can see of the sky.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LeafSky {
    /// Inside solid: the view clears to black.
    Solid,
    /// No sky visible: nothing is drawn behind the world.
    None,
    /// The 2D sky only.
    Sky2d,
    /// The 3D skybox (with the 2D sky behind it).
    Sky3d,
}

/// A BSP tree for point queries, in engine space (meters).
#[derive(Clone, Debug, Default)]
pub struct MapSkyVis {
    /// Plane normal and distance.
    pub planes: Vec<(Vec3, f32)>,
    /// Plane index and children (front, back); a negative child `c` is
    /// leaf `-c - 1`.
    pub nodes: Vec<(usize, [i32; 2])>,
    pub leaves: Vec<LeafSky>,
}

impl MapSkyVis {
    pub fn at(&self, p: Vec3) -> LeafSky {
        self.leaf(p)
            .and_then(|l| self.leaves.get(l).copied())
            .unwrap_or(LeafSky::None)
    }

    /// The index of the leaf containing `p`.
    pub fn leaf(&self, p: Vec3) -> Option<usize> {
        let mut node = 0i32;
        while node >= 0 {
            let &(plane, children) = self.nodes.get(node as usize)?;
            let (n, d) = self.planes[plane];
            node = if n.dot(p) - d >= 0.0 { children[0] } else { children[1] };
        }
        Some((-node - 1) as usize)
    }
}

/// The light arriving at any point (engine space), for models that move
/// (view models): the same probe static props bake, queried at run time.
#[derive(Clone)]
pub struct MapLightField(pub Arc<dyn Fn(Vec3) -> LightProbe + Send + Sync>);

impl std::fmt::Debug for MapLightField {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("MapLightField")
    }
}

/// A dust mote volume (`func_dustmotes`, `func_dustcloud`): slow specks
/// spawned inside its brushes, fading in and out over their life and with
/// distance (specs/cs_source/sprites_dust.md). Engine space, meters,
/// seconds.
#[derive(Clone, Debug)]
pub struct MapDust {
    /// The brushes' bounds.
    pub min: Vec3,
    pub max: Vec3,
    /// The brushes (planes, outward normals: n.p <= d inside); a mote
    /// starts inside one. Empty: anywhere in the bounds.
    pub brushes: Vec<Vec<(Vec3, f32)>>,
    /// Motes per second.
    pub rate: f32,
    /// Half-width range: screen-constant (half-width = size / 10000 x view
    /// depth) unless `world_size`, then meters.
    pub size: (f32, f32),
    /// A dust cloud: `size` is a half-width in the world.
    pub world_size: bool,
    /// Frozen: `rate` motes at once that never move, age or die.
    pub frozen: bool,
    /// Initial speed per axis, up to this (m/s).
    pub speed: f32,
    pub life: (f32, f32),
    /// Not drawn beyond this view depth; fade toward it (meters).
    pub fade_distance: f32,
    /// sRGB color and alpha (0-1).
    pub color: [f32; 4],
    /// Index into `MapData::textures`.
    pub texture: Option<usize>,
    /// The entity it comes from (index into `MapData::entities`), which
    /// the logic turns on and off (`EntityPart`).
    pub entity: Option<usize>,
    /// Spawning motes at map start.
    pub start_on: bool,
}

/// A map's detail sprites (Source detail props: grass and weeds over its
/// ground), drawn by `detail` in a few merged meshes.
#[derive(Clone, Debug)]
pub struct MapDetailProps {
    /// The sprite sheet, index into `MapData::textures`.
    pub texture: usize,
    pub quads: Vec<DetailQuad>,
    /// Fade distances (start, gone), meters, when the map sets its own
    /// (env_detail_controller); None: `render::detail::DEFAULT_FADE`.
    pub fade: Option<(f32, f32)>,
    /// The wind they sway in (env_wind): horizontal direction (engine) and
    /// speed (m/s); None: still.
    pub wind: Option<(Vec3, f32)>,
}

/// One detail sprite quad.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetailQuad {
    /// Engine space; the quad's foot.
    pub origin: Vec3,
    /// Corners from `origin` (bottom-left, top-left, top-right,
    /// bottom-right), meters; unused for billboards.
    pub corners: [Vec3; 4],
    /// Facing the view instead of fixed corners.
    pub billboard: Option<DetailBillboard>,
    /// Texture coordinates of the four corners.
    pub uv: [Vec2; 4],
    /// Its baked light, linear lightmap units.
    pub light: [f32; 3],
    /// How much its top sways (0-1).
    pub sway: f32,
}

/// A detail sprite facing the view: its rectangle (across, up) from the
/// origin, meters; `vertical` turns only about the vertical axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DetailBillboard {
    pub vertical: bool,
    pub lo: Vec2,
    pub hi: Vec2,
}

/// A camera-facing sprite (lamp glows): a quad parallel to the view plane,
/// drawn additively (specs/cs_source/sprites_dust.md).
#[derive(Clone, Debug)]
pub struct MapSprite {
    /// Engine space, meters.
    pub position: Vec3,
    /// Index into `MapData::textures`.
    pub texture: usize,
    /// Full width and height, meters.
    pub size: Vec2,
    /// Color x texture is added to the image: RGB scale and alpha weight.
    pub color: Vec4,
    /// A glow: drawn over everything, faded by how much of its occlusion
    /// proxy can be seen. Otherwise depth tested against the world.
    pub glow: bool,
    /// Added, blended by its alpha or opaque (its render mode).
    pub blend: SpriteBlend,
    /// Glow occlusion proxy: half-diagonal of a square pulled this far
    /// toward the viewer, meters.
    pub proxy: f32,
    /// The entity it comes from (index into `MapData::entities`), which
    /// the logic shows and hides (`EntityPart`).
    pub entity: Option<usize>,
    /// Shown at map start.
    pub start_on: bool,
}

/// A rope or cable: a line of points drawn as a strip that always faces the
/// camera (the rope shader widens it per view). Source's Cable look:
/// texture x normal map's blue squared x per-point light.
#[derive(Clone, Debug, Default)]
pub struct MapRope {
    /// Engine space, meters.
    pub points: Vec<Vec3>,
    /// Texture V per point; U runs across the strip.
    pub v: Vec<f32>,
    /// Light per point, lightmap units (0-1).
    pub light: Vec<Vec3>,
    /// Full strip width, meters.
    pub width: f32,
    pub texture: Option<usize>,
    pub normal_map: Option<usize>,
    /// Texture and normal map of a translucent strip drawn behind the rope
    /// that keeps thin, distant ropes visible (fake anti-aliasing).
    pub back: Option<(Option<usize>, Option<usize>)>,
    /// Its settled simulation, which the wind moves again near the camera
    /// (`rope_sway`); None: it hangs still.
    pub sway: Option<rope_sway::RopeSway>,
}

/// A 3D skybox: a miniature scene (meshes and props marked `skybox`) drawn
/// behind the world from `origin + eye / scale`, so it appears `scale` times
/// larger and moves with the player.
#[derive(Clone, Debug)]
pub struct MapSkyCamera {
    /// Engine space, meters.
    pub origin: Vec3,
    pub scale: f32,
    pub fog: Option<MapFog>,
}

/// Distance fog, distances in meters as seen in the world. Source's range
/// fog: f = clamp(min(max_density, (depth - start) / (end - start))), and
/// the color moves toward the fog color by f^2 (specs/cs_source/shaders.md);
/// with `linear`, by f (LithTech's table fog, specs/combat_arms/world_dat.md
/// "Sky").
#[derive(Clone, Debug, Default)]
pub struct MapFog {
    /// sRGB color.
    pub color: [f32; 3],
    pub start: f32,
    pub end: f32,
    pub max_density: f32,
    pub linear: bool,
}

/// A sky cubemap: per engine face (+X, -X, +Y, -Y, +Z, -Z), an index into
/// `MapData::textures` and an orientation (0-3 clockwise quarter turns, 4-7
/// the same after mirroring horizontally).
#[derive(Clone, Debug)]
pub struct MapSky {
    pub faces: [(usize, u8); 6],
    /// Each face material's `$basetexturetransform` (specs/cs_source/
    /// shaders.md 6: the face coordinates go through it; community skies
    /// use "scale 1 2" to show a half-height texture above the horizon).
    pub transforms: [MapUvTransform; 6],
    /// HDR versions of the faces (same order and orientations), shown
    /// instead when the map loads in HDR.
    pub hdr: Option<Arc<[MapHdrImage; 6]>>,
}

/// A linear RGB image with values above 1 (an HDR sky face), top row first.
#[derive(Clone, Debug, Default)]
pub struct MapHdrImage {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<[f32; 3]>,
}

/// Presentation choices that differ between games, measured against the
/// original (see `refcmp`).
#[derive(Clone, Debug)]
pub struct MapLook {
    /// Multiplier on all baked light (lightmaps and prop probes).
    pub light_scale: f32,
    /// Blend between mip levels (trilinear) or snap to the nearest one.
    pub trilinear: bool,
    /// Anisotropic filtering level; 1 = off.
    pub anisotropy: u16,
    /// Apply a filmic tonemapper (off: plain clamped output, like LDR games).
    pub tonemapping: bool,
    /// Store lightmaps the way Source's LDR path does (8-bit, gamma, 2x
    /// overbright), with its banding, clamp and gamma-space filtering.
    pub source_ldr_lightmaps: bool,
    /// Sample lightmaps with a bicubic B-spline filter (4 bilinear taps)
    /// instead of bilinear.
    pub bicubic_lightmaps: bool,
    /// HDR presentation (Source's mat_hdr_level 1 and 2): cameras render
    /// in HDR with bloom and, at level 2, auto exposure. None: LDR.
    pub hdr: Option<MapHdr>,
}

/// How a map is shown in HDR: the client's cameras follow it.
#[derive(Clone, Debug, PartialEq)]
pub struct MapHdr {
    /// Auto exposure: the scale on the scene's light stays within
    /// (min, max). None: no exposure (Source's mat_hdr_level 1, bloom
    /// only).
    pub exposure: Option<(f32, f32)>,
    /// Bloom strength, 1 = the game's default (Source's bloom scale).
    pub bloom_scale: f32,
    /// How fast exposure adapts, 1 = the default (Source's tone-map rate).
    pub rate: f32,
}

/// Auto exposure bounds without a controller: Source's
/// mat_autoexposure_min and mat_autoexposure_max cvars at their public
/// defaults.
pub const DEFAULT_AUTO_EXPOSURE: (f32, f32) = (0.5, 2.0);

impl Default for MapLook {
    fn default() -> Self {
        Self {
            light_scale: 1.0,
            trilinear: true,
            anisotropy: 8,
            tonemapping: true,
            source_ldr_lightmaps: false,
            bicubic_lightmaps: false,
            hdr: None,
        }
    }
}

impl MapData {
    pub fn triangle_count(&self) -> usize {
        self.meshes.iter().map(|m| m.indices.len() / 3).sum()
    }

    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = Vec3::splat(f32::MAX);
        let mut hi = Vec3::splat(f32::MIN);
        for p in self.meshes.iter().flat_map(|m| &m.positions) {
            lo = lo.min(Vec3::from(*p));
            hi = hi.max(Vec3::from(*p));
        }
        (lo, hi)
    }
}
/// How a sprite meets the image (its render mode).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SpriteBlend {
    /// Added (additive and glow modes 3, 5, 9).
    #[default]
    Add,
    /// Blended by its alpha (colour, texture and solid modes 1, 2, 4):
    /// depth tested, not written.
    Blend,
    /// An opaque square (normal mode 0): depth tested and written.
    Opaque,
}


#[cfg(test)]
mod render_tests {
    use super::*;

    /// A baked brush entity's rendercolor tints it in any render mode;
    /// the mode picks blend (alpha) or add (colour scaled by the opacity).
    #[test]
    fn render_color_tints_in_every_mode() {
        let mut m = MapMesh::default();
        assert_eq!(m.render_factor(), Vec4::ONE);
        m.render_color = Some([255, 0, 0]);
        let normal = m.render_factor();
        assert!((normal.x - 1.0).abs() < 1e-6 && normal.y == 0.0 && normal.w == 1.0, "{normal}");
        m.render = Some((MapAlpha::Blend, 0.5));
        let blended = m.render_factor();
        assert!(blended.x > 0.99 && blended.y == 0.0 && (blended.w - 0.5).abs() < 1e-6, "{blended}");
        m.render = Some((MapAlpha::Add, 0.5));
        let added = m.render_factor();
        assert!((added.x - 0.5).abs() < 1e-6 && added.w == 1.0, "{added}");
        // Grey in sRGB multiplies in gamma space: 128 is about 0.216 linear.
        m.render = None;
        m.render_color = Some([128; 3]);
        assert!((m.render_factor().x - 0.2159).abs() < 1e-3);
    }

    /// models_ltb.md R5: five frames at 10 per second (100 ms each): 0-99
    /// ms frame 0, 250 ms frame 2, 520 ms frame 0 again (looped); 16 ms
    /// steps advance the clock by 16 ms each.
    #[test]
    fn skin_frames_loop_on_their_clock() {
        let s = MapSkinFrames {
            frames: vec![10, 11, 12, 13, 14],
            frame_ms: 100,
        };
        assert_eq!(s.step(0, 0), (0, 0));
        assert_eq!(s.step(0, 99), (99, 0));
        assert_eq!(s.step(0, 250), (250, 2));
        assert_eq!(s.step(0, 520), (20, 0));
        let mut t = 0;
        let mut shown = Vec::new();
        for _ in 0..10 {
            let (next, f) = s.step(t, 16);
            assert_eq!(next, t + 16);
            t = next;
            shown.push(f);
        }
        assert_eq!(shown, [0, 0, 0, 0, 0, 0, 1, 1, 1, 1]);
    }
}
