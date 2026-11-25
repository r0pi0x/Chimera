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
//!   effect materials, particle systems, muzzle flashes, spent shells, the
//!   HUD look) by the
//!   same rules (`add_sounds`, `add_decals`, `MapData::add_guest_hud`,
//!   `shells::add_shells`); a guest's impact looks (decal groups and
//!   material letters, surfaces and their bullet impact entries, effect
//!   materials) whose names are taken are kept under its own names too
//!   (`map::impact::own_name`, `add_decals_of`, `add_sounds_of`), so its
//!   guns' impacts are its own on any map.
//! - **Refreshes** on late files: a dressing whose models missed files
//!   the game has but couldn't give then (the asset service's, not fetched
//!   yet: `MapModel::late_files`) is kept with that list; the next load
//!   asks the game's mount whether any of them can be read now and, if
//!   so, dresses again, so a file that came after the first dressing
//!   reaches the next map instead of waiting for a restart.
//! - **Logs** each conversion's (or read's) time and instructions (this
//!   thread and those it starts), counts them (`conversions`) and keeps
//!   them as a phase of the load (`map::load_report`: `kept <game>: <how>`).
//! - **On disk, on by default** (`CHIMERA_GUEST_CACHE=0` turns it off,
//!   `DISK_ENV`; on since 2026-11-17, plan converted-cache step 6: a cold
//!   first dressing read back takes 1.7x (CS:S) to 6.8x (HL2:DM) fewer
//!   instructions and 3-7x less time in the playtest build, the same
//!   bytes): a session's first ask reads the dressing back from the converted-map
//!   cache (`map::cache`: its folder, cap and switch, `CHIMERA_MAP_CACHE=0`
//!   turning guests off too) when one was written from the same files,
//!   else converts and writes it. The key: `guest:<game>`, the converter's
//!   version, the map format's, whether textures are decoded, the
//!   dressing's `variant` and the mount's layers; the entry lists every
//!   file the dressing read with its fingerprint (a late file's absence
//!   too), so a changed or arrived file converts again. The entry's bytes
//!   are the same for the same files (`pod_serde::sorted`), as the asset
//!   service's records need (docs/plans/active/converted-cache.md).
//!   Read back: `disk_reads`.
//! - **Memos**: what a dressing leaves in the map's memos (`MapData::
//!   memos`: Combat Arms' rigs, body groups and effect library) is kept
//!   with its parts when the game gives `Dressing::memos` (`KeptMemos`):
//!   its `append` adds them to each map; on disk its `save` writes them
//!   beside the parts (`map::KeptMemoBytes`, taken out again after the
//!   read) and its `load` reads them back (memos that don't read back
//!   convert again, as a miss).
//! - **Served**: on a miss the disk layer first takes the converted
//!   entry the asset service serves for this game, converter, map format
//!   and options, when its inputs are the app's mount's files (the store
//!   alone, as players have it: `cache::cached_served`, named
//!   `served_name`); counted by `served_reads`. The packer
//!   (`chimera_pack convert`) puts an `Export` in the app's context and
//!   gets each entry its dressings write instead.
//! - **Variants**: an app's choice that changes the dressing (Combat
//!   Arms' view models, none when they load on demand) is the dressing's
//!   `variant`, kept apart in the session and on disk.
//!
//! The Source games (CS:S, HL2:DM, TF2) and Combat Arms dress through it.
//! Porting a game: docs/new-game.md, "Order of work", step 3.
//!
//! Known shortcuts (docs/tech-debt.md): an install changed meanwhile
//! doesn't reach a kept dressing until the app restarts, nor does a late
//! file of what isn't a model (a sound, a decal, the HUD's look); the map
//! showing keeps the dressing it was loaded with (the next map takes the
//! new one); a kept texture of its blocks alone casts an opaque shadow
//! (`render::shadows` reads texels for alpha); Halo keeps its donors as
//! its own types, outside this module (docs/plans/active/
//! converted-cache.md, step 5, says how they would move here); a hit
//! whose memos don't read back converts without rewriting the entry, so
//! it does so again in each session until the entry changes. Why it's OK
//! for now: a dressing reads only the install's `base` files, which come
//! before any map loads; the guests' bodies are opaque; memos that are
//! written read back (a heavy test checks Combat Arms').

