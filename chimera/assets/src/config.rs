//! Where the store is, whether the install is used and which asset service
//! fills the store: `[assets]` in chimera.local.toml (`store`,
//! `use_install`, `builds.<game>`, `url`, `token`, `key`), with the
//! environment over it (`CHIMERA_ASSET_STORE`, `CHIMERA_ASSET_USE_INSTALL`
//! = 0/1, `CHIMERA_ASSET_URL`, `CHIMERA_ASSET_TOKEN`, `CHIMERA_ASSET_KEY`)
//! and the repository's `assets.lock` (`lock`). With no service named
//! there, a player's platform account (`account`: its hub's asset service
//! with its current ticket), else a player's bound access (`access`: a token and the service a game
//! server named, pinned by its certificate) is the service. The default store is per
//! user and shared by every checkout and slot: `~/.cache/chimera/assets`
//! (Linux), `%LOCALAPPDATA%\chimera\assets` (Windows).
//!
//! With a service (`url` and `token`), a game's build is chosen as without
//! one (`[assets] builds`, else `assets.lock`'s, else the newest), but a
//! manifest the store lacks, or keeps in another version than the one
//! `assets.lock` pins, is fetched from the service (the newest
//! there when nothing pins one), checked and kept; `fetcher` gets the
//! records a `CachedFiles` lacks. A service that can't be reached or
//! refuses is reported once per game and process, and the game is then
//! treated as not served.
//!
//! Whether an install is used (`use_install`, `service_first`): the
//! environment's `CHIMERA_ASSET_USE_INSTALL` (0/1), else `[assets]
//! use_install`, else the process's default (`InstallDefault`,
//! `set_install_default`). Tests and tools keep the default `Install`
//! (an install first, the store under it: as before 2026-11-15); the game
//! apps set `ServiceFirst` (a client: `ServiceFirstClient`; the player's
//! "Use my installed games" setting sets `Install`), with which a game the
//! store and its service serve is read from them alone whenever a service
//! is set (`[assets]`, a hub account, a bound token) or, on a client, a
//! token or account waits for the first join to bind it: everyone plays
//! the published build's files and data, keys or no keys. A game they
//! don't serve still reads its install; with no service at all, the
//! installs are used as before (`resolve`, `policy`).

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
};

use serde::Deserialize;

use crate::{
    AssetsLock, Store,
    remote::{self, Fetcher, Remote, RemoteConfig},
};

pub const STORE_ENV: &str = "CHIMERA_ASSET_STORE";
pub const USE_INSTALL_ENV: &str = "CHIMERA_ASSET_USE_INSTALL";

/// `[assets]` as written.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct AssetsConfig {
    /// The store folder (`~` expanded); default: the user's cache.
    pub store: Option<PathBuf>,
    /// False: games with a manifest in the store load from it alone, the
    /// install hidden. True: the store is layered under the install (and
    /// stands in for a missing one). Unset: the process's default
    /// (`InstallDefault`: tests and tools the install, the game apps the
    /// service first).
    pub use_install: Option<bool>,
    /// The build to mount per game when the store serves it alone
    /// (default: the newest in the store); under an install, only the
    /// install's own build is mounted, and only if this names it or
    /// names none (`chimera_core::mount::served`).
    #[serde(default)]
    pub builds: BTreeMap<String, String>,
    /// The asset service (`https://...`); with `token`, files the store
    /// lacks are fetched from it.
    pub url: Option<String>,
    /// This user's token for `url` (`chimera_pack user add` printed it).
    pub token: Option<String>,
    /// The service's public key (hex; `chimera_assets login` prints it):
    /// pinned, else the key the service names is trusted.
    pub key: Option<String>,
}

