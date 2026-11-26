//! Combat Arms movement in volumes (specs/combat_arms/movement.md,
//! "Volumes (containers)", "Other modifiers", "Speed limit", "Jumping",
//! "Integration and ground friction", "Stopping", "Falling and fall
//! damage"; the volume classes of world_dat.md "Moving world models and
//! volumes"): ladders, liquids and swimming, viscosity, currents,
//! friction and speed factors, gravity volumes, jump pads, safety nets
//! and pushers, as scenario tests on `CaSim` with `movement`'s test map
//! and player (`movement::T`). The spec's volume cases (56-79) are named
//! `case_<n>_...`; the rest check the prose's rules with numbers worked
//! out from the spec's formulas.
//!
//! Volumes are placed as boxes of their class with the class defaults
//! (`CaVolume::class_box`) and the properties a test sets. The stock
//! water of the maps (viscosity 0.1, current (0, 1500, 0) u/s) is
//! `stock_water`. A volume acts when the hull's box touches it; free
//! movement (swimming, ladders) follows the eye point.

use bevy::prelude::*;
use chimera_core::core::{DamageKind, SimClock, Velocity};
use combat_arms::game::{
    movement::{CaPusher, PusherPhase, movement_data},
    volumes::CaVolume,
};

use crate::movement::{T, TOL, block, close, floor, on};

/// Engine meters per world unit.
pub(crate) fn s() -> f32 {
    movement_data().meters_per_unit
}

/// A volume of `class` from `min` to `max` (u), `with` changing it.
pub(crate) fn volume(t: &mut T, class: &str, min: Vec3, max: Vec3, with: impl FnOnce(&mut CaVolume)) {
    let mut v = CaVolume::class_box(class, min * s(), max * s()).expect(class);
    with(&mut v);
    t.sim.app.world_mut().spawn(v);
}

/// The maps' usual water: viscosity 0.1, current (0, 1500, 0).
pub(crate) fn stock_water(v: &mut CaVolume) {
    v.viscosity = 0.1;
    v.current = Vec3::new(0.0, 1500.0, 0.0);
}

/// Water with no current and viscosity `c`.
fn still_water(c: f32) -> impl FnOnce(&mut CaVolume) {
    move |v| v.viscosity = c
}

/// Water with no current or viscosity from `lo` to `hi` (u).
pub(crate) fn still_water_volume(t: &mut T, (lo, hi): (Vec3, Vec3)) {
    volume(t, "Water", lo, hi, still_water(0.0));
}

pub(crate) fn set_vel(t: &mut T, v: Vec3) {
    t.sim.app.world_mut().get_mut::<Velocity>(t.p).unwrap().0 = v * s();
}

fn now(t: &T) -> f64 {
    t.sim.app.world().resource::<SimClock>().now
}

/// A wide box around the origin from height `y0` to `y1` (u).
pub(crate) fn slab(y0: f32, y1: f32) -> (Vec3, Vec3) {
    (Vec3::new(-5000.0, y0, -5000.0), Vec3::new(5000.0, y1, 5000.0))
}

const DT: f32 = 0.015;
/// e^(-k dt) with k = 5.
const E: f32 = 0.927_743_5;
const UP: f32 = std::f32::consts::FRAC_PI_2;
const DEG30: f32 = std::f32::consts::FRAC_PI_6;

// --- Ladders (cases 56-61) ---

/// Case 29 (ladder part): with a full gauge and sprint held, on a ladder:
/// no sprint.
#[test]
fn case_29_sprint_blocked_on_a_ladder() {
    let mut t = T::standing();
    t.insert(movement_data().character.clone());
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "Ladder", lo, hi, |_| {});
    t.forward(true);
    t.keys().sprint = true;
    t.ticks(5);
    assert!(t.mv().on_ladder, "on the ladder");
    assert!(!t.mv().sprint, "no sprint on a ladder");
    assert!(t.sim.state(t.p).on_ladder);
}

/// Case 31 (ladder and safety-net part): a landing from 400 u does 50
/// (case 31), but none on a ladder or in a safety-net volume.
#[test]
fn case_31_no_fall_damage_on_a_ladder_or_in_a_safety_net() {
    // A ladder from the floor up: looking down and holding forward the
    // player comes down it fast (no gravity, the viscosity only), lands,
    // and takes nothing.
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "Ladder", lo, hi, |_| {});
    t.place_in_air(on(0.0, 400.0, 0.0));
    t.keys().pitch = -UP;
    t.forward(true);
    t.ticks(200);
    assert!(t.feet() < 1.0, "came down the ladder: feet at {}", t.feet());
    assert!(t.damage().is_empty(), "{:?}", t.damage());
    // A safety net at the floor: landing from 400 u in it does nothing.
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 100.0);
    volume(&mut t, "SafteyNet", lo, hi, |_| {});
    t.place_in_air(on(0.0, 400.0, 0.0));
    t.ticks(100);
    assert!(t.mv().on_ground);
    assert!(t.damage().is_empty(), "{:?}", t.damage());
}

/// On a ladder: gravity off (a player at rest in it stays put), the
/// ladder's 200 as the speed limit (not used as a clamp), the crouch rate
/// held at 0 and the jump key does nothing.
#[test]
fn ladder_holds_the_player() {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "Ladder", lo, hi, |_| {});
    t.place_in_air(on(0.0, 300.0, 0.0));
    let before = t.pos();
    t.ticks(50);
    assert!((t.pos() - before).length() < 1e-3, "{} -> {}", before, t.pos());
    assert!(t.mv().on_ladder && t.mv().standing.is_none());
    close(t.mv().speed_limit, 200.0, TOL, "ladder speed limit");
    // Crouch held: the rate stays 0 (and the hull standing).
    t.keys().crouch = true;
    t.ticks(10);
    assert_eq!(t.mv().crouch_rate, 0.0);
    close(t.mv().half_height, 50.0, TOL, "standing hull");
}

