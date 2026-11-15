//! The projectile tag (specs/halo_ce/projectiles.md, "The projectile tag"):
//! a `proj` tag read whole as `Projectile { object, own }`, the object base
//! of `crate::object` and the projectile's own part (flags, timers, flight
//! numbers, the effect and damage references) with its material responses
//! block (one element per material type), as plain data in the game's own
//! values with no conversion on load.
//!
//! Units, as the spec states them: velocities (initial, final, minimum and
//! the response bounds and noise) in world units per tick; ranges, radii in
//! world units; arming time and timer bounds in seconds; guided angular
//! velocity in radians per second; response angles in radians.
//!
//! - `Projectile::read(map, tag)`; `ProjectilePart::response(type)` looks a
//!   material type's response up (`None`: the built-in one, see
//!   `MaterialResponse::built_in`).
//! - Formulas on tag fields alone: `ProjectilePart::air_deceleration` and
//!   `water_deceleration` (flight, behaviour 3), `ProjectilePart::
//!   direct_hit_scale` (behaviour 8) and `direct_hit_amount` over a damage
//!   effect (`crate::damage_effect`).
//!
//! Known shortcuts:
//! - The material responses block is capped at 64 elements (the spec's
//!   damage effects have 40 modifier slots and the disc 33 elements: our
//!   sanity bound); more is refused.
//! - The padding ranges (0x14, 0x4C and 0x78 of a response; 0x1F2 and
//!   0x234 of the part) are not read; the spec says they are zero.

use std::io;

use crate::cache::{Bounds, ClassCode, MapFile, TagEntry, TagId, TagRef, TagStruct, View, class};
use crate::collision::MaterialType;
use crate::object::{ObjectBase, ObjectType, bit_flags, check_class, check_type, size};
use crate::scenario::coded_enum;

/// Where the projectile's own fields begin.
pub const PROJECTILE_PART_OFFSET: usize = 0x17C;
/// One material response element's size.
pub const RESPONSE_SIZE: usize = 0xA0;
/// Most material response elements read.
pub const MAX_RESPONSES: usize = 64;
/// Game ticks per second.
pub const TICKS: f32 = 30.0;

bit_flags! {
    /// The projectile flags (offset 0x17C); bits 4 to 6 are not read by the
    /// engine.
    pub struct ProjectileFlags(u32) {
        /// The object's forward follows its velocity (presentation).
        ORIENTED_ALONG_VELOCITY = 0,
        AI_MUST_USE_BALLISTIC_AIMING = 1,
        /// The timer starts from the lower bound; on sticking it is re-armed
        /// to the upper bound.
        DETONATION_MAX_TIME_IF_ATTACHED = 2,
        HAS_SUPER_COMBINING_EXPLOSION = 3,
        COMBINE_INITIAL_VELOCITY_WITH_PARENT_VELOCITY = 4,
        RANDOM_ATTACHED_DETONATION_TIME = 5,
        MINIMUM_UNATTACHED_DETONATION_TIME = 6,
    }
}

coded_enum! {
    /// When the detonation timer starts (offset 0x180). 1 and 2 act the same
    /// (behaviour 2).
    pub enum TimerStarts {
        Immediately = 0,
        AfterFirstBounce = 1,
        WhenAtRest = 2,
    }
}

coded_enum! {
    /// How loud an impact or a detonation is to AI hearing (names inferred,
    /// as objects.md).
    pub enum NoiseLevel {
        Silent = 0,
        Medium = 1,
        Loud = 2,
        Shout = 3,
        Quiet = 4,
    }
}

coded_enum! {
    /// The source of one of the projectile's object inputs A to D.
    pub enum ProjectileInput {
        None = 0,
        RangeRemaining = 1,
        TimeRemaining = 2,
        Tracer = 3,
    }
}

coded_enum! {
    /// What a projectile does at a surface (default and potential response).
    pub enum ResponseType {
        Disappear = 0,
        Detonate = 1,
        /// Bounce, with the two frictions.
        Reflect = 2,
        /// Pass through.
        Overpenetrate = 3,
        /// Stick.
        Attach = 4,
    }
}

coded_enum! {
    /// What a response scales its impact effect by.
    pub enum ScaleEffectsBy {
        Damage = 0,
        Angle = 1,
    }
}

bit_flags! {
    /// A response's flags (element 0x00); not read by the engine.
    pub struct ResponseFlags(u16) {
        CANNOT_BE_OVERPENETRATED = 0,
    }
}

bit_flags! {
    /// A response's potential flags (element 0x26).
    pub struct PotentialFlags(u16) {
        /// The potential response applies only against units.
        ONLY_AGAINST_UNITS = 0,
    }
}

