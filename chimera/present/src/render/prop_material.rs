//! The material for props lit by a light probe: texture x the per-vertex
//! light baked into the mesh's vertex colors (static props), or x the
//! probe evaluated per pixel from uniforms (moving models: view models),
//! plus the scene's point lights (muzzle flashes), with Source's range fog
//! (the same f^2 curve as the world, see world.wesl). See prop.wesl.
//!
//! Props lit by a light probe (vertex colours, or per pixel from uniforms for
//! moving models such as view models), plus the scene's point lights
//! (`DynamicLight`, read from Bevy's light clusters), Source range fog,
//! `$envmap` reflections (the cubemap a material names, or for `env_cubemap`
//! the nearest baked one) masked as the material says (`$envmapmask`,
//! `$basealphaenvmapmask`, `$normalmapalphaenvmapmask`: the normal map's
//! alpha, bound as `envmap_mask`), and the model
//! shaders' `$detail` (every blend mode, 5 and 6 after lighting), `$selfillum`
//! (base alpha or `$selfillummask`) and `$basetexturetransform`; a second
//! texture on generated coordinates (`map::MapLayer`: LithTech's
//! environment and rim maps, `layer`), and a skin's frames
//! (`map::MapSkinFrames`, swapped by `skin_frames`).
//!
//! Faces: back ones culled except on `$nocull` materials, front ones on
//! mirrored (left-handed) view models, in the depth prepass as in the main
//! pass (`face_cull`).

use bevy::{
    asset::embedded_asset, prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef,
};

#[derive(Clone, Copy, Debug, Default, bevy::render::render_resource::ShaderType)]
pub struct PropParams {
    pub base_color: Vec4,
    /// Cut out below this alpha (0: off).
    pub alpha_cutoff: f32,
    /// `$basetexturetransform` rows (m0, m1, m2, scroll), see
    /// `MapUvTransform::shader_rows`; prop_prepass.wesl reads them too.
    pub base_uv_u: Vec4,
    pub base_uv_v: Vec4,
    /// Linear fog color, w = 1 when fog is on.
    pub fog_color: Vec4,
    /// Fog start, end (meters), max density.
    pub fog_range: Vec4,
    /// 1 when alpha blended, 2 additive; otherwise the output alpha is 1 (see
    /// `WorldParams::translucent`).
    pub translucent: f32,
    /// 1 when the prop reflects `envmap` (Source VertexLitGeneric
    /// `$envmap`: no fresnel).
    pub envmap: f32,
    /// 0 no mask, 1 `envmap_mask` alpha (the normal map's: `$normalmapalphaenvmapmask`),
    /// 2 one minus base alpha, 3 `envmap_mask` colour.
    pub envmap_mask: f32,
    pub envmap_contrast: f32,
    pub envmap_saturation: f32,
    /// Linear tint (the model shader converts the material's gamma value).
    pub envmap_tint: Vec4,
    /// 1: add the scene's point lights (`DynamicLight`s), see
    /// `dynamic_light` in prop.wesl.
    pub dynamic: f32,
    /// 1: lit per pixel by the probe below instead of vertex colours.
    pub probe: f32,
    /// `LightProbe::cube` (+X -X +Y -Y +Z -Z), lightmap units.
    pub probe_cube: [Vec4; 6],
    /// Up to four directional lights: direction toward the light (xyz)
    /// and colour; unused ones are zero.
    pub probe_light_dir: [Vec4; 4],
    pub probe_light_color: [Vec4; 4],
    /// Under water (map::water): the water's range fog, linear colour (w = 1
    /// when on), for points below `water_fog_range.w` (the surface plus
    /// the fudge); start, end (meters), max density in xyz.
    pub water_fog_color: Vec4,
    pub water_fog_range: Vec4,
    /// `$detail`: 0 none, else 1 + `$detailblendmode` (0-9).
    pub detail: f32,
    pub detail_factor: f32,
    pub detail_scale: Vec2,
    /// `$detailtint`, linear.
    pub detail_tint: Vec4,
    /// 1 with `$selfillum` (mask: base alpha), 2 with a `$selfillummask`.
    pub selfillum: f32,
    pub selfillum_tint: Vec4,
    /// A second texture on generated coordinates (`map::MapLayer`, bound
    /// as `layer`): 0 none, else 1 + its combine (`layer_combine`).
    pub layer: f32,
    /// Its coordinates: 0 camera-space reflection, 1 camera-space normal,
    /// 2 sphere map, 3 world-space reflection.
    pub layer_coords: f32,
    /// 0 wrap, 1 clamp, 2 mirror.
    pub layer_address: f32,
    pub layer_factor: f32,
    /// The source game's lit colour as a fraction of the drawn light.
    pub layer_light: f32,
    /// 1: the layer's lit colour and specular come from a lighting
    /// material (`map::LayerLighting`) over the probe's ambient and
    /// lights, as below (needs `probe`); 0: `layer_light`'s fraction.
    pub layer_lit: f32,
    /// The material's emissive, ambient and diffuse colours, and its
    /// specular colour (w = its power).
    pub layer_emissive: Vec4,
    pub layer_ambient: Vec4,
    pub layer_diffuse: Vec4,
    pub layer_specular: Vec4,
}

