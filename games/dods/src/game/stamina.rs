//! A DoD:S player's stamina and sprint (specs/dods/movement.md, "Stamina
//! and sprint"; weapons.md, "Alt fire"), on with `dods:movement`
//! (`DodsStamina`, predicted): a pool of 100 at spawn, stepped each tick
//! before Source's movement (`step`, after `stance::step` has read it for
//! the tick's speed cap):
//!
//! - **Sprinting** (`+speed`, `Intent::sprint`) needs forward input (alone
//!   or with a side key), standing (not ducked, duck not held, not in a
//!   prone stance) and, to start, a ground speed of at least 80 units/s
//!   on the previous tick (`SPRINT_MIN_SPEED`); stamina isn't needed.
//!   Starting costs 15 (`SPRINT_COST`) at once; sprinting faster than 100
//!   units/s (`DRAIN_ABOVE`, the previous tick's speed) drains 0.3 a tick
//!   (`DRAIN`, 20 a second), slower it regains the moving rate. In the air
//!   the sprint (and its drain) goes on as it was.
//! - **Regeneration** otherwise (no delay): 0.9 a tick still (standing,
//!   ducked, prone or in the air; still is a previous-tick ground speed of
//!   exactly 0: sliding to a stop regenerates at the moving rate), 0.75
//!   moving ducked, 0.15 moving standing or crawling (`REGEN_STILL`,
//!   `REGEN_DUCKED`, `REGEN_MOVING`; per second here, 60, 50 and 10), to
//!   100.
//! - **Going prone while sprinting** (movement.md, "Prone"): the sprint
//!   flag stays while the speed, decaying by friction, is still 80 units/s
//!   or more (`SPRINT_MIN_SPEED`), and drains as a sprint does; then it
//!   ends and the regeneration runs.
//! - **The speed cap** (`cap`, read by `stance::step` with the stamina as
//!   it was before the tick): running 120 + stamina (at most 220),
//!   sprinting 230 + stamina, 120 at stamina 0. Sprinting from a full
//!   pool the first ticks reach 48, 90, 127.2 units/s (`SPRINT_TOP`, 320:
//!   the wish they accelerate by, 48 = 10 × 320 × 0.015, before the cost
//!   brings the cap to 230 + 85); ducked and prone speeds don't depend on
//!   it.
//! - A machine gun fired undeployed costs its `dods:mg` `stamina_per_shot`
//!   (15) a shot (`mg_shots`; with the still rate the measured run: .30
//!   cal held standing, 100 to 85 on the first shot, then 8.7 less a shot,
//!   7 ticks apart); deployed, nothing (measured).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - `SPRINT_TOP` (320) is read off the measured first sprint ticks; the
//!   wish while the sprint keys are held below 80 units/s is ours.
//! - The machine gun's cost is the server's (the predicted pool learns it
//!   from the next correction); its regeneration after a shot follows the
//!   rates above (weapons.md, open question 4).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
use source_engine::source_movement::QuakeMovement;

use super::{
    movement::RUN_SPEED,
    stance::{DodsStance, Stance},
    weapons::mg::DodsMg,
};
use crate::core::{Dead, Intent, SimClock, Velocity};

/// The pool's size, and a fresh player's stamina (measured).
pub const MAX: f32 = 100.0;

/// What starting a sprint costs (measured).
pub const SPRINT_COST: f32 = 15.0;

/// The previous tick's ground speed a sprint needs to start, units/s.
pub const SPRINT_MIN_SPEED: f32 = 80.0;

/// Sprinting faster than this (units/s) drains.
pub const DRAIN_ABOVE: f32 = 100.0;

/// Drain and regeneration, stamina a second (0.3, 0.9, 0.75 and 0.15 a
/// tick of 0.015 s).
pub const DRAIN: f32 = 20.0;
pub const REGEN_STILL: f32 = 60.0;
pub const REGEN_DUCKED: f32 = 50.0;
pub const REGEN_MOVING: f32 = 10.0;

/// The caps' bases (units/s): running 120 + stamina, sprinting 230 +
/// stamina, at most `SPRINT_TOP`; sprinting empty 120.
pub const RUN_CAP: f32 = 120.0;
pub const SPRINT_CAP: f32 = 230.0;
pub const SPRINT_TOP: f32 = 320.0;
pub const EMPTY_CAP: f32 = 120.0;

const U: f32 = 0.0254;

