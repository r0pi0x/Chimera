//! `cs16`'s integration tests, as one test binary (each file a module):
//! the skeleton on the headless harness (`skeleton`: `harness::Cs16Sim`,
//! the greybox), and, in `heavy`, the user's install (found through
//! `[games.cs16] path` or `$CHIMERA_CS16_ROOT`), each test passing
//! with a note without one. `cargo test -p cs16 --features dev --test it`;
//! where tests go: docs/OBSERVABILITY.md, section 1.

mod skeleton;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod install;
}
