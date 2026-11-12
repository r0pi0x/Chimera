//! Combat Arms over the network (docs/plans/active/combat-arms.md,
//! "Network part"): a server and two clients on `NetSim`'s in-memory link
//! (chimera_net's harness) with Combat Arms' simulation on the greybox.
//!
//! - Joining: each client comes in on its own team at a free spawn point
//!   with the rifle drawn and its armour (the default join flow).
//! - Movement (walking, sprinting, jumping, crouching) is predicted with
//!   no error, and the other client sees it.
//! - Shots: the shooter's client predicts its spread, kick, back push
//!   and clip without error; the server's hit reaches both clients (health and
//!   armour replicated, the shooter's hit confirmed, the target hurt) and
//!   the target's client draws the shots.
//! - A scoped sniper (zoom, sway, the bolt's zoom-out) is predicted
//!   without error.
//! - Switching (class keys, Q) and the knife (its swing predicted, its hit
//!   the server's) are predicted without error; the knife is lag
//!   compensated: at 100 ms its hits are where the swinger saw the target
//!   (in reach on its screen: hit; out of reach there: missed).
//! - From the first state on: joining and dying are restarts (the server
//!   moves the player in, gives or drops its weapons), not errors.
//!
//! `cargo test -p combat_arms --features dev --test it net:: -- --nocapture`
//! prints the numbers.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{DamageTaken, Health, Intent, PredictedComponents, SimTick, Team};
use chimera_net::{
    harness::NetSim,
    net::{
        memory::LinkConditions,
        predict::{NetGraph, PredictionHistory},
    },
};
use chimera_sim::weapon::{
    Inventory, Magazine, ViewBackPush, Weapon, WeaponEvent, WeaponEventKind, lagcomp::LagCompStats, remote::RemoteShot,
};
use chimera_world::greybox::{self, GreyboxMapPlugin};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        inventory::{CaSelection, KNIFE, Knife, KnifePhase},
        movement::CaMovement,
        weapons::{CaKick, CaRecoil, CaSpread, CaZoom, RIFLE},

    },
    harness::CaSim,
};

/// What a world saw, gathered every frame (`watch`).
#[derive(Resource, Default)]
struct Seen {
    /// Others' rounds traced here (`RemoteShot`), by shooter.
    remote_shots: Vec<Entity>,
    /// Damage our player took, as the server reported it.
    taken: Vec<f32>,
    /// Hits confirmed to our player (`WeaponEventKind::Hit`).
    hits: Vec<(Entity, f32)>,
    /// Knife swings traced here: (swinger, what it hit, a replay).
    swings: Vec<(Entity, Option<Entity>, bool)>,
}

fn watch(
    mut shots: MessageReader<RemoteShot>,
    mut taken: MessageReader<DamageTaken>,
    mut events: MessageReader<WeaponEvent>,
    mut seen: ResMut<Seen>,
) {
    seen.remote_shots.extend(shots.read().map(|s| s.shooter));
    seen.taken.extend(taken.read().map(|t| t.damage.amount));
    for e in events.read() {
        match e.kind {
            WeaponEventKind::Hit { target, amount, .. } => seen.hits.push((target, amount)),
            WeaponEventKind::Swing { at, .. } => seen.swings.push((e.owner, at.map(|a| a.2), e.replay)),
            _ => {}
        }
    }
}

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin)
        .init_resource::<Seen>()
        .add_message::<RemoteShot>()
        .add_message::<DamageTaken>()
        .add_systems(Last, watch);
}

fn link(latency_ms: u64) -> LinkConditions {
    LinkConditions {
        latency: Duration::from_millis(latency_ms),
        jitter: Duration::ZERO,
        loss: 0.0,
    }
}

