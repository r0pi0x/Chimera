//! Drawn character bodies: each character's body (its model skinned to a
//! skeleton of joints at its feet), what it holds in the body's hand, and
//! a dead character's body handed over to its ragdoll. What is drawn is
//! the simulation's own state (`map::bodies`: the character's team,
//! `BodyName` and class pick the model, `BodySkin` its skin family, `BodyNumber` its body parts,
//! `Held` the held model; a
//! `map::ragdoll::Ragdoll` takes the body over); these systems fulfil it,
//! and a server without rendering leaves it undrawn.
//!
//! Character bodies (by team or `BodyName`; the local one hidden unless
//! `ShowLocalBody`), held models with their muzzles and bone-merged parts,
//! posed from the character's `Animator`, culled by their posed bounds
//! (`DynamicSkinnedMeshBounds`, not the bind pose's), lit where they are
//! (`probe_lit`); `PlayModels`/`HeldAssets` are built when the map
//! spawns (`render::spawn`), held models a game adds later
//! (`map::add_held_model`) on a task and over the next frames, and again on
//! each map (`build_late_held`, `render::late_build`), and character models a
//! game adds later (`map::add_character_model`, appended to each map's
//! `CharacterModels`) likewise (`build_late_bodies`); face parts (`map::face`: eyeballs with
//! their iris projected toward the character's `EyeTarget`, teeth) set
//! each frame from where the pose puts the head (`aim_faces`), face meshes
//! with flexes drawn from the body's own copy, moved by the character's
//! `map::FaceWeights` (`apply_face_morphs`);
//! `adopt_bodies`/`pose_ragdolls` hand a dead
//! character's body to its ragdoll and pose it from the ragdoll's bodies.
//! A skeleton never drives another's model: an animator says which skeleton
//! its pose is for (`Animator::skeleton`), `pose_bodies` poses only a body
//! on it, and a character changing bodies keeps the one showing until the
//! new one is built and its pose is on hand (`attach_bodies`), then changes
//! both in one frame.
//!
//! Known shortcuts (docs/tech-debt.md): Frame-time shortcuts
//! (docs/performance.md, "Frame time pass"): bodies no camera drew last frame
//! are posed every 4th frame (`render::bodies::UNSEEN_POSE_INTERVAL`), so an
//! unseen shooter's muzzle (where its tracer starts) can be up to three frames
//! behind its animation; dust motes' and particles' meshes are padded to
//! power-of-two sizes with degenerate triangles; a physics prop's shadow is
//! redrawn only after it moves by a quarter of a shadow texel (a creeping
//! prop's shadow lags it by up to that); the per-frame schedules run on
//! single-threaded executors (`CHIMERA_EXECUTOR=multi`: Bevy's). Why it's OK for
//! now: Measured faster on the dev box; hitboxes pose every tick on their own;
//! the Windows PC is still to be measured.

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{
    build_mesh,
    late_build::{AddAsset, LateQueue, LateStores, Providers, Stores},
    probe_lit,
    prop_material::PropMaterial,
};
#[allow(unused_imports)]
use crate::map::*;
use crate::{
    core::Team,
    map::ragdoll::{Ragdoll, RagdollBody},
};

/// The models parts made in play are drawn with: each character model's
/// meshes and materials (index as in `CharacterModels`), and each gib
/// list's (`render::breakables`). One resource, inserted where the
/// character bodies' always was (resources are entities: their order
/// decides ids).
#[derive(Resource)]
pub struct PlayModels {
    pub bodies: Vec<BodyAssets>,
    pub gibs: super::breakables::GibDraws,
}

/// Meshes and materials of each held model, by key.
#[derive(Resource)]
#[allow(clippy::type_complexity)]
pub struct HeldAssets(pub HashMap<String, HeldParts>);

/// A held model's meshes on its bone, its muzzle there, and the parts
/// that follow other bones (`MapHeldModel::merged`).
#[allow(clippy::type_complexity)]
pub struct HeldParts {
    pub bone: String,
    /// The whole model on `bone` (loose items).
    pub whole: Vec<(Handle<Mesh>, Handle<PropMaterial>)>,
    /// What stays on `bone` when held.
    pub own: Vec<(Handle<Mesh>, Handle<PropMaterial>)>,
    pub muzzle: Option<Transform>,
    /// Its attachments on `bone` (`MapHeldModel::attachments`).
    pub attachments: Vec<(String, Transform)>,
    pub merged: Vec<(String, Vec<(Handle<Mesh>, Handle<PropMaterial>)>)>,
}

/// What held models added after the map spawned (`map::LateHeldModels`)
/// are built with: the spawned map's fog and look, whether it lights
/// models from a light field, its debug view, and its first character
/// model's root (their loose form's frame: `map::loose::asset`).
#[derive(Resource)]
pub struct HeldLateBase {
    pub data: MapData,
    pub lit: bool,
    pub view: MapDebugView,
    pub root: Option<Transform>,
    /// This spawn (`late_build::next_generation`): what was being built for
    /// an earlier map is dropped.
    pub generation: u64,
}

impl HeldLateBase {
    /// What a task needs of it.
    fn for_task(&self) -> LateBase {
        LateBase {
            data: MapData {
                fog: self.data.fog.clone(),
                look: self.data.look.clone(),
                ..default()
            },
            lit: self.lit,
            view: self.view,
            root: self.root,
        }
    }
}

/// `HeldLateBase` as a task owns it.
struct LateBase {
    data: MapData,
    lit: bool,
    view: MapDebugView,
    root: Option<Transform>,
}

impl LateBase {
    /// A late model's material for mesh `m`.
    fn material(&self, m: &MapMesh, textures: &[Handle<Image>]) -> PropMaterial {
        let mut material = super::lit_prop_material(m, textures, &self.data, self.view, false);
        if !m.unlit && self.view != MapDebugView::Albedo && self.lit {
            material.params.set_probe(&crate::map::LightProbe::default(), 1.0);
        }
        material
    }
}

/// Mesh `m` as the GPU's alone: nothing in the main world reads a model's
/// vertices once it is drawn (its bounds stay with the mesh), and a mesh the main world
/// keeps is copied whole there to go to the render world.
fn gpu_only(mut mesh: Mesh) -> Mesh {
    mesh.asset_usage = bevy::asset::RenderAssetUsages::RENDER_WORLD;
    mesh
}

