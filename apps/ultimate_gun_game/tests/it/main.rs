//! `ultimate_gun_game`'s integration tests, as one test binary (each file
//! a module): the automatic gun game on the party's simulation, headless
//! (`combat_arms::harness::CaSim` with `PartyPartsPlugin` and
//! `UltimateGunGamePlugin`). `cargo test -p ultimate_gun_game --features
//! dev --test it`; where tests go: docs/OBSERVABILITY.md, section 1.

mod chat;
mod fallbacks;
mod hl2_alt_ammo;
mod hl2_set_pieces;
mod heavy {
    mod death_frames;
    mod ladder_build;
    mod hl2_map_items;
    mod map_lights;
    mod map_swap_frames;
    mod map_swap_render;
    mod map_ticks;
    mod nav_in_play;
    mod no_fetch_in_match;
    mod pool_maps;
    mod rung_frames;
    mod session_maps;
    mod spawn_views;
    mod server_map_change;
    mod server_map_memory;
    mod session_materials;
}
mod minimal_hud;
mod no_damage;
mod stand_ins;
mod offered_weapons;
mod net;
mod match_end_layout;
mod names_fit;
mod net_ui_sounds;
mod parts;
mod pickups;
mod power_curve;
mod props;
mod sandbox;
mod sandbox_gun_files;
mod sandbox_mechanics;
mod sandbox_platform;
mod sandbox_range;
mod session;
mod sandbox_screens;
mod sandbox_entry;
mod target_staging;
mod targets;
mod ui_sounds;
mod variety;