/// The settings in effect.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetSettings {
    pub store: Option<Store>,
    /// Whether an install may be read (false: the store alone, an
    /// install hidden even for a game the store doesn't serve).
    pub use_install: bool,
    /// The service first (`InstallDefault::ServiceFirst`, nothing chosen):
    /// a game the store and its service serve is read from them alone,
    /// its install unused; only a game they don't serve reads its install
    /// (and only while a service is set: a client's token waiting for its
    /// first join hides the installs until the join binds it).
    pub service_first: bool,
    pub builds: BTreeMap<String, String>,
    /// The asset service, when `url` and `token` are both set.
    pub remote: Option<RemoteConfig>,
    /// The repository's `assets.lock`, when there is one.
    pub lock: Option<AssetsLock>,
}

/// How a process uses an install when neither `CHIMERA_ASSET_USE_INSTALL`
/// nor `[assets] use_install` says (`set_install_default`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InstallDefault {
    /// The install first, the store under it (tests, tools, a player's
    /// "Use my installed games").
    #[default]
    Install,
    /// The store and its service first whenever a service is set (a
    /// dedicated server).
    ServiceFirst,
    /// `ServiceFirst`, and a token or hub account waiting for its first
    /// join counts as a service: the installs stay hidden until the join
    /// binds it (a game client).
    ServiceFirstClient,
}

static INSTALL_DEFAULT: AtomicU8 = AtomicU8::new(0);

/// This process's default (`InstallDefault`), set by an app at its start,
/// before anything mounts a game.
pub fn set_install_default(default: InstallDefault) {
    INSTALL_DEFAULT.store(default as u8, Ordering::Relaxed);
}

/// This process's default (`Install` unless an app set another).
pub fn install_default() -> InstallDefault {
    match INSTALL_DEFAULT.load(Ordering::Relaxed) {
        1 => InstallDefault::ServiceFirst,
        2 => InstallDefault::ServiceFirstClient,
        _ => InstallDefault::Install,
    }
}

/// `CHIMERA_ASSET_USE_INSTALL`'s value as a choice (None: unset or not
/// a yes/no word).
pub fn parse_use_install(value: Option<&str>) -> Option<bool> {
    match value.map(str::trim) {
        Some("0" | "false" | "no" | "off") => Some(false),
        Some("1" | "true" | "yes" | "on") => Some(true),
        _ => None,
    }
}

/// `(use_install, service_first)` for a choice (`chosen`: the variable's,
/// else `[assets] use_install`), the process's `default`, whether a
/// service is set (`remote`) and whether a token or account waits for a
/// join to bind it (`waiting`).
pub fn policy(chosen: Option<bool>, default: InstallDefault, remote: bool, waiting: bool) -> (bool, bool) {
    match (chosen, default) {
        (Some(chosen), _) => (chosen, false),
        (None, InstallDefault::Install) => (true, false),
        (None, InstallDefault::ServiceFirst) => (true, remote),
        (None, InstallDefault::ServiceFirstClient) => (true, remote || waiting),
    }
}

/// The user-wide store folder.
pub fn default_store() -> Option<PathBuf> {
    Some(dirs::cache_dir()?.join("chimera").join("assets"))
}

fn expand_home(path: PathBuf) -> PathBuf {
    match (path.strip_prefix("~"), dirs::home_dir()) {
        (Ok(rest), Some(home)) => home.join(rest),
        _ => path,
    }
}

impl AssetsConfig {
    /// With the environment's overrides and the repository's
    /// `assets.lock`.
    pub fn resolve(&self) -> AssetSettings {
        let env = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let mut s =
            self.resolve_with(std::env::var_os(STORE_ENV).map(PathBuf::from), std::env::var(USE_INSTALL_ENV).ok());
        let url = env(remote::URL_ENV).or_else(|| self.url.clone());
        let token = env(remote::TOKEN_ENV).or_else(|| self.token.clone());
        let key = env(remote::KEY_ENV).or_else(|| self.key.clone());
        // Else this user's platform account's asset service (`account`),
        // else the service a game server bound this user's token to
        // (`access`: a player's, not a developer's).
        s.remote = remote_config(url, token, key)
            .or_else(crate::account::user_remote)
            .or_else(|| crate::access::Access::load().remote());
        s.lock = AssetsLock::current();
        let default = install_default();
        let waiting = default == InstallDefault::ServiceFirstClient
            && s.remote.is_none()
            && (crate::access::Access::load().token.is_some() || crate::account::has_account());
        let chosen = parse_use_install(std::env::var(USE_INSTALL_ENV).ok().as_deref()).or(self.use_install);
        (s.use_install, s.service_first) = policy(chosen, default, s.remote.is_some(), waiting);
        s
    }

