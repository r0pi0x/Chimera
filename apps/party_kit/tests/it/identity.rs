//! Each player is the game of the gun in hand (the gun game's `identity =
//! "weapon"`, `chimera_sim::identity`): a CS:S gun moves and looks as
//! CS:S (its body among the map's CS:S models), a Combat Arms gun or the
//! knife as Combat Arms. Headless on the party's simulation (scripted
//! soldiers, the rungs' swaps and the knife; two stand-in bodies, one per
//! game, as the map's models), and over the network (`NetSim`, two
//! clients): the switch is predicted on the owning client's own tick with
//! no prediction error, and the other client sees the new body.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    core::{Intent, PredictedComponents, SimTick},
    slots::{FollowsWeapon, GameIdentity, MovementSlot},
};
use chimera_net::{
    harness::NetSim,
    net::{
        memory::LinkConditions,
        predict::{NetGraph, PredictionHistory},
    },
};
use chimera_sim::{
    minigame::{
        self,
        ladder::{self, Ladder, Rung},
    },
    testkit,
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{BodyModel, BodyName, CharacterModels, MapCharacterModel, MapSimPlugin},
};
use combat_arms::{
    game::{TICK_INTERVAL, movement as ca_movement},
    harness::CaSim,
};
use cs_source::game::{movement as cs_movement, weapons as cs_weapons};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

use crate::gun_game::{held, line_up, party, rung, shoot_until_dead, soldier};

/// `party` with the map's systems (which give characters their bodies)
/// and `two_bodies`.
fn party_with_bodies() -> Sim {
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin));
    two_bodies(&mut sim.app);
    crate::gun_game::plain_rungs(sim.app.world_mut());
    minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim
}

/// The map's bodies: one of Combat Arms' (0) and one of CS:S's (1),
/// stand-ins with no mesh.
pub(crate) fn two_bodies(app: &mut App) {
    let body = |game| MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    };
    app.insert_resource(CharacterModels(std::sync::Arc::new(vec![body("combat_arms"), body("cs_source")])));
}

/// A Combat Arms gun given besides the rung's (a pickup, `give`).
const M4: &str = "combat_arms:m4a1";

const CA_BODY: usize = 0;
const CS_BODY: usize = 1;

fn body(world: &World, e: Entity) -> Option<usize> {
    world.get::<BodyModel>(e).map(|b| b.0)
}

fn movement(world: &World, e: Entity) -> Option<&'static str> {
    world.get::<MovementSlot>(e).map(|m| m.0)
}

fn identity(world: &World, e: Entity) -> Option<String> {
    world.get::<GameIdentity>(e).map(|i| i.0.clone())
}

/// `e`'s movement and identity are `game`'s.
fn assert_is(world: &World, e: Entity, game: &str, what: &str) {
    let want = if game == "cs_source" { cs_movement::ID } else { ca_movement::ID };
    assert_eq!(movement(world, e), Some(want), "{what}: {game}'s movement");
    assert_eq!(identity(world, e).as_deref(), Some(game), "{what}: {game}'s identity");
}

