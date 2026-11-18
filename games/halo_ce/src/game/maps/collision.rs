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
//! Ladders (docs/DESIGN.md, "Ladders across games"): each climbable
//! surface (one of each two-sided twin pair) is also a ladder brush in
//! `collision_brushes` (`ladder`, `MapBrush::ladder`; in `trace_skip`, so
//! shots pass it): a slab `LADDER_SKIN` thick on each side of the sheet
//! (past the terrain's thickness, which a two-sided sheet's twin has on
//! the climber's side),
//! bounded by the polygon's edges (`ladder_brush`). Chimera publishes
//! these as `core::MapLadders::volumes`, which every game's movement
//! climbs by its own rule (Halo's: `movement`'s climbable ground). The
//! sheet itself stays a solid triangle as before. Because a map with
//! brushes gets a kill height under its lowest brush unless it says where
//! it is played, `playable` is the level's collision box (`add`).
//!
//! Where the level is played, for the bots' navigation (`nav_within`): the
//! structure BSP's world bounds, the box of every rendered vertex
//! (structure_bsp.md, "World bounds"), which the surfaces with a material
//! span exactly while the material-less ones reach past it, to the outer
//! cliffs and a lid (collision.md, "Materials": Blood Gulch's collision
//! goes to z 68 WU, its drawn level to 26.2). Handed to the engine as
//! `nav_gen::NavWithin` (a map extra): a cell of a generated mesh is kept
//! only when its floor is inside the drawn level's box, so a walker that
//! climbs past the drawn cliffs stops at the drawn level's edge. (The
//! level's point test, `CollisionBsp::point_leaf`, would add nothing: every
//! leaf of the multiplayer maps has a cluster, and the space between the
//! drawn level and the boundary surfaces is open leaves too.)
//!
//! Known shortcuts:
//! - Breakable surfaces (Prisoner's 8) stay solid: breaking them belongs
//!   to a later breakables spec (`collision.md`, "Breakable state").

use std::io;

use std::collections::BTreeSet;

use bevy::math::Vec3;
use blam_formats::collision::{LevelCollision, MaterialType};

use super::{space, surfaces};
use crate::map::{MapBrush, MapCollisionSet, MapData, nav_gen::NavWithin};

/// How far a ladder brush stands out on each side of its sheet (m): past
/// the terrain's thickness (`map::TERRAIN_THICKNESS`), since a two-sided
/// sheet's twin is a terrain solid that thick on the climber's side, and a
/// slab inside it is never reached (a CS:S or TF2 character's ladder probe
/// met the twin's back face instead, on Prisoner's real sheets).
pub const LADDER_SKIN: f32 = crate::map::TERRAIN_THICKNESS + 0.01;

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
    /// Ladder brushes (climbable surfaces, one per twin pair).
    pub ladders: usize,
}

/// A climbable sheet (a convex polygon, engine space, in order) as a ladder
/// brush: the slab `LADDER_SKIN` either side of its plane, cut by a plane
/// through each edge. None for a degenerate polygon.
pub fn ladder_brush(points: &[Vec3]) -> Option<MapBrush> {
    if points.len() < 3 {
        return None;
    }
    // Newell's normal (either sense: the slab is symmetric).
    let mut n = Vec3::ZERO;
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        n += Vec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
    }
    let n = n.try_normalize()?;
    let d = n.dot(points[0]);
    let mut planes = vec![(n, d + LADDER_SKIN), (-n, -d + LADDER_SKIN)];
    for (i, a) in points.iter().enumerate() {
        let b = points[(i + 1) % points.len()];
        // Outward from the polygon in its plane: away from its centre.
        let mut out = (b - *a).cross(n).try_normalize()?;
        let centre = points.iter().copied().sum::<Vec3>() / points.len() as f32;
        if out.dot(centre - *a) > 0.0 {
            out = -out;
        }
        planes.push((out, out.dot(*a)));
    }
    let skin = n.abs() * LADDER_SKIN;
    let (min, max) = points
        .iter()
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| (lo.min(*p), hi.max(*p)));
    Some(MapBrush {
        planes,
        min: min - skin,
        max: max + skin,
        ladder: true,
        surface: None,
    })
}

/// How far (m) the navigation's box reaches past the drawn level's (a
/// floor point on its edge, rounding).
pub const NAV_MARGIN: f32 = 0.05;

/// The drawn level's box (`world_bounds`, the structure BSP's, in world
/// units) as the box the bots' generated navigation is held to (module
/// doc), engine space.
pub fn nav_within(world_bounds: &blam_formats::cache::RealRect3d) -> Option<NavWithin> {
    let b = world_bounds;
    let corners = [
        space::point([b.x.lower, b.y.lower, b.z.lower]),
        space::point([b.x.upper, b.y.upper, b.z.upper]),
    ];
    let (min, max) = (corners[0].min(corners[1]), corners[0].max(corners[1]));
    (min.is_finite() && max.is_finite() && min.cmplt(max).all()).then(|| NavWithin {
        min: min - Vec3::splat(NAV_MARGIN),
        max: max + Vec3::splat(NAV_MARGIN),
    })
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
            let code = level.surface_material_type(s).map_or(-1, |t| t.code() as i8);
            map.extras.entry::<crate::game::weapons::materials::LevelCodes>().0.push(code);
        }
    }
    // Ladders: one brush per climbable sheet, twins once.
    let mut seen = BTreeSet::new();
    for s in 0..bsp.surfaces.len() {
        if !bsp.surfaces[s].flags.climbable() {
            continue;
        }
        let mut key = bsp.polygon(s)?;
        key.sort_unstable();
        if !seen.insert(key) {
            continue;
        }
        let points: Vec<Vec3> = bsp.polygon_points(s)?.into_iter().map(space::point).collect();
        if let Some(brush) = ladder_brush(&points) {
            map.trace_skip.push(map.collision_brushes.len());
            map.collision_brushes.push(brush);
            counts.ladders += 1;
        }
    }
    // Where the level is played (its collision's box): a map with ladder
    // brushes is killed under it, not under its lowest ladder.
    let (lo, hi) = map
        .collision_positions
        .iter()
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        });
    if lo.cmple(hi).all() {
        map.playable = Some((lo, hi));
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
