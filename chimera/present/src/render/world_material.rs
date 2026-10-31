//! The material for lightmapped world surfaces: texture x baked light, with
//! radiosity normal mapping where the surface has directional lightmaps.
//! See world.wesl.
//!
//! World surfaces: texture x baked light with radiosity normal mapping, plus
//! the scene's point lights (muzzle flashes), plus baked-cubemap reflections
//! (Source LightmappedGeneric at LDR); `$detail` blend modes 0-4 and 7-9 with
//! `$detailtint`, `$selfillum`, `$basetexturetransform` (and
//! `$basetexturetransform2` on the second layer).
//!
//! Second layers of other games (off unless a mesh asks; Source surfaces
//! never do): a second texture at its own coordinates (`MapMesh::uvs2`)
//! combined LithTech's dual-texture way (`BlendMode::BaseAlpha`,
//! `BaseAlphaAdd`), a detail texture as a signed add at rotated
//! coordinates (`DetailMode::SignedAdd`, `MapDetail::rotation`) and a flat
//! environment map (`MapFlatEnvmap`), each in gamma space before lighting
//! (`map::material_fx` mirrors the maths).
//!
//! Known shortcuts (docs/tech-debt.md): the second coordinate set rides in
//! the vertex colour's red and green (Bevy's world vertex output has two
//! UV sets, the lightmap's taking the second), so a surface can't have
//! both it and a window's jagged mask; the depth prepass cuts alpha-tested
//! surfaces by the base texture's alpha, not a dual texture's blended one.
//! Why it's OK for now: neither combination occurs in the maps we load.
//!
//! Back faces are culled, in the depth prepass too, except on `$nocull`
//! materials (`prop_material::face_cull`), as Source draws them.
//!
//! Known shortcuts (docs/tech-debt.md): World reflections take the bumped
//! normal through a screen-space tangent frame (no per-vertex tangents); the
//! cubemaps carry their mips (nearest mip, the reference's bilinear filtering);
//! CS:S's highlight is still slightly broader on dust2's bumped tunnel floor
//! (refcmp `tunnel_floor_e`, abs diff 0.016, unchanged by the mips). Why it's
//! OK for now: Visually matches; per-vertex tangents if it matters.
//!
//! Known shortcuts (docs/tech-debt.md): WorldTwoTextureBlend
//! (specs/cs_source/shaders_two_texture_blend.md) follows the public SDK's
//! shader; that CS:S ships the same revision (fixed 2.0 lightmap scale, raw
//! detail) is confirmed only by matching brightness on de_aztec's refcmp views,
//! not by a capture of the shader; mode 0 with `$bumpmap` is undefined. Why
//! it's OK for now: refcmp luma matches to 0.01 on all 13 aztec views.

