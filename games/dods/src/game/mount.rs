//! DoD:S's search path: the install's own `dod/gameinfo.txt`, read by
//! Source's shared search path (`source_engine::search_path`), then
//! Chimera's content cache; with the asset store under or instead of it
//! (`open`, `mount::served`). As the stock file lists it
//! (specs/dods/install.md, "The search path"): `dod/custom/*`,
//! `dod/dod_english.vpk` (the client's only: absent from the dedicated
//! server's layout, skipped), `dod/dod_pak.vpk`, the `hl2` VPKs and
//! `platform`'s, missing entries skipped. The content is unencrypted.

use std::{io, path::Path};

use source_engine::search_path::SearchPath;

use super::{CONTENT_DIR, GAME};
use crate::mount::{LooseDir, Mount};

/// Mount DoD:S: its install (`open_install`) and/or the asset store, as
/// the local config's `[assets]` says (`mount::served`).
pub fn open(install: &Path) -> io::Result<Mount> {
    crate::mount::served::open(GAME, install, open_install)
}

/// The install's search path (the folder holding `dod/`, `hl2/` and
/// `platform/`), from `dod/gameinfo.txt`.
pub fn search_path(install: &Path) -> io::Result<SearchPath> {
    if !install.join(CONTENT_DIR).is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: no {CONTENT_DIR}/ folder; is this a DoD:S install?", install.display()),
        ));
    }
    SearchPath::read_gameinfo(install, CONTENT_DIR)
}

/// Mount a DoD:S install: `search_path`, then Chimera's content cache
/// (last: it never replaces the game's own files).
pub fn open_install(install: &Path) -> io::Result<Mount> {
    let mut mount = search_path(install)?.mount()?;
    if let Some(cache) = crate::mount::config::content_dir(GAME) {
        mount.push(LooseDir::new(cache).hiding(&["vpk"]));
    }
    Ok(mount)
}

/// Copy a map (`.bsp`, or `.bsp.bz2`) into Chimera's content cache for
/// DoD:S (`source_engine::maps::import_map_into`). Returns its name.
pub fn import_map(file: &Path) -> Result<String, String> {
    let cache = crate::mount::config::content_dir(GAME).ok_or("no per-user data folder")?;
    source_engine::maps::import_map_into(file, &cache)
}
