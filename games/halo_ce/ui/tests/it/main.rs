//! `halo_ce_ui`'s integration tests, as one test binary (each file a
//! module): the HUD's rules on their own (`hud`), the match's text on it
//! (`hud_score`), the scopes' blur without the disc (`scope_blur`), the party apps' zoom on a client's camera
//! (`party_zoom`), the front end's other plain functions (`pure`: the
//! tracker's sweep at the frame's time, an effect light's fall-off); in
//! `heavy`, the HUD and the scopes' blur from the user's disc image (`hud`,
//! `scope_blur`), passing with a note
//! without one. The simulation's tests are `halo_ce`'s
//! (games/halo_ce/tests/it/).
//! `cargo test -p halo_ce_ui --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1.

mod hud;
mod hud_score;
mod party_zoom;
mod pure;
mod scope_blur;

// Slow ones (the user's disc image): the fast tier skips `heavy::`.
mod heavy {
    mod disc;
    mod hud;
    mod scope_blur;
}
