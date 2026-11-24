//! The jump: its impulse, the held-jump rule, stamina and the bunny-hop cap.


use crate::mover::Mover;

impl Mover<'_, '_, '_, '_> {
    pub(crate) fn jump(&mut self) {
        // Without sv_enablebunnyhopping, jumping caps the speed: at a fixed
        // speed (CS:S) or a share of the player's max speed P (TF2).
        let cap = if self.cfg.bunnyhop_cap_scale > 0.0 {
            // P: the player's own max speed (not M: no surface or ceiling).
            let p = self.speed.unwrap_or(self.player_maxspeed);
            self.cfg.bunnyhop_cap_scale * p
        } else {
            self.cfg.bunnyhop_cap
        };
        if !self.cfg.enable_bunnyhopping && cap > 0.0 {
            let speed = self.v.length();
            if speed > cap {
                self.v *= cap / speed;
            }
        }
        let impulse = self.cfg.jump_impulse * self.ground_factors().1;
        self.set_ground(false, None);
        if self.me.ducked || self.me.ducking {
            self.v.z = impulse;
        } else {
            self.v.z += impulse;
        }
        if self.me.stamina > 0.0 {
            self.v.z *= 1.0 - self.cfg.stamina_jump_scale * self.me.stamina;
        }
        self.me.stamina = self.cfg.jump_stamina;
        self.half_gravity();
        self.me.jump_held = true;
        // A full-volume step on the surface jumped from.
        if self.cfg.model_footsteps {
            self.model_step(1.0);
        } else if let Some(s) = self.me.surface.clone() {
            self.play_step(&s, 1.0);
        }
    }
}
