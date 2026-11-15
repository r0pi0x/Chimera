//! The level's material types where a projectile meets it
//! (specs/halo_ce/projectiles.md behaviour 5: "the material type (level:
//! the collision material's type, -1 for material-less surfaces)";
//! collision.md "Materials"), for the material responses.
//!
//! The map loader knows each collision triangle's material type; it hands
//! the list to the simulation as one map entity (`ENTITY_CLASS`, its
//! `codes` key: the types of `MapData::collision_indices` in order, run
//! length encoded by `encode`), so a cached conversion and a network
//! client carry it like any entity. `follow` keeps `LevelMaterials` (the
//! decoded list) with the loaded map's entities. A trace that hits the
//! map's surfaces collider (a triangle mesh of `collision_indices` in
//! order) finds the triangle by a ray cast against that mesh in its own
//! space (`LevelMaterials::at`); the glass set's collider is glass (type
//! 9).
//!
//! A level without the list (the greybox, another game's map) has no
//! Halo material types: its surfaces count as stone (`NO_TABLE`, the
//! port's choice; the projectiles' tests use stone as the spec's floor).

use std::sync::Arc;

use avian3d::{
    parry::{query::Ray, query::RayCast},
    prelude::{Collider, Position, Rotation},
};
use bevy::prelude::*;

use crate::map::{MapEntities, MapEntity};

/// What a trace knows of the collider it met.
pub type Shape<'a> = (&'a Collider, Option<&'a Position>, Option<&'a Rotation>, Option<&'a Name>);

/// The entity's class name.
pub const ENTITY_CLASS: &str = "halo_ce_level_materials";
/// The key holding the codes.
pub const KEY: &str = "codes";
/// What a level without a table is made of: stone (type 2).
pub const NO_TABLE: u8 = 2;
/// Glass (type 9).
pub const GLASS: u8 = 9;

/// The material type codes of a map's collision triangles, gathered while
/// converting (a scratch value of `MapData::extras`, taken out into the
/// entity when the conversion ends).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct LevelCodes(pub Vec<i8>);

/// Run-length encoded codes: `code*count` pairs separated by spaces.
pub fn encode(codes: &[i8]) -> String {
    let mut out = String::new();
    let mut i = 0;
    while i < codes.len() {
        let c = codes[i];
        let n = codes[i..].iter().take_while(|x| **x == c).count();
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(&format!("{c}*{n}"));
        i += n;
    }
    out
}

/// `encode`'s text back (None when malformed or over a million codes).
pub fn decode(s: &str) -> Option<Vec<i8>> {
    let mut out = Vec::new();
    for pair in s.split_whitespace() {
        let (c, n) = pair.split_once('*')?;
        let (c, n): (i8, usize) = (c.parse().ok()?, n.parse().ok()?);
        if out.len() + n > 1 << 20 {
            return None;
        }
        out.extend(std::iter::repeat_n(c, n));
    }
    Some(out)
}

/// The entity that carries `codes`.
pub fn entity(codes: &[i8]) -> MapEntity {
    MapEntity {
        keyvalues: vec![("classname".into(), ENTITY_CLASS.into()), (KEY.into(), encode(codes))],
        ..Default::default()
    }
}

/// The loaded level's material types, by collision triangle.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct LevelMaterials(pub Arc<Vec<i8>>);

impl LevelMaterials {
    /// The material type where the ray `from` along `dir` (metres) meets
    /// `collider` (an entity of the map), `distance` metres along: by the
    /// triangle when it is the surfaces mesh (None: material-less), glass
    /// for the glass set, else stone.
    pub fn at(&self, collider: Option<Shape>, from: Vec3, dir: Vec3, distance: f32) -> Option<u8> {
        let Some((c, pos, rot, name)) = collider else { return Some(NO_TABLE) };
        if name.is_some_and(|n| n.as_str().contains("(glass)")) {
            return Some(GLASS);
        }
        let Some(mesh) = c.shape_scaled().as_trimesh() else { return Some(NO_TABLE) };
        if mesh.num_triangles() != self.0.len() {
            return Some(NO_TABLE);
        }
        let p = pos.map_or(Vec3::ZERO, |p| p.0);
        let r = rot.map_or(Quat::IDENTITY, |r| r.0).inverse();
        let ray = Ray::new(r * (from - p), r * dir);
        let hit = mesh.cast_local_ray_and_get_normal(&ray, distance + 0.01, true)?;
        let code = *self.0.get(hit.subshape as usize)?;
        (code >= 0).then_some(code as u8)
    }
}

/// `LevelMaterials` from the map's entities, kept with them.
pub fn follow(entities: Option<Res<MapEntities>>, mut commands: Commands, mut seen: Local<Option<usize>>) {
    let Some(map) = entities else {
        if seen.take().is_some() {
            commands.remove_resource::<LevelMaterials>();
        }
        return;
    };
    let key = Arc::as_ptr(&map.entities) as usize;
    if *seen == Some(key) {
        return;
    }
    *seen = Some(key);
    let codes = map
        .entities
        .iter()
        .find(|e| e.classname() == ENTITY_CLASS)
        .and_then(|e| e.get(KEY))
        .and_then(decode);
    match codes {
        Some(c) => commands.insert_resource(LevelMaterials(Arc::new(c))),
        None => commands.remove_resource::<LevelMaterials>(),
    }
}
