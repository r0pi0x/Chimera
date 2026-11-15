//! A Halo weapon's numbers as its data document gives them (the
//! `halo_ce:weapon` hook's table in data/weapons/<name>.toml): the
//! weapon, magazine, trigger and projectile tag fields the simulation
//! reads, and the first-person animation frame counts that time the
//! draw, the reloads and the melee, in the game's own units (world units,
//! radians, 30 Hz ticks, per-tick rates; specs/halo_ce/weapons.md
//! "Multiplayer weapon numbers", projectiles.md "Flight numbers",
//! weapons.md 16 and animations.md "First-person graphs"). No number of
//! the spec is in Rust: every one is a field here, filled by the data.
//!
//! Also the derived rules the spec states as formulas: ticks between
//! shots (`Trigger::required_ticks`), the zoom's magnification
//! (`Gun::magnification`), a new weapon's rounds and what picking up a
//! duplicate weapon gives (multiplayer_rules.md 4.6 and 5.5).

use serde::Deserialize;

use super::super::vitality::{EffectId, ids};

/// The `halo_ce:weapon` hook's table.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Gun {
    /// The weapon tag's path on the disc (the heavy test reads it).
    pub tag: String,
    /// Weapon flags (part 0x000).
    pub flags: u32,
    /// The weapon type (0x1DA): 0 undefined, 1 shotgun, 3 plasma pistol,
    /// 4 plasma rifle.
    pub weapon_type: u16,
    /// Zoom levels (0x0D2) and the magnification range (0x0D4; given
    /// only for a weapon that zooms).
    pub zoom_levels: i16,
    #[serde(default)]
    pub magnification: Option<[f32; 2]>,
    pub heat: Heat,
    pub age: Age,
    pub aim_assist: AimAssist,
    /// Active camouflage ding and regrowth (0x1C4, 0x1C8).
    pub camo: [f32; 2],
    /// The holder's camera field of view, radians (the multiplayer
    /// Spartan's unit, objects.md: 70 degrees): a zoom divides it.
    pub camera_field_of_view: f32,
    /// The melee damage effect (0x08C) by our id (data/damage.toml).
    pub melee_damage: String,
    /// The magazine (none: the trigger needs none).
    #[serde(default)]
    pub magazine: Option<Magazine>,
    pub trigger: Trigger,
    pub animations: Animations,
    pub projectile: Projectile,
}

/// Weapon flag bits (weapons.md "Weapon flags").
pub mod weapon_flags {
    pub const AIM_ASSISTS_ONLY_WHEN_ZOOMED: u32 = 1 << 5;
    pub const PREVENTS_MELEE: u32 = 1 << 9;
    pub const CANNOT_FIRE_AT_MAXIMUM_AGE: u32 = 1 << 11;
}

/// Trigger flag bits (weapons.md "Trigger flags").
pub mod trigger_flags {
    pub const CAN_FIRE_WITH_PARTIAL_AMMO: u32 = 1 << 2;
    pub const DOES_NOT_REPEAT_AUTOMATICALLY: u32 = 1 << 3;
    pub const USE_ERROR_WHEN_UNZOOMED: u32 = 1 << 10;
    pub const PROJECTILES_HAVE_IDENTICAL_ERROR: u32 = 1 << 12;
}

/// Magazine flag bits.
pub mod magazine_flags {
    pub const WASTES_ROUNDS_WHEN_RELOADED: u32 = 1;
    pub const EVERY_ROUND_MUST_BE_CHAMBERED: u32 = 2;
}

/// Heat (weapon part 0x044 to 0x058).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Heat {
    pub recovery_threshold: f32,
    pub overheated_threshold: f32,
    pub detonation_threshold: f32,
    pub detonation_fraction: f32,
    /// Per second (per tick: x 1/30).
    pub loss_per_second: f32,
    pub illumination: f32,
}

/// The battery ("age", 0x138 to 0x144).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Age {
    pub heat_recovery_penalty: f32,
    pub rate_of_fire_penalty: f32,
    pub misfire_start: f32,
    pub misfire_chance: f32,
}

/// Aim assist (0x0DC to 0x0EC), radians and world units, and the switch
/// that turns it on (weapons.md 15: off by default, a controller
/// feature; not implemented beyond the data, see the module doc).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AimAssist {
    #[serde(default)]
    pub enabled: bool,
    pub autoaim_angle: f32,
    pub autoaim_range: f32,
    pub magnetism_angle: f32,
    pub magnetism_range: f32,
    pub deviation_angle: f32,
}

