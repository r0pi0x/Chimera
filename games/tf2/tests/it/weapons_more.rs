//! weapons_more.md's eight guns on the headless harness (`harness::Tf2Sim`,
//! the greybox), its test cases (their numbers in the test names): the
//! Pistol, SMG and Revolver by distance, the Pistol's rate and clip, the
//! SMG's reload; the Sniper Rifle unscoped, charged and its headshot, the
//! scoped speed; the grenade's launch, direct hit and fuse; the sticky's
//! charge, arming, limit and its owner's death; the Flame Thrower's reach,
//! damage and afterburn; the needle's flight and damage. Ticks at TF2's
//! 0.015 s; the targets' health raised so a hit's whole damage shows.

use bevy::prelude::*;
use chimera_core::core::{Health, MaxSpeed};
use chimera_sim::{
    testkit,
    weapon::{
        AltModes, Magazine,
        projectile::{Flyer, FlyerGone},
    },
};
use chimera_world::{greybox, harness::Sim};
use tf2::game::{
    TICK_INTERVAL,
    weapons::{
        FLAMETHROWER, GRENADE_LAUNCHER, PISTOL_SCOUT, REVOLVER, SMG, SNIPER_RIFLE, STICKY_LAUNCHER, SYRINGE_GUN,
        flame::Burning,
    },
};

use super::weapons::{fired, health_points, hold_still, holding, no_spread, rockets as flyers, run, sim, target};

const U: f32 = 0.0254;

/// One shot of `id` at a target `units` ahead, dead centre: what it took.
fn one_shot(id: &str, units: f32) -> f32 {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, w) = holding(&mut sim, from, id);
    no_spread(&mut sim, w);
    let t = target(&mut sim, from, units);
    let at = sim.position(t);
    testkit::aim_at(&mut sim, p, at);
    let before = health_points(&sim, t);
    let ev = run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(fired(&ev).len(), 1, "{id}");
    before - health_points(&sim, t)
}

#[test]
fn pistol_smg_revolver_by_distance_1_2_3() {
    for (id, units, want) in [
        (PISTOL_SCOUT, 512.0, 15.0),
        (PISTOL_SCOUT, 1024.0, 8.0),
        (SMG, 512.0, 8.0),
        (SMG, 1024.0, 4.0),
        (REVOLVER, 256.0, 54.0),
        (REVOLVER, 512.0, 40.0),
        (REVOLVER, 1024.0, 21.0),
    ] {
        let got = one_shot(id, units);
        assert!((got - want).abs() < 0.01, "{id} at {units}: {got}, want {want}");
    }
}

/// Test 5: held, the Scout's Pistol fires every 0.15 s (10 ticks), 12
/// shots, then reloads the clip in 1.005 s from 36 carried.
#[test]
fn the_pistol_fires_every_10_ticks_and_reloads_its_clip_5() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], PISTOL_SCOUT);
    let ev = run(&mut sim, p, 300, |s, k| s.intent(p).fire = k < 170);
    let shots = fired(&ev);
    assert!(shots.len() >= 12, "{shots:?}");
    assert!(shots[..12].windows(2).all(|w| w[1] - w[0] == 10), "{shots:?}");
    // The clip reloaded by itself: 12 back (or 12 minus what fired after).
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    assert_eq!(m.clip + m.reserve + (shots.len() as u32 - 12), 12 + 24, "{m:?}");
    assert!(m.reserve <= 24, "{m:?}");
}

/// Test 7: an SMG with 3 in and 1 carried reloads 4 in 1.1 s.
#[test]
fn the_smg_moves_what_it_carries_7() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SMG);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 3;
        m.reserve = 1;
    }
    run(&mut sim, p, 2, |s, k| s.intent(p).reload = k == 0);
    run(&mut sim, p, 60, |s, _| s.intent(p).reload = false);
    assert_eq!(sim.app.world().get::<Magazine>(w).unwrap().clip, 3, "not yet at 0.9 s");
    run(&mut sim, p, 20, |_, _| {});
    let m = sim.app.world().get::<Magazine>(w).unwrap();
    assert_eq!((m.clip, m.reserve), (4, 0));
}

