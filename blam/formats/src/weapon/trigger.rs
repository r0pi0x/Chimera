//! The weapon's magazines and triggers (specs/halo_ce/weapons.md, "Magazine
//! element", "Trigger element"): the 0x70-byte magazine with its
//! ammunition objects, the 0x114-byte trigger with its firing effects, and
//! the rate of fire rule on the trigger's fields.

use super::ROF_EPSILON;
use crate::cache::{Bounds, TagRef};
use crate::object::bit_flags;
use crate::scenario::coded_enum;

bit_flags! {
    /// A magazine's flags.
    pub struct MagazineFlags(u32) {
        /// Loaded rounds are thrown away when the magazine is reloaded.
        WASTES_ROUNDS_WHEN_RELOADED = 0,
        EVERY_ROUND_MUST_BE_CHAMBERED = 1,
    }
}

bit_flags! {
    /// A trigger's flags.
    pub struct TriggerFlags(u32) {
        TRACKS_FIRED_PROJECTILE = 0,
        RANDOM_FIRING_EFFECTS = 1,
        CAN_FIRE_WITH_PARTIAL_AMMO = 2,
        /// Semi-automatic for a player.
        DOES_NOT_REPEAT_AUTOMATICALLY = 3,
        LOCKS_IN_ON_OFF_STATE = 4,
        PROJECTILES_USE_WEAPON_ORIGIN = 5,
        STICKS_WHEN_DROPPED = 6,
        EJECTS_DURING_CHAMBER = 7,
        DISCHARGING_SPEWS = 8,
        ANALOG_RATE_OF_FIRE = 9,
        USE_ERROR_WHEN_UNZOOMED = 10,
        PROJECTILE_VECTOR_CANNOT_BE_ADJUSTED = 11,
        PROJECTILES_HAVE_IDENTICAL_ERROR = 12,
        PROJECTILE_IS_CLIENT_SIDE_ONLY = 13,
    }
}

coded_enum! {
    /// How loud the weapon's firing is for AI hearing.
    pub enum FiringNoise {
        Silent = 0,
        Medium = 1,
        Loud = 2,
    }
}

coded_enum! {
    /// What happens after the charged time.
    pub enum OverchargedAction {
        None = 0,
        Explodes = 1,
        Discharges = 2,
    }
}

coded_enum! {
    /// How a shot's projectiles are spread.
    pub enum Distribution {
        Point = 0,
        HorizontalFan = 1,
    }
}

crate::tag_struct! {
    /// An ammunition object (element 0x1C): equipment that refills a
    /// magazine.
    pub struct AmmunitionObject: 0x1C {
        /// Rounds one pickup gives.
        0x00 rounds: i16,
        /// Class `eqip`.
        0x0C equipment: TagRef,
    }
}

crate::tag_struct! {
    /// A magazine (element 0x70).
    pub struct Magazine: 0x70 {
        0x00 flags: MagazineFlags,
        /// Rounds per second added to the loaded rounds.
        0x04 rounds_recharged: i16,
        /// A new weapon's rounds (loaded plus reserve).
        0x06 rounds_total_initial: i16,
        /// The reserve cap.
        0x08 rounds_total_maximum: i16,
        /// The magazine size.
        0x0A rounds_loaded_maximum: i16,
        /// Seconds; unused (the reload length is the first-person
        /// animation).
        0x14 reload_time: f32,
        /// Rounds one reload moves into the magazine.
        0x18 rounds_reloaded: i16,
        /// Seconds, x 30 truncated.
        0x1C chamber_time: f32,
        /// `effe` or `snd!`: at reload start.
        0x38 reloading_effect: TagRef,
        /// `effe` or `snd!`: at chamber start.
        0x48 chambering_effect: TagRef,
        0x64 magazines: Vec<AmmunitionObject>,
    }
}

crate::tag_struct! {
    /// A firing effect (element 0x84).
    pub struct FiringEffect: 0x84 {
        /// Shots (rounds) this entry is used for: lower and upper.
        0x00 shot_count: Bounds<i16>,
        /// `effe` or `snd!`.
        0x24 firing_effect: TagRef,
        0x34 misfire_effect: TagRef,
        0x44 empty_effect: TagRef,
        /// `jpt!`: applied to the shooter.
        0x54 firing_damage: TagRef,
        0x64 misfire_damage: TagRef,
        0x74 empty_damage: TagRef,
    }
}

