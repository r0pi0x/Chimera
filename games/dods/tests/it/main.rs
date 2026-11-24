//! `dods`'s integration tests, as one test binary (each file a module):
//! what the game tells the shell and its spawn classes (`info`), the
//! skeleton on the headless harness (`skeleton`: `harness::DodsSim`, the
//! greybox, a DoD:S player standing at 220 u/s with its eyes at 58 and,
//! ducked, 34; its envelope), its weapons (`weapons`: fire delays in
//! ticks, the sights' block, hit zones, the slow, melee from behind),
//! the bodies' animation (`player_anim`: the sequence families by stance,
//! movement, sights and deploy; the gestures; in `heavy`, the real model
//! and the held gun in the hands),
//! its prone, deployed machine guns and stamina (`stance`), its movement
//! (`movement`: the run, sprint and stamina, jump, duck, getting up, the
//! hit slow), its explosives (`explosives`: grenades, rifle grenades,
//! rockets, the blast), and, in
//! `heavy`, the user's install (its mount, the nine stock maps converted
//! with their spawns, the class bodies, its guns' sounds under its own
//! names: `sounds`; a rung's kit from the asset service alone: `served`), found through Steam, `[games.dods] path` or
//! `$CHIMERA_DODS_ROOT`, each test passing with a note without one.
//! `cargo test -p dods --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod explosives;
mod info;
mod movement;
mod player_anim;
mod skeleton;
mod stance;
mod weapons;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod install;
    mod scripts;
    mod sounds;
    mod player_anim;
    mod served;
    mod projectiles;
}
