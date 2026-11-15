//! TF2's guns on the headless harness (`harness::Tf2Sim`, the greybox),
//! specs/tf2/weapons.md's test cases (their numbers in the test names):
//! the distance modifier on summed pellets, the first pellet dead centre
//! after a pause, the shell-by-shell reload a shot stops, the rocket
//! (where it starts, how fast it flies, its blast, a rocket jump, through
//! teammates), the Minigun's spin, held holder and damage ramp. Ticks at
//! TF2's 0.015 s; the guns given (`give`), the targets' health raised so
//! a hit's whole damage shows.

use bevy::prelude::*;
use chimera_core::core::{Health, Team, Velocity};
use chimera_sim::{
    testkit,
    weapon::{
        HolsterLocked, Hitscan, Inventory, Magazine, SpreadShape, Weapon, WeaponEvent, WeaponEventKind, give,
        projectile::{Flyer, FlyerGone},
    },
};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use tf2::{
    game::{
        TICK_INTERVAL, movement,
        weapons::{self, MINIGUN, ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN, minigun::Tf2Minigun},
    },
    harness::Tf2Sim,
};

const U: f32 = 0.0254;

pub(crate) fn sim() -> Sim {
    let mut sim = Sim::new(Tf2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(TICK_INTERVAL);
    sim
}

/// A player at `at` holding `id`, drawn and settled.
pub(crate) fn holding(sim: &mut Sim, at: Vec3, id: &str) -> (Entity, Entity) {
    let p = sim.spawn_character(at, movement::ID);
    sim.seconds(1.0);
    let w = give(sim.app.world_mut(), p, id).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    assert_eq!(testkit::active_id(sim.app.world(), p), Some(sim.app.world().get::<Weapon>(w).unwrap().id));
    (p, w)
}

/// A target `units` ahead of the shooter at `from` (along -Z), its health
/// 1000 points.
pub(crate) fn target(sim: &mut Sim, from: Vec3, units: f32) -> Entity {
    let t = sim.spawn_character(from - Vec3::Z * units * U, movement::ID);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 10.0;
    t
}

pub(crate) fn health_points(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current * 100.0
}

pub(crate) fn no_spread(sim: &mut Sim, w: Entity) {
    sim.app.world_mut().get_mut::<Hitscan>(w).unwrap().spread = SpreadShape::Template(0.0);
}

/// Run `n` ticks, `input` setting the buttons before each (its index),
/// and return what the player's weapons did on which tick.
pub(crate) fn run(sim: &mut Sim, p: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Vec<(u64, WeaponEventKind)> {
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

pub(crate) fn fired(events: &[(u64, WeaponEventKind)]) -> Vec<u64> {
    events.iter().filter(|(_, k)| matches!(k, WeaponEventKind::Fired { .. })).map(|(t, _)| *t).collect()
}

/// One shot of `id` at a target `units` away, every pellet on its centre:
/// the damage it took.
fn one_shot(id: &str, units: f32) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, id);
    no_spread(&mut sim, w);
    let t = target(&mut sim, from, units);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = health_points(&sim, t);
    // The Minigun spins up first: a shot after a second and more spun.
    let n = if id == MINIGUN { 50 + 90 } else { 1 };
    let ev = run(&mut sim, p, n + 3, |s, k| s.intent(p).fire = k < n);
    let shots = fired(&ev).len();
    if id == MINIGUN {
        // The damage of the last shot alone (every one at 100 % by now).
        let hits: Vec<f32> = ev
            .iter()
            .filter_map(|(_, k)| match k {
                WeaponEventKind::Hit { amount, .. } => Some(*amount * 100.0),
                _ => None,
            })
            .collect();
        return *hits.last().expect("hit");
    }
    assert_eq!(shots, 1);
    before - health_points(&sim, t)
}

#[test]
fn scattergun_by_distance_2_to_6() {
    for (units, want) in [(256.0, 91.0), (512.0, 60.0), (768.0, 39.0), (1024.0, 32.0), (1200.0, 32.0)] {
        let got = one_shot(SCATTERGUN, units);
        assert!((got - want).abs() < 0.01, "{units} units: {got}, want {want}");
    }
}

#[test]
fn shotgun_by_distance_10_11() {
    for (units, want) in [(256.0, 81.0), (512.0, 60.0), (1024.0, 32.0)] {
        let got = one_shot(SHOTGUN, units);
        assert!((got - want).abs() < 0.01, "{units} units: {got}, want {want}");
    }
}

#[test]
fn minigun_spun_a_second_12() {
    let got = one_shot(MINIGUN, 512.0);
    assert!((got - 36.0).abs() < 0.01, "{got}");
}

/// 7.2, tests 13, 14 and 33: held from idle, the first shot 50 ticks after
/// the press, then every 7; its damage 50 % then rising; the holder at
/// 110 units/s, no jump, no switch while it spins.
#[test]
fn minigun_spins_up_fires_every_7_ticks_and_ramps_13_14_33_35_36() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, MINIGUN);
    no_spread(&mut sim, w);
    let t = target(&mut sim, from, 512.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let mut jumped = false;
    let ev = run(&mut sim, p, 90, |s, k| {
        s.intent(p).fire = true;
        if k == 30 {
            s.intent(p).jump = true;
        }
        if k == 31 {
            s.intent(p).jump = false;
            jumped |= s.velocity(p).y > 0.5;
        }
    });
    assert!(!jumped, "jumped while spinning up");
    let shots = fired(&ev);
    assert_eq!(shots[0], 50, "{shots:?}");
    assert!(shots.windows(2).all(|w| w[1] - w[0] == 7), "{shots:?}");
    let hits: Vec<f32> = ev
        .iter()
        .filter_map(|(_, k)| match k {
            WeaponEventKind::Hit { amount, .. } => Some(*amount * 100.0),
            _ => None,
        })
        .collect();
    assert!((hits[0] - 18.0).abs() < 0.01, "{hits:?}");
    assert!(hits[1] > hits[0] && hits[2] > hits[1], "{hits:?}");
    let world = sim.app.world();
    assert_eq!(world.get::<Weapon>(w).unwrap().max_speed, Some(110.0 * U));
    assert!(world.get::<HolsterLocked>(w).is_some());
    // Let go: winds down 1.15 s, then free again.
    run(&mut sim, p, 80, |s, _| s.intent(p).fire = false);
    let world = sim.app.world();
    assert_eq!(world.get::<Weapon>(w).unwrap().max_speed, None);
    assert!(world.get::<HolsterLocked>(w).is_none());
    assert_eq!(world.get::<Tf2Minigun>(w).unwrap().spin, weapons::minigun::Spin::Idle);
}

