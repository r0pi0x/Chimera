//! Water: the water level, swimming, water jumps and the splash sounds.

use bevy::prelude::*;

use crate::{
    core::Intent,
    mover::{
        Mover, SWIM_SINK, SWIM_WISH_SCALE, WALKABLE_NORMAL_Z, WATER_FEET_PROBE, WATER_JUMP_DROP, WATER_JUMP_EYE_EXTRA,
        WATER_JUMP_MIN_VZ, WATER_JUMP_PUSH, WATER_JUMP_REACH, WATER_JUMP_TIME, WATER_JUMP_UP,
    },
};

impl Mover<'_, '_, '_, '_> {
    /// Water level from three points at the box's centre line: 1 unit
    /// above the feet, mid-box, and the eye.
    pub(crate) fn water_check(&mut self) {
        let height = if self.me.ducked {
            self.trace.heights.1
        } else {
            self.trace.heights.0
        };
        let feet = self.feet;
        let points = [WATER_FEET_PROBE, height / 2.0, self.me.eye].map(|z| feet + Vec3::Z * z);
        // Contents per test point are cached for the tick: a point within
        // 1 unit of the last one tested for that slot reuses its answer.
        let mut test = |slot: usize| -> Option<bool> {
            if let Some((p, answer)) = self.water_cache[slot]
                && p.distance(points[slot]) < 1.0
            {
                return answer;
            }
            let answer = self.trace.point_water(points[slot]);
            self.water_cache[slot] = Some((points[slot], answer));
            answer
        };
        self.me.water_level = 0;
        let Some(slime) = test(0) else {
            return;
        };
        self.me.in_slime = slime;
        self.me.water_level = 1;
        if test(1).is_some() {
            self.me.water_level = 2;
            if test(2).is_some() {
                self.me.water_level = 3;
            }
        }
    }

    /// Climbing out of water at a ledge (only at waist depth).
    pub(crate) fn water_jump_check(&mut self, intent: &Intent) {
        if self.me.water_jump_time > 0.0 || self.v.z < WATER_JUMP_MIN_VZ {
            return;
        }
        let yaw = std::f32::consts::FRAC_PI_2 + intent.yaw;
        let flat = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
        let horizontal = self.v.with_z(0.0);
        if horizontal.length_squared() > 0.0 && horizontal.normalize().dot(flat) < 0.0 {
            return;
        }
        let height = if self.me.ducked {
            self.trace.heights.1
        } else {
            self.trace.heights.0
        };
        let from = self.feet + Vec3::Z * (height / 2.0);
        let wall = self.trace.sweep(self.me.ducked, from, from + flat * WATER_JUMP_REACH);
        if !wall.hit() {
            return;
        }
        let high = self.feet + Vec3::Z * (self.me.eye + WATER_JUMP_EYE_EXTRA);
        let over = self.trace.sweep(self.me.ducked, high, high + flat * WATER_JUMP_REACH);
        if over.hit() {
            return;
        }
        let down = self
            .trace
            .sweep(self.me.ducked, over.end, over.end - Vec3::Z * WATER_JUMP_DROP);
        if !down.hit() || down.normal.z < WALKABLE_NORMAL_Z {
            return;
        }
        self.v.z = WATER_JUMP_UP;
        self.me.water_jump_vel = -wall.normal * WATER_JUMP_PUSH;
        self.me.jump_held = true;
        self.me.water_jump_time = WATER_JUMP_TIME;
    }

    /// Swimming (waist deep or more): 3D wish, proportional friction, no
    /// gravity; holding jump rises.
    pub(crate) fn swim(&mut self, intent: &Intent, f: f32, s: f32, max_speed: f32) {
        let (forward, right) = Self::view(intent);
        let mut wish = forward * f + right * s;
        let lift = if self.cfg.swim_lift_max_speed {
            self.speed.unwrap_or(self.player_maxspeed)
        } else {
            self.cfg.client_maxspeed
        };
        if intent.jump {
            wish.z += lift;
        } else if f == 0.0 && s == 0.0 {
            wish.z -= SWIM_SINK;
        } else {
            wish.z += (2.0 * f * forward.z).clamp(0.0, lift);
        }
        let wish_speed = wish.length().min(max_speed) * SWIM_WISH_SCALE;
        let dir = wish.normalize_or_zero();
        let speed = self.v.length();
        let mut new_speed = speed - self.dt * speed * self.cfg.friction * self.me.surface_friction;
        if new_speed < 0.1 {
            new_speed = 0.0;
        }
        self.v = if speed > 0.0 {
            self.v * (new_speed / speed)
        } else {
            Vec3::ZERO
        };
        if wish_speed >= 0.1 {
            let add = wish_speed - new_speed;
            if add > 0.0 {
                let amount = (self.cfg.accelerate * wish_speed * self.dt * self.me.surface_friction).min(add);
                self.v += dir * amount;
            }
        }
        let dest = self.feet + self.v * self.dt;
        let tr = self.trace.sweep(self.me.ducked, self.feet, dest);
        if !tr.hit() {
            // Ride up onto steps while swimming.
            let top = dest + Vec3::Z * (self.cfg.stepsize + 1.0);
            let settle = self.trace.sweep(self.me.ducked, top, dest);
            if settle.start_solid {
                self.slide();
            } else {
                self.feet = settle.end;
            }
        } else if self.me.on_ground {
            self.step_move();
        } else {
            self.slide();
        }
    }

    /// Entering or leaving water this tick splashes.
    #[inline]
    pub(crate) fn water_sound(&mut self, before: u8) {
        if (before == 0) != (self.me.water_level == 0) {
            self.sounds.push((self.cfg.swim_sound.into(), self.feet, None));
        }
    }
}
