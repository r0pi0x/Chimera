//! What a client fetched on demand while a match ran (the match's files
//! should all come with the join's or level change's download: the
//! offered sets, `net::assets`). Each on-demand fetch of the asset store
//! (`chimera_assets::cached::on_demand_since`: a file a loader read that
//! the store lacked, fetched with its group) is taken every frame and
//! counted against the match on screen: from the moment the client is in
//! the game with no pre-match lobby showing (`JoinStage::Joined`, no
//! `LobbyShowing`: `prematch` keeps it while its `NetLobby` exists) until
//! it leaves it (a level change, a disconnect, the next match's lobby).
//! On the console and the log (stderr: a tester's `ugg-last.log`):
//!
//! - each fetch during the match, with its path, set, records and stored
//!   bytes, and the match's running total:
//!   `assets: fetched on demand during the match on <map>: <game> <path> (set <set>, 3 files, 120 KB); this match 2 fetches, 0.31 MB`;
//! - when the match starts, what was fetched on demand while joining or
//!   loading it:
//!   `assets: the match on <map> started (4 fetches on demand, 1.20 MB, while joining or loading it)`;
//! - when it ends, its total, zero included:
//!   `assets: the match on <map> ended: 0 fetches on demand during it (0.00 MB)`.
//!
//! `OnDemandCount` holds the current match's (tests). A client with the
//! installs fetches nothing and prints only the totals.
//!
//! Known shortcuts: the fetches are the process's, so an app counts
//! every app's in the same process (tests with several clients); one made
//! on a loader thread is counted at the next frame's end, against the
//! match then on screen.

use bevy::prelude::*;
use chimera_assets::cached::{self, OnDemand};

use super::client::{JoinProgress, JoinStage, Joined};
use crate::core::NetRole;

/// A pre-match lobby shows: the match hasn't started (the lobby's own
/// layer keeps it: `prematch`, while its `NetLobby` exists).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LobbyShowing(pub bool);

/// The on-demand fetches of the match on screen (`map` Some while it
/// runs) and those made since the last match while joining or loading.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct OnDemandCount {
    /// The match's map while one runs.
    pub map: Option<String>,
    /// This match's fetches, oldest first.
    pub during: Vec<OnDemand>,
    /// Fetches outside a match since the last one ended (joining, loading).
    pub outside: Vec<OnDemand>,
    /// Where this app is in the process's fetches (`on_demand_since`).
    cursor: u64,
}

impl OnDemandCount {
    /// Fetches and stored bytes of `list`.
    pub fn total(list: &[OnDemand]) -> (usize, u64) {
        (list.len(), list.iter().map(|f| f.stored).sum())
    }
}

pub(super) fn plugin(app: &mut App) {
    app.insert_resource(OnDemandCount { cursor: cached::on_demand_end(), ..default() })
        .init_resource::<LobbyShowing>()
        .add_systems(Last, count.run_if(resource_equals(NetRole::Client)));
}

fn mb(bytes: u64) -> f64 {
    bytes as f64 / 1e6
}

fn count(
    progress: Option<Res<JoinProgress>>,
    joined: Option<Res<Joined>>,
    lobby: Res<LobbyShowing>,
    mut c: ResMut<OnDemandCount>,
) {
    // The match on screen: in the game, past the join, no lobby showing.
    let in_game = progress.is_some_and(|p| p.stage == JoinStage::Joined);
    let running = joined.map(|j| j.map.clone()).filter(|_| in_game && !lobby.0);
    let fetched = cached::on_demand_since(&mut c.cursor);
    if c.map.is_some() && c.map != running {
        let (n, bytes) = OnDemandCount::total(&c.during);
        eprintln!(
            "assets: the match on {} ended: {n} fetches on demand during it ({:.2} MB)",
            c.map.as_deref().unwrap_or_default(),
            mb(bytes)
        );
        c.map = None;
        c.during.clear();
    }
    if c.map.is_none()
        && let Some(map) = running
    {
        let (n, bytes) = OnDemandCount::total(&c.outside);
        eprintln!("assets: the match on {map} started ({n} fetches on demand, {:.2} MB, while joining or loading it)", mb(bytes));
        c.outside.clear();
        c.map = Some(map);
    }
    for f in fetched {
        if let Some(map) = c.map.clone() {
            c.during.push(f.clone());
            let (n, bytes) = OnDemandCount::total(&c.during);
            eprintln!(
                "assets: fetched on demand during the match on {map}: {} {} (set {}, {} files, {:.0} KB); this match {n} fetches, {:.2} MB",
                f.game,
                f.path,
                f.set.as_deref().unwrap_or("-"),
                f.records,
                f.stored as f64 / 1e3,
                mb(bytes)
            );
        } else {
            c.outside.push(f);
        }
    }
}
