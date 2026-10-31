//! First-person view models (any game): the model a character sees of its
//! own weapon, with hands, drawn at the eye over the world. Games load
//! `MapViewModel`s with the map (or later: `map::add_view_model`, built
//! on the next frame by `build_late` with its own textures, also with no
//! map spawned: `assets_without_map`), drive each
//! character's `ViewAnimator`
//! (which sequence plays) and `ViewModelOffset` (bob and sway), and send
//! `ViewModelEvent`s (muzzle flashes; their types are in `map::view_model`);
//! this module draws the view model of
//! the character whose camera carries `ViewModelAnchor`.
//!
//! The model is drawn by its own camera on `VIEW_MODEL_LAYER`, after the
//! world camera and with a fresh depth buffer, so it never clips into walls,
//! and with its own field of view (`ViewModelSettings`). It is mirrored
//! across the eye's vertical plane when its handedness differs from the
//! player's, and lit per pixel by the map's light at its lighting origin
//! (`LightField`) plus the scene's point lights (`DynamicLight`).
//!
//! First-person view models for any game: `MapViewModel` (skinned meshes in eye
//! space, own skeleton, `AnimSet`, handedness, attachments, lighting origin),
//! `ViewModels`, the `ViewAnimator` and `ViewModelOffset` (bob/sway) games
//! drive on each character, `ViewModelSettings` (FOV, hand, draw), and the
//! drawing at a `ViewModelAnchor` camera (another character's with
//! `ViewModelSource` on it: a spectator in first person; a child camera on
//! `VIEW_MODEL_LAYER` with its own depth and FOV, so the model never clips into
//! walls; mirrored when its hand differs from the player's; lit per pixel by
//! the `LightField` at its lighting origin; it draws only while something
//! on its layer is visible, or in HDR: `view_model_cameras_draw`); muzzle flashes from
//! `ViewModelEvent`s (sprites re-projected into the world view, or at a held
//! weapon's muzzle, and a pooled `DynamicLight`); `ViewModelScreen`: a picture
//! on the model's screen between its `controlpanel0_ll`/`_ur` attachments
//! (Source's VGUI screens; the C4's keypad).
//!
//! Known shortcuts (docs/tech-debt.md): View models: near plane 1 cm (not the
//! spec's 1 unit) and their own depth buffer (not a squeezed depth range); no
//! light smoothing or ambient boost; muzzle flash look and light falloff, and the shell direction,
//! are our choices from the spec's data (plans/active/view-models.md open
//! questions). Why it's OK for now: Each needs a CS:S comparison to settle.

use std::collections::HashMap;

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::{
    camera::visibility::RenderLayers,
    light::{NotShadowCaster, NotShadowReceiver},
    prelude::*,
};

use super::{
    bodies::BodyJoint,
    prop_material::PropMaterial,
    sprite_material::{SpriteMaterial, SpriteParams},
};
use crate::map::{
    LightField, LightProbe, MapBone, MapData, MapDebugView, MapEnvmap, MapFlashLight, MapMuzzleFlash,
    view_model::*,
};

/// The drawn view model under a `ViewModelCamera`.
#[derive(Component)]
pub struct ViewModelBody {
    key: String,
    mirrored: bool,
    joints: Vec<Entity>,
    materials: Vec<Handle<PropMaterial>>,
    /// Where the probe was last sampled (world), and the cubemap used.
    lit_at: Option<Vec3>,
    cubemap: Option<usize>,
}

/// One drawable part of a view model: its mesh and its material drawn
/// normally and mirrored (front faces culled).
pub struct ViewModelPart {
    mesh: Handle<Mesh>,
    materials: [Handle<PropMaterial>; 2],
    envmap: Option<MapEnvmap>,
    /// Its `$bumpmap` (`MapData::textures`): the reflection mask under
    /// `$normalmapalphaenvmapmask` (the hands').
    normal_map: Option<usize>,
}

pub struct ViewModelAsset {
    parts: Vec<ViewModelPart>,
    bindposes: Handle<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
    bones: Vec<MapBone>,
    root: Transform,
}

/// Meshes, materials and bind poses of each view model, by key, and what
/// lights them.
#[derive(Resource)]
pub struct ViewModelAssets {
    models: HashMap<String, ViewModelAsset>,
    /// Cubemap samples (position, image) for `env_cubemap` reflections.
    cubemaps: Vec<(Vec3, Handle<Image>)>,
    /// All cubemaps, for materials that name theirs.
    cubemap_images: Vec<Handle<Image>>,
    /// The map's textures, then those of view models added later
    /// (`build_late`).
    textures: Vec<Handle<Image>>,
    light_scale: f32,
    /// What later view models' materials take from the map: its fog and
    /// look (`late_base`), whether it has a light field, the debug view.
    late_base: MapData,
    lit: bool,
    view: MapDebugView,
}

/// What it takes to build one view model's render assets.
struct Build<'a> {
    textures: &'a [Handle<Image>],
    data: &'a MapData,
    lit: bool,
    view: MapDebugView,
}

