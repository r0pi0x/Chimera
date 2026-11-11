//! `ultimate_gun_game`'s integration tests, as one test binary (each file
//! a module): the automatic gun game on the party's simulation, headless
//! (`combat_arms::harness::CaSim` with `PartyPartsPlugin` and
//! `UltimateGunGamePlugin`). `cargo test -p ultimate_gun_game --features
//! dev --test it`; where tests go: docs/OBSERVABILITY.md, section 1.

mod fallbacks;
mod heavy {
    mod map_swap_frames;
    mod map_swap_render;
    mod nav_in_play;
    mod rung_frames;
    mod session_maps;
    mod server_map_memory;
    mod session_materials;
}
mod minimal_hud;
mod no_stand_ins;
mod net;
mod parts;
mod pickups;
mod power_curve;
mod session;
mod variety;
