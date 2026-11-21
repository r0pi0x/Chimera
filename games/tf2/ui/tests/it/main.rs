//! `tf2_ui`'s integration tests, as one test binary (each file a module):
//! what of TF2 only the client side shows. `interp`: a needle drawn
//! between its ticks by the client's interpolation; in `heavy`, the
//! user's install: the kill feed's icons cut from its sheet (`kill_icons`)
//! and TF2's particle systems run in the presentation (`particles`), each
//! test passing with a note without one. The simulation's tests are
//! `tf2`'s (games/tf2/tests/it/).
//! `cargo test -p tf2_ui --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod interp;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod install;
    mod kill_icons;
    mod particles;
}
