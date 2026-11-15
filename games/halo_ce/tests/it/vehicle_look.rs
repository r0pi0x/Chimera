//! What shows on a vehicle's model against specs/halo_ce/vehicles.md's
//! cases: the wheel position and the ground-speed overlay's frame, the
//! suspension's target and stored values from a trace on flat ground, the
//! overlays played on a synthetic one-bone set (continuous, wrapping), and
//! a networked client's estimate of the turn from the replicated motion.

use bevy::math::{Quat, Vec3};
use halo_ce::game::vehicles::data::VehicleDef;
use halo_ce::game::vehicles::drive::Drive;
use halo_ce::game::vehicles::look::{
    self, GROUND_SPEED, Suspension, VehicleLook, suspension_store, suspension_target, suspension_trace,
};
use halo_ce::game::vehicles::physics::{Body, FlatLevel, Level};
use chimera_world::map::anim::{AnimSet, Animation, Sequence, Track};

#[test]
fn the_wheel_position_and_the_ground_speed_frame() {
    // s = 0.255 for one tick from W = 0.9: W = 0.155 (mod 1), frame 0.62
    // of the 4-frame overlay.
    let d = VehicleDef::warthog();
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    v.wheel = 0.9;
    v.speed = 0.255;
    v.prepare(&d);
    assert!((v.wheel - 0.155).abs() < 1e-5, "W {}", v.wheel);
    assert!((v.wheel / d.wheel_circumference * 4.0 - 0.62).abs() < 1e-4);
}

/// The suspension value of a Warthog tyre whose centre is `h` above flat
/// ground, level.
fn tyre_k(h: f32) -> f32 {
    let d = VehicleDef::warthog();
    let p = d.physics_def();
    let tyre = &p.points[11];
    let c = p.center_of_mass;
    let body = Body::at(Vec3::new(0.0, 0.0, h - tyre.position.z + c.z), 0.0);
    let s = Suspension {
        point: 11,
        extension: -0.31445,
        compression: -0.13452,
    };
    let (from, by) = suspension_trace(&body, tyre.position, Vec3::Z, c, &s);
    let floor = FlatLevel {
        z: 0.0,
        material: None,
    };
    suspension_target(floor.sweep(from, from + by).map(|(t, _)| t))
}

#[test]
fn the_suspension() {
    // At rest: the tyre centre 0.2227 above, k = 0.510, a stored 0.51 kept.
    let k = tyre_k(0.2227);
    assert!((k - (-0.2227 + 0.31445) / 0.17993).abs() < 1e-4, "k {k}");
    assert!((k - 0.510).abs() < 1e-3);
    let stored = suspension_store(k, 0.51);
    assert!((stored - 0.51).abs() < 1.0 / 255.0, "stored {stored}");
    assert!(tyre_k(0.40) == 0.0, "hanging");
    assert!(tyre_k(0.10) == 1.0, "compressed");
}

/// One bone, a 4-frame overlay turning it by 0, 90, 180, 270 degrees about
/// z (the ground-speed overlay's shape).
fn spinning_set() -> AnimSet {
    let mut set = AnimSet {
        defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
        ..Default::default()
    };
    set.animations.push(Animation {
        name: "spin".into(),
        fps: 30.0,
        frames: 4,
        delta: true,
        tracks: vec![Track {
            bone: 0,
            rotation: Some((0..4).map(|f| Quat::from_rotation_z(f as f32 * std::f32::consts::FRAC_PI_2)).collect()),
            position: None,
        }],
        ..Default::default()
    });
    set.sequences.push(Sequence {
        name: GROUND_SPEED.into(),
        anims: vec![0],
        grid: (1, 1),
        ..Default::default()
    });
    set.names.insert(GROUND_SPEED.into(), 0);
    set
}

#[test]
fn the_overlays_play_continuously_and_wrap() {
    let set = spinning_set();
    let angle = |w: f32| {
        let look = VehicleLook {
            wheel: w,
            ..Default::default()
        };
        let pose = look::posed(&set, &look, 1.0);
        let (axis, a) = pose[0].0.to_axis_angle();
        a * axis.z.signum()
    };
    // W = 0.125: frame 0.5, half way to 90 degrees.
    assert!((angle(0.125) - std::f32::consts::FRAC_PI_4).abs() < 1e-4);
    // W = 0.875: frame 3.5, half way from 270 back to 0 (wrapping).
    let a = angle(0.875).rem_euclid(std::f32::consts::TAU);
    assert!((a - 7.0 * std::f32::consts::FRAC_PI_4).abs() < 1e-3, "{a}");
}

#[test]
fn a_clients_turn_estimate() {
    // A four-wheel turn τ at speed v yaws at v tan τ / (L/2).
    let (v, tau, half) = (0.2f32, 0.3f32, 0.65f32);
    let w = v * tau.tan() / half;
    let got = look::turn_from_motion(v, w, half, 30f32.to_radians()).unwrap();
    assert!((got - tau).abs() < 1e-5);
    assert_eq!(look::turn_from_motion(0.001, w, half, 1.0), None, "held when slow");
    let lock = 30f32.to_radians();
    assert!((look::turn_from_motion(v, 10.0, half, lock).unwrap() - lock).abs() < 1e-6, "clamped");
}

#[test]
fn a_body_back_from_its_drawn_transform() {
    use halo_ce::game::vehicles::{engine_rotation, level::to_engine};
    let c = Vec3::new(0.107, 0.0, 0.32);
    let mut b = Body::at(Vec3::new(3.0, -2.0, 1.5), 0.7);
    (b.forward, b.up) = b.turned(Vec3::new(0.1, 0.2, 0.0));
    let t = bevy::prelude::Transform::from_translation(to_engine(b.origin(c))).with_rotation(engine_rotation(&b));
    let back = look::body_from(&t, c);
    assert!((back.forward - b.forward).length() < 1e-4, "{} {}", back.forward, b.forward);
    assert!((back.up - b.up).length() < 1e-4, "{} {}", back.up, b.up);
    assert!((back.position - b.position).length() < 1e-4);
}
