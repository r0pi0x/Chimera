//! `halo_ce`'s integration tests, as one test binary (each file a module):
//! what the game tells the shell (`info`), the skeleton on the headless
//! harness (`skeleton`: `harness::HaloSim` and the greybox), the map
//! converter's pure pieces (`maps`), the movement (`movement`; over the
//! network: `movement_net`), a player's vitality through the damage seam
//! (`vitality`, its tables `vitality_data`, across games `cross_game`,
//! over the network `net_vitality`), and, in `heavy`, the user's disc
//! image (its mount, every multiplayer map converted, a player standing on
//! Hang 'Em High, the vitality and damage tables against the disc) (found
//! through `$CHIMERA_HALO_CE_ROOT` or
//! `[games.halo_ce] path`, each test passing with a note without one).
//! `cargo test -p halo_ce --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod cross_game;
mod info;
mod maps;
mod movement;
mod movement_net;
mod net_vitality;
mod skeleton;
mod vitality;
mod vitality_data;

// Slow ones (the user's disc image): the fast tier skips `heavy::`.
mod heavy {
    mod maps;
    mod mount;
    mod vitality_data;
    mod walk;
}
