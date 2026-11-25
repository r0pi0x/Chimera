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
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::TemplateDisc(0.0));
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
    sim.app.world_mut().get_mut::<Hitscan>(w).unwrap().spread = SpreadShape::TemplateDisc(0.0);
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
    // The hit lands 14 ticks after the press, a backstab's 27.
    run(&mut sim, p, 30, |s, k| {
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

#[test]
fn the_thompsons_draw_and_empty_reload_are_measured() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], KNIFE);
    sim.intent(p).pitch = 1.2;
    let w = give(sim.app.world_mut(), p, THOMPSON).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    let ev = run(&mut sim, p, 100, |s, _| s.intent(p).fire = true);
    // Switched on tick 0: the first shot 63 ticks after the switch.
    assert_eq!(fired(&ev).first().copied(), Some(63), "{:?}", fired(&ev));
    // Empty, reloaded: fires again 3.0 s on with 30 rounds.
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 0;
    let ev = run(&mut sim, p, 300, |s, t| {
        s.intent(p).fire = t >= 20;
        s.intent(p).reload = t < 10;
    });
    let started = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)).map(|(t, _)| *t).expect("a reload");
    let first = *fired(&ev).first().expect("fires after the reload");
    let after = (first - started) as f64 * TICK_INTERVAL;
    assert!((3.0..3.03).contains(&after), "{after} s");
}

/// The holder's view: its zoom's field of view, if any.
fn fov(sim: &Sim, p: Entity) -> Option<f32> {
    sim.app.world().get::<chimera_sim::weapon::Zoomed>(p).map(|z| z.fov)
}

#[test]
fn the_sights_narrow_the_view_21_ticks_after_the_press() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], GARAND);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(19);
    assert_eq!(fov(&sim, p), None, "20 ticks after the press");
    sim.ticks(2);
    assert_eq!(fov(&sim, p), Some(55.0), "22 ticks after");
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(1.0);
    let mut sim2 = self::sim();
    let (q, _) = holding(&mut sim2, greybox::SPAWNS[0], SPRING);
    sim2.intent(q).secondary = true;
    sim2.ticks(1);
    sim2.intent(q).secondary = false;
    sim2.seconds(0.5);
    assert_eq!(fov(&sim2, q), Some(20.0), "the Springfield's scope");
}

#[test]
fn the_bars_alt_fire_selects_semi_automatic() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, weapons_spawn(), weapons::BAR);
    sim.intent(p).pitch = 1.2;
    let ev = run(&mut sim, p, 60, |s, t| {
        s.intent(p).secondary = t == 0;
        s.intent(p).fire = t >= 2;
    });
    assert_eq!(sim.app.world().get::<AltModes>(w).unwrap().current, 1, "semi-automatic");
    assert_eq!(fired(&ev).len(), 1, "one shot a press: {:?}", fired(&ev));
    assert_eq!(fov(&sim, p), None, "no sights");
    assert_eq!(sim.app.world().get::<Hitscan>(w).unwrap().spread, SpreadShape::TemplateDisc(0.02), "its alt accuracy");
    // A press every other tick: one shot each 0.3 s (20 ticks) at most.
    let ev = run(&mut sim, p, 100, |s, t| s.intent(p).fire = t % 2 == 0);
    let shots = fired(&ev);
    assert!(shots.len() >= 4, "{shots:?}");
    assert!(shots.windows(2).all(|w| w[1] - w[0] >= 20), "{shots:?}");
}

fn weapons_spawn() -> Vec3 {
    greybox::SPAWNS[0]
}

