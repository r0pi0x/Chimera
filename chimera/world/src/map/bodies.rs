//! Character bodies, the simulation's side: which model a character uses
//! (its team's, its class's or the one `BodyName` names, among its
//! identity's game's models when it has one: `slots::GameIdentity`,
//! `CharacterModels::for_identity`; a free-for-all player of an identity's
//! game with interchangeable skins (player classes) wears one by its team
//! id, so the players differ), what it holds
//! (`Held`), its animations and its hitboxes following the skeleton (also
//! headless, for the server's hit tests). The drawn body, held model and
//! their posing are `render::bodies`, which draws what these say.
//!
//! `CharacterModels`, `BodyModel`, `BodyName`, `BodySkin`, `BodyNumber`, `Corpse`, `Held`,
//! `ShowLocalBody`; held models a game adds after the map spawned
//! (`add_held_model`, `LateHeldModels`: one the map's kit lacks), and
//! character models likewise (`add_character_model`,
//! `LateCharacterModels`, appended to each map's `CharacterModels`);
//! hitboxes on the skeleton (also headless), `SimAnimator`/`SimPose`.
//!
//! Known shortcuts (docs/tech-debt.md): Maps without character models fall back
//! to height bands on the collider for hitgroups; in single player hitboxes
//! follow the client-rate animation (a network server poses them per tick from
//! its own animation, `map::SimAnimator`), which takes other characters' look
//! as drawn (eased between ticks, `map::interp`), so their hitboxes trail the
//! simulated look by up to a tick; the server's per-tick animation runs CS:S's
//! player animation state a second time (twice the posing work per character on
//! a listen server) and its hitboxes for tick N come from the end of tick N-1.
//! Why it's OK for now: Matches what players see; lag compensation hits rewound
//! poses (games/cs_source/tests/it/heavy/map_net_weapons.rs).
//!
#[allow(unused_imports)]
use super::*;

/// What a character holds (a `MapHeldModel` key), drawn in its body's
/// hand. Games set it.
#[derive(Component, Clone, Default, Debug, PartialEq)]
pub struct Held(pub Option<String>);

/// Held models added after the map spawned (`add_held_model`): a game's
/// weapon whose held model the map's kit doesn't carry (Combat Arms'
/// install weapons, loaded the first time a character holds one), each
/// with its own textures (its meshes' texture indices name them). Kept
/// across map changes: `render::bodies::build_late_held` builds what the
/// spawned map's held models lack.
#[derive(Resource, Clone, Default)]
pub struct LateHeldModels(pub Vec<(std::sync::Arc<MapHeldModel>, std::sync::Arc<Vec<MapTexture>>)>);

impl LateHeldModels {
    /// Whether a held model of `key` was added.
    pub fn has(&self, key: &str) -> bool {
        self.0.iter().any(|(m, _)| m.key == key)
    }
}

/// Add held model `model` (drawn from the next frame, on this map and the
/// next ones), its textures `textures`; one of the same key is replaced.
pub fn add_held_model(world: &mut World, model: MapHeldModel, textures: std::sync::Arc<Vec<MapTexture>>) {
    let mut late = world.get_resource_or_init::<LateHeldModels>();
    late.0.retain(|(m, _)| m.key != model.key);
    late.0.push((std::sync::Arc::new(model), textures));
}

/// Character models added after the map spawned (`add_character_model`):
/// a game's body the map's kit doesn't carry (Combat Arms' install bodies
/// a ladder rung dresses its players in, loaded the first time one is
/// worn), each with its own textures (its meshes' texture indices name
/// them). Kept across map changes: appended to each map's
/// `CharacterModels` (`append_late_characters`, after the map's own, so
/// their indices stay) on a map that has a model of the same game, and
/// drawn by `render::bodies::build_late_bodies`.
#[derive(Resource, Clone, Default)]
pub struct LateCharacterModels(pub Vec<(Arc<MapCharacterModel>, Arc<Vec<MapTexture>>)>);

impl LateCharacterModels {
    /// The model named `name` and its textures.
    pub fn get(&self, name: &str) -> Option<&(Arc<MapCharacterModel>, Arc<Vec<MapTexture>>)> {
        self.0.iter().find(|(m, _)| m.name.as_deref() == Some(name))
    }
}

