//! `ultimate_gun_game`'s integration tests, as one test binary (each file
//! a module): the automatic gun game on the party's simulation, headless
//! (`combat_arms::harness::CaSim` with `PartyPartsPlugin` and
//! `UltimateGunGamePlugin`). `cargo test -p ultimate_gun_game --features
//! dev --test it`; where tests go: docs/OBSERVABILITY.md, section 1.

mod fallbacks;
mod hl2_alt_ammo;
mod hl2_set_pieces;
mod heavy {
    mod hl2_map_items;
    mod map_swap_frames;
    mod map_swap_render;
    mod map_ticks;
    mod nav_in_play;
    mod no_fetch_in_match;
    mod pool_maps;
    mod rung_frames;
    mod session_maps;
    mod spawn_views;
    mod server_map_memory;
    mod session_materials;
}
mod minimal_hud;
mod no_damage;
mod no_stand_ins;
mod offered_weapons;
mod net;
mod parts;
mod pickups;
mod power_curve;
mod session;
mod ui_sounds;
mod variety;
