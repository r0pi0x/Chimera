//! The startup fetch (docs/plans/active/asset-service.md, "Playtest
//! delivery"): what the games read before a client's window shows
//! (Combat Arms' data, the menus: ~300 MB the first time) comes from the
//! asset service; rather than on demand with nothing on screen, the client
//! app (`chimera_client::client::startup`) fetches it first behind a
//! progress window, then starts the game.
//!
//! - `plan` (from the store alone, no network): with an asset service set
//!   (a bound token, or `[assets]`), the startup sets
//!   (`chimera_assets::sets::STARTUP_SETS`) of each of the app's games
//!   that isn't installed, when the store lacks a manifest or one of their
//!   records. An installed game is never fetched (the hybrid model).
//!   `plan_here` asks this machine (its config, its installs, Steam's
//!   library); a process started after a startup fetch (`FETCHED_ENV`)
//!   plans nothing, so a record that won't come can't loop.
//! - `StartupFetch` runs it on a thread (`chimera_assets::sets::fetch`,
//!   games the service doesn't list skipped) and says how it goes
//!   (`Status`); `progress_lines` is what the window shows.
//!
//! Known shortcuts: what the games read at startup beyond the startup sets
//! still comes on demand (in the console only); a game the service doesn't
//! serve and the store has no manifest of opens the window each start
//! (briefly: nothing to fetch).

use std::{
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

pub use chimera_assets::{AssetSettings, RemoteConfig, Store, sets::Wanted};
use chimera_assets::{
    progress::{Transfer, clock},
    sets::{self, STARTUP_SETS},
};
use chimera_core::{
    game_registry::GameInfo,
    mount::{config::LocalConfig, install},
};

/// Set in the game a startup fetch starts: it doesn't fetch again.
pub const FETCHED_ENV: &str = "CHIMERA_STARTUP_FETCHED";

/// What the window calls the download.
pub const WHAT: &str = "game files";

/// One of the app's games, and whether this machine has it installed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppGame {
    pub game: String,
    pub installed: bool,
}

/// The startup sets to fetch of `games` (None: nothing to fetch, or no
/// service): those of every game not installed (all of them when the
/// install is off), if the store lacks any of it.
pub fn plan(settings: &AssetSettings, games: &[AppGame]) -> Option<Vec<Wanted>> {
    settings.remote.as_ref()?;
    let wanted: Vec<Wanted> = games
        .iter()
        .filter(|g| !(g.installed && settings.use_install))
        .map(|g| Wanted { game: g.game.clone(), sets: STARTUP_SETS.iter().map(|s| s.to_string()).collect() })
        .collect();
    (!wanted.is_empty() && sets::missing(settings, &wanted)).then_some(wanted)
}

/// `plan` on this machine for `games`: its local config's asset settings,
/// its installs (the config, `$CHIMERA_<GAME>_ROOT`, Steam's library).
/// None in a game a startup fetch started (`FETCHED_ENV`).
pub fn plan_here(games: &[&'static GameInfo]) -> Option<(AssetSettings, Vec<Wanted>)> {
    if std::env::var_os(FETCHED_ENV).is_some() {
        return None;
    }
    let config = LocalConfig::load().ok()?;
    let settings = config.assets.resolve();
    settings.remote.as_ref()?;
    let games: Vec<AppGame> = games
        .iter()
        .map(|g| AppGame {
            game: g.id.to_string(),
            installed: install::resolve(&config, g.id, g.steam.as_ref()).install.is_some(),
        })
        .collect();
    let wanted = plan(&settings, &games)?;
    Some((settings, wanted))
}

/// How a startup fetch goes.
#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Asking the service who we are and what it has.
    Connecting,
    /// `done` of `total` bytes so far, `elapsed` since they started.
    Downloading { done: u64, total: u64, elapsed: Duration },
    /// Done (the summary line).
    Done(String),
    /// Failed, why (in a player's words).
    Failed(String),
}

/// A startup fetch on its thread.
pub struct StartupFetch {
    job: Option<JoinHandle<Result<String, String>>>,
    transfer: Arc<Mutex<Option<Arc<Transfer>>>>,
    end: Option<Status>,
    /// The service's URL.
    pub from: String,
}

impl StartupFetch {
    /// Fetch `wanted` with `settings` (`plan_here`'s).
    pub fn start(settings: AssetSettings, wanted: Vec<Wanted>) -> Self {
        let from = settings.remote.as_ref().map(|r| r.url.clone()).unwrap_or_default();
        let transfer: Arc<Mutex<Option<Arc<Transfer>>>> = Arc::default();
        let shared = transfer.clone();
        let job = std::thread::Builder::new()
            .name("startup fetch".into())
            .spawn(move || {
                let options = sets::Options { skip_unlisted: true, ..Default::default() };
                sets::fetch(&settings, &wanted, WHAT, &shared, options)
            })
            .ok();
        Self {
            end: job.is_none().then(|| Status::Failed("couldn't start the download".into())),
            job,
            transfer,
            from,
        }
    }

    /// How it goes now.
    pub fn status(&mut self) -> Status {
        if let Some(end) = &self.end {
            return end.clone();
        }
        if self.job.as_ref().is_some_and(|j| j.is_finished()) {
            let result = self.job.take().map_or(Err("no download".into()), |j| {
                j.join().unwrap_or(Err("the download failed unexpectedly".into()))
            });
            let end = match result {
                Ok(summary) => Status::Done(summary),
                Err(e) => Status::Failed(e),
            };
            self.end = Some(end.clone());
            return end;
        }
        match self.transfer.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            Some(t) => {
                let (_, done, total) = t.done();
                Status::Downloading { done, total, elapsed: t.elapsed() }
            }
            None => Status::Connecting,
        }
    }
}

/// The window's two lines for `done` of `total` bytes after `elapsed`:
/// "Downloading game files: X of Y MB", then the speed and time left
/// (once something has arrived).
pub fn progress_lines(done: u64, total: u64, elapsed: Duration) -> (String, String) {
    let mb = |b: u64| b as f64 / 1e6;
    let head = format!("Downloading {WHAT}: {:.1} of {:.1} MB", mb(done), mb(total));
    let secs = elapsed.as_secs_f64();
    if done == 0 || secs < 0.5 {
        return (head, "Starting...".into());
    }
    let rate = done as f64 / secs;
    let left = Duration::from_secs_f64(total.saturating_sub(done) as f64 / rate.max(1.0));
    (head, format!("{:.1} MB/s, about {} left", rate / 1e6, clock(left)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lines_say_mb_speed_and_time_left() {
        let (head, rest) = progress_lines(50_000_000, 300_000_000, Duration::from_secs(10));
        assert_eq!(head, "Downloading game files: 50.0 of 300.0 MB");
        assert_eq!(rest, "5.0 MB/s, about 50s left");
        assert_eq!(progress_lines(0, 300_000_000, Duration::from_secs(1)).1, "Starting...");
    }
}
