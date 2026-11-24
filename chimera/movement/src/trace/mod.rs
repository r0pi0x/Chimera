//! The movement's traces: a box swept through the map's brushes and terrain,
//! the other characters' boxes, moving solids and ladder volumes (`boxes`,
//! exactly, plane by plane) and through props (physics shape casts), the
//! nearest hit reported as the spec's traces report it (`Trace`), in Source
//! units from the feet; solid and water point tests.

mod boxes;

use avian3d::prelude::*;
use bevy::prelude::*;

use crate::{
    core::MapBrush,
    map::{MapBrushTree, MapBrushes, MapTerrain, MapWater, sound::SurfaceGrid},
    units::{METERS_PER_UNIT, to_engine, to_units},
};

/// How far sweeps stop short of what they hit, along the move. Our stand-in
/// for the BSP trace's distance epsilon, so the box never rests touching.
pub(crate) const TRACE_BACKOFF: f32 = 0.031_25;

/// Solid tests use the box shrunk by this much, so resting contact (within
/// the back-off) doesn't count as stuck.
pub(crate) const SOLID_SKIN: f32 = 0.01;

/// Result of sweeping a box, as the spec's traces report it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Trace {
    /// What was hit is a ladder.
    pub(crate) ladder: bool,
    pub(crate) fraction: f32,
    /// Feet position at the end of the sweep.
    pub(crate) end: Vec3,
    /// The moving solid hit, if any.
    pub(crate) owner: Option<Entity>,
    pub(crate) normal: Vec3,
    pub(crate) start_solid: bool,
    /// What was hit, when it carries its own surface property (props).
    /// None for the world: footsteps look it up from the faces below.
    pub(crate) surface: Option<HitSurface>,
}

/// Where a trace hit's surface property lives (`Tracer::surface_name`).
#[derive(Clone, Copy, Debug)]
pub(crate) enum HitSurface {
    /// Index into `MapBrushes`.
    Brush(usize),
    /// A physics collider with a `PropSurface`.
    Collider(Entity),
}

impl Trace {
    #[inline]
    pub(crate) fn hit(&self) -> bool {
        self.fraction < 1.0
    }
}

/// Box sweeps against the world, in Source units. Brushes and terrain
/// triangles (displacements, as thin solids with bevel planes) are swept
/// exactly against their planes (as Source traces do), so both stop the
/// same distance short and count resting contact the same way; props go
/// through physics shape casts. (Shape casts against the terrain's one
/// triangle-mesh collider stopped players dead now and then: edge contacts
/// on flat ground gave sideways normals, and a contact ignored as grazing
/// hid any real hit behind it, leaving the box inside the terrain.)
pub(crate) struct Tracer<'a, 'w, 's> {
    pub(crate) query: &'a SpatialQuery<'w, 's>,
    pub(crate) filter: &'a SpatialQueryFilter,
    pub(crate) brushes: Option<&'a MapBrushes>,
    /// A broad phase over `brushes` (built from them; ignored if it was
    /// built from another list).
    pub(crate) grid: Option<&'a crate::core::BrushGrid>,
    /// The BSP tree over `brushes` (Source maps): decides which of two
    /// faces hit at the same distance a trace reports.
    pub(crate) tree: Option<&'a MapBrushTree>,
    pub(crate) water: Option<&'a MapWater>,
    /// Other games' liquid volumes (`WaterZone`), engine space.
    pub(crate) water_zones: &'a [MapBrush],
    pub(crate) surfaces: Option<&'a SurfaceGrid>,
    /// Surface property of a physics collider (props), by entity.
    pub(crate) prop_surfaces: &'a dyn Fn(Entity) -> Option<String>,

    pub(crate) sounds: Option<&'a crate::map::MapSounds>,
    /// Standing and ducked box heights.
    pub(crate) heights: (f32, f32),
    /// Half the box's width (`QuakeConfig::half_width`).
    pub(crate) half_width: f32,
    /// Other characters' boxes (engine space), swept exactly like brushes:
    /// players are axis-aligned boxes to each other, as in Source.
    pub(crate) others: &'a [&'a MapBrush],
    /// Terrain triangles (displacements), swept exactly like brushes.
    pub(crate) terrain: Option<&'a MapTerrain>,
    /// For each of `others`: the moving solid it belongs to (None for
    /// characters).
    pub(crate) owners: &'a [Option<Entity>],
}

impl<'a> Tracer<'a, '_, '_> {
    /// The surface property name a trace hit carries, if any.
    pub(crate) fn surface_name(&self, hit: HitSurface) -> Option<String> {
        match hit {
            HitSurface::Brush(i) => self.brushes?.0.get(i)?.surface.clone(),
            HitSurface::Collider(e) => (self.prop_surfaces)(e),
        }
    }

    /// Sweep a box (`lo`..`hi` relative to the feet, Source units) from feet
    /// position `from` to `to`.
    #[inline]
    pub(crate) fn sweep_box(&self, lo: Vec3, hi: Vec3, from: Vec3, to: Vec3) -> Trace {
        self.sweep_box_by(lo, hi, from, to - from)
    }

