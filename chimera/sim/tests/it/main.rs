//! `chimera_sim`'s integration tests, as one test binary (each file a
//! module): scenario tests of the engine's simulation frameworks on the
//! headless harness (`chimera_world::harness::Sim`) with the engine's
//! base alone (`SimBasePlugins`) and the greybox (or the range); nothing of a game.
//! `cargo test -p chimera_sim --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod armour_ratio;
mod equipment;
mod game_damage;
mod map_lifetime;
mod minigame;
mod mode;
mod nav_bounds;
mod nav_links;
mod movement;
mod noclip_damage;
mod offline;
mod range_map;
mod rung_extras;
mod rung_passives;
mod sim_waits;
mod spawn_score;
mod spawns;
mod transform_marks;
mod view_anim;
mod weapon_frame;
