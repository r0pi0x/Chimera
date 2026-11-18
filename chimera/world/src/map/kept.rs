//! Kept dressings: a game's parts on other games' maps (its guest
//! dressing: players, guns, HUD look, sounds, effect looks), converted once
//! per app and load kind, then copied into each map (docs/performance.md,
//! "Guest dressings kept for the session"). One API for every game: a game
//! gives a `Dressing` (its id, its converter's version, its mount and its
//! dress function onto an empty map) and its `MapDressing` calls
//! `dress_cached`; Chimera does the rest:
//!
//! - **Keeps** what the dressing made, in the app's `LoadContext` (with or
//!   without textures: `MapLoadOptions::textures`). The dressing runs once
//!   on an empty map marked `BlocksOnly` (its textures and cubemaps from 0;
//!   a texture of whole DXT blocks keeps those alone, never decoded:
//!   `MapTexture::keep_blocks_only`, what the GPU takes, and less to copy
//!   into each map). A failed dressing (no install) is not kept: the next
//!   load tries again.
//! - **Appends** a copy to every later map (`append`): its textures and
//!   cubemaps after the map's, every index its parts hold moved past them
//!   (`texture_refs`), its bodies, held and view models after the map's,
//!   and what a dressing merges by name (sound entries, decal groups,
//!   effect materials, muzzle flashes, spent shells, the HUD look) by the
//!   same rules (`add_sounds`, `add_decals`, `MapData::add_guest_hud`,
//!   `shells::add_shells`).
//! - **Refreshes** on late files: a dressing whose models missed files
//!   the game has but couldn't give then (the asset service's, not fetched
//!   yet: `MapModel::late_files`) is kept with that list; the next load
//!   asks the game's mount whether any of them can be read now and, if
//!   so, dresses again, so a file that came after the first dressing
//!   reaches the next map instead of waiting for a restart.
//! - **Logs** each conversion's time, and counts them (`conversions`).
//! - **On disk, off by default** (`CHIMERA_GUEST_CACHE=1`, `DISK_ENV`):
//!   a session's first ask reads the dressing back from the converted-map
//!   cache (`map::cache`: its folder, cap and switch, `CHIMERA_MAP_CACHE=0`
//!   turning guests off too) when one was written from the same files,
//!   else converts and writes it. The key: `guest:<game>`, the converter's
//!   version, the map format's, whether textures are decoded and the
//!   mount's layers; the entry lists every file the dressing read with its
//!   fingerprint (a late file's absence too), so a changed or arrived file
//!   converts again. The entry's bytes are the same for the same files
//!   (`pod_serde::sorted`), as the asset service's records will need
//!   (docs/plans/active/converted-cache.md). Read back: `disk_reads`.
//!
//! The Source games (CS:S, HL2:DM, TF2) dress through it. Porting a game:
//! docs/new-game.md, "Order of work", step 3.
//!
//! Known shortcuts (docs/tech-debt.md): an install changed meanwhile
//! doesn't reach a kept dressing until the app restarts, nor does a late
//! file of what isn't a model (a sound, a decal, the HUD's look); the map
//! showing keeps the dressing it was loaded with (the next map takes the
//! new one); a kept texture of its blocks alone casts an opaque shadow
//! (`render::shadows` reads texels for alpha); a dressing's memos
//! (`MapData::memos`) aren't kept, so Combat Arms (its shared models'
//! index, its effect library) and Halo (its donors, kept as their own
//! types) keep their parts their own way. Why it's OK for now: a dressing
//! reads only the install's `base` files, which come before any map
//! loads; the guests' bodies are opaque.

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use chimera_core::mount::Mount;

use super::{
    BlocksOnly, LoadContext, MapData, MapLoadOptions,
    cache::{self, MapCache, Outcome},
    decal::MapDecals,
    sound::MapSounds,
    texture_refs::TextureRefs,
};

