//! The game binary: `SimPlugins` + map + `NetPlugin` + `ClientPlugin`, run
//! by the shell's `client::app::run` (the per-frame schedules, main and
//! render world, on single-threaded executors, `CHIMERA_EXECUTOR=multi`:
//! Bevy's default, docs/performance.md; `CHIMERA_SYSTEM_DUMP`).

use bevy::prelude::*;
use cs_source_app::{
    SimPlugins, client, games,
    gameplay::{Gameplay, GameplayAppExt},
    greybox,
    map::{MapDebugView, MapPlugin},
    slots::Loadout,
};

fn main() {
    let args = client::Args::parse();
    let args_map_is_cs_source = args.game_map().is_some_and(|id| id.starts_with("cs_source:"));
    let mut app = App::new();
    match args.game_map() {
        // At the HDR level the console will start with (mat_hdr_level).
        Some(id) => match games::load_map_level(id, client::hdr::startup_level(&args.console)) {
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
                let view = match (args.debug_view.as_deref(), args.lightmap_only) {
                    (Some("lighting"), _) => MapDebugView::Lighting { scale: 0.25 },
                    (Some("albedo"), _) => MapDebugView::Albedo,
                    (Some(other), _) => {
                        eprintln!("error: --debug-view {other}: expected lighting or albedo");
                        std::process::exit(2);
                    }
                    (None, true) => MapDebugView::Lighting { scale: 1.0 },
                    (None, false) => MapDebugView::Normal,
                };
                app.add_plugins(MapPlugin {
                    view,
                    ..MapPlugin::new(data)
                });
            }
            Err(e) => {
                eprintln!("error: --map {id}: {e}");
                std::process::exit(1);
            }
        },
        None => {
            // The greybox (behind the main menu unless the run starts
            // playing), and map systems without a map, so `map <name>` can
            // load one.
            app.add_plugins((greybox::GreyboxMapPlugin, MapPlugin::empty()));
        }
    }
    app.add_plugins((
        // The install as `cs_source://` assets (before `AssetPlugin`).
        chimera_present::assets::InstallSourcesPlugin(vec![chimera_present::assets::InstallSource::of(
            &cs_source_app::games::cs_source::INFO,
        )]),
        // The window (none with `--offscreen`), the log into the console
        // too (`client::app::default_plugins`, shared by every client).
        client::app::default_plugins(&args, "Counter-Strike Source"),
        // Avian as Chimera runs it: rendering eases between ticks with
        // `map::interp` (characters' eyes and looks too).
        chimera_world::physics_plugins(),
        SimPlugins,
        cs_source_app::games::cs_source::CsSourcePlugins,
        chimera_present::PresentationPlugins,
    ))
    // CS:S is the gameplay: its rules, HUD, binds and tick.
    .set_gameplay(Gameplay::of(&cs_source_app::games::cs_source::INFO));
    // The greybox (or another game's map) runs at the engine's tick, a
    // CS:S map at the gameplay's.
    if !args_map_is_cs_source {
        app.insert_resource(Time::<Fixed>::from_hz(cs_source_app::DEFAULT_TICK_HZ));
    }
    app.insert_resource(movement_config(&args))
    .insert_resource(Loadout {
        movement: cs_source_app::games::cs_source::movement::ID,
    })
    // Network play: a listen server, or a client (`connect`).
    .add_plugins(cs_source_app::net::NetPlugin)
    // The client shell with CS:S's HUD in it.
    .add_plugins(client::ClientPlugin { args })
    // CS:S's menus, panels and HUD (the shell reaches them through
    // `client::front_end`).
    .add_plugins(cs_source_app::games::cs_source::ui::CsSourceUiPlugin);
    // The executors, `CHIMERA_SYSTEM_DUMP`, then the game
    // (`client::app::run`, shared with the other games' apps).
    client::app::run(app);
}

/// Source movement settings: CS:S's, then `--exec` files, then `--cvar`s.
fn movement_config(args: &client::Args) -> cs_source_app::games::cs_source::movement::SourceMovementConfig {
    let mut config = cs_source_app::games::cs_source::movement::SourceMovementConfig::default();
    for file in &args.exec {
        match std::fs::read_to_string(file) {
            Ok(text) => {
                for problem in config.exec(&text) {
                    eprintln!("warning: {}: {problem}", file.display());
                }
            }
            Err(e) => {
                eprintln!("error: --exec {}: {e}", file.display());
                std::process::exit(2);
            }
        }
    }
    for (name, value) in &args.cvars {
        if let Err(e) = config.set_cvar(name, value) {
            eprintln!("error: --cvar {name}={value}: {e}");
            std::process::exit(2);
        }
    }
    config
}
