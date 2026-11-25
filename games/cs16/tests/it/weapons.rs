//! Counter-Strike 1.6's guns on the headless harness (`harness::Cs16Sim`,
//! the greybox): specs/cs16/weapons.md's and damage.md's test cases (the
//! AK-47's first shots' spread and accuracy, its fire rate and kick, a
//! reload, the Deagle's accuracy by time, the Glock's burst on
//! consecutive commands, the AWP's spread and zoom, the knife's slash and
//! backstab, the M3's pellets, HE damage through a wall and its reach,
//! the flash at its centre, the M4A1's silencer).

use bevy::prelude::*;
use chimera_core::core::Health;
use chimera_sim::{
    testkit,
    weapon::{AltModes, Armor, Inventory, Magazine, SpreadShape, ViewPunch, Weapon, WeaponEvent, WeaponEventKind, Zoomed, give},
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use cs16::{
    game::{TICK_INTERVAL, movement, weapons::*},
    harness::Cs16Sim,
};

const U: f32 = 0.0254;

fn sim() -> Sim {
    let mut sim = Sim::new(Cs16Sim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// A player at `at` holding `id`, drawn and settled.
fn holding(sim: &mut Sim, at: Vec3, id: &str) -> (Entity, Entity) {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.0);
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(2.0);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(sim.app.world().get::<Weapon>(w).unwrap().id));
    (p, w)
}

/// What `p`'s weapons did over `ticks` ticks with `fire` held.
fn run(sim: &mut Sim, p: Entity, fire: bool, ticks: u32) -> Vec<WeaponEventKind> {
    let mut cursor = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    sim.intent(p).fire = fire;
    let mut out = Vec::new();
    for _ in 0..ticks {
        sim.ticks(1);
        out.extend(cursor.read(sim.app.world().resource::<Messages<WeaponEvent>>()).filter(|e| e.owner == p).map(|e| e.kind.clone()));
    }
    sim.intent(p).fire = false;
    out
}

fn spreads(events: &[WeaponEventKind]) -> Vec<f32> {
    events
        .iter()
        .filter_map(|k| match k {
            WeaponEventKind::Fired { spread: SpreadShape::Template(s), .. } => Some(*s),
            _ => None,
        })
        .collect()
}

/// The AK-47 standing still: shot 1 at 0.0275 × 0.2, shots 2-6 at
/// 0.0275 × 0.35 (whole-number accuracy), from the seventh 0.0275 × 1.25;
/// 11 shots in a second of 10 ms commands; the punch of the first two.
#[test]
fn the_ak47s_first_shots_accuracy_rate_and_kick() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], AK47);
    sim.intent(p).pitch = 0.0;
    let first = run(&mut sim, p, true, 1);
    let punch = sim.app.world().get::<ViewPunch>(p).map_or(Vec3::ZERO, |p| p.0);
    assert!((punch.x.to_degrees() - 1.0).abs() < 1e-3, "first shot kicks 1.0 up: {}", punch.x.to_degrees());
    assert!((punch.y.to_degrees() + 0.375).abs() < 1e-3, "and 0.375 right: {}", punch.y.to_degrees());
    let rest = run(&mut sim, p, true, 100);
    let mut s = spreads(&first);
    s.extend(spreads(&rest));
    assert_eq!(s.len(), 11, "one shot every 10 commands, from 0 to 100");
    assert!((s[0] - 0.0055).abs() < 1e-6, "{}", s[0]);
    for (i, x) in s[1..6].iter().enumerate() {
        assert!((x - 0.009625).abs() < 1e-6, "shot {}: {x}", i + 2);
    }
    for x in &s[6..] {
        assert!((x - 0.034375).abs() < 1e-6, "{x}");
    }
}

