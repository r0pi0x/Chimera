//! One tick of one character (`Mover`, in Source units): the tick's order
//! (`tick`), gravity and the velocity limit, the view's directions, and the
//! tick's constants. Its parts: `ground` (friction, acceleration, the walk,
//! slide and step moves, the ground test), `air`, `duck`, `jump`, `water`,
//! `ladders` (ladder brushes), `falls` (landings, the punch roll), `steps`
//! (footsteps) and `stuck` (stuck recovery, seating from another movement).

mod air;
mod duck;
pub(crate) mod falls;
mod ground;
mod jump;
mod ladders;
pub mod lines;
mod steps;
pub(crate) mod stuck;
mod water;

use bevy::prelude::*;

use crate::{
    config::QuakeConfig,
    core::{ExternalPush, Intent},
    hooks::{MoveStance, StepSounds},
    mover::lines::UseableLadder,
    state::QuakeMovement,
    trace::Tracer,
};

/// Landing view roll per unit/s of fall speed (degrees), and the punch
/// spring's damping and constant (movement spec).
pub(crate) const LAND_PUNCH_SCALE: f32 = 0.013;

pub(crate) const PUNCH_DAMPING: f32 = 9.0;

pub(crate) const PUNCH_SPRING: f32 = 65.0;

pub(crate) const AIR_WISH_CAP: f32 = 30.0;

pub(crate) const WALKABLE_NORMAL_Z: f32 = 0.7;

pub(crate) const GROUND_PROBE: f32 = 2.0;

pub(crate) const LEAVE_GROUND_VZ: f32 = 140.0;

pub(crate) const UNGROUND_VZ: f32 = 250.0;

pub(crate) const MAX_CLIP_PLANES: usize = 5;

pub(crate) const BUMPS: usize = 4;

pub(crate) const STEP_EPSILON: f32 = 0.031_25;

pub(crate) const STAY_ON_GROUND_UP: f32 = 2.0;

pub(crate) const SNAP_MIN: f32 = 1.0 / 64.0;

pub(crate) const DUCK_TIMER_START: f32 = 1000.0;

pub(crate) const TIME_TO_UNDUCK: f32 = 0.2;

pub(crate) const UPWARD_AIR_FRICTION: f32 = 0.25;

// Ladders and water (spec "Ladders", "Water").
pub(crate) const LADDER_REACH: f32 = 2.0;

/// Half the width of the post another game's ladder line stands for, for
/// a movement climbing contents ladders (units; ours).
pub(crate) const POST_HALF: f32 = 1.0;

pub(crate) const LADDER_SPEED: f32 = 200.0;

pub(crate) const LADDER_JUMP_OFF: f32 = 270.0;

pub(crate) const LADDER_BACK_OFF: f32 = 200.0;

/// A ladder face whose normal is this steep or steeper (|z|) is a ladder
/// brush's top or bottom, not something to climb: ours, the spec is
/// silent (docs/tech-debt.md). The walkable-floor limit.
pub(crate) const LADDER_MAX_NORMAL_Z: f32 = 0.7;

pub(crate) const WATER_FEET_PROBE: f32 = 1.0;

pub(crate) const SWIM_SINK: f32 = 60.0;

pub(crate) const SWIM_WISH_SCALE: f32 = 0.8;

pub(crate) const SWIM_JUMP_WATER: f32 = 100.0;

pub(crate) const SWIM_JUMP_SLIME: f32 = 80.0;

pub(crate) const WATER_JUMP_UP: f32 = 256.0;

pub(crate) const WATER_JUMP_PUSH: f32 = 50.0;

pub(crate) const WATER_JUMP_REACH: f32 = 24.0;

pub(crate) const WATER_JUMP_EYE_EXTRA: f32 = 8.0;

pub(crate) const WATER_JUMP_DROP: f32 = 1024.0;

pub(crate) const WATER_JUMP_MIN_VZ: f32 = -180.0;

