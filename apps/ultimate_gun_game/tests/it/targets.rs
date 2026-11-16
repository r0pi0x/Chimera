//! The sandbox's practice targets (chimera_sim's `minigame::targets`, the
//! bots behind them Combat Arms'): placed at the distance asked on the
//! floor, refused outside the sandbox and past the floor, a protected one
//! measured but unhurt, a killed one back after its delay with its
//! settings, a still one that may not fire neither fires nor moves, and a
//! kill's shots and hits counted. On the party's simulation, headless.

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{Damage, DamageKind, Dead, Health, Hitgroup, LocalPlayer, SimClock},
};
use chimera_sim::{
    minigame::{
        self,
        targets::{self, HitMeasured, KillMeasured, SpawnTarget, Target, TargetMotion, TargetRespawn},
    },
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const GUN: &str = "combat_arms:m4a1";

/// What the facts said, kept.
#[derive(Resource, Default)]
struct Seen {
    hits: Vec<HitMeasured>,
    kills: Vec<KillMeasured>,
    shots: Vec<Entity>,
}

fn keep(mut seen: ResMut<Seen>, mut h: MessageReader<HitMeasured>, mut k: MessageReader<KillMeasured>, mut w: MessageReader<WeaponEvent>) {
    seen.hits.extend(h.read().cloned());
    seen.kills.extend(k.read().cloned());
    for e in w.read() {
        if matches!(e.kind, WeaponEventKind::ShotMade { .. } | WeaponEventKind::Fired { .. }) {
            seen.shots.push(e.owner);
        }
    }
}

/// The sandbox playing with the local player `me` (where its spawn put it).
fn play() -> (Sim, Entity) {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.app.init_resource::<Seen>().add_systems(FixedPostUpdate, keep);
    sim.seconds(1.5);
    (sim, me)
}

fn add(sim: &mut Sim, req: SpawnTarget) -> Entity {
    let e = targets::spawn(sim.app.world_mut(), &req).unwrap_or_else(|e| panic!("the target: {e}"));
    sim.seconds(1.5);
    e
}

fn at(distance_m: f32) -> SpawnTarget {
    SpawnTarget { gun: Some(GUN.into()), distance_m, ..default() }
}

fn hit(sim: &mut Sim, by: Entity, victim: Entity, amount: f32) {
    let point = sim.position(victim);
    sim.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(by),
        amount,
        point,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: Some(GUN),
        force: Vec3::ZERO,
    });
}

fn shot(sim: &mut Sim, by: Entity, toward: Vec3) {
    let origin = sim.position(by) + Vec3::Y * 0.6;
    sim.app.world_mut().write_message(WeaponEvent {
        owner: by,
        weapon: Entity::PLACEHOLDER,
        kind: WeaponEventKind::ShotMade { id: GUN, origin, dir: (toward - origin).normalize() },
        replay: false,
    });
}

#[test]
fn a_target_stands_on_the_floor_at_the_distance_asked() {
    let (mut sim, me) = play();
    let e = add(&mut sim, at(10.0));
    let t = sim.app.world().get::<Target>(e).cloned().expect("a target");
    let mine = sim.position(me);
    assert!(((t.home - mine).xz().length() - 10.0).abs() < 0.05, "10 m out: {:?} from {mine:?}", t.home);
    let yaw = sim.app.world().get::<chimera_core::core::Intent>(me).unwrap().yaw;
    let view = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    assert!((t.home - mine).xz().normalize().dot(view.xz()) > 0.999, "along the view {view:?}: {:?}", t.home);
    assert_eq!((t.gun.as_str(), t.game.as_str(), t.character.as_str()), (GUN, "combat_arms", "combat_arms:default"));
    let body = sim.position(e);
    assert!((body - t.home).xz().length() < 0.5, "the body at home: {body:?} {:?}", t.home);
    assert!(sim.app.world().get::<Dead>(e).is_none(), "alive");
    // Past the greybox's floor: refused, nothing added.
    let far = targets::spawn(sim.app.world_mut(), &at(5000.0)).unwrap_err();
    assert!(far.contains("no floor"), "{far}");
    let list = {
        let before = sim.app.world().resource::<Console>().output.len();
        sim.app.world_mut().resource_mut::<Console>().submit("target list");
        sim.ticks(2);
        sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
    };
    assert!(list.contains("1 targets") && list.contains(GUN), "{list}");
}

