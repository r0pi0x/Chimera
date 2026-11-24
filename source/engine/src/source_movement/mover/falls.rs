//! Landings: the landing's speed and sound, fall damage's inputs and the punch
//! roll's spring (`punch_spring`, also for games' own punch angles).

use bevy::prelude::*;

use crate::source_movement::mover::{Mover, PUNCH_DAMPING, PUNCH_SPRING};

impl Mover<'_, '_, '_, '_> {
    /// The fall speed a landing counts (movement.md "Falling"; specs/
    /// hl2dm/movement.md "Falling and landing"): from `fall_punch` up and
    /// out of water, less `floating_bonus` on a floating body, plus the
    /// ground's own speed when it is going down (at least 0.1).
    pub(crate) fn landing_speed(&self, fall: f32) -> f32 {
        if fall < self.cfg.fall_punch || self.me.water_level > 0 {
            return fall;
        }
        let mut fall = fall;
        if self.ground_collider.or(self.me.ground_entity).is_some_and(|c| (self.floating)(c)) {
            fall -= self.cfg.floating_bonus;
        }
        if let Some(g) = self.me.ground_entity {
            let vz = (self.mover_velocity)(g).z;
            if vz < 0.0 {
                fall = (fall + vz).max(0.1);
            }
        }
        fall
    }

    /// Landing (sound spec 3): fall speed tiers, of the speed the landing
    /// counts (`landing_speed`) once the real one reaches `fall_punch`.
    pub(crate) fn landing_sound(&mut self, raw: f32, fall: f32) {
        if raw < self.cfg.fall_punch {
            return;
        }
        let volume = if self.me.water_level > 0 {
            0.5
        } else if fall > self.cfg.fall_loud.unwrap_or(self.cfg.fall_safe) {
            1.0
        } else if fall > self.cfg.fall_half_safe {
            0.85
        } else if fall < self.cfg.fall_bounce {
            0.0
        } else {
            0.5
        };
        if self.cfg.loud_landings_only && volume < 1.0 {
            return;
        }
        self.impact_step(volume);
    }

    /// The punch angle's spring (movement spec, "Punch angle decay"), for
    /// its roll: the only part movement sets (recoil is the weapons').
    pub(crate) fn decay_punch(&mut self) {
        let (p, v) = (self.me.punch_roll, self.me.punch_roll_vel);
        if p * p <= 0.001 && v * v <= 0.001 {
            self.me.punch_roll = 0.0;
            self.me.punch_roll_vel = 0.0;
            return;
        }
        let dt = self.dt;
        let p = p + v * dt;
        let v = v * (1.0 - PUNCH_DAMPING * dt).max(0.0);
        let v = v - p * (PUNCH_SPRING * dt).clamp(0.0, 2.0);
        self.me.punch_roll = p.clamp(-89.0, 89.0);
        self.me.punch_roll_vel = v;
    }
}

/// One tick of Source's shared punch-angle spring (the movement's punch
/// decay: the angle moves by its velocity, the velocity is damped by
/// `PUNCH_DAMPING` and pulled back by `PUNCH_SPRING`; both zeroed once
/// small): angle `p` and velocity `v` in degrees (and per second). For
/// games whose weapons kick the punch (TF2's `PunchAngle`).
pub fn punch_spring(p: Vec3, v: Vec3, dt: f32) -> (Vec3, Vec3) {
    if p.length_squared() <= 0.001 && v.length_squared() <= 0.001 {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let p = (p + v * dt).clamp(Vec3::splat(-89.0), Vec3::splat(89.0));
    let v = v * (1.0 - PUNCH_DAMPING * dt).max(0.0);
    let v = v - p * (PUNCH_SPRING * dt).clamp(0.0, 2.0);
    (p, v)
}