impl Build<'_> {
    fn model(
        &self,
        v: &MapViewModel,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<PropMaterial>,
        bindposes: &mut Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
    ) -> ViewModelAsset {
        let (textures, data, view) = (self.textures, self.data, self.view);
        let parts = v
            .model
            .meshes
            .iter()
            .map(|m| {
                let mut mesh = crate::render::build_mesh(m, false);
                if m.joints.len() == m.positions.len() && !v.bones.is_empty() {
                    mesh.insert_attribute(
                        Mesh::ATTRIBUTE_JOINT_INDEX,
                        bevy::mesh::VertexAttributeValues::Uint16x4(m.joints.clone()),
                    );
                    mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, m.joint_weights.clone());
                }
                let mut material = crate::render::lit_prop_material(m, textures, data, view, false);
                // Without a light field it shows its texture as is.
                if !m.unlit && view != MapDebugView::Albedo && self.lit {
                    material.params.set_probe(&LightProbe::default(), 1.0);
                }
                let mut mirrored = material.clone();
                mirrored.cull_front = true;
                ViewModelPart {
                    mesh: meshes.add(mesh),
                    materials: [materials.add(material), materials.add(mirrored)],
                    envmap: m.envmap.filter(|_| view == MapDebugView::Normal),
                    normal_map: m.normal_map,
                }
            })
            .collect();
        // Each bone's reference pose in the eye's space, inverted.
        let mut global: Vec<Mat4> = Vec::with_capacity(v.bones.len());
        for b in &v.bones {
            let local = Mat4::from_rotation_translation(b.rotation, b.position);
            let parent = b
                .parent
                .and_then(|p| global.get(p).copied())
                .unwrap_or(v.root.to_matrix());
            global.push(parent * local);
        }
        let inverse: Vec<Mat4> = global.iter().map(|m| m.inverse()).collect();
        ViewModelAsset {
            parts,
            bindposes: bindposes.add(bevy::mesh::skinning::SkinnedMeshInverseBindposes::from(inverse)),
            bones: v.bones.clone(),
            root: v.root,
        }
    }
}

/// Build the view models' render assets.
#[allow(clippy::too_many_arguments)]
pub fn build_assets(
    data: &MapData,
    textures: &[Handle<Image>],
    cubemaps: &[Handle<Image>],
    view: MapDebugView,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<PropMaterial>,
    bindposes: &mut Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
) -> ViewModelAssets {
    let build = Build {
        textures,
        data,
        lit: data.light_field.is_some(),
        view,
    };
    let models = data
        .kit.view_models
        .iter()
        .map(|v| (v.key.clone(), build.model(v, meshes, materials, bindposes)))
        .collect();
    let lighting_scale = if let MapDebugView::Lighting { scale } = view {
        scale
    } else {
        1.0
    };
    ViewModelAssets {
        models,
        cubemaps: data
            .cubemap_samples
            .iter()
            .filter_map(|(p, i)| Some((*p, cubemaps.get(*i)?.clone())))
            .collect(),
        cubemap_images: cubemaps.to_vec(),
        textures: textures.to_vec(),
        light_scale: data.look.light_scale * lighting_scale,
        late_base: MapData {
            fog: data.fog.clone(),
            look: data.look.clone(),
            ..default()
        },
        lit: data.light_field.is_some(),
        view,
    }
}

/// Build the render assets of view models added after the map spawned
/// (`map::add_view_model`): their own textures (`LateViewModelTextures`)
/// become images after the map's, their meshes' indices moved past them.
/// With no map's assets (`assets_without_map`) only theirs.
#[allow(clippy::too_many_arguments)]
pub fn build_late(
    models: Option<Res<ViewModels>>,
    late: Option<Res<LateViewModelTextures>>,
    mut assets: ResMut<ViewModelAssets>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    mut bindposes: ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    compressed: Option<Res<bevy::image::CompressedImageFormatSupport>>,
) {
    let (Some(models), Some(late), Some(mut materials)) = (models, late, materials) else {
        return;
    };
    for v in models.0.iter() {
        let Some(own) = late.0.get(&v.key).filter(|_| !assets.models.contains_key(&v.key)) else {
            continue;
        };
        let base = assets.textures.len();
        let look = assets.late_base.look.clone();
        let bc = super::images::bc_wanted(compressed.as_deref());
        assets
            .textures
            .extend(own.iter().map(|t| images.add(super::images::to_image(t, &look, bc))));
        let mut moved = v.clone();
        for m in &mut moved.model.meshes {
            m.texture = m.texture.map(|t| t + base);
            m.normal_map = m.normal_map.map(|t| t + base);
            if let Some(d) = &mut m.detail {
                d.texture += base;
            }
            if let Some(s) = &mut m.selfillum {
                s.mask = s.mask.map(|t| t + base);
            }
            if let Some(f) = &mut m.skin_frames {
                f.frames.iter_mut().for_each(|t| *t += base);
            }
            if let Some(l) = &mut m.layer {
                l.texture = l.texture.map(|t| t + base);
            }
        }
        let asset = Build {
            textures: &assets.textures,
            data: &assets.late_base,
            lit: assets.lit,
            view: assets.view,
        }
        .model(&moved, &mut meshes, &mut materials, &mut bindposes);
        assets.models.insert(v.key.clone(), asset);
    }
}