    pub fn resolve_with(&self, store_env: Option<PathBuf>, use_install_env: Option<String>) -> AssetSettings {
        let store = store_env
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| self.store.clone())
            .map(expand_home)
            .or_else(default_store)
            .map(Store::new);
        let use_install = parse_use_install(use_install_env.as_deref()).or(self.use_install).unwrap_or(true);
        let remote = remote_config(self.url.clone(), self.token.clone(), self.key.clone());
        AssetSettings { store, use_install, service_first: false, builds: self.builds.clone(), remote, lock: None }
    }
}

fn remote_config(url: Option<String>, token: Option<String>, key: Option<String>) -> Option<RemoteConfig> {
    let (url, token) = (
        url.filter(|u| !u.trim().is_empty())?,
        token.filter(|t| !t.trim().is_empty())?,
    );
    Some(RemoteConfig {
        url: url.trim().into(),
        token: token.trim().into(),
        key: key.filter(|k| !k.trim().is_empty()),
        fingerprint: None,
    })
}

/// (service url, game or game build) pairs the service failed for in this process: not
/// asked again.
static FAILED: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());

impl AssetSettings {
    /// The build of `game` to mount: the pinned one (`[assets] builds`,
    /// else `assets.lock`), else the store's newest, else the service's
    /// newest; a manifest the store lacks is fetched from the service
    /// (when there is one; not while the game's fetches are held,
    /// `cached::hold_fetches`). None when neither has it.
    pub fn build(&self, game: &str) -> Option<String> {
        let store = self.store.as_ref()?;
        let locked = self.lock.as_ref().and_then(|l| l.games.get(game));
        let pinned = self.builds.get(game).cloned().or_else(|| locked.map(|p| p.build.clone()));
        // A kept manifest that isn't the one `assets.lock` pins is old: fetched again.
        let stale = |b: &str| {
            locked.filter(|p| p.build == b).is_some_and(|p| {
                std::fs::read(store.manifest_path(game, b)).is_ok_and(|bytes| crate::sha256_hex(&bytes) != p.manifest)
            })
        };
        match pinned {
            Some(b) if store.manifest_path(game, &b).is_file() && !(stale(&b) && self.remote.is_some()) => Some(b),
            Some(b) => {
                let expect = locked.filter(|p| p.build == b).map(|p| p.manifest.clone());
                let locked_id = expect.as_deref().map(|e| format!(", locked manifest {}", &e[..e.len().min(12)]));
                let what = format!("{game} build {b}{}", locked_id.unwrap_or_default());
                self.with_remote(&what, |r| r.fetch_manifest(store, game, &b, expect.as_deref()).map(|_| b.clone()))
            }
            None => store.latest_build(game).or_else(|| {
                self.with_remote(game, |r| {
                    let b = r.latest_build(game).map_err(|e| e.to_string())?.ok_or("not on the service")?;
                    r.fetch_manifest(store, game, &b, None).map(|_| b)
                })
            }),
        }
    }

    /// The service, if one is set and usable.
    pub fn remote(&self) -> Option<Arc<Remote>> {
        let config = self.remote.as_ref()?;
        Remote::shared(config).inspect_err(|e| warn_once(&config.url, "", e)).ok()
    }

