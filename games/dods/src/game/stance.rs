//! DoD:S's prone and its deployed machine guns (specs/dods/movement.md,
//! "Prone"; weapons.md, "Alt fire"), a DoD:S player's `DodsStance`, stepped
//! each tick before Source's movement (`step`, predicted) and handed to it
//! as a `SourceStance` (the box, the eye, the speed, no jump):
//!
//! - **Going prone** (`+alt1`, `PRONE_ACTION`, one press, on the ground):
//!   the eye eases from where it is to 10 over 1.2 s (80 ticks, `GO_PRONE`)
//!   by `prone_ease` (the measured heights: 56.61 after the press's tick,
//!   46.38 at 9, 26.87 at 29, 12.15 at 59, 10.00 at 79, within 0.03), no
//!   movement and no jump meanwhile, the standing box kept; on the 80th tick
//!   the box becomes 24 high (`PRONE_HEIGHT`) and the player is prone.
//! - **Prone**: eye 10, crawling at 50 units/s (`PRONE_SPEED`; the sprint
//!   key doesn't change it), no jump (public descriptions).
//! - **Deployed** (a machine gun in hand, `dods:mg`: `weapons::mg`): the
//!   alt fire while prone deploys it at once, its next shot no sooner than
//!   1.2 s later (`DodsMg::deploy_time`, measured); deployed, the player
//!   doesn't move, and the gun's spread is its sights' (`dods:gun`).
//!   Leaving prone, or another weapon in hand, undeploys it.
//! - **Getting up** (`+alt1` again): the eye eases back to the standing
//!   eye, the low box kept, no movement; then the box stands up where it
//!   fits (Source's movement keeps it ducked until it does).
//!
//! The stance goes on and off with `dods:movement` (`movement::put_on`,
//! `leave`), as its stamina does; a death drops it.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - Getting up is unmeasured (movement.md, open question 1): its time is
//!   a stand-in (`GET_UP`, the going-down time mirrored), with the same
//!   ease and no movement; what jump or duck do while prone, and a ceiling
//!   too low to get up, are unmeasured (kept prone: Source's ducked box
//!   until there is room).
//! - Going prone off the ground, on a ladder or in water is refused (our
//!   choice; unmeasured); begun ducked, the eye eases from the ducked eye.
//! - Undeploying is the alt fire again, at once (unmeasured: weapons.md,
//!   "Alt fire"); deploying on a ledge standing or crouched
//!   (`dod_bodyheightoffset`) and the deployed yaw limits are unmeasured
//!   and not done.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{Inventory, WeaponState};
use source_engine::source_movement::{SourceMovement, SourceStance};

use super::weapons::mg::DodsMg;
use crate::core::{ActionRegistry, Dead, Intent, SimClock};

/// The named action that goes prone and gets up (DoD:S's `+alt1`).
pub const PRONE_ACTION: &str = "alt1";

/// Going prone, seconds (movement.md: 80 ticks, measured).
pub const GO_PRONE: f32 = 1.2;

/// Getting up, seconds (a stand-in: module doc).
pub const GET_UP: f32 = 1.2;

/// The prone box's height and its eye, units (measured).
pub const PRONE_HEIGHT: f32 = 24.0;
pub const EYE_PRONE: f32 = 10.0;

/// Crawling speed, units/s (measured).
pub const PRONE_SPEED: f32 = 50.0;

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
    /// A machine gun deployed (only while prone).
    pub deployed: bool,
    /// `+alt1` and the alt fire as held last tick (presses act).
    pub prone_key: bool,
    pub alt_key: bool,
}

impl DodsStance {
    /// Whether the player is fully prone.
    pub fn prone(&self) -> bool {
        self.stance == Stance::Prone
    }

