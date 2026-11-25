//! Loose items lying in the world, for any game (dropped weapons): a held
//! model (`MapHeldModel`) as a small physics body that players walk
//! through (`core::ITEM_LAYER`), like Source's dropped weapons. Shots push
//! it like any physics prop; it slides and tumbles over displacements.
//!
//! Loose items for any game (`LooseItem`: a held model's key; `ShownItem`: the
//! meshes only (drawn by render), for things that move themselves such as
//! grenades; `SitOnOrigin`: resting on its origin, turned, such as the planted
//! bomb): a dynamic box body sized from the held model (also headless), swept
//! against fast moves, on `core::ITEM_LAYER` so characters neither collide with
//! it nor walk into it (`PushAway::Ignore`), its meshes; shots push it like any
//! body; its contacts are reported (`CollisionEventsEnabled`, with its model's
//! surface as `core::PropSurface`) for games' impact sounds and dust. The
//! item components are requests for drawing too: `render::loose`
//! adds the meshes (none headless). A held model may have a loose form of
//! its own (`loose_key`: GoldSrc's `w_` model beside its `p_`), which
//! both kinds of item take (`LooseAssets::form`).

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{MapHeldModel, MapPart, PhysicsProp, PushAway};

/// A loose item drawn with a held model (its `MapHeldModel` key).
/// Spawners give it a `Transform` and velocities; the map adds the body
/// (`render::loose` the meshes) and removes it with the map.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct LooseItem(pub String);

/// Kilograms (a rifle is about 4; the game's own weight isn't known).
const MASS: f32 = 3.0;
/// Friction (multiplied with the world's 0.8: 0.4, as when it was 0.3
/// averaged with the old 0.5). Measured target (2026-10-15): an AK-47
/// thrown standing on de_dust2 stops 95 units from the hand in CS:S; ours
/// lands a little sooner and stops at about 72 with this (0.5 gave 66;
/// lower barely helps).
const FRICTION: f32 = 0.5;

#[derive(Resource, Default)]
pub struct LooseAssets(pub HashMap<String, LooseAsset>);

/// What a held model's own loose form is keyed by (`loose_key`): a game
/// whose items lie and fly as another model than the one in hand
/// (GoldSrc's `w_` beside its `p_`) adds that model to the kit's held
/// models under the held key with this after it.
pub const LOOSE_SUFFIX: &str = "@loose";

/// The key of held model `key`'s own loose form (`LOOSE_SUFFIX`).
pub fn loose_key(key: &str) -> String {
    format!("{key}{LOOSE_SUFFIX}")
}

impl LooseAssets {
    /// The key a loose or shown item named `key` is drawn by: its own
    /// loose form's where the kit has one (`loose_key`), else `key`.
    pub fn drawn_key<'a>(&self, key: &'a str) -> std::borrow::Cow<'a, str> {
        let own = loose_key(key);
        if self.0.contains_key(&own) { own.into() } else { key.into() }
    }

    /// The loose form an item named `key` takes (`drawn_key`'s).
    pub fn form(&self, key: &str) -> Option<&LooseAsset> {
        self.0.get(self.drawn_key(key).as_ref())
    }
}

pub struct LooseAsset {
    /// From the held model's space (skeleton axes and units) to the item's
    /// frame, centred on its bounds (where its meshes are drawn).
    pub frame: Transform,
    pub half: Vec3,
    /// The model's surface property (impact sounds), lower-case.
    pub surface: Option<String>,
    /// The way the model faces in its own game (`MapHeldModel::nose`) in
    /// the item's frame, unit; None: not known. `nose_turn` turns it to
    /// −Z, the axis movers put along their flight.
    pub nose: Option<Vec3>,
    /// The model's named points (`MapHeldModel::attachments`) in the
    /// item's frame (meters, from its bounds' centre): where a game starts
    /// what follows the item (TF2's rocket trail on the rocket's `trail`,
    /// specs/tf2/effects.md 1), with their axes in that frame (a system
    /// started there takes them: the rocket's trail leaves backward).
    pub attachments: Vec<(String, Transform)>,
}