    /// What fetches the records a `CachedFiles` lacks: the service.
    pub fn fetcher(&self) -> Option<Arc<dyn Fetcher>> {
        self.remote().map(|r| r as Arc<dyn Fetcher>)
    }

    /// `f` on the service, not asked again in this process once it failed
    /// for `what` (a game, or a game's build).
    fn with_remote(&self, what: &str, f: impl FnOnce(&Remote) -> Result<String, String>) -> Option<String> {
        if crate::cached::fetches_held(what.split_whitespace().next().unwrap_or_default()) {
            return None;
        }
        let remote = self.remote()?;
        let key = (remote.url().to_string(), what.to_string());
        if FAILED.lock().unwrap_or_else(|e| e.into_inner()).contains(&key) {
            return None;
        }
        f(&remote).inspect_err(|e| warn_once(remote.url(), what, e)).ok()
    }

    /// Whether the store (or the service through it) can serve `game`.
    pub fn serves(&self, game: &str) -> bool {
        self.build(game).is_some()
    }

    /// Where `game`'s files are read from, given the install found
    /// (`found`, asked only when it may be used): the store's stand-in
    /// (`stand_in_install`) when the store serves the game alone (the
    /// install off, or the service first), else the install, else the
    /// stand-in for a missing one. None: neither.
    pub fn game_path(&self, game: &str, found: impl FnOnce() -> Option<PathBuf>) -> Option<PathBuf> {
        let served = || self.serves(game).then(|| self.stand_in_install(game)).flatten();
        if !self.use_install {
            return served();
        }
        if self.service_first {
            // A game the service doesn't serve reads its install, but not
            // while a client's token waits for the join that binds it (the
            // join fetches what the service has, then reads again).
            return served().or_else(|| self.remote.is_some().then(found).flatten());
        }
        found().or_else(served)
    }

    /// The path that stands for `game`'s install when it is served from
    /// the store alone (its manifest folder): `chimera_core::mount::served::alone` recognizes it.
    pub fn stand_in_install(&self, game: &str) -> Option<PathBuf> {
        Some(self.store.as_ref()?.manifest_dir(game))
    }
}

fn warn_once(url: &str, game: &str, e: &str) {
    if FAILED.lock().unwrap_or_else(|e| e.into_inner()).insert((url.to_string(), game.to_string())) {
        eprintln!("asset service {url}{}: {e}", if game.is_empty() { String::new() } else { format!(" ({game})") });
    }
}

/// `[assets]` of the local config, for tools below Chimera's `config`
/// (the `chimera_assets` CLI): the file `chimera_core`'s `LocalConfig`
/// reads (`CHIMERA_CONFIG`, else `chimera.local.toml` here, else in the
/// checkout; the old `mashup` names too). No file: an empty section.
pub fn load_local() -> Result<AssetsConfig, String> {
    local_section("assets")
}

/// One section of the local config (`[assets]`, `[feedback]`), and only
/// it: the rest of the file (the games' keys) is never kept. No file or
/// no section: the default.
pub fn local_section<T: serde::de::DeserializeOwned + Default>(name: &str) -> Result<T, String> {
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let candidates = [
        std::env::var_os("CHIMERA_CONFIG").or_else(|| std::env::var_os("MASHUP_CONFIG")).map(PathBuf::from),
        Some(PathBuf::from("chimera.local.toml")),
        Some(PathBuf::from("mashup.local.toml")),
        Some(checkout.join("chimera.local.toml")),
        Some(checkout.join("mashup.local.toml")),
    ];
    let Some(path) = candidates.into_iter().flatten().find(|p| p.is_file()) else {
        return Ok(T::default());
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let table: toml::Table = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    match table.get(name) {
        Some(section) => {
            section.clone().try_into().map_err(|e: toml::de::Error| format!("{}: [{name}]: {e}", path.display()))
        }
        None => Ok(T::default()),
    }
}
