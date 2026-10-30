//! `chimera_sim`'s integration tests, as one test binary (each file a
//! module): scenario tests of the engine's simulation frameworks on the
//! headless harness (`chimera_world::harness::Sim`) with the engine's
//! base alone (`SimBasePlugins`) and the greybox; nothing of a game.
//! `cargo test -p chimera_sim --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod equipment;
mod map_lifetime;
mod minigame;
mod mode;
mod nav_bounds;
mod movement;
mod noclip_damage;
mod rung_extras;
mod spawns;
mod view_anim;
mod weapon_frame;
