//! Drawing Combat Arms' effects (`game::effects`; specs/combat_arms/
//! effects_sounds.md): the textures come from the install
//! (`FxImages`: `.dtx`, a sprite's first frame), loaded once each.
//!
//! - Decals (6): flat quads of the surface's bullet-hole sprite, 64·k u
//!   wide, spun about the normal, grey `MarkColorRatio`, lit by the scene,
//!   alpha α(t) (`rules::mark_alpha`).
//! - Tracers (4, Corrections (2026-10-30) 3): a strip `Width`/2 u across
//!   from tail to head with one width direction for the whole strip,
//!   parallel to the screen and perpendicular to the strip in camera space
//!   (`rules::tracer_corners`, a mesh rebuilt every frame; seen end-on:
//!   not drawn), the record's texture alone (`Color` and the alpha fade
//!   show nothing), alpha blended, not additive, depth tested and not
//!   written: a wall hides them, they never hide each other. Your own
//!   starts at the data flash point (`CaViewModel::flash_point`, also
//!   while scoped). `cl_tracer_through_walls 1` (archived, default 0)
//!   draws them over everything (`OnTop`: no depth test; the spec's
//!   earlier reading, kept as an option).
//! - Casings (8): the FireFX's casing model and skin (`guns_sc`) at
//!   `ShellScale` × the growth, else a small brass box.
//! - The first-person muzzle flash (3): the muzzle part's or weapon's
//!   `PVMuzzleFXName` group's particle (texture, size keys over its life,
//!   colour keys, additive, a random spin per shot) at the view model's
//!   flash place, in the view-model pass; a missing group shows nothing
//!   (grade C and Red silencers). Its light and every effect group's
//!   `DynaLight` are point lights without shadows (`lights`).
//! - The view model's attached effects (view_models.md 3): `attached_fx`.
//! - Effect groups' particles are Chimera's pool's (drawn by the map).
//! - The textures load through the asset server (`FxImages`,
//!   `textures::DtxLoader`), off the main thread: the first time a
//!   texture is used its effect shows once the texture has arrived (a few
//!   milliseconds later).
//! - The first-person camera (`camera_motion`, after the shell places it
//!   and before transforms propagate): while it is the eye (first person,
//!   no spectating, free or map camera), its upward smoothing
//!   (specs/combat_arms/movement.md "Eye (camera) height",
//!   `movement::CameraRise`, per rendered frame, held off in a jump or a
//!   free-movement volume: `CaMovement::in_jump`, `eye_free`; only on
//!   Combat Arms' movement: a player moving as another game, Lucker
//!   Party's CS:S identity, keeps its own eye); then the
//!   effect groups' camera jitter (CamJitter: a random offset in [−a, a] u
//!   on each world axis every frame, a = `CaFx::jitter_at` the camera).
//!
//! Known shortcuts: lights light what Bevy lights (the lightmapped world
//! may not show them); decals aren't clipped to their polygon (as the
//! game's `MarksClip` 0) and lie 0.2 cm off the surface (their point is
//! the impact's, 1 u back along the ray: 0.5 u off a wall hit at 60°, Open
//! question 19); several jitters add up (`CaFx::jitter_at`, Open question
//! 24); tracers blend by the texture's alpha (the spec's assumption,
//! its Open question 30).

use std::collections::HashMap;

use bevy::{
    camera::visibility::RenderLayers,
    mesh::MeshVertexBufferLayoutRef,
    pbr::{ExtendedMaterial, MaterialExtension, MaterialExtensionKey, MaterialExtensionPipeline},
    prelude::*,
    render::render_resource::{AsBindGroup, CompareFunction, RenderPipelineDescriptor, SpecializedMeshPipelineError},
};
use lithtech_formats::fxf::sample_keys;

use crate::{
    client::{
        FirstPersonCamera,
        spectate::SpecView,
        view::{CameraMode, FreeCam, Watch},
    },
    core::{LocalPlayer, MapView},
    game::{
        attachments::CaMods,
        effects::{
            CaShell, CaTracers,
            debris::{CaDebrisPiece, CaDebrisTable},
            fx::CaFx,
            data::CaFxTables,
            library::{self, normalize},
            pv_muzzle, rules,
        },
        inventory::UNIT,
        movement::{CaMovement, CameraRise, movement_data},
        view_model::{CaViewModel, CaViewParams, VIEW_SCALE},
    },
    map::view_model::{VIEW_MODEL_LAYER, ViewModelCamera},
    weapon::Inventory,
};