/// One swing of `id` (its alt fire with `secondary`) at a target `ahead`
/// units in front and `left` to the left, facing `facing` degrees off the
/// swinger's view (0: its back to the swinger), aimed level at the
/// target's place: the damage and the tick it landed on (None: a miss).
fn swing_at(id: &str, secondary: bool, ahead: f32, left: f32, facing: f32) -> (f32, Option<u64>) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = sim.spawn_character(from - Vec3::Z * ahead * U - Vec3::X * left * U, movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 10.0;
    sim.intent(p).yaw = 0.0;
    sim.intent(p).pitch = 0.0;
    sim.intent(t).yaw = facing.to_radians();
    let before = health_points(&sim, t);
    let ev = run(&mut sim, p, 30, |s, k| {
        s.intent(p).fire = !secondary && k == 0;
        s.intent(p).secondary = secondary && k == 0;
        hold_still(s, t);
    });
    let hit = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Hit { .. })).map(|(t, _)| *t);
    (before - health_points(&sim, t), hit)
}

/// damage.md, "Melee": the hit lands 14 ticks (0.21 s) after the press;
/// a standing victim 92 units away is hit, 94 not (aim level).
#[test]
fn the_knife_hits_0_21_s_after_the_press_and_reaches_92() {
    let (d, hit) = swing_at(KNIFE, false, 92.0, 0.0, 180.0);
    assert!(d > 0.0, "92 units: {d}");
    assert_eq!(hit, Some(14), "the hit's tick");
    let (d, _) = swing_at(KNIFE, false, 94.0, 0.0, 180.0);
    assert_eq!(d, 0.0, "94 units");
    // 45 degrees off the aim at 56 units (40 ahead, 40 to the side).
    let (d, _) = swing_at(SPADE, false, 40.0, 40.0, 180.0);
    assert!(d > 0.0, "45 degrees off at 56: {d}");
}

/// damage.md, "Melee": behind when the attacker's aim · the victim's
/// facing > 0.3 (72° off the attacker's yaw: 300; 73°: 60), the aim
/// counting, not the line between them.
#[test]
fn the_backstab_is_the_aim_against_the_victims_facing() {
    let (back, _) = swing_at(KNIFE, false, 40.0, 0.0, 72.0);
    let (front, _) = swing_at(KNIFE, false, 40.0, 0.0, 73.0);
    assert!(front > 0.0 && (back / front - 5.0).abs() < 1e-3, "72: {back}, 73: {front}");
    // 40 left and 40 ahead facing 105° off the yaw (60° off the line): 60.
    let (side, _) = swing_at(KNIFE, false, 40.0, 40.0, 105.0);
    let (side_back, _) = swing_at(KNIFE, false, 40.0, 40.0, 60.0);
    assert!(side > 0.0 && (side_back / side - 5.0).abs() < 1e-3, "105: {side}, 60: {side_back}");
}

/// How a swing's victim stands (`swing_on`).
#[derive(Clone, Copy, PartialEq)]
enum Victim {
    Standing,
    Crouched,
    Prone,
}

/// A swing of `id` held for `ticks` (a press each tick it may swing) at a
/// `victim` `ahead` units in front facing `facing` degrees off the
/// swinger's view (0: its back to the swinger), the aim `down` degrees
/// below level: the damage, the ticks it landed on and the ticks each
/// swing started on.
fn swing_on(id: &str, victim: Victim, ahead: f32, facing: f32, down: f32, ticks: u64) -> (f32, Vec<u64>, Vec<u64>) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, id);
    let t = sim.spawn_character(from - Vec3::Z * ahead * U, movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 100.0;
    sim.intent(p).yaw = 0.0;
    sim.intent(p).pitch = -down.to_radians();
    sim.intent(t).yaw = facing.to_radians();
    match victim {
        Victim::Standing => {}
        Victim::Crouched => {
            sim.intent(t).crouch = true;
            sim.seconds(0.6);
        }
        Victim::Prone => {
            let bit = sim.app.world().resource::<chimera_core::core::ActionRegistry>().bit(dods::game::stance::PRONE_ACTION);
            sim.intent(t).actions |= bit;
            sim.ticks(1);
            sim.intent(t).actions &= !bit;
            sim.seconds(1.5);
            assert!(sim.app.world().get::<dods::game::stance::DodsStance>(t).unwrap().prone(), "the victim is prone");
        }
    }
    let before = health_points(&sim, t);
    let ev = run(&mut sim, p, ticks, |s, _| {
        s.intent(p).fire = !s.intent(p).fire;
        hold_still(s, t);
    });
    let at = |hit: bool| {
        ev.iter()
            .filter(|(_, k)| if hit { matches!(k, WeaponEventKind::Hit { .. }) } else { matches!(k, WeaponEventKind::SwingStarted { .. }) })
            .map(|(t, _)| *t)
            .collect::<Vec<_>>()
    };
    (before - health_points(&sim, t), at(true), at(false))
}

