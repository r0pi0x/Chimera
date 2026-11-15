//! The damage effect tag (specs/halo_ce/projectiles.md, "The damage effect
//! tag"): a `jpt!` tag read whole as `DamageEffect` (0x2A0 bytes): the
//! radius bounds and flags, the screen flash, controller rumble, camera
//! impulse and shake presentation fields, the sound, the breaking effect
//! values, the damage part (side effect, category, flags, core radius, the
//! damage bounds, vehicle passthrough penalty, active camouflage damage,
//! stun, instantaneous acceleration) and the 40-slot material modifier
//! table, as plain data in the game's own values (world units, radians,
//! seconds, points of vitality; no conversion on load).
//!
//! - `DamageEffect::read(map, tag)`; `material_modifier(type)` looks a
//!   material type's damage modifier up.
//! - Formulas on tag fields alone: `area_scale` (area damage, behaviour 10),
//!   `area_amount`, `amount` (the event's amount from its scale and a
//!   uniform draw, behaviour 8 and 18).
//!
//! Known shortcuts:
//! - Presentation values (screen flash, rumble, camera impulse and shake)
//!   are read, never evaluated: they belong to the HUD and camera.
//! - Modifier slots 33 to 39 are kept as stored (zero on the disc).
//! - The pad ranges are not read.

use std::io;

use crate::cache::{Bounds, Field, MapFile, RealArgb, TagEntry, TagRef, TagStruct, View, class};
use crate::collision::MaterialType;
use crate::object::{TransitionFunction, bit_flags, check_class};
use crate::scenario::coded_enum;
use crate::shader::PeriodicFunction;

/// The damage effect's main struct size.
pub const DAMAGE_EFFECT_SIZE: usize = 0x2A0;
/// Material modifier slots in a damage effect (33 used).
pub const MATERIAL_SLOTS: usize = 40;

bit_flags! {
    /// The effect's flags (offset 0x00C).
    pub struct EffectFlags(u32) {
        /// Area damage is not scaled by distance (the 11 weapon melee
        /// effects only).
        DO_NOT_SCALE_DAMAGE_BY_DISTANCE = 0,
    }
}

bit_flags! {
    /// The damage part's flags (offset 0x1C8).
    pub struct DamageFlags(u32) {
        /// Area damage skips the owner object.
        DOES_NOT_HURT_OWNER = 0,
        /// A hit with body damage on a "head" material kills outright in a
        /// multiplayer game or against a victim that is not a player's biped.
        CAN_CAUSE_HEADSHOTS = 1,
        /// Animation only.
        PINGS_RESISTANT_UNITS = 2,
        /// Area damage skips objects of a team that is not the owner's
        /// enemy.
        DOES_NOT_HURT_FRIENDS = 3,
        /// Animation only.
        DOES_NOT_PING_UNITS = 4,
        /// Doubles the push on vehicles; hurts objects only explosives hurt.
        DETONATES_EXPLOSIVES = 5,
        /// No body damage.
        ONLY_HURTS_SHIELDS = 6,
        /// The victim burns when killed.
        CAUSES_FLAMING_DEATH = 7,
        /// HUD.
        DAMAGE_INDICATORS_ALWAYS_POINT_DOWN = 8,
        /// The shield is ignored.
        SKIPS_SHIELDS = 9,
        /// Campaign only.
        DOES_NOT_HURT_INFECTION_FORMS = 10,
        /// In a multiplayer game, body damage to a head material is doubled.
        CAN_CAUSE_MULTIPLAYER_HEADSHOTS = 11,
        /// Campaign only.
        INFECTION_FORM_POP = 12,
    }
}

coded_enum! {
    /// The screen flash type (names from general knowledge of the tag, as
    /// the spec says).
    pub enum ScreenFlashType {
        None = 0,
        Lighten = 1,
        Darken = 2,
        Max = 3,
        Min = 4,
        Invert = 5,
        Tint = 6,
    }
}

coded_enum! {
    /// The screen flash priority.
    pub enum FlashPriority {
        Low = 0,
        Medium = 1,
        High = 2,
    }
}

coded_enum! {
    /// The damage side effect (offset 0x1C4).
    pub enum SideEffect {
        None = 0,
        Harmless = 1,
        LethalToTheUnsuspecting = 2,
        Emp = 3,
    }
}

