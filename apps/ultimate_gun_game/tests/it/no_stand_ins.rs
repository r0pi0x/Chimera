//! Ultimate Gun Game never draws a stand-in (`game_parts::NoStandIns`;
//! backlog "must never show the greybox stand-ins"): through rung changes
//! with a model that never finishes loading (the test's data has no install
//! to load from), a map change and a join's first frames, no player is a
//! capsule (`client::hud::CapsuleBody`) and the local weapon no box
//! (`ui::visuals::BoxGun`), a body once shown keeps showing. The same world
//! without the flag draws them (the greybox app and tests keep theirs). And
//! the ladder's models (`characters::preload`): what a rung gives is what
//! the plan loads, on a network client and a host alike.

use std::sync::Arc;

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_client::client::{FirstPersonCamera, hud::{CapsuleBody, character_bodies}};
use chimera_core::{
    core::{Dead, LocalPlayer, Team},
    game_parts::NoStandIns,
};
use chimera_present::render::{bodies::CharacterBody, prop_material::PropMaterial};
use chimera_sim::{
    minigame::{self, ladder::Ladder},
    weapon::{Inventory, Weapon},
};
use chimera_world::{
    harness::Sim,
    map::{CharacterModels, GameKit, MapBone, MapBrush, MapCharacterModel, MapData, MapMesh, MapModel, anim::AnimSet},
};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        attachments::CaMods,
        characters::{preload::LadderModels, records::Selection},
        data::CaData,
        movement,
        view_model::{on_demand::LoadOnDemand, parts::view_key},
    },
    harness::CaSim,
    ui::visuals::{BoxGun, CaVisualsPlugin},
};
use party_kit::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin};

fn character(game: &'static str) -> MapCharacterModel {
    let bone = |name: &str, parent| MapBone { name: name.into(), parent, position: Vec3::Y, rotation: Quat::IDENTITY };
    MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: MapModel {
            meshes: vec![MapMesh {
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                uvs: vec![[0.0, 0.0]; 3],
                indices: vec![0, 1, 2],
                ..default()
            }],
            ..default()
        },
        hitboxes: Vec::new(),
        bones: vec![bone("pelvis", None), bone(game, Some(0))],
        root: Transform::IDENTITY,
        animations: Some(Arc::new(AnimSet::default())),
        boxes: Vec::new(),
        ragdoll: None,
    }
}

fn floor() -> MapData {
    let (lo, hi) = (Vec3::new(-30.0, -1.0, -30.0), Vec3::new(30.0, 0.0, 30.0));
    MapData {
        name: "test:no_stand_ins".into(),
        collision_brushes: vec![MapBrush::from_box(lo, hi)],
        kit: GameKit {
            characters: ["combat_arms", "cs_source", "hl2dm"].map(character).to_vec(),
            ..default()
        },
        ..default()
    }
}

fn app_parts(app: &mut App) {
    app.init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<PropMaterial>()
        .init_asset::<SkinnedMeshInverseBindposes>()
        .add_plugins(CaVisualsPlugin)
        .add_systems(Update, character_bodies);
    app.world_mut().spawn(FirstPersonCamera);
}

