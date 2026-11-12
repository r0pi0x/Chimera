//! `hl2dm`'s integration tests, as one test binary (each file a module).
//! `cargo test -p hl2dm --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1. Scenario tests run on
//! `harness::Hl2Sim` (`skeleton`); heavy ones read the user's install
//! (the mount, the stock maps converted, the map cache's key), found
//! through Steam, `[games.hl2dm] path` or `$CHIMERA_HL2DM_ROOT`, each
//! passing with a note without one.
//! The gameinfo parser's fast tests are `source_engine`'s own.

mod animation;
mod blast_rules;
mod effects;
mod gravity_gun;
mod hitboxes;
mod info;
mod map_physics_rules;
mod melee;
mod movement;
mod net;
mod net_effects;
mod net_gravity_gun;
mod net_prediction;
mod net_projectiles;
mod physics_clips;
mod pickups;
mod projectile_fx;
mod projectiles;
mod ragdolls;
mod rpg_laser;
mod rules;
mod set_pieces;
mod skeleton;
mod spark_showers;
mod suit;
mod weapon_details;
mod weapons;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod effect_materials;
    mod game_menu_look;
    mod hud_look;
    mod sound_stock;
    mod map_cache;
    mod map_ladders;
    mod map_pickups;
    mod map_prop_walks;
    mod map_props;
    mod map_set_pieces;
    mod map_stock;
    mod mount;
    mod player_models;
    mod scripts;
    mod served;
    mod view_models;
}
