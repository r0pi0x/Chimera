//! The vehicle part and physics readers on synthetic `vehi` and `phys` tags
//! written from specs/halo_ce/vehicles.md's tables (the Warthog's vehicle
//! fields and seats; the Warthog's, Ghost's and Scorpion's mass points),
//! the recomputations the spec says the stored values equal (point masses,
//! densities, centre of mass, the inertia tensor against the spec's stored
//! moments) and the rejections.

// The spec's numbers are written as it states them, rounded.
#![allow(
    clippy::approx_constant,
    clippy::excessive_precision,
    clippy::needless_range_loop,
    clippy::too_many_arguments
)]

use blam_formats::cache::MapFile;
use blam_formats::object::{SeatFlags, Vehicle};
use blam_formats::vehicle::{FrictionType, Physics, PoweredMassPointFlags, VehicleFlags, VehicleType};

use crate::map_fixture::{CacheFile, Struct, TagData, null_ref};

const SCNR: usize = 0;
const OBJ: usize = 1;

fn build(class: &'static str, path: &'static str, main: impl FnOnce(&mut TagData) -> Vec<u8>) -> MapFile {
    let mut td = TagData::new(&[("scnr", "scenarios\\syn"), (class, path)]);
    td.set_struct(SCNR, &Struct::new(0x5B0).0);
    let bytes = main(&mut td);
    td.set_struct(OBJ, &bytes);
    MapFile::from_image(CacheFile::new("syn", 1, td.finish()).image()).unwrap()
}

