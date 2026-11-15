//! What TF2 lends other games' apps (`chimera_core::game_parts`): nothing
//! yet but its id. Its guns come with M2 (the Scattergun, Rocket Launcher,
//! Shotgun and Minigun as rungs), its classes' bodies and movement with M3
//! (docs/plans/active/tf2.md), so it is not `playable` in a gun game's
//! ladder. Its maps aren't claimed either: an automatic pool doesn't draw
//! them yet; a party plays one when asked by id (`tf2:ctf_2fort`).

use chimera_core::game_parts::GameParts;

/// TF2's parts.
pub static PARTS: GameParts = GameParts {
    game: super::GAME,
    guns: &[],
    knife: None,
    movement: None,
    characters: None,
    hud: None,
    maps: &[],
    announcer: None,
    match_won_sound: None,
    match_lost_sound: None,
    music: false,
    cues: None,
};
