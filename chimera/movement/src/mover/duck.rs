//! Ducking: the duck and unduck with their timers and eye blend, the air
//! duck's lift and its limit, what blocks standing up, and a game's own low
//! stance held as the ducked box (`MoveStance`).

use bevy::prelude::*;

use crate::mover::{DUCK_TIMER_START, Mover, TIME_TO_UNDUCK};

impl Mover<'_, '_, '_, '_> {
    #[inline]
    pub(crate) fn eye_blend(&self, fraction: f32) -> f32 {
        self.cfg.eye_duck * fraction + self.cfg.eye_stand * (1.0 - fraction)
    }

    #[inline]
    pub(crate) fn smooth(x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        3.0 * x * x - 2.0 * x * x * x
    }

    pub(crate) fn finish_duck(&mut self) {
        if self.me.ducked {
            return;
        }
        self.me.ducked = true;
        self.me.ducking = false;
        self.me.eye = self.cfg.eye_duck;
        if !self.me.on_ground {
            self.feet.z += self.cfg.air_duck_lift();
        }
        let before = self.feet;
        let mut free = !self.trace.solid(true, self.feet);
        for _ in 0..36 {
            if free {
                break;
            }
            self.feet.z += 1.0;
            free = !self.trace.solid(true, self.feet);
        }
        if !free {
            self.feet = before;
        }
        self.categorize();
    }

    /// Whether the standing box fits: on the ground, where it stands; in the
    /// air, swept from here down by the hull difference (room is needed
    /// above the ducked head and below the feet).
    /// A game's low stance (`MoveStance::height`): the ducked box, at
    /// that height (the config's), held whatever the duck key says.
    pub(crate) fn hold_stance(&mut self) {
        self.me.ducked = true;
        self.me.ducking = false;
        self.me.duck_held = false;
        self.me.duck_timer = 0.0;
        self.me.stance_low = true;
        self.me.stance_crouch = self.stance.crouch;
    }

    /// Out of a game's low stance: up at once where the standing box fits,
    /// else the usual ducked box, which stands up once there is room.
    pub(crate) fn leave_stance(&mut self) {
        if !self.me.stance_low {
            return;
        }
        self.me.stance_low = false;
        if std::mem::take(&mut self.me.stance_crouch) {
            // A crouch: ducked, duck as if held (`MoveStance::crouch`).
            self.me.ducked = true;
            self.me.ducking = false;
            self.me.duck_held = true;
            self.me.eye = self.cfg.eye_duck;
            return;
        }
        if self.can_stand() {
            self.me.ducked = false;
            self.me.ducking = false;
            self.me.eye = self.cfg.eye_stand;
        }
    }

    #[inline]
    pub(crate) fn can_stand(&self) -> bool {
        if self.me.on_ground {
            return !self.trace.solid(false, self.feet);
        }
        let down = self.feet - Vec3::Z * self.cfg.air_duck_lift();
        let tr = self.trace.sweep(false, self.feet, down);
        !tr.start_solid && !tr.hit()
    }

    pub(crate) fn finish_unduck(&mut self) {
        if !self.me.on_ground {
            self.feet.z -= self.cfg.air_duck_lift();
        }
        self.me.ducked = false;
        self.me.ducking = false;
        self.me.eye = self.cfg.eye_stand;
        self.categorize();
    }

    /// GoldSrc's unduck (`QuakeConfig::unduck_at_once`): the standing box
    /// tried where the original puts it (its centre 18 over the ducked
    /// box's on the ground, at it in the air; a duck not yet completed has
    /// the standing box's centre still, so it is tried 18 higher on the
    /// ground), and taken at once when it fits; else nothing, tried again
    /// every tick.
    pub(crate) fn unduck_now(&mut self) {
        let lift = if self.me.ducked {
            if self.me.on_ground { 0.0 } else { -self.cfg.air_duck_lift() }
        } else if self.me.on_ground {
            (self.cfg.stand_height - self.cfg.duck_height) / 2.0
        } else {
            0.0
        };
        let at = self.feet + Vec3::Z * lift;
        if self.trace.solid(false, at) {
            return;
        }
        self.feet = at;
        self.me.ducked = false;
        self.me.ducking = false;
        self.me.duck_timer = 0.0;
        self.me.eye = self.cfg.eye_stand;
        if self.cfg.goldsrc_steps {
            // Standing up takes 100 ms off the step timer.
            self.me.step_timer = (self.me.step_timer - 100.0).max(0.0);
        }
        self.categorize();
    }

    /// A duck timer started this tick (`duck_counts_first_tick`).
    #[inline]
    pub(crate) fn duck_timer_start(&self) -> f32 {
        if self.cfg.duck_counts_first_tick {
            DUCK_TIMER_START - 1000.0 * self.dt
        } else {
            DUCK_TIMER_START
        }
    }

