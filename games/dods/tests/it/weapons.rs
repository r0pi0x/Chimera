//! DoD:S's weapons on the headless harness (`harness::DodsSim`, the
//! greybox), from the committed documents (the spec's numbers; the
//! scripts' are checked against them in `heavy::scripts`):
//! specs/dods/weapons.md's and damage.md's test cases: the fire delay
//! rounded up to ticks (the Thompson's shot every 6 ticks, 17 in 100,
//! the Springfield's bolt), the Garand's one shot a press and its reload
//! only when empty, the sights blocking the trigger 0.49 s, the hit zones
//! (head x2.5, limbs x0.75, no falloff), the knife from behind x5, and
//! being slowed when hit (0.5 s, a leg 2 s). Targets' health raised so a
//! hit's whole damage shows.

use bevy::prelude::*;
use chimera_core::core::{Health, SimClock, Velocity};
use chimera_sim::{
    testkit,
    weapon::{AltModes, Hitscan, Inventory, Magazine, SpreadShape, Weapon, WeaponEvent, WeaponEventKind, give},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use dods::{
    game::{
        TICK_INTERVAL, movement,
        weapons::{self, GARAND, K98, KNIFE, SPADE, SPRING, THOMPSON, hit_slow::DodsSlowed},
    },
    harness::DodsSim,
};
use source_engine::source_movement::SourceMovement;

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

/// A target `units` ahead of `from` (along -Z), 1000 points of health.
fn target(sim: &mut Sim, from: Vec3, units: f32) -> Entity {
    let t = sim.spawn_character(from - Vec3::Z * units * U, movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 10.0;
    t
}

fn hold_still(sim: &mut Sim, e: Entity) {
    sim.app.world_mut().get_mut::<Velocity>(e).unwrap().0 = Vec3::ZERO;
}

fn health_points(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current * 100.0
}

/// `n` ticks, `input` setting the buttons before each: what the player's
/// weapons did on which tick.
fn run(sim: &mut Sim, p: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Vec<(u64, WeaponEventKind)> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let mut out = Vec::new();
    for t in 0..n {
        input(sim, t);
        sim.ticks(1);
        let events = sim.app.world().resource::<Messages<WeaponEvent>>();
        for e in cursor.read(events) {
            if e.owner == p && e.shown() {
                out.push((t, e.kind.clone()));
            }
        }
    }
    out
}

fn fired(events: &[(u64, WeaponEventKind)]) -> Vec<u64> {
    events.iter().filter(|(_, k)| matches!(k, WeaponEventKind::Fired { .. })).map(|(t, _)| *t).collect()
}

fn reloads(events: &[(u64, WeaponEventKind)]) -> usize {
    events.iter().filter(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)).count()
}

fn clip(sim: &Sim, w: Entity) -> u32 {
    sim.app.world().get::<Magazine>(w).unwrap().clip
}

#[test]
fn every_weapon_registers_from_the_data() {
    let set = weapons::dods_weapons();
    for id in weapons::GUNS.iter().chain(weapons::MELEE) {
        assert!(set.get(id).is_some(), "{id}");
    }
    // The fire delays rounded up to ticks: 0.085 is 6 ticks, 0.1 is 7.
    let cycle = |id: &str| set.get(id).unwrap().trigger.as_ref().unwrap().cycle;
    assert!((cycle(THOMPSON) - 0.09).abs() < 1e-6, "{}", cycle(THOMPSON));
    assert!((cycle(weapons::COLT) - 0.105).abs() < 1e-6);
    assert!((cycle(SPRING) - 1.86).abs() < 1e-6);
}

#[test]
fn the_thompson_fires_every_6_ticks_17_in_100() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], THOMPSON);
    sim.intent(p).pitch = 1.2;
    let ev = run(&mut sim, p, 100, |s, _| s.intent(p).fire = true);
    let shots = fired(&ev);
    assert_eq!(shots.len(), 17, "{shots:?}");
    assert!(shots.windows(2).all(|w| w[1] - w[0] == 6), "{shots:?}");
    assert_eq!(clip(&sim, w), 13);
}

#[test]
fn the_garand_fires_once_a_press_and_reloads_only_when_empty() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], GARAND);
    sim.intent(p).pitch = 1.2;
    let ev = run(&mut sim, p, 60, |s, _| s.intent(p).fire = true);
    assert_eq!(fired(&ev).len(), 1, "semi-automatic");
    assert_eq!(clip(&sim, w), 7);
    let ev = run(&mut sim, p, 300, |s, _| {
        s.intent(p).fire = false;
        s.intent(p).reload = true;
    });
    assert_eq!(reloads(&ev), 0, "7 rounds left: no reload");
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 0;
    let ev = run(&mut sim, p, 20, |s, t| s.intent(p).reload = t < 3);
    assert_eq!(reloads(&ev), 1, "empty: it reloads");
}

#[test]
fn the_springfields_bolt_waits_its_delay() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], SPRING);
    sim.intent(p).pitch = 1.2;
    // A press every other tick.
    let ev = run(&mut sim, p, 200, |s, t| s.intent(p).fire = t % 2 == 0);
    let shots = fired(&ev);
    assert!(shots.len() >= 2, "{shots:?}");
    let gap = (shots[1] - shots[0]) as f64 * TICK_INTERVAL;
    assert!(gap >= 1.85 - 1e-9 && gap < 1.85 + 0.035, "{gap} s");
}