/// The Sniper Rifle at a target `units` ahead, scoped for `scoped`
/// seconds first (None: not), aimed `up` metres above its centre.
fn sniper_shot(units: f32, scoped: Option<f32>, up: f32) -> (f32, f32) {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SNIPER_RIFLE);
    let t = target(&mut sim, from, units);
    let at = sim.position(t) + Vec3::Y * up;
    testkit::aim_at(&mut sim, p, at);
    let mut speed = 0.0;
    if let Some(s) = scoped {
        run(&mut sim, p, 2, |s, k| s.intent(p).secondary = k == 0);
        let n = (s as f64 / TICK_INTERVAL).round() as u64 - 2;
        run(&mut sim, p, n, |s, _| s.intent(p).secondary = false);
        speed = sim.app.world().get::<MaxSpeed>(p).and_then(|m| m.0).unwrap_or(0.0) / U;
    }
    let before = health_points(&sim, t);
    let ev = run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(fired(&ev).len(), 1);
    (before - health_points(&sim, t), speed)
}

/// Tests 10, 13, 17: 50 unscoped far away (1100 units: the greybox); scoped 2.3 s, 100; the
/// holder at 80 units/s scoped.
#[test]
fn the_sniper_rifle_charges_while_scoped_10_13_17() {
    assert!((sniper_shot(1100.0, None, 0.0).0 - 50.0).abs() < 0.01);
    let (took, speed) = sniper_shot(1000.0, Some(2.3), 0.0);
    assert!((took - 100.0).abs() < 1.5, "took {took}");
    assert!((speed - 80.0).abs() < 0.01, "speed {speed}");
}

/// Tests 11, 12: a head hit after 0.1 s scoped is a body hit (50); after
/// 0.25 s a crit (150); unscoped no crit.
#[test]
fn a_scoped_head_hit_is_a_crit_11_12() {
    // The head: the top of the 82-unit box, about 0.85 m above its centre.
    let head = 0.85;
    assert!((sniper_shot(1000.0, None, head).0 - 50.0).abs() < 0.01);
    assert!((sniper_shot(1000.0, Some(0.1), head).0 - 50.0).abs() < 0.01);
    assert!((sniper_shot(1000.0, Some(0.25), head).0 - 150.0).abs() < 0.01);
}

/// Test 16 (corrections from the SDK): after a scoped shot the scope stays
/// up until the first tick after +0.5 s (no jump meanwhile), is down until
/// the first tick after +1.4 s, then up again with the charge rising at
/// once (no 0.3 s wait): over 50 by +2.5 s.
#[test]
fn the_scope_drops_at_half_a_second_and_comes_back_at_1_4_16() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SNIPER_RIFLE);
    run(&mut sim, p, 30, |s, k| s.intent(p).secondary = k == 0);
    let mode = |sim: &Sim| sim.app.world().get::<AltModes>(w).unwrap().current;
    assert_eq!(mode(&sim), 1);
    let mut modes = Vec::new();
    let mut rose = false;
    let ev = run(&mut sim, p, 170, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).jump = k == 10;
        rose |= s.velocity(p).y > 0.5;
        modes.push(s.app.world().get::<AltModes>(w).unwrap().current);
    });
    assert_eq!(fired(&ev), [0]);
    assert!(!rose, "no jump in the 0.5 s after a scoped shot");
    // modes[k] is read before tick k runs: the shot on tick 0.
    let first_down = modes.iter().position(|m| *m == 0).expect("dropped") as f64;
    let back = modes.iter().skip(first_down as usize).position(|m| *m == 1).expect("back") as f64 + first_down;
    let t = |k: f64| (k - 1.0) * TICK_INTERVAL;
    assert!((t(first_down) - 0.5).abs() <= 0.031, "down at {} s", t(first_down));
    assert!((t(back) - 1.4).abs() <= 0.031, "back at {} s", t(back));
    let c = sim.app.world().get::<tf2::game::weapons::sniper::Tf2Sniper>(w).unwrap().state.charge;
    // 170 ticks: 2.55 s after the shot, about 1.15 s charging at once.
    assert!(c > 50.0, "charge {c}");
}

