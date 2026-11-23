//! Being slowed when hit (specs/dods/damage.md, "Being slowed when hit";
//! movement.md): a hit by a DoD:S weapon (`dods:hit_slow`, `HitSlow`)
//! sets its victim's "slowed until" time (`DodsSlowed`, on every
//! character) ahead of now, 0.5 s for a hit on
//! the head, chest or arms and 2.0 s for a leg (`DodsSlowed`, measured);
//! while that time runs the victim's Source movement is held at
//! `SLOWED_SPEED` of its max speed (`SourceMovement::velocity_modifier`),
//! then climbs back as Source's movement lets it.
//!
//! Only hits the server deals count (`WeaponEventKind::Hit`, written for
//! damage dealt to the living), so a refused hit (a teammate's, god mode)
//! doesn't slow.
//!
//! Known shortcuts (docs/tech-debt.md):
//! - How much the victim is slowed is unmeasured (damage.md, open question
//!   5; movement.md, open question 5): `SLOWED_SPEED` is a stand-in.
//! - A new life within the time stays slowed for the rest of it.
//! - The slow is the server's: a predicting victim's own client learns it
//!   from its movement state's next correction, not before.

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
use source_engine::source_movement::SourceMovement;

use crate::core::{Health, Hitgroup, SimClock};

/// The fraction of its max speed a slowed victim moves at (a stand-in:
/// module doc).
pub const SLOWED_SPEED: f32 = 0.5;

/// `dods:hit_slow`'s parameters: how long a hit slows, seconds.
#[derive(Component, Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct HitSlow {
    /// A hit on anything but a leg.
    pub body: f32,
    /// A leg hit.
    pub legs: f32,
}

impl HitSlow {
    /// How long a hit on `group` slows.
    pub fn of(&self, group: Hitgroup) -> f32 {
        if matches!(group, Hitgroup::LeftLeg | Hitgroup::RightLeg) { self.legs } else { self.body }
    }
}

/// On every character (`CharacterPartsExt::on_every_character`: no
/// archetype per hit): slowed until this time (`SimClock`; never by
/// default).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct DodsSlowed {
    pub until: f64,
}

impl Default for DodsSlowed {
    fn default() -> Self {
        Self { until: f64::NEG_INFINITY }
    }
}

impl DodsSlowed {
    /// Whether the slow runs at `now`.
    pub fn at(&self, now: f64) -> bool {
        now < self.until
    }
}

/// Each hit by a DoD:S weapon on the living sets the victim's time
/// (a later hit's end replaces it).
pub fn on_hit(
    mut events: MessageReader<WeaponEvent>,
    slows: Query<&HitSlow>,
    living: Query<&Health>,
    clock: Res<SimClock>,
    mut slowed: Query<&mut DodsSlowed>,
) {
    for e in events.read() {
        let WeaponEventKind::Hit { target, hitgroup, .. } = e.kind else { continue };
        let Ok(slow) = slows.get(e.weapon) else { continue };
        if !living.get(target).is_ok_and(|h| h.current > 0.0) {
            continue;
        }
        if let Ok(mut s) = slowed.get_mut(target) {
            s.until = clock.now + slow.of(hitgroup) as f64;
        }
    }
}

/// While the time runs the victim's movement is held slow.
pub fn hold(mut slowed: Query<(&DodsSlowed, &mut SourceMovement)>, clock: Res<SimClock>) {
    for (s, mut m) in &mut slowed {
        if !s.at(clock.now) {
            continue;
        }
        if m.velocity_modifier > SLOWED_SPEED {
            m.velocity_modifier = SLOWED_SPEED;
        }
    }
}
