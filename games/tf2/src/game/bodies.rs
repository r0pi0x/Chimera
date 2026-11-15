//! TF2's player bodies (specs/tf2/player_animation.md 1, 7, 8; classes.md
//! "The class table"): each class's player model (`models/player/<class>
//! .mdl`, the Demoman's `demo.mdl`) as a Source character body of this
//! game (`props::load_character` with `GAME`), animated by its include
//! models' sequences (`<class>_animations.mdl`: Source's loader puts the
//! include's real sequences over the model's same-named 1-frame stubs,
//! install.md 4.3), with its hitbox set (the head group 1 on `bip_head`)
//! and its ragdoll. The bodies read are those of the classes whose guns
//! the kit wants (`add_bodies`), so a map carries a body for each class a
//! TF2 rung can make.
//!
//! Which body a player wears follows the class of the TF2 gun in hand
//! (`wear_bodies`, on every machine from the inventory it sees): the
//! class's model by name (`map::BodyName`), the default class's (the
//! Soldier's) the one model without a name, so a TF2 player whose gun
//! names no class still has a body; and the team's skin (`map::BodySkin`:
//! 0 RED, 1 BLU; a free-for-all player by its team id, odd RED, even BLU).
//! The weapon's `c_` model is held bone-merged on `weapon_bone` (the held
//! models of `maps::add_guns`, keyed by the gun: `player_anim::
//! hold_weapons`).
//!
//! Known shortcuts: body groups stay at their first choice (the Soldier's
//! `rocket` group is never shown in his hand during a reload, 5's
//! event); no death animations or gibs (9).

use bevy::prelude::*;
use source_engine::content::{material::MaterialLoader, props, surfaceprops::SurfaceProps};

use super::{
    GAME,
    classes::{Classes, DEFAULT},
};
use crate::{
    core::Team,
    map::{BodyModel, BodyName, BodySkin, CharacterModels, GameKit, KitLoad, MapCharacterModel},
    slots::GameIdentity,
    weapon::{Inventory, Weapon},
};

/// Model `path` as a TF2 body: named by its path unless it is the default
/// class's (worn by a TF2 player without a class body of its own).
pub fn load(materials: &mut MaterialLoader, surfaces: &SurfaceProps, path: &str, default: &str) -> Result<MapCharacterModel, String> {
    let mut m = props::load_character(GAME, materials, surfaces, path, None)?;
    if !path.eq_ignore_ascii_case(default) {
        m.name = Some(path.to_string());
    }
    Ok(m)
}

/// The player models of the classes whose guns `wants` (each once, the
/// default class's always), in class order.
pub fn wanted_models(table: &Classes, wants: &dyn Fn(&str) -> bool) -> Vec<String> {
    let guns = super::weapons::tf2_weapons();
    let mut out: Vec<String> = Vec::new();
    for c in &table.classes {
        let used = c.name == DEFAULT
            || guns
                .weapons
                .iter()
                .any(|w| wants(&w.id) && table.of_gun(&w.id).is_some_and(|g| g.name == c.name));
        if used && !out.contains(&c.model) {
            out.push(c.model.clone());
        }
    }
    out
}

/// The bodies `kit` asks for (the classes of the guns it wants) into
/// `into`.
pub fn add_bodies(
    materials: &mut MaterialLoader,
    surfaces: &SurfaceProps,
    kit: KitLoad,
    into: &mut GameKit,
    warnings: &mut Vec<String>,
) {
    if !kit.bodies() {
        return;
    }
    let table = Classes::default();
    let default = table.get(DEFAULT).map(|c| c.model.clone()).unwrap_or_default();
    let paths = wanted_models(&table, &|g| kit.wants(g));
    debug!("TF2 bodies: {} ({kit:?})", paths.join(", "));
    for path in paths {
        match load(materials, surfaces, &path, &default) {
            Ok(m) => into.characters.push(m),
            Err(e) => warnings.push(e),
        }
    }
}