use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use chimera_core::mount::Mount;

use super::{
    BlocksOnly, LoadContext, MapData, MapLoadOptions, MapMemos,
    extras::KeptMemoBytes,
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
    /// What else the dressing depends on, besides the game, its converter,
    /// the mount and `MapLoadOptions::textures` (an app's choice: Combat
    /// Arms' view models loaded on demand or not); `""`: nothing. Part of
    /// the session's key and the disk's.
    pub variant: &'static str,
    /// The memos its dressing leaves (`MapData::memos`) that later maps
    /// need (module doc, "Memos"); None: none.
    pub memos: Option<&'a KeptMemos<'a>>,
}

/// How a game keeps the memos its dressing leaves (`MapData::memos`:
/// Combat Arms' rigs, body groups and effect library): copied into each
/// map (`append`) and, for the disk layer, written as bytes beside the
/// parts (`map::KeptMemoBytes`) and read back.
pub struct KeptMemos<'a> {
    /// The kept memos as bytes (the same for the same memos: the
    /// generated-assets rule).
    pub save: &'a dyn Fn(&MapMemos) -> Result<Vec<u8>, String>,
    /// `save`'s bytes back into `memos`, with the game's files at hand
    /// (`Err`: the entry is converted again, as a miss).
    pub load: &'a dyn Fn(&[u8], &Mount, &mut MapMemos) -> Result<(), String>,
    /// The kept memos (the second) added to a map's (the first), as the
    /// dressing would have left them.
    pub append: &'a dyn Fn(&mut MapMemos, &MapMemos),
}

/// The app's kept dressings, by game and whether textures were decoded.
#[derive(Default)]
struct Cache(Mutex<BTreeMap<(&'static str, &'static str, bool), Arc<Entry>>>);

/// One game's kept dressing (a lock of its own: two loads at once convert
/// once) and how many times it was converted.
#[derive(Default)]
struct Entry {
    parts: Mutex<Option<Arc<MapData>>>,
    conversions: AtomicUsize,
    disk_reads: AtomicUsize,
    served_reads: AtomicUsize,
}

/// The packer's request in an app's context (`chimera_pack convert`):
/// each dressing of that context hands over the entry its disk layer
/// wrote or read (`cache::Served::export`), to serve from the asset
/// service (module doc, "Served").
#[derive(Default)]
pub struct Export(pub Mutex<Vec<cache::ServedEntry>>);

/// The variable that turns the disk layer off (`0`, `off`, `false`,
/// `no`; module doc).
pub const DISK_ENV: &str = "CHIMERA_GUEST_CACHE";

/// Whether `DISK_ENV`'s value (None: unset) leaves the disk layer on.
pub fn disk_on(value: Option<&str>) -> bool {
    !value.is_some_and(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "0" | "off" | "false" | "no"))
}

/// The disk layer's cache: the converted-map cache's (`MapCache::
/// from_env`, none when `CHIMERA_MAP_CACHE=0`) unless `DISK_ENV` turns
/// it off.
pub fn disk_cache() -> Option<MapCache> {
    disk_on(std::env::var(DISK_ENV).ok().as_deref())
        .then(MapCache::from_env)
        .flatten()
}

fn entry(context: &LoadContext, game: &'static str, variant: &'static str, textures: bool) -> Arc<Entry> {
    let cache = context.get_or_init(Cache::default);
    let mut entries = cache.0.lock().unwrap_or_else(|e| e.into_inner());
    entries.entry((game, variant, textures)).or_default().clone()
}