/// Two shots without decay: the second kicks 1.0 + 2 × 0.175 more.
#[test]
fn the_ak47s_second_kick() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], AK47);
    run(&mut sim, p, true, 1);
    sim.app.world_mut().get_mut::<ViewPunch>(p).unwrap().0 = Vec3::new(1f32.to_radians(), 0.0, 0.0);
    // The next shot is due 10 commands on; hold the punch still meanwhile.
    sim.ticks(9);
    sim.app.world_mut().get_mut::<ViewPunch>(p).unwrap().0 = Vec3::new(1f32.to_radians(), 0.0, 0.0);
    sim.app.world_mut().get_mut::<cs16::game::weapons::gun::Cs16Gun>(w).unwrap().released = None;
    run(&mut sim, p, true, 1);
    let x = sim.app.world().get::<ViewPunch>(p).unwrap().0.x.to_degrees();
    // The tick's decay applies before the shot.
    let decayed = 1.0 - (10.0 + 0.5) * TICK_INTERVAL as f32;
    assert!((x - (decayed + 1.35)).abs() < 1e-3, "{x}");
}

/// Reloading an AK-47 with 10 in the clip and 90 in reserve.
#[test]
fn an_ak47_reload() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], AK47);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 10;
        m.reserve = 90;
    }
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    sim.seconds(2.4);
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 10, "not before 2.45 s");
    sim.seconds(0.1);
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    assert_eq!((m.clip, m.reserve), (30, 70));
}

/// The Deagle's accuracy by time: its 0.225 s cycle is 23 commands, so
/// the second shot (0.23 s on) uses the first's 0.9 and leaves
/// 0.9 − (0.4 − 0.23) × 0.35 = 0.8405 for the third.
#[test]
fn the_deagles_accuracy_by_time() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], DEAGLE);
    let mut s = Vec::new();
    for _ in 0..3 {
        s.extend(spreads(&run(&mut sim, p, true, 1)));
        sim.ticks(22);
    }
    assert_eq!(s.len(), 3);
    assert!((s[0] - 0.013).abs() < 1e-6 && (s[1] - 0.013).abs() < 1e-6, "{s:?}");
    assert!((s[2] - 0.13 * (1.0 - 0.8405)).abs() < 1e-5, "{s:?}");
}

/// The Glock in burst mode: one press, three bullets on three
/// consecutive commands; the follow-ups at 0.05.
#[test]
fn the_glocks_burst_on_consecutive_commands() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], GLOCK);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.seconds(0.5);
    assert_eq!(sim.app.world().get::<AltModes>(w).unwrap().current, 1);
    let mut ticks = Vec::new();
    let mut fired = Vec::new();
    for t in 0..10 {
        let ev = run(&mut sim, p, t == 0, 1);
        let s = spreads(&ev);
        if !s.is_empty() {
            ticks.push(t);
            fired.extend(s);
        }
    }
    assert_eq!(ticks, [0, 1, 2], "{fired:?}");
    assert!((fired[1] - 0.05).abs() < 1e-6 && (fired[2] - 0.05).abs() < 1e-6, "{fired:?}");
    assert!((fired[0] - 0.3 * (1.0 - 0.9)).abs() < 1e-6, "the first: burst mode's standing row {fired:?}");
}

/// The AWP: 0.081 unzoomed standing; zoomed and ducked 0, the view
/// back to 90 at the shot and to the zoom 1.45 s on.
#[test]
fn the_awps_spread_and_zoom() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], AWP);
    let s = spreads(&run(&mut sim, p, true, 1));
    assert!((s[0] - 0.081).abs() < 1e-6, "{s:?}");
    sim.seconds(1.6);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.intent(p).crouch = true;
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<AltModes>(w).unwrap().current, 1);
    assert!(sim.app.world().get::<Zoomed>(p).is_some_and(|z| (z.fov - 40.0).abs() < 1e-3));
    let s = spreads(&run(&mut sim, p, true, 1));
    assert_eq!(s, [0.0]);
    sim.ticks(1);
    assert!(sim.app.world().get::<Zoomed>(p).is_none(), "unzoomed by the shot");
    sim.seconds(1.5);
    assert!(sim.app.world().get::<Zoomed>(p).is_some_and(|z| (z.fov - 40.0).abs() < 1e-3), "back to the zoom");
}

