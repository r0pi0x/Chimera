//! The weapon tag (specs/halo_ce/weapons.md, "The weapon tag"): a `weap`
//! tag read whole as `Weapon { object, item, own }`, the object and item
//! layers of `crate::object` and the weapon's own part (flags, heat, zoom,
//! aim assist, movement penalties, the references, the weapon type, the
//! magazines and the triggers with their firing effects), as plain data in
//! the game's own values (world units; radians; seconds unless a field says
//! per tick). No behaviour: only reading and the pure formulas the spec
//! defines on tag fields.
//!
//! - `Weapon::read(map, tag)`; `Weapon::magazine_of(trigger)` resolves a
//!   trigger's magazine index.
//! - Formulas: `Trigger::required_ticks` and `Trigger::ticks_between_shots`
//!   (the rate of fire rule, section 4), `WeaponPart::magnification` and
//!   `zoomed_field_of_view` (section 12), `WeaponPart::aim_assist` and
//!   `AimAssist::falloff` (section 15), `per_tick_rate` (the build's
//!   runtime rates) and `seconds_to_ticks` (the truncating conversion).
//!
//! Known shortcuts:
//! - At most two magazines and two triggers are kept (the engine runs no
//!   more); a trigger's magazine index is checked against the kept
//!   magazines and a bad one is refused (the spec: treat the tag as
//!   damaged). The flags "bit 0 wastes rounds" and the rest are typed bit
//!   sets; the unused block at 0x1B8 is not read.
//! - The weapon struct is read as 0x508 bytes: the one weapon tag whose
//!   first block lies 0x528 bytes in (the objects spec's open question)
//!   reads like the others.

use std::io;

use crate::cache::{Bounds, MapFile, TagEntry, TagId, TagRef, TagStruct, View, class};
use crate::object::{ItemBase, ObjectBase, ObjectType, PredictedResource, bit_flags, check_class, check_type, size};
use crate::scenario::coded_enum;

pub mod trigger;

pub use trigger::{
    AmmunitionObject, Distribution, FiringEffect, FiringNoise, Magazine, MagazineFlags, OverchargedAction, Trigger,
    TriggerFlags,
};

/// Where the weapon part begins in a `weap` tag.
pub const WEAPON_PART_OFFSET: usize = 0x308;
/// The weapon part's size.
pub const WEAPON_PART_SIZE: usize = 0x200;
/// Triggers a weapon runs.
pub const MAX_TRIGGERS: usize = 2;
/// Magazines a weapon has.
pub const MAX_MAGAZINES: usize = 2;
/// Game ticks per second.
pub const TICK_RATE: f32 = 30.0;
/// A rate at or below this means "no limit".
pub const ROF_EPSILON: f32 = 0.0001;
/// A zoomed field of view outside `ZOOM_FOV_MIN..=ZOOM_FOV_MAX` (radians)
/// is not applied.
pub const ZOOM_FOV_MIN: f32 = std::f32::consts::PI / 100.0;
/// See `ZOOM_FOV_MIN`.
pub const ZOOM_FOV_MAX: f32 = 99.0 * std::f32::consts::PI / 100.0;

bit_flags! {
    /// The weapon flags (weapon part offset 0).
    pub struct WeaponFlags(u32) {
        VERTICAL_HEAT_DISPLAY = 0,
        MUTUALLY_EXCLUSIVE_TRIGGERS = 1,
        ATTACKS_AUTOMATICALLY_ON_BUMP = 2,
        /// The flag and the ball: always in hand, a trigger drops it.
        MUST_BE_READIED = 3,
        DOESNT_COUNT_TOWARD_MAXIMUM = 4,
        AIM_ASSISTS_ONLY_WHEN_ZOOMED = 5,
        PREVENTS_GRENADE_THROWING = 6,
        MUST_BE_PICKED_UP = 7,
        HOLDS_TRIGGERS_WHEN_DROPPED = 8,
        PREVENTS_MELEE_ATTACK = 9,
        DETONATES_WHEN_DROPPED = 10,
        CANNOT_FIRE_AT_MAXIMUM_AGE = 11,
        SECONDARY_TRIGGER_OVERRIDES_GRENADES = 12,
        DOES_NOT_DEPOWER_ACTIVE_CAMO = 13,
        ENABLES_INTEGRATED_NIGHT_VISION = 14,
        AI_USE_WEAPON_MELEE_DAMAGE = 15,
    }
}

