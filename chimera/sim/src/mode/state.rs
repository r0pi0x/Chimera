//! The match a mode plays (lifted from Combat Arms' `CaMatch`,
//! specs/combat_arms/game_modes.md S3, S7): its phase and clocks, the two
//! sides' counters, the round results and the match's result (`Match`);
//! the team ids every mode shares (sides 1 and 2, free-for-all players'
//! own ids, observers); a death as a mode counts it (`Death`, `classify`).
//! The server's; a game sends what its clients show of it.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// The first and second sides (Combat Arms' Alpha and Bravo; CS:S's
/// terrorists and counter-terrorists are the same ids).
pub const ALPHA: u8 = 1;
pub const BRAVO: u8 = 2;

/// A team id's index in the two sides' counters (`ALPHA` 0, `BRAVO` 1);
/// None for anything else (no team, observers, a free-for-all player).
pub fn side(team: u8) -> Option<usize> {
    match team {
        ALPHA => Some(0),
        BRAVO => Some(1),
        _ => None,
    }
}

/// The other side of `ALPHA` or `BRAVO`.
pub fn enemy(team: u8) -> u8 {
    if team == ALPHA { BRAVO } else { ALPHA }
}

/// The first and last team ids a free-for-all player is given (every
/// player a team of their own, the lowest free id from 3).
pub const FFA_FIRST: u8 = 3;
pub const FFA_LAST: u8 = 254;

/// A team id that plays: a side or a free-for-all player's own; not 0
/// (none) or 255 (observers).
pub fn plays(team: u8) -> bool {
    (ALPHA..=FFA_LAST).contains(&team)
}

/// The match's phases. A mission start and a round end are steps between
/// them (`flow`), not phases of their own.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Phase {
    /// The map is loaded, nobody has come in yet.
    #[default]
    Waiting,
    /// The first player came in: the mission starts at the next frame.
    Starting,
    /// Round setup: everyone respawned and frozen until `until`.
    Setup { until: f64 },
    /// Kills count; the round timer runs (`Match::round_ends`).
    Live,
    /// A winner (or the time-out) ended the round; players keep playing
    /// but nothing counts until `until`.
    RoundOver { until: f64 },
    /// After a round's end, until the next mission start or the match's
    /// result.
    Intermission { until: f64 },
    /// The match's result is in.
    MatchOver,
}

/// One round's result.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct RoundResult {
    /// The round's number (rounds played before it, the warm-up not
    /// counted; with `FlowOptions::count_at_start`, its own, from 1).
    pub round: u32,
    pub warmup: bool,
    /// The winner's team id; None: no winner (a draw, the warm-up).
    pub winner: Option<u8>,
    /// The sides' counters when it ended.
    pub score: [i32; 2],
}

/// The match (the server's).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Match {
    pub phase: Phase,
    /// The round under way is the warm-up.
    pub warmup: bool,
    /// Rounds played (the warm-up not counted); with
    /// `FlowOptions::count_at_start`, rounds started.
    pub round: u32,
    /// Live: when the round timer reaches 0 (game seconds); None: no
    /// timer.
    pub round_ends: Option<f64>,
    /// When the round under way went live (game seconds).
    pub live_since: f64,
    /// The round's winner so far (team id).
    pub winner: Option<u8>,
    /// A rule asked for the round's end (the next frame's step enters
    /// round over).
    pub end_requested: bool,
    /// The counter the mode decides by (kills, captures, points), per
    /// side.
    pub score: [i32; 2],
    /// Round wins per side.
    pub round_wins: [u32; 2],
    /// This round's enemy kills and deaths per side (time-out
    /// tie-breaks).
    pub round_kills: [u32; 2],
    pub round_deaths: [u32; 2],
    /// The match's enemy kills, deaths and flag returns per side (the
    /// match's tie-breaks).
    pub kills: [u32; 2],
    pub deaths: [u32; 2],
    pub returns: [u32; 2],
    /// The round ends without a winner whatever the time-out rule says (a
    /// void round).
    pub void: bool,
    /// The match's result, once decided: Some(winner), Some(None) a draw.
    pub result: Option<Option<u8>>,
    /// The round results so far, in order.
    pub results: Vec<RoundResult>,
    /// The warm-up's next revive.
    pub next_revive: f64,
    /// Live: the round's result is decided and the round over waits until
    /// then; the timer no longer ends it.
    pub hold_until: Option<f64>,
    /// Living players take no damage (a decided round's hold).
    pub immune: bool,
    /// The next mission start plays a warm-up round (a side swap, a half
    /// time).
    pub warmup_next: bool,
}

impl Match {
    /// Kills count now: only in the live phase, never in a warm-up.
    pub fn counting(&self) -> bool {
        self.phase == Phase::Live && !self.warmup
    }

    /// Ask for the round's end with `winner` (a win condition): the next
    /// frame's step enters round over.
    pub fn win(&mut self, winner: u8) {
        self.winner = Some(winner);
        self.end_requested = true;
    }

    /// Decide the round for `winner` now and end it after `hold` s from
    /// `now` (the round stays live meanwhile: kills count, the winner
    /// stays).
    pub fn win_after(&mut self, winner: u8, now: f64, hold: f64) {
        self.winner = Some(winner);
        self.hold_until = Some(now + hold);
    }

    /// The round's result is decided (a winner set, an end asked for or
    /// held).
    pub fn decided(&self) -> bool {
        self.winner.is_some() || self.end_requested || self.hold_until.is_some()
    }

    /// Ask for the round's end with no winner.
    pub fn end_without_winner(&mut self) {
        self.winner = None;
        self.void = true;
        self.end_requested = true;
    }
}

/// How a death came about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathKind {
    /// An enemy's damage.
    Enemy,
    /// A teammate's.
    TeamKill,
    /// The victim's own damage (its grenade, a telefrag).
    Suicide,
    /// No player's (a fall, the world).
    NoKiller,
}

/// A death the mode counts (`Mode::on_death`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Death {
    pub victim: Entity,
    pub victim_team: u8,
    pub killer: Option<Entity>,
    pub kind: DeathKind,
    /// The weapon that dealt the killing damage (the damage's own, else
    /// what the killer held when it struck: a thrown grenade's, not the
    /// knife the killer holds by now).
    pub weapon: Option<&'static str>,
    /// How the killing damage was dealt.
    pub damage_kind: crate::core::DamageKind,
}

/// Classify a death of a player on `victim_team` by `killer` (on
/// `killer_team`): the victim's own damage is a suicide unless it was a
/// `fall`'s.
pub fn classify(victim: Entity, killer: Option<Entity>, killer_team: Option<u8>, victim_team: u8, fall: bool) -> DeathKind {
    match killer {
        None => DeathKind::NoKiller,
        Some(k) if k == victim => {
            if fall {
                DeathKind::NoKiller
            } else {
                DeathKind::Suicide
            }
        }
        Some(_) if killer_team == Some(victim_team) => DeathKind::TeamKill,
        Some(_) => DeathKind::Enemy,
    }
}
