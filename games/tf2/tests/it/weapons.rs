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

/// 7.1 (corrected by minigun.md), tests 35 and 36: no gameplay
/// wind-down. Let go of attack2 while spun: idle in play 0.3 s later
/// (20 ticks: the speed back, a jump works), only the switch waits for
/// the spool-down to count as finished (77 ticks, minigun.md 3.3).
#[test]
fn minigun_stops_0_3_s_after_attack2_and_only_the_switch_waits_35_36() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], MINIGUN);
    run(&mut sim, p, 80, |s, _| s.intent(p).secondary = true);
    assert!(sim.app.world().get::<Tf2Minigun>(w).unwrap().state.turning());
    run(&mut sim, p, 19, |s, _| s.intent(p).secondary = false);
    assert_eq!(sim.app.world().get::<Weapon>(w).unwrap().max_speed, Some(110.0 * U), "still slowed 19 ticks on");
    run(&mut sim, p, 1, |_, _| {});
    assert_eq!(sim.app.world().get::<Weapon>(w).unwrap().max_speed, None, "the speed back 20 ticks on");
    let mut rose = false;
    run(&mut sim, p, 10, |s, k| {
        s.intent(p).jump = k == 0;
        rose |= s.velocity(p).y > 0.5;
    });
    assert!(rose, "a jump once idle");
    assert!(sim.app.world().get::<HolsterLocked>(w).is_some(), "the switch waits for the spool-down");
    // Allowed from the 77th tick after the wind-down (minigun.md 3.3:
    // `m_spool_down` past cycle 0.85, seen before the tick's advance).
    run(&mut sim, p, 66, |_, _| {});
    assert!(sim.app.world().get::<HolsterLocked>(w).is_some(), "still locked 76 ticks after the wind-down");
    run(&mut sim, p, 1, |_, _| {});
    assert!(sim.app.world().get::<HolsterLocked>(w).is_none(), "unlocked 77 ticks after the wind-down");
    assert_eq!(sim.app.world().get::<Tf2Minigun>(w).unwrap().state, weapons::minigun::State::Idle);
}

/// minigun.md 3.3: an empty Minigun let go of switches away only with the
/// player's `cl_autoswitch_empty 1` (ours' default is 0: it stays in
/// hand, dry). On: the switch comes 77 ticks after the wind-down.
fn empty_minigun_with(on: bool) -> (Sim, Entity, Entity, Entity) {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], MINIGUN);
    let other = give(sim.app.world_mut(), p, SCATTERGUN).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.ticks(100);
    if on {
        let info = chimera_core::core::UserInfo(
            [("cl_autoswitch_empty".to_string(), "1".to_string())].into_iter().collect(),
        );
        sim.app.world_mut().entity_mut(p).insert(info);
    }
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 0;
        m.reserve = 0;
    }
    (sim, p, w, other)
}

#[test]
fn an_empty_minigun_stays_in_hand_without_cl_autoswitch_empty() {
    let (mut sim, p, w, _) = empty_minigun_with(false);
    run(&mut sim, p, 400, |s, _| s.intent(p).fire = false);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "still in hand");
}

#[test]
fn an_empty_minigun_switches_away_with_cl_autoswitch_empty() {
    let (mut sim, p, w, other) = empty_minigun_with(true);
    run(&mut sim, p, 400, |s, _| s.intent(p).fire = false);
    let inv = sim.app.world().get::<Inventory>(p).unwrap();
    assert_eq!(inv.active, Some(other), "switched to the weapon with ammo (was {w:?})");
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
/// at 0.7 s and every 0.5 s after (in whole ticks, as measured); a shot
/// with shells in stops it.
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
    // 0.7 s for the first, then 0.5 s each, rounded to ticks as TF2
    // does: 34 ticks a loop (`the_scattergun_loads_a_shell_every_34_ticks`).
    let want: Vec<u64> = (0..6).map(|n| 48 + 34 * n).collect();
    assert_eq!(ins, want, "shells in (ticks after the start)");
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

/// `cl_autoreload 0` (the player's option, sent as userinfo; ours'
/// default, and a player without the setting): a clip not full stays so
/// with attack let go; reload still reloads it. 1 reloads by itself.
#[test]
fn cl_autoreload_0_leaves_a_clip_not_full_alone() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SCATTERGUN);
    let clip = |sim: &Sim| sim.app.world().get::<Magazine>(w).unwrap().clip;
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    assert_eq!(clip(&sim), 5);
    let ev = run(&mut sim, p, 150, |s, _| s.intent(p).fire = false);
    assert!(!ev.iter().any(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)), "no reload by itself");
    assert_eq!(clip(&sim), 5);
    run(&mut sim, p, 60, |s, k| s.intent(p).reload = k == 0);
    assert_eq!(clip(&sim), 6, "reload still reloads");
    sim.app.world_mut().entity_mut(p).insert(autoreload_on());
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    let ev = run(&mut sim, p, 60, |s, _| s.intent(p).fire = false);
    assert!(ev.iter().any(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)), "on again: reloads");
}