crate::tag_struct! {
    /// One material response (element 0xA0): what happens when the
    /// projectile meets a surface of one material type (behaviour 6 and 7).
    pub struct MaterialResponse: 0xA0 {
        0x00 flags: ResponseFlags,
        0x02 default_response: ResponseType,
        /// `effe`: the impact effect with the default response.
        0x04 default_effect: TagRef,
        /// `ResponseType::Disappear` means no potential response.
        0x24 potential_response: ResponseType,
        0x26 potential_flags: PotentialFlags,
        /// Fraction: the chance the potential response is skipped.
        0x28 skip_fraction: f32,
        /// Radians: the impact angle window (0 grazing, pi/2 head-on); an
        /// upper bound of 0 is no angle condition.
        0x2C between_angle: Bounds<f32>,
        /// WU per tick: the impact speed window (normal component); an upper
        /// bound of 0 is no velocity condition.
        0x34 and_velocity: Bounds<f32>,
        /// `effe`: the impact effect with the potential response.
        0x3C potential_effect: TagRef,
        0x5C scale_effects_by: ScaleEffectsBy,
        /// Radians: added to the measured impact angle, turns the outgoing
        /// velocity.
        0x60 angular_noise: f32,
        /// WU per tick: lowers the measured impact speed, changes the
        /// outgoing speed.
        0x64 velocity_noise: f32,
        /// `effe`: extra effect when the projectile later detonates having
        /// hit this material.
        0x68 detonation_effect: TagRef,
        /// Fraction: speed lost passing through an object.
        0x90 initial_friction: f32,
        /// Not read by the engine (100 on some needle entries).
        0x94 maximum_distance: f32,
        /// Fraction of the velocity component along the surface normal lost
        /// in a bounce (open question 1 of the spec on the axes).
        0x98 parallel_friction: f32,
        /// Fraction of the component along the surface lost in a bounce.
        0x9C perpendicular_friction: f32,
    }
}

impl MaterialResponse {
    /// The engine's built-in response for a material type with no element:
    /// default disappear, no potential response, no effects, all zeros.
    pub fn built_in() -> MaterialResponse {
        let none = |c: ClassCode| TagRef {
            class: c,
            path_address: 0,
            id: TagId::NONE,
        };
        MaterialResponse {
            flags: ResponseFlags(0),
            default_response: ResponseType::Disappear,
            default_effect: none(class::EFFECT),
            potential_response: ResponseType::Disappear,
            potential_flags: PotentialFlags(0),
            skip_fraction: 0.0,
            between_angle: Bounds::default(),
            and_velocity: Bounds::default(),
            potential_effect: none(class::EFFECT),
            scale_effects_by: ScaleEffectsBy::Damage,
            angular_noise: 0.0,
            velocity_noise: 0.0,
            detonation_effect: none(class::EFFECT),
            initial_friction: 0.0,
            maximum_distance: 0.0,
            parallel_friction: 0.0,
            perpendicular_friction: 0.0,
        }
    }

    /// Whether the potential response exists (it is not "disappear").
    pub fn has_potential(&self) -> bool {
        self.potential_response != ResponseType::Disappear
    }

    /// The potential response's conditions on the impact: the angle `alpha`
    /// (radians) and the speed `u` (WU per tick) are inside their windows
    /// (an upper bound of 0 is no condition), and "only against units" is
    /// clear or the object hit is a unit. The skip chance is the caller's
    /// (a fresh uniform draw `>= skip_fraction`). Behaviour 6, step 4.
    pub fn potential_applies(&self, alpha: f32, u: f32, hit_is_unit: bool) -> bool {
        self.has_potential()
            && (self.between_angle.upper == 0.0
                || (self.between_angle.lower <= alpha && alpha <= self.between_angle.upper))
            && (self.and_velocity.upper == 0.0 || (self.and_velocity.lower <= u && u <= self.and_velocity.upper))
            && (!self.potential_flags.contains(PotentialFlags::ONLY_AGAINST_UNITS) || hit_is_unit)
    }
}

crate::tag_struct! {
    /// The projectile's own part, absolute offsets of the tag's main struct
    /// (`SIZE` is the struct's end, 0x24C).
    pub struct ProjectilePart: 0x24C {
        0x17C flags: ProjectileFlags,
        0x180 detonation_timer_starts: TimerStarts,
        0x182 impact_noise: NoiseLevel,
        0x184 input_a: ProjectileInput,
        0x186 input_b: ProjectileInput,
        0x188 input_c: ProjectileInput,
        0x18A input_d: ProjectileInput,
        /// `effe`: the effect of a supercombine.
        0x18C super_detonation: TagRef,
        /// WU: how close AI notices it (AI only).
        0x19C ai_perception_radius: f32,
        /// WU: extra width against objects; 0 is a line.
        0x1A0 collision_radius: f32,
        /// Seconds: a requested detonation waits until armed.
        0x1A4 arming_time: f32,
        /// WU: AI flee radius (AI only).
        0x1A8 danger_radius: f32,
        /// `effe`: the detonation effect (it carries the area damage).
        0x1AC effect: TagRef,
        /// Seconds: the detonation timer's lower and upper bounds (an upper
        /// bound of 0: no timer).
        0x1BC detonation_timer: Bounds<f32>,
        /// WU per tick: after a collision response, slower than this
        /// detonates.
        0x1C4 minimum_velocity: f32,
        /// WU: travelled distance at which it detonates; 0 is none.
        0x1C8 maximum_range: f32,
        /// Times the engine gravity while in the air.
        0x1CC air_gravity_scale: f32,
        /// WU: the deceleration window in the air.
        0x1D0 air_damage_range: Bounds<f32>,
        /// Times the engine gravity while wholly under water.
        0x1D8 water_gravity_scale: f32,
        /// WU: the deceleration window under water (its upper bound is also
        /// used, in the air, by one disappearance test).
        0x1DC water_damage_range: Bounds<f32>,
        /// WU per tick: the speed added along the aim at creation.
        0x1E4 initial_velocity: f32,
        /// WU per tick: the speed after the deceleration window.
        0x1E8 final_velocity: f32,
        /// Radians per second: the turning rate toward the target.
        0x1EC guided_angular_velocity: f32,
        0x1F0 detonation_noise: NoiseLevel,
        /// `effe`: played once when the timer first starts after a stop or a
        /// stick.
        0x1F4 detonation_started: TagRef,
        /// `snd!`.
        0x204 flyby_sound: TagRef,
        /// `jpt!`: applied to the object it is stuck in when it detonates.
        0x214 attached_detonation_damage: TagRef,
        /// `jpt!`: applied to an object it hits.
        0x224 impact_damage: TagRef,
        /// One element per material type in code order (0 dirt first).
        0x240 material_responses: Vec<MaterialResponse>,
    }
}

