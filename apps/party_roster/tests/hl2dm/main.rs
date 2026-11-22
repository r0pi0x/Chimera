//! `party_roster`'s tests of HL2:DM in the party, one test binary: its
//! guns, bodies, HUD and Controls tab on the party's simulation, and the
//! mixed ladder (`common/mixed.rs`). A change to HL2:DM selects this binary
//! and the cross-game one (`it`), not the other games' (checks/src/affected.rs,
//! "tests by game"). `cargo test -p party_roster --features dev --test hl2dm`.

// Shared helpers (tests/common/): each binary includes the ones it uses.
#[path = "../common/aim.rs"]
mod aim;
#[path = "../common/bodies.rs"]
mod bodies;
#[path = "../common/huds.rs"]
mod huds;
#[path = "../common/local.rs"]
mod local;
#[path = "../common/mixed.rs"]
mod mixed;
#[path = "../common/net_play.rs"]
mod net_play;
#[path = "../common/play.rs"]
mod play;

mod bodies_drawn;
mod clips_across_games;
mod controls;
mod hl2dm_parts;
mod hud;
mod identity;
mod map_files;
mod net_hl2dm;
mod rung_characters;
mod shipped_map_pool;
mod voice_lines;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod bots_on_rungs;
    mod clips_on_maps;
    mod death_bodies;
    mod guest_cache;
    mod guest_effects;
    mod guest_shells;
    mod head_matrix;
    mod hud_looks;
    mod icon_glyphs;
    mod ladders_across_maps;
}