/// The tick-1 player of case 56: standing on the floor at a ladder's
/// foot (the ladder reaching `top`), looking straight up.
fn at_ladder_foot(top: f32) -> T {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, top);
    volume(&mut t, "Ladder", lo, hi, |_| {});
    t.keys().pitch = UP;
    t
}

/// Case 56: from the floor, looking up and holding forward: gravity off
/// and no ground rules, so the player rises at once; the viscosity wipes
/// each tick's start speed while the acceleration adds less than 30 u/s.
#[test]
fn case_56_ladder_from_the_floor() {
    let mut t = at_ladder_foot(5000.0);
    let start = t.pos().y;
    t.forward(true);
    let want = [
        (1, 500.0, 7.5, 0.028125),
        (2, 590.0, 8.85, 0.0613125),
        (3, 680.0, 10.2, 0.0995625),
        (17, 1940.0, 29.1, 1.166625),
        (18, 2030.0, 30.45, 1.2808125),
        (19, 2120.0, 32.25, 1.4068125),
        (29, 3000.0, 124.2, 7.770375),
    ];
    let mut n = 0;
    for (tick, accel, speed, height) in want {
        t.ticks(tick - n);
        n = tick;
        close(t.mv().accel.y, accel, TOL, &format!("upward accel on tick {tick}"));
        close(t.vel().y, speed, TOL, &format!("end speed after tick {tick}"));
        close(t.pos().y - start, height, 2e-3, &format!("height after tick {tick}"));
        assert!(t.mv().on_ladder);
    }
    t.ticks(37 - n);
    for tick in 38..=40 {
        let before = t.pos().y;
        t.tick();
        close(t.vel().y, 245.0, TOL, &format!("end speed on tick {tick}"));
        close(t.pos().y - before, 3.16875, TOL, &format!("climb on tick {tick}"));
    }
    close(t.pos().y - start, 36.230625, 5e-3, "height after tick 40");
    close(t.vel().xz().length(), 0.0, TOL, "no horizontal speed");
}

/// Case 57: letting go at steady state: the viscosity alone stops the
/// player (200, 170, ..., 20 after it and the cap), then it hangs.
#[test]
fn case_57_letting_go_on_a_ladder() {
    let mut t = at_ladder_foot(5000.0);
    t.forward(true);
    t.ticks(45);
    close(t.vel().y, 245.0, TOL, "steady");
    t.release_all();
    let start = t.pos().y;
    for (tick, want) in [200.0, 170.0, 140.0, 110.0, 80.0, 50.0, 20.0].into_iter().enumerate() {
        t.tick();
        close(t.vel().y, want, TOL, &format!("speed after tick {}", tick + 1));
    }
    close(t.pos().y - start, 11.55, 2e-3, "height over ticks 1-7");
    let hang = t.pos();
    t.ticks(10);
    assert_eq!(t.vel(), Vec3::ZERO, "stopped from tick 8");
    assert!((t.pos() - hang).length() < 1e-3, "hangs");
}

/// Case 58: hanging in a ladder: the jump key does nothing; crouch
/// climbs down at -3000 u/s^2 (not halved, no ramp, no start), the crouch
/// rate held at 0.
#[test]
fn case_58_jump_and_crouch_on_a_ladder() {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "Ladder", lo, hi, |_| {});
    t.place_in_air(on(0.0, 300.0, 0.0));
    t.tick();
    t.keys().jump = true;
    t.ticks(3);
    let m = t.mv();
    assert!(!m.jump && !m.moving && !m.in_jump, "no jump, not moving");
    assert_eq!(t.vel(), Vec3::ZERO);
    t.keys().jump = false;
    t.keys().crouch = true;
    let start = t.pos().y;
    for (tick, speed, height) in [(1, -45.0, -0.16875), (2, -60.0, -0.5625)] {
        t.tick();
        assert_eq!(t.mv().accel, Vec3::new(0.0, -3000.0, 0.0), "tick {tick}");
        close(t.vel().y, speed, TOL, &format!("speed after tick {tick}"));
        close(t.pos().y - start, height, TOL, &format!("height after tick {tick}"));
        assert_eq!(t.mv().crouch_rate, 0.0);
    }
}

/// Case 59: climbing out over a ladder's top (the eye left it earlier):
/// the velocity kept, gravity back one tick late: 3.675 u up at 245 u/s,
/// then 3.613125 u, 228.5 u/s.
#[test]
fn case_59_ladder_top_hop() {
    let mut t = at_ladder_foot(300.0);
    t.forward(true);
    let mut ticks = 0;
    loop {
        let (before, v) = (t.pos().y, t.vel().y);
        t.tick();
        ticks += 1;
        assert!(ticks < 300, "still on the ladder at {}", t.pos());
        if !t.mv().on_ladder {
            close(v, 245.0, TOL, "steady when leaving");
            assert!(!t.mv().eye_free, "the eye left the ladder before the hull");
            close(t.pos().y - before, 3.675, TOL, "tick 1: no gravity");
            close(t.vel().y, 245.0, TOL, "tick 1: velocity kept");
            break;
        }
    }
    let before = t.pos().y;
    t.tick();
    close(t.pos().y - before, 3.613125, TOL, "tick 2: gravity back");
    close(t.vel().y, 228.5, TOL, "tick 2 speed");
}

