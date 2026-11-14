//! A player's access to an asset service (docs/plans/active/asset-service.md,
//! "Playtest delivery"): the token the service's admin gave them, and the
//! service a game server named for it, kept per user outside any checkout
//! (`<data_local>/chimera/asset-access.toml`: `~/.local/share/...`,
//! `%LOCALAPPDATA%\chimera\asset-access.toml`; mode 0600 on Unix;
//! `CHIMERA_ASSET_ACCESS` names another file). A release ships none.
//!
//! - The token is entered once (`set_token`: the game's
//!   `+chimera:asset_token <token>`, or the console).
//! - The first game server that offers its asset service binds the token
//!   to it (`bind`): its URL and its certificate's fingerprint. From then
//!   on the token goes only to a service with that certificate (the URL
//!   may change: a home connection's address does); a server naming
//!   another certificate is refused with the reason, never sent the token.
//!   Entering the token again clears the binding.
//! - `AssetsConfig::resolve` uses a bound access as the asset service when
//!   `[assets] url`/`token` (or their variables) don't name one, so a game
//!   started later reads every game's files from the service as a
//!   developer's does.
//! - A game that binds the token while it runs (a join) saves the
//!   binding (so `AssetsConfig::resolve` gives that service from then on)
//!   and binds it in the process too (`bind_live`): the store's files a
//!   game mounted before it (`late_fetcher`, which `mount::served` gives a
//!   mount opened with no service) fetch what they lack from it, so the
//!   game reads the service's files without starting again.

use std::{
    io,
    path::{Path, PathBuf},
    sync::{Arc, RwLock},
};

use serde::{Deserialize, Serialize};

use crate::{
    Store,
    remote::{Fetcher, Remote, RemoteConfig},
    store::write_atomic,
    tls::normalize_fingerprint,
};

/// The service this process bound the token to while running (`bind_live`).
static LIVE: RwLock<Option<RemoteConfig>> = RwLock::new(None);

/// From now on this process's `late_fetcher`s fetch from `remote` (a join
/// bound the token to it).
pub fn bind_live(remote: RemoteConfig) {
    *LIVE.write().unwrap_or_else(|e| e.into_inner()) = Some(remote);
}

/// The service this process bound while running, if any.
pub fn live_remote() -> Option<RemoteConfig> {
    LIVE.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// A fetcher for a mount opened before any service was set: it fetches
/// from the one this process binds later (`bind_live`); until then every
/// record it is asked for is a miss, as with no fetcher.
pub fn late_fetcher() -> Arc<dyn Fetcher> {
    Arc::new(LateFetcher)
}

struct LateFetcher;

impl LateFetcher {
    fn remote() -> io::Result<Arc<Remote>> {
        let config = live_remote().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no asset service"))?;
        Remote::shared(&config).map_err(io::Error::other)
    }
}

impl Fetcher for LateFetcher {
    fn fetch(&self, store: &Store, id: &str, size: u64) -> io::Result<Vec<u8>> {
        Self::remote()?.fetch(store, id, size)
    }

    fn fetch_many(&self, store: &Store, wanted: &[(String, u64)]) -> io::Result<()> {
        Self::remote()?.fetch_many(store, wanted)
    }
}

/// The variable naming another access file.
pub const ENV: &str = "CHIMERA_ASSET_ACCESS";

/// The access file's contents.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Access {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// The service the token is bound to (the last URL a game server named
    /// for it).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Its certificate's SHA-256 (hex).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

/// What `bind` did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Bound {
    /// Already bound to this service at this URL.
    Same,
    /// Bound now (the first service offered), or its URL changed.
    New,
}

/// Whether `token` looks like a token (one word of printable ASCII).
pub fn is_token(token: &str) -> bool {
    (8..=256).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_graphic())
}

impl Access {
    /// The access file: `CHIMERA_ASSET_ACCESS`, else the user's.
    pub fn path() -> Option<PathBuf> {
        match std::env::var_os(ENV).filter(|v| !v.is_empty()) {
            Some(p) => Some(PathBuf::from(p)),
            None => Some(dirs::data_local_dir()?.join("chimera").join("asset-access.toml")),
        }
    }

    /// The user's access (none yet: empty).
    pub fn load() -> Self {
        Self::path().map(|p| Self::load_from(&p)).unwrap_or_default()
    }