pub struct CaEffectsDrawPlugin;

impl Plugin for CaEffectsDrawPlugin {
    fn build(&self, app: &mut App) {
        let mount = crate::game::data::CaData::of(app).mount();
        app.add_plugins(MaterialPlugin::<OnTopMaterial>::default())
            .init_resource::<TracerSettings>()
            .insert_resource(FxImages::new(mount))
            .add_systems(
                Update,
                (decals, tracers, casings, debris_looks, muzzle_flash, super::attached_fx::attached_effects, lights)
                    .after(crate::map::DriveAnimation)
                    .after(crate::game::effects::CaEffectsSet),
            )
            .add_systems(PostUpdate, camera_motion.before(bevy::transform::TransformSystems::Propagate));
        if app.world().get_resource::<crate::console::Console>().is_some() {
            crate::console::resource_cvar::<TracerSettings, u8>(
                app,
                "cl_tracer_through_walls",
                "0: tracers are hidden by walls (default, as the original game); 1: drawn through walls.",
                |s| &mut s.through_walls,
            );
            app.world_mut().resource_mut::<crate::console::Console>().archive("cl_tracer_through_walls");
        }
    }
}

/// Whether tracers show through walls (`cl_tracer_through_walls`; the
/// game depth-tests them, Corrections (2026-10-30) 3).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct TracerSettings {
    pub through_walls: u8,
}

/// A tracer's material: depth tested, or (`through_walls`) drawn over
/// everything.
pub type OnTopMaterial = ExtendedMaterial<StandardMaterial, OnTop>;

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone, Default)]
#[bind_group_data(OnTopKey)]
pub struct OnTop {
    pub through_walls: bool,
}

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct OnTopKey {
    through_walls: bool,
}

impl From<&OnTop> for OnTopKey {
    fn from(m: &OnTop) -> Self {
        Self {
            through_walls: m.through_walls,
        }
    }
}

impl MaterialExtension for OnTop {
    fn specialize(
        _pipeline: &MaterialExtensionPipeline,
        descriptor: &mut RenderPipelineDescriptor,
        _layout: &MeshVertexBufferLayoutRef,
        key: MaterialExtensionKey<Self>,
    ) -> Result<(), SpecializedMeshPipelineError> {
        if let Some(depth) = descriptor.depth_stencil.as_mut() {
            OnTop::apply_depth(depth, key.bind_group_data.through_walls);
        }
        Ok(())
    }
}

impl OnTop {
    /// The depth state of a tracer: writes nothing; through walls (the
    /// option) it always passes, else Bevy's compare stays. Tests check it, as
    /// the pipeline can't be built without a GPU.
    pub fn apply_depth(depth: &mut bevy::render::render_resource::DepthStencilState, through_walls: bool) {
        if through_walls {
            depth.depth_compare = Some(CompareFunction::Always);
        }
        depth.depth_write_enabled = Some(false);
    }
}

/// The install's effect textures as images (None: not in the install),
/// loaded by the asset server (`textures::DtxLoader`, the effects' plain
/// look) from `combat_arms://`; a sprite (`.spr`) is its first frame's
/// texture, named in the sprite file (read here: a few bytes).
#[derive(Resource)]
pub struct FxImages {
    /// The install (None: nothing reads; `CaData::mount`).
    mount: Option<std::sync::Arc<crate::mount::Mount>>,
    images: HashMap<String, Option<Handle<Image>>>,
}

impl FxImages {
    pub fn new(mount: Option<std::sync::Arc<crate::mount::Mount>>) -> Self {
        Self {
            mount,
            images: HashMap::new(),
        }
    }

    /// The install the images come from.
    pub fn mount(&self) -> Option<std::sync::Arc<crate::mount::Mount>> {
        self.mount.clone()
    }

