//! `chimera_ui`'s integration tests, one binary (each file a module): the
//! kit on a bare Bevy app, headless. `cargo test -p chimera_ui --features
//! dev`; where tests go: docs/OBSERVABILITY.md, section 1.

mod controls;
mod fonts;
mod glyphs;
mod icons;
mod motion;
mod scroll;
mod shell;
mod style;
mod text_fit;
mod theme;
mod units;
