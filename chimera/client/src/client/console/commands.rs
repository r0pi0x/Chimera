use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Mutex, OnceLock},
};

use bevy::{
    app::AppExit,
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
        mouse::{MouseScrollUnit, MouseWheel},
    },
    log::{BoxedLayer, tracing_subscriber::Layer},
    prelude::*,
    window::CursorOptions,
};

use chimera_ui::clipboard::Clipboard;
use crate::client::{FirstPersonCamera, fonts::UiFonts, widgets::Interaction};
use super::*;
use crate::{
    console::{Console, ConsoleAppExt, Level, cfg_dir, parse, resource_cvar},
    core::{LocalPlayer, MovementState, Velocity},
    game_registry::{GameInfo, Games},
    map::PlaySound,
};

// ------------------------------------------------------------- commands

/// What each game's install offers for maps, read once (the install is
/// read from disk): the names for `map` completion and the map cycle,
/// keyed by the game's id, until an import changes them. A resource of
/// the console's plugin, so apps in one process keep their own. Behind a
/// lock so the callers that only have a `&World` (completion, the
/// debug UI) can fill it.
#[derive(Resource, Debug, Default)]
pub struct MapLists {
    names: Mutex<BTreeMap<&'static str, Vec<String>>>,
    pub(super) cycles: Mutex<BTreeMap<&'static str, Vec<String>>>,
}

impl MapLists {
    /// The install changed: find the maps again.
    pub fn forget(&self) {
        self.names.lock().unwrap_or_else(|e| e.into_inner()).clear();
        self.cycles.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }

    fn names(&self, game: &GameInfo) -> Vec<String> {
        self.names_by(game.id, || read_map_names(game))
    }

    /// Game `id`'s names: `read` them on the first ask only.
    pub(super) fn names_by(&self, id: &'static str, read: impl FnOnce() -> Vec<String>) -> Vec<String> {
        let mut cached = self.names.lock().unwrap_or_else(|e| e.into_inner());
        cached.entry(id).or_insert_with(read).clone()
    }

    fn cycle(&self, game: &GameInfo) -> Vec<String> {
        let mut cached = self.cycles.lock().unwrap_or_else(|e| e.into_inner());
        cached
            .entry(game.id)
            .or_insert_with(|| game.mount().map(|m| (game.map_cycle)(&m)).unwrap_or_default())
            .clone()
    }
}

/// Every game's map list read on a task when the app starts
/// (`read_map_lists_ahead`): the first `map` command (or completion) read
/// it on its frame (and the map cycle when the map changed), opening the install's archives (Combat Arms': 300 M
/// instructions, a 60 ms frame at the start of the swap; docs/performance.md,
/// "A map's swap in the real app"). An ask before it is in reads as before.
#[derive(Resource)]
pub(super) struct MapListsAhead(bevy::tasks::Task<Vec<(&'static str, Vec<String>, Vec<String>)>>);

pub(super) fn read_map_lists_ahead(mut commands: Commands, games: Option<Res<Games>>) {
    let games: Vec<&'static GameInfo> = games.iter().flat_map(|g| g.all()).collect();
    let Some(pool) = bevy::tasks::AsyncComputeTaskPool::try_get().filter(|_| !games.is_empty()) else { return };
    let task = chimera_core::tasks::spawn(&pool, async move {
        games
            .into_iter()
            .map(|g| {
                let Some(mount) = g.mount() else { return (g.id, Vec::new(), Vec::new()) };
                let mut names = (g.map_names)(&mount);
                names.sort();
                names.dedup();
                (g.id, names, (g.map_cycle)(&mount))
            })
            .collect()
    });
    commands.insert_resource(MapListsAhead(task));
}

pub(super) fn take_map_lists_ahead(mut commands: Commands, ahead: Option<ResMut<MapListsAhead>>, lists: Res<MapLists>) {
    let Some(mut ahead) = ahead else { return };
    let Some(read) = bevy::tasks::block_on(bevy::tasks::poll_once(&mut ahead.0)) else { return };
    commands.remove_resource::<MapListsAhead>();
    let mut names = lists.names.lock().unwrap_or_else(|e| e.into_inner());
    let mut cycles = lists.cycles.lock().unwrap_or_else(|e| e.into_inner());
    for (id, list, cycle) in read {
        names.entry(id).or_insert(list);
        cycles.entry(id).or_insert(cycle);
    }
}

pub(super) fn read_map_names(game: &GameInfo) -> Vec<String> {
    let Some(mount) = game.mount() else { return Vec::new() };
    let mut names = (game.map_names)(&mount);
    names.sort();
    names.dedup();
    names
}

/// The install changed: find its maps again.
pub fn forget_map_names(w: &World) {
    if let Some(lists) = w.get_resource::<MapLists>() {
        lists.forget();
    }
}

/// Every map `game` (the default game: `game_registry`) can load: the
/// install's, its downloads and Chimera's cache (cached in `MapLists`;
/// `import` refreshes it). None without a game.
pub fn map_names(w: &World, game: Option<&'static GameInfo>) -> Vec<String> {
    let Some(game) = game else { return Vec::new() };
    match w.get_resource::<MapLists>() {
        Some(lists) => lists.names(game),
        None => read_map_names(game),
    }
}

/// The rules' map cycle follows the map loaded (the end of a game moves
/// on to the next one: `rules::game_end`).
pub(super) fn follow_map_cycle(
    name: Option<Res<crate::map::LoadedMapName>>,
    games: Option<Res<Games>>,
    lists: Option<Res<MapLists>>,
    mut cycle: ResMut<crate::core::MapCycle>,
) {
    let Some(name) = name.filter(|n| n.is_changed()) else { return };
    let short = crate::net::maps::map_name(&name.0).to_string();
    if cycle.current != short {
        cycle.current = short;
    }
    if cycle.maps.is_empty()
        && let Some(game) = games.and_then(|g| g.default_game())
    {
        cycle.maps = match lists {
            Some(l) => l.cycle(game),
            None => game.mount().map(|m| (game.map_cycle)(&m)).unwrap_or_default(),
        };
    }
}

/// Where a map of `game`'s comes from: the game's own content, its
/// download folder, or Chimera's cache.
pub(super) fn map_source(game: &GameInfo, name: &str) -> &'static str {
    let Some(file) = (game.map_file)(name) else { return "game" };
    let in_dir = |dir: Option<std::path::PathBuf>| dir.is_some_and(|d| d.join(&file).is_file());
    if in_dir(game.content_cache()) {
        "imported"
    } else if in_dir(game.download_dir()) {
        "downloaded"
    } else {
        "game"
    }
}

/// Whether a `map` command's load is under way.
pub fn map_loading(w: &World) -> bool {
    w.contains_resource::<MapLoad>() || w.contains_resource::<Spawning>() || w.contains_resource::<Settling>()
}

/// Where a `map` command's load is (`map_load_stage`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapLoadStage {
    /// No load under way.
    None,
    /// Loading on a task.
    Loading,
    /// Spawning a step a frame.
    Spawning,
    /// Spawned, settling behind the loading screen.
    Settling,
}

