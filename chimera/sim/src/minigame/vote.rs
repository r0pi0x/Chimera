//! The map vote on a minigame's match-end screen (a definition's
//! `map_vote`, `MapVoteSettings`; ours, neither game has one): when the
//! match is decided and its results are up, the host (a server or a
//! standalone game: `chimera_net::minigame_net`'s rotation) puts a few
//! candidate maps on the ballot (`candidates`: the rotation's own pick
//! first, then other maps of the pool every player can play, the ones
//! not played lately first, drawn on the host's `MapHistory` generator,
//! then this map if the pool is short), and the match-end hold
//! (`NextMatch`) waits for the vote: it closes `time` s after it opened,
//! then the winner shows for `reveal` s before the map changes. Every
//! player in the match may vote once and change their vote while it is
//! open (`MapVote::cast`; a network client sends its choice, the server
//! counts it); the map with the most votes wins, a tie is drawn at
//! random among the tied, and with no votes the rotation's pick (the
//! first candidate) stands (`MapVote::tally`). `MapVote` is the vote as
//! the host keeps it and, without the ballots, as a client knows it
//! (replicated with the match): the shell's match-end screen shows it
//! (`chimera_client::client::match_end`). Gone with the results when
//! the next match starts.
//!
//! Known shortcuts: bots don't vote; a player who leaves keeps their vote
//! counted.

use bevy::prelude::*;
use serde::Deserialize;

use super::{
    MinigameDef,
    auto::Rng,
    def::MapVoteParam,
};

/// The built-in settings (chimera/sim/data/map_vote.toml).
pub const DATA: &str = include_str!("../../data/map_vote.toml");

/// A definition's map vote settings (`map_vote`, over `DATA`).
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapVoteSettings {
    /// Maps on the ballot (at least 2).
    pub candidates: usize,
    /// How long the vote is open (s).
    #[serde(deserialize_with = "super::def::seconds")]
    pub time: f32,
    /// How long the winner shows before the change (s).
    #[serde(deserialize_with = "super::def::seconds")]
    pub reveal: f32,
}

impl MapVoteSettings {
    /// `DATA` with `t`'s keys in place.
    pub fn with(t: &toml::Table) -> Result<Self, String> {
        let mut base: toml::Table = toml::from_str(DATA).map_err(|e| e.to_string())?;
        for (k, v) in t {
            base.insert(k.clone(), v.clone());
        }
        let s: Self = toml::Value::Table(base).try_into().map_err(|e: toml::de::Error| e.to_string())?;
        if s.candidates < 2 {
            return Err(format!("candidates = {}: at least 2", s.candidates));
        }
        Ok(s)
    }

    /// `def`'s settings: None without a vote (`map_vote` absent or false,
    /// or `rotate = false`).
    pub fn of(def: &MinigameDef) -> Result<Option<Self>, String> {
        let s = match &def.map_vote {
            None | Some(MapVoteParam::On(false)) => return Ok(None),
            Some(MapVoteParam::On(true)) => Self::with(&toml::Table::new())?,
            Some(MapVoteParam::Tuned(t)) => Self::with(t)?,
        };
        Ok(def.rotate.then_some(s))
    }
}

/// The vote (the host's, with its ballots; a network client's from the
/// server, without).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MapVote {
    /// The maps on the ballot (`net::normalize_map`'s form), the
    /// rotation's pick first.
    pub candidates: Vec<String>,
    /// Votes per candidate.
    pub counts: Vec<u32>,
    /// The host's: each voter's character and choice.
    pub ballots: Vec<(Entity, usize)>,
    /// When it closes (`SimClock`).
    pub closes: f64,
    /// Once closed: the winning candidate.
    pub winner: Option<usize>,
}

impl MapVote {
    /// A vote on `candidates` closing at `closes`.
    pub fn new(candidates: Vec<String>, closes: f64) -> Self {
        Self {
            counts: vec![0; candidates.len()],
            candidates,
            ballots: Vec::new(),
            closes,
            winner: None,
        }
    }

    /// Whether it still takes votes.
    pub fn open(&self) -> bool {
        self.winner.is_none()
    }

    /// `voter` votes for candidate `choice` (its earlier vote replaced):
    /// false once closed or for no such candidate.
    pub fn cast(&mut self, voter: Entity, choice: usize) -> bool {
        if !self.open() || choice >= self.candidates.len() {
            return false;
        }
        match self.ballots.iter_mut().find(|(v, _)| *v == voter) {
            Some(b) => b.1 = choice,
            None => self.ballots.push((voter, choice)),
        }
        self.counts = vec![0; self.candidates.len()];
        for (_, c) in &self.ballots {
            self.counts[*c] += 1;
        }
        true
    }

    /// The winner: the most votes, a tie drawn on `rng` among the tied;
    /// no votes, the rotation's pick (0).
    pub fn tally(&self, rng: &mut Rng) -> usize {
        let best = self.counts.iter().copied().max().unwrap_or(0);
        if best == 0 {
            return 0;
        }
        let tied: Vec<usize> = (0..self.counts.len()).filter(|i| self.counts[*i] == best).collect();
        tied[rng.below(tied.len())]
    }

    /// Closes it (its `winner` from `tally`): the winning map.
    pub fn close(&mut self, rng: &mut Rng) -> String {
        let w = self.tally(rng);
        self.winner = Some(w);
        self.candidates[w].clone()
    }

