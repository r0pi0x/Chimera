//! `tf2`'s integration tests, as one test binary (each file a module):
//! what the game tells the shell (`info`), the skeleton on the headless
//! harness (`skeleton`: `harness::Tf2Sim` and the greybox), the spawn
//! rule on made-up entities (`spawns`), and, in `heavy`, the user's
//! install (its mount, ctf_2fort and cp_badlands converted with their
//! spawns, the converted-map cache's key, a player walking on ctf_2fort),
//! found through Steam, `[games.tf2] path` or `$CHIMERA_TF2_ROOT`, each
//! test passing with a note without one.
//! `cargo test -p tf2 --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod info;
mod skeleton;
mod spawns;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod map_cache;
    mod maps;
    mod mount;
    mod walk;
}