/// A DoD:S player's stamina, and whether it sprints.
#[derive(Component, Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct DodsStamina {
    pub value: f32,
    pub sprinting: bool,
}

impl Default for DodsStamina {
    fn default() -> Self {
        Self { value: MAX, sprinting: false }
    }
}

/// The speed cap (units/s) at `stamina` with the sprint keys held or not.
pub fn cap(stamina: f32, sprint_keys: bool) -> f32 {
    if !sprint_keys {
        (RUN_CAP + stamina).min(RUN_SPEED)
    } else if stamina > 0.0 {
        (SPRINT_CAP + stamina).min(SPRINT_TOP)
    } else {
        EMPTY_CAP
    }
}

/// Whether the keys and the stance ask for a sprint: `+speed` with forward
/// input, standing up, duck neither held nor in effect.
pub fn sprint_keys(intent: &Intent, me: &QuakeMovement, stance: Option<&DodsStance>) -> bool {
    intent.sprint
        && intent.move_axis.y > 0.0
        && !intent.crouch
        && !me.ducked
        && !me.ducking
        && stance.is_none_or(|s| s.stance == Stance::Up)
}

/// Each tick, before Source's movement: the sprint's start and end, the
/// drain or the regeneration (from the previous tick's speed); the dead
/// are full again.
#[allow(clippy::type_complexity)]
pub fn step(
    mut players: Query<(&Intent, &mut DodsStamina, &QuakeMovement, Option<&DodsStance>, Option<&Velocity>, Has<Dead>)>,
    clock: Res<SimClock>,
) {
    let dt = clock.dt();
    for (intent, mut s, me, stance, velocity, dead) in &mut players {
        let new = if dead {
            DodsStamina::default()
        } else {
            let prev = velocity.map_or(0.0, |v| v.0.with_y(0.0).length() / U);
            let keys = sprint_keys(intent, me, stance);
            let mut n = *s;
            let mut started = false;
            let going_down = stance.is_some_and(|s| s.stance == Stance::GoingDown);
            if going_down {
                // The flag stays while the speed is a sprint's.
                n.sprinting &= prev >= SPRINT_MIN_SPEED;
            } else if me.on_ground {
                if keys && (n.sprinting || prev >= SPRINT_MIN_SPEED) {
                    if !n.sprinting {
                        n.sprinting = true;
                        n.value = (n.value - SPRINT_COST).max(0.0);
                        started = true;
                    }
                } else {
                    n.sprinting = false;
                }
            }
            if !started {
                let rate = if n.sprinting {
                    if prev > DRAIN_ABOVE || going_down { -DRAIN } else { REGEN_MOVING }
                } else if !me.on_ground || prev == 0.0 {
                    REGEN_STILL
                } else if stance.is_some_and(|s| s.stance != Stance::Up) {
                    REGEN_MOVING
                } else if me.ducked || me.ducking {
                    REGEN_DUCKED
                } else {
                    REGEN_MOVING
                };
                n.value = (n.value + rate * dt).clamp(0.0, MAX);
            }
            n
        };
        if *s != new {
            *s = new;
        }
    }
}

/// Each machine-gun shot fired undeployed takes its cost (the server's,
/// after the weapons).
pub fn mg_shots(mut events: MessageReader<WeaponEvent>, mgs: Query<&DodsMg>, mut players: Query<(&mut DodsStamina, Option<&DodsStance>)>) {
    for e in events.read() {
        if e.replay || !matches!(e.kind, WeaponEventKind::Fired { .. }) {
            continue;
        }
        let Ok(mg) = mgs.get(e.weapon) else { continue };
        let Ok((mut s, stance)) = players.get_mut(e.owner) else { continue };
        if stance.is_some_and(|st| st.deployed) {
            continue;
        }
        s.value = (s.value - mg.stamina_per_shot).max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// movement.md, "Stamina and sprint": the caps.
    #[test]
    fn the_caps_follow_the_stamina() {
        assert_eq!(cap(100.0, false), 220.0);
        assert_eq!(cap(90.0, false), 210.0);
        assert_eq!(cap(10.0, false), 130.0);
        assert_eq!(cap(90.0, true), 320.0);
        assert_eq!(cap(50.0, true), 280.0);
        assert_eq!(cap(85.0, true), 315.0);
        assert_eq!(cap(0.0, true), 120.0);
        assert_eq!(cap(100.0, true), 320.0);
    }
}
