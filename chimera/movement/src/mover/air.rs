//! In the air: air acceleration with the 30-unit wish cap, the air move, air
//! jumps (TF2's Scout).

use bevy::prelude::*;

use crate::mover::{AIR_WISH_CAP, Mover};

impl Mover<'_, '_, '_, '_> {
    #[inline]
    pub(crate) fn air_accelerate(&mut self, dir: Vec3, wish_speed: f32, accel: f32) {
        let add = wish_speed.min(AIR_WISH_CAP) - self.v.dot(dir);
        if add <= 0.0 {
            return;
        }
        // A push's air current (`core::ExternalPush::air_control`).
        let amount = (accel * self.push.air_share() * wish_speed * self.dt * self.me.surface_friction).min(add);
        self.v += dir * amount;
    }

    pub(crate) fn air(&mut self, f: f32, s: f32, forward: Vec3, right: Vec3, max_speed: f32) {
        let (dir, speed) = self.wish(f, s, forward, right, max_speed);
        self.wish = dir * speed;
        self.air_accelerate(dir, speed, self.cfg.airaccelerate);
        let mut base = Vec3::new(self.base.x, self.base.y, 0.0);
        if self.cfg.liquid_hazards {
            base += Vec3::new(self.current.x, self.current.y, 0.0);
        }
        self.v += base;
        self.slide();
        if self.cfg.liquid_hazards {
            self.water_check();
            base += Vec3::new(self.current.x, self.current.y, 0.0);
        }
        self.v -= base;
    }

    /// An air jump (`QuakeConfig::air_jumps`; TF2's Scout,
    /// specs/tf2/movement.md "The Scout's air jump"): the vertical speed
    /// replaced by `air_jump_speed` (after this tick's first gravity
    /// half-step, with no second), the horizontal velocity by the keys'
    /// wish velocity (none held: stopped).
    pub(crate) fn air_jump(&mut self, f: f32, s: f32, forward: Vec3, right: Vec3, max_speed: f32) {
        let (dir, speed) = self.wish(f, s, forward, right, max_speed);
        let wish = dir * speed;
        self.v = Vec3::new(wish.x, wish.y, self.cfg.air_jump_speed);
        self.me.air_jumps_used += 1;
        self.me.air_ducks = 0;
        self.me.jump_held = true;
    }
}