    /// A texture (`.dtx`) or a sprite's (`.spr`: its first frame's), once
    /// asked for loading: None when the install hasn't the file (or the
    /// load failed: the effect then draws untextured from then on).
    pub fn get(&mut self, path: &str, server: &AssetServer) -> Option<Handle<Image>> {
        let path = normalize(path);
        let mount = self.mount.clone();
        let entry = self.images.entry(path.clone()).or_insert_with(|| {
            let mount = mount?;
            let tex = if path.ends_with(".spr") {
                library::sprite_texture(&mount.read(&path).ok()?)?
            } else {
                path.clone()
            };
            mount.contains(&tex).then(|| {
                server
                    .load_builder()
                    .with_settings(|s: &mut super::textures::DtxSettings| s.plain = true)
                    .load(crate::assets::install_path(crate::game::INFO.id, &tex))
            })
        });
        if entry.as_ref().is_some_and(|h| server.load_state(h).is_failed()) {
            *entry = None;
        }
        entry.clone()
    }
}

/// A unit quad in its XY plane facing +Z.
fn quad(meshes: &mut Assets<Mesh>) -> Handle<Mesh> {
    meshes.add(Rectangle::new(1.0, 1.0))
}

// ---------------------------------------------------------------------------
// Decals

#[derive(Component)]
struct DecalQuad(usize);

#[allow(clippy::too_many_arguments)]
fn decals(
    marks: Res<rules::CaMarks>,
    mut drawn: Query<(Entity, &DecalQuad, &mut Transform, &MeshMaterial3d<StandardMaterial>, &mut Visibility)>,
    server: Res<AssetServer>,
    mut fx_images: ResMut<FxImages>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mesh: Local<Option<Handle<Mesh>>>,
    mut commands: Commands,
) {
    let mesh = mesh.get_or_insert_with(|| quad(&mut meshes)).clone();
    let mut slots: HashMap<usize, (Entity, Mut<Transform>, Handle<StandardMaterial>, Mut<Visibility>)> = drawn
        .iter_mut()
        .map(|(e, d, t, m, v)| (d.0, (e, t, m.0.clone(), v)))
        .collect();
    for (i, m) in marks.marks.iter().enumerate() {
        let Some(alpha) = m.alpha() else { continue };
        let n = m.normal.normalize_or(Vec3::Y);
        let rot = Quat::from_rotation_arc(Vec3::Z, n) * Quat::from_rotation_z(m.angle);
        let pose = Transform::from_translation(m.at * UNIT + n * 0.002)
            .with_rotation(rot)
            .with_scale(Vec3::splat(m.width * UNIT));
        let tex = fx_images.get(&m.sprite, &server);
        let color = Color::srgba(m.grey * 2.0, m.grey * 2.0, m.grey * 2.0, alpha);
        match slots.remove(&i) {
            Some((_, mut t, mat, mut v)) => {
                // A mark stays put: rewriting its transform every frame
                // made Bevy propagate it again.
                if *t != pose {
                    *t = pose;
                }
                if *v != Visibility::Inherited {
                    *v = Visibility::Inherited;
                }
                // Written only when it changes: a material borrowed
                // mutably is prepared for the GPU again (a mark that
                // isn't fading every frame; docs/performance.md, "Lucker
                // Party").
                let stale = materials.get(&mat).is_some_and(|m| {
                    let want = if m.base_color_texture.is_some() { color } else { color.with_alpha(alpha * 0.8) };
                    m.base_color != want || (tex.is_some() && m.base_color_texture != tex)
                });
                if stale && let Some(mut mat) = materials.get_mut(&mat) {
                    mat.base_color = if mat.base_color_texture.is_some() { color } else { color.with_alpha(alpha * 0.8) };
                    if tex.is_some() && mat.base_color_texture != tex {
                        mat.base_color_texture = tex;
                    }
                }
            }
            None => {
                let has_tex = tex.is_some();
                commands.spawn((
                    Name::new("Decal"),
                    DecalQuad(i),
                    Mesh3d(mesh.clone()),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color: if has_tex { color } else { Color::srgba(0.05, 0.05, 0.05, alpha * 0.8) },
                        base_color_texture: tex,
                        alpha_mode: AlphaMode::Blend,
                        perceptual_roughness: 1.0,
                        depth_bias: 100.0,
                        ..default()
                    })),
                    pose,
                    bevy::light::NotShadowCaster,
                ));
            }
        }
    }
    for (_, (_, _, _, mut v)) in slots {
        *v = Visibility::Hidden;
    }
}

