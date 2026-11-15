//! Halo's ladders (specs/halo_ce/movement.md section 8, "Climbable
//! surfaces", and its "Ladders" test section): every row as a scenario
//! test on `HaloSim` at 30 Hz, on the spec's level built in code: a
//! two-sided vertical sheet (a triangle mesh of zero thickness, published
//! as a ladder brush like a map's climbable sheet) in the plane x = 0, 1 WU
//! wide, over a floor, the player at (0.5, 0, 0) settled, aiming at the
//! sheet 0.5 radian up. Then the edge neighbour's hold
//! (`movement::mover::hold_on_neighbour`, `space::ladder_support`) on
//! data of our own, and through `tick` on a level of nothing.
//!
//! Tolerances: `EXACT` where the climb is the spec's exact arithmetic;
//! `TOP` near the sheet's top edge (the spec: "to within one tick for an
//! implementation whose edge shapes differ in the last 1/128", and the
//! engine's cast shapes are 1e-3 thinner than the pill: `space::SKIN`).

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use chimera_core::core::MapBrushes;
use halo_ce::game::{
    maps::{collision::ladder_brush, space},
    movement::{
        HaloMovement, MovementData,
        mover::{Hit, Neighbour, Space, Support, hold_on_neighbour},
        space::ladder_support,
        tick::{Controls, tick},
    },
};

use crate::movement::{T, aabb, floor};

const EXACT: f32 = 2e-5;
/// Positions and speeds over the sheet's top (module doc).
const TOP: f32 = 2e-3;

fn close(got: f32, want: f32, tol: f32, what: &str) {
    assert!((got - want).abs() <= tol, "{what}: got {got}, want {want} (tolerance {tol})");
}

/// The sheet's corners (spec frame) from z = `lo` to `hi`.
fn corners(lo: f32, hi: f32) -> [Vec3; 4] {
    [
        Vec3::new(0.0, -0.5, lo),
        Vec3::new(0.0, 0.5, lo),
        Vec3::new(0.0, 0.5, hi),
        Vec3::new(0.0, -0.5, hi),
    ]
}

/// The spec's level: the sheet of height `h` (from z = `lo`), the floor
/// when `with_floor`, more blocks `extra`; the player's origin at `at`
/// (not ticked yet).
fn level(lo: f32, h: f32, with_floor: bool, extra: Vec<crate::movement::Block>, at: Vec3) -> T {
    let mut blocks = extra;
    if with_floor {
        blocks.push(floor());
    }
    let mut t = T::new(blocks, at);
    let c = corners(lo, h).map(|p| space::point(p.to_array()));
    t.sim
        .app
        .world_mut()
        .spawn((RigidBody::Static, Collider::trimesh(c.to_vec(), vec![[0, 1, 2], [0, 2, 3]]), Transform::default()));
    t.sim.app.world_mut().insert_resource(MapBrushes(vec![ladder_brush(&c).unwrap()]));
    t.face(180.0);
    t
}

/// Settled on the floor at (0.5, 0, 0) before the sheet of height `h`,
/// aiming at `pitch`; no throttle yet.
fn at_the_foot(h: f32, pitch: f32) -> T {
    let mut t = level(0.0, h, true, vec![], Vec3::new(0.5, 0.0, 0.0));
    t.ticks(3);
    assert!(!t.mv().airborne, "settled");
    t.sim.intent(t.p).pitch = pitch;
    t
}

fn sheet_normal() -> Vec3 {
    space::direction([1.0, 0.0, 0.0])
}

fn on_sheet(t: &T) -> bool {
    let m = t.mv();
    !m.airborne && m.ground_normal.dot(sheet_normal()) > 0.999
}

/// From the foot, forward held, until the first tick whose z is above 1
/// (the spec's start of the climbing rows), H = 6.
fn climbing() -> T {
    let mut t = at_the_foot(6.0, 0.5);
    t.throttle(1.0, 0.0);
    for _ in 0..60 {
        t.tick();
        if t.origin().z > 1.0 {
            close(t.vel().z, 0.04875, EXACT, "climbing at full speed");
            return t;
        }
    }
    panic!("never climbed above 1");
}