#[test]
fn the_sights_block_the_trigger_049_s() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], GARAND);
    sim.intent(p).pitch = 1.2;
    let ev = run(&mut sim, p, 60, |s, t| {
        s.intent(p).secondary = t == 0;
        s.intent(p).fire = t >= 1;
    });
    assert_eq!(sim.app.world().get::<AltModes>(w).unwrap().current, 1, "sighted");
    let first = *fired(&ev).first().expect("a shot after the block");
    let after = (first + 1) as f64 * TICK_INTERVAL;
    assert!((0.48..0.52).contains(&after), "first shot {after} s after the sights went up");
    // Sighted, the cone is the script's alt accuracy (0 for the Garand).
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::Cone(0.0));
}

/// One shot of `id` at a target `units` away, aimed `aim` units over its
/// feet: the damage dealt and how long the target is slowed.
fn one_shot(id: &str, units: f32, aim: f32) -> (f32, f64) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, id);
    let t = target(&mut sim, from, units);
    // The sights up: a perfect shot (alt accuracy 0) where the gun has them.
    if sim.app.world().get::<AltModes>(w).is_some() {
        run(&mut sim, p, 40, |s, k| s.intent(p).secondary = k == 0);
    }
    sim.app.world_mut().get_mut::<Hitscan>(w).unwrap().spread = SpreadShape::Cone(0.0);
    let feet = sim.position(t) - Vec3::Y * 36.0 * U;
    testkit::aim_at(&mut sim, p, feet + Vec3::Y * aim * U);
    let before = health_points(&sim, t);
    let ev = run(&mut sim, p, 2, |s, k| {
        s.intent(p).fire = k == 0;
        hold_still(s, t);
    });
    assert_eq!(fired(&ev).len(), 1, "{ev:?}");
    let now = sim.app.world().resource::<SimClock>().now;
    let slowed = sim.app.world().get::<DodsSlowed>(t).map_or(0.0, |s| (s.until - now).max(0.0));
    (before - health_points(&sim, t), slowed)
}

// The greybox's targets wear no hit boxes: their zones are the box's
// (`weapon::deliver::hitgroup_at`), where a frontal hit between the legs
// and the head is an arm's (x0.75); the head (x2.5) and the legs (x0.75)
// are reached by aiming at them.
#[test]
fn hit_zones_and_the_slow() {
    // damage.md's test cases: the Garand's head shot, the K98's leg; the
    // same head shot far away (no falloff), an arm.
    let (head, slow) = one_shot(GARAND, 150.0, 64.0);
    assert!((head - 200.0).abs() < 0.01, "head {head}");
    assert!((slow - 0.5).abs() < 0.05, "slowed {slow}");
    let (leg, slow) = one_shot(K98, 150.0, 20.0);
    assert!((leg - 82.5).abs() < 0.01, "leg {leg}");
    assert!((slow - 2.0).abs() < 0.05, "slowed {slow}");
    let (far, _) = one_shot(K98, 1200.0, 64.0);
    assert!((far - 275.0).abs() < 0.01, "a head far away {far}");
    let (arm, slow) = one_shot(THOMPSON, 150.0, 50.0);
    assert!((arm - 30.0).abs() < 0.01, "arm {arm}");
    assert!((slow - 0.5).abs() < 0.05, "slowed {slow}");
}

#[test]
fn a_slowed_player_moves_slower() {
    let mut sim = sim();
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.0);
    let until = sim.app.world().resource::<SimClock>().now + 2.0;
    sim.app.world_mut().get_mut::<DodsSlowed>(p).expect("on every character").until = until;
    sim.ticks(2);
    let m = sim.app.world().get::<SourceMovement>(p).unwrap().velocity_modifier;
    // Held at the slow's speed before each move (the move's own recovery
    // adds a tick's worth).
    assert!(m <= weapons::hit_slow::SLOWED_SPEED + 0.01, "{m}");
    sim.seconds(2.5);
    let now = sim.app.world().resource::<SimClock>().now;
    assert!(!sim.app.world().get::<DodsSlowed>(p).unwrap().at(now), "the slow ends");
    let m = sim.app.world().get::<SourceMovement>(p).unwrap().velocity_modifier;
    assert!(m > 0.99, "back to full speed: {m}");
}

/// One swing of `id` at a target 40 units ahead facing `away` (true: its
/// back to the swinger), aimed `aim` units over its feet: the damage.
fn one_swing(id: &str, away: bool, aim: f32) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = target(&mut sim, from, 40.0);
    // The target faces -Z (away from the swinger) or +Z (towards it).
    sim.intent(t).yaw = if away { 0.0 } else { std::f32::consts::PI };
    let feet = sim.position(t) - Vec3::Y * 36.0 * U;
    testkit::aim_at(&mut sim, p, feet + Vec3::Y * aim * U);
    let before = health_points(&sim, t);
    run(&mut sim, p, 10, |s, k| {
        s.intent(p).fire = k == 0;
        hold_still(s, t);
    });
    before - health_points(&sim, t)
}

#[test]
fn the_knife_and_spade_60_and_five_times_from_behind() {
    // The head's x2.5 on both (the box's zones: `hit_zones_and_the_slow`):
    // 60 x 2.5 from the front, 300 x 2.5 from behind.
    let got = [one_swing(KNIFE, false, 64.0), one_swing(KNIFE, true, 64.0), one_swing(SPADE, false, 12.0)];
    assert!((got[0] - 150.0).abs() < 0.01, "{got:?}");
    assert!((got[1] - 750.0).abs() < 0.01, "{got:?}");
    assert!((got[2] - 45.0).abs() < 0.01, "a leg: 60 x 0.75: {got:?}");
}