coded_enum! {
    /// The damage category (offset 0x1C6).
    pub enum DamageCategory {
        None = 0,
        Falling = 1,
        Bullet = 2,
        Grenade = 3,
        HighExplosive = 4,
        Sniper = 5,
        Melee = 6,
        Flame = 7,
        MountedWeapon = 8,
        Vehicle = 9,
        Plasma = 10,
        Needle = 11,
        Shotgun = 12,
    }
}

/// The 40 per-material damage modifiers (offset 0x200): slot `t` multiplies
/// damage to material type `t` (types 0 to 32; 33 to 39 unused).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MaterialModifiers(pub [f32; MATERIAL_SLOTS]);

impl MaterialModifiers {
    /// The modifier for a material type.
    pub fn get(&self, material: MaterialType) -> f32 {
        self.0[material.code() as usize]
    }
}

impl Field for MaterialModifiers {
    const SIZE: usize = 4 * MATERIAL_SLOTS;
    fn read(v: &View<'_>, offset: usize) -> io::Result<Self> {
        let mut m = [0.0; MATERIAL_SLOTS];
        for (i, x) in m.iter_mut().enumerate() {
            *x = v.f32(offset + 4 * i)?;
        }
        Ok(MaterialModifiers(m))
    }
}

crate::tag_struct! {
    /// A damage effect tag (`jpt!`), absolute offsets of its main struct.
    pub struct DamageEffect: 0x2A0 {
        /// WU: the falloff radius (full scale inside it) and the cutoff
        /// radius (nothing at or beyond it).
        0x000 radius: Bounds<f32>,
        /// Not read by the engine for damage (0 on every disc tag).
        0x008 cutoff_scale: f32,
        0x00C flags: EffectFlags,
        0x024 screen_flash_type: ScreenFlashType,
        0x026 screen_flash_priority: FlashPriority,
        /// Seconds.
        0x034 screen_flash_duration: f32,
        0x038 screen_flash_fade_function: TransitionFunction,
        /// Fraction.
        0x044 screen_flash_maximum_intensity: f32,
        /// Fraction: the intensity at scale 0.
        0x048 screen_flash_zero_scale_factor: f32,
        0x04C screen_flash_color: RealArgb,
        /// Fraction: controller rumble, left motor.
        0x05C low_frequency_vibrate_frequency: f32,
        /// Seconds.
        0x060 low_frequency_vibrate_duration: f32,
        0x064 low_frequency_vibrate_fade_function: TransitionFunction,
        /// Fraction: right motor.
        0x070 high_frequency_vibrate_frequency: f32,
        /// Seconds.
        0x074 high_frequency_vibrate_duration: f32,
        0x078 high_frequency_vibrate_fade_function: TransitionFunction,
        /// Fraction.
        0x084 vibrate_zero_scale_factor: f32,
        /// Seconds.
        0x098 temporary_camera_impulse_duration: f32,
        0x09C temporary_camera_impulse_fade_function: TransitionFunction,
        /// Radians.
        0x0A0 temporary_camera_impulse_rotation: f32,
        /// WU.
        0x0A4 temporary_camera_impulse_pushback: f32,
        /// WU.
        0x0A8 temporary_camera_impulse_jitter: Bounds<f32>,
        /// Fraction.
        0x0B0 temporary_camera_impulse_zero_scale_factor: f32,
        /// Radians.
        0x0B8 permanent_camera_impulse_angle: f32,
        /// Fraction.
        0x0BC permanent_camera_impulse_zero_scale_factor: f32,
        /// Seconds.
        0x0CC camera_shaking_duration: f32,
        0x0D0 camera_shaking_falloff_function: TransitionFunction,
        /// WU.
        0x0D4 camera_shaking_random_translation: f32,
        /// Radians.
        0x0D8 camera_shaking_random_rotation: f32,
        0x0E8 camera_shaking_wobble_function: PeriodicFunction,
        /// Seconds (30 on most disc tags).
        0x0EC camera_shaking_wobble_period: f32,
        /// Fraction.
        0x0F0 camera_shaking_wobble_weight: f32,
        /// Fraction.
        0x0F4 camera_shaking_zero_scale_factor: f32,
        /// `snd!`: played unspatialised for a damaged local player.
        0x114 sound: TagRef,
        /// WU per second.
        0x194 breaking_effect_forward_velocity: f32,
        /// WU.
        0x198 breaking_effect_forward_radius: f32,
        0x19C breaking_effect_forward_exponent: f32,
        /// WU per second.
        0x1AC breaking_effect_outward_velocity: f32,
        /// WU.
        0x1B0 breaking_effect_outward_radius: f32,
        0x1B4 breaking_effect_outward_exponent: f32,
        0x1C4 side_effect: SideEffect,
        0x1C6 category: DamageCategory,
        0x1C8 damage_flags: DamageFlags,
        /// WU: the width of the occlusion probes for units.
        0x1CC area_of_effect_core_radius: f32,
        /// Points: the amount at scale 0 (the "minimum").
        0x1D0 damage_lower_bound: f32,
        /// Points: the random amount at scale 1 is U(lower, upper) of this
        /// pair.
        0x1D4 damage_upper_bound: Bounds<f32>,
        /// Fraction: the share withheld from a vehicle's riders on a direct
        /// hit on the vehicle.
        0x1DC vehicle_passthrough_penalty: f32,
        /// Fraction: taken off a camouflaged unit's camouflage per hit.
        0x1E0 active_camouflage_damage: f32,
        /// Fraction: added to a player's stun per hit, times the event scale.
        0x1E4 stun: f32,
        /// Fraction: the stun is not raised above this times the scale.
        0x1E8 maximum_stun: f32,
        /// Seconds: added to the player's stun ticks.
        0x1EC stun_time: f32,
        /// WU per second of velocity change (the engine divides by 30): the
        /// push.
        0x1F4 instantaneous_acceleration: f32,
        /// Factors: slot `t` multiplies damage to material type `t`.
        0x200 material_modifiers: MaterialModifiers,
    }
}

