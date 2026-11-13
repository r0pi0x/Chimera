//! `party_kit`'s (Lucker Party's) integration tests, as one test binary (each file a
//! module): minigames on the party's simulation, headless
//! (`combat_arms::harness::CaSim` with `PartyPartsPlugin`); `heavy::`, the
//! user's installs. `cargo test -p party_kit --features dev --test it`;
//! where tests go: docs/OBSERVABILITY.md, section 1.

mod announcer;
mod archetypes;
mod bodies_drawn;
mod console_scope;
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
mod hl2dm_parts;
mod hud;
mod identity;
mod identity_sounds;
mod kill_feed;
mod overlay_guard;
mod kill_icons;
mod ladders_across_games;
mod last_shot;
mod listen_server;
mod main_menu;
mod map_files;
mod map_pool_by_count;
mod map_vote;
mod net_gun_game;
mod party_music;
mod persist;
mod protection_look;
mod net_party;
mod results_hud;
mod rotation;
mod rung_characters;
mod rung_extras;
mod rung_holders;
mod shipped_map_pool;
mod startup;
mod swap_effect;
mod surprise;
mod switches;

mod heavy {
    mod bot_cost;
    mod bots_on_rungs;
    mod ca_mode_objects;
    mod cs_decals;
    mod cross_hits;
    mod css_map_moves;
    mod hud_looks;
    mod impacts;
    mod ladders_across_maps;
    mod nav_bounds;
    mod spawn_spots;
}
