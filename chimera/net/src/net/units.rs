//! Units of game files on demand (the sandbox's progressive downloads,
//! docs/backlog.md; docs/plans/active/ingest.md 3.5a): a game's set
//! (`weapon/<id>`, `players/<class>`, `character/<id>`, `voice/<id>`,
//! `map/<name>`) fetched in the background from the bound asset service
//! with the sets it needs, within a bandwidth budget, never blocking a
//! frame (`chimera_assets::units::Downloads` does the fetching on its
//! threads; this is its place in the app).
//!
//! - **Asking**: `request_unit(world, game, set)` (a pick: first in the
//!   queue), `request_unit_at` (`Priority::Pin`, `Later`), `retry_unit`
//!   (after `Failed`). The closure (`closure`): the unit, its game's
//!   `base`, `players` and `common` (`resolve::MATCH_SETS`; skipped where
//!   the manifest doesn't name them), and what the `UnitNeeds` hooks
//!   name, followed through (TF2: a gun's class bodies, `players/<class>`,
//!   with the classes `classes.toml` says they need). Only games read from
//!   the store (`AssetSession::served`, a server's `ServerFiles::served`)
//!   are fetched; another game's units count as `Ready` (its install has
//!   them).
//! - **State** (`UnitProgress`, by `(game, set)`): `Absent { size }`,
//!   `Queued { position }`, `Fetching`, `Ready`, `Failed { reason }`, with
//!   files and bytes done and in all (`UnitStatus`). `describe_units`
//!   fills in `Absent { size }` from the kept manifest before anything is
//!   fetched (the tiles' "not here: 4.8 MB"). `UnitChanged` is written on
//!   every change. Once `Ready`, the files are in the store, which every
//!   mount reads at once.
//! - **Guns**: `gun_unit(world, weapon id)` (`weapon/<id>`; for a game
//!   whose guns come with its `base`, `sets::MATCH_BASE_GAMES` (Halo's
//!   donor map), its `base`), `gun_units` (its closure, the served games'
//!   only), `want_units` (asks for what isn't ready, says whether all is),
//!   `units_ready` (looks only). `chimera_sim::minigame::gun_files::
//!   GunFiles` gets a hook (`gun_files_hook`), so a rung switch below this
//!   crate can wait with `gun_files::gun_ready(world, id)`.
//! - **The no-fetch-in-match guard** (`guard`): a unit is fetched on
//!   demand only while the sandbox plays (`rung_command::holds`), or when
//!   it is `deferred` (`DeferredUnits`, by kind: `voice/`, Halo's voices,
//!   wanted after the body is usable). In a normal match or the menu
//!   nothing new is fetched: a request is refused with the reason, and the
//!   match's files come with the join's or level change's download as
//!   before (`assets`). When the sandbox ends (to the menu, a map or a
//!   match), every queued unit that isn't `deferred` is unqueued
//!   (`end_sandbox`: back to `Absent`) and nothing further starts; the
//!   fetch already running ends by itself. On-demand reads of
//!   single files by the mounts (`CachedFiles`) are not this guard's: they
//!   are counted and reported (`on_demand`).
//! - **Budget** (`UnitBudget`, `chimera:unit_rate` KB/s, 0: no cap):
//!   every unit shares one rate; downloads hold while a foreground
//!   download runs (a join's, the next map ahead, a server's own match
//!   fetch: `assets::foreground_busy`, `server_assets::fetching`) and stop
//!   at quit. They run on threads of their own and only write records to
//!   the store (nothing decodes or mounts with them), so they take nothing
//!   from the async compute pool (`chimera_core::tasks::ReadAhead` is for
//!   loads started ahead of need there); a ready gun's files load when it
//!   is held, as any load.
//! - **Server side**: on a server the units come into its own store with
//!   its own credential (`ServerFiles`, as `server_assets` fetches); on a
//!   listen server that's the host's store, shared with its client. A
//!   rung switch waits with `gun_files::gun_ready` (or `want_units` and
//!   `gun_units` here) and switches once true.
//!
//! Known shortcuts: settings are taken once, when the first unit is asked
//! for or described (a token bound later is used by the next process); the
//! running fetch isn't cut short by a newer pick.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use chimera_assets::{
    AssetSettings,
    resolve,
    sets,
    units::{Downloads, UnitKey},
};
pub use chimera_assets::units::{Priority, UnitState, UnitStatus};
use chimera_core::console::{ConsoleAppExt, resource_cvar};

use super::{
    assets::{self, AssetSession},
    server_assets::{self, ServerFiles},
};

/// Every unit asked for or described, by `(game, set)` (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct UnitProgress(pub BTreeMap<UnitKey, UnitStatus>);

impl UnitProgress {
    pub fn status(&self, game: &str, set: &str) -> Option<&UnitStatus> {
        self.0.get(&(game.to_string(), set.to_string()))
    }