/// The first table: tick by tick from the floor to over the top (H = 2.5).
#[test]
fn a_ladder_is_mounted_from_the_floor_and_climbed_over_the_top() {
    let mut t = at_the_foot(2.5, 0.5);
    t.throttle(1.0, 0.0);
    for n in 1..=7 {
        t.tick();
        assert!(!t.mv().airborne, "tick {n}: on the floor");
        assert!(!on_sheet(&t), "tick {n}: not on the sheet yet");
        close(t.speed(), 0.0106667 * n as f32, EXACT, &format!("tick {n}: speed"));
    }
    close(t.origin().x, 0.2013333, EXACT, "tick 7: x");
    // Tick 8 (k = 0): the sheet beats the floor.
    t.tick();
    assert!(on_sheet(&t), "tick 8: grounded on the sheet: {:?}", t.mv());
    close(t.origin().x, 0.2, EXACT, "tick 8: x");
    close(t.vel().length(), 0.0, EXACT, "tick 8: velocity");
    close(t.mv().landing_velocity, 0.075, EXACT, "tick 8: landing velocity");
    let vz = [0.0106667, 0.0213333, 0.032, 0.0426667, 0.04875];
    let z = [0.0106667, 0.032, 0.064, 0.1066667, 0.1554167];
    for k in 1..=5 {
        t.tick();
        close(t.vel().z, vz[k - 1], EXACT, &format!("k = {k}: v_z"));
        close(t.origin().z, z[k - 1], EXACT, &format!("k = {k}: z"));
    }
    for k in 6..=40 {
        t.tick();
        assert!(on_sheet(&t), "k = {k}: on the sheet");
        close(t.vel().z, 0.04875, EXACT, &format!("k = {k}: v_z"));
        close(t.origin().z, 0.1554167 + 0.04875 * (k - 5) as f32, 4.0 * EXACT, &format!("k = {k}: z"));
        close(t.origin().x, 0.2, EXACT, &format!("k = {k}: x"));
    }
    // To the top: the last supported tick is k = 50 (within one tick).
    let mut last = 40;
    let mut first_air = None;
    let mut last_normal = Vec3::ZERO;
    for k in 41..=55 {
        t.tick();
        if t.mv().airborne {
            first_air = Some((k, t.origin(), t.vel()));
            break;
        }
        last = k;
        last_normal = t.mv().ground_normal;
    }
    let (k, at, v) = first_air.expect("leaves the sheet at the top");
    assert!((50..=51).contains(&last) || (49..=51).contains(&last), "last supported k = {last} (spec: 50)");
    assert_eq!(k, last + 1);
    // The last contact is the top edge's: a slanted normal.
    let n = Vec3::from(space::to_game(last_normal * halo_ce::game::METERS_PER_UNIT));
    assert!(n.x > 0.9 && n.z >= 0.0 && n.z < 0.3, "the top edge's normal: {n}");
    if k == 51 {
        close(at.z, 2.397916, TOP, "k = 51: z");
        close(v.z, 0.048734, TOP, "k = 51: v_z");
        close(v.x, -0.007915, TOP, "k = 51: v_x");
    }
    // The free arc: over the sheet, the peak about 2.707 about 13 ticks on.
    let mut peak = (at.z, 0, at.x);
    for n in 1..=20 {
        t.tick();
        assert!(t.mv().airborne, "the free arc, tick {n} after leaving");
        if t.origin().z > peak.0 {
            peak = (t.origin().z, n, t.origin().x);
        }
    }
    close(peak.0, 2.7070, 0.01, "the arc's peak");
    assert!(peak.2 < 0.0, "passed over the sheet at the peak: x = {}", peak.2);
    assert!((12..=14).contains(&peak.1), "the peak 13 ticks after leaving: {}", peak.1);
}

