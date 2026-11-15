//! Halo's vehicle documents (data/vehicles/) against the user's disc image:
//! every number of each vehicle's fields, seats, camera tracks and physics
//! (mass points, channels with their antigravity, the tensor) equals what
//! `blam_formats` reads from `bloodgulch.map`, within the spec's digits.
//! Passes with a note without an install.

use blam_formats::{
    cache::{MapFile, class},
    object::Vehicle,
    vehicle::{FrictionType, Physics},
};
use halo_ce::game::vehicles::data::{FrictionDoc, Kind, VehicleDef};

fn bloodgulch() -> Option<MapFile> {
    let mount = super::maps::mount()?;
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    Some(MapFile::load(&bytes).unwrap())
}

/// Equal within `rel` of the disc's value (the spec's digits).
fn loose(ours: f32, disc: f32, what: &str) {
    assert!(
        (ours - disc).abs() <= disc.abs() * 2e-5 + 2e-6,
        "{what}: data {ours}, disc {disc}"
    );
}

#[test]
fn the_warthogs_numbers_are_the_disc_s() {
    let Some(map) = bloodgulch() else { return };
    check(&map, &VehicleDef::warthog(), Kind::Jeep, 1);
}

#[test]
fn the_ghosts_numbers_are_the_disc_s() {
    let Some(map) = bloodgulch() else { return };
    check(&map, &VehicleDef::ghost(), Kind::Scout, 4);
}