/// Case 60: last tick's eye volume a liquid (or ladder) and this tick no
/// free movement: the velocity is zeroed (then gravity: -16.5).
#[test]
fn case_60_leaving_with_the_eye() {
    for class in ["Water", "Ladder"] {
        let mut t = T::standing();
        let (lo, hi) = slab(0.0, 1000.0);
        volume(&mut t, class, lo, hi, |v| v.viscosity = 0.0);
        t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
        t.tick();
        assert!(t.mv().eye_free, "{class}: the eye in it");
        // Out above it, eye and hull together.
        t.place_in_air(Vec3::new(0.0, 1200.0, 0.0));
        set_vel(&mut t, Vec3::new(100.0, 50.0, 0.0));
        t.tick();
        assert!(!t.mv().free_movement);
        let v = t.vel();
        close(v.x, 0.0, TOL, &format!("{class}: stopped"));
        // Off a ladder gravity returns a tick late.
        let fall = if class == "Ladder" { 0.0 } else { -16.5 };
        close(v.y, fall, TOL, &format!("{class}: from rest"));
    }
}

/// Case 61: no horizontal limit on a ladder: 300 u/s isn't clamped to the
/// run's 285; the viscosity takes 30.
#[test]
fn case_61_no_horizontal_limit_on_a_ladder() {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "Ladder", lo, hi, |_| {});
    t.place_in_air(on(0.0, 300.0, 0.0));
    t.tick();
    set_vel(&mut t, Vec3::new(300.0, 0.0, 0.0));
    t.tick();
    close(t.vel().x, 270.0, TOL, "after viscosity, not clamped");
}

/// Climbing to a ledge: a ladder in front of a 300 u ledge, looking up
/// and toward it from the floor, the player climbs past the ledge's edge
/// and ends standing on it.
#[test]
fn ladder_dismount_at_the_top() {
    // The ledge: from z = -1000 to z = -20, top at 300.
    let ledge = block(Vec3::new(0.0, 150.0, -510.0), Vec3::new(500.0, 150.0, 490.0));
    let mut t = T::new(vec![floor(), ledge], on(0.0, 0.0, -4.0));
    // The ladder: in front of the ledge's face, up to 320.
    volume(
        &mut t,
        "Ladder",
        Vec3::new(-50.0, 0.0, -20.0),
        Vec3::new(50.0, 320.0, 20.0),
        |_| {},
    );
    t.keys().pitch = 1.2;
    t.forward(true);
    // Climbing, then off the ladder and on the ledge: keys released.
    let mut ticks = 0;
    while t.mv().on_ladder || !t.mv().on_ground || t.feet() < 250.0 {
        t.tick();
        ticks += 1;
        assert!(ticks < 400, "still climbing at {}", t.pos());
    }
    t.release_all();
    t.ticks(30);
    assert!(!t.mv().on_ladder);
    assert!(t.mv().on_ground, "standing: {:?} at {}", t.mv().standing, t.pos());
    close(t.feet(), 300.0, 0.5, "on the ledge");
    assert!(t.pos().z < -20.0, "over the ledge: {}", t.pos());
}

// --- Membership, liquids and swimming (cases 62-69) ---

/// Case 62: stock water 20 u deep on the floor: the hull touches it
/// (viscosity and current act: the current lifts the player off the
/// floor, braking off), the eye doesn't (no free movement: crouching as
/// on land).
#[test]
fn case_62_box_versus_eye() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 20.0);
    volume(&mut t, "Water", lo, hi, stock_water);
    t.keys().crouch = true;
    t.tick();
    let m = t.mv();
    assert!(!m.free_movement && !m.in_liquid, "the eye isn't in it");
    close(t.vel().y, 6.0, TOL, "current 22.5 - gravity 16.5");
    close(m.crouch_rate, 0.259615, 1e-5, "crouch rate tick 1");
    t.tick();
    close(t.mv().crouch_rate, 0.463208, 1e-5, "crouch rate tick 2");
}

/// Case 63: a current acts at any viscosity and speed; a viscosity only
/// above 1 u/s.
#[test]
fn case_63_current_without_viscosity_and_slow_speeds() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "VolumeBrush", lo, hi, |v| {
        v.current = Vec3::new(0.0, 0.0, 100.0)
    });
    let before = t.pos();
    t.tick();
    close(t.vel().z, 1.5 * E, TOL, "end speed from rest");
    close(t.pos().z - before.z, 1.5 * (1.0 - E) / 5.0, 1e-4, "distance");
    for (speed, want) in [(0.9, 0.9), (5.0, 0.0), (20.0, 12.5)] {
        let mut t = T::standing();
        let (lo, hi) = slab(-10.0, 1000.0);
        volume(&mut t, "VolumeBrush", lo, hi, |v| v.viscosity = 0.1);
        t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
        set_vel(&mut t, Vec3::new(speed, 0.0, 0.0));
        t.tick();
        close(t.vel().x, want, TOL, &format!("speed {speed}"));
    }
}

/// Case 64: viscosity is skipped on every tick in a jump (2-42 for a jump
/// on tick 1): the arc is case 13's; it acts again from tick 43.
#[test]
fn case_64_viscosity_during_a_jump() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "VolumeBrush", lo, hi, |v| v.viscosity = 0.1);
    let start = t.pos().y;
    t.keys().jump = true;
    t.tick();
    t.keys().jump = false;
    close(t.vel().y, 313.5, TOL, "the jump tick");
    for tick in 2..=20 {
        t.tick();
        close(t.vel().y, 330.0 - 16.5 * tick as f32, TOL, &format!("tick {tick}"));
    }
    close(t.pos().y - start, 50.7375, 5e-3, "apex as case 13");
    let mut tick = 20;
    while t.mv().in_jump {
        t.tick();
        tick += 1;
    }
    assert_eq!(tick, 42, "landing detected on tick 42");
    // Tick 43: slowed again (and no braking).
    set_vel(&mut t, Vec3::new(100.0, 0.0, 0.0));
    t.tick();
    close(t.vel().x, 92.5 * E, TOL, "viscosity back on tick 43");
}