impl LooseAsset {
    /// Its attachment `name`'s place in the item's frame (case
    /// insensitive), if the model has one.
    pub fn attachment(&self, name: &str) -> Option<Vec3> {
        self.attachment_frame(name).map(|t| t.translation)
    }

    /// Its attachment `name`'s place and axes in the item's frame (case
    /// insensitive), if the model has one.
    pub fn attachment_frame(&self, name: &str) -> Option<Transform> {
        self.attachments.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, t)| *t)
    }

    /// The turn that puts this model's nose on the item's −Z (identity
    /// when its nose isn't known): a shown item turned `facing(dir) *
    /// turn` flies nose first, whatever its game or its model's axes.
    pub fn nose_turn(&self) -> Quat {
        self.nose.map_or(Quat::IDENTITY, |n| Quat::from_rotation_arc(n, Vec3::NEG_Z))
    }
}

/// The turn that puts shown model `key`'s nose on −Z
/// (`LooseAsset::nose_turn`; identity without its loose form or nose):
/// a mover facing its flight with −Z (`weapon::projectile::facing`, a
/// game's own) turns its model by this so it flies nose first.
pub fn nose_turn(assets: Option<&LooseAssets>, key: &str) -> Quat {
    assets.and_then(|a| a.form(key)).map_or(Quat::IDENTITY, LooseAsset::nose_turn)
}

/// Each held model's loose form, sized by the first character's scale
/// (none without characters), or by the first of its own game's where
/// its key names one (`<game>:...`: a guest game's models on another
/// game's map keep their own units).
pub fn assets(data: &super::MapData) -> Option<LooseAssets> {
    let first = data.kit.characters.first()?.root;
    let root_of = |key: &str| {
        data.kit
            .characters
            .iter()
            .find(|c| c.game.is_some_and(|g| key.strip_prefix(g).is_some_and(|r| r.starts_with(':'))))
            .map_or(first, |c| c.root)
    };
    Some(LooseAssets(
        data.kit.held.iter().map(|h| (h.key.clone(), asset(h, root_of(&h.key)))).collect(),
    ))
}

/// A held model's loose form: `root` takes the skeleton's space to meters
/// (a character model's `root`; its translation is ignored).
pub fn asset(held: &MapHeldModel, root: Transform) -> LooseAsset {
    let to = Transform::from_rotation(root.rotation).with_scale(root.scale);
    let (lo, hi) = held
        .model
        .meshes
        .iter()
        .flat_map(|m| &m.positions)
        .map(|p| to.transform_point(Vec3::from(*p)))
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min(p), hi.max(p))
        });
    let (centre, half) = if lo.x <= hi.x {
        ((lo + hi) / 2.0, ((hi - lo) / 2.0).max(Vec3::splat(0.01)))
    } else {
        (Vec3::ZERO, Vec3::splat(0.1))
    };
    let frame = to.with_translation(-centre);
    LooseAsset {
        frame,
        half,
        surface: held.model.surfaceprop.clone(),
        nose: held.nose.map(|n| (to.rotation * (to.scale * n)).normalize_or(Vec3::NEG_Z)),
        attachments: held
            .attachments
            .iter()
            .map(|(n, t)| (n.clone(), Transform::from_translation(frame.transform_point(t.translation)).with_rotation(frame.rotation * t.rotation)))
            .collect(),
    }
}

