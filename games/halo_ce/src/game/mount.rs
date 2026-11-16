//! Halo's install as a mount: a disc image file or a folder of the disc's
//! files (`blam_formats::disc::open_install`, per specs/halo_ce/
//! disc_image.md), with the asset store instead of it as the local
//! config's `[assets]` says (`mount::served`). An image is one file, so
//! what asks whether an install is there asks for the install path itself
//! (`served::usable(GAME, path, "")`).
//!
//! The install's build (`build`): the build string of its map files'
//! headers (cache_file.md, "The map file on disc": `01.10.12.2276` on the
//! NTSC disc; another region's disc has another build string), read from
//! the first 2 KB of one map, never the whole disc. It names the asset
//! store's manifest (`manifests/halo_ce/<build>.txt`, `chimera_pack`'s
//! per-game build). The store serves Halo instead of an install, or under
//! one at the disc's build (`served::open_built` with `build`): a player
//! has the whole disc or none of it, so what a store fills in under a
//! disc is only what no disc holds, the generated sets (the campaign
//! characters' units, `campaign::unit`).

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

use blam_formats::{
    cache::{HEADER_SIZE, Header},
    disc::DiscImage,
};

use super::{GAME, MULTIPLAYER_MAPS, maps::map_path};
use crate::mount::Mount;

/// Mount Halo: its install (`open_install`) and/or the asset store, as the
/// local config's `[assets]` says (`mount::served`).
pub fn open(install: &Path) -> io::Result<Mount> {
    crate::mount::served::open_built(GAME, install, open_install, build)
}

/// Mount a Halo install: `install` is a disc image file or a folder holding
/// the disc's files.
pub fn open_install(install: &Path) -> io::Result<Mount> {
    blam_formats::disc::open_install(install)
}

/// The maps whose header `build` reads, in order: the menu's (on every
/// disc), then the multiplayer maps.
fn build_sources() -> impl Iterator<Item = String> {
    std::iter::once(map_path("ui")).chain(MULTIPLAYER_MAPS.iter().map(|m| map_path(m)))
}

/// The build of the install at `install` (a disc image or a folder): the
/// build string in the header of its first map of `build_sources`
/// (module doc). An error when it holds none of them or the header is
/// not a map's.
pub fn build(install: &Path) -> io::Result<String> {
    let head = if install.is_file() {
        let image = DiscImage::open(install)?;
        let (path, (offset, size)) = build_sources()
            .find_map(|p| Some((p.clone(), image.extent(&p)?)))
            .ok_or_else(|| no_map(install))?;
        if size < HEADER_SIZE as u64 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{path}: shorter than a map header")));
        }
        let mut file = File::open(install)?;
        file.seek(SeekFrom::Start(offset))?;
        let mut head = vec![0; HEADER_SIZE];
        file.read_exact(&mut head)?;
        head
    } else {
        // A folder of the disc's files: through its mount (any casing).
        let mount = open_install(install)?;
        let path = build_sources().find(|p| mount.contains(p)).ok_or_else(|| no_map(install))?;
        mount.read(&path)?
    };
    Ok(Header::parse(&head)?.build)
}

fn no_map(install: &Path) -> io::Error {
    io::Error::new(io::ErrorKind::NotFound, format!("{}: no map file (maps/ui.map or a multiplayer map)", install.display()))
}