    pub fn state(&self, game: &str, set: &str) -> Option<&UnitState> {
        self.status(game, set).map(|s| &s.state)
    }

    pub fn ready(&self, game: &str, set: &str) -> bool {
        self.state(game, set) == Some(&UnitState::Ready)
    }
}

/// A unit's status changed.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct UnitChanged {
    pub game: String,
    pub set: String,
    pub status: UnitStatus,
}

/// What a unit needs beyond its game's shared sets, per part that knows
/// (`fn(world, game, set) -> [(game, set)]`; TF2: a gun's class bodies).
#[derive(Resource, Clone, Default)]
pub struct UnitNeeds(pub Vec<fn(&World, &str, &str) -> Vec<(String, String)>>);

/// Kinds of unit fetched in a match too, after what they belong to is
/// usable (set name prefixes; the asset graph's `deferred` edges).
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct DeferredUnits(pub Vec<String>);

impl Default for DeferredUnits {
    fn default() -> Self {
        Self(vec!["voice/".into()])
    }
}

/// The units' bandwidth budget (module doc).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnitBudget {
    /// KB (1000 bytes) a second; 0 or less: no cap.
    pub kb_per_sec: i64,
}

impl Default for UnitBudget {
    fn default() -> Self {
        // A share of a host's upload, as the streams assume (`StreamLink`).
        Self { kb_per_sec: 4000 }
    }
}

impl UnitBudget {
    fn rate(self) -> u64 {
        u64::try_from(self.kb_per_sec).ok().filter(|k| *k > 0).map_or(u64::MAX, |k| k.saturating_mul(1000))
    }
}

/// The cvar of `UnitBudget`.
pub const RATE_CVAR: &str = "chimera:unit_rate";

/// The downloads, made the first time a unit is asked for or described
/// with a service bound.
#[derive(Resource)]
pub struct UnitDownloads {
    pub downloads: Downloads,
    /// The games read from the store (the others' units are here).
    pub served: BTreeSet<String>,
}

/// The store, its service and the games read from it: a server's own
/// (`ServerFiles`), else the client's session (`AssetSession`, made from
/// the local config the first time).
fn source(world: &mut World) -> Option<(AssetSettings, BTreeSet<String>)> {
    if let Some(f) = world.get_resource::<ServerFiles>() {
        return Some((f.settings.clone(), f.served.clone()));
    }
    let session = assets::session(world);
    let settings = session.settings.filter(|s| s.remote.is_some() && s.store.is_some())?;
    Some((settings, session.served))
}

/// The downloads, made now if there's a service (None: no service; every
/// unit is as the installs have it).
fn downloads(world: &mut World) -> Option<Mut<'_, UnitDownloads>> {
    if !world.contains_resource::<UnitDownloads>() {
        let (settings, served) = source(world)?;
        let rate = world.get_resource::<UnitBudget>().copied().unwrap_or_default().rate();
        world.insert_resource(UnitDownloads { downloads: Downloads::new(settings, rate), served });
    }
    world.get_resource_mut::<UnitDownloads>()
}

/// The no-fetch-in-match guard (module doc): Ok when `game`'s `set` may be
/// fetched now.
pub fn guard(world: &World, game: &str, set: &str) -> Result<(), String> {
    let deferred = world.get_resource::<DeferredUnits>().cloned().unwrap_or_default();
    if deferred.0.iter().any(|k| set.starts_with(k.as_str())) {
        return Ok(());
    }
    if chimera_sim::minigame::rung_command::holds(world) {
        return Ok(());
    }
    Err(format!(
        "{game}'s {set} isn't fetched outside the sandbox (nothing new downloads in a match or the menu: the match's \
         files come with the join); in the sandbox it would be"
    ))
}

/// `game`'s `set` with what it needs, followed through (module doc), the
/// unit first.
pub fn closure(world: &World, game: &str, set: &str) -> Vec<UnitKey> {
    let hooks = world.get_resource::<UnitNeeds>().cloned().unwrap_or_default();
    let shared = |s: &str| s == resolve::BASE || resolve::MATCH_SETS.contains(&s);
    let mut out = vec![(game.to_string(), set.to_string())];
    let mut i = 0;
    while let Some((g, s)) = out.get(i).cloned() {
        let mut next: Vec<UnitKey> = Vec::new();
        if !shared(&s) {
            next.extend(std::iter::once(resolve::BASE).chain(resolve::MATCH_SETS.iter().copied()).map(|x| (g.clone(), x.to_string())));
        }
        for hook in &hooks.0 {
            next.extend(hook(world, &g, &s));
        }
        for k in next {
            if !out.contains(&k) {
                out.push(k);
            }
        }
        i += 1;
    }
    out
}