/// The game's guest parts into `map`: converted the first time the app
/// (`options.context`) asks with these options' `textures`, copied from
/// then on; converted again when the kept one missed late files and one
/// of them can be read now (module doc).
pub fn dress_cached(map: &mut MapData, options: &MapLoadOptions, dressing: &Dressing) -> Result<(), String> {
    dress_cached_with(map, options, dressing, disk_cache().as_ref())
}

/// `dress_cached` with `disk` as the disk layer (`kept_with`).
pub fn dress_cached_with(
    map: &mut MapData,
    options: &MapLoadOptions,
    dressing: &Dressing,
    disk: Option<&MapCache>,
) -> Result<(), String> {
    let parts = kept_with(options, dressing, disk)?;
    append(map, &parts);
    if let Some(memos) = dressing.memos {
        (memos.append)(&mut map.memos, &parts.memos);
    }
    Ok(())
}

/// How many times `game`'s dressing (of `variant`, `Dressing::variant`)
/// was converted for the app of `context` (with textures or without).
pub fn conversions(context: &LoadContext, game: &'static str, variant: &'static str, textures: bool) -> usize {
    entry(context, game, variant, textures).conversions.load(Ordering::Relaxed)
}

/// How many times `game`'s dressing was read back from the disk layer
/// for the app of `context`.
pub fn disk_reads(context: &LoadContext, game: &'static str, variant: &'static str, textures: bool) -> usize {
    entry(context, game, variant, textures).disk_reads.load(Ordering::Relaxed)
}

/// How many times `game`'s dressing was taken from the asset service
/// (a served entry, module doc) for the app of `context`.
pub fn served_reads(context: &LoadContext, game: &'static str, variant: &'static str, textures: bool) -> usize {
    entry(context, game, variant, textures).served_reads.load(Ordering::Relaxed)
}