/// View models added while no `MapData` map is spawned (the greybox, or
/// before the first map: `map::add_view_model` inserts `ViewModels`, but
/// only a map's spawn built `ViewModelAssets`, so nothing drew them and
/// the game's placeholder had stepped aside): the drawing's assets with
/// nothing of a map (no textures, cubemaps, fog or light field, so the
/// models show their textures as they are), for `build_late` to fill. A
/// map spawned later replaces them with its own.
pub fn assets_without_map(
    models: Option<Res<ViewModels>>,
    assets: Option<Res<ViewModelAssets>>,
    meshes: Option<ResMut<Assets<Mesh>>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    bindposes: Option<ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>>,
    mut commands: Commands,
) {
    if models.is_none() || assets.is_some() {
        return;
    }
    let (Some(mut meshes), Some(mut materials), Some(mut bindposes)) = (meshes, materials, bindposes) else {
        return;
    };
    commands.insert_resource(build_assets(
        &MapData::default(),
        &[],
        &[],
        MapDebugView::Normal,
        &mut meshes,
        &mut materials,
        &mut bindposes,
    ));
}

/// The camera's horizontal 4:3 field of view (degrees) from its vertical
/// one.
fn camera_fov_43(projection: &Projection) -> Option<f32> {
    let Projection::Perspective(p) = projection else {
        return None;
    };
    Some(2.0 * ((p.fov / 2.0).tan() / 0.75).atan().to_degrees())
}

