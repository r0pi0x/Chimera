//! A game's map list (`MapList`: the maps its install holds and its map
//! cycle) read back from the converted cache without mounting the
//! install (`GameInfo::map_list`). Listing a Source game's maps opens
//! every archive of its search path and walks every file of the mount;
//! a party app did that for each game in its first frame, on the main
//! thread (the pool's installed maps: chimera_sim's
//! `auto::find_installed_maps`), and a client once more on a task for the
//! console's `map` command: 5.1 G instructions with six games
//! (docs/performance.md, "Start cost, fifth pass").
//!
//! A game that can name its mount without opening it (`GameInfo::
//! unopened`: its loose folders and the asset store, and the archive
//! files it would open) has its list kept as an entry of the converted
//! cache (chimera_assets' `map_cache::cached_unopened`). The entry's key
//! names the archives by path, size and modification time and holds the
//! maps the other layers list now (a map dropped into a loose folder or
//! a new store manifest is another key), and its inputs are the files the
//! cycle read, checked against those layers. A hit opens no archive.
//!
//! Known shortcuts: a game without `unopened` (Combat Arms, Halo, CS 1.6:
//! their mounts cost little) mounts as before; so does every game while
//! the reads are recorded (`CHIMERA_VFS_LOG`) or with the cache off.

use std::path::Path;

use chimera_assets::map_cache::{Key, MapCache, Outcome, cached_unopened};

use super::GameInfo;
use crate::mount::Mount;

/// The kept list's version: bump it when what `MapList::of` gives for
/// the same mount changes.
const READER: &str = "1";

/// The maps a game's install holds and the cycle a server plays in turn.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MapList {
    /// `GameInfo::map_names`' names, as it gives them.
    pub names: Vec<String>,
    /// `GameInfo::map_cycle`'s.
    pub cycle: Vec<String>,
}

impl MapList {
    /// `game`'s list from its install mounted as `mount`.
    pub fn of(game: &GameInfo, mount: &Mount) -> Self {
        Self { names: (game.map_names)(mount), cycle: (game.map_cycle)(mount) }
    }
}

impl GameInfo {
    /// The install's map list, read back from the converted cache when
    /// the same install gave it before (module doc); None without an
    /// install that opens.
    pub fn map_list(&self) -> Option<MapList> {
        let disk = if chimera_assets::Recorder::from_env().is_some() { None } else { MapCache::from_env() };
        Some(kept(self, &self.install()?, disk.as_ref())?.0)
    }

    /// The install's map cycle alone: the kept list's (`map_list`) for a
    /// game that names its mount unopened, else read from the mount as
    /// before, without listing the mount's maps (a server asks in the
    /// tick its map changes); None without an install that opens.
    pub fn installed_cycle(&self) -> Option<Vec<String>> {
        if self.unopened.is_some() {
            return self.map_list().map(|l| l.cycle);
        }
        Some((self.map_cycle)(&self.mount()?))
    }
}

/// `game`'s list from the install at `install`, with `disk` as the cache,
/// and how it went (`Outcome::Off`: mounted and listed).
pub fn kept(game: &GameInfo, install: &Path, disk: Option<&MapCache>) -> Option<(MapList, Outcome)> {
    let whole = || {
        let mount = (game.open)(install).map_err(|e| e.to_string())?;
        let tracking = mount.track();
        let list = MapList::of(game, &mount);
        Ok::<_, String>((list, tracking.finish()))
    };
    let unopened = disk.and(game.unopened).and_then(|open| open(install).ok());
    // No archive to leave closed: the other layers are the mount.
    let Some((light, archives)) = unopened.filter(|(_, archives)| !archives.is_empty()) else {
        return whole().ok().map(|(list, _)| (list, Outcome::Off));
    };
    let key = Key::new(&format!("{}:map_list", game.id), READER).part("light maps", (game.map_names)(&light).join("\n"));
    cached_unopened(disk, &light, &archives, key, || whole().map(|(l, asked)| ((l.names, l.cycle), asked)))
        .ok()
        .map(|((names, cycle), outcome)| (MapList { names, cycle }, outcome))
}