/// What a game gives Chimera to keep its guest dressing (module doc).
pub struct Dressing<'a> {
    /// The game's id (`cs_source`).
    pub game: &'static str,
    /// Its converter's version (the hash of its sources its build.rs
    /// takes, `CHIMERA_CONVERTER_SOURCES`): what a kept copy on disk would
    /// be keyed by.
    pub converter: &'static str,
    /// The game's files (its install, the asset store or both); `Err`: no
    /// install, nothing dressed.
    pub mount: &'a dyn Fn() -> Result<Arc<Mount>, String>,
    /// The dressing, onto `map` (empty, marked `BlocksOnly`) from `mount`.
    pub convert: &'a dyn Fn(&Mount, &mut MapData),
}

/// The app's kept dressings, by game and whether textures were decoded.
#[derive(Default)]
struct Cache(Mutex<BTreeMap<(&'static str, bool), Arc<Entry>>>);

/// One game's kept dressing (a lock of its own: two loads at once convert
/// once) and how many times it was converted.
#[derive(Default)]
struct Entry {
    parts: Mutex<Option<Arc<MapData>>>,
    conversions: AtomicUsize,
    disk_reads: AtomicUsize,
}

/// The variable that turns the disk layer on (`1`; module doc).
pub const DISK_ENV: &str = "CHIMERA_GUEST_CACHE";

/// The disk layer's cache: the converted-map cache's (`MapCache::
/// from_env`) when `DISK_ENV` is `1` and that cache is on; else none.
pub fn disk_cache() -> Option<MapCache> {
    let on = std::env::var(DISK_ENV).is_ok_and(|v| v.trim() == "1");
    on.then(MapCache::from_env).flatten()
}

fn entry(context: &LoadContext, game: &'static str, textures: bool) -> Arc<Entry> {
    let cache = context.get_or_init(Cache::default);
    let mut entries = cache.0.lock().unwrap_or_else(|e| e.into_inner());
    entries.entry((game, textures)).or_default().clone()
}

/// The game's guest parts into `map`: converted the first time the app
/// (`options.context`) asks with these options' `textures`, copied from
/// then on; converted again when the kept one missed late files and one
/// of them can be read now (module doc).
pub fn dress_cached(map: &mut MapData, options: &MapLoadOptions, dressing: &Dressing) -> Result<(), String> {
    let parts = kept(options, dressing)?;
    append(map, &parts);
    Ok(())
}

/// How many times `game`'s dressing was converted for the app of
/// `context` (with textures or without).
pub fn conversions(context: &LoadContext, game: &'static str, textures: bool) -> usize {
    entry(context, game, textures).conversions.load(Ordering::Relaxed)
}

/// How many times `game`'s dressing was read back from the disk layer
/// for the app of `context`.
pub fn disk_reads(context: &LoadContext, game: &'static str, textures: bool) -> usize {
    entry(context, game, textures).disk_reads.load(Ordering::Relaxed)
}

/// The files a dressing's models missed (`MapModel::late_files`), each
/// once.
pub fn late_files(parts: &MapData) -> Vec<String> {
    let kit = &parts.kit;
    let models = kit
        .characters
        .iter()
        .map(|c| &c.model)
        .chain(kit.held.iter().map(|h| &h.model))
        .chain(kit.view_models.iter().map(|v| &v.model));
    let mut out: Vec<String> = Vec::new();
    for f in models.flat_map(|m| &m.late_files) {
        if !out.contains(f) {
            out.push(f.clone());
        }
    }
    out
}

/// The game's kept parts (converted on an empty map) for the app of
/// `options` (module doc), the disk layer from the environment
/// (`disk_cache`).
pub fn kept(options: &MapLoadOptions, dressing: &Dressing) -> Result<Arc<MapData>, String> {
    kept_with(options, dressing, disk_cache().as_ref())
}

/// `kept` with `disk` as the disk layer (none: always converted).
pub fn kept_with(
    options: &MapLoadOptions,
    dressing: &Dressing,
    disk: Option<&MapCache>,
) -> Result<Arc<MapData>, String> {
    let game = dressing.game;
    let entry = entry(&options.context, game, options.textures);
    let mut parts = entry.parts.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = parts.as_ref() {
        let late = late_files(p);
        if late.is_empty() {
            return Ok(p.clone());
        }
        let here = (dressing.mount)().is_ok_and(|m| late.iter().any(|f| m.read(f).is_ok()));
        if !here {
            return Ok(p.clone());
        }
        bevy::log::info!(
            "{game}: its guest parts dressed again (of {} files it missed, one is here now)",
            late.len()
        );
    }
    let started = std::time::Instant::now();
    let mount = (dressing.mount)()?;
    let key = cache::key(&format!("guest:{game}"), dressing.converter).part("textures", [options.textures as u8]);
    let (scratch, outcome) = cache::cached(disk, &mount, key, || {
        let mut scratch = MapData::default();
        scratch.memos.insert(BlocksOnly);
        (dressing.convert)(&mount, &mut scratch);
        scratch.memos.remove::<BlocksOnly>();
        for t in &mut scratch.textures {
            t.keep_blocks_only();
        }
        Ok(scratch)
    })?;
    let how = if outcome == Outcome::Hit {
        entry.disk_reads.fetch_add(1, Ordering::Relaxed);
        "read from disk"
    } else {
        entry.conversions.fetch_add(1, Ordering::Relaxed);
        if outcome == Outcome::Miss {
            "converted and written to disk"
        } else {
            "converted"
        }
    };
    bevy::log::info!(
        "{game}: its guest parts {how} in {} ms ({} textures, {} bodies, {} held, {} view models)",
        started.elapsed().as_millis(),
        scratch.textures.len(),
        scratch.kit.characters.len(),
        scratch.kit.held.len(),
        scratch.kit.view_models.len()
    );
    let p = Arc::new(scratch);
    *parts = Some(p.clone());
    Ok(p)
}

/// A guest's parts (converted on an empty map) into `map`, as its
/// dressing would have added them (module doc).
pub fn append(map: &mut MapData, parts: &MapData) {
    let (t, c) = (map.textures.len(), map.cubemaps.len());
    fn moved<T: Clone + TextureRefs>(x: &T, t: usize, c: usize) -> T {
        let mut x = x.clone();
        x.shift(t, c);
        x
    }
    map.textures.extend(parts.textures.iter().cloned());
    map.cubemaps.extend(parts.cubemaps.iter().cloned());
    map.texture_anims
        .extend(parts.texture_anims.iter().map(|a| moved(a, t, c)));
    let kit = &parts.kit;
    map.kit.characters.extend(kit.characters.iter().map(|x| moved(x, t, c)));
    map.kit.held.extend(kit.held.iter().map(|x| moved(x, t, c)));
    map.kit
        .view_models
        .extend(kit.view_models.iter().map(|x| moved(x, t, c)));
    for look in kit.hud.iter().chain(&kit.guest_huds) {
        map.kit.add_guest_hud(Some(Arc::new(moved(&**look, t, c))));
    }
    add_decals(&mut map.kit.decals, moved(&kit.decals, t, c));
    for m in &parts.particles.materials {
        if map.particles.find(&m.name).is_none() {
            map.particles.materials.push(moved(m, t, c));
        }
    }
    for game in &parts.impacts.looks {
        map.impacts.add(game);
    }
    if map.kit.muzzle_flash.is_none() {
        map.kit.muzzle_flash = kit.muzzle_flash.as_ref().map(|m| moved(m, t, c));
    }
    for (kind, flash) in &kit.muzzle_flash_kinds {
        if !map
            .kit
            .muzzle_flash_kinds
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case(kind))
        {
            map.kit.muzzle_flash_kinds.push((kind.clone(), moved(flash, t, c)));
        }
    }
    super::shells::add_shells(&mut map.kit.shells, kit.shells.iter().map(|s| moved(s, t, c)).collect());
    add_sounds(Arc::make_mut(&mut map.sounds), (*parts.sounds).clone());
    map.warnings.extend(parts.warnings.iter().cloned());
}

/// A guest's decals added to a map's: appended, each group, material
/// letter and spray logo the map lacks pointing at them (the map's own
/// stay).
pub fn add_decals(map: &mut MapDecals, guest: MapDecals) {
    let base = map.decals.len();
    map.decals.extend(guest.decals);
    let mut groups: Vec<_> = guest
        .groups
        .into_iter()
        .filter(|(k, _)| !map.groups.contains_key(k))
        .collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, variants) in groups {
        map.groups
            .insert(name, variants.into_iter().map(|(w, i)| (w, i + base)).collect());
    }
    for (letter, group) in guest.by_material {
        map.by_material.entry(letter).or_insert(group);
    }
    for (name, i) in guest.logos {
        map.logos.entry(name).or_insert(i + base);
    }
}

