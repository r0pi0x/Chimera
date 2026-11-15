//! The vehicle tag's own fields and the physics tag (specs/halo_ce/
//! vehicles.md, "The vehicle tag" and "The physics tag") as plain data, in
//! the game's own values (world units, per tick, degrees where the spec says
//! the disc stores degrees).
//!
//! - `VehiclePart` is the `vehi` struct from 0x2F0 to 0x3F0 (speeds, turn
//!   limits, the type, function sources, flipping, sounds);
//!   `crate::object::Vehicle` composes it after the object and unit layers.
//!   The unit layer's seats are `crate::object::Seat` (read in full there).
//! - `Physics` is a `phys` tag: the rigid body's mass, centre of mass, ground,
//!   water and air parameters, the stored inertia tensor and its inverse, the
//!   powered mass points (channels) and the mass points (the contact
//!   spheres). `Physics::recomputed_inertia`, `recomputed_masses` and
//!   `recomputed_densities` give what the spec says the stored values equal.
//! - `material_ground_scales` reads the globals' 32 materials' vehicle
//!   terrain scales (globals.md "Materials": ground friction, normal k1 and
//!   k0, depth and damp fraction scales).
//!
//! Known shortcuts:
//! - The four vehicle function sources (0x31C to 0x322) are read as raw
//!   `i16` codes: the spec's table of 37 sources is for the exporter, and
//!   only 0, 1, 12, 20, 21, 35 and 36 occur on the disc.

use std::io;

use crate::cache::{MapFile, TagEntry, TagRef, TagStruct, View, class};
use crate::object::bit_flags;
use crate::scenario::coded_enum;

/// The `vehi` main struct's size.
pub const VEHICLE_SIZE: usize = 0x3F0;
/// The `phys` main struct's size.
pub const PHYSICS_SIZE: usize = 0x80;
/// Most mass points or powered mass points read (the disc has at most 22).
pub const MAX_MASS_POINTS: usize = 64;

coded_enum! {
    /// The vehicle's type (0x2F4): it picks the driving model.
    pub enum VehicleType {
        HumanTank = 0,
        HumanJeep = 1,
        HumanBoat = 2,
        HumanPlane = 3,
        AlienScout = 4,
        AlienFighter = 5,
        Turret = 6,
    }
}

bit_flags! {
    /// The vehicle flags (0x2F0).
    pub struct VehicleFlags(u32) {
        SPEED_WAKES_PHYSICS = 0,
        TURN_WAKES_PHYSICS = 1,
        DRIVER_POWER_WAKES_PHYSICS = 2,
        GUNNER_POWER_WAKES_PHYSICS = 3,
        CONTROL_OPPOSITE_SPEED_SETS_BRAKE = 4,
        SLIDE_WAKES_PHYSICS = 5,
        KILLS_RIDERS_AT_TERMINAL_VELOCITY = 6,
        CAUSES_COLLISION_DAMAGE = 7,
        AI_WEAPON_CANNOT_ROTATE = 8,
        AI_DOES_NOT_REQUIRE_DRIVER = 9,
        AI_UNUSED = 10,
        AI_DRIVER_ENABLE = 11,
        AI_DRIVER_FLYING = 12,
        AI_DRIVER_CAN_SIDESTEP = 13,
        AI_DRIVER_HOVERING = 14,
    }
}

