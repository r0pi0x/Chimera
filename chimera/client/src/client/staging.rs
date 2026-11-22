//! Staging between matches (docs/plans/active/ugg-ui.md M9): the world's
//! view off while a full-screen state stands in for the match, and the
//! map a match left unloaded behind it.
//!
//! - **The world's view off.** The pre-match lobby (`prematch_lobby`,
//!   `LOBBY`) and an app's loading screens (`party_kit::main_menu`,
//!   `MENU`) each put up a cover by name (`WorldCover`, as `hud::HudCover`
//!   does for the HUDs); while any is up, every 3D camera drawing to a
//!   window is inactive (`apply`, in `PostUpdate`), so the map, the
//!   players and the view model cost no frame and show nowhere, and a
//!   blank camera clears the window under the UI (the UI cameras clear
//!   nothing). While a map settles behind it (`map::MapSettling`) they
//!   draw under the blank camera instead (`UNDER`), unseen: the first
//!   view's pipelines compile and its materials are prepared then, not as
//!   the cover goes (`first_view`: 26-31 meshes' pipelines compiling for
//!   5-10 frames as it went on Combat Arms' warehouse and dm_lockdown). When the last cover goes, each camera is set back to what
//!   it was (or to what its owner set meanwhile). Cameras drawing to an
//!   image (monitors, the menu's Showcase, captures) are left alone.
//! - **The map left, unloaded.** A network client told the server
//!   changes level once the match is over (the next match of a party
//!   server: `UnloadAtCover`, set by `client::net`) unloads the map it is
//!   on (`map::unload_map`) as soon as a cover is up (the logo pass's
//!   swap to the loading screen), so nothing of the last match stays
//!   loaded under the lobby while the next map downloads and loads; the
//!   load itself replaces it otherwise (a change mid-match, a CS:S level
//!   change: as before).
//!
//! - **The world unheard.** While a cover is up the game's sounds are held
//!   too (`chimera_present::audio::sound::WorldUnheard`, `held`).
//!
//! Part of `ClientPlugins` (`StagingPlugin`); headless apps get the
//! resources and the unload (the cameras' part is a no-op without
//! cameras).

use std::collections::HashMap;

use bevy::{camera::RenderTarget, prelude::*};

/// The `WorldCover` name of the pre-match lobby.
pub const LOBBY: &str = "lobby";
/// The `WorldCover` name of an app's own screens between matches (the
/// party apps' loading panel and lobby screen).
pub const MENU: &str = "menu";

/// Screens standing in for the match, by name: while any is up the
/// world's cameras are off.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct WorldCover(Vec<&'static str>);

impl WorldCover {
    /// `by` up (or down).
    pub fn set(&mut self, by: &'static str, up: bool) {
        let at = self.0.iter().position(|b| *b == by);
        match (at, up) {
            (None, true) => self.0.push(by),
            (Some(i), false) => {
                self.0.remove(i);
            }
            _ => {}
        }
    }

    /// Whether `by` covers the world.
    pub fn is(&self, by: &str) -> bool {
        self.0.contains(&by)
    }

    /// `by` up (or down), the resource changed only when that differs.
    pub fn put(cover: &mut ResMut<Self>, by: &'static str, up: bool) {
        if cover.is(by) != up {
            cover.set(by, up);
        }
    }

    /// Whether any screen covers the world.
    pub fn covered(&self) -> bool {
        !self.0.is_empty()
    }
}

/// The camera that clears the window while the world is off.
#[derive(Component)]
pub struct BlankCamera;

/// `WorldCover` and what applies it.
pub struct StagingPlugin;

impl Plugin for StagingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WorldCover>()
            .add_systems(Update, (unload_when_covered, unheard_when_covered))
            .add_systems(PostUpdate, apply.before(bevy::camera::CameraUpdateSystems));
    }
}

/// The world not heard while it is covered, as it is not seen
/// (`chimera_present::audio::sound::WorldUnheard`: the game's sounds held).
fn unheard_when_covered(cover: Res<WorldCover>, unheard: Option<Res<chimera_present::audio::sound::WorldUnheard>>, mut commands: Commands) {
    match (cover.covered(), unheard.is_some()) {
        (true, false) => commands.insert_resource(chimera_present::audio::sound::WorldUnheard),
        (false, true) => commands.remove_resource::<chimera_present::audio::sound::WorldUnheard>(),
        _ => {}
    }
}

/// A network client's map to unload once the world is covered: the one
/// it was on when the server, its match over, began changing level.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct UnloadAtCover(pub String);

/// Called by `client::net` when the server begins changing level: with
/// the match over (a party server's next match), the map this client is
/// on goes once a cover is up (`unload_when_covered`).
pub fn changing_level(world: &mut World) {
    let over = world
        .get_resource::<chimera_sim::mode::Match>()
        .is_some_and(|m| m.phase == chimera_sim::mode::Phase::MatchOver);
    let here = crate::net::current_map(world);
    if over && !here.is_empty() && !world.contains_resource::<UnloadAtCover>() {
        world.insert_resource(UnloadAtCover(here));
    }
}

