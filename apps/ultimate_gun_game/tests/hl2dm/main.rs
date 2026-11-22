//! `ultimate_gun_game`'s tests that name HL2:DM (its guns, pickups, maps):
//! a change to HL2:DM selects this binary and the cross-game one (`it`)
//! (checks/src/affected.rs, "tests by game").
//! `cargo test -p ultimate_gun_game --features dev --test hl2dm`.

// The helpers (tests/common/): the ladder's party and what reads it.
#[path = "../common/parts.rs"]
mod parts;

mod hl2_alt_ammo;
mod hl2_set_pieces;
mod pickups;
mod session;
mod stand_ins;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod hl2_map_items;
    mod map_swap_frames;
    mod map_swap_render;
    mod server_map_change;
    mod server_map_memory;
    mod session_maps;
    mod session_materials;
    mod spawn_views;
}
