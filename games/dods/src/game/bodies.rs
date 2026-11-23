//! DoD:S's player bodies (specs/dods/classes.md, "Loadouts"; install.md,
//! "Models"): the twelve classes' player models
//! (`models/player/{american,german}_<role>.mdl`, MDL version 44; their
//! sequences come from the shared `dod_player_shared.mdl`, version 48,
//! through Source's include models) as Source character bodies of this
//! game (`props::load_character` with `GAME`), with their hitbox sets.
//!
//! Every body a map's kit asks for is loaded when the model is here
//! (`Mount::held`: an install, or the asset store with the `players` set),
//! the US rifleman's unnamed (the body a DoD:S player wears with no class,
//! `DEFAULT_MODEL`) and the others named by their path (`map::BodyName`).
//! A body is worn by name (`wear`: the sandbox's practice targets, one
//! per class); a player who leaves DoD:S has the name it set emptied
//! (`unwear`).
//!
//! Known shortcuts (M0):
//! - No class follows the gun in hand yet (M3: a DoD:S gun makes its
//!   holder the class that carries it); players wear the default body.
//! - No player animation: bodies stand in their bind pose (Source's
//!   shared animation driver per game, as TF2's `player_anim`, is M3's).
//! - Bodies have no team skin or helmet/hair body groups chosen
//!   (`helmetgroup`, `hairgroup` in the class scripts).

use bevy::prelude::*;
use source_engine::content::{material::MaterialLoader, props, surfaceprops::SurfaceProps};

use super::GAME;
use crate::{
    map::{BodyName, GameKit, KitLoad, MapCharacterModel},
    slots::GameIdentity,
};

/// One class's body: its id (`dods:us_rifleman`), display name and
/// player model.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClassBody {
    pub id: &'static str,
    pub name: &'static str,
    pub model: &'static str,
}

const fn body(id: &'static str, name: &'static str, model: &'static str) -> ClassBody {
    ClassBody { id, name, model }
}

/// The twelve classes' bodies, Allies then Axis, in classes.md's order.
pub const CLASSES: [ClassBody; 12] = [
    body("dods:us_rifleman", "US Rifleman", "models/player/american_rifleman.mdl"),
    body("dods:us_assault", "US Assault", "models/player/american_assault.mdl"),
    body("dods:us_support", "US Support", "models/player/american_support.mdl"),
    body("dods:us_sniper", "US Sniper", "models/player/american_sniper.mdl"),
    body("dods:us_mg", "US Machine Gunner", "models/player/american_mg.mdl"),
    body("dods:us_rocket", "US Rocket", "models/player/american_rocket.mdl"),
    body("dods:axis_rifleman", "Axis Rifleman", "models/player/german_rifleman.mdl"),
    body("dods:axis_assault", "Axis Assault", "models/player/german_assault.mdl"),
    body("dods:axis_support", "Axis Support", "models/player/german_support.mdl"),
    body("dods:axis_sniper", "Axis Sniper", "models/player/german_sniper.mdl"),
    body("dods:axis_mg", "Axis Machine Gunner", "models/player/german_mg.mdl"),
    body("dods:axis_rocket", "Axis Rocket", "models/player/german_rocket.mdl"),
];

/// The body a DoD:S player without a class wears (the US rifleman's).
pub const DEFAULT_MODEL: &str = "models/player/american_rifleman.mdl";

/// The shared animation model the bodies include.
pub const SHARED_ANIMATIONS: &str = "models/player/dod_player_shared.mdl";

/// The class body of id `id`.
pub fn class(id: &str) -> Option<&'static ClassBody> {
    CLASSES.iter().find(|c| c.id == id)
}

/// Model `path` as a DoD:S body: named by its path unless it is the
/// default (`DEFAULT_MODEL`).
pub fn load(materials: &mut MaterialLoader, surfaces: &SurfaceProps, path: &str) -> Result<MapCharacterModel, String> {
    let mut m = props::load_character(GAME, materials, surfaces, path, None)?;
    if !path.eq_ignore_ascii_case(DEFAULT_MODEL) {
        m.name = Some(path.to_string());
    }
    Ok(m)
}

/// The bodies `kit` asks for whose model is here (`Mount::held`) into
/// `into`: every class's when the kit wants any DoD:S gun or is the
/// game's own map's.
pub fn add_bodies(materials: &mut MaterialLoader, surfaces: &SurfaceProps, kit: KitLoad, into: &mut GameKit, warnings: &mut Vec<String>) {
    if !kit.bodies() || !super::weapons::GUNS.iter().any(|g| kit.wants(g)) {
        return;
    }
    for c in &CLASSES {
        if !materials.mount().held(c.model) {
            debug!("DoD:S body not here: {}", c.model);
            continue;
        }
        match load(materials, surfaces, c.model) {
            Ok(m) => into.characters.push(m),
            Err(e) => warnings.push(e),
        }
    }
}

/// Wear class body `id` (a practice target's `TargetCharacter::wear`):
/// its model by name (the default's name falls back to the unnamed
/// default body).
pub fn wear(world: &mut World, e: Entity, id: &str) {
    let Some(c) = class(id) else { return };
    if let Ok(mut ent) = world.get_entity_mut(e)
        && ent.get::<BodyName>().is_none_or(|n| n.0 != c.model)
    {
        ent.insert(BodyName(c.model.to_string()));
    }
}

/// A character that is no longer DoD:S's has the body name DoD:S set
/// emptied (`BodyName::none`, kept on: docs/DESIGN.md, "ECS style").
pub fn unwear(mut players: Query<(&mut BodyName, Option<&GameIdentity>), Changed<GameIdentity>>) {
    for (mut name, identity) in &mut players {
        if identity.is_some_and(|i| i.0 == GAME) {
            continue;
        }
        if CLASSES.iter().any(|c| c.model.eq_ignore_ascii_case(&name.0)) {
            *name = BodyName::none();
        }
    }
}

/// The class bodies as a sandbox's practice targets
/// (`chimera_sim::minigame::targets`): each may hold any DoD:S gun, worn
/// by name (`wear`), fresh at 100 health (classes.md, "Health and speed")
/// and no second bar.
pub fn add_targets(app: &mut App) {
    use chimera_sim::minigame::targets::{TargetCharacter, TargetsAppExt};
    for c in &CLASSES {
        app.target_character(TargetCharacter {
            id: c.id,
            name: c.name,
            game: GAME,
            holds: |_, _, gun| gun.starts_with("dods:"),
            wear,
            vitals: |_, _| (1.0, 0.0),
            second_bar: None,
        });
    }
}

/// DoD:S's bodies worn and taken off.
pub struct DodsBodiesPlugin;

impl Plugin for DodsBodiesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, unwear);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn twelve_classes_one_default() {
        assert_eq!(CLASSES.len(), 12);
        assert_eq!(CLASSES.iter().filter(|c| c.model == DEFAULT_MODEL).count(), 1);
        assert!(CLASSES.iter().all(|c| c.id.starts_with("dods:")));
        assert_eq!(class("dods:axis_mg").map(|c| c.model), Some("models/player/german_mg.mdl"));
    }
}