/// Build the held models added after the map spawned
/// (`map::add_held_model`) that the map's `HeldAssets` lack (none at
/// first after each map spawn): their own textures as images, their
/// meshes and materials as the spawn builds the kit's, and their loose
/// (dropped) form, on a task and over the next frames (`late_build`): a
/// weapon whose model is not ready waits with the hand it had.
#[allow(clippy::too_many_arguments)]
pub fn build_late_held(
    late: Option<Res<LateHeldModels>>,
    base: Option<Res<HeldLateBase>>,
    held: Option<ResMut<HeldAssets>>,
    loose: Option<ResMut<crate::map::loose::LooseAssets>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    mut bindposes: ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    compressed: Option<Res<bevy::image::CompressedImageFormatSupport>>,
    mut queue: Local<LateQueue<(HeldParts, Option<crate::map::loose::LooseAsset>)>>,
) {
    let (Some(late), Some(base), Some(mut held), Some(mut materials)) = (late, base, held, materials) else {
        return;
    };
    let mut loose = loose;
    queue.for_generation(base.generation);
    let mut to = LateStores {
        images: &mut images,
        stores: Stores {
            meshes: &mut meshes,
            materials: &mut materials,
            bindposes: &mut bindposes,
        },
    };
    let bc = super::images::bc_wanted(compressed.as_deref());
    for (h, own) in late.0.iter().filter(|(h, _)| !held.0.contains_key(&h.key)) {
        if queue.has(&h.key) {
            continue;
        }
        let (h, own, base) = (h.clone(), own.clone(), base.for_task());
        queue.start(h.key.clone(), Providers::of(&to), move |stage| {
            let textures: Vec<Handle<Image>> =
                own.iter().map(|t| stage.add_asset(super::images::to_image(t, &base.data.look, bc))).collect();
            let mut build = |model: &MapModel| -> Vec<(Handle<Mesh>, Handle<PropMaterial>)> {
                model
                    .meshes
                    .iter()
                    .map(|m| {
                        let mesh = stage.add_asset(gpu_only(build_mesh(m, false)));
                        (mesh, stage.add_asset(base.material(m, &textures)))
                    })
                    .collect()
            };
            let whole = build(&h.model);
            let own_parts = h.own.as_ref().map_or_else(|| whole.clone(), &mut build);
            let merged = h.merged.iter().map(|(b, m)| (b.clone(), build(m))).collect();
            let parts = HeldParts {
                bone: h.bone.clone(),
                whole,
                own: own_parts,
                muzzle: h.muzzle,
                attachments: h.attachments.clone(),
                merged,
            };
            (parts, base.root.map(|root| crate::map::loose::asset(&h, root)))
        });
    }
    if let Some((key, (parts, shape))) = queue.step(&mut to) {
        if let (Some(loose), Some(shape)) = (loose.as_mut(), shape)
            && !loose.0.contains_key(&key)
        {
            loose.0.insert(key.clone(), shape);
        }
        held.0.insert(key, parts);
    }
}

/// Build the character models appended after the map spawned
/// (`map::LateCharacterModels`, `map::append_late_characters`) that the
/// map's `PlayModels` lack (by index: they come after the map's own), as
/// the spawn builds the kit's, with their own textures, on a task and over
/// the next frames (`late_build`): until then the character wears what it
/// wore (`attach_bodies`).
#[allow(clippy::too_many_arguments)]
pub fn build_late_bodies(
    late: Option<Res<crate::map::LateCharacterModels>>,
    models: Option<Res<CharacterModels>>,
    base: Option<Res<HeldLateBase>>,
    play: Option<ResMut<PlayModels>>,
    mut images: ResMut<Assets<Image>>,
    mut meshes: ResMut<Assets<Mesh>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
    mut bindposes: ResMut<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>,
    compressed: Option<Res<bevy::image::CompressedImageFormatSupport>>,
    mut queue: Local<LateQueue<BodyAssets>>,
) {
    let (Some(late), Some(models), Some(base), Some(mut play), Some(mut materials)) = (late, models, base, play, materials)
    else {
        return;
    };
    queue.for_generation(base.generation);
    if play.bodies.len() >= models.0.len() {
        return;
    }
    let mut to = LateStores {
        images: &mut images,
        stores: Stores {
            meshes: &mut meshes,
            materials: &mut materials,
            bindposes: &mut bindposes,
        },
    };
    let bc = super::images::bc_wanted(compressed.as_deref());
    // The next ones not on their way (they come out in this order).
    for c in models.0.iter().skip(play.bodies.len() + queue.len()) {
        // An appended model is a late one; anything else (the map's own:
        // none past the built ones) leaves the rest unbuilt.
        let Some((c, own)) = c.name.as_deref().and_then(|n| late.get(n)) else { break };
        let (c, own, base) = (c.clone(), own.clone(), base.for_task());
        queue.start(c.name.clone().unwrap_or_default(), Providers::of(&to), move |stage| {
            let textures: Vec<Handle<Image>> =
                own.iter().map(|t| stage.add_asset(super::images::to_image(t, &base.data.look, bc))).collect();
            body_assets_into(&c.model, &c.bones, c.root, stage, &|m| base.material(m, &textures))
        });
    }
    if let Some((_, assets)) = queue.step(&mut to) {
        play.bodies.push(assets);
    }
}

pub struct BodyAssets {
    pub parts: Vec<(Handle<Mesh>, Handle<PropMaterial>)>,
    /// Each skin family's material per part (same order as `parts`), when
    /// the model has more than one (`MapModel::skins`).
    pub skins: Vec<Vec<Handle<PropMaterial>>>,
    pub bindposes: Handle<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
    pub bones: Vec<MapBone>,
    /// `bones`' shape (`anim::skeleton_signature`).
    pub skeleton: u64,
    pub root: Transform,
    /// Each part's meshes at every level of detail (LOD 0 first; the
    /// same joints), and the body's `LodGroup`, when the model has levels.
    pub levels: Vec<Vec<Handle<Mesh>>>,
    pub lod_group: Option<super::lod::LodGroup>,
    /// Choices per body part and each part's (part, choice), when the
    /// model has body parts (`MapModel::body_parts`, `MapMesh::body`).
    pub body_parts: Vec<u16>,
    pub mesh_bodies: Vec<Option<(u16, u16)>>,
    /// Each part's face (an eyeball or teeth: `map::face`), drawn where its
    /// character looks (`aim_faces`).
    pub faces: Vec<Option<Face>>,
    /// Each part's facial flexes (`MapMesh::morphs`; empty: none): its
    /// own copy of the mesh follows its character's `FaceWeights`.
    pub morphs: Vec<std::sync::Arc<Vec<MapMorph>>>,
}

