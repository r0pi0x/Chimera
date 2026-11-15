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
use halo_ce::game::vehicles::drive::{self, Controls, Drive, SpeedParams, seek, speed_step};
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

/// The 1 WU drop: the settled Warthog raised 1 WU, still.
fn dropped(sleep: bool) -> (VehicleDef, PhysicsDef, Drive, f32) {
    let (d, p, mut v) = settled();
    v.power = 0.0;
    v.sleep_enabled = sleep;
    v.body.velocity = Vec3::ZERO;
    v.body.angular_velocity = Vec3::ZERO;
    let rest = v.body.position.z;
    v.body.position.z += 1.0;
    (d, p, v, rest)
}

#[test]
fn settling_from_a_one_unit_drop() {
    let (d, p, mut v, rest) = dropped(false);
    let mut lowest = (f32::MAX, 0i32);
    let mut settled_at = None;
    let mut heights = Vec::new();
    let mut vz = Vec::new();
    for t in 1..=200i32 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
        assert!(v.last.stuck.is_empty(), "the freeze never acts (tick {t})");
        let h = v.body.position.z - rest;
        heights.push(h);
        vz.push(v.body.velocity.z);
        if h < lowest.0 {
            lowest = (h, t);
        }
        if h.abs() < 0.01 && v.body.velocity.z.abs() < 0.001 {
            settled_at.get_or_insert(t);
        } else {
            settled_at = None;
        }
    }
    eprintln!(
        "ticks 20..30 {:?} lowest {lowest:?} settled {settled_at:?} tick 57 {}",
        &heights[19..30],
        heights[56]
    );
    // Spec: ticks 20 to 30 relative to equilibrium; v_z at 23 and 27; the
    // lowest -0.1215 on tick 27; within 0.01 WU and |v_z| < 0.001 from tick
    // 57 on (+0.0031 there).
    let want = [
        0.2705, 0.1986, 0.1234, 0.0451, -0.0287, -0.0844, -0.1139, -0.1215, -0.1087, -0.0871, -0.0622,
    ];
    for (k, w) in want.iter().enumerate() {
        close(heights[19 + k], *w, 0.002, &format!("height on tick {}", 20 + k));
    }
    close(vz[22], -0.0784, 0.001, "v_z on tick 23");
    close(vz[26], -0.0077, 0.001, "v_z on tick 27");
    close(lowest.0, -0.1215, 0.002, "lowest");
    assert_eq!(lowest.1, 27, "lowest on tick 27");
    close(heights[56], 0.0031, 0.002, "tick 57");
    assert_eq!(settled_at, Some(57), "settled from tick 57");
}

#[test]
fn the_drop_with_sleep_on() {
    // Spec: asleep on tick 42 (0.010 above, moving down 0.0077), frozen
    // 0.026 below equilibrium.
    let (d, p, mut v, rest) = dropped(true);
    let mut asleep = None;
    for t in 1..=200i32 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
        if v.asleep && asleep.is_none() {
            asleep = Some((t, v.body.position.z - rest, v.body.velocity.z));
        }
    }
    eprintln!("asleep {asleep:?}, frozen at {}", v.body.position.z - rest);
    let (t, h, vz) = asleep.expect("falls asleep");
    assert_eq!(t, 42, "asleep on tick 42");
    close(h, 0.010, 0.002, "height when falling asleep");
    close(vz, -0.0077, 0.001, "v_z when falling asleep");
    close(v.body.position.z - rest, -0.026, 0.002, "frozen height");
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

/// A Warthog falling onto the plane from its centre of mass at `z`, sleep
/// on: (tick it falls asleep, v_z then, frozen height above equilibrium,
/// forward.z frozen).
fn spawned(z: f32) -> (i32, f32, f32, f32) {
    let (d, p) = warthog();
    let mut v = Drive::new(Body::at(Vec3::new(0.0, 0.0, z), 0.0));
    let mut asleep = None;
    for t in 1..=200i32 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
        if v.asleep && asleep.is_none() {
            asleep = Some((t, v.body.velocity.z));
        }
    }
    let (t, vz) = asleep.expect("falls asleep");
    (t, vz, v.body.position.z - 0.35360, v.body.forward.z)
}