    pub fn load_from(path: &Path) -> Self {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|t| toml::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<PathBuf, String> {
        let path = Self::path().ok_or("no per-user data folder")?;
        self.save_to(&path)?;
        Ok(path)
    }

    pub fn save_to(&self, path: &Path) -> Result<(), String> {
        let text = toml::to_string(self).map_err(|e| e.to_string())?;
        write_atomic(path, text.as_bytes()).map_err(|e| format!("{}: {e}", path.display()))?;
        crate::sign::restrict(path);
        Ok(())
    }

    /// A new token (any binding forgotten: the next server offering its
    /// service binds it).
    pub fn set_token(&mut self, token: &str) -> Result<(), String> {
        let token = token.trim();
        if !is_token(token) {
            return Err("that isn't a token (one word, as `chimera_pack user add` printed it)".into());
        }
        *self = Self {
            token: Some(token.to_string()),
            url: None,
            fingerprint: None,
        };
        Ok(())
    }

    /// Bind the token to the service at `url` whose certificate is
    /// `fingerprint`: the first one offered, or the same one at a new URL.
    /// Another certificate than the bound one is refused (the token never
    /// goes there).
    pub fn bind(&mut self, url: &str, fingerprint: &str) -> Result<Bound, String> {
        let fingerprint =
            normalize_fingerprint(fingerprint).ok_or("the offered certificate fingerprint is malformed")?;
        let url = url.trim().trim_end_matches('/').to_string();
        if !url.starts_with("https://") {
            return Err(format!("the offered asset service {url:?} isn't https://"));
        }
        if self.token.is_none() {
            return Err("no asset token here".into());
        }
        match &self.fingerprint {
            Some(f) if *f != fingerprint => Err(format!(
                "this server's asset service (certificate {}) isn't the one your token is for (certificate {}): \
                 enter your token again (+chimera:asset_token <token>) only if the service's admin says it changed",
                crate::tls::short(&fingerprint),
                crate::tls::short(f),
            )),
            Some(_) if self.url.as_deref() == Some(url.as_str()) => Ok(Bound::Same),
            _ => {
                self.url = Some(url);
                self.fingerprint = Some(fingerprint);
                Ok(Bound::New)
            }
        }
    }

    /// The bound service with the token, if both are there.
    pub fn remote(&self) -> Option<RemoteConfig> {
        Some(RemoteConfig {
            url: self.url.clone()?,
            token: self.token.clone()?,
            key: None,
            fingerprint: Some(self.fingerprint.clone()?),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_token_binds_to_the_first_service_and_refuses_another_certificate() {
        let (a, b) = ("aa".repeat(32), "bb".repeat(32));
        let mut access = Access::default();
        assert!(access.bind("https://h:1", &a).is_err(), "no token yet");
        assert!(access.set_token("not a token").is_err());
        access.set_token(&format!("chimera1_{}", "0".repeat(64))).unwrap();
        assert!(access.remote().is_none(), "not bound yet");
        assert_eq!(access.bind("https://h:1/", &a), Ok(Bound::New));
        assert_eq!(access.bind("https://h:1", &a.to_uppercase()), Ok(Bound::Same));
        assert_eq!(
            access.bind("https://other:2", &a),
            Ok(Bound::New),
            "same certificate, new address"
        );
        assert!(
            access
                .bind("https://other:2", &b)
                .unwrap_err()
                .contains("isn't the one your token is for")
        );
        assert!(access.bind("http://h:1", &a).is_err(), "never plain HTTP");
        let remote = access.remote().unwrap();
        assert_eq!(
            (remote.url.as_str(), remote.fingerprint.as_deref()),
            ("https://other:2", Some(a.as_str()))
        );
        // Saved and read back; a new token forgets the binding.
        let path = std::env::temp_dir().join(format!("chimera-access-{}.toml", std::process::id()));
        access.save_to(&path).unwrap();
        assert_eq!(Access::load_from(&path), access);
        let _ = std::fs::remove_file(&path);
        access.set_token(&format!("chimera1_{}", "1".repeat(64))).unwrap();
        assert_eq!(access.bind("https://h:1", &b), Ok(Bound::New));
    }
}
