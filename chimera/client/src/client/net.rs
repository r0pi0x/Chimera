//! What the game does about the network (`net`): load the server's map
//! when it asks (by name, or a copy it downloaded), show joining and the
//! server's map changes in the loading dialog (`game_menu`), leave the
//! menu once joined, and come back to the main menu (a fresh local player
//! on the greybox) when dropped, with why in the dialog (a server that went
//! away is joined again on its own, `net::reconnect`; giving up on it shows
//! the dialog).
//!
//! The game's side of the network: loads the server's map when asked, leaves
//! the menu once joined, back to the main menu with a fresh local player when
//! dropped; `listen_if_hosting` after a map loads with `maxplayers` > 1.
//! `relaunch` (its own module): a join bound the player's asset token to the server's asset
//! service (`net::assets::AssetRelaunch`): the game starts again (the same
//! program and options, joining that server) and this one quits, since the
//! games' mounts are opened at startup.

use std::sync::Arc;

use bevy::prelude::*;

use crate::{
    console::{Console, Level},
    core::{LocalPlayer, NetRole},
    net::{NetEvent, NetSettings, maps::MapFiles},
};

pub struct ClientNetPlugin;

impl Plugin for ClientNetPlugin {
    fn build(&self, app: &mut App) {
        // Map files as the game loads them (the install, its downloads,
        // Chimera's content cache: the default game's, `game_registry`), for
        // the server's offer and the client's checks and downloads.
        // The app reads them (`AppHooks::map_file_bytes`).
        let game = crate::game_registry::default_game(app.world());
        let read = app.world().get_resource::<super::AppHooks>().map(|h| h.map_file_bytes);
        app.insert_resource(MapFiles {
            read: Arc::new(move |id: &str| read.and_then(|f| f(id))),
            cache: game.and_then(|g| g.content_cache()),
        })
        .add_systems(Update, react);
        app.add_systems(
            Update,
            super::relaunch::relaunch.run_if(resource_added::<crate::net::assets::AssetRelaunch>),
        );
    }
}

fn react(world: &mut World, mut cursor: Local<bevy::ecs::message::MessageCursor<NetEvent>>) {
    let events: Vec<NetEvent> = cursor.read(world.resource::<Messages<NetEvent>>()).cloned().collect();
    for e in events {
        match e {
            NetEvent::Listening(addr) => {
                let map = crate::net::current_map(world);
                world.resource_mut::<Console>().info(format!(
                    "Hosting {map} on UDP port {} (players join with: connect <your ip>:{})",
                    addr.port(),
                    addr.port()
                ));
            }
            NetEvent::Connecting(addr) => {
                world.resource_mut::<Console>().info(format!("Connecting to {addr}..."));
                super::front_end::joining(world, &addr.to_string());
            }
            NetEvent::LoadMap { map, file } => {
                super::console::load_server_map(world, &map, file);
                let name = crate::net::maps::map_name(&map).to_string();
                super::front_end::joining(world, &name);
            }
            NetEvent::ChangingLevel(map) => {
                world
                    .resource_mut::<Console>()
                    .info(format!("The server is changing level to {map}..."));
                let name = crate::net::maps::map_name(&map).to_string();
                super::front_end::joining(world, &name);
            }
            NetEvent::Joined { map } => {
                world
                    .resource_mut::<Console>()
                    .info(format!("Joined the game on {map}."));
                super::front_end::entered_game(world);
            }
            NetEvent::Disconnected(reason) => {
                world
                    .resource_mut::<Console>()
                    .print(Level::Warn, format!("Disconnected: {reason}"));
                back_to_menu(world);
            }
            NetEvent::GaveUpReconnecting(why) => {
                world
                    .resource_mut::<Console>()
                    .print(Level::Warn, format!("Gave up reconnecting: {why}"));
                super::front_end::show_failure(world);
            }
        }
    }
}

/// After leaving a server: our own player back on the greybox behind the
/// main menu, as `disconnect` leaves single player; the dialog shows why
/// if the game ended on its own (`net::client::JoinProgress::failure`).
fn back_to_menu(world: &mut World) {
    let failed = world
        .get_resource::<crate::net::client::JoinProgress>()
        .is_some_and(|p| p.failure.is_some());
    let has_player = world
        .query_filtered::<(), With<LocalPlayer>>()
        .iter(world)
        .next()
        .is_some();
    if has_player {
        if failed {
            super::front_end::left_game(world);
            super::front_end::show_failure(world);
        }
        return;
    }
    super::console::load_greybox(world);
    super::front_end::left_game(world);
    if failed {
        super::front_end::show_failure(world);
    }
    if let Err(e) = world.run_system_cached(super::spawn_local_player) {
        error!("spawning the local player: {e}");
    }
}

/// A map finished loading (or the greybox came back): host it when
/// `maxplayers` asks for other players (Source's `maxplayers 8; map x`).
pub(super) fn listen_if_hosting(world: &mut World) {
    let standalone = world
        .get_resource::<NetRole>()
        .is_none_or(|r| *r == NetRole::Standalone);
    let wanted = world.get_resource::<NetSettings>().is_some_and(|s| s.maxplayers > 1);
    if !standalone || !wanted {
        return;
    }
    if let Err(e) = crate::net::server::listen(world) {
        world
            .resource_mut::<Console>()
            .print(Level::Error, format!("Can't host: {e}"));
    }
}