/// 4, measured on real TF2 (2026-11-20, the view model's sequence and the
/// clip read every tick): the Rocket Launcher's reload plays its whole
/// 0.5 s start, then a 0.8 s loop a round, the rocket in 0.33 s into it;
/// the next loop starts as the last ends.
#[test]
fn the_rocket_launchers_reload_plays_its_start_then_a_loop_a_round() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], ROCKET_LAUNCHER);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 2;
        m.reserve = 20;
    }
    let mut clips = Vec::new();
    let ev = run(&mut sim, p, 200, |s, k| {
        s.intent(p).reload = k == 0;
        clips.push(s.app.world().get::<Magazine>(w).unwrap().clip);
    });
    let at = |want: fn(&WeaponEventKind) -> bool| ev.iter().filter(|(_, k)| want(k)).map(|(t, _)| *t).collect::<Vec<_>>();
    let start = at(|k| matches!(k, WeaponEventKind::ReloadStarted))[0];
    let loops: Vec<u64> = at(|k| matches!(k, WeaponEventKind::ShellInserting)).iter().map(|t| t - start).collect();
    // TF2: the loops 34 and 88 ticks after the start, rounds 56 and 110.
    assert_eq!(loops, [34, 88], "loops (ticks after the start)");
    let rounds: Vec<u64> = (3..=4).map(|n| clips.iter().position(|c| *c >= n).unwrap() as u64 - 1 - start).collect();
    assert_eq!(rounds, [56, 110], "rounds in (ticks after the start)");
    let done = at(|k| matches!(k, WeaponEventKind::Reloaded));
    assert_eq!(done.iter().map(|t| t - start).collect::<Vec<_>>(), [142], "the finish (TF2: 143)");
}

/// 4, measured on real TF2 (2026-11-20, the view model's sequence and the
/// clip read every tick): the Scattergun's start takes 14 ticks, then a
/// loop every 34 (0.5 s timed from the tick the last ended, 33.3 ticks
/// rounded up each time: `ShellReload::from_tick`), each shell in at its
/// loop's end as the next starts. From the due time they'd alternate 33
/// and 34.
#[test]
fn the_scattergun_loads_a_shell_every_34_ticks() {
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
    let loops: Vec<u64> =
        ev.iter().filter(|(_, k)| matches!(k, WeaponEventKind::ShellInserting)).map(|(t, _)| t - start).collect();
    assert_eq!(loops, [14, 48, 82, 116, 150, 184], "loops (ticks after the start)");
    let ins: Vec<u64> = (1..=6).map(|n| clips.iter().position(|c| *c >= n).unwrap() as u64 - 1 - start).collect();
    assert_eq!(ins, [48, 82, 116, 150, 184, 218], "shells in at each loop's end");
}

/// The player's `cl_autoreload 1` (userinfo; ours defaults to 0).
fn autoreload_on() -> chimera_core::core::UserInfo {
    chimera_core::core::UserInfo([("cl_autoreload".to_string(), "1".to_string())].into_iter().collect())
}

/// The flyers of `owner`.
pub(crate) fn rockets(sim: &mut Sim, owner: Entity) -> Vec<(Entity, Vec3, Vec3)> {
    let mut q = sim.app.world_mut().query::<(Entity, &Flyer, &Transform)>();
    q.iter(sim.app.world()).filter(|(_, f, _)| f.owner == Some(owner)).map(|(e, f, t)| (e, t.translation, f.velocity)).collect()
}

/// 5.1, tests 24 and 25: the rocket starts 23.5 ahead, 12 right, 3 down
/// of the eye, flies at 1100 units/s toward the crosshair's point and
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
    // Nose first: `weapons_more::every_projectile_flies_nose_first`.
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

/// A rocket among targets is measured (`BlastDealt`, the sandbox's blast
/// rings): its centre, its radius in metres, the damage at the centre and
/// each body it hurt with its distance and damage.
#[test]
fn a_rocket_among_targets_is_measured() {
    use chimera_sim::weapon::grenade::BlastDealt;
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, ROCKET_LAUNCHER);
    let t = target(&mut sim, from, 512.0);
    let beside = target(&mut sim, from + Vec3::X * 60.0 * U, 512.0);
    let aim = sim.position(t) - Vec3::X * 12.0 * U + Vec3::Y * 3.0 * U;
    testkit::aim_at(&mut sim, p, aim);
    let mut seen = sim.app.world().resource::<Messages<BlastDealt>>().get_cursor_current();
    let mut blasts: Vec<BlastDealt> = Vec::new();
    run(&mut sim, p, 60, |s, k| {
        s.intent(p).fire = k == 0;
        blasts.extend(seen.read(s.app.world().resource::<Messages<BlastDealt>>()).cloned());
    });
    assert_eq!(blasts.len(), 1, "one blast: {blasts:?}");
    let b = &blasts[0];
    assert_eq!(b.attacker, Some(p));
    assert!(b.weapon.is_some_and(|w| w.contains("rocketlauncher")), "{:?}", b.weapon);
    assert!(b.outer > 1.0 && b.outer < 5.0, "the radius in metres: {}", b.outer);
    assert!(b.max_damage > 0.5, "90 points at the centre: {}", b.max_damage);
    let direct = b.victims.iter().find(|v| v.victim == t).unwrap_or_else(|| panic!("the one hit: {b:?}"));
    assert!(direct.damage > 0.8, "a direct hit's 90 points: {}", direct.damage);
    if let Some(v) = b.victims.iter().find(|v| v.victim == beside) {
        assert!(v.distance_m > direct.distance_m && v.damage < direct.damage, "the splash falls off: {v:?} {direct:?}");
    }
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

