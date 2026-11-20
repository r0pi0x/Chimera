//! Held models' bone-merged parts (`render::bodies::attach_held`): each
//! part rides its own joint of the body; a body lacking a joint some part
//! merges onto holds the model whole on its bone (TF2's Revolver on the
//! Spy, whose model lacks `weapon_bone_4`), so no part goes undrawn; one
//! held model a body, swapped for another in one frame (specs/tf2/
//! player_animation.md 7a).

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::core::Intent;
use chimera_present::{
    MapPlugin,
    render::{
        bodies::CharacterBody,
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{Corpse, GameKit, Held, MapBone, MapBrush, MapCharacterModel, MapData, MapHeldModel, MapMesh, MapModel},
};

fn model(meshes: usize) -> MapModel {
    let mesh = MapMesh {
        material: "test/held".into(),
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 1.0, 0.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        joints: vec![[0, 0, 0, 0]; 3],
        joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        color: [128, 128, 128],
        ..default()
    };
    MapModel { meshes: vec![mesh; meshes], ..default() }
}

/// A gun of 3 meshes on `hand`: 1 its own, 1 merged onto `pelvis`, 1 onto
/// `other` (absent from `with_other: false` bodies).
fn gun(key: &str, other: &str) -> MapHeldModel {
    MapHeldModel {
        key: key.into(),
        bone: "hand".into(),
        model: model(3),
        muzzle: None,
        attachments: Vec::new(),
        merged: vec![("pelvis".into(), model(1)), (other.into(), model(1))],
        own: Some(model(1)),
        nose: None,
        turned: Vec::new(),
    }
}

fn sim() -> Sim {
    let bone = |name: &str, parent| MapBone { name: name.into(), parent, position: Vec3::Y, rotation: Quat::IDENTITY };
    let body = MapCharacterModel {
        game: Some("test"),
        team: None,
        name: None,
        class: None,
        model: model(1),
        hitboxes: Vec::new(),
        bones: vec![bone("pelvis", None), bone("hand", Some(0)), bone("finger", Some(1))],
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    let map = MapData {
        name: "test:held_merge".into(),
        collision_brushes: vec![MapBrush::from_box(Vec3::new(-5.0, -1.0, -5.0), Vec3::new(5.0, 0.0, 5.0))],
        kit: GameKit { characters: vec![body], held: vec![gun("test:fits", "finger"), gun("test:lacks", "thumb")], ..default() },
        ..default()
    };
    let mut sim = Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(map));
    });
    sim.ticks(3);
    sim
}

/// The meshes under the held model's entity, and the merged parts' count.
fn held(sim: &mut Sim, key: &str) -> (usize, usize) {
    let e = sim.app.world_mut().spawn((Intent::default(), Corpse { feet: 0.0 }, Transform::default(), Held(Some(key.into())))).id();
    sim.ticks(4);
    let w = sim.app.world_mut();
    let body = w.query::<(&CharacterBody, &ChildOf)>().iter(w).find(|(_, c)| c.parent() == e).map(|(b, _)| b).expect("a body");
    let (model, _) = body.held.clone().expect("held");
    let parts = body.held_parts.len();
    let meshes = w.get::<Children>(model).map_or(0, |c| c.iter().filter(|c| w.get::<Mesh3d>(*c).is_some()).count());
    (meshes, parts)
}

#[test]
fn merged_parts_ride_their_joints_or_the_model_is_held_whole() {
    let mut sim = sim();
    assert_eq!(held(&mut sim, "test:fits"), (1, 2), "its own mesh on the hand, a part on each joint");
    assert_eq!(held(&mut sim, "test:lacks"), (3, 0), "no `thumb` on the body: held whole");
}

/// specs/tf2/player_animation.md 7a, tests 57, 58: a body draws one held
/// model, its character's `Held` (the active weapon; nothing holstered),
/// and a switch takes the old one away and puts the new one in its hand
/// in the same frame.
#[test]
fn a_switch_swaps_the_held_model_in_one_frame_57_58() {
    let mut sim = sim();
    let e = sim.app.world_mut().spawn((Intent::default(), Corpse { feet: 0.0 }, Transform::default(), Held(Some("test:fits".into())))).id();
    sim.ticks(4);
    let shown = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let body = w.query::<(&CharacterBody, &ChildOf)>().iter(w).find(|(_, c)| c.parent() == e).map(|(b, _)| b).expect("a body");
        (body.held.clone(), body.held_parts.len())
    };
    let (held, parts) = shown(&mut sim);
    let (old, key) = held.expect("held");
    assert_eq!((key.as_str(), parts), ("test:fits", 2), "57: the one held model and its parts");
    sim.app.world_mut().get_mut::<Held>(e).unwrap().0 = Some("test:lacks".into());
    sim.app.update();
    let (held, parts) = shown(&mut sim);
    let (new, key) = held.expect("the new one at once");
    assert_eq!((key.as_str(), parts), ("test:lacks", 0), "58: the new model (whole) in the switch's frame");
    assert!(sim.app.world().get_entity(old).is_err(), "58: the old one gone in the same frame");
    assert_ne!(old, new);
}