impl PropParams {
    /// Light the material per pixel with `probe` (times `scale`).
    pub fn set_probe(&mut self, probe: &crate::map::LightProbe, scale: f32) {
        self.probe = 1.0;
        for (c, v) in self.probe_cube.iter_mut().zip(probe.cube) {
            *c = (v * scale).extend(0.0);
        }
        let mut lights: Vec<&(Vec3, Vec3)> = probe.lights.iter().collect();
        lights.sort_by(|a, b| b.1.max_element().total_cmp(&a.1.max_element()));
        for i in 0..4 {
            let (dir, color) = lights.get(i).map_or((Vec3::ZERO, Vec3::ZERO), |l| (l.0, l.1 * scale));
            self.probe_light_dir[i] = dir.extend(0.0);
            self.probe_light_color[i] = color.extend(0.0);
        }
    }
}

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, Default)]
#[bind_group_data(PropMaterialKey)]
pub struct PropMaterial {
    #[uniform(0)]
    pub params: PropParams,
    #[texture(1)]
    #[sampler(2)]
    pub base: Option<Handle<Image>>,
    /// Cube texture sampled with Source-frame (Z-up) directions.
    #[texture(3, dimension = "cube")]
    #[sampler(4)]
    pub envmap: Option<Handle<Image>>,
    #[texture(5)]
    pub envmap_mask: Option<Handle<Image>>,
    /// `$detail`, sampled like `base`.
    #[texture(6)]
    pub detail: Option<Handle<Image>>,
    /// `$selfillummask`.
    #[texture(7)]
    pub selfillum_mask: Option<Handle<Image>>,
    pub alpha_mode: AlphaMode,
    pub double_sided: bool,
    /// Cull front faces instead of back ones (drawn mirrored).
    pub cull_front: bool,
    /// `map::MapLayer`'s texture, sampled like `base` at coordinates the
    /// shader makes (`PropParams::layer`).
    #[texture(8)]
    pub layer: Option<Handle<Image>>,
    /// A skin that is a looping sequence (`map::MapSkinFrames`): each
    /// frame's image (`base` shows one) and how long each shows, ms; each
    /// drawn copy runs its own clock (`skin_frames::SkinClock`).
    pub skin_frames: Vec<Handle<Image>>,
    pub skin_frame_ms: u32,
}