/// Keep a view-model camera under each anchor, matching the anchor
/// camera's target, order and tonemapping, and the shown view model under
/// it; then place, pose and light it.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn draw_view_models(
    time: Res<Time>,
    assets: Res<ViewModelAssets>,
    settings: Option<Res<ViewModelSettings>>,
    game_fovs: Option<Res<crate::map::GameViewModelFovs>>,
    identities: Query<&chimera_core::slots::GameIdentity>,
    models: Option<Res<ViewModels>>,
    light_field: Option<Res<LightField>>,
    anchors: Query<
        (
            Entity,
            &ChildOf,
            &Camera,
            &Projection,
            &GlobalTransform,
            Option<&Children>,
            Option<&bevy::core_pipeline::tonemapping::Tonemapping>,
            Option<&bevy::camera::RenderTarget>,
            Option<&ViewModelSource>,
        ),
        (With<ViewModelAnchor>, Without<ViewModelCamera>),
    >,
    owners: Query<(&ViewAnimator, Option<&ViewModelOffset>)>,
    // Weapons put away by map entities (`core::MapControls`): no view model.
    put_away: Query<&crate::core::MapControls>,
    third_person: Option<Res<crate::map::ShowLocalBody>>,
    mut cameras: Query<
        (
            Entity,
            &mut Camera,
            &mut Projection,
            Option<&Children>,
            Option<&bevy::camera::RenderTarget>,
            Option<&bevy::core_pipeline::tonemapping::Tonemapping>,
        ),
        With<ViewModelCamera>,
    >,
    mut bodies: Query<(&mut ViewModelBody, &mut Transform, &mut Visibility), Without<BodyJoint>>,
    mut joints: Query<&mut Transform, With<BodyJoint>>,
    mut materials: Option<ResMut<Assets<PropMaterial>>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let settings = settings.map(|s| *s).unwrap_or_default();
    for (anchor, parent, anchor_camera, anchor_projection, eye, children, tonemapping, anchor_target, source) in
        &anchors
    {
        let state = owners.get(anchor_owner(parent, source)).ok();
        // In third person the own body holds the weapon; no view model.
        let hidden = third_person.as_ref().is_some_and(|t| t.0)
            || settings.draw == 0
            || state.is_some_and(|(s, _)| s.hidden)
            || put_away
                .get(anchor_owner(parent, source))
                .is_ok_and(|c| c.weapon_hidden != 0);
        let shown = state
            .filter(|_| !hidden)
            .and_then(|(s, _)| s.key.as_deref())
            .and_then(|k| Some((k, assets.models.get(k)?, models.as_ref()?.get(k)?)));
        let camera = children.into_iter().flatten().find(|c| cameras.contains(**c)).copied();
        let Some((camera, mut cam, mut projection, cam_children, target, cam_tonemapping)) =
            camera.and_then(|c| cameras.get_mut(c).ok())
        else {
            let mut e = commands.spawn((
                Name::new("View model camera"),
                ViewModelCamera,
                Camera3d::default(),
                Camera {
                    order: anchor_camera.order + 1,
                    clear_color: ClearColorConfig::None,
                    ..default()
                },
                Projection::Perspective(PerspectiveProjection::default()),
                RenderLayers::layer(VIEW_MODEL_LAYER),
                Transform::default(),
                Visibility::default(),
                ChildOf(anchor),
            ));
            if let Some(t) = anchor_target {
                // Born on the anchor's target, not the window's.
                e.insert(t.clone());
            }
            if let Some(t) = tonemapping {
                e.insert(*t);
            }
            continue;
        };
        // Whether it draws at all is `view_model_cameras_draw`'s.
        if cam.order != anchor_camera.order + 1 {
            cam.order = anchor_camera.order + 1;
        }
        if let Some(t) = tonemapping
            && cam_tonemapping != Some(t)
        {
            commands.entity(camera).insert(*t);
        }
        // Off-screen captures (`--views`) retarget the anchor camera; the
        // view model must draw into the same image.
        if let Some(t) = anchor_target
            && target.and_then(|c| c.normalize(None)) != t.normalize(None)
        {
            commands.entity(camera).insert(t.clone());
        }
        // Field of view (spec view_models.md 5): the view model's, zoomed
        // with the player's, constant vertically. Near plane: Source's is 1
        // game unit, but that cuts into the knife's hands here (open
        // question in docs/plans/active/view-models.md), so 1 cm.
        if let (Some(player), Projection::Perspective(p)) = (camera_fov_43(anchor_projection), &mut *projection) {
            let own = game_fovs.as_ref().map_or(settings, |g| {
                let who = identities.get(anchor_owner(parent, source)).ok().map(|i| i.0.as_str());
                g.settings_for(settings, who)
            });
            let fov = vertical_fov(view_model_fov(&own, player)).to_radians();
            let near = 0.01;
            if (p.fov - fov).abs() > 1e-5 || p.near != near {
                p.fov = fov;
                p.near = near;
            }
        }
        let body = cam_children
            .into_iter()
            .flatten()
            .find(|c| bodies.contains(**c))
            .copied();
        let mirror =
            shown.is_some_and(|(_, _, m)| mirrored(m.right_handed, m.allow_flipping, settings.right_hand != 0));
        let current = body
            .and_then(|b| bodies.get(b).ok())
            .map(|(b, ..)| (b.key.clone(), b.mirrored));
        if current != shown.map(|(k, _, _)| (k.to_string(), mirror)) {
            if let Some(e) = body {
                commands.entity(e).try_despawn();
            }
            if let Some((key, a, _)) = shown {
                spawn_body(&mut commands, camera, key, a, mirror, materials.as_deref_mut());
            }
            continue;
        }
        let (Some(body), Some((_, asset, model))) = (body, shown) else {
            continue;
        };
        let Ok((mut body, mut placed, mut visibility)) = bodies.get_mut(body) else {
            continue;
        };
        let offset = state.and_then(|(_, o)| o.copied()).unwrap_or_default();
        placed.set_if_neq(placement(&offset, mirror));
        // Pose the shown model.
        let animator = state
            .and_then(|(s, _)| s.animator.as_ref())
            .filter(|a| a.main.is_some());
        if let Some(animator) = animator {
            for (joint, (q, p)) in body.joints.iter().zip(animator.pose(now)) {
                if let Ok(mut t) = joints.get_mut(*joint) {
                    t.rotation = q;
                    t.translation = p;
                }
            }
        }
        // A new body is hidden until its first pose: its skeleton spawns in
        // the reference pose, which for view models is nothing like the
        // drawn weapon (a full-screen mess for a frame on every switch).
        // And one with nothing playing (no sequence for it) stays hidden
        // rather than T-posed: CS:S draws no hands without a weapon.
        // (A finished sequence keeps its last pose: only no animator at
        // all hides it again.)
        let unanimated = model.animations.is_some() && state.is_none_or(|(s, _)| s.animator.is_none());
        if unanimated {
            visibility.set_if_neq(Visibility::Hidden);
        } else if *visibility == Visibility::Hidden && (animator.is_some() || model.animations.is_none()) {
            *visibility = Visibility::Inherited;
        }
        // Light it from one point near the eye (spec view_models.md 9): its
        // lighting origin, placed with the model but not mirrored.
        let Some(materials) = materials.as_mut() else { continue };
        let origin = eye.transform_point(
            Transform::from_translation(offset.translation)
                .with_rotation(offset.rotation)
                .transform_point(asset.root.transform_point(model.light_origin)),
        );
        let moved = body
            .lit_at
            .is_none_or(|at| at.distance(origin) > 0.5 * asset.root.scale.x);
        if let (true, Some(field)) = (moved, light_field.as_ref()) {
            let probe = (field.0.0)(origin);
            for m in &body.materials {
                if let Some(mut material) = materials.get_mut(m)
                    && material.params.probe > 0.5
                {
                    material.params.set_probe(&probe, assets.light_scale);
                }
            }
            body.lit_at = Some(origin);
        }
        // Reflections: the cubemap nearest the eye.
        let nearest = assets
            .cubemaps
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.0.distance(origin).total_cmp(&b.1.0.distance(origin)))
            .map(|(i, _)| i);
        if nearest != body.cubemap
            && let Some(i) = nearest
        {
            for (part, m) in asset.parts.iter().zip(&body.materials) {
                if let (Some(env), Some(mut material)) = (part.envmap, materials.get_mut(m)) {
                    let cube = env
                        .cubemap
                        .and_then(|c| assets.cubemap_images.get(c).cloned())
                        .unwrap_or_else(|| assets.cubemaps[i].1.clone());
                    crate::render::set_prop_envmap(&mut material, &env, cube, &assets.textures, part.normal_map);
                }
            }
            body.cubemap = nearest;
        }
    }
}