coded_enum! {
    /// The secondary trigger mode (weapon part 0x24).
    pub enum SecondaryTriggerMode {
        Normal = 0,
        SlavedToPrimary = 1,
        InhibitsPrimary = 2,
        LoadsAlternateAmmunition = 3,
        LoadsMultiplePrimaryAmmunition = 4,
    }
}

coded_enum! {
    /// Where a weapon-layer incoming function value comes from (inputs A to
    /// D at part 0x28).
    pub enum WeaponInput {
        None = 0,
        Heat = 1,
        PrimaryAmmunition = 2,
        SecondaryAmmunition = 3,
        PrimaryRateOfFire = 4,
        SecondaryRateOfFire = 5,
        Ready = 6,
        PrimaryEjectionPort = 7,
        SecondaryEjectionPort = 8,
        Overheated = 9,
        PrimaryCharged = 10,
        SecondaryCharged = 11,
        Illumination = 12,
        Age = 13,
        IntegratedLight = 14,
        PrimaryFiring = 15,
        SecondaryFiring = 16,
    }
}

coded_enum! {
    /// When the movement penalty applies (unused by the engine).
    pub enum MovementPenalized {
        Always = 0,
        WhenZoomed = 1,
        WhenZoomedOrReloading = 2,
    }
}

coded_enum! {
    /// The weapon type (part 0x1DA).
    pub enum WeaponType {
        Undefined = 0,
        Shotgun = 1,
        Needler = 2,
        PlasmaPistol = 3,
        PlasmaRifle = 4,
    }
}