/// Add character model `model` (named: `MapCharacterModel::name`), its
/// textures `textures`, from the next tick on this map and the next ones;
/// one of the same name is replaced.
pub fn add_character_model(world: &mut World, model: MapCharacterModel, textures: Arc<Vec<MapTexture>>) {
    let mut late = world.get_resource_or_init::<LateCharacterModels>();
    late.0.retain(|(m, _)| m.name != model.name);
    late.0.push((Arc::new(model), textures));
}

/// The late character models (`LateCharacterModels`) the map's
/// `CharacterModels` lack, appended: each on a map with a model of its
/// game (where that game's bodies are worn).
pub fn append_late_characters(late: Option<Res<LateCharacterModels>>, models: Option<ResMut<CharacterModels>>) {
    let (Some(late), Some(mut models)) = (late, models) else { return };
    let missing: Vec<MapCharacterModel> = late
        .0
        .iter()
        .filter(|(m, _)| {
            !models.0.iter().any(|x| x.name == m.name) && models.0.iter().any(|x| x.game.is_some() && x.game == m.game)
        })
        .map(|(m, _)| (**m).clone())
        .collect();
    if missing.is_empty() {
        return;
    }
    // Not a copy of every body (a frame's work): only when another holds
    // the list.
    Arc::make_mut(&mut models.0).extend(missing);
}

/// A body drawn on an entity that is not a character: the corpse a game
/// leaves behind. It takes the same components as a character's body
/// (`BodyName`, `BodySkin`, `BodyNumber`, an `Animator`) and the body
/// hangs `feet` metres below the entity's origin, turned by the entity's
/// own rotation.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Corpse {
    pub feet: f32,
}

/// Which character model a character uses (index in `CharacterModels`).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug)]
#[component(storage = "SparseSet")]
pub struct BodyModel(pub usize);
/// The loaded map's character models (see `MapCharacterModel`).
#[derive(Resource, Clone)]
pub struct CharacterModels(pub Arc<Vec<MapCharacterModel>>);

impl CharacterModels {
    /// The model for `team`: its own, else one without a team, else any.
    pub fn for_team(&self, team: Option<Team>) -> Option<(usize, &MapCharacterModel)> {
        self.for_team_of(None, team)
    }

    /// Whether game `game` may wear `m` (`MapCharacterModel::game`; no
    /// game: every one).
    fn wears(game: Option<&str>, m: &MapCharacterModel) -> bool {
        match (game, m.game) {
            (Some(g), Some(own)) => own.eq_ignore_ascii_case(g),
            _ => true,
        }
    }

    fn for_team_of(&self, game: Option<&str>, team: Option<Team>) -> Option<(usize, &MapCharacterModel)> {
        let all = self
            .0
            .iter()
            .enumerate()
            .filter(|(_, m)| Self::wears(game, m) && m.name.is_none() && m.class.is_none());
        all.clone()
            .find(|(_, m)| team.is_some() && m.team == team)
            .or_else(|| all.clone().find(|(_, m)| m.team.is_none()))
            .or_else(|| all.clone().next())
    }

    /// The model a character uses: the one its `BodyName` names, else its
    /// class's (`core::PlayerClass`), else its team's.
    pub fn for_character(
        &self,
        team: Option<Team>,
        name: Option<&BodyName>,
        class: Option<&crate::core::PlayerClass>,
    ) -> Option<(usize, &MapCharacterModel)> {
        self.for_identity(None, team, name, class)
    }