pub(crate) const WATER_JUMP_TIME: f32 = 2000.0;

/// Smallest gap between two stuck nudges, seconds.
pub(crate) const STUCK_NUDGE_GAP: f32 = 0.05;

/// One tick for one character, in Source units.
pub(crate) struct Mover<'a, 'b, 'w, 's> {
    pub(crate) cfg: &'a QuakeConfig,
    pub(crate) trace: &'a Tracer<'b, 'w, 's>,
    pub(crate) me: &'a mut QuakeMovement,
    pub(crate) feet: Vec3,
    pub(crate) v: Vec3,
    pub(crate) dt: f32,
    /// Water contents per test point (feet, waist, eye) this tick.
    pub(crate) water_cache: [Option<(Vec3, Option<bool>)>; 3],
    /// Move input added by props pushing the player back (forward, side).
    pub(crate) push_input: Vec2,
    /// Sounds made this tick: entry, where (Source units), volume.
    pub(crate) sounds: Vec<(String, Vec3, Option<f32>)>,
    /// Max speed from the held weapon, units/s.
    pub(crate) player_maxspeed: f32,
    /// Fall damage taken this tick, health points.
    pub(crate) fall_damage: f32,
    /// Base velocity (specs/source/triggers.md, trigger_push), units/s;
    /// set by a push during the last tick; taken off the ground by logic.
    pub(crate) base: Vec3,
    pub(crate) base_touched: bool,
    pub(crate) unground: bool,
    /// A game's push waiting for the ground (`core::ExternalPush`).
    pub(crate) push: ExternalPush,
    /// Gravity multiplier (0 = normal).
    pub(crate) gravity_scale: f32,
    /// Moving solids' velocities (Source units/s), for riders.
    pub(crate) mover_velocity: &'a dyn Fn(Entity) -> Vec3,
    /// Whether a collider's body floats in water (`map::buoyancy::Floating`).
    pub(crate) floating: &'a dyn Fn(Entity) -> bool,
    /// Waist-deep step attempts so far, every player's (`waist_step_skip`).
    pub(crate) waist_steps: &'a mut u32,
    /// Physics colliders the move ran into this tick, the one stood on,
    /// and the wish velocity (units/s): the physics shadow's inputs
    /// (physics_props.md 4.1).
    pub(crate) touched: Vec<Entity>,
    pub(crate) ground_collider: Option<Entity>,
    pub(crate) wish: Vec3,
    /// Down onto the floor found (`SettleOnGround`).
    pub(crate) settle: bool,
    /// This tick's max speed from the game (`MoveSpeed`), units/s.
    pub(crate) speed: Option<f32>,
    /// The character's own footsteps (`StepSounds`).
    pub(crate) step_sounds: Option<&'a StepSounds>,
    /// The map's useable ladders (and the other ladders as lines).
    pub(crate) ladders: &'a [UseableLadder],
    /// The map isn't this movement's game's (`core::foreign_map`): its
    /// ladder brushes are climbed as lines too.
    pub(crate) foreign_map: bool,
    /// Map outputs fired this tick (entity index, output).
    pub(crate) outputs: Vec<(usize, &'static str)>,
    /// The character's max health in points (`fall_by_max_health`).
    pub(crate) max_health: f32,
    /// A game's own stance this tick (`MoveStance`).
    pub(crate) stance: MoveStance,
}

impl Mover<'_, '_, '_, '_> {
    #[inline]
    pub(crate) fn clamp_velocity(&mut self) {
        let m = self.cfg.maxvelocity;
        self.v = Vec3::new(
            if self.v.x.is_nan() { 0.0 } else { self.v.x.clamp(-m, m) },
            if self.v.y.is_nan() { 0.0 } else { self.v.y.clamp(-m, m) },
            if self.v.z.is_nan() { 0.0 } else { self.v.z.clamp(-m, m) },
        );
    }