/// Where a `map` command's load is.
pub fn map_load_stage(w: &World) -> MapLoadStage {
    if w.contains_resource::<Spawning>() {
        MapLoadStage::Spawning
    } else if w.contains_resource::<Settling>() {
        MapLoadStage::Settling
    } else if w.contains_resource::<MapLoad>() {
        MapLoadStage::Loading
    } else {
        MapLoadStage::None
    }
}

/// The map a `map` command is loading (its name, without the game).
pub fn loading_map(w: &World) -> Option<String> {
    let id = match (w.get_resource::<MapLoad>(), w.get_resource::<Spawning>()) {
        (Some(load), _) => &load.id,
        (None, Some(spawning)) => &spawning.id,
        (None, None) => &w.get_resource::<Settling>()?.id,
    };
    Some(id.rsplit(':').next().unwrap_or(id).to_string())
}

/// The longest a spawned map stays behind the loading screen (`Settling`); a
/// flood not done by then goes on a slice a tick in play.
pub(super) const SETTLE_MAX_SECS: f32 = 10.0;

/// Ticks a spawned map runs before its settling starts: the collision is in
/// the spatial queries' trees after the first physics steps.
pub(super) const SETTLE_AFTER_TICKS: u64 = 3;

/// A spawned map still behind the loading screen, doing what needs its physics
/// a slice a frame (`map::MapHooks::settle`, docs/performance.md, "Nothing
/// blocks the main thread"): the game is entered when that's done.
#[derive(Resource)]
pub(super) struct Settling {
    id: String,
    tick: u64,
    started: std::time::Instant,
}

/// The most of a frame a loaded map's spawn takes behind the loading screen
/// (`Spawning`): its steps run until this is spent, at least one a frame.
/// With the frame's own work (the HUD, the loading screen, extraction) a
/// frame stays under 33 ms on an optimized build (it was 20 ms, and its
/// frames reached 30-90 ms: docs/performance.md, "A map's swap in the real
/// app"); a map takes a few frames more to come in than in one (each
/// frame's own work, the render world's above all, is paid once per frame).
pub(super) const SPAWN_BUDGET: std::time::Duration = std::time::Duration::from_millis(12);

/// The most the GPU is sent a frame while a map comes in (Bevy's
/// `RenderAssetBytesPerFrame`, images' and meshes' bytes; at least one
/// asset a frame): the map's are uploaded over frames behind the loading
/// screen, not in one (32 MiB of textures cost the render thread 10-15 ms
/// a frame; meshes have their own pace, `upload_pace`). Unlimited again
/// once in the game, which waits for them (`upload_pace::uploads_settled`).
pub(super) const LOAD_UPLOAD_BYTES: usize = 16 << 20;

/// A loaded map spawning a step at a time behind the loading screen
/// (`map::prepare::begin_map_change`; docs/performance.md, "A map's
/// swap"). The simulation waits meanwhile (`Time<Virtual>` paused): its
/// first tick on the map is after the whole map is in, as when it was
/// spawned in one frame, the colliders that follow the map's frame too
/// (`spawn_some` says done only then).
#[derive(Resource)]
pub(super) struct Spawning {
    id: String,
    summary: String,
    /// `Time<Virtual>` was paused already (`host_timescale 0`).
    was_paused: bool,
}

/// What a map's load changes while it comes in, given back on entering the
/// game: the longest a frame may advance the simulation (one tick's worth
/// meanwhile, so a slow frame behind the loading screen leaves no
/// catch-up ticks owed) and the GPU's upload budget.
#[derive(Resource)]
pub(super) struct LoadCaps {
    max_delta: std::time::Duration,
    upload: Option<usize>,
}

impl LoadCaps {
    /// Caps the frames' advance at one tick and the uploads, keeping what
    /// they were (once per load).
    fn apply(w: &mut World) {
        if w.contains_resource::<LoadCaps>() {
            return;
        }
        let tick = w.get_resource::<Time<Fixed>>().map(|t| t.timestep());
        let max_delta = match (w.get_resource_mut::<Time<Virtual>>(), tick) {
            (Some(mut time), Some(tick)) if !tick.is_zero() => {
                let before = time.max_delta();
                time.set_max_delta(tick);
                before
            }
            (Some(time), _) => time.max_delta(),
            (None, _) => return,
        };
        let upload = w
            .get_resource_mut::<bevy::render::render_asset::RenderAssetBytesPerFrame>()
            .map(|mut b| b.max_bytes.replace(LOAD_UPLOAD_BYTES))
            .unwrap_or(None);
        w.insert_resource(LoadCaps { max_delta, upload });
    }

    /// Gives them back.
    fn lift(w: &mut World) {
        let Some(caps) = w.remove_resource::<LoadCaps>() else { return };
        if let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
            time.set_max_delta(caps.max_delta);
        }
        if let Some(mut b) = w.get_resource_mut::<bevy::render::render_asset::RenderAssetBytesPerFrame>() {
            b.max_bytes = caps.upload;
        }
    }
}