/// Where `view_model_cameras_draw` runs (PostUpdate, after visibility
/// propagation and before each view's visibility check): systems that put
/// post-processing on the last camera drawing the view run after it.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct ViewModelCameraSet;

/// A view-model camera draws only while its anchor's camera does and
/// something is on `VIEW_MODEL_LAYER` to draw (a view model, a shell, the
/// under-water overlay, a muzzle flash): with nothing there (spectating
/// in free look, dead, in menus, no weapon drawn) it would still run its
/// passes, clear its depth and copy the resolved image back into the
/// multisampled one (`msaa_writeback`) for an unchanged picture. The
/// post-processing that goes on the last camera drawing the view (night
/// vision, colour correction) then goes on the anchor's own camera.
/// A camera in HDR (`mat_hdr_level` 1 or 2: `Hdr` on it) always draws:
/// its auto exposure keeps a state per view, which moving it to another
/// camera would restart.
pub fn view_model_cameras_draw(
    anchors: Query<&Camera, (With<ViewModelAnchor>, Without<ViewModelCamera>)>,
    mut cameras: Query<(&ChildOf, &mut Camera, Has<bevy::camera::Hdr>), With<ViewModelCamera>>,
    drawn: Query<(&RenderLayers, &InheritedVisibility), With<Mesh3d>>,
) {
    if cameras.is_empty() {
        return;
    }
    let layer = RenderLayers::layer(VIEW_MODEL_LAYER);
    let something = drawn.iter().any(|(l, v)| v.get() && l.intersects(&layer));
    for (parent, mut cam, hdr) in &mut cameras {
        let anchor = anchors.get(parent.parent()).is_ok_and(|c| c.is_active);
        let active = anchor && (something || hdr);
        if cam.is_active != active {
            cam.is_active = active;
        }
    }
}

fn spawn_body(
    commands: &mut Commands,
    camera: Entity,
    key: &str,
    asset: &ViewModelAsset,
    mirror: bool,
    mut assets: Option<&mut Assets<PropMaterial>>,
) {
    let layer = RenderLayers::layer(VIEW_MODEL_LAYER);
    let body = commands
        .spawn((
            Name::new(format!("View model {key}")),
            placement(&ViewModelOffset::default(), mirror),
            // Shown once posed (draw_view_models).
            Visibility::Hidden,
            ChildOf(camera),
        ))
        .id();
    let root = commands.spawn((asset.root, Visibility::Inherited, ChildOf(body))).id();
    let mut joints: Vec<Entity> = Vec::with_capacity(asset.bones.len());
    for (i, b) in asset.bones.iter().enumerate() {
        let parent = b.parent.and_then(|p| joints.get(p).copied()).unwrap_or(root);
        joints.push(
            commands
                .spawn((
                    BodyJoint(i),
                    Transform::from_translation(b.position).with_rotation(b.rotation),
                    Visibility::Inherited,
                    ChildOf(parent),
                ))
                .id(),
        );
    }
    let mut materials = Vec::with_capacity(asset.parts.len());
    for part in &asset.parts {
        // Mirroring turns triangles inside out: the mirrored material culls
        // front faces.
        let mut material = part.materials[mirror as usize].clone();
        // A skin that is a sequence runs on this body's own clock
        // (`skin_frames`): its own material.
        if let Some(assets) = assets.as_deref_mut()
            && assets.get(&material).is_some_and(|m| m.skin_frames.len() > 1)
        {
            material = super::probe_lit::instance(assets, &material);
        }
        materials.push(material.clone());
        commands.spawn((
            Mesh3d(part.mesh.clone()),
            MeshMaterial3d(material),
            bevy::mesh::skinning::SkinnedMesh {
                inverse_bindposes: asset.bindposes.clone(),
                joints: joints.clone(),
            },
            layer.clone(),
            NotShadowCaster,
            NotShadowReceiver,
            // Its bounds are the reference pose's, not the animated one's.
            bevy::camera::visibility::NoFrustumCulling,
            ChildOf(body),
        ));
    }
    commands.entity(body).insert(ViewModelBody {
        key: key.to_string(),
        mirrored: mirror,
        joints,
        materials,
        lit_at: None,
        cubemap: None,
    });
}

/// The screen quad drawn on a view model body (on the body: the quad).
#[derive(Component)]
pub struct ScreenQuad(Entity);