/// The skin of a body on `team` (0 RED, 1 BLU; classes.md, install.md
/// 4.5): our team 1 is RED, 2 BLU; past the two sides (a free-for-all's
/// own ids) odd RED, even BLU.
pub fn skin_of(team: Option<Team>) -> i32 {
    match team.map(|t| t.0) {
        Some(2) => 1,
        Some(t) if t > 2 && t % 2 == 0 => 1,
        _ => 0,
    }
}

/// A body whose skin TF2 set (`wear_bodies`), taken off with it.
#[derive(Component, Clone, Copy, Debug)]
pub struct Tf2Skin;

/// Players whose identity is TF2's wear their gun's class's body by name
/// and their team's skin; leaving TF2, the names and skins it set go.
#[allow(clippy::type_complexity)]
pub fn wear_bodies(
    players: Query<(
        Entity,
        Option<&GameIdentity>,
        Option<&Inventory>,
        Option<&Team>,
        Option<&BodyName>,
        Option<&BodySkin>,
        Option<&BodyModel>,
        Has<Tf2Skin>,
    )>,
    weapons: Query<&Weapon>,
    models: Option<Res<CharacterModels>>,
    table: Option<Res<Classes>>,
    mut commands: Commands,
) {
    let Some(table) = table else { return };
    let default = table.get(DEFAULT).map(|c| c.model.as_str()).unwrap_or_default();
    let ours = |name: &str| table.classes.iter().any(|c| c.model.eq_ignore_ascii_case(name));
    for (e, identity, inv, team, name, skin, body, skinned) in &players {
        let tf2 = identity.is_some_and(|i| i.0 == GAME);
        if !tf2 {
            let mut c = commands.entity(e);
            if name.is_some_and(|n| ours(&n.0)) {
                c.remove::<BodyName>();
            }
            if skinned {
                c.remove::<(BodySkin, Tf2Skin)>();
            }
            continue;
        }
        let held = inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
        if let Some(class) = held.and_then(|w| table.of_gun(&w.id)) {
            let want = (!class.model.eq_ignore_ascii_case(default)).then_some(class.model.as_str());
            match (want, name) {
                (Some(w), Some(n)) if n.0 == w => {}
                (Some(w), _) => {
                    commands.entity(e).insert(BodyName(w.to_string()));
                }
                (None, Some(n)) if ours(&n.0) => {
                    commands.entity(e).remove::<BodyName>();
                }
                _ => {}
            }
        }
        // The team's skin on a TF2 body (one with several skins).
        let on_tf2_body = body
            .and_then(|b| models.as_ref()?.0.get(b.0))
            .is_some_and(|m| m.game == Some(GAME) && !m.model.skins.is_empty());
        if on_tf2_body {
            let want = BodySkin(skin_of(team.copied()));
            if skin != Some(&want) || !skinned {
                commands.entity(e).insert((want, Tf2Skin));
            }
        }
    }
}

/// TF2's bodies worn.
pub struct Tf2BodiesPlugin;

impl Plugin for Tf2BodiesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Classes>().add_systems(Update, wear_bodies);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each M2 gun's class's model, and the Soldier's always (the
    /// default); a kit wanting no TF2 gun carries the Soldier alone.
    #[test]
    fn the_bodies_are_the_wanted_guns_classes() {
        let t = Classes::default();
        let all = wanted_models(&t, &|g| g.starts_with("tf2:"));
        for m in ["models/player/scout.mdl", "models/player/soldier.mdl", "models/player/heavy.mdl", "models/player/engineer.mdl"] {
            assert!(all.iter().any(|x| x == m), "{m} in {all:?}");
        }
        assert_eq!(wanted_models(&t, &|_| false), ["models/player/soldier.mdl"]);
        let scout = wanted_models(&t, &|g| g == super::super::weapons::SCATTERGUN);
        assert_eq!(scout, ["models/player/scout.mdl", "models/player/soldier.mdl"]);
    }

    #[test]
    fn red_and_blu_skins() {
        assert_eq!(skin_of(Some(Team(1))), 0);
        assert_eq!(skin_of(Some(Team(2))), 1);
        assert_eq!(skin_of(None), 0);
        assert_eq!(skin_of(Some(Team(3))), 0);
        assert_eq!(skin_of(Some(Team(4))), 1);
    }
}
