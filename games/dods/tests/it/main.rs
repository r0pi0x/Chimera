//! `dods`'s integration tests, as one test binary (each file a module):
//! what the game tells the shell and its spawn classes (`info`), the
//! skeleton on the headless harness (`skeleton`: `harness::DodsSim`, the
//! greybox, a DoD:S player standing at 220 u/s with its eyes at 58 and,
//! ducked, 34; its envelope), its weapons (`weapons`: fire delays in
//! ticks, the sights' block, hit zones, the slow, melee from behind),
//! and, in `heavy`, the user's install (its
//! mount, the nine stock maps converted with their spawns, the class
//! bodies), found through Steam, `[games.dods] path` or
//! `$CHIMERA_DODS_ROOT`, each test passing with a note without one.
//! `cargo test -p dods --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod info;
mod skeleton;
mod weapons;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod install;
    mod scripts;
}