    /// `for_character` among the models of the character's identity's game
    /// (`slots::GameIdentity`; without one, among all): None when that
    /// game has none here.
    pub fn for_identity(
        &self,
        identity: Option<&chimera_core::slots::GameIdentity>,
        team: Option<Team>,
        name: Option<&BodyName>,
        class: Option<&crate::core::PlayerClass>,
    ) -> Option<(usize, &MapCharacterModel)> {
        let game = identity.map(|i| i.0.as_str());
        let named = name.and_then(|n| {
            self.0.iter().enumerate().find(|(_, m)| {
                Self::wears(game, m) && m.name.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(&n.0))
            })
        });
        match (name, named) {
            (_, Some(found)) => Some(found),
            // A name the map lacks among its identity's game's bodies (an
            // HL2:DM player model a map's kit doesn't carry): its game's
            // usual body. Without an identity's game, nothing (a hostage's
            // model).
            (Some(_), None) if identity.is_none() => None,
            _ => class
                // A class counts for its team, or in a free-for-all
                // (a team id past the two sides) whichever it is.
                .filter(|c| c.0 != 0 && (c.team() == team || team.is_some_and(|t| t.0 > 2)))
                .and_then(|c| {
                    let own = self.0.iter().enumerate().find(|(_, m)| Self::wears(game, m) && m.class == Some(c.0));
                    // A team's first class has no class tag: its default.
                    own.or_else(|| {
                        let side = c.team()?;
                        let first = (c.0 - 1) % crate::core::PlayerClass::PER_TEAM == 0;
                        let all = self.0.iter().enumerate().filter(|(_, m)| {
                            Self::wears(game, m) && m.name.is_none() && m.class.is_none()
                        });
                        first.then(|| all.clone().find(|(_, m)| m.team == Some(side)).or_else(|| all.clone().find(|(_, m)| m.team.is_none())))?
                    })
                })
                .or_else(|| identity.and_then(|_| self.varied(game, team)))
                .or_else(|| self.for_team_of(game, team)),
        }
    }

    /// A free-for-all player's body (a team id past the two sides, one
    /// each) among its game's skins when the game has interchangeable ones
    /// (player classes): by its team id, so the players of a match wear
    /// different bodies instead of all the fallback's. None for a side's
    /// player, a game with one body per team, or no identity's game.
    fn varied(&self, game: Option<&str>, team: Option<Team>) -> Option<(usize, &MapCharacterModel)> {
        // The sides are ids 1 and 2 (`mode::ALPHA`, `BRAVO`).
        let t = team.filter(|t| t.0 > 2)?;
        let pool: Vec<(usize, &MapCharacterModel)> = self
            .0
            .iter()
            .enumerate()
            .filter(|(_, m)| Self::wears(game, m) && m.name.is_none())
            .collect();
        if !pool.iter().any(|(_, m)| m.class.is_some()) {
            return None;
        }
        Some(pool[(t.0 as usize - 3) % pool.len()])
    }

    /// Whether the body a character wears (`BodyModel`) is game `game`'s,
    /// of no game in particular, or none of the map's: which game's
    /// animation drives it.
    pub fn is_of(&self, body: Option<&BodyModel>, game: &str) -> bool {
        body.and_then(|b| self.0.get(b.0))
            .is_none_or(|m| m.game.is_none_or(|g| g.eq_ignore_ascii_case(game)))
    }
}

/// A character drawn with a named model (`MapCharacterModel::name`)
/// instead of its team's: a hostage.
#[derive(Component, Clone, Debug, PartialEq)]
#[component(storage = "SparseSet")]
pub struct BodyName(pub String);

/// Which skin family a character's body shows (Source `skin`: a map's
/// hostage keeps its own); without it, the model's first.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct BodySkin(pub i32);

/// Which choice of each body part a character's body shows (Source's
/// body number: `MapModel::body_choice` over the model's `body_parts`;
/// meshes with no `MapMesh::body` always show); without it, choice 0 of
/// each.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[component(storage = "SparseSet")]
pub struct BodyNumber(pub i32);

/// Whether the local player's own body is drawn (third person). The
/// client sets it; the body is there either way, animated, and hidden
/// when this is false.
#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShowLocalBody(pub bool);

/// Game systems that decide what bodies play run in this set, before
/// the joints are posed.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct DriveAnimation;

/// Characters get their team's hitboxes from the loaded character models.
/// Characters also get the model's animations (an `Animator`, driven by
/// the game).
pub fn attach_hitboxes(
    models: Option<Res<CharacterModels>>,
    characters: Query<
        (
            Entity,
            Option<&Team>,
            Option<&BodyName>,
            Option<&crate::core::PlayerClass>,
            Option<&BodyModel>,
            Option<&chimera_core::slots::GameIdentity>,
        ),
        With<crate::core::Intent>,
    >,
    mut commands: Commands,
) {
    let Some(models) = models else { return };
    for (e, team, name, class, current, identity) in &characters {
        let Some((index, m)) = models.for_identity(identity, team.copied(), name, class) else {
            // Its identity's game has no body here: none (it was another
            // game's).
            if current.is_some() {
                commands
                    .entity(e)
                    .remove::<(BodyModel, crate::core::Hitboxes, anim::Animator)>();
            }
            continue;
        };
        if current == Some(&BodyModel(index)) {
            continue;
        }
        let mut c = commands.entity(e);
        c.insert(BodyModel(index));
        // A body without hitboxes (a game whose hits are its own, as
        // Combat Arms' spheres) leaves other games' bullets its collider.
        if m.hitboxes.is_empty() {
            c.remove::<crate::core::Hitboxes>();
        } else {
            c.insert(crate::core::Hitboxes(m.hitboxes.clone()));
        }
        match &m.animations {
            Some(set) => c.insert(anim::Animator {
                skeleton: Some(m.skeleton_signature()),
                ..anim::Animator::new(set.clone())
            }),
            None => c.remove::<anim::Animator>(),
        };
    }
}

