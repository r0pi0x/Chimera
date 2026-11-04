//! Parts made during play are drawn by `render` from the simulation's
//! plain-data requests (docs/ARCHITECTURE.md, "Drawn-part requests"): a
//! character's body and what it holds, a dead character's body handed to
//! its ragdoll, loose and shown items, gibs. With the render asset stores
//! each gets its meshes; headless the same requests make the simulation's
//! parts (bodies, colliders) and no meshes, and so with the map's systems
//! alone (`map::MapSimPlugin`, the dedicated server's).

use bevy::prelude::*;
use crate::stack::{
    core::{Health, Hitgroup},
    harness::{CsSim, Sim},
    map::{
        Held, MapData, MapDebugView, MapHeldModel, MapMesh, MapModel, MapPart, MapPlugin, MapSimPlugin, PlaySound,
        Ragdoll, SoundControl, SpawnGibs,
        ragdoll::Ragdolled,
        breakables::{GibPiece, MapGibSet},
        change_map,
        loose::{LooseItem, ShownItem},
    },
    render::{bodies::CharacterBody, prop_material::PropMaterial, vis::VisStats},
    rules::Deathmatch,
};

use crate::ragdoll::{SCALE, floor_map, kill, model};

/// A one-triangle mesh, inches about the origin.
fn triangle() -> MapMesh {
    MapMesh {
        positions: vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 1.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        ..default()
    }
}

fn one_mesh_model() -> MapModel {
    MapModel {
        meshes: vec![triangle()],
        bounds: (Vec3::ZERO, Vec3::splat(4.0 * SCALE)),
        ..default()
    }
}

/// The ragdoll tests' floor and character, with a drawn mesh on the
/// body, a held model "gun" and a gib list "chunks".
fn map() -> MapData {
    let mut character = model();
    character.model = one_mesh_model();
    let mut data = floor_map(character);
    data.kit.held = vec![MapHeldModel {
        key: "gun".into(),
        model: one_mesh_model(),
        bone: "pelvis".into(),
        own: None,
        merged: Vec::new(),
        muzzle: None,
        attachments: Vec::new(),
    }];
    data.gibs = vec![MapGibSet {
        name: "chunks".into(),
        models: vec![one_mesh_model()],
    }];
    data
}

/// The asset stores `render` draws into (headless `Sim` has meshes only).
fn render_assets(app: &mut App) {
    app.init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<PropMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
}

fn sim(drawn: bool) -> (Sim, Entity) {
    let mut sim = if drawn {
        Sim::new(CsSim, (render_assets, MapPlugin::new(map())))
    } else {
        Sim::new(CsSim, MapPlugin::new(map()))
    };
    sim.app.world_mut().resource_mut::<Deathmatch>().respawn_delay = 1000.0;
    let p = crate::common::spawn_still(&mut sim, Vec3::new(0.0, 0.9, 0.0));
    sim.ticks(10);
    (sim, p)
}

/// Prop-material meshes under `e` (itself and its descendants).
fn drawn_under(sim: &mut Sim, e: Entity) -> usize {
    let world = sim.app.world_mut();
    let mut q = world.query::<&Children>();
    let mut stack = vec![e];
    let mut n = 0;
    while let Some(x) = stack.pop() {
        if world.get::<MeshMaterial3d<PropMaterial>>(x).is_some() && world.get::<Mesh3d>(x).is_some() {
            n += 1;
        }
        if let Ok(c) = q.get(world, x) {
            stack.extend(c.iter());
        }
    }
    n
}

fn all_meshes(sim: &mut Sim) -> usize {
    let world = sim.app.world_mut();
    world.query::<&Mesh3d>().iter(world).count()
}

fn gibs(sim: &mut Sim) -> Vec<Entity> {
    let world = sim.app.world_mut();
    let mut q = world.query::<(Entity, &Name)>();
    q.iter(world).filter(|(_, n)| n.as_str() == "Gib").map(|(e, _)| e).collect()
}