/// (name, channel, rel mass, rel density, position, friction type, along,
/// across, radius) as the spec's tables give them.
type Point = (&'static str, i16, f32, f32, [f32; 3], i16, f32, f32, f32);

const WARTHOG_POINTS: [Point; 15] = [
    (
        "bumper back left",
        -1,
        1.0,
        1.0,
        [-0.77711, 0.18628, 0.40146],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "bumper back right",
        -1,
        1.0,
        1.0,
        [-0.77711, -0.18628, 0.40146],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "hull bottom middle right",
        -1,
        3.0,
        1.0,
        [0.00113, -0.18131, 0.39201],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "hull bottom middle left",
        -1,
        3.0,
        1.0,
        [0.00113, 0.18131, 0.39201],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "hull top back left",
        -1,
        1.0,
        1.0,
        [-0.41087, 0.21523, 0.40143],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "hull top back right",
        -1,
        1.0,
        1.0,
        [-0.41087, -0.21523, 0.40143],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "hull top front right",
        -1,
        1.5,
        1.0,
        [0.42573, -0.16515, 0.40126],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "hull top front left",
        -1,
        1.5,
        1.0,
        [0.42573, 0.16515, 0.40126],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "bumper front left",
        -1,
        1.0,
        1.0,
        [0.84754, 0.16494, 0.32011],
        0,
        1.0,
        1.0,
        0.3,
    ),
    (
        "bumper front right",
        -1,
        1.0,
        1.0,
        [0.84754, -0.16494, 0.32011],
        0,
        1.0,
        1.0,
        0.3,
    ),
    ("top of gun", -1, 0.5, 1.0, [-0.22703, 0.0, 0.7437], 0, 1.0, 1.0, 0.3),
    ("tire back left", 1, 1.5, 0.5, [-0.63, 0.38, 0.19], 1, 0.45, 0.65, 0.26),
    (
        "tire back right",
        1,
        1.5,
        0.5,
        [-0.63, -0.38, 0.19],
        1,
        0.45,
        0.65,
        0.26,
    ),
    ("tire front left", 0, 3.0, 0.5, [0.67, 0.38, 0.19], 1, 0.75, 0.45, 0.26),
    (
        "tire front right",
        0,
        3.0,
        0.5,
        [0.67, -0.38, 0.19],
        1,
        0.75,
        0.45,
        0.26,
    ),
];

fn ghost_points() -> Vec<Point> {
    vec![
        ("hull back seat", 1, 1.0, 1.0, [-0.25, 0.0, 0.08], 0, 1.0, 1.0, 0.22),
        ("hull mid", -1, 1.0, 1.0, [0.2, 0.0, 0.2], 0, 1.0, 1.0, 0.3),
        ("wing right", 0, 1.0, 1.0, [0.62, -0.4, 0.06], 0, 1.0, 1.0, 0.23),
        ("hull front", -1, 1.0, 1.0, [0.62, 0.0, 0.14], 0, 1.0, 1.0, 0.32),
        ("wing left", 0, 1.0, 1.0, [0.62, 0.4, 0.06], 0, 1.0, 1.0, 0.23),
    ]
}

fn scorpion_points() -> Vec<Point> {
    let t = |name, ch, p: [f32; 3], r| (name, ch, 1.0, 1.0, p, 0, 1.0, 1.0, r);
    vec![
        t("tread front left front", 0, [1.2, 0.78, 0.218], 0.32),
        t("tread front left back", 0, [0.32, 0.78, 0.256], 0.36),
        t("tread back left front", 0, [-0.53, 0.87, 0.256], 0.36),
        t("tread back left back", 0, [-1.15927, 0.87, 0.218], 0.32),
        t("tread back right back", 1, [-1.15927, -0.87, 0.218], 0.32),
        t("tread back right front", 1, [-0.53, -0.87, 0.256], 0.36),
        t("tread front right back", 1, [0.32, -0.78, 0.256], 0.36),
        t("tread front right front", 1, [1.2, -0.78, 0.218], 0.32),
        t("turret front", -1, [-0.285636, 0.0, 1.11114], 0.35),
        t("turret back", -1, [-0.944265, 0.0, 1.00178], 0.48),
        t("hull back left", -1, [-1.05126, 0.2, 0.48964], 0.4),
        t("hull mid right", -1, [0.543795, -0.2, 0.413791], 0.38),
        t("hull front right", -1, [1.18333, -0.22, 0.40408], 0.3),
        t("hull front left", -1, [1.18333, 0.22, 0.40408], 0.3),
        t("hull mid left", -1, [0.543795, 0.2, 0.413791], 0.38),
        t("hull hatch left", -1, [-0.196007, 0.22, 0.5], 0.44),
        t("hull hatch right", -1, [-0.196007, -0.22, 0.5], 0.44),
        t("hull back right", -1, [-1.05126, -0.2, 0.48964], 0.4),
    ]
}

/// A `phys` tag of `points` with mass `mass`, density `density` and the
/// precomputed fields written as the spec says the tool writes them
/// (masses, densities, centre of mass from the points). The inertia block
/// holds `stored` and a placeholder inverse.
fn physics_tag(
    mass: f32,
    density: f32,
    com: [f32; 3],
    points: &[Point],
    channels: &[(u32, f32)],
    stored: [f32; 4],
) -> MapFile {
    build("phys", "vehicles\\syn\\syn", |td| {
        let rel: f32 = points.iter().map(|p| p.2).sum();
        let ratio: f32 = points.iter().map(|p| p.2 / p.3).sum();
        let elems: Vec<Vec<u8>> = points
            .iter()
            .map(|&(name, ch, rm, rd, pos, ft, a, b, r)| {
                Struct::new(0x80)
                    .str(0, name)
                    .i16(0x20, ch)
                    .i16(0x22, 0)
                    .f32(0x28, rm)
                    .f32(0x2C, mass * rm / rel)
                    .f32(0x30, rd)
                    .f32(0x34, density * rd * ratio / rel)
                    .f32s(0x38, &pos)
                    .f32s(0x44, &[1.0, 0.0, 0.0])
                    .f32s(0x50, &[0.0, 0.0, 1.0])
                    .i16(0x5C, ft)
                    .f32(0x60, a)
                    .f32(0x64, b)
                    .f32(0x68, r)
                    .into()
            })
            .collect();
        let pts = td.block(&elems);
        let chans: Vec<Vec<u8>> = channels
            .iter()
            .enumerate()
            .map(|(i, &(flags, strength))| {
                Struct::new(0x80)
                    .str(0, if i == 0 { "front" } else { "back" })
                    .u32(0x20, flags)
                    .f32(0x24, strength)
                    .f32(0x2C, 0.75)
                    .f32(0x30, 0.02)
                    .f32(0x34, 0.5)
                    .f32(0x38, 0.258819)
                    .into()
            })
            .collect();
        let ch = td.block(&chans);
        let [xx, yy, zz, xz] = stored;
        let tensor = Struct::new(0x24)
            .f32s(0, &[xx, 0.0, xz, 0.0, yy, 0.0, xz, 0.0, zz])
            .into();
        let inv = Struct::new(0x24)
            .f32s(0, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0])
            .into();
        let inertia = td.block(&[tensor, inv]);
        Struct::new(0x80)
            .f32(0x00, -1.0)
            .f32(0x04, 1.0)
            .f32(0x08, mass)
            .f32s(0x0C, &com)
            .f32(0x18, density)
            .f32(0x1C, 1.0)
            .f32(0x20, 0.2)
            .f32(0x24, 0.15)
            .f32(0x28, 0.05)
            .f32(0x2C, 0.707107)
            .f32(0x30, 0.5)
            .f32(0x38, 0.05)
            .f32(0x3C, 0.25)
            .f32(0x40, 1.0)
            .f32(0x48, 0.005)
            .f32(0x50, xx)
            .f32(0x54, yy)
            .f32(0x58, zz)
            .put(0x5C, &inertia)
            .put(0x68, &ch)
            .put(0x74, &pts)
            .into()
    })
}

