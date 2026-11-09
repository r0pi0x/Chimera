//! Source's HUD, for every Source game's HUD (lift S7 of
//! docs/plans/active/hl2dm.md): `look` reads a game's HUD look from its
//! install (`scripts/hudlayout.res`, the client scheme and its font files,
//! the HUD texture lists) into `map::hud::GameHud`; `anim` runs the HUD
//! animation scripts (`scripts/hudanimations.txt`); `vgui` reads `.res`
//! layouts; `gameui` a game's GameUI look (the menus', drawn by
//! `source_gameui`) into `map::hud::GameUi`; `spray_import` turns a picture
//! the player picks (Options > Multiplayer > Import Spray) into a spray VTF.

pub mod anim;
pub mod gameui;
pub mod look;
pub mod spray_import;
pub mod vgui;
