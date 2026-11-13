//! Navigation meshes for bots, game-neutral, in engine space (meters, Y
//! up). Areas are axis-aligned XZ rectangles with a height at each corner;
//! links between them are directed. Queries and the A* search follow
//! specs/cs_source/nav.md (the Source nav system, the only one loaded so
//! far), so its constants are Source's, converted to meters.
//!
//! `NavMesh` (any game): areas with corner heights, directed links, ladders,
//! hiding spots, potentially visible areas and encounter spots in engine space;
//! area queries, A* with the stock cost or any step cost (`find_path_with`),
//! crossing points (optionally leaning along the edge), `route`/`points_along`,
//! `within` (areas by path distance), `sees`. `area_at` looks in a grid over
//! the areas (`AreaGrid`), not at every area.

use bevy::prelude::*;

const UNIT: f32 = 0.0254;
/// Climbable without jumping.
pub const STEP_HEIGHT: f32 = 18.0 * UNIT;
/// How far below a query point an area may lie and still contain it.
const BENEATH_LIMIT: f32 = 120.0 * UNIT;
/// How far above a query point an area may lie (feet on a slope).
const ABOVE_LIMIT: f32 = 5.0 * UNIT;
/// Stock cost penalties for entering crouch and jump areas (× distance).
const CROUCH_PENALTY: f64 = 20.0;
const JUMP_PENALTY: f64 = 5.0;
/// Path crossing points stay this far inside an edge with no area beyond.
const EDGE_MARGIN: f32 = 25.0 * UNIT;

/// Area attribute bits (the Source file's values).
pub mod flags {
    pub const CROUCH: u32 = 0x0001;
    pub const JUMP: u32 = 0x0002;
    pub const PRECISE: u32 = 0x0004;
    pub const NO_JUMP: u32 = 0x0008;
    pub const AVOID: u32 = 0x0080;
    pub const STAND: u32 = 0x0400;
    pub const STAIRS: u32 = 0x1000;
}

/// Hiding spot flag bits (the Source file's values).
pub mod spot {
    pub const IN_COVER: u8 = 0x01;
    pub const GOOD_SNIPER: u8 = 0x02;
    pub const IDEAL_SNIPER: u8 = 0x04;
    pub const EXPOSED: u8 = 0x08;
}

/// A place a bot can hide or hold from, on the ground of its area.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct HidingSpot {
    pub pos: Vec3,
    /// `spot` bits.
    pub flags: u8,
}

/// Potential visibility bits of `NavArea::visible`.
pub mod vis {
    pub const PARTLY: u8 = 0x01;
    pub const FULLY: u8 = 0x02;
}

/// Hiding spots that come into view, in order, when walking through an
/// area from one neighbour to another (spec "Encounter record").
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Encounter {
    /// Area indices entered from and left toward.
    pub from: usize,
    pub to: usize,
    /// Each spot's position and the fraction (0..1) of the way through
    /// where it becomes visible.
    pub spots: Vec<(Vec3, f32)>,
}

/// Which edge of an area a link leaves by.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Side {
    MinX,
    MaxX,
    MinZ,
    MaxZ,
}

impl Side {
    pub fn opposite(self) -> Self {
        match self {
            Side::MinX => Side::MaxX,
            Side::MaxX => Side::MinX,
            Side::MinZ => Side::MaxZ,
            Side::MaxZ => Side::MinZ,
        }
    }
}

