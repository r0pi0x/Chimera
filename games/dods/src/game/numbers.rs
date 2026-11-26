//! DoD:S's weapons' numbers kept between starts. `DodsGunsPlugin` reads
//! each weapon's script through DoD:S's mount (`weapons::script_numbers::
//! GunNumbers::from_scripts`) unless the app already has the numbers as
//! a resource, and mounting DoD:S opens every archive's directory twice
//! (`mount::search_path`): 0.6 G instructions at every start for some
//! thirty small scripts (docs/performance.md, "Start cost, fourth pass").
//! `DodsNumbersPlugin`, before it, gives the app the numbers read back
//! from the converted cache when the same mount gave them before
//! (`kept`: keyed by the mount unopened, `mount::unopened`, so a hit
//! opens no archive), else reads them as `from_scripts` does and keeps
//! them. Never while the reads are recorded (`CHIMERA_VFS_LOG`: a read
//! back reads no file, and the record is what the asset store packs).
//! Numbers with a script missing (a file not fetched yet) aren't kept:
//! they are read again at the next start.

use std::collections::BTreeSet;

use bevy::prelude::*;

use super::weapons::script_numbers::{GunNumbers, GunSource};
use crate::mount::{
    Access, Mount,
    assets::map_cache::{Key, MapCache, Outcome, cached_unopened},
};

/// The version of what `read` gives, for its entry in the converted
/// cache: the hash of the mapping's sources and the documents'
/// (build.rs, `NUMBERS`).
pub const NUMBERS_READER: &str = env!("CHIMERA_DODS_NUMBER_SOURCES");

/// The numbers as the entry holds them (the documents; each weapon's
/// id, whether its script set a number, and its source).
type Kept = (Vec<(String, String)>, Vec<(String, bool, String)>);

/// The numbers with each weapon's script read through `mount` (None:
/// every one's the spec fallback), as `GunNumbers::from_mount`, and
/// whether every script asked for was there.
pub fn read(mount: Option<&Mount>) -> (GunNumbers, bool) {
    let missing = std::cell::Cell::new(false);
    let numbers = GunNumbers::with_scripts(|name| {
        let text = mount.and_then(|m| m.read(&format!("scripts/{name}.txt")).ok()).map(|b| String::from_utf8_lossy(&b).into_owned());
        if text.is_none() {
            missing.set(true);
        }
        text
    });
    (numbers, !missing.get())
}

/// `GunNumbers::from_scripts`, read back from the converted cache when
/// the same mount gave them before (module doc).
pub fn from_scripts() -> GunNumbers {
    let disk = if crate::mount::assets::Recorder::from_env().is_some() { None } else { chimera_world::map::kept::disk_cache() };
    from_scripts_kept(disk.as_ref()).0
}

/// `from_scripts` with `disk` as the cache, and how it went.
pub fn from_scripts_kept(disk: Option<&MapCache>) -> (GunNumbers, Outcome) {
    let whole = || {
        let mount = super::INFO.mount();
        let tracking = mount.as_ref().map(|m| m.track());
        let (numbers, whole) = read(mount.as_ref());
        (numbers, tracking.map(|t| t.finish()).unwrap_or_default(), whole)
    };
    let unopened = disk.and_then(|_| super::INFO.install()).and_then(|i| super::mount::unopened(&i).ok());
    let Some((light, archives)) = unopened else { return (whole().0, Outcome::Off) };
    kept(disk, &light, &archives, whole)
}

/// `read`'s numbers (with what it asked of the mount and whether every
/// script was there), kept in `disk` for the mount `light` and
/// `archives` name (`map_cache::cached_unopened`).
pub fn kept(
    disk: Option<&MapCache>,
    light: &Mount,
    archives: &[std::path::PathBuf],
    read: impl FnOnce() -> (GunNumbers, BTreeSet<Access>, bool),
) -> (GunNumbers, Outcome) {
    let mut unkept = None;
    let kept = cached_unopened(disk, light, archives, Key::new("dods:gun_numbers", NUMBERS_READER), || {
        let (numbers, asked, whole) = read();
        if whole {
            let sources = numbers.sources.iter().map(|s| (s.gun.clone(), s.from_script, s.source.clone())).collect();
            Ok(((numbers.docs, sources) as Kept, asked))
        } else {
            unkept = Some(numbers);
            Err(String::new())
        }
    });
    match (kept, unkept) {
        (Ok(((docs, sources), outcome)), _) => {
            let sources = sources.into_iter().map(|(gun, from_script, source)| GunSource { gun, from_script, source }).collect();
            (GunNumbers { docs, sources }, outcome)
        }
        (Err(_), Some(numbers)) => (numbers, Outcome::Off),
        (Err(e), None) => {
            warn!("dods: the weapons' numbers weren't read ({e}): the spec's");
            (GunNumbers::builtin(), Outcome::Off)
        }
    }
}

/// The app's numbers before `DodsGunsPlugin` asks for them: `from_scripts`
/// unless it has them already (the harness's built-in ones).
pub struct DodsNumbersPlugin;

impl Plugin for DodsNumbersPlugin {
    fn build(&self, app: &mut App) {
        if app.world().get_resource::<GunNumbers>().is_none() {
            app.insert_resource(from_scripts());
        }
    }
}