#[test]
fn targets_are_refused_outside_the_sandbox() {
    let mut sim = ugg();
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(0.5);
    let e = targets::spawn(sim.app.world_mut(), &at(10.0)).unwrap_err();
    assert!(e.contains("sandbox"), "{e}");
}

#[test]
fn a_protected_target_is_measured_but_keeps_its_health() {
    let (mut sim, me) = play();
    let e = add(&mut sim, SpawnTarget { protected: true, ..at(8.0) });
    for _ in 0..4 {
        hit(&mut sim, me, e, 0.4);
        sim.ticks(2);
    }
    sim.ticks(2);
    let h = sim.app.world().get::<Health>(e).copied().unwrap();
    assert!(h.current >= h.max - 1e-4 && sim.app.world().get::<Dead>(e).is_none(), "unhurt: {h:?}");
    let seen = sim.app.world().resource::<Seen>();
    let on: Vec<&HitMeasured> = seen.hits.iter().filter(|m| m.victim == e).collect();
    assert_eq!(on.len(), 4, "every hit measured: {:?}", seen.hits);
    assert!(on.iter().all(|m| m.damage > 0.0 && m.attacker == Some(me) && m.gun == Some(GUN)), "{on:?}");
    assert!(on.iter().all(|m| (m.distance_m - 8.0).abs() < 1.0), "{on:?}");
    assert!(seen.kills.is_empty(), "no kill");
}

#[test]
fn a_killed_target_returns_after_its_delay_with_its_settings() {
    let (mut sim, me) = play();
    let e = add(&mut sim, SpawnTarget { motion: TargetMotion::Strafes, respawn_secs: 2.0, ..at(12.0) });
    let before = sim.app.world().get::<Target>(e).cloned().unwrap();
    hit(&mut sim, me, e, 50.0);
    sim.ticks(3);
    let now = sim.app.world().resource::<SimClock>().now;
    let back = sim.app.world().get::<TargetRespawn>(e).copied().expect("dead, its return set");
    assert!(sim.app.world().get::<Dead>(e).is_some());
    assert!((back.at - now - 2.0).abs() < 0.2, "2 s after the death: {back:?} now {now}");
    sim.seconds(1.0);
    assert!(sim.app.world().get::<Dead>(e).is_some(), "still dead a second on");
    sim.seconds(1.5);
    assert!(sim.app.world().get::<Dead>(e).is_none() && sim.app.world().get::<TargetRespawn>(e).is_none(), "back");
    let after = sim.app.world().get::<Target>(e).cloned().unwrap();
    assert_eq!(after, before, "the same settings");
    let body = sim.position(e);
    assert!((body - before.home).xz().length() < targets::STRAFE_HALF + 1.0, "at home: {body:?} {:?}", before.home);
}

#[test]
fn a_still_target_that_may_not_fire_neither_fires_nor_moves() {
    let (mut sim, me) = play();
    let e = add(&mut sim, at(8.0));
    let from = sim.position(e);
    sim.seconds(6.0);
    let to = sim.position(e);
    assert!((to - from).xz().length() < 0.3, "it stood: {from:?} -> {to:?}");
    let seen = sim.app.world().resource::<Seen>();
    assert!(!seen.shots.contains(&e), "it never fired");
    assert!(sim.app.world().get::<Dead>(me).is_none(), "the player is unhurt");
}

#[test]
fn a_kill_counts_its_shots_and_hits() {
    let (mut sim, me) = play();
    let e = add(&mut sim, at(10.0));
    let c = sim.position(e);
    shot(&mut sim, me, c);
    hit(&mut sim, me, e, 0.3);
    sim.ticks(3);
    // A near miss: at the target, no hit.
    shot(&mut sim, me, c + Vec3::X * 0.5);
    sim.ticks(3);
    shot(&mut sim, me, c);
    hit(&mut sim, me, e, 50.0);
    sim.ticks(3);
    let seen = sim.app.world().resource::<Seen>();
    let k = seen.kills.iter().find(|k| k.victim == e).unwrap_or_else(|| panic!("the kill: {:?}", seen.kills));
    assert_eq!((k.shots_fired, k.shots_hit, k.headshots), (3, 2, 0), "{k:?}");
    assert_eq!(k.attacker, Some(me));
    assert!(k.from_first_shot > 0.05 && (k.from_first_shot - k.from_first_hit).abs() < 1e-4, "{k:?}");
    assert!((k.distance_m - 10.0).abs() < 1.0 && k.total_damage > 0.3, "{k:?}");
}