#[test]
fn created_from_a_placement_falls_asleep() {
    // A placement on the plane: centre of mass at the placement raised
    // 0.55, asleep on tick 19 moving up 0.0093, frozen 0.033 above.
    let (t, vz, h, _) = spawned(0.55);
    eprintln!("placement: asleep {t} v_z {vz} frozen {h}");
    assert_eq!(t, 19, "asleep on tick");
    close(vz, 0.0093, 0.001, "v_z when falling asleep");
    close(h, 0.033, 0.002, "frozen height above equilibrium");
    // The model origin 0.55 above the plane (centre of mass 0.8718): asleep
    // on tick 25 moving up 0.018, frozen 0.062 above, forward.z -0.043.
    let (t, vz, h, fz) = spawned(0.8718);
    eprintln!("origin: asleep {t} v_z {vz} frozen {h} forward.z {fz}");
    assert_eq!(t, 25, "asleep on tick");
    close(vz, 0.018, 0.001, "v_z when falling asleep");
    close(h, 0.062, 0.002, "frozen height above equilibrium");
    close(fz, -0.043, 0.002, "frozen pitch");
}

/// A level whose first `hits` sweeps hit at fraction 0.5 against an
/// up-facing surface, and whose spheres touch nothing.
struct Sweeps {
    hits: usize,
    seen: std::cell::Cell<usize>,
}

impl physics::Level for Sweeps {
    fn sphere(&self, _: Vec3, _: f32) -> Option<physics::Contact> {
        None
    }
    fn sweep(&self, _: Vec3, _: Vec3) -> Option<(f32, Vec3)> {
        let n = self.seen.get();
        self.seen.set(n + 1);
        (n < self.hits).then_some((0.5, Vec3::Z))
    }
}

#[test]
fn the_penetration_freeze() {
    // The spec's synthetic case: t_h 0.5, n (0, 0, 1), sweep (0, 0, -0.1).
    let (v, w) = physics::freeze_response(
        0.5,
        Vec3::Z,
        Vec3::new(0.0, 0.0, -0.1),
        Vec3::new(0.2, 0.0, -0.1),
        Vec3::new(0.0, 0.01, 0.0),
    );
    close(v.x, 0.2, 1e-7, "tangential part kept");
    close(v.z, -0.0421875, 1e-6, "normal part scaled by t'");
    close(w.y, 0.00421875, 1e-7, "angular velocity scaled by t'");
    // Moving away from the surface: v unchanged, w scaled.
    let (v, w) = physics::freeze_response(
        0.5,
        Vec3::Z,
        Vec3::new(0.0, 0.0, -0.1),
        Vec3::new(0.2, 0.0, 0.1),
        Vec3::new(0.0, 0.01, 0.0),
    );
    assert_eq!(v, Vec3::new(0.2, 0.0, 0.1));
    close(w.y, 0.00421875, 1e-7, "angular velocity scaled");
    // One point, falling: the first pass hits, the next is clear, and the
    // body moves by the scaled velocity.
    let (_, mut p) = warthog();
    p.points.truncate(1);
    p.air_friction = 0.0;
    let mut b = Body::at(Vec3::new(0.0, 0.0, 5.0), 0.0);
    b.velocity = Vec3::new(0.2, 0.0, -0.1);
    let level = Sweeps {
        hits: 1,
        seen: Default::default(),
    };
    let out = physics::step(&p, &mut b, &[Default::default(); 2], &level, &[], Vec3::ZERO, Vec3::ZERO);
    assert!(!out.frozen);
    assert_eq!(out.stuck, vec![0]);
    let vz = -0.1 - GRAVITY;
    let t = (0.5 - 0.0078125 / vz.abs()).max(0.0);
    close(b.velocity.z, vz * t, 1e-6, "scaled v_z stored");
    close(b.velocity.x, 0.2, 1e-6, "v_x kept");
    close(b.position.z, 5.0 + vz * t, 1e-6, "moved by the new velocity");
    // Every pass hits: the vehicle stays where it was, the reduced
    // velocities are kept.
    let mut b = Body::at(Vec3::new(0.0, 0.0, 5.0), 0.0);
    b.velocity = Vec3::new(0.2, 0.0, -0.1);
    b.angular_velocity = Vec3::new(0.0, 0.01, 0.0);
    let level = Sweeps {
        hits: usize::MAX,
        seen: Default::default(),
    };
    let out = physics::step(&p, &mut b, &[Default::default(); 2], &level, &[], Vec3::ZERO, Vec3::ZERO);
    assert!(out.frozen);
    assert_eq!(b.position, Vec3::new(0.0, 0.0, 5.0));
    assert_eq!((b.forward, b.up), (Vec3::X, Vec3::Z));
    assert!(b.velocity.z.abs() < vz.abs(), "reduced: {}", b.velocity.z);
    close(b.velocity.x, 0.2, 1e-6, "v_x kept");
}