crate::tag_struct! {
    /// The vehicle's own fields (absolute offsets in the `vehi` struct).
    pub struct VehiclePart: 0x3F0 {
        0x2F0 flags: VehicleFlags,
        0x2F4 vehicle_type: VehicleType,
        /// WU per tick.
        0x2F8 maximum_forward_speed: f32,
        /// WU per tick, a positive number.
        0x2FC maximum_reverse_speed: f32,
        /// WU per tick per tick: the speed variable's step away from 0.
        0x300 speed_acceleration: f32,
        /// WU per tick per tick: its step toward 0.
        0x304 speed_deceleration: f32,
        /// Degrees for the jeep (the fighter's bank scale, radians).
        0x308 maximum_left_turn: f32,
        /// Degrees, negative.
        0x30C maximum_right_turn: f32,
        /// WU: the wheel and tread spin period.
        0x310 wheel_circumference: f32,
        /// Degrees per second for the jeep (the fighter's attitude rate).
        0x314 turn_rate: f32,
        /// WU per tick: the `~blur` permutation shows at or above it.
        0x318 blur_speed: f32,
        /// Function sources A to D (raw codes, vehicles.md's table).
        0x31C input_a: i16,
        0x31E input_b: i16,
        0x320 input_c: i16,
        0x322 input_d: i16,
        0x330 maximum_left_slide: f32,
        0x334 maximum_right_slide: f32,
        0x338 slide_acceleration: f32,
        0x33C slide_deceleration: f32,
        /// Rad per tick per unit: the flip strength's clamp.
        0x340 minimum_flipping_angular_velocity: f32,
        0x344 maximum_flipping_angular_velocity: f32,
        /// Radians.
        0x360 fixed_gun_yaw: f32,
        /// Radians.
        0x364 fixed_gun_pitch: f32,
        0x380 ai_sideslip_distance: f32,
        0x384 ai_destination_radius: f32,
        0x388 ai_avoidance_distance: f32,
        0x38C ai_pathfinding_radius: f32,
        0x390 ai_charge_repeat_timeout: f32,
        0x394 ai_strafing_abort_range: f32,
        0x398 ai_oversteering_bounds: [f32; 2],
        0x3A0 ai_steering_maximum: f32,
        0x3A4 ai_throttle_maximum: f32,
        0x3A8 ai_move_position_time: f32,
        /// Class `snd!`.
        0x3B0 suspension_sound: TagRef,
        /// Class `snd!`.
        0x3C0 crash_sound: TagRef,
        /// Class `foot`.
        0x3D0 material_effects: TagRef,
        /// Class `effe`.
        0x3E0 effect: TagRef,
    }
}

impl VehiclePart {
    /// Read the part from a view of the tag's main struct (0x3F0 bytes).
    pub fn read_view(v: &View<'_>) -> io::Result<VehiclePart> {
        <VehiclePart as TagStruct>::read(v)
    }

    /// The four function sources, A to D.
    pub fn inputs(&self) -> [i16; 4] {
        [self.input_a, self.input_b, self.input_c, self.input_d]
    }
}

bit_flags! {
    /// A powered mass point's flags.
    pub struct PoweredMassPointFlags(u32) {
        GROUND_FRICTION = 0,
        WATER_FRICTION = 1,
        AIR_FRICTION = 2,
        WATER_LIFT = 3,
        AIR_LIFT = 4,
        THRUST = 5,
        ANTIGRAV = 6,
    }
}

bit_flags! {
    /// A mass point's flags.
    pub struct MassPointFlags(u32) {
        /// Picks the sliding effect type.
        METALLIC = 0,
    }
}

coded_enum! {
    /// How a mass point's friction is shaped.
    pub enum FrictionType {
        Point = 0,
        Forward = 1,
        Left = 2,
        Up = 3,
    }
}

crate::tag_struct! {
    /// A powered mass point (element 0x80): a channel the vehicle drives.
    pub struct PoweredMassPoint: 0x80 {
        0x00 name: String,
        0x20 flags: PoweredMassPointFlags,
        /// In units of the vehicle's weight per point.
        0x24 antigrav_strength: f32,
        0x28 antigrav_offset: f32,
        /// WU.
        0x2C antigrav_height: f32,
        /// Per tick.
        0x30 antigrav_damp_fraction: f32,
        0x34 antigrav_normal_k1: f32,
        0x38 antigrav_normal_k0: f32,
    }
}