crate::tag_struct! {
    /// The weapon's own part (offsets relative to 0x308, as the weapons
    /// spec's table gives them; read from `Weapon::read`). Fields the
    /// reference never reads ("unused", "AI") are kept: they are tag data.
    pub struct WeaponPart: 0x200 {
        0x000 flags: WeaponFlags,
        /// The weapon class label the holder's animation graph looks up
        /// (`ar`, `hp`, `sg`).
        0x004 label: String,
        0x024 secondary_trigger_mode: SecondaryTriggerMode,
        0x026 maximum_alternate_shots_loaded: i16,
        0x028 input_a: WeaponInput,
        0x02A input_b: WeaponInput,
        0x02C input_c: WeaponInput,
        0x02E input_d: WeaponInput,
        /// Seconds; unused (the draw length is the first-person animation).
        0x030 ready_time: f32,
        /// `effe` or `snd!`.
        0x034 ready_effect: TagRef,
        /// Fraction: an overheated weapon may fire again below this heat.
        0x044 heat_recovery_threshold: f32,
        /// Fraction: heat at or above this overheats.
        0x048 overheated_threshold: f32,
        /// Fraction: heat above this risks a detonation per round.
        0x04C heat_detonation_threshold: f32,
        /// Fraction: the chance per round of that detonation.
        0x050 heat_detonation_fraction: f32,
        /// Per second (per tick: / 30).
        0x054 heat_loss_per_second: f32,
        /// The illumination function = this x heat.
        0x058 heat_illumination: f32,
        /// `effe` or `snd!`: looping while overheated.
        0x06C overheated: TagRef,
        /// `effe` or `snd!`: when the weapon detonates.
        0x07C detonation: TagRef,
        /// `jpt!`: a player's melee with this weapon in hand.
        0x08C melee_damage: TagRef,
        /// `jpt!`: applied to the attacker when the melee hits.
        0x09C melee_response: TagRef,
        /// `actv`: AI only.
        0x0B4 actor_firing_parameters: TagRef,
        /// World units (HUD and AI).
        0x0C4 near_reticle_range: f32,
        0x0C8 far_reticle_range: f32,
        0x0CC intersection_reticle_range: f32,
        /// Zoom levels (0: no zoom).
        0x0D2 magnification_levels: i16,
        /// Lower and upper magnification (x).
        0x0D4 magnification_range: Bounds<f32>,
        /// Radians: the bullet bending cone.
        0x0DC autoaim_angle: f32,
        /// World units.
        0x0E0 autoaim_range: f32,
        /// Radians: the look stickiness cone.
        0x0E4 magnetism_angle: f32,
        /// World units.
        0x0E8 magnetism_range: f32,
        /// Radians: the most a shot may be bent from the aim (the larger of
        /// this and the autoaim angle is used).
        0x0EC deviation_angle: f32,
        /// Unused by the reference.
        0x0F4 movement_penalized: MovementPenalized,
        /// Fraction; unused by the reference.
        0x0F8 forward_movement_penalty: f32,
        /// Fraction; unused by the reference.
        0x0FC sideways_movement_penalty: f32,
        /// World units (AI).
        0x104 minimum_target_range: f32,
        /// AI.
        0x108 looking_time_modifier: f32,
        /// Seconds; unused by the reference.
        0x110 light_power_on_time: f32,
        0x114 light_power_off_time: f32,
        /// `effe` or `snd!`.
        0x118 light_power_on_effect: TagRef,
        0x128 light_power_off_effect: TagRef,
        /// Heat loss x (1 - age x this).
        0x138 age_heat_recovery_penalty: f32,
        /// Required ticks x (1 + age x this).
        0x13C age_rate_of_fire_penalty: f32,
        /// Misfires are possible above this age.
        0x140 age_misfire_start: f32,
        /// The misfire chance at age 1.
        0x144 age_misfire_chance: f32,
        /// Class `mode`.
        0x154 first_person_model: TagRef,
        /// Class `antr`: its first-person weapon animations time draws,
        /// reloads and melee.
        0x164 first_person_animations: TagRef,
        /// Class `wphi`.
        0x178 hud_interface: TagRef,
        /// Class `snd!`: ammunition taken from a weapon of the same type.
        0x188 pickup_sound: TagRef,
        /// Class `snd!`.
        0x198 zoom_in_sound: TagRef,
        0x1A8 zoom_out_sound: TagRef,
        /// Fraction of camouflage lost per shot (0: 0.1).
        0x1C4 active_camo_ding: f32,
        /// Per tick (0: 1/120).
        0x1C8 active_camo_regrowth_rate: f32,
        0x1DA weapon_type: WeaponType,
        0x1DC predicted_resources: Vec<PredictedResource>,
        /// At most 2 are kept.
        0x1E8 magazines: Vec<Magazine>,
        /// At most 2 are kept.
        0x1F4 triggers: Vec<Trigger>,
    }
}

/// A whole weapon tag: the object and item layers and the weapon part.
#[derive(Clone, Debug, PartialEq)]
pub struct Weapon {
    pub object: ObjectBase,
    pub item: ItemBase,
    pub own: WeaponPart,
}

impl Weapon {
    /// Read a `weap` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Weapon> {
        let v = check_class(map, tag, class::WEAPON, "weapon", size::WEAPON)?;
        Self::read_view(&v)
    }

    /// Read from a view of the tag's main struct (0x508 bytes).
    pub fn read_view(v: &View<'_>) -> io::Result<Weapon> {
        let object = ObjectBase::read_view(v)?;
        check_type(v, &object, ObjectType::Weapon)?;
        let item = ItemBase::read_view(v)?;
        let part = v.sub(WEAPON_PART_OFFSET, WEAPON_PART_SIZE)?;
        let mut own = <WeaponPart as TagStruct>::read(&part)?;
        own.magazines.truncate(MAX_MAGAZINES);
        own.triggers.truncate(MAX_TRIGGERS);
        for (i, t) in own.triggers.iter().enumerate() {
            if t.magazine < -1 || t.magazine as i32 >= own.magazines.len() as i32 {
                return Err(part.error(
                    0x1F4,
                    format_args!(
                        "trigger {i} draws from magazine {} of {}",
                        t.magazine,
                        own.magazines.len()
                    ),
                ));
            }
        }
        Ok(Weapon { object, item, own })
    }

    /// The magazine trigger `trigger` draws from (`None`: it needs none, or
    /// there is no such trigger).
    pub fn magazine_of(&self, trigger: usize) -> Option<&Magazine> {
        let i = self.own.triggers.get(trigger)?.magazine_index()?;
        self.own.magazines.get(i)
    }

    /// Ticks between shots of a held trigger (`Trigger::ticks_between_shots`
    /// with this weapon's age penalty), or `None` for a missing trigger.
    pub fn ticks_between_shots(&self, trigger: usize, rate_of_fire: f32, age: f32) -> Option<u32> {
        let t = self.own.triggers.get(trigger)?;
        Some(t.ticks_between_shots(rate_of_fire, age, self.own.age_rate_of_fire_penalty))
    }
}