/// 6.1, test 38: the first pellet after a pause goes along the aim; a
/// Minigun's next shot 0.105 s later scatters all four.
#[test]
fn the_first_pellet_after_a_pause_goes_dead_centre_38() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, MINIGUN);
    let ev = run(&mut sim, p, 60, |s, _| {
        s.intent(p).fire = true;
        s.intent(p).yaw = 0.0;
        s.intent(p).pitch = 0.0;
    });
    let shots: Vec<Vec<Vec3>> = {
        let mut out: Vec<Vec<Vec3>> = Vec::new();
        for (_, k) in &ev {
            match k {
                WeaponEventKind::Fired { .. } => out.push(Vec::new()),
                WeaponEventKind::Shot { from, to, .. } => {
                    if let Some(s) = out.last_mut() {
                        s.push((*to - *from).normalize());
                    }
                }
                _ => {}
            }
        }
        out
    };
    assert!(shots.len() >= 2, "{} shots", shots.len());
    let ahead = Vec3::NEG_Z;
    assert!(shots[0][0].dot(ahead) > 0.99999, "first pellet {:?}", shots[0][0]);
    assert!(shots[0][1..].iter().any(|d| d.dot(ahead) < 0.9999), "the rest scatter");
    assert!(shots[1].iter().all(|d| d.dot(ahead) < 0.99999), "a shot 7 ticks on: none centred {:?}", shots[1]);
}

/// 4, tests 27 and 28: an empty Scattergun reloads by itself, a shell in
/// at 0.7 s and every 0.5 s after; a shot with shells in stops it.
#[test]
fn shells_go_in_one_at_a_time_and_a_shot_stops_them_27_28() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SCATTERGUN);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 0;
        m.reserve = 32;
    }
    let mut clips = Vec::new();
    let ev = run(&mut sim, p, 230, |s, _| clips.push(s.app.world().get::<Magazine>(w).unwrap().clip));
    clips.push(sim.app.world().get::<Magazine>(w).unwrap().clip);
    let start = ev.iter().find(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)).map(|(t, _)| *t).expect("reloads");
    // The tick on which each shell went in (the clip read after it).
    let ins: Vec<u64> = (1..=6).map(|n| clips.iter().position(|c| *c >= n).unwrap() as u64 - 1 - start).collect();
    let want: Vec<f64> = (0..6).map(|n| (0.7 + 0.5 * n as f64) / TICK_INTERVAL).collect();
    for (got, want) in ins.iter().zip(&want) {
        assert!((*got as f64 - want).abs() <= 1.5, "shells in at {ins:?}, want {want:?}");
    }
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().reserve, 26);
    // Test 28: two in, reloading again: attack fires at once.
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 2;
    }
    sim.intent(p).reload = true;
    sim.ticks(2);
    sim.intent(p).reload = false;
    sim.ticks(30);
    let ev = run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(fired(&ev), [0], "{ev:?}");
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 1);
    let ev = run(&mut sim, p, 50, |s, _| s.intent(p).fire = false);
    assert!(!ev.iter().any(|(_, k)| matches!(k, WeaponEventKind::ShellInserting)), "the reload stopped");
}

/// The flyers of `owner`.
pub(crate) fn rockets(sim: &mut Sim, owner: Entity) -> Vec<(Entity, Vec3, Vec3)> {
    let mut q = sim.app.world_mut().query::<(Entity, &Flyer, &Transform)>();
    q.iter(sim.app.world()).filter(|(_, f, _)| f.owner == Some(owner)).map(|(e, f, t)| (e, t.translation, f.velocity)).collect()
}

