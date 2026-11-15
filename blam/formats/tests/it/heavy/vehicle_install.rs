//! The vehicle part, seats and physics readers on the user's Halo disc image
//! against specs/halo_ce/vehicles.md's tables ("Vehicle fields of the
//! four", "Seats", "Physics: the four", the Warthog's mass points): the
//! Warthog, `ghost_mp` and `scorpion_mp` on Blood Gulch, every physics tag's
//! stored masses, densities, centre of mass and inertia tensor equal to
//! their recomputation from the points, and the three vehicles identical on
//! every multiplayer map. Each test passes with a note when there is no
//! image.

#![allow(clippy::approx_constant, clippy::excessive_precision, clippy::needless_range_loop)]

use blam_formats::cache::{MapFile, class};
use blam_formats::object::{SeatFlags, Vehicle};
use blam_formats::vehicle::{FrictionType, Physics, VehicleType};

use super::maps::{MULTIPLAYER, load};
use super::object_install::{near, normalized, path_of};

const WARTHOG: &str = "vehicles\\warthog\\warthog";
const GHOST: &str = "vehicles\\ghost\\ghost_mp";
const SCORPION: &str = "vehicles\\scorpion\\scorpion_mp";

fn vehicle(map: &MapFile, path: &str) -> Vehicle {
    Vehicle::read(
        map,
        map.find(class::VEHICLE, path).unwrap_or_else(|| panic!("no {path}")),
    )
    .unwrap()
}

fn physics(map: &MapFile, v: &Vehicle) -> Physics {
    Physics::read(map, map.tag(v.object.physics.id).expect("physics reference")).unwrap()
}

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

#[test]
fn the_three_vehicles_fields() {
    let Some(map) = load("bloodgulch") else { return };
    // (path, flags, type, fwd, rev, acc, dec, left, right, circumference,
    // turn rate, blur, inputs)
    let rows = [
        (
            WARTHOG,
            0xCD1,
            VehicleType::HumanJeep,
            [0.255, 0.1, 0.00275, 0.011, 30.0, -30.0, 1.0, 180.0, 0.08],
            [0, 12, 35, 0],
        ),
        (
            GHOST,
            0x29C5,
            VehicleType::AlienScout,
            [0.225, 0.0, 0.005, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0, 35, 0, 0],
        ),
        (
            SCORPION,
            0x8D1,
            VehicleType::HumanTank,
            [0.14, 0.12, 0.002, 0.006, 0.0, 0.0, 2.0, 0.0, 0.05],
            [20, 21, 0, 35],
        ),
    ];
    for (path, flags, ty, f, inputs) in rows {
        let v = vehicle(&map, path);
        let o = &v.own;
        assert_eq!(o.flags.bits(), flags, "{path}");
        assert_eq!(o.vehicle_type, ty, "{path}");
        let got = [
            o.maximum_forward_speed,
            o.maximum_reverse_speed,
            o.speed_acceleration,
            o.speed_deceleration,
            o.maximum_left_turn,
            o.maximum_right_turn,
            o.wheel_circumference,
            o.turn_rate,
            o.blur_speed,
        ];
        for (g, w) in got.iter().zip(f) {
            near(*g, w, path);
        }
        assert_eq!(o.inputs(), inputs, "{path}");
        assert_eq!((o.maximum_left_slide, o.slide_acceleration), (0.0, 0.0), "{path}");
        near(o.minimum_flipping_angular_velocity, 0.2, path);
        near(o.maximum_flipping_angular_velocity, 0.75, path);
        assert_eq!((o.fixed_gun_yaw, o.fixed_gun_pitch), (0.0, 0.0), "{path}");
        near(v.unit.rider_damage_fraction, 0.18, path);
        assert_eq!(
            path_of(&map, &o.material_effects).as_deref(),
            Some("vehicles\\vehicle"),
            "{path}"
        );
    }
    let w = vehicle(&map, WARTHOG);
    near(w.object.bounding_radius, 1.1, WARTHOG);
    near(w.object.acceleration_scale, 0.3, WARTHOG);
    assert!(w.own.crash_sound.id != vehicle(&map, SCORPION).own.crash_sound.id);
    assert!(vehicle(&map, SCORPION).own.crash_sound.is_null());
    assert_eq!(
        path_of(&map, &w.own.suspension_sound).as_deref(),
        Some("sound\\sfx\\impulse\\impacts\\warthog_suspension")
    );
    near(vehicle(&map, SCORPION).object.acceleration_scale, 0.07, SCORPION);
}

