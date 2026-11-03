//! HL2:DM's player bodies (specs/hl2dm/player_models.md 1, 9-11): its 19
//! player models (`PLAYER_MODELS`: HL2's citizens, Combine soldiers and
//! metro police) as Source character bodies of this game
//! (`props::load_character` with `GAME`), animated through `hl2mp/`'s
//! include overrides (`models/player/male_anims.mdl`, `female_anims.mdl`:
//! the search path puts `hl2mp/` first and includes resolve whatever their
//! case and slashes, `mount::normalize`), each with its hitbox set, its
//! ragdoll, and on citizens eyeballs and teeth (`map::face`); the weapons'
//! world models held on them (`load_held`, bone-merged on the skeleton).
//!
//! Which body a player wears is its `Hl2Model` (the rules' choice, a
//! rung's), worn by name (`map::BodyName`), the DM default
//! (`inventory::DEFAULT_MODEL`) the one model without a name, so a player
//! with none (or whose model the map lacks) still has a body. `Hl2Model`
//! is replicated (`net`), so clients wear what the server chose.
//!
//! The dead look at their own ragdoll (`DEAD_VIEW`, HL2:DM's own app:
//! `Hl2DeadViewPlugin`). Their ragdolls follow HL2:DM's rules (`RAGDOLL`, `map::ragdoll::
//! RagdollGameRules`: spec 10).
//!
//! `add_bodies` puts them in a map's kit (HL2:DM's own maps, `maps`) and
//! `dress_guest` on another game's map (the party apps').
//!
//! Known shortcuts: every map load converts all 19 models (their animation
//! models read again for each); a ragdoll's push is the killing hit's
//! force, not the sum of the forces of the victim's last command (spec
//! 10.1: a shotgun kill pushes with the pellet that killed, not all
//! seven); the dead view's camera is pulled in by the shell's sphere
//! (0.2 m), not the spec's box of half-size 6.

use bevy::prelude::*;
use source_engine::content::{material::MaterialLoader, props, surfaceprops::SurfaceProps};

use super::{
    GAME,
    weapons::inventory::{DEFAULT_MODEL, Hl2Model},
};
use crate::map::{BodyName, GameKit, KitLoad, MapCharacterModel};

/// The 19 player models (specs/hl2dm/source_deltas.md, "Player models";
/// player_models.md 1.1): the Combine's four, then the fifteen citizens.
pub const PLAYER_MODELS: [&str; 19] = [
    "models/combine_soldier.mdl",
    "models/combine_soldier_prisonguard.mdl",
    "models/combine_super_soldier.mdl",
    "models/police.mdl",
    "models/humans/group03/female_01.mdl",
    "models/humans/group03/female_02.mdl",
    "models/humans/group03/female_03.mdl",
    "models/humans/group03/female_04.mdl",
    "models/humans/group03/female_06.mdl",
    "models/humans/group03/female_07.mdl",
    "models/humans/group03/male_01.mdl",
    "models/humans/group03/male_02.mdl",
    "models/humans/group03/male_03.mdl",
    "models/humans/group03/male_04.mdl",
    "models/humans/group03/male_05.mdl",
    "models/humans/group03/male_06.mdl",
    "models/humans/group03/male_07.mdl",
    "models/humans/group03/male_08.mdl",
    "models/humans/group03/male_09.mdl",
];

/// Model `path` as an HL2:DM body: named by its path unless it is the
/// default (`DEFAULT_MODEL`, worn by anyone without a model of its own).
pub fn load(materials: &mut MaterialLoader, surfaces: &SurfaceProps, path: &str) -> Result<MapCharacterModel, String> {
    let mut m = props::load_character(GAME, materials, surfaces, path, None)?;
    if path != DEFAULT_MODEL {
        m.name = Some(path.to_string());
    }
    Ok(m)
}

/// Model `path` read from `mount` alone (tests, tools): its textures
/// read and dropped.
pub fn load_one(mount: &crate::mount::Mount, path: &str) -> Result<MapCharacterModel, String> {
    let mut materials = MaterialLoader::guest(mount, 0, 0).lean(true);
    let surfaces = SurfaceProps::load(&mut materials);
    load(&mut materials, &surfaces, path)
}

/// The bodies `kit` asks for (all 19) into `into`.
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
    for path in PLAYER_MODELS {
        match load(materials, surfaces, path) {
            Ok(m) => into.characters.push(m),
            Err(e) => warnings.push(e),
        }
    }
}