/// A rung-up's swap (CS:S's Glock to Combat Arms' Beretta) makes the
/// killer Combat Arms'; the next (the Desert Eagle) CS:S's again; its
/// knife is CS:S's too (the gun's game's); a Combat Arms gun given (a
/// pickup, `give`) Combat Arms', and the Desert Eagle back CS:S's.
#[test]
fn movement_follows_the_gun_on_a_rung_up_and_back() {
    let mut sim = party();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    assert!(sim.app.world().get::<FollowsWeapon>(a).is_some(), "the gun game's players follow their gun");
    assert_eq!(held(&sim, a), Some(cs_weapons::GLOCK));
    assert_is(sim.app.world(), a, "cs_source", "the Glock");
    // A kill: the Beretta, Combat Arms'.
    line_up(&mut sim, a, b, 4.0);
    assert!(shoot_until_dead(&mut sim, a, b, 8.0), "the Glock kills");
    sim.ticks(2);
    assert_eq!(rung(&sim, a), 1);
    assert_eq!(held(&sim, a), Some("combat_arms:beretta_m92fs"));
    assert_is(sim.app.world(), a, "combat_arms", "the Beretta");
    // The victim comes back CS:S's, with its Glock.
    line_up(&mut sim, a, b, 4.0);
    assert_is(sim.app.world(), b, "cs_source", "b respawned with the Glock");
    // Another kill: the Desert Eagle, CS:S's again.
    assert!(shoot_until_dead(&mut sim, a, b, 8.0), "the Beretta kills");
    sim.ticks(2);
    assert_eq!(held(&sim, a), Some(cs_weapons::DEAGLE));
    assert_is(sim.app.world(), a, "cs_source", "the Desert Eagle");
    // The knife: the Desert Eagle's game's, CS:S's.
    sim.intent(a).select = Some(2);
    sim.ticks(1);
    sim.intent(a).select = None;
    sim.seconds(0.5);
    assert_eq!(held(&sim, a), Some(cs_weapons::KNIFE));
    assert_is(sim.app.world(), a, "cs_source", "the Desert Eagle's knife");
    sim.intent(a).last_weapon = true;
    sim.ticks(1);
    sim.intent(a).last_weapon = false;
    sim.seconds(0.5);
    assert_eq!(held(&sim, a), Some(cs_weapons::DEAGLE));
    // Combat Arms' M4A1 given: Combat Arms'; class key 2 back to the
    // Desert Eagle: CS:S's.
    chimera_sim::weapon::give(sim.app.world_mut(), a, M4);
    sim.ticks(2);
    assert_eq!(held(&sim, a), Some(M4));
    assert_is(sim.app.world(), a, "combat_arms", "the M4A1");
    sim.intent(a).select = Some(1);
    sim.ticks(1);
    sim.intent(a).select = None;
    sim.ticks(2);
    assert_eq!(held(&sim, a), Some(cs_weapons::DEAGLE));
    assert_is(sim.app.world(), a, "cs_source", "back to the Desert Eagle");
    // It still walks: CS:S's movement moves it on the greybox's floor.
    let from = sim.position(a);
    sim.intent(a).move_axis = Vec2::Y;
    sim.seconds(0.5);
    sim.intent(a).move_axis = Vec2::ZERO;
    assert!(sim.position(a).distance(from) > 1.0, "walked as CS:S: {from} -> {}", sim.position(a));
}

/// The body follows too: CS:S's model with a CS:S gun, Combat Arms' with
/// the Beretta and the knife, CS:S's again with the Desert Eagle.
#[test]
fn the_body_follows_the_gun() {
    let mut sim = party_with_bodies();
    let a = soldier(&mut sim);
    let b = soldier(&mut sim);
    sim.seconds(1.5);
    assert_eq!(body(sim.app.world(), a), Some(CS_BODY), "the Glock: CS:S's body");
    assert!(sim.app.world().get::<BodyName>(a).is_none(), "no Combat Arms look");
    line_up(&mut sim, a, b, 4.0);
    assert!(shoot_until_dead(&mut sim, a, b, 8.0));
    sim.ticks(3);
    assert_eq!(held(&sim, a), Some("combat_arms:beretta_m92fs"));
    assert_eq!(body(sim.app.world(), a), Some(CA_BODY), "the Beretta: Combat Arms' body");
    line_up(&mut sim, a, b, 4.0);
    assert_eq!(body(sim.app.world(), b), Some(CS_BODY), "b back with its Glock: CS:S's");
    assert!(shoot_until_dead(&mut sim, a, b, 8.0));
    sim.ticks(3);
    assert_eq!(held(&sim, a), Some(cs_weapons::DEAGLE));
    assert_eq!(body(sim.app.world(), a), Some(CS_BODY), "the Desert Eagle: CS:S's again");
    chimera_sim::weapon::give(sim.app.world_mut(), a, M4);
    sim.ticks(3);
    assert_eq!(held(&sim, a), Some(M4));
    assert_eq!(body(sim.app.world(), a), Some(CA_BODY), "the M4A1: Combat Arms'");
}