/// The games the coming match deals (`chimera_net::minigame_net::
/// dealt_games`; None: any), kept as the loaded map's
/// (`map::sources::MapDealt`) for a match started on it later.
pub(super) fn dealt(w: &mut World) -> Option<std::sync::Arc<std::collections::BTreeSet<String>>> {
    let dealt = chimera_net::minigame_net::dealt_games(w).map(std::sync::Arc::new);
    w.insert_resource(chimera_world::map::sources::MapDealt(dealt.clone()));
    dealt
}

/// A map loading in the background for the `map` command.
#[derive(Resource)]
pub(super) struct MapLoad {
    id: String,
    pub(super) task: bevy::tasks::Task<Result<(crate::map::MapData, crate::map::prepare::Prepared), String>>,
}

/// Load map `id` in the background (from `file` when given: a copy
/// downloaded from a server), then swap it in place (`finish_map_load`):
/// everyone respawns at the new spawn points.
pub fn start_map_load(w: &mut World, id: &str, file: Option<std::path::PathBuf>) {
    w.remove_resource::<FreshGame>();
    // A code-built map's kit still loading is called off.
    w.remove_resource::<CodeMapKit>();
    w.insert_resource(crate::map::LoadedMapName(id.to_string()));
    // The local player's map isn't ready until the game is entered
    // (`settle_map`): what starts play waits for it.
    w.insert_resource(crate::map::LocalMapLoad { map: id.to_string() });
    // Its stages from the map's (`map::loading::LoadWork`: the spawn and
    // the settling count theirs in).
    w.insert_resource(crate::map::loading::LoadWork::default());
    // At the HDR level set now (mat_hdr_level), as the game does.
    let hdr_level = w.get_resource::<crate::client::hdr::HdrSettings>().map_or(0, |s| s.level);
    // This load's own progress (the loading screen reads the resource).
    let progress = crate::map::loading::LoadReporter::default();
    w.insert_resource(progress.clone());
    // The app's context: what its games keep for their loads.
    let context = crate::map::LoadContext::of(w);
    // Only the guests whose games the coming match deals (any when not known).
    let dealt = dealt(w);
    let options = crate::map::MapLoadOptions { hdr_level, progress, context, dealt, ..default() };
    // The app loads it (`AppHooks`), then the spawn's own work is done
    // there too (`map::prepare`): the frame that swaps it in only spawns.
    let hooks = w.get_resource::<crate::client::AppHooks>().copied();
    let prepare = crate::map::prepare::preparation(w);
    let task = chimera_core::tasks::spawn(&bevy::tasks::AsyncComputeTaskPool::get(), {
        let id = id.to_string();
        async move {
            let hooks = hooks.ok_or("this app loads no maps")?;
            let data = match file {
                Some(f) => (hooks.load_map_file)(&id, &f, &options),
                None => (hooks.load_map)(&id, &options),
            }?;
            let prepared = prepare.run(&data);
            Ok((data, prepared))
        }
    });
    w.insert_resource(MapLoad { id: id.to_string(), task });
}

/// The server's map, for joining it (`net::NetEvent::LoadMap`): the
/// greybox at once, another in the background (from the downloaded file
/// when there is one; a bare name is the default game's). The handshake
/// checks the result.
pub fn load_server_map(w: &mut World, map: &str, file: Option<std::path::PathBuf>) {
    if crate::net::is_code_map(map) {
        load_code_map(w, map);
        return;
    }
    let id = match crate::game_registry::default_game(w) {
        Some(game) if !map.contains(':') => game.map_id(map),
        _ => map.to_string(),
    };
    start_map_load(w, &id, file);
}

/// Swap in a map once its background load finishes, then settle it behind
/// the loading screen (the console's system; public for a headless test
/// of the whole `map` flow, `heavy::map_swap_frames`).
pub fn finish_map_load(w: &mut World) {
    finish_code_map_kit(w);
    if w.contains_resource::<Settling>() {
        settle_map(w);
        return;
    }
    if w.contains_resource::<Spawning>() {
        spawn_map_steps(w, SPAWN_BUDGET);
        return;
    }
    let wait = chimera_core::tasks::waits(w);
    let Some(mut load) = w.get_resource_mut::<MapLoad>() else {
        return;
    };
    let Some(result) = chimera_core::tasks::poll(&mut load.task, wait) else {
        return;
    };
    let id = load.id.clone();
    w.remove_resource::<MapLoad>();
    let (data, prepared) = match result {
        Ok(d) => d,
        Err(e) => {
            w.remove_resource::<crate::map::LocalMapLoad>();
            w.resource_mut::<Console>()
                .print(crate::console::Level::Error, format!("map {id}: {e}"));
            crate::client::front_end::map_load_failed(w);
            // Joining a server needs its map.
            match w.get_resource::<crate::core::NetRole>() {
                Some(crate::core::NetRole::Client) => {
                    crate::net::disconnect(w, &format!("couldn't load the server's map {id}: {e}"));
                }
                Some(crate::core::NetRole::Server) => {
                    // Still on the map that plays (its players were told of a
                    // change: `net::maps` calls it off).
                    let game = crate::game_registry::default_game(w);
                    let back = w
                        .get_resource::<crate::map::MapFile>()
                        .zip(game)
                        .map(|(f, game)| game.map_id(&f.name))
                        .unwrap_or_else(|| GREYBOX.into());
                    w.insert_resource(crate::map::LoadedMapName(back));
                    crate::client::front_end::entered_game(w);
                }
                _ => {}
            }
            return;
        }
    };
    let summary = format!(
        "loaded {id}: {} triangles, {} spawns",
        data.triangle_count(),
        data.spawns.len()
    );
    unload_greybox(w);
    // Spawned over the next frames (`spawn_map_steps`), the simulation
    // waiting.
    crate::map::prepare::begin_map_change(w, data, prepared, crate::map::MapDebugView::Normal);
    let was_paused = w.get_resource::<Time<Virtual>>().is_none_or(|t| t.is_paused());
    if let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
        time.pause();
    }
    LoadCaps::apply(w);
    w.insert_resource(Spawning { id, summary, was_paused });
    // `CHIMERA_SPAWN_AT_ONCE=1`: every step and the commands in this frame,
    // as before the spawn was spread (an A/B for measuring).
    if chimera_core::env::var("CHIMERA_SPAWN_AT_ONCE").is_ok_and(|v| v == "1") {
        while w.contains_resource::<Spawning>() {
            spawn_map_steps(w, std::time::Duration::MAX);
        }
        return;
    }
    spawn_map_steps(w, SPAWN_BUDGET);
}