/// Tests 17b, 17c: a jump drops the scope and empties the charge, and
/// attack2 in the jump doesn't raise it; after an unscoped shot the scope
/// can't be raised for 1.67 s.
#[test]
fn a_jump_drops_the_scope_and_an_unscoped_shot_blocks_it_17b_17c() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SNIPER_RIFLE);
    let mode = |sim: &Sim| sim.app.world().get::<AltModes>(w).unwrap().current;
    run(&mut sim, p, 60, |s, k| s.intent(p).secondary = k == 0);
    assert_eq!(mode(&sim), 1);
    run(&mut sim, p, 3, |s, k| s.intent(p).jump = k == 0);
    assert_eq!(mode(&sim), 0, "the jump dropped it");
    let c = sim.app.world().get::<tf2::game::weapons::sniper::Tf2Sniper>(w).unwrap().state.charge;
    assert_eq!(c, 0.0);
    run(&mut sim, p, 4, |s, k| {
        s.intent(p).jump = false;
        s.intent(p).secondary = k == 0;
    });
    assert_eq!(mode(&sim), 0, "no scope in the jump");
    // Landed: an unscoped shot, then attack2 at +1.0 s does nothing, at
    // +1.8 s it scopes.
    run(&mut sim, p, 120, |s, _| s.intent(p).secondary = false);
    run(&mut sim, p, 2, |s, k| s.intent(p).fire = k == 0);
    run(&mut sim, p, 64, |s, k| {
        s.intent(p).fire = false;
        s.intent(p).secondary = k == 62;
    });
    assert_eq!(mode(&sim), 0, "blocked at +1.0 s");
    run(&mut sim, p, 60, |s, k| s.intent(p).secondary = k == 50);
    assert_eq!(mode(&sim), 1, "scoped at +1.8 s");
}

/// Test 21: a grenade fired level leaves at 1200 forward and 200 up and
/// falls at 800 units/s².
#[test]
fn a_grenade_leaves_at_1200_and_200_up_21() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], GRENADE_LAUNCHER);
    run(&mut sim, p, 2, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).yaw = 0.0;
        s.intent(p).pitch = 0.0;
    });
    let g = flyers(&mut sim, p);
    assert_eq!(g.len(), 1);
    let v = g[0].2 / U;
    // One move made: gravity's 800 x 0.015 off the 200 up.
    assert!((v.z + 1200.0).abs() < 0.5 && (v.y - (200.0 - 800.0 * TICK_INTERVAL as f32)).abs() < 0.5, "{v}");
}

/// Tests 22, 25, 26-like: a grenade straight into an enemy explodes on him
/// with its full 100, at any range.
#[test]
fn a_direct_grenade_hit_takes_100_22() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, GRENADE_LAUNCHER);
    let t = target(&mut sim, from, 150.0);
    // Aim so the grenade, 8 right and 6 down of the eye, meets his centre
    // (its rise over 150 units is small).
    let aim = sim.position(t) - Vec3::X * 8.0 * U + Vec3::Y * 4.0 * U;
    testkit::aim_at(&mut sim, p, aim);
    let before = health_points(&sim, t);
    run(&mut sim, p, 40, |s, k| s.intent(p).fire = k == 0);
    let took = before - health_points(&sim, t);
    assert!((took - 100.0).abs() < 0.01, "took {took}");
}

/// Test 23 and the corrections from the SDK: a grenade at the floor
/// bounces and explodes on its 2.0 s fuse, at the tenth 0.2 s check (2.1 s
/// after launch), where it is.
#[test]
fn a_rolling_grenade_explodes_at_2_1_s_23() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], GRENADE_LAUNCHER);
    let mut gone = sim.app.world().resource::<Messages<FlyerGone>>().get_cursor_current();
    let mut when = None;
    run(&mut sim, p, 200, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).pitch = -60f32.to_radians();
        let events = s.app.world().resource::<Messages<FlyerGone>>();
        if when.is_none() && gone.read(events).any(|g| g.owner == Some(p)) {
            when = Some(k);
        }
    });
    let ticks = when.expect("exploded") as f64;
    assert!((ticks * TICK_INTERVAL - 2.1).abs() < 0.05, "after {} s", ticks * TICK_INTERVAL);
}

/// weapons_more.md 6 and its corrections from the SDK: a Demoman on the
/// ground detonating his sticky at his feet takes the base 120 x the
/// splash x0.75 (grounded too), and is pushed up at scale 9 into a sticky
/// jump.
#[test]
fn a_sticky_at_his_feet_hurts_the_demoman_three_quarters_and_lifts_him() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], STICKY_LAUNCHER);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 10.0;
    run(&mut sim, p, 2, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).pitch = -89f32.to_radians();
    });
    run(&mut sim, p, 80, |s, _| s.intent(p).fire = false);
    assert_eq!(stickies(&mut sim, p).len(), 1, "stuck at his feet");
    let before = health_points(&sim, p);
    let mut up = 0f32;
    let mut jumping = false;
    run(&mut sim, p, 10, |s, k| {
        s.intent(p).secondary = k < 2;
        up = up.max(s.velocity(p).y / U);
        jumping |= s.app.world().get::<tf2::game::weapons::blast::BlastJumping>(p).is_some();
    });
    let took = before - health_points(&sim, p);
    // 120 x f(r) x 0.75 with r a few units: 85 to 90 (on the ground the
    // old rule gave over 110).
    assert!((80.0..=90.0).contains(&took), "self damage {took}");
    assert!(up > 500.0, "pushed up at {up} (scale 9 x {took})");
    assert!(jumping, "sticky jumping");
}

