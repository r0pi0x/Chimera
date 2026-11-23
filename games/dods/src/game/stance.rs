//! DoD:S's prone, its deployed machine guns and the tick's speed cap
//! (specs/dods/movement.md, "Prone", "Getting up", "Stamina and sprint";
//! damage.md, "Being slowed when hit"; weapons.md, "Alt fire"), a DoD:S
//! player's `DodsStance`, stepped each tick before Source's movement
//! (`step`, predicted) and handed to it as a `SourceStance` (the box, the
//! eye, the speed, no jump):
//!
//! - **Going prone** (`+alt1`, `PRONE_ACTION`, one press, on the ground):
//!   the eye eases from where it is to 10 over 1.2 s (80 ticks, `GO_PRONE`)
//!   by `prone_ease` (the measured heights: 56.61 after the press's tick,
//!   46.38 at 9, 26.87 at 29, 12.15 at 59, 10.00 at 79, within 0.03), no
//!   movement and no jump meanwhile, the standing box kept; on the 80th tick
//!   the box becomes 24 high (`PRONE_HEIGHT`) and the player is prone.
//! - **Prone**: eye 10, crawling at 50 units/s (`PRONE_SPEED`; the sprint
//!   key doesn't change it), the jump key does nothing (measured).
//! - **Getting up** (`+alt1` again): the standing box at once, the eye
//!   eased 10 to 58 over 1.2 s (`GET_UP`), movement allowed with a cap
//!   rising from the crawl's 50 to the run's 220 by a smoothstep of the
//!   time (`rise`: the measured speeds, 7.51 on the press tick, 95.09 at
//!   28, 172.11 at 52, 218.77 at 76, 220 at 80). **Duck pressed while
//!   prone** gets up to a crouch the same way: the ducked box at once, the
//!   eye to 34, the cap rising to the ducked 73.33; at the end the player
//!   is ducked as if duck were held (`SourceStance::crouch`), and stands
//!   by the usual unduck once it isn't. No jump meanwhile.
//! - **Deployed** (a machine gun in hand, `dods:mg`: `weapons::mg`): the
//!   alt fire while prone deploys it at once, its next shot
//!   `DodsMg::deploy_time` later (1.2163 the .30 cal, 1.1667 the MG42); the
//!   alt fire again undeploys, the next shot `undeploy_time` later (1.3635,
//!   1.1667); turning more than `yaw_limit` (45°) either way from where it
//!   was deployed undeploys it too. Deployed, the player doesn't move and
//!   the gun's spread is its sights' (`dods:gun`). Leaving prone, or
//!   another weapon in hand, undeploys it.
//! - **The speed cap** standing up: the stamina's (`stamina::cap`: 120 +
//!   stamina running, 230 + stamina sprinting), 120 while slowed by a hit
//!   (`hit_slow::DodsSlowed`, `SLOWED_CAP`), 50 with a rocket shouldered
//!   (`weapons::explosives::Shoulder`); ducked or ducking the run's 220,
//!   which Source's movement scales by a third, whatever the stamina or
//!   the slow. The movement clamps the ground speed to it the same tick
//!   (`movement::config`'s `ground_clamp`).
//!
//! The stance goes on and off with `dods:movement` (`movement::put_on`,
//! `leave`), as its stamina does; a death drops it.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - The eye's curve getting up is the speed's smoothstep (only its ends
//!   are measured); a ceiling too low to get up is unmeasured (Source's
//!   movement keeps the ducked box until there is room).
//! - Going prone off the ground, on a ladder or in water is refused (our
//!   choice; unmeasured); begun ducked, the eye eases from the ducked eye.
//! - Past the yaw limit the gun undeploys (seen once, at about 58°); the
//!   view isn't held at the limit, and nothing narrows the limit
//!   (weapons.md, open question 11); deploying on a ledge standing or
//!   crouched (`dod_bodyheightoffset`) is unmeasured and not done.
//! - The slow's state is the server's (`hit_slow`): a predicting victim
//!   learns its cap from the next correction.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{Inventory, WeaponState};
use source_engine::source_movement::{SourceMovement, SourceStance};