/// A guest's sound entries added to a map's: each entry the map lacks,
/// its clips appended (their indices moved past the map's own), and each
/// surface the map lacks. The map's entries, surfaces and soundscapes
/// stay.
pub fn add_sounds(map: &mut MapSounds, guest: MapSounds) {
    for (name, surface) in guest.surfaces {
        map.surfaces.entry(name).or_insert(surface);
    }
    let base = map.clips.len();
    let mut used = std::collections::BTreeMap::new();
    let mut entries: Vec<_> = guest
        .entries
        .into_iter()
        .filter(|(k, _)| !map.entries.contains_key(k))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    for (name, mut entry) in entries {
        for w in &mut entry.waves {
            let next = base + used.len();
            *w = *used.entry(*w).or_insert(next);
        }
        map.entries.insert(name, entry);
    }
    let mut order: Vec<(usize, usize)> = used.into_iter().collect();
    order.sort_by_key(|&(_, new)| new);
    map.clips
        .extend(order.into_iter().map(|(old, _)| guest.clips[old].clone()));
}

#[cfg(test)]
mod tests {
    use bevy::prelude::default;

    use super::*;
    use crate::map::{MapHeldModel, MapModel, MapTexture};

    /// A store that lists a file but gives it only once it "arrived"
    /// (an asset service's record not fetched yet).
    #[derive(Clone, Default)]
    struct Late(Arc<Mutex<bool>>);