#[test]
fn the_flip() {
    // Strength k = clamp(-2 up.z, 0.2, 0.75): rate 0.3 k.
    for (up_z, rate) in [(-1.0f32, 0.225), (-0.5, 0.225), (-0.2, 0.12), (0.0, 0.06), (0.5, 0.06)] {
        let s = (1.0 - up_z * up_z).sqrt();
        let b = Body {
            up: Vec3::new(0.0, -s, up_z),
            ..Body::at(Vec3::ZERO, 0.0)
        };
        let w = drive::flip_omega(&b, 2, [0.2, 0.75]);
        close(w.length(), rate, 1e-5, &format!("flip rate at up.z {up_z}"));
    }
    // Counting the set rate alone: on its roof up.z > 0.9 after 28 ticks,
    // on its side after 19, at 135 degrees after 24 (the player on the side
    // that turns it back up: the faster of the two side types).
    for (roll, ticks) in [(180f32, 28), (90.0, 19), (135.0, 24)] {
        let r = roll.to_radians();
        let count = |kind: u8| {
            let mut b = Body {
                up: Vec3::new(0.0, -r.sin(), r.cos()),
                ..Body::at(Vec3::ZERO, 0.0)
            };
            let mut n = 0;
            while b.up.z <= drive::FLIP_DONE_UP && n < 60 {
                let w = drive::flip_omega(&b, kind, [0.2, 0.75]);
                (b.forward, b.up) = b.turned(w);
                n += 1;
            }
            n
        };
        assert_eq!(count(1).min(count(2)), ticks, "flip from {roll} degrees");
    }
}

#[test]
fn which_placements_spawn_and_where() {
    use blam_formats::game_type::VehicleSet;
    use halo_ce::game::vehicles::place::{self, Placed};
    let warthog_tag = Some(r"vehicles\warthog\warthog");
    assert!(place::spawns(0, warthog_tag, VehicleSet::Default));
    assert!(!place::spawns(1, warthog_tag, VehicleSet::Default), "not automatic");
    assert!(place::spawns(0b1110, warthog_tag, VehicleSet::Default), "difficulty bits ignored");
    assert!(!place::spawns(0, None, VehicleSet::Default), "no palette entry");
    assert!(
        !place::spawns(0, Some(r"vehicles\ghost\ghost"), VehicleSet::Default),
        "the campaign Ghost is not one of the globals' three"
    );
    assert!(!place::spawns(0, warthog_tag, VehicleSet::Ghost));
    assert!(place::spawns(0, Some(r"vehicles\ghost\ghost_mp"), VehicleSet::Ghost));
    assert!(!place::spawns(0, warthog_tag, VehicleSet::None));
    // A placement at (x, y, 0.29): the centre of mass at (x, y, 0.84).
    let (d, p) = warthog();
    let placed = Placed {
        prop: 0,
        vehicle: "warthog".into(),
        position: [10.0, -20.0, 0.29],
        forward: [1.0, 0.0, 0.0],
        up: [0.0, 0.0, 1.0],
        markers: Default::default(),
        hull: Vec::new(),
    };
    let v = halo_ce::game::vehicles::spawn_state(&d, &p, 0, &placed);
    close(v.drive.body.position.z, 0.84, 1e-6, "centre of mass");
    assert_eq!(v.drive.body.position.truncate(), bevy::math::Vec2::new(10.0, -20.0));
    close(v.drive.body.origin(p.center_of_mass).z, 0.84 - 0.321783, 1e-6, "model origin");
}
