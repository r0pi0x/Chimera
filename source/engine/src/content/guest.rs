//! A Source game's parts on other games' maps (its guest dressing:
//! CS:S's, HL2:DM's and TF2's `dress_guest_with`), converted once per app
//! and load kind, then copied into each map (docs/performance.md, "Guest
//! dressings kept for the session"). The game's own dressing runs once on
//! an empty map (`dress_cached`: its textures and cubemaps from 0); what it
//! made is kept in the app's `LoadContext` (with or without textures:
//! `MapLoadOptions::textures`) and appended to every later map (`append`):
//! its textures and cubemaps after the map's, every index its parts hold
//! moved past them (`map::texture_refs`), its bodies, held and view models
//! after the map's, and what the dressing merges by name (sound entries,
//! decal groups, effect materials, muzzle flashes, spent shells, the HUD
//! look) by the same rules (`bsp::add_guest_sounds`, `add_guest_decals`,
//! `MapData::add_guest_hud`, `shells::add_shells`). A failed dressing (no
//! install) is not kept: the next load tries again.
//!
//! Kept textures that are whole DXT blocks keep those alone
//! (`MapTexture::keep_blocks_only`), as Combat Arms' shared models' do:
//! what the GPU takes, and less to copy into each map.
//!
//! Known shortcuts (docs/tech-debt.md): what the dressing read is kept for
//! the app's life, so files that arrive later (the asset service's, after
//! a dressing that missed them) or an install changed meanwhile don't
//! reach it until the app restarts; a kept texture of its blocks alone
//! casts an opaque shadow (`render::shadows` reads texels for alpha). Why
//! it's OK for now: a dressing reads only the install's `base` files,
//! which come before any map loads; the guests' bodies are opaque.

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::map::{LoadContext, MapData, MapLoadOptions, texture_refs::TextureRefs};

/// The app's kept dressings, by game and whether textures were decoded.
#[derive(Default)]
struct Cache(Mutex<HashMap<(&'static str, bool), Arc<Entry>>>);

/// One game's kept dressing (a lock of its own: two loads at once convert
/// once) and how many times it was converted.
#[derive(Default)]
struct Entry {
    parts: Mutex<Option<Arc<MapData>>>,
    conversions: AtomicUsize,
}

fn entry(context: &LoadContext, game: &'static str, textures: bool) -> Arc<Entry> {
    let cache = context.get_or_init(Cache::default);
    let mut entries = cache.0.lock().unwrap_or_else(|e| e.into_inner());
    entries.entry((game, textures)).or_default().clone()
}

/// `game`'s guest parts into `map`: converted by `convert` (the game's
/// dressing, given an empty map) the first time the app (`options.
/// context`) asks with these options' `textures`, copied from then on.
pub fn dress_cached(
    game: &'static str,
    map: &mut MapData,
    options: &MapLoadOptions,
    convert: impl FnOnce(&mut MapData) -> Result<(), String>,
) -> Result<(), String> {
    let parts = kept(game, options, convert)?;
    append(map, &parts);
    Ok(())
}

/// How many times `game`'s dressing was converted for the app of
/// `context` (with textures or without).
pub fn conversions(context: &LoadContext, game: &'static str, textures: bool) -> usize {
    entry(context, game, textures).conversions.load(Ordering::Relaxed)
}

fn kept(
    game: &'static str,
    options: &MapLoadOptions,
    convert: impl FnOnce(&mut MapData) -> Result<(), String>,
) -> Result<Arc<MapData>, String> {
    let entry = entry(&options.context, game, options.textures);
    let mut parts = entry.parts.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(p) = parts.as_ref() {
        return Ok(p.clone());
    }
    let mut scratch = MapData::default();
    convert(&mut scratch)?;
    entry.conversions.fetch_add(1, Ordering::Relaxed);
    for t in &mut scratch.textures {
        t.keep_blocks_only();
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
    map.texture_anims.extend(parts.texture_anims.iter().map(|a| moved(a, t, c)));
    let kit = &parts.kit;
    map.kit.characters.extend(kit.characters.iter().map(|x| moved(x, t, c)));
    map.kit.held.extend(kit.held.iter().map(|x| moved(x, t, c)));
    map.kit.view_models.extend(kit.view_models.iter().map(|x| moved(x, t, c)));
    for look in kit.hud.iter().chain(&kit.guest_huds) {
        map.kit.add_guest_hud(Some(Arc::new(moved(&**look, t, c))));
    }
    super::bsp::add_guest_decals(&mut map.kit.decals, moved(&kit.decals, t, c));
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
        if !map.kit.muzzle_flash_kinds.iter().any(|(k, _)| k.eq_ignore_ascii_case(kind)) {
            map.kit.muzzle_flash_kinds.push((kind.clone(), moved(flash, t, c)));
        }
    }
    crate::map::shells::add_shells(&mut map.kit.shells, kit.shells.iter().map(|s| moved(s, t, c)).collect());
    super::bsp::add_guest_sounds(Arc::make_mut(&mut map.sounds), (*parts.sounds).clone());
    map.warnings.extend(parts.warnings.iter().cloned());
}