/// Draw `ViewModelScreen`s on the view model bodies showing them.
#[allow(clippy::type_complexity)]
pub fn view_model_screens(
    bodies: Query<(Entity, &ViewModelBody, &ChildOf, Option<&ScreenQuad>)>,
    cameras: Query<&ChildOf, With<ViewModelCamera>>,
    anchors: Query<(&ChildOf, Option<&ViewModelSource>), With<ViewModelAnchor>>,
    screens: Query<&ViewModelScreen>,
    models: Option<Res<ViewModels>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (body, view, camera, quad) in &bodies {
        let screen = cameras
            .get(camera.parent())
            .ok()
            .and_then(|c| anchors.get(c.parent()).ok())
            .and_then(|(parent, source)| screens.get(anchor_owner(parent, source)).ok())
            .filter(|s| s.key == view.key);
        match (screen, quad) {
            (None, Some(q)) => {
                commands.entity(q.0).try_despawn();
                commands.entity(body).remove::<ScreenQuad>();
            }
            (Some(screen), None) => {
                let Some((bone, at, size)) = models.as_ref().and_then(|m| m.get(&view.key)).and_then(screen_corners)
                else {
                    continue;
                };
                let Some(&joint) = view.joints.get(bone) else { continue };
                let mut mesh = Mesh::new(
                    bevy::mesh::PrimitiveTopology::TriangleList,
                    bevy::asset::RenderAssetUsages::default(),
                );
                // A hair off the model's surface (skeleton units), facing
                // either way.
                let z = 0.02;
                mesh.insert_attribute(
                    Mesh::ATTRIBUTE_POSITION,
                    vec![[0.0, 0.0, z], [size.x, 0.0, z], [size.x, size.y, z], [0.0, size.y, z]],
                );
                mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0.0f32, 0.0, 1.0]; 4]);
                // The picture's bottom-left at `_ll`, across along the
                // attachment's x and up its y; on a mirrored model flipped
                // across, so the text still reads.
                let uvs = if view.mirrored {
                    vec![[1.0f32, 1.0], [0.0, 1.0], [0.0, 0.0], [1.0, 0.0]]
                } else {
                    vec![[0.0f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]
                };
                mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
                mesh.insert_indices(bevy::mesh::Indices::U32(vec![0, 1, 2, 0, 2, 3]));
                let material = materials.add(StandardMaterial {
                    base_color_texture: Some(screen.image.clone()),
                    unlit: true,
                    alpha_mode: AlphaMode::Blend,
                    cull_mode: None,
                    double_sided: true,
                    ..default()
                });
                let q = commands
                    .spawn((
                        Name::new("View model screen"),
                        Mesh3d(meshes.add(mesh)),
                        MeshMaterial3d(material),
                        at,
                        RenderLayers::layer(VIEW_MODEL_LAYER),
                        NotShadowCaster,
                        NotShadowReceiver,
                        bevy::camera::visibility::NoFrustumCulling,
                        ChildOf(joint),
                    ))
                    .id();
                commands.entity(body).insert(ScreenQuad(q));
            }
            _ => {}
        }
    }
}

/// A flash sprite, removed after `life` seconds.
#[derive(Component)]
pub struct FlashSprite {
    life: f32,
    age: f32,
}

/// What muzzle flashes look like, ready to draw.
#[derive(Resource)]
pub struct FlashAssets {
    data: MapMuzzleFlash,
    mesh: Handle<Mesh>,
    material: Option<Handle<SpriteMaterial>>,
}

pub fn build_flash_assets(
    data: &MapData,
    textures: &[Handle<Image>],
    meshes: &mut Assets<Mesh>,
    sprites: Option<&mut Assets<SpriteMaterial>>,
) -> Option<FlashAssets> {
    let flash = data.kit.muzzle_flash.clone()?;
    let size = flash
        .texture
        .and_then(|t| data.textures.get(t))
        .map_or(UVec2::ONE, |t| UVec2::new(t.width, t.height));
    let [r, g, b] = flash.color;
    let material = sprites
        .zip(flash.texture.and_then(|t| textures.get(t)))
        .map(|(sprites, t)| {
            sprites.add(SpriteMaterial {
                params: SpriteParams {
                    color: Vec4::new(r, g, b, 1.0),
                    size: Vec2::ONE,
                },
                texture: Some(t.clone()),
                glow: false,
                blend: crate::render::sprite_material::SpriteBlend::Add,
            })
        });
    Some(FlashAssets {
        mesh: meshes.add(crate::render::sprite_material::sprite_mesh(size)),
        material,
        data: flash,
    })
}

/// A small deterministic random source for effect variation.
#[derive(Resource, Default)]
pub struct FlashDice(u32);

