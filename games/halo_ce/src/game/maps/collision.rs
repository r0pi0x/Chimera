//! The level's collision BSP as the engine's collision triangles
//! (specs/halo_ce/collision.md; read by `blam_formats::collision::
//! read_level`): every surface polygon fan-triangulated from its first
//! vertex (`CollisionBsp::triangulate`), in engine space (`space::point`),
//! counter-clockwise about the outward normal as stored (the axis mapping
//! is a rotation, so the winding is kept).
//!
//! What goes where ("Surface flags: who collides with what"):
//! - `collision_indices`: every surface players and shots meet, the
//!   material-less ones included (the level's invisible walls and lid,
//!   "Materials": they must stay in the physics mesh), and both twins of a
//!   two-sided surface (coincident, opposite windings: whichever side a
//!   player comes from, a triangle faces him), ladders (climbable) and
//!   breakable surfaces (intact at load) too.
//! - A `MapCollisionSet` "glass" for the surfaces of material type glass:
//!   solid to players and shots, but light passes (`see_through`, what
//!   bots see through), with the surface name `glass`.
//! - A `MapCollisionSet` "invisible" for the invisible-flagged surfaces:
//!   bipeds and vehicles collide, projectiles and sight ignore them, so
//!   characters only (none on the multiplayer maps).
//!
//! Footsteps and impacts read the surface under them from the drawn
//! meshes (`MapMesh::surface`, from the same shaders' material types:
//! `surfaces`), as the other games' do; the collision mesh is one
//! collider.
//!
//! Known shortcuts:
//! - Breakable surfaces (Prisoner's 8) stay solid: breaking them belongs
//!   to a later breakables spec (`collision.md`, "Breakable state").
//! - Climbable surfaces (ladders) are plain solid triangles: climbing is
//!   the movement's (`movement.md`).

use std::io;

use blam_formats::collision::{LevelCollision, MaterialType};

use super::{space, surfaces};
use crate::map::{MapCollisionSet, MapData};

/// The glass set's name.
pub const GLASS: &str = "glass";
/// The invisible set's name.
pub const INVISIBLE: &str = "invisible";

/// What the collision conversion counted, for the map's notes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub triangles: usize,
    pub two_sided: usize,
    pub climbable: usize,
    pub breakable: usize,
    pub invisible: usize,
    pub glass: usize,
    pub materialless: usize,
}

/// Fill `map`'s collision from the level's collision BSP.
pub fn add(level: &LevelCollision, map: &mut MapData) -> io::Result<Counts> {
    let bsp = &level.bsp;
    let base = map.collision_positions.len() as u32;
    map.collision_positions
        .extend(bsp.vertices.iter().map(|v| space::point(v.point).to_array()));
    let mut counts = Counts::default();
    let mut glass = Vec::new();
    let mut invisible = Vec::new();
    for t in bsp.triangulate()? {
        let s = t.surface as usize;
        let surface = &bsp.surfaces[s];
        let tri = t.vertices.map(|v| base + v);
        counts.triangles += 1;
        if surface.flags.two_sided() {
            counts.two_sided += 1;
        }
        if surface.flags.climbable() {
            counts.climbable += 1;
        }
        if surface.flags.breakable() {
            counts.breakable += 1;
        }
        if surface.material < 0 {
            counts.materialless += 1;
        }
        if surface.flags.invisible() {
            counts.invisible += 1;
            invisible.push(tri);
        } else if level.surface_material_type(s) == Some(MaterialType::Glass) {
            counts.glass += 1;
            glass.push(tri);
        } else {
            map.collision_indices.push(tri);
        }
    }
    if !glass.is_empty() {
        map.collision_sets.push(MapCollisionSet {
            name: GLASS.into(),
            indices: glass,
            characters: true,
            traces: true,
            see_through: true,
            surface: surfaces::surface_name(MaterialType::Glass),
        });
    }
    if !invisible.is_empty() {
        map.collision_sets.push(MapCollisionSet {
            name: INVISIBLE.into(),
            indices: invisible,
            characters: true,
            traces: false,
            see_through: true,
            surface: None,
        });
    }
    Ok(counts)
}

/// Every collision triangle of `map` (the main mesh and every set), as
/// points.
pub fn all_triangles(map: &MapData) -> impl Iterator<Item = [[f32; 3]; 3]> + '_ {
    map.collision_indices
        .iter()
        .chain(map.collision_sets.iter().flat_map(|s| s.indices.iter()))
        .map(|t| t.map(|i| map.collision_positions[i as usize]))
}
