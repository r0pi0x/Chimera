//! `halo_ce`'s integration tests, as one test binary (each file a module):
//! what the game tells the shell (`info`), the skeleton on the headless
//! harness (`skeleton`: `harness::HaloSim` and the greybox), the map
//! converter's pure pieces (`maps`), and, in `heavy`, the user's disc
//! image (its mount, every multiplayer map converted, a player standing on
//! Hang 'Em High) (found through `$CHIMERA_HALO_CE_ROOT` or
//! `[games.halo_ce] path`, each test passing with a note without one).
//! `cargo test -p halo_ce --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod info;
mod maps;
mod skeleton;

// Slow ones (the user's disc image): the fast tier skips `heavy::`.
mod heavy {
    mod maps;
    mod mount;
    mod walk;
}