/// The stickies of `owner` with their speed.
fn stickies(sim: &mut Sim, owner: Entity) -> Vec<(Entity, Flyer)> {
    let mut q = sim.app.world_mut().query::<(Entity, &Flyer)>();
    q.iter(sim.app.world()).filter(|(_, f)| f.owner == Some(owner) && f.weapon == STICKY_LAUNCHER).map(|(e, f)| (e, f.clone())).collect()
}


/// Tests 28, 29: tapped, 900 forward; held 2 s, 1650; held on, it fires by
/// itself at 4 s at 2400.
#[test]
fn the_sticky_charge_sets_its_speed_28_29() {
    for (hold, want) in [(1u64, 900.0), (133, 1650.0), (400, 2400.0)] {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, greybox::SPAWNS[0], STICKY_LAUNCHER);
        let mut launched = None;
        for k in 0..hold + 2 {
            let ev = run(&mut sim, p, 1, |s, _| {
                s.intent(p).fire = k < hold;
                s.intent(p).yaw = 0.0;
                s.intent(p).pitch = 0.0;
            });
            if !fired(&ev).is_empty() {
                launched = Some(k);
                break;
            }
        }
        let k = launched.unwrap_or_else(|| panic!("held {hold}: never fired"));
        // Held on: it went off by itself at 4 s.
        if hold > 300 {
            assert!(((k as f64 * TICK_INTERVAL) - 4.0).abs() < 0.05, "at {k}");
        }
        sim.ticks(1);
        let st = stickies(&mut sim, p);
        assert_eq!(st.len(), 1);
        let fwd = -st[0].1.velocity.z / U;
        assert!((fwd - want).abs() < want * 0.02, "held {hold}: {fwd}, want {want}");
    }
}

/// Tests 30, 31, 38 and the corrections from the SDK: attack2 at 0.5 s
/// does nothing, armed at 0.8 s the sticky goes off; a ninth blows the
/// oldest; a dead Demoman's vanish.
#[test]
fn stickies_arm_limit_and_vanish_30_31_38() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], STICKY_LAUNCHER);
    let tap = |sim: &mut Sim| {
        run(sim, p, 2, |s, k| {
            s.intent(p).fire = k == 0;
            s.intent(p).pitch = -60f32.to_radians();
        });
    };
    tap(&mut sim);
    run(&mut sim, p, 31, |s, _| s.intent(p).fire = false);
    run(&mut sim, p, 1, |s, _| s.intent(p).secondary = true);
    assert_eq!(stickies(&mut sim, p).len(), 1, "not armed at 0.5 s");
    run(&mut sim, p, 20, |s, _| s.intent(p).secondary = false);
    run(&mut sim, p, 2, |s, _| s.intent(p).secondary = true);
    assert_eq!(stickies(&mut sim, p).len(), 0, "armed at 0.8 s: gone off");
    run(&mut sim, p, 2, |s, _| s.intent(p).secondary = false);
    // Nine, 0.6 s apart: eight remain.
    sim.app.world_mut().get_mut::<Magazine>(w).unwrap().reserve = 24;
    for _ in 0..9 {
        sim.app.world_mut().get_mut::<Magazine>(w).unwrap().clip = 8;
        tap(&mut sim);
        run(&mut sim, p, 40, |s, _| s.intent(p).fire = false);
    }
    assert_eq!(stickies(&mut sim, p).len(), 8);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.0;
    sim.ticks(3);
    assert_eq!(stickies(&mut sim, p).len(), 0, "a dead Demoman's vanish");
}

