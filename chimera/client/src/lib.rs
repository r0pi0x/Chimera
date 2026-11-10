//! The client shell (docs/ARCHITECTURE.md, "Crates"): everything that
//! needs a window, for any game: local input and binds, the local
//! player's camera, the console UI, chat, the fallback HUD and HUD text,
//! the scheme's fonts, the VGUI-like widgets and the drawing of a game's
//! VGUI layouts, spectating, picture and sound settings, captures, the
//! debug UI and overlays, the network glue, and the front end's interface
//! (`front_end`: the hooks through which the shell reaches a game's
//! menus). It sits on the engine's crates, the presentation
//! (`chimera_present`: its picture settings are render passes, its sound
//! settings the mixer's) and the network (`chimera_net`), and names no
//! game, the app or Source's logic: a game says what it needs as data
//! (`game_registry`, `core`'s vocabulary and hooks, `debug_ui::GameDebug`),
//! the map logic as `map::logic_view`, the app as `client::AppHooks`. A
//! game's front end builds on it (CS:S's: `cs_source::ui`, its `ui`
//! feature) and puts its HUD among the shell's plugins
//! (`client::ClientPlugins`). A game's client app is a `GameApp` static
//! run by `client::app::run` (the start map, the plugins' order, the
//! usage text from the game's `GameInfo`), as every app's is.
//!
//! `mashup::client` re-exports `client` at its old path.

pub mod client;

// `crate::console`, `crate::map`, ... inside `client`, as in the game crate.
use chimera_core::{character, console, core, game_registry, gameplay, metrics, slots};
use chimera_net::net;
use chimera_present::{audio, render};
use chimera_sim::{movement, weapon};
use chimera_world::map;