/// Every number of `d` against the disc's tag, of type `code`.
fn check(map: &MapFile, d: &VehicleDef, kind: Kind, code: i16) {
    let tag = map.find(class::VEHICLE, &d.tag).expect("the vehicle");
    let v = Vehicle::read(map, tag).unwrap();
    let o = &v.own;
    assert_eq!(d.kind, kind);
    assert_eq!(o.vehicle_type.code(), code);
    assert_eq!(d.flags, o.flags.bits());
    for (ours, disc, what) in [
        (d.max_forward_speed, o.maximum_forward_speed, "forward"),
        (d.max_reverse_speed, o.maximum_reverse_speed, "reverse"),
        (d.speed_acceleration, o.speed_acceleration, "acceleration"),
        (d.speed_deceleration, o.speed_deceleration, "deceleration"),
        (d.max_left_turn, o.maximum_left_turn, "left turn"),
        (d.max_right_turn, o.maximum_right_turn, "right turn"),
        (d.wheel_circumference, o.wheel_circumference, "circumference"),
        (d.turn_rate, o.turn_rate, "turn rate"),
        (d.blur_speed, o.blur_speed, "blur"),
        (
            d.flipping_angular_velocity[0],
            o.minimum_flipping_angular_velocity,
            "flip min",
        ),
        (
            d.flipping_angular_velocity[1],
            o.maximum_flipping_angular_velocity,
            "flip max",
        ),
        (d.bounding_radius, v.object.bounding_radius, "bounding radius"),
        (d.acceleration_scale, v.object.acceleration_scale, "acceleration scale"),
        (d.rider_damage_fraction, v.unit.rider_damage_fraction, "rider damage"),
        (d.power_up_time, v.unit.powered_seats[0].power_up_time, "power up"),
        (d.power_down_time, v.unit.powered_seats[0].power_down_time, "power down"),
    ] {
        loose(ours, disc, what);
    }
    assert_eq!(d.seats.len(), v.unit.seats.len());
    for (s, t) in d.seats.iter().zip(&v.unit.seats) {
        assert_eq!(
            (s.label.as_str(), s.marker.as_str()),
            (t.label.as_str(), t.marker_name.as_str())
        );
        assert_eq!(s.flags, t.flags.bits(), "{}", s.label);
        assert_eq!(s.camera_marker, t.camera_marker_name, "{}", s.label);
        assert_eq!(s.hud_text, t.hud_text_message_index, "{}", s.label);
        loose(s.yaw_rate, t.yaw_rate, &s.label);
        loose(s.pitch_rate, t.pitch_rate, &s.label);
        loose(s.pitch_auto_level, t.pitch_auto_level, &s.label);
        loose(s.pitch_range[0], t.pitch_range_minimum, &s.label);
        loose(s.pitch_range[1], t.pitch_range_maximum, &s.label);
        loose(s.yaw_range[0], t.yaw_minimum, &s.label);
        loose(s.yaw_range[1], t.yaw_maximum, &s.label);
        let track = t
            .camera_tracks
            .first()
            .and_then(|r| map.tag(r.track.id))
            .map(|t| t.path.clone());
        let want = (!s.track.is_empty()).then(|| format!(r"camera\{}", s.track));
        assert_eq!(track, want, "{}", s.label);
    }
    let p = Physics::read(map, map.tag(v.object.physics.id).unwrap()).unwrap();
    let ours = &d.physics;
    for (a, b, what) in [
        (ours.mass, p.mass, "mass"),
        (ours.density, p.density, "density"),
        (ours.gravity_scale, p.gravity_scale, "gravity scale"),
        (ours.ground_friction, p.ground_friction, "ground friction"),
        (ours.ground_depth, p.ground_depth, "ground depth"),
        (ours.ground_damp_fraction, p.ground_damp_fraction, "damp"),
        (ours.ground_normal_k1, p.ground_normal_k1, "k1"),
        (ours.ground_normal_k0, p.ground_normal_k0, "k0"),
        (ours.water_friction, p.water_friction, "water friction"),
        (ours.water_depth, p.water_depth, "water depth"),
        (ours.water_density, p.water_density, "water density"),
        (ours.air_friction, p.air_friction, "air friction"),
    ] {
        loose(a, b, what);
    }
    for k in 0..3 {
        loose(ours.center_of_mass[k], p.center_of_mass[k], "centre of mass");
    }
    let t = p.inertia().unwrap();
    for (k, m) in ours.moments.iter().enumerate() {
        loose(*m, t[k][k], "moment");
    }
    loose(ours.product_xz, t[0][2], "xz");
    assert!(t[0][1].abs() < 1e-3 && t[1][2].abs() < 1e-3);
    assert_eq!(ours.channels.len(), p.powered_mass_points.len());
    for (c, q) in ours.channels.iter().zip(&p.powered_mass_points) {
        assert_eq!((c.name.as_str(), c.flags), (q.name.as_str(), q.flags.bits()));
        loose(c.antigrav_strength, q.antigrav_strength, &c.name);
        loose(c.antigrav_height, q.antigrav_height, &c.name);
        loose(c.antigrav_damp_fraction, q.antigrav_damp_fraction, &c.name);
        loose(c.antigrav_normal_k1, q.antigrav_normal_k1, &c.name);
        loose(c.antigrav_normal_k0, q.antigrav_normal_k0, &c.name);
    }
    assert_eq!(ours.points.len(), p.mass_points.len());
    for (a, b) in ours.points.iter().zip(&p.mass_points) {
        let n = &a.name;
        assert_eq!(n, &b.name);
        assert_eq!((a.channel, a.node), (b.powered_mass_point, b.model_node), "{n}");
        assert_eq!(a.metallic, b.flags.bits() & 1 == 1, "{n}");
        loose(a.mass, b.mass, n);
        loose(a.radius, b.radius, n);
        for k in 0..3 {
            loose(a.position[k], b.position[k], n);
            loose(a.forward[k], b.forward[k], n);
            loose(a.up[k], b.up[k], n);
        }
        let ty = match a.friction {
            FrictionDoc::Point => FrictionType::Point,
            FrictionDoc::Forward => FrictionType::Forward,
            FrictionDoc::Left => FrictionType::Left,
            FrictionDoc::Up => FrictionType::Up,
        };
        assert_eq!(ty, b.friction_type, "{n}");
        loose(a.friction_scales[0], b.friction_parallel_scale, n);
        loose(a.friction_scales[1], b.friction_perpendicular_scale, n);
    }
}
