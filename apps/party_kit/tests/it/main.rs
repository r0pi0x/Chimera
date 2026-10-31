//! `party_kit`'s (Lucker Party's) integration tests, as one test binary (each file a
//! module): minigames on the party's simulation, headless
//! (`combat_arms::harness::CaSim` with `PartyPartsPlugin`); `heavy::`, the
//! user's installs. `cargo test -p party_kit --features dev --test it`;
//! where tests go: docs/OBSERVABILITY.md, section 1.

mod announcer;
mod archetypes;
mod bodies_drawn;
mod controls;
mod cross_game;
mod cross_glass;
mod cross_water_ladders;
mod cs_parts;
mod death_view;
mod drawn_ladder;
mod esc_menu;
mod footstep_levels;
mod game_hooks;
mod grenade_looks;
mod gun_game;
mod hud;
mod identity;
mod identity_sounds;
mod kill_feed;
mod kill_icons;
mod last_shot;
mod listen_server;
mod main_menu;
mod net_gun_game;
mod party_music;
mod persist;
mod protection_look;
mod net_party;
mod results_hud;
mod rotation;
mod rung_characters;
mod rung_extras;
mod startup;
mod swap_effect;
mod switches;

mod heavy {
    mod bot_cost;
    mod cs_decals;
    mod cross_hits;
    mod css_map_moves;
    mod impacts;
    mod nav_bounds;
}
