//! Halo's vehicles as data (`VehicleDef`): one document per vehicle in
//! data/vehicles/ (the Warthog's, the Ghost's), compiled in and parsed once, in the tag's own units
//! (specs/halo_ce/vehicles.md: world units, WU per tick, the turn limits,
//! turn rate and seat rates in degrees). `VehicleDef::physics_def` derives
//! what the mass-point step reads (`physics::PhysicsDef`: the inverse
//! inertia from the stored tensor, the point frames and friction shapes).

use std::collections::BTreeMap;

use bevy::math::{Mat3, Vec3};
use serde::{Deserialize, Serialize};

use super::physics::{ChannelDef, Friction, PhysicsDef, PointDef};

/// data/vehicles/warthog.toml, built in.
pub const WARTHOG: &str = include_str!("../../../data/vehicles/warthog.toml");
/// data/vehicles/ghost.toml, built in.
pub const GHOST: &str = include_str!("../../../data/vehicles/ghost.toml");

/// The driving model a vehicle's type picks (vehicles.md "The vehicle
/// tag", type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Tank,
    Jeep,
    Boat,
    Plane,
    Scout,
    Fighter,
    Turret,
}

/// A vehicle document.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VehicleDef {
    pub name: String,
    /// The tag's path on the disc.
    pub tag: String,
    pub kind: Kind,
    /// The vehicle flags (`blam_formats::vehicle::VehicleFlags`).
    pub flags: u32,
    pub max_forward_speed: f32,
    pub max_reverse_speed: f32,
    pub speed_acceleration: f32,
    pub speed_deceleration: f32,
    /// Degrees (jeep).
    pub max_left_turn: f32,
    pub max_right_turn: f32,
    pub wheel_circumference: f32,
    /// Degrees per second (jeep).
    pub turn_rate: f32,
    pub blur_speed: f32,
    /// Minimum and maximum flipping angular velocity.
    pub flipping_angular_velocity: [f32; 2],
    pub bounding_radius: f32,
    pub acceleration_scale: f32,
    pub rider_damage_fraction: f32,
    /// The driver's powered seat, seconds.
    pub power_up_time: f32,
    pub power_down_time: f32,
    pub physics: PhysicsDoc,
    pub seats: Vec<SeatDef>,
    #[serde(default)]
    pub tracks: BTreeMap<String, TrackDef>,
}

/// The physics tag's fields the step reads.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PhysicsDoc {
    pub mass: f32,
    pub center_of_mass: [f32; 3],
    pub density: f32,
    pub gravity_scale: f32,
    pub ground_friction: f32,
    pub ground_depth: f32,
    pub ground_damp_fraction: f32,
    pub ground_normal_k1: f32,
    pub ground_normal_k0: f32,
    pub water_friction: f32,
    pub water_depth: f32,
    pub water_density: f32,
    pub air_friction: f32,
    /// The stored tensor's diagonal.
    pub moments: [f32; 3],
    /// Its xz (= zx) product.
    pub product_xz: f32,
    pub channels: Vec<ChannelDoc>,
    pub points: Vec<PointDoc>,
}

/// A powered mass point.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelDoc {
    pub name: String,
    pub flags: u32,
    #[serde(default)]
    pub antigrav_strength: f32,
    #[serde(default)]
    pub antigrav_height: f32,
    #[serde(default)]
    pub antigrav_damp_fraction: f32,
    #[serde(default)]
    pub antigrav_normal_k1: f32,
    #[serde(default)]
    pub antigrav_normal_k0: f32,
}

/// A mass point.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PointDoc {
    pub name: String,
    #[serde(default = "no_channel")]
    pub channel: i16,
    #[serde(default)]
    pub node: i16,
    pub position: [f32; 3],
    #[serde(default = "x_axis")]
    pub forward: [f32; 3],
    #[serde(default = "z_axis")]
    pub up: [f32; 3],
    pub mass: f32,
    pub radius: f32,
    #[serde(default)]
    pub friction: FrictionDoc,
    #[serde(default = "unit_scales")]
    pub friction_scales: [f32; 2],
    #[serde(default)]
    pub metallic: bool,
}

/// A mass point's friction type.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrictionDoc {
    #[default]
    Point,
    Forward,
    Left,
    Up,
}

/// A seat.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SeatDef {
    pub label: String,
    pub marker: String,
    pub flags: u32,
    /// Degrees per second (0: the player's).
    pub yaw_rate: f32,
    pub pitch_rate: f32,
    pub camera_marker: String,
    pub pitch_auto_level: f32,
    pub pitch_range: [f32; 2],
    pub yaw_range: [f32; 2],
    /// A key of `tracks` (empty: the default unit track).
    pub track: String,
    pub hud_text: i16,
}