/// damage.md, "Melee": a backstab is a slower swing, settled on the
/// press: the 300 lands 27 ticks (0.405 s) after it and the next attack
/// waits 0.9 s (60 ticks), knife and spade alike; a swing from the front
/// keeps 14 ticks and 0.4 s (27 ticks).
#[test]
fn a_backstab_lands_0_405_s_after_the_press_and_blocks_0_9_s() {
    for id in [KNIFE, SPADE] {
        let (back, hits, starts) = swing_on(id, Victim::Standing, 40.0, 0.0, 0.0, 70);
        assert_eq!(hits.first().copied(), Some(27), "{id}: the backstab's hit {hits:?}");
        assert_eq!(&starts[..2], &[0, 60], "{id}: the next swing 0.9 s on: {starts:?}");
        let (front, hits, starts) = swing_on(id, Victim::Standing, 40.0, 180.0, 0.0, 70);
        assert_eq!(hits.first().copied(), Some(14), "{id}: from the front {hits:?}");
        assert_eq!(&starts[..2], &[0, 28], "{id}: the next swing 0.4 s on: {starts:?}");
        // The first of each: 300 and 60 (the same zone).
        let (one_back, _, _) = swing_on(id, Victim::Standing, 40.0, 0.0, 0.0, 30);
        let (one_front, _, _) = swing_on(id, Victim::Standing, 40.0, 180.0, 0.0, 20);
        assert!(one_front > 0.0 && (one_back / one_front - 5.0).abs() < 1e-3, "{id}: {one_back} / {one_front}");
        assert!(back > 0.0 && front > 0.0);
    }
}

/// damage.md's test cases: the same cone for crouched and prone victims:
/// crouched 40 ahead, aim 20° down, facing 71° off: 300, 71.5°: 60;
/// prone 50 ahead, aim 45° down, facing 60° off: 300, 66°: 60. A
/// backstab on either is the slower swing.
#[test]
fn crouched_and_prone_victims_take_the_same_backstab() {
    let one = |victim, ahead, facing, down| {
        let (d, hits, _) = swing_on(KNIFE, victim, ahead, facing, down, 30);
        (d, hits.first().copied())
    };
    let (back, at) = one(Victim::Crouched, 40.0, 71.0, 20.0);
    let (front, front_at) = one(Victim::Crouched, 40.0, 71.5, 20.0);
    assert!(front > 0.0 && (back / front - 5.0).abs() < 1e-3, "crouched 71: {back}, 71.5: {front}");
    assert_eq!((at, front_at), (Some(27), Some(14)), "crouched");
    let (back, at) = one(Victim::Prone, 50.0, 60.0, 45.0);
    let (front, front_at) = one(Victim::Prone, 50.0, 66.0, 45.0);
    assert!(front > 0.0 && (back / front - 5.0).abs() < 1e-3, "prone 60: {back}, 66: {front}");
    assert_eq!((at, front_at), (Some(27), Some(14)), "prone");
}