/// The second table: the first airborne tick by the sheet's height, to
/// within one tick.
#[test]
fn the_sheet_is_lost_by_its_height() {
    for (h, want) in [(0.4, 16i32), (0.6, 20), (1.0, 28), (1.75, 43), (2.0, 49), (3.0, 69), (5.0, 110)] {
        let mut t = at_the_foot(h, 0.5);
        t.throttle(1.0, 0.0);
        let mut on = false;
        let mut got = None;
        for n in 1..=130 {
            t.tick();
            on |= on_sheet(&t);
            if on && t.mv().airborne {
                got = Some(n);
                break;
            }
        }
        let got = got.unwrap_or_else(|| panic!("H = {h}: never left"));
        assert!((got - want).abs() <= 1, "H = {h}: first airborne tick {got}, spec {want}");
    }
}

#[test]
fn at_level_aim_the_biped_stands_against_the_ladder() {
    let mut t = at_the_foot(6.0, 0.0);
    t.throttle(1.0, 0.0);
    t.ticks(8);
    for n in 0..60 {
        t.tick();
        assert!(on_sheet(&t), "tick {n}: grounded on the sheet");
        close(t.origin().x, 0.2, EXACT, "x");
        close(t.origin().z, 0.0, EXACT, "z");
        close(t.vel().length(), 0.0, EXACT, "still");
    }
}

/// Any pitch whose wish survives epsilon climbs at the full speed.
#[test]
fn the_smallest_pitches_climb_or_stand() {
    let z_after = |pitch: f32| {
        let mut t = at_the_foot(6.0, pitch);
        t.throttle(1.0, 0.0);
        t.ticks(30);
        t.origin().z
    };
    let full = z_after(0.5);
    close(z_after(0.01), full, EXACT, "pitch 0.01 climbs as 0.5");
    close(z_after(0.0003), full, EXACT, "pitch 0.0003 climbs as 0.5");
    close(z_after(0.0001), 0.0, 1e-4, "pitch 0.0001 stands");
}

#[test]
fn aiming_down_climbs_down_to_the_floor() {
    let mut t = climbing();
    t.sim.intent(t.p).pitch = -0.5;
    for n in 1..=14 {
        t.tick();
        let want = (0.04875 - 0.0106667 * n as f32).max(-0.09375);
        close(t.vel().z, want, EXACT, &format!("tick {n}: v_z"));
        assert_eq!(t.vel().z < 0.0, n >= 5, "tick {n}: negative from the 5th");
    }
    for _ in 0..40 {
        t.tick();
        assert!(on_sheet(&t), "still on the sheet");
    }
    close(t.origin().z, 0.0, EXACT, "stopped by the floor");
    close(t.vel().length(), 0.0, EXACT, "at rest");
}

#[test]
fn pulling_back_drops_off_the_ladder() {
    let mut t = climbing();
    let x0 = t.origin().x;
    t.throttle(-1.0, 0.0);
    t.tick();
    assert!(t.mv().airborne, "no contact on the first tick");
    for n in 2..=6 {
        t.tick();
        assert!(t.mv().airborne, "falling");
        close(t.vel().x, 0.0011667 * (n - 1) as f32, 1e-5, &format!("tick {n}: away at the air's rate"));
    }
    assert!(t.origin().x > x0, "away from the sheet");
}

#[test]
fn releasing_the_throttle_holds_the_biped_on_the_ladder() {
    let mut t = climbing();
    t.throttle(0.0, 0.0);
    for n in 1..=5 {
        t.tick();
        close(t.vel().z, (0.04875 - 0.0106667 * n as f32).max(0.0), EXACT, &format!("tick {n}: v_z"));
    }
    let z = t.origin().z;
    for _ in 0..30 {
        t.tick();
        assert!(on_sheet(&t), "stays on the ladder");
    }
    close(t.origin().z, z, EXACT, "does not slide down");
}