fn sim() -> Sim {
    let mut sim = Sim::new(
        CaSim,
        (
            app_parts,
            chimera_present::MapPlugin::new(floor()),
            PartyPartsPlugin,
            UltimateGunGamePlugin { seed: 3, rungs: 13, ..Default::default() },
        ),
    );
    sim.set_tick_interval(TICK_INTERVAL);
    minigame::start(sim.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    sim
}

/// The stand-ins showing and the living players without a body.
fn stand_ins(sim: &mut Sim) -> (usize, usize) {
    let w = sim.app.world_mut();
    (w.query::<&CapsuleBody>().iter(w).count(), w.query::<&BoxGun>().iter(w).count())
}

fn bodies(sim: &mut Sim, e: Entity) -> usize {
    let w = sim.app.world_mut();
    let Some(children) = w.get::<Children>(e) else { return 0 };
    let children: Vec<Entity> = children.iter().collect();
    children.iter().filter(|c| w.get::<CharacterBody>(**c).is_some()).count()
}

fn step(sim: &mut Sim, other: Entity, frames: usize, had: &mut bool, what: &str) {
    for _ in 0..frames {
        sim.ticks(1);
        assert_eq!(stand_ins(sim), (0, 0), "{what}: a stand-in shows");
        let n = bodies(sim, other);
        assert!(n <= 1, "{what}: two bodies");
        if sim.app.world().get::<Dead>(other).is_none() {
            assert!(n == 1 || !*had, "{what}: its body went away with none to replace it");
        }
        *had |= n == 1;
    }
}

#[test]
fn a_ugg_player_never_shows_a_stand_in() {
    let mut sim = sim();
    assert!(sim.app.world().contains_resource::<NoStandIns>());
    // A join's first frames: characters before the map's models are in.
    let models = sim.app.world_mut().remove_resource::<CharacterModels>().expect("the map's models");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let other = sim.spawn_character(Vec3::new(3.0, 1.0, 0.0), movement::ID);
    let world = sim.app.world_mut();
    world.entity_mut(me).insert((LocalPlayer, Team(3)));
    world.entity_mut(other).insert(Team(4));
    let mut had = false;
    step(&mut sim, other, 20, &mut had, "before the map's models");
    sim.app.world_mut().insert_resource(models);
    step(&mut sim, other, 60, &mut had, "their first frames");
    assert!(had, "the other player has its body");
    // Every rung: another game, another body (a Combat Arms rung's model
    // never finishes loading: the test's data has no install).
    let ladder = sim.app.world().resource::<Ladder>().clone();
    assert!(ladder.rungs.iter().any(|r| r.starts_with("combat_arms:")) && ladder.rungs.iter().any(|r| r.starts_with("cs_source:")));
    for rung in 0..ladder.rungs.len() {
        minigame::ladder::set_rung(sim.app.world_mut(), me, rung, &ladder);
        minigame::ladder::set_rung(sim.app.world_mut(), other, rung, &ladder);
        step(&mut sim, other, 10, &mut had, &format!("rung {rung} ({})", ladder.rungs[rung]));
    }
    // A map change's frames: the models gone and back.
    let models = sim.app.world_mut().remove_resource::<CharacterModels>().unwrap();
    step(&mut sim, other, 5, &mut had, "a map change");
    sim.app.world_mut().insert_resource(models);
    step(&mut sim, other, 5, &mut had, "after a map change");
}

/// The test above isn't passing for want of stand-ins: the same world
/// without the flag has both (the greybox app's and tests').
#[test]
fn without_the_flag_the_stand_ins_draw() {
    let mut sim = sim();
    sim.app.world_mut().remove_resource::<NoStandIns>();
    let models = sim.app.world_mut().remove_resource::<CharacterModels>().unwrap();
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    let other = sim.spawn_character(Vec3::new(3.0, 1.0, 0.0), movement::ID);
    let world = sim.app.world_mut();
    world.entity_mut(me).insert((LocalPlayer, Team(3)));
    world.entity_mut(other).insert(Team(4));
    sim.ticks(30);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let rung = ladder.rungs.iter().position(|r| r.starts_with("combat_arms:")).unwrap();
    minigame::ladder::set_rung(sim.app.world_mut(), me, rung, &ladder);
    sim.ticks(10);
    let (capsules, boxes) = stand_ins(&mut sim);
    assert!(capsules > 0, "a capsule with no models here");
    assert!(boxes > 0, "a box for the weapon with no view model");
    sim.app.world_mut().insert_resource(models);
}

/// What a rung's player holds and wears is what the plan loaded.
#[test]
fn the_ladder_plan_is_what_its_rungs_give() {
    let mut sim = sim();
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert((LocalPlayer, Team(3)));
    sim.seconds(0.5);
    let plan = sim.app.world().resource::<LadderModels>().clone();
    assert!(!plan.keys.is_empty());
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let data = sim.app.world().resource::<CaData>().clone();
    let mut checked = 0;
    for rung in 0..ladder.rungs.len() {
        if !ladder.rungs[rung].starts_with("combat_arms:") {
            continue;
        }
        minigame::ladder::set_rung(sim.app.world_mut(), me, rung, &ladder);
        sim.seconds(0.3);
        let world = sim.app.world();
        let inventory = world.get::<Inventory>(me).unwrap();
        for w in inventory.weapons.iter().filter_map(|w| world.get_entity(*w).ok()) {
            let weapon = w.get::<Weapon>().unwrap();
            if weapon.id.starts_with("combat_arms:") {
                let key = view_key(weapon.id, w.get::<CaMods>());
                assert!(plan.keys.contains(&weapon.id.to_string()), "rung {rung}: {} planned: {:?}", weapon.id, plan.keys);
                assert!(plan.keys.contains(&key), "rung {rung}: {key} planned: {:?}", plan.keys);
                checked += 1;
            }
        }
        if ladder.extras(rung).is_some_and(|x| x.value(combat_arms::game::rungs::MODEL).is_some()) {
            let look = world.get::<Selection>(me).unwrap();
            let id = data.characters().effective(look).0.id;
            assert!(plan.bodies.contains(&id), "rung {rung}: body {id} planned: {:?}", plan.bodies);
        }
    }
    assert!(checked > 0);
}

/// A network client plans the same from the server's ladder, and a host
/// with a front end (a listen server) as a client does.
#[test]
fn a_client_plans_the_servers_ladder() {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    let setup = |app: &mut App| {
        app.init_resource::<LoadOnDemand>();
        app.add_plugins((chimera_world::greybox::GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin { seed: 3, rungs: 13, ..Default::default() }));
    };
    let link = LinkConditions { latency: std::time::Duration::from_millis(20), jitter: std::time::Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(CaSim, link, 3, 1, setup);
    sim.set_frame(TICK_INTERVAL);
    minigame::start(sim.server.app.world_mut(), MINIGAME).expect("the automatic gun game starts");
    sim.until_joined(900);
    sim.ticks(120);
    let host = sim.server.app.world().resource::<LadderModels>().clone();
    let client = sim.clients[0].app.world().resource::<LadderModels>().clone();
    assert!(!host.keys.is_empty());
    assert_eq!(host, client, "the client plans the server's ladder");
}