/// A minigame without the option leaves movement to its `movement` slot.
#[test]
fn without_the_option_the_slots_movement_stays() {
    let mut sim = party();
    let text = originals::MINIGAMES.iter().find(|(f, _)| f.contains("gun_game_dust2")).unwrap().1;
    let text = text.replace("identity = \"weapon\"\n", "").replace("originals:gun_game_dust2", "test:gun_game_same");
    chimera_sim::minigame::MinigameAppExt::add_minigames(&mut sim.app, &[("same.toml", &text)]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_same").unwrap();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    assert_eq!(held(&sim, a), Some(cs_weapons::GLOCK));
    assert_eq!(movement(sim.app.world(), a), Some(ca_movement::ID), "the slot's");
    assert!(sim.app.world().get::<FollowsWeapon>(a).is_none() && identity(sim.app.world(), a).is_none());
}

// Over the network.

fn link(latency_ms: u64) -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(latency_ms),
        jitter: Duration::ZERO,
        loss: 0.0,
    }
}

/// The party's server (the gun game started) and two clients at
/// `latency_ms`, joined and in the game.
pub(crate) fn party_net(latency_ms: u64, seed: u64) -> NetSim {
    let mut sim = NetSim::new(CaSim, link(latency_ms), seed, 2, |app| {
        app.add_plugins((GreyboxMapPlugin, MapSimPlugin::empty(), PartyPartsPlugin));
        two_bodies(app);
    });
    crate::gun_game::plain_rungs(sim.server.app.world_mut());
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(300);
    sim
}

fn graph(sim: &NetSim, i: usize) -> NetGraph {
    sim.clients[i].app.world().resource::<NetGraph>().clone()
}

fn reset_graphs(sim: &mut NetSim) {
    for c in &mut sim.clients {
        *c.app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    }
}

fn client_intent(sim: &mut NetSim, i: usize) -> Mut<'_, Intent> {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap()
}

/// Client `i`'s copy of client `j`'s character.
fn copy_of(sim: &mut NetSim, i: usize, j: usize) -> Entity {
    let id = sim.client_id(j);
    NetSim::owned_by(&mut sim.clients[i], Some(id)).unwrap()
}

fn client_held(sim: &mut NetSim, i: usize) -> Option<&'static str> {
    let me = sim.local_player(i).unwrap();
    testkit::active_id(sim.clients[i].app.world(), me)
}

/// The server's character of client `i` and its own copy both move as
/// `game` does.
fn assert_moves_as(sim: &mut NetSim, i: usize, game: &str, what: &str) {
    let want = if game == "cs_source" { cs_movement::ID } else { ca_movement::ID };
    let theirs = sim.character_of(i).unwrap();
    let me = sim.local_player(i).unwrap();
    assert_eq!(movement(sim.server.app.world(), theirs), Some(want), "{what}: the server's");
    assert_eq!(movement(sim.clients[i].app.world(), me), Some(want), "{what}: client {i}'s prediction");
}

pub(crate) fn assert_no_errors(sim: &NetSim, what: &str) {
    for i in 0..sim.clients.len() {
        let g = graph(sim, i);
        println!(
            "{what}, client {i}: {} states checked, {} errors (last differing: {:?}), {} resyncs",
            g.checked, g.errors, g.last_differing, g.resyncs
        );
        assert!(g.checked > 0, "{what}: client {i} compared no state");
        assert_eq!(g.errors, 0, "{what}: client {i} mispredicted ({:?})", g.last_differing);
    }
}

/// 30 more ticks: every tick client `i` predicted that the server ran is
/// the server's state, bit for bit.
fn assert_agrees(sim: &mut NetSim, i: usize, what: &str) {
    let theirs = sim.character_of(i).unwrap();
    let mut server = std::collections::BTreeMap::new();
    for _ in 0..30 {
        sim.step();
        let world = sim.server.app.world();
        let tick = world.resource::<SimTick>().0;
        server.insert(tick, world.resource::<PredictedComponents>().encode(world, theirs));
    }
    let history = &sim.clients[i].app.world().resource::<PredictionHistory>().0;
    let compared = history
        .iter()
        .filter(|p| !p.state.is_empty())
        .filter_map(|p| server.get(&p.tick).map(|s| (p, s)))
        .inspect(|(p, s)| assert!(**s == p.state, "{what}: the prediction for tick {} isn't the server's", p.tick))
        .count();
    assert!(compared > 0, "{what}: no predicted tick to compare");
}