/// Give new loose items their body (`render::loose` their meshes).
pub fn attach_loose(
    items: Query<(Entity, &LooseItem), Without<RigidBody>>,
    assets: Option<Res<LooseAssets>>,
    mut commands: Commands,
) {
    let Some(assets) = assets else { return };
    for (e, item) in &items {
        let half = assets.form(&item.0).map_or(Vec3::splat(0.1), |a| a.half);
        let mut ent = commands.entity(e);
        // Its landings and knocks sound by its model's surface, and kick up
        // dust when hard (the game's impact sounds and effects listen).
        if let Some(s) = assets.form(&item.0).and_then(|a| a.surface.clone()) {
            ent.try_insert(crate::core::PropSurface(s));
        }
        // `try_`: an item can be gone by the time this applies (picked up,
        // or a network client's copy despawned, in the same frame; the
        // network soak saw it: a failed command each time).
        ent.try_insert((
            RigidBody::Dynamic,
            Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0),
            CollisionLayers::new(crate::core::ITEM_LAYER, LayerMask::ALL),
            Mass(MASS),
            Friction::new(FRICTION).with_combine_rule(CoefficientCombine::Multiply),
            Restitution::new(0.1),
            LinearDamping(0.1),
            AngularDamping(0.5),
            // Small and fast when shot: swept so it can't pass through
            // thin surfaces, against everything (avian sweeps a dynamic
            // body against static and kinematic ones by default). Only
            // while it moves: avian 0.8 sweeps a body once it travels more
            // than half its thinnest collider in a step (`threshold`); an
            // unthresholded sweep (avian 0.7's default) of every resting
            // item against everything its box touched cost 19 ms a tick
            // with 250 placed weapons on mg_swag_multigames_v1.
            SweptCcd::default().with_filter(CcdFilter::ALL),
            MaxLinearSpeed(2000.0 * 0.0254),
            MaxAngularSpeed(3600f32.to_radians()),
            PhysicsProp {
                push: PushAway::Ignore,
                mass: MASS,
                bounds: (-half, half),
            },
            (MapPart, Visibility::default(), CollisionEventsEnabled),
            // Hidden where the camera can't see it, wherever it goes
            // (`tag_moved_brush_entities`): placed weapons in other rooms
            // showed through the sky.
            (
                super::BrushEntityBounds { min: -half, max: half },
                super::VisClusters::new(Vec::new()),
            ),
        ));
    }
}

/// An item drawn with a held model's meshes (its `MapHeldModel` key) and
/// no physics body: something that moves itself (a thrown grenade).
/// Spawners give it a `Transform`; `render::loose` adds the meshes.
/// Reflected: a live game's projectiles in flight are a remote query
/// (`world.query` with it and `Transform`, docs/OBSERVABILITY.md 3).
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct ShownItem(pub String);

/// A `ShownItem` whose model rests on its transform's origin (its
/// bounds' bottom there) instead of being centred on it: something set
/// down on the ground (a planted bomb).
/// The rotation turns the model first (one whose own up isn't the
/// held-model frame's).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SitOnOrigin(pub Quat);

/// Shown items drawn in the past on every machine that draws, by their
/// `ShownItem` keys (a game's projectiles whose weapon data says
/// `interp`: TF2's rocket, specs/tf2/effects.md 11). Such an item isn't
/// drawn itself: a `LateCopy` of it is, at its place `LateDelay` ago
/// (`chimera_net`'s `late`), and its trails start on the copy.
/// `running`: the copies are made in this app (`chimera_net`'s `late`
/// sets it); without them every item is drawn where it is. `hidden`: how
/// long a key's copy keeps its model hidden (s; its trail shows), none: 0.
#[derive(Resource, Clone, Debug, Default)]
pub struct DrawnLate {
    pub keys: std::collections::BTreeSet<String>,
    /// The weapons whose projectiles these are (their marks wait the delay
    /// too: `weapon::projectile::FlyerMark`).
    pub weapons: std::collections::BTreeSet<String>,
    pub hidden: std::collections::BTreeMap<String, f32>,
    /// A key's whole unit (m): its first received place is rounded to
    /// whole units of it (Source sends a projectile's origin in whole
    /// inches: specs/tf2/effects.md 11.2); none: not rounded.
    pub unit: std::collections::BTreeMap<String, f32>,
    pub running: bool,
}

impl DrawnLate {
    /// `key` drawn late, its model hidden its first `hidden` seconds.
    pub fn add(&mut self, key: String, hidden: f32) {
        if hidden > 0.0 {
            self.hidden.insert(key.clone(), hidden);
        } else {
            self.hidden.remove(&key);
        }
        self.keys.insert(key);
    }

    /// `key`'s first received place rounded to whole `unit`s (m).
    pub fn set_unit(&mut self, key: &str, unit: f32) {
        if unit > 0.0 {
            self.unit.insert(key.to_string(), unit);
        }
    }