/// A body's own copy of a face mesh with flexes: its rest positions, its
/// flexes and the weights it shows.
#[derive(Component, Clone, Debug)]
pub struct FaceMorph {
    pub rest: std::sync::Arc<Vec<[f32; 3]>>,
    pub morphs: std::sync::Arc<Vec<MapMorph>>,
    pub shown: Vec<[f32; 2]>,
}

/// A face part of a body's model.
#[derive(Clone, Debug, PartialEq)]
pub enum Face {
    Eye(MapEye),
    Teeth(MapTeeth),
}

/// A body's mesh drawn as a face part, on its bone's joint.
#[derive(Component, Clone, Debug)]
pub struct FacePart {
    pub face: Face,
    pub joint: Entity,
}

impl BodyAssets {
    /// Whether mesh `k` shows with body number `body` (`BodyNumber`).
    pub fn shows(&self, k: usize, body: i32) -> bool {
        match self.mesh_bodies.get(k).copied().flatten() {
            Some((part, choice)) => body_choice(&self.body_parts, body, part as usize) == choice,
            None => true,
        }
    }
}

/// A joint of a character body's skeleton: its bone index.
#[derive(Component)]
pub struct BodyJoint(pub usize);

/// A character's drawn body: which model, at its feet.
#[derive(Component)]
pub struct CharacterBody {
    pub model: usize,
    /// Its skeleton (`anim::skeleton_signature`): only an animator of it
    /// poses it (`pose_bodies`).
    pub skeleton: u64,
    /// The skin family shown (`BodySkin`).
    pub skin: i32,
    /// The body number shown (`BodyNumber`).
    pub body: i32,
    /// Joint entities by bone index.
    pub joints: Vec<Entity>,
    /// The held model shown and its key.
    pub held: Option<(Entity, String)>,
    /// Its bone-merged parts on other joints (`MapHeldModel::merged`).
    pub held_parts: Vec<Entity>,
}

/// How long a character waits for its animator to have a pose for its
/// new body (s; a game driving it does in a tick or two): after that the
/// body comes in its reference pose.
pub const BODY_POSE_WAIT: f64 = 0.25;

