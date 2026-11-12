//! What Combat Arms lends a game made of several
//! (`chimera_core::game_parts`; docs/plans/active/ultimate-gun-game.md):
//! its guns by class and its knife (`weapons`, data/weapons/; the
//! others, its launchers and its install's weapons, by the `class` in
//! their data), its
//! movement (`movement::ID`), its soldiers (`characters`), its HUD
//! (`ui`, the composing app's), its maps, its announcer pack
//! (data/announcer.toml), its minigame cue pack (data/cues.toml), its
//! result screen's win and lose sounds for a party match's end, and its
//! music director (`music`, which plays a party match's moments:
//! `music::director`'s party cues). The app registers it
//! (`register_parts(&PARTS)`). The pack's weapon skins (kill_streaks.md
//! A8, B7) are the install's, added when its data is read
//! (`data::CaDataPlugin`: each weapon's `HUDEffectOverride`).
//!
//! Known shortcuts: without the install (or its key) the pack has no
//! weapon skins.

use chimera_core::game_parts::{GameParts, GunClass, GunPart};

const fn gun(id: &'static str, class: GunClass) -> GunPart {
    GunPart { id, class }
}

/// Combat Arms' parts.
pub static PARTS: GameParts = GameParts {
    game: super::GAME,
    guns: &[
        gun("combat_arms:beretta_m92fs", GunClass::Pistol),
        gun("combat_arms:mp5a4", GunClass::Smg),
        gun("combat_arms:remington_870", GunClass::Shotgun),
        gun("combat_arms:m4a1", GunClass::Rifle),
        gun("combat_arms:m60", GunClass::Heavy),
        gun("combat_arms:dragunov_svd", GunClass::Sniper),
        gun("combat_arms:l96a1", GunClass::Sniper),
        gun("combat_arms:m67", GunClass::Grenade),
    ],
    knife: Some("combat_arms:m9_knife"),
    movement: Some(super::movement::ID),
    characters: Some("combat_arms:soldier"),
    hud: Some(super::GAME),
    // The lobby's maps (data/lobby.toml; `key` is the install's world)
    // with spawns for a free-for-all, but HGOILRIG (our conversion has no
    // decks under its spawns: combat-arms.md). Not every installed world
    // (missions, training, the lobby's own): none by pattern.
    maps: &["combat_arms:warehouse", "combat_arms:junk_flea", "combat_arms:darkhills", "combat_arms:bloodmoney"],
    map_patterns: &[],
    announcer: Some(ANNOUNCER),
    // Its result screen's win and lose sounds (`ResultWinSound`,
    // `ResultLoseSound`, music_ambience.md 4, 7: the files inferred by
    // name, in UI_SND.rez).
    match_won_sound: Some(RESULT_WIN_SOUND),
    match_lost_sound: Some(RESULT_LOSE_SOUND),
    music: true,
    cues: Some(CUES),
};

/// `ResultWinSound`'s file (music_ambience.md 7, inferred).
pub const RESULT_WIN_SOUND: &str = "UI_SND/WIN.WAV";
/// `ResultLoseSound`'s file (inferred).
pub const RESULT_LOSE_SOUND: &str = "UI_SND/LOSE.WAV";

/// Combat Arms' minigame cue pack (data/cues.toml; `chimera_core::cues`):
/// Arms Race's interface sounds for a rung up and the last rung.
pub const CUES: &str = include_str!("../../data/cues.toml");

/// Combat Arms' announcer pack (specs/combat_arms/kill_streaks.md Part B;
/// `chimera_core::announcer`): its streak steps, special kills, banner
/// layout and timings, the files' paths in the install.
pub const ANNOUNCER: &str = include_str!("../../data/announcer.toml");