use super::{
    movement::{DUCK_HEIGHT, EYE_DUCK, EYE_STAND, RUN_SPEED},
    stamina::{self, DodsStamina},
    weapons::{explosives::Shoulder, hit_slow::DodsSlowed, mg::DodsMg},
};
use crate::core::{ActionRegistry, Dead, Intent, SimClock};

/// The named action that goes prone and gets up (DoD:S's `+alt1`).
pub const PRONE_ACTION: &str = "alt1";

/// Going prone, seconds (movement.md: 80 ticks, measured).
pub const GO_PRONE: f32 = 1.2;

/// Getting up, to standing or to a crouch, seconds (measured: 80 ticks).
pub const GET_UP: f32 = 1.2;

/// The prone box's height and its eye, units (measured).
pub const PRONE_HEIGHT: f32 = 24.0;
pub const EYE_PRONE: f32 = 10.0;

/// Crawling speed, units/s (measured).
pub const PRONE_SPEED: f32 = 50.0;

/// The cap while slowed by a hit, units/s (damage.md, measured).
pub const SLOWED_CAP: f32 = 120.0;

/// The duck's share of the run (movement.md: 73.33 of 220).
pub const DUCK_FACTOR: f32 = 1.0 / 3.0;

/// The ease's exponent, fitted to the measured eye heights going prone
/// (`prone_ease`).
pub const EASE_POWER: f32 = 2.325;

/// How far through a transition a fraction `t` (0..1) of its time is: an
/// ease out, 1 − (1 − t)^2.325, fitted to movement.md's eye heights going
/// prone (58 to 10 over 80 ticks: 56.61, 46.38, 35.57, 26.87, 20.16,
/// 15.31, 12.15, 10.47, 10.00 after 1, 9, 19 ... 79 ticks, each within
/// 0.03 units).
pub fn prone_ease(t: f32) -> f32 {
    1.0 - (1.0 - t.clamp(0.0, 1.0)).powf(EASE_POWER)
}

/// How far through getting up a fraction `t` of its time is: a
/// smoothstep, 3t² − 2t³ (the measured speeds while getting up: the cap
/// 50 + 170 × it, module doc).
pub fn rise(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Where a DoD:S player is between standing and prone.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Stance {
    /// Standing or ducked: Source's own stances.
    #[default]
    Up,
    GoingDown,
    Prone,
    GettingUp,
}

/// A DoD:S player's stance (on with `dods:movement`; predicted).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct DodsStance {
    pub stance: Stance,
    /// Seconds into the transition (going down or getting up).
    pub elapsed: f32,
    /// The eye the transition started from, units.
    pub from_eye: f32,
    /// Getting up to a crouch (duck pressed while prone).
    #[serde(default)]
    pub to_crouch: bool,
    /// A machine gun deployed (only while prone), and the view's yaw
    /// (radians, `Intent::yaw`) it was deployed at.
    pub deployed: bool,
    #[serde(default)]
    pub deploy_yaw: f32,
    /// `+alt1`, the alt fire and duck as held last tick (presses act).
    pub prone_key: bool,
    pub alt_key: bool,
    #[serde(default)]
    pub duck_key: bool,
}

impl DodsStance {
    /// Whether the player is fully prone.
    pub fn prone(&self) -> bool {
        self.stance == Stance::Prone
    }