#[test]
fn a_jump_with_forward_held_resumes_the_climb() {
    let mut t = climbing();
    t.jump(true);
    t.tick();
    t.jump(false);
    assert!(t.mv().airborne, "airborne after the jump's tick");
    close(t.vel().z, 0.07, EXACT, "the jump's v_z");
    let z = t.origin().z;
    t.tick();
    assert!(on_sheet(&t), "the sheet takes it again");
    close(t.origin().z - z, 0.0664, 2e-4, "then 0.0664");
    let z = t.origin().z;
    t.tick();
    close(t.origin().z - z, 0.0558, 2e-4, "then 0.0558");
    let z = t.origin().z;
    t.tick();
    close(t.origin().z - z, 0.04875, EXACT, "then the climb");
}

#[test]
fn a_jump_with_no_throttle_leaves_the_ladder() {
    let mut t = climbing();
    t.throttle(0.0, 0.0);
    let x = t.origin().x;
    t.jump(true);
    t.tick();
    t.jump(false);
    assert!(t.mv().airborne);
    close(t.vel().z, 0.07, EXACT, "v_z");
    t.tick();
    assert!(t.mv().airborne, "leaves");
    close(t.vel().z, 0.0664, 2e-4, "a ballistic arc");
    for _ in 0..10 {
        t.tick();
        assert!(t.mv().airborne);
    }
    close(t.origin().x, x, EXACT, "x unchanged");
}

#[test]
fn crouching_slows_the_climb_but_holds() {
    let mut t = climbing();
    t.throttle(0.9, 0.0);
    t.crouch(true);
    t.ticks(8);
    close(t.vel().length(), 0.01755, 2e-4, "crouched climb by the 8th tick");
    assert!(on_sheet(&t), "still on the sheet");
}

#[test]
fn backing_off_at_the_foot_walks_away() {
    let mut t = at_the_foot(6.0, 0.0);
    t.throttle(1.0, 0.0);
    t.ticks(10);
    assert!(on_sheet(&t), "at the foot, grounded on the sheet");
    t.throttle(-1.0, 0.0);
    t.tick();
    assert!(t.mv().airborne, "tick 1: no contact");
    t.tick();
    assert!(!t.mv().airborne && !on_sheet(&t), "tick 2: on the floor");
    t.ticks(10);
    assert!(t.origin().x > 0.3, "walks away: {}", t.origin().x);
}

#[test]
fn strafing_slides_along_the_sheet() {
    let mut t = at_the_foot(6.0, 0.0);
    t.throttle(1.0, 0.0);
    t.ticks(10);
    t.throttle(0.0, 1.0);
    for n in 1..=8 {
        t.tick();
        assert!(on_sheet(&t), "tick {n}: on the sheet");
        close(t.vel().y, -(0.0106667 * n as f32).min(0.0666667), EXACT, &format!("tick {n}: toward -y"));
    }
}

#[test]
fn a_falling_biped_catches_the_ladder() {
    let mut t = level(-20.0, 20.0, false, vec![], Vec3::new(0.4, 0.0, 10.0));
    t.set_vel(Vec3::new(0.0, 0.0, -0.2));
    t.throttle(1.0, 0.0);
    let mut caught = None;
    for n in 1..=30 {
        t.tick();
        if on_sheet(&t) {
            caught = Some(n);
            break;
        }
    }
    let n = caught.expect("caught");
    assert!((18..=20).contains(&n), "caught on tick {n} (spec 19)");
    close(t.origin().x, 0.2, EXACT, "at the sheet");
    close(t.vel().z, -0.268, 0.005, "falling at the catch");
    let v = t.vel().z;
    t.tick();
    close(t.vel().z - v, 0.0106667, EXACT, "braked at the acceleration limit");
}