/// The M4A1's silencer: 2.0 s before the next shot.
#[test]
fn the_m4a1s_silencer_blocks_two_seconds() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], M4A1);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    assert_eq!(sim.app.world().get::<AltModes>(w).unwrap().current, 1);
    assert!(spreads(&run(&mut sim, p, true, 190)).is_empty(), "no shot within 2 s");
    assert_eq!(spreads(&run(&mut sim, p, true, 15)).len(), 1);
}

/// Two players `distance` units apart on the greybox, the second facing
/// away (or towards) the first; health 100, no armour.
fn pair(sim: &mut Sim, id: &str, distance: f32, behind: bool) -> (Entity, Entity) {
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(sim, from, id);
    let t = sim.spawn_character(from - Vec3::Z * distance * U, movement::ID);
    sim.seconds(0.5);
    // Both look along −Z: the target faces away from the attacker.
    sim.intent(p).yaw = 0.0;
    sim.intent(t).yaw = if behind { 0.0 } else { std::f32::consts::PI };
    sim.ticks(2);
    (p, t)
}

fn health(sim: &Sim, e: Entity) -> f32 {
    (sim.app.world().get::<Health>(e).unwrap().current * 100.0).round()
}

/// The knife: a slash to the chest 15; a stab from behind 195.
#[test]
fn the_knifes_slash_and_backstab() {
    let mut sim = sim();
    let (p, t) = pair(&mut sim, KNIFE, 30.0, false);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 2.0;
    let chest = sim.position(t) + Vec3::Y * 10.0 * U;
    testkit::aim_at(&mut sim, p, chest);
    run(&mut sim, p, true, 1);
    sim.ticks(2);
    assert_eq!(health(&sim, t), 185.0, "a slash: 15");

    let mut sim = self::sim();
    let (p, t) = pair(&mut sim, KNIFE, 28.0, true);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 3.0;
    let chest = sim.position(t) + Vec3::Y * 10.0 * U;
    testkit::aim_at(&mut sim, p, chest);
    sim.intent(p).secondary = true;
    sim.ticks(1);
    sim.intent(p).secondary = false;
    sim.ticks(2);
    assert_eq!(health(&sim, t), 105.0, "a stab from behind: 195");
}

/// The M3 close up: nine pellets of 20 × (1 − distance / 3000) in one
/// damage event (all nine in the chest point blank: 180).
#[test]
fn the_m3_close_up() {
    let mut sim = sim();
    let (p, t) = pair(&mut sim, M3, 40.0, false);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 3.0;
    let at = sim.position(t) + Vec3::Y * 10.0 * U;
    testkit::aim_at(&mut sim, p, at);
    let ev = run(&mut sim, p, true, 1);
    sim.ticks(2);
    let hits = ev.iter().filter(|k| matches!(k, WeaponEventKind::Hit { target, .. } if *target == t)).count();
    assert_eq!(hits, 1, "one damage event");
    let taken = 300.0 - health(&sim, t);
    // 40 units apart: 9 × 20 × (1 − 40/3000) = 177.6.
    assert!((170.0..=178.0).contains(&taken), "{taken}");
}

/// The point an HE blast aims at on `t` (its box centre: the blast's
/// `body_target` of its eye height) and its feet's height.
fn blast_target(sim: &Sim, t: Entity) -> (Vec3, f32) {
    let at = sim.position(t);
    let state = sim.app.world().get::<chimera_core::core::MovementState>(t).unwrap();
    // As the blast finds them: the hull's bottom, else the collider's.
    let feet = if state.hull_min != Vec3::ZERO {
        at.y + state.hull_min.y
    } else {
        sim.app.world().get::<avian3d::prelude::ColliderAabb>(t).unwrap().min.y
    };
    let eye = at.y + state.eye_offset.y;
    let h = cs16::game::weapons::grenades::he_blast().body_target.0;
    (at.with_y(feet + (eye - feet) * h), feet)
}