/// A magazine element (0x70).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Magazine {
    pub flags: u32,
    pub rounds_recharged: i16,
    pub rounds_total_initial: i16,
    pub rounds_total_maximum: i16,
    pub rounds_loaded_maximum: i16,
    pub rounds_reloaded: i16,
    /// Seconds (x 30, truncated).
    pub chamber_time: f32,
}

impl Magazine {
    /// A new weapon's rounds: (loaded, reserve) (weapons.md 1,
    /// multiplayer_rules.md 4.6 step 4).
    pub fn new_rounds(&self) -> (i16, i16) {
        let loaded = self.rounds_total_initial.min(self.rounds_loaded_maximum);
        (loaded, self.rounds_total_initial - loaded)
    }

    /// The chambering's length in ticks.
    pub fn chamber_ticks(&self) -> i32 {
        seconds_to_ticks(self.chamber_time)
    }

    pub fn wastes_rounds(&self) -> bool {
        self.flags & magazine_flags::WASTES_ROUNDS_WHEN_RELOADED != 0
    }

    pub fn chambers_every_round(&self) -> bool {
        self.flags & magazine_flags::EVERY_ROUND_MUST_BE_CHAMBERED != 0
    }
}

/// Ammunition taken from a weapon of the same tag on the ground
/// (multiplayer_rules.md 5.5): (held reserve, ground reserve, the ground
/// weapon is deleted). Nothing is taken when the held reserve is full.
pub fn take_ammunition(held: i16, maximum: i16, ground: i16) -> (i16, i16, bool) {
    let take = ground.min((maximum - held).max(0));
    if take <= 0 {
        return (held, ground, false);
    }
    (held + take, ground - take, ground - take <= 0)
}

/// A trigger element (0x114): the fields the four weapons' rules read.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Trigger {
    pub flags: u32,
    /// Rounds per second at rate of fire 0 and 1.
    pub rounds_per_second: [f32; 2],
    /// The runtime per-tick rates (0x0F8, 0x0FC, 0x100, 0x104).
    pub rate_of_fire_acceleration: f32,
    pub rate_of_fire_deceleration: f32,
    pub error_acceleration: f32,
    pub error_deceleration: f32,
    /// The magazine index (-1: none).
    pub magazine: i16,
    pub rounds_per_shot: i16,
    pub minimum_rounds_loaded: i16,
    pub rounds_between_tracers: i16,
    pub minimum_error: f32,
    /// Error angle at error 0 and 1, radians.
    pub error_angle: [f32; 2],
    pub projectiles_per_shot: i16,
    /// 0 point, 1 horizontal fan (and its step: none uses a fan).
    pub distribution: u16,
    #[serde(default)]
    pub distribution_angle: f32,
    /// Forward, left, up, world units.
    pub first_person_offset: [f32; 3],
    pub heat_per_round: f32,
    pub age_per_round: f32,
    /// The firing effect entries (their count; every shot count 1 - 1).
    pub firing_effects: u8,
    /// The projectile tag's path.
    pub projectile: String,
}

impl Trigger {
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// The rounds per second at rate of fire `f` (weapons.md 4).
    pub fn rate(&self, f: f32) -> f32 {
        self.rounds_per_second[0] + (self.rounds_per_second[1] - self.rounds_per_second[0]) * f
    }

    /// The ticks a trigger must have been idle (plus one) to fire again,
    /// in single precision (weapons.md 4, "Can fire again").
    pub fn required_ticks(&self, f: f32, age: f32, age_penalty: f32) -> f32 {
        let rate = self.rate(f);
        let mut required = if rate > ROF_EPSILON { 30.0 / rate } else { 0.0 };
        if age_penalty > 0.0 {
            required *= 1.0 + age * age_penalty;
        }
        required
    }

    /// The error angle limit at error `e`.
    pub fn theta_max(&self, e: f32) -> f32 {
        (1.0 - e) * self.error_angle[0] + e * self.error_angle[1]
    }
}

/// A rate at or below this means "no limit" (weapons.md, "Constants").
pub const ROF_EPSILON: f32 = 0.0001;