/// Case 65 (the maps' water lifts a player): hull and eye in it at rest,
/// each tick ends at +6 u/s and rises 0.275625 u. At the top the eye
/// leaves (the velocity zeroed once) while the hull still touches it: the
/// player floats there, bobbing.
#[test]
fn case_65_floating_in_stock_water() {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "Water", lo, hi, stock_water);
    t.place_in_air(Vec3::new(0.0, 900.0, 0.0));
    for tick in 1..=3 {
        let before = t.pos();
        t.tick();
        close(t.vel().y, 6.0, TOL, &format!("speed after tick {tick}"));
        close(t.pos().y - before.y, 0.275625, TOL, &format!("rise on tick {tick}"));
        assert!(t.mv().in_liquid && t.mv().free_movement);
    }
    // To the surface (about 70 u at 18.4 u/s) and floating there.
    t.ticks(500);
    for _ in 0..200 {
        t.tick();
        let feet = t.feet();
        assert!(feet > 900.0 && feet < 1001.0, "floating at the surface: feet {feet}");
    }
}

/// Case 66: still water (viscosity 0.03) sinks a player at rest.
#[test]
fn case_66_sinking_in_still_water() {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 5000.0);
    volume(&mut t, "Water", lo, hi, still_water(0.03));
    t.place_in_air(Vec3::new(0.0, 2000.0, 0.0));
    let start = t.pos().y;
    for (tick, want) in [(1, -16.5), (2, -30.75), (3, -45.0)] {
        t.tick();
        close(t.vel().y, want, TOL, &format!("tick {tick}"));
    }
    t.ticks(7);
    close(t.vel().y, -144.75, TOL, "tick 10");
    close(start - t.pos().y, 10.2375, 2e-3, "fallen after tick 10");
    t.ticks(10);
    close(t.vel().y, -287.25, TOL, "tick 20");
    close(start - t.pos().y, 41.85, 5e-3, "fallen after tick 20");
}

/// Case 67: swim controls: forward is the camera without an upward part;
/// crouch sinks at -1500 with the walk limit.
#[test]
fn case_67_swim_controls() {
    let swimmer = || {
        let mut t = T::standing();
        let (lo, hi) = slab(0.0, 5000.0);
        volume(&mut t, "Water", lo, hi, still_water(0.0));
        t.place_in_air(Vec3::new(0.0, 2000.0, 0.0));
        t.tick();
        t.place_in_air(Vec3::new(0.0, 2000.0, 0.0));
        t
    };
    for (pitch, back, h, y) in [
        (-DEG30, false, 433.0127, -250.0),
        (DEG30, false, 500.0, 0.0),
        (-DEG30, true, 433.0127, 250.0),
    ] {
        let mut t = swimmer();
        t.keys().pitch = pitch;
        t.keys().move_axis.y = if back { -1.0 } else { 1.0 };
        t.tick();
        let a = t.mv().accel;
        close(
            a.xz().length(),
            h,
            TOL,
            &format!("pitch {pitch} back {back}: horizontal"),
        );
        close(a.y, y, TOL, &format!("pitch {pitch} back {back}: vertical"));
    }
    let mut t = swimmer();
    t.keys().crouch = true;
    t.tick();
    let m = t.mv();
    assert_eq!(m.accel, Vec3::new(0.0, -1500.0, 0.0));
    close(m.speed_limit, 70.0, TOL, "walk limit");
    assert_eq!(m.crouch_rate, 0.0);
}

/// Case 68: a jump from the bottom of a liquid is a normal jump plus
/// +3000 u/s^2 on its tick; not standing, the key does nothing.
#[test]
fn case_68_jump_in_water() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "Water", lo, hi, still_water(0.0));
    t.tick();
    assert!(t.mv().free_movement && t.mv().on_ground, "standing on the bottom");
    t.keys().jump = true;
    t.tick();
    let m = t.mv();
    assert!(m.in_jump);
    close(m.jump_factor, 0.3, TOL, "jump factor");
    close(m.accel.y, 3000.0, TOL, "the push up");
    close(t.vel().y, 330.0 + 1900.0 * DT, TOL, "330 then +3000 - 1100");
    // Floating: nothing.
    let mut t = T::standing();
    volume(&mut t, "Water", lo, hi, still_water(0.0));
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    t.keys().jump = true;
    t.ticks(3);
    assert!(!t.mv().in_jump && !t.mv().jump_pending);
}

/// Case 69: corrosive fluid never gives free movement: forward stays
/// level, crouch halves the acceleration and crouches; its viscosity
/// and burn still act.
#[test]
fn case_69_corrosive_fluid_is_not_free_movement() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "CorrosiveFluid", lo, hi, |_| {});
    t.keys().pitch = -DEG30;
    t.forward(true);
    t.tick();
    let m = t.mv();
    assert!(!m.free_movement && !m.in_liquid);
    close(m.accel.y, 0.0, TOL, "level");
    t.keys().crouch = true;
    t.tick();
    let m = t.mv();
    close(m.accel.length(), 1500.0, TOL, "crouched: halved");
    close(m.crouch_rate, 0.259615, 1e-5, "crouching");
    assert!(!t.damage().is_empty(), "burns");
}

/// The corrosive liquid burns: 5 HP per second, as burn damage.
#[test]
fn corrosive_fluid_burns() {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "CorrosiveFluid", lo, hi, |v| v.viscosity = 0.0);
    t.ticks(10);
    let d = t.damage();
    assert!(!d.is_empty());
    for d in &d {
        assert_eq!(d.kind, DamageKind::Burn);
        close(
            d.amount * movement_data().health_points,
            5.0 * 0.05 * DT,
            1e-5,
            "burn per tick",
        );
    }
}