/// The held models (world models) of the weapons `kit` wants, on the
/// first HL2:DM body's skeleton (bone merge by name: every HL2 player
/// skeleton has the `ValveBiped` hand bones), into `into`.
pub fn add_held(
    materials: &mut MaterialLoader,
    surfaces: &SurfaceProps,
    kit: KitLoad,
    into: &mut GameKit,
    warnings: &mut Vec<String>,
) {
    let skeleton = match into.characters.iter().find(|c| c.game == Some(GAME)) {
        Some(c) => Some(c.bones.clone()),
        None if kit.any_models() => load(materials, surfaces, DEFAULT_MODEL).ok().map(|c| c.bones),
        None => None,
    };
    let Some(skeleton) = skeleton else { return };
    for (weapon, path) in super::weapons::world_models() {
        if !kit.wants(&weapon) {
            continue;
        }
        match props::load_held(materials, &path, &weapon, &skeleton) {
            Ok(h) => into.held.push(h),
            Err(e) => warnings.push(e),
        }
    }
    // The projectiles, drawn by key (`weapons::launch::MODELS`).
    for (key, path, weapon) in super::weapons::launch::MODELS {
        if !kit.wants(weapon) {
            continue;
        }
        match props::load_held(materials, path, key, &skeleton) {
            Ok(h) => into.held.push(h),
            Err(e) => warnings.push(e),
        }
    }
}

/// HL2:DM's ragdolls (specs/hl2dm/player_models.md 10): the killing
/// force (the damage's own: bullets' impulse, melee damage x 300) into
/// body 0, the pelvis, whatever was hit; no death pose; the ragdoll stays
/// on the map after its owner respawns, until the owner dies again.
pub const RAGDOLL: crate::map::ragdoll::RagdollRules = crate::map::ragdoll::RagdollRules {
    death_pose: false,
    push_root: true,
    damage_force: true,
    outlive_respawn: true,
};

/// The dead player's view (specs/hl2dm/rules.md "The dead view"): its
/// own ragdoll from 96 units behind, 14 above its root, until it lives
/// again; no death cam, freeze cam or watching others.
pub const DEAD_VIEW: crate::core::OrbitDeadView = crate::core::OrbitDeadView {
    distance: 96.0,
    height: 14.0,
    wall: 6.0,
};

/// HL2:DM's dead view in its own app (`DEAD_VIEW`; the party apps keep
/// theirs).
pub struct Hl2DeadViewPlugin;

impl Plugin for Hl2DeadViewPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(DEAD_VIEW);
    }
}

/// Players wear their `Hl2Model` by name (none, or the default: the
/// unnamed default body).
#[allow(clippy::type_complexity)]
pub fn wear_models(
    players: Query<(Entity, Option<&Hl2Model>, Option<&BodyName>), Or<(Changed<Hl2Model>, Added<BodyName>)>>,
    mut gone: RemovedComponents<Hl2Model>,
    names: Query<&BodyName>,
    mut commands: Commands,
) {
    for (e, model, name) in &players {
        let want = model.map(|m| m.0.as_str()).filter(|m| *m != DEFAULT_MODEL);
        match (want, name) {
            (Some(w), Some(n)) if n.0 == w => {}
            (Some(w), _) => {
                commands.entity(e).insert(BodyName(w.to_string()));
            }
            (None, Some(n)) if is_ours(&n.0) => {
                commands.entity(e).remove::<BodyName>();
            }
            _ => {}
        }
    }
    for e in gone.read() {
        if names.get(e).is_ok_and(|n| is_ours(&n.0))
            && let Ok(mut c) = commands.get_entity(e)
        {
            c.remove::<BodyName>();
        }
    }
}

/// Whether `name` names one of HL2:DM's player models.
fn is_ours(name: &str) -> bool {
    PLAYER_MODELS.iter().any(|p| p.eq_ignore_ascii_case(name))
}

/// HL2:DM's bodies worn.
pub struct Hl2BodiesPlugin;

impl Plugin for Hl2BodiesPlugin {
    fn build(&self, app: &mut App) {
        use chimera_net::net::parts::NetAppExt;
        app.register_type::<Hl2Model>()
            .add_systems(Update, wear_models)
            .net_part("hl2dm_bodies", register);
        // Its ragdolls (spec 10): no death pose, the killing force on the
        // pelvis, kept after the respawn until the next death.
        let mut rules = app.world_mut().get_resource_or_init::<crate::map::ragdoll::RagdollGameRules>();
        if !rules.0.iter().any(|(g, _)| *g == GAME) {
            rules.0.push((GAME, RAGDOLL));
        }
    }
}

/// The protocol: each player's model, from the server.
fn register(app: &mut App) {
    use bevy_replicon::prelude::AppRuleExt;
    use chimera_net::net::ordered::OrderedAppExt;
    app.replicate::<Hl2Model>().ordered::<Hl2Model>();
}
