//! `cs16`'s integration tests, as one test binary (each file a module):
//! what it tells the shell (`info`), a body animating (`bodies`), the
//! grenades' looks going off (`grenade_looks`),
//! the skeleton on the headless harness (`skeleton`: `harness::Cs16Sim`,
//! the greybox), its weapons' rules (`weapons`: specs/cs16/weapons.md's
//! and damage.md's test cases), its movement from the spec's test cases
//! and its golden run (`movement`), that movement predicted over `NetSim`
//! (`net_prediction`), its light styles (`light_styles`, server to client), a converted water leaf swum in (`water`), its footsteps (`footsteps`), its spent cases (`shells`), and, in `heavy`, the user's install (`movement`:
//! hull-traced walks, stairs and ladders; `guns`: every weapon's models
//! and sounds; `hud`: every weapon's HUD pictures and kill icon; `served`: CS 1.6
//! from the asset service alone, packed from it; `textures`: what texture
//! names' prefixes and chrome draw as; `entities`: its maps' logic, from
//! the entity spec's worked examples; `net_logic`: that logic on a
//! network client; `water`: every stock map's liquids as water, as_oilrig's
//! sea swum in) (found through
//! `~/games/cs16`, Steam, `[games.cs16] path` or `$CHIMERA_CS16_ROOT`:
//! `mount::install`), each test passing
//! with a note without one. `cargo test -p cs16 --features dev --test it`;
//! where tests go: docs/OBSERVABILITY.md, section 1.

mod bodies;
mod footsteps;
mod grenade_looks;
mod info;
mod light_styles;
mod movement;
mod net_prediction;
mod shells;
mod skeleton;
mod water;
mod weapons;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod bodies;
    mod drawn_light;
    mod entities;
    mod footsteps;
    mod guns;
    mod hud;
    mod install;
    mod maps;
    mod movement;
    mod net_logic;
    mod served;
    mod textures;
    mod water;
}
