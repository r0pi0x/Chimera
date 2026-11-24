//! The exact box sweep against brushes (the map's through its broad phase,
//! terrain triangles, other characters and movers as boxes): each brush's
//! planes pushed out by the box, the BSP tree's tie-break where the map has
//! one (`Tracer::sweep_brushes`), and the box-in-brush test.

use bevy::prelude::*;

use crate::{
    core::MapBrush,
    source_movement::{
        trace::{SOLID_SKIN, TRACE_BACKOFF, Tracer},
        units::METERS_PER_UNIT,
    },
};

/// Float noise allowance (meters) when deciding whether a sweep runs
/// parallel to a brush plane: about a thousandth of a unit.
pub(crate) const PARALLEL_SLOP: f32 = 2.5e-5;

/// Brush sweep result, engine space.
pub(crate) struct BrushHit {
    pub(crate) ladder: bool,
    /// The hit brush, when it has its own surface property.
    pub(crate) surface: Option<usize>,
    /// The moving solid hit, if any.
    pub(crate) owner: Option<Entity>,
    pub(crate) fraction: f32,
    pub(crate) normal: Vec3,
    pub(crate) start_solid: bool,
}

impl<'a> Tracer<'a, '_, '_> {
    /// Sweep an axis-aligned box (centre `from` moved by `delta`, half size
    /// `half`, engine meters) through the brushes: each brush's planes
    /// pushed out by the box's extent along their normals, then the
    /// centre's segment clipped against them. Hits stop a distance epsilon
    /// short.
    ///
    /// The move is a start and a delta, not two end points: how far the
    /// move goes into a plane is `n . delta`, which is exact enough to tell
    /// a move clipped along a plane from one into it. The difference of the
    /// end points' distances isn't, far from the origin: there, rounding a
    /// position to f32 moves it ~1e-5 m, as much as the parallel allowance,
    /// so a velocity just clipped along a surf ramp (surf_sedona's, 200 m
    /// up) read as entering it again and the player stopped dead.
    pub(crate) fn sweep_brushes(&self, half: Vec3, from: Vec3, delta: Vec3) -> BrushHit {
        let eps = TRACE_BACKOFF * METERS_PER_UNIT;
        let touch = SOLID_SKIN * METERS_PER_UNIT;
        let to = from + delta;
        let mut out = BrushHit {
            ladder: false,
            surface: None,
            owner: None,
            fraction: 1.0,
            normal: Vec3::ZERO,
            start_solid: false,
        };
        let brushes = self.brushes.map_or(&[][..], |b| &b.0[..]);
        let lo = from.min(to) - half - Vec3::splat(eps);
        let hi = from.max(to) + half + Vec3::splat(eps);
        let terrain = self.terrain_near(lo, hi);
        // Brushes hit at the current fraction: (index, normal, ladder,
        // where the box really reaches the brush).
        let mut tied: Vec<(usize, Vec3, bool, f32)> = Vec::new();
        'brush: for (i, b) in self.world_near(lo, hi).chain(
            self.others
                .iter()
                .copied()
                .chain(terrain)
                .enumerate()
                .map(|(j, b)| (brushes.len() + j, b)),
        ) {
            if b.max.cmplt(lo).any() || b.min.cmpgt(hi).any() {
                continue;
            }
            let (mut enter, mut leave) = (-1.0f32, 1.0f32);
            let (mut starts_out, mut gets_out) = (false, false);
            let mut clip = Vec3::ZERO;
            let prop = b.surface.is_some();
            // The last plane the box really crosses (no back-off): the
            // contact's normal.
            let mut last_crossed = -1.0f32;
            for (n, d) in &b.planes {
                let dist = d + n.abs().dot(half);
                let mut d1 = n.dot(from) - dist;
                // A box touching the plane within float noise (feet put
                // exactly on a floor: a teleport destination at floor
                // height) touches it from outside, as the stuck test sees
                // it (`SOLID_SKIN`), instead of starting inside the brush:
                // the sweeps then meet the plane at once (ground found)
                // rather than finding the box start solid, which hung the
                // player above the floor. Our choice; the spec is silent
                // (movement.md, open question 18).
                if d1 <= 0.0 && d1 > -touch {
                    d1 = f32::MIN_POSITIVE;
                }
                let d2 = d1 + n.dot(delta);
                gets_out |= d2 > 0.0;
                starts_out |= d1 > 0.0;
                // Entirely in front of this plane: misses the brush, even
                // when the end is nearer than the back-off (CS:S: a ground
                // probe ending 0.0024 short of the floor finds no ground;
                // movecmp `militia_porch_wall_right`, `hover_2_010`).
                // A prop's hull (its own surface; VPhysics collision in
                // CS:S, not a BSP brush) still counts an end within the
                // back-off as a hit: de_nuke's fuzz water runs land on a
                // prop's near-flat top from 0.0008 above the probe's reach
                // in CS:S (our hull's planes are only that exact).
                // Moving parallel to it counts as not entering, within
                // float noise (planes and positions are meters at map
                // scale), or a velocity just clipped along an angled
                // surface would catch on it forever.
                let reach = if prop { eps } else { 0.0 };
                if d1 > 0.0 && (d2 > reach || d2 >= d1 - PARALLEL_SLOP) {
                    continue 'brush;
                }
                if d1 <= 0.0 && d2 <= 0.0 {
                    continue;
                }
                if d1 > d2 {
                    // Where the move stops: the back-off short of the
                    // plane, along its normal.
                    enter = enter.max(((d1 - eps) / (d1 - d2)).max(0.0));
                    // Which plane it stops against: the one it would
                    // really reach last. Ranking the backed-off fractions
                    // instead favours planes met head-on (their back-off
                    // costs less of the move): gliding a back-off above a
                    // surf ramp, a box nears the edge where the next
                    // segment's face rises within the back-off well before
                    // it nears that face, so the edge's bevel (facing back
                    // along the ramp) came last and stopped it dead.
                    let crossed = d1 / (d1 - d2);
                    if crossed > last_crossed {
                        last_crossed = crossed;
                        clip = *n;
                    }
                } else {
                    leave = leave.min(((d1 + eps) / (d1 - d2)).min(1.0));
                }
            }
            if !starts_out {
                out.start_solid = true;
                if !gets_out {
                    out.fraction = 0.0;
                }
                continue;
            }
            if !(enter < leave && enter > -1.0) {
                continue;
            }
            let f = enter.max(0.0);
            let hit = (i, clip, b.ladder, last_crossed);
            if out.fraction < 1.0 && (f - out.fraction).abs() < 1e-6 {
                if f < out.fraction {
                    out.fraction = f;
                    tied.insert(0, hit);
                } else {
                    tied.push(hit);
                }
            } else if f < out.fraction {
                out.fraction = f;
                tied.clear();
                tied.push(hit);
            }
        }
        // A ladder flush with another face (maps flank ladders with player
        // clip): of the brushes hit at the same distance, the one the trace
        // reaches first through the BSP tree wins, as in Source. Otherwise
        // the one the box really reaches first (the earliest in our list
        // of those): two brushes stop the box at the same back-off when
        // one is met head-on and the other only after it, at a glancing
        // plane whose back-off costs more of the move. A box coming down
        // onto a terrain triangle next to a coplanar one it doesn't
        // overlap met the neighbour's steep side (the 2-unit thickness
        // overhangs the shared edge) at the same back-off as the face
        // under it, took the side's normal and didn't stay on the ground
        // (de_dust2, the probe's `walk_stop`: 0.35 u high for a tick).
        let mut winner = tied.iter().copied().min_by(|a, b| a.3.total_cmp(&b.3));
        if tied.iter().any(|t| t.2) && tied.iter().any(|t| !t.2)
            && let Some(tree) = self.tree
        {
            let order = tree.sweep_order(half, from, to, METERS_PER_UNIT);
            let rank = |k: usize| order.iter().position(|&o| o as usize == k).unwrap_or(usize::MAX);
            winner = tied.iter().copied().min_by_key(|t| rank(t.0));
        }
        if let Some((i, normal, ladder, _)) = winner {
            out.normal = normal;
            out.ladder = ladder;
            out.surface = brushes.get(i).and_then(|b| b.surface.is_some().then_some(i));
            out.owner = i
                .checked_sub(brushes.len())
                .and_then(|j| self.owners.get(j).copied().flatten());
        }
        out
    }

    /// The map's brushes (with their index) that may overlap `lo`..`hi`
    /// (engine space), in index order: all of them without a grid. Those
    /// left out are ones the callers' own bounds tests skip anyway.
    pub(crate) fn world_near(&self, lo: Vec3, hi: Vec3) -> Box<dyn Iterator<Item = (usize, &'a MapBrush)> + 'a> {
        let brushes = self.brushes.map_or(&[][..], |b| &b.0[..]);
        match self.grid {
            Some(g) if g.len == brushes.len() && lo.is_finite() && hi.is_finite() => {
                let mut near = Vec::new();
                g.near(brushes, lo, hi, &mut near);
                Box::new(near.into_iter().map(move |i| (i as usize, &brushes[i as usize])))
            }
            _ => Box::new(brushes.iter().enumerate()),
        }
    }

    /// Terrain brushes whose bounds overlap `lo`..`hi` (engine space).
    pub(crate) fn terrain_near(&self, lo: Vec3, hi: Vec3) -> impl Iterator<Item = &MapBrush> {
        let mut near = Vec::new();
        if let Some(t) = self.terrain {
            t.near(lo, hi, &mut near);
        }
        near.into_iter()
            .filter_map(|i| self.terrain.map(|t| &t.brushes[i as usize]))
    }

    /// Whether a box (engine centre and half size) overlaps a brush by more
    /// than the solid skin.
    pub(crate) fn in_brush(&self, half: Vec3, centre: Vec3) -> bool {
        let skin = SOLID_SKIN * METERS_PER_UNIT;
        let terrain = self.terrain_near(centre - half, centre + half);
        self.world_near(centre - half, centre + half)
            .map(|(_, b)| b)
            .chain(self.others.iter().copied())
            .chain(terrain)
            .any(|b| {
                b.max.cmpgt(centre - half).all()
                    && b.min.cmplt(centre + half).all()
                    && b.planes
                        .iter()
                        .all(|(n, d)| n.dot(centre) - (d + n.abs().dot(half)) < -skin)
            })
    }
}
