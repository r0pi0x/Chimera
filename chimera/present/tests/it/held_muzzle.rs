//! Every game's held weapon reports its muzzle the same way
//! (`map::muzzle::HeldMuzzles`, `render::bodies::attach_held`): a held
//! model's own muzzle where it has one, its bone (the hand) where it has
//! none, and on a body of another game's skeleton (no bone of the model's:
//! a Halo gun on a CS:S body) the body's right hand; the third-person
//! effects (tracers, flashes, a Halo projectile's drawn start) start there,
//! near the hand and in front of the body, never at the head; the eye only
//! where nothing is drawn.

use bevy::{ecs::system::RunSystemOnce, mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::core::Intent;
use chimera_present::{
    MapPlugin,
    render::{
        prop_material::PropMaterial, rope_material::RopeMaterial, shadows::ShadowMaterial,
        sprite_material::SpriteMaterial, world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        Corpse, GameKit, Held, MapBone, MapBrush, MapCharacterModel, MapData, MapHeldModel, MapMesh, MapModel,
        muzzle::HeldMuzzles,
    },
};

fn model() -> MapModel {
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
    MapModel { meshes: vec![mesh], ..default() }
}

/// A gun held on `bone`, its muzzle (if any) 0.45 m along the bone's +X.
fn gun(key: &str, bone: &str, muzzle: bool) -> MapHeldModel {
    MapHeldModel {
        key: key.into(),
        bone: bone.into(),
        model: model(),
        muzzle: muzzle.then(|| Transform::from_xyz(0.45, 0.05, 0.0)),
        attachments: Vec::new(),
        merged: Vec::new(),
        own: None,
        nose: None,
        turned: Vec::new(),
    }
}

/// Where the head and the right hand are (metres, the body's frame: it
/// faces −Z).
const HEAD: Vec3 = Vec3::new(0.0, 1.7, 0.0);
const HAND: Vec3 = Vec3::new(0.2, 1.25, -0.35);

/// A body named in `game`'s way (`hand`: its right hand's name; None: no
/// right hand), holding guns of its own game and another's.
fn sim(hand: Option<&str>) -> Sim {
    let bone = |name: &str, parent, position| MapBone { name: name.into(), parent, position, rotation: Quat::IDENTITY };
    let mut bones = vec![bone("pelvis", None, Vec3::Y * 0.95), bone("head", Some(0), HEAD - Vec3::Y * 0.95)];
    // The hand's +X along the body's forward (−Z), as a gun points.
    if let Some(h) = hand {
        bones.push(MapBone {
            name: h.into(),
            parent: Some(0),
            position: HAND - Vec3::Y * 0.95,
            rotation: Quat::from_rotation_y(std::f32::consts::FRAC_PI_2),
        });
    }
    let body = MapCharacterModel {
        game: Some("test"),
        team: None,
        name: None,
        class: None,
        model: model(),
        hitboxes: Vec::new(),
        bones,
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    let held = vec![
        gun("test:own", hand.unwrap_or("none"), true),
        gun("test:no_muzzle", hand.unwrap_or("none"), false),
        // Another game's gun: its skeleton's hand bone, not this body's.
        gun("other:halo_gun", "frame r hand", true),
    ];
    let map = MapData {
        name: "test:held_muzzle".into(),
        collision_brushes: vec![MapBrush::from_box(Vec3::new(-5.0, -1.0, -5.0), Vec3::new(5.0, 0.0, 5.0))],
        kit: GameKit { characters: vec![body], held, ..default() },
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

/// The muzzle point `key` held shows from (None: none drawn) and what
/// `at_or_eye` gives with the eye at `HEAD`.
fn muzzle(sim: &mut Sim, key: &str) -> (Option<Vec3>, Vec3) {
    let e = sim.app.world_mut().spawn((Intent::default(), Corpse { feet: 0.0 }, Transform::default(), Held(Some(key.into())))).id();
    sim.ticks(4);
    let got = sim
        .app
        .world_mut()
        .run_system_once(move |mut m: HeldMuzzles| (m.of(e).map(|m| m.at), m.at_or_eye(e, HEAD)))
        .unwrap();
    sim.app.world_mut().entity_mut(e).despawn();
    sim.ticks(1);
    got
}

#[test]
fn every_held_weapon_shoots_from_near_the_hand_not_the_head() {
    for hand in ["ValveBiped.Bip01_R_Hand", "bip_hand_R", "bip01 r hand", "RightHand"] {
        let mut sim = sim(Some(hand));
        for (key, near) in [("test:own", 0.5), ("test:no_muzzle", 0.01), ("other:halo_gun", 0.01)] {
            let (at, used) = muzzle(&mut sim, key);
            let at = at.unwrap_or_else(|| panic!("{hand}: {key}: no muzzle drawn"));
            assert_eq!(used, at, "{hand}: {key}: the muzzle, not the eye");
            assert!(at.distance(HAND) <= near, "{hand}: {key}: {at:?} by the hand {HAND:?}");
            assert!(at.distance(HEAD) > 0.45 && at.y < HEAD.y - 0.3, "{hand}: {key}: {at:?} not at the head");
            assert!(at.z < -0.3, "{hand}: {key}: {at:?} in front of the body");
        }
        // Its own gun's muzzle out along the barrel (the hand's +X: forward).
        let (at, _) = muzzle(&mut sim, "test:own");
        assert!(at.unwrap().z < HAND.z - 0.3, "{hand}: the muzzle ahead of the hand: {at:?}");
    }
    // A body with no right hand and another game's gun: the eye.
    let mut sim = sim(None);
    assert_eq!(muzzle(&mut sim, "other:halo_gun"), (None, HEAD));
}