/// 5.1, tests 24 and 25: the rocket starts 23.5 ahead, 12 right, 3 down
/// of the eye, flies at 1100 units/s along the view, and explodes on the
/// wall it meets.
#[test]
fn the_rocket_starts_under_the_gun_and_flies_at_1100_24_25() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, ROCKET_LAUNCHER);
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let mut gone = sim.app.world().resource::<Messages<FlyerGone>>().get_cursor_current();
    let ev = run(&mut sim, p, 2, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).yaw = 0.0;
        s.intent(p).pitch = 0.0;
    });
    assert_eq!(fired(&ev), [0]);
    let r = rockets(&mut sim, p);
    assert_eq!(r.len(), 1);
    let (_, at, v) = r[0];
    // One move made on the tick after its launch: back to where it began.
    let start = at - v * TICK_INTERVAL as f32;
    let want = eye + Vec3::NEG_Z * 23.5 * U + Vec3::X * 12.0 * U - Vec3::Y * 3.0 * U;
    assert!((start - want).length() < 0.02, "started at {start}, want {want}");
    assert!((v - Vec3::NEG_Z * 1100.0 * U).length() < 1e-3, "{v}");
    // Whatever it meets ahead: gone within a tick or two of its flight
    // time from where it started.
    let mut ticks = 1u64;
    let at = loop {
        sim.ticks(1);
        ticks += 1;
        let events = sim.app.world().resource::<Messages<FlyerGone>>();
        if let Some(g) = gone.read(events).find(|g| g.owner == Some(p)) {
            break g.at;
        }
        assert!(ticks < 400, "never exploded");
    };
    let want = (at - start).length() / U / 1100.0 / TICK_INTERVAL as f32;
    assert!(want > 30.0, "flew {want} ticks");
    assert!((ticks as f32 - want).abs() <= 2.0, "exploded after {ticks} ticks, want {want}");
}

/// 5.2, test 16-like: a direct hit at 512 units takes 90 (the modifier 1,
/// full splash).
#[test]
fn a_direct_hit_at_512_takes_90_15() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, ROCKET_LAUNCHER);
    let t = target(&mut sim, from, 512.0);
    // Aim so the rocket, 12 right and 3 down of the eye, meets its centre.
    let aim = sim.position(t) - Vec3::X * 12.0 * U + Vec3::Y * 3.0 * U;
    testkit::aim_at(&mut sim, p, aim);
    let before = health_points(&sim, t);
    run(&mut sim, p, 60, |s, k| s.intent(p).fire = k == 0);
    let took = before - health_points(&sim, t);
    assert!((took - 90.0).abs() < 1.5, "took {took}");
}

/// 5.3, 5.4, tests 21-22: a rocket at his feet hurts the Soldier with no
/// distance modifier and throws him up.
#[test]
fn a_rocket_at_his_feet_hurts_and_lifts_the_soldier_21() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], ROCKET_LAUNCHER);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 10.0;
    let y0 = sim.position(p).y;
    let before = health_points(&sim, p);
    let mut top = y0;
    run(&mut sim, p, 60, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).pitch = -89f32.to_radians();
        top = top.max(s.position(p).y);
    });
    let took = before - health_points(&sim, p);
    assert!((40.0..=90.0).contains(&took), "self damage {took}");
    assert!((top - y0) / U > 20.0, "rose {} units", (top - y0) / U);
}

/// 5.1, test 26: a teammate in the way: the rocket passes through him.
#[test]
fn rockets_pass_through_teammates_26() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, ROCKET_LAUNCHER);
    let mate = target(&mut sim, from, 200.0);
    sim.app.world_mut().entity_mut(p).insert(Team(1));
    sim.app.world_mut().entity_mut(mate).insert(Team(1));
    let aim = sim.position(mate) - Vec3::X * 12.0 * U + Vec3::Y * 3.0 * U;
    testkit::aim_at(&mut sim, p, aim);
    let before = health_points(&sim, mate);
    run(&mut sim, p, 20, |s, k| s.intent(p).fire = k == 0);
    let r = rockets(&mut sim, p);
    assert_eq!(r.len(), 1, "still flying past the teammate");
    assert!(r[0].1.z < sim.position(mate).z, "beyond him");
    assert_eq!(health_points(&sim, mate), before);
    let _ = sim.app.world().get::<Velocity>(mate);
}

/// The Rocket Launcher hurts (its blast through `tf2:rocket`'s harm
/// reader): an automatic ladder may draw it.
#[test]
fn every_gun_can_hurt_a_player() {
    let sim = sim();
    let w = sim.app.world();
    let readers = w.resource::<chimera_sim::minigame::power::PowerReaders>();
    let catalog = w.resource::<chimera_sim::weapon::data::WeaponCatalog>();
    for id in weapons::GUNS {
        let data = catalog.get(id).unwrap_or_else(|| panic!("{id} in the catalog"));
        assert_eq!(chimera_sim::weapon::harm::can_damage_player(data, &readers.harm), Some(true), "{id}");
    }
}
