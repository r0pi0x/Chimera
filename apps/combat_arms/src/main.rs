//! The Combat Arms game binary (`combat_arms`): Combat Arms' simulation
//! (`combat_arms::SimPlugins` + `CombatArmsPlugins`) on the greybox, the
//! presentation, the network and the client shell with its HUD. Wave 0:
//! walk and shoot the placeholder weapons. The options are the shell's,
//! as the CS:S app's (`--help`); `--map combat_arms:<name>` loads a map
//! from the install (`combat_arms::game::world`), else the greybox.
//!
//! Network play is the CS:S app's (the shell's `connect`, `-port`,
//! `maxplayers`, `map`): host a listen server with `combat_arms -port
//! 27016 +maxplayers 4 +map greybox`, join it with `combat_arms +connect
//! <ip>:27016` (Combat Arms' part of it: `combat_arms::game::net`), or a
//! dedicated server (apps/combat_arms_server, `combat_arms_server`).

use bevy::prelude::*;
use chimera_client::client::Args;
use chimera_present::{MapPlugin, PresentationPlugins};
use chimera_world::greybox;
use combat_arms_app::ClientPlugin;

fn main() {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        println!(
            "combat_arms: Combat Arms on Chimera (its movement, the M4A1 and the M9 on the greybox,\n\
             or a map of the install: --map combat_arms:<name>, e.g. combat_arms:warehouse).\n\
             The options are the shell's (below, named after the CS:S app; all apply here).\n\
             Network play (a listen server, or the dedicated combat_arms_server):\n\
             \x20 host:    combat_arms -port 27016 +maxplayers 4 +map greybox\n\
             \x20 join:    combat_arms +connect 127.0.0.1:27016\n"
        );
    }
    let mut args = Args::parse();
    let mut app = App::new();
    match args.game_map() {
        Some(id) => match combat_arms_app::load_map(id, 0) {
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
            // The install as `combat_arms://` assets (before `AssetPlugin`).
            chimera_present::assets::InstallSourcesPlugin(vec![chimera_present::assets::InstallSource::shared(combat_arms::game::INFO.id, combat_arms::game::world::mount)]),
            // The window (none with `--offscreen`), the log into the console
            // too (`client::app::default_plugins`, shared by every client).
            chimera_client::client::app::default_plugins(&args, "Combat Arms"),
            // Avian as Chimera runs it (rendering eases between ticks with
            // `map::interp`).
            chimera_world::physics_plugins(),
            // The simulation at Combat Arms' tick (the app's tests run it
            // headless).
            combat_arms_app::SimPlugin,
        ))
        // What the map's drawing needs.
        .add_plugins(PresentationPlugins)
        // This build's version (`version`, bug reports; build.rs).
        .insert_resource(chimera_core::core::BuildVersion(combat_arms_app::VERSION))
        // Network play: a listen server, or a client (`connect`).
        .add_plugins(chimera_net::net::NetPlugin)
        // The client shell with Combat Arms' HUD, then its other looks.
        .add_plugins(ClientPlugin { args })
        .add_plugins(combat_arms::ui::CaUiPlugin);
    // The player's profile (options): loaded at start, saved at exit.
    combat_arms::game::options::use_user_profile(&mut app, None);
    // The executors, `CHIMERA_SYSTEM_DUMP`, then the game, as every app
    // (`client::app::run`).
    chimera_client::client::app::run(app);
}
