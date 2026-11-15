//! The Warthog's driving rules and mass-point step against
//! specs/halo_ce/vehicles.md's test cases, on the spec's setup: a flat
//! infinite stone floor at z = 0, Halo's 30 Hz tick, the data document's
//! numbers (data/vehicles/warthog.toml), sleep off for the driving cases.
//! The formula cases are exact; the simulation cases (the spec's numbers
//! come from a scratch simulation of the same model) are checked within the
//! tolerances stated per case.

// The spec's numbers are written as it states them.
#![allow(clippy::approx_constant)]

use bevy::math::Vec3;
use halo_ce::game::vehicles::data::VehicleDef;
use halo_ce::game::vehicles::drive::{Controls, Drive, SpeedParams, seek, speed_step};
use halo_ce::game::vehicles::physics::{self, Body, FlatLevel, GRAVITY, PhysicsDef};

const FLOOR: FlatLevel = FlatLevel {
    z: 0.0,
    material: Some(2),
};

fn warthog() -> (VehicleDef, PhysicsDef) {
    let d = VehicleDef::warthog();
    let p = d.physics_def();
    (d, p)
}

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{what}: {a} against the spec's {b} (tolerance {tol})"
    );
}

/// A Warthog level at rest at its spring equilibrium (settled with sleep
/// off and nobody driving), facing +x.
fn settled() -> (VehicleDef, PhysicsDef, Drive) {
    let (d, p) = warthog();
    let mut v = Drive::new(Body::at(Vec3::new(0.0, 0.0, 0.3536), 0.0));
    v.sleep_enabled = false;
    for _ in 0..900 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
    }
    v.power = 1.0;
    (d, p, v)
}

fn throttle(v: &Drive, t: f32) -> Controls {
    Controls {
        driver: true,
        throttle: [t, 0.0],
        facing: v.body.forward,
        ..Default::default()
    }
}

/// The ground speed v·f.
fn ground_speed(v: &Drive) -> f32 {
    v.body.velocity.dot(v.body.forward)
}

const P: SpeedParams = SpeedParams {
    pos: 0.255,
    neg: 0.1,
    acc: 0.00275,
    dec: 0.011,
};

#[test]
fn the_speed_variable() {
    assert_eq!(seek(0.0, 1.0, P, 1.0), 0.00275);
    let mut s = 0.0;
    let mut n = 0;
    while s < 0.255 {
        s = seek(s, 1.0, P, 1.0);
        n += 1;
    }
    assert_eq!((n, s), (93, 0.255), "clamped on tick 93");
    let mut s = 0.255;
    for _ in 0..23 {
        s = seek(s, 0.0, P, 1.0);
    }
    close(s, 0.002, 1e-6, "23 ticks of release");
    assert_eq!(seek(s, 0.0, P, 1.0), 0.0, "tick 24");
    assert_eq!(seek(0.0, -1.0, P, 1.0), -0.00275);
    close(
        speed_step(-0.005, P, 1.0),
        (-0.005 / 0.011 + 1.0) * 0.00275,
        1e-7,
        "crossing step",
    );
    close(speed_step(-0.005, P, 1.0), 0.0015, 1e-5, "crossing step");
}

#[test]
fn the_brake_flag_and_the_turn_variable() {
    let (d, _) = warthog();
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    // Throttle against the speed sets the brake: s seeks 0 and stops on it.
    v.speed = 0.005;
    let mut c = throttle(&v, -1.0);
    v.update_variables(&d, &c);
    assert!(v.brake);
    assert_eq!(v.speed, 0.0);
    // Camera 90 degrees left, moving forward: 6 degrees per tick to 30.
    v.speed = 0.1;
    c = Controls {
        driver: true,
        throttle: [1.0, 0.0],
        facing: Vec3::Y,
        ..Default::default()
    };
    v.update_variables(&d, &c);
    close(v.turn, 0.10472, 1e-5, "tau after 1 tick");
    for _ in 0..4 {
        v.update_variables(&d, &c);
    }
    close(v.turn, 0.5236, 1e-4, "tau after 5 ticks");
    v.update_variables(&d, &c);
    close(v.turn, 0.5236, 1e-4, "held at 30 degrees");
    // Reversing, camera 20 degrees left: tau follows -sigma.
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    v.speed = -0.05;
    let a = 20f32.to_radians();
    let c = Controls {
        driver: true,
        throttle: [-1.0, 0.0],
        facing: Vec3::new(a.cos(), a.sin(), 0.0),
        ..Default::default()
    };
    for _ in 0..4 {
        v.update_variables(&d, &c);
    }
    close(v.turn, -0.3491, 1e-4, "reverse tau");
}