// ---------------------------------------------------------------------------
// Tracers

#[derive(Component)]
struct TracerQuad(usize);

/// A tracer's mesh: a strip, u along it (0 tail, 1 head), v across.
fn strip_mesh(corners: [Vec3; 4]) -> Mesh {
    use bevy::asset::RenderAssetUsages;
    use bevy::mesh::{Indices, PrimitiveTopology};
    Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, corners.to_vec())
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0, 1.0, 0.0]; 4])
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]])
        .with_inserted_indices(Indices::U32(vec![0, 1, 2, 0, 2, 3]))
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn tracers(
    list: Res<CaTracers>,
    settings: Res<TracerSettings>,
    camera: Option<Single<&GlobalTransform, With<FirstPersonCamera>>>,
    mut drawn: Query<(&TracerQuad, &Mesh3d, &MeshMaterial3d<OnTopMaterial>, &mut Visibility)>,
    server: Res<AssetServer>,
    mut fx_images: ResMut<FxImages>,
    mut materials: ResMut<Assets<OnTopMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    let camera = camera.map_or(Transform::IDENTITY, |c| c.compute_transform());
    let mut slots: HashMap<usize, (Handle<Mesh>, Handle<OnTopMaterial>, Mut<Visibility>)> =
        drawn.iter_mut().map(|(d, g, m, v)| (d.0, (g.0.clone(), m.0.clone(), v))).collect();
    for (i, tr) in list.0.iter().enumerate() {
        let Some((tail, head)) = tr.path.ends(tr.age) else { continue };
        let (a, b) = (
            (tr.path.from + tr.path.dir * tail) * UNIT,
            (tr.path.from + tr.path.dir * head) * UNIT,
        );
        // Seen end-on: nothing to draw (Corrections (2026-10-30) 3).
        let Some(corners) = rules::tracer_corners(a, b, &camera, tr.look.width * UNIT) else { continue };
        // The texture alone: `Color` and the alpha fade show nothing.
        let color = Color::WHITE;
        let tex = fx_images.get(&tr.look.texture, &server);
        match slots.remove(&i) {
            Some((mesh, mat, mut v)) => {
                *v = Visibility::Inherited;
                if let Some(mut m) = meshes.get_mut(&mesh) {
                    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, corners.to_vec());
                }
                if let Some(mut m) = materials.get_mut(&mat) {
                    let through = settings.through_walls != 0;
                    if m.extension.through_walls != through {
                        m.extension.through_walls = through;
                    }
                    if m.base.base_color_texture != tex {
                        m.base.base_color_texture = tex;
                    }
                }
            }
            None => {
                commands.spawn((
                    Name::new("Tracer"),
                    TracerQuad(i),
                    Mesh3d(meshes.add(strip_mesh(corners))),
                    MeshMaterial3d(materials.add(OnTopMaterial {
                        base: StandardMaterial {
                            base_color: color,
                            base_color_texture: tex,
                            alpha_mode: AlphaMode::Blend,
                            unlit: true,
                            double_sided: true,
                            cull_mode: None,
                            ..default()
                        },
                        extension: OnTop {
                            through_walls: settings.through_walls != 0,
                        },
                    })),
                    Transform::default(),
                    bevy::light::NotShadowCaster,
                    // Its mesh moves every frame, its bounds were taken once.
                    bevy::camera::visibility::NoFrustumCulling,
                ));
            }
        }
    }
    for (_, (_, _, mut v)) in slots {
        *v = Visibility::Hidden;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A tracer is hidden by walls unless `cl_tracer_through_walls` is on
    /// (test case 75); the effect groups' particles
    /// (`ParticleDrawMaterial`) always are.
    #[test]
    fn tracers_are_depth_tested_unless_through_walls_is_on() {
        use bevy::render::render_resource::{CompareFunction as Cmp, DepthStencilState, TextureFormat};
        let tested = || DepthStencilState {
            format: TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(Cmp::GreaterEqual),
            stencil: default(),
            bias: default(),
        };
        assert_eq!(TracerSettings::default().through_walls, 0, "off by default");
        let mut tracer = tested();
        OnTop::apply_depth(&mut tracer, false);
        assert_eq!(tracer.depth_compare, Some(Cmp::GreaterEqual), "a wall hides it");
        assert_eq!(tracer.depth_write_enabled, Some(false));
        OnTop::apply_depth(&mut tracer, true);
        assert_eq!(tracer.depth_compare, Some(Cmp::Always), "the option");
        let mut particle = tested();
        chimera_present::render::particles::ParticleDrawMaterial::apply_depth(&mut particle);
        assert_eq!(particle.depth_compare, Some(Cmp::GreaterEqual), "particles are hidden by walls");
    }
}