/// How a link is travelled.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Via {
    Walk(Side),
    /// Climb ladder (index into `ladders`) up or down.
    LadderUp(usize),
    LadderDown(usize),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NavArea {
    /// The file's id.
    pub id: u32,
    pub flags: u32,
    pub min: Vec2,
    pub max: Vec2,
    /// Heights at (min.x, min.y), (max.x, min.y), (max.x, max.y),
    /// (min.x, max.y), where `.y` of the Vec2s is engine Z.
    pub heights: [f32; 4],
    /// Centre for path costs. Its height is the mean of two diagonal
    /// corners, as the source format defines it (spec quirk), not the
    /// surface height there.
    pub center: Vec3,
    /// Outgoing links in the file's order: (area index, how).
    pub links: Vec<(usize, Via)>,
    /// Index into `NavMesh::places`, if named.
    pub place: Option<usize>,
    pub hiding: Vec<HidingSpot>,
    /// Potentially visible areas (index, `vis` bits), sorted by index;
    /// empty when the file has none (versions before 16).
    pub visible: Vec<(usize, u8)>,
    /// Corner checks along paths through this area (version 9 files).
    pub encounters: Vec<Encounter>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct NavLadder {
    pub top: Vec3,
    pub bottom: Vec3,
    /// Path cost length, m.
    pub length: f32,
    /// Outward normal of the climbable face.
    pub normal: Vec3,
}

#[derive(Resource, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct NavMesh {
    pub areas: Vec<NavArea>,
    pub ladders: Vec<NavLadder>,
    pub places: Vec<String>,
    /// Made for its maker's walker, not a map's own navigation file: from
    /// the collision (`nav_gen`, bots' jump-ups), or from a map's AI volumes
    /// (Combat Arms': links through doors, whether or not anything opens
    /// them). Either may reach where players can't.
    pub generated: bool,
    /// `area_at`'s grid over the areas, made by the first query (`AreaGrid`).
    #[serde(skip)]
    pub grid: std::sync::OnceLock<AreaGrid>,
}

impl Clone for NavMesh {
    /// The copy makes its own grid on its first query (a grid is for one
    /// `areas` allocation).
    fn clone(&self) -> Self {
        Self {
            areas: self.areas.clone(),
            ladders: self.ladders.clone(),
            places: self.places.clone(),
            generated: self.generated,
            grid: std::sync::OnceLock::new(),
        }
    }
}

/// The areas over each cell of a square XZ grid, for `NavMesh::area_at`: a
/// generated mesh of a large open map has ten thousand areas (cs_militia:
/// 11,510), and bots ask for the area under a point many times a tick
/// (docs/performance.md, "cs_militia's spikes"). Made lazily from the
/// areas as they are at the first query, for that `areas` allocation and
/// length: after areas are added or removed the queries search every area
/// again, as before the grid.
///
/// Known shortcuts: an area whose rectangle is changed in place after the
/// first query is still found only in the cells it lay over then (nothing
/// edits a mesh in use; tests edit theirs before asking).
#[derive(Debug, Default)]
pub struct AreaGrid {
    /// The `areas` buffer and length the grid was made from.
    of: (usize, usize),
    min: Vec2,
    /// Cells per metre.
    scale: f32,
    nx: usize,
    nz: usize,
    /// Cell `i`'s areas are `items[starts[i]..starts[i + 1]]`, ascending
    /// (file order, which breaks `area_at`'s ties).
    starts: Vec<u32>,
    items: Vec<u32>,
}

impl AreaGrid {
    /// Most cells a side.
    const MAX_SIDE: usize = 512;

    fn new(areas: &[NavArea]) -> Self {
        let of = (areas.as_ptr() as usize, areas.len());
        let (mut min, mut max) = (Vec2::INFINITY, Vec2::NEG_INFINITY);
        for a in areas {
            min = min.min(a.min);
            max = max.max(a.max);
        }
        let size = (max - min).max(Vec2::ZERO);
        // A rectangle that isn't finite: no grid, every query searches all.
        if areas.iter().any(|a| !(a.min.is_finite() && a.max.is_finite())) {
            return Self {
                of: (0, usize::MAX),
                ..default()
            };
        }
        // About four cells an area, as if the areas tiled the bounds.
        let cell = (size.x * size.y / (4.0 * areas.len() as f32))
            .sqrt()
            .max(size.max_element() / Self::MAX_SIDE as f32)
            .max(0.25);
        let scale = 1.0 / cell;
        let mut grid = Self {
            of,
            min,
            scale,
            nx: ((size.x * scale) as usize + 1).min(Self::MAX_SIDE),
            nz: ((size.y * scale) as usize + 1).min(Self::MAX_SIDE),
            ..default()
        };
        let mut counts = vec![0u32; grid.nx * grid.nz + 1];
        let cells = |g: &Self, a: &NavArea| {
            let (x0, z0) = g.cell(a.min.x, a.min.y);
            let (x1, z1) = g.cell(a.max.x, a.max.y);
            (x0..=x1.max(x0), z0..=z1.max(z0))
        };
        for a in areas {
            let (xs, zs) = cells(&grid, a);
            for z in zs {
                for x in xs.clone() {
                    counts[z * grid.nx + x + 1] += 1;
                }
            }
        }
        for i in 1..counts.len() {
            counts[i] += counts[i - 1];
        }
        let mut items = vec![0u32; *counts.last().unwrap() as usize];
        let mut next = counts.clone();
        for (i, a) in areas.iter().enumerate() {
            let (xs, zs) = cells(&grid, a);
            for z in zs {
                for x in xs.clone() {
                    let slot = &mut next[z * grid.nx + x];
                    items[*slot as usize] = i as u32;
                    *slot += 1;
                }
            }
        }
        grid.starts = counts;
        grid.items = items;
        grid
    }

