//! Navigation meshes generated from a map's collision, for maps that ship
//! none (`nav::NavMesh` is otherwise a game's file, CS:S's `.nav`): a
//! flood fill over a square grid from seed points (spawns), each cell's
//! floor found by a ray down, kept when a standing box fits above it and
//! nothing stands within the walker's radius, neighbours linked when a box
//! sweeps from one cell to the next at step height (up a step, or down a
//! drop as a one-way link); then the cells merged into axis-aligned
//! rectangles of nearly level, mutually linked cells (`NavArea`s with
//! their corner heights) and the cell links turned into area links
//! (`Via::Walk` across the side they leave by), marked
//! `NavMesh::generated`. Game-neutral: the walker's
//! sizes are `NavGenParams`, the colliders that count are the caller's
//! (`generate`'s `ignore`).
//!
//! Bounded (`generate_within`, `Bounds`): on a map with visibility data a
//! walker that climbs more than a player walks (a bot's 0.8 m jump-ups, a
//! narrower hull) can leave the playable space, over or through its
//! walls (de_aztec's jungle behind a gap a player doesn't fit, `heavy::
//! nav_bounds` in party_kit), and the jungle is in valid clusters too. So
//! a bounded mesh keeps only cells whose chest (1 m up) is in a cluster
//! a walking player (`walking_player`) reaches from the same seeds:
//! jump-up ledges inside the rooms a player walks stay, anything beyond
//! the rooms goes, and so does whatever is reached only through it.
//!
//! A whole flood is thousands of millions of instructions on a large map
//! (cs_office: about 4 G), so a front end runs it off the main thread
//! (`NavGenTask`: a task flooding a slice at a time against a copy of the
//! world's collision taken once, `CollisionSnapshot`), or a slice at a time
//! on it (`NavGenJob`, docs/performance.md, "Nothing blocks the main
//! thread"); the result is the same (`FloodQueries`: the same queries on
//! the live world or the copy).
//!
//! Known shortcuts: no ladders, jumps, crouch-only areas, hiding spots or
//! visibility data; cells are tested at their centres only (a wall
//! thinner than a cell between two cell centres is found by the sweep,
//! a pillar between centres narrower than the radius is not); areas'
//! corner heights are the corner cells' floors (a ramp is a staircase of
//! areas no wider than `max_area` cells). The bound is by clusters: a
//! jump-up ledge in a cluster no walk enters (a box top in a leaf of its
//! own, a wall's top) is dropped with the outside (de_dust2 loses about
//! 70 of its 4950 m2, cs_office 45 of 4010), and a jump over a low wall
//! into a walked cluster stays; a map without visibility data (Combat
//! Arms') isn't bounded; the walk is generated a second time for it.

use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};

use avian3d::{collider_tree::ColliderTrees, collision::collider::contact_query, prelude::*};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, TaskPool, futures_lite::future},
};

use super::{
    nav::{NavArea, NavMesh, Side, Via},
    vis::MapVisibility,
};

/// The walker and the grid, metres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavGenParams {
    /// Grid cell edge.
    pub cell: f32,
    /// Highest step walked up without a jump.
    pub step: f32,
    /// Standing height (feet to head).
    pub height: f32,
    /// Horizontal half-size of the walker.
    pub radius: f32,
    /// Steepest walkable floor: the floor normal's least `y`.
    pub min_normal_y: f32,
    /// Deepest drop walked off (one way).
    pub max_drop: f32,
    /// Stop growing the grid at this many cells.
    pub max_cells: usize,
    /// Merged areas are at most this many cells a side.
    pub max_area: u32,
    /// The physics layers the walker meets (a game's characters' filter,
    /// e.g. `core::CHARACTER_FILTER`: players-only clips but not
    /// traces-only surfaces).
    pub mask: LayerMask,
}

impl Default for NavGenParams {
    fn default() -> Self {
        Self {
            cell: 0.5,
            step: 0.45,
            height: 1.8,
            radius: 0.3,
            min_normal_y: 0.7,
            max_drop: 2.0,
            max_cells: 300_000,
            max_area: 8,
            mask: crate::core::CHARACTER_FILTER,
        }
    }
}

/// A walking player: the default step and height, a little wider than a
/// CS:S player (radius 0.4 m), walks off drops up to 3 m; no jumps.
pub fn walking_player() -> NavGenParams {
    NavGenParams {
        radius: 0.4,
        max_drop: 3.0,
        ..Default::default()
    }
}

