//! A skeleton never drives another's model (`render::bodies`): a pose goes
//! only on a body of the skeleton it is for (`Animator::skeleton`), and a
//! character changing bodies keeps the one showing until the new one's pose
//! is on hand, then changes both in one frame, the new body in that pose
//! from its first frame. Backlog "changing Combat Arms characters hitches
//! and garbles".

use std::sync::Arc;

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_present::{
    MapPlugin,
    render::{
        bodies::{BODY_POSE_WAIT, BodyJoint, CharacterBody},
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{BodyName, MapBone, MapCharacterModel, MapData, MapMesh, MapModel, anim::{AnimSet, Animator, BonePose}},
};

fn model(name: &str, bones: &[(&str, Option<usize>)]) -> MapCharacterModel {
    let mesh = MapMesh {
        material: "test/body".into(),
        positions: vec![[-0.2, 0.0, 0.0], [0.2, 0.0, 0.0], [0.0, 0.4, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        joints: vec![[0, 0, 0, 0]; 3],
        joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
        color: [128; 3],
        ..default()
    };
    MapCharacterModel {
        game: None,
        team: None,
        name: Some(name.into()),
        class: None,
        model: MapModel { meshes: vec![mesh], ..default() },
        hitboxes: Vec::new(),
        bones: bones
            .iter()
            .map(|(n, parent)| MapBone { name: (*n).into(), parent: *parent, position: Vec3::Y * 0.1, rotation: Quat::IDENTITY })
            .collect(),
        root: Transform::IDENTITY,
        animations: Some(Arc::new(AnimSet::default())),
        boxes: Vec::new(),
        ragdoll: None,
    }
}

const A: &[(&str, Option<usize>)] = &[("root", None), ("arm", Some(0))];
/// Another shape: the bones in another order, one more.
const B: &[(&str, Option<usize>)] = &[("root", None), ("leg", Some(0)), ("arm", Some(0))];

fn sim() -> Sim {
    let mut data = MapData { name: "test:body_swap".into(), entity_scale: 0.0254, ..default() };
    data.kit.characters.push(model("a", A));
    data.kit.characters.push(model("b", B));
    Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), move |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(data.clone()));
    })
}

fn sig(bones: &[(&str, Option<usize>)]) -> u64 {
    chimera_world::map::anim::skeleton_signature(bones.iter().copied())
}

fn pose(n: usize, x: f32) -> chimera_core::later::Later<Vec<BonePose>> {
    chimera_core::later::Later::ready(vec![(Quat::IDENTITY, Vec3::X * x); n])
}

fn drive(sim: &mut Sim, e: Entity, skeleton: u64, n: usize, x: f32) {
    let mut a = sim.app.world_mut().get_mut::<Animator>(e).unwrap();
    a.skeleton = Some(skeleton);
    a.main = Some(0);
    a.fixed = Some(pose(n, x));
}

/// The one body showing (fails on two): its joints' x places by bone.
fn shown(sim: &mut Sim) -> (usize, Vec<f32>) {
    let w = sim.app.world_mut();
    let bodies: Vec<(usize, Vec<Entity>)> = w.query::<&CharacterBody>().iter(w).map(|b| (b.model, b.joints.clone())).collect();
    assert_eq!(bodies.len(), 1, "one body at all times");
    let (model, joints) = bodies.into_iter().next().unwrap();
    (model, joints.iter().map(|j| w.get::<Transform>(*j).unwrap().translation.x).collect())
}

#[test]
fn a_skeleton_never_drives_another_models_body() {
    let mut sim = sim();
    let e = sim
        .app
        .world_mut()
        .spawn((
            chimera_core::core::Intent::default(),
            BodyName("a".into()),
            avian3d::prelude::ColliderAabb::new(Vec3::ZERO, Vec3::splat(0.3)),
            Transform::default(),
        ))
        .id();
    // Its first body waits for a pose only for a moment.
    sim.seconds(BODY_POSE_WAIT + 0.1);
    drive(&mut sim, e, sig(A), A.len(), 1.0);
    sim.ticks(2);
    assert_eq!(shown(&mut sim), (0, vec![1.0; A.len()]), "its own skeleton poses it");
    // Another skeleton's pose: not on this body.
    drive(&mut sim, e, sig(B), B.len(), 2.0);
    sim.ticks(3);
    assert_eq!(shown(&mut sim), (0, vec![1.0; A.len()]), "another skeleton's pose stays off it");
    // The change of body: the animator comes back empty on the new
    // skeleton (`attach_hitboxes`), and the old body stays until it has a
    // pose, whatever the wait (within `BODY_POSE_WAIT`).
    drive(&mut sim, e, sig(A), A.len(), 1.0);
    sim.ticks(1);
    sim.app.world_mut().entity_mut(e).insert(BodyName("b".into()));
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<Animator>(e).unwrap().skeleton, Some(sig(B)));
    assert_eq!(shown(&mut sim).0, 0, "the old body stays, no pose yet");
    // Both change in one frame: the new body has its pose from its first.
    drive(&mut sim, e, sig(B), B.len(), 3.0);
    sim.ticks(1);
    assert_eq!(shown(&mut sim), (1, vec![3.0; B.len()]), "the new body, in its pose, at once");
    // A game that never poses it (no main): the body comes in its reference
    // pose after the wait, not never.
    sim.app.world_mut().entity_mut(e).insert(BodyName("a".into()));
    sim.seconds(BODY_POSE_WAIT + 0.1);
    assert_eq!(shown(&mut sim).0, 0, "it came");
    let w = sim.app.world_mut();
    let n = w.query::<&BodyJoint>().iter(w).count();
    assert_eq!(n, A.len(), "the old body's joints went");
}