/// Volume damage by the hull box: a burning volume 20 u deep on the floor
/// (below the centre) burns the player standing in it.
#[test]
fn damage_by_the_hull_box() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 20.0);
    volume(&mut t, "Burn", lo, hi, |_| {});
    t.ticks(3);
    assert!(!t.damage().is_empty());
}

// --- Viscosity, currents, friction, speed factors ---

/// Viscosity takes 5000 x c x dt off the speed, and stops the braking:
/// on the floor in a volume of viscosity 0.1, sliding at 200 u/s with no
/// key: 200 - 7.5 = 192.5, then the friction alone: 192.5 e^(-k dt) =
/// 178.5906 u/s, 192.5 (1 - e^(-k dt)) / k = 2.781876 u.
#[test]
fn viscosity_slows_and_stops_the_braking() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "VolumeBrush", lo, hi, |v| v.viscosity = 0.1);
    t.tick();
    set_vel(&mut t, Vec3::new(200.0, 0.0, 0.0));
    let before = t.pos();
    t.tick();
    close(t.vel().x, 192.5 * E, TOL, "end speed");
    close(t.pos().x - before.x, 192.5 * (1.0 - E) / 5.0, TOL, "distance");
}

/// Two opposite currents cancel: braking runs as without them.
#[test]
fn opposite_currents_let_braking_run() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "Wind", lo, hi, |v| v.current = Vec3::new(100.0, 0.0, 0.0));
    volume(&mut t, "Wind", lo, hi, |v| v.current = Vec3::new(-100.0, 0.0, 0.0));
    t.tick();
    set_vel(&mut t, Vec3::new(200.0, 0.0, 0.0));
    t.tick();
    close(t.vel().x, (200.0 - 42.75) * E, TOL, "braked");
}

/// A current adds current x dt to the velocity each tick and stops the
/// braking: standing in a (0-viscosity) volume with current (100, 0, 0):
/// 1.5 u/s, then the friction: 1.5 e^(-k dt) = 1.391615 u/s.
#[test]
fn current_carries_the_player() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "Wind", lo, hi, |v| v.current = Vec3::new(100.0, 0.0, 0.0));
    let before = t.pos();
    t.tick();
    close(t.vel().x, 1.5 * E, TOL, "end speed");
    close(t.pos().x - before.x, 1.5 * (1.0 - E) / 5.0, 1e-4, "distance");
}

/// `PlayerSpeedFactor` multiplies the speed limit (f_container): 0.5 in
/// a volume: run 142.5, and two such volumes 71.25.
#[test]
fn speed_factor_scales_the_limit() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "VolumeBrush", lo, hi, |v| v.speed_factor = 0.5);
    t.steady_forward();
    close(t.mv().speed_limit, 142.5, TOL, "one volume");
    volume(&mut t, "VolumeBrush", lo, hi, |v| v.speed_factor = 0.5);
    t.tick();
    close(t.mv().speed_limit, 71.25, TOL, "two volumes");
}

/// Case 72: frictionless ground (k = 0): a steady run ends each tick at
/// 285 + 45 = 330 u/s and moves 285 dt + 3000 dt^2 / 4 = 4.44375 u.
/// Letting go: clamped to 285 at the next tick's start, then braked by
/// 42.75 a tick with no friction: 242.25, 199.5, ..., 28.5, 0 on tick 7.
/// (The spec's list, 287.25, 244.5, ..., 30.75, 0 on tick 8, leaves the
/// clamp out; the clamp is case 9's.)
#[test]
fn case_72_frictionless_ground() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "Ice", lo, hi, |_| {});
    t.steady_forward();
    let before = t.pos();
    t.tick();
    close(t.hspeed(), 330.0, TOL, "end speed");
    close(before.z - t.pos().z, 4.44375, TOL, "distance per tick");
    t.forward(false);
    let start = t.pos();
    for (tick, want) in [242.25, 199.5, 156.75, 114.0, 71.25, 28.5].into_iter().enumerate() {
        t.tick();
        close(t.hspeed(), want, TOL, &format!("speed after tick {}", tick + 1));
    }
    close(start.z - t.pos().z, 812.25 * DT, 2e-3, "distance over ticks 1-6");
    t.tick();
    assert_eq!(t.hspeed(), 0.0, "stopped on tick 7");
}

/// Case 73: the friction coefficient from the touched volumes: 5 x the
/// least `Friction` (clamped to 1), hidden ones too, never filters or
/// safety nets.
#[test]
fn case_73_friction_combinations() {
    let k = |vols: &[(&str, f32, bool)]| {
        let mut t = T::standing();
        let (lo, hi) = slab(-10.0, 1000.0);
        for &(class, friction, hidden) in vols {
            volume(&mut t, class, lo, hi, |v| {
                v.friction = friction;
                v.hidden = hidden;
            });
        }
        t.tick();
        t.mv().friction_k
    };
    close(k(&[("Water", 0.1, false)]), 0.5, 1e-5, "water with Friction 0.1");
    close(k(&[("SafteyNet", 0.0, false)]), 5.0, 1e-5, "safety net");
    close(k(&[("Filter", 0.0, false)]), 5.0, 1e-5, "filter");
    close(k(&[("Ice", 0.0, true)]), 0.0, 1e-5, "hidden ice");
    close(
        k(&[("Ice", 0.0, false), ("Water", 1.0, false)]),
        0.0,
        1e-5,
        "ice in water",
    );
    close(k(&[("VolumeBrush", 2.0, false)]), 5.0, 1e-5, "clamped to 1");
}