/// What bounds a generated mesh to the map's playable space
/// (`generate_within`): the map's visibility tree and the walker whose
/// reach marks the playable clusters.
#[derive(Clone, Copy)]
pub struct Bounds<'a> {
    pub vis: &'a MapVisibility,
    pub walker: NavGenParams,
}

/// The height above a cell's floor at which its cluster is taken (a
/// standing character's chest, as `spawns::inside_map`).
const CHEST: f32 = 1.0;

/// One grid cell's floor.
#[derive(Clone, Copy, Debug)]
struct Cell {
    ix: i32,
    iz: i32,
    y: f32,
}

const DIRS: [(i32, i32, Side); 4] = [
    (1, 0, Side::MaxX),
    (-1, 0, Side::MinX),
    (0, 1, Side::MaxZ),
    (0, -1, Side::MinZ),
];

/// The three spatial queries a flood makes, answered by the live world
/// (`SpatialQuery`) or by a copy of its collision (`CollisionSnapshot`, on
/// a task): the same answers from either.
pub trait FloodQueries {
    /// The closest hit of a solid ray: its distance and normal.
    fn ray_hit(&self, origin: Vec3, dir: Dir3, max: f32, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> Option<(f32, Vec3)>;
    /// Whether `shape` at `at` (unrotated) overlaps a collider.
    fn overlaps(&self, shape: &Collider, at: Vec3, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> bool;
    /// Whether `shape` swept from `at` along `dir` for `max` meets a collider.
    fn sweep_hits(&self, shape: &Collider, at: Vec3, dir: Dir3, max: f32, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> bool;
}

impl FloodQueries for SpatialQuery<'_, '_> {
    fn ray_hit(&self, origin: Vec3, dir: Dir3, max: f32, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> Option<(f32, Vec3)> {
        self.cast_ray_predicate(origin, dir, max, true, filter, &|e| !ignore(e)).map(|h| (h.distance, h.normal))
    }

    fn overlaps(&self, shape: &Collider, at: Vec3, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> bool {
        let mut blocked = false;
        self.shape_intersections_callback(shape, at, Quat::IDENTITY, filter, |e| {
            if ignore(e) {
                true
            } else {
                blocked = true;
                false
            }
        });
        blocked
    }

    fn sweep_hits(&self, shape: &Collider, at: Vec3, dir: Dir3, max: f32, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> bool {
        let config = ShapeCastConfig::from_max_distance(max);
        self.cast_shape_predicate(shape, at, Quat::IDENTITY, dir, &config, filter, &|e| !ignore(e)).is_some()
    }
}

/// The world's collision at one moment, for a flood on a task: Avian's
/// collider trees (`ColliderTrees`, cloned) and each collider's shape and
/// pose (shapes are shared, not copied). The queries walk the same trees
/// and test the same shapes as `SpatialQuery`'s.
pub struct CollisionSnapshot {
    trees: ColliderTrees,
    colliders: HashMap<Entity, (Vec3, Quat, Collider)>,
}

impl CollisionSnapshot {
    pub fn of(world: &mut World) -> Self {
        let trees = world.get_resource::<ColliderTrees>().cloned().unwrap_or_default();
        let colliders = world
            .query::<(Entity, &Position, &Rotation, &Collider)>()
            .iter(world)
            .map(|(e, p, r, c)| (e, (p.0, r.0, c.clone())))
            .collect();
        Self { trees, colliders }
    }

    /// The collider behind a tree's proxy, if the filter and `ignore` take it.
    fn collider(
        &self,
        proxy: Option<&avian3d::collider_tree::ColliderTreeProxy>,
        filter: &SpatialQueryFilter,
        ignore: &dyn Fn(Entity) -> bool,
    ) -> Option<&(Vec3, Quat, Collider)> {
        let proxy = proxy?;
        if !filter.test(proxy.collider, proxy.layers) || ignore(proxy.collider) {
            return None;
        }
        self.colliders.get(&proxy.collider)
    }
}

impl FloodQueries for CollisionSnapshot {
    fn ray_hit(&self, origin: Vec3, dir: Dir3, max: f32, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> Option<(f32, Vec3)> {
        let ray = Ray3d::new(origin, dir);
        let mut best = max;
        let mut hit = None;
        for tree in self.trees.iter_trees() {
            tree.ray_traverse_closest(ray, best, |id| {
                let Some((p, r, c)) = self.collider(tree.get_proxy(id), filter, ignore) else { return f32::MAX };
                let Some((d, n)) = c.cast_ray(*p, *r, origin, dir.as_vec3(), best, true) else { return f32::MAX };
                if d < best {
                    best = d;
                    hit = Some((d, n));
                }
                d
            });
        }
        hit
    }

    fn overlaps(&self, shape: &Collider, at: Vec3, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> bool {
        let aabb = shape.aabb(at, Quat::IDENTITY, 0.0);
        let mut blocked = false;
        for tree in self.trees.iter_trees() {
            tree.aabb_traverse(aabb.into(), |id| {
                let Some((p, r, c)) = self.collider(tree.get_proxy(id), filter, ignore) else { return true };
                blocked = contact_query::intersection_test(c, *p, *r, shape, at, Quat::IDENTITY).unwrap_or(false);
                !blocked
            });
            if blocked {
                return true;
            }
        }
        false
    }

    fn sweep_hits(&self, shape: &Collider, at: Vec3, dir: Dir3, max: f32, filter: &SpatialQueryFilter, ignore: &dyn Fn(Entity) -> bool) -> bool {
        let aabb = shape.aabb(at, Quat::IDENTITY, 0.0);
        let mut hit = false;
        for tree in self.trees.iter_trees() {
            tree.sweep_traverse_closest(aabb.into(), dir, max, 0.0, |id| {
                let Some((p, r, c)) = self.collider(tree.get_proxy(id), filter, ignore) else { return f32::MAX };
                match contact_query::time_of_impact(c, *p, *r, Vec3::ZERO, shape, at, Quat::IDENTITY, dir.as_vec3(), max) {
                    Ok(Some(t)) => {
                        hit = true;
                        // Any hit answers: nothing nearer is needed.
                        t.time_of_impact.min(0.0)
                    }
                    _ => f32::MAX,
                }
            });
            if hit {
                return true;
            }
        }
        false
    }
}

struct Grid<'a> {
    q: &'a dyn FloodQueries,
    filter: SpatialQueryFilter,
    ignore: &'a dyn Fn(Entity) -> bool,
    p: NavGenParams,
    cells: Vec<Cell>,
    /// Cells of each column (by index), lowest first not guaranteed.
    columns: HashMap<(i32, i32), Vec<usize>>,
    /// Outgoing links per cell: (cell, side).
    links: Vec<Vec<(usize, Side)>>,
    /// Cells whose neighbours are yet to be looked at.
    open: VecDeque<usize>,
}

/// A `Grid`'s state between slices of a `NavGenJob`.
#[derive(Default)]
struct Flood {
    cells: Vec<Cell>,
    columns: HashMap<(i32, i32), Vec<usize>>,
    links: Vec<Vec<(usize, Side)>>,
    open: VecDeque<usize>,
}

impl Grid<'_> {
    fn centre(&self, ix: i32, iz: i32) -> (f32, f32) {
        ((ix as f32 + 0.5) * self.p.cell, (iz as f32 + 0.5) * self.p.cell)
    }

    /// The walkable floor below `from_y` within `depth` at a column's
    /// centre (None: no floor, or too steep).
    fn floor(&self, ix: i32, iz: i32, from_y: f32, depth: f32) -> Option<f32> {
        let (x, z) = self.centre(ix, iz);
        let (distance, normal) = self.q.ray_hit(Vec3::new(x, from_y, z), Dir3::NEG_Y, depth, &self.filter, self.ignore)?;
        (normal.y >= self.p.min_normal_y).then_some(from_y - distance)
    }

    /// Whether a walker stands on the floor `y` of a column: room above it
    /// (from the step up to the head) and no wall within the radius.
    fn stands(&self, ix: i32, iz: i32, y: f32) -> bool {
        let (x, z) = self.centre(ix, iz);
        let half = (self.p.height - self.p.step) * 0.5;
        let shape = Collider::cuboid(self.p.radius * 2.0, half * 2.0, self.p.radius * 2.0);
        !self.q.overlaps(&shape, Vec3::new(x, y + self.p.step + half, z), &self.filter, self.ignore)
    }

    /// Whether a walker passes from one column's floor `y` to the next
    /// column (a box at step height swept the cell's length).
    fn passes(&self, ix: i32, iz: i32, y: f32, dx: i32, dz: i32) -> bool {
        let (x, z) = self.centre(ix, iz);
        let half = (self.p.height - self.p.step) * 0.5 - 0.02;
        let shape = Collider::cuboid(self.p.radius, half * 2.0, self.p.radius);
        let dir = Dir3::new(Vec3::new(dx as f32, 0.0, dz as f32)).unwrap();
        !self.q.sweep_hits(&shape, Vec3::new(x, y + self.p.step + 0.02 + half, z), dir, self.p.cell, &self.filter, self.ignore)
    }

    /// The cell of column (ix, iz) whose floor is within a step of `y`.
    fn find(&self, ix: i32, iz: i32, y: f32) -> Option<usize> {
        self.columns
            .get(&(ix, iz))?
            .iter()
            .copied()
            .find(|&i| (self.cells[i].y - y).abs() < self.p.step * 0.5)
    }

    fn add(&mut self, ix: i32, iz: i32, y: f32) -> usize {
        self.cells.push(Cell { ix, iz, y });
        self.links.push(Vec::new());
        let i = self.cells.len() - 1;
        self.columns.entry((ix, iz)).or_default().push(i);
        i
    }
}

/// A navigation mesh over the floors reachable from `seeds` (points
/// standing on or just above a floor, metres), by the colliders the
/// spatial query sees but those `ignore` says to (characters, items).
pub fn generate(
    q: &dyn FloodQueries,
    ignore: &dyn Fn(Entity) -> bool,
    seeds: &[Vec3],
    p: &NavGenParams,
) -> NavMesh {
    generate_within(q, ignore, seeds, p, None)
}

/// `generate`, and with `bounds` only the cells in the clusters a
/// walking player reaches from the same seeds (the module doc; without
/// visibility data, as `generate`).
pub fn generate_within(
    q: &dyn FloodQueries,
    ignore: &dyn Fn(Entity) -> bool,
    seeds: &[Vec3],
    p: &NavGenParams,
    bounds: Option<Bounds>,
) -> NavMesh {
    let mut job = NavGenJob::new(seeds, p, bounds.map(|b| b.walker));
    loop {
        if let Some(mesh) = job.step(q, ignore, bounds.map(|b| b.vis), usize::MAX) {
            return mesh;
        }
    }
}

/// A front end floods off the main thread (`NavGenTask`) or a slice a tick
/// (`NavGenJob`), so a mesh made in play stops no frame; without it (a
/// dedicated server, a test) a flood runs whole. The client shell inserts it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct SliceFloods;

/// Cells of a flood a tick takes where floods are sliced (`SliceFloods`):
/// about 200 k instructions each.
pub const CELLS_PER_TICK: usize = 80;

/// Cells a frame takes behind the loading screen (`MapHooks::settle`): about
/// 30 M instructions, so that with the settling's other slices (glyphs,
/// prop shadows) a frame stays under 33 ms (250 cells' 50 M did not: docs/performance.md,
/// "A map's swap in the real app").
pub const CELLS_PER_LOAD_FRAME: usize = 160;

/// `generate_within` a slice at a time: `step` takes up to that many cells of
/// the flood, and gives the mesh at the end (the same mesh, whatever the
/// slices).
pub struct NavGenJob {
    seeds: Vec<Vec3>,
    p: NavGenParams,
    /// The bound's walker (`Bounds`), its flood first.
    walker: Option<NavGenParams>,
    walk: Flood,
    /// What the walker's flood reached (`Bounds`), once it is done.
    clusters: Option<std::collections::BTreeSet<u32>>,
    own: Flood,
}

impl NavGenJob {
    pub fn new(seeds: &[Vec3], p: &NavGenParams, walker: Option<NavGenParams>) -> Self {
        Self {
            seeds: seeds.to_vec(),
            p: *p,
            walker,
            walk: Flood::default(),
            clusters: None,
            own: Flood::default(),
        }
    }

    /// Up to `cells` cells more (`vis`: the map's visibility, the bound's
    /// clusters; none or empty: no bound); the mesh when the flood is done.
    pub fn step(
        &mut self,
        q: &dyn FloodQueries,
        ignore: &dyn Fn(Entity) -> bool,
        vis: Option<&MapVisibility>,
        cells: usize,
    ) -> Option<NavMesh> {
        let vis = vis.filter(|v| !v.nodes.is_empty());
        if let (Some(walker), Some(vis), None) = (self.walker, vis, &self.clusters) {
            let mut walk = Grid::resume(q, ignore, &walker, std::mem::take(&mut self.walk));
            if !walk.flood_some(&self.seeds, &|_| true, cells) {
                self.walk = walk.pause();
                return None;
            }
            self.clusters = Some(
                walk.cells
                    .iter()
                    .filter_map(|c| {
                        let (x, z) = walk.centre(c.ix, c.iz);
                        vis.cluster_at(Vec3::new(x, c.y + CHEST, z))
                    })
                    .collect(),
            );
            return None;
        }
        let set = self.clusters.as_ref().zip(vis);
        let keep = |at: Vec3| set.is_none_or(|(set, vis)| vis.cluster_at(at + Vec3::Y * CHEST).is_some_and(|c| set.contains(&c)));
        let mut g = Grid::resume(q, ignore, &self.p, std::mem::take(&mut self.own));
        if !g.flood_some(&self.seeds, &keep, cells) {
            self.own = g.pause();
            return None;
        }
        Some(merge(&g))
    }
}

/// Cells a task floods between two looks at whether it was dropped.
const CELLS_PER_TASK_SLICE: usize = 500;

/// A `NavGenJob` run to its end on a task (`AsyncComputeTaskPool`) against a
/// `CollisionSnapshot` (the colliders `ignore` names left out, as
/// `generate_within`'s), the main thread only polling: the same mesh as on
/// the world. Dropping it stops the flood at its next slice (a map's
/// change).
pub struct NavGenTask(Task<NavMesh>);

impl NavGenTask {
    pub fn spawn(mut job: NavGenJob, snapshot: CollisionSnapshot, ignore: HashSet<Entity>, vis: Option<Arc<MapVisibility>>) -> Self {
        let pool = AsyncComputeTaskPool::get_or_init(TaskPool::default);
        Self(pool.spawn(async move {
            loop {
                if let Some(mesh) = job.step(&snapshot, &|e| ignore.contains(&e), vis.as_deref(), CELLS_PER_TASK_SLICE) {
                    return mesh;
                }
                // Other tasks get their turn, and a dropped task stops here.
                future::yield_now().await;
            }
        }))
    }

    /// The mesh, once the task is done (with `wait`, waits for it:
    /// `chimera_core::tasks::WaitForTasks`).
    pub fn poll(&mut self, wait: bool) -> Option<NavMesh> {
        chimera_core::tasks::poll(&mut self.0, wait)
    }
}

impl<'a> Grid<'a> {
    fn resume(q: &'a dyn FloodQueries, ignore: &'a dyn Fn(Entity) -> bool, p: &NavGenParams, f: Flood) -> Self {
        Grid {
            q,
            filter: SpatialQueryFilter::from_mask(p.mask),
            ignore,
            p: *p,
            cells: f.cells,
            columns: f.columns,
            links: f.links,
            open: f.open,
        }
    }

    fn pause(self) -> Flood {
        Flood {
            cells: self.cells,
            columns: self.columns,
            links: self.links,
            open: self.open,
        }
    }

    /// Up to `budget` more of the cells reachable from `seeds` through cells
    /// whose floor point `keep` takes (seeds are always taken, the first
    /// call): true when there are no more.
    fn flood_some(&mut self, seeds: &[Vec3], keep: &dyn Fn(Vec3) -> bool, mut budget: usize) -> bool {
        let p = self.p;
        if self.cells.is_empty() {
            for s in seeds {
                let (ix, iz) = ((s.x / p.cell).floor() as i32, (s.z / p.cell).floor() as i32);
                let Some(y) = self.floor(ix, iz, s.y + 0.5, 4.0) else { continue };
                if self.find(ix, iz, y).is_some() || !self.stands(ix, iz, y) {
                    continue;
                }
                let i = self.add(ix, iz, y);
                self.open.push_back(i);
            }
        }
        while budget > 0 {
            let Some(i) = self.open.pop_front() else { return true };
            budget -= 1;
            let Cell { ix, iz, y } = self.cells[i];
            for (dx, dz, side) in DIRS {
                let (jx, jz) = (ix + dx, iz + dz);
                let top = y + p.step + 0.05;
                let Some(y1) = self.floor(jx, jz, top, p.step + 0.05 + p.max_drop) else { continue };
                if !self.passes(ix, iz, y, dx, dz) {
                    continue;
                }
                let j = match self.find(jx, jz, y1) {
                    Some(j) => j,
                    None => {
                        let (x, z) = self.centre(jx, jz);
                        if self.cells.len() >= p.max_cells || !keep(Vec3::new(x, y1, z)) || !self.stands(jx, jz, y1) {
                            continue;
                        }
                        let j = self.add(jx, jz, y1);
                        self.open.push_back(j);
                        j
                    }
                };
                if !self.links[i].iter().any(|l| l.0 == j) {
                    self.links[i].push((j, side));
                }
            }
        }
        self.open.is_empty()
    }
}

/// Cells into rectangles and their links into area links.
fn merge(g: &Grid<'_>) -> NavMesh {
    let n = g.cells.len();
    let linked = |a: usize, b: usize| g.links[a].iter().any(|l| l.0 == b) && g.links[b].iter().any(|l| l.0 == a);
    let level = |a: usize, b: usize| (g.cells[a].y - g.cells[b].y).abs() <= g.p.step * 0.5;
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by_key(|&i| (g.cells[i].iz, g.cells[i].ix));
    let mut area_of = vec![usize::MAX; n];
    let mut rects: Vec<(i32, i32, i32, i32, [usize; 4])> = Vec::new();
    let cell_at = |ix: i32, iz: i32, near: usize, area_of: &[usize]| -> Option<usize> {
        let c = g.find(ix, iz, g.cells[near].y)?;
        (area_of[c] == usize::MAX).then_some(c)
    };
    for &a in &order {
        if area_of[a] != usize::MAX {
            continue;
        }
        let Cell { ix, iz, .. } = g.cells[a];
        // Grow along +x.
        let mut row = vec![a];
        while (row.len() as u32) < g.p.max_area {
            let last = *row.last().unwrap();
            let Some(c) = cell_at(ix + row.len() as i32, iz, last, &area_of) else { break };
            if !linked(last, c) || !level(a, c) || row.contains(&c) {
                break;
            }
            row.push(c);
        }
        // Then rows along +z while every cell joins.
        let mut rows = vec![row];
        while (rows.len() as u32) < g.p.max_area {
            let prev = rows.last().unwrap();
            let mut next = Vec::with_capacity(prev.len());
            for (k, &above) in prev.iter().enumerate() {
                let Some(c) = cell_at(ix + k as i32, iz + rows.len() as i32, above, &area_of) else { break };
                if !linked(above, c) || !level(a, c) || (k > 0 && !linked(next[k - 1], c)) {
                    break;
                }
                next.push(c);
            }
            if next.len() != prev.len() {
                break;
            }
            rows.push(next);
        }
        let id = rects.len();
        for r in &rows {
            for &c in r {
                area_of[c] = id;
            }
        }
        let (w, h) = (rows[0].len(), rows.len());
        let corners = [rows[0][0], rows[0][w - 1], rows[h - 1][w - 1], rows[h - 1][0]];
        rects.push((ix, iz, ix + w as i32, iz + h as i32, corners));
    }
    let s = g.p.cell;
    let mut areas: Vec<NavArea> = rects
        .iter()
        .enumerate()
        .map(|(id, &(x0, z0, x1, z1, corners))| {
            let heights = corners.map(|c| g.cells[c].y);
            let min = Vec2::new(x0 as f32 * s, z0 as f32 * s);
            let max = Vec2::new(x1 as f32 * s, z1 as f32 * s);
            let mid = (min + max) * 0.5;
            NavArea {
                id: id as u32,
                flags: 0,
                min,
                max,
                heights,
                center: Vec3::new(mid.x, (heights[0] + heights[2]) * 0.5, mid.y),
                links: Vec::new(),
                place: None,
                hiding: Vec::new(),
                visible: Vec::new(),
                encounters: Vec::new(),
            }
        })
        .collect();
    for (i, links) in g.links.iter().enumerate() {
        for &(j, side) in links {
            let (a, b) = (area_of[i], area_of[j]);
            if a == b || a == usize::MAX || b == usize::MAX {
                continue;
            }
            let link = (b, Via::Walk(side));
            if !areas[a].links.contains(&link) {
                areas[a].links.push(link);
            }
        }
    }
    NavMesh {
        areas,
        ladders: Vec::new(),
        places: Vec::new(),
        generated: true,
        ..default()
    }
}