#[test]
fn the_seats() {
    let Some(map) = load("bloodgulch") else { return };
    let w = vehicle(&map, WARTHOG);
    let s = &w.unit.seats;
    assert_eq!(s.len(), 3);
    let labels: Vec<&str> = s.iter().map(|s| s.label.as_str()).collect();
    assert_eq!(labels, ["W-driver", "W-passenger", "W-gunner"]);
    let flags: Vec<u32> = s.iter().map(|s| s.flags.bits()).collect();
    assert_eq!(flags, [0x14, 0x60, 0x198]);
    let markers: Vec<&str> = s.iter().map(|s| s.marker_name.as_str()).collect();
    assert_eq!(markers, ["driver", "passenger", "gunner"]);
    let cams: Vec<&str> = s.iter().map(|s| s.camera_marker_name.as_str()).collect();
    assert_eq!(cams, ["", "camera rider", "camera gunner"]);
    assert_eq!((s[0].yaw_rate, s[0].pitch_rate), (60.0, 30.0));
    assert_eq!((s[2].yaw_rate, s[2].pitch_rate), (60.0, 0.0));
    near(s[0].pitch_auto_level, -0.3141593, "driver auto level");
    near(s[0].pitch_range_minimum, -0.5235988, "driver pitch min");
    near(s[0].pitch_range_maximum, 0.2617994, "driver pitch max");
    near(s[1].pitch_range_minimum, -0.8726646, "passenger pitch min");
    near(s[1].pitch_range_maximum, 1.570796, "passenger pitch max");
    near(s[1].yaw_minimum, -1.570796, "passenger yaw min");
    near(s[1].yaw_maximum, 1.570796, "passenger yaw max");
    near(s[2].pitch_range_minimum, -0.3490658, "gunner pitch min");
    assert_eq!(
        s.iter().map(|s| s.hud_text_message_index).collect::<Vec<_>>(),
        [13, 15, 14]
    );
    assert_eq!(s[0].acceleration_scale, [100.0, 100.0, 50.0]);
    assert_eq!(s[0].camera_tracks.len(), 2);
    assert_eq!(
        path_of(&map, &s[0].camera_tracks[0].track).as_deref(),
        Some("camera\\warthog_driver_loose")
    );
    assert_eq!(
        path_of(&map, &s[0].camera_tracks[1].track).as_deref(),
        Some("camera\\warthog_driver_tight")
    );
    assert_eq!(
        path_of(&map, &s[0].hud_interfaces[0].hud).as_deref(),
        Some("ui\\hud\\warthog driver")
    );
    assert!(s[1].flags.contains(SeatFlags::ALLOWS_WEAPONS));
    for seat in s {
        assert!(seat.built_in_actor.is_null());
    }
    let g = vehicle(&map, GHOST);
    assert_eq!(g.unit.seats.len(), 1);
    assert_eq!(
        (g.unit.seats[0].label.as_str(), g.unit.seats[0].flags.bits()),
        ("G-driver", 0x1C)
    );
    assert_eq!(g.unit.seats[0].camera_marker_name, "driver camera");
    let t = vehicle(&map, SCORPION);
    assert_eq!(t.unit.seats.len(), 5);
    assert_eq!(t.unit.seats[0].label, "scorpion-driver");
    assert!(
        t.unit.seats[1..]
            .iter()
            .all(|s| s.flags.bits() == 0x200 && s.camera_tracks.is_empty())
    );
}