    /// What Source's movement does with it this tick (`eye_stand`: the
    /// standing eye a getting-up eases to).
    pub fn source(&self, eye_stand: f32) -> SourceStance {
        match self.stance {
            Stance::Up => SourceStance::default(),
            Stance::GoingDown => SourceStance {
                height: None,
                eye: Some(self.from_eye + (EYE_PRONE - self.from_eye) * prone_ease(self.elapsed / GO_PRONE)),
                max_speed: Some(0.0),
                no_jump: true,
            },
            Stance::Prone => SourceStance {
                height: Some(PRONE_HEIGHT),
                eye: Some(EYE_PRONE),
                max_speed: Some(if self.deployed { 0.0 } else { PRONE_SPEED }),
                no_jump: true,
            },
            Stance::GettingUp => SourceStance {
                height: Some(PRONE_HEIGHT),
                eye: Some(EYE_PRONE + (eye_stand - EYE_PRONE) * prone_ease(self.elapsed / GET_UP)),
                max_speed: Some(0.0),
                no_jump: true,
            },
        }
    }
}

/// Each DoD:S player's stance this tick, before Source's movement: the
/// presses of `+alt1` (prone, up) and of the alt fire with a machine gun
/// (deploy, undeploy), the transitions' time, and what the movement
/// does with it (`SourceStance`).
#[allow(clippy::type_complexity)]
pub fn step(
    mut players: Query<(&Intent, &mut DodsStance, &mut SourceStance, &SourceMovement, Option<&Inventory>, Has<Dead>)>,
    mut guns: Query<(&DodsMg, &mut WeaponState)>,
    registry: Option<Res<ActionRegistry>>,
    clock: Res<SimClock>,
) {
    let dt = clock.dt();
    for (intent, mut st, mut source, me, inv, dead) in &mut players {
        let mut s = *st;
        let prone_key = registry.as_deref().is_some_and(|r| intent.action(r, PRONE_ACTION));
        let pressed = prone_key && !s.prone_key;
        let alt_pressed = intent.secondary && !s.alt_key;
        s.prone_key = prone_key;
        s.alt_key = intent.secondary;
        if dead {
            s = DodsStance { prone_key, alt_key: intent.secondary, ..default() };
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
                Stance::GoingDown => {
                    s.elapsed += dt;
                }
                Stance::Prone => {
                    if pressed {
                        s.stance = Stance::GettingUp;
                        s.elapsed = dt;
                    }
                }
                Stance::GettingUp => {
                    s.elapsed += dt;
                }
            }
            // A transition ends on the tick its time is reached (the 80th).
            let ends = |t: f32| s.elapsed >= t - dt * 0.5;
            if s.stance == Stance::GoingDown && ends(GO_PRONE) {
                s.stance = Stance::Prone;
                s.elapsed = 0.0;
            } else if s.stance == Stance::GettingUp && ends(GET_UP) {
                s.stance = Stance::Up;
                s.elapsed = 0.0;
            }
        }
        // The machine gun in hand: deployed by the alt fire while prone.
        let gun = inv.and_then(|i| i.active).filter(|g| guns.contains(*g));
        match gun {
            Some(g) if s.prone() => {
                if alt_pressed {
                    s.deployed = !s.deployed;
                    if s.deployed
                        && let Ok((mg, mut state)) = guns.get_mut(g)
                    {
                        let ready = clock.now + mg.deploy_time as f64;
                        if state.next_primary < ready {
                            state.next_primary = ready;
                        }
                    }
                }
            }
            _ => s.deployed = false,
        }
        if *st != s {
            *st = s;
        }
        let new = s.source(super::movement::EYE_STAND);
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

    #[test]
    fn prone_holds_the_low_box_crawls_and_never_jumps() {
        let s = DodsStance { stance: Stance::Prone, ..default() };
        let src = s.source(58.0);
        assert_eq!((src.height, src.eye, src.max_speed, src.no_jump), (Some(24.0), Some(10.0), Some(50.0), true));
        let deployed = DodsStance { deployed: true, ..s };
        assert_eq!(deployed.source(58.0).max_speed, Some(0.0));
        assert_eq!(DodsStance::default().source(58.0), SourceStance::default());
    }
}