crate::tag_struct! {
    /// A mass point (element 0x80): one contact sphere.
    pub struct MassPoint: 0x80 {
        0x00 name: String,
        /// Channel index, -1 none.
        0x20 powered_mass_point: i16,
        0x22 model_node: i16,
        0x24 flags: MassPointFlags,
        0x28 relative_mass: f32,
        0x2C mass: f32,
        0x30 relative_density: f32,
        0x34 density: f32,
        /// WU, model frame.
        0x38 position: [f32; 3],
        0x44 forward: [f32; 3],
        0x50 up: [f32; 3],
        0x5C friction_type: FrictionType,
        0x60 friction_parallel_scale: f32,
        0x64 friction_perpendicular_scale: f32,
        /// WU.
        0x68 radius: f32,
    }
}

crate::tag_struct! {
    /// A 3x3 matrix, row major (element 0x24 of the inertial block).
    pub struct Matrix3: 0x24 {
        0x00 x: [f32; 3],
        0x0C y: [f32; 3],
        0x18 z: [f32; 3],
    }
}

impl Matrix3 {
    /// The rows as an array.
    pub fn rows(&self) -> [[f32; 3]; 3] {
        [self.x, self.y, self.z]
    }
}

crate::tag_struct! {
    /// A `phys` tag's main struct.
    pub struct Physics: 0x80 {
        /// <= 0: the mass-point model; > 0 the older one.
        0x00 radius: f32,
        0x04 moment_scale: f32,
        0x08 mass: f32,
        /// WU, model frame.
        0x0C center_of_mass: [f32; 3],
        0x18 density: f32,
        0x1C gravity_scale: f32,
        /// Per tick.
        0x20 ground_friction: f32,
        /// WU.
        0x24 ground_depth: f32,
        /// Per tick.
        0x28 ground_damp_fraction: f32,
        0x2C ground_normal_k1: f32,
        0x30 ground_normal_k0: f32,
        0x38 water_friction: f32,
        0x3C water_depth: f32,
        0x40 water_density: f32,
        0x48 air_friction: f32,
        0x50 xx_moment: f32,
        0x54 yy_moment: f32,
        0x58 zz_moment: f32,
        /// Element 0 the inertia tensor, element 1 its inverse.
        0x5C inertial_matrix_and_inverse: Vec<Matrix3>,
        0x68 powered_mass_points: Vec<PoweredMassPoint>,
        0x74 mass_points: Vec<MassPoint>,
    }
}

