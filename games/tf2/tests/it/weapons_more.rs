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

use super::weapons::{fired, health_points, holding, no_spread, rockets as flyers, run, sim, target};

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

/// After a scoped shot the scope drops and comes back 1.5 s later.
#[test]
fn the_scope_comes_back_after_a_shot_16() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], SNIPER_RIFLE);
    run(&mut sim, p, 30, |s, k| s.intent(p).secondary = k == 0);
    let mode = |sim: &Sim| sim.app.world().get::<AltModes>(w).unwrap().current;
    assert_eq!(mode(&sim), 1);
    run(&mut sim, p, 3, |s, k| s.intent(p).fire = k == 0);
    assert_eq!(mode(&sim), 0, "unscoped by the shot");
    run(&mut sim, p, 110, |s, _| s.intent(p).fire = false);
    assert_eq!(mode(&sim), 1, "back in");
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

/// Test 23: a grenade at the floor bounces and explodes 2.3 s after
/// launch, where it is.
#[test]
fn a_rolling_grenade_explodes_at_2_3_s_23() {
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
    assert!((ticks * TICK_INTERVAL - 2.3).abs() < 0.05, "after {} s", ticks * TICK_INTERVAL);
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

/// Tests 30, 31, 38: attack2 at 0.5 s does nothing, at 0.8 s the sticky
/// goes off; a ninth blows the oldest; a dead Demoman's vanish.
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

/// Tests 41, 42, 44: an enemy 100 units ahead takes flame ticks, rising
/// (the burn ramp), and burns; at 450 units nothing; after one tick he
/// burns at least 3 s, 4 every 0.5 s.
#[test]
fn the_flame_thrower_hurts_near_and_sets_on_fire_41_42_44() {
    let mut sim = sim();
    let from = greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, FLAMETHROWER);
    let near = target(&mut sim, from, 100.0);
    let at = sim.position(near);
    testkit::aim_at(&mut sim, p, at);
    let mut took = Vec::new();
    let mut last = health_points(&sim, near);
    run(&mut sim, p, 100, |s, _| {
        s.intent(p).fire = true;
        let h = health_points(s, near);
        if h < last {
            took.push(last - h);
        }
        last = h;
    });
    assert!(took.len() >= 15, "{took:?}");
    // 13 x the age factor at 100 units (0.83) x the burn ramp: from about 5
    // up to about 11 (the afterburn's 4s between).
    assert!(took[0] <= 6.0 && took.iter().any(|t| *t >= 10.0), "rising: {took:?}");
    assert!(sim.app.world().get::<Burning>(near).is_some(), "burning");
    // Out of the flames: the afterburn alone.
    run(&mut sim, p, 2, |s, _| s.intent(p).fire = false);
    let before = health_points(&sim, near);
    run(&mut sim, p, (3.0 / TICK_INTERVAL) as u64, |s, _| s.intent(p).fire = false);
    let burnt = before - health_points(&sim, near);
    assert!((20.0..=32.0).contains(&burnt), "afterburn {burnt}");

    // Far: nothing.
    let mut sim2 = super::weapons::sim();
    let (p2, _) = holding(&mut sim2, from, FLAMETHROWER);
    let far = target(&mut sim2, from, 450.0);
    let at = sim2.position(far);
    testkit::aim_at(&mut sim2, p2, at);
    let before = health_points(&sim2, far);
    run(&mut sim2, p2, 60, |s, _| s.intent(p2).fire = true);
    assert_eq!(health_points(&sim2, far), before);
}

/// Tests 48, 50: a needle flies at 990 and drops; at 512 units it deals 10.
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
    assert!((v.z + 990.0).abs() < 0.5 && v.y < 0.0, "{v}");
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