    #[inline]
    pub(crate) fn half_gravity(&mut self) {
        let scale = if self.gravity_scale == 0.0 { 1.0 } else { self.gravity_scale };
        self.v.z -= scale * self.cfg.gravity * self.dt * 0.5;
    }

    /// The first gravity half-step also applies the vertical base velocity
    /// as an acceleration (triggers.md, trigger_push, step 2).
    #[inline]
    pub(crate) fn start_gravity(&mut self) {
        self.half_gravity();
        self.v.z += self.base.z * self.dt;
        self.base.z = 0.0;
    }

    /// Full 3D view vectors (Source axes): forward and right.
    #[inline]
    pub(crate) fn view(intent: &Intent) -> (Vec3, Vec3) {
        // Intent yaw 0 looks down engine -Z, which is Source yaw 90;
        // intent pitch is up-positive, Source pitch down-positive.
        let yaw = std::f32::consts::FRAC_PI_2 + intent.yaw;
        let pitch = -intent.pitch;
        let forward = Vec3::new(pitch.cos() * yaw.cos(), pitch.cos() * yaw.sin(), -pitch.sin());
        let right = Vec3::new(yaw.sin(), -yaw.cos(), 0.0);
        (forward, right)
    }

    pub(crate) fn tick(&mut self, intent: &Intent) {
        // Move input: full keys send key_speed, rescaled to the max speed.
        // Walking (+speed) lowers the max speed itself (CS:S, measured): the
        // keys rescale to it, and swimming's wish speed is capped by it.
        // A recent hit holds it lower (M18): faster than that slows by
        // the usual friction.
        let walk = if intent.walk { self.cfg.walk_speed } else { 1.0 };
        let max_speed = match self.speed {
            // The game chose it (HL2:DM's walk, run or sprint).
            Some(p) => p.min(self.cfg.maxspeed) * self.me.velocity_modifier,
            None => self.player_maxspeed.min(self.cfg.maxspeed) * walk * self.me.velocity_modifier,
        } * self.ground_factors().0;
        let keys = intent.move_axis;
        let (mut f, mut s) = (
            keys.y * self.cfg.key_speed + self.push_input.x,
            keys.x * self.cfg.key_speed + self.push_input.y,
        );
        let len = (f * f + s * s).sqrt();
        if len > max_speed {
            f *= max_speed / len;
            s *= max_speed / len;
        }
        self.me.duck_timer = (self.me.duck_timer - 1000.0 * self.dt).max(0.0);
        self.me.stamina = (self.me.stamina - 1000.0 * self.dt).max(0.0);
        self.me.swim_timer = (self.me.swim_timer - 1000.0 * self.dt).max(0.0);
        self.decay_punch();

        // Intent yaw 0 looks down engine -Z, which is Source yaw 90.
        let yaw = std::f32::consts::FRAC_PI_2 + intent.yaw;
        let forward = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
        let right = Vec3::new(yaw.sin(), -yaw.cos(), 0.0);

        // Leftover base velocity becomes real velocity when no push
        // touched the player during the last tick (triggers.md, step 1).
        if !self.base_touched {
            self.v += self.base * (1.0 + self.dt / 2.0);
            self.base = Vec3::ZERO;
        }
        self.base_touched = false;
        // A pull onto or off a useable ladder: nothing else moves the
        // player until it arrives (movement.md "Forced moves").
        let mut arrived_off = false;
        if self.me.forced.is_some() {
            match self.ladder_step(intent, f, s, max_speed) {
                lines::LadderStep::Done => {
                    self.me.last_feet = Some(self.feet);
                    return;
                }
                lines::LadderStep::Climb => {
                    self.ladder_move(intent, f, s, max_speed);
                    self.me.last_feet = Some(self.feet);
                    return;
                }
                lines::LadderStep::Walk => {
                    // A dismount's arrival: the walking move without input.
                    f = 0.0;
                    s = 0.0;
                    arrived_off = true;
                }
            }
        }
        if !self.check_stuck() {
            return;
        }
        // On a ladder, or moved by game code (teleport, spawn): full ground
        // detection now. Otherwise only rising fast removes the ground.
        // Taken off the ground by logic (push, teleport): stay off.
        let moved = self.me.last_feet.is_none_or(|f| f.distance_squared(self.feet) > 1e-4);
        if self.unground {
            self.set_ground(false, None);
        } else if self.me.ladder.is_some() || moved {
            self.categorize();
        } else if self.v.z > UNGROUND_VZ {
            self.set_ground(false, None);
        }
        if !self.me.on_ground {
            self.me.fall_speed = -self.v.z;
        }
        let water_before = self.me.water_level;
        self.footsteps();
        // A game's low stance held last tick is no duck (its own speed:
        // `MoveStance::max_speed`), so leaving it isn't slowed.
        let was_ducking = (self.me.ducked || self.me.ducking) && !self.me.stance_low;
        let low = self.stance.height.is_some();
        let mut scale = if low {
            self.hold_stance();
            1.0
        } else {
            self.leave_stance();
            let crouch = self.air_duck_allowed(intent.crouch);
            self.duck(crouch)
        };
        if let Some(eye) = self.stance.eye {
            self.me.eye = eye;
        }
        // CS:S (measured): input is slowed while duck is held, or when the
        // tick started ducked or mid-duck (on the ground or off it).
        if self.cfg.duck_slows_everywhere && !low {
            scale = if intent.crouch || was_ducking {
                self.cfg.duck_speed
            } else {
                1.0
            };
        }
        f *= scale;
        s *= scale;

        // CS:S (measured, movecmp fuzz): the climb is slowed by the duck as
        // it stands after this tick's ducking: releasing duck while ducked
        // on a ladder in the air (an instant unduck) climbs at full speed
        // that tick (272.8 up, not 92.7).
        let ladder_scale = if self.cfg.duck_slows_everywhere && !(intent.crouch || self.me.ducked || self.me.ducking) {
            1.0
        } else {
            scale
        };
        if self.cfg.useable_ladders && !arrived_off {
            match self.ladder_step(intent, f, s, max_speed) {
                lines::LadderStep::Done => {
                    self.me.last_feet = Some(self.feet);
                    return;
                }
                lines::LadderStep::Climb => {
                    self.ladder_move(intent, f, s, max_speed);
                    self.me.last_feet = Some(self.feet);
                    return;
                }
                lines::LadderStep::Walk => {}
            }
        }
        if self.cfg.climbs_contents(self.foreign_map) {
            self.ladder(intent, f, s, ladder_scale);
        }
        if self.me.ladder.is_some() {
            // Ladder move: no gravity, friction or ground snapping.
            self.water_check();
            if !intent.jump {
                self.me.jump_held = false;
            }
            self.slide();
            self.me.last_feet = Some(self.feet);
            return;
        }

        self.water_check();
        if self.me.water_level < 2 {
            self.start_gravity();
            self.clamp_velocity();
        }
        if self.me.water_jump_time > 0.0 {
            self.me.water_jump_time -= 1000.0 * self.dt;
            if self.me.water_jump_time <= 0.0 || self.me.water_level == 0 {
                self.me.water_jump_time = 0.0;
            }
            self.v.x = self.me.water_jump_vel.x;
            self.v.y = self.me.water_jump_vel.y;
            self.slide();
            self.water_check();
            self.water_sound(water_before);
            self.me.last_feet = Some(self.feet);
            return;
        }
        if self.me.water_level >= 2 {
            if self.me.water_level == 2 {
                self.water_jump_check(intent);
            }
            if intent.jump && self.me.swim_timer <= 0.0 {
                // A swim stroke at most once a second while holding jump.
                self.me.swim_timer = 1000.0;
                self.sounds.push((self.cfg.swim_sound.into(), self.feet, None));
            }
            if intent.jump {
                // Jump does nothing while water-jumping: a water jump that
                // started this tick keeps its lift.
                if self.me.water_jump_time <= 0.0 {
                    self.v.z = if self.me.in_slime {
                        SWIM_JUMP_SLIME
                    } else {
                        SWIM_JUMP_WATER
                    };
                    self.me.on_ground = false;
                }
            } else {
                self.me.jump_held = false;
            }
            self.swim(intent, f, s, max_speed);
            self.categorize();
            if self.me.on_ground {
                self.v.z = 0.0;
            }
            self.water_sound(water_before);
            self.me.last_feet = Some(self.feet);
            return;
        }

        if self.stance.no_jump {
            self.me.jump_held = intent.jump;
        } else if intent.jump {
            let fresh = !self.me.jump_held || self.cfg.auto_bunnyhopping;
            let fully_ducked = self.me.ducked && !self.me.ducking;
            let air_jump = !self.me.on_ground && !self.me.jump_held && self.me.air_jumps_used < self.cfg.air_jumps;
            if self.me.on_ground && fresh && !(self.me.ducked && self.me.ducking) && !(self.cfg.no_ducked_jump && fully_ducked) {
                self.jump();
            } else if air_jump {
                self.air_jump(f, s, forward, right, max_speed);
            } else {
                if self.cfg.unduck_jump_ducks && self.me.on_ground && fresh && self.me.ducked && self.me.ducking {
                    // CS:S (measured, movecmp `unduck_jump_*`): a jump
                    // pressed while standing up from a duck doesn't jump,
                    // but it costs the jump's stamina, the view drops back
                    // to the ducked eye, and the player stands up at once
                    // on the next tick (timer run out).
                    self.me.stamina = self.cfg.jump_stamina;
                    self.me.eye = self.cfg.eye_duck;
                    self.me.ducking = false;
                    self.me.duck_timer = 0.0;
                }
                self.me.jump_held = true;
            }
        } else {
            self.me.jump_held = false;
        }
        if self.me.on_ground {
            self.v.z = 0.0;
            self.friction();
            if let Some(limit) = self.push.take_on_ground() {
                // A push met the ground (`core::ExternalPush`): after
                // friction, cut to the limit of this body's max speed.
                self.clamp_flat(limit * max_speed);
            }
        }
        self.clamp_velocity();
        if self.me.on_ground {
            self.walk(f, s, forward, right, max_speed);
        } else {
            self.air(f, s, forward, right, max_speed);
        }
        self.categorize();
        self.clamp_velocity();
        if self.me.water_level < 2 {
            self.half_gravity();
            self.clamp_velocity();
        }
        if self.me.on_ground {
            self.v.z = 0.0;
            if self.me.fall_speed > 0.0 {
                let raw = self.me.fall_speed;
                let fall = self.landing_speed(raw);
                self.me.fall_speed = fall;
                if self.me.water_level == 0 && fall > self.cfg.fall_safe {
                    let by_speed = (fall - self.cfg.fall_safe) * self.cfg.fall_damage_per_speed;
                    self.fall_damage = match self.cfg.fall_fixed {
                        Some(d) => d,
                        // TF2: a share of max health by the whole fall
                        // speed (rounded after the spread: `step`).
                        None if self.cfg.fall_by_max_health > 0.0 => self.max_health * fall * self.cfg.fall_by_max_health,
                        None if self.cfg.fall_fraction => {
                            // HL2: the fall speed itself is lowered, so the
                            // landing rolls the view by what is left.
                            self.me.fall_speed = fall - self.cfg.fall_safe;
                            by_speed
                        }
                        None => by_speed.floor(),
                    };
                }
                self.landing_sound(raw, fall);
                self.me.fall_speed = fall;
                self.me.last_landing_speed = self.me.fall_speed;
                self.me.fall_speed = 0.0;
            }
        }
        if self.me.on_ground {
            self.me.air_jumps_used = 0;
            self.me.air_ducks = 0;
        }
        self.water_sound(water_before);
        self.me.last_feet = Some(self.feet);
    }
}