/// A server and two clients at `latency_ms`, joined, in the game with
/// their weapons drawn, and predicting.
fn joined(latency_ms: u64, seed: u64) -> NetSim {
    let mut sim = NetSim::new(CaSim, link(latency_ms), seed, 2, setup);
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    for i in 0..2 {
        let me = sim.local_player(i).unwrap();
        let world = sim.clients[i].app.world();
        let inv = world.get::<Inventory>(me).expect("predicts what it carries");
        let active = inv.active.expect("a weapon drawn");
        assert_eq!(world.get::<Weapon>(active).unwrap().id, RIFLE, "client {i} draws the rifle");
        assert!(world.get::<CaMovement>(me).is_some(), "client {i} predicts its movement");
    }
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

fn assert_no_errors(sim: &NetSim, what: &str) {
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

/// Step 30 more ticks, keeping the server's state of client `i`'s player;
/// each tick the client predicted that the server has run must be the
/// server's state, bit for bit.
fn assert_agrees(sim: &mut NetSim, i: usize, what: &str) {
    let theirs = sim.character_of(i).unwrap();
    let mut server = std::collections::HashMap::new();
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

/// The server puts client `i`'s character at `at`, still (a correction its
/// client takes), and its client looks along `yaw`, `pitch`.
fn place(sim: &mut NetSim, i: usize, at: Vec3, yaw: f32, pitch: f32) {
    let e = sim.character_of(i).unwrap();
    let world = sim.server.app.world_mut();
    world.get_mut::<Transform>(e).unwrap().translation = at;
    if let Some(mut p) = world.get_mut::<avian3d::prelude::Position>(e) {
        p.0 = at;
    }
    *client_intent(sim, i) = Intent {
        yaw,
        pitch,
        ..default()
    };
}

/// Client 0 at the north end of the floor looking south (-Z) at client 1,
/// `distance` m away, both settled and the graphs reset.
fn face_off(sim: &mut NetSim, distance: f32) {
    place(sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0, 0.0);
    place(sim, 1, Vec3::new(0.0, 1.0, 12.0 - distance), std::f32::consts::PI, 0.0);
    sim.ticks(120);
    reset_graphs(sim);
}

fn server_health(sim: &mut NetSim, i: usize) -> f32 {
    let e = sim.character_of(i).unwrap();
    sim.server.app.world().get::<Health>(e).unwrap().current
}

/// Client `i`'s copy of client `j`'s character.
fn copy_of(sim: &mut NetSim, i: usize, j: usize) -> Entity {
    let id = sim.client_id(j);
    NetSim::owned_by(&mut sim.clients[i], Some(id)).unwrap()
}

fn active_id(sim: &mut NetSim, i: usize) -> &'static str {
    let me = sim.local_player(i).unwrap();
    let world = sim.clients[i].app.world();
    world.get::<Weapon>(world.get::<Inventory>(me).unwrap().active.unwrap()).unwrap().id
}

fn server_active_id(sim: &mut NetSim, i: usize) -> &'static str {
    let e = sim.character_of(i).unwrap();
    let world = sim.server.app.world();
    world.get::<Weapon>(world.get::<Inventory>(e).unwrap().active.unwrap()).unwrap().id
}

/// Client `i` holds class key `class` (1-6) for a tick, then lets go.
fn key(sim: &mut NetSim, i: usize, class: u8) {
    client_intent(sim, i).select = Some(class - 1);
    sim.step();
    client_intent(sim, i).select = None;
    sim.step();
}

#[test]
fn players_join_on_their_own_teams_at_the_spawns() {
    let mut sim = joined(50, 1);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let world = sim.server.app.world();
    let team = |e| world.get::<Team>(e).copied();
    assert_ne!(team(a), team(b), "the smaller team each");
    let at = |e| world.get::<Transform>(e).unwrap().translation;
    for (e, spawn) in [(a, greybox::SPAWNS[0]), (b, greybox::SPAWNS[1])] {
        assert!(at(e).xz().distance(spawn.xz()) < 0.1, "at a spawn point: {} vs {spawn}", at(e));
    }
    for e in [a, b] {
        assert_eq!(world.get::<Health>(e).unwrap().current, 1.0);
        assert_eq!(world.get::<chimera_sim::weapon::Armor>(e).map(combat_arms::game::weapons::points).unwrap(), 100.0);
    }
    // Each client has its armour from the server, and the other player.
    for i in 0..2 {
        let me = sim.local_player(i).unwrap();
        assert_eq!(sim.clients[i].app.world().get::<chimera_sim::weapon::Armor>(me).map(combat_arms::game::weapons::points), Some(100.0));
        let other = copy_of(&mut sim, i, 1 - i);
        assert!(sim.clients[i].app.world().get::<Health>(other).is_some());
    }
}

#[test]
fn movement_is_predicted_without_errors() {
    for (latency, seed) in [(50, 11), (150, 12)] {
        let mut sim = joined(latency, seed);
        reset_graphs(&mut sim);
        let theirs = sim.character_of(0).unwrap();
        let start = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
        let mut top = f32::MIN;
        let mut crouched = false;
        // Walk south, sprint, jump, crouch, strafe while turning, stop.
        for k in 0..200u32 {
            let mut i = client_intent(&mut sim, 0);
            i.move_axis = match k {
                0..=59 => Vec2::Y,
                60..=119 => Vec2::new(1.0, 1.0).normalize(),
                120..=169 => Vec2::new(-1.0, 0.0),
                _ => Vec2::ZERO,
            };
            i.yaw = std::f32::consts::PI + k as f32 * 0.004;
            i.sprint = (20..=50).contains(&k);
            i.walk = i.sprint;
            i.jump = (70..=72).contains(&k);
            i.crouch = (100..=125).contains(&k);
            sim.step();
            let world = sim.server.app.world();
            top = top.max(world.get::<Transform>(theirs).unwrap().translation.y);
            crouched |= world.get::<CaMovement>(theirs).unwrap().crouch_rate > 0.5;
        }
        sim.ticks(80);
        let end = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
        assert!(end.distance(start) > 3.0, "{latency} ms: moved {} m", end.distance(start));
        assert!(top > start.y + 0.2, "{latency} ms: jumped to {top} from {}", start.y);
        assert!(crouched, "{latency} ms: crouched");
        assert_no_errors(&sim, &format!("{latency} ms, moving"));
        assert_agrees(&mut sim, 0, &format!("{latency} ms"));
        // The other client draws it where the server has it.
        let copy = copy_of(&mut sim, 1, 0);
        let drawn = sim.clients[1].app.world().get::<Transform>(copy).unwrap().translation;
        assert!(drawn.distance(end) < 0.1, "{latency} ms: client 1 draws it at {drawn}, the server has {end}");
    }
}

/// The back push (`ViewBackPush`) is the shooter's client's own: worked
/// out in its predicted ticks (saved and replayed with them, never sent),
/// so its camera goes back with each shot at once, with no prediction
/// error, and comes back. The rifle (no push of its own) is given the
/// record defaults' push (0.5 a shot, a return of 15 a second) on both
/// sides.
#[test]
fn back_push_is_predicted_by_the_shooter() {
    let mut sim = joined(100, 23);
    face_off(&mut sim, 6.0);
    let me = sim.local_player(0).unwrap();
    let theirs = sim.character_of(0).unwrap();
    for (world, who) in [(sim.clients[0].app.world_mut(), me), (sim.server.app.world_mut(), theirs)] {
        let rifle = world.get::<Inventory>(who).unwrap().active.unwrap();
        let mut r = world.get_mut::<CaRecoil>(rifle).unwrap();
        r.params.back_jitter = 0.5;
        r.params.back_jitter_recover = 3.0;
    }
    client_intent(&mut sim, 0).pitch = 0.3;
    client_intent(&mut sim, 0).fire = true;
    let mut most = 0.0f32;
    for _ in 0..40 {
        sim.step();
        let push = sim.clients[0].app.world().get::<ViewBackPush>(me).map_or(0.0, |b| b.0);
        most = most.max(push);
    }
    client_intent(&mut sim, 0).fire = false;
    // 0.5 a shot is 5 u, 0.09 m; it returns within a few ticks.
    assert!(most > 0.05 && most <= 0.0901, "the camera went back {most} m");
    sim.ticks(60);
    let push = sim.clients[0].app.world().get::<ViewBackPush>(me).map_or(0.0, |b| b.0);
    assert_eq!(push, 0.0, "and came back");
    assert_no_errors(&sim, "back push");
}

#[test]
fn shots_and_damage_are_seen_by_both() {
    let mut sim = joined(100, 21);
    face_off(&mut sim, 6.0);
    let shooter = sim.local_player(0).unwrap();
    let rifle = sim.clients[0].app.world().get::<Inventory>(shooter).unwrap().active.unwrap();
    let clip = sim.clients[0].app.world().get::<Magazine>(rifle).unwrap().clip;
    let before = server_health(&mut sim, 1);
    // Three rounds held on the target's chest.
    client_intent(&mut sim, 0).pitch = -0.08;
    client_intent(&mut sim, 0).fire = true;
    sim.until(200, |s| {
        let me = s.local_player(0).unwrap();
        let w = s.clients[0].app.world();
        w.get::<Magazine>(w.get::<Inventory>(me).unwrap().active.unwrap()).unwrap().clip <= clip - 3
    });
    client_intent(&mut sim, 0).fire = false;
    // The shooter's client predicted its spread and kick.
    {
        let me = sim.local_player(0).unwrap();
        let world = sim.clients[0].app.world();
        assert!(world.get::<CaSpread>(rifle).unwrap().fire > 0.0, "the firing meter grew");
        assert!(world.get::<CaKick>(me).unwrap().pitch > 0.0, "the view kicked");
    }
    sim.ticks(60);
    assert_no_errors(&sim, "shooting");
    assert_agrees(&mut sim, 0, "shooting");
    // The server's damage, and everyone sees it.
    let after = server_health(&mut sim, 1);
    println!("health {before} -> {after}");
    let rewinds = sim.server.app.world().resource::<LagCompStats>().rewinds;
    assert!(rewinds >= 3, "the server traced the rounds where client 0 saw client 1: {rewinds} rewinds");
    assert!(after < before, "the server hurt client 1");
    let target = sim.character_of(1).unwrap();
    let target_armour = sim.server.app.world().get::<chimera_sim::weapon::Armor>(target).map(combat_arms::game::weapons::points).unwrap();
    assert!(target_armour < 100.0, "armour took its share: {target_armour}");
    let me1 = sim.local_player(1).unwrap();
    let copy1 = copy_of(&mut sim, 0, 1);
    let w1 = sim.clients[1].app.world();
    assert_eq!(w1.get::<Health>(me1).unwrap().current, after, "client 1's own health");
    assert_eq!(w1.get::<chimera_sim::weapon::Armor>(me1).map(combat_arms::game::weapons::points).unwrap(), target_armour, "client 1's own armour");
    let seen1 = w1.resource::<Seen>();
    assert!(!seen1.taken.is_empty(), "client 1 heard it was hurt");
    assert!(
        seen1.remote_shots.len() >= 3,
        "client 1 drew the rounds: {}",
        seen1.remote_shots.len()
    );
    let w0 = sim.clients[0].app.world();
    assert_eq!(w0.get::<Health>(copy1).unwrap().current, after, "client 0 sees the damage");
    let seen0 = w0.resource::<Seen>();
    assert!(seen0.hits.iter().any(|(t, _)| *t == copy1), "client 0's hits confirmed: {:?}", seen0.hits);
    assert!(seen0.remote_shots.is_empty(), "the shooter predicted its own rounds");
}

#[test]
fn switching_weapons_is_predicted() {
    let mut sim = joined(100, 31);
    reset_graphs(&mut sim);
    key(&mut sim, 0, 3);
    sim.ticks(60);
    assert_eq!(active_id(&mut sim, 0), KNIFE, "the knife drawn at once");
    assert_eq!(server_active_id(&mut sim, 0), KNIFE);
    // Q: back to the rifle.
    client_intent(&mut sim, 0).last_weapon = true;
    sim.step();
    client_intent(&mut sim, 0).last_weapon = false;
    sim.ticks(60);
    assert_eq!(active_id(&mut sim, 0), RIFLE);
    assert_eq!(server_active_id(&mut sim, 0), RIFLE);
    let me = sim.local_player(0).unwrap();
    let sel = sim.clients[0].app.world().get::<CaSelection>(me).unwrap().clone();
    let knife = sim.clients[0].app.world().get::<Inventory>(me).unwrap().weapons[1];
    assert_eq!(sel.previous, Some(knife), "Q's previous weapon, as a weapon of ours");
    // And key 1 again, then a quick pair.
    key(&mut sim, 0, 3);
    key(&mut sim, 0, 1);
    sim.ticks(80);
    assert_eq!(active_id(&mut sim, 0), RIFLE);
    assert_no_errors(&sim, "switching");
    assert_agrees(&mut sim, 0, "switching");
    // The other client sees what it holds.
    let copy = copy_of(&mut sim, 1, 0);
    let held = sim.clients[1].app.world().get::<chimera_net::net::NetHeld>(copy).cloned();
    assert!(held.is_some_and(|h| h.weapon.is_some()), "client 1 knows what client 0 holds");
}

#[test]
fn a_scoped_bolt_shot_is_predicted() {
    // weapon_classes.md 7: the zoom (`CaZoom`), its view (`Zoomed`), the
    // sway on the view punch and the bolt's zoom-out, predicted.
    let mut sim = joined(100, 51);
    face_off(&mut sim, 6.0);
    let e = sim.character_of(0).unwrap();
    chimera_sim::weapon::give(sim.server.app.world_mut(), e, "combat_arms:l96a1").unwrap();
    sim.ticks(150);
    assert_eq!(active_id(&mut sim, 0), "combat_arms:l96a1");
    reset_graphs(&mut sim);
    client_intent(&mut sim, 0).secondary = true;
    sim.step();
    client_intent(&mut sim, 0).secondary = false;
    sim.ticks(30);
    let me = sim.local_player(0).unwrap();
    assert!(
        sim.clients[0].app.world().get::<chimera_sim::weapon::Zoomed>(me).is_some(),
        "the client zooms"
    );
    // On client 1's chest: the bolt kills it, and its client takes its
    // death (the primary dropped) as a restart, not an error.
    client_intent(&mut sim, 0).pitch = -0.08;
    client_intent(&mut sim, 0).fire = true;
    sim.step();
    client_intent(&mut sim, 0).fire = false;
    sim.ticks(120);
    assert_eq!(server_health(&mut sim, 1), 0.0, "client 1 killed");
    let w = sim.clients[0].app.world().get::<Inventory>(me).unwrap().active.unwrap();
    let z = sim.clients[0].app.world().get::<CaZoom>(w).unwrap().clone();
    assert_eq!(z.level, 0, "the bolt's zoom-out");
    assert_no_errors(&sim, "scoped");
    assert_agrees(&mut sim, 0, "scoped");
}

#[test]
fn a_knife_hit_is_predicted_and_the_servers() {
    let mut sim = joined(100, 41);
    face_off(&mut sim, 0.8);
    key(&mut sim, 0, 3);
    sim.ticks(60);
    assert_eq!(active_id(&mut sim, 0), KNIFE);
    reset_graphs(&mut sim);
    let before = server_health(&mut sim, 1);
    client_intent(&mut sim, 0).pitch = -0.1;
    client_intent(&mut sim, 0).fire = true;
    // The swing starts on the client at once.
    let mut swinging = false;
    for _ in 0..20 {
        sim.step();
        let me = sim.local_player(0).unwrap();
        let w = sim.clients[0].app.world();
        let k = w.get::<Inventory>(me).unwrap().active.unwrap();
        swinging |= w.get::<Knife>(k).unwrap().state.phase != KnifePhase::Idle;
    }
    client_intent(&mut sim, 0).fire = false;
    assert!(swinging, "the client predicts the swing");
    sim.ticks(120);
    let after = server_health(&mut sim, 1);
    println!("knife: health {before} -> {after}");
    assert!(after < before, "the server's knife hit");
    let me1 = sim.local_player(1).unwrap();
    assert_eq!(sim.clients[1].app.world().get::<Health>(me1).unwrap().current, after);
    assert!(!sim.clients[1].app.world().resource::<Seen>().taken.is_empty(), "client 1 hurt");
    assert_no_errors(&sim, "knifing");
    assert_agrees(&mut sim, 0, "knifing");
}

/// Yaw and pitch that look from `eye` at `at`.
fn look_at(eye: Vec3, at: Vec3) -> (f32, f32) {
    let d = at - eye;
    ((-d.x).atan2(-d.z).rem_euclid(std::f32::consts::TAU), d.y.atan2(d.xz().length()))
}

/// What one knife run saw: each hit event of client 0's swings, in order,
/// as client 0 traced it on its screen and as the server did.
#[derive(Debug, Default)]
struct KnifeRun {
    /// (client 0's own trace met client 1, the server's met client 1)
    events: Vec<(bool, bool)>,
}

impl KnifeRun {
    fn count(&self, client: bool, server: bool) -> usize {
        self.events.iter().filter(|e| **e == (client, server)).count()
    }
}

/// Client 0 (100 ms each way) stands at the north end of the floor with
/// the knife, aiming at client 1's body as it draws it and holding fire,
/// while client 1 starts at `start` facing it and runs along `axis` for
/// `ticks` ticks (`sv_unlag` as `unlag`; client 1 can't die of it).
fn knife_run(unlag: bool, start: Vec3, axis: Vec2, ticks: u32) -> KnifeRun {
    let mut sim = joined(100, 47);
    sim.server.app.world_mut().resource_mut::<chimera_sim::weapon::lagcomp::LagCompSettings>().unlag = unlag as u8;
    let (shooter, target) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    sim.server.app.world_mut().entity_mut(target).insert(chimera_core::core::God);
    key(&mut sim, 0, 3);
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0, 0.0);
    place(&mut sim, 1, start, std::f32::consts::PI, 0.0);
    sim.ticks(90);
    assert_eq!(active_id(&mut sim, 0), KNIFE);
    sim.server.app.world_mut().resource_mut::<Seen>().swings.clear();
    sim.clients[0].app.world_mut().resource_mut::<Seen>().swings.clear();
    let (me, seen) = (sim.local_player(0).unwrap(), copy_of(&mut sim, 0, 1));
    for _ in 0..ticks {
        client_intent(&mut sim, 1).move_axis = axis;
        let w = sim.clients[0].app.world();
        let eye = w.get::<Transform>(me).unwrap().translation
            + w.get::<chimera_core::core::MovementState>(me).unwrap().eye_offset;
        let (yaw, pitch) = look_at(eye, w.get::<Transform>(seen).unwrap().translation);
        let mut i = client_intent(&mut sim, 0);
        (i.yaw, i.pitch, i.fire) = (yaw, pitch, true);
        sim.step();
    }
    client_intent(&mut sim, 0).fire = false;
    client_intent(&mut sim, 1).move_axis = Vec2::ZERO;
    sim.ticks(90);
    let mine: Vec<bool> = sim.clients[0].app.world().resource::<Seen>().swings.iter()
        .filter(|(o, _, replay)| *o == me && !replay)
        .map(|(_, hit, _)| *hit == Some(seen))
        .collect();
    let servers: Vec<bool> = sim.server.app.world().resource::<Seen>().swings.iter()
        .filter(|(o, ..)| *o == shooter)
        .map(|(_, hit, _)| *hit == Some(target))
        .collect();
    assert_eq!(mine.len(), servers.len(), "the server ran each hit event the client predicted");
    assert!(!mine.is_empty(), "client 0 swung");
    KnifeRun {
        events: mine.into_iter().zip(servers).collect(),
    }
}

