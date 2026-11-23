//! What DoD:S lends other games' apps (`chimera_core::game_parts`): its
//! guns by their ladder class (`weapons`; the machine guns `Heavy`, fired
//! deployed while prone: `stance`; the launchers and rifle grenades
//! `Launcher`, the frags `Grenade`: `weapons::explosives`), its knife beside them (`weapons::KNIFE`; the spade
//! is the Axis classes', M3 picks by class), and its movement
//! (`movement::ID`), which makes it `playable` in a gun game's ladder: a
//! player holding a DoD:S gun is DoD:S's, wears its body and moves on its
//! movement (its stances). Its HUD is the shell's minimal one until it has
//! its own (M3).
//!
//! No map is claimed (`PARTS.maps` is empty): its maps enter an app's
//! automatic pool from the install or the asset store the machine has,
//! once a pool's rules list them (none yet: M3's fitness sweep); any map
//! plays when asked by id (`dods:dod_avalanche`).

use chimera_core::game_parts::{GameParts, GunClass, GunPart};

use super::weapons as w;

/// DoD:S's parts.
pub static PARTS: GameParts = GameParts {
    game: super::GAME,
    guns: &[
        GunPart { id: w::GARAND, class: GunClass::Rifle },
        GunPart { id: w::K98, class: GunClass::Marksman },
        GunPart { id: w::SPRING, class: GunClass::Sniper },
        GunPart { id: w::K98_SCOPED, class: GunClass::Sniper },
        GunPart { id: w::THOMPSON, class: GunClass::Smg },
        GunPart { id: w::MP40, class: GunClass::Smg },
        GunPart { id: w::BAR, class: GunClass::Rifle },
        GunPart { id: w::MP44, class: GunClass::Rifle },
        GunPart { id: w::COLT, class: GunClass::Pistol },
        GunPart { id: w::P38, class: GunClass::Pistol },
        GunPart { id: w::C96, class: GunClass::Pistol },
        GunPart { id: w::M1CARBINE, class: GunClass::Carbine },
        GunPart { id: w::CAL30, class: GunClass::Heavy },
        GunPart { id: w::MG42, class: GunClass::Heavy },
        GunPart { id: w::BAZOOKA, class: GunClass::Launcher },
        GunPart { id: w::PSCHRECK, class: GunClass::Launcher },
        GunPart { id: w::RIFLEGREN_US, class: GunClass::Launcher },
        GunPart { id: w::RIFLEGREN_GER, class: GunClass::Launcher },
        GunPart { id: w::FRAG_US, class: GunClass::Grenade },
        GunPart { id: w::FRAG_GER, class: GunClass::Grenade },
    ],
    knife: Some(w::KNIFE),
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
