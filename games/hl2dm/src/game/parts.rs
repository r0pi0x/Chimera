//! What Half-Life 2: Deathmatch lends a game made of several
//! (`chimera_core::game_parts`; docs/plans/active/hl2dm.md, M4): its guns
//! by class (the pistol warms up, the .357 is a heavy pistol, the SMG1
//! and the shotgun rise, the AR2 is a rifle) and its knife (the crowbar;
//! a Combine rung's stunstick, `rungs`), its movement (`movement::ID`),
//! its stock maps (the map cycle's, and any `dm_*` map the install holds).
//! Its HUD is its own id: a rung of its gun shows it, on any game's map
//! (`ui::party`). Its characters aren't claimed here (its bodies come with
//! the bundle's dressing, M8). The app registers it through the game's
//! bundle (`super::BUNDLE`).

use chimera_core::game_parts::{GameParts, GunClass, GunPart};

use super::weapons::{AR2, CROWBAR, MAGNUM, PISTOL, SHOTGUN, SMG1};

const fn gun(id: &'static str, class: GunClass) -> GunPart {
    GunPart { id, class }
}

/// HL2:DM's parts.
pub static PARTS: GameParts = GameParts {
    game: super::GAME,
    guns: &[
        gun(PISTOL, GunClass::Pistol),
        gun(MAGNUM, GunClass::HeavyPistol),
        gun(SMG1, GunClass::Smg),
        gun(SHOTGUN, GunClass::Shotgun),
        gun(AR2, GunClass::Rifle),
    ],
    knife: Some(CROWBAR),
    movement: Some(super::movement::ID),
    // Its player models come with M8; its HUD (M9) is this game's id.
    characters: None,
    hud: Some(super::GAME),
    maps: &["hl2dm:dm_lockdown", "hl2dm:dm_overwatch"],
    map_patterns: &["dm_*"],
    announcer: None,
    match_won_sound: None,
    match_lost_sound: None,
    music: false,
    cues: None,
};
