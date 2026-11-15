//! What TF2 lends other games' apps (`chimera_core::game_parts`): its
//! twelve guns by class (the Scattergun and the Engineer's Shotgun
//! shotguns, the Rocket Launcher, Grenade and Stickybomb Launchers
//! launchers, the Minigun a heavy gun, the Pistol a pistol, the SMG and
//! the Syringe Gun SMGs, the Sniper Rifle a sniper, the Revolver a heavy
//! pistol, the Flame Thrower a flamethrower;
//! `weapons`), and its movement (`movement::ID`: Source's with TF2's
//! classes), which makes it `playable` in a gun game's ladder: a rung of a
//! TF2 gun makes its holder TF2's and the gun's class (`classes`). Its maps aren't
//! claimed either: an automatic pool doesn't draw them yet; a party plays
//! one when asked by id (`tf2:ctf_2fort`).

use chimera_core::game_parts::{GameParts, GunClass, GunPart};

use super::weapons::{
    FLAMETHROWER, GRENADE_LAUNCHER, MINIGUN, PISTOL_SCOUT, REVOLVER, ROCKET_LAUNCHER, SCATTERGUN, SHOTGUN, SMG, SNIPER_RIFLE,
    STICKY_LAUNCHER, SYRINGE_GUN,
};

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
        gun(PISTOL_SCOUT, GunClass::Pistol),
        gun(SMG, GunClass::Smg),
        gun(SNIPER_RIFLE, GunClass::Sniper),
        gun(REVOLVER, GunClass::HeavyPistol),
        gun(GRENADE_LAUNCHER, GunClass::Launcher),
        gun(STICKY_LAUNCHER, GunClass::Launcher),
        gun(FLAMETHROWER, GunClass::Flamethrower),
        gun(SYRINGE_GUN, GunClass::Smg),
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
