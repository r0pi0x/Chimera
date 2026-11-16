//! Downloads on demand, as the client's screens see them (the sandbox's
//! design, section 20: "a clickable size, picking auto-downloads"): the
//! seam between the screens and the asset side, which is built
//! separately.
//!
//! - **The interface** (`UnitFetcher`, in `UnitFetch`): `request_unit(game,
//!   unit)` asks for a unit of a game's files (a gun's model, sounds and
//!   data: what the asset service packs as one download); `retry` asks
//!   again after a failure; `poll` writes each unit's state into
//!   `UnitProgress` once a frame (`poll_units`, in `Update`).
//! - **The state** (`UnitProgress`, by `(game, unit)`): `UnitState::Here`,
//!   `NotHere` (its size, for the clickable size), `Queued`,
//!   `Downloading` (bytes done of total: the progress component, variant
//!   B) and `Failed` (a reason; R retries). A unit not listed is here (the
//!   base install's files need no download).
//! - **The fake** (`FakeFetcher`, `ui_scene` stagings and tests): units
//!   start not here at their listed sizes; a request queues one, one
//!   downloads at a time at `rate` bytes a second of the UI clock, then
//!   it's here; units in `fail` fail at half way. The real fetcher (the
//!   asset service's client) replaces it with `UnitFetch(Box::new(..))`.
//!
//! Known shortcuts: no cancel, no priority (a pick could jump the queue);
//! no app inserts a real fetcher yet, so outside the stagings every unit is here.

use std::collections::{BTreeMap, VecDeque};

use bevy::prelude::*;

/// A unit's state (module doc).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnitState {
    Here,
    NotHere { bytes: u64 },
    Queued { bytes: u64 },
    Downloading { done: u64, total: u64 },
    Failed { reason: String },
}

impl UnitState {
    /// Its share done, 0..1 (1 here).
    pub fn share(&self) -> f32 {
        match self {
            UnitState::Here => 1.0,
            UnitState::Downloading { done, total } => (*done as f64 / (*total).max(1) as f64) as f32,
            _ => 0.0,
        }
    }

    pub fn is_here(&self) -> bool {
        matches!(self, UnitState::Here)
    }
}

/// A size as the tiles show it ("6.4 MB").
pub fn size_words(bytes: u64) -> String {
    let mb = bytes as f64 / 1_000_000.0;
    if mb >= 10.0 { format!("{mb:.0} MB") } else { format!("{mb:.1} MB") }
}

/// Every unit's state (module doc), written by the fetcher's `poll`.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct UnitProgress(pub BTreeMap<(String, String), UnitState>);

impl UnitProgress {
    /// A unit's state (not listed: here).
    pub fn state(&self, game: &str, unit: &str) -> UnitState {
        self.0.get(&(game.to_string(), unit.to_string())).cloned().unwrap_or(UnitState::Here)
    }
}

/// The asset side's client, as the screens use it (module doc).
pub trait UnitFetcher: Send + Sync {
    /// Ask for `unit` of `game`'s files (queued behind others).
    fn request_unit(&mut self, game: &str, unit: &str);
    /// Ask again for a unit that failed.
    fn retry(&mut self, game: &str, unit: &str) {
        self.request_unit(game, unit);
    }
    /// Write every unit's state (once a frame; `now` the UI clock, s).
    fn poll(&mut self, now: f64, out: &mut UnitProgress);
}

/// The fetcher in use (module doc).
#[derive(Resource)]
pub struct UnitFetch(pub Box<dyn UnitFetcher>);

/// Ask for a unit (a screen's click, a pick of a gun not here).
#[derive(Message, Clone, Debug, PartialEq, Eq)]
pub enum UnitRequest {
    Fetch { game: String, unit: String },
    Retry { game: String, unit: String },
}

/// A fetcher where everything is here (no asset side).
pub struct AllHere;