#[test]
fn one_contact_at_half_the_ground_depth_holds_half_the_weight() {
    let (_, mut p) = warthog();
    // One tyre alone, no air friction: the velocity change is -g + N/M.
    p.points.retain(|q| q.radius == 0.26);
    p.points.truncate(1);
    p.air_friction = 0.0;
    let q = p.points[0].position;
    let c = p.center_of_mass;
    // Put the tyre's centre 0.26 - 0.075 above the floor.
    let mut b = Body::at(Vec3::ZERO, 0.0);
    b.position.z = 0.26 - 0.075 - (q - c).z;
    let out = physics::step(
        &p,
        &mut b,
        &[Default::default(); 2],
        &FLOOR,
        &[],
        Vec3::ZERO,
        Vec3::ZERO,
    );
    assert_eq!(out.touching, 1);
    close(b.velocity.z, -GRAVITY / 2.0, 1e-8, "net vertical acceleration");
}

#[test]
fn a_rocket_push() {
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    v.push(Vec3::new(0.12, 0.0, 0.0));
    assert_eq!(v.body.velocity, Vec3::new(0.12, 0.0, 0.0));
    close(v.body.angular_velocity.y, 0.37699, 1e-5, "spin");
}

#[test]
fn driver_power_and_the_flip() {
    let (d, p) = warthog();
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    v.power = 1.0;
    for _ in 0..3 {
        v.update_power(&d, false);
    }
    assert_eq!(v.power, 0.0, "0.1 s power-down");
    // Upside down: the first flip tick sets |w| = 0.3 x 0.75.
    let mut v = Drive::new(Body {
        up: -Vec3::Z,
        ..Body::at(Vec3::new(0.0, 0.0, 2.0), 0.0)
    });
    v.start_flip(Vec3::new(0.0, -2.0, 2.0));
    v.sleep_enabled = false;
    v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
    assert!(v.flip.is_some());
    // The step after the set adds its own torques: compare the set value.
    let mut v = Drive::new(Body {
        up: -Vec3::Z,
        ..Body::at(Vec3::new(0.0, 0.0, 9.0), 0.0)
    });
    v.start_flip(Vec3::new(0.0, -2.0, 9.0));
    v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
    close(v.body.angular_velocity.length(), 0.225, 1e-3, "upside down");
    let mut v = Drive::new(Body {
        up: Vec3::Y,
        ..Body::at(Vec3::new(0.0, 0.0, 9.0), 0.0)
    });
    v.start_flip(Vec3::new(0.0, -2.0, 9.0));
    v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
    close(v.body.angular_velocity.length(), 0.06, 1e-3, "on its side");
}

#[test]
fn resting_ride_height() {
    let (_, p, v) = settled();
    eprintln!("com z {} forward.z {}", v.body.position.z, v.body.forward.z);
    // Spec: centre of mass z 0.35360, forward.z -0.0078; tyre depths 0.03243
    // (rear) and 0.04257 (front), summing over four to 0.15.
    close(v.body.position.z, 0.35360, 5e-4, "centre of mass height");
    close(v.body.forward.z, -0.0078, 5e-4, "pitch");
    let depth = |i: usize| v.last.contacts[i].map_or(0.0, |c| c.depth);
    close(depth(11), 0.03243, 5e-4, "rear tyre depth");
    close(depth(13), 0.04257, 5e-4, "front tyre depth");
    close(depth(11) + depth(12) + depth(13) + depth(14), 0.15, 1e-3, "depth sum");
    let origin = v.body.origin(p.center_of_mass);
    close(origin.z, 0.03182, 1e-3, "model origin height");
}