/// Give characters their team's body (a child at the feet); the local
/// player's is hidden unless `ShowLocalBody`.
///
/// A new body replaces the one showing only once it is ready: its model
/// built (`build_late_bodies`) and its animator on its skeleton with
/// something playing (`Animator::poses`, or `BODY_POSE_WAIT` up); it comes
/// in that pose, so model and skeleton change together, in one frame.
/// With `game_parts::NoStandIns` a character whose game has no body here
/// keeps what it wears, or wears any body.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn attach_bodies(
    models: Option<Res<CharacterModels>>,
    bodies: Option<Res<PlayModels>>,
    characters: Query<
        (
            Entity,
            (
                Option<&Team>,
                Option<&BodyName>,
                Option<&crate::core::PlayerClass>,
                Option<&BodySkin>,
                Option<&BodyNumber>,
            ),
            Option<&ColliderAabb>,
            &GlobalTransform,
            Option<&Children>,
            (
                Has<crate::core::LocalPlayer>,
                Option<&Corpse>,
                Option<&chimera_core::slots::GameIdentity>,
                Option<&anim::Animator>,
            ),
        ),
        (Or<(With<crate::core::Intent>, With<Corpse>)>, Without<ragdoll::Ragdolled>),
    >,
    existing: Query<&CharacterBody>,
    show_local: Res<ShowLocalBody>,
    (time, no_stand_ins): (Res<Time>, Option<Res<chimera_core::game_parts::NoStandIns>>),
    mut materials: Option<ResMut<Assets<PropMaterial>>>,
    mut meshes: Option<ResMut<Assets<Mesh>>>,
    mut waiting: Local<HashMap<Entity, f64>>,
    mut commands: Commands,
) {
    let (Some(models), Some(bodies)) = (models, bodies) else {
        return;
    };
    let now = time.elapsed_secs_f64();
    waiting.retain(|e, _| characters.contains(*e));
    for (e, (team, name, class, skin, number), aabb, at, children, (local, corpse, identity, animator)) in &characters {
        let current = children
            .into_iter()
            .flatten()
            .find_map(|c| existing.get(*c).ok().map(|b| (*c, (b.model, b.skin, b.body))));
        let index = match models.for_identity(identity, team.copied(), name, class) {
            Some((index, _)) => index,
            None => {
                // An app without stand-ins: what it wears stays; if nothing,
                // any body here rather than none (no capsule comes).
                let any = no_stand_ins.is_some().then(|| current.map(|(_, m)| m.0)).flatten();
                let any = any.or_else(|| no_stand_ins.is_some().then(|| models.for_identity(None, team.copied(), None, class)).flatten().map(|m| m.0));
                match any {
                    Some(index) => index,
                    None => {
                        // Its identity's game has no body here: the other game's goes.
                        if let Some((old, _)) = current {
                            commands.entity(old).despawn();
                        }
                        continue;
                    }
                }
            }
        };
        let skin = skin.map_or(0, |s| s.0);
        let number = number.map_or(0, |n| n.0);
        if current.is_some_and(|(_, m)| m == (index, skin, number)) {
            waiting.remove(&e);
            continue;
        }
        let feet = match (corpse, aabb) {
            (Some(c), _) => c.feet,
            (None, Some(aabb)) => aabb.min.y - at.translation().y,
            (None, None) => continue,
        };
        // A model appended since (`build_late_bodies`) not built yet: the
        // one showing stays.
        let Some(assets) = bodies.bodies.get(index) else { continue };
        // The same skeleton's pose is on hand; another's isn't until the
        // animator is on it: until then the one showing stays.
        let posed = animator.filter(|a| a.poses(assets.skeleton));
        if animator.is_some() && posed.is_none() && now - *waiting.entry(e).or_insert(now) < BODY_POSE_WAIT {
            continue;
        }
        waiting.remove(&e);
        if let Some((old, _)) = current {
            commands.entity(old).despawn();
        }
        let pose = posed.map(|a| a.pose(now)).unwrap_or_default();
        let body = commands
            .spawn((
                Name::new("Body"),
                Transform::from_xyz(0.0, feet, 0.0),
                body_visibility(local, *show_local),
                ChildOf(e),
            ))
            .id();
        // The skeleton: a root in the source game's axes and units, then
        // each bone under its parent, in its first pose (the reference
        // pose without one).
        let root = commands.spawn((assets.root, Visibility::Inherited, ChildOf(body))).id();
        let mut joints: Vec<Entity> = Vec::with_capacity(assets.bones.len());
        for (i, b) in assets.bones.iter().enumerate() {
            let parent = b.parent.and_then(|p| joints.get(p).copied()).unwrap_or(root);
            let placed = pose.get(i).map_or_else(
                || Transform::from_translation(b.position).with_rotation(b.rotation),
                |(q, p)| Transform::from_translation(*p).with_rotation(*q),
            );
            joints.push(
                commands
                    .spawn((BodyJoint(i), placed, Visibility::Inherited, ChildOf(parent)))
                    .id(),
            );
        }
        commands.entity(body).insert(CharacterBody {
            model: index,
            skeleton: assets.skeleton,
            skin,
            body: number,
            joints: joints.clone(),
            held: None,
            held_parts: Vec::new(),
        });
        // Its own materials, lit where it stands (probe_lit), at its centre.
        // A skin family the model doesn't have shows its first.
        let family = usize::try_from(skin).ok().and_then(|s| assets.skins.get(s));
        let mut own = Vec::new();
        for (k, (mesh, material)) in assets.parts.iter().enumerate() {
            if !assets.shows(k, number) {
                continue;
            }
            let material = family.and_then(|f| f.get(k)).unwrap_or(material);
            let material = match materials.as_mut() {
                Some(m) => probe_lit::instance(m, material),
                None => material.clone(),
            };
            own.push(material.clone());
            // A face mesh with flexes: the body's own copy.
            let flexes = assets.morphs.get(k).filter(|m| !m.is_empty()).cloned();
            let mut morph = None;
            let mesh = match (&flexes, meshes.as_mut()) {
                (Some(f), Some(meshes)) => match meshes.get(mesh).cloned() {
                    Some(copy) => {
                        let rest = match copy.attribute(Mesh::ATTRIBUTE_POSITION) {
                            Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) => p.clone(),
                            _ => Vec::new(),
                        };
                        morph = Some(FaceMorph { rest: std::sync::Arc::new(rest), morphs: f.clone(), shown: Vec::new() });
                        meshes.add(copy)
                    }
                    None => mesh.clone(),
                },
                _ => mesh.clone(),
            };
            let mut part = commands.spawn((
                Mesh3d(mesh),
                MeshMaterial3d(material),
                bevy::mesh::skinning::SkinnedMesh {
                    inverse_bindposes: assets.bindposes.clone(),
                    joints: joints.clone(),
                },
                // Culled by where the pose puts it: a bind pose far from
                // the drawn one (a body bound standing on its origin and
                // drawn half a body lower) culled parts still on screen.
                bevy::camera::visibility::DynamicSkinnedMeshBounds,
                ChildOf(body),
            ));
            match morph {
                // Its own copy stays at its level of detail.
                Some(m) => {
                    part.insert(m);
                }
                None => {
                    if let (Some(_), Some(levels)) = (&assets.lod_group, assets.levels.get(k)) {
                        part.insert(super::lod::LodMeshes(levels.clone()));
                    }
                }
            }
            if let Some(face) = assets.faces.get(k).cloned().flatten() {
                let bone = match &face {
                    Face::Eye(e) => e.bone,
                    Face::Teeth(t) => t.bone,
                };
                if let Some(&joint) = joints.get(bone) {
                    part.insert(FacePart { face, joint });
                }
            }
        }
        if let Some(group) = &assets.lod_group {
            commands.entity(body).insert(group.clone());
        }
        commands
            .entity(body)
            .insert(probe_lit::ProbeLit::new(own, Vec3::Y * BODY_LIGHT_HEIGHT));
    }
}

/// Height of a body's lighting point above its feet, meters (about the
/// models' own illumination position, their middle).
pub const BODY_LIGHT_HEIGHT: f32 = 0.9;

pub fn body_visibility(local: bool, show_local: ShowLocalBody) -> Visibility {
    if local && !show_local.0 {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    }
}

/// Show or hide the local player's body when `ShowLocalBody` changes.
pub fn show_local_body(
    show: Res<ShowLocalBody>,
    local: Query<&Children, With<crate::core::LocalPlayer>>,
    mut bodies: Query<&mut Visibility, With<CharacterBody>>,
) {
    let want = body_visibility(true, *show);
    for children in &local {
        let mut it = bodies.iter_many_mut(children).matched();
        while let Some(mut vis) = it.fetch_next() {
            if *vis != want {
                *vis = want;
            }
        }
    }
}

/// Turn bodies with their character's yaw (as drawn: `interp`).
pub fn turn_bodies(
    characters: Query<(
        &crate::core::Intent,
        &Children,
        Option<&anim::Animator>,
        Option<&interp::RenderedView>,
    )>,
    mut bodies: Query<&mut Transform, With<CharacterBody>>,
) {
    for (intent, children, animator, view) in &characters {
        let yaw = animator
            .and_then(|a| a.yaw)
            .unwrap_or(view.map_or(intent.yaw, |v| v.now.yaw));
        for c in children {
            let turned = Quat::from_rotation_y(yaw);
            if let Ok(mut t) = bodies.get_mut(*c)
                && t.rotation != turned
            {
                t.rotation = turned;
            }
        }
    }
}