    /// `sweep_box` by a move `delta` (Source units) from `from`.
    pub(crate) fn sweep_box_by(&self, lo: Vec3, hi: Vec3, from: Vec3, delta: Vec3) -> Trace {
        let size = hi - lo;
        let centre = |feet: Vec3| to_engine(feet + (lo + hi) / 2.0);
        let start_solid = self.solid_box(lo, hi, from);
        let to = from + delta;
        let len = delta.length();
        let miss = Trace {
            ladder: false,
            owner: None,
            fraction: 1.0,
            end: to,
            normal: Vec3::ZERO,
            start_solid,
            surface: None,
        };
        if len < 1e-6 {
            return Trace { end: from, ..miss };
        }
        // Box half size in engine axes (x, up, z).
        let half = Vec3::new(size.x, size.z, size.y) * METERS_PER_UNIT / 2.0;
        let brush = self.sweep_brushes(half, centre(from), to_engine(delta));
        let mut best = Trace {
            owner: brush.owner,
            ladder: brush.ladder,
            fraction: brush.fraction,
            end: from + delta * brush.fraction,
            normal: to_units(brush.normal).normalize_or_zero(),
            start_solid,
            surface: brush.surface.map(HitSurface::Brush),
        };
        let shape = Collider::cuboid(half.x * 2.0, half.y * 2.0, half.z * 2.0);
        let Ok(dir) = Dir3::new(to_engine(delta / len)) else {
            return best;
        };
        let config = ShapeCastConfig {
            max_distance: len * METERS_PER_UNIT,
            ignore_origin_penetration: true,
            ..default()
        };
        // A surface that doesn't face against the move (we're sliding along
        // it, or already leaving it) doesn't block it; physics casts still
        // report such grazing contacts.
        if let Some(hit) = self
            .query
            .cast_shape(&shape, centre(from), Quat::IDENTITY, dir, &config, self.filter)
            .filter(|hit| hit.normal1.dot(*dir) < -1e-3)
        {
            // Keep the margin along the surface normal, as Source's traces
            // do (a margin along the move leaves a glancing box almost
            // touching, which the stuck test then flags).
            let facing = (-hit.normal1.dot(*dir)).max(0.05);
            let travelled = (hit.distance / METERS_PER_UNIT - TRACE_BACKOFF / facing).max(0.0);
            let fraction = (travelled / len).clamp(0.0, 0.999_999);
            if fraction < best.fraction {
                best = Trace {
                    owner: None,
                    ladder: false,
                    fraction,
                    end: from + delta * fraction,
                    normal: to_units(hit.normal1).normalize_or_zero(),
                    start_solid,
                    surface: Some(HitSurface::Collider(hit.entity)),
                };
            }
        }
        best
    }

    #[inline]
    pub(crate) fn hull(&self, ducked: bool) -> (Vec3, Vec3) {
        let height = if ducked { self.heights.1 } else { self.heights.0 };
        (
            Vec3::new(-self.half_width, -self.half_width, 0.0),
            Vec3::new(self.half_width, self.half_width, height),
        )
    }

    #[inline]
    pub(crate) fn sweep(&self, ducked: bool, from: Vec3, to: Vec3) -> Trace {
        let (lo, hi) = self.hull(ducked);
        self.sweep_box(lo, hi, from, to)
    }

    /// `sweep` by a move `delta` from `from` (`sweep_brushes`: exact for
    /// moves along a plane).
    #[inline]
    pub(crate) fn sweep_by(&self, ducked: bool, from: Vec3, delta: Vec3) -> Trace {
        let (lo, hi) = self.hull(ducked);
        self.sweep_box_by(lo, hi, from, delta)
    }

    pub(crate) fn solid_box(&self, lo: Vec3, hi: Vec3, feet: Vec3) -> bool {
        let full = hi - lo;
        if self.in_brush(
            Vec3::new(full.x, full.z, full.y) * METERS_PER_UNIT / 2.0,
            to_engine(feet + (lo + hi) / 2.0),
        ) {
            return true;
        }
        let size = (hi - lo - Vec3::splat(2.0 * SOLID_SKIN)).max(Vec3::splat(0.01));
        let shape = Collider::cuboid(
            size.x * METERS_PER_UNIT,
            size.z * METERS_PER_UNIT,
            size.y * METERS_PER_UNIT,
        );
        !self
            .query
            .shape_intersections(&shape, to_engine(feet + (lo + hi) / 2.0), Quat::IDENTITY, self.filter)
            .is_empty()
    }

    #[inline]
    pub(crate) fn solid(&self, ducked: bool, feet: Vec3) -> bool {
        let (lo, hi) = self.hull(ducked);
        self.solid_box(lo, hi, feet)
    }

    /// Point contents: solid.
    #[inline]
    pub(crate) fn point_solid(&self, p: Vec3) -> bool {
        self.solid_box(Vec3::splat(-0.05), Vec3::splat(0.05), p)
    }

    /// Point contents: water (`Some(false)`) or slime (`Some(true)`).
    pub(crate) fn point_water(&self, p: Vec3) -> Option<bool> {
        let at = to_engine(p);
        self.water
            .and_then(|w| w.0.iter().find(|w| w.brush.contains(at)).map(|w| w.slime))
            .or_else(|| self.water_zones.iter().any(|z| z.contains(at)).then_some(false))
    }
}
