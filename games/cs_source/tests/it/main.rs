//! `cs_source`'s integration tests of its simulation, as one test binary
//! (each file a module, so the crate links once for all of them:
//! docs/performance.md, "Test cycle"): CS:S's loaders and spec test cases
//! on the user's install, and the scenario tests on the engine's headless
//! harness with CS:S's simulation as the base (`harness::Sim`, `NetSim`
//! on `CsSim`: `cs_source::SimPlugins`), the greybox and real maps, with
//! nothing above the game (no app, client shell or front end). `cargo test
//! -p cs_source --features dev --test it`, one file's with a name filter
//! (`weapons::`); the front end's are tests/ui/ (feature `ui`). Where
//! tests go: docs/OBSERVABILITY.md, section 1.
//!
//! `stack` (tests/common/stack.rs) re-exports what the scenario tests run
//! on at the CS:S app's paths (`crate::stack::{core, map, harness, ...}`).
//! A new file here (or in `heavy/`) gets a `mod` line below;
//! `architecture::tests_are_one_binary_per_crate` fails for a file without
//! one.
//!
//! `heavy` holds the slow ones (real maps from the install, long bot
//! simulations): the fast tier skips them with `-- --skip heavy::`.

#[path = "../common/mod.rs"]
mod common;
#[path = "../common/stack.rs"]
mod stack;
#[allow(unused_imports)]
use common::{load_map, load_map_lean, load_map_level, load_map_level_lean};

mod animation;
mod auto_switch;
mod bot_buying;
mod bot_grenades;
mod bot_manners;
mod bomb_leave;
mod bot_sight;
mod cs_grenades;
mod cs_guns;
mod demo_playback;
mod demo_resim;
mod drawn_parts;
mod economy_data;
mod footstep_control;
mod game_log;
mod hints;
mod hit_reaction;
mod impact_effects;
mod interpolation;
mod logic_physics;
mod map_game_entities;
mod map_logic;
mod map_sound_fx;
mod mount_cs_source;
mod mount_install;
mod movement;
mod nav;
mod net;
mod net_bots;
mod net_breakables;
mod net_graph;
mod net_interp;
mod net_maps;
mod net_prediction;
mod net_upnp;
mod net_votes;
mod net_weapons;
mod noclip_damage;
mod pain_sounds;
mod player_lods;
mod phy_spec;
mod prediction;
mod prop_motion;
mod radio;
mod refused_hits;
mod ragdoll;
mod ragdoll_spec;
mod rope_spec;
mod rounds;
mod schedules;
mod source_movement;
mod source_movement_css;
mod source_movement_ladder_water;
mod source_movement_tricks;
mod spawn_protection;
mod sprays;
mod teams;
mod view_model_spec;
mod view_models;
mod wav_decode;
mod weapon_data;
mod weapons;

/// Slow tests: the fast tier (`-- --skip heavy::`) leaves them out.
mod heavy {
    mod bot_nav;
    mod bot_rounds;
    mod demo_playback;
    mod demo_resim;
    mod map_ambient;
    mod map_areaportals;
    mod map_breakables;
    mod map_brush_decals;
    mod map_brush_entities;
    mod map_cache;
    mod map_community;
    mod map_courses;
    mod map_de_dust2;
    mod map_debris_settle;
    mod map_de_nuke;
    mod map_de_nuke_doors;
    mod map_de_port;
    mod map_fire;
    mod map_flows;
    mod map_hdr;
    mod map_ladders;
    mod map_loose;
    mod map_material_extras;
    mod map_militia_walls;
    mod map_net_changelevel;
    mod map_net_weapons;
    mod map_objectives;
    mod map_occluders;
    mod map_physics_shadow;
    mod map_prop_damage;
    mod map_prop_settle;
    mod map_props;
    mod map_sound_stock;
    mod map_spawn_dump;
    mod map_stock;
    mod map_surf;
    mod map_surfaces;
    mod map_vis;
    mod map_water_props;
    mod map_visuals;
    mod net_big_map;
    mod net_capacity;
    mod net_soak;
    mod served;
    mod weapon_names;
}
