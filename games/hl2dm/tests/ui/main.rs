//! `hl2dm`'s front end's tests (feature `ui`), as one test binary (each
//! file a module): the HUD's state machine and its draw list on a stand-in
//! look carrying the spec's numbers (specs/hl2dm/hud.md "Test cases": no
//! game needed), the install's look is checked in tests/it/heavy/hud_look.rs.
//! `cargo test -p hl2dm --features dev,ui --test ui`.

mod common;
mod damage_and_death;
mod feed;
mod history;
mod menus;
mod motd;
mod panels;
mod quick_info;
mod selection;