/// Case 74: a freezing damage volume (speed factor 0.7, friction 0): run
/// limit 199.5, steady end 244.5 and 3.16125 u per tick; two: 139.65.
#[test]
fn case_74_freezing_damage_volume() {
    let freeze = |v: &mut CaVolume| {
        v.speed_factor = 0.7;
        v.friction = 0.0;
    };
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "DamageVolume", lo, hi, freeze);
    t.steady_forward();
    close(t.mv().speed_limit, 199.5, TOL, "limit");
    let before = t.pos();
    t.tick();
    close(t.hspeed(), 244.5, TOL, "end speed");
    close(before.z - t.pos().z, 3.16125, TOL, "distance");
    volume(&mut t, "DamageVolume", lo, hi, freeze);
    t.tick();
    close(t.mv().speed_limit, 139.65, TOL, "two of them");
}

// --- Gravity volumes, jump pads (cases 70, 71, 75) ---

/// A gravity volume replaces the player's gravity (-2000 by default):
/// at rest in the air, -30 u/s and -2000 dt^2 / 4 = -0.1125 u after a
/// tick; two of them add up (-4000: -60 u/s).
#[test]
fn gravity_volume_replaces_gravity() {
    let mut t = T::standing();
    let (lo, hi) = slab(0.0, 1000.0);
    volume(&mut t, "Gravity", lo, hi, |_| {});
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    let before = t.pos();
    t.tick();
    close(t.vel().y, -30.0, TOL, "speed");
    close(t.pos().y - before.y, -0.1125, TOL, "drop");
    volume(&mut t, "Gravity", lo, hi, |_| {});
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    t.tick();
    close(t.vel().y, -60.0, TOL, "two volumes");
}

/// Case 75: a jump in a -300 gravity volume.
#[test]
fn case_75_gravity_volume() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "Gravity", lo, hi, |v| v.player_gravity = Some(-300.0));
    let start = t.pos().y;
    t.keys().jump = true;
    for (tick, height, speed) in [(1, 4.933125, 325.5), (2, 9.79875, 321.0)] {
        t.tick();
        close(t.pos().y - start, height, TOL, &format!("height after tick {tick}"));
        close(t.vel().y, speed, TOL, &format!("speed after tick {tick}"));
    }
}

/// A pad launching straight up at 800 u/s.
fn pad_up(v: &mut CaVolume) {
    v.jump_velocity = Some(Vec3::new(0.0, 800.0, 0.0));
}

/// Case 70: a jump pad fires by itself (no key) when the player stood on
/// ground last tick: its velocity added (not scaled by the jump factor),
/// then a jump; standing on it, it relaunches after each landing.
#[test]
fn case_70_jump_pad() {
    for j in [1.0, 0.3] {
        let mut t = T::standing();
        t.mv_mut().jump_factor = j;
        let (lo, hi) = slab(-10.0, 2000.0);
        volume(&mut t, "JumpVolume", lo, hi, pad_up);
        let before = t.pos();
        t.tick();
        close(t.vel().y, 783.5, TOL, &format!("j {j}: speed"));
        close(t.pos().y - before.y, 11.938125, TOL, &format!("j {j}: rise"));
        assert!(t.mv().in_jump);
        close(t.mv().jump_factor, 0.3, TOL, "jump factor");
        if j < 1.0 {
            continue;
        }
        let mut ticks = 0;
        while t.mv().in_jump {
            t.tick();
            ticks += 1;
            assert!(ticks < 500);
        }
        t.tick();
        assert!(t.mv().in_jump, "relaunched on the tick after the landing");
        close(t.vel().y, 783.5, TOL, "again");
    }
}

/// A pad's launch follows its direction (here 45 degrees toward +X): the
/// whole velocity gets it on the launch tick.
#[test]
fn jump_pad_direction() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 2000.0);
    let dir = Vec3::new(1.0, 1.0, 0.0).normalize();
    volume(&mut t, "JumpVolume", lo, hi, |v| v.jump_velocity = Some(dir * 1000.0));
    t.tick();
    close(t.vel().x, 1000.0 * dir.x, TOL, "horizontal part");
    close(t.vel().y, 1000.0 * dir.y - 16.5, TOL, "vertical part");
}

/// Case 71: falling through a pad: no launch in the air; launched on the
/// tick after the landing tick (the landing counted on ground first).
#[test]
fn case_71_falling_onto_a_pad() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 2000.0);
    // In the air first, so it didn't stand on ground last tick.
    t.place_in_air(on(0.0, 200.0, 0.0));
    t.tick();
    volume(&mut t, "JumpVolume", lo, hi, pad_up);
    let mut ticks = 0;
    while t.mv().standing.is_none() {
        t.tick();
        ticks += 1;
        assert!(!t.mv().in_jump && t.vel().y <= 0.0, "no launch in the air");
        assert!(ticks < 200);
    }
    t.tick();
    assert!(!t.mv().in_jump, "the landing tick: on ground now");
    t.tick();
    assert!(t.mv().in_jump, "launched on the next");
    close(t.vel().y, 783.5, TOL, "launch");
}

// --- Pushers (cases 76, 77) ---

fn pusher(t: &mut T, origin: Vec3, radius: f32, strength: f32, duration: f32) {
    let added = now(t);
    t.sim.app.world_mut().spawn(CaPusher {
        origin: origin * s(),
        radius,
        strength,
        added,
        delay: 0.0,
        duration,
    });
}

/// Case 76: a bullet's pusher (radius 0) never pushes.
#[test]
fn case_76_bullet_pusher() {
    let mut t = T::standing();
    let at = t.pos();
    pusher(&mut t, at - Vec3::new(10.0, 50.0, 0.0), 0.0, 400.0, 0.5);
    t.ticks(5);
    assert_eq!(t.vel(), Vec3::ZERO);
}