crate::tag_struct! {
    /// A trigger (element 0x114).
    pub struct Trigger: 0x114 {
        0x000 flags: TriggerFlags,
        /// Rounds per second at rate of fire 0 and at 1.
        0x004 rounds_per_second: Bounds<f32>,
        /// Seconds: rate of fire 0 to 1 while held.
        0x00C acceleration_time: f32,
        /// Seconds: 1 to 0 when released.
        0x010 deceleration_time: f32,
        /// Above this rate of fire the weapon shows its blur permutation.
        0x014 blurred_rate_of_fire: f32,
        /// The magazine this trigger draws from (NONE, -1: needs none);
        /// `magazine_index` gives it as an `Option`.
        0x020 magazine: i16,
        0x022 rounds_per_shot: i16,
        /// A fresh pull needs at least this many loaded.
        0x024 minimum_rounds_loaded: i16,
        0x026 rounds_between_tracers: i16,
        0x02E firing_noise: FiringNoise,
        /// Unused by the reference.
        0x030 error: Bounds<f32>,
        /// Seconds: error 0 to 1 while held.
        0x038 error_acceleration_time: f32,
        /// Seconds: error 1 to 0 when released.
        0x03C error_deceleration_time: f32,
        /// Seconds, x 30 truncated: ticks to full charge.
        0x048 charging_time: f32,
        /// Seconds, x 30 truncated: ticks a full charge can be held.
        0x04C charged_time: f32,
        0x050 overcharged_action: OverchargedAction,
        /// Glow while charging, x the charged fraction.
        0x054 charged_illumination: f32,
        /// Seconds: the discharge length after a charge (0: a single
        /// release).
        0x058 spew_time: f32,
        /// `effe` or `snd!`.
        0x05C charging_effect: TagRef,
        0x06C distribution_function: Distribution,
        0x06E projectiles_per_shot: i16,
        /// Radians: the fan step.
        0x070 distribution_angle: f32,
        /// Radians: the lower limit of the random angle.
        0x078 minimum_error: f32,
        /// Radians: the angle limit at error 0 and at error 1.
        0x07C error_angle: Bounds<f32>,
        /// World units: forward, left, up from the shot origin (players
        /// only).
        0x084 first_person_offset: [f32; 3],
        /// Class `proj` (any object): what one round creates.
        0x094 projectile: TagRef,
        /// Seconds.
        0x0A4 ejection_port_recovery_time: f32,
        /// Seconds: the muzzle glow fade.
        0x0A8 illumination_recovery_time: f32,
        /// Fraction of heat per round.
        0x0B8 heat_generated_per_round: f32,
        /// Fraction of battery age per round.
        0x0BC age_generated_per_round: f32,
        /// Seconds, x 30 truncated.
        0x0C4 overload_time: f32,
        /// Runtime, per tick: 1 / (30 x the illumination recovery time),
        /// 1 if 0.
        0x0F0 runtime_illumination_recovery: f32,
        /// Runtime, per tick, from the ejection port recovery time.
        0x0F4 runtime_ejection_port_recovery: f32,
        /// Runtime, per tick, from the acceleration time.
        0x0F8 runtime_rate_of_fire_acceleration: f32,
        /// Runtime, per tick, from the deceleration time.
        0x0FC runtime_rate_of_fire_deceleration: f32,
        /// Runtime, per tick, from the error acceleration time.
        0x100 runtime_error_acceleration: f32,
        /// Runtime, per tick, from the error deceleration time.
        0x104 runtime_error_deceleration: f32,
        0x108 firing_effects: Vec<FiringEffect>,
    }
}

impl Trigger {
    /// The magazine index, `None` for NONE.
    pub fn magazine_index(&self) -> Option<usize> {
        (self.magazine >= 0).then_some(self.magazine as usize)
    }

    /// The rounds per second at rate of fire `f` (0 to 1, or the analog
    /// pull): lower + (upper - lower) f.
    pub fn rate(&self, rate_of_fire: f32) -> f32 {
        let r = self.rounds_per_second;
        r.lower + (r.upper - r.lower) * rate_of_fire
    }

    /// The idle ticks a trigger needs before it may fire again: 30 / rate
    /// (0 at or below 0.0001 rounds per second), times (1 + age x
    /// `age_penalty`) when the weapon's age rate of fire penalty is above
    /// 0. In single precision, as the engine computes it.
    pub fn required_ticks(&self, rate_of_fire: f32, age: f32, age_penalty: f32) -> f32 {
        let rate = self.rate(rate_of_fire);
        let mut required = if rate > ROF_EPSILON { 30.0 / rate } else { 0.0 };
        if age_penalty > 0.0 {
            required *= 1.0 + age * age_penalty;
        }
        required
    }

    /// The ticks between the shots of a held trigger: max(1,
    /// ceil(required)) (the idle counter is 0 on the tick after a shot and
    /// rises by one a tick, and it fires when counter + 1 reaches the
    /// required ticks). A trigger with no rate limit gives 1; a plasma
    /// pistol's "press-limited" trigger is not shown by it.
    pub fn ticks_between_shots(&self, rate_of_fire: f32, age: f32, age_penalty: f32) -> u32 {
        (self.required_ticks(rate_of_fire, age, age_penalty).ceil() as u32).max(1)
    }
}