/// Ask for `game`'s `set` as a pick (module doc). Err: refused by the
/// guard, or no service to fetch from.
pub fn request_unit(world: &mut World, game: &str, set: &str) -> Result<(), String> {
    request_unit_at(world, game, set, Priority::Pick)
}

/// `request_unit` at `priority`.
pub fn request_unit_at(world: &mut World, game: &str, set: &str, priority: Priority) -> Result<(), String> {
    guard(world, game, set)?;
    let all = closure(world, game, set);
    let mut d = downloads(world).ok_or("no asset service to fetch from")?;
    let key = (game.to_string(), set.to_string());
    if !d.served.contains(game) {
        drop(d);
        mark_here(world, &key);
        return Ok(());
    }
    let served = d.served.clone();
    let deps: Vec<UnitKey> = all.into_iter().filter(|(g, _)| served.contains(g)).collect();
    d.downloads.request(key, deps, priority);
    Ok(())
}

/// Ask again for a unit that failed (at a pick's priority).
pub fn retry_unit(world: &mut World, game: &str, set: &str) -> Result<(), String> {
    guard(world, game, set)?;
    let key = (game.to_string(), set.to_string());
    let mut d = downloads(world).ok_or("no asset service to fetch from")?;
    if !d.downloads.retry(&key) {
        drop(d);
        return request_unit(world, game, set);
    }
    Ok(())
}

/// A unit of a game not read from the store: here (its install).
fn mark_here(world: &mut World, key: &UnitKey) {
    let status = UnitStatus { state: UnitState::Ready, ..Default::default() };
    let mut p = world.get_resource_or_init::<UnitProgress>();
    if p.0.get(key) != Some(&status) {
        p.0.insert(key.clone(), status.clone());
        world.write_message(UnitChanged { game: key.0.clone(), set: key.1.clone(), status });
    }
}

/// Find out, in the background, which of `units` are here and what the
/// others would download (`Absent { size }`); no guard (nothing is
/// fetched). Without a service, nothing (every unit is as installed).
pub fn describe_units(world: &mut World, units: &[UnitKey]) {
    let with: Vec<(UnitKey, Vec<UnitKey>)> = units.iter().map(|(g, s)| ((g.clone(), s.clone()), closure(world, g, s))).collect();
    let Some(mut d) = downloads(world) else { return };
    let served = d.served.clone();
    let (mine, installed): (Vec<_>, Vec<_>) = with.into_iter().partition(|(k, _)| served.contains(&k.0));
    let mine = mine.into_iter().map(|(k, c)| (k, c.into_iter().filter(|(g, _)| served.contains(g)).collect())).collect();
    d.downloads.describe(mine);
    drop(d);
    for (k, _) in installed {
        mark_here(world, &k);
    }
}

/// The unit holding gun `weapon`'s files (`game:id`; module doc). None for
/// an id without a game.
pub fn gun_unit(weapon: &str) -> Option<UnitKey> {
    let (game, name) = weapon.split_once(':')?;
    let set = if sets::MATCH_BASE_GAMES.contains(&game) { resolve::BASE.to_string() } else { assets::weapon_set(name) };
    Some((game.to_string(), set))
}

/// Gun `weapon`'s unit and what it needs (`closure`).
pub fn gun_units(world: &World, weapon: &str) -> Vec<UnitKey> {
    gun_unit(weapon).map(|(g, s)| closure(world, &g, &s)).unwrap_or_default()
}

/// Whether every unit of `keys` is here: `Ready`, or of a game not read
/// from the store (or no service: as installed). Looks only.
pub fn units_ready(world: &World, keys: &[UnitKey]) -> bool {
    let Some(d) = world.get_resource::<UnitDownloads>() else {
        // Nothing was asked for: with a server's or session's service
        // bound, a served game's unit isn't known to be here.
        let served = world
            .get_resource::<ServerFiles>()
            .map(|f| f.served.clone())
            .or_else(|| {
                world
                    .get_resource::<AssetSession>()
                    .filter(|s| s.settings.as_ref().is_some_and(|s| s.remote.is_some()))
                    .map(|s| s.served.clone())
            })
            .unwrap_or_default();
        return keys.iter().all(|(g, _)| !served.contains(g));
    };
    keys.iter().all(|(g, s)| !d.served.contains(g) || d.downloads.status(&(g.clone(), s.clone())).is_some_and(|x| x.state == UnitState::Ready))
}

