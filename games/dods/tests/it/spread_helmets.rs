//! DoD:S's spread pattern and helmets on the headless harness
//! (`harness::DodsSim`, the greybox): specs/dods/spread.md's test cases
//! (a shot on a command whose seed is 8 leaves along the measured second
//! pair, in the aim's frame, the spread the script's 0.055) and
//! bodygroups.md's (a bullet to the head knocks the helmet off, lethal or
//! not, one elsewhere doesn't, a new life puts it back).

use bevy::prelude::*;
use chimera_core::core::{Dead, Health, Seed, SimTick};
use chimera_sim::{
    testkit,
    weapon::{self, Hitscan, Inventory, SpreadShape, Weapon, WeaponEvent, WeaponEventKind, give},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use dods::{
    game::{
        TICK_INTERVAL,
        helmets::DodsHelmet,
        movement,
        weapons::{GARAND, THOMPSON},
    },
    harness::DodsSim,
};

const U: f32 = 0.0254;

fn sim() -> Sim {
    let mut sim = Sim::new(DodsSim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// A player at `at` holding `id`, drawn and settled.
fn holding(sim: &mut Sim, at: Vec3, id: &str) -> (Entity, Entity) {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.0);
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.5);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(sim.app.world().get::<Weapon>(w).unwrap().id));
    (p, w)
}

/// The first command at or after the next tick whose shot seed is `seed`,
/// as `p`'s `Seed` (command = seed + tick).
fn on_shot_seed(sim: &mut Sim, p: Entity, seed: u32) {
    let next = sim.app.world().resource::<SimTick>().0 as u32 + 1;
    let c = (0..100_000u32).find(|c| weapon::random::shot_seed(*c) == seed).unwrap();
    sim.app.world_mut().entity_mut(p).insert(Seed(c.wrapping_sub(next) as u64));
}

#[test]
fn a_bullet_follows_its_commands_seed_inside_the_spread() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], THOMPSON);
    sim.intent(p).yaw = 0.3;
    sim.intent(p).pitch = -0.2;
    sim.ticks(2);
    on_shot_seed(&mut sim, p, 8);
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    let events: Vec<_> = cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()).map(|e| e.kind.clone()).collect();
    let (yaw, pitch, seed, spread) = events
        .iter()
        .find_map(|k| match k {
            WeaponEventKind::Fired { yaw, pitch, seed, spread, .. } => Some((*yaw, *pitch, *seed, *spread)),
            _ => None,
        })
        .expect("fired");
    assert_eq!(seed, 8);
    assert_eq!(spread, SpreadShape::TemplateDisc(0.055));
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().pellets, 1);
    let (from, to) = events
        .iter()
        .find_map(|k| match k {
            WeaponEventKind::Shot { from, to, .. } => Some((*from, *to)),
            _ => None,
        })
        .expect("traced");
    // spread.md: seed 8's first pair is outside the circle; the bullet is
    // the second, (0.1701, -0.3028) spreads right and up of the aim.
    let aim = weapon::aim_of(yaw, pitch);
    let d = (to - from).normalize();
    let ahead = d.dot(aim * Vec3::NEG_Z);
    let x = d.dot(aim * Vec3::X) / ahead / 0.055;
    let y = d.dot(aim * Vec3::Y) / ahead / 0.055;
    assert!((x - 0.1701).abs() < 2e-3 && (y + 0.3028).abs() < 2e-3, "({x}, {y})");
}

/// One Garand shot, sights up (spread 0), at a target 150 units ahead,
/// aimed `aim` units over its feet, with `health` points: the target.
fn shoot_target(sim: &mut Sim, aim: f32, health: f32) -> Entity {
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(sim, from, GARAND);
    let t = sim.spawn_character(from - Vec3::Z * 150.0 * U, movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = health / 100.0;
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(0.7);
    sim.app.world_mut().get_mut::<Hitscan>(w).unwrap().spread = SpreadShape::TemplateDisc(0.0);
    let feet = sim.position(t) - Vec3::Y * 36.0 * U;
    testkit::aim_at(sim, p, feet + Vec3::Y * aim * U);
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.ticks(1);
    t
}

#[test]
fn a_head_shot_knocks_the_helmet_off_until_the_next_life() {
    // Not lethal: off, and stays off.
    let mut sim = sim();
    let t = shoot_target(&mut sim, 64.0, 100_000.0);
    assert!(sim.app.world().get::<Health>(t).unwrap().current * 100.0 < 100_000.0, "hit");
    assert!(sim.app.world().get::<DodsHelmet>(t).unwrap().off, "a head shot");
    // A new life (`Dead` taken off) puts it back.
    sim.app.world_mut().entity_mut(t).insert(Dead { since: 0.0 });
    sim.ticks(1);
    sim.app.world_mut().entity_mut(t).remove::<Dead>();
    sim.ticks(1);
    assert!(!sim.app.world().get::<DodsHelmet>(t).unwrap().off, "back on after a respawn");

    // Lethal (200 on 150): off as it dies.
    let mut sim = self::sim();
    let t = shoot_target(&mut sim, 64.0, 150.0);
    assert!(sim.app.world().get::<Health>(t).unwrap().current <= 0.0, "killed");
    assert!(sim.app.world().get::<DodsHelmet>(t).unwrap().off, "a lethal head shot");

    // Elsewhere (a leg): on.
    let mut sim = self::sim();
    let t = shoot_target(&mut sim, 20.0, 100_000.0);
    assert!(sim.app.world().get::<Health>(t).unwrap().current * 100.0 < 100_000.0, "hit");
    assert!(!sim.app.world().get::<DodsHelmet>(t).unwrap().off, "a leg shot leaves it on");
}
