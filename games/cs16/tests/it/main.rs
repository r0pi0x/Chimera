//! `cs16`'s integration tests, as one test binary (each file a module):
//! what it tells the shell (`info`), a body animating (`bodies`),
//! the skeleton on the headless harness (`skeleton`: `harness::Cs16Sim`,
//! the greybox), and, in `heavy`, the user's install (`served`: CS 1.6
//! from the asset service alone, packed from it; `textures`: what texture
//! names' prefixes and chrome draw as; `entities`: its maps' logic, from
//! the entity spec's worked examples; `net_logic`: that logic on a
//! network client) (found through
//! `[games.cs16] path` or `$CHIMERA_CS16_ROOT`), each test passing
//! with a note without one. `cargo test -p cs16 --features dev --test it`;
//! where tests go: docs/OBSERVABILITY.md, section 1.

mod bodies;
mod info;
mod skeleton;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod bodies;
    mod entities;
    mod install;
    mod maps;
    mod net_logic;
    mod served;
    mod textures;
}