    /// What Source's movement does with it this tick, `cap` the speed cap
    /// standing up (units/s; `step`).
    pub fn source(&self, cap: f32) -> SourceStance {
        match self.stance {
            Stance::Up => SourceStance { max_speed: Some(cap), ..default() },
            Stance::GoingDown => SourceStance {
                height: None,
                eye: Some(self.from_eye + (EYE_PRONE - self.from_eye) * prone_ease(self.elapsed / GO_PRONE)),
                max_speed: Some(0.0),
                no_jump: true,
                crouch: false,
            },
            Stance::Prone => SourceStance {
                height: Some(PRONE_HEIGHT),
                eye: Some(EYE_PRONE),
                max_speed: Some(if self.deployed { 0.0 } else { PRONE_SPEED }),
                no_jump: true,
                crouch: false,
            },
            Stance::GettingUp => {
                let r = rise(self.elapsed / GET_UP);
                let (eye, top, height) = if self.to_crouch {
                    (EYE_DUCK, RUN_SPEED * DUCK_FACTOR, Some(DUCK_HEIGHT))
                } else {
                    (EYE_STAND, RUN_SPEED, None)
                };
                SourceStance {
                    height,
                    eye: Some(EYE_PRONE + (eye - EYE_PRONE) * r),
                    max_speed: Some(PRONE_SPEED + (top - PRONE_SPEED) * r),
                    no_jump: true,
                    crouch: self.to_crouch,
                }
            }
        }
    }
}

/// The smallest turn from `a` to `b` (radians).
fn turn(a: f32, b: f32) -> f32 {
    use std::f32::consts::{PI, TAU};
    (b - a + PI).rem_euclid(TAU) - PI
}

