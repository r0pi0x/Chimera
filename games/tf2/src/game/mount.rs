//! TF2's search path: the install's own `tf/gameinfo.txt`, read by
//! Source's shared search path (`source_engine::search_path`), then
//! Chimera's content cache; with the asset store under or instead of it
//! (`open`, `mount::served`). As the stock file lists it
//! (specs/tf2/install.md 1.4): `tf/custom/*`, the `tf` VPKs (textures,
//! sounds, misc), the `hl2` VPKs, `platform`'s VPK, then the loose `tf/`,
//! `hl2/` and `platform/` folders and `tf/download` last: every VPK before
//! any loose folder, missing entries skipped.
//!
//! The dedicated server's layout (app 232250) has the texture and sound
//! VPKs' `_dir` files without their data archives (install.md 1.1): the
//! VPK reader leaves a missing archive's files out (`source_formats::vpk`),
//! so the server's install mounts with its maps, models and material text,
//! and a texture it lacks is missing (a stand-in), not an error.

use std::{io, path::Path};

use source_engine::search_path::SearchPath;

use super::{CONTENT_DIR, GAME};
use crate::mount::{LooseDir, Mount};

/// Mount TF2: its install (`open_install`) and/or the asset store, as the
/// local config's `[assets]` says (`mount::served`).
pub fn open(install: &Path) -> io::Result<Mount> {
    crate::mount::served::open(GAME, install, open_install)
}

/// The install's search path (the folder holding `tf/`, `hl2/` and
/// `platform/`), from `tf/gameinfo.txt`.
pub fn search_path(install: &Path) -> io::Result<SearchPath> {
    if !install.join(CONTENT_DIR).is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: no {CONTENT_DIR}/ folder; is this a TF2 install?", install.display()),
        ));
    }
    SearchPath::read_gameinfo(install, CONTENT_DIR)
}

/// Mount a TF2 install: `search_path`, then Chimera's content cache (last:
/// it never replaces the game's own files).
pub fn open_install(install: &Path) -> io::Result<Mount> {
    let mut mount = search_path(install)?.mount()?;
    if let Some(cache) = crate::mount::config::content_dir(GAME) {
        mount.push(LooseDir::new(cache).hiding(&["vpk"]));
    }
    Ok(mount)
}

/// The install's loose layers alone, as `open_install` orders them
/// (`unopened`).
fn open_install_loose(install: &Path) -> io::Result<Mount> {
    let (mut mount, _) = search_path(install)?.unopened();
    if let Some(cache) = crate::mount::config::content_dir(GAME) {
        mount.push(LooseDir::new(cache).hiding(&["vpk"]));
    }
    Ok(mount)
}

/// What `open` would mount, named without opening an archive
/// (`SearchPath::unopened`: a VPK's directory is read whole when it is
/// opened, 1.4 G instructions for the install's): its loose folders and
/// the asset store, layered as `open` layers them, and the archive files
/// it would open (none when the store serves the game alone). What an
/// entry of the converted cache is keyed by when its value should come
/// back without the mount (chimera_assets' `map_cache::cached_unopened`:
/// the guns' numbers, `weapons::script_numbers`).
pub fn unopened(install: &Path) -> io::Result<(Mount, Vec<std::path::PathBuf>)> {
    let light = crate::mount::served::open(GAME, install, open_install_loose)?;
    let archives = if crate::mount::served::alone(GAME, install) { Vec::new() } else { search_path(install)?.unopened().1 };
    Ok((light, archives))
}

/// Whether the install at `install` is the retail client's full content
/// (its texture and sound archives there), not the dedicated server's
/// (install.md 1.3).
pub fn full_content(install: &Path) -> bool {
    install.join("tf/tf2_textures_000.vpk").is_file() && install.join("tf/tf2_sound_misc_000.vpk").is_file()
}

/// Copy a map (`.bsp`, or `.bsp.bz2`) into Chimera's content cache for
/// TF2 (`source_engine::maps::import_map_into`). Returns its name.
pub fn import_map(file: &Path) -> Result<String, String> {
    let cache = crate::mount::config::content_dir(GAME).ok_or("no per-user data folder")?;
    source_engine::maps::import_map_into(file, &cache)
}
