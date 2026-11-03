//! The Half-Life 2: Deathmatch game binary (`hl2dm`): HL2:DM's simulation
//! (`hl2dm::SimPlugins` + `Hl2dmPlugins`) on a map of the install
//! (`--map hl2dm:<name>`, e.g. `hl2dm:dm_lockdown`) or the greybox, the
//! presentation, the network and the client shell. M1: walk the stock
//! maps with the stand-in movement and shoot its bullet and melee weapons. The options
//! are the shell's, as the CS:S app's (`--help`).
//!
//! Network play is the shell's (`connect`, `-port`, `maxplayers`, `map`):
//! host a listen server with `hl2dm -port 27016 +maxplayers 4 +map
//! hl2dm:dm_lockdown`, join it with `hl2dm +connect <ip>:27016`, or a
//! dedicated server (apps/hl2dm_server, `hl2dm_server`).

use bevy::prelude::*;
use chimera_client::client::Args;
use chimera_present::{MapPlugin, PresentationPlugins};
use chimera_world::greybox;
use hl2dm_app::ClientPlugin;

fn main() {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        println!(
            "hl2dm: Half-Life 2: Deathmatch on Chimera (M1: its maps, a stand-in movement, its bullet and melee weapons;\n\
             --map hl2dm:<name>, e.g. hl2dm:dm_lockdown, else the greybox).\n\
             The options are the shell's (below, named after the CS:S app; all apply here).\n\
             Network play (a listen server, or the dedicated hl2dm_server):\n\
             \x20 host:    hl2dm -port 27016 +maxplayers 4 +map hl2dm:dm_lockdown\n\
             \x20 join:    hl2dm +connect 127.0.0.1:27016\n"
        );
    }
    let mut args = Args::parse();
    let mut app = App::new();
    match args.game_map() {
        Some(id) => match hl2dm_app::load_map(id, 0) {
            Ok(data) => {
                eprintln!(
                    "loaded {}: {} triangles, {} textures, {} spawns, {} warnings",
                    data.name,
                    data.triangle_count(),
                    data.textures.len(),
                    data.spawns.len(),
                    data.warnings.len()
                );
                for w in &data.warnings {
                    eprintln!("  warning: {w}");
                }
                app.add_plugins(MapPlugin::new(data));
            }
            Err(e) => {
                eprintln!("error: --map {id}: {e}");
                std::process::exit(1);
            }
        },
        None => {
            // No menus yet: straight into the greybox.
            args.map.get_or_insert_with(|| "chimera:greybox".into());
            app.add_plugins((greybox::GreyboxMapPlugin, MapPlugin::empty()));
        }
    }
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "Half-Life 2: Deathmatch".into(),
                    resolution: match (args.window, args.scale) {
                        (Some(s), scale) => bevy::window::WindowResolution::new(s.x, s.y)
                            .with_scale_factor_override(scale.unwrap_or(1.0)),
                        (None, Some(scale)) => bevy::window::WindowResolution::default().with_scale_factor_override(scale),
                        (None, None) => default(),
                    },
                    resizable: args.window.is_none(),
                    ..default()
                }),
                ..default()
            })
            // Warnings and errors also show in the in-game console.
            .set(bevy::log::LogPlugin {
                custom_layer: chimera_client::client::console::log_layer,
                ..default()
            }),
        // Avian as Chimera runs it (rendering eases between ticks with
        // `map::interp`).
        chimera_world::physics_plugins(),
        // The simulation at HL2:DM's tick (the app's tests run it
        // headless).
        hl2dm_app::SimPlugin,
    ))
    // What the map's drawing needs.
    .add_plugins(PresentationPlugins)
    // This build's version (`version`, bug reports; build.rs).
    .insert_resource(chimera_core::core::BuildVersion(hl2dm_app::VERSION))
    // Network play: a listen server, or a client (`connect`).
    .add_plugins(chimera_net::net::NetPlugin)
    // The client shell.
    .add_plugins(ClientPlugin { args });
    // The executors, `CHIMERA_SYSTEM_DUMP`, then the game, as every app
    // (`client::app::run`).
    chimera_client::client::app::run(app);
}