/// Draw what each character holds in its body's hand.
pub fn attach_held(
    held: Option<Res<HeldAssets>>,
    bodies: Option<Res<PlayModels>>,
    characters: Query<(Entity, &Held, &Children)>,
    mut body_query: Query<&mut CharacterBody>,
    mut materials: Option<ResMut<Assets<PropMaterial>>>,
    mut commands: Commands,
) {
    let (Some(held), Some(bodies)) = (held, bodies) else {
        return;
    };
    for (character, want, children) in &characters {
        let Some(c) = children.iter().find(|c| body_query.contains(*c)) else {
            continue;
        };
        let mut body = body_query.get_mut(c).unwrap();
        if body.held.as_ref().map(|(_, k)| k.as_str()) == want.0.as_deref() {
            continue;
        }
        if let Some((e, _)) = body.held.take() {
            commands.entity(e).despawn();
        }
        for e in std::mem::take(&mut body.held_parts) {
            commands.entity(e).despawn();
        }
        let Some(key) = &want.0 else { continue };
        let Some(parts) = held.0.get(key) else {
            continue;
        };
        let joint_of = |bone: &str| {
            bodies.bodies[body.model]
                .bones
                .iter()
                .position(|b| b.name.eq_ignore_ascii_case(bone))
                .and_then(|i| body.joints.get(i).copied())
        };
        let Some(joint) = joint_of(&parts.bone) else {
            continue;
        };
        // Its own materials, lit where it is (probe_lit).
        let mut instance = |list: &[(Handle<Mesh>, Handle<PropMaterial>)]| -> Vec<Handle<PropMaterial>> {
            list.iter()
                .map(|(_, m)| match materials.as_mut() {
                    Some(assets) => probe_lit::instance(assets, m),
                    None => m.clone(),
                })
                .collect()
        };
        let own = instance(&parts.own);
        let model = commands
            .spawn((
                Transform::default(),
                Visibility::Inherited,
                ChildOf(joint),
                probe_lit::ProbeLit::new(own.clone(), Vec3::ZERO),
            ))
            .with_children(|m| {
                for ((mesh, _), material) in parts.own.iter().zip(own) {
                    m.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material)));
                }
                if let Some(t) = parts.muzzle {
                    m.spawn((t, view_model::HeldMuzzle { owner: character }));
                }
                for (index, (name, t)) in parts.attachments.iter().enumerate() {
                    m.spawn((
                        *t,
                        view_model::HeldAttachment { owner: character, index, name: name.clone() },
                    ));
                }
            })
            .id();
        // Bone-merged parts on their own joints, despawned with the rest.
        let mut merged = Vec::new();
        for (bone, list) in &parts.merged {
            let Some(at) = joint_of(bone) else { continue };
            let lit = instance(list);
            let part = commands
                .spawn((
                    Transform::default(),
                    Visibility::Inherited,
                    ChildOf(at),
                    probe_lit::ProbeLit::new(lit.clone(), Vec3::ZERO),
                ))
                .with_children(|m| {
                    for ((mesh, _), material) in list.iter().zip(lit) {
                        m.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(material)));
                    }
                })
                .id();
            merged.push(part);
        }
        body.held_parts = merged;
        body.held = Some((model, key.clone()));
    }
}

/// Bodies no camera saw last frame are posed every this many frames
/// (staggered by entity), not every frame.
pub const UNSEEN_POSE_INTERVAL: u32 = 4;

/// Pose each animated body's joints from its character's `Animator`.
///
/// Only bodies some camera drew last frame (`ViewVisibility` of their
/// meshes) are posed every frame; the rest (bots out of view, the hidden
/// local body) every `UNSEEN_POSE_INTERVAL` frames, so their joints (and
/// what hangs on them: the held weapon and its muzzle) are never far
/// behind. Hitboxes don't read the joints (they pose from
/// `ragdoll::SkeletonPose`, every tick). Bodies aren't culled by their
/// joints (their bounds are the mesh's, at the body), so posing late
/// doesn't hide one. Without a camera (headless) every body is posed.
pub fn pose_bodies(
    time: Res<Time>,
    characters: Query<(&anim::Animator, &Children)>,
    bodies: Query<(Entity, &CharacterBody, Option<&Children>)>,
    seen: Query<&ViewVisibility, With<Mesh3d>>,
    cameras: Query<(), With<Camera>>,
    mut frame: Local<u32>,
    mut joints: Query<&mut Transform, With<BodyJoint>>,
) {
    let now = time.elapsed_secs_f64();
    *frame = frame.wrapping_add(1);
    let culling = !cameras.is_empty();
    for (animator, children) in &characters {
        let Some((body_entity, body, parts)) = children.iter().find_map(|c| bodies.get(c).ok()) else {
            continue;
        };
        // Never another skeleton's pose on this body (a character changing
        // bodies: `attach_bodies`).
        if animator.main.is_none() || !animator.drives(body.skeleton) {
            continue;
        }
        if culling
            && !body_seen(parts, &seen)
            && !frame
                .wrapping_add(body_entity.index_u32())
                .is_multiple_of(UNSEEN_POSE_INTERVAL)
        {
            continue;
        }
        let pose = animator.pose(now);
        for (joint, (q, p)) in body.joints.iter().zip(pose) {
            if let Ok(mut t) = joints.get_mut(*joint)
                && (t.rotation != q || t.translation != p)
            {
                t.rotation = q;
                t.translation = p;
            }
        }
    }
}

/// Whether any of a body's meshes was drawn by some camera last frame.
pub fn body_seen(parts: Option<&Children>, seen: &Query<&ViewVisibility, With<Mesh3d>>) -> bool {
    parts.into_iter().flatten().any(|p| seen.get(*p).is_ok_and(|v| v.get()))
}

