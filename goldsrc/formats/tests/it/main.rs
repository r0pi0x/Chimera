//! `goldsrc_formats`' integration tests, as one test binary: the readers on
//! fixtures written in code from the specs (no game data), and in `heavy`
//! on the user's GoldSrc install (`$CHIMERA_CS16_ROOT`, else `[games.cs16]
//! path`; skipped without one; the fast tier skips them with `-- --skip
//! heavy::`). `cargo test -p goldsrc_formats`. A new file here gets a `mod`
//! line below.

mod fixtures;

mod heavy {
    mod bsp_install;
    mod collision_spike;
    mod install;
    mod mdl_install;
    mod mount_install;
    mod spr_install;
    mod wad_install;
}