impl Material for PropMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_present/render/prop.wesl".into()
    }

    /// Alpha-tested surfaces count as masked, so the depth prepass (main
    /// views with water) runs `prop_prepass.wesl` and drops the same texels.
    fn alpha_mode(&self) -> AlphaMode {
        if self.alpha_mode == AlphaMode::Opaque && self.params.alpha_cutoff > 0.0 {
            return AlphaMode::Mask(self.params.alpha_cutoff);
        }
        self.alpha_mode
    }

    fn prepass_fragment_shader() -> ShaderRef {
        "embedded://chimera_present/render/prop_prepass.wesl".into()
    }

    fn specialize(
        _pipeline: &bevy::pbr::MaterialPipeline,
        descriptor: &mut bevy::render::render_resource::RenderPipelineDescriptor,
        _layout: &bevy::mesh::MeshVertexBufferLayoutRef,
        key: bevy::pbr::MaterialPipelineKey<Self>,
    ) -> Result<(), bevy::render::render_resource::SpecializedMeshPipelineError> {
        // Opaque props get a depth-only prepass (no fragment stage); inside
        // a fade band it must drop the dithered-out pixels too.
        if descriptor.fragment.is_none()
            && key
                .mesh_key
                .contains(bevy::pbr::MeshPipelineKey::VISIBILITY_RANGE_DITHER)
        {
            descriptor.fragment = Some(bevy::render::render_resource::FragmentState {
                shader: DITHER_PREPASS_SHADER,
                shader_defs: descriptor.vertex.shader_defs.clone(),
                entry_point: Some("fragment".into()),
                targets: Vec::new(),
                constants: Vec::new(),
            });
        }
        descriptor.primitive.cull_mode = face_cull(key.bind_group_data.double_sided, key.bind_group_data.cull_front);
        Ok(())
    }
}

/// The faces a world or prop pipeline culls: back faces (Source culls them
/// on everything but `$nocull` materials), none when `double_sided`, front
/// ones on a mirrored model. Set on every pipeline, the depth prepass's too:
/// Bevy's main pass culls back faces by default but its prepass culls
/// nothing, so a back face left in the prepass's depth hid what lies behind
/// it while the main pass culled it (the sky or black showed through).
pub(crate) fn face_cull(double_sided: bool, cull_front: bool) -> Option<bevy::render::render_resource::Face> {
    use bevy::render::render_resource::Face;
    match (double_sided, cull_front) {
        (true, _) => None,
        (false, true) => Some(Face::Front),
        (false, false) => Some(Face::Back),
    }
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct PropMaterialKey {
    double_sided: bool,
    cull_front: bool,
}

impl From<&PropMaterial> for PropMaterialKey {
    fn from(m: &PropMaterial) -> Self {
        Self {
            double_sided: m.double_sided,
            cull_front: m.cull_front,
        }
    }
}

/// `prop_dither_prepass.wesl`: the depth prepass fragment stage for opaque
/// props in a fade band (see `specialize`).
const DITHER_PREPASS_SHADER: Handle<Shader> = bevy::asset::uuid_handle!("6f3d2b8e-5c1a-4e7f-9a42-1d8b7c3e5f60");

pub struct PropMaterialPlugin;

impl Plugin for PropMaterialPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "prop.wesl");
        embedded_asset!(app, "prop_prepass.wesl");
        bevy::shader::load_shader_library!(app, "dither.wesl");
        bevy::asset::load_internal_asset!(app, DITHER_PREPASS_SHADER, "prop_dither_prepass.wesl", Shader::from_wesl);
        app.add_plugins((super::fog::FogShaderPlugin, MaterialPlugin::<PropMaterial>::default()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::render::render_resource::{Face, FrontFace, RenderPipelineDescriptor};

    /// Back faces are culled except on `$nocull` (double-sided) materials,
    /// front ones on mirrored models.
    #[test]
    fn culls_back_faces_unless_two_sided() {
        assert_eq!(face_cull(false, false), Some(Face::Back));
        assert_eq!(face_cull(true, false), None);
        assert_eq!(face_cull(true, true), None);
        assert_eq!(face_cull(false, true), Some(Face::Front));
    }

    /// Why `specialize` must set the cull mode itself: a pipeline Bevy
    /// builds without one (its depth prepass's) culls nothing, with
    /// counter-clockwise front faces (the map's and models' winding).
    #[test]
    fn a_pipeline_left_alone_culls_nothing() {
        let d = RenderPipelineDescriptor::default();
        assert_eq!(d.primitive.cull_mode, None);
        assert_eq!(d.primitive.front_face, FrontFace::Ccw);
    }
}