/// Take the owner's drawn body over: out of the character, visible (also
/// the local player's), without its held weapon, never culled (its mesh
/// bounds are the standing pose's).
#[allow(clippy::type_complexity)]
pub fn adopt_bodies(
    mut ragdolls: Query<&mut Ragdoll>,
    owners: Query<&Children>,
    mut bodies: Query<(&mut CharacterBody, Option<&Children>)>,
    meshes: Query<(), With<Mesh3d>>,
    mut commands: Commands,
) {
    for mut r in &mut ragdolls {
        if r.visual.is_some() || r.placed {
            continue;
        }
        let Ok(children) = owners.get(r.owner) else { continue };
        let Some(body) = children.iter().find(|c| bodies.contains(*c)) else {
            continue;
        };
        let (mut b, parts) = bodies.get_mut(body).unwrap();
        if let Some((held, _)) = b.held.take() {
            commands.entity(held).despawn();
        }
        for part in parts.into_iter().flatten() {
            if meshes.contains(*part) {
                commands.entity(*part).insert(bevy::camera::visibility::NoFrustumCulling);
            }
        }
        commands
            .entity(body)
            .remove::<ChildOf>()
            .insert((Transform::IDENTITY, Visibility::Inherited, MapPart));
        // Lit where the ragdoll lies (its root body), not at the origin
        // where the drawn body now sits.
        if let Some(&root) = r.bodies.first() {
            commands.queue(move |w: &mut World| {
                if let Some(mut lit) = w.get_mut::<probe_lit::ProbeLit>(body) {
                    lit.follow = Some(root);
                    lit.offset = Vec3::ZERO;
                }
            });
        }
        r.visual = Some(body);
    }
}

/// Pose a ragdoll's drawn skeleton from its bodies (spec 5): a simulated
/// bone takes its body's rotation, and its position from the parent
/// body's bone and the joint anchor (the root body: its own); the other
/// bones keep their bind pose under their parent.
#[allow(clippy::type_complexity)]
pub fn pose_ragdolls(
    ragdolls: Query<&Ragdoll>,
    models: Option<Res<CharacterModels>>,
    drawn: Query<&CharacterBody>,
    physics: Query<&Transform, (With<RagdollBody>, Without<BodyJoint>)>,
    mut joints: Query<&mut Transform, With<BodyJoint>>,
) {
    let Some(models) = models else { return };
    for r in &ragdolls {
        let (Some(visual), Some(m)) = (r.visual, models.0.get(r.model)) else {
            continue;
        };
        let (Ok(drawn), Some(ragdoll)) = (drawn.get(visual), m.ragdoll.as_ref()) else {
            continue;
        };
        let to_skeleton = m.root.compute_affine().inverse();
        let inv_root = m.root.rotation.inverse();
        let mut simulated: Vec<Option<usize>> = vec![None; m.bones.len()];
        for (i, b) in ragdoll.bodies.iter().enumerate() {
            if let Some(s) = simulated.get_mut(b.bone) {
                *s = Some(i);
            }
        }
        let mut global: Vec<(Quat, Vec3)> = Vec::with_capacity(m.bones.len());
        for (bone, b) in m.bones.iter().enumerate() {
            let parent = b.parent.and_then(|p| global.get(p).copied());
            let g = match simulated[bone].and_then(|i| Some((i, physics.get(*r.bodies.get(i)?).ok()?))) {
                Some((i, t)) => {
                    let q = inv_root * t.rotation;
                    let p = match r.parents[i] {
                        Some((pi, anchor)) => {
                            let (pq, pp) = global[ragdoll.bodies[pi].bone];
                            pp + pq * anchor
                        }
                        None => to_skeleton.transform_point3(t.translation),
                    };
                    (q, p)
                }
                None => match parent {
                    Some((pq, pp)) => (pq * b.rotation, pp + pq * b.position),
                    None => (b.rotation, b.position),
                },
            };
            global.push(g);
        }
        for (bone, b) in m.bones.iter().enumerate() {
            let (q, p) = global[bone];
            let local = match b.parent.map(|i| global[i]) {
                Some((pq, pp)) => (pq.inverse() * q, pq.inverse() * (p - pp)),
                None => (q, p),
            };
            // Only real changes: a ragdoll at rest leaves its joints (and
            // transform propagation under them) alone.
            if let Some(mut t) = drawn.joints.get(bone).and_then(|j| joints.get_mut(*j).ok())
                && (t.rotation != local.0 || t.translation != local.1)
            {
                t.rotation = local.0;
                t.translation = local.1;
            }
        }
    }
}

/// A map-placed ragdoll's meshes once it is simulated
/// (`map::placed_ragdoll`): never culled, their bounds being the
/// reference pose's.
pub fn uncull_placed_ragdolls(
    nodes: Query<(&placed_ragdoll::PlacedRagdoll, &Children), Changed<placed_ragdoll::PlacedRagdoll>>,
    meshes: Query<(), (With<Mesh3d>, Without<bevy::camera::visibility::NoFrustumCulling>)>,
    mut commands: Commands,
) {
    for (placed, children) in &nodes {
        if placed.sim.is_none() {
            continue;
        }
        for c in children {
            if meshes.contains(*c) {
                commands.entity(*c).insert(bevy::camera::visibility::NoFrustumCulling);
            }
        }
    }
}

/// Render assets of a skinned model: meshes with joint attributes (when
/// it has a skeleton), materials, and inverse bind poses from the bones'
/// reference pose under `root`.
pub fn body_assets(
    model: &MapModel,
    bones: &[MapBone],
    root: Transform,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<PropMaterial>,
    bindposes: &mut Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
    material: &dyn Fn(&MapMesh) -> PropMaterial,
) -> BodyAssets {
    let mut stores = Stores { meshes, materials, bindposes };
    body_assets_into(model, bones, root, &mut stores, material)
}