use bevy::{
    asset::embedded_asset, prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

#[derive(Clone, Copy, Debug, Default, bevy::render::render_resource::ShaderType)]
pub struct WorldParams {
    pub base_color: Vec4,
    pub light_scale: f32,
    pub bumped: f32,
    pub normal_g_sign: f32,
    pub alpha_cutoff: f32,
    /// `$basetexturetransform` rows (m0, m1, m2, scroll), see
    /// `MapUvTransform::shader_rows`; world_prepass.wesl reads them too.
    pub base_uv_u: Vec4,
    pub base_uv_v: Vec4,
    pub debug_view: f32,
    pub normal_x_sign: f32,
    /// Multiplier on sampled lightmap values (decodes Source's LDR encoding).
    pub lightmap_scale: f32,
    /// 1 when a second texture is blended in by the vertex alpha.
    pub blend: f32,
    /// 1 when `blend_mask` shapes that blend.
    pub blend_masked: f32,
    /// 1 when the second layer has its own normal map.
    pub blend_normal: f32,
    /// Detail texture: 0 none, 1 + Source's `$detailblendmode`, 20 and 21
    /// WorldTwoTextureBlend's modes (`DetailMode::shader_value`).
    pub detail: f32,
    pub detail_factor: f32,
    pub detail_scale: Vec2,
    /// Range fog: linear color (w = 1 when on), and start, end (meters),
    /// max density.
    pub fog_color: Vec4,
    pub fog_range: Vec4,
    /// 1: bicubic lightmap sampling.
    pub bicubic: f32,
    /// 1 when alpha blended, 2 when additive (output alpha 0, which adds
    /// under premultiplied blending), 3 for a modulate-2x decal (`modulate`). Otherwise the output alpha is 1: textures
    /// often keep other data in alpha (env map masks), and the camera's
    /// output is composited over the sky camera by alpha.
    pub translucent: f32,
    /// 1 when the surface reflects `envmap` (Source `$envmap`); 2 when it
    /// shows it in the view direction, unlit (WindowImposter).
    pub envmap: f32,
    /// What scales the reflection: 0 nothing, 1 normal-map alpha, 2 one
    /// minus base alpha, 3 `envmap_mask` colour.
    pub envmap_mask: f32,
    /// 1 when `normal` is bound (reflections follow the normal map).
    pub has_normal: f32,
    pub envmap_contrast: f32,
    pub envmap_saturation: f32,
    /// Fresnel R0; 1 = none.
    pub envmap_fresnel: f32,
    pub envmap_tint: Vec4,
    /// Under water (map::water): the water's range fog, linear colour (w = 1
    /// when on), for points below `water_fog_range.w` (the surface plus
    /// the fudge); start, end (meters), max density in xyz.
    pub water_fog_color: Vec4,
    pub water_fog_range: Vec4,
    /// `$basetexturetransform2` rows (the second layer's coordinates).
    pub base2_uv_u: Vec4,
    pub base2_uv_v: Vec4,
    /// `$detailtint` (linear multiplier on the detail texel).
    pub detail_tint: Vec4,
    /// 1 with `$selfillum`: the base alpha lerps toward `selfillum_tint` x
    /// albedo, unlit.
    pub selfillum: f32,
    pub selfillum_tint: Vec4,
    /// 1: drawn at the texture's own brightness times `modulation` (an
    /// UnlitGeneric material on a brush, or the `$emissiveblend` stand-in),
    /// not lit by the lightmap; between 0 and 1, the lighting that far
    /// toward full brightness (`MapMesh::unlit_factor`).
    pub unlit: f32,
    /// `$color`: the lighting's multiplier (lightmap, bump pages and
    /// dynamic lights; not the reflection or self-illumination,
    /// specs/cs_source/shaders.md 2), or the unlit surface's brightness.
    pub modulation: Vec4,
    /// 1: a window's broken look (`MapMesh::pane_mask`): an edge piece's
    /// alpha times its tile of `blend_mask` (the vertex colour's blue),
    /// sampled at the colour's red and green (meshes without vertex
    /// colours, the cracked face, are left as they are).
    pub pane_mask: f32,
    /// How the second texture combines (`map::BlendMode::shader_value`):
    /// 0 Source's mix, 1 and 2 LithTech's dual texture (gamma space).
    pub blend_mode: f32,
    /// 1: the second texture is sampled at the mesh's second coordinate
    /// set (`MapMesh::uvs2`, riding in the vertex colour's red and green).
    pub blend_uv2: f32,
    /// The detail coordinates' rotation, radians (`MapDetail::rotation`).
    pub detail_rotation: f32,
    /// A flat environment map (`MapFlatEnvmap`, bound as `flat_envmap`):
    /// 0 none, 1 signed add, 2 multiply.
    pub flat_envmap: f32,
    /// 1: weighted by the base texture's alpha.
    pub flat_envmap_alpha: f32,
    /// `EnvScale`.
    pub flat_envmap_scale: f32,
}

impl WorldParams {
    /// Identity texture transforms and a white detail tint, for params
    /// built field by field.
    pub fn identity_uv() -> Self {
        let [u, v] = crate::map::MapUvTransform::IDENTITY.shader_rows();
        Self {
            base_uv_u: u,
            base_uv_v: v,
            base2_uv_u: u,
            base2_uv_v: v,
            detail_tint: Vec4::ONE,
            selfillum_tint: Vec4::ONE,
            modulation: Vec4::ONE,
            flat_envmap_scale: 1.0,
            ..Default::default()
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
#[bind_group_data(WorldMaterialKey)]
pub struct WorldMaterial {
    #[uniform(0)]
    pub params: WorldParams,
    #[texture(1)]
    #[sampler(2)]
    pub base: Option<Handle<Image>>,
    #[texture(3)]
    #[sampler(4)]
    pub normal: Option<Handle<Image>>,
    #[texture(5)]
    #[sampler(6)]
    pub lightmap: Option<Handle<Image>>,
    #[texture(7)]
    pub lightmap_b0: Option<Handle<Image>>,
    #[texture(8)]
    pub lightmap_b1: Option<Handle<Image>>,
    #[texture(9)]
    pub lightmap_b2: Option<Handle<Image>>,
    /// Second layer (WorldVertexTransition), sampled like `base`.
    #[texture(10)]
    pub base2: Option<Handle<Image>>,
    #[texture(11)]
    pub normal2: Option<Handle<Image>>,
    /// The blend's shaping mask, or a window edge piece's jagged mask
    /// (`WorldParams::pane_mask`).
    #[texture(12)]
    pub blend_mask: Option<Handle<Image>>,
    #[texture(13)]
    pub detail: Option<Handle<Image>>,
    /// Cube texture sampled with Source-frame (Z-up) directions.
    #[texture(14, dimension = "cube")]
    #[sampler(15)]
    pub envmap: Option<Handle<Image>>,
    #[texture(16)]
    pub envmap_mask: Option<Handle<Image>>,
    /// A flat environment map (`WorldParams::flat_envmap`), sampled like
    /// `base`.
    #[texture(17)]
    pub flat_envmap: Option<Handle<Image>>,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
    /// A map overlay or decal lying on a surface: drawn with its own blend
    /// and no depth writes before everything else see-through (particles,
    /// smoke, glows, muzzle flashes draw over it rather than being sorted
    /// against it), overlapping ones in the map's order
    /// (`blend_order::DecalOrder`; blended phase, as only that phase's
    /// order can be set: in the alpha-mask phase overlapping decals drew
    /// in pipeline creation order, which depends on what was seen first).
    pub decal: bool,
    /// A decal that multiplies the surface: blended as `src x dst + dst x
    /// src` (Source's DecalModulate), the shader returning half the factor.
    pub modulate: bool,
    /// A sprite section (`map::MapSkinFrames` on a world mesh): each
    /// frame's image (`base` shows one) and how long each shows, ms; the
    /// section runs its own clock while it is drawn
    /// (`skin_frames::advance_sections`). Empty: a still texture.
    pub skin_frames: Vec<Handle<Image>>,
    pub skin_frame_ms: u32,
}

impl Material for WorldMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_present/render/world.wesl".into()
    }

    /// Alpha-tested surfaces count as masked, so the depth prepass (main
    /// views with water) runs `world_prepass.wesl` and drops the same texels.
    fn alpha_mode(&self) -> AlphaMode {
        if self.decal {
            return AlphaMode::Blend;
        }
        if self.alpha_mode == AlphaMode::Opaque && self.params.alpha_cutoff > 0.0 {
            return AlphaMode::Mask(self.params.alpha_cutoff);
        }
        self.alpha_mode
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "embedded://chimera_present/render/world_prepass.wesl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        descriptor.primitive.cull_mode = super::prop_material::face_cull(key.bind_group_data.double_sided, false);
        // Decals: their own blend (see `decal`).
        let blend = match key.bind_group_data.decal_blend {
            1 => Some(bevy::render::render_resource::BlendState::ALPHA_BLENDING),
            2 => Some(bevy::render::render_resource::BlendState {
                color: bevy::render::render_resource::BlendComponent {
                    src_factor: bevy::render::render_resource::BlendFactor::SrcAlpha,
                    dst_factor: bevy::render::render_resource::BlendFactor::One,
                    operation: bevy::render::render_resource::BlendOperation::Add,
                },
                alpha: bevy::render::render_resource::BlendComponent::OVER,
            }),
            // Modulate 2x; the destination's alpha stays (the sky
            // composite reads it).
            3 => Some(bevy::render::render_resource::BlendState {
                color: bevy::render::render_resource::BlendComponent {
                    src_factor: bevy::render::render_resource::BlendFactor::Dst,
                    dst_factor: bevy::render::render_resource::BlendFactor::Src,
                    operation: bevy::render::render_resource::BlendOperation::Add,
                },
                alpha: bevy::render::render_resource::BlendComponent {
                    src_factor: bevy::render::render_resource::BlendFactor::Zero,
                    dst_factor: bevy::render::render_resource::BlendFactor::One,
                    operation: bevy::render::render_resource::BlendOperation::Add,
                },
            }),
            _ => None,
        };
        if blend.is_some() {
            if let Some(fragment) = descriptor.fragment.as_mut() {
                for target in fragment.targets.iter_mut().flatten() {
                    target.blend = blend;
                }
            }
            if let Some(depth) = descriptor.depth_stencil.as_mut() {
                depth.depth_write_enabled = Some(false);
            }
        }
        Ok(())
    }
}

/// Pipeline key data: what changes the pipeline rather than uniforms.
#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct WorldMaterialKey {
    double_sided: bool,
    /// Decals only: 1 alpha blend, 2 additive, 3 modulate 2x (0: not a
    /// decal).
    decal_blend: u8,
}

impl From<&WorldMaterial> for WorldMaterialKey {
    fn from(m: &WorldMaterial) -> Self {
        Self {
            double_sided: m.double_sided,
            decal_blend: match (m.decal, m.alpha_mode) {
                (false, _) => 0,
                (true, _) if m.modulate => 3,
                (true, AlphaMode::Add) => 2,
                (true, _) => 1,
            },
        }
    }
}

/// Registers the world material and its shader. Needs rendering; maps
/// still load headless without it.
pub struct WorldMaterialPlugin;

impl Plugin for WorldMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "world.wesl");
        embedded_asset!(app, "world_prepass.wesl");
        app.add_plugins((super::fog::FogShaderPlugin, MaterialPlugin::<WorldMaterial>::default()));
    }
}