impl FlashDice {
    fn roll(&mut self) -> f32 {
        let mut x = self.0.wrapping_add(0x9e37_79b9).max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn range(&mut self, (lo, hi): (f32, f32)) -> f32 {
        lo + (hi - lo) * self.roll()
    }
}

/// Show muzzle flashes: at the view model's (re-projected) attachment for
/// the first-person view (spec view_models.md 5, "Attachment positions for
/// effects"), else at the held weapon's muzzle; a sprite plume and a
/// point light.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn muzzle_flashes(
    mut events: MessageReader<ViewModelEvent>,
    flash: Option<Res<FlashAssets>>,
    models: Option<Res<ViewModels>>,
    time: Res<Time>,
    anchors: Query<
        (
            &ChildOf,
            &GlobalTransform,
            &Projection,
            &Children,
            Option<&ViewModelSource>,
        ),
        With<ViewModelAnchor>,
    >,
    vm_cameras: Query<&Projection, (With<ViewModelCamera>, Without<ViewModelAnchor>)>,
    bodies: Query<&ViewModelBody>,
    owners: Query<(&ViewAnimator, Option<&ViewModelOffset>)>,
    cam_children: Query<&Children, With<ViewModelCamera>>,
    muzzles: Query<(&HeldMuzzle, &GlobalTransform)>,
    characters: Query<Entity, With<crate::core::Intent>>,
    spatial: SpatialQuery,
    mut pool: LightPool,
    mut dice: Local<FlashDice>,
    effects: Option<Res<EffectSettings>>,
    mut commands: Commands,
) {
    let Some(flash) = flash else {
        events.clear();
        return;
    };
    // Lights ready before the first shot (see `spawn_flash_light`).
    if pool.is_empty() && flash.data.light.is_some() {
        for _ in 0..4 {
            commands.spawn((
                Name::new("Muzzle flash light"),
                PointLight {
                    color: Color::BLACK,
                    intensity: 4.0 * std::f32::consts::PI,
                    range: 1e-4,
                    radius: 0.0,
                    shadow_maps_enabled: false,
                    ..default()
                },
                DynamicLight {
                    radius: 0.0,
                    life: 0.0,
                    age: 0.0,
                },
                RenderLayers::from_layers(&[0, VIEW_MODEL_LAYER]),
                Transform::default(),
            ));
        }
    }
    let effects = effects.map(|e| *e).unwrap_or_default();
    let now = time.elapsed_secs_f64();
    for ev in events.read() {
        let ViewModelEventKind::MuzzleFlash { attachment } = ev.kind else {
            continue;
        };
        // First person: the owner's anchor has its view model drawn.
        let first_person = anchors.iter().find_map(|(parent, eye, projection, children, source)| {
            if anchor_owner(parent, source) != ev.owner {
                return None;
            }
            let vm_camera = children.iter().find(|c| vm_cameras.contains(*c))?;
            let body = cam_children
                .get(vm_camera)
                .ok()?
                .iter()
                .find_map(|c| bodies.get(c).ok())?;
            let (view, offset) = owners.get(ev.owner).ok()?;
            let model = models.as_ref()?.get(&body.key)?;
            let animator = view.animator.as_ref()?;
            let pose = animator.pose(now);
            let placed = placement(&offset.copied().unwrap_or_default(), body.mirrored);
            let m = attachment_transform(model, &pose, attachment, &placed)?;
            let (Projection::Perspective(world), Ok(Projection::Perspective(vm))) =
                (projection, vm_cameras.get(vm_camera))
            else {
                return None;
            };
            let (wf, vf) = (world.fov.to_degrees(), vm.fov.to_degrees());
            let muzzle = m.transform_point3(Vec3::ZERO);
            let at = reproject(muzzle, wf, vf);
            // The plume goes along the model's forward (the skeleton's +X),
            // not the attachment's own axes (on the AK they point sideways).
            let forward = (placed.to_matrix() * model.root.to_matrix()).transform_vector3(Vec3::X);
            let ahead = reproject(muzzle + forward, wf, vf);
            Some((
                eye.transform_point(at),
                (eye.transform_point(ahead) - eye.transform_point(at)).normalize_or_zero(),
            ))
        });
        let world_model = || {
            muzzles
                .iter()
                .find(|(m, _)| m.owner == ev.owner)
                .map(|(_, g)| (g.translation(), g.rotation() * Vec3::X))
        };
        let Some((at, forward)) = first_person.or_else(world_model) else {
            continue;
        };
        // Tracers start here (map::tracer).
        let owner = ev.owner;
        commands.queue(move |w: &mut World| {
            w.resource_mut::<crate::map::tracer::MuzzleCache>()
                .0
                .insert(owner, (at, now));
        });
        let data = &flash.data;
        if let Some(material) = &flash.material {
            let scale = dice.range(data.scale);
            for (ahead, size) in &data.sprites {
                commands.spawn((
                    Name::new("Muzzle flash"),
                    Mesh3d(flash.mesh.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_translation(at + forward * *ahead * scale),
                    FlashSprite {
                        life: data.life,
                        age: 0.0,
                    },
                    FlashSize(*size * scale),
                    NotShadowCaster,
                    bevy::camera::visibility::NoFrustumCulling,
                ));
            }
        }
        if let Some(light) = data.light.filter(|_| effects.muzzle_light != 0) {
            spawn_flash_light(
                &mut commands,
                &mut pool,
                &spatial,
                &characters,
                at,
                forward,
                light,
                &mut dice,
            );
        }
    }
}

/// A flash sprite's full size (meters); its material is shared, so the
/// size is applied per entity by `size_flash_sprites`.
#[derive(Component, Clone, Copy)]
pub struct FlashSize(f32);

/// Most muzzle-flash lights at once; more flashes reuse the oldest.
const MAX_FLASH_LIGHTS: usize = 8;

type LightPool<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static mut DynamicLight,
        &'static mut PointLight,
        &'static mut Transform,
    ),
>;