/// The aim assist parameters at a magnification (section 15).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimAssist {
    /// Radians.
    pub autoaim_angle: f32,
    /// World units.
    pub autoaim_range: f32,
    /// Radians.
    pub magnetism_angle: f32,
    /// World units.
    pub magnetism_range: f32,
    /// Radians: max(deviation angle, autoaim angle) / m.
    pub deviation: f32,
}

impl AimAssist {
    /// The target falloff g(x, X): 1 up to X / 2, then linear to 0 at X.
    pub fn falloff(x: f32, max: f32) -> f32 {
        if x <= max / 2.0 {
            1.0
        } else if x < max {
            (max - x) / (max / 2.0)
        } else {
            0.0
        }
    }
}

impl WeaponPart {
    /// The magnification of zoom level `level` (0-based) of this weapon:
    /// m0 (m1 / m0)^(i / (n - 1)) for n > 1 levels, m0 for n = 1, each of
    /// m0 and m1 taken as 1 when not above 0. `None` for a weapon with no
    /// levels or a level out of range.
    pub fn magnification(&self, level: usize) -> Option<f32> {
        let n = self.magnification_levels;
        if n <= 0 || level >= n as usize {
            return None;
        }
        let fix = |m: f32| if m > 0.0 { m } else { 1.0 };
        let (m0, m1) = (fix(self.magnification_range.lower), fix(self.magnification_range.upper));
        Some(if n == 1 {
            m0
        } else {
            m0 * (m1 / m0).powf(level as f32 / (n - 1) as f32)
        })
    }

    /// The aim assist parameters when the player is zoomed by
    /// `magnification` (`None` unzoomed). `None` when the weapon has "aim
    /// assists only when zoomed" and the player is not zoomed.
    pub fn aim_assist(&self, magnification: Option<f32>) -> Option<AimAssist> {
        if magnification.is_none() && self.flags.contains(WeaponFlags::AIM_ASSISTS_ONLY_WHEN_ZOOMED) {
            return None;
        }
        let m = magnification.unwrap_or(1.0);
        Some(AimAssist {
            autoaim_angle: self.autoaim_angle / m,
            autoaim_range: self.autoaim_range * m,
            magnetism_angle: self.magnetism_angle / m,
            magnetism_range: self.magnetism_range * m,
            deviation: self.deviation_angle.max(self.autoaim_angle) / m,
        })
    }
}

/// The field of view while zoomed by `magnification`: the base field of
/// view divided by it (the angle itself, not its tangent), applied only
/// when it lies between pi/100 and 99 pi/100 (`None` otherwise).
pub fn zoomed_field_of_view(base: f32, magnification: f32) -> Option<f32> {
    let fov = base / magnification;
    (ZOOM_FOV_MIN..=ZOOM_FOV_MAX).contains(&fov).then_some(fov)
}

/// The per-tick rate the build stores for a time in seconds: 1 / (30 t)
/// for t > 0, 1 for t = 0.
pub fn per_tick_rate(seconds: f32) -> f32 {
    if seconds > 0.0 { 1.0 / (30.0 * seconds) } else { 1.0 }
}

/// A time in seconds as the engine's tick count: times 30 in single
/// precision, truncated toward zero.
pub fn seconds_to_ticks(seconds: f32) -> i32 {
    (seconds * 30.0) as i32
}

/// A tag id of a reference, for tests and sweeps: `None` for a null one.
pub fn ref_id(r: &TagRef) -> Option<TagId> {
    (!r.is_null()).then_some(r.id)
}
