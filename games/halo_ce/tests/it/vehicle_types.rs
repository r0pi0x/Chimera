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