    /// Ducking transitions; returns the move-input scale, decided from the
    /// state before this tick's changes.
    pub(crate) fn duck(&mut self, held: bool) -> f32 {
        let scale = if self.me.ducked && (self.me.on_ground || self.cfg.duck_slows_everywhere) {
            self.cfg.duck_speed
        } else {
            1.0
        };
        let pressed = held && !self.me.duck_held;
        let released = !held && self.me.duck_held;
        let elapsed = (DUCK_TIMER_START - self.me.duck_timer) / 1000.0;
        if held {
            // A press while still flagged ducked (mid-unduck) doesn't
            // restart anything: the eye stays where it is while held.
            if pressed && !self.me.ducked {
                self.me.duck_timer = self.duck_timer_start();
                self.me.ducking = true;
            }
            if self.me.ducking && !self.me.ducked {
                let elapsed = (DUCK_TIMER_START - self.me.duck_timer) / 1000.0;
                let done = elapsed > self.cfg.duck_time || (self.cfg.duck_ends_on_time && elapsed >= self.cfg.duck_time);
                if done || !self.me.on_ground {
                    self.finish_duck();
                } else {
                    self.me.eye = self.eye_blend(Self::smooth(elapsed / self.cfg.duck_time));
                }
            }
        } else if self.cfg.unduck_at_once && (self.me.ducked || self.me.ducking) {
            self.unduck_now();
        } else if self.me.ducked || self.me.ducking {
            if released {
                if !self.me.ducking || self.me.ducked {
                    self.me.duck_timer = if self.cfg.unduck_counts_first_tick {
                        DUCK_TIMER_START - 1000.0 * self.dt
                    } else {
                        self.duck_timer_start()
                    };
                } else if !self.cfg.unduck_keeps_duck_timer {
                    // Released partway down: the unduck starts from the
                    // matching point (shared code).
                    let fraction = (elapsed / self.cfg.duck_time).clamp(0.0, 1.0);
                    self.me.duck_timer =
                        DUCK_TIMER_START - TIME_TO_UNDUCK * 1000.0 + fraction * TIME_TO_UNDUCK * 1000.0;
                }
                self.me.ducking = true;
            }
            if self.can_stand() {
                let elapsed = (DUCK_TIMER_START - self.me.duck_timer) / 1000.0;
                if elapsed > TIME_TO_UNDUCK || !self.me.on_ground {
                    self.finish_unduck();
                } else {
                    self.me.eye = self.eye_blend(Self::smooth(1.0 - elapsed / TIME_TO_UNDUCK));
                    self.me.ducking = true;
                }
            } else if self.me.duck_timer != DUCK_TIMER_START {
                // No room to stand: forced fully ducked until there is.
                self.me.ducked = true;
                self.me.ducking = false;
                self.me.eye = self.cfg.eye_duck;
                self.me.duck_timer = DUCK_TIMER_START;
            }
        } else if (self.me.eye - self.cfg.eye_stand).abs() > 0.1 {
            self.me.eye = self.cfg.eye_stand;
        }
        self.me.duck_held = held;
        scale
    }

    /// The duck key as TF2's limits let it through: the air duck limit
    /// (`air_ducks_max`: releases in the air counted, the key not held in
    /// the air once the limit is reached), the re-duck delay
    /// (`reduck_delay`: not held on the ground until it runs out after a
    /// release, nor anywhere during an unduck transition) and no duck in
    /// water (`no_water_duck`).
    pub(crate) fn air_duck_allowed(&mut self, held: bool) -> bool {
        let released = !held && self.me.duck_key;
        self.me.duck_key = held;
        self.me.reduck_timer = (self.me.reduck_timer - self.dt).max(0.0);
        if released {
            if !self.me.on_ground && self.cfg.air_ducks_max > 0 {
                self.me.air_ducks = self.me.air_ducks.saturating_add(1);
            }
            self.me.reduck_timer = self.cfg.reduck_delay;
        }
        if !held {
            return false;
        }
        if !self.me.on_ground && self.cfg.air_ducks_max > 0 && self.me.air_ducks >= self.cfg.air_ducks_max {
            return false;
        }
        if self.cfg.reduck_delay > 0.0 {
            if self.me.on_ground && self.me.reduck_timer > 1e-6 {
                return false;
            }
            // Mid-unduck (still flagged ducked): the unduck goes on.
            if self.me.ducked && self.me.ducking && !self.me.duck_held {
                return false;
            }
        }
        if self.cfg.no_water_duck && ((self.me.water_level >= 1 && !self.me.on_ground) || self.me.water_level >= 3) {
            return false;
        }
        true
    }
}