/// Hitboxes follow the animated skeleton (as the server places them),
/// except where a network server animates the body in the fixed tick
/// (`SimAnimator`).
#[allow(clippy::type_complexity)]
pub fn pose_hitboxes(
    models: Option<Res<CharacterModels>>,
    mut characters: Query<
        (
            &anim::Animator,
            &BodyModel,
            &crate::core::Intent,
            &ragdoll::SkeletonPose,
            &mut crate::core::Hitboxes,
        ),
        Without<SimAnimator>,
    >,
) {
    let Some(models) = models else { return };
    for (animator, model, intent, pose, mut hitboxes) in &mut characters {
        let Some(m) = models.0.get(model.0) else { continue };
        if animator.main.is_none() || m.boxes.len() != hitboxes.0.len() {
            continue;
        }
        // Bones in the skeleton's space (this tick's pose), then to the
        // character's frame (which the trace turns by the look yaw; the
        // body turns by its own yaw).
        let Some(frame) = pose.frames.back() else { continue };
        let global = &frame.bones;
        let turn = Quat::from_rotation_y(animator.yaw.unwrap_or(intent.yaw) - intent.yaw) * m.root.rotation;
        let scale = m.root.scale.x;
        for (h, b) in hitboxes.0.iter_mut().zip(&m.boxes) {
            let Some((q, p)) = global.get(b.bone) else { continue };
            h.center = turn * (*p + *q * b.center) * scale;
            h.half = b.half * scale;
            h.rotation = turn * *q;
        }
    }
}

/// A network server's own animation of a character's body, driven in the
/// fixed tick from simulation values (games drive it before `SimPose`,
/// e.g. CS:S's player animation state): the character's hitboxes are
/// posed from it after every tick (`pose_sim_hitboxes`), not from the
/// drawn animation (which runs at the frame rate and eases the look), so
/// what a tick traces and what lag compensation keeps is the tick's own
/// pose (docs/plans/active/multiplayer.md, slice 4).
#[derive(Component, Clone, Debug)]
pub struct SimAnimator(pub anim::Animator);

/// After each tick (`FixedLast`): hitboxes posed from `SimAnimator`s.
/// Games drive the animators before it; lag compensation records after.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct SimPose;

/// Hitboxes from the server's own animation of each body, at the tick's
/// time (as `pose_hitboxes` does from the drawn one).
pub fn pose_sim_hitboxes(
    models: Option<Res<CharacterModels>>,
    clock: Res<crate::core::SimClock>,
    mut characters: Query<(
        &SimAnimator,
        &BodyModel,
        &crate::core::Intent,
        &mut crate::core::Hitboxes,
    )>,
) {
    let Some(models) = models else { return };
    for (animator, model, intent, mut hitboxes) in &mut characters {
        let animator = &animator.0;
        let Some(m) = models.0.get(model.0) else { continue };
        if animator.main.is_none() || m.boxes.len() != hitboxes.0.len() {
            continue;
        }
        let global = ragdoll::globals(m, &animator.pose(clock.now));
        let turn = Quat::from_rotation_y(animator.yaw.unwrap_or(intent.yaw) - intent.yaw) * m.root.rotation;
        let scale = m.root.scale.x;
        for (h, b) in hitboxes.0.iter_mut().zip(&m.boxes) {
            let Some((q, p)) = global.get(b.bone) else { continue };
            h.center = turn * (*p + *q * b.center) * scale;
            h.half = b.half * scale;
            h.rotation = turn * *q;
        }
    }
}
