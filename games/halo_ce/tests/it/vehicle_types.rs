//! The Ghost's and the Scorpion's driving rules and mass-point step against
//! specs/halo_ce/vehicles.md's formula and simulation cases, on the spec's
//! setup (a flat infinite stone floor at z = 0, 30 Hz, the data documents'
//! numbers, sleep off for the driving cases, driver power 1). Each case's
//! numbers are printed (`--nocapture`) beside the spec's.

#![allow(clippy::approx_constant)]

use bevy::math::Vec3;
use halo_ce::game::vehicles::data::VehicleDef;
use halo_ce::game::vehicles::drive::{Controls, Drive};
use halo_ce::game::vehicles::physics::{Body, FlatLevel, PhysicsDef};

const FLOOR: FlatLevel = FlatLevel {
    z: 0.0,
    material: Some(2),
};

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!(
        (a - b).abs() <= tol,
        "{what}: {a} against the spec's {b} (tolerance {tol})"
    );
}

fn ghost() -> (VehicleDef, PhysicsDef) {
    let d = VehicleDef::ghost();
    let p = d.physics_def();
    (d, p)
}

fn drive(v: &Drive, ti: f32, tj: f32) -> Controls {
    Controls {
        driver: true,
        throttle: [ti, tj],
        facing: v.body.forward,
        ..Default::default()
    }
}

