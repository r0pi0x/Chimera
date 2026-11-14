//! A match's temporary effects (docs/plans/active/ugg-ui.md, M3): what a
//! host's surprise (an air strike, moon gravity) or any other rule puts on
//! everyone for a while, as data the shell draws (`chimera_client::client::
//! match_effects`: under the Signal look a banner folding into a chip in
//! the top-left stack and, for a threat, an edge pulse; otherwise a plain
//! banner), never as HUD text. `MatchEffects` lists them; the authority
//! writes it (`start`, `end`, `prune`), `chimera_net::minigame_net`
//! replicates it with the match (times as server ticks, turned into the
//! client's clock).
//!
//! Times are the simulation's (`SimClock::now`, seconds).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// One effect (module doc).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    /// Its number, unique on the server for its run: a client tells a new
    /// one from one it shows.
    pub id: u32,
    /// What it is (`air_strike`, `moon_gravity`): a client's look may key
    /// on it; the text below says it anyway.
    pub kind: String,
    /// Its name ("Air strike").
    pub title: String,
    /// What to do or what it does ("Take cover", "Jump higher • fall
    /// slower").
    pub detail: String,
    /// The last seconds' line in the plain look, `{n}` the seconds left
    /// ("Gravity returns in {n}"); empty: none.
    pub countdown: String,
    /// The plain look's line at its natural end ("Gravity is back!");
    /// empty: none.
    pub over: String,
    /// A danger (red, the edge pulse, the warning mark).
    pub threat: bool,
    /// The plain look's colour (display-space RGB).
    pub color: [u8; 3],
    pub starts: f64,
    /// When it strikes (a countdown to it), if it does.
    pub impact_at: Option<f64>,
    pub ends: f64,
    /// Ended early (`MatchEffects::end`): no line for its natural end.
    pub stopped: bool,
}

/// The effects on now (module doc), oldest first.
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MatchEffects {
    pub effects: Vec<Effect>,
    /// The last id given.
    pub last_id: u32,
}

/// How long an effect stays listed after its end (s): a client sees it
/// end on its own clock before it goes.
pub const LINGER: f64 = 1.0;

impl MatchEffects {
    /// Puts `e` on with a new id (in place of one of its kind still on).
    pub fn start(&mut self, mut e: Effect) -> u32 {
        self.effects.retain(|x| x.kind != e.kind);
        self.last_id = self.last_id.wrapping_add(1).max(1);
        e.id = self.last_id;
        self.effects.push(e);
        self.last_id
    }

    /// Ends the effect of `kind` at `now` (stopped early: listed until
    /// `LINGER` after).
    pub fn end(&mut self, kind: &str, now: f64) {
        for e in self.effects.iter_mut().filter(|e| e.kind == kind) {
            if now < e.ends {
                e.ends = now;
                e.stopped = true;
            }
            if let Some(i) = e.impact_at.as_mut() {
                *i = i.min(now);
            }
        }
    }

    /// Drops the effects over for `LINGER`; true if any went.
    pub fn prune(&mut self, now: f64) -> bool {
        let n = self.effects.len();
        self.effects.retain(|e| e.ends + LINGER > now);
        self.effects.len() != n
    }

    pub fn get(&self, kind: &str) -> Option<&Effect> {
        self.effects.iter().find(|e| e.kind == kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effects_start_end_and_prune() {
        let mut m = MatchEffects::default();
        let a = m.start(Effect { kind: "air_strike".into(), starts: 0.0, impact_at: Some(3.0), ends: 11.0, ..default() });
        let b = m.start(Effect { kind: "moon_gravity".into(), starts: 1.0, ends: 61.0, ..default() });
        assert_ne!(a, b);
        // Again: in place of the one on, a new id.
        let c = m.start(Effect { kind: "air_strike".into(), starts: 2.0, impact_at: Some(5.0), ends: 13.0, ..default() });
        assert_eq!(m.effects.len(), 2);
        assert!(c > b);
        m.end("moon_gravity", 4.0);
        assert_eq!(m.get("moon_gravity").unwrap().ends, 4.0);
        assert!(m.get("moon_gravity").unwrap().stopped);
        assert!(!m.prune(4.5), "listed a while after its end");
        assert!(m.prune(5.1));
        assert!(m.get("moon_gravity").is_none() && m.get("air_strike").is_some());
    }
}
