//! `chimera_movement`'s integration tests, as one test binary (each file a
//! module): scenario tests of the shared movement's own seams on the
//! headless harness (`chimera_world::harness::Sim`) with the engine's base
//! alone (`SimBasePlugins`), the greybox and the reference config; nothing
//! of a game or a loader (docs/plans/active/shared-movement.md, section 7).
//! `course`: a small course of brushes high over the greybox, and the same
//! solids as clip hulls (`course::BrushHulls`, a test's `ClipHulls`);
//! `hulls`: the world traced through clip hulls.
//! `cargo test -p chimera_movement --features dev --test it`; where tests
//! go: docs/OBSERVABILITY.md, section 1.

mod course;
mod hulls;
