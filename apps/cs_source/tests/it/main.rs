//! `cs_source_app`'s integration tests, as one test binary: what only the
//! app has (its build's version, `core::VERSION`). The scenario tests run
//! below it, on the engine's harness with the game's base (cs_source's
//! tests/it/ and tests/ui/, `harness::CsSim`), so a change to the app or
//! CS:S's front end doesn't run them. A test goes in the lowest crate
//! whose public API it needs (docs/OBSERVABILITY.md, section 1): here
//! only for the app's own glue (its `SimPlugins`, `client_hooks`, map
//! loading by id, its build stamp). `cargo test -p cs_source_app
//! --features dev --test it`.

mod console_commands;

// Slow ones (a GPU, the game binary built): the fast tier skips `heavy::`.
mod heavy {
    mod offscreen;
}