/// `body_assets`, its assets going to `to`: a map's stores or a late model's
/// `late_build::Stage`.
pub fn body_assets_into<T>(
    model: &MapModel,
    bones: &[MapBone],
    root: Transform,
    to: &mut T,
    material: &dyn Fn(&MapMesh) -> PropMaterial,
) -> BodyAssets
where
    T: AddAsset<Mesh> + AddAsset<PropMaterial> + AddAsset<bevy::mesh::skinning::SkinnedMeshInverseBindposes>,
{
    let skinned_mesh = |m: &MapMesh, to: &mut T| {
        let mut mesh = build_mesh(m, false);
        if m.joints.len() == m.positions.len() && !bones.is_empty() {
            mesh.insert_attribute(
                Mesh::ATTRIBUTE_JOINT_INDEX,
                bevy::mesh::VertexAttributeValues::Uint16x4(m.joints.clone()),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_JOINT_WEIGHT, m.joint_weights.clone());
            // Per-joint boxes, so the drawn part's bounds follow the pose
            // (`DynamicSkinnedMeshBounds`) instead of the bind pose's.
            let _ = mesh.generate_skinned_mesh_bounds();
        }
        // A face with flexes is copied for each body from the main world's
        // (`attach_bodies`); no other part is read there.
        to.add_asset(if m.morphs.is_empty() { gpu_only(mesh) } else { mesh })
    };
    let parts: Vec<(Handle<Mesh>, Handle<PropMaterial>)> = model
        .meshes
        .iter()
        .map(|m| {
            let mesh = skinned_mesh(m, to);
            (mesh, to.add_asset(material(m)))
        })
        .collect();
    // Lower levels (a level without a part keeps the finer one's).
    let levels: Vec<Vec<Handle<Mesh>>> = parts
        .iter()
        .enumerate()
        .map(|(k, (root, _))| {
            let mut levels = vec![root.clone()];
            for l in &model.lods {
                let h = match l.meshes.get(k).and_then(Option::as_ref) {
                    Some(lm) => skinned_mesh(lm, to),
                    None => levels.last().cloned().unwrap_or_default(),
                };
                levels.push(h);
            }
            levels
        })
        .collect();
    // The body's bounds in its own space (the bind pose: the meshes'
    // vertices are in it).
    let lod_group = (!model.lods.is_empty()).then(|| {
        let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
        for p in model.meshes.iter().flat_map(|m| &m.positions) {
            lo = lo.min(Vec3::from(*p));
            hi = hi.max(Vec3::from(*p));
        }
        super::lod::LodGroup::new((lo, hi), &model.lods)
    });
    let skins = model
        .skins
        .iter()
        .map(|looks| {
            model
                .meshes
                .iter()
                .zip(looks)
                .map(|(m, look)| to.add_asset(material(&look.apply(m))))
                .collect()
        })
        .collect();
    // Each bone's reference pose in body space, inverted.
    let mut global: Vec<Mat4> = Vec::with_capacity(bones.len());
    for b in bones {
        let local = Mat4::from_rotation_translation(b.rotation, b.position);
        let parent = b
            .parent
            .and_then(|p| global.get(p).copied())
            .unwrap_or(root.to_matrix());
        global.push(parent * local);
    }
    let inverse: Vec<Mat4> = global.iter().map(|m| m.inverse()).collect();
    BodyAssets {
        parts,
        levels,
        lod_group,
        skins,
        bindposes: to.add_asset(bevy::mesh::skinning::SkinnedMeshInverseBindposes::from(inverse)),
        bones: bones.to_vec(),
        skeleton: anim::skeleton_signature(bones.iter().map(|b| (b.name.as_str(), b.parent))),
        root,
        body_parts: model.body_parts.clone(),
        mesh_bodies: model.meshes.iter().map(|m| m.body).collect(),
        faces: model
            .meshes
            .iter()
            .map(|m| match (&m.eye, &m.teeth) {
                (Some(e), _) => Some(Face::Eye((**e).clone())),
                (None, Some(t)) => Some(Face::Teeth(t.clone())),
                _ => None,
            })
            .collect(),
        morphs: model.meshes.iter().map(|m| std::sync::Arc::new(m.morphs.clone())).collect(),
    }
}

/// Each face mesh with flexes takes its character's `FaceWeights` (when
/// they change): its rest positions plus each flex's offsets at its
/// side's weight. A ragdoll's keeps what it showed.
pub fn apply_face_morphs(
    mut parts: Query<(&mut FaceMorph, &Mesh3d, &ChildOf)>,
    bodies: Query<Option<&ChildOf>, With<CharacterBody>>,
    weights: Query<&FaceWeights>,
    meshes: Option<ResMut<Assets<Mesh>>>,
) {
    let Some(mut meshes) = meshes else { return };
    for (mut face, mesh, body) in &mut parts {
        let Some(owner) = bodies.get(body.parent()).ok().flatten().map(ChildOf::parent) else { continue };
        let Ok(w) = weights.get(owner) else { continue };
        if w.0 == face.shown {
            continue;
        }
        let mut positions: Vec<[f32; 3]> = face.rest.to_vec();
        for m in face.morphs.iter() {
            let [w1, w3] = w.0.get(m.flex as usize).copied().unwrap_or([0.0; 2]);
            if w1.abs() < 1e-4 && w3.abs() < 1e-4 {
                continue;
            }
            for ((v, d), side) in m.vertices.iter().zip(&m.deltas).zip(&m.sides) {
                let k = w1 * (1.0 - side) + w3 * side;
                if let Some(p) = positions.get_mut(*v as usize) {
                    p[0] += d[0] * k;
                    p[1] += d[1] * k;
                    p[2] += d[2] * k;
                }
            }
        }
        if let Some(mut m) = meshes.get_mut(&mesh.0) {
            m.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
        }
        face.shown = w.0.clone();
    }
}

/// Each face part's material from where it is drawn this frame: an
/// eyeball's centre, up and iris planes toward its character's
/// `EyeTarget` (ahead without one, or once its body is a ragdoll's: spec
/// 10.2), teeth toward the mouth's forward, unlit while the mouth is shut
/// (spec 11.5 at rest).
pub fn aim_faces(
    parts: Query<(&FacePart, &MeshMaterial3d<PropMaterial>, &ChildOf)>,
    bodies: Query<Option<&ChildOf>, With<CharacterBody>>,
    targets: Query<&EyeTarget>,
    joints: Query<&GlobalTransform, With<BodyJoint>>,
    materials: Option<ResMut<Assets<PropMaterial>>>,
) {
    let Some(mut materials) = materials else { return };
    for (part, material, body) in &parts {
        let Ok(bone) = joints.get(part.joint) else { continue };
        let owner = bodies.get(body.parent()).ok().flatten().map(ChildOf::parent);
        let target = owner.and_then(|o| targets.get(o).ok()).map(|t| t.0);
        let scale = bone.compute_transform().scale.x;
        let Some(mut m) = materials.get_mut(&material.0) else { continue };
        let p = &mut m.params;
        match &part.face {
            Face::Eye(eye) => {
                let aim = eye.aim(bone, target, scale);
                let (centre, up, _) = eye.world(bone);
                let [u, v] = eye.projection(bone, aim, scale);
                p.face_center = centre.extend(1.0);
                p.face_up = up.extend(0.0);
                p.face_u = u;
                p.face_v = v;
            }
            Face::Teeth(t) => {
                let forward = bone.affine().transform_vector3(t.forward).normalize_or_zero();
                p.face_up = forward.extend(0.0);
            }
        }
    }
}

