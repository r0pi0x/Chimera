//! What DoD:S lends other games' apps (`chimera_core::game_parts`): M0's
//! stand-in gun, the Colt (`weapons::COLT`, a pistol), and its movement
//! (`movement::ID`), which makes it `playable` in a gun game's ladder: a
//! player holding the Colt is DoD:S's, wears its body and moves on its
//! movement (its stances). Its HUD is the shell's minimal one until it has
//! its own (M3).
//!
//! No map is claimed (`PARTS.maps` is empty): its maps enter an app's
//! automatic pool from the install or the asset store the machine has,
//! once a pool's rules list them (none yet: M3's fitness sweep); any map
//! plays when asked by id (`dods:dod_avalanche`).

use chimera_core::game_parts::{GameParts, GunClass, GunPart};

/// DoD:S's parts.
pub static PARTS: GameParts = GameParts {
    game: super::GAME,
    guns: &[GunPart { id: super::weapons::COLT, class: GunClass::Pistol }],
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