/// Each DoD:S player's stance this tick, before Source's movement: the
/// presses of `+alt1` (prone, up), duck while prone (up to a crouch) and
/// the alt fire with a machine gun (deploy, undeploy), the transitions'
/// time, the tick's speed cap, and what the movement does with it
/// (`SourceStance`).
#[allow(clippy::type_complexity)]
pub fn step(
    mut players: Query<(
        &Intent,
        &mut DodsStance,
        &mut SourceStance,
        &SourceMovement,
        Option<&Inventory>,
        Option<&DodsStamina>,
        Option<&DodsSlowed>,
        Has<Dead>,
    )>,
    mut guns: Query<(&DodsMg, &mut WeaponState)>,
    shoulders: Query<&Shoulder>,
    registry: Option<Res<ActionRegistry>>,
    clock: Res<SimClock>,
) {
    let dt = clock.dt();
    for (intent, mut st, mut source, me, inv, stamina, slowed, dead) in &mut players {
        let mut s = *st;
        let mut last_rise = None;
        let prone_key = registry.as_deref().is_some_and(|r| intent.action(r, PRONE_ACTION));
        let pressed = prone_key && !s.prone_key;
        let alt_pressed = intent.secondary && !s.alt_key;
        let duck_pressed = intent.crouch && !s.duck_key;
        s.prone_key = prone_key;
        s.alt_key = intent.secondary;
        s.duck_key = intent.crouch;
        if dead {
            s = DodsStance { prone_key, alt_key: intent.secondary, duck_key: intent.crouch, ..default() };
        } else {
            match s.stance {
                Stance::Up => {
                    if pressed && me.on_ground && me.ladder.is_none() && me.water_level < 2 {
                        s.stance = Stance::GoingDown;
                        s.from_eye = me.eye;
                        // The press's tick is the first of the 80.
                        s.elapsed = dt;
                    }
                }
                Stance::GoingDown | Stance::GettingUp => {
                    s.elapsed += dt;
                }
                Stance::Prone => {
                    if pressed || duck_pressed {
                        s.stance = Stance::GettingUp;
                        s.to_crouch = !pressed;
                        s.elapsed = dt;
                    }
                }
            }
            // A transition ends on the tick its time is reached (the 80th).
            let ends = |t: f32| s.elapsed >= t - dt * 0.5;
            if s.stance == Stance::GettingUp && ends(GET_UP) {
                // Its last tick still eases (the eye at its end).
                last_rise = Some(DodsStance { elapsed: GET_UP, ..s });
            }
            if s.stance == Stance::GoingDown && ends(GO_PRONE) {
                s.stance = Stance::Prone;
                s.elapsed = 0.0;
            } else if s.stance == Stance::GettingUp && ends(GET_UP) {
                s.stance = Stance::Up;
                s.elapsed = 0.0;
                s.to_crouch = false;
            }
        }
        // The machine gun in hand: deployed by the alt fire while prone,
        // undeployed by it again or by turning past its limit.
        let active = inv.and_then(|i| i.active);
        match active.filter(|g| guns.contains(*g)) {
            Some(g) if s.prone() => {
                let Ok((mg, mut state)) = guns.get_mut(g) else { unreachable!() };
                let past_limit = s.deployed && turn(s.deploy_yaw, intent.yaw).abs() > mg.yaw_limit.to_radians();
                if alt_pressed || past_limit {
                    s.deployed = !s.deployed;
                    s.deploy_yaw = intent.yaw;
                    let wait = if s.deployed { mg.deploy_time } else { mg.undeploy_time };
                    let ready = clock.now + wait as f64;
                    if state.next_primary < ready {
                        state.next_primary = ready;
                    }
                }
            }
            _ => s.deployed = false,
        }
        if *st != s {
            *st = s;
        }
        // The cap standing up (module doc), from the stamina before this
        // tick's change (`stamina::step` runs after).
        let ducked = me.ducked || me.ducking || intent.crouch;
        let shouldered = active.and_then(|a| shoulders.get(a).ok()).is_some_and(|sh| sh.shouldered);
        let cap = if ducked {
            if shouldered { (Shoulder::MAX_SPEED / DUCK_FACTOR).min(RUN_SPEED) } else { RUN_SPEED }
        } else {
            let keys = stamina::sprint_keys(intent, me, Some(&s));
            let mut c = stamina::cap(stamina.map_or(stamina::MAX, |s| s.value), keys);
            if slowed.is_some_and(|sl| sl.at(clock.now)) {
                c = c.min(SLOWED_CAP);
            }
            if shouldered {
                c = c.min(Shoulder::MAX_SPEED);
            }
            c
        };
        let new = last_rise.unwrap_or(s).source(cap);
        if *source != new {
            *source = new;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// movement.md, "Prone": the eye after n ticks of going prone.
    #[test]
    fn the_ease_fits_the_measured_eye_heights() {
        let measured = [(1, 56.61), (2, 55.25), (9, 46.38), (19, 35.57), (29, 26.87), (39, 20.16), (49, 15.31), (59, 12.15), (69, 10.47), (79, 10.0)];
        for (n, eye) in measured {
            let ours = 58.0 + (10.0 - 58.0) * prone_ease(n as f32 / 80.0);
            assert!((ours - eye).abs() < 0.03, "tick {n}: {ours} against {eye}");
        }
    }

    /// movement.md, "Getting up": the cap rises from 50 to 220 (the
    /// measured speeds track it: 95.09 at tick 28, 172.11 at 52, 218.77 at
    /// 76, 220 at 80; a speed lags its rising cap by a little).
    #[test]
    fn getting_up_raises_the_cap_by_a_smoothstep() {
        let at = |k: f32| PRONE_SPEED + (RUN_SPEED - PRONE_SPEED) * rise(k / 80.0);
        assert!((at(1.0) - 50.08).abs() < 0.01);
        for (k, v) in [(52.0, 172.11), (64.0, 202.32), (76.0, 218.77), (80.0, 220.0)] {
            assert!((at(k) - v).abs() < 1.0, "tick {k}: {} against {v}", at(k));
        }
    }

    #[test]
    fn prone_holds_the_low_box_crawls_and_never_jumps() {
        let s = DodsStance { stance: Stance::Prone, ..default() };
        let src = s.source(220.0);
        assert_eq!((src.height, src.eye, src.max_speed, src.no_jump), (Some(24.0), Some(10.0), Some(50.0), true));
        let deployed = DodsStance { deployed: true, ..s };
        assert_eq!(deployed.source(220.0).max_speed, Some(0.0));
        assert_eq!(DodsStance::default().source(170.0), SourceStance { max_speed: Some(170.0), ..default() });
    }
}
