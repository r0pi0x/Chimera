//! Shared mount plumbing: the read-only virtual filesystem over a game's
//! archives and loose files, plus local configuration and finding the
//! installs (`install`: config, saved settings, Steam libraries). Archive
//! formats live in `source_formats` (VPK) or `games/<name>/`.
//!
//! `Mount` (ordered layers, first match wins), `FileSource`, `LooseDir`,
//! path normalization and KeyValues text (Steam's files and our saved ones
//! read with `Escapes::On`) come from `chimera_vfs` and `chimera_formats` (`keyvalues`),
//! re-exported here; `chimera.local.toml` loading (`config.rs`); finding
//! installs (`install.rs`: the config's path, else the one saved in the
//! user's `settings.toml` (beside the cfg folder; `$CHIMERA_SETTINGS`), else
//! Steam: its folders per system (Windows registry `SteamPath`;
//! `$CHIMERA_STEAM_ROOT` replaces them), each `libraryfolders.vdf` library
//! (old and new formats), the app's `appmanifest_<id>.acf` (fully
//! installed; its build id for the mount doctor) and the files the game
//! needs; `GameInfo::steam` names the games it looks for;
//! `LocalConfig::game_path` goes through it); a game's mount with the asset
//! store under or instead of its install, and the vfs access log
//! (`served.rs`).

pub mod config;
pub mod install;
pub mod served;

pub use chimera_assets as assets;
pub use chimera_vfs::{Entry, FileSource, LooseDir, Mount, normalize};
pub use chimera_formats::keyvalues;
