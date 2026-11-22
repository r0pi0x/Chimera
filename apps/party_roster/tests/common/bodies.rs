//! Drawn stand-in bodies of Combat Arms and CS:S on a small floor.

#![allow(dead_code, unused_imports)]

use std::collections::HashMap;
use bevy::{camera::visibility::RenderLayers, prelude::*};
use chimera_core::{
    core::{Damage, DamageKind, Dead, God, Health, Hitgroup},
    slots::GameIdentity,
};
use chimera_sim::minigame::{self, ladder::Rung};
use chimera_world::{
    harness::Sim,
    map::{
        MapBone, MapBrush, MapCharacterModel, MapData, MapDebugView, MapMesh, MapModel, MapRagdoll, MapRagdollBody,
        MapRagdollJoint, change_map,
        ragdoll::Ragdolled,
    },
};
use combat_arms::harness::CaSim;
use party_roster::{DEFAULT_MINIGAME, PartyPartsPlugin};
use chimera_present::render::{bodies::CharacterBody, prop_material::PropMaterial};
use crate::play::{place, soldier};
use crate::play::*;

pub(crate) const SCALE: f32 = 0.0254;

/// The map's bodies: Combat Arms' (0) and CS:S's (1).
pub(crate) const CA_BODY: usize = 0;

pub(crate) const CS_BODY: usize = 1;

/// A box's corners.
pub(crate) fn cube(lo: Vec3, hi: Vec3) -> Vec<Vec3> {
    (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        })
        .collect()
}

/// A small drawn character of `game`: pelvis and spine (inches, Source
/// axes), one triangle, a two-body ragdoll.
pub(crate) fn character(game: &'static str) -> MapCharacterModel {
    let bone = |name: &str, parent, at| MapBone { name: name.into(), parent, position: at, rotation: Quat::IDENTITY };
    let body = |bone, lo, hi, mass| MapRagdollBody {
        bone,
        pieces: vec![cube(lo, hi)],
        mass,
        damping: 0.05,
        rotdamping: 3.0,
        inertia: 10.0,
        surfaceprop: "flesh".into(),
        friction: 0.8,
        elasticity: 0.0,
    };
    let face = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let axes = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    MapCharacterModel {
        game: Some(game),
        team: None,
        name: None,
        class: None,
        model: MapModel {
            meshes: vec![MapMesh {
                positions: vec![[0.0, 0.0, 0.0], [4.0, 0.0, 0.0], [0.0, 4.0, 1.0]],
                normals: vec![[0.0, 0.0, 1.0]; 3],
                uvs: vec![[0.0, 0.0]; 3],
                indices: vec![0, 1, 2],
                ..default()
            }],
            bounds: (Vec3::ZERO, Vec3::splat(4.0 * SCALE)),
            ..default()
        },
        hitboxes: Vec::new(),
        bones: vec![bone("pelvis", None, Vec3::new(0.0, 0.0, 38.0)), bone("spine", Some(0), Vec3::new(0.0, 0.0, 6.0))],
        root: Transform::from_rotation(face * axes).with_scale(Vec3::splat(SCALE)),
        animations: None,
        boxes: Vec::new(),
        ragdoll: Some(MapRagdoll {
            bodies: vec![
                body(0, Vec3::new(-4.0, -6.0, -3.0), Vec3::new(4.0, 6.0, 6.0), 10.0),
                body(1, Vec3::new(-4.0, -7.0, 0.0), Vec3::new(4.0, 7.0, 20.0), 30.0),
            ],
            joints: vec![MapRagdollJoint {
                parent: 0,
                child: 1,
                limits: [(-10.0f32, 10.0f32), (-20.0, 30.0), (-16.0, 16.0)].map(|(a, b)| (a.to_radians(), b.to_radians())),
            }],
            collision_pairs: Some(Vec::new()),
        }),
    }
}

/// A floor whose top is at y = 0, 60 m across, with both games' bodies.
pub(crate) fn floor(name: &str) -> MapData {
    let (lo, hi) = (Vec3::new(-30.0, -1.0, -30.0), Vec3::new(30.0, 0.0, 30.0));
    MapData {
        name: name.into(),
        collision_brushes: vec![MapBrush::from_box(lo, hi)],
        collision_hulls: vec![cube(lo, hi).iter().map(|v| v.to_array()).collect()],
        kit: chimera_world::map::GameKit {
            characters: vec![character("combat_arms"), character("cs_source")],
            ..default()
        },
        ..default()
    }
}

/// The asset stores `render` draws into (headless `Sim` has meshes only).
pub(crate) fn render_assets(app: &mut App) {
    app.init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<PropMaterial>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
}

pub(crate) fn alive(world: &World, e: Entity) -> bool {
    world.get::<Dead>(e).is_none() && world.get::<Health>(e).is_some_and(|h| h.current > 0.0)
}

/// Why living `e` isn't drawn as its identity's body, if it isn't.
pub(crate) fn undrawn(world: &mut World, e: Entity) -> Option<String> {
    if world.get::<Ragdolled>(e).is_some() {
        return Some("still marked Ragdolled".into());
    }
    let want = match world.get::<GameIdentity>(e).map(|i| i.0.as_str()) {
        Some("cs_source") => CS_BODY,
        Some("combat_arms") => CA_BODY,
        other => return Some(format!("identity {other:?}")),
    };
    // Nothing above it hidden.
    let mut at = Some(e);
    while let Some(x) = at {
        if world.get::<Visibility>(x) == Some(&Visibility::Hidden) {
            return Some(format!("{x} above the body hidden"));
        }
        at = world.get::<ChildOf>(x).map(ChildOf::parent);
    }
    let kids: Vec<Entity> = world.get::<Children>(e).map(|c| c.to_vec()).unwrap_or_default();
    let bodies: Vec<Entity> = kids.into_iter().filter(|c| world.get::<CharacterBody>(*c).is_some()).collect();
    let [body] = bodies[..] else {
        return Some(format!("{} bodies", bodies.len()));
    };
    let model = world.get::<CharacterBody>(body).unwrap().model;
    if model != want {
        return Some(format!("body {model}, its identity's is {want}"));
    }
    if world.get::<Visibility>(body) == Some(&Visibility::Hidden) {
        return Some("body hidden".into());
    }
    let main = RenderLayers::layer(0);
    let off_layer = |x: Entity| world.get::<RenderLayers>(x).is_some_and(|l| !l.intersects(&main));
    if off_layer(body) {
        return Some("body off the main layer".into());
    }
    let parts: Vec<Entity> = world.get::<Children>(body).map(|c| c.to_vec()).unwrap_or_default();
    let drawn = parts
        .iter()
        .filter(|p| world.get::<Mesh3d>(**p).is_some())
        .filter(|p| world.get::<Visibility>(**p) != Some(&Visibility::Hidden) && !off_layer(**p))
        .count();
    (drawn == 0).then(|| "no drawn mesh".into())
}

/// A small deterministic generator (xorshift).
pub(crate) struct Dice(pub(crate) u64);

/// Why living `e` has no single drawn body, whatever game's.
pub(crate) fn undrawn_any(world: &mut World, e: Entity) -> Option<String> {
    if world.get::<Ragdolled>(e).is_some() {
        return Some("still marked Ragdolled".into());
    }
    let kids: Vec<Entity> = world.get::<Children>(e).map(|c| c.to_vec()).unwrap_or_default();
    let n = kids.iter().filter(|c| world.get::<CharacterBody>(**c).is_some()).count();
    (n != 1).then(|| format!("{n} bodies"))
}

impl Dice {
    pub(crate) fn below(&mut self, n: usize) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 % n as u64) as usize
    }
}