fn read_phys(map: &MapFile) -> std::io::Result<Physics> {
    Physics::read(map, &map.tags()[OBJ])
}

fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} against {b}");
}

/// The recomputed tensor's (xx, yy, zz, xz) against the spec's stored
/// values, to the 4 decimals the spec checked (relative for big values).
fn check_inertia(p: &Physics, want: [f32; 4]) {
    let t = p.recomputed_inertia();
    let got = [t[0][0], t[1][1], t[2][2], t[0][2]];
    for (k, (g, w)) in got.iter().zip(want).enumerate() {
        close(*g, w, 2e-4 * w.abs().max(1.0), &format!("tensor component {k}"));
    }
    close(t[0][2], t[2][0], 1e-3, "symmetric");
    assert!(
        t[0][1].abs() < 1e-2 && t[1][2].abs() < 1e-2,
        "y-symmetric vehicle: {t:?}"
    );
}

#[test]
fn the_warthogs_physics_reads_and_recomputes() {
    let stored = [595.045, 1689.021, 1990.222, 85.45504];
    let map = physics_tag(
        5000.0,
        5.0,
        [0.106921, 0.0, 0.321783],
        &WARTHOG_POINTS,
        &[(1, 0.0), (1, 0.0)],
        stored,
    );
    let p = read_phys(&map).unwrap();
    assert_eq!(
        (p.radius, p.mass, p.ground_depth, p.air_friction),
        (-1.0, 5000.0, 0.15, 0.005)
    );
    assert_eq!(p.mass_points.len(), 15);
    assert_eq!(p.powered_mass_points.len(), 2);
    assert!(
        p.powered_mass_points[0]
            .flags
            .contains(PoweredMassPointFlags::GROUND_FRICTION)
    );
    let tyre = &p.mass_points[13];
    assert_eq!((tyre.name.as_str(), tyre.powered_mass_point), ("tire front left", 0));
    assert_eq!(tyre.friction_type, FrictionType::Forward);
    assert_eq!(
        (
            tyre.friction_parallel_scale,
            tyre.friction_perpendicular_scale,
            tyre.radius
        ),
        (0.75, 0.45, 0.26)
    );
    // Masses: relative mass x 5000 / 24.5.
    for (m, want) in p.recomputed_masses().iter().zip(p.mass_points.iter().map(|q| q.mass)) {
        close(*m, want, 1e-3, "mass");
    }
    close(p.mass_points[0].mass, 204.0816, 1e-3, "bumper mass");
    close(p.mass_points[13].mass, 612.2449, 1e-3, "front tyre mass");
    // Densities 6.836735 (hull), 3.418367 (tyres); total volume M/density.
    let d = p.recomputed_densities();
    close(d[0], 6.836735, 1e-4, "hull density");
    close(d[11], 3.418367, 1e-4, "tyre density");
    let volume: f32 = p.mass_points.iter().zip(&d).map(|(q, d)| q.mass / d).sum();
    close(volume, 5000.0 / 5.0, 1e-2, "volume");
    let c = p.recomputed_center_of_mass();
    close(c[0], 0.106921, 1e-5, "com x");
    close(c[1], 0.0, 1e-6, "com y");
    close(c[2], 0.321783, 1e-5, "com z");
    check_inertia(&p, stored);
    assert_eq!(p.inertia().unwrap()[0][0], 595.045);
    assert_eq!(p.inverse_inertia().unwrap()[1][1], 1.0);
}

