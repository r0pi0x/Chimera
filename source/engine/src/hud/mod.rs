//! Source's HUD, for every Source game's HUD (lift S7 of
//! docs/plans/active/hl2dm.md): `look` reads a game's HUD look from its
//! install (`scripts/hudlayout.res`, the client scheme and its font files,
//! the HUD texture lists) into `map::hud::GameHud`; `anim` runs the HUD
//! animation scripts (`scripts/hudanimations.txt`).
//!
//! CS:S's menus' readers (`vgui`, `gameui`) stay in `cs_source` until a
//! second game draws such menus.

pub mod anim;
pub mod look;
