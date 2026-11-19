//! The last hits on the server, for a feedback report (docs/plans/active/
//! feedback-reports.md): `RecentHits`, a ring of at most `CAP` short lines,
//! one per damage that landed (`core::DamageTaken`, with the target's
//! health after it) and per death (`core::Died`), and the lines a game adds
//! for a hit that did nothing (`RecentHits::note`: Halo's damage seam, a
//! body immune to an effect). The client's report puts them in `game.txt`
//! ("recent hits"): a listen host's report shows what its bullets did,
//! which a "can't kill X with Y" report needs. Entities are named by index
//! and generation and their game's body id when the identity says it,
//! never by a player's name.

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::core::{DamageTaken, Died, Health, SimClock};

/// Lines kept.
pub const CAP: usize = 48;

/// The ring (module doc).
#[derive(Resource, Clone, Debug, Default)]
pub struct RecentHits {
    lines: VecDeque<String>,
}

impl RecentHits {
    /// Add a line at engine tick `tick` (the oldest goes past `CAP`).
    pub fn note(&mut self, tick: u64, line: impl AsRef<str>) {
        if self.lines.len() >= CAP {
            self.lines.pop_front();
        }
        let mut l: String = line.as_ref().chars().take(200).collect();
        l.insert_str(0, &format!("t{tick} "));
        self.lines.push_back(l);
    }

    /// The lines, oldest first.
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.lines.iter().map(|l| l.as_str())
    }

    /// The report's section ("" when empty).
    pub fn text(&self) -> String {
        if self.lines.is_empty() {
            return String::new();
        }
        let mut out = String::from("recent hits (server, oldest first):\n");
        for l in &self.lines {
            out += "  ";
            out += l;
            out += "\n";
        }
        out
    }
}

/// An entity as the lines name it.
pub fn who(e: Option<Entity>) -> String {
    e.map_or("world".into(), |e| format!("{e}"))
}

/// The ring and its recorder on a simulation (`chimera_sim::
/// SimBasePlugins` adds it; the core's damage can't name this module).
pub struct RecentHitsPlugin;

impl Plugin for RecentHitsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecentHits>().add_systems(
            FixedUpdate,
            record.after(crate::core::apply_damage).run_if(crate::core::authoritative),
        );
    }
}

/// Record the tick's landed damage and deaths (after `core::apply_damage`).
pub fn record(
    mut taken: MessageReader<DamageTaken>,
    mut died: MessageReader<Died>,
    health: Query<&Health>,
    clock: Res<SimClock>,
    mut hits: ResMut<RecentHits>,
) {
    for t in taken.read() {
        let d = &t.damage;
        let after = health.get(d.target).map_or(-1.0, |h| h.current);
        hits.note(
            clock.tick,
            format!(
                "{} -> {}: {} {:?} {:?} amount {:.3}, health after {:.3}",
                who(d.attacker),
                who(Some(d.target)),
                d.weapon.unwrap_or("-"),
                d.kind,
                d.hitgroup,
                d.amount,
                after
            ),
        );
    }
    for d in died.read() {
        hits.note(clock.tick, format!("{} died, killer {}", who(Some(d.entity)), who(d.attacker)));
    }
}