fn throw_gib(sim: &mut Sim) {
    sim.app.world_mut().write_message(SpawnGibs {
        set: "chunks".into(),
        glass: false,
        prop: false,
        pieces: vec![GibPiece::thrown(Vec3::new(1.0, 0.5, 0.0), Vec3::ZERO, Vec3::ZERO, 0.2)],
        shatters: None,
        bounce: None,
    });
}

#[test]
fn play_time_parts_are_drawn_from_their_requests() {
    let (mut sim, p) = sim(true);
    // The character's body, and what it holds on its joint.
    assert_eq!(drawn_under(&mut sim, p), 1, "the body's mesh");
    sim.app.world_mut().entity_mut(p).insert(Held(Some("gun".into())));
    sim.ticks(2);
    assert_eq!(drawn_under(&mut sim, p), 2, "and the held model's");
    // Loose and shown items.
    let loose = sim.app.world_mut().spawn((LooseItem("gun".into()), Transform::from_xyz(2.0, 0.2, 0.0))).id();
    let shown = sim.app.world_mut().spawn((ShownItem("gun".into()), Transform::from_xyz(-2.0, 0.2, 0.0))).id();
    sim.ticks(2);
    assert!(sim.app.world().get::<avian3d::prelude::RigidBody>(loose).is_some(), "the loose item's body");
    assert_eq!(drawn_under(&mut sim, loose), 1, "the loose item's mesh");
    assert_eq!(drawn_under(&mut sim, shown), 1, "the shown item's mesh");
    // A gib, drawn and faded out after its life.
    throw_gib(&mut sim);
    sim.ticks(1);
    let gib = *gibs(&mut sim).first().expect("a gib");
    assert_eq!(drawn_under(&mut sim, gib), 1, "the gib's mesh");
    let material = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        let mut q = world.query::<(&MeshMaterial3d<PropMaterial>, &ChildOf)>();
        let h = q.iter(world).find(|(_, c)| c.parent() == gib).map(|(m, _)| m.0.clone()).unwrap();
        world.resource::<Assets<PropMaterial>>().get(&h).unwrap().params.base_color.w
    };
    assert_eq!(material(&mut sim), 1.0);
    sim.seconds(0.5);
    let alpha = material(&mut sim);
    assert!(alpha < 1.0 && alpha > 0.0, "fading: {alpha}");
    // The dead's body goes to its ragdoll, drawn where the ragdoll lies
    // (the dead hold nothing: games drop it).
    sim.app.world_mut().entity_mut(p).insert(Held(None));
    kill(&mut sim, p, Hitgroup::Chest, Vec3::new(0.0, 1.2, 0.0), Vec3::X);
    sim.ticks(2);
    let world = sim.app.world_mut();
    let r = world.query::<&Ragdoll>().iter(world).next().cloned().expect("a ragdoll");
    let body = r.visual.expect("the drawn body taken over");
    assert!(world.get::<CharacterBody>(body).is_some());
    assert!(world.get::<ChildOf>(body).is_none(), "out of the character");
    assert_eq!(drawn_under(&mut sim, body), 1);
}

/// A character dead at a map change (the gun game's last victim: the
/// winning shot ends the match and the next map loads) lost its drawn
/// body to its ragdoll, which went with the old map; alive again on the
/// new one it gets a body. It used to keep `Ragdolled` with no ragdoll
/// left to clear it, and so no body ever again ("a bot stays invisible",
/// as either game's character).
#[test]
fn a_character_dead_at_a_map_change_is_drawn_again() {
    let (mut sim, p) = sim(true);
    kill(&mut sim, p, Hitgroup::Chest, Vec3::new(0.0, 1.2, 0.0), Vec3::X);
    sim.ticks(2);
    assert!(sim.app.world().get::<Ragdolled>(p).is_some(), "ragdolled");
    assert_eq!(drawn_under(&mut sim, p), 0, "its body is the ragdoll's");
    change_map(sim.app.world_mut(), map(), MapDebugView::Normal);
    sim.ticks(2);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&Ragdoll>().iter(world).count(), 0, "the ragdoll went with the map");
    assert_eq!(drawn_under(&mut sim, p), 0, "still dead: no body yet");
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 100.0;
    sim.ticks(2);
    assert!(sim.app.world().get::<Ragdolled>(p).is_none(), "no ragdoll mark left");
    assert_eq!(drawn_under(&mut sim, p), 1, "the body's mesh again");
}