impl ProjectilePart {
    /// The response element for a material type, or `None` when the block
    /// has no such element (use `MaterialResponse::built_in`).
    pub fn response(&self, material: MaterialType) -> Option<&MaterialResponse> {
        self.material_responses.get(material.code() as usize)
    }

    /// The response for a hit that may report no material (`None` is the
    /// "none" type): the element, or the built-in response.
    pub fn response_or_built_in(&self, material: Option<MaterialType>) -> MaterialResponse {
        material
            .and_then(|m| self.response(m))
            .cloned()
            .unwrap_or_else(MaterialResponse::built_in)
    }

    /// The deceleration state's `a` for a damage window (behaviour 3):
    /// `(v0^2 - vf^2) / (2 (upper - lower))` in WU per tick squared when the
    /// velocities differ and the window is not empty, else 0.
    pub fn deceleration(&self, window: Bounds<f32>) -> f32 {
        let (v0, vf) = (self.initial_velocity, self.final_velocity);
        if v0 != vf && window.upper != window.lower {
            (v0 * v0 - vf * vf) / (2.0 * (window.upper - window.lower))
        } else {
            0.0
        }
    }

    /// `deceleration` over the air damage range.
    pub fn air_deceleration(&self) -> f32 {
        self.deceleration(self.air_damage_range)
    }

    /// `deceleration` over the water damage range.
    pub fn water_deceleration(&self) -> f32 {
        self.deceleration(self.water_damage_range)
    }

    /// The deceleration timer's step for a window: `lower / initial
    /// velocity` (a distance over a speed, as the spec's executable check
    /// says) when `lower > 0`, else 0 (the timer starts at 1).
    pub fn deceleration_step(&self, window: Bounds<f32>) -> f32 {
        if window.lower > 0.0 && self.initial_velocity != 0.0 {
            window.lower / self.initial_velocity
        } else {
            0.0
        }
    }

    /// The scale of a direct hit's damage event (behaviour 8): the speed at
    /// impact (`speed`, WU per tick) minus the final velocity over the
    /// initial minus the final, clamped to 0 to 1, when the two differ, else 1.
    pub fn direct_hit_scale(&self, speed: f32) -> f32 {
        let (v0, vf) = (self.initial_velocity, self.final_velocity);
        if v0 != vf {
            ((speed - vf) / (v0 - vf)).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// The detonation timer's range as ticks (`seconds x 30`, truncated).
    pub fn timer_ticks(&self) -> Bounds<i32> {
        Bounds {
            lower: (self.detonation_timer.lower * TICKS) as i32,
            upper: (self.detonation_timer.upper * TICKS) as i32,
        }
    }

    /// Whether the projectile has a detonation timer (upper bound not 0).
    pub fn has_timer(&self) -> bool {
        self.detonation_timer.upper != 0.0
    }
}

/// A projectile tag: the object base and the projectile's own part.
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub object: ObjectBase,
    pub own: ProjectilePart,
}

impl Projectile {
    /// Read a `proj` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Projectile> {
        let v = check_class(map, tag, class::PROJECTILE, "projectile", size::PROJECTILE)?;
        Self::read_view(&v)
    }

    /// Read from a view of the tag's main struct (0x24C bytes).
    pub fn read_view(v: &View<'_>) -> io::Result<Projectile> {
        let object = ObjectBase::read_view(v)?;
        check_type(v, &object, ObjectType::Projectile)?;
        let own = <ProjectilePart as TagStruct>::read(v)?;
        if own.material_responses.len() > MAX_RESPONSES {
            return Err(v.error(
                0x240,
                format_args!(
                    "{} material responses over the maximum {MAX_RESPONSES}",
                    own.material_responses.len()
                ),
            ));
        }
        Ok(Projectile { object, own })
    }
}