/// The name a dressing's served entries go by (`cache::Served::name`):
/// `guest/<game>/<converter>`, one folder per converter version.
pub fn served_name(game: &str, converter: &str) -> String {
    format!("guest/{game}/{converter}")
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
    let entry = entry(&options.context, game, dressing.variant, options.textures);
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
    let mut running = super::load_report::Running::start();
    let mount = (dressing.mount)()?;
    let key = cache::key(&format!("guest:{game}"), dressing.converter)
        .part("textures", [options.textures as u8])
        .part("variant", dressing.variant.as_bytes());
    let export = options.context.get::<Export>();
    let served = cache::Served {
        name: served_name(game, dressing.converter),
        parts: [
            ("game", game.to_string()),
            ("converter", dressing.converter.to_string()),
            ("format", cache::format()),
            ("textures", (options.textures as u8).to_string()),
            ("variant", dressing.variant.to_string()),
        ]
        .into_iter()
        .map(|(n, v)| (n.to_string(), v))
        .collect(),
        export: export.as_deref().map(|e| &e.0),
    };
    let mut outcome = Outcome::Off;
    let mut scratch = None;
    // A hit whose memos don't read back converts again (once).
    for disk in [disk, None] {
        let (mut parts, how) = cache::counted(|| cache::cached_served(disk, &mount, key.clone(), disk.and(Some(&served)), || {
            let mut scratch = MapData::default();
            scratch.memos.insert(BlocksOnly);
            (dressing.convert)(&mount, &mut scratch);
            scratch.memos.remove::<BlocksOnly>();
            for t in &mut scratch.textures {
                t.keep_blocks_only();
            }
            if let Some(memos) = dressing.memos.filter(|_| disk.is_some()) {
                let bytes = (memos.save)(&scratch.memos)?;
                scratch.extras.insert(KeptMemoBytes(bytes));
            }
            Ok(scratch)
        }))?;
        let bytes = parts.extras.get_mut::<KeptMemoBytes>().map(|b| std::mem::take(&mut b.0));
        parts.extras.remove::<KeptMemoBytes>();
        if matches!(how, Outcome::Hit | Outcome::Served)
            && let Some(memos) = dressing.memos
        {
            let read = bytes
                .ok_or_else(|| "no memos in the entry".to_string())
                .and_then(|b| (memos.load)(&b, &mount, &mut parts.memos));
            if let Err(e) = read {
                bevy::log::warn!("{game}: its kept guest parts' memos don't read back ({e}): converted again");
                continue;
            }
        }
        outcome = how;
        scratch = Some(parts);
        break;
    }
    let scratch = scratch.expect("converted without the disk");
    let how = if outcome == Outcome::Hit {
        entry.disk_reads.fetch_add(1, Ordering::Relaxed);
        "read from disk"
    } else if outcome == Outcome::Served {
        entry.served_reads.fetch_add(1, Ordering::Relaxed);
        "taken as the asset service serves them"
    } else {
        entry.conversions.fetch_add(1, Ordering::Relaxed);
        if outcome == Outcome::Miss {
            "converted and written to disk"
        } else {
            "converted"
        }
    };
    let instructions = running
        .end(&options.context, format!("kept {game}: {how}"))
        .instructions
        .map_or(String::new(), |i| format!(", {} M instructions", i / 1_000_000));
    bevy::log::info!(
        "{game}: its guest parts {how} in {} ms{instructions} ({} textures, {} bodies, {} held, {} view models)",
        started.elapsed().as_millis(),
        scratch.textures.len(),
        scratch.kit.characters.len(),
        scratch.kit.held.len(),
        scratch.kit.view_models.len()
    );
    if std::env::var_os("CHIMERA_MAP_CACHE_LOG").is_some() {
        for (part, bytes) in cache::encoded_sizes(&scratch) {
            eprintln!("map cache: {game}'s guest parts ({}): {part}: {} MB", if options.textures { "textures" } else { "lean" }, bytes >> 20);
        }
    }
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
    // The game whose impact looks these are (one dressing's game).
    let game = match &parts.impacts.looks[..] {
        [g] => Some(g.as_str()),
        _ => None,
    };
    add_decals_of(&mut map.kit.decals, moved(&kit.decals, t, c), game);
    // Each of the guest's effect materials as the map now holds it (its
    // own, or the map's of the same name): its particle systems point at
    // them.
    let mut material_at = Vec::with_capacity(parts.particles.materials.len());
    for m in &parts.particles.materials {
        let at = if let Some(i) = map.particles.find(&m.name) {
            match game {
                Some(g) => {
                    let own = super::impact::own_name(g, &m.name);
                    Some(match map.particles.find(&own) {
                        Some(i) => i,
                        None => {
                            let mut m = moved(m, t, c);
                            m.name = own;
                            map.particles.materials.push(m);
                            map.particles.materials.len() - 1
                        }
                    })
                }
                None => Some(i),
            }
        } else {
            map.particles.materials.push(moved(m, t, c));
            Some(map.particles.materials.len() - 1)
        };
        material_at.push(at);
    }
    add_particle_systems(map, &parts.particle_systems, &material_at);
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
    add_sounds_of(Arc::make_mut(&mut map.sounds), (*parts.sounds).clone(), game);
    map.warnings.extend(parts.warnings.iter().cloned());
}

