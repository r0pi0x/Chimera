//! The level as a vehicle's mass points see it (`physics::Level`), on the
//! engine's collision: the deepest surface within a point's sphere is the
//! closest point of any solid collider but the vehicle's own and the
//! characters' (avian's point projection), its material from the level's
//! material table (`weapons::materials::LevelMaterials`, a short ray toward
//! the surface); a centre's sweep is a ray cast that keeps front-facing
//! hits. World units and Halo's axes on the physics' side, metres and the
//! engine's axes on avian's (`maps::space`).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::physics::{Contact, Level};
use crate::core::{ITEM_LAYER, RAGDOLL_LAYER, SOLID_LAYERS};
use crate::game::maps::space;
use crate::game::weapons::materials::LevelMaterials;

/// A Halo direction from the engine's.
pub fn to_halo_dir(v: Vec3) -> Vec3 {
    Vec3::new(-v.z, -v.x, v.y)
}

/// A Halo point (WU) from the engine's (m).
pub fn to_halo(p: Vec3) -> Vec3 {
    Vec3::from(space::to_game(p))
}

/// An engine point (m) from Halo's (WU).
pub fn to_engine(p: Vec3) -> Vec3 {
    space::point(p.to_array())
}

/// What a vehicle's queries meet: solid layers but loose items and
/// ragdolls.
pub fn mask() -> LayerMask {
    LayerMask(SOLID_LAYERS.0 & !ITEM_LAYER.0 & !RAGDOLL_LAYER.0)
}

/// What the material lookup reads of a collider.
pub type Shape = (
    &'static Collider,
    Option<&'static Position>,
    Option<&'static Rotation>,
    Option<&'static Name>,
);

/// The engine's collision for one vehicle.
pub struct EngineLevel<'a, 'w, 's> {
    pub query: &'a SpatialQuery<'w, 's>,
    pub filter: SpatialQueryFilter,
    /// Entities not to meet (characters, the vehicle's riders).
    pub skip: &'a dyn Fn(Entity) -> bool,
    pub materials: Option<&'a LevelMaterials>,
    pub shapes: &'a Query<'w, 's, Shape>,
}

impl Level for EngineLevel<'_, '_, '_> {
    fn sphere(&self, center: Vec3, radius: f32) -> Option<Contact> {
        let s = space::METERS_PER_UNIT;
        let c = to_engine(center);
        let hit = self
            .query
            .project_point_predicate(c, false, &self.filter, &|e| !(self.skip)(e))?;
        let d = (c - hit.point).length() / s;
        if d >= radius {
            return None;
        }
        let normal = if d > 1e-6 {
            to_halo_dir((c - hit.point) / (d * s))
        } else {
            Vec3::Z
        };
        let material = self.materials.and_then(|m| {
            let dir = (hit.point - c).normalize_or(Vec3::NEG_Y);
            m.at(self.shapes.get(hit.entity).ok(), c, dir, (radius + 0.05) * s)
        });
        Some(Contact {
            normal,
            depth: radius - d,
            material,
        })
    }

    fn sweep(&self, from: Vec3, to: Vec3) -> Option<(f32, Vec3)> {
        let (a, b) = (to_engine(from), to_engine(to));
        let len = (b - a).length();
        let dir = Dir3::new(b - a).ok()?;
        let hit = self
            .query
            .cast_ray_predicate(a, dir, len, false, &self.filter, &|e| !(self.skip)(e))?;
        (hit.normal.dot(*dir) < 0.0).then(|| (hit.distance / len, to_halo_dir(hit.normal)))
    }
}
