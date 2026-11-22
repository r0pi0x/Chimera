//! Counter-Strike: Source as a game crate (docs/ARCHITECTURE.md,
//! "Crates"): its simulation on the engine's crates (its front end, the
//! GameUI menus and dialogs, the join flow, the buy, team, radio and
//! spectator menus, the scoreboard and CS:S's HUD on the client shell, is
//! its own crate above this one, `cs_source_ui`, games/cs_source/ui). `game` is the
//! game's parts: its loaders (BSP, materials, models,
//! sounds, the install's search path), Source movement, the weapons and
//! grenades, impacts and effects, animation and `CsSourcePlugins`;
//! `rules` (deathmatch, team joining, rounds, the end of a game) and
//! `objectives` (bomb, hostages, zones) are CS:S's rules, which its rule
//! modules (`game::rules`) describe; `bot` is CS:S's bots (aim,
//! navigation, team plans, radio, buying, grenades, the bomb and
//! hostage goals); `game::net` is its part of the network (its round, bomb
//! and hostages replicated, its join flow, bots and holds behind the
//! network's hooks). It uses the engine crates
//! (`chimera_core`, `chimera_world`, `chimera_sim`, `chimera_net`),
//! Source's map logic (`source_logic`) and `source_formats`, never
//! `mashup` or another game, nor the presentation or the shell (checked),
//! so a dedicated server builds and runs the game without drawing.
//!
//! `SimPlugins`: everything CS:S's simulation needs but its parts (the
//! engine's base, Source's map logic, CS:S's rules, objectives, bots and
//! network part; `game::CsSourcePlugins` adds the parts after it), what
//! the app runs; `swap_map`: a map in place of the one playing, as the
//! game's `map` does without the view (the dedicated server's
//! `changelevel`, tests); `harness`: CS:S on the engine's headless
//! harness (`harness::CsSim`, the base of its scenario tests, tests/it/
//! and the front end's tests; maps for network tests).
//!
//! The game crate re-exports `game` as `mashup::games::cs_source` with
//! `ui` (`cs_source_ui`) beside it, and `rules`, `objectives` and `bot` at
//! their old paths (`mashup::rules`, ...).

pub mod bot;
pub mod game;
pub mod harness;
pub mod objectives;
pub mod rules;

// `crate::core`, `crate::map` and the rest inside, as in the game crate.
use chimera_core::{character, console, core, game_log, game_registry, gameplay, mount, slots};
use chimera_sim::weapon;
use chimera_net::net;
use chimera_world::map;

// Source's map logic: in `SimPlugins` (and the front end's objectives).
use source_logic::logic;

use bevy::prelude::*;

/// Everything CS:S's simulation needs but its parts, with no window,
/// rendering or input: the engine's base (`chimera_sim::SimBasePlugins`:
/// the fixed tick, `core`, the console, movement, the weapon framework),
/// CS:S's objectives, rules, bots and part of the network, Source's map
/// logic, the tick metrics. The app runs it (with its build's version:
/// `cs_source_app::SimPlugins`), with the game's parts
/// (`game::CsSourcePlugins`) after it; tests and tools on the headless
/// harness through `harness::CsSim`.
pub struct SimPlugins;

impl Plugin for SimPlugins {
    fn build(&self, app: &mut App) {
        app.add_plugins(chimera_sim::SimBasePlugins)
            .add_plugins(CsSourceParts)
            .add_plugins(chimera_core::metrics::TickMetricsPlugin);
    }
}

/// CS:S's simulation parts without the engine's base: its objectives,
/// rules, bots and part of the network (nothing without `NetPlugin`) and
/// Source's map logic. `SimPlugins` is the base, these, and the tick
/// metrics; an app with several games' parts adds the base once and each
/// game's parts. The guns alone (data and hooks, no rules) are
/// `CsSourceGuns`.
///
/// Its match-wide systems (rules, objectives, bots, the network part's
/// match state) run only while CS:S is the gameplay's rules
/// (`objectives::plays`); CS:S becomes the gameplay when the app has none
/// (`gameplay::GameplayAppExt::default_gameplay`).
pub struct CsSourceParts;

impl Plugin for CsSourceParts {
    fn build(&self, app: &mut App) {
        use gameplay::GameplayAppExt;
        app.default_gameplay(gameplay::Gameplay::of(&game::INFO));
        app.add_plugins((
            objectives::ObjectivesPlugin,
            rules::DeathmatchPlugin,
            bot::BotPlugin,
            // CS:S's part of the network (its round, bomb and hostages,
            // its join flow and bots' hooks); nothing without `NetPlugin`.
            game::net::CsSourceNetPlugin,
        ))
        .add_plugins(logic::LogicPlugin);
    }
}

/// CS:S's weapons alone: their data, hooks, accuracy and recoil, for an
/// app that offers its guns under another game's rules
/// (`game::weapons::CsGunsPlugin`).
pub use game::weapons::CsGunsPlugin as CsSourceGuns;

/// Register CS:S's `GameInfo` in an app that doesn't play it as a whole
/// (the harness): the network's map offer finds a map's file through it.
pub fn register_game(app: &mut App) {
    game_registry::GameAppExt::register_game(app, &game::INFO);
}

/// Put a map in place of the one playing, as the game's `map` does
/// without the view (the dedicated server's `changelevel`, tests): `data`
/// for map `id`, or the greybox (None); the tick its game runs at;
/// everyone respawned in a new game there. The fixed clock keeps running
/// (`core::set_tick_length`); a network server takes its clients along
/// (`net::maps`).
pub fn swap_map(world: &mut World, id: &str, data: Option<map::MapData>, tick: std::time::Duration) {
    use chimera_world::greybox;
    match data {
        Some(data) => {
            greybox::unload(world);
            map::change_map(world, data, map::MapDebugView::Normal);
            world.insert_resource(map::LoadedMapName(id.to_string()));
        }
        None => {
            map::unload_map(world);
            greybox::unload(world);
            greybox::respawn(world);
            world.remove_resource::<map::ActiveMapLook>();
            world.insert_resource(map::LoadedMapName(net::GREYBOX.into()));
        }
    }
    core::set_tick_length(world, tick);
    rules::new_game(world);
}
