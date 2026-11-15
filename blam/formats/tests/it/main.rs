//! `blam_formats`' integration tests, as one test binary: the readers on
//! fixtures written in code from the specs (no game data), and in `heavy`
//! on the user's Halo disc image (skipped without one; the fast tier skips
//! them with `-- --skip heavy::`). `cargo test -p blam_formats`. A new
//! file here gets a `mod` line below.

mod animation;
mod bitmap;
mod bsp_fixture;
mod cache;
mod collision;
mod disc;
mod effect;
mod fixture;
mod map_fixture;
mod model;
mod scenario;
mod shaders;
mod sound;
mod structure_bsp;
mod object;
mod weapon;
mod projectile;
mod damage_effect;
mod vitality;
mod globals;
mod game_type;

mod heavy {
    mod animation_install;
    mod bitmap_install;
    mod bsp_install;
    mod cache_install;
    mod disc_install;
    mod effect_install;
    mod maps;
    mod model_install;
    mod scenario_install;
    mod shader_install;
    mod sound_install;
    mod object_install;
    mod projectile_install;
    mod vitality_install;
    mod weapon_install;
}
