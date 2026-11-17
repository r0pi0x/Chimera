//! A vehicle drawn at the frame rate (docs/backlog.md, "Halo: view models
//! and animations at frame rate"): a Warthog driven on the movement tests'
//! flat floor with the client's drawing on (`T::drawn`), under a party
//! app's 64 Hz engine tick and Halo's own 30 Hz, at 64 Hz and 144 Hz
//! frames. Its drawn transform (eased between Halo's ticks on Halo's
//! clock: `game::eased`), its driver (drawn with it: the engine's
//! `Carried`, `map::interp`) and the driver's third-person seat camera
//! (`MapView`, made each frame from the vehicle as drawn) each move by
//! about the same distance every frame: no frame without motion followed
//! by one with a whole tick's.

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::core::{LocalPlayer, MapView};
use halo_ce::game::{maps::space, vehicles};

use crate::movement::{T, floor};

/// What moved each frame (metres): the vehicle, its driver, the camera.
struct Steps {
    vehicle: Vec<f32>,
    rider: Vec<f32>,
    camera: Vec<f32>,
}

/// A Warthog driven forward for three seconds at a `tick_hz` engine tick,
/// then `frames` frames of `frame_hz` drawn.
fn drive(tick_hz: f64, frame_hz: f64, frames: usize) -> Steps {
    let mut t = T::drawn(vec![floor()], Vec3::new(0.0, 0.8, 0.0), tick_hz);
    let p = t.p;
    t.sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let w = vehicles::spawn_vehicle(t.sim.app.world_mut(), "warthog", Vec3::ZERO, 0.0).unwrap();
    t.sim.ticks(tick_hz as u64);
    t.sim.intent(p).use_key = true;
    t.sim.ticks(1);
    t.sim.intent(p).use_key = false;
    t.sim.ticks(1);
    assert!(t.sim.app.world().get::<vehicles::Seated>(p).is_some(), "seated");
    {
        let mut i = t.sim.intent(p);
        i.yaw = space::yaw(0.0);
        i.move_axis = Vec2::new(0.0, 1.0);
    }
    t.sim.ticks((tick_hz * 3.0) as u64);
    t.sim
        .app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(1.0 / frame_hz)));
    let at = |t: &T| {
        let w_ = t.sim.app.world();
        (
            w_.get::<Transform>(w).unwrap().translation,
            w_.get::<Transform>(p).unwrap().translation,
            w_.get::<MapView>(p).expect("the driver's third-person camera").origin,
        )
    };
    t.sim.app.update();
    let mut last = at(&t);
    let mut steps = Steps {
        vehicle: Vec::new(),
        rider: Vec::new(),
        camera: Vec::new(),
    };
    for _ in 0..frames {
        t.sim.app.update();
        let now = at(&t);
        steps.vehicle.push(now.0.distance(last.0));
        steps.rider.push(now.1.distance(last.1));
        steps.camera.push(now.2.distance(last.2));
        last = now;
    }
    steps
}

/// Every frame's step within half of the median step either way (the
/// vehicle speeding up slowly meanwhile): stepping at Halo's tick under
/// another rate gives frames without motion and frames with a tick's.
fn smooth(what: &str, steps: &[f32]) {
    let mut sorted = steps.to_vec();
    sorted.sort_by(f32::total_cmp);
    let median = sorted[sorted.len() / 2];
    assert!(median > 0.01, "{what}: moving ({median} m a frame)");
    for (k, s) in steps.iter().enumerate() {
        assert!(
            (s - median).abs() <= 0.5 * median,
            "{what}: frame {k} moved {s} m, the median {median} m: {steps:?}"
        );
    }
}

fn check(tick_hz: f64, frame_hz: f64) {
    let s = drive(tick_hz, frame_hz, (frame_hz / 2.0) as usize);
    let at = format!("{tick_hz} Hz ticks, {frame_hz} Hz frames");
    smooth(&format!("the Warthog, {at}"), &s.vehicle);
    smooth(&format!("its driver, {at}"), &s.rider);
    smooth(&format!("the seat's camera, {at}"), &s.camera);
}

#[test]
fn a_driven_warthog_moves_smoothly_under_a_party_tick() {
    check(64.0, 64.0);
    check(64.0, 144.0);
}

#[test]
fn a_driven_warthog_moves_smoothly_under_halos_own_tick() {
    check(30.0, 144.0);
}

