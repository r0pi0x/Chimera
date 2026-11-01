//! Source engine behaviour every Source game shares, converted into
//! Chimera's types (docs/ARCHITECTURE.md, "Crates";
//! docs/plans/active/hl2dm.md, section 3, lift S0): what CS:S and
//! Half-Life 2: Deathmatch both need moves here from `games/cs_source`,
//! generic, with each game supplying its data and hooks ("move first,
//! generalize second"). It uses the engine's crates and Source's formats,
//! never the presentation (`chimera_present`), `mashup`, a game or an app
//! (checked: `workspace_crates`); what it draws will sit behind a `ui`
//! feature, as a game's front end does.
//!
//! So far: `search_path`, a Source game's search path as a mount (lift
//! S1): read from the game's `gameinfo.txt`, or given as the game's own
//! list (CS:S's, kept as it was); `maps`, the maps an install holds and
//! its map cycle.

pub mod content;
pub mod maps;
pub mod search_path;

// `crate::map`, `crate::core` and `crate::mount` inside, as in the game
// crates the content modules came from.
#[allow(unused_imports)]
use chimera_core::{core, mount};
#[allow(unused_imports)]
use chimera_world::map;
