//! Halo's install as a mount: a disc image file or a folder of the disc's
//! files (`blam_formats::disc::open_install`, per specs/halo_ce/
//! disc_image.md), with the asset store under or instead of it as the
//! local config's `[assets]` says (`mount::served`). An image is one file,
//! so what asks whether an install is there asks for the install path
//! itself (`served::usable(GAME, path, "")`).

use std::{io, path::Path};

use super::GAME;
use crate::mount::Mount;

/// Mount Halo: its install (`open_install`) and/or the asset store, as the
/// local config's `[assets]` says (`mount::served`).
pub fn open(install: &Path) -> io::Result<Mount> {
    crate::mount::served::open(GAME, install, open_install)
}

/// Mount a Halo install: `install` is a disc image file or a folder holding
/// the disc's files.
pub fn open_install(install: &Path) -> io::Result<Mount> {
    blam_formats::disc::open_install(install)
}
