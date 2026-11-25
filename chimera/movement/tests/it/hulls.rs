//! The world traced through clip hulls (`QuakeConfig::clip_hulls`,
//! `MapClipHulls`; docs/plans/active/shared-movement.md, S7): a hull-traced
//! run of the course (`course`: a walk, a step, a slope, a landing, a duck
//! under a ceiling, a wall) goes where the box sweep's goes; the ducked
//! box is the ducked hull; a map without hulls, a movement that doesn't ask
//! for them and a box no hull is made for are swept.

use bevy::prelude::*;
use chimera_movement::QuakeConfig;

use crate::course::{self, World};

fn hulled() -> QuakeConfig {
    QuakeConfig { clip_hulls: true, ..QuakeConfig::reference() }
}

/// One run of the course's script: each tick's feet and flags (on the
/// ground, ducked).
fn run(config: QuakeConfig, world: World, ticks: u64) -> Vec<(Vec3, bool, bool)> {
    let (mut sim, p) = course::sim(config, world);
    (0..ticks)
        .map(|t| {
            let at = course::feet(&sim, p);
            course::script(t, at, &mut sim.intent(p));
            sim.ticks(1);
            let me = course::state(&sim, p);
            (course::feet(&sim, p), me.on_ground, me.ducked)
        })
        .collect()
}

#[test]
fn a_hull_traced_run_goes_where_the_box_sweep_goes() {
    const TICKS: u64 = 700;
    let swept = run(QuakeConfig::reference(), World::Brushes, TICKS);
    // The course's solids only as hulls: without the hull trace the
    // character would fall through.
    let traced = run(hulled(), World::Hulls, TICKS);
    // What the swept run met: the step's top, the ramp's top, the air and
    // a landing behind the ramp, the ducked box under the ceiling, the wall.
    let on = |x0: f32, x1: f32, z: f32| {
        swept.iter().any(|(f, ground, _)| *ground && f.x > x0 && f.x < x1 && (f.z - z).abs() < 0.1)
    };
    assert!(on(course::STEP_X, course::STEP_X + 100.0, course::STEP_HEIGHT), "walked over the step");
    assert!(swept.iter().any(|(f, ground, _)| *ground && f.z > course::ramp_top() - 2.0), "walked up the ramp");
    assert!(swept.iter().any(|(f, ground, _)| !*ground && f.x > course::RAMP_X.1), "fell off the ramp's top");
    assert!(on(course::RAMP_X.1 + 16.0, course::CEILING_X.0, 0.0), "landed behind the ramp");
    assert!(swept.iter().any(|(f, _, ducked)| *ducked && f.x > course::CEILING_X.0 && f.x < course::CEILING_X.1), "ducked under the ceiling");
    let (end, ground, ducked) = *swept.last().unwrap();
    assert!(ground && !ducked && (end.x - (course::WALL_X - 16.0)).abs() < 0.1, "stopped at the wall: {end}");
    // The hull-traced run is the same run.
    for (t, (a, b)) in swept.iter().zip(&traced).enumerate() {
        assert!(a.0.distance(b.0) < 0.05 && (a.1, a.2) == (b.1, b.2), "tick {t}: swept {a:?}, hull traced {b:?}");
    }
}

#[test]
fn the_ducked_box_is_the_ducked_hull() {
    // Standing under the 50-unit ceiling there is no room: placed ducked
    // there, a hull-traced character stays ducked with duck let go, and
    // stands up once out from under it.
    let (mut sim, p) = course::sim(hulled(), World::Hulls);
    sim.intent(p).crouch = true;
    sim.ticks(60);
    assert!(course::state(&sim, p).ducked);
    course::place(&mut sim, p, Vec3::new(course::CEILING_X.0 + 50.0, 0.0, 0.1));
    sim.ticks(10);
    let at = course::feet(&sim, p);
    assert!(course::state(&sim, p).on_ground && at.z.abs() < 0.1, "on the floor under the ceiling: {at}");
    sim.intent(p).crouch = false;
    sim.ticks(60);
    assert!(course::state(&sim, p).ducked, "no room for the standing hull");
    sim.intent(p).move_axis = Vec2::Y;
    sim.ticks(200);
    let me = course::state(&sim, p);
    assert!(!me.ducked && course::feet(&sim, p).x > course::CEILING_X.1 + 16.0, "stood up past the ceiling");
}

#[test]
fn without_hulls_or_without_asking_the_box_sweep() {
    const TICKS: u64 = 400;
    let swept = run(QuakeConfig::reference(), World::Brushes, TICKS);
    // A movement asking for hulls on a map without them.
    assert_eq!(swept, run(hulled(), World::Brushes, TICKS), "no hulls on the map: the box sweep, bit for bit");
    // A movement not asking, on a map with them.
    assert_eq!(swept, run(QuakeConfig::reference(), World::Both, TICKS), "not asked for: the box sweep, bit for bit");
    // A body no hull is made for (24 units wide): swept, so it stands on
    // the brushes and falls through hulls alone.
    let wide = QuakeConfig { half_width: 24.0, ..hulled() };
    let on_brushes = run(wide.clone(), World::Both, 60);
    assert!(on_brushes[30].1, "a 48-unit box stands on the brushes");
    let through = run(wide, World::Hulls, 60);
    assert!(!through.last().unwrap().1 && through.last().unwrap().0.z < -100.0, "and falls through hulls alone");
}
