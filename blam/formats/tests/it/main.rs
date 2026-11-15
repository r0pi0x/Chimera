//! `blam_formats`' integration tests, as one test binary: the readers on
//! fixtures written in code from the specs (no game data), and in `heavy`
//! on the user's Halo disc image (skipped without one; the fast tier skips
//! them with `-- --skip heavy::`). `cargo test -p blam_formats`. A new
//! file here gets a `mod` line below.

mod disc;
mod fixture;

mod heavy {
    mod disc_install;
}
