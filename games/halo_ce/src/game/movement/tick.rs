//! One tick of a Halo player's movement (specs/halo_ce/movement.md,
//! "Per-tick order"), as a function of the movement's state, the body
//! (origin and velocity), the tick's controls and the level (`mover::Space`):
//! throttle, speeds and the wanted motion, the crouch step and its air lift,
//! ground acceleration on the ground plane with the slope scale and the
//! ground push, or air control and gravity, the collision move, the ground
//! test, standing room, the landing check, the jump, falling damage, the
//! landing counter and the stance. World units and the 30 Hz tick, engine
//! axes (+Y up).
//!
//! `k` is the engine tick's length in Halo ticks (1 at 30 Hz, exactly):
//! per-tick steps (accelerations, gravity, the crouch rate, the timers) are
//! scaled by it and the displacement is the velocity times it (the module
//! doc of `super` says why).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    data::MovementData,
    mover::{self, Space},
};

/// A landing in progress (spec section 6).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Landing {
    pub hard: bool,
    /// Grounded ticks counted so far (the landing tick counts).
    pub counter: f32,
    /// Its length R in ticks.
    pub ticks: f32,
}

/// Halo's movement state on a player: the slot's marker and everything the
/// movement keeps between ticks (predicted: `predicted_net`). World units,
/// engine axes (+Y up).
#[derive(Component, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HaloMovement {
    /// Crouch fraction c (0 standing, 1 crouched).
    pub crouch: f32,
    /// The stance decided at the end of the last tick: crouch.
    pub stance_crouch: bool,
    /// Airborne after the last tick (spec section 5, "The ground test").
    pub airborne: bool,
    /// The stored ground plane's normal.
    pub ground_normal: Vec3,
    /// The jump timer (ticks; starts at the data's maximum).
    pub jump_timer: f32,
    pub landing: Option<Landing>,
    /// The last tick's landing velocity lambda (WU/t; 0 when airborne).
    pub landing_velocity: f32,
    /// The last tick jumped.
    pub jumped: bool,
    /// The last tick's standing-room test failed.
    pub no_room: bool,
    /// Set up (the first tick, and each spawn: `super`).
    pub initialized: bool,
    /// Dead last tick (a respawn starts it over).
    pub was_dead: bool,
}

impl Default for HaloMovement {
    fn default() -> Self {
        Self {
            crouch: 0.0,
            stance_crouch: false,
            airborne: true,
            ground_normal: Vec3::Y,
            jump_timer: 0.0,
            landing: None,
            landing_velocity: 0.0,
            jumped: false,
            no_room: false,
            initialized: false,
            was_dead: false,
        }
    }
}

impl HaloMovement {
    /// In a hard landing.
    pub fn hard_landing(&self) -> bool {
        self.landing.is_some_and(|l| l.hard)
    }
}

/// One tick's controls (spec section 1, after the input layer).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Controls {
    /// Throttle (t_x forward, t_y left), each in [-1, 1].
    pub throttle: Vec2,
    /// The movement facing's horizontal forward and left (unit).
    pub forward: Vec3,
    pub left: Vec3,
    pub jump: bool,
    pub crouch: bool,
    /// The controller's crouch rule applies (a stick; keyboards: no).
    pub controller: bool,
    /// Body stun s_b (0 to 1).
    pub stun: f32,
    /// The player's speed multiplier m_p (1 unless the game type sets it).
    pub speed_multiplier: f32,
}

/// What a tick did.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outcome {
    /// The origin (between the feet) and velocity (WU/t) stored.
    pub origin: Vec3,
    pub velocity: Vec3,
    /// Grounded after the move.
    pub grounded: bool,
    /// Falling damage dealt this tick, in points.
    pub damage: Option<f32>,
    /// The move's contacts' normals.
    pub contacts: Vec<Vec3>,
}