/// Case 77: the door kick (radius 96, strength 1000, 1 s): 48 u from the
/// centre on the ground, +500 u/s along the line each active tick
/// (ticks 1-66, removed on 67); standing on nothing +50; an upward speed
/// above 100 cut to 100 after the push.
#[test]
fn case_77_door_kick_pusher() {
    let kick = CaPusher::door_kick(Vec3::ZERO, 0.0);
    let phase = |k: i32| kick.phase(k as f64 * DT as f64, DT);
    assert_eq!(phase(0), PusherPhase::Waiting);
    assert_eq!(phase(1), PusherPhase::Active);
    assert_eq!(phase(66), PusherPhase::Active);
    assert_eq!(phase(67), PusherPhase::Over);
    // On the ground: 500, then the friction.
    let mut t = T::standing();
    let at = t.pos();
    pusher(&mut t, at - Vec3::new(48.0, 0.0, 0.0), 96.0, 1000.0, 1.0);
    t.tick();
    close(t.vel().x, 500.0 * E, TOL, "pushed");
    let v1 = t.vel().x;
    t.tick();
    assert!(t.vel().x > v1 - 42.75, "pushed again on tick 2: {}", t.vel());
    // In the air from the side: a tenth.
    let mut t = T::standing();
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    pusher(&mut t, Vec3::new(-48.0, 500.0, 0.0), 96.0, 1000.0, 1.0);
    t.tick();
    close(t.vel().x, 50.0, TOL, "a tenth");
    // In the air rising at 330, pushed from below: cut to 100.
    let mut t = T::standing();
    t.place_in_air(Vec3::new(0.0, 500.0, 0.0));
    set_vel(&mut t, Vec3::new(0.0, 330.0, 0.0));
    pusher(&mut t, Vec3::new(0.0, 452.0, 0.0), 96.0, 1000.0, 1.0);
    t.tick();
    close(t.vel().y, 100.0 - 16.5, TOL, "cut to 100");
    // Removed when over.
    let mut t = T::standing();
    pusher(&mut t, Vec3::new(0.0, 5000.0, 0.0), 96.0, 1000.0, 0.03);
    t.ticks(3);
    let w = t.sim.app.world_mut();
    let left = w.query::<&CaPusher>().iter(w).count();
    assert_eq!(left, 0, "despawned");
}

/// A wall between the pusher and the player: no push.
#[test]
fn pusher_needs_a_clear_line() {
    let wall = block(Vec3::new(-50.0, 100.0, 0.0), Vec3::new(5.0, 100.0, 100.0));
    let mut t = T::new(vec![floor(), wall], on(0.0, 0.0, 0.0));
    t.tick();
    let at = t.pos();
    pusher(&mut t, at - Vec3::new(90.0, 0.0, 0.0), 200.0, 400.0, 1.0);
    t.tick();
    close(t.vel().x, 0.0, TOL, "not pushed");
}

// --- Safety nets (cases 78, 79) ---

/// Case 78: the safety net is tested at the hull's centre: 30 u deep the
/// centre lands outside it (a 500 u drop does 100), 60 u deep inside.
#[test]
fn case_78_safety_net_at_the_centre() {
    for (depth, want) in [(30.0, Some(100.0)), (60.0, None)] {
        let mut t = T::standing();
        let (lo, hi) = slab(-10.0, depth);
        volume(&mut t, "SafteyNet", lo, hi, |_| {});
        t.place_in_air(on(0.0, 500.0, 0.0));
        t.ticks(120);
        assert!(t.mv().on_ground);
        let hp: f32 = t
            .damage()
            .iter()
            .map(|d| d.amount * movement_data().health_points)
            .sum();
        match want {
            Some(w) => close(hp, w, 0.05, &format!("net {depth} u deep")),
            None => assert_eq!(hp, 0.0, "net {depth} u deep"),
        }
    }
}

/// Case 79: a safety net (stored `Friction` 0) changes no physics: a
/// steady run in it moves 4.44784 u a tick as case 4.
#[test]
fn case_79_safety_net_physics() {
    let mut t = T::standing();
    let (lo, hi) = slab(-10.0, 1000.0);
    volume(&mut t, "SafteyNet", lo, hi, |v| v.friction = 0.0);
    t.steady_forward();
    let before = t.pos();
    t.tick();
    close(t.mv().speed_limit, 285.0, TOL, "limit");
    close(before.z - t.pos().z, 4.44784, TOL, "distance");
}

// --- Map ladder brushes (Source maps' climbable contents) ---

/// A ladder brush (`MapBrush::ladder`, as a Source map carries them) in
/// front of the player (engine -Z), 16 u thick and `top` u tall.
fn brush_ladder(t: &mut T, top: f32) {
    use chimera_core::core::{MapBrush, MapBrushes};
    let (lo, hi) = (Vec3::new(-50.0, 0.0, -80.0), Vec3::new(50.0, top, -64.0));
    let mut b = MapBrush::from_box(lo * s(), hi * s());
    b.ladder = true;
    t.sim.app.world_mut().insert_resource(MapBrushes(vec![b]));
    t.ticks(2);
}

/// A map's ladder brush works with Combat Arms' movement: walking into it
/// while looking up climbs it, at the spec's steady 245 u/s (the 200 cap, then a tick's acceleration).
#[test]
fn a_map_ladder_brush_is_climbed() {
    let mut t = T::standing();
    brush_ladder(&mut t, 600.0);
    t.keys().pitch = UP;
    t.forward(true);
    let start = t.pos().y;
    let mut peak = 0.0f32;
    for _ in 0..120 {
        t.tick();
        peak = peak.max(t.vel().y);
    }
    assert!(t.mv().on_ladder, "on the ladder at {}", t.pos());
    assert!(t.pos().y - start > 100.0, "climbed {} u", t.pos().y - start);
    // The cap (200) is applied before the tick's acceleration (case 56's
    // steady state ends each tick at 245).
    close(peak, 245.0, TOL, "steady climb speed");
}

