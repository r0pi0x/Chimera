//! A DoD:S player's stamina (specs/dods/movement.md, "Sprint and stamina";
//! weapons.md, "Alt fire"): a pool of 100 at spawn (measured), on with
//! `dods:movement` (`DodsStamina`). A machine gun fired undeployed costs
//! its `dods:mg` `stamina_per_shot` (15) a shot; the pool climbs back
//! `REGEN` a second to 100. The two numbers fit the measured run (.30 cal
//! held standing: 100 to 85 on the first shot, then 8.7 less a shot, 7
//! ticks apart: 15 − 7 × 0.9); deployed, no change (measured).
//!
//! Known shortcuts (docs/tech-debt.md):
//! - No sprint and no jump cost: their speed, drain, regeneration, delay
//!   and thresholds are unmeasured (movement.md, open questions 3 and 4;
//!   the probe's bot could not sprint or jump): nothing reads the pool yet
//!   but the tests and, later, the HUD (M3).
//! - The regeneration (0.9 a tick, `REGEN`) is the spec's fit of one
//!   measured run (weapons.md, open question 4), with no delay after a
//!   shot.
//! - The server's alone (not predicted or sent): nothing a client shows
//!   reads it yet.

use bevy::prelude::*;

use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};

use super::{stance::DodsStance, weapons::mg::DodsMg};
use crate::core::{Dead, SimClock};

/// The pool's size, and a fresh player's stamina (measured).
pub const MAX: f32 = 100.0;

/// Regeneration, stamina a second (0.9 a tick of 0.015 s; module doc).
pub const REGEN: f32 = 60.0;

/// A DoD:S player's stamina.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct DodsStamina(pub f32);

impl Default for DodsStamina {
    fn default() -> Self {
        Self(MAX)
    }
}

/// Each tick: the pool climbs back, then each machine-gun shot fired
/// undeployed takes its cost; the dead are full again.
pub fn tick(
    mut events: MessageReader<WeaponEvent>,
    mgs: Query<&DodsMg>,
    mut players: Query<(&mut DodsStamina, Option<&DodsStance>, Has<Dead>)>,
    clock: Res<SimClock>,
) {
    let dt = clock.dt();
    for (mut s, _, dead) in &mut players {
        let v = if dead { MAX } else { (s.0 + REGEN * dt).min(MAX) };
        if s.0 != v {
            s.0 = v;
        }
    }
    for e in events.read() {
        if e.replay || !matches!(e.kind, WeaponEventKind::Fired { .. }) {
            continue;
        }
        let Ok(mg) = mgs.get(e.weapon) else { continue };
        let Ok((mut s, stance, _)) = players.get_mut(e.owner) else { continue };
        if stance.is_some_and(|st| st.deployed) {
            continue;
        }
        s.0 = (s.0 - mg.stamina_per_shot).max(0.0);
    }
}
