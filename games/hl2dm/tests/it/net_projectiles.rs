//! HL2:DM's projectiles over the network (`NetSim` on `Hl2Sim`, the
//! greybox; specs/hl2dm/projectiles.md 12): a client's player fires each
//! projectile weapon (the SMG1's grenade, the AR2's ball, the RPG, the
//! crossbow, the frag, the S.L.A.M.'s satchel and trip mine) with its own
//! buttons; the server makes the projectile (none is predicted), the
//! client draws it from its `NetItem` with the projectile's model key, it
//! moves there, and the weapon's own rounds the client predicted are the
//! server's.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::Intent;
use chimera_net::{
    harness::NetSim,
    net::{NetItem, memory::LinkConditions},
};
use chimera_sim::weapon::{Inventory, Magazine, Weapon, give};
use chimera_world::greybox::GreyboxMapPlugin;
use hl2dm::{
    game::weapons::{
        self,
        launch::{self, Hl2Rounds, U},
    },
    harness::Hl2Sim,
};

fn joined() -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(Hl2Sim, link, 37, 1, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.until_joined(600);
    sim.ticks(200);
    sim
}

fn client_intent(sim: &mut NetSim) -> Mut<'_, Intent> {
    let me = sim.local_player(0).unwrap();
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap()
}

/// The server's player holding `id` with `rounds` of its own (or a loaded
/// magazine), drawn on both sides; looking west (yaw 90), `pitch`
/// degrees.
fn arm(sim: &mut NetSim, id: &str, rounds: u32, pitch: f32) -> Entity {
    let theirs = sim.character_of(0).unwrap();
    let w = sim.server.app.world_mut();
    // The spawn's own (the SMG1), else a new one.
    let mine = w
        .get::<Inventory>(theirs)
        .unwrap()
        .weapons
        .iter()
        .copied()
        .find(|e| w.get::<Weapon>(*e).is_some_and(|x| x.id == id));
    let gun = mine.unwrap_or_else(|| give(w, theirs, id).unwrap());
    if let Some(mut r) = w.get_mut::<Hl2Rounds>(gun) {
        r.count = rounds;
    }
    if let Some(mut m) = w.get_mut::<Magazine>(gun) {
        m.clip = 1;
        m.reserve = rounds;
    }
    w.get_mut::<Inventory>(theirs).unwrap().wanted = Some(gun);
    sim.ticks(150);
    let mut i = client_intent(sim);
    i.yaw = 90f32.to_radians();
    i.pitch = pitch.to_radians();
    sim.ticks(10);
    gun
}

/// The client's drawn items with `model`.
fn drawn(sim: &mut NetSim, model: &str) -> Vec<Vec3> {
    let w = sim.clients[0].app.world_mut();
    w.query::<(&NetItem, &Transform)>()
        .iter(w)
        .filter(|(i, _)| i.model == model)
        .map(|(_, t)| t.translation)
        .collect()
}

/// Press `button` (fire or attack2) for `hold` steps, then watch up to
/// `watch` steps for the client to draw `model`; returns the drawn
/// positions seen, a step apart.
fn shoot(sim: &mut NetSim, secondary: bool, hold: usize, watch: usize, model: &str) -> Vec<Vec3> {
    let mut seen = Vec::new();
    for k in 0..watch {
        {
            let mut i = client_intent(sim);
            let on = k < hold;
            if secondary {
                i.secondary = on;
            } else {
                i.fire = on;
            }
        }
        sim.step();
        if let Some(at) = drawn(sim, model).first() {
            seen.push(*at);
        }
    }
    seen
}

/// The client's predicted rounds of `gun` (its copy, by weapon id) agree
/// with the server's.
fn rounds_agree(sim: &mut NetSim, id: &str, server_gun: Entity) {
    let theirs = sim.server.app.world().get::<Hl2Rounds>(server_gun).map(|r| r.count);
    let me = sim.local_player(0).unwrap();
    let w = sim.clients[0].app.world();
    let inv = w.get::<Inventory>(me).unwrap();
    let mine = inv
        .weapons
        .iter()
        .find(|e| w.get::<Weapon>(**e).is_some_and(|x| x.id == id))
        .and_then(|e| w.get::<Hl2Rounds>(*e))
        .map(|r| r.count);
    assert_eq!(mine, theirs, "{id}: the client's rounds");
}