/// `UnloadAtCover` done once a cover is up (`unload_now`, queued).
fn unload_when_covered(pending: Option<Res<UnloadAtCover>>, cover: Res<WorldCover>, mut commands: Commands) {
    if pending.is_some() && cover.covered() {
        commands.queue(unload_now);
    }
}

/// The map left unloaded (`map::unload_map`: the world's job), unless a
/// load already took its place (its own unload does it), the change got
/// that far, or the client left.
fn unload_now(world: &mut World) {
    let Some(UnloadAtCover(map)) = world.remove_resource::<UnloadAtCover>() else { return };
    use crate::net::client::{JoinProgress, JoinStage};
    let client = world.get_resource::<crate::core::NetRole>() == Some(&crate::core::NetRole::Client);
    let changing = world
        .get_resource::<JoinProgress>()
        .is_some_and(|p| matches!(p.stage, JoinStage::ChangingLevel | JoinStage::Verifying | JoinStage::Downloading));
    if !client || !changing || super::console::map_loading(world) || crate::net::current_map(world) != map {
        return;
    }
    let parts = world.query_filtered::<(), With<chimera_world::map::MapPart>>().iter(world).count();
    chimera_world::map::unload_map(world);
    info!("staging: unloaded {map} ({parts} parts) under the cover; the next map loads behind it");
    world.insert_resource(MapLeft(map));
}

/// The map a client unloaded under the cover (`unload_when_covered`),
/// until the next one loads (tests, the log).
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct MapLeft(pub String);

/// Adds `StagingPlugin` unless it is in.
pub fn staging(app: &mut App) {
    if !app.is_plugin_added::<StagingPlugin>() {
        app.add_plugins(StagingPlugin);
    }
}

/// How far below its own order a camera draws while it warms under the
/// cover: below the blank camera (-1000), which clears over it.
const UNDER: isize = -2000;

/// The world's cameras off while covered, each back after; the blank
/// camera on meanwhile. While a map settles behind the cover they draw
/// under the blank camera instead (`UNDER`).
#[allow(clippy::type_complexity)]
fn apply(
    cover: Res<WorldCover>,
    settling: Option<Res<crate::map::MapSettling>>,
    mut cameras: Query<(Entity, &mut Camera, Option<&RenderTarget>), (With<Camera3d>, Without<BlankCamera>)>,
    mut blank: Query<&mut Camera, (With<BlankCamera>, Without<Camera3d>)>,
    mut off: Local<HashMap<Entity, bool>>,
    mut under: Local<HashMap<Entity, isize>>,
    mut commands: Commands,
) {
    let covered = cover.covered();
    // A map settling behind the cover: its cameras draw under the blank
    // camera (`UNDER`), unseen, so the first view's pipelines compile and
    // its materials are prepared behind the loading screen (`first_view`).
    let warm = covered && settling.is_some();
    for (e, mut cam, _) in &mut cameras {
        match (warm && off.get(&e) == Some(&true), under.get(&e).copied()) {
            (true, None) => {
                under.insert(e, cam.order);
                cam.order += UNDER;
                cam.is_active = true;
            }
            (false, Some(order)) => {
                under.remove(&e);
                cam.order = order;
                if covered {
                    cam.is_active = false;
                }
            }
            _ => {}
        }
    }
    under.retain(|e, _| cameras.contains(*e));
    if !covered && off.is_empty() {
        if let Some(mut b) = blank.iter_mut().next()
            && b.is_active
        {
            b.is_active = false;
        }
        return;
    }
    for (e, mut cam, target) in &mut cameras {
        let window = target.is_none_or(|t| matches!(t, RenderTarget::Window(_)));
        if !window {
            continue;
        }
        if covered {
            if under.contains_key(&e) {
                continue;
            }
            if cam.is_active {
                off.entry(e).or_insert(true);
                cam.is_active = false;
            }
        } else if let Some(was) = off.remove(&e)
            && cam.is_active != was
        {
            cam.is_active = was;
        }
    }
    if !covered {
        off.clear();
    } else {
        off.retain(|e, _| cameras.contains(*e));
    }
    match blank.iter_mut().next() {
        Some(mut b) => {
            if b.is_active != covered {
                b.is_active = covered;
            }
        }
        None if covered => {
            commands.spawn((
                Name::new("Blank camera (world covered)"),
                BlankCamera,
                Camera2d,
                Camera {
                    order: -1000,
                    clear_color: ClearColorConfig::Custom(Color::BLACK),
                    ..default()
                },
                // Draws nothing of its own: a layer nothing is on.
                bevy::camera::visibility::RenderLayers::layer(29),
            ));
        }
        None => {}
    }
}
