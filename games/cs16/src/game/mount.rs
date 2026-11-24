//! Counter-Strike 1.6's install as a mount (specs/goldsrc/mount.md): the
//! content folder chain as loose files, highest priority first, the game
//! folder `cstrike` then the fallback `liblist.gam` names (`fallback_dir`,
//! else Half-Life's `valve`), then Chimera's content cache; with the asset
//! store under or instead of it (`open`, `mount::served`). Lookups ignore
//! case (`LooseDir`), as GoldSrc's data and file names disagree on it.
//!
//! Known shortcuts:
//! - The client's language folder (`cstrike_<language>`) is not mounted
//!   yet: no client install has been checked (goldsrc.md, open question 2).

use std::{io, path::Path};

use goldsrc_formats::liblist::LibList;

use super::{CONTENT_DIR, GAME};
use crate::mount::{LooseDir, Mount};

/// Mount the game: its install (`open_install`) and/or the asset store, as
/// the local config's `[assets]` says (`mount::served`).
pub fn open(install: &Path) -> io::Result<Mount> {
    crate::mount::served::open(GAME, install, open_install)
}

/// The content folders searched, highest priority first, from the
/// install's `cstrike/liblist.gam` (absent: `cstrike`, `valve`).
pub fn folder_chain(install: &Path) -> Vec<String> {
    let liblist = std::fs::read(install.join(CONTENT_DIR).join("liblist.gam"))
        .map(|b| LibList::parse(&String::from_utf8_lossy(&b)))
        .unwrap_or_default();
    liblist.folder_chain(CONTENT_DIR, None)
}

/// Mount an install (the folder holding `cstrike/` and `valve/`): its
/// folder chain, then Chimera's content cache (last: it never replaces
/// the game's own files).
pub fn open_install(install: &Path) -> io::Result<Mount> {
    if !install.join(CONTENT_DIR).is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{}: no {CONTENT_DIR}/ folder; is this a Counter-Strike 1.6 install?",
                install.display()
            ),
        ));
    }
    let mut mount = Mount::default();
    for folder in folder_chain(install) {
        let dir = install.join(&folder);
        if dir.is_dir() {
            mount.push(LooseDir::new(dir));
        }
    }
    if let Some(cache) = crate::mount::config::content_dir(GAME) {
        mount.push(LooseDir::new(cache));
    }
    Ok(mount)
}
