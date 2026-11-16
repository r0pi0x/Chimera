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

/// Hold `e` still (bullets push: weapons.md 5.4), so the distance stays.
pub(crate) fn hold_still(sim: &mut Sim, e: Entity) {
    sim.app.world_mut().get_mut::<Velocity>(e).unwrap().0 = Vec3::ZERO;
}

pub(crate) fn health_points(sim: &Sim, e: Entity) -> f32 {
    sim.app.world().get::<Health>(e).unwrap().current * 100.0
}

pub(crate) fn no_spread(sim: &mut Sim, w: Entity) {
    sim.app.world_mut().get_mut::<Hitscan>(w).unwrap().spread = SpreadShape::Template(0.0);
    // The Minigun sets its spread each tick from its own base.
    if let Some(mut m) = sim.app.world_mut().get_mut::<Tf2Minigun>(w) {
        m.base_spread = 0.0;
    }
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
    let ev = run(&mut sim, p, n + 3, |s, k| {
        s.intent(p).fire = k < n;
        hold_still(s, t);
    });
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

/// 7.1, 7.2 and the corrections from the SDK, tests 13, 14 and 33: held
/// from idle, the first shot 57 ticks after the press (spun at 50, the
/// shot 0.1 s on), then every 7; its damage 50 % until 0.2 s spun, then
/// rising (round(36 g(tau)) at 512); the holder at 110 units/s, no jump,
/// no switch while it spins.
#[test]
fn minigun_spins_up_fires_every_7_ticks_and_ramps_13_14_33() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, MINIGUN);
    no_spread(&mut sim, w);
    let t = target(&mut sim, from, 512.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let mut jumped = false;
    let ev = run(&mut sim, p, 120, |s, k| {
        s.intent(p).fire = true;
        hold_still(s, t);
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
    assert_eq!(shots[0], 57, "{shots:?}");
    assert!(shots.windows(2).all(|w| w[1] - w[0] == 7), "{shots:?}");
    let hits: Vec<(u64, f32)> = ev
        .iter()
        .filter_map(|(t, k)| match k {
            WeaponEventKind::Hit { amount, .. } => Some((*t, *amount * 100.0)),
            _ => None,
        })
        .collect();
    let p_ = weapons::hook_params::<weapons::minigun::MinigunParams>(MINIGUN, weapons::MINIGUN_HOOK);
    for (tick, got) in &hits {
        // The press was on run tick 0: tau = its time less the wind-up.
        let tau = (*tick as f32 * TICK_INTERVAL as f32 - 0.75).max(0.0);
        let want = (36.0 * weapons::minigun::damage_factor(&p_, tau) + 1e-3).round();
        assert!((got - want).abs() < 0.01, "tick {tick} (tau {tau}): {got}, want {want}; {hits:?}");
    }
    assert!((hits[0].1 - 18.0).abs() < 0.01, "{hits:?}");
    assert!(hits.iter().any(|(_, h)| (*h - 25.0).abs() < 0.01), "a 25 near tau 0.5: {hits:?}");
    let world = sim.app.world();
    assert_eq!(world.get::<Weapon>(w).unwrap().max_speed, Some(110.0 * U));
    assert!(world.get::<HolsterLocked>(w).is_some());
}

/// 7.1 and the corrections from the SDK, tests 35 and 36: no gameplay
/// wind-down. Let go of attack2 while spun: idle in play at once (the
/// speed back, a jump works the next tick), only the switch waits for the
/// 1.33 s spool-down.
#[test]
fn minigun_stops_at_once_and_only_the_switch_waits_35_36() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], MINIGUN);
    run(&mut sim, p, 80, |s, _| s.intent(p).secondary = true);
    assert!(sim.app.world().get::<Tf2Minigun>(w).unwrap().spin.turning());
    let mut rose = false;
    run(&mut sim, p, 2, |s, k| {
        s.intent(p).secondary = false;
        s.intent(p).jump = k == 1;
    });
    assert_eq!(sim.app.world().get::<Weapon>(w).unwrap().max_speed, None, "the speed back at once");
    run(&mut sim, p, 10, |s, _| {
        s.intent(p).jump = false;
        rose |= s.velocity(p).y > 0.5;
    });
    assert!(rose, "a jump right after the release");
    assert!(sim.app.world().get::<HolsterLocked>(w).is_some(), "the switch waits for the spool-down");
    run(&mut sim, p, 90, |_, _| {});
    assert!(sim.app.world().get::<HolsterLocked>(w).is_none());
    assert_eq!(sim.app.world().get::<Tf2Minigun>(w).unwrap().spin, weapons::minigun::Spin::Idle);
}

/// 7.2, test 49: spun on attack2 for 2 s from the press, then attack: the
/// first shot is full damage (36 at 512).
#[test]
fn minigun_spun_two_seconds_fires_full_at_once_49() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, MINIGUN);
    no_spread(&mut sim, w);
    let t = target(&mut sim, from, 512.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let ev = run(&mut sim, p, 140, |s, k| {
        s.intent(p).secondary = true;
        hold_still(s, t);
        s.intent(p).fire = k >= 134;
    });
    let hits: Vec<f32> = ev
        .iter()
        .filter_map(|(_, k)| match k {
            WeaponEventKind::Hit { amount, .. } => Some(*amount * 100.0),
            _ => None,
        })
        .collect();
    assert_eq!(fired(&ev), [134], "fires on the press: its next shot long due");
    assert!((hits[0] - 36.0).abs() < 0.01, "{hits:?}");
}