/// The damage `e` took each tick over `n` ticks of `input`: (tick, points).
fn hurts(sim: &mut Sim, p: Entity, e: Entity, n: u64, mut input: impl FnMut(&mut Sim, u64)) -> Vec<(u64, f32)> {
    let mut took = Vec::new();
    let mut last = health_points(sim, e);
    run(sim, p, n, |s, k| {
        input(s, k);
        let h = health_points(s, e);
        if h < last {
            took.push((k, last - h));
        }
        last = h;
    });
    took
}

/// Test 41 (corrections from the SDK): an enemy 100 units ahead in the
/// stream: damaging hits at least 0.075 s (5 ticks) apart, each between
/// 6.5 x g_age and 13 (whole points, at least 1), and he burns.
#[test]
fn the_flame_hits_every_5_ticks_and_ignites_41() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let near = target(&mut sim, from, 100.0);
    let at = sim.position(near);
    testkit::aim_at(&mut sim, p, at);
    let took = hurts(&mut sim, p, near, 100, |s, _| {
        s.intent(p).fire = true;
        hold_still(s, near);
    });
    // The afterburn's 4s come every 0.5 s; the flame's are the rest.
    assert!(took.len() >= 15, "{took:?}");
    let hits: Vec<(u64, f32)> = took.iter().copied().filter(|(_, d)| (d - 4.0).abs() > 0.01).collect();
    assert!(hits.windows(2).all(|w| w[1].0 - w[0].0 >= 5), "{hits:?}");
    assert!(hits.iter().all(|(_, d)| (5.0..=13.0).contains(d)), "{hits:?}");
    assert!(sim.app.world().get::<Burning>(near).is_some(), "burning");
}

/// Test 42: an enemy whose near side is 360 units ahead is out of reach
/// (about 340).
#[test]
fn the_flame_reaches_about_340_units_42() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let far = target(&mut sim, from, 384.0);
    let at = sim.position(far);
    testkit::aim_at(&mut sim, p, at);
    let took = hurts(&mut sim, p, far, 80, |s, _| s.intent(p).fire = true);
    assert!(took.is_empty(), "{took:?}");
}

/// Test 43: 0.28 ammo a point, a point every 2 ticks: 3 s of fire uses
/// 28 of the 200 (100 points; 1 more for the truncated remainder).
#[test]
fn the_flame_uses_0_28_ammo_a_point_43() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], FLAMETHROWER);
    let before = sim.app.world().get::<Magazine>(w).unwrap().clip;
    let ev = run(&mut sim, p, 200, |s, _| s.intent(p).fire = true);
    let shots = fired(&ev);
    assert!(shots.windows(2).all(|w| w[1] - w[0] == 2), "every 2 ticks: {shots:?}");
    let used = before - sim.app.world().get::<Magazine>(w).unwrap().clip;
    let want = (shots.len() as f32 * 0.28).floor() as u32;
    assert!(used == want || used + 1 == want, "{} points used {used}, want {want}", shots.len());
}

/// Test 44: lit by one flame hit, then out of the flames: 3.4 s of burn,
/// 4 at +0.5, +1.0 ... +3.5: 7 ticks, 28.
#[test]
fn one_hit_burns_seven_ticks_44() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let near = target(&mut sim, from, 100.0);
    let at = sim.position(near);
    testkit::aim_at(&mut sim, p, at);
    let took = hurts(&mut sim, p, near, 300, |s, k| {
        s.intent(p).fire = k == 0;
        hold_still(s, near);
    });
    assert!(!took.is_empty() && (took[0].1 - 4.0).abs() > 0.01, "the flame's hit first: {took:?}");
    let burns: Vec<(u64, f32)> = took[1..].to_vec();
    assert_eq!(burns.len(), 7, "{took:?}");
    assert!(burns.iter().all(|(_, d)| (d - 4.0).abs() < 0.01), "{took:?}");
    let first = took[0].0;
    for (i, (k, _)) in burns.iter().enumerate() {
        let want = first as f64 + 0.5 * (i + 1) as f64 / TICK_INTERVAL;
        assert!((*k as f64 - want).abs() <= 1.0, "burn {} at {k}, want {want}: {took:?}", i + 1);
    }
    assert!(sim.app.world().get::<Burning>(near).is_none(), "out");
}

