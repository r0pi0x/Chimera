//! `blam_formats`' integration tests, as one test binary: the readers on
//! fixtures written in code from the specs (no game data), and in `heavy`
//! on the user's Halo disc image (skipped without one; the fast tier skips
//! them with `-- --skip heavy::`). `cargo test -p blam_formats`. A new
//! file here gets a `mod` line below.

mod bitmap;
mod bsp_fixture;
mod cache;
mod collision;
mod disc;
mod fixture;
mod map_fixture;
mod structure_bsp;
mod scenario;
mod shaders;

mod heavy {
    mod bitmap_install;
    mod bsp_install;
    mod cache_install;
    mod disc_install;
    mod maps;
    mod scenario_install;
    mod shader_install;
}