    /// The cell of (x, z), clamped onto the grid: the same rising function
    /// for an area's corners and a query point, so a point on an area
    /// (edges included) is in one of the area's cells.
    fn cell(&self, x: f32, z: f32) -> (usize, usize) {
        let at = |v: f32, lo: f32, n: usize| (((v - lo) * self.scale) as usize).min(n.saturating_sub(1));
        (at(x, self.min.x, self.nx), at(z, self.min.y, self.nz))
    }

    /// The areas that may lie over (x, z), ascending; None when the grid
    /// isn't of `areas` as they are now.
    fn over(&self, areas: &[NavArea], x: f32, z: f32) -> Option<&[u32]> {
        if self.of != (areas.as_ptr() as usize, areas.len()) {
            return None;
        }
        if self.starts.is_empty() {
            return Some(&[]);
        }
        let (cx, cz) = self.cell(x, z);
        let i = cz * self.nx + cx;
        Some(&self.items[self.starts[i] as usize..self.starts[i + 1] as usize])
    }
}

impl NavArea {
    /// Surface height at (x, z), clamped onto the rectangle (bilinear).
    pub fn height_at(&self, x: f32, z: f32) -> f32 {
        let size = self.max - self.min;
        if size.x <= 0.0 || size.y <= 0.0 {
            return self.heights[2];
        }
        let u = ((x - self.min.x) / size.x).clamp(0.0, 1.0);
        let v = ((z - self.min.y) / size.y).clamp(0.0, 1.0);
        let [a, b, c, d] = self.heights;
        let near = a + u * (b - a);
        let far = d + u * (c - d);
        near + v * (far - near)
    }

    /// Whether (x, z) lies on the rectangle, edges included.
    pub fn overlaps(&self, x: f32, z: f32) -> bool {
        self.min.x <= x && x <= self.max.x && self.min.y <= z && z <= self.max.y
    }

    /// The point of the area nearest to `p` (clamped, on the surface).
    pub fn closest_point(&self, p: Vec3) -> Vec3 {
        let x = p.x.clamp(self.min.x, self.max.x);
        let z = p.z.clamp(self.min.y, self.max.y);
        Vec3::new(x, self.height_at(x, z), z)
    }

    pub fn has(&self, flag: u32) -> bool {
        self.flags & flag != 0
    }
}