/// Looking down and walking forward climbs down it (look-direction
/// climbing), and a jump does nothing on it.
#[test]
fn a_map_ladder_brush_is_climbed_down_looking_down() {
    let mut t = T::standing();
    brush_ladder(&mut t, 600.0);
    set_vel(&mut t, Vec3::ZERO);
    t.keys().pitch = UP;
    t.forward(true);
    t.ticks(120);
    let high = t.pos().y;
    assert!(t.mv().on_ladder);
    t.keys().pitch = -UP;
    t.ticks(60);
    assert!(t.pos().y < high - 20.0, "went down from {high} to {}", t.pos().y);
}

/// A Source map's ladder brush is solid too (the map's collision stops
/// the hull a skin short of its face): walking into it head-on, from the
/// left or from the right, looking up, climbs it. Head-on the hull never
/// overlapped the brush (the user's cs_office ladder, 2026-10-28); at a
/// slant only the hull box's corners did.
#[test]
fn a_solid_map_ladder_brush_is_climbed_from_any_side() {
    use chimera_core::core::{MapBrush, MapBrushes};
    // A ladder 100 u wide and 16 u thick against nothing, 600 u tall.
    let (lo, hi) = (Vec3::new(-50.0, 0.0, -216.0), Vec3::new(50.0, 600.0, -200.0));
    let solid = block((lo + hi) / 2.0, (hi - lo) / 2.0);
    // Toward it head-on (-Z), from its left (-X side, moving +X) and from
    // its right (+X side, moving -X), each square to the face it meets.
    for (name, at, dir) in [
        ("head-on", Vec3::new(0.0, 0.0, 0.0), Vec3::NEG_Z),
        ("from the left", Vec3::new(-200.0, 0.0, -208.0), Vec3::X),
        ("from the right", Vec3::new(200.0, 0.0, -208.0), Vec3::NEG_X),
    ] {
        let mut t = T::new(vec![floor(), solid], on(at.x, 0.0, at.z));
        t.tick();
        let mut b = MapBrush::from_box(lo * s(), hi * s());
        b.ladder = true;
        t.sim.app.world_mut().insert_resource(MapBrushes(vec![b]));
        t.ticks(2);
        t.keys().yaw = (-dir.x).atan2(-dir.z);
        t.keys().pitch = UP / 2.0;
        t.forward(true);
        let start = t.pos().y;
        let mut grabbed = false;
        for _ in 0..200 {
            t.tick();
            grabbed |= t.mv().on_ladder;
        }
        assert!(grabbed, "{name}: never on the ladder, at {}", t.pos());
        assert!(
            t.pos().y - start > 100.0,
            "{name}: climbed {} u, at {}",
            t.pos().y - start,
            t.pos()
        );
    }
}

/// Standing on top of a ladder brush (its upward face carries the flag)
/// does not grab it, as with Source's movement.
#[test]
fn standing_on_a_map_ladder_brushs_top_does_not_grab() {
    use chimera_core::core::{MapBrush, MapBrushes};
    let mut t = T::new(
        vec![floor(), block(Vec3::new(0.0, 25.0, 0.0), Vec3::new(50.0, 25.0, 50.0))],
        on(0.0, 50.0, 0.0),
    );
    let mut b = MapBrush::from_box(Vec3::new(-50.0, 0.0, -50.0) * s(), Vec3::new(50.0, 50.0, 50.0) * s());
    b.ladder = true;
    t.sim.app.world_mut().insert_resource(MapBrushes(vec![b]));
    t.ticks(60);
    assert!(!t.mv().on_ladder, "standing on the top at {}", t.pos());
    assert!(t.mv().on_ground);
}

// --- Map water (Source maps' water brushes) ---

/// A water brush of the map (`MapWater`, as a Source map carries them):
/// the slab from `y0` to `y1` (u).
fn brush_water(t: &mut T, y0: f32, y1: f32) {
    use chimera_core::core::{MapBrush, MapWater, MapWaterVolume};
    let (lo, hi) = slab(y0, y1);
    let brush = MapBrush::from_box(lo * s(), hi * s());
    t.sim
        .app
        .world_mut()
        .insert_resource(MapWater(vec![MapWaterVolume { brush, slime: false , lava: false, current: None }]));
    t.ticks(2);
}

/// A map's water brush is a liquid to Combat Arms' movement: a player in
/// it swims (free movement, in the liquid) and sinks at the liquid's
/// pace, as in case 66 (viscosity 0.035 instead of 0.03).
#[test]
fn a_map_water_brush_is_swum_in() {
    let mut t = T::standing();
    brush_water(&mut t, 0.0, 5000.0);
    t.place_in_air(Vec3::new(0.0, 2000.0, 0.0));
    t.tick();
    assert!(t.mv().in_liquid && t.mv().free_movement, "swimming");
    t.ticks(19);
    // Still water slows the fall: 20 ticks of plain gravity reach -330.
    assert!(t.vel().y < -200.0 && t.vel().y > -320.0, "sinking at {}", t.vel().y);
    // Dry, the same fall is free-fall, not swimming.
    let mut dry = T::standing();
    dry.place_in_air(Vec3::new(0.0, 2000.0, 0.0));
    dry.ticks(20);
    assert!(!dry.mv().in_liquid && !dry.mv().free_movement);
}

/// Swimming up from the bottom of a map's water: the jump key pushes up
/// as in case 68.
#[test]
fn a_map_water_brush_can_be_swum_up_in() {
    let mut t = T::standing();
    brush_water(&mut t, -10.0, 1000.0);
    assert!(t.mv().free_movement && t.mv().on_ground, "standing on the bottom");
    t.keys().jump = true;
    t.tick();
    close(t.mv().accel.y, 3000.0, TOL, "the push up");
    assert!(t.vel().y > 300.0, "rising at {}", t.vel().y);
}