/// A frame of `Spawning`: the map's next steps; once it is all in, the
/// game goes on with it (the tick, the look, a new game) behind the loading
/// screen while it settles (`Settling`).
pub(super) fn spawn_map_steps(w: &mut World, budget: std::time::Duration) {
    if !crate::map::prepare::spawn_some(w, budget) {
        return;
    }
    let Some(Spawning { id, summary, was_paused }) = w.remove_resource::<Spawning>() else { return };
    if !was_paused && let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
        time.unpause();
    }
    // At the gameplay's server tick (a mashup plays another game's map at
    // its gameplay's tick); without a gameplay the default game's, else
    // the map's game's.
    let tick = w.get_resource::<crate::gameplay::Gameplay>().map(|g| g.tick).or_else(|| {
        crate::game_registry::default_game(w)
            .or_else(|| {
                id.split_once(':')
                    .and_then(|(game, _)| w.get_resource::<Games>()?.get(game))
            })
            .map(|g| g.tick())
    });
    let tick = tick.unwrap_or(std::time::Duration::from_secs_f64(1.0 / crate::core::DEFAULT_TICK_HZ));
    crate::core::set_tick_length(w, tick);
    // At most a tick's worth a frame until in the game (the cap was set at
    // the old tick).
    if let Some(mut time) = w.get_resource_mut::<Time<Virtual>>() {
        time.set_max_delta(tick);
    }
    // Follow the map's presentation (Source LDR: no tonemapping).
    let tonemapping = w.resource::<crate::map::ActiveMapLook>().0.tonemapping;
    let cams: Vec<Entity> = w
        .query_filtered::<Entity, With<crate::client::FirstPersonCamera>>()
        .iter(w)
        .collect();
    for c in cams {
        let t = if tonemapping {
            bevy::core_pipeline::tonemapping::Tonemapping::default()
        } else {
            bevy::core_pipeline::tonemapping::Tonemapping::Linear
        };
        w.entity_mut(c).insert(t);
    }
    crate::gameplay::new_game(w);
    w.resource_mut::<Console>().info(summary);
    let tick = w.get_resource::<crate::core::SimClock>().map_or(0, |c| c.tick);
    w.insert_resource(Settling {
        id,
        tick,
        started: std::time::Instant::now(),
    });
    // A network client predicts once in the game (`net::predict`).
    w.insert_resource(crate::map::MapSettling);
}

/// A frame of `Settling`: every hook's slice, and into the game when they are
/// all done.
pub(super) fn settle_map(w: &mut World) {
    let settling = w.resource::<Settling>();
    let (from, started) = (settling.tick, settling.started);
    let tick = w.get_resource::<crate::core::SimClock>().map_or(from + SETTLE_AFTER_TICKS, |c| c.tick);
    if tick < from + SETTLE_AFTER_TICKS {
        return;
    }
    let hooks: Vec<fn(&mut World) -> bool> =
        w.get_resource::<crate::map::MapHooks>().map(|h| h.settle.clone()).unwrap_or_default();
    let mut done = true;
    let mut hooks_done = 0u64;
    let hooks_n = hooks.len() as u64;
    for hook in hooks {
        let d = hook(w);
        hooks_done += u64::from(d);
        done &= d;
    }
    // The physics stage: after the colliders, the settle hooks done of all.
    if let Some(mut work) = w.get_resource_mut::<crate::map::loading::LoadWork>() {
        let colliders = work.colliders;
        work.set_if_neq(crate::map::loading::LoadWork {
            step: crate::map::loading::LoadStep::Physics,
            done: colliders + hooks_done,
            total: colliders + hooks_n,
            colliders,
        });
    }
    if !done && started.elapsed().as_secs_f32() < SETTLE_MAX_SECS {
        return;
    }
    info!("map settled behind the loading screen in {:.2} s", started.elapsed().as_secs_f32());
    w.remove_resource::<Settling>();
    w.remove_resource::<crate::map::MapSettling>();
    w.remove_resource::<crate::map::LocalMapLoad>();
    LoadCaps::lift(w);
    // A HUD still being shown (out of time) gives the HUD back.
    w.remove_resource::<crate::gameplay::HudWarm>();
    crate::client::front_end::entered_game(w);
    crate::client::net::listen_if_hosting(w);
    if w.remove_resource::<FreshGame>().is_some() {
        crate::client::front_end::start_game(w);
    }
}

/// The map loading is a game the `map` command starts (not a server's
/// map change or a server's map we join): the local player joins it as
/// `rules::JoinChoice` says (`join_menus::start_game`).
#[derive(Resource)]
pub(super) struct FreshGame;

/// The greybox map in place of whatever map is loaded (`map greybox`, and
/// `disconnect`, which leaves it behind the main menu), everyone respawned
/// on it at the default tick.
pub fn load_greybox(w: &mut World) {
    load_code_map(w, GREYBOX);
}

/// No map at all, as a run that starts at a menu with no game behind it
/// (`NoPlayerAtMenu`, `StartMap::Empty`): what a map change takes out (a
/// load or spawn under way, the map, the greybox) without the greybox
/// and its kit coming in. Leaving a game for such a menu: the greybox's
/// kit was dressed on the main thread (6 s, 24 G instructions in one
/// frame, docs/performance.md "Leaving a game for the menu").
pub fn unload_to_menu(w: &mut World) {
    w.remove_resource::<MapLoad>();
    w.remove_resource::<CodeMapKit>();
    w.remove_resource::<crate::map::LocalMapLoad>();
    if let Some(spawning) = w.remove_resource::<Spawning>()
        && !spawning.was_paused
        && let Some(mut time) = w.get_resource_mut::<Time<Virtual>>()
    {
        time.unpause();
    }
    LoadCaps::lift(w);
    crate::map::unload_map(w);
    unload_greybox(w);
    w.remove_resource::<crate::map::LoadedMapName>();
}

