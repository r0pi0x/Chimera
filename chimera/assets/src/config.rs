//! Where the store is, whether the install is used and which asset service
//! fills the store: `[assets]` in chimera.local.toml (`store`,
//! `use_install`, `builds.<game>`, `url`, `token`, `key`), with the
//! environment over it (`CHIMERA_ASSET_STORE`, `CHIMERA_ASSET_USE_INSTALL`
//! = 0/1, `CHIMERA_ASSET_URL`, `CHIMERA_ASSET_TOKEN`, `CHIMERA_ASSET_KEY`)
//! and the repository's `assets.lock` (`lock`). The default store is per
//! user and shared by every checkout and slot: `~/.cache/chimera/assets`
//! (Linux), `%LOCALAPPDATA%\chimera\assets` (Windows).
//!
//! With a service (`url` and `token`), a game's build is chosen as without
//! one (`[assets] builds`, else `assets.lock`'s, else the newest), but a
//! manifest the store lacks is fetched from the service (the newest
//! there when nothing pins one), checked and kept; `fetcher` gets the
//! records a `CachedFiles` lacks. A service that can't be reached or
//! refuses is reported once per game and process, and the game is then
//! treated as not served.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::{Arc, Mutex},
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
    /// install hidden. Default true: the store is layered under the
    /// install (and stands in for a missing one).
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
    pub use_install: bool,
    pub builds: BTreeMap<String, String>,
    /// The asset service, when `url` and `token` are both set.
    pub remote: Option<RemoteConfig>,
    /// The repository's `assets.lock`, when there is one.
    pub lock: Option<AssetsLock>,
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
        s.remote = remote_config(url, token, key);
        s.lock = AssetsLock::current();
        s
    }

    pub fn resolve_with(&self, store_env: Option<PathBuf>, use_install_env: Option<String>) -> AssetSettings {
        let store = store_env
            .filter(|p| !p.as_os_str().is_empty())
            .or_else(|| self.store.clone())
            .map(expand_home)
            .or_else(default_store)
            .map(Store::new);
        let use_install = match use_install_env.as_deref().map(str::trim) {
            Some("0" | "false" | "no" | "off") => false,
            Some("1" | "true" | "yes" | "on") => true,
            _ => self.use_install.unwrap_or(true),
        };
        let remote = remote_config(self.url.clone(), self.token.clone(), self.key.clone());
        AssetSettings { store, use_install, builds: self.builds.clone(), remote, lock: None }
    }
}

fn remote_config(url: Option<String>, token: Option<String>, key: Option<String>) -> Option<RemoteConfig> {
    let (url, token) = (url.filter(|u| !u.trim().is_empty())?, token.filter(|t| !t.trim().is_empty())?);
    Some(RemoteConfig { url: url.trim().into(), token: token.trim().into(), key: key.filter(|k| !k.trim().is_empty()) })
}

/// (service url, game or game build) pairs the service failed for in this process: not
/// asked again.
static FAILED: Mutex<BTreeSet<(String, String)>> = Mutex::new(BTreeSet::new());

impl AssetSettings {
    /// The build of `game` to mount: the pinned one (`[assets] builds`,
    /// else `assets.lock`), else the store's newest, else the service's
    /// newest; a manifest the store lacks is fetched from the service
    /// (when there is one). None when neither has it.
    pub fn build(&self, game: &str) -> Option<String> {
        let store = self.store.as_ref()?;
        let locked = self.lock.as_ref().and_then(|l| l.games.get(game));
        let pinned = self.builds.get(game).cloned().or_else(|| locked.map(|p| p.build.clone()));
        match pinned {
            Some(b) if store.manifest_path(game, &b).is_file() => Some(b),
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
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let candidates = [
        std::env::var_os("CHIMERA_CONFIG").or_else(|| std::env::var_os("MASHUP_CONFIG")).map(PathBuf::from),
        Some(PathBuf::from("chimera.local.toml")),
        Some(PathBuf::from("mashup.local.toml")),
        Some(checkout.join("chimera.local.toml")),
        Some(checkout.join("mashup.local.toml")),
    ];
    let Some(path) = candidates.into_iter().flatten().find(|p| p.is_file()) else {
        return Ok(AssetsConfig::default());
    };
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    // Only `[assets]` is read; the rest of the file is the games'.
    let table: toml::Table = toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    match table.get("assets") {
        Some(section) => {
            section.clone().try_into().map_err(|e: toml::de::Error| format!("{}: [assets]: {e}", path.display()))
        }
        None => Ok(AssetsConfig::default()),
    }
}