/// Test 46: an enemy Pyro takes the flame's hits but no afterburn.
#[test]
fn an_enemy_pyro_takes_no_afterburn_46() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let pyro = target(&mut sim, from, 100.0);
    let w = chimera_sim::weapon::give(sim.app.world_mut(), pyro, FLAMETHROWER).unwrap();
    sim.app.world_mut().get_mut::<chimera_sim::weapon::Inventory>(pyro).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    sim.app.world_mut().get_mut::<Health>(pyro).unwrap().current = 10.0;
    let at = sim.position(pyro);
    testkit::aim_at(&mut sim, p, at);
    let took = hurts(&mut sim, p, pyro, 300, |s, k| {
        s.intent(p).fire = k < 20;
        hold_still(s, pyro);
    });
    assert!(!took.is_empty(), "hit");
    assert!(took.iter().all(|(k, _)| *k < 30), "nothing after the flames: {took:?}");
}

/// Tests 48, 50 (corrections from the SDK): a needle flies at 1000 toward
/// the crosshair's point, within 1.5 degrees of it in pitch and yaw, and
/// drops; at 512 units it deals 10.
#[test]
fn needles_fall_and_hurt_by_distance_48_50() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SYRINGE_GUN);
    run(&mut sim, p, 2, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).yaw = 0.0;
        s.intent(p).pitch = 0.0;
    });
    let n = flyers(&mut sim, p);
    assert_eq!(n.len(), 1);
    let v = n[0].2 / U;
    // One tick of 240 units/s² off its 1000.
    assert!((v.length() - 1000.0).abs() < 4.0, "{v}");
    let off = v.normalize().angle_between(Vec3::NEG_Z).to_degrees();
    assert!(off < 2.2, "{off} degrees off the aim: {v}");
    run(&mut sim, p, 200, |_, _| {});

    let mut sim = super::weapons::sim();
    let (p, _) = holding(&mut sim, from, SYRINGE_GUN);
    let t = target(&mut sim, from, 512.0);
    // Aim above to meet his centre after the drop (0.52 s at 240/s²: 32
    // units) from 6 right, 8 down.
    let aim = sim.position(t) - Vec3::X * 6.0 * U + Vec3::Y * 40.0 * U;
    testkit::aim_at(&mut sim, p, aim);
    let before = health_points(&sim, t);
    run(&mut sim, p, 60, |s, k| s.intent(p).fire = k == 0);
    let took = before - health_points(&sim, t);
    assert!((took - 10.0).abs() < 0.01, "took {took}");
}

/// Zoom (3.1), the scope a screen shows: one press of attack2 puts the
/// holder behind the Sniper Rifle's scope (`Zoomed`: a scope, field of
/// view 20) and it stays up while nothing else happens; attack2 *held*
/// toggles it at most once per 0.3 s, so a held key's scope is up or
/// down by when the frame is taken (the Windows check's run with no
/// scope in one of two identical runs held it).
#[test]
fn one_press_raises_the_scope_and_a_held_key_toggles_it_3_1() {
    use chimera_sim::weapon::Zoomed;
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], SNIPER_RIFLE);
    let zoomed = |sim: &Sim| sim.app.world().get::<Zoomed>(p).copied();
    assert_eq!(zoomed(&sim), None);
    run(&mut sim, p, 1, |s, _| s.intent(p).secondary = true);
    run(&mut sim, p, 1, |s, _| s.intent(p).secondary = false);
    for _ in 0..20 {
        run(&mut sim, p, 10, |_, _| {});
        let z = zoomed(&sim).expect("the scope stays up");
        assert!(z.scope && (z.fov - 20.0).abs() < 0.01, "{z:?}");
    }
    // Held from scoped: down at once, up again 0.3 s (20 ticks) later.
    let mut seen = Vec::new();
    for _ in 0..60 {
        run(&mut sim, p, 1, |s, _| s.intent(p).secondary = true);
        seen.push(zoomed(&sim).is_some());
    }
    let flips = seen.windows(2).filter(|w| w[0] != w[1]).count();
    assert!(flips >= 2, "a held attack2 toggles the scope: {seen:?}");
}

/// A map keeps the held models of the guns its match offers by key prefix
/// (`GameKit::stay`): every projectile's model key starts with its gun's,
/// so a kept gun keeps its projectile. Before, `tf2:rocket`, `tf2:sticky`
/// ... were dropped on every map and TF2's projectiles flew unseen (only
/// their trails showed; tools/tf2ref `stickies.tf2`).
#[test]
fn a_kept_gun_keeps_its_projectiles_model() {
    use chimera_world::map::{GameKit, MapHeldModel};
    for (gun, key, _) in tf2::game::weapons::PROJECTILES {
        let held = |k: &str| MapHeldModel {
            key: k.into(),
            model: Default::default(),
            bone: String::new(),
            muzzle: None,
            attachments: Vec::new(),
            merged: Vec::new(),
            own: None,
            nose: None,
            turned: Vec::new(),
        };
        let kit = GameKit { held: vec![held(gun), held(key)], ..Default::default() };
        let kept = kit.stay(&[gun], true);
        assert!(kept.held.iter().any(|h| h.key == *key), "{gun} keeps {key}");
    }
}