#[test]
fn over_the_top_onto_a_platform() {
    let wall = aabb(Vec3::new(-5.0, -5.0, -5.0), Vec3::new(0.0, 5.0, 2.5));
    let mut t = level(0.0, 2.5, true, vec![wall], Vec3::new(0.5, 0.0, 0.0));
    t.ticks(3);
    t.sim.intent(t.p).pitch = 0.5;
    t.throttle(1.0, 0.0);
    let mut left = None;
    let mut landed = None;
    for n in 1..=100 {
        t.tick();
        let m: HaloMovement = t.mv();
        if left.is_none() && n > 10 && m.airborne {
            left = Some(n);
        }
        if left.is_some() && !m.airborne {
            landed = Some(n);
            break;
        }
    }
    let left = left.expect("leaves the sheet");
    let landed = landed.expect("lands");
    assert!((58..=60).contains(&left), "lost on tick {left} (spec 59)");
    assert!((82..=84).contains(&landed), "lands on tick {landed} (spec 83)");
    close(t.origin().z, 2.5, EXACT, "on the platform");
    close(t.origin().x, -0.348, 0.02, "where it lands");
    t.ticks(20);
    assert!(!t.mv().airborne && t.origin().x < -0.5, "walks away over it");
}

// ---- The edge neighbour's hold (item 8) ----

/// A support in the plane x = 0 (normal +X) with a neighbour folded back
/// at its top edge (z = 1), normal `n`, through the edge.
fn folded(n: Vec3) -> Support {
    let edge = [Vec3::new(0.0, -0.5, 1.0), Vec3::new(0.0, 0.5, 1.0)];
    Support {
        normal: Vec3::X,
        offset: 0.0,
        neighbours: vec![Neighbour {
            normal: n,
            offset: n.dot(edge[0]),
            edge,
        }],
    }
}

#[test]
fn the_neighbour_takes_a_pill_that_ran_off_its_support() {
    let r = 0.2;
    // A neighbour leaning back by 30 degrees; the pill just past the edge,
    // moving up and slightly away from it.
    let n = Vec3::new(30f32.to_radians().cos(), 0.0, 30f32.to_radians().sin());
    let s = folded(n);
    let p = Vec3::new(0.2, 0.0, 1.05);
    let v = Vec3::new(-0.0078, 0.0, 0.04875);
    assert!(n.dot(v) > 0.0);
    let (at, held, got) = hold_on_neighbour(&s, p, v, r).expect("held");
    assert_eq!(got, n);
    close(n.dot(at) - s.neighbours[0].offset, r, 1e-6, "r in front of the neighbour");
    close(held.dot(n), -1.0 / 30.0, 1e-6, "the velocity into it");
    // The part along the plane is kept.
    close((held - n * held.dot(n)).distance(v - n * v.dot(n)), 0.0, 1e-6, "along the plane");
}

#[test]
fn the_neighbour_holds_only_within_its_limits() {
    let r = 0.2;
    let n = Vec3::new(30f32.to_radians().cos(), 0.0, 30f32.to_radians().sin());
    let s = folded(n);
    let v = Vec3::new(-0.0078, 0.0, 0.04875);
    // Moving into it (N.V <= 0): skipped.
    assert!(hold_on_neighbour(&s, Vec3::new(0.2, 0.0, 1.05), Vec3::new(-0.05, 0.0, 0.0), r).is_none());
    // Too fast away from it (N.V above 1.6/30).
    assert!(hold_on_neighbour(&s, Vec3::new(0.2, 0.0, 1.05), Vec3::new(0.1, 0.0, 0.0), r).is_none());
    // Too far from the edge ((2r)^2).
    assert!(hold_on_neighbour(&s, Vec3::new(0.2, 0.0, 1.6), v, r).is_none());
    // Behind its plane by more than r/2.
    assert!(hold_on_neighbour(&s, Vec3::new(-0.2, 0.0, 1.05), v, r).is_none());
    // A coplanar neighbour (the disc's stacks) never holds: N.V < 0 with the push.
    let flat = folded(Vec3::X);
    assert!(hold_on_neighbour(&flat, Vec3::new(0.2, 0.0, 1.05), Vec3::new(-1.0 / 128.0, 0.0, 0.04875), r).is_none());
}