    impl chimera_core::mount::FileSource for Late {
        fn name(&self) -> String {
            "late".into()
        }
        fn entries(&self) -> Vec<chimera_core::mount::Entry> {
            vec![chimera_core::mount::Entry {
                path: "late.bin".into(),
                size: 0,
            }]
        }
        fn read(&self, path: &str) -> Option<std::io::Result<Vec<u8>>> {
            (path == "late.bin").then(|| {
                if *self.0.lock().unwrap() {
                    Ok(vec![1])
                } else {
                    Err(std::io::Error::other("asset miss"))
                }
            })
        }
        fn contains(&self, path: &str) -> bool {
            path == "late.bin"
        }
    }

    fn texture(name: &str) -> MapTexture {
        MapTexture {
            name: name.into(),
            srgb: true,
            mips: Vec::new(),
            width: 1,
            height: 1,
            rgba8: vec![9; 4],
            bc: None,
        }
    }

    /// A dressing that adds a texture and a held model pointing at it,
    /// late on `late.bin` until it arrives; the dressing sees `BlocksOnly`.
    fn convert(mount: &Mount, map: &mut MapData) {
        assert!(map.memos.get::<BlocksOnly>().is_some(), "dressed as kept");
        assert!(map.textures.is_empty(), "on an empty map");
        let here = mount.read("late.bin").is_ok();
        map.textures.push(texture(if here { "full" } else { "partial" }));
        let mut model = MapModel {
            late_files: if here { Vec::new() } else { vec!["late.bin".into()] },
            ..default()
        };
        model.meshes.push(crate::map::MapMesh {
            texture: Some(0),
            ..default()
        });
        map.kit.held.push(MapHeldModel {
            key: "test:gun".into(),
            model,
            bone: String::new(),
            muzzle: None,
            attachments: Vec::new(),
            merged: Vec::new(),
            own: None,
        });
    }