/// One tick (module doc). `origin` and `velocity` as stored after the last
/// tick (WU, WU/t).
pub fn tick(
    d: &MovementData,
    me: &mut HaloMovement,
    origin: Vec3,
    velocity: Vec3,
    c: Controls,
    k: f32,
    space: &dyn Space,
) -> Outcome {
    let tps = super::data::TICKS_PER_SECOND;
    let eps = d.epsilon;
    // 1. Controls: the throttle normalised; the controller's crouch rule
    // with last tick's airborne flag.
    let mut t = c.throttle;
    let m2 = t.length_squared();
    if m2 > 1.0 {
        t /= m2.sqrt();
    }
    let crouch_held = c.crouch && (me.airborne || !c.controller || t.length_squared() < d.crouch_stick * d.crouch_stick);
    // 3. The dead zone.
    if t.length_squared() < d.throttle_dead * d.throttle_dead {
        t = Vec2::ZERO;
    }
    // 6. Setup: the pill from the start-of-tick crouch, speeds, wanted
    // motion, acceleration limits.
    let c0 = me.crouch;
    let h = d.segment(c0);
    let u = 1.0 - c0;
    let forward_speed = if t.x > 0.0 {
        d.run_forward * u + d.sneak_forward * c0
    } else {
        d.run_backward * u + d.sneak_backward * c0
    };
    let side_speed = d.run_sideways * u + d.sneak_sideways * c0;
    let accel = d.run_accel * u + d.sneak_accel * c0;
    let k_s = (1.0 - d.stun_move * c.stun) * c.speed_multiplier;
    let (mut a, mut b) = (t.x * k_s * forward_speed / tps, t.y * k_s * side_speed / tps);
    let hard = me.hard_landing();
    let (ground_max, air_max) = if hard {
        a = 0.0;
        b = 0.0;
        (d.hard_land_decel, 0.0)
    } else {
        (accel / tps, d.air_accel / tps)
    };
    // 7. The crouch fraction toward the stance; the air lift.
    let target = if me.stance_crouch { 1.0 } else { 0.0 };
    let step = d.crouch_rate * k;
    let c1 = if (target - c0).abs() <= step { target } else { c0 + step * (target - c0).signum() };
    me.crouch = c1;
    let delta = c1 - c0;
    let lift = if me.airborne && delta.abs() > d.crouch_lift_min {
        (d.height_stand - d.height_crouch) * delta
    } else {
        0.0
    };
    // 8. The velocity.
    let wish_h = c.forward * a + c.left * b;
    let mut v = velocity;
    let mut gravity_fix = 0.0;
    if me.airborne {
        let mut dh = Vec3::new(wish_h.x - v.x, 0.0, wish_h.z - v.z);
        let len = dh.length();
        let max = air_max * k;
        if len > max {
            dh *= if len > 0.0 { max / len } else { 0.0 };
        }
        v += dh;
        v.y -= d.gravity * k;
        // Off the 30 Hz tick, the displacement keeps the 30 Hz path
        // (super's module doc): 0 at k = 1.
        gravity_fix = -d.gravity * k * (1.0 - k) / 2.0;
    } else {
        let n = me.ground_normal;
        let dir = if n.y > eps {
            Vec3::new(wish_h.x, -(wish_h.x * n.x + wish_h.z * n.z) / n.y, wish_h.z).normalize_or_zero()
        } else {
            Vec3::ZERO
        };
        let sigma = (a * a + b * b).sqrt() * d.slope_scale(dir.y);
        let w = dir * sigma;
        let mut dv = w - v;
        let len = dv.length();
        let max = ground_max * k;
        if len > max {
            dv *= max / len;
        }
        dv -= n * d.ground_push;
        v += dv;
    }
    // 9. The move: the velocity plus the crouch lift.
    let lift_v = if k > 0.0 { lift / k } else { 0.0 };
    let swept = v + Vec3::Y * lift_v;
    let r = d.radius;
    let p = origin + Vec3::Y * r;
    let moved = mover::sweep(space, d, p, h, swept * k + Vec3::Y * gravity_fix);
    // 10. The ground test.
    let best = moved
        .contacts
        .iter()
        .copied()
        .fold(None, |best: Option<Vec3>, n| match best {
            Some(b) if b.y >= n.y => Some(b),
            _ => Some(n),
        });
    let ground = best.filter(|n| n.y >= d.max_slope_cos);
    let grounded = ground.is_some();
    let lambda = ground.map_or(0.0, |n| -n.dot(swept));
    // 11. Standing room.
    let no_room = c1 > 0.0 && !crouch_held && space.sphere_blocked(moved.p, Vec3::Y * (d.height_stand - 2.0 * r));
    // 12. Store.
    let new_origin = moved.p - Vec3::Y * r;
    let mut v = moved.clip_velocity(swept) - Vec3::Y * lift_v;
    me.airborne = !grounded;
    if let Some(n) = ground {
        me.ground_normal = n;
    }
    me.landing_velocity = lambda;
    me.no_room = no_room;
    // 13. The landing check.
    if grounded
        && lambda > 0.0
        && let Some((hard, ticks)) = d.landing(lambda)
    {
        me.landing = Some(Landing {
            hard,
            counter: 0.0,
            ticks,
        });
    }
    // 14. The jump timer and the jump.
    me.jumped = false;
    if grounded && !me.hard_landing() && !no_room {
        me.jump_timer = (me.jump_timer + k).min(d.jump_timer_max);
        if c.jump && me.jump_timer > d.jump_timer_above {
            let j = d.jump_velocity * (1.0 - c.stun * d.stun_turn);
            v.y = v.y.max(j);
            me.airborne = true;
            me.jump_timer = 0.0;
            me.jumped = true;
        }
    }
    // 15. Falling damage.
    let damage = if grounded && lambda > d.fall_v_min {
        d.fall_damage(lambda)
    } else if v.y < -d.fall_v_kill {
        Some(d.fall_kill_damage)
    } else {
        None
    };
    // 17. The landing counter.
    if grounded && let Some(l) = me.landing.as_mut() {
        l.counter += k;
        if l.counter >= l.ticks {
            me.landing = None;
        }
    }
    // 19. The stance for the next tick.
    me.stance_crouch = crouch_held || no_room;
    Outcome {
        origin: new_origin,
        velocity: v,
        grounded,
        damage,
        contacts: moved.contacts,
    }
}
