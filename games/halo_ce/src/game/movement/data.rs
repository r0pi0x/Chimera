//! Halo's movement numbers (`MovementData`): data/movement.toml, compiled
//! in and parsed once at startup, in the spec's own units (world units, the
//! 30 Hz tick, seconds; specs/halo_ce/movement.md, "Constants"). The
//! per-tick values the rules use (speeds and accelerations divided by 30,
//! landing velocities and times in ticks) are derived here, not stored.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// data/movement.toml, built in.
pub const TABLE: chimera_core::game_data::GameData = chimera_core::game_data!("data/movement.toml");

/// Ticks per second of Halo's simulation (cache_file.md, "The world"): the
/// unit the per-second fields are divided by. Structural, not a tuning.
pub const TICKS_PER_SECOND: f32 = 30.0;

/// The movement's numbers (data/movement.toml; each field names the spec's
/// constant there).
#[derive(Resource, Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MovementData {
    pub gravity: f32,
    pub run_forward: f32,
    pub run_backward: f32,
    pub run_sideways: f32,
    pub run_accel: f32,
    pub sneak_forward: f32,
    pub sneak_backward: f32,
    pub sneak_sideways: f32,
    pub sneak_accel: f32,
    pub air_accel: f32,
    pub stun_move: f32,
    pub stun_turn: f32,
    pub hard_land_decel: f32,
    pub ground_push: f32,
    pub throttle_dead: f32,
    pub crouch_stick: f32,
    pub jump_velocity: f32,
    pub jump_timer_above: f32,
    pub jump_timer_max: f32,
    pub height_stand: f32,
    pub height_crouch: f32,
    pub radius: f32,
    pub ride_step: f32,
    /// Ours, not in the table: the step-up (WU) the pill climbs on a map
    /// whose envelope steps higher than Halo's `ride_step`
    /// (`movement::body`, `mover::step_up`). 0: none (the spec's mover).
    #[serde(default)]
    pub step_up: f32,
    pub camera_stand: f32,
    pub camera_crouch: f32,
    pub crouch_rate: f32,
    pub crouch_lift_min: f32,
    pub max_slope_cos: f32,
    pub uphill_falloff_sin: f32,
    pub uphill_cutoff_sin: f32,
    pub uphill_scale: f32,
    pub downhill_falloff_sin: f32,
    pub downhill_cutoff_sin: f32,
    pub downhill_scale: f32,
    pub soft_land_min: f32,
    pub hard_land_min: f32,
    pub hard_land_max: f32,
    pub soft_land_time: f32,
    pub hard_land_time: f32,
    pub fall_v_min: f32,
    pub fall_v_max: f32,
    pub fall_v_kill: f32,
    pub epsilon: f32,
    pub max_contacts: u32,
}

impl MovementData {
    /// The built-in table (panics only if data/movement.toml is broken; a
    /// test parses it).
    pub fn builtin() -> Self {
        toml::from_str(&TABLE).expect("data/movement.toml parses")
    }

    /// The pill's segment height h at crouch fraction `c` (spec "The
    /// pill"): 0.3 WU standing, 0.1 crouched.
    pub fn segment(&self, c: f32) -> f32 {
        self.height_stand + (self.height_crouch - self.height_stand) * c - 2.0 * self.radius
    }

    /// The eye's height above the origin at crouch fraction `c` (spec
    /// section 7).
    pub fn eye(&self, c: f32) -> f32 {
        (1.0 - c) * self.camera_stand + c * self.camera_crouch
    }

    /// The pill's centre above the origin at crouch fraction `c`: the
    /// character's `Transform` is the pill's centre.
    pub fn centre_above_origin(&self, c: f32) -> f32 {
        self.radius + self.segment(c) / 2.0
    }

    /// The speed scale of a path climbing with sine `k` (spec section 2,
    /// "Slope speed scale").
    pub fn slope_scale(&self, k: f32) -> f32 {
        if k <= self.downhill_cutoff_sin {
            self.downhill_scale
        } else if k < self.downhill_falloff_sin {
            1.0 + (k - self.downhill_falloff_sin) * (self.downhill_scale - 1.0)
                / (self.downhill_cutoff_sin - self.downhill_falloff_sin)
        } else if k >= self.uphill_cutoff_sin {
            self.uphill_scale
        } else if k > self.uphill_falloff_sin {
            1.0 + (k - self.uphill_falloff_sin) * (self.uphill_scale - 1.0)
                / (self.uphill_cutoff_sin - self.uphill_falloff_sin)
        } else {
            1.0
        }
    }

    /// The landing a landing velocity `lambda` (WU/t) starts, if any (spec
    /// section 6): hard or soft, and its length in ticks.
    pub fn landing(&self, lambda: f32) -> Option<(bool, f32)> {
        let v_s = self.soft_land_min / TICKS_PER_SECOND;
        let v_h = self.hard_land_min / TICKS_PER_SECOND;
        let v_m = self.hard_land_max / TICKS_PER_SECOND;
        if lambda < v_s {
            None
        } else if lambda < v_h {
            let q = ((lambda - v_s) / (v_h - v_s)).clamp(0.0, 1.0);
            Some((false, (self.soft_land_time * TICKS_PER_SECOND * q).trunc()))
        } else {
            // Quirks: measured from zero, not from v_h, so q is always 1.
            let q = (lambda / (v_m - v_h)).clamp(0.0, 1.0);
            Some((true, (self.hard_land_time * TICKS_PER_SECOND * q).trunc()))
        }
    }

    /// The falling damage's scale q_f of a landing at `lambda` (WU/t), if
    /// it hurts (spec section 6, step 1): the damage effect `globals\falling`
    /// is then dealt at that scale.
    pub fn fall_scale(&self, lambda: f32) -> Option<f32> {
        (lambda > self.fall_v_min).then(|| ((lambda - self.fall_v_min) / (self.fall_v_max - self.fall_v_min)).clamp(0.0, 1.0))
    }
}