#[test]
fn headless_the_same_requests_draw_nothing() {
    let (mut sim, p) = sim(false);
    sim.app.world_mut().entity_mut(p).insert(Held(Some("gun".into())));
    let loose = sim.app.world_mut().spawn((LooseItem("gun".into()), Transform::from_xyz(2.0, 0.2, 0.0))).id();
    sim.app.world_mut().spawn((ShownItem("gun".into()), Transform::from_xyz(-2.0, 0.2, 0.0)));
    throw_gib(&mut sim);
    sim.ticks(2);
    assert!(sim.app.world().get::<avian3d::prelude::RigidBody>(loose).is_some(), "the loose item's body");
    assert_eq!(gibs(&mut sim).len(), 1, "the gib flies");
    kill(&mut sim, p, Hitgroup::Chest, Vec3::new(0.0, 1.2, 0.0), Vec3::X);
    sim.ticks(2);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&Ragdoll>().iter(world).count(), 1, "the ragdoll's bodies");
    assert_eq!(all_meshes(&mut sim), 0, "nothing drawn");
}

/// A dedicated server's map (`MapSimPlugin`: map's systems alone, nothing
/// of render or audio registered): the same requests make the
/// simulation's parts, what the simulation writes for the audio is
/// registered without it, and `change_map` spawns a map with the drawer
/// that draws nothing.
#[test]
fn the_map_alone_makes_the_same_parts() {
    let mut sim = Sim::new(CsSim, MapSimPlugin::new(map()));
    sim.app.world_mut().resource_mut::<Deathmatch>().respawn_delay = 1000.0;
    let p = crate::common::spawn_still(&mut sim, Vec3::new(0.0, 0.9, 0.0));
    sim.ticks(10);
    let parts = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        world.query::<&MapPart>().iter(world).count()
    };
    let spawned = parts(&mut sim);
    assert!(spawned > 0, "the map's parts");
    let world = sim.app.world();
    assert!(world.contains_resource::<Messages<PlaySound>>(), "the sounds' messages");
    assert!(world.contains_resource::<Messages<SoundControl>>());
    assert!(!world.contains_resource::<VisStats>(), "no render resources");
    sim.app.world_mut().entity_mut(p).insert(Held(Some("gun".into())));
    let loose = sim.app.world_mut().spawn((LooseItem("gun".into()), Transform::from_xyz(2.0, 0.2, 0.0))).id();
    throw_gib(&mut sim);
    sim.ticks(2);
    assert!(sim.app.world().get::<avian3d::prelude::RigidBody>(loose).is_some(), "the loose item's body");
    assert_eq!(gibs(&mut sim).len(), 1, "the gib flies");
    kill(&mut sim, p, Hitgroup::Chest, Vec3::new(0.0, 1.2, 0.0), Vec3::X);
    sim.ticks(2);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&Ragdoll>().iter(world).count(), 1, "the ragdoll's bodies");
    assert_eq!(all_meshes(&mut sim), 0, "nothing drawn");
    // The map again, unloaded (with what play made) and spawned by the
    // hook `MapSimPlugin` sets.
    change_map(sim.app.world_mut(), map(), MapDebugView::Normal);
    sim.ticks(2);
    assert_eq!(parts(&mut sim), spawned, "the map's parts, spawned again");
}
