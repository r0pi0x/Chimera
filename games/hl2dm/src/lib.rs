//! Half-Life 2: Deathmatch (Steam app 320) as a game crate on Chimera
//! (docs/plans/active/hl2dm.md): a third game library beside CS:S and
//! Combat Arms. M0 so far: what the game tells the shell about itself
//! (`game::INFO`) and its install as a mount (`game::mount`, from its
//! `gameinfo.txt`). Source behaviour it shares with CS:S comes from
//! `source_engine`, never from `cs_source` (a lift, not a copy).

pub mod game;

// `crate::game_registry` and `crate::mount` inside, as in the other game
// crates (the architecture check reads `crate::<module>` against its
// layering table).
use chimera_core::{game_registry, mount};
