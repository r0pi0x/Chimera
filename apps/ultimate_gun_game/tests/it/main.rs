//! `ultimate_gun_game`'s integration tests that name several guest games
//! (the ladder's parts, gun rules, props, the sandbox's mechanics and
//! screens, targets, the heavy no-fetch and pool-map runs): any game's
//! change selects this binary; the others select by the game they name
//! (checks/src/affected.rs, "tests by game"). Each file a module, on the
//! party's simulation, headless. `cargo test -p ultimate_gun_game --features
//! dev --test it`; where tests go: docs/OBSERVABILITY.md, section 1.

mod gun_rules;
mod parts;
mod props;
mod sandbox_mechanics;
mod sandbox_screens;
mod targets;

// Slow ones (the user's installs): the fast tier skips `heavy::`.
mod heavy {
    mod no_fetch_in_match;
    mod pool_maps;
}