/// Every TF2 projectile (the rocket, the grenade, the sticky, the needle)
/// but a spinning one (the sticky, weapons_more.md 5.1: it tumbles from
/// its launch) flies nose first: its shown model's own forward (`MapHeldModel::nose`,
/// here a made-up slant, as a model's bone frame leaves it) stays along
/// its velocity from its launch until it stops (playtest 2026-11-18: the
/// needles flew sideways).
#[test]
fn every_projectile_flies_nose_first() {
    use chimera_world::map::loose::{LooseAsset, LooseAssets};
    for (i, (gun, key, _)) in tf2::game::weapons::PROJECTILES.iter().enumerate() {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, greybox::SPAWNS[0], gun);
        let nose = [Vec3::new(0.3, -0.8, 0.5), Vec3::Y, Vec3::NEG_X, Vec3::new(-0.2, 0.1, 0.9)][i % 4].normalize();
        let asset = LooseAsset { frame: Transform::IDENTITY, half: Vec3::splat(0.05), surface: None, nose: Some(nose) };
        sim.app.world_mut().get_resource_or_insert_with(LooseAssets::default).0.insert(key.to_string(), asset);
        let (mut seen, mut spun) = (0, 0);
        for k in 0..60u64 {
            run(&mut sim, p, 1, |s, _| {
                s.intent(p).fire = k < 2;
                s.intent(p).yaw = 0.0;
                s.intent(p).pitch = -10.0;
            });
            let w = sim.app.world_mut();
            let mut q = w.query::<(&Flyer, &Transform)>();
            for (f, t) in q.iter(w).filter(|(f, _)| f.owner == Some(p) && f.stuck_until.is_none()) {
                if f.spin != Vec3::ZERO {
                    spun += 1;
                    continue;
                }
                let Some(v) = f.velocity.try_normalize() else { continue };
                let n = t.rotation * nose;
                assert!(n.dot(v) > 0.999, "{gun} tick {k}: nose {n} along {v}");
                seen += 1;
            }
        }
        let spins = *gun == tf2::game::weapons::STICKY_LAUNCHER;
        assert!(if spins { spun > 3 && seen == 0 } else { seen > 3 && spun == 0 }, "{gun}: {seen} nose-first and {spun} spinning ticks seen");
    }
}

/// A sticky laid with the launcher still goes off on attack2 once its
/// owner holds another TF2 weapon (the game: any of the Demoman's; a
/// ladder's rung swap took the launcher, and the stickies stayed forever
/// with no explosion, playtest 2026-11-18), with its explosion drawn.
#[test]
fn stickies_detonate_with_another_tf2_weapon_out() {
    use chimera_sim::weapon::{Inventory, give};
    use tf2::game::weapons::{SHOTGUN, blast::Tf2Explosion};
    let mut sim = sim();
    let (p, _) = holding(&mut sim, greybox::SPAWNS[0], STICKY_LAUNCHER);
    run(&mut sim, p, 2, |s, k| {
        s.intent(p).fire = k == 0;
        s.intent(p).pitch = -89f32.to_radians();
    });
    run(&mut sim, p, 80, |s, _| s.intent(p).fire = false);
    assert_eq!(stickies(&mut sim, p).len(), 1, "stuck at his feet");
    let w = give(sim.app.world_mut(), p, SHOTGUN).unwrap();
    sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    sim.seconds(1.0);
    assert_eq!(stickies(&mut sim, p).len(), 1, "still there with the shotgun out");
    let mut drawn = sim.app.world().resource::<Messages<Tf2Explosion>>().get_cursor_current();
    let mut explosions = 0;
    for k in 0..5 {
        run(&mut sim, p, 1, |s, _| s.intent(p).secondary = k < 2);
        explosions += drawn.read(sim.app.world().resource::<Messages<Tf2Explosion>>()).count();
    }
    assert!(stickies(&mut sim, p).is_empty(), "detonated");
    assert_eq!(explosions, 1, "its explosion drawn");
}