/// First-person animation frame counts that time gameplay (weapons.md 16,
/// animations.md "Gameplay events driven by animations"), and the weapon's
/// own world animations (weapons.md 1).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Animations {
    /// `ready`: the draw.
    pub ready: i32,
    /// `reload-empty`: every reload (none: no reload).
    #[serde(default)]
    pub reload_empty: Option<i32>,
    /// `enter`: the shotgun's first shell.
    #[serde(default)]
    pub enter: Option<i32>,
    /// `melee` frames and key frame.
    pub melee: i32,
    pub melee_key: i32,
    /// The weapon's own graph: `fire-1` (primary recoil) and `chamber-1`
    /// (primary chamber) frame counts, where it has them.
    #[serde(default)]
    pub world_recoil: Option<i32>,
    #[serde(default)]
    pub world_chamber: Option<i32>,
}

/// The projectile a trigger fires (`proj`, projectiles.md "Flight
/// numbers"): speeds in WU per tick, lengths in WU.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Projectile {
    pub tag: String,
    pub initial_velocity: f32,
    pub final_velocity: f32,
    pub air_damage_range: [f32; 2],
    pub water_damage_range: [f32; 2],
    /// 0: none.
    pub maximum_range: f32,
    pub air_gravity_scale: f32,
    pub water_gravity_scale: f32,
    pub collision_radius: f32,
    pub minimum_velocity: f32,
    /// Seconds (both 0: no timer).
    pub timer: [f32; 2],
    /// The impact damage effect by our id.
    pub impact_damage: String,
    /// What it does on a character's cyborg body (material type 21) and
    /// shield (22), and on the level (projectiles.md "Material
    /// responses"): `disappear` or `overpenetrate`.
    pub on_body: Response,
    pub on_shield: Response,
    pub on_level: Response,
}

/// A material response the four projectiles' rows use.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields, rename_all = "snake_case", tag = "response")]
pub enum Response {
    Disappear,
    /// Pass through, keeping (1 - initial friction) of the speed.
    Overpenetrate { initial_friction: f32 },
}

/// The engine's gravity, WU per tick squared (projectiles.md, constants).
pub const GRAVITY: f32 = 0.0035651792;

impl Projectile {
    /// The deceleration a in WU per tick squared for a damage range
    /// window (projectiles.md 3).
    pub fn deceleration(&self, window: [f32; 2]) -> f32 {
        let (v0, vf) = (self.initial_velocity, self.final_velocity);
        if v0 != vf && window[1] != window[0] {
            (v0 * v0 - vf * vf) / (2.0 * (window[1] - window[0]))
        } else {
            0.0
        }
    }

    /// The deceleration timer's start and step for a window (lower / v0,
    /// the disc's executable's quirk).
    pub fn deceleration_timer(&self, window: [f32; 2]) -> (f32, f32) {
        if window[0] > 0.0 { (0.0, window[0] / self.initial_velocity) } else { (1.0, 0.0) }
    }

    /// The direct hit's scale for a speed at impact (projectiles.md 8).
    pub fn direct_hit_scale(&self, speed: f32) -> f32 {
        let (v0, vf) = (self.initial_velocity, self.final_velocity);
        if v0 != vf { ((speed - vf) / (v0 - vf)).clamp(0.0, 1.0) } else { 1.0 }
    }

    pub fn has_timer(&self) -> bool {
        self.timer[1] != 0.0
    }
}

impl Gun {
    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }

    /// The magnification of zoom level `i` (weapons.md 12).
    pub fn magnification(&self, level: i16) -> f32 {
        let [m0, m1] = self.magnification.unwrap_or([1.0, 1.0]).map(|m| if m > 0.0 { m } else { 1.0 });
        let n = self.zoom_levels;
        if n > 1 { m0 * (m1 / m0).powf(level as f32 / (n - 1) as f32) } else { m0 }
    }

    /// The melee damage effect's id.
    pub fn melee_effect(&self) -> Option<EffectId> {
        effect_id(&self.melee_damage)
    }

    /// The melee's length (the counter's start, F - floor(F / 4)) and the
    /// counter value on its hit tick (F - K - floor(F / 4)) (weapons.md 13).
    pub fn melee_timing(&self) -> (i32, i32) {
        let f = self.animations.melee;
        let s = f / 4;
        (f - s, f - self.animations.melee_key - s)
    }
}

/// Our id of a damage effect named in data (`vitality::ids::ALL`).
pub fn effect_id(name: &str) -> Option<EffectId> {
    ids::ALL.into_iter().find(|e| e.0 == name)
}

/// Seconds to ticks: x 30 in single precision, truncated (weapons.md,
/// "Units and conventions").
pub fn seconds_to_ticks(s: f32) -> i32 {
    (s * 30.0) as i32
}
