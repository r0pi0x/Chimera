//! `hl2dm`'s integration tests, as one test binary (each file a module).
//! `cargo test -p hl2dm --features dev --test it`; where tests go:
//! docs/OBSERVABILITY.md, section 1. M0 has the install's mount only
//! (heavy: the user's install, found through Steam, `[games.hl2dm] path`
//! or `$CHIMERA_HL2DM_ROOT`; each test passes with a note without one).
//! The gameinfo parser's fast tests are `source_engine`'s own.

mod info;

// Slow ones (the user's install): the fast tier skips `heavy::`.
mod heavy {
    mod mount;
}