/// Client 0's own switches between a Combat Arms gun the server gave it
/// (the M4A1) and its Glock (Q, then class key 1, standing and crouched),
/// then the server's rung-ups (the Beretta, Combat Arms'; the Desert
/// Eagle, CS:S's): its movement switches on its own predicted tick, as
/// the server's does, seated on the same feet, with no prediction error
/// while it walks; client 1 sees its body change with each.
#[test]
fn a_switch_is_predicted_without_error() {
    let mut sim = party_net(80, 7);
    assert_eq!(client_held(&mut sim, 0), Some(cs_weapons::GLOCK), "the first rung's Glock");
    assert_moves_as(&mut sim, 0, "cs_source", "the Glock");
    let me = sim.local_player(0).unwrap();
    assert!(sim.clients[0].app.world().get::<FollowsWeapon>(me).is_some(), "the client knows its player follows");
    reset_graphs(&mut sim);
    let walk = |sim: &mut NetSim, n: u32| {
        for k in 0..n {
            let mut i = client_intent(sim, 0);
            i.move_axis = if (k / 20) % 2 == 0 { Vec2::Y } else { -Vec2::Y };
            i.yaw = 0.3 + k as f32 * 0.01;
            sim.step();
        }
        client_intent(sim, 0).move_axis = Vec2::ZERO;
    };
    walk(&mut sim, 30);
    // The knife: class key 3, walking on: the Glock's game's, CS:S's.
    client_intent(&mut sim, 0).select = Some(2);
    sim.step();
    client_intent(&mut sim, 0).select = None;
    walk(&mut sim, 60);
    assert_eq!(client_held(&mut sim, 0), Some(cs_weapons::KNIFE));
    assert_moves_as(&mut sim, 0, "cs_source", "the knife");
    // The server gives it Combat Arms' M4A1 (drawn: Combat Arms').
    let theirs = sim.character_of(0).unwrap();
    chimera_sim::weapon::give(sim.server.app.world_mut(), theirs, M4);
    chimera_core::core::rebase(sim.server.app.world_mut(), theirs);
    walk(&mut sim, 60);
    assert_eq!(client_held(&mut sim, 0), Some(M4));
    assert_moves_as(&mut sim, 0, "combat_arms", "the M4A1");
    reset_graphs(&mut sim);
    for crouch in [false, true] {
        client_intent(&mut sim, 0).crouch = crouch;
        walk(&mut sim, 40);
        // Q: the knife, the slot drawn before (by slot, not by weapon),
        // CS:S's; class key 1: the M4A1, Combat Arms'.
        client_intent(&mut sim, 0).last_weapon = true;
        sim.step();
        client_intent(&mut sim, 0).last_weapon = false;
        walk(&mut sim, 60);
        assert_eq!(client_held(&mut sim, 0), Some(cs_weapons::KNIFE));
        assert_moves_as(&mut sim, 0, "cs_source", "the knife again");
        client_intent(&mut sim, 0).select = Some(0);
        sim.step();
        client_intent(&mut sim, 0).select = None;
        walk(&mut sim, 60);
        assert_eq!(client_held(&mut sim, 0), Some(M4));
        assert_moves_as(&mut sim, 0, "combat_arms", "the M4A1 again");
    }
    client_intent(&mut sim, 0).crouch = false;
    walk(&mut sim, 40);
    assert_no_errors(&sim, "the client's own switches, standing and crouched");
    // Q: the knife.
    client_intent(&mut sim, 0).last_weapon = true;
    sim.step();
    client_intent(&mut sim, 0).last_weapon = false;
    walk(&mut sim, 60);
    assert_eq!(client_held(&mut sim, 0), Some(cs_weapons::KNIFE));
    assert_moves_as(&mut sim, 0, "cs_source", "the knife again");
    // The server's rung-ups, while it walks.
    // The other client sees CS:S's body on it.
    let copy = copy_of(&mut sim, 1, 0);
    assert_eq!(body(sim.clients[1].app.world(), copy), Some(CS_BODY), "client 1 sees the Glock's body");
    for (rung, gun, game, model) in [
        (1, "combat_arms:beretta_m92fs", "combat_arms", CA_BODY),
        (2, cs_weapons::DEAGLE, "cs_source", CS_BODY),
    ] {
        let theirs = sim.character_of(0).unwrap();
        let w = sim.server.app.world_mut();
        let l = w.resource::<Ladder>().clone();
        ladder::set_rung(w, theirs, rung, &l);
        walk(&mut sim, 80);
        assert_eq!(sim.server.app.world().get::<Rung>(theirs).map(|r| r.0), Some(rung));
        assert_eq!(client_held(&mut sim, 0), Some(gun), "rung {rung}");
        assert_moves_as(&mut sim, 0, game, gun);
        let w1 = sim.clients[1].app.world();
        assert_eq!(identity(w1, copy).as_deref(), Some(game), "client 1 hears {gun}'s identity");
        assert_eq!(body(w1, copy), Some(model), "client 1 sees {gun}'s body");
        assert_eq!(body(sim.server.app.world(), theirs), Some(model), "the server's body for {gun}");
    }
    // Q goes by slot, over the wire: after a rung-up (the knife swapped to
    // the new gun's game's too) it is the knife, then the gun again.
    for (rung, gun, knife) in [(3, "combat_arms:mp5a4", "combat_arms:m9_knife"), (4, cs_weapons::P90, cs_weapons::KNIFE)] {
        let theirs = sim.character_of(0).unwrap();
        let w = sim.server.app.world_mut();
        let l = w.resource::<Ladder>().clone();
        ladder::set_rung(w, theirs, rung, &l);
        walk(&mut sim, 80);
        assert_eq!(client_held(&mut sim, 0), Some(gun), "rung {rung}");
        for want in [knife, gun, knife] {
            client_intent(&mut sim, 0).last_weapon = true;
            sim.step();
            client_intent(&mut sim, 0).last_weapon = false;
            walk(&mut sim, 60);
            assert_eq!(client_held(&mut sim, 0), Some(want), "rung {rung}: Q");
        }
        // The knife in hand: the next rung's gun is drawn.
    }
    sim.ticks(60);
    assert_no_errors(&sim, "switches");
    assert_agrees(&mut sim, 0, "switches");
    assert!(graph(&sim, 0).resyncs >= 2, "the server's swaps were restarts: {:?}", graph(&sim, 0));
}

