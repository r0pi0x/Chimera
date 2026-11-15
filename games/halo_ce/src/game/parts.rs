//! What Halo lends other games' apps (`chimera_core::game_parts`): nothing
//! yet. Its guns, movement, characters and HUD come with their specs
//! (weapons.md, movement.md, hud.md), so it is not `playable` in a gun
//! game's ladder.

use chimera_core::game_parts::GameParts;

/// Halo's parts.
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