impl Physics {
    /// Read a `phys` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Physics> {
        if tag.class != class::PHYSICS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {} `{}` is not a physics tag",
                    map.header().name,
                    tag.class,
                    tag.path
                ),
            ));
        }
        let v = map.tag_view(tag, PHYSICS_SIZE)?;
        Physics::read_view(&v)
    }

    /// Read from a view of the main struct, checking counts and indices.
    pub fn read_view(v: &View<'_>) -> io::Result<Physics> {
        v.block_max(0x5C, Matrix3::SIZE, 2)?;
        v.block_max(0x68, PoweredMassPoint::SIZE, MAX_MASS_POINTS)?;
        v.block_max(0x74, MassPoint::SIZE, MAX_MASS_POINTS)?;
        let out = <Physics as TagStruct>::read(v)?;
        let channels = out.powered_mass_points.len() as i64;
        for (i, p) in out.mass_points.iter().enumerate() {
            let c = p.powered_mass_point as i64;
            if c < -1 || c >= channels {
                return Err(v.error(0x74, format_args!("mass point {i} names channel {c} of {channels}")));
            }
        }
        Ok(out)
    }

    /// The stored inertia tensor (element 0), if present.
    pub fn inertia(&self) -> Option<[[f32; 3]; 3]> {
        self.inertial_matrix_and_inverse.first().map(Matrix3::rows)
    }

    /// The stored inverse tensor (element 1), if present.
    pub fn inverse_inertia(&self) -> Option<[[f32; 3]; 3]> {
        self.inertial_matrix_and_inverse.get(1).map(Matrix3::rows)
    }

    /// Each point's mass recomputed: M · rel_i / Σ rel.
    pub fn recomputed_masses(&self) -> Vec<f32> {
        let sum: f32 = self.mass_points.iter().map(|p| p.relative_mass).sum();
        self.mass_points
            .iter()
            .map(|p| {
                if sum > 0.0 {
                    self.mass * p.relative_mass / sum
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// Each point's density recomputed: density · reld_i · Σ(rel/reld) /
    /// Σ rel.
    pub fn recomputed_densities(&self) -> Vec<f32> {
        let sum: f32 = self.mass_points.iter().map(|p| p.relative_mass).sum();
        let ratio: f32 = self
            .mass_points
            .iter()
            .map(|p| {
                if p.relative_density != 0.0 {
                    p.relative_mass / p.relative_density
                } else {
                    0.0
                }
            })
            .sum();
        self.mass_points
            .iter()
            .map(|p| {
                if sum > 0.0 {
                    self.density * p.relative_density * ratio / sum
                } else {
                    0.0
                }
            })
            .collect()
    }

    /// The mass-weighted mean of the points' positions (equal to the stored
    /// centre of mass on the disc).
    pub fn recomputed_center_of_mass(&self) -> [f32; 3] {
        let total: f32 = self.mass_points.iter().map(|p| p.mass).sum();
        let mut c = [0.0f32; 3];
        if total <= 0.0 {
            return c;
        }
        for p in &self.mass_points {
            for (k, ck) in c.iter_mut().enumerate() {
                *ck += p.mass * p.position[k] / total;
            }
        }
        c
    }

    /// The inertia tensor recomputed from the points: moment scale · Σ m_i
    /// [(|r_i|² + 0.4 R_i²) E − r_i r_iᵀ], r_i = q_i − c, with the stored
    /// masses and centre of mass.
    pub fn recomputed_inertia(&self) -> [[f32; 3]; 3] {
        let mut t = [[0.0f64; 3]; 3];
        let c = self.center_of_mass;
        for p in &self.mass_points {
            let r = [0, 1, 2].map(|k| (p.position[k] - c[k]) as f64);
            let r2 = r[0] * r[0] + r[1] * r[1] + r[2] * r[2];
            let rad = p.radius as f64;
            for (i, row) in t.iter_mut().enumerate() {
                for (j, e) in row.iter_mut().enumerate() {
                    let diag = if i == j { r2 + 0.4 * rad * rad } else { 0.0 };
                    *e += p.mass as f64 * (diag - r[i] * r[j]);
                }
            }
        }
        t.map(|row| row.map(|e| (e * self.moment_scale as f64) as f32))
    }
}

crate::tag_struct! {
    /// A globals material's vehicle terrain scales (globals.md "Materials",
    /// element 0x374 of the block at 0x194; 0 = no scaling).
    pub struct MaterialGroundScales: 0x374 {
        /// Applies only to physics of at most 7500 mass.
        0x094 ground_friction_scale: f32,
        0x098 ground_normal_k1_scale: f32,
        0x09C ground_normal_k0_scale: f32,
        0x0A0 ground_depth_scale: f32,
        0x0A4 ground_damp_fraction_scale: f32,
    }
}

/// The 32 materials' vehicle terrain scales of a globals tag (`matg`),
/// indexed by material type.
pub fn material_ground_scales(map: &MapFile, tag: &TagEntry) -> io::Result<Vec<MaterialGroundScales>> {
    if tag.class != class::GLOBALS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{}: {} `{}` is not a globals tag",
                map.header().name,
                tag.class,
                tag.path
            ),
        ));
    }
    let v = map.tag_view(tag, crate::globals::GLOBALS_SIZE)?;
    v.block_max(0x194, MaterialGroundScales::SIZE, 64)?.read_all()
}
