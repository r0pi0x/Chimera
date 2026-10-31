//! `lithtech_formats`' integration tests, as one test binary: the readers
//! on fixtures written in code from the specs (no game data), and in
//! `heavy` on the user's Combat Arms install (skipped without one; the
//! fast tier skips them with `-- --skip heavy::`). `cargo test -p
//! lithtech_formats`. A new file here gets a `mod` line below.

mod anim;
mod attributes;
mod dtx;
mod fixture;
mod ltb;
mod render_style;
mod rez;
mod world;

mod heavy {
    mod dtx_install;
    mod keyed_install;
    mod ltb_install;
    mod render_style_install;
    mod rez_install;
    mod world_install;
}