/// A guest's particle systems (its guns' explosions, muzzle flashes and
/// flames: TF2's `pcf::add_guest_particle_systems`, which its dressing
/// ran on the empty map) added to the map's, as that function adds them:
/// each the map lacks by name, after the map's, its children renumbered
/// and its material moved to where the map now holds it (`material_at`,
/// by the guest's index). Before, a kept dressing dropped them: a guest's
/// guns showed no explosion or muzzle flash on any map but their own.
fn add_particle_systems(map: &mut MapData, guest: &super::psys::MapParticleSystems, material_at: &[Option<usize>]) {
    if guest.defs.is_empty() {
        return;
    }
    let out = Arc::make_mut(&mut map.particle_systems);
    let base = out.defs.len();
    let mut at = Vec::with_capacity(guest.defs.len());
    let mut next = base;
    for d in &guest.defs {
        match out.find(&d.name) {
            Some(i) => at.push((i, false)),
            None => {
                at.push((next, true));
                next += 1;
            }
        }
    }
    for (d, (_, new)) in guest.defs.iter().zip(&at) {
        if !new {
            continue;
        }
        let mut d = d.clone();
        d.material = d.material.and_then(|m| material_at.get(m).copied().flatten());
        for c in &mut d.children {
            c.0 = at.get(c.0).map_or(c.0, |a| a.0);
        }
        out.defs.push(d);
    }
    if base == 0 || out.scale <= 0.0 {
        out.scale = guest.scale;
    }
}

/// A guest's decals added to a map's: appended, each group, material
/// letter and spray logo the map lacks pointing at them (the map's own
/// stay).
pub fn add_decals(map: &mut MapDecals, guest: MapDecals) {
    add_decals_of(map, guest, None);
}