/// Combat Arms' knife is lag compensated (inventory.md 8.3, the server
/// re-checks the target the client hit; specs/cs_source/weapons.md 4.6):
/// at 100 ms, client 1 running across client 0's reach is hit by the
/// server wherever client 0 hit it on its screen (and missed without
/// compensation); running at client 0 from beyond reach, the server hits
/// it only once client 0's screen has it in reach (without compensation,
/// before).
#[test]
fn knife_swings_hit_where_the_swinger_saw_the_target() {
    let print = |what: &str, unlag: bool, r: &KnifeRun| {
        println!(
            "{what}, sv_unlag {}: {} hit events; both hit {}, screen only {}, server only {}, neither {}",
            unlag as u8,
            r.events.len(),
            r.count(true, true),
            r.count(true, false),
            r.count(false, true),
            r.count(false, false)
        )
    };
    // Across: 1.2 m ahead, running sideways.
    let across = |unlag| knife_run(unlag, Vec3::new(-3.0, 1.0, 10.8), Vec2::new(-1.0, 0.0), 110);
    let (on, off) = (across(true), across(false));
    print("across", true, &on);
    print("across", false, &off);
    assert!(on.count(true, true) >= 2, "swings in reach on the screen hit ({on:?})");
    assert_eq!(on.count(true, false) + on.count(false, true), 0, "the server agrees with the screen ({on:?})");
    assert!(off.count(true, false) >= 2, "without compensation those miss ({off:?})");
    // Towards: from 5 m, running at client 0.
    let towards = |unlag| knife_run(unlag, Vec3::new(0.0, 1.0, 7.0), Vec2::new(0.0, 1.0), 110);
    let (on, off) = (towards(true), towards(false));
    print("towards", true, &on);
    print("towards", false, &off);
    assert!(on.count(false, false) >= 1, "swings out of reach on the screen ({on:?})");
    assert_eq!(on.count(false, true), 0, "out of reach on the screen: missed ({on:?})");
    assert!(off.count(false, true) >= 1, "without compensation the server hit what the screen had out of reach ({off:?})");
}