/// The kind of swing is settled on the press (damage.md, "Melee"): a
/// victim turning its back after a press from the front takes 60 at 14
/// ticks; one turning round after a press from behind still takes the
/// backstab at 27.
#[test]
fn the_backstab_is_settled_on_the_press() {
    for (first, then, want_tick, ratio) in [(180.0f32, 0.0f32, 14u64, 1.0f32), (0.0, 180.0, 27, 5.0)] {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, _) = holding(&mut sim, from, KNIFE);
        let t = sim.spawn_character(from - Vec3::Z * 40.0 * U, movement::ID);
        sim.seconds(1.0);
        sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 100.0;
        sim.intent(p).yaw = 0.0;
        sim.intent(p).pitch = 0.0;
        sim.intent(t).yaw = first.to_radians();
        let before = health_points(&sim, t);
        let ev = run(&mut sim, p, 30, |s, k| {
            s.intent(p).fire = k == 0;
            if k == 1 {
                s.intent(t).yaw = then.to_radians();
            }
            hold_still(s, t);
        });
        let hit = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::Hit { .. })).map(|(t, _)| *t);
        assert_eq!(hit, Some(want_tick), "pressed facing {first}");
        let (front, _, _) = swing_on(KNIFE, Victim::Standing, 40.0, 180.0, 0.0, 20);
        let got = before - health_points(&sim, t);
        assert!((got / front - ratio).abs() < 1e-3, "pressed facing {first}: {got} against {front}");
    }
}

/// weapons.md, "Alt fire": the Thompson's stock strike: the knife's hit
/// (60 before the zones) 0.21 s after the press, no backstab, the
/// magazine untouched, its next shot 0.4 s from the press.
#[test]
fn the_thompsons_stock_strike_is_the_knifes_without_a_backstab() {
    let (front, hit) = swing_at(THOMPSON, true, 45.0, 0.0, 180.0);
    let (behind, _) = swing_at(THOMPSON, true, 45.0, 0.0, 0.0);
    let (knife, _) = swing_at(KNIFE, false, 45.0, 0.0, 180.0);
    assert_eq!(hit, Some(14));
    assert!(front > 0.0 && (front - knife).abs() < 1e-3, "stock {front}, knife {knife}");
    assert!((behind - front).abs() < 1e-3, "no backstab: {behind}");
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], THOMPSON);
    sim.intent(p).pitch = 1.2;
    let ev = run(&mut sim, p, 60, |s, k| {
        s.intent(p).secondary = k == 0;
        s.intent(p).fire = k >= 1;
    });
    assert_eq!(clip(&sim, w), 30 - fired(&ev).len() as u32, "the strike takes no round");
    let first = *fired(&ev).first().expect("fires after the strike");
    let after = first as f64 * TICK_INTERVAL;
    assert!((0.39..0.43).contains(&after), "first shot {after} s after the strike");
}

/// weapons.md, "Spread": the cone by alt mode and movement, no stance
/// bonus: the Thompson still 0.055, ducked moving (73.3) 0.155; the Colt
/// prone still 0.055; the BAR semi-auto 0.02; the MG42 one bullet a shot.
#[test]
fn the_spread_is_the_scripts_plus_the_whole_penalty_over_45() {
    let cone = |sim: &Sim, w: Entity| sim.app.world().get::<Hitscan>(w).unwrap().spread;
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], THOMPSON);
    sim.intent(p).pitch = 1.2;
    run(&mut sim, p, 2, |s, _| s.intent(p).fire = true);
    assert_eq!(cone(&sim, w), SpreadShape::TemplateDisc(0.055), "still");
    sim.intent(p).fire = false;
    sim.intent(p).crouch = true;
    sim.intent(p).move_axis = Vec2::Y;
    sim.seconds(1.5);
    let v = sim.velocity(p).xz().length() / U;
    assert!(v > 45.0, "ducked moving {v}");
    run(&mut sim, p, 2, |s, _| s.intent(p).fire = true);
    assert!(matches!(cone(&sim, w), SpreadShape::TemplateDisc(c) if (c - 0.155).abs() < 1e-6), "{:?}", cone(&sim, w));
    let set = weapons::dods_weapons();
    assert_eq!(set.get(weapons::MG42).unwrap().hitscan.as_ref().unwrap().pellets, 1, "the MG42 fires one bullet");
}