// ---------------------------------------------------------------------------
// Casings

/// A casing's look: a mesh and material per FireFX record (the model's
/// first piece).
#[derive(Default)]
struct CasingLooks(HashMap<String, (Handle<Mesh>, Handle<StandardMaterial>)>);

#[allow(clippy::too_many_arguments)]
fn casings(
    new: Query<(Entity, &CaShell), Without<Mesh3d>>,
    mut shells: Query<(&CaShell, &mut Transform), With<Mesh3d>>,
    tables: Res<CaFxTables>,
    server: Res<AssetServer>,
    mut fx_images: ResMut<FxImages>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut looks: Local<CasingLooks>,
    mut commands: Commands,
) {
    for (e, s) in &new {
        let record = tables.fire(&s.fire).cloned().unwrap_or_default();
        let (mesh, material) = looks
            .0
            .entry(s.fire.clone())
            .or_insert_with(|| {
                let mesh = fx_images
                    .mount()
                    .and_then(|m| casing_mesh(&record.shell_model, &m))
                    .map(|m| meshes.add(m))
                    .unwrap_or_else(|| meshes.add(Cuboid::new(0.006, 0.006, 0.016)));
                let tex = (!record.shell_skin.is_empty())
                    .then(|| fx_images.get(&record.shell_skin, &server))
                    .flatten();
                let material = materials.add(StandardMaterial {
                    base_color: if tex.is_some() { Color::WHITE } else { Color::srgb(0.8, 0.62, 0.25) },
                    base_color_texture: tex,
                    metallic: 0.6,
                    perceptual_roughness: 0.4,
                    ..default()
                });
                (mesh, material)
            })
            .clone();
        commands.entity(e).insert((Mesh3d(mesh), MeshMaterial3d(material), Visibility::default()));
    }
    for (s, mut t) in &mut shells {
        let base = tables.fire(&s.fire).map_or(1.0, |f| if f.shell_scale > 0.0 { f.shell_scale } else { 1.0 });
        t.scale = Vec3::splat(base * s.scale());
    }
}

// ---------------------------------------------------------------------------
// Prop debris

/// Debris pieces' meshes by model path, and their skins' materials' base
/// (each piece fades on a material of its own).
#[derive(Default)]
struct DebrisLooks(HashMap<String, Handle<Mesh>>);

/// Each new debris piece gets its record's model (the first piece of the
/// `.ltb`; a small block without it) and skin, alpha-blended when the
/// record's `Alpha` is below 1; every frame its fade
/// (`game::effects::debris`).
#[allow(clippy::too_many_arguments)]
fn debris_looks(
    new: Query<(Entity, &CaDebrisPiece), Without<Mesh3d>>,
    shown: Query<(&CaDebrisPiece, &MeshMaterial3d<StandardMaterial>)>,
    table: Option<Res<CaDebrisTable>>,
    server: Res<AssetServer>,
    mut fx_images: ResMut<FxImages>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut looks: Local<DebrisLooks>,
    mut commands: Commands,
) {
    let Some(table) = table else { return };
    for (e, p) in &new {
        let Some(r) = table.get(p.record) else { continue };
        let Some(model) = r.models.get(p.piece.model) else {
            // A record without models (`None`): nothing drawn.
            commands.entity(e).insert((Mesh3d(Handle::default()), Visibility::Hidden));
            continue;
        };
        let mesh = looks
            .0
            .entry(model.clone())
            .or_insert_with(|| {
                fx_images
                    .mount()
                    .and_then(|m| casing_mesh(model, &m))
                    .map(|m| meshes.add(m))
                    .unwrap_or_else(|| meshes.add(Cuboid::new(0.05, 0.01, 0.05)))
            })
            .clone();
        let tex = r.skins.first().filter(|s| !s.is_empty()).and_then(|s| fx_images.get(s, &server));
        let alpha = p.piece.alpha;
        let material = materials.add(StandardMaterial {
            base_color: if tex.is_some() { Color::srgba(1.0, 1.0, 1.0, alpha) } else { Color::srgba(0.7, 0.75, 0.8, alpha) },
            base_color_texture: tex,
            alpha_mode: if alpha < 1.0 { AlphaMode::Blend } else { AlphaMode::Opaque },
            double_sided: true,
            cull_mode: None,
            perceptual_roughness: 0.5,
            ..default()
        });
        commands.entity(e).insert((Mesh3d(mesh), MeshMaterial3d(material), Visibility::default()));
    }
    for (p, m) in &shown {
        let Some(r) = table.get(p.record) else { continue };
        let a = p.piece.alpha_now(r.fade);
        if let Some(mut mat) = materials.get_mut(&m.0)
            && (mat.base_color.alpha() - a).abs() > 1e-3
        {
            mat.base_color.set_alpha(a);
            if a < 1.0 {
                mat.alpha_mode = AlphaMode::Blend;
            }
        }
    }
}