/// Ask (as a pick) for the units of `keys` not ready, and say whether all
/// are. A unit the guard refuses, or with no service, doesn't hold this
/// up (its files come on demand as before).
pub fn want_units(world: &mut World, keys: &[UnitKey]) -> bool {
    if units_ready(world, keys) {
        return true;
    }
    let mut waiting = false;
    for (g, s) in keys {
        if world.get_resource::<UnitDownloads>().is_some_and(|d| d.downloads.status(&(g.clone(), s.clone())).is_some_and(|x| x.state == UnitState::Ready)) {
            continue;
        }
        match world.get_resource::<UnitDownloads>().and_then(|d| d.downloads.status(&(g.clone(), s.clone())).cloned()) {
            Some(UnitStatus { state: UnitState::Failed { .. }, .. }) => continue,
            Some(UnitStatus { state: UnitState::Queued { .. } | UnitState::Fetching, .. }) => waiting = true,
            _ => match request_unit(world, g, s) {
                Ok(()) => waiting |= world.get_resource::<UnitDownloads>().is_some_and(|d| d.served.contains(g)),
                Err(e) => debug!("units: {g} {s}: {e}"),
            },
        }
    }
    !waiting
}

/// `GunFiles`' hook (module doc): gun `weapon`'s units wanted and ready.
pub fn gun_files_hook(world: &mut World, weapon: &str) -> bool {
    let keys = gun_units(world, weapon);
    want_units(world, &keys)
}

/// Each frame with downloads: polled, held while a foreground download
/// runs, their changes into `UnitProgress` and `UnitChanged`.
fn run_units(
    mut d: ResMut<UnitDownloads>,
    budget: Option<Res<UnitBudget>>,
    fetch: Option<Res<assets::AssetFetch>>,
    ahead: Option<Res<assets::MapAhead>>,
    server: Option<Res<server_assets::ServerFetching>>,
    mut progress: ResMut<UnitProgress>,
    mut changed: MessageWriter<UnitChanged>,
) {
    let busy = fetch.is_some() || ahead.is_some_and(|a| a.running()) || server.is_some();
    d.downloads.pause(busy);
    d.downloads.set_rate(budget.map(|b| *b).unwrap_or_default().rate());
    let changes = d.downloads.poll();
    for ((game, set), status) in changes {
        if let UnitState::Failed { reason } = &status.state {
            info!("units: {game} {set} didn't arrive: {reason}");
        }
        progress.0.insert((game.clone(), set.clone()), status.clone());
        changed.write(UnitChanged { game, set, status });
    }
}

/// The sandbox ended: unqueue every on-demand unit not `deferred` (module
/// doc). `was`: whether it played last frame.
fn end_sandbox(
    hold: Option<Res<chimera_sim::minigame::rung_command::HoldRungs>>,
    active: Option<Res<chimera_sim::minigame::ActiveMinigame>>,
    deferred: Res<DeferredUnits>,
    mut d: ResMut<UnitDownloads>,
    mut was: Local<bool>,
) {
    let now = hold.zip(active).is_some_and(|(h, a)| a.def.id == h.0);
    if !std::mem::replace(&mut *was, now) || now {
        return;
    }
    d.downloads.cancel_queued(|(_, s)| deferred.0.iter().any(|k| s.starts_with(k.as_str())));
}

/// Quitting: the running download stops at its next chunk.
fn stop_units(world: &mut World) {
    if let Some(d) = world.get_resource::<UnitDownloads>() {
        d.downloads.stop();
    }
}

pub(super) fn plugin(app: &mut App) {
    use chimera_core::shutdown::{ShutdownAppExt, Stage};
    app.shutdown_flush(Stage::Persist, "unit downloads", stop_units);
    app.init_resource::<UnitProgress>()
        .init_resource::<UnitNeeds>()
        .init_resource::<DeferredUnits>()
        .init_resource::<UnitBudget>()
        .add_message::<UnitChanged>()
        .add_systems(Update, (end_sandbox, run_units).chain().run_if(resource_exists::<UnitDownloads>));
    resource_cvar::<UnitBudget, i64>(
        app,
        RATE_CVAR,
        "The most the sandbox's on-demand downloads (a gun picked, its files) read, KB/s; 0: no cap.",
        |b| &mut b.kb_per_sec,
    );
    app.world_mut()
        .get_resource_or_init::<chimera_sim::minigame::gun_files::GunFiles>()
        .0
        .push(gun_files_hook);
    app.console_command(
        "chimera:unit_fetch",
        "unit_fetch <game> <set>: download a unit of game files in the background (the sandbox's way; refused \
         during a match), e.g. `unit_fetch tf2 weapon/tf_weapon_minigun`; alone: every unit asked for and its state.",
        |w, a| match (a.first(), a.get(1)) {
            (Some(g), Some(s)) => request_unit(w, g, s).map(|()| Some(format!("{g} {s}: asked for"))),
            _ => Ok(Some(
                w.get_resource::<UnitProgress>()
                    .map(|p| {
                        p.0.iter()
                            .map(|((g, s), st)| format!("{g} {s}: {:?} ({} of {} bytes)", st.state, st.bytes_done, st.bytes_total))
                            .collect::<Vec<_>>()
                            .join("\n")
                    })
                    .unwrap_or_default(),
            )),
        },
    );
}