/// Two ladder brushes sharing an edge: the support is the sheet the pill
/// rests on, its neighbour's face on the same side of the fold.
#[test]
fn ladder_support_finds_the_sheet_and_its_neighbour() {
    let s = halo_ce::game::METERS_PER_UNIT;
    let to = |p: Vec3| space::point(p.to_array());
    let lower = corners(0.0, 1.0).map(to);
    // The upper sheet leans back (toward -x) by 30 degrees from z = 1.
    let lean = Vec3::new(-30f32.to_radians().sin(), 0.0, 30f32.to_radians().cos());
    let upper = [
        Vec3::new(0.0, -0.5, 1.0),
        Vec3::new(0.0, 0.5, 1.0),
        Vec3::new(0.0, 0.5, 1.0) + lean,
        Vec3::new(0.0, -0.5, 1.0) + lean,
    ]
    .map(to);
    let brushes = vec![ladder_brush(&lower).unwrap(), ladder_brush(&upper).unwrap()];
    let p = space::direction([0.2, 0.0, 0.9]);
    let n = sheet_normal();
    let sup = ladder_support(&brushes, s, 0.2, p, n).expect("on the lower sheet");
    close(sup.normal.dot(n), 1.0, 1e-5, "the support's face is the touched one");
    close(sup.offset, 0.0, 1e-4, "through x = 0");
    assert_eq!(sup.neighbours.len(), 1);
    let nb = sup.neighbours[0];
    let want = space::direction([30f32.to_radians().cos(), 0.0, 30f32.to_radians().sin()]);
    close(nb.normal.dot(want), 1.0, 1e-4, "the neighbour's face on the support's side");
    close(nb.normal.dot(nb.edge[0]), nb.offset, 1e-4, "its plane holds the edge");
    // Off the sheet: no support.
    assert!(ladder_support(&brushes, s, 0.2, space::direction([2.0, 0.0, 0.5]), n).is_none());
}

/// A level of nothing but a support with a neighbour (`Space::support`):
/// the tick whose sweep meets nothing is held by the neighbour.
struct Nothing(Support);

impl Space for Nothing {
    fn cast(&self, _: Vec3, _: f32, _: Vec3) -> Option<Hit> {
        None
    }

    fn sphere_blocked(&self, _: Vec3, _: Vec3) -> bool {
        false
    }

    fn support(&self, _: Vec3, _: Vec3) -> Option<Support> {
        Some(self.0.clone())
    }
}

#[test]
fn a_tick_that_meets_nothing_is_held_by_the_neighbour() {
    let d = MovementData::builtin();
    // Engine axes (+Y up): the support's normal +X, the neighbour leaning
    // back by 30 degrees above the edge at y = 1.
    let n = Vec3::new(30f32.to_radians().cos(), 30f32.to_radians().sin(), 0.0);
    let edge = [Vec3::new(0.0, 1.0, -0.5), Vec3::new(0.0, 1.0, 0.5)];
    let space = Nothing(Support {
        normal: Vec3::X,
        offset: 0.0,
        neighbours: vec![Neighbour {
            normal: n,
            offset: n.dot(edge[0]),
            edge,
        }],
    });
    let mut me = HaloMovement {
        airborne: false,
        ground_normal: Vec3::X,
        initialized: true,
        jump_timer: d.jump_timer_max,
        ..default()
    };
    let origin = Vec3::new(0.2, 1.05 - d.radius, 0.0);
    let controls = Controls {
        aim: Vec3::NEG_X,
        forward: Vec3::NEG_X,
        left: Vec3::NEG_Z,
        speed_multiplier: 1.0,
        ..default()
    };
    let out = tick(&d, &mut me, origin, Vec3::new(0.0, 0.04875, 0.0), controls, 1.0, &space);
    assert_eq!(out.contacts, vec![n], "the neighbour is the tick's only contact");
    let p = out.origin + Vec3::Y * d.radius;
    close(n.dot(p) - space.0.neighbours[0].offset, d.radius, 1e-5, "r in front of it");
    close(out.velocity.dot(n), -1.0 / 30.0, 1e-5, "into it at 1/30");
    // Its flags are cleared: grounded only if its n_z is at least 0.7071 (0.5 here).
    assert!(!out.grounded && me.airborne, "not walkable: airborne");
}
