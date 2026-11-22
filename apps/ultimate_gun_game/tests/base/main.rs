//! `ultimate_gun_game`'s tests of the default party's gun game (Combat Arms'
//! and CS:S's guns, no guest game named): the automatic gun game on the
//! party's simulation, headless (`combat_arms::harness::CaSim` with
//! `PartyPartsPlugin` and `UltimateGunGamePlugin`). A change to a guest game
//! does not select this binary (checks/src/affected.rs, "tests by game").
//! `cargo test -p ultimate_gun_game --features dev --test base`.

// The helpers (tests/common/): the ladder's party and what reads it.
#[path = "../common/parts.rs"]
mod parts;

mod chat;
mod fallbacks;
mod match_end_layout;
mod minimal_hud;
mod names_fit;
mod net;
mod net_ui_sounds;
mod no_damage;
mod offered_weapons;
mod power_curve;
mod sandbox_entry;
mod ui_sounds;
mod variety;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod death_frames;
    mod first_view;
    mod ladder_build;
    mod map_lights;
    mod map_ticks;
    mod nav_in_play;
    mod report_frame;
    mod rung_frames;
}