impl UnitFetcher for AllHere {
    fn request_unit(&mut self, _: &str, _: &str) {}
    fn poll(&mut self, _: f64, out: &mut UnitProgress) {
        if !out.0.is_empty() {
            out.0.clear();
        }
    }
}

/// The staging fetcher (module doc).
#[derive(Clone, Debug, Default)]
pub struct FakeFetcher {
    pub units: BTreeMap<(String, String), UnitState>,
    queue: VecDeque<(String, String)>,
    /// Bytes a second.
    pub rate: f64,
    pub fail: Vec<(String, String)>,
    last: Option<f64>,
    carry: f64,
}

impl FakeFetcher {
    /// Units not here at their sizes, `rate` bytes a second.
    pub fn new(units: &[(&str, &str, u64)], rate: f64) -> Self {
        FakeFetcher {
            units: units.iter().map(|(g, u, b)| ((g.to_string(), u.to_string()), UnitState::NotHere { bytes: *b })).collect(),
            rate,
            ..default()
        }
    }

    /// A unit's state set by hand (a staging's frozen moment).
    pub fn set(&mut self, game: &str, unit: &str, s: UnitState) {
        self.units.insert((game.into(), unit.into()), s);
    }
}

impl UnitFetcher for FakeFetcher {
    fn request_unit(&mut self, game: &str, unit: &str) {
        let key = (game.to_string(), unit.to_string());
        let bytes = match self.units.get(&key) {
            Some(UnitState::NotHere { bytes }) => *bytes,
            Some(UnitState::Failed { .. }) => 4_000_000,
            _ => return,
        };
        self.fail.retain(|k| *k != key || !matches!(self.units.get(&key), Some(UnitState::Failed { .. })));
        self.units.insert(key.clone(), UnitState::Queued { bytes });
        self.queue.push_back(key);
    }

    fn poll(&mut self, now: f64, out: &mut UnitProgress) {
        let dt = self.last.map_or(0.0, |l| (now - l).max(0.0));
        self.last = Some(now);
        self.carry += dt * self.rate;
        // The head of the queue downloads.
        while let Some(key) = self.queue.front().cloned() {
            let st = self.units.get(&key).cloned();
            let (done, total) = match st {
                Some(UnitState::Queued { bytes }) => (0, bytes),
                Some(UnitState::Downloading { done, total }) => (done, total),
                _ => {
                    self.queue.pop_front();
                    continue;
                }
            };
            let step = self.carry.floor() as u64;
            self.carry -= step as f64;
            let done = (done + step).min(total);
            if self.fail.contains(&key) && done * 2 >= total {
                self.units.insert(key.clone(), UnitState::Failed { reason: "The server stopped answering.".into() });
                self.queue.pop_front();
                continue;
            }
            if done >= total {
                self.units.insert(key.clone(), UnitState::Here);
                self.queue.pop_front();
                continue;
            }
            self.units.insert(key, UnitState::Downloading { done, total });
            break;
        }
        if self.queue.is_empty() {
            self.carry = 0.0;
        }
        if out.0 != self.units {
            out.0 = self.units.clone();
        }
    }
}

/// Requests to the fetcher, then its states into `UnitProgress`.
pub fn poll_units(
    clock: Option<Res<chimera_ui::UiClock>>,
    fetch: Option<ResMut<UnitFetch>>,
    mut requests: MessageReader<UnitRequest>,
    mut progress: ResMut<UnitProgress>,
) {
    let Some(mut fetch) = fetch else {
        requests.clear();
        return;
    };
    for r in requests.read() {
        match r {
            UnitRequest::Fetch { game, unit } => fetch.0.request_unit(game, unit),
            UnitRequest::Retry { game, unit } => fetch.0.retry(game, unit),
        }
    }
    let now = clock.map_or(0.0, |c| c.now);
    let mut next = progress.clone();
    fetch.0.poll(now, &mut next);
    if *progress != next {
        *progress = next;
    }
}