fn moved(seen: &[Vec3], what: &str) {
    assert!(seen.len() >= 3, "{what}: drawn {} times", seen.len());
    let d = seen.last().unwrap().distance(seen[0]);
    assert!(d > 10.0 * U, "{what}: moved {} units", d / U);
}

#[test]
fn the_smg1_grenade_is_drawn_on_the_client() {
    let mut sim = joined();
    let gun = arm(&mut sim, weapons::SMG1, 2, 0.0);
    let seen = shoot(&mut sim, true, 2, 40, launch::SMG1_GRENADE_MODEL);
    moved(&seen, "grenade");
    sim.ticks(40);
    rounds_agree(&mut sim, weapons::SMG1, gun);
    assert_eq!(sim.server.app.world().get::<Hl2Rounds>(gun).unwrap().count, 1);
}

#[test]
fn the_ar2_ball_is_drawn_on_the_client() {
    let mut sim = joined();
    let gun = arm(&mut sim, weapons::AR2, 1, 0.0);
    let seen = shoot(&mut sim, true, 2, 90, launch::BALL_MODEL);
    moved(&seen, "ball");
    rounds_agree(&mut sim, weapons::AR2, gun);
}

#[test]
fn the_rpg_missile_is_drawn_on_the_client() {
    let mut sim = joined();
    let gun = arm(&mut sim, weapons::RPG, 3, 0.0);
    let seen = shoot(&mut sim, false, 2, 60, launch::MISSILE_MODEL);
    moved(&seen, "missile");
    rounds_agree(&mut sim, weapons::RPG, gun);
    assert_eq!(sim.server.app.world().get::<Hl2Rounds>(gun).unwrap().count, 2);
}

#[test]
fn the_crossbow_bolt_is_drawn_on_the_client() {
    let mut sim = joined();
    // Up a little: a long flight across the greybox.
    arm(&mut sim, weapons::CROSSBOW, 4, 3.0);
    let seen = shoot(&mut sim, false, 2, 30, launch::BOLT_MODEL);
    moved(&seen, "bolt");
}

#[test]
fn the_frag_is_drawn_on_the_client() {
    let mut sim = joined();
    let gun = arm(&mut sim, weapons::FRAG, 2, 10.0);
    let seen = shoot(&mut sim, false, 2, 60, weapons::FRAG);
    moved(&seen, "frag");
    sim.ticks(40);
    rounds_agree(&mut sim, weapons::FRAG, gun);
    assert_eq!(sim.server.app.world().get::<Hl2Rounds>(gun).unwrap().count, 1);
}

#[test]
fn the_satchel_is_drawn_on_the_client() {
    let mut sim = joined();
    let gun = arm(&mut sim, weapons::SLAM, 2, 10.0);
    let seen = shoot(&mut sim, false, 2, 80, launch::SATCHEL_MODEL);
    moved(&seen, "satchel");
    sim.ticks(40);
    rounds_agree(&mut sim, weapons::SLAM, gun);
}

#[test]
fn the_trip_mine_is_drawn_on_the_client() {
    let mut sim = joined();
    // Up to the west wall.
    let theirs = sim.character_of(0).unwrap();
    sim.server
        .app
        .world_mut()
        .get_mut::<Transform>(theirs)
        .unwrap()
        .translation = Vec3::new(-40.0 + 30.0 * U, 1.0, 12.0);
    sim.ticks(60);
    let gun = arm(&mut sim, weapons::SLAM, 2, 0.0);
    // Its mode changes first (1.3333 s), then the attach.
    sim.ticks(120);
    let seen = shoot(&mut sim, false, 2, 60, launch::MINE_MODEL);
    assert!(!seen.is_empty(), "the mine is drawn");
    rounds_agree(&mut sim, weapons::SLAM, gun);
}