/// `add_decals` for `game`'s impact looks: its groups whose names the map
/// took are kept under its own names (`impact::own_name`), and each
/// material letter the map took as `impact::own_material_group`, so its
/// impacts find its own (`MapDecals::material_group`).
pub fn add_decals_of(map: &mut MapDecals, guest: MapDecals, game: Option<&str>) {
    let base = map.decals.len();
    map.decals.extend(guest.decals);
    let mut groups: Vec<_> = guest.groups.into_iter().collect();
    groups.sort_by(|a, b| a.0.cmp(&b.0));
    // The guest's group names as the map now holds them.
    let mut held = BTreeMap::new();
    for (name, variants) in groups {
        let key = match game {
            _ if !map.groups.contains_key(&name) => name.clone(),
            Some(g) => super::impact::own_name(g, &name),
            None => continue,
        };
        if !map.groups.contains_key(&key) {
            map.groups
                .insert(key.clone(), variants.into_iter().map(|(w, i)| (w, i + base)).collect());
        }
        held.insert(name, key);
    }
    // Chains name the guest's groups as the map now holds them.
    for d in &mut map.decals[base..] {
        if let Some(next) = d.then.as_mut()
            && let Some(key) = held.get(next.as_str())
        {
            *next = key.clone();
        }
    }
    for (letter, group) in guest.by_material {
        let Some(key) = held.get(&group).cloned() else {
            map.by_material.entry(letter).or_insert(group);
            continue;
        };
        match (map.by_material.get(&letter), game) {
            (None, _) => {
                map.by_material.insert(letter, key);
            }
            (Some(_), Some(g)) => {
                let own = super::impact::own_material_group(g, letter);
                if !map.groups.contains_key(&own) {
                    let variants = map.groups[&key].clone();
                    map.groups.insert(own, variants);
                }
            }
            (Some(_), None) => {}
        }
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
    add_sounds_of(map, guest, None);
}

/// `add_sounds` for `game`'s impact looks: its surfaces whose names the
/// map took, and their bullet impact entries, are kept under its own
/// names too (`impact::own_name`; `MapSounds::game_surface`,
/// `MapSounds::game_entry`).
pub fn add_sounds_of(map: &mut MapSounds, guest: MapSounds, game: Option<&str>) {
    // The guest's impact entries (its surfaces' bullet impacts).
    let impacts: std::collections::BTreeSet<String> = guest
        .surfaces
        .values()
        .filter_map(|s| s.bullet_impact.as_deref().map(str::to_lowercase))
        .collect();
    for (name, surface) in guest.surfaces {
        match (map.surfaces.contains_key(&name), game) {
            (false, _) => {
                map.surfaces.insert(name, surface);
            }
            (true, Some(g)) => {
                map.surfaces.entry(super::impact::own_name(g, &name)).or_insert(surface);
            }
            (true, None) => {}
        }
    }
    let base = map.clips.len();
    let mut used = BTreeMap::new();
    let mut entries: Vec<_> = guest
        .entries
        .into_iter()
        .filter_map(|(k, e)| match game {
            _ if !map.entries.contains_key(&k) => Some((k, e)),
            Some(g) if impacts.contains(&k) => Some((super::impact::own_name(g, &k), e)),
            _ => None,
        })
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
            nose: None,
            turned: Vec::new(),
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
            variant: "",
            memos: None,
        };
        let load = || {
            let mut map = MapData::default();
            map.textures.push(texture("own"));
            dress_cached_with(&mut map, &options, &dressing, None).unwrap();
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
            conversions(&options.context, "kept_test", "", false),
            1,
            "copied, not converted"
        );
        *late.0.lock().unwrap() = true;
        let third = load();
        assert_eq!(
            conversions(&options.context, "kept_test", "", false),
            2,
            "the late file arrived"
        );
        assert_eq!(third.textures[1].name, "full");
        load();
        assert_eq!(conversions(&options.context, "kept_test", "", false), 2, "complete: kept");
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
            variant: "",
            memos: None,
        };
        let app = |disk: &MapCache| {
            let options = MapLoadOptions::lean();
            let parts = kept_with(&options, &dressing, Some(disk)).unwrap();
            let counts = (
                conversions(&options.context, "kept_disk", "", false),
                disk_reads(&options.context, "kept_disk", "", false),
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

    /// Served (module doc, "Served"): the packer's app (an `Export` in its
    /// context) hands over the dressing it converted from a store's
    /// records; a client of that store takes it (nothing converted, the
    /// same bytes), and one of another converter version converts.
    #[test]
    fn a_served_dressing_is_taken_by_a_client_of_the_same_files() {
        use chimera_core::mount::{
            LooseDir,
            assets::{CachedFiles, Manifest, Store},
        };
        let dir = std::env::temp_dir().join(format!("chimera-kept-served-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("install")).unwrap();
        std::fs::write(dir.join("install/look.txt"), b"served look").unwrap();
        let mut originals = Mount::default();
        originals.push(LooseDir::new(dir.join("install")));
        let store = Store::new(dir.join("store"));
        let mut manifest = Manifest::new("kept_served", "1", None);
        chimera_core::mount::assets::pack(&originals, &["look.txt".into()], &store, &mut manifest, None, 1).unwrap();
        let mount_of = |m: &Manifest| {
            let mut mount = Mount::default();
            mount.push(CachedFiles::new(store.clone(), m));
            Arc::new(mount)
        };
        let convert = |mount: &Mount, map: &mut MapData| {
            let name = String::from_utf8(mount.read("look.txt").unwrap()).unwrap();
            map.textures.push(texture(&name));
        };
        let app = |manifest: &Manifest, converter: &'static str, cache: &str, export: bool| {
            let mount = mount_of(manifest);
            let mount_of = || Ok(mount.clone());
            let dressing = Dressing {
                game: "kept_served",
                converter,
                mount: &mount_of,
                convert: &convert,
                variant: "",
                memos: None,
            };
            let options = MapLoadOptions::lean();
            if export {
                options.context.insert(Export::default());
            }
            let parts = kept_with(&options, &dressing, Some(&MapCache::at(dir.join(cache), u64::MAX))).unwrap();
            let entries = options
                .context
                .get::<Export>()
                .map(|e| std::mem::take(&mut *e.0.lock().unwrap()))
                .unwrap_or_default();
            let counts = (
                conversions(&options.context, "kept_served", "", false),
                served_reads(&options.context, "kept_served", "", false),
            );
            (postcard::to_allocvec(&*parts).unwrap(), counts, entries)
        };
        let (made, counts, entries) = app(&manifest, "v1", "packer", true);
        assert_eq!((counts, entries.len()), ((1, 0), 1), "the packer converts and hands it over");
        assert!(entries[0].entry_path.starts_with(&format!("_converted/{}/", served_name("kept_served", "v1"))));
        for e in &entries {
            cache::served::enter(&store, &mut manifest, e).unwrap();
        }
        let (taken, counts, _) = app(&manifest, "v1", "client", false);
        assert_eq!(counts, (0, 1), "a client takes it");
        assert!(taken == made, "the same parts");
        assert_eq!(app(&manifest, "v2", "client-v2", false).1, (1, 0), "another converter converts");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The disk layer is on unless its variable turns it off.
    #[test]
    fn the_disk_layer_is_on_unless_turned_off() {
        assert!(disk_on(None));
        assert!(disk_on(Some("1")) && disk_on(Some("")));
        for off in ["0", " off", "False", "no"] {
            assert!(!disk_on(Some(off)), "{off}");
        }
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
            variant: "",
            memos: None,
        };
        assert!(dress_cached_with(&mut MapData::default(), &options, &dressing, None).is_err());
        assert!(dress_cached_with(&mut MapData::default(), &options, &dressing, None).is_err());
        assert_eq!(conversions(&options.context, "kept_none", "", false), 0);
    }

    /// One game's impact looks, all under the names every Source game
    /// uses (`tag` tells whose: the surface's hardness, the clip's rate,
    /// the decal's size, the material's texture).
    fn looks(game: &str, tag: usize) -> MapData {
        use bevy::math::Vec2;
        use crate::map::{
            decal::{DecalBlend, MapDecal},
            particles::ParticleMaterial,
            sound::{Interval, MapSoundClip, MapSoundEntry, MapSurface, SoundLevel},
        };
        let mut map = MapData::default();
        let sounds = Arc::make_mut(&mut map.sounds);
        sounds.surfaces.insert(
            "concrete".into(),
            MapSurface {
                game_material: 'C',
                bullet_impact: Some("Concrete.BulletImpact".into()),
                hardness: tag as f32,
                ..default()
            },
        );
        sounds.entries.insert(
            "concrete.bulletimpact".into(),
            MapSoundEntry {
                waves: vec![0],
                volume: Interval::fixed(1.0),
                pitch: Interval::fixed(100.0),
                level: SoundLevel::Db(75.0),
                channel: 0,
                dry: false,
                hearing: None,
            },
        );
        sounds.clips.push(MapSoundClip {
            rate: tag as u32,
            channels: 1,
            samples: Arc::from(&[0i16][..]),
            loop_start: None,
        });
        map.kit.decals.decals.push(MapDecal {
            texture: 0,
            uv_min: Vec2::ZERO,
            uv_max: Vec2::ONE,
            size: Vec2::splat(tag as f32),
            blend: DecalBlend::Modulate2x,
            fog_fade: crate::map::decal::SOURCE_FOG_FADE,
            ..Default::default()
        });
        map.kit.decals.groups.insert("impact.concrete".into(), vec![(1.0, 0)]);
        map.kit.decals.by_material.insert('C', "impact.concrete".into());
        map.particles.materials.push(ParticleMaterial {
            name: "effects/spark".into(),
            texture: Some(tag),
            ..default()
        });
        map.impacts.add(game);
        map
    }

    /// A guest's impact looks named as the map's (TF2's and CS:S's
    /// `Concrete.BulletImpact`, 'C' decals, `effects/spark`) are its own
    /// for its impacts; the map's game keeps its own under the bare names.
    /// A guest's particle systems (TF2's explosion on de_dust2) reach the
    /// map: each the map lacks, its material and children where the map
    /// holds them; the map's own of a name stays.
    #[test]
    fn a_guests_particle_systems_are_appended() {
        use crate::map::{particles::ParticleMaterial, psys::SystemDef};
        let mat = |n: &str| ParticleMaterial { name: n.into(), ..Default::default() };
        let def = |n: &str, material, children: Vec<(usize, f32)>| SystemDef { name: n.into(), material, children, ..Default::default() };
        let mut map = MapData::default();
        map.particles.materials = vec![mat("effects/map_only"), mat("effects/shared")];
        Arc::make_mut(&mut map.particle_systems).defs = vec![def("Shared_Child", Some(0), Vec::new())];
        let mut parts = MapData::default();
        parts.particles.materials = vec![mat("effects/shared"), mat("effects/fire")];
        {
            let ps = Arc::make_mut(&mut parts.particle_systems);
            ps.defs = vec![
                def("ExplosionCore_MidAir", Some(1), vec![(1, 0.0), (2, 0.1)]),
                def("Shared_Child", Some(0), Vec::new()),
                def("Explosion_Flash", Some(0), Vec::new()),
            ];
            ps.scale = 0.0254;
        }
        append(&mut map, &parts);
        let ps = &map.particle_systems;
        let names: Vec<&str> = ps.defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Shared_Child", "ExplosionCore_MidAir", "Explosion_Flash"]);
        let core = &ps.defs[1];
        assert_eq!(map.particles.materials[core.material.unwrap()].name, "effects/fire");
        assert_eq!(core.children, vec![(0, 0.0), (2, 0.1)], "the map's child of the name, the new one after");
        assert_eq!(map.particles.materials[ps.defs[2].material.unwrap()].name, "effects/shared");
        assert_eq!(ps.defs[0].material, Some(0), "the map's own stays");
        assert!((ps.scale - 0.0254).abs() < 1e-6);
    }

    #[test]
    fn a_guests_impact_looks_are_its_own_where_its_names_are_taken() {
        let mut map = looks("cs_source", 1);
        map.impacts.game = Some("cs_source".into());
        append(&mut map, &looks("tf2", 2));
        let sounds = &map.sounds;
        assert_eq!(sounds.game_surface("tf2", "concrete").unwrap().hardness, 2.0);
        assert_eq!(sounds.game_surface("cs_source", "concrete").unwrap().hardness, 1.0);
        assert_eq!(sounds.surface("concrete").unwrap().hardness, 1.0, "the map's stays bare");
        let clip = |entry: &str| sounds.clips[sounds.entry(entry).unwrap().waves[0]].rate;
        let tf2 = sounds.game_entry("tf2", "Concrete.BulletImpact");
        assert_eq!(tf2, "tf2:concrete.bulletimpact");
        assert_eq!(clip(&tf2), 2, "TF2's own clip");
        let cs = sounds.game_entry("cs_source", "Concrete.BulletImpact");
        assert_eq!(cs, "Concrete.BulletImpact");
        assert_eq!(clip(&cs), 1);
        let decals = &map.kit.decals;
        let size = |game| {
            let group = decals.material_group(Some(game), 'c').unwrap();
            decals.decals[decals.groups[group][0].1].size.x
        };
        assert_eq!(size("tf2"), 2.0, "TF2's bullet hole");
        assert_eq!(size("cs_source"), 1.0, "CS:S's");
        assert_eq!(decals.material_group(None, 'C'), Some("impact.concrete"));
        let texture = |game| map.particles.materials[map.particles.find_of(Some(game), "effects/spark").unwrap()].texture;
        assert_eq!(texture("tf2"), Some(2));
        assert_eq!(texture("cs_source"), Some(1));
        // A third game with none of its own takes the map's.
        assert_eq!(sounds.game_entry("hl2dm", "Concrete.BulletImpact"), "Concrete.BulletImpact");
        assert_eq!(size("hl2dm"), 1.0);
        // Appended again (the next map's copy), nothing doubles.
        let entries = map.sounds.entries.len();
        append(&mut map, &looks("tf2", 2));
        assert_eq!(map.sounds.entries.len(), entries);
    }
}
