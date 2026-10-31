//! `assets.lock`: what a branch's tests and tools expect from the asset
//! service, committed at the repository's root (docs/plans/active/
//! asset-service.md, section 5, item 3). Per game, the build and the
//! manifest's id (SHA-256 of its exact bytes); build ids and hashes are
//! facts about files, never their contents. A manifest fetched for a
//! locked build must have the locked id, so a branch's tests are
//! reproducible and a game update is a one-line diff
//! (`chimera_assets lock` writes it):
//!
//! ```text
//! # assets.lock: ...
//! [cs_source]
//! build = "25338318"
//! manifest = "<64 hex>"
//! ```
//!
//! Found as `CHIMERA_ASSET_LOCK`, else the first `assets.lock` in the
//! working folder or a folder above it (a test runs in its crate's
//! folder).

use std::{
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

pub const LOCK_ENV: &str = "CHIMERA_ASSET_LOCK";
pub const FILE_NAME: &str = "assets.lock";

/// One game's pin.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pin {
    pub build: String,
    /// SHA-256 of the manifest's bytes, hex.
    pub manifest: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AssetsLock {
    pub games: BTreeMap<String, Pin>,
}

const HEADER: &str = "\
# assets.lock: the game builds this branch's tests and tools expect from
# the asset service (docs/plans/active/asset-service.md). Facts only (build
# ids, manifest hashes). Written by `chimera_assets lock`.
";

impl AssetsLock {
    pub fn parse(text: &str) -> Result<Self, String> {
        let lock: Self = toml::from_str(text).map_err(|e| e.to_string())?;
        for (game, pin) in &lock.games {
            if !crate::store::is_name(game) || !crate::store::is_name(&pin.build) || !crate::store::is_id(&pin.manifest)
            {
                return Err(format!("[{game}]: a game, a build and a 64-hex manifest id"));
            }
        }
        Ok(lock)
    }

    pub fn to_text(&self) -> String {
        format!("{HEADER}\n{}", toml::to_string(self).unwrap_or_default())
    }

    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        crate::store::write_atomic(path, self.to_text().as_bytes())
    }

    /// The lock file in effect: `CHIMERA_ASSET_LOCK`, else the nearest
    /// `assets.lock` at or above the working folder.
    pub fn find() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os(LOCK_ENV).filter(|p| !p.is_empty()) {
            return Some(PathBuf::from(p));
        }
        let cwd = std::env::current_dir().ok()?;
        cwd.ancestors().map(|d| d.join(FILE_NAME)).find(|p| p.is_file())
    }

    /// The lock in effect, if any (a broken one is reported and ignored).
    pub fn current() -> Option<Self> {
        let path = Self::find()?;
        if !path.is_file() {
            return None;
        }
        Self::load(&path).inspect_err(|e| eprintln!("warning: {e}")).ok()
    }
}
