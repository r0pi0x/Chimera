//! The mashup game binary (`mashup_demo`): a Counter-Strike: Source map
//! (de_dust2 without `--map`) played as Combat Arms: its soldier,
//! movement, weapons, HUD, bots and Team Death Match (`mashup_demo`'s
//! crate docs). The options are the shell's (`--help`); `--map
//! cs_source:<name>` or `combat_arms:<name>` picks the map,
//! `+MaxPlayer <n>` the bots.

use bevy::prelude::*;
use chimera_client::client::Args;
use chimera_core::gameplay::{Gameplay, GameplayAppExt};
use chimera_present::{MapPlugin, PresentationPlugins};
use mashup_demo::{ClientPlugin, MashupPlugins};

fn main() {
    if std::env::args().skip(1).any(|a| a == "--help" || a == "-h") {
        println!(
            "mashup_demo: Counter-Strike: Source's maps played as Combat Arms (its soldier, movement, M4A1,\n\
             knife and grenades, HUD, bots and Team Death Match). Starts on {} with {} bots;\n\
             --map cs_source:<name> (or combat_arms:<name>) and +MaxPlayer <n> change them.\n\
             The options are the shell's (below, named after the CS:S app; all apply here).\n",
            mashup_demo::DEFAULT_MAP,
            mashup_demo::DEFAULT_BOTS,
        );
    }
    let args = mashup_demo::with_defaults(Args::parse());
    let mut app = App::new();
    let id = args.game_map().unwrap_or(mashup_demo::DEFAULT_MAP).to_string();
    match mashup_demo::load_map(&id, 0) {
        Ok(data) => {
            eprintln!(
                "loaded {id}: {} triangles, {} textures, {} spawns, {} warnings",
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
    }
    app.add_plugins((
        // Both installs as `cs_source://` and `combat_arms://` assets
        // (before `AssetPlugin`).
        chimera_present::assets::InstallSourcesPlugin(vec![
            chimera_present::assets::InstallSource::of(&cs_source::game::INFO),
            chimera_present::assets::InstallSource::shared(combat_arms::game::INFO.id, combat_arms::game::world::mount),
        ]),
        // The window (none with `--offscreen`), the log into the console
        // too (`client::app::default_plugins`, shared by every client).
        chimera_client::client::app::default_plugins(&args, "Lucker Party: mashup demo"),
        // Avian as Chimera runs it (rendering eases between ticks with
        // `map::interp`).
        chimera_world::physics_plugins(),
        MashupPlugins,
    ))
    // What the map's drawing needs.
    .add_plugins(PresentationPlugins)
    // Combat Arms is the gameplay on every map: its rules, HUD, binds and
    // tick (CS:S's parts in the app are its maps, logic and guns only).
    .set_gameplay(Gameplay::of(&combat_arms::game::INFO))
    .insert_resource(chimera_core::core::BuildVersion(mashup_demo::VERSION))
    // Network play: a listen server, or a client (`connect`).
    .add_plugins(chimera_net::net::NetPlugin)
    // The client shell with Combat Arms' HUD, then its other looks.
    .add_plugins(ClientPlugin { args })
    .add_plugins(combat_arms::ui::CaUiPlugin);
    // The executors, `CHIMERA_SYSTEM_DUMP`, then the game, as every app.
    chimera_client::client::app::run(app);
}