/// From the very first state, no prediction error (the counters never
/// reset): joining (the server lets the player in at a spawn point with
/// its loadout: a restart, `OwnState::alive` changing), idling, walking
/// and switching weapons.
#[test]
fn no_errors_from_joining_on() {
    let mut sim = NetSim::new(CaSim, link(50), 52, 2, setup);
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    let check = |sim: &NetSim, what: &str| {
        for i in 0..2 {
            let g = graph(sim, i);
            assert_eq!(
                g.errors, 0,
                "{what}: client {i} mispredicted tick {} ({:?})",
                g.last_error_tick, g.last_differing
            );
        }
    };
    for _ in 0..240 {
        sim.step();
        check(&sim, "idle");
    }
    for k in 0..120u32 {
        let mut i = client_intent(&mut sim, 0);
        i.move_axis = if k < 80 { Vec2::Y } else { Vec2::ZERO };
        i.select = match k {
            20 => Some(2),
            60 => Some(0),
            _ => None,
        };
        sim.step();
        check(&sim, "walking and switching");
    }
    sim.ticks(60);
    assert_eq!(active_id(&mut sim, 0), RIFLE);
    assert_eq!(server_active_id(&mut sim, 0), RIFLE);
    assert_no_errors(&sim, "joined, idle, walking and switching");
    for i in 0..2 {
        assert!(graph(&sim, i).resyncs >= 1, "client {i}: the join is a restart");
    }
}