/// `load_greybox` for any code-built map (`net::is_code_map`: the greybox,
/// the range; `chimera_world::greybox::CodeMap` picks it for the app's
/// `respawn_greybox`).
pub fn load_code_map(w: &mut World, map: &str) {
    let id = chimera_world::greybox::select(w, map);
    w.remove_resource::<MapLoad>();
    w.remove_resource::<CodeMapKit>();
    w.remove_resource::<crate::map::LocalMapLoad>();
    // A spawn under way is called off (`unload_map` drops it).
    if let Some(spawning) = w.remove_resource::<Spawning>()
        && !spawning.was_paused
        && let Some(mut time) = w.get_resource_mut::<Time<Virtual>>()
    {
        time.unpause();
    }
    LoadCaps::lift(w);
    crate::map::unload_map(w);
    unload_greybox(w);
    if let Some(hooks) = w.get_resource::<crate::client::AppHooks>().copied() {
        // Its kit (the party's bodies, guns, HUD looks), when the app's
        // maps dress code-built maps (`map::sources::CODE_MAP_KIT`): loaded
        // on a task (`CodeMapKit`; a cold dressing took seconds on the main
        // thread: docs/performance.md, "Guest dressings kept for the
        // session") and spawned when it is in, before the code-built map
        // is spawned again (a map's spawn clears what the map before
        // left). The code-built map is in at once, without it meanwhile.
        let hdr_level = w.get_resource::<crate::client::hdr::HdrSettings>().map_or(0, |s| s.level);
        let options = crate::map::MapLoadOptions {
            hdr_level,
            context: crate::map::LoadContext::of(w),
            dealt: dealt(w),
            ..default()
        };
        let began = std::time::Instant::now();
        let kit_id = chimera_world::greybox::kit_id(id);
        // Its spawn's own work (images, colliders) on the task too.
        let prepare = crate::map::prepare::preparation(w);
        let load = async move {
            let kit = (hooks.load_map)(&kit_id, &options)?;
            let prepared = prepare.run(&kit);
            Ok((kit, prepared))
        };
        // A world with no async compute pool (a bare app without Bevy's
        // `TaskPoolPlugin`, as the menu's tests): in this frame, as
        // `map::drop_later` without one.
        match bevy::tasks::AsyncComputeTaskPool::try_get() {
            None => {
                let result = bevy::tasks::block_on(load);
                spawn_code_map_kit(w, id, result, began);
            }
            Some(pool) => {
                let mut task = pool.spawn(load);
                // Tests that wait for tasks (`WaitForTasks`): in this frame, as before.
                if chimera_core::tasks::waits(w)
                    && let Some(result) = chimera_core::tasks::poll(&mut task, true)
                {
                    spawn_code_map_kit(w, id, result, began);
                } else {
                    w.insert_resource(CodeMapKit { id, task, began });
                }
            }
        }
        (hooks.respawn_greybox)(w);
    }
    w.remove_resource::<crate::map::ActiveMapLook>();
    w.insert_resource(crate::map::LoadedMapName(id.into()));
    crate::core::set_tick_length(w, std::time::Duration::from_secs_f64(1.0 / crate::core::DEFAULT_TICK_HZ));
    code_map_view(w);
    crate::gameplay::new_game(w);
}

/// A code-built map's kit loading on a task (`load_code_map`).
#[derive(Resource)]
pub(super) struct CodeMapKit {
    id: &'static str,
    pub(super) task: bevy::tasks::Task<Result<(crate::map::MapData, crate::map::prepare::Prepared), String>>,
    began: std::time::Instant,
}

/// The code-built map's kit spawned once its task is done (a frame of the
/// console's `finish_map_load`), the code-built map spawned again after
/// it, the view as `load_code_map` left it.
pub(super) fn finish_code_map_kit(w: &mut World) {
    let wait = chimera_core::tasks::waits(w);
    let Some(mut load) = w.get_resource_mut::<CodeMapKit>() else { return };
    let Some(result) = chimera_core::tasks::poll(&mut load.task, wait) else { return };
    let Some(CodeMapKit { id, began, .. }) = w.remove_resource::<CodeMapKit>() else { return };
    // Another map since (its load removes this): nothing to do.
    if w.get_resource::<crate::map::LoadedMapName>().is_none_or(|n| n.0 != id) {
        return;
    }
    if spawn_code_map_kit(w, id, result, began) {
        unload_greybox(w);
        if let Some(hooks) = w.get_resource::<crate::client::AppHooks>().copied() {
            (hooks.respawn_greybox)(w);
        }
        w.remove_resource::<crate::map::ActiveMapLook>();
        code_map_view(w);
    }
}

/// The kit `result` of code-built map `id` spawned (`change_map`);
/// whether there was one.
pub(super) fn spawn_code_map_kit(
    w: &mut World,
    id: &str,
    result: Result<(crate::map::MapData, crate::map::prepare::Prepared), String>,
    began: std::time::Instant,
) -> bool {
    match result {
        Ok((kit, prepared)) => {
            info!("{id}'s kit: {} bodies, loaded in {:.0} ms", kit.kit.characters.len(), began.elapsed().as_secs_f64() * 1000.0);
            crate::map::prepare::change_map_prepared(w, kit, prepared, crate::map::MapDebugView::Normal);
            true
        }
        Err(e) => {
            debug!("{id} has no kit: {e}");
            false
        }
    }
}

/// A code-built map's view: tonemapped, the camera clearing (no sky), the
/// skybox cameras off.
pub(super) fn code_map_view(w: &mut World) {
    let cams: Vec<Entity> = w
        .query_filtered::<Entity, With<crate::client::FirstPersonCamera>>()
        .iter(w)
        .collect();
    for c in cams {
        let mut e = w.entity_mut(c);
        e.insert(bevy::core_pipeline::tonemapping::Tonemapping::default());
        // The map's sky drew under the view (no clear); the greybox has none.
        if let Some(mut camera) = e.get_mut::<Camera>() {
            camera.clear_color = ClearColorConfig::Default;
        }
    }
    let mut skies = w.query_filtered::<&mut Camera, With<crate::map::SkyboxCamera>>();
    for mut camera in skies.iter_mut(w) {
        camera.is_active = false;
    }
}

