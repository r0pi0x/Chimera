//! The material for props lit by a light probe: texture x the per-vertex
//! light baked into the mesh's vertex colors (static props), or x the
//! probe evaluated per pixel from uniforms (moving models: view models),
//! plus the scene's point lights (muzzle flashes), with Source's range fog
//! (the same f^2 curve as the world, see world.wesl). See prop.wesl.
//!
//! Props lit by a light probe (vertex colours, or per pixel from uniforms for
//! moving models such as view models), plus the scene's point lights
//! (`DynamicLight`, read from Bevy's light clusters) with the mesh's own
//! game's falloff (`map::LightFalloff`: Source's 1 - d^2/r^2, or
//! LithTech's 1 / (1 + 19 d^2/r^2) on Combat Arms' models), Source range fog,
//! `$envmap` reflections (the cubemap a material names, or for `env_cubemap`
//! the nearest baked one) masked as the material says (`$envmapmask`,
//! `$basealphaenvmapmask`, `$normalmapalphaenvmapmask`: the normal map's
//! alpha, bound as `envmap_mask`), and the model
//! shaders' `$detail` (every blend mode, 5 and 6 after lighting), `$selfillum`
//! (base alpha or `$selfillummask`) and `$basetexturetransform`; a second
//! texture on generated coordinates (`map::MapLayer`: LithTech's
//! environment and rim maps, `layer`), and a skin's frames
//! (`map::MapSkinFrames`, swapped by `skin_frames`). VertexLitGeneric
//! `$phong` materials (`map::MapPhong`) take Source's Phong shading per
//! pixel (`PropParams::phong`, specs/cs_source/shaders_phong.md): forced
//! half-Lambert from the probe's ambient cube and lights, the
//! reflection-vector specular with mask, fresnel ramp, boost and tint, the
//! rim light, and the envmap's own mask rules; `map::phong` mirrors the
//! maths on the CPU. `CHIMERA_PHONG=0` turns it off (`phong_enabled`).
//!
//! Faces: back ones culled except on `$nocull` materials, front ones on
//! mirrored (left-handed) view models, in the depth prepass as in the main
//! pass (`face_cull`).
//!
//! The kit's models' materials (bodies, skins, held, loose, gib and view
//! models) wait outside the store until an entity draws them
//! (`DormantProps`, `wake_drawn`): each one in the store is a GPU buffer
//! prepared whether drawn or not.

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
    /// Add the scene's point lights (`DynamicLight`s), see
    /// `dynamic_light` in prop.wesl: 0 off, else the falloff's
    /// `map::LightFalloff::shader_value` (1 Source's, 2 LithTech's).
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
    /// A face part (`map::face`): 0 none, 1 an eyeball (its iris bound as
    /// `layer`), 2 teeth.
    pub face: f32,
    /// Eyeball: its centre (world, xyz); teeth: unused.
    pub face_center: Vec4,
    /// Eyeball: its up (world, unit); teeth: the mouth's forward (world,
    /// unit) and, in w, how lit the teeth are (0 to 1).
    pub face_up: Vec4,
    /// Eyeball: the iris texture's planes over world points (u, v).
    pub face_u: Vec4,
    pub face_v: Vec4,
    /// Source's Phong model shading (specs/cs_source/shaders_phong.md,
    /// `map::phong`): 0 off (the plain model shader), 1 on, 2 its cheap
    /// variant (vertex normal, base-alpha mask, constant exponent, no rim
    /// or tint).
    pub phong: f32,
    /// `$phongfresnelranges` (min, mid, max) and `$phongboost` (w).
    pub phong_fresnel: Vec4,
    /// The exponent k = x + y × the exponent texture's red; the rim's
    /// exponent (z) and boost (w).
    pub phong_exponent: Vec4,
    /// S_t: a colour (rgb, w = 0), or w = 1 for lerp(1, base, exp.g).
    pub phong_tint: Vec4,
    /// Rim light on (x), its mask is the exponent texture's alpha (y),
    /// `$basemapalphaphongmask` (z), a normal map is bound (w).
    pub phong_flags: Vec4,
    /// The envmap mask (`map::PhongEnvMask`): x 0 base alpha, 1 the
    /// specular mask, 2 the constant w; y 1 inverts it; z `$envmapfresnel`.
    pub phong_env: Vec4,
    /// A lightwarp (x) and a phong warp (y) texture are bound.
    pub phong_warp: Vec4,
    /// The probe light comes from this slot of the probe table
    /// (`probe_lit::ProbeTable`, bound as `probes`) instead of `probe_cube`
    /// and the lights above: a moving model's, rewritten there as it moves
    /// so its materials stay unchanged. 0: the material's own.
    pub probe_slot: u32,
    /// A face part's face (`face_center` to `face_v`) comes from this
    /// slot of the same table (its first four rows; `bodies::FaceSlot`),
    /// rewritten every frame as the head moves. 0: the material's own.
    pub face_slot: u32,
}

