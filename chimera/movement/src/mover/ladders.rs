//! Ladder brushes (contents ladders, ladder zones, other games' ladder lines
//! as posts): the probe, the climb and the jump off. Ladder lines climbed as
//! lines are `useable_ladders`.

use bevy::prelude::*;

use crate::{
    core::Intent,
    mover::{LADDER_BACK_OFF, LADDER_JUMP_OFF, LADDER_MAX_NORMAL_Z, LADDER_REACH, LADDER_SPEED, Mover},
};

impl Mover<'_, '_, '_, '_> {
    /// Ladder detection (after ducking, before the move): probe 2 units
    /// toward the ladder (along the last normal, or the 3D input direction)
    /// and, on a ladder, set this tick's velocity.
    pub(crate) fn ladder(&mut self, intent: &Intent, f: f32, s: f32, duck_scale: f32) {
        let (forward, right) = Self::view(intent);
        let dir = match self.me.ladder {
            Some(n) => -n,
            None => {
                if f == 0.0 && s == 0.0 {
                    return;
                }
                (forward * f + right * s).normalize_or_zero()
            }
        };
        let tr = self
            .trace
            .sweep(self.me.ducked, self.feet, self.feet + dir * LADDER_REACH);
        // `CHIMERA_SLIDE_DEBUG=1`: the ladder probe too.
        static DEBUG: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *DEBUG.get_or_init(|| chimera_core::env::var("CHIMERA_SLIDE_DEBUG").is_ok()) {
            eprintln!(
                "    ladder probe: feet {:.3?} dir {:.3?} frac {:.5} normal {:.4?} ladder {} start_solid {}",
                self.feet.to_array(),
                dir.to_array(),
                tr.fraction,
                tr.normal.to_array(),
                tr.ladder,
                tr.start_solid
            );
        }
        // A ladder brush's top is a floor (standing on it, looking down
        // while walking must not grab it: the ladder velocity along a
        // vertical normal leaves only sideways movement, and the jump-off
        // push would add to a jump).
        if !tr.hit() || !tr.ladder || tr.normal.z.abs() >= LADDER_MAX_NORMAL_Z {
            self.me.ladder = None;
            return;
        }
        let n = tr.normal;
        self.me.ladder = Some(n);
        if intent.jump {
            // Off the ladder, pushed away from it; walking from this tick.
            self.me.ladder = None;
            self.v = n * LADDER_JUMP_OFF;
            return;
        }
        let button = |x: f32| {
            if x > 0.0 {
                LADDER_SPEED
            } else if x < 0.0 {
                -LADDER_SPEED
            } else {
                0.0
            }
        };
        let (fwd, side) = (button(intent.move_axis.y), button(intent.move_axis.x));
        if fwd == 0.0 && side == 0.0 {
            self.v = Vec3::ZERO;
            return;
        }
        let u = forward * fwd + right * side;
        let a = u.dot(n);
        let into = n * a;
        let mut lateral = u - into;
        let perp = Vec3::Z.cross(n).normalize_or_zero();
        let up = n.cross(perp);
        // CS:S: mostly pushing into the ladder damps the sideways part.
        let (t, p) = (up.dot(lateral), perp.dot(lateral));
        if (perp * p + into).normalize_or_zero().dot(n) < self.cfg.ladder_angle {
            lateral = up * t + perp * (self.cfg.ladder_dampen * p);
        }
        self.v = lateral - up * a;
        if self.cfg.duck_slows_ladder {
            self.v *= duck_scale;
        }
        let below = self.feet - Vec3::Z * 1.0;
        let on_floor = self.me.on_ground || self.trace.point_solid(below);
        if on_floor && a > 0.0 {
            self.v += n * LADDER_BACK_OFF;
        }
    }
}
