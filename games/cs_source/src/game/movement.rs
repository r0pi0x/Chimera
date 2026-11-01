//! CS:S's movement (`cs_source:movement`): Source's player movement
//! (`source_engine::source_movement`, lift S6 of docs/plans/active/hl2dm.md,
//! re-exported here at its old paths, with its drowning, physics shadow
//! and push-away) with CS:S's hurt sounds (`super::pain`), and the
//! movement the characters CS:S spawns get (`match_movement`).

pub use source_engine::source_movement::*;

/// The movement's id.
pub const ID: &str = "cs_source:movement";

use bevy::prelude::*;

/// The movement a character the game spawns gets (bots, hostages): the
/// match's (`Loadout`) unless that is noclip, else CS:S's when registered,
/// else one some character uses that isn't noclip (headless tests with
/// another movement). Never the local player's current slot: a player
/// flying with noclip (or a `--movement chimera:noclip` run) gave every bot
/// and hostage noclip, so they hung in the air and flew through walls.
pub fn match_movement(world: &mut World) -> Option<&'static str> {
    use crate::slots::{Loadout, MovementRegistry, MovementSlot};
    let noclip = chimera_sim::movement::noclip::ID;
    if let Some(l) = world.get_resource::<Loadout>()
        && l.movement != noclip
    {
        return Some(l.movement);
    }
    if world.get_resource::<MovementRegistry>().is_some_and(|r| r.get(ID).is_some()) {
        return Some(ID);
    }
    world
        .query::<&MovementSlot>()
        .iter(world)
        .map(|m| m.0)
        .find(|m| *m != noclip)
}

/// Source's movement as CS:S's (`ID`): its config is the
/// `SourceMovementConfig` resource (CS:S's values, `Default`), its cvars
/// the bare Source names (CS:S's by the console's rule); with its pain
/// sounds (a hard landing's `Player.FallDamage`).
pub struct SourceMovementPlugin;

impl Plugin for SourceMovementPlugin {
    fn build(&self, app: &mut App) {
        app.source_movement(ID, None, None);
        if !app.is_plugin_added::<super::pain::PainSoundsPlugin>() {
            app.add_plugins(super::pain::PainSoundsPlugin);
        }
    }
}
