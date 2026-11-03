//! CS:S's impact sounds and decals: Source's
//! (`source_engine::effects::impacts`, lift S5 of
//! docs/plans/active/hl2dm.md, re-exported here at its old path) with
//! CS:S's style (`super::impact_effects::STYLE`) and its surfaces. A
//! bullet hit on a player sounds once, the server's choice (`pain.rs`),
//! so the impact leaves it out; CS:S's hostages keep theirs there
//! (`HitSoundHere`).

pub use source_engine::effects::impacts::*;

use bevy::prelude::*;
use source_engine::effects::impacts as engine;

use crate::map::{
    impact::{GameSurfaces, SurfaceClasses},
};
use crate::objectives::hostages::Hostage;

/// CS:S's surfaces by class (`map::impact`).
pub const SURFACES: GameSurfaces = engine::surfaces(super::GAME);

/// `name`, a surface of the loaded map's game, as CS:S names it: itself
/// on CS:S's maps (and maps that don't say), else by its class
/// (`map::impact::SurfaceClasses`: a Combat Arms "Cement" wall is CS:S's
/// `concrete`).
pub fn as_cs(name: String, classes: Option<&SurfaceClasses>, map_game: Option<&str>) -> String {
    engine::as_game(name, classes, map_game, super::GAME)
}

/// Bullets' and knives' impact sounds and decals alone (what an app with
/// CS:S's guns but not its game adds, Lucker Party). Registers CS:S as an
/// impact provider.
pub struct BulletImpactsPlugin;

impl Plugin for BulletImpactsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(engine::BulletImpactsPlugin(super::impact_effects::STYLE));
        add_hostage_sounds(app);
    }
}

/// Bullets' impacts with the physics objects' impact sounds and scrapes.
pub struct ImpactSoundsPlugin;

impl Plugin for ImpactSoundsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(engine::ImpactSoundsPlugin(super::impact_effects::STYLE));
        add_hostage_sounds(app);
    }
}

/// A hostage's bullet hit sounds as a flesh impact (`HitSoundHere`).
fn add_hostage_sounds(app: &mut App) {
    if !app.world().contains_resource::<HostageSounds>() {
        app.insert_resource(HostageSounds).add_observer(
            |added: On<Add<Hostage>>, mut commands: Commands| {
                commands.entity(added.entity).insert(HitSoundHere);
            },
        );
    }
}

#[derive(Resource)]
struct HostageSounds;