/// 5.4's push on a body of another size: the volume is TF2's hull for
/// the stance, whatever the box (blast.rs, `tf2_hull`), so a TF2 body
/// fitted into a CS:S-sized map's envelope (32 x 32 x 72: 2.56 x less
/// volume, once pushed 922 units/s) and another game's body (the
/// placeholder movement's) both take test 47's 360 units/s.
#[test]
fn a_scattergun_pushes_any_body_as_a_tf2_player_47() {
    use chimera_core::{
        core::MapGame,
        envelope::{MovementEnvelope, MovementEnvelopes},
    };
    let push_on = |small_map: bool, movement: &'static str| {
        let mut sim = sim();
        if small_map {
            let env = MovementEnvelope { radius: 16.0 * U, height: 72.0 * U, crouch_height: 54.0 * U, step: 18.0 * U, eye: 64.0 * U, crouch_eye: 46.0 * U };
            let w = sim.app.world_mut();
            w.resource_mut::<MovementEnvelopes>().0.push(("knockback_test", env));
            w.insert_resource(MapGame("knockback_test".into()));
        }
        let from = greybox::SPAWNS[0];
        let (p, w) = holding(&mut sim, from, SCATTERGUN);
        no_spread(&mut sim, w);
        let t = sim.spawn_character(from - Vec3::Z * 512.0 * U, movement);
        sim.seconds(1.0);
        sim.app.world_mut().get_mut::<Health>(t).unwrap().current = 10.0;
        let at = sim.position(t);
        testkit::aim_at(&mut sim, p, at);
        let before = health_points(&sim, t);
        let (_, flat) = peak_speeds(&mut sim, p, t, 4, |s, k| s.intent(p).fire = k == 0);
        (before - health_points(&sim, t), flat)
    };
    for (small, movement) in [(true, movement::ID), (false, chimera_sim::movement::placeholder::ID)] {
        let (took, flat) = push_on(small, movement);
        assert!((took - 60.0).abs() < 0.01, "{movement} small map {small}: took {took}");
        assert!((330.0..=361.0).contains(&flat), "{movement} small map {small}: pushed at {flat}");
    }
}

/// weapons.md 4, step 4 (`cl_autoreload` 1; real TF2 captured 2026-11-19:
/// the Scattergun's reload starts about 0.7 s after one shot): a clip not
/// full reloads once neither attack is held and the gun may fire again;
/// an attack stops it at once, in its start too (step 3).
#[test]
fn a_clip_not_full_reloads_when_attack_is_let_go() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SCATTERGUN);
    sim.app.world_mut().entity_mut(p).insert(autoreload_on());
    let clip = |sim: &Sim| sim.app.world().get::<Magazine>(w).unwrap().clip;
    let events = run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    assert!(events.iter().any(|(_, k)| matches!(k, WeaponEventKind::Fired { .. })), "fired: {events:?}");
    assert_eq!(clip(&sim), 5);
    let held = run(&mut sim, p, 60, |s, _| s.intent(p).fire = false);
    let start = held.iter().find(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)).map(|(t, _)| *t);
    let start = start.expect("reloads with attack let go");
    // The Scattergun's 0.625 s interval: about 42 ticks after the shot.
    assert!((38..=44).contains(&start), "the reload started {start} ticks after the shot");
    run(&mut sim, p, 60, |s, _| s.intent(p).fire = false);
    assert_eq!(clip(&sim), 6, "the round went back in");
    // Step 3: an attack in the reload's start stops it and fires at once.
    run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    let mut started = false;
    for _ in 0..60 {
        if run(&mut sim, p, 1, |s, _| s.intent(p).fire = false).iter().any(|(_, k)| matches!(k, WeaponEventKind::ReloadStarted)) {
            started = true;
            break;
        }
    }
    assert!(started, "the reload started");
    let shot = run(&mut sim, p, 1, |s, _| s.intent(p).fire = true);
    assert!(shot.iter().any(|(_, k)| matches!(k, WeaponEventKind::Fired { .. })), "fired in the start: {shot:?}");
}
