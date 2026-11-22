//! The user's Halo disc image for the heavy tests (as `halo_ce`'s own heavy
//! tests find it: `$CHIMERA_HALO_CE_ROOT` or `[games.halo_ce] path`).

use std::sync::Arc;

use chimera_core::mount::{Mount, served};
use halo_ce::game::{GAME, INFO};

/// The disc image mounted, if there is one.
pub fn mount() -> Option<Arc<Mount>> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return None;
    };
    if !served::usable(GAME, &path, "") {
        eprintln!("skipping: no Halo CE install at {}", path.display());
        return None;
    }
    Some(Arc::new((INFO.open)(&path).unwrap()))
}
