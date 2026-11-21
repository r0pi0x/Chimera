//! Halo's movement on another game's map (the movement envelope rule,
//! `chimera_core::envelope`, `movement::body`): on synthetic levels built
//! in code (`movement`'s test level, the spec's frame, world units), a
//! Halo player on a map whose game's envelope is smaller than the Spartan
//! passes a 0.9 m corridor with a 0.81 m wide envelope and is blocked
//! without one, climbs a 0.45 m step with a 0.46 m envelope step and not
//! without, and walks under a 1.7 m ceiling standing with a 1.58 m tall
//! envelope, its eye under it; on a map of Halo's own game the positions
//! are the same to the bit as with no envelope.

use bevy::prelude::*;
use chimera_core::{
    core::MapGame,
    envelope::{MovementEnvelope, MovementEnvelopes},
};
use halo_ce::game::{
    METERS_PER_UNIT,
    maps::space,
    movement::{self, HaloMovement, MovementData},
};

use crate::movement::{T, aabb, floor};

/// The made-up game whose map the tests stand on.
const GAME: &str = "envelope_test";

fn spartan() -> MovementEnvelope {
    movement::envelope(&MovementData::builtin())
}

/// A player standing at rest at the origin of `blocks`, on a map whose
/// game's envelope is `env` (None: no map game).
fn on_map(blocks: Vec<crate::movement::Block>, env: Option<MovementEnvelope>) -> T {
    let mut t = T::new(blocks, Vec3::ZERO);
    if let Some(env) = env {
        let w = t.sim.app.world_mut();
        w.resource_mut::<MovementEnvelopes>().0.push((GAME, env));
        w.insert_resource(MapGame(GAME.into()));
    }
    t.ticks(10);
    assert!(!t.mv().airborne, "settled");
    t
}

/// The origin (feet) in the spec's frame (WU) for the body `env` gives.
fn feet(t: &T, env: Option<&MovementEnvelope>) -> Vec3 {
    let d = MovementData::builtin();
    let b = movement::body(&d, env);
    let p = Vec3::from(space::to_game(t.sim.position(t.p)));
    p - Vec3::Z * b.centre_above_origin(t.mv().crouch)
}

/// Run forward for `n` ticks.
fn run(t: &mut T, n: u64) {
    t.throttle(1.0, 0.0);
    t.ticks(n);
}

const M: f32 = 1.0 / METERS_PER_UNIT;

/// A corridor along +X from x = 1 to 4 WU, `width` metres wide.
fn corridor(width: f32) -> Vec<crate::movement::Block> {
    let half = width * M / 2.0;
    vec![
        floor(),
        aabb(Vec3::new(1.0, half, -1.0), Vec3::new(4.0, 3.0, 3.0)),
        aabb(Vec3::new(1.0, -3.0, -1.0), Vec3::new(4.0, -half, 3.0)),
    ]
}

#[test]
fn a_narrow_corridor_passes_with_the_envelope_and_blocks_without() {
    let source = MovementEnvelope { radius: 0.81 / 2.0, ..spartan() };
    let mut t = on_map(corridor(0.9), Some(source));
    run(&mut t, 80);
    let x = feet(&t, Some(&source)).x;
    assert!(x > 4.0, "through the corridor with a 0.81 m body: x {x}");

    let mut t = on_map(corridor(0.9), None);
    run(&mut t, 80);
    let x = feet(&t, None).x;
    assert!(x < 1.0, "the Spartan's 1.22 m stops at the corridor: x {x}");
}

#[test]
fn a_step_of_0_45_m_is_climbed_with_the_envelope_step() {
    let step = 0.45 * M;
    let level = || vec![floor(), aabb(Vec3::new(0.6, -60.0, -1.0), Vec3::new(60.0, 60.0, step))];
    let env = MovementEnvelope { step: 0.46, ..spartan() };
    let mut t = on_map(level(), Some(env));
    run(&mut t, 40);
    let f = feet(&t, Some(&env));
    assert!(!t.mv().airborne && (f.z - step).abs() < 1e-4 && f.x > 1.0, "on the step: {f:?}");

    let mut t = on_map(level(), None);
    run(&mut t, 40);
    let f = feet(&t, None);
    assert!(f.z < 1e-3 && f.x < 0.6, "blocked below it: {f:?}");
}

#[test]
fn a_low_ceiling_is_walked_under_standing_the_eye_under_it() {
    let ceiling = 1.7 * M;
    let level = || vec![floor(), aabb(Vec3::new(1.0, -60.0, ceiling), Vec3::new(4.0, 60.0, ceiling + 1.0))];
    // A map whose own character's eyes are low (1.4 m, 1.0 m): the
    // Spartan's eye goes EYE_MARGIN under its lowered top.
    let env = MovementEnvelope { height: 1.58, crouch_height: 1.14, eye: 1.4, crouch_eye: 1.0, ..spartan() };
    let mut t = on_map(level(), Some(env));
    let mut lowest = f32::MAX;
    t.throttle(1.0, 0.0);
    for _ in 0..40 {
        t.tick();
        let eye = (t.sim.position(t.p) + t.sim.state(t.p).eye_offset).y;
        let gap = ceiling * METERS_PER_UNIT - eye;
        lowest = lowest.min(gap);
    }
    let f = feet(&t, Some(&env));
    let m: HaloMovement = t.mv();
    assert!(f.x > 2.0 && m.crouch == 0.0 && !m.airborne, "under it standing: {f:?} {m:?}");
    assert!(lowest >= 0.1, "the eye stays 0.1 m under the ceiling: {lowest}");

    let mut t = on_map(level(), None);
    run(&mut t, 40);
    let x = feet(&t, None).x;
    assert!(x < 1.0, "the Spartan standing stops at the ceiling: x {x}");
}