    /// `at` as `key`'s first received place: in whole units when its
    /// data's units are set (`set_unit`).
    pub fn received(&self, key: &str, at: bevy::math::Vec3) -> bevy::math::Vec3 {
        match self.unit.get(key) {
            Some(&u) => (at / u).round() * u,
            None => at,
        }
    }

    /// How long a copy shown as `key` keeps its model hidden (s).
    pub fn hidden_for(&self, key: &str) -> f32 {
        self.hidden.get(key).copied().unwrap_or(0.0)
    }

    /// Whether an item shown as `key` is drawn where it is (`is_copy`: it
    /// is a `LateCopy`): every item but one drawn late, and late ones' copies.
    pub fn drawn_here(late: Option<&Self>, key: &str, is_copy: bool) -> bool {
        is_copy || late.is_none_or(|l| !l.running || !l.keys.contains(key))
    }
}

/// How far in the past items drawn late are drawn this frame (s): a
/// network client's interpolation delay, else what its settings would give
/// (`cl_interp`, `cl_interp_ratio` over the default update rate: 0.1 s, in
/// whole ticks plus one: 0.12 s at 0.015 s ticks).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct LateDelay(pub f32);

/// The copy an item drawn late (`DrawnLate`) is drawn as: where `of` was
/// `LateDelay` ago (`chimera_net`'s `late`); `owner` is the item's
/// (its flyer's), for team looks; `first` its first received place.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LateCopy {
    pub of: Entity,
    pub owner: Option<Entity>,
    /// The item's first received place (effects.md 11.2's p0, in whole
    /// units), where the copy is drawn until the drawn time reaches it
    /// and TF2 makes the trail that follows the copy; the water choice
    /// is by it.
    pub first: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::{MapMesh, MapModel};

    #[test]
    fn loose_items_are_centred_and_scaled() {
        let mut model = MapModel::default();
        model.meshes.push(MapMesh {
            positions: vec![[0.0, 0.0, 0.0], [40.0, 4.0, 2.0]],
            ..default()
        });
        let held = MapHeldModel {
            merged: Vec::new(),
            own: None,
            key: "gun".into(),
            model,
            bone: "b".into(),
            muzzle: None,
            attachments: Vec::new(),
            nose: None,
            turned: Vec::new(),
        };
        let a = asset(&held, Transform::from_scale(Vec3::splat(0.0254)));
        assert!((a.half - Vec3::new(0.508, 0.0508, 0.0254)).length() < 1e-5);
        // The model's far corner lands at +half.
        let corner = a.frame.transform_point(Vec3::new(40.0, 4.0, 2.0));
        assert!((corner - a.half).length() < 1e-5);
    }

    /// A held model keyed by a game's id (`halo_ce:...`) takes the scale of
    /// that game's character, not the map's first one; any other the first.
    #[test]
    fn a_games_own_items_keep_its_scale() {
        let body = |game: &'static str, scale: f32| crate::map::MapCharacterModel {
            game: Some(game),
            team: None,
            name: None,
            class: None,
            model: default(),
            hitboxes: Vec::new(),
            bones: Vec::new(),
            root: Transform::from_scale(Vec3::splat(scale)),
            animations: None,
            boxes: Vec::new(),
            ragdoll: None,
        };
        let held = |key: &str| {
            let mut model = MapModel::default();
            model.meshes.push(MapMesh { positions: vec![[0.0, 0.0, 0.0], [2.0, 2.0, 2.0]], ..default() });
            MapHeldModel { merged: Vec::new(), own: None, key: key.into(), model, bone: String::new(), muzzle: None, attachments: Vec::new(), nose: None, turned: Vec::new() }
        };
        let mut data = crate::map::MapData::default();
        data.kit.characters = vec![body("cs_source", 0.0254), body("halo_ce", 3.0)];
        data.kit.held = vec![held("weapon_ak47"), held("halo_ce:obj:frag"), held("halo_cex:other")];
        let a = assets(&data).unwrap();
        assert!((a.0["weapon_ak47"].half.x - 0.0254).abs() < 1e-5);
        assert!((a.0["halo_ce:obj:frag"].half.x - 3.0).abs() < 1e-5);
        assert!((a.0["halo_cex:other"].half.x - 0.0254).abs() < 1e-5, "a prefix of another name is not its game");
    }
}
