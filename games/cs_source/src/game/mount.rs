//! The CS:S search path: what the game itself would see (`open_install`),
//! with the asset store under or instead of it (`open`, `mount::served`).
//! The order is CS:S's own list (`SEARCH_PATH`), mounted by Source's
//! shared search path (`source_engine::search_path`, lift S1).
//!
//! Known shortcuts:
//! - `SEARCH_PATH` isn't the install's `cstrike/gameinfo.txt`: it puts
//!   the loose `cstrike/` folder before `cstrike_pak_dir.vpk` and mounts
//!   `cstrike/custom` itself rather than each folder and VPK inside it,
//!   where the game's file searches every VPK before the loose folders.
//!   Kept as it was when the search path was lifted (its tests and the
//!   converted maps unchanged); moving CS:S to `SearchPath::read_gameinfo`
//!   is a parity change of its own (docs/plans/active/hl2dm.md, S1).

use std::{io, path::Path};

use source_engine::search_path::{SearchPath, Spec};

use crate::mount::{LooseDir, Mount};

/// Mount CS:S: its install (`open_install`) and/or the asset store, as the
/// local config's `[assets]` says (`mount::served`).
pub fn open(install: &Path) -> io::Result<Mount> {
    crate::mount::served::open(super::GAME, install, open_install)
}

/// CS:S's search path, relative to the install (the folder containing
/// `cstrike/` and `hl2/`), first match wins: CS:S content first, then the
/// shared HL2 content it depends on, then the engine's `platform/` folder
/// (the GameUI scheme's base, its fonts); loose files before archives at
/// each level. Maps and content fetched from servers (the game's own
/// download folder, read-only) last: they never replace the game's own
/// files.
pub const SEARCH_PATH: &[Spec] = &[
    Spec::loose("cstrike/custom"),
    Spec::loose("cstrike"),
    Spec::vpk("cstrike/cstrike_pak_dir.vpk"),
    Spec::vpk("hl2/hl2_textures_dir.vpk").optional(),
    Spec::vpk("hl2/hl2_sound_vo_english_dir.vpk").optional(),
    Spec::vpk("hl2/hl2_sound_misc_dir.vpk").optional(),
    Spec::vpk("hl2/hl2_misc_dir.vpk").optional(),
    Spec::loose("hl2"),
    Spec::loose("platform").optional(),
    Spec::vpk("platform/platform_misc_dir.vpk").optional(),
    Spec::loose("cstrike/download"),
];

/// Mount a CS:S install (the folder containing `cstrike/` and `hl2/`):
/// `SEARCH_PATH`, then Chimera's content cache.
pub fn open_install(install: &Path) -> io::Result<Mount> {
    if !install.join("cstrike").is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{}: no cstrike/ folder; is this a CS:S install?", install.display()),
        ));
    }
    let mut mount = SearchPath::from_specs(install, SEARCH_PATH).mount()?;
    // Chimera's cache, last: it never replaces the game's own files.
    if let Some(cache) = crate::mount::config::content_dir(super::GAME) {
        mount.push(LooseDir::new(cache).hiding(&["vpk"]));
    }
    Ok(mount)
}

/// Copy a map (`.bsp`, or `.bsp.bz2` as servers send them) into Chimera's
/// content cache as `maps/<name>.bsp`, and note it in `index.toml`
/// (`source_engine::maps::import_map_into`). Returns the map's name.
pub fn import_map(file: &Path) -> Result<String, String> {
    let cache = crate::mount::config::content_dir(super::GAME).ok_or("no per-user data folder")?;
    import_map_into(file, &cache)
}

pub use source_engine::maps::import_map_into;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_plain_and_bzip2_maps() {
        let dir = std::env::temp_dir().join(format!("chimera-import-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let bsp = [b"VBSP".as_slice(), &[20, 0, 0, 0], &[7u8; 64]].concat();
        let plain = dir.join("mg_Test.bsp");
        std::fs::write(&plain, &bsp).unwrap();
        let mut packed = Vec::new();
        {
            use std::io::Write;
            let mut enc = bzip2::write::BzEncoder::new(&mut packed, bzip2::Compression::default());
            enc.write_all(&bsp).unwrap();
            enc.finish().unwrap();
        }
        let bz = dir.join("deathrun_x.bsp.bz2");
        std::fs::write(&bz, &packed).unwrap();
        let cache = dir.join("cache");
        assert_eq!(import_map_into(&plain, &cache).unwrap(), "mg_test");
        assert_eq!(import_map_into(&bz, &cache).unwrap(), "deathrun_x");
        assert_eq!(std::fs::read(cache.join("maps/deathrun_x.bsp")).unwrap(), bsp);
        let index = std::fs::read_to_string(cache.join("index.toml")).unwrap();
        assert_eq!(index.matches("[[map]]").count(), 2);
        // Not a map.
        let junk = dir.join("junk.bsp");
        std::fs::write(&junk, b"nope").unwrap();
        assert!(import_map_into(&junk, &cache).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