#[test]
fn the_ghosts_and_scorpions_tensors_recompute() {
    let stored = [189.408, 301.6, 423.648, 5.712];
    let map = physics_tag(
        2000.0,
        3.0,
        [0.362, 0.0, 0.108],
        &ghost_points(),
        &[(0x40, 1.5), (0x40, 1.5)],
        stored,
    );
    let g = read_phys(&map).unwrap();
    let c = g.recomputed_center_of_mass();
    close(c[0], 0.362, 1e-5, "ghost com x");
    close(c[2], 0.108, 1e-5, "ghost com z");
    check_inertia(&g, stored);
    let ch = &g.powered_mass_points[1];
    assert!(ch.flags.contains(PoweredMassPointFlags::ANTIGRAV));
    assert_eq!(
        (ch.antigrav_strength, ch.antigrav_height, ch.antigrav_damp_fraction),
        (1.5, 0.75, 0.02)
    );
    assert_eq!((ch.antigrav_normal_k1, ch.antigrav_normal_k0), (0.5, 0.258819));

    let stored = [8785.955, 16731.90, 21959.68, 1015.991];
    let map = physics_tag(
        20000.0,
        8.0,
        [-0.033818, 0.0, 0.423552],
        &scorpion_points(),
        &[(1, 0.0), (1, 0.0)],
        stored,
    );
    let s = read_phys(&map).unwrap();
    close(s.mass_points[5].mass, 1111.111, 1e-2, "scorpion point mass");
    close(s.recomputed_densities()[3], 8.0, 1e-5, "scorpion density");
    let c = s.recomputed_center_of_mass();
    close(c[0], -0.033818, 1e-5, "scorpion com x");
    close(c[2], 0.423552, 1e-5, "scorpion com z");
    check_inertia(&s, stored);
}

#[test]
fn physics_rejections() {
    // A mass point naming a channel that does not exist.
    let mut pts = WARTHOG_POINTS.to_vec();
    pts[11].1 = 2;
    let map = physics_tag(5000.0, 5.0, [0.0; 3], &pts, &[(1, 0.0), (1, 0.0)], [1.0; 4]);
    let e = read_phys(&map).unwrap_err().to_string();
    assert!(e.contains("mass point 11 names channel 2"), "{e}");
    // A tag of another class.
    let map = build("vehi", "vehicles\\syn\\syn", |_| Struct::new(0x3F0).i16(0, 1).into());
    assert!(
        read_phys(&map)
            .unwrap_err()
            .to_string()
            .contains("is not a physics tag")
    );
}

fn warthog_seat(
    td: &mut TagData,
    flags: u32,
    label: &str,
    marker: &str,
    camera: &str,
    pitch: [f32; 3],
    yaw: [f32; 2],
    text: i16,
) -> Vec<u8> {
    let tracks = td.block(&[Struct::new(0x1C).put(0, &null_ref("trak")).into()]);
    let huds = td.block(&[Struct::new(0x30).put(0, &null_ref("unhi")).into()]);
    Struct::new(0x11C)
        .u32(0, flags)
        .str(0x04, label)
        .str(0x24, marker)
        .f32s(0x64, &[100.0, 100.0, 50.0])
        .f32(0x7C, if flags & 4 != 0 { 60.0 } else { 0.0 })
        .f32(0x80, if flags & 4 != 0 { 30.0 } else { 0.0 })
        .str(0x84, camera)
        .f32(0xC4, pitch[0])
        .f32(0xC8, pitch[1])
        .f32(0xCC, pitch[2])
        .put(0xD0, &tracks)
        .put(0xDC, &huds)
        .i16(0xEC, text)
        .f32(0xF0, yaw[0])
        .f32(0xF4, yaw[1])
        .put(0xF8, &null_ref("actv"))
        .into()
}