#[test]
fn settling_from_a_one_unit_drop() {
    let (d, p, mut v) = settled();
    v.power = 0.0;
    let rest = v.body.position.z;
    v.body.position.z += 1.0;
    let mut lowest = (f32::MAX, 0i32);
    let mut settled_at = None;
    let mut heights = Vec::new();
    for t in 1..=200i32 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
        let h = v.body.position.z - rest;
        heights.push(h);
        if h < lowest.0 {
            lowest = (h, t);
        }
        if h.abs() < 0.01 && v.body.velocity.z.abs() < 0.001 {
            settled_at.get_or_insert(t);
        } else {
            settled_at = None;
        }
    }
    eprintln!("tick 23 {} lowest {:?} settled {:?}", heights[22], lowest, settled_at);
    // Spec: 0.045 above on tick 23; lowest -0.131 on tick 27; within 0.01 WU
    // and |v_z| < 0.001 from tick 57 on.
    close(heights[22], 0.045, 0.01, "tick 23");
    close(lowest.0, -0.131, 0.01, "lowest");
    assert!((lowest.1 - 27).abs() <= 1, "lowest on tick {}", lowest.1);
    let s = settled_at.expect("settles");
    assert!((s - 57).abs() <= 3, "settled from tick {s}");
}

#[test]
fn full_throttle_speed_curve_and_top_speed() {
    let (d, p, mut v) = settled();
    let start = v.body.position;
    let mut speeds = Vec::new();
    let mut dist = Vec::new();
    for _ in 0..300 {
        let c = throttle(&v, 1.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        speeds.push(ground_speed(&v));
        dist.push((v.body.position - start).dot(Vec3::X));
    }
    eprintln!(
        "v at 1, 2, 30, 60, 93, 150, 300: {:?}; dist at 30, 60, 150, 300: {:?}",
        [1, 2, 30, 60, 93, 150, 300].map(|t| speeds[t - 1]),
        [30, 60, 150, 300].map(|t| dist[t - 1])
    );
    let want = [
        (1, 0.00013, 5e-5),
        (2, 0.00039, 5e-5),
        (30, 0.03895, 0.002),
        (60, 0.10720, 0.002),
        (93, 0.18880, 0.002),
        (150, 0.23156, 0.002),
        (300, 0.23367, 0.001),
    ];
    for (t, s, tol) in want {
        close(speeds[t - 1], s, tol, &format!("speed on tick {t}"));
    }
    for (t, x, tol) in [
        (30, 0.463, 0.05),
        (60, 2.661, 0.1),
        (150, 20.12, 0.3),
        (300, 55.13, 0.5),
    ] {
        close(dist[t - 1], x, tol, &format!("distance on tick {t}"));
    }
    assert!(v.body.velocity.y.abs() < 1e-4, "straight");
}

#[test]
fn stopping_and_reversing() {
    let (d, p, mut v) = settled();
    for _ in 0..300 {
        let c = throttle(&v, 1.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
    }
    let top = v.clone();
    // Release with sleep on: stops 86 ticks later, 6.71 WU on.
    v.sleep_enabled = true;
    let start = v.body.position;
    let mut stopped = None;
    for t in 1..=300i32 {
        let c = throttle(&v, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        if v.body.velocity == Vec3::ZERO && stopped.is_none() {
            stopped = Some(t);
        }
    }
    let gone = (v.body.position - start).dot(Vec3::X);
    eprintln!("stopped {stopped:?} after {gone}");
    let t = stopped.expect("stops");
    assert!((t - 86).abs() <= 10, "stopped on tick {t}");
    close(gone, 6.71, 0.4, "stopping distance");
    // Full reverse held: below 0.001 after 50 ticks, 5.96 WU on.
    let mut v = top;
    let start = v.body.position;
    let mut below = None;
    for t in 1..=600i32 {
        let c = throttle(&v, -1.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        if ground_speed(&v) < 0.001 && below.is_none() {
            below = Some((t, (v.body.position - start).dot(Vec3::X)));
        }
    }
    eprintln!("reverse: below 0.001 at {below:?}, after 600 {}", ground_speed(&v));
    let (t, x) = below.unwrap();
    assert!((t - 50).abs() <= 4, "braked on tick {t}");
    close(x, 5.96, 0.3, "braking distance");
    close(ground_speed(&v), -0.0916, 0.002, "reverse top speed");
}

/// The heading of the forward, degrees.
fn heading(v: &Drive) -> f32 {
    v.body.forward.y.atan2(v.body.forward.x).to_degrees()
}

#[test]
fn steering_toward_a_fixed_camera() {
    let (d, p, mut v) = settled();
    for _ in 0..300 {
        let c = throttle(&v, 1.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
    }
    let a = 10f32.to_radians();
    let cam = Vec3::new(a.cos(), a.sin(), 0.0);
    let mut h = Vec::new();
    for _ in 0..120 {
        let c = Controls {
            facing: cam,
            ..throttle(&v, 1.0)
        };
        v.tick(&d, &p, &c, &FLOOR, &[]);
        h.push(heading(&v));
    }
    eprintln!("heading at 10, 30, 120: {} {} {}", h[9], h[29], h[119]);
    // Spec: 5.9, 14.0 (overshoot), 10.08 degrees.
    close(h[9], 5.9, 0.5, "heading on tick 10");
    close(h[29], 14.0, 0.7, "heading on tick 30");
    close(h[119], 10.08, 0.3, "heading on tick 120");
}

#[test]
fn full_lock_spins() {
    for (t, speed, yaw_rate, radius) in [(1.0, 0.0683, 0.1568, 0.44), (0.08, 0.051, 0.049, 1.03)] {
        let (d, p, mut v) = settled();
        for _ in 0..300 {
            let c = throttle(&v, 1.0);
            v.tick(&d, &p, &c, &FLOOR, &[]);
        }
        for _ in 0..600 {
            let c = Controls {
                facing: v.body.left(),
                ..throttle(&v, t)
            };
            v.tick(&d, &p, &c, &FLOOR, &[]);
        }
        let s = v.body.velocity.length();
        let w = v.body.angular_velocity.z;
        eprintln!(
            "t_i {t}: |v| {s} sideways {} yaw rate {w} radius {}",
            v.body.velocity.dot(v.body.left()),
            s / w
        );
        close(s, speed, 0.1 * speed, "speed at full lock");
        close(w, yaw_rate, 0.1 * yaw_rate, "yaw rate at full lock");
        close(s / w, radius, 0.15 * radius, "path radius");
    }
}

#[test]
fn spawned_on_the_ground_falls_asleep() {
    // Spawned with its origin on the plane (0.55 WU above, as created, then
    // falling onto its springs): asleep on tick 25, frozen 0.062 above
    // equilibrium.
    let (d, p) = warthog();
    let c = p.center_of_mass;
    let mut v = Drive::new(Body::at(Vec3::new(0.0, 0.0, 0.55 + c.z), 0.0));
    let mut asleep = None;
    for t in 1..=200i32 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
        if v.asleep && asleep.is_none() {
            asleep = Some(t);
        }
    }
    eprintln!("asleep {asleep:?}, frozen at {}", v.body.position.z - 0.35360);
    let t = asleep.expect("falls asleep");
    assert!((t - 25).abs() <= 5, "asleep on tick {t}");
    close(
        v.body.position.z - 0.35360,
        0.062,
        0.03,
        "frozen height above equilibrium",
    );
}