/// The greybox map's name for `map` and `--map`.
pub const GREYBOX: &str = "greybox";

/// What `map <name>` and `changelevel <name>` load: None for a code-built
/// map (`greybox`, `chimera:greybox`, `range`; `load_code_map`), else the map's id and whether to check
/// it against the install's list. A map of the default game (`game`) by
/// its name (`de_dust2`) or its id (`cs_source:de_dust2`, as the command
/// line and `--map` take it); another game's id as given (a bare name
/// too, without a default game).
pub fn map_target(game: Option<&GameInfo>, name: &str) -> Option<(String, bool)> {
    if crate::net::is_code_map(&crate::net::normalize_map(name)) {
        return None;
    }
    match (name.split_once(':'), game) {
        (Some((id, map)), Some(game)) if id.eq_ignore_ascii_case(game.id) => Some((game.map_id(map), true)),
        (None, Some(game)) => Some((game.map_id(name), true)),
        _ => Some((name.to_string(), false)),
    }
}

/// The player a command acts on (a network client's on its server).
pub(super) fn local_player(w: &mut World) -> Result<Entity, String> {
    crate::console::command_player(w)
}

/// The eye and the feet relative to a character's origin
/// (`MovementState::eye_offset`, `hull_min`). As CS:S's, `getpos` prints
/// the view's origin (the eye; noclip keeps the offset it was entered
/// with) and `setpos` takes the player's origin, Source's: the feet. So a
/// pasted `setpos` line puts the camera where CS:S's was (64 units above
/// the numbers standing), and `setpos` with `getpos`'s numbers lands one
/// view offset higher, as in CS:S.
pub(super) fn eye_and_feet(w: &World, p: Entity) -> (Vec3, Vec3) {
    w.get::<MovementState>(p)
        .map_or((Vec3::ZERO, Vec3::ZERO), |s| (s.eye_offset, Vec3::Y * s.hull_min.y))
}