#[cfg(test)]
mod pose_tests {
    use super::*;
    use std::sync::Arc;

    /// One bone whose pose is always (identity, (1, 2, 3)).
    fn animator() -> anim::Animator {
        let pose = (Quat::IDENTITY, Vec3::new(1.0, 2.0, 3.0));
        let set = anim::AnimSet {
            defaults: vec![pose],
            bases: vec![vec![pose]],
            animations: vec![anim::Animation {
                frames: 1,
                fps: 30.0,
                ..default()
            }],
            sequences: vec![anim::Sequence {
                grid: (1, 1),
                anims: vec![0],
                looping: true,
                bone_weights: vec![1.0],
                ..default()
            }],
            ..default()
        };
        let mut a = anim::Animator::new(Arc::new(set));
        a.play(0, 0.0);
        a
    }

    /// A character with a one-joint body and one mesh part drawn or not;
    /// returns the joint.
    fn spawn_character(app: &mut App, drawn: bool) -> Entity {
        let world = app.world_mut();
        let character = world.spawn(animator()).id();
        let body = world.spawn(ChildOf(character)).id();
        let joint = world.spawn((BodyJoint(0), Transform::default(), ChildOf(body))).id();
        let seen = if drawn {
            ViewVisibility::VISIBLE
        } else {
            ViewVisibility::HIDDEN
        };
        world.spawn((Mesh3d::default(), seen, ChildOf(body)));
        world.entity_mut(body).insert(CharacterBody {
            skeleton: 0,
            model: 0,
            skin: 0,
            body: 0,
            joints: vec![joint],
            held: None,
            held_parts: Vec::new(),
        });
        joint
    }

    /// Frames out of `n` in which the joint was posed.
    fn posed_frames(app: &mut App, joint: Entity, n: u32) -> u32 {
        let mut posed = 0;
        for _ in 0..n {
            app.world_mut().get_mut::<Transform>(joint).unwrap().translation = Vec3::ZERO;
            app.update();
            posed += (app.world().get::<Transform>(joint).unwrap().translation == Vec3::new(1.0, 2.0, 3.0)) as u32;
        }
        posed
    }

    fn app(camera: bool) -> App {
        let mut app = App::new();
        app.init_resource::<Time>().add_systems(Update, pose_bodies);
        if camera {
            app.world_mut().spawn(Camera::default());
        }
        app
    }

    #[test]
    fn unseen_bodies_are_not_posed_every_frame() {
        let mut app = app(true);
        let seen = spawn_character(&mut app, true);
        let unseen = spawn_character(&mut app, false);
        let n = 8 * UNSEEN_POSE_INTERVAL;
        assert_eq!(posed_frames(&mut app, seen, n), n, "a drawn body is posed every frame");
        assert_eq!(
            posed_frames(&mut app, unseen, n),
            n / UNSEEN_POSE_INTERVAL,
            "a body no camera drew is posed every {UNSEEN_POSE_INTERVAL} frames"
        );
    }

    #[test]
    fn without_cameras_every_body_is_posed() {
        let mut app = app(false);
        let unseen = spawn_character(&mut app, false);
        assert_eq!(posed_frames(&mut app, unseen, 8), 8);
    }
}

#[cfg(test)]
mod late_held_tests {
    use super::*;
    use std::sync::Arc;

    /// A held model added after the map spawned (`map::add_held_model`)
    /// is built into the map's `HeldAssets` with its own texture, and its
    /// loose form into `LooseAssets`; on the next map (new, empty
    /// `HeldAssets`) it is built again.
    #[test]
    fn late_held_models_are_built() {
        bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default);
        let mut app = App::new();
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<PropMaterial>>()
            .init_resource::<Assets<bevy::mesh::skinning::SkinnedMeshInverseBindposes>>()
            .insert_resource(HeldAssets(HashMap::new()))
            .init_resource::<crate::map::loose::LooseAssets>()
            .insert_resource(HeldLateBase {
                data: MapData::default(),
                lit: false,
                view: MapDebugView::Normal,
                root: Some(Transform::IDENTITY),
                generation: 1,
            })
            .add_systems(Update, build_late_held);
        let mesh = MapMesh {
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            uvs: vec![[0.0; 2]; 3],
            indices: vec![0, 1, 2],
            texture: Some(0),
            ..default()
        };
        let model = MapHeldModel {
            key: "game:gun".into(),
            model: MapModel {
                meshes: vec![mesh],
                ..default()
            },
            bone: "hand".into(),
            muzzle: None,
            attachments: Vec::new(),
            merged: Vec::new(),
            own: None,
        };
        let texture = MapTexture {
            name: "gun.dtx".into(),
            srgb: true,
            mips: Vec::new(),
            width: 1,
            height: 1,
            rgba8: vec![255, 0, 0, 255],
            bc: None,
        };
        crate::map::add_held_model(app.world_mut(), model, Arc::new(vec![texture]));
        // It comes from a task, over frames (`late_build`).
        let until_built = |app: &mut App| {
            for _ in 0..20_000 {
                app.update();
                if app.world().resource::<HeldAssets>().0.contains_key("game:gun") {
                    return;
                }
                std::thread::sleep(std::time::Duration::from_micros(100));
            }
            panic!("never built");
        };
        until_built(&mut app);
        let parts = &app.world().resource::<HeldAssets>().0["game:gun"];
        assert_eq!((parts.bone.as_str(), parts.whole.len(), parts.own.len()), ("hand", 1, 1));
        let material = app.world().resource::<Assets<PropMaterial>>().get(&parts.whole[0].1).unwrap();
        assert!(material.base.is_some(), "drawn with its own texture");
        assert!(app.world().resource::<crate::map::loose::LooseAssets>().0.contains_key("game:gun"));
        // The next map's spawn replaces `HeldAssets`.
        app.world_mut().insert_resource(HeldAssets(HashMap::new()));
        until_built(&mut app);
    }
}