#[test]
fn the_physics_of_the_three() {
    let Some(map) = load("bloodgulch") else { return };
    // (vehicle, mass, com, density, ground depth, air friction, xx, yy, zz,
    // xz, points, relative mass sum)
    let rows = [
        (
            WARTHOG,
            5000.0,
            [0.106921, 0.0, 0.321783],
            5.0,
            0.15,
            0.005,
            [595.045, 1689.021, 1990.222, 85.45504],
            15,
            24.5,
        ),
        (
            GHOST,
            2000.0,
            [0.362, 0.0, 0.108],
            3.0,
            0.15,
            0.0025,
            [189.408, 301.6, 423.648, 5.712],
            5,
            5.0,
        ),
        (
            SCORPION,
            20000.0,
            [-0.033818, 0.0, 0.423552],
            8.0,
            0.25,
            0.001,
            [8785.955, 16731.90, 21959.68, 1015.991],
            18,
            18.0,
        ),
    ];
    for (path, mass, com, density, depth, air, moments, n, rel) in rows {
        let p = physics(&map, &vehicle(&map, path));
        assert_eq!(p.radius, -1.0, "{path}");
        assert_eq!((p.moment_scale, p.gravity_scale), (1.0, 1.0), "{path}");
        near(p.mass, mass, path);
        for k in 0..3 {
            close(p.center_of_mass[k], com[k], 1e-5, path);
        }
        near(p.density, density, path);
        near(p.ground_friction, 0.2, path);
        near(p.ground_depth, depth, path);
        near(p.ground_damp_fraction, 0.05, path);
        near(p.ground_normal_k1, 0.707107, path);
        near(p.ground_normal_k0, 0.5, path);
        near(p.water_friction, 0.05, path);
        near(p.air_friction, air, path);
        assert_eq!(p.mass_points.len(), n, "{path}");
        close(p.mass_points.iter().map(|q| q.relative_mass).sum(), rel, 1e-4, path);
        let stored = p.inertia().unwrap();
        let got = [stored[0][0], stored[1][1], stored[2][2], stored[0][2]];
        for (g, w) in got.iter().zip(moments) {
            close(*g, w, 1e-3 * w.abs().max(1.0), path);
        }
        near(p.xx_moment, stored[0][0], path);
        near(p.yy_moment, stored[1][1], path);
        near(p.zz_moment, stored[2][2], path);
        assert_eq!(p.powered_mass_points.len(), 2, "{path}");
    }
    let w = physics(&map, &vehicle(&map, WARTHOG));
    let names: Vec<&str> = w.powered_mass_points.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["front", "back"]);
    assert!(w.powered_mass_points.iter().all(|c| c.flags.bits() == 1));
    let tyres = [
        (11, "tire back left", 1, 0.45, 0.65),
        (14, "tire front right", 0, 0.75, 0.45),
    ];
    for (i, name, ch, a, b) in tyres {
        let q = &w.mass_points[i];
        assert_eq!((q.name.as_str(), q.powered_mass_point), (name, ch));
        assert_eq!(q.friction_type, FrictionType::Forward);
        near(q.friction_parallel_scale, a, name);
        near(q.friction_perpendicular_scale, b, name);
        near(q.radius, 0.26, name);
        near(q.position[2], 0.19, name);
    }
    near(w.mass_points[13].mass, 612.2449, "front tyre mass");
    near(w.mass_points[0].mass, 204.0816, "bumper mass");
    assert!(w.mass_points[4].flags.bits() & 1 == 1 && w.mass_points[0].flags.bits() & 1 == 0);
    let g = physics(&map, &vehicle(&map, GHOST));
    let c = &g.powered_mass_points[0];
    assert_eq!(c.flags.bits(), 0x40);
    near(c.antigrav_strength, 1.5, "ghost strength");
    near(c.antigrav_height, 0.75, "ghost height");
    near(c.antigrav_normal_k0, 0.258819, "ghost k0");
}

#[test]
fn every_physics_tag_matches_its_recomputation() {
    let Some(map) = load("bloodgulch") else { return };
    let mut n = 0;
    for tag in map.of_class(class::PHYSICS) {
        let p = Physics::read(&map, tag).unwrap();
        n += 1;
        for (q, m) in p.mass_points.iter().zip(p.recomputed_masses()) {
            close(q.mass, m, 1e-3 * m.max(1.0), &tag.path);
        }
        for (q, d) in p.mass_points.iter().zip(p.recomputed_densities()) {
            close(q.density, d, 1e-3 * d.max(1.0), &tag.path);
        }
        let c = p.recomputed_center_of_mass();
        for k in 0..3 {
            close(p.center_of_mass[k], c[k], 1e-4, &tag.path);
        }
        let stored = p.inertia().unwrap();
        let got = p.recomputed_inertia();
        for i in 0..3 {
            for j in 0..3 {
                close(got[i][j], stored[i][j], 1e-4 * stored[i][i].abs().max(1.0), &tag.path);
            }
        }
    }
    assert!(n >= 3, "{n} physics tags");
}

#[test]
fn the_three_are_identical_on_every_multiplayer_map() {
    let Some(base) = load("bloodgulch") else { return };
    let want: Vec<(String, String)> = [WARTHOG, GHOST, SCORPION]
        .iter()
        .map(|p| {
            let v = vehicle(&base, p);
            (normalized(&base, &v.own), normalized(&base, &physics(&base, &v)))
        })
        .collect();
    for name in MULTIPLAYER {
        let Some(map) = load(name) else { return };
        for (p, w) in [WARTHOG, GHOST, SCORPION].iter().zip(&want) {
            let v = vehicle(&map, p);
            let got = (normalized(&map, &v.own), normalized(&map, &physics(&map, &v)));
            assert_eq!(&got, w, "{name}: {p}");
        }
    }
}