/// A Ghost hovering still with full driver power (settled, sleep off).
fn hovering() -> (VehicleDef, PhysicsDef, Drive) {
    let (d, p) = ghost();
    let mut v = Drive::new(Body::at(Vec3::new(0.0, 0.0, 0.66), 0.0));
    v.sleep_enabled = false;
    v.power = 1.0;
    v.hover = 1.0;
    for _ in 0..900 {
        let c = drive(&v, 0.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
    }
    (d, p, v)
}

/// Sphere bottoms above the floor, per point.
fn bottoms(p: &PhysicsDef, v: &Drive) -> Vec<f32> {
    p.points
        .iter()
        .map(|q| v.body.point(q.position, p.center_of_mass).z - q.radius)
        .collect()
}

#[test]
fn the_ghost_hovers() {
    let (_, p, v) = hovering();
    let b = bottoms(&p, &v);
    eprintln!("hover: com z {} bottoms {b:?}", v.body.position.z);
    close(v.body.position.z, 0.66474, 2e-3, "centre of mass");
    close(b[3], 0.377, 3e-3, "hull front");
    close(b[2], 0.387, 3e-3, "wing right");
    close(b[4], 0.387, 3e-3, "wing left");
    close(b[0], 0.417, 3e-3, "back seat");
    close(b[1], 0.457, 3e-3, "hull mid");
}

#[test]
fn an_empty_ghost_rests_on_its_spheres_and_rises_with_its_driver() {
    let (d, p) = ghost();
    let mut v = Drive::new(Body::at(Vec3::new(0.0, 0.0, 0.3), 0.0));
    v.sleep_enabled = false;
    for _ in 0..900 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
    }
    let lowest = bottoms(&p, &v).into_iter().fold(f32::MAX, f32::min);
    eprintln!("empty: lowest {lowest}");
    close(lowest, -0.045, 3e-3, "lowest sphere bottom");
    let mut z = Vec::new();
    for _ in 0..90 {
        let c = drive(&v, 0.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        z.push(v.body.position.z);
    }
    eprintln!("driver enters: com z at 30, 60, 90: {} {} {}", z[29], z[59], z[89]);
    close(v.power, 1.0, 1e-6, "power after 60 ticks");
    close(z[29], 0.449, 0.01, "tick 30");
    close(z[59], 0.662, 0.01, "tick 60");
    close(z[89], 0.668, 0.01, "tick 90");
}

#[test]
fn the_ghost_drives_and_strafes() {
    let (d, p, mut v) = hovering();
    let start = v.body.position;
    let mut speeds = Vec::new();
    let mut dist = Vec::new();
    for _ in 0..300 {
        let c = drive(&v, 1.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        speeds.push(v.body.velocity.dot(v.body.forward));
        dist.push((v.body.position - start).truncate().length());
    }
    eprintln!(
        "ghost v at 1, 10, 30, 45, 60: {:?}; distance at 1, 2, 5, 10 s: {:?}",
        [1, 10, 30, 45, 60].map(|t| speeds[t - 1]),
        [30, 60, 150, 300].map(|t| dist[t - 1])
    );
    for (t, s, tol) in [
        (1, 0.005, 1e-4),
        (10, 0.04944, 1e-3),
        (30, 0.14469, 2e-3),
        (45, 0.21306, 2e-3),
        (60, 0.22444, 1e-3),
    ] {
        close(speeds[t - 1], s, tol, &format!("speed on tick {t}"));
    }
    for (t, x, tol) in [(30, 2.27, 0.05), (60, 8.35, 0.1), (150, 28.55, 0.3), (300, 62.21, 0.5)] {
        close(dist[t - 1], x, tol, &format!("distance on tick {t}"));
    }
    // Release: below 0.001 after 43 ticks, 4.57 WU on.
    let start = v.body.position;
    let mut below = None;
    for t in 1..=200i32 {
        let c = drive(&v, 0.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        if v.body.velocity.truncate().length() < 0.001 && below.is_none() {
            below = Some((t, (v.body.position - start).truncate().length()));
        }
    }
    eprintln!("ghost release: {below:?}");
    let (t, x) = below.expect("stops");
    assert!((t - 43).abs() <= 2, "stopped on tick {t}");
    close(x, 4.57, 0.1, "stopping distance");
    // Strafe, reverse and the diagonal: 0.22444 in every direction, no yaw.
    for (ti, tj, want) in [
        (0.0, 1.0, Vec3::new(0.0, 0.22444, 0.0)),
        (-1.0, 0.0, Vec3::new(-0.22444, 0.0, 0.0)),
        (0.7071, 0.7071, Vec3::new(0.15870, 0.15870, 0.0)),
    ] {
        let (_, _, mut v) = hovering();
        for _ in 0..300 {
            let c = drive(&v, ti, tj);
            v.tick(&d, &p, &c, &FLOOR, &[]);
        }
        let local = Vec3::new(v.body.velocity.dot(v.body.forward), v.body.velocity.dot(v.body.left()), 0.0);
        eprintln!("ghost ({ti}, {tj}): {local}, heading {}", v.body.forward);
        close(local.x, want.x, 1e-3, "forward part");
        close(local.y, want.y, 1e-3, "sideways part");
        close(local.truncate().length(), 0.22444, 1e-3, "speed");
        assert!(v.body.forward.y.abs() < 1e-3, "no yaw");
    }
}

#[test]
fn the_ghost_yaws_toward_the_camera() {
    let (d, p, mut v) = hovering();
    let cam = Vec3::Y;
    let mut yaw = Vec::new();
    let mut peak = 0.0f32;
    for _ in 0..60 {
        let c = Controls {
            facing: cam,
            ..drive(&v, 0.0, 0.0)
        };
        v.tick(&d, &p, &c, &FLOOR, &[]);
        yaw.push(v.body.forward.y.atan2(v.body.forward.x).to_degrees());
    }
    for k in 1..yaw.len() {
        peak = peak.max(yaw[k] - yaw[k - 1]);
    }
    eprintln!(
        "ghost yaw at 5, 10, 20, 30, 40, 60: {:?}; peak {peak}",
        [5, 10, 20, 30, 40, 60].map(|t| yaw[t - 1])
    );
    for (t, a, tol) in [(5, 3.0, 0.3), (10, 10.9, 0.5), (20, 41.4, 1.0), (30, 76.0, 1.0), (40, 90.5, 0.5)] {
        close(yaw[t - 1], a, tol, &format!("yaw on tick {t}"));
    }
    // The spec says a peak of 3.9 degrees per tick; its own yaw angles
    // (matched above) and the 0.2 degrees per tick per tick cap give 4.21
    // (reported as a spec gap): held within that difference.
    close(peak, 3.9, 0.35, "peak yaw rate");
    close(yaw[59], 90.0, 0.3, "settled");
    // The turn variable stays 0 (its limits and rate are 0): the flaps'
    // steering overlay stays centred.
    assert_eq!(v.turn, 0.0);
}

#[test]
fn the_ghost_formula_cases() {
    let (d, _) = ghost();
    // Driver power: 0.5 after 30 ticks, 1 after 60.
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    for _ in 0..30 {
        v.update_power(&d, true);
    }
    close(v.power, 0.5, 1e-5, "power after 30 ticks");
    for _ in 0..30 {
        v.update_power(&d, true);
    }
    close(v.power, 1.0, 1e-6, "power after 60 ticks");
    // The yaw rule: sigma 1.0, still, h_v 1: the yaw rate changes by the
    // cap; desired rate sqrt(0.0069813) = 0.08356; sigma 0.01: sigma/2.
    use halo_ce::game::vehicles::drive::scout_yaw_rate;
    close(scout_yaw_rate(1.0), 0.08356, 1e-5, "desired rate");
    close(scout_yaw_rate(0.01), 0.005, 1e-7, "desired rate, small sigma");
}

fn scorpion() -> (VehicleDef, PhysicsDef) {
    let d = VehicleDef::scorpion();
    let p = d.physics_def();
    (d, p)
}

/// A Scorpion settled on its springs, sleep off.
fn settled_tank() -> (VehicleDef, PhysicsDef, Drive) {
    let (d, p) = scorpion();
    let mut v = Drive::new(Body::at(Vec3::new(0.0, 0.0, 0.5), 0.0));
    v.sleep_enabled = false;
    for _ in 0..900 {
        v.tick(&d, &p, &Controls::default(), &FLOOR, &[]);
    }
    v.power = 1.0;
    (d, p, v)
}

#[test]
fn the_tank_formula_cases() {
    let (d, _) = scorpion();
    // s = 0.05, camera 90 degrees left: tau 0.004 after a tick, 0.14 after
    // 35; the treads at s -/+ tau.
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    let c = Controls {
        driver: true,
        throttle: [0.05, 0.0],
        facing: Vec3::Y,
        ..Default::default()
    };
    v.speed = 0.05;
    v.update_variables(&d, &c);
    close(v.turn, 0.004, 1e-6, "tau after a tick");
    for _ in 0..34 {
        v.update_variables(&d, &c);
    }
    close(v.turn, 0.14, 1e-5, "tau after 35 ticks");
    let ch = v.prepare(&d);
    close(ch[0].ground_speed, v.speed - v.turn, 1e-7, "left tread");
    close(ch[1].ground_speed, v.speed + v.turn, 1e-7, "right tread");
    // s = 0: no turn whatever the camera.
    let mut v = Drive::new(Body::at(Vec3::ZERO, 0.0));
    for _ in 0..10 {
        v.update_variables(
            &d,
            &Controls {
                throttle: [0.0, 0.0],
                ..c
            },
        );
    }
    assert_eq!(v.turn, 0.0);
}

#[test]
fn the_scorpion_rests_and_drives() {
    let (d, p, mut v) = settled_tank();
    eprintln!("scorpion: com z {}", v.body.position.z);
    close(v.body.position.z, 0.49530, 1e-3, "centre of mass");
    close(v.body.origin(p.center_of_mass).z, 0.07175, 1e-3, "model origin");
    let top0 = v.clone();
    let start = v.body.position;
    let mut speeds = Vec::new();
    let mut dist = Vec::new();
    for _ in 0..300 {
        let c = drive(&v, 1.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        speeds.push(v.body.velocity.dot(v.body.forward));
        dist.push((v.body.position - start).dot(Vec3::X));
    }
    eprintln!(
        "scorpion v at 30, 60, 120, 300: {:?}; distance at 1, 2, 4, 5, 10 s: {:?}",
        [30, 60, 120, 300].map(|t| speeds[t - 1]),
        [30, 60, 120, 150, 300].map(|t| dist[t - 1])
    );
    for (t, s, tol) in [(30, 0.04048, 1e-3), (60, 0.09871, 1e-3), (120, 0.13827, 1e-3), (300, 0.13844, 5e-4)] {
        close(speeds[t - 1], s, tol, &format!("speed on tick {t}"));
    }
    for (t, x, tol) in [(30, 0.509, 0.03), (60, 2.619, 0.06), (120, 10.44, 0.15), (150, 14.59, 0.2), (300, 35.36, 0.4)] {
        close(dist[t - 1], x, tol, &format!("distance on tick {t}"));
    }
    // Release: s = 0 after 24 ticks; below 0.001 after 66, 2.94 WU on.
    let start = v.body.position;
    let mut zero = None;
    let mut below = None;
    for t in 1..=200i32 {
        let c = drive(&v, 0.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
        if v.speed == 0.0 && zero.is_none() {
            zero = Some(t);
        }
        if v.body.velocity.dot(v.body.forward) < 0.001 && below.is_none() {
            below = Some((t, (v.body.position - start).dot(Vec3::X)));
        }
    }
    eprintln!("scorpion release: s 0 at {zero:?}, below 0.001 at {below:?}");
    assert_eq!(zero, Some(24));
    let (t, x) = below.unwrap();
    assert!((t - 66).abs() <= 3, "stopped on tick {t}");
    close(x, 2.94, 0.1, "stopping distance");
    // Reverse held: -0.11866.
    let mut v = top0.clone();
    for _ in 0..600 {
        let c = drive(&v, -1.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
    }
    let r = v.body.velocity.dot(v.body.forward);
    eprintln!("scorpion reverse {r}");
    close(r, -0.11866, 1e-3, "reverse speed");
    // At rest with the camera 90 degrees left and no throttle: no turn.
    let mut v = top0;
    for _ in 0..60 {
        let c = Controls {
            facing: v.body.left(),
            ..drive(&v, 0.0, 0.0)
        };
        v.tick(&d, &p, &c, &FLOOR, &[]);
    }
    assert!(v.body.angular_velocity.z.abs() < 1e-5, "yaw rate {}", v.body.angular_velocity.z);
}

#[test]
fn the_scorpion_turns_by_its_treads() {
    let (d, p, mut v) = settled_tank();
    for _ in 0..300 {
        let c = drive(&v, 1.0, 0.0);
        v.tick(&d, &p, &c, &FLOOR, &[]);
    }
    let mut taus = Vec::new();
    let mut rates = Vec::new();
    for _ in 0..300 {
        let c = Controls {
            facing: v.body.left(),
            ..drive(&v, 1.0, 0.0)
        };
        v.tick(&d, &p, &c, &FLOOR, &[]);
        taus.push(v.turn);
        rates.push(v.body.angular_velocity.z);
    }
    let speed = v.body.velocity.truncate().length();
    eprintln!(
        "scorpion turn: tau at 10, 30, 60 {:?}, yaw rate at 10, 30, 60, 300 {:?}, speed {speed}, radius {}",
        [10, 30, 60].map(|t| taus[t - 1]),
        [10, 30, 60, 300].map(|t| rates[t - 1]),
        speed / rates[299]
    );
    for (t, tau) in [(10, 0.04), (30, 0.12), (60, 0.14)] {
        close(taus[t - 1], tau, 1e-4, &format!("tau on tick {t}"));
    }
    for (t, w, tol) in [(10, 0.0105, 1e-3), (30, 0.0506, 2e-3), (60, 0.0767, 2e-3), (300, 0.0774, 1e-3)] {
        close(rates[t - 1], w, tol, &format!("yaw rate on tick {t}"));
    }
    close(speed, 0.107, 3e-3, "speed in the turn");
    close(speed / rates[299], 1.39, 0.05, "path radius");
}