    /// The winning map, once closed.
    pub fn winning_map(&self) -> Option<&str> {
        self.winner.and_then(|w| self.candidates.get(w)).map(String::as_str)
    }
}

/// The ballot: `first` (the rotation's pick), then up to `n` - 1 other
/// maps of `pool` that `playable` allows, the ones not in `recent` first,
/// each group shuffled on `rng`, then `here` if there is still room (a
/// short pool: playing it again is a choice).
pub fn candidates(
    first: &str,
    pool: &[String],
    here: &str,
    recent: &[String],
    n: usize,
    playable: &dyn Fn(&str) -> bool,
    rng: &mut Rng,
) -> Vec<String> {
    let mut out = vec![first.to_string()];
    let mut others: Vec<&String> = Vec::new();
    for m in pool {
        if m != first && m != here && playable(m) && !others.contains(&m) {
            others.push(m);
        }
    }
    let (mut fresh, mut stale): (Vec<&String>, Vec<&String>) = others.into_iter().partition(|m| !recent.contains(m));
    shuffle(&mut fresh, rng);
    shuffle(&mut stale, rng);
    for m in fresh.into_iter().chain(stale) {
        if out.len() >= n {
            break;
        }
        out.push(m.clone());
    }
    if out.len() < n && first != here && playable(here) {
        out.push(here.to_string());
    }
    out
}

fn shuffle<T>(items: &mut [T], rng: &mut Rng) {
    for i in (1..items.len()).rev() {
        items.swap(i, rng.below(i + 1));
    }
}

/// The map's name as a player reads it: its id without the game.
pub fn map_name(map: &str) -> &str {
    map.rsplit(':').next().unwrap_or(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pool(n: usize) -> Vec<String> {
        (0..n).map(|i| format!("g:m{i}")).collect()
    }

    #[test]
    fn the_ballot_starts_with_the_rotations_pick_and_prefers_fresh_maps() {
        let p = pool(6);
        let mut rng = Rng::new(3);
        let recent = vec!["g:m2".to_string(), "g:m3".to_string()];
        for _ in 0..20 {
            let c = candidates("g:m1", &p, "g:m0", &recent, 3, &|_| true, &mut rng);
            assert_eq!(c.len(), 3);
            assert_eq!(c[0], "g:m1");
            assert!(c[1..].iter().all(|m| ["g:m4", "g:m5"].contains(&m.as_str())), "{c:?}");
        }
        // A short pool: this map is a choice.
        let c = candidates("g:m1", &pool(2), "g:m0", &[], 3, &|_| true, &mut rng);
        assert_eq!(c, ["g:m1", "g:m0"]);
        // Unplayable maps never.
        let c = candidates("g:m1", &p, "g:m0", &[], 6, &|m| m != "g:m4", &mut rng);
        assert!(!c.contains(&"g:m4".to_string()) && c.len() == 5, "{c:?}");
    }

    #[test]
    fn the_most_votes_win_ties_are_random_and_no_votes_keep_the_rotation() {
        let (a, b, c) = (Entity::from_raw_u32(1).unwrap(), Entity::from_raw_u32(2).unwrap(), Entity::from_raw_u32(3).unwrap());
        let mut rng = Rng::new(1);
        let v = MapVote::new(pool(3), 10.0);
        assert_eq!(v.clone().close(&mut rng), "g:m0", "no votes: the rotation's pick");
        let mut v = MapVote::new(pool(3), 10.0);
        assert!(v.cast(a, 2) && v.cast(b, 2) && v.cast(c, 1));
        assert!(!v.cast(a, 3), "no such candidate");
        assert_eq!(v.counts, [0, 1, 2]);
        assert!(v.cast(b, 1), "a changed vote");
        assert_eq!(v.counts, [0, 2, 1]);
        assert_eq!(v.clone().close(&mut rng), "g:m1");
        // A tie: either, both over many draws.
        assert!(v.cast(a, 2));
        assert!(v.cast(c, 1) && v.cast(b, 2));
        v.cast(a, 1);
        assert_eq!(v.counts, [0, 2, 1]);
        v.cast(b, 0);
        v.cast(c, 2);
        assert_eq!(v.counts, [1, 1, 1]);
        let mut seen = [false; 3];
        for _ in 0..60 {
            seen[v.tally(&mut rng)] = true;
        }
        assert_eq!(seen, [true; 3], "a three-way tie draws each");
        let w = v.close(&mut rng);
        assert!(!v.open() && !v.cast(a, 0), "closed");
        assert_eq!(v.winning_map(), Some(w.as_str()));
    }

    #[test]
    fn settings_come_from_the_data_tuned_by_the_definition() {
        let s = MapVoteSettings::with(&toml::Table::new()).unwrap();
        assert_eq!((s.candidates, s.time, s.reveal), (3, 12.0, 3.0));
        let t: toml::Table = toml::from_str("candidates = 4\ntime = \"20s\"").unwrap();
        let s = MapVoteSettings::with(&t).unwrap();
        assert_eq!((s.candidates, s.time), (4, 20.0));
        let bad: toml::Table = toml::from_str("candidates = 1").unwrap();
        assert!(MapVoteSettings::with(&bad).is_err());
    }
}
