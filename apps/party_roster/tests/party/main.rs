//! `party_roster`'s tests of the party as such, one test binary (each file a
//! module): the menus, the lobby, the host's settings, the network and the
//! gun game on the default party (Combat Arms the base, the original gun
//! game's CS:S and Combat Arms guns), headless on the party's simulation
//! (`combat_arms::harness::CaSim` with `PartyPartsPlugin`); `heavy::`, the
//! user's installs. No test here names HL2:DM, Halo or TF2: their changes
//! don't select this binary (checks/src/affected.rs, "tests by game").
//! `cargo test -p party_roster --features dev --test party`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

// Shared helpers (tests/common/): each binary includes the ones it uses.
#[path = "../common/aim.rs"]
mod aim;
#[path = "../common/bodies.rs"]
mod bodies;
#[path = "../common/huds.rs"]
mod huds;
#[path = "../common/lobby.rs"]
mod lobby;
#[path = "../common/local.rs"]
mod local;
#[path = "../common/net_play.rs"]
mod net_play;
#[path = "../common/play.rs"]
mod play;

mod account_page;
mod announcer;
mod archetypes;
mod arena_pool;
mod console_scope;
mod cross_game;
mod cross_glass;
mod cross_water_ladders;
mod cs_parts;
mod death_view;
mod domination_ui;
mod drawn_ladder;
mod esc_menu;
mod feedback;
mod footstep_levels;
mod game_hooks;
mod grenade_looks;
mod gun_game;
mod host_lobby;
mod identity_sounds;
mod join_flow;
mod kill_icons;
mod kit_conformance;
mod last_shot;
mod listen_server;
mod lobby_staging;
mod main_menu;
mod map_pool_by_count;
mod map_vote;
mod menu_backdrops;
mod net_gun_game;
mod net_party;
mod onboarding;
mod party_music;
mod protection_look;
mod range_bodies;
mod results_hud;
mod rotation;
mod rung_extras;
mod rung_holders;
mod settings;
mod signal_menu;
mod signal_pages;
mod slice_gen;
mod surprise;
mod swap_effect;
mod switches;
mod turrets;
#[cfg(feature = "present")]
mod window_icons;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod bot_cost;
    mod ca_mode_objects;
    mod cross_hits;
    mod cs_decals;
    mod css_map_moves;
    mod impacts;
    mod nav_bounds;
    mod range_kit;
    mod spawn_spots;
}