/// An HE blast at `at`: where it went off (pulled out of the ground under
/// it) and what `t` lost in health.
fn blast(sim: &mut Sim, at: Vec3, t: Entity) -> (Vec3, f32) {
    use chimera_sim::weapon::grenade::{BlastAt, BlastDealt};
    let before = health(sim, t);
    let mut cursor = sim.app.world().resource::<Messages<BlastDealt>>().get_cursor_current();
    sim.app.world_mut().write_message(BlastAt {
        at,
        blast: std::sync::Arc::new(cs16::game::weapons::grenades::he_blast()),
        thrower: None,
        weapon: HEGRENADE,
        inflictor: None,
    });
    let mut origin = None;
    for _ in 0..2 {
        sim.ticks(1);
        if let Some(d) = cursor.read(sim.app.world().resource::<Messages<BlastDealt>>()).last() {
            origin = Some(d.at);
        }
    }
    (origin.expect("the blast went off"), before - health(sim, t))
}

/// An HE on the ground: 100 − d × 100/350 to the box centre, whole points,
/// the point 45.6 units off the ground; at 175 units 50 (49 or 50 as
/// the float distance falls), at 350 none.
#[test]
fn he_damage_by_distance() {
    for (distance, expected) in [(175.0f32, 50.0f32), (350.0, 0.0), (100.0, 71.0)] {
        let mut sim = sim();
        let t = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
        sim.seconds(1.0);
        let (centre, feet) = blast_target(&sim, t);
        let up = feet + 45.6 * U - centre.y;
        let across = ((distance * U).powi(2) - up * up).max(0.0).sqrt();
        let (origin, lost) = blast(&mut sim, centre.with_y(feet + U) - Vec3::Z * across, t);
        assert!((origin.y - (feet + 45.6 * U)).abs() < 0.01, "pulled out of the ground: {origin}");
        let d = origin.distance(centre) / U;
        let formula = (100.0 - d * 100.0 / 350.0).max(0.0).floor();
        assert_eq!(lost, formula, "at {d} units");
        assert!((lost - expected).abs() <= 1.0, "at {distance}: {lost}");
    }
}

/// Walls don't block it: a player just inside the north wall takes the
/// blast from the other side.
#[test]
fn he_damage_through_walls() {
    let mut sim = sim();
    let t = sim.spawn_character(Vec3::new(0.0, 1.0, greybox::NORTH_WALL_Z + 1.0), movement::ID);
    sim.seconds(1.0);
    let (centre, _) = blast_target(&sim, t);
    let at = centre - Vec3::Z * 120.0 * U;
    let (origin, lost) = blast(&mut sim, at, t);
    assert!(origin.z < greybox::NORTH_WALL_Z, "the blast behind the wall: {origin}");
    let d = origin.distance(centre) / U;
    assert!(lost > 0.0 && lost == (100.0 - d * 100.0 / 350.0).floor(), "{lost} at {d} units");
}

/// An armoured player: half the blast to health, a quarter of it from
/// the armour (damage.md, "Blasts").
#[test]
fn he_on_armour() {
    let mut sim = sim();
    let t = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().entity_mut(t).insert(Armor { amount: 1.0, helmet: false });
    let (centre, feet) = blast_target(&sim, t);
    let (origin, lost) = blast(&mut sim, centre.with_y(feet + U), t);
    let d = origin.distance(centre) / U;
    let damage = 100.0 - d * 100.0 / 350.0;
    assert_eq!(lost, (damage * 0.5).floor(), "{d} units");
    let armour = sim.app.world().get::<Armor>(t).unwrap().amount * 100.0;
    assert!((armour - (100.0 - damage * 0.25)).abs() < 1.01, "{armour}");
}