impl PropParams {
    /// Light the material per pixel with `probe` (times `scale`).
    pub fn set_probe(&mut self, probe: &crate::map::LightProbe, scale: f32) {
        self.probe = 1.0;
        let rows = super::probe_lit::probe_rows(probe, scale);
        let own = self.probe_cube.iter_mut().chain(&mut self.probe_light_dir).chain(&mut self.probe_light_color);
        for (v, r) in own.zip(rows) {
            *v = Vec4::from_array(r);
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
    /// The Phong path's textures (`PropParams::phong`): the normal map
    /// (rgb normal, alpha the specular mask), `$phongexponenttexture`
    /// (white when unset), `$lightwarptexture` and `$phongwarptexture`,
    /// all raw and sampled like `base`.
    #[texture(9)]
    pub normal_map: Option<Handle<Image>>,
    #[texture(10)]
    pub phong_exponent: Option<Handle<Image>>,
    #[texture(11)]
    pub lightwarp: Option<Handle<Image>>,
    #[texture(12)]
    pub phong_warp: Option<Handle<Image>>,
    /// The probe table (`probe_lit::ProbeTable`): the same buffer in every
    /// material, read at `PropParams::probe_slot`.
    #[storage(13, read_only)]
    pub probes: super::probe_lit::ProbeTableBinding,
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

/// Prop materials nothing draws yet: the models a map's kit and its late
/// models carry (every body's parts and skin families, every held, loose and
/// view model's), which entities draw copies of (`probe_lit::instance`) or,
/// a view model's, draw when shown. A material in `Assets<PropMaterial>` is
/// prepared for the GPU (a uniform buffer and a bind group) whether drawn or
/// not, and wgpu's per-pass trackers are sized by the most buffers ever alive
/// at once (docs/performance.md, "A session's render thread"): kept here, a
/// handle reserved in the store and its material beside it, they cost the
/// render world nothing. One moves into the store when an entity draws it
/// (`wake_drawn`), and goes when its last handle is dropped (`prune`, as
/// the store drops it).
///
/// Readers of a material that may be one of these use `prop` (the store, then
/// here); what changes every material (fog, animated textures) changes these
/// too (`values_mut`). Without this resource (tests that build their own
/// stores) materials go into the store as before.
#[derive(Resource, Default)]
pub struct DormantProps {
    held: bevy::platform::collections::HashMap<
        AssetId<PropMaterial>,
        (std::sync::Weak<bevy::asset::StrongHandle>, PropMaterial),
    >,
    /// `held`'s size after the last prune: the next comes at twice it.
    pruned: usize,
}

impl DormantProps {
    /// Hold `m` under a handle reserved in `store`.
    pub fn add(&mut self, store: &Assets<PropMaterial>, m: PropMaterial) -> Handle<PropMaterial> {
        let handle = store.reserve_handle();
        self.insert(&handle, m);
        handle
    }

    /// Hold `m` under `handle`, reserved in the store and empty there (a late
    /// model's: `late_build::Stage`).
    pub fn insert(&mut self, handle: &Handle<PropMaterial>, m: PropMaterial) {
        let Handle::Strong(strong) = handle else { return };
        if self.held.len() >= (2 * self.pruned).max(1024) {
            self.prune();
        }
        self.held.insert(handle.id(), (std::sync::Arc::downgrade(strong), m));
    }

    pub fn get(&self, id: impl Into<AssetId<PropMaterial>>) -> Option<&PropMaterial> {
        self.held.get(&id.into()).map(|(_, m)| m)
    }

    /// Every material held, to change (fog, animated textures).
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut PropMaterial> {
        self.held.values_mut().map(|(_, m)| m)
    }

    /// How many are held (dropped ones count until the next prune).
    pub fn len(&self) -> usize {
        self.held.len()
    }

    /// How many are held that something still has a handle to.
    pub fn live(&self) -> usize {
        self.held.values().filter(|(h, _)| h.strong_count() > 0).count()
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Drop the materials whose last handle has gone.
    pub fn prune(&mut self) {
        self.held.retain(|_, (h, _)| h.strong_count() > 0);
        self.pruned = self.held.len();
    }
}

/// Material `id` from the store, or held dormant.
pub fn prop<'a>(
    store: &'a Assets<PropMaterial>,
    dormant: Option<&'a DormantProps>,
    id: impl Into<AssetId<PropMaterial>>,
) -> Option<&'a PropMaterial> {
    let id = id.into();
    store.get(id).or_else(|| dormant.and_then(|d| d.get(id)))
}

/// An entity draws a dormant material (`DormantProps`): into the store, to
/// be prepared.
pub fn wake_drawn(
    on: On<Insert<MeshMaterial3d<PropMaterial>>>,
    meshes: Query<&MeshMaterial3d<PropMaterial>>,
    dormant: Option<ResMut<DormantProps>>,
    store: Option<ResMut<Assets<PropMaterial>>>,
) {
    let (Some(mut dormant), Some(mut store)) = (dormant, store) else { return };
    let Ok(handle) = meshes.get(on.entity) else { return };
    if let Some((_, m)) = dormant.held.remove(&handle.0.id()) {
        let _ = store.insert(handle.0.id(), m);
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
        super::probe_lit::ProbeTable::add_to(app);
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

    /// A kit's material waits outside the store (no GPU buffer) until an
    /// entity draws it; a copy made from it goes into the store; it goes
    /// with its last handle.
    #[test]
    fn dormant_materials_enter_the_store_only_when_drawn() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, AssetPlugin::default()))
            .init_asset::<PropMaterial>()
            .init_resource::<DormantProps>()
            .add_observer(wake_drawn);
        let mut add = |app: &mut App, alpha: f32| {
            let w = app.world_mut();
            let m = PropMaterial { params: PropParams { alpha_cutoff: alpha, ..default() }, ..default() };
            w.resource_scope(|w, mut d: Mut<DormantProps>| d.add(w.resource::<Assets<PropMaterial>>(), m))
        };
        let (drawn, copied, dropped) = (add(&mut app, 0.1), add(&mut app, 0.2), add(&mut app, 0.3));
        app.update();
        let store = |app: &App| app.world().resource::<Assets<PropMaterial>>().len();
        assert_eq!(store(&app), 0, "nothing drawn: nothing to prepare");
        let w = app.world_mut();
        w.spawn(MeshMaterial3d(drawn.clone()));
        let copy = w.resource_scope(|w, mut store: Mut<Assets<PropMaterial>>| {
            super::super::probe_lit::instance(&mut store, w.get_resource::<DormantProps>(), &copied)
        });
        app.update();
        let w = app.world();
        let assets = w.resource::<Assets<PropMaterial>>();
        assert_eq!(assets.get(&drawn).map(|m| m.params.alpha_cutoff), Some(0.1), "drawn: in the store");
        assert_eq!(assets.get(&copy).map(|m| m.params.alpha_cutoff), Some(0.2), "a copy of a dormant one");
        assert!(assets.get(&copied).is_none(), "what it was copied from still waits");
        assert_eq!(store(&app), 2);
        drop(dropped);
        app.update();
        let mut d = app.world_mut().resource_mut::<DormantProps>();
        d.prune();
        assert_eq!((d.len(), d.live()), (1, 1), "only the copied one's template is still held");
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