/// 6.1, test 38: the first pellet after a pause goes along the aim; a
/// Minigun's next shot 0.105 s later scatters all four.
#[test]
fn the_first_pellet_after_a_pause_goes_dead_centre_38() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, MINIGUN);
    let ev = run(&mut sim, p, 70, |s, _| {
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
/// of the eye, flies at 1100 units/s toward the crosshair's point, and
/// explodes on the wall it meets.
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
    // At 1100 toward the crosshair's point (the 2000-unit line's, or the
    // wall it meets): from 12 right and 3 down, a little left and up.
    assert!((v.length() - 1100.0 * U).abs() < 1e-3, "{v}");
    assert!(v.normalize().dot(Vec3::NEG_Z) > 0.999 && v.x < 0.0 && v.y > 0.0, "{v}");
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

/// The largest upward and horizontal speeds (units/s) `e` reaches over
/// `n` ticks of `input`.
fn peak_speeds(sim: &mut Sim, p: Entity, e: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> (f32, f32) {
    let (mut up, mut flat) = (0f32, 0f32);
    run(sim, p, n, |s, k| {
        input(s, k);
        let v = s.velocity(e) / U;
        up = up.max(v.y);
        flat = flat.max(Vec2::new(v.x, v.z).length());
    });
    (up, flat)
}

/// 5.3, 5.4 and the corrections from the SDK, test 46: a Soldier on the
/// ground, his own rocket at his feet: 90 damage (89.63 by the distance to
/// his feet), a push of about 448 units/s (scale 5, from 10 below the
/// blast) that takes him off the ground (above 250), and the
/// blast-jumping state until he lands.
#[test]
fn his_own_rocket_at_his_feet_on_the_ground_46() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], ROCKET_LAUNCHER);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 10.0;
    let before = health_points(&sim, p);
    let mut jumping = false;
    let (up, _) = peak_speeds(&mut sim, p, p, 30, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).pitch = -89f32.to_radians();
        jumping |= s.app.world().get::<weapons::blast::BlastJumping>(p).is_some();
    });
    let took = before - health_points(&sim, p);
    assert!((85.0..=90.0).contains(&took), "self damage {took}");
    // 5 x 89.6 = 448 along the line from 10 below the blast to his centre
    // (mostly up).
    assert!((380.0..=450.0).contains(&up), "pushed up at {up}");
    assert!(jumping, "blast jumping");
    sim.seconds(2.0);
    assert!(sim.app.world().get::<weapons::blast::BlastJumping>(p).is_none(), "landed: no longer blast jumping");
}

/// 5.4, test 43: a Heavy takes half the push a Soldier takes from the
/// same rocket.
#[test]
fn a_heavy_takes_half_the_knockback_43() {
    let push_on = |gun: &str| {
        let mut sim = sim();
        let from = greybox::SPAWNS[0];
        let (p, _) = holding(&mut sim, from, ROCKET_LAUNCHER);
        let t = target(&mut sim, from, 400.0);
        let w = give(sim.app.world_mut(), t, gun).unwrap();
        sim.app.world_mut().get_mut::<Inventory>(t).unwrap().wanted = Some(w);
        sim.seconds(1.0);
        sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 10.0;
        let aim = sim.position(t) - Vec3::X * 12.0 * U + Vec3::Y * 3.0 * U;
        testkit::aim_at(&mut sim, p, aim);
        let before = health_points(&sim, t);
        let (_, flat) = peak_speeds(&mut sim, p, t, 60, |s, k| s.intent(p).fire = k == 0);
        (before - health_points(&sim, t), flat)
    };
    let (soldier_took, soldier) = push_on(ROCKET_LAUNCHER);
    let (heavy_took, heavy) = push_on(MINIGUN);
    assert!((soldier_took - heavy_took).abs() < 0.01 && soldier_took > 80.0, "{soldier_took} {heavy_took}");
    assert!(soldier > 300.0, "the Soldier pushed at {soldier}");
    assert!((heavy / soldier - 0.5).abs() < 0.05, "Heavy {heavy}, Soldier {soldier}");
}

/// 5.4, test 47: bullets push too: a Scattergun's 10 pellets on an enemy
/// at 512 (60 damage) push him 360 units/s from the shooter.
#[test]
fn a_scattergun_hit_pushes_360_47() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, SCATTERGUN);
    no_spread(&mut sim, w);
    let t = target(&mut sim, from, 512.0);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = health_points(&sim, t);
    let (_, flat) = peak_speeds(&mut sim, p, t, 4, |s, k| s.intent(p).fire = k == 0);
    assert!((before - health_points(&sim, t) - 60.0).abs() < 0.01);
    // Friction may take a tick's share (4 x 0.015 x 360) before it's read.
    assert!((330.0..=361.0).contains(&flat), "pushed at {flat}");
    assert!(sim.velocity(t).z < 0.0, "away from the shooter (along -Z)");
}