/// A casing model's first piece as a mesh (metres).
fn casing_mesh(path: &str, mount: &crate::mount::Mount) -> Option<Mesh> {
    if path.is_empty() {
        return None;
    }
    let file = |p: &str| mount.read(p).ok();
    let loaded = crate::game::model::load(path, &file).ok()?;
    let model = crate::game::model::convert(&loaded, UNIT, &|_| String::new());
    let m = model.meshes.into_iter().find(|m| !m.positions.is_empty())?;
    let mut mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    );
    let n = m.positions.len();
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, m.positions);
    mesh.insert_attribute(
        Mesh::ATTRIBUTE_NORMAL,
        if m.normals.len() == n { m.normals } else { vec![[0.0, 1.0, 0.0]; n] },
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, if m.uvs.len() == n { m.uvs } else { vec![[0.0; 2]; n] });
    mesh.insert_indices(bevy::mesh::Indices::U32(m.indices));
    Some(mesh)
}

// ---------------------------------------------------------------------------
// The first-person muzzle flash

/// The flash quad, a child of the view-model camera, and its light.
#[derive(Component)]
struct FlashQuad;

#[derive(Component)]
struct FlashLight;

#[derive(Default)]
struct FlashState {
    shot: Option<f64>,
    roll: f32,
    seed: u32,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn muzzle_flash(
    time: Res<Time>,
    player: Option<Single<(&CaViewModel, &Inventory), With<LocalPlayer>>>,
    weapons: Query<(Option<&CaViewParams>, Option<&CaMods>)>,
    camera: Option<Single<Entity, With<ViewModelCamera>>>,
    tables: Res<CaFxTables>,
    mut quads: Query<(&mut Transform, &mut Visibility, &MeshMaterial3d<StandardMaterial>), (With<FlashQuad>, Without<FlashLight>)>,
    mut light: Query<(&mut Transform, &mut PointLight, &mut Visibility), (With<FlashLight>, Without<FlashQuad>)>,
    server: Res<AssetServer>,
    mut fx_images: ResMut<FxImages>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut state: Local<FlashState>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let Ok((mut qt, mut qv, qm)) = quads.single_mut() else {
        let Some(camera) = camera else { return };
        let q = commands
            .spawn((
                FlashQuad,
                Mesh3d(quad(&mut meshes)),
                MeshMaterial3d(materials.add(StandardMaterial {
                    base_color: Color::srgb(1.0, 0.75, 0.4),
                    alpha_mode: AlphaMode::Add,
                    unlit: true,
                    double_sided: true,
                    cull_mode: None,
                    ..default()
                })),
                Transform::default(),
                Visibility::Hidden,
                RenderLayers::layer(VIEW_MODEL_LAYER),
                bevy::light::NotShadowCaster,
            ))
            .id();
        commands.entity(*camera).add_child(q);
        commands.spawn((
            FlashLight,
            PointLight {
                shadow_maps_enabled: false,
                intensity: 0.0,
                radius: 0.0,
                ..default()
            },
            Transform::default(),
            Visibility::Hidden,
            RenderLayers::from_layers(&[0, VIEW_MODEL_LAYER]),
        ));
        return;
    };
    let shown = player.as_ref().and_then(|p| {
        let (vm, inv) = (p.0, p.1);
        let at = vm.flash_at?;
        let (view, mods) = inv.active.and_then(|w| weapons.get(w).ok())?;
        let view = view?;
        let group = pv_muzzle(&tables, &view.muzzle_fx, &view.flash_hider_fx, mods).to_string();
        Some((vm.clone(), at, group))
    });
    let Some((vm, at, group)) = shown else {
        *qv = Visibility::Hidden;
        if let Ok((_, _, mut v)) = light.single_mut() {
            *v = Visibility::Hidden;
        }
        state.shot = None;
        return;
    };
    if vm.flash.shot_at != state.shot {
        state.shot = vm.flash.shot_at;
        state.seed = state.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        state.roll = (state.seed >> 8) as f32 / (1u32 << 24) as f32 * std::f32::consts::TAU;
    }
    let age = state.shot.map_or(0.0, |s| (now - s) as f32);
    let lib = library::library();
    // Without the effect file: a plain glow (no install); with it, the
    // group's particle, or nothing when the group is missing.
    let (size, color, tex, light_keys) = match lib.as_ref() {
        None => (0.016, Vec3::new(1.0, 0.75, 0.4), None, None),
        Some(lib) => {
            let Some(g) = lib.group(&group) else {
                *qv = Visibility::Hidden;
                return;
            };
            let p = g.keys.iter().find(|k| k.kind == "ParticleSystem" && !library::shaded(k));
            let l = g.keys.iter().find(|k| k.kind == "DynaLight");
            let Some(p) = p else {
                *qv = Visibility::Hidden;
                return;
            };
            let life = p.float("MaxParticleLifeSpan").unwrap_or(0.05).max(1e-3);
            let t = (age / life).min(1.0);
            let s = sample_keys(&p.scale_keys(), t, 0.5);
            let ck = p.color_keys();
            let ch = |i: usize| sample_keys(&ck.iter().map(|(k, c)| (*k, c[i])).collect::<Vec<_>>(), t, 1.0);
            let tex = lib.texture_of(p).and_then(|path| fx_images.get(&path, &server));
            // A particle of size s is a square of side 4 s view units.
            (4.0 * s * VIEW_SCALE, Vec3::new(ch(0), ch(1), ch(2)), tex, l.map(|l| (l.scale_keys(), l.color_keys(), l.end - l.start)))
        }
    };
    *qt = Transform::from_translation(at)
        .with_rotation(Quat::from_rotation_z(state.roll))
        .with_scale(Vec3::splat(size));
    *qv = Visibility::Inherited;
    if let Some(mut m) = materials.get_mut(&qm.0) {
        m.base_color = Color::srgb(color.x, color.y, color.z);
        if tex.is_some() && m.base_color_texture != tex {
            m.base_color_texture = tex;
        }
    }
    if let Ok((mut lt, mut pl, mut lv)) = light.single_mut() {
        match light_keys {
            Some((radius, color, life)) if age <= life => {
                let t = (age / life.max(1e-3)).min(1.0);
                let r = sample_keys(&radius, t, 0.0) * UNIT;
                let ch = |i: usize| sample_keys(&color.iter().map(|(k, c)| (*k, c[i])).collect::<Vec<_>>(), t, 1.0);
                lt.translation = vm.flash_point;
                pl.range = r.max(0.01);
                pl.color = Color::srgb(ch(0), ch(1), ch(2));
                pl.intensity = LIGHT_LUMENS;
                *lv = Visibility::Inherited;
            }
            _ => *lv = Visibility::Hidden,
        }
    }
}

/// Effect lights' intensity: Chimera's world shader reads colour ×
/// intensity / 4π as lightmap units (render::view_model's flash lights),
/// so a light adds its colour at its centre, falling to 0 at its radius.
const LIGHT_LUMENS: f32 = 4.0 * std::f32::consts::PI;

// ---------------------------------------------------------------------------
// Effect groups' lights

#[derive(Component)]
struct FxLightSlot(usize);

/// At most this many effect lights at once (the strongest).
const MAX_LIGHTS: usize = 8;

fn lights(fx: Res<CaFx>, mut slots: Query<(&FxLightSlot, &mut Transform, &mut PointLight, &mut Visibility)>, mut commands: Commands) {
    if slots.is_empty() {
        for i in 0..MAX_LIGHTS {
            commands.spawn((
                FxLightSlot(i),
                PointLight {
                    shadow_maps_enabled: false,
                    intensity: 0.0,
                    radius: 0.0,
                    ..default()
                },
                Transform::default(),
                Visibility::Hidden,
                RenderLayers::from_layers(&[0, VIEW_MODEL_LAYER]),
            ));
        }
        return;
    }
    let mut list: Vec<(f32, Vec3, Vec3)> = fx
        .lights
        .iter()
        .map(|l| (l.radius_now(), l.at, l.color_now()))
        .filter(|l| l.0 > 0.0)
        .collect();
    list.sort_by(|a, b| b.0.total_cmp(&a.0));
    for (slot, mut t, mut pl, mut v) in &mut slots {
        match list.get(slot.0) {
            Some((r, at, c)) => {
                t.translation = *at;
                pl.range = r * UNIT;
                pl.color = Color::srgb(c.x, c.y, c.z);
                pl.intensity = LIGHT_LUMENS;
                *v = Visibility::Inherited;
            }
            None => *v = Visibility::Hidden,
        }
    }
}

// ---------------------------------------------------------------------------
// The camera's rise and jitter

/// `camera_motion`'s state: the rise's last height and the jitter's draws.
#[derive(Default)]
pub struct CameraMotion {
    pub rise: CameraRise,
    rng: u32,
}

impl CameraMotion {
    /// A draw in [−1, 1].
    fn signed(&mut self) -> f32 {
        self.rng = self.rng.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.rng >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }
}

/// A camera jitter's offset (metres) for amplitude `a` (u): each world
/// axis uniform in [−a, a] (effects_sounds.md, CamJitter).
pub fn jitter_offset(a: f32, draws: [f32; 3]) -> Vec3 {
    Vec3::from(draws).clamp(Vec3::splat(-1.0), Vec3::ONE) * a * UNIT
}

/// The local player's first-person camera, as the shell placed it this
/// frame: risen smoothly (while it is the eye) and jittered.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn camera_motion(
    time: Res<Time>,
    fx: Option<Res<CaFx>>,
    player: Option<Single<(&Transform, Option<&CaMovement>, Has<MapView>, &Children), (With<LocalPlayer>, Without<FirstPersonCamera>)>>,
    mut cameras: Query<&mut Transform, With<FirstPersonCamera>>,
    (mode, freecam, spec, watch): (Option<Res<CameraMode>>, Option<Res<FreeCam>>, Option<Res<SpecView>>, Option<Res<Watch>>),
    mut state: Local<CameraMotion>,
) {
    let Some(player) = player else {
        state.rise = CameraRise::default();
        return;
    };
    let (body, movement, map_view, children) = *player;
    let mut cams = cameras.iter_many_mut(children).matched();
    let Some(mut cam) = cams.fetch_next() else { return };
    let at = body.transform_point(cam.translation);
    let mut offset = Vec3::ZERO;
    let eye = !mode.is_some_and(|m| m.third_person)
        && freecam.is_none_or(|f| f.mode == 0)
        && spec.is_none_or(|s| s.pose.is_none())
        && watch.is_none_or(|w| w.0 == 0)
        && !map_view;
    // The rise is Combat Arms' movement's: a player moving as another
    // game (Lucker Party's CS:S identity) has its own eye, unsmoothed.
    if eye && let Some(m) = movement {
        let wanted = at.y / UNIT;
        let in_jump = m.in_jump || m.eye_free;
        let h = state.rise.step(wanted, time.delta_secs(), in_jump, movement_data());
        offset.y += (h - wanted) * UNIT;
    } else {
        state.rise = CameraRise::default();
    }
    let a = fx.map_or(0.0, |fx| fx.jitter_at(at));
    if a > 0.0 {
        let draws = [state.signed(), state.signed(), state.signed()];
        offset += jitter_offset(a, draws);
    }
    if offset != Vec3::ZERO {
        cam.translation += body.rotation.inverse() * offset / body.scale;
    }
}