#[test]
fn the_warthogs_vehicle_fields_and_seats() {
    let map = build("vehi", "vehicles\\warthog\\warthog", |td| {
        let seats = vec![
            warthog_seat(
                td,
                0x14,
                "W-driver",
                "driver",
                "",
                [-0.3141593, -0.5235988, 0.2617994],
                [0.0, 0.0],
                13,
            ),
            warthog_seat(
                td,
                0x60,
                "W-passenger",
                "passenger",
                "camera rider",
                [0.0, -0.8726646, 1.570796],
                [-1.570796, 1.570796],
                15,
            ),
            warthog_seat(
                td,
                0x198,
                "W-gunner",
                "gunner",
                "camera gunner",
                [0.0, -0.3490658, 0.6108652],
                [0.0, 0.0],
                14,
            ),
        ];
        let seats = td.block(&seats);
        Struct::new(0x3F0)
            .i16(0, 1)
            .f32(0x020, 0.3)
            .f32(0x184, 0.18)
            .put(0x2E4, &seats)
            .u32(0x2F0, 0xCD1)
            .i16(0x2F4, 1)
            .f32s(0x2F8, &[0.255, 0.1, 0.00275, 0.011, 30.0, -30.0, 1.0, 180.0, 0.08])
            .i16(0x31E, 12)
            .i16(0x320, 35)
            .f32(0x340, 0.2)
            .f32(0x344, 0.75)
            .f32(0x384, 1.75)
            .f32(0x38C, 1.0)
            .put(0x3B0, &null_ref("snd!"))
            .put(0x3C0, &null_ref("snd!"))
            .put(0x3D0, &null_ref("foot"))
            .put(0x3E0, &null_ref("effe"))
            .into()
    });
    let v = Vehicle::read(&map, &map.tags()[OBJ]).unwrap();
    let o = &v.own;
    assert_eq!(o.flags.bits(), 0xCD1);
    for f in [
        VehicleFlags::SPEED_WAKES_PHYSICS,
        VehicleFlags::CONTROL_OPPOSITE_SPEED_SETS_BRAKE,
        VehicleFlags::KILLS_RIDERS_AT_TERMINAL_VELOCITY,
        VehicleFlags::CAUSES_COLLISION_DAMAGE,
    ] {
        assert!(o.flags.contains(f));
    }
    assert!(!o.flags.contains(VehicleFlags::TURN_WAKES_PHYSICS));
    assert_eq!(o.vehicle_type, VehicleType::HumanJeep);
    assert_eq!(
        (
            o.maximum_forward_speed,
            o.maximum_reverse_speed,
            o.speed_acceleration,
            o.speed_deceleration
        ),
        (0.255, 0.1, 0.00275, 0.011)
    );
    assert_eq!(
        (
            o.maximum_left_turn,
            o.maximum_right_turn,
            o.wheel_circumference,
            o.turn_rate
        ),
        (30.0, -30.0, 1.0, 180.0)
    );
    assert_eq!(o.blur_speed, 0.08);
    assert_eq!(o.inputs(), [0, 12, 35, 0]);
    assert_eq!(
        (o.minimum_flipping_angular_velocity, o.maximum_flipping_angular_velocity),
        (0.2, 0.75)
    );
    assert_eq!((o.ai_destination_radius, o.ai_pathfinding_radius), (1.75, 1.0));
    assert!(o.crash_sound.is_null() && o.effect.is_null());
    assert_eq!(v.object.acceleration_scale, 0.3);
    assert_eq!(v.unit.seats.len(), 3);
    let d = &v.unit.seats[0];
    assert!(d.flags.contains(SeatFlags::DRIVER) && d.flags.contains(SeatFlags::THIRD_PERSON_CAMERA));
    assert_eq!(
        (d.label.as_str(), d.marker_name.as_str(), d.camera_marker_name.as_str()),
        ("W-driver", "driver", "")
    );
    assert_eq!(
        (d.pitch_auto_level, d.pitch_range_minimum, d.pitch_range_maximum),
        (-0.3141593, -0.5235988, 0.2617994)
    );
    assert_eq!((d.yaw_rate, d.pitch_rate, d.hud_text_message_index), (60.0, 30.0, 13));
    assert_eq!((d.camera_tracks.len(), d.hud_interfaces.len()), (1, 1));
    let p = &v.unit.seats[1];
    assert!(p.flags.contains(SeatFlags::ALLOWS_WEAPONS) && p.flags.contains(SeatFlags::THIRD_PERSON_ON_ENTER));
    assert_eq!(
        (p.camera_marker_name.as_str(), p.yaw_minimum, p.yaw_maximum),
        ("camera rider", -1.570796, 1.570796)
    );
    let g = &v.unit.seats[2];
    assert_eq!(g.flags.bits(), 0x198);
    assert!(g.flags.contains(SeatFlags::GUNNER) && g.flags.contains(SeatFlags::ALLOW_VEHICLE_COMMUNICATION_ANIMATIONS));
    assert_eq!(g.hud_text_message_index, 14);
}