/// The client's console commands (noclip, getpos/setpos/setang, god, kill,
/// map, ...), registered by `ConsoleUiPlugin`; public for tests.
pub fn client_commands(app: &mut App) {
    crate::client::binds::commands(app);
    for a in ACTIONS {
        let a = *a;
        for (sign, on) in [("+", true), ("-", false)] {
            app.console_command(
                &format!("{sign}{a}"),
                &format!("{} {a} (for binds).", if on { "Start" } else { "Stop" }),
                move |w, _| {
                    let mut h = w.resource_mut::<HeldActions>();
                    match a {
                        "jump" => h.jump = on,
                        "duck" => h.duck = on,
                        "speed" => h.speed = on,
                        "walk" => h.walk = on,
                        "sprint" => h.sprint = on,
                        "forward" => h.forward = on,
                        "back" => h.back = on,
                        "moveleft" => h.moveleft = on,
                        "moveright" => h.moveright = on,
                        "attack2" => h.attack2 = on,
                        "reload" => h.reload = on,
                        "use" => h.use_key = on,
                        "showscores" => h.showscores = on,
                        "freelook" => h.freelook = on,
                        _ => h.attack = on,
                    }
                    Ok(None)
                },
            );
        }
    }
    // noclip, god, setpos, setang: a dedicated server runs them too (for a
    // client's player).
    crate::net::cheats::player_commands(app);
    app.console_command(
        "chimera:host_if_asked",
        "Host the loaded map for other players when maxplayers asks for them (what map and the menu's \
         Create Server do after a load); the command line's --map runs it after its +commands.",
        |w, _| {
            crate::client::net::listen_if_hosting(w);
            Ok(None)
        },
    )
    .console_command(
        "getpos",
        "Your position and view, as a setpos/setang line (CS:S units).",
        |w, _| {
            let p = local_player(w)?;
            let t = *w.get::<Transform>(p).ok_or("no transform")?;
            // The view's origin, as CS:S prints it: in third person the
            // camera's (a CS:S capture's getpos was its camera).
            let third = w.get_resource::<crate::client::view::CameraMode>().is_some_and(|m| m.third_person);
            let camera = third
                .then(|| {
                    let mut q = w.query_filtered::<&GlobalTransform, With<FirstPersonCamera>>();
                    q.iter(w).next().map(|g| g.translation())
                })
                .flatten();
            let eye = camera.unwrap_or(t.translation + eye_and_feet(w, p).0);
            let i = w.get::<crate::core::Intent>(p).ok_or("no intent")?;
            let s = Vec3::new(eye.x, -eye.z, eye.y) / 0.0254;
            Ok(Some(format!(
                "setpos {:.2} {:.2} {:.2};setang {:.2} {:.2} 0.00",
                s.x,
                s.y,
                s.z,
                -i.pitch.to_degrees(),
                (i.yaw.to_degrees() + 90.0).rem_euclid(360.0)
            )))
        },
    )
    .console_command(
        "chimera:hurtme",
        "hurtme <head|chest|stomach|leftarm|rightarm|leftleg|rightleg|generic> [amount=100] [from yaw, degrees: 0 = shot from the front] [attacker name]: a bullet hit on yourself at that hitbox (e.g. to see your ragdoll in thirdperson; killed by a named player, its death cam and freeze cam).",
        |w, a| {
            use crate::core::{Damage, DamageKind, Health, Hitgroup, Intent};
            let group = match a.first().map(|s| s.to_ascii_lowercase()).as_deref() {
                Some("head") => Hitgroup::Head,
                Some("chest") => Hitgroup::Chest,
                Some("stomach") => Hitgroup::Stomach,
                Some("leftarm") => Hitgroup::LeftArm,
                Some("rightarm") => Hitgroup::RightArm,
                Some("leftleg") => Hitgroup::LeftLeg,
                Some("rightleg") => Hitgroup::RightLeg,
                Some("generic") => Hitgroup::Generic,
                _ => return Err("chimera_hurtme <hitgroup> [amount] [from yaw] [attacker]".into()),
            };
            let attacker = match a.get(3) {
                Some(name) => {
                    let mut q = w.query::<(Entity, &Name)>();
                    let found = q
                        .iter(w)
                        .find(|(_, n)| n.as_str().eq_ignore_ascii_case(name))
                        .map(|(e, _)| e);
                    Some(found.ok_or_else(|| format!("nobody is called {name}"))?)
                }
                None => None,
            };
            let amount = a.get(1).and_then(|v| v.parse::<f32>().ok()).unwrap_or(100.0) / 100.0;
            let from = a.get(2).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).to_radians();
            let p = local_player(w)?;
            let t = *w.get::<Transform>(p).ok_or("no transform")?;
            let yaw = w.get::<Intent>(p).ok_or("no intent")?.yaw_rotation();
            let feet = w
                .get::<avian3d::prelude::ColliderAabb>(p)
                .map_or(t.translation, |b| t.translation.with_y(b.min.y));
            let point = crate::core::hitboxes_in(w, p)
                .and_then(|h| h.0.iter().find(|b| b.group() == group).map(|b| feet + yaw * b.center))
                .unwrap_or(t.translation);
            // Shot from `from` around the facing: travelling toward it.
            let dir = yaw * Quat::from_rotation_y(from) * Vec3::Z;
            let _ = w.get::<Health>(p).ok_or("no health")?;
            w.write_message(Damage {
                force: bevy::math::Vec3::ZERO,
                target: p,
                attacker,
                amount,
                point,
                dir,
                hitgroup: group,
                kind: DamageKind::Bullet,
                weapon: None,
            });
            Ok(None)
        },
    )
    .console_command(
        "maps",
        "maps [filter]: list the maps that can be loaded (the game's, its downloads, imported).",
        |w, a| {
            let filter = a.first().map(|f| f.to_lowercase()).unwrap_or_default();
            let Some(game) = crate::game_registry::default_game(w) else {
                return Err("no game registered".into());
            };
            let lines: Vec<String> = map_names(w, Some(game))
                .into_iter()
                .filter(|m| m.contains(&filter))
                .map(|m| {
                    let from = map_source(game, &m);
                    format!(
                        "{m}{}",
                        if from == "game" {
                            String::new()
                        } else {
                            format!("  ({from})")
                        }
                    )
                })
                .collect();
            Ok(Some(if lines.is_empty() {
                "no maps match".into()
            } else {
                format!("{}\n{} maps", lines.join("\n"), lines.len())
            }))
        },
    )
    .console_command(
        "chimera:import",
        "import <file.bsp|file.bsp.bz2>: copy a map into Chimera's content cache (never the game folder).",
        |w, a| {
            let path = a.join(" ");
            if path.is_empty() {
                return Err("import <file.bsp|file.bsp.bz2>".into());
            }
            let game = crate::game_registry::default_game(w).ok_or("no game registered")?;
            let name = (game.import_map)(std::path::Path::new(&path))?;
            forget_map_names(w);
            Ok(Some(format!("imported {name}; load it with: map {name}")))
        },
    )
    .console_command(
        "map",
        "map <name>: load a map of the game (Tab lists the install's maps); map greybox: Chimera's test map; map range: its firing range. \
         Hosting, it takes the players along (as changelevel); on a client, it leaves the server first.",
        |w, a| {
            let name = a.first().ok_or("map <name>")?.clone();
            if w.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client) {
                crate::net::disconnect(w, crate::net::client::BY_USER);
            }
            let game = crate::game_registry::default_game(w);
            let Some((id, check)) = map_target(game, &name) else {
                w.resource_mut::<Console>().submit("bot_kick");
                let id = crate::net::normalize_map(&name);
                load_code_map(w, &id);
                crate::client::front_end::entered_game(w);
                crate::client::net::listen_if_hosting(w);
                crate::client::front_end::start_game(w);
                return Ok(Some(format!("loaded the {id}")));
            };
            let short = crate::net::maps::map_name(&id).to_string();
            let names = map_names(w, game);
            if check && !names.is_empty() && !names.iter().any(|m| *m == short) {
                return Err(format!("no map \"{short}\" in the install"));
            }
            start_map_load(w, &id, None);
            // A game of our own starts: we join it once it's loaded.
            w.insert_resource(FreshGame);
            Ok(Some(format!("loading {id}...")))
        },
    )
    .console_command(
        "changelevel",
        "changelevel <name>: the server moves to another map; connected players load it and play on there \
         (bots stay). In a party app with no name: the minigame's next match, its map drawn as a rotation's.",
        |w, a| {
            if w.get_resource::<crate::core::NetRole>() != Some(&crate::core::NetRole::Server) {
                // Source's words.
                return Err("Can't changelevel, not running server.".into());
            }
            let Some(name) = a.first().cloned() else {
                // A party app's minigame: its next match, the map drawn
                // as a rotation's (for the players present).
                return crate::net::host_next_level(w).unwrap_or(Err("changelevel <name>".into()));
            };
            let game = crate::game_registry::default_game(w);
            let target = map_target(game, &name);
            // A party app's minigame: a new match there (its lobby), not
            // only the map.
            let id = target.as_ref().map_or_else(|| crate::net::normalize_map(&name), |(id, _)| id.clone());
            if let Some(r) = crate::net::host_changelevel(w, &id) {
                return r;
            }
            let Some((id, check)) = target else {
                load_code_map(w, &id);
                crate::client::front_end::entered_game(w);
                return Ok(Some(format!("changed level to the {id}")));
            };
            let short = crate::net::maps::map_name(&id).to_string();
            let names = map_names(w, game);
            if check && !names.is_empty() && !names.iter().any(|m| *m == short) {
                return Err(format!("no map \"{short}\" in the install"));
            }
            start_map_load(w, &id, None);
            Ok(Some(format!("changing level to {id}...")))
        },
    )
    .console_command(
        "disconnect",
        "Leave the game (or stop hosting it): bots kicked, the map unloaded, back to the main menu.",
        |w, _| {
            crate::net::disconnect(w, "Disconnect by user.");
            // A game without bots (HL2:DM) has no such command.
            let mut console = w.resource_mut::<Console>();
            if console.command("bot_kick").is_some() {
                console.submit("bot_kick");
            }
            // A menu with no game behind it (`NoPlayerAtMenu`): no map at
            // all, as at its start; else the greybox behind the menu.
            if w.contains_resource::<crate::client::NoPlayerAtMenu>() {
                unload_to_menu(w);
            } else {
                load_greybox(w);
            }
            crate::client::front_end::left_game(w);
            Ok(None)
        },
    )
    .console_command("toggleconsole", "Open or close the console.", |w, _| {
        w.resource_mut::<ConsoleUi>().toggle_request = true;
        Ok(None)
    })
    .console_command(
        "quit",
        "quit [prompt]: quit (binds and changed cvars are saved); prompt (CS:S's F10): open the game menu, \
         whose Quit quits (no quit dialog yet).",
        |w, a| {
            if a.first().is_some_and(|x| x.eq_ignore_ascii_case("prompt")) {
                w.resource_mut::<Console>().submit("menu main");
                return Ok(None);
            }
            crate::net::disconnect(w, "Quit.");
            w.write_message(AppExit::Success);
            Ok(None)
        },
    )
    .console_command("exit", "Quit.", |w, _| {
        crate::net::disconnect(w, "Quit.");
        w.write_message(AppExit::Success);
        Ok(None)
    })
    .console_command(
        "chimera:con_input",
        "con_input \"<text>\" [n]: open the console with text on its input line and its nth suggestion picked \
         (1: the first), as typing and Down would (for screenshots of the console).",
        |w, a| {
            let text = a.first().cloned().unwrap_or_default();
            let pick = a.get(1).and_then(|n| n.parse::<usize>().ok()).filter(|n| *n > 0);
            let mut ui = w.resource_mut::<ConsoleUi>();
            ui.open = true;
            ui.cursor = text.chars().count();
            ui.input = text;
            ui.highlight = pick.map(|n| n - 1);
            ui.browsing = None;
            ui.dismissed = None;
            ui.cycle = None;
            let mut roots = w.query_filtered::<&mut Visibility, With<ConsoleRoot>>();
            for mut v in roots.iter_mut(w) {
                *v = Visibility::Visible;
            }
            Ok(None)
        },
    )
    .console_command(
        "con_filter",
        "con_filter [text]: show only lines containing text (none: all).",
        |w, a| {
            w.resource_mut::<ConsoleUi>().filter = a.join(" ");
            Ok(None)
        },
    )
    .console_command(
        "chimera:con_dump",
        "con_dump [file]: write the console output to the cfg folder (condump.txt).",
        |w, a| {
            let name = a.first().map_or("condump.txt", |s| s.as_str());
            let text: Vec<String> = w.resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
            let path = cfg_dir().ok_or("no config folder")?.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
            std::fs::write(&path, text.join("\n")).map_err(|e| e.to_string())?;
            Ok(Some(format!("wrote {}", path.display())))
        },
    )
    .console_command(
        "chimera:repair_options",
        "repair_options: put back a volume, music volume and brightness the party apps' options bug saved (run once \
         at start for a config written before the fix).",
        |w, _| {
            let mut value = |name: &str| {
                let c = w.resource::<Console>().cvar(name).cloned()?;
                (c.get)(w).and_then(|v| v.trim().parse::<f32>().ok())
            };
            let (v, m, g) = (value("volume"), value("snd_musicvolume"), value("mat_monitorgamma"));
            let lines = repair_options(v, m, g);
            if lines.is_empty() {
                return Ok(None);
            }
            let mut console = w.resource_mut::<Console>();
            for l in &lines {
                console.submit(l.clone());
            }
            Ok(Some(format!("options repaired: {}", lines.join("; "))))
        },
    )
    .console_command(
        "chimera:con_copy",
        "con_copy [lines]: copy the console output (or its last lines) to the clipboard.",
        |w, a| {
            let output = &w.resource::<Console>().output;
            let n = match a.first() {
                Some(n) => n.parse::<usize>().map_err(|_| format!("bad line count \"{n}\""))?,
                None => output.len(),
            };
            let text: Vec<String> = output.iter().skip(output.len().saturating_sub(n)).map(|l| l.text.clone()).collect();
            let count = text.len();
            w.get_resource_or_init::<Clipboard>().set(&text.join("\n"));
            Ok(Some(format!("copied {count} lines")))
        },
    );
}

