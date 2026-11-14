//! Each player's climb through a match (the match-end screen's climb
//! chart, docs/plans/active/ugg-ui.md M4): when their rung changed, and
//! when they killed and died, in seconds from the match going live (the
//! warm-up not counted). Recorded on the authoritative side while the
//! match counts (`mode::Match::counting`): `track_rungs` (a `Rung` that
//! changed, a step to the new rung, up or down), `on_death` (the playing
//! mode's `ModeDeath`: a kill for the attacker, unless it was its own
//! death, and a death for the victim, the last victim's name kept for the
//! winner's "final kill"). `ClimbLog` is cleared when a match starts
//! (`results::on_step`); `results::collect` puts each player's `Climb`
//! on its `ResultRow` and the match's length on `MatchResults`, which a
//! server replicates with the results.
//!
//! Known shortcuts: a player who leaves takes their climb with them (rows
//! are collected from the players present); team kills count as kills.

use std::collections::BTreeMap;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{ActiveMinigame, ladder::Rung, runtime::InMinigame};
use crate::{core::SimClock, mode::Match};

/// One player's climb (module doc). Times are seconds from the match
/// going live.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Climb {
    /// Each rung change: when, and the rung reached (counted from 1).
    /// Before the first, the player was on rung 1.
    pub steps: Vec<(f32, u16)>,
    pub kills: Vec<f32>,
    pub deaths: Vec<f32>,
    /// The name of the last player this one killed.
    pub last_victim: Option<String>,
}

impl Climb {
    /// The rung (from 1) at `t` s.
    pub fn rung_at(&self, t: f32) -> u16 {
        self.steps.iter().take_while(|(at, _)| *at <= t).last().map_or(1, |(_, r)| *r)
    }

    /// The longest time spent on one rung up to `end` s: (from, to, the
    /// rung). None for an empty match.
    pub fn longest_stall(&self, end: f32) -> Option<(f32, f32, u16)> {
        let mut at = 0.0;
        let mut rung = 1;
        let mut best: Option<(f32, f32, u16)> = None;
        for &(t, r) in self.steps.iter().chain(std::iter::once(&(end, 0))) {
            let t = t.min(end);
            if best.is_none_or(|(a, b, _)| t - at > b - a) && t > at {
                best = Some((at, t, rung));
            }
            at = t;
            rung = r;
        }
        best
    }
}

/// The match's climbs as they are recorded (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ClimbLog {
    /// When the match went live (`SimClock`); none before.
    pub start: Option<f64>,
    pub players: BTreeMap<Entity, Climb>,
}

impl ClimbLog {
    /// Seconds into the match at `now` (0 before it went live).
    pub fn at(&self, now: f64) -> f32 {
        self.start.map_or(0.0, |s| (now - s).max(0.0) as f32)
    }

    /// The match's start, from `m` once it counts.
    fn started(&mut self, m: &Match) -> bool {
        if !m.counting() {
            return false;
        }
        if self.start.is_none() {
            self.start = Some(m.live_since);
        }
        true
    }
}

/// A step a player whose `Rung` changed while the match counts (on the
/// authoritative side, with the runtime's rules).
pub(super) fn track_rungs(
    rungs: Query<(Entity, &Rung), (Changed<Rung>, With<InMinigame>)>,
    game: Res<Match>,
    clock: Res<SimClock>,
    mut log: ResMut<ClimbLog>,
) {
    if rungs.is_empty() || !log.started(&game) {
        return;
    }
    let t = log.at(clock.now);
    for (e, r) in &rungs {
        let rung = r.0 as u16 + 1;
        let climb = log.players.entry(e).or_default();
        let last = climb.steps.last().map_or(1, |(_, r)| *r);
        if last != rung {
            climb.steps.push((t, rung));
        }
    }
}

/// A death while the match counts: the victim's death and the attacker's
/// kill (module doc).
pub(super) fn on_death(
    death: On<crate::mode::ModeDeath>,
    active: Option<Res<ActiveMinigame>>,
    game: Option<Res<Match>>,
    clock: Option<Res<SimClock>>,
    names: Query<&Name>,
    mut log: ResMut<ClimbLog>,
) {
    let (Some(_), Some(game), Some(clock)) = (active, game, clock) else { return };
    if !log.started(&game) {
        return;
    }
    let t = log.at(clock.now);
    let d = &death.died;
    log.players.entry(d.entity).or_default().deaths.push(t);
    if let Some(k) = d.attacker.filter(|k| *k != d.entity) {
        let victim = names.get(d.entity).ok().map(|n| n.as_str().to_string());
        let climb = log.players.entry(k).or_default();
        climb.kills.push(t);
        if victim.is_some() {
            climb.last_victim = victim;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_stall_and_the_rung_at_a_time() {
        let c = Climb { steps: vec![(10.0, 2), (40.0, 3), (45.0, 2), (100.0, 3)], ..default() };
        assert_eq!(c.rung_at(0.0), 1);
        assert_eq!(c.rung_at(42.0), 3);
        assert_eq!(c.rung_at(50.0), 2);
        assert_eq!(c.longest_stall(120.0), Some((45.0, 100.0, 2)));
        assert_eq!(Climb::default().longest_stall(30.0), Some((0.0, 30.0, 1)));
        assert_eq!(Climb::default().longest_stall(0.0), None);
    }
}
