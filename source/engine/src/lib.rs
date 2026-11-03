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
//! its map cycle; `weapon_script`, a plain-text weapon script's keys;
//! `view_anim` and `view_motion`, what Source view models play and how
//! they move with the player (lift S8: each game's rules over Source's
//! activities, CS:S's cvar-driven bob and history sway, HL2's fixed bob
//! and spring sway); `player_anim`, the shared half of Source's player
//! animation (lift S9: the inputs, the drawn and the server's per-tick
//! updates, the hands' IK locks; each game's state picks its sequences).

pub mod content;
pub mod hud;
pub mod maps;
pub mod player_anim;
pub mod source_movement;
pub mod search_path;
pub mod view_anim;
pub mod view_motion;
pub mod weapon_script;

// `crate::map`, `crate::core` and `crate::mount` inside, as in the game
// crates the content modules came from.
#[allow(unused_imports)]
use chimera_core::{console, core, mount, slots};
#[allow(unused_imports)]
use chimera_world::map;
#[allow(unused_imports)]
use chimera_sim::weapon;
