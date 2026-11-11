//! The faithful Counter-Strike: Source app's glue (apps/cs_source, crate
//! `cs_source_app`): what composes Chimera, Source's logic and CS:S into
//! the game (`APP`, a `client::app::GameApp` the `cs_source` binary,
//! `main.rs`, runs), the dedicated server
//! (apps/cs_source_server), the tools that drive the simulation, and its
//! own integration tests (tests/it/: what only the app has, its build's
//! version). `mashup`, the repository root, re-exports this crate at the
//! old `mashup::` paths. Module layering is documented in
//! docs/ARCHITECTURE.md and enforced by checks/tests/it/architecture.rs.
//!
//! `SimPlugins`: CS:S's simulation (`cs_source::SimPlugins`) with this
//! build's version. At their old paths: `harness` (the engine's headless
//! `Sim` and `NetSim` with CS:S's base and test maps), `greybox`
//! (chimera_world's), `swap_map` (cs_source's) and
//! `single_threaded_schedules` (chimera_world's harness: the game, the
//! dedicated server, `perfgate`).

// The client shell: window, input, camera, HUD; not in the dedicated
// server's build (the `present` feature, Cargo.toml).
#[cfg(feature = "present")]
pub mod client;
pub mod core;
pub mod games;
pub mod net;

/// The headless harness (docs/OBSERVABILITY.md, section 1) at its old
/// path: `Sim` (chimera_world), `NetSim` (chimera_net), CS:S's base
/// `CsSim` and its test maps (`cs_source::harness`).
pub mod harness {
    pub use chimera_net::harness::*;
    pub use chimera_world::harness::*;
    pub use cs_source::harness::*;
}

// The greybox (chimera_world), the map change (cs_source) and the
// executor setup (chimera_world's harness), at their old paths.
pub use chimera_world::{greybox, harness::single_threaded_schedules};
pub use cs_source::swap_map;

// The engine's stable base (chimera/core), at its old paths; `core` is
// a module of its own here (it adds the build's `VERSION`).
pub use chimera_core::{character, console, env, game_log, game_registry, gameplay, metrics, mount, slots};

/// The engine's map (chimera/world, crate `chimera_world`) at its old path,
/// with `MapPlugin` (chimera/present's `map_plugin`: map's registration
/// with `audio` and `render`) when built with the presentation; the
/// dedicated server adds `MapSimPlugin` (map's systems alone).
pub mod map {
    #[cfg(feature = "present")]
    pub use chimera_present::MapPlugin;
    pub use chimera_world::map::*;
}

// The engine's presentation (chimera/present), at its old paths; not in
// the dedicated server's build (the `present` feature, Cargo.toml).
#[cfg(feature = "present")]
pub use chimera_present::{audio, render};

// Source's map logic (source/logic: entity I/O, logic classes, triggers,
// movers), at its old path.
pub use source_logic::logic;

// The simulation's frameworks (chimera/sim: weapons, the movement
// slot's own implementations), at their old paths.
pub use chimera_sim::{movement, weapon};

// CS:S's rules (games/cs_source: deathmatch, rounds, the bomb and the
// hostages), at their old paths; `games::cs_source` re-exports the rest.
pub use cs_source::{bot, objectives, rules};

use bevy::prelude::*;

/// Default fixed tick rate until a ruleset or movement spec sets one
/// (`core::DEFAULT_TICK_HZ`).
pub use chimera_core::core::DEFAULT_TICK_HZ;

/// Everything the simulation needs, with no window, rendering or input:
/// CS:S's (`cs_source::SimPlugins`) with this build's version
/// (`core::BuildVersion`). The game and the dedicated server run it.
pub struct SimPlugins;

impl Plugin for SimPlugins {
    fn build(&self, app: &mut App) {
        app.insert_resource(core::BuildVersion(core::VERSION))
            .add_plugins(cs_source::SimPlugins);
    }
}

/// What this app tells the client shell (`client::AppHooks`): its commit,
/// how it loads maps (`games::load_map_level`, `load_map_file`,
/// `map_file_bytes`) and its greybox (`APP`'s).
#[cfg(feature = "present")]
pub fn client_hooks() -> client::AppHooks {
    APP.app_hooks()
}

/// The game (the `cs_source` binary, `main.rs`), on Chimera's client app
/// (`client::app`): CS:S's simulation (`SimPlugins`, `CsSourcePlugins`)
/// as the gameplay, on a map of the install at the console's HDR level,
/// with the debug views (`--debug-view`, `--lightmap-only`), or the
/// greybox (behind the main menu unless the run starts playing; at the
/// engine's tick, as another game's map); `--exec` and `--cvar` set its
/// movement (`client::movement_config`); the shell with CS:S's HUD
/// (`games::cs_source::ui::with_hud`), then its menus and panels
/// (`CsSourceUiPlugin`).
#[cfg(feature = "present")]
pub static APP: client::app::GameApp = client::app::GameApp {
    bin: "cs_source",
    info: &games::cs_source::INFO,
    title: "Counter-Strike Source",
    about: String::new,
    version: core::VERSION,
    build: core::BUILD,
    maps: client::app::AppMaps {
        load: games::load_map_with,
        load_file: games::load_map_file_with,
        file_bytes: games::map_file_bytes,
    },
    // The install as `cs_source://` assets.
    installs: || vec![chimera_present::assets::InstallSource::of(&games::cs_source::INFO)],
    sim: |app, args| {
        use gameplay::{Gameplay, GameplayAppExt};
        app.add_plugins((SimPlugins, games::cs_source::CsSourcePlugins))
            // CS:S is the gameplay: its rules, HUD, binds and tick.
            .set_gameplay(Gameplay::of(&games::cs_source::INFO));
        // The greybox (or another game's map) runs at the engine's tick, a
        // CS:S map at the gameplay's.
        if !args.game_map().is_some_and(|id| id.starts_with("cs_source:")) {
            app.insert_resource(Time::<Fixed>::from_hz(DEFAULT_TICK_HZ));
        }
        app.insert_resource(client::movement_config(args)).insert_resource(slots::Loadout {
            movement: games::cs_source::movement::ID,
        });
    },
    hud: games::cs_source::ui::with_hud,
    // CS:S's menus, panels and HUD (the shell reaches them through
    // `client::front_end`).
    front_end: |app, _| {
        app.add_plugins(games::cs_source::ui::CsSourceUiPlugin);
    },
    hooks: client::app::GameAppHooks {
        hdr_start: true,
        debug_views: true,
        server: Some("cs_source_server"),
        ..client::app::GameAppHooks::NONE
    },
};