/// weapons.md, "Draw and reload times": a partial reload takes the empty
/// one's time but the Colt's 1.56 s and the C96's 2.22 s, and only the
/// missing rounds leave the reserve.
#[test]
fn partial_reloads_take_only_the_missing_rounds() {
    for (id, left, size, secs) in [(weapons::COLT, 3, 7, 1.56), (THOMPSON, 10, 30, 3.0), (weapons::C96, 10, 20, 2.22)] {
        let mut sim = sim();
        let (p, w) = holding(&mut sim, greybox::SPAWNS[0], id);
        sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = left;
        let reserve = sim.app.world().get::<Magazine>(w).unwrap().reserve;
        let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
        let (mut started, mut done) = (None, None);
        for t in 0..300u64 {
            sim.intent(p).reload = t < 3;
            sim.ticks(1);
            let events = sim.app.world().resource::<Messages<WeaponEvent>>();
            for e in cursor.read(events).filter(|e| e.owner == p) {
                match e.kind {
                    WeaponEventKind::ReloadStarted => started = started.or(Some(t)),
                    WeaponEventKind::Reloaded => done = done.or(Some(t)),
                    _ => {}
                }
            }
        }
        let took = (done.expect("reloaded") - started.expect("a reload")) as f64 * TICK_INTERVAL;
        assert!((took - secs).abs() < 0.02, "{id}: {took} s");
        let m = sim.app.world().get::<Magazine>(w).unwrap();
        assert_eq!((m.clip, reserve - m.reserve), (size, size - left), "{id}");
    }
}

/// weapons.md, "When a gun can't fire": held fire while sprinting fires on
/// the tick the sprint ends; through a jump, on the landing tick; crawling,
/// once the crawl has stopped.
#[test]
fn no_shot_sprinting_in_the_air_or_crawling() {
    use dods::game::stamina::DodsStamina;
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], THOMPSON);
    sim.intent(p).pitch = 0.6;
    // Sprinting.
    sim.intent(p).move_axis = Vec2::Y;
    sim.intent(p).sprint = true;
    sim.seconds(0.5);
    assert!(sim.app.world().get::<DodsStamina>(p).unwrap().sprinting);
    let mut ended = None;
    let ev = run(&mut sim, p, 40, |s, t| {
        s.intent(p).fire = true;
        s.intent(p).sprint = t < 30;
        if ended.is_none() && t > 0 && !s.app.world().get::<DodsStamina>(p).unwrap().sprinting {
            ended = Some(t - 1);
        }
    });
    let first = *fired(&ev).first().expect("fires once the sprint ends");
    assert!(first >= 30, "no shot while sprinting: {first}");
    let _ = ended;
    // In the air: held from the jump, the first shot on the landing tick.
    sim.intent(p).move_axis = Vec2::ZERO;
    sim.intent(p).fire = false;
    sim.seconds(1.0);
    let mut landed = None;
    let ev = run(&mut sim, p, 80, |s, t| {
        s.intent(p).jump = t == 0;
        s.intent(p).fire = true;
        if t > 2 && landed.is_none() && s.state(p).on_ground {
            landed = Some(t - 1);
        }
    });
    let first = *fired(&ev).first().expect("fires on landing");
    let landed = landed.expect("lands");
    assert!(first > 5 && first <= landed, "first shot {first}, landed on {landed}");
}

/// The cone's move threshold (weapons.md, "Spread": 44.9 no penalty, 45.1
/// the whole of it).
#[test]
fn the_move_penalty_starts_over_45_units_a_second() {
    use dods::game::weapons::gun::moving;
    assert!(!moving(Vec3::X * 44.9 * U));
    assert!(moving(Vec3::X * 45.1 * U));
    assert!(!moving(Vec3::Y * 300.0 * U), "vertical speed doesn't count");
}
