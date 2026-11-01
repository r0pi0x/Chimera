//! CS:S's sounds: Source's sound scripts (`source_engine::content::sound`,
//! lift S5 of docs/plans/active/hl2dm.md, re-exported here at their old
//! paths) with the entries CS:S itself plays (`load`: its weapons, radio,
//! fire, pain, hints, the announcer's round sounds, the HUD's weapon
//! selection, `+use` on nothing, chat and sprays) and its round sounds
//! (`round_sounds`).

pub use source_engine::content::sound::*;

use super::{material::MaterialLoader, surfaceprops::SurfaceProps};
use crate::map::{MapEntity, MapSounds};

/// The entries CS:S plays whatever the map (beyond the surfaces' and the
/// map's own).
const ALWAYS: &[&str] = &[
    "Event.TERWin",
    "Event.CTWin",
    "Event.RoundDraw",
    "radio.moveout",
    "radio.letsgo",
    "radio.locknload",
    // +use on nothing, a chat line (buying and night vision: the shop's
    // data, `weapons::sounds`), a spray.
    "Player.UseDeny",
    super::radio::CHAT_SOUND,
    super::weapons::SPRAY_SOUND,
    // The HUD's weapon selection.
    WEAPON_MOVE,
    WEAPON_PICK,
    WEAPON_DENY,
];

/// The HUD weapon selection's sounds (the install's game_sounds.txt; all
/// `common/wpn_hudoff.wav`, the move and deny at half volume). When CS:S
/// plays each is inferred from their names (docs/tech-debt.md).
const WEAPON_MOVE: &str = "Player.WeaponSelectionMoveSlot";
const WEAPON_PICK: &str = "Player.WeaponSelected";
const WEAPON_DENY: &str = "Player.DenyWeaponSelection";

/// The announcer's round sounds (game_sounds_radio.txt entries).
pub fn round_sounds() -> crate::map::RoundSounds {
    crate::map::RoundSounds {
        attackers_win: Some("Event.TERWin".into()),
        defenders_win: Some("Event.CTWin".into()),
        draw: Some("Event.RoundDraw".into()),
        start: ["radio.moveout", "radio.letsgo", "radio.locknload"].map(String::from).to_vec(),
        // The install defines it twice (common/use_deny.wav, then
        // common/wpn_select.wav at 0.4); the later definition wins here.
        // Which CS:S plays is an open question (doors_buttons.md Q2).
        use_deny: Some("Player.UseDeny".into()),
        weapon_move: Some(WEAPON_MOVE.into()),
        weapon_pick: Some(WEAPON_PICK.into()),
        weapon_deny: Some(WEAPON_DENY.into()),
    }
}

/// The map's sounds (`source_engine`'s `load`) with the entries CS:S
/// plays: `ALWAYS`, its weapons', radio's, fire's, pain's and the
/// automatic hints'.
pub fn load(materials: &mut MaterialLoader, map: &str, surfaces: &SurfaceProps, entities: &[MapEntity]) -> MapSounds {
    let weapons = super::weapons::sounds();
    let game = ALWAYS
        .iter()
        .chain(weapons.iter())
        .copied()
        .chain(super::radio::sound_entries())
        .chain(super::fire::SOUNDS.iter().copied())
        .chain(super::pain::SOUNDS.iter().copied())
        .map(String::from)
        // The automatic hints' (their data's: `rules::hints`).
        .chain(crate::rules::hints::sound_entry().map(String::from));
    source_engine::content::sound::load(materials, map, surfaces, entities, game)
}
