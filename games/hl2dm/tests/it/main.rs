//! `hl2dm`'s integration tests, as one test binary (each file a module).
//! `cargo test -p hl2dm --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1. Scenario tests run on
//! `harness::Hl2Sim` (`skeleton`); heavy ones read the user's install
//! (the mount, the stock maps converted, the map cache's key), found
//! through Steam, `[games.hl2dm] path` or `$CHIMERA_HL2DM_ROOT`, each
//! passing with a note without one.
//! The gameinfo parser's fast tests are `source_engine`'s own.

mod info;
mod movement;
mod net_prediction;
mod skeleton;
mod suit;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod map_cache;
    mod map_ladders;
    mod map_stock;
    mod mount;
}