    /// Converted once, copied after (its texture index moved past each
    /// map's own), dressed again when a late file arrives, then kept.
    #[test]
    fn a_dressing_is_kept_appended_and_refreshed_on_late_files() {
        let late = Late::default();
        let mount = {
            let mut m = Mount::default();
            m.push(late.clone());
            Arc::new(m)
        };
        let options = MapLoadOptions::lean();
        let mount_of = || Ok(mount.clone());
        let dressing = Dressing {
            game: "kept_test",
            converter: "v1",
            mount: &mount_of,
            convert: &convert,
        };
        let load = || {
            let mut map = MapData::default();
            map.textures.push(texture("own"));
            dress_cached(&mut map, &options, &dressing).unwrap();
            map
        };
        let first = load();
        assert_eq!(first.textures.len(), 2);
        assert_eq!(
            first.kit.held[0].model.meshes[0].texture,
            Some(1),
            "moved past the map's own"
        );
        assert_eq!(first.textures[1].name, "partial");
        load();
        assert_eq!(
            conversions(&options.context, "kept_test", false),
            1,
            "copied, not converted"
        );
        *late.0.lock().unwrap() = true;
        let third = load();
        assert_eq!(
            conversions(&options.context, "kept_test", false),
            2,
            "the late file arrived"
        );
        assert_eq!(third.textures[1].name, "full");
        load();
        assert_eq!(conversions(&options.context, "kept_test", false), 2, "complete: kept");
    }

    /// A file whose bytes a test changes (a patched install).
    #[derive(Clone)]
    struct Patched(Arc<Mutex<Vec<u8>>>);

    impl chimera_core::mount::FileSource for Patched {
        fn name(&self) -> String {
            "patched".into()
        }
        fn entries(&self) -> Vec<chimera_core::mount::Entry> {
            vec![chimera_core::mount::Entry {
                path: "look.txt".into(),
                size: 0,
            }]
        }
        fn read(&self, path: &str) -> Option<std::io::Result<Vec<u8>>> {
            (path == "look.txt").then(|| Ok(self.0.lock().unwrap().clone()))
        }
    }

    /// The disk layer: a second app reads the first one's dressing back
    /// (the same parts, nothing converted), and a changed input file
    /// converts again.
    #[test]
    fn the_disk_layer_reads_back_and_misses_on_a_changed_file() {
        let dir = std::env::temp_dir().join(format!("chimera-kept-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let disk = MapCache::at(&dir, u64::MAX);
        let file = Patched(Arc::new(Mutex::new(b"first".to_vec())));
        let mount = {
            let mut m = Mount::default();
            m.push(file.clone());
            Arc::new(m)
        };
        let mount_of = || Ok(mount.clone());
        let convert = |mount: &Mount, map: &mut MapData| {
            let name = String::from_utf8(mount.read("look.txt").unwrap()).unwrap();
            map.textures.push(texture(&name));
        };
        let dressing = Dressing {
            game: "kept_disk",
            converter: "v1",
            mount: &mount_of,
            convert: &convert,
        };
        let app = |disk: &MapCache| {
            let options = MapLoadOptions::lean();
            let parts = kept_with(&options, &dressing, Some(disk)).unwrap();
            let counts = (
                conversions(&options.context, "kept_disk", false),
                disk_reads(&options.context, "kept_disk", false),
            );
            (parts.textures[0].name.clone(), counts)
        };
        assert_eq!(app(&disk), ("first".into(), (1, 0)), "converted and written");
        assert_eq!(app(&disk), ("first".into(), (0, 1)), "another app reads it back");
        *file.0.lock().unwrap() = b"second".to_vec();
        assert_eq!(app(&disk), ("second".into(), (1, 0)), "a changed file converts again");
        assert_eq!(app(&disk), ("second".into(), (0, 1)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// No install: nothing kept, the next load tries again.
    #[test]
    fn a_failed_dressing_is_not_kept() {
        let options = MapLoadOptions::lean();
        let none = || Err("no install".to_string());
        let dressing = Dressing {
            game: "kept_none",
            converter: "v1",
            mount: &none,
            convert: &convert,
        };
        assert!(dress_cached(&mut MapData::default(), &options, &dressing).is_err());
        assert!(dress_cached(&mut MapData::default(), &options, &dressing).is_err());
        assert_eq!(conversions(&options.context, "kept_none", false), 0);
    }
}