/// Remote protocol method `chimera/console` (and its old name, `mashup/console`): params `{"line": "..."}` runs
/// the line now and returns the output lines it printed.
#[cfg(feature = "dev")]
pub fn remote_exec(
    bevy::ecs::system::In(params): bevy::ecs::system::In<Option<serde_json::Value>>,
    world: &mut World,
) -> bevy::remote::BrpResult {
    let line = params
        .as_ref()
        .and_then(|p| p.get("line"))
        .and_then(|l| l.as_str())
        .ok_or(bevy::remote::BrpError {
            code: bevy::remote::error_codes::INVALID_PARAMS,
            message: "expected {\"line\": \"command\"}".into(),
            data: None,
        })?
        .to_string();
    let start = world.resource::<Console>().printed;
    {
        let mut c = world.resource_mut::<Console>();
        c.print(Level::Input, format!("] {line}"));
        c.submit(line);
    }
    crate::console::run_queue(world);
    // The lines printed since (the output keeps only the last ones, and
    // logs print into it too).
    let c = world.resource::<Console>();
    let new = ((c.printed - start).saturating_sub(1) as usize).min(c.output.len());
    let out: Vec<String> = c.output[c.output.len() - new..]
        .iter()
        .map(|l| l.text.clone())
        .collect();
    Ok(serde_json::json!({ "output": out }))
}


/// Take the app's greybox out (`AppHooks::unload_greybox`) for a map.
pub(super) fn unload_greybox(w: &mut World) {
    if let Some(hooks) = w.get_resource::<crate::client::AppHooks>().copied() {
        (hooks.unload_greybox)(w);
    }
}