/// Across the three games: HL2:DM's pistol makes the player HL2:DM's
/// (its movement and identity), CS:S's Glock CS:S's, Combat Arms'
/// Beretta Combat Arms', and HL2:DM's SMG1 HL2:DM's again, each on the
/// rung's swap (`hl2dm_parts::LADDER`).
#[test]
fn identity_follows_the_gun_across_three_games() {
    use crate::hl2dm_parts::{LADDER, mixed_text};
    let mut sim = Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin));
    crate::gun_game::no_finale(sim.app.world_mut());
    chimera_sim::minigame::MinigameAppExt::add_minigames(&mut sim.app, &[("three.toml", &mixed_text(false))]).unwrap();
    minigame::start(sim.app.world_mut(), "test:gun_game_hl2dm").unwrap();
    let a = soldier(&mut sim);
    let _b = soldier(&mut sim);
    sim.seconds(1.5);
    let l = sim.app.world().resource::<Ladder>().clone();
    for (r, want) in [(0, "hl2dm"), (1, "cs_source"), (3, "combat_arms"), (2, "hl2dm")] {
        ladder::set_rung(sim.app.world_mut(), a, r, &l);
        sim.seconds(1.2);
        assert_eq!(held(&sim, a), Some(LADDER[r]));
        let w = sim.app.world();
        assert_eq!(identity(w, a).as_deref(), Some(want), "rung {r}");
        let wanted = match want {
            "hl2dm" => hl2dm::game::movement::ID,
            "cs_source" => cs_movement::ID,
            _ => ca_movement::ID,
        };
        assert_eq!(movement(w, a), Some(wanted), "rung {r}");
    }
}
