//! What TF2 lends other games' apps (`chimera_core::game_parts`): its
//! four M2 guns by class (the Scattergun and the Engineer's Shotgun
//! shotguns, the Rocket Launcher a launcher, the Minigun a heavy gun;
//! `weapons`), and its stand-in movement (`movement::ID`), which makes it
//! `playable` in a gun game's ladder: a rung of a TF2 gun makes its
//! holder TF2's. Its classes' bodies come with M3
//! (docs/plans/active/tf2.md), its HUD later: until then a rung of its gun
//! wears the party's character and the shell's HUD. Its maps aren't
//! claimed either: an automatic pool doesn't draw them yet; a party plays
//! one when asked by id (`tf2:ctf_2fort`).

use chimera_core::game_parts::{GameParts, GunClass, GunPart};

use super::weapons::{MINIGUN, ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN};

const fn gun(id: &'static str, class: GunClass) -> GunPart {
    GunPart { id, class }
}

/// TF2's parts.
pub static PARTS: GameParts = GameParts {
    game: super::GAME,
    guns: &[
        gun(SCATTERGUN, GunClass::Shotgun),
        gun(ROCKET_LAUNCHER, GunClass::Launcher),
        gun(SHOTGUN, GunClass::Shotgun),
        gun(MINIGUN, GunClass::Heavy),
    ],
    knife: None,
    movement: Some(super::movement::ID),
    characters: None,
    hud: None,
    maps: &[],
    announcer: None,
    match_won_sound: None,
    match_lost_sound: None,
    music: false,
    cues: None,
};