impl NavMesh {
    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.areas.iter().position(|a| a.id == id)
    }

    /// The area under `p`: the highest one whose surface lies between
    /// 120 units below and 5 units above it (ties: first in file order).
    pub fn area_at(&self, p: Vec3) -> Option<usize> {
        let grid = self.grid.get_or_init(|| AreaGrid::new(&self.areas));
        match grid.over(&self.areas, p.x, p.z) {
            Some(over) => self.highest_at(p, over.iter().map(|&i| i as usize)),
            None => self.highest_at(p, 0..self.areas.len()),
        }
    }

    /// `area_at` among `areas` (ascending indices).
    fn highest_at(&self, p: Vec3, areas: impl Iterator<Item = usize>) -> Option<usize> {
        let mut best: Option<(usize, f32)> = None;
        for i in areas {
            let a = &self.areas[i];
            if !a.overlaps(p.x, p.z) {
                continue;
            }
            let y = a.height_at(p.x, p.z);
            if y > p.y + ABOVE_LIMIT || y < p.y - BENEATH_LIMIT {
                continue;
            }
            if best.is_none_or(|(_, by)| y > by) {
                best = Some((i, y));
            }
        }
        best.map(|(i, _)| i)
    }

    /// `area_at`, else the area whose closest point is nearest to `p`
    /// (ties: first in file order).
    pub fn nearest_area(&self, p: Vec3) -> Option<usize> {
        self.area_at(p).or_else(|| {
            let grid = self.grid.get_or_init(|| AreaGrid::new(&self.areas));
            let nearer = |best: Option<(usize, f32)>, i: usize| {
                let d = self.areas[i].closest_point(p).distance_squared(p);
                match best {
                    Some((b, bd)) if bd < d || (bd == d && b <= i) => best,
                    _ => Some((i, d)),
                }
            };
            if grid.over(&self.areas, p.x, p.z).is_none() || !p.is_finite() {
                return (0..self.areas.len()).fold(None, nearer).map(|(i, _)| i);
            }
            // Rings of cells around the point's: every area in a ring past
            // `r` is at least `r` cells away, so the search ends once the
            // nearest found is nearer than that.
            let (cx, cz) = grid.cell(p.x, p.z);
            let mut best: Option<(usize, f32)> = None;
            for r in 0..grid.nx.max(grid.nz) {
                if best.is_some_and(|(_, d)| d < ((r as f32 - 1.0).max(0.0) / grid.scale).powi(2)) {
                    break;
                }
                let (x0, x1) = (cx as isize - r as isize, cx as isize + r as isize);
                let (z0, z1) = (cz as isize - r as isize, cz as isize + r as isize);
                for z in z0.max(0)..=z1.min(grid.nz as isize - 1) {
                    let rim = z == z0 || z == z1;
                    let mut x = x0;
                    while x <= x1 {
                        if x >= 0 && x < grid.nx as isize {
                            let i = z as usize * grid.nx + x as usize;
                            for &a in &grid.items[grid.starts[i] as usize..grid.starts[i + 1] as usize] {
                                best = nearer(best, a as usize);
                            }
                        }
                        // The ring's rows at its ends are whole, the others
                        // their two end cells.
                        x += if rim || r == 0 { 1 } else { 2 * r as isize };
                    }
                }
            }
            best.map(|(i, _)| i)
        })
    }

    /// Stock path cost of entering `to` from `from` (spec "shortest path").
    pub fn step_cost(&self, from: usize, to: usize, via: Via) -> f64 {
        let (a, b) = (&self.areas[from], &self.areas[to]);
        let dist = match via {
            Via::LadderUp(l) | Via::LadderDown(l) => self.ladders[l].length as f64,
            Via::Walk(_) => (b.center.as_dvec3() - a.center.as_dvec3()).length(),
        };
        let mut cost = dist;
        if b.has(flags::CROUCH) {
            cost += CROUCH_PENALTY * dist;
        }
        if b.has(flags::JUMP) {
            cost += JUMP_PENALTY * dist;
        }
        cost
    }

    /// A* from area `start` to area `goal` with the stock cost. Returns the
    /// areas with how each was entered (the first has none) and the total
    /// cost, or None when unreachable.
    pub fn find_path(&self, start: usize, goal: usize) -> Option<(Vec<(usize, Option<Via>)>, f64)> {
        self.find_path_with(start, goal, |from, to, via| self.step_cost(from, to, via))
    }

    /// The step cost's distance part: ladder length or centre distance.
    pub fn step_length(&self, from: usize, to: usize, via: Via) -> f64 {
        match via {
            Via::LadderUp(l) | Via::LadderDown(l) => self.ladders[l].length as f64,
            Via::Walk(_) => (self.areas[to].center.as_dvec3() - self.areas[from].center.as_dvec3()).length(),
        }
    }

    /// `find_path` with another step cost (`cost(from, to, via)`, the cost
    /// of entering `to`; infinite: impassable). The heuristic stays the
    /// straight distance, so costs should not fall below the distance.
    pub fn find_path_with(
        &self,
        start: usize,
        goal: usize,
        cost_of: impl Fn(usize, usize, Via) -> f64,
    ) -> Option<(Vec<(usize, Option<Via>)>, f64)> {
        let n = self.areas.len();
        let goal_pos = self.areas[goal].center.as_dvec3();
        let h = |i: usize| (self.areas[i].center.as_dvec3() - goal_pos).length();
        let mut g = vec![f64::INFINITY; n];
        let mut parent: Vec<Option<(usize, Via)>> = vec![None; n];
        // Open list sorted by f; equal f keeps insertion order (FIFO).
        let mut open: Vec<(f64, usize)> = vec![(h(start), start)];
        let mut is_open = vec![false; n];
        g[start] = 0.0;
        is_open[start] = true;
        while !open.is_empty() {
            let (_, cur) = open.remove(0);
            is_open[cur] = false;
            if cur == goal {
                let mut out = vec![(cur, parent[cur].map(|p| p.1))];
                let mut at = cur;
                while let Some((p, _)) = parent[at] {
                    out.push((p, parent[p].map(|q| q.1)));
                    at = p;
                }
                out.reverse();
                return Some((out, g[goal]));
            }
            for &(next, via) in &self.areas[cur].links {
                if next == cur || parent[cur].is_some_and(|(p, _)| p == next) {
                    continue;
                }
                let step = cost_of(cur, next, via);
                if !step.is_finite() {
                    continue;
                }
                let mut cost = g[cur] + step;
                cost = cost.max(1.00001 * g[cur] + 0.00001);
                if g[next] <= cost {
                    continue;
                }
                g[next] = cost;
                parent[next] = Some((cur, via));
                if is_open[next] {
                    open.retain(|e| e.1 != next);
                }
                let f = cost + h(next);
                let at = open.partition_point(|e| e.0 <= f);
                open.insert(at, (f, next));
                is_open[next] = true;
            }
        }
        None
    }

    /// Where to cross from area `from` into `to` across `side`, nearest to
    /// `p`: on `from`'s edge, within both areas' shared extent, kept off
    /// edges with no area beyond (spec "closest crossing point").
    pub fn crossing(&self, from: usize, to: usize, side: Side, p: Vec3) -> Vec3 {
        let (a, b) = (&self.areas[from], &self.areas[to]);
        let along_x = matches!(side, Side::MinZ | Side::MaxZ);
        let (lo_a, hi_a, lo_b, hi_b, fixed) = if along_x {
            let z = if side == Side::MinZ { a.min.y } else { a.max.y };
            (a.min.x, a.max.x, b.min.x, b.max.x, z)
        } else {
            let x = if side == Side::MinX { a.min.x } else { a.max.x };
            (a.min.y, a.max.y, b.min.y, b.max.y, x)
        };
        let mut lo = lo_a.max(lo_b);
        let mut hi = hi_a.min(hi_b);
        let (lo_side, hi_side) = if along_x {
            (Side::MinX, Side::MaxX)
        } else {
            (Side::MinZ, Side::MaxZ)
        };
        if !self.two_way(to, lo_side) {
            lo += EDGE_MARGIN;
        }
        if !self.two_way(to, hi_side) {
            hi -= EDGE_MARGIN;
        }
        if lo > hi {
            lo = (lo + hi) / 2.0;
            hi = lo;
        }
        let t = if along_x { p.x } else { p.z }.clamp(lo, hi);
        let (x, z) = if along_x { (t, fixed) } else { (fixed, t) };
        Vec3::new(x, a.height_at(x, z), z)
    }

    /// Whether some neighbour across `side` of `area` links back to it.
    fn two_way(&self, area: usize, side: Side) -> bool {
        self.areas[area].links.iter().any(|&(n, via)| {
            via == Via::Walk(side)
                && self.areas[n]
                    .links
                    .iter()
                    .any(|&(m, v)| m == area && v == Via::Walk(side.opposite()))
        })
    }

    /// World points to walk through from `from` to `to` (engine space):
    /// crossing points between consecutive areas, ladder ends, then `to`.
    pub fn route(&self, from: Vec3, to: Vec3) -> Option<Vec<Vec3>> {
        let start = self.nearest_area(from)?;
        let goal = self.nearest_area(to)?;
        let (areas, _) = self.find_path(start, goal)?;
        Some(
            self.points_along(&areas, from, to, |_, _, _| 0.0)
                .into_iter()
                .map(|p| p.0)
                .collect(),
        )
    }

    /// World points (with the area each one leads into) to walk along an
    /// area path from `from` to `to`, as `route` makes them. `lean(from,
    /// to)` (-1..1) shifts each floor crossing from the nearest point
    /// toward one end of the shared edge (0: nearest; used to spread
    /// walkers over wide openings); it also gets the edge's length.
    pub fn points_along(
        &self,
        areas: &[(usize, Option<Via>)],
        from: Vec3,
        to: Vec3,
        lean: impl Fn(usize, usize, f32) -> f32,
    ) -> Vec<(Vec3, usize)> {
        let mut points = Vec::new();
        let mut at = from;
        for w in areas.windows(2) {
            let (prev, (next, via)) = (w[0].0, w[1]);
            let p = match via {
                Some(Via::Walk(side)) => {
                    let near = self.crossing(prev, next, side, at);
                    let (lo, hi) = self.portal_ends(prev, next, side);
                    let l = lean(prev, next, lo.distance(hi)).clamp(-1.0, 1.0);
                    let end = if l < 0.0 { lo } else { hi };
                    near.lerp(end, l.abs())
                }
                Some(Via::LadderUp(l)) => {
                    let ladder = &self.ladders[l];
                    points.push((ladder.bottom + ladder.normal * 32.0 * UNIT, prev));
                    ladder.top
                }
                Some(Via::LadderDown(l)) => {
                    let ladder = &self.ladders[l];
                    points.push((ladder.top - ladder.normal * 32.0 * UNIT, prev));
                    ladder.bottom
                }
                None => continue,
            };
            points.push((p, next));
            at = p;
        }
        points.push((to, areas.last().map_or(0, |a| a.0)));
        points
    }

    /// The two ends of the crossing range from `from` into `to` across
    /// `side` (as `crossing` clamps to), on `from`'s edge.
    pub fn portal_ends(&self, from: usize, to: usize, side: Side) -> (Vec3, Vec3) {
        let far = Vec3::splat(1e6);
        (self.crossing(from, to, side, -far), self.crossing(from, to, side, far))
    }

    /// Areas reachable from `start` within `max` of path length (centre
    /// distances, ladders by length), with their distance, nearest first.
    pub fn within(&self, start: usize, max: f32) -> Vec<(usize, f32)> {
        let mut dist = vec![f32::INFINITY; self.areas.len()];
        let mut out = Vec::new();
        let mut open = vec![(0.0f32, start)];
        dist[start] = 0.0;
        while let Some(i) = open
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.0.total_cmp(&b.1.0))
            .map(|(i, _)| i)
        {
            let (d, cur) = open.swap_remove(i);
            if d > dist[cur] {
                continue;
            }
            out.push((cur, d));
            for &(next, via) in &self.areas[cur].links {
                let nd = d + self.step_length(cur, next, via) as f32;
                if nd <= max && nd < dist[next] {
                    dist[next] = nd;
                    open.push((nd, next));
                }
            }
        }
        out
    }

    /// Whether area `b` is in `a`'s potentially visible set (any bits),
    /// or None when the mesh has no visibility data for `a`.
    pub fn sees(&self, a: usize, b: usize) -> Option<bool> {
        let list = &self.areas[a].visible;
        if list.is_empty() {
            return None;
        }
        Some(a == b || list.binary_search_by_key(&b, |v| v.0).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(min: Vec2, max: Vec2, heights: [f32; 4]) -> NavArea {
        NavArea {
            id: 0,
            flags: 0,
            min,
            max,
            heights,
            center: Vec3::ZERO,
            links: Vec::new(),
            place: None,
            hiding: Vec::new(),
            visible: Vec::new(),
            encounters: Vec::new(),
        }
    }

    /// Overlapping rectangles of many sizes on several floors, from a
    /// fixed sequence.
    fn scattered(n: usize) -> NavMesh {
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed >> 40) as f32 / (1u64 << 24) as f32
        };
        let areas = (0..n)
            .map(|_| {
                let min = Vec2::new(next() * 200.0 - 100.0, next() * 120.0 - 60.0);
                // Whole quarter metres, so many edges and corners coincide.
                let min = (min * 4.0).round() / 4.0;
                let size = Vec2::new((next() * 24.0).round() / 4.0, (next() * 24.0).round() / 4.0);
                let y = (next() * 4.0).floor() * 3.0;
                area(min, min + size, [y, y + next(), y + next(), y])
            })
            .collect();
        NavMesh { areas, ..default() }
    }

    #[test]
    fn the_grid_finds_what_a_search_of_every_area_finds() {
        let mut mesh = scattered(3000);
        let mut points: Vec<Vec3> = Vec::new();
        for a in mesh.areas.iter().step_by(7) {
            // Corners, edges, the middle, and just off each.
            for (x, z) in [
                (a.min.x, a.min.y),
                (a.max.x, a.max.y),
                (a.min.x, a.max.y),
                ((a.min.x + a.max.x) / 2.0, a.max.y),
            ] {
                for dy in [-2.0, 0.05, 1.0, 3.5] {
                    points.push(Vec3::new(x, a.height_at(x, z) + dy, z));
                    points.push(Vec3::new(x + 1e-4, a.height_at(x, z) + dy, z - 1e-4));
                }
            }
        }
        points.extend([
            Vec3::splat(1e6),
            Vec3::splat(-1e6),
            Vec3::new(f32::NAN, 0.0, 0.0),
            Vec3::new(-100.0, 0.0, -60.0),
        ]);
        let every = |m: &NavMesh, p: Vec3| m.highest_at(p, 0..m.areas.len());
        let found = points.iter().filter(|p| mesh.area_at(**p).is_some()).count();
        assert!(
            found > points.len() / 3,
            "{found} of {} points over an area",
            points.len()
        );
        for p in &points {
            assert_eq!(mesh.area_at(*p), every(&mesh, *p), "at {p}");
        }
        let grid = mesh.grid.get().unwrap();
        assert!(
            grid.over(&mesh.areas, 0.0, 0.0).is_some_and(|over| over.len() < 300),
            "a cell holds a few of the areas"
        );
        // A copy asks its own grid; areas added later are found too.
        let copy = mesh.clone();
        assert!(copy.grid.get().is_none());
        assert_eq!(copy.area_at(points[0]), mesh.area_at(points[0]));
        assert!(copy.grid.get().unwrap().over(&copy.areas, 0.0, 0.0).is_some());
        let far = Vec2::new(500.0, 500.0);
        mesh.areas.push(area(far, far + Vec2::ONE, [0.0; 4]));
        assert_eq!(mesh.area_at(Vec3::new(500.5, 0.1, 500.5)), Some(3000));
        for p in points.iter().step_by(11) {
            assert_eq!(mesh.area_at(*p), every(&mesh, *p), "at {p}");
        }
    }

    #[test]
    fn the_nearest_area_is_the_nearest_of_every_area() {
        let mesh = scattered(800);
        let nearest = |m: &NavMesh, p: Vec3| {
            m.areas
                .iter()
                .enumerate()
                .map(|(i, a)| (i, a.closest_point(p).distance_squared(p)))
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        };
        let mut off = 0;
        for k in 0..4000 {
            // Over, beside, above, below and far outside the areas.
            let f = k as f32;
            let p = Vec3::new(
                (f * 0.618).fract() * 260.0 - 130.0,
                (f * 0.371).fract() * 60.0 - 30.0,
                (f * 0.137).fract() * 180.0 - 90.0,
            );
            let p = if k % 97 == 0 { p * 40.0 } else { p };
            let Some(i) = mesh.nearest_area(p) else {
                panic!("an area for {p}")
            };
            if mesh.area_at(p).is_none() {
                off += 1;
                let (want, d) = (nearest(&mesh, p).unwrap(), |i: usize| {
                    mesh.areas[i].closest_point(p).distance_squared(p)
                });
                assert_eq!(i, want, "at {p}: {} m against {} m", d(i).sqrt(), d(want).sqrt());
            }
        }
        assert!(off > 1000, "{off} points over no area");
    }

    #[test]
    fn no_areas_and_one_point_area() {
        assert_eq!(NavMesh::default().area_at(Vec3::ZERO), None);
        let mesh = NavMesh {
            areas: vec![area(Vec2::ONE, Vec2::ONE, [0.0; 4])],
            ..default()
        };
        assert_eq!(mesh.area_at(Vec3::new(1.0, 0.1, 1.0)), Some(0));
        assert_eq!(mesh.area_at(Vec3::new(1.5, 0.1, 1.0)), None);
    }
}