impl DamageEffect {
    /// Read a `jpt!` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<DamageEffect> {
        let v = check_class(map, tag, class::DAMAGE_EFFECT, "damage effect", DAMAGE_EFFECT_SIZE)?;
        <DamageEffect as TagStruct>::read(&v)
    }

    /// The damage modifier for a material type.
    pub fn material_modifier(&self, material: MaterialType) -> f32 {
        self.material_modifiers.get(material)
    }

    /// The modifier for a hit that may report no material (the "none" type
    /// multiplies by 1: the spec's built-in handling is the target's).
    pub fn material_modifier_or_one(&self, material: Option<MaterialType>) -> f32 {
        material.map_or(1.0, |m| self.material_modifier(m))
    }

    /// The area damage scale at distance `d` (WU) from the epicenter to the
    /// target's bounding-sphere centre (behaviour 10): `clamp(1 - (d - r0) /
    /// (r1 - r0), 0, 1)` when the cutoff radius `r1` exceeds the falloff
    /// radius `r0`, else 1.
    pub fn area_scale(&self, d: f32) -> f32 {
        let Bounds { lower: r0, upper: r1 } = self.radius;
        if r1 > r0 {
            (1.0 - (d - r0) / (r1 - r0)).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }

    /// The scale an area damage event carries: `area_scale(d)`, or the
    /// effect's own scale (`event_scale`) when the effect has "do not scale
    /// damage by distance".
    pub fn area_event_scale(&self, d: f32, event_scale: f32) -> f32 {
        if self.flags.contains(EffectFlags::DO_NOT_SCALE_DAMAGE_BY_DISTANCE) {
            event_scale
        } else {
            self.area_scale(d)
        }
    }

    /// The damage event's amount for a scale `s` and a uniform draw `u` in
    /// 0 to 1 (behaviour 8 and 18): `(1 - s) lower + s U(upper pair)`, with
    /// `U(a, b) = a + u (b - a)`.
    pub fn amount(&self, s: f32, u: f32) -> f32 {
        let Bounds { lower, upper } = self.damage_upper_bound;
        (1.0 - s) * self.damage_lower_bound + s * (lower + u * (upper - lower))
    }

    /// The area damage amount at distance `d` for a draw `u` (behaviour
    /// 10's one line): 0 at or beyond the cutoff radius, else `amount` at
    /// `area_scale(d)`.
    pub fn area_amount(&self, d: f32, u: f32) -> f32 {
        if d >= self.radius.upper {
            0.0
        } else {
            self.amount(self.area_scale(d), u)
        }
    }
}