/// Seat flag bits (vehicles.md "Seat flags").
pub mod seat_flags {
    pub const INVISIBLE: u32 = 1 << 0;
    pub const DRIVER: u32 = 1 << 2;
    pub const GUNNER: u32 = 1 << 3;
    pub const THIRD_PERSON_CAMERA: u32 = 1 << 4;
    pub const ALLOWS_WEAPONS: u32 = 1 << 5;
    pub const THIRD_PERSON_ON_ENTER: u32 = 1 << 6;
    pub const NOT_VALID_WITHOUT_DRIVER: u32 = 1 << 9;
}

/// Vehicle flag bits (vehicles.md "Vehicle flags").
pub mod vehicle_flags {
    pub const SPEED_WAKES_PHYSICS: u32 = 1 << 0;
    pub const TURN_WAKES_PHYSICS: u32 = 1 << 1;
    pub const DRIVER_POWER_WAKES_PHYSICS: u32 = 1 << 2;
    pub const CONTROL_OPPOSITE_SPEED_SETS_BRAKE: u32 = 1 << 4;
    pub const CAUSES_COLLISION_DAMAGE: u32 = 1 << 7;
}

/// A camera track: (x, z) control points from pitch -90 to +90 degrees.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrackDef {
    pub points: Vec<[f32; 2]>,
}

fn no_channel() -> i16 {
    -1
}
fn x_axis() -> [f32; 3] {
    [1.0, 0.0, 0.0]
}
fn z_axis() -> [f32; 3] {
    [0.0, 0.0, 1.0]
}
fn unit_scales() -> [f32; 2] {
    [1.0, 1.0]
}

impl VehicleDef {
    /// The built-in Warthog (panics only if the document is broken; a test
    /// parses it).
    pub fn warthog() -> Self {
        toml::from_str(WARTHOG).expect("data/vehicles/warthog.toml parses")
    }

    /// The built-in Ghost.
    pub fn ghost() -> Self {
        toml::from_str(GHOST).expect("data/vehicles/ghost.toml parses")
    }

    /// Every built-in vehicle, by name.
    pub fn builtin() -> Vec<VehicleDef> {
        vec![Self::warthog(), Self::ghost()]
    }

    /// The seat with the driver flag, if any.
    pub fn driver_seat(&self) -> Option<usize> {
        self.seats.iter().position(|s| s.flags & seat_flags::DRIVER != 0)
    }

    /// What the mass-point step reads.
    pub fn physics_def(&self) -> PhysicsDef {
        let p = &self.physics;
        let [xx, yy, zz] = p.moments;
        let xz = p.product_xz;
        let tensor = Mat3::from_cols(Vec3::new(xx, 0.0, xz), Vec3::new(0.0, yy, 0.0), Vec3::new(xz, 0.0, zz));
        PhysicsDef {
            mass: p.mass,
            center_of_mass: Vec3::from(p.center_of_mass),
            gravity_scale: p.gravity_scale,
            ground_friction: p.ground_friction,
            ground_depth: p.ground_depth,
            ground_damp: p.ground_damp_fraction,
            ground_k1: p.ground_normal_k1,
            ground_k0: p.ground_normal_k0,
            air_friction: p.air_friction,
            xx_moment: xx,
            yy_moment: yy,
            zz_moment: zz,
            inverse_inertia: tensor.inverse(),
            channels: p
                .channels
                .iter()
                .map(|c| ChannelDef {
                    ground_friction: c.flags & 1 != 0,
                    antigrav: c.flags & (1 << 6) != 0,
                    antigrav_strength: c.antigrav_strength,
                    antigrav_height: c.antigrav_height,
                    antigrav_damp: c.antigrav_damp_fraction,
                    antigrav_k1: c.antigrav_normal_k1,
                    antigrav_k0: c.antigrav_normal_k0,
                })
                .collect(),
            points: p
                .points
                .iter()
                .map(|q| PointDef {
                    position: Vec3::from(q.position),
                    forward: Vec3::from(q.forward),
                    up: Vec3::from(q.up),
                    mass: q.mass,
                    radius: q.radius,
                    channel: usize::try_from(q.channel).ok(),
                    friction: match q.friction {
                        FrictionDoc::Point => Friction::Point,
                        FrictionDoc::Forward => Friction::Forward,
                        FrictionDoc::Left => Friction::Left,
                        FrictionDoc::Up => Friction::Up,
                    },
                    along: q.friction_scales[0],
                    across: q.friction_scales[1],
                })
                .collect(),
        }
    }
}