#[allow(clippy::too_many_arguments)]
fn spawn_flash_light(
    commands: &mut Commands,
    pool: &mut LightPool,
    spatial: &SpatialQuery,
    characters: &Query<Entity, With<crate::core::Intent>>,
    at: Vec3,
    forward: Vec3,
    light: MapFlashLight,
    dice: &mut FlashDice,
) {
    // Keep the light off a wall the muzzle touches or pokes into: look
    // along the barrel from behind and stop `CLEARANCE` short of what is
    // hit, so the wall is lit from the front rather than grazed.
    const BACK: f32 = 0.6;
    const CLEARANCE: f32 = 0.2;
    let from = at - forward * BACK;
    let mut position = at;
    if let Ok(dir) = Dir3::new(forward) {
        let filter = SpatialQueryFilter::from_excluded_entities(characters.iter()).with_mask(crate::core::NOT_SHADOW);
        if let Some(hit) = spatial.cast_ray(from, dir, BACK + CLEARANCE, true, &filter) {
            position = from + forward * (hit.distance - CLEARANCE).clamp(0.0, BACK);
        }
    }
    let radius = dice.range(light.radius);
    let color = Color::LinearRgba(LinearRgba::rgb(light.color.x, light.color.y, light.color.z));
    let state = DynamicLight {
        radius,
        life: light.life,
        age: 0.0,
    };
    // Reuse a light that has gone out (or the oldest): new light entities
    // only reach the light clusters a frame or two after they appear,
    // longer than a flash lasts.
    let count = pool.iter().count();
    let reuse = pool
        .iter()
        .max_by(|a, b| (a.1.age / a.1.life.max(1e-6)).total_cmp(&(b.1.age / b.1.life.max(1e-6))))
        .filter(|(_, d, _, _)| d.age >= d.life || count >= MAX_FLASH_LIGHTS)
        .map(|(e, ..)| e);
    if let Some((_, mut d, mut point, mut t)) = reuse.and_then(|e| pool.get_mut(e).ok()) {
        *d = state;
        point.color = color;
        point.range = radius;
        t.translation = position;
        return;
    }
    commands.spawn((
        Name::new("Muzzle flash light"),
        PointLight {
            // The shaders read colour x intensity / (4 pi) as lightmap
            // units (see `dynamic_light` in world.wesl).
            color,
            intensity: 4.0 * std::f32::consts::PI,
            range: radius,
            radius: 0.0,
            shadow_maps_enabled: false,
            ..default()
        },
        state,
        RenderLayers::from_layers(&[0, VIEW_MODEL_LAYER]),
        Transform::from_translation(position),
    ));
}

/// Shrink dynamic lights' radius to 0 over their life, then turn them off
/// (kept for reuse); remove flash sprites after their life.
pub fn age_effects(
    time: Res<Time>,
    mut lights: Query<(&mut DynamicLight, &mut PointLight)>,
    mut sprites: Query<(Entity, &mut FlashSprite)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (mut d, mut light) in &mut lights {
        if d.age >= d.life {
            continue;
        }
        d.age += dt;
        if d.age >= d.life {
            light.color = Color::BLACK;
            light.range = 1e-4;
        } else {
            light.range = (d.radius * (1.0 - d.age / d.life)).max(1e-4);
        }
    }
    for (e, mut s) in &mut sprites {
        // Shown for at least one frame.
        if s.age >= s.life {
            commands.entity(e).try_despawn();
        }
        s.age += dt;
    }
}

/// Flash sprites share one material; give each its own size by a material
/// per size bucket.
pub fn size_flash_sprites(
    flash: Option<Res<FlashAssets>>,
    mut sprites: Query<(&FlashSize, &mut MeshMaterial3d<SpriteMaterial>), Added<FlashSize>>,
    mut materials: Option<ResMut<Assets<SpriteMaterial>>>,
    mut cache: Local<HashMap<u32, Handle<SpriteMaterial>>>,
) {
    let (Some(flash), Some(materials)) = (flash, materials.as_mut()) else {
        return;
    };
    let Some(base) = flash.material.as_ref().and_then(|m| materials.get(m)).cloned() else {
        return;
    };
    for (size, mut material) in &mut sprites {
        // Millimetre buckets.
        let key = (size.0 * 1000.0).round() as u32;
        let handle = cache
            .entry(key)
            .or_insert_with(|| {
                let mut m = base.clone();
                m.params.size = Vec2::splat(key as f32 / 1000.0);
                materials.add(m)
            })
            .clone();
        material.0 = handle;
    }
}

/// Remove drawn view models (map unload); the cameras stay.
pub fn unload(world: &mut World) {
    world.remove_resource::<ViewModels>();
    world.remove_resource::<LateViewModelTextures>();
    world.remove_resource::<ViewModelAssets>();
    world.remove_resource::<FlashAssets>();
    world.remove_resource::<LightField>();
    let bodies: Vec<Entity> = world
        .query_filtered::<Entity, Or<(With<ViewModelBody>, With<FlashSprite>, With<DynamicLight>)>>()
        .iter(world)
        .collect();
    for b in bodies {
        world.entity_mut(b).despawn();
    }
    let states: Vec<Entity> = world
        .query_filtered::<Entity, With<ViewAnimator>>()
        .iter(world)
        .collect();
    for e in states {
        world.entity_mut(e).insert(ViewAnimator::default());
    }
}
