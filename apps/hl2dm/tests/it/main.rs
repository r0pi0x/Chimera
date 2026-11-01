//! The HL2:DM app's integration tests: the simulation as the binary
//! composes it (`hl2dm_app::SimPlugin`) on the headless harness, with maps
//! loaded as `--map` loads them (`hl2dm_app::load_map_with`).

mod greybox;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod map_start;
}
