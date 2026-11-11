//! Network play (docs/ARCHITECTURE.md, "Crates";
//! docs/plans/active/multiplayer.md): the transport, the join handshake,
//! replication of what every game has (characters, bodies, props, items,
//! weapons' events, chat, cvars, maps), prediction and interpolation
//! (`net`). A game, and Source's map logic, register what they replicate
//! of their own through `net::NetAppExt::net_part` (CS:S's round, bomb
//! and hostages; Source's movers and HUD messages) and fill the hooks the
//! network calls (`net::parts`: how a player joins, bots, what the rules
//! hold a predicted player to). It uses the engine's base
//! (`chimera_core`), the simulation's map (`chimera_world`) and frameworks
//! (`chimera_sim`) and nothing above them, so an edit in a game or the
//! client doesn't rebuild it.
//!
//! The game crate re-exports `net` at its old path (`mashup::net`), with
//! the game's replicated types beside it.
//!
//! `harness`: `NetSim`, a server and clients in one process over the
//! in-memory link, each a headless `Sim` on the caller's base plugins.

pub mod dedicated;
pub mod harness;
pub mod minigame_net;
pub mod net;
pub mod prematch;

// `crate::core`, `crate::map` and the rest inside `net`, as in the game
// crate.
use chimera_core::{character, console, core, game_log, slots};
use chimera_sim::{minigame, mode, movement, weapon};
use chimera_world::map;
