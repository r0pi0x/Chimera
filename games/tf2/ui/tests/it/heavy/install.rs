//! The user's TF2 install for the heavy tests (as `tf2`'s own heavy tests
//! find it: Steam, `[games.tf2] path` or `$CHIMERA_TF2_ROOT`), and a map of
//! another game dressed in TF2's looks.

use chimera_core::mount::{Mount, served};
use chimera_world::map::MapData;
use source_engine::content::material::MaterialLoader;
use tf2::game::{GAME, INFO, maps, mount};

/// The install, mounted, if usable.
pub(crate) fn install_mount() -> Option<Mount> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no TF2 install (Steam, [games.tf2] path, or $CHIMERA_TF2_ROOT)");
        return None;
    };
    if !served::usable(GAME, &path, "tf") {
        eprintln!("skipping: no TF2 install at {}", path.display());
        return None;
    }
    Some(mount::open(&path).expect("the install mounts"))
}

/// A map of another game (CS:S's, as de_dust2 is) with TF2's looks on it.
pub(crate) fn dressed() -> Option<MapData> {
    let mount = install_mount()?;
    let mut map = MapData::default();
    map.impacts.game = Some("cs_source".into());
    map.impacts.add("cs_source");
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    maps::add_guest_looks(&mut materials, &mut map);
    Some(map)
}