/// On a map of Halo's own game the envelope is the identity: the same
/// positions to the bit as without a map game, through a run, a jump and
/// a crouch.
#[test]
fn on_halos_own_map_nothing_changes() {
    let trace = |own: bool| {
        let mut t = T::new(vec![floor()], Vec3::ZERO);
        if own {
            t.sim.app.world_mut().insert_resource(MapGame(halo_ce::game::GAME.into()));
        }
        let mut out = Vec::new();
        t.throttle(1.0, 0.3);
        for n in 0..90 {
            t.jump(n == 20);
            t.crouch((40..60).contains(&n));
            t.tick();
            out.push((t.sim.position(t.p).to_array().map(f32::to_bits), t.sim.state(t.p).eye_offset.y.to_bits()));
        }
        out
    };
    assert_eq!(trace(true), trace(false));
    let d = MovementData::builtin();
    assert!(matches!(movement::body(&d, Some(&spartan())), std::borrow::Cow::Borrowed(_)));
}

/// Over the network (`NetSim`, 50 ms): a client whose Halo player is on a
/// map of another game (an envelope of CS:S's size, 0.81 m wide, 1.58 m
/// tall, 0.46 m step) walks through a doorway 0.9 m wide and 1.7 m tall
/// that the Spartan's own body does not fit, back and forth with jumps,
/// for 20 seconds: no prediction error (as on Halo's own maps,
/// `movement_net`). Server and client derive the body from the map alike.
#[test]
fn a_smaller_body_is_predicted_without_errors_through_a_doorway() {
    use std::time::Duration;

    use avian3d::prelude::{Collider, RigidBody};
    use chimera_core::core::Intent;
    use chimera_net::{
        harness::NetSim,
        net::{memory::LinkConditions, predict::NetGraph},
    };
    use chimera_world::greybox::GreyboxMapPlugin;
    use halo_ce::{game::TICK_INTERVAL, harness::HaloSim};

    let source = MovementEnvelope { radius: 0.405, height: 1.575, crouch_height: 1.143, step: 0.457, eye: 1.626, crouch_eye: 1.194 };
    let link = LinkConditions { latency: Duration::from_millis(50), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 31, 1, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(60);
    let me = sim.local_player(0).expect("a local player");
    let theirs = sim.character_of(0).expect("the server's character");
    for app in std::iter::once(&mut sim.server.app).chain(sim.clients.iter_mut().map(|c| &mut c.app)) {
        let w = app.world_mut();
        w.resource_mut::<MovementEnvelopes>().0.push((GAME, source));
        w.insert_resource(MapGame(GAME.into()));
    }
    sim.ticks(30);
    let at = sim.server.app.world().get::<Transform>(theirs).unwrap().translation;
    let d = MovementData::builtin();
    let feet = at.y - movement::body(&d, Some(&source)).centre_above_origin(0.0) * METERS_PER_UNIT;
    // A wall across -Z 3 m ahead with a 0.9 x 1.7 m doorway in it.
    let door_z = at.z - 3.0;
    let blocks = [
        (Vec3::new(at.x - 0.45 - 2.0, feet + 2.0, door_z), Vec3::new(2.0, 2.0, 0.1)),
        (Vec3::new(at.x + 0.45 + 2.0, feet + 2.0, door_z), Vec3::new(2.0, 2.0, 0.1)),
        (Vec3::new(at.x, feet + 1.7 + 1.0, door_z), Vec3::new(0.5, 1.0, 0.1)),
    ];
    for app in std::iter::once(&mut sim.server.app).chain(sim.clients.iter_mut().map(|c| &mut c.app)) {
        for (c, h) in blocks {
            app.world_mut().spawn((RigidBody::Static, Collider::cuboid(2.0 * h.x, 2.0 * h.y, 2.0 * h.z), Transform::from_translation(c)));
        }
    }
    sim.ticks(10);
    *sim.clients[0].app.world_mut().resource_mut::<NetGraph>() = NetGraph::default();
    let (mut furthest, mut jumps) = (f32::MAX, 0);
    for k in 0..600u32 {
        let mut i = sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap();
        i.move_axis = if k % 120 < 60 { Vec2::Y } else { Vec2::NEG_Y };
        i.yaw = 0.0;
        i.jump = k % 60 == 30;
        sim.step();
        let w = sim.server.app.world();
        furthest = furthest.min(w.get::<Transform>(theirs).unwrap().translation.z);
        jumps += w.get::<HaloMovement>(theirs).unwrap().jumped as u32;
    }
    assert!(furthest < door_z - 1.0, "through the doorway: z {furthest}, the door at {door_z}");
    assert!(jumps > 3, "jumped {jumps} times");
    let g = sim.clients[0].app.world().resource::<NetGraph>().clone();
    println!("{} states checked, {} errors ({:?})", g.checked, g.errors, g.last_differing);
    assert!(g.checked > 0, "compared no state");
    assert_eq!(g.errors, 0, "mispredicted ({:?})", g.last_differing);
}
