//! The arena-slice candidate generator (docs/plans/active/arena-slices.md,
//! section 4 and A5): from a map's walkable mesh (the bots' areas as a
//! graph of cells linked by the width of the side they share), slices a
//! person can pick from. Plain functions over the graph, the spawn spots
//! and a line-of-sight test the caller gives, so `map_fitness
//! --candidates` (a physics ray) and the unit tests (toy maps drawn in
//! code, `apps/party_roster/tests/party/slice_gen.rs`) generate alike.
//!
//! - Seeds (`pick_seeds`): the map's spawn clusters' middles, then
//!   farthest-point sampling over the cells, each cell's distance to the
//!   seeds so far weighed up by how much solid stands near it (`score`:
//!   open ground is what fails, so slices start where there is cover).
//! - Growth and the cut (`grow`): Dijkstra over walking distance from the
//!   seed until the cells reached sum to 0.7 of the target area (the core)
//!   and on to 1.4 (the outer ring); a minimum cut (Dinic's) between the
//!   core and what lies past the ring, each link's capacity the width of
//!   the side the two cells share, so the region ends at the narrowest
//!   necks between the two (a side with no neighbour, a wall or a drop,
//!   costs nothing).
//! - The outline (`outline`): the region drawn on a 0.5 m raster, closed
//!   (2 m), its holes filled, its largest piece traced and simplified
//!   (Douglas-Peucker, the tolerance grown until it has at most
//!   `max_corners` corners and is a simple polygon). Membership is then the
//!   polygon's (the cells whose centre is inside); a candidate whose
//!   area moves more than 25% from the cut's is dropped (the plan's 15%
//!   and the outline's 1 m margin) (it took floors
//!   over or under the region, or cut a building in plan).
//! - The wall's cost (`wall`): each edge sampled every 0.5 m; a sample over
//!   walkable floor is open wall (`open_m`; a player meets an invisible
//!   wall in the field), one with a roof over it or a floor above another
//!   cuts a building (`indoor_m`: refused over 1 m: never through a door or
//!   a stair; a floor over another, `stacked_m`, is told).
//! - Measuring (`measure`): the A3 measures inside the polygon
//!   (`slice_safety`), the line of sight stopped by the polygon (the wall is
//!   solid), on the spawn points the slice would have: the map's inside,
//!   one at its seed, topped up by `choose_points` as the game does; judged
//!   by `[slice]` (`SliceLimits::judge`) and ranked by the pool's weight at
//!   the band's middle times `1 - seen_at_spawn` at its top times `1 -
//!   open_share / 2` (the plan's section 5).
//! - `generate`: every seed for a band (grown twice: at the map's sight,
//!   then at the first cut's), outlined, near-duplicates (over 80% of the
//!   smaller's area shared) left to the better ranked, measured, sorted.
//!
//! Known shortcuts: the outline is simplified both ways (inward too, not
//! only outward as the plan asks; a 1 m margin and the area check guard
//! it); the seed bias is
//! level rays from cell centres, not a clearance map; the candidates'
//! spawn points are seeded by a provisional key (`cand_<band>_<n>`), so the
//! slice a person picks makes other points in the game: `--slice` measures
//! the picked one as played.

use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use chimera_sim::minigame::{
    map_fit::{FitSettings, Fitness, SliceVerdict},
    slice_safety::{self, PointRules, Report, SeeTable},
};
use chimera_world::map::{
    nav::{NavMesh, Via},
    play_area::PlanPolygon,
};
use std::collections::{BTreeMap, BinaryHeap, VecDeque};

/// One walkable cell: a rectangle in plan view (x, z) and its floor's
/// height at the middle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cell {
    pub min: Vec2,
    pub max: Vec2,
    pub y: f32,
    /// Heights at (min.x, min.z), (max.x, min.z), (max.x, max.z), (min.x,
    /// max.z), as `NavArea`'s.
    pub heights: [f32; 4],
}

impl Cell {
    pub fn area(&self) -> f32 {
        let s = self.max - self.min;
        s.x * s.y
    }
    pub fn centre(&self) -> Vec2 {
        (self.min + self.max) / 2.0
    }
    pub fn floor(&self) -> Vec3 {
        let c = self.centre();
        Vec3::new(c.x, self.y, c.y)
    }
    /// The floor's height at `p` (bilinear over the corners).
    pub fn height_at(&self, p: Vec2) -> f32 {
        let size = self.max - self.min;
        if size.x <= 0.0 || size.y <= 0.0 {
            return self.y;
        }
        let u = ((p.x - self.min.x) / size.x).clamp(0.0, 1.0);
        let v = ((p.y - self.min.y) / size.y).clamp(0.0, 1.0);
        let [a, b, c, d] = self.heights;
        let near = a + u * (b - a);
        let far = d + u * (c - d);
        near + v * (far - near)
    }

    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.min.x - 0.01 && p.x <= self.max.x + 0.01 && p.y >= self.min.y - 0.01 && p.y <= self.max.y + 0.01
    }
}

/// The walkable cells and their links: (cell, width of the shared side, m).
#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub cells: Vec<Cell>,
    pub links: Vec<Vec<(usize, f32)>>,
    /// Each cell's walking cost per metre (1 by default; the tool makes
    /// open ground dearer, so a region fills the sheltered floor first).
    pub cost: Vec<f32>,
    /// Each cell's price for a cut through it, times the side's width (1 by
    /// default; the tool makes a roofed cell, a tunnel or a doorway, dear,
    /// so a wall stands there only if nothing else will do).
    pub cut_price: Vec<f32>,
}

/// The width (m) two touching rectangles share; a corner or an overlap (a
/// jump, a step between floors) counts as a narrow 0.25 m.
fn portal(a: &Cell, b: &Cell) -> f32 {
    const E: f32 = 0.05;
    let ox = a.max.x.min(b.max.x) - a.min.x.max(b.min.x);
    let oz = a.max.y.min(b.max.y) - a.min.y.max(b.min.y);
    if ((a.max.x - b.min.x).abs() < E || (b.max.x - a.min.x).abs() < E) && oz > E {
        return oz;
    }
    if ((a.max.y - b.min.y).abs() < E || (b.max.y - a.min.y).abs() < E) && ox > E {
        return ox;
    }
    0.25
}

impl Graph {
    /// The mesh's areas `ids` (its main region) as a graph: walk links
    /// between them only (teleporters and ladders left out: a slice cuts
    /// them).
    pub fn from_nav(nav: &NavMesh, ids: &[usize]) -> Self {
        let mut index = vec![usize::MAX; nav.areas.len()];
        for (k, i) in ids.iter().enumerate() {
            index[*i] = k;
        }
        let cells: Vec<Cell> = ids
            .iter()
            .map(|i| {
                let a = &nav.areas[*i];
                let c = (a.min + a.max) / 2.0;
                Cell {
                    min: a.min,
                    max: a.max,
                    y: a.height_at(c.x, c.y),
                    heights: a.heights,
                }
            })
            .collect();
        let links = ids
            .iter()
            .map(|i| {
                nav.areas[*i]
                    .links
                    .iter()
                    .filter(|(_, via)| matches!(via, Via::Walk(_)))
                    .filter_map(|(j, _)| (index[*j] != usize::MAX).then_some(index[*j]))
                    .map(|k| (k, portal(&cells[index[*i]], &cells[k])))
                    .collect()
            })
            .collect();
        let cost = vec![1.0; cells.len()];
        let cut_price = cost.clone();
        Self { cells, links, cost, cut_price }
    }

    /// A toy map drawn in text: each `.` (or a digit, a room's mark) a
    /// `size` m square cell, row r at z = r * size, column c at x = c *
    /// size; neighbours across a side linked (the unit tests' toy maps).
    #[allow(dead_code)]
    pub fn from_text(rows: &[&str], size: f32) -> Self {
        let mut at = BTreeMap::new();
        let mut cells = Vec::new();
        for (r, row) in rows.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                if ch != '#' && ch != ' ' {
                    at.insert((r as i64, c as i64), cells.len());
                    let min = Vec2::new(c as f32 * size, r as f32 * size);
                    cells.push(Cell {
                        min,
                        max: min + Vec2::splat(size),
                        y: 0.0,
                        heights: [0.0; 4],
                    });
                }
            }
        }
        let mut links = vec![Vec::new(); cells.len()];
        for (&(r, c), &i) in &at {
            for (dr, dc) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                if let Some(&j) = at.get(&(r + dr, c + dc)) {
                    links[i].push((j, size));
                }
            }
        }
        let cost = vec![1.0; cells.len()];
        let cut_price = cost.clone();
        Self { cells, links, cost, cut_price }
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Walking distance (centre to centre) from `from` to every cell.
    pub fn walk(&self, from: usize) -> Vec<f32> {
        let mut d = vec![f32::INFINITY; self.len()];
        let mut heap = BinaryHeap::new();
        d[from] = 0.0;
        heap.push(Item(0.0, from));
        while let Some(Item(at, i)) = heap.pop() {
            if at > d[i] {
                continue;
            }
            let p = self.cells[i].floor();
            for (j, _) in &self.links[i] {
                let next = at + p.distance(self.cells[*j].floor()) * (self.cost[i] + self.cost[*j]) / 2.0;
                if next < d[*j] {
                    d[*j] = next;
                    heap.push(Item(next, *j));
                }
            }
        }
        d
    }

    /// The area (m2) of the cells `mask` marks.
    pub fn area(&self, mask: &[bool]) -> f32 {
        self.cells.iter().zip(mask).filter(|(_, m)| **m).map(|(c, _)| c.area()).sum()
    }
}

#[derive(PartialEq)]
struct Item(f32, usize);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
    }
}

/// The cells over a point in plan view, through a grid of 4 m buckets.
pub struct CellIndex {
    lo: Vec2,
    nx: usize,
    nz: usize,
    buckets: Vec<Vec<usize>>,
}

const BUCKET: f32 = 4.0;

impl CellIndex {
    pub fn new(g: &Graph) -> Self {
        let (mut lo, mut hi) = (Vec2::MAX, Vec2::MIN);
        for c in &g.cells {
            lo = lo.min(c.min);
            hi = hi.max(c.max);
        }
        if g.is_empty() {
            (lo, hi) = (Vec2::ZERO, Vec2::ONE);
        }
        let nx = ((hi.x - lo.x) / BUCKET) as usize + 1;
        let nz = ((hi.y - lo.y) / BUCKET) as usize + 1;
        let mut buckets = vec![Vec::new(); nx * nz];
        for (i, c) in g.cells.iter().enumerate() {
            let (x0, z0) = (((c.min.x - lo.x) / BUCKET) as usize, ((c.min.y - lo.y) / BUCKET) as usize);
            let (x1, z1) = (((c.max.x - lo.x) / BUCKET) as usize, ((c.max.y - lo.y) / BUCKET) as usize);
            for x in x0..=x1.min(nx - 1) {
                for z in z0..=z1.min(nz - 1) {
                    buckets[z * nx + x].push(i);
                }
            }
        }
        Self { lo, nx, nz, buckets }
    }

    /// The cells whose rectangle holds `p`.
    pub fn at<'a>(&'a self, g: &'a Graph, p: Vec2) -> impl Iterator<Item = usize> + 'a {
        let q = (p - self.lo) / BUCKET;
        let ok = q.x >= 0.0 && q.y >= 0.0 && (q.x as usize) < self.nx && (q.y as usize) < self.nz;
        let list: &[usize] = if ok { &self.buckets[q.y as usize * self.nx + q.x as usize] } else { &[] };
        list.iter().copied().filter(move |i| g.cells[*i].contains(p))
    }
}

/// `n` seeds (module doc): `must` first (the spawn clusters' cells), then
/// the cell farthest in plan from those already chosen, its distance
/// weighed by `0.25 + score` (0 to 1, the solid near it). Deterministic.
pub fn pick_seeds(g: &Graph, score: &[f32], must: &[usize], n: usize) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    for m in must {
        if !out.contains(m) {
            out.push(*m);
        }
    }
    if g.is_empty() {
        return out;
    }
    if out.is_empty() {
        let best = (0..g.len()).max_by(|a, b| score[*a].total_cmp(&score[*b]).then(b.cmp(a))).unwrap_or(0);
        out.push(best);
    }
    let mut near = vec![f32::INFINITY; g.len()];
    let update = |near: &mut Vec<f32>, s: usize| {
        let p = g.cells[s].centre();
        for (i, c) in g.cells.iter().enumerate() {
            near[i] = near[i].min(c.centre().distance(p));
        }
    };
    for s in &out {
        update(&mut near, *s);
    }
    while out.len() < n {
        let key = |i: usize| near[i] * (0.25 + score[i].clamp(0.0, 1.0));
        let Some(best) = (0..g.len()).filter(|i| near[*i] > 1.0).max_by(|a, b| key(*a).total_cmp(&key(*b)).then(b.cmp(a))) else {
            break;
        };
        out.push(best);
        update(&mut near, best);
    }
    out
}

/// A grown and cut region (`grow`).
#[derive(Clone, Debug)]
pub struct Grown {
    /// The cells on the core's side of the cut.
    pub region: Vec<bool>,
    pub area_m2: f32,
    /// The cut's width (m): the sides closed between the region and the
    /// rest of the graph.
    pub cut_m: f32,
}

/// From `seed`, a region of about `target` m2 ending at the narrowest
/// necks between 0.7 and 1.4 of it (module doc). None: the seed's part of
/// the graph is smaller than 0.7 of the target.
pub fn grow(g: &Graph, seed: usize, target: f32) -> Option<Grown> {
    let d = g.walk(seed);
    let mut order: Vec<usize> = (0..g.len()).filter(|i| d[*i].is_finite()).collect();
    order.sort_by(|a, b| d[*a].total_cmp(&d[*b]).then(a.cmp(b)));
    // 0 beyond, 1 ring, 2 core.
    let mut tier = vec![0u8; g.len()];
    let mut acc = 0.0;
    let mut reached_outer = false;
    for i in &order {
        let t = if acc < 0.7 * target { 2 } else if acc < 1.4 * target { 1 } else {
            reached_outer = true;
            break;
        };
        tier[*i] = t;
        acc += g.cells[*i].area();
    }
    if acc < 0.7 * target {
        return None;
    }
    if !reached_outer {
        let region: Vec<bool> = tier.iter().map(|t| *t > 0).collect();
        return Some(Grown {
            area_m2: g.area(&region),
            region,
            cut_m: 0.0,
        });
    }
    // The flow network: the ring's cells, the source (the core), the sink
    // (beyond).
    let ring: Vec<usize> = (0..g.len()).filter(|i| tier[*i] == 1).collect();
    let mut node = vec![usize::MAX; g.len()];
    for (k, i) in ring.iter().enumerate() {
        node[*i] = k;
    }
    let (s, t) = (ring.len(), ring.len() + 1);
    let mut net = Dinic::new(ring.len() + 2);
    for i in 0..g.len() {
        if tier[i] == 0 {
            continue;
        }
        let from = if tier[i] == 2 { s } else { node[i] };
        for (j, w) in &g.links[i] {
            let to = match tier[*j] {
                2 => continue,
                1 => node[*j],
                _ => t,
            };
            if from == s && to == t {
                continue;
            }
            net.add(from, to, (*w * g.cut_price[i].max(g.cut_price[*j])) as f64);
        }
    }
    let cut = net.max_flow(s, t);
    let side = net.source_side(s);
    let region: Vec<bool> = (0..g.len()).map(|i| tier[i] == 2 || (tier[i] == 1 && side[node[i]])).collect();
    Some(Grown {
        area_m2: g.area(&region),
        region,
        cut_m: cut as f32,
    })
}

/// Dinic's maximum flow (a minimum cut's capacity), f64 capacities.
struct Dinic {
    to: Vec<usize>,
    cap: Vec<f64>,
    head: Vec<Vec<usize>>,
}

const EPS: f64 = 1e-9;

impl Dinic {
    fn new(n: usize) -> Self {
        Self {
            to: Vec::new(),
            cap: Vec::new(),
            head: vec![Vec::new(); n],
        }
    }

    fn add(&mut self, a: usize, b: usize, c: f64) {
        self.head[a].push(self.to.len());
        self.to.push(b);
        self.cap.push(c);
        self.head[b].push(self.to.len());
        self.to.push(a);
        self.cap.push(0.0);
    }

    fn levels(&self, s: usize) -> Vec<i32> {
        let mut level = vec![-1; self.head.len()];
        level[s] = 0;
        let mut q = VecDeque::from([s]);
        while let Some(u) = q.pop_front() {
            for e in &self.head[u] {
                let v = self.to[*e];
                if self.cap[*e] > EPS && level[v] < 0 {
                    level[v] = level[u] + 1;
                    q.push_back(v);
                }
            }
        }
        level
    }

    fn max_flow(&mut self, s: usize, t: usize) -> f64 {
        let mut flow = 0.0;
        loop {
            let level = self.levels(s);
            if level[t] < 0 {
                return flow;
            }
            let mut it = vec![0usize; self.head.len()];
            loop {
                let f = self.push(s, t, f64::INFINITY, &level, &mut it);
                if f <= EPS {
                    break;
                }
                flow += f;
            }
        }
    }

    /// One blocking-flow augmentation, iteratively (no deep recursion on
    /// long paths).
    fn push(&mut self, s: usize, t: usize, limit: f64, level: &[i32], it: &mut [usize]) -> f64 {
        let mut path: Vec<usize> = Vec::new();
        let mut u = s;
        loop {
            if u == t {
                let f = path.iter().map(|e| self.cap[*e]).fold(limit, f64::min);
                for e in &path {
                    self.cap[*e] -= f;
                    self.cap[*e ^ 1] += f;
                }
                return f;
            }
            let mut advanced = false;
            while it[u] < self.head[u].len() {
                let e = self.head[u][it[u]];
                let v = self.to[e];
                if self.cap[e] > EPS && level[v] == level[u] + 1 {
                    path.push(e);
                    u = v;
                    advanced = true;
                    break;
                }
                it[u] += 1;
            }
            if !advanced {
                // A dead end: retreat.
                let Some(e) = path.pop() else { return 0.0 };
                u = self.to[e ^ 1];
                it[u] += 1;
            }
        }
    }

    /// The nodes the source still reaches (the cut's source side).
    fn source_side(&self, s: usize) -> Vec<bool> {
        self.levels(s).iter().map(|l| *l >= 0).collect()
    }
}

/// Raster step of the outline (m) and the closing radius (cells).
const RES: f32 = 0.5;
const CLOSE: i64 = 4;
/// Then grown by a margin (cells: 1, 1.5 and 2 m tried by `generate`)
/// before it is simplified, so the simplification's inward cuts stay out of
/// the region (the plan's "only ever outward", roughly).
pub const MARGINS: [i64; 3] = [2, 3, 4];

/// The region's outline as a simple polygon (module doc), at most
/// `max_corners` corners; None when no tolerance up to 12 m gives one.
pub fn outline(g: &Graph, region: &[bool], max_corners: usize, margin: i64) -> Option<Vec<Vec2>> {
    let (mut lo, mut hi) = (Vec2::MAX, Vec2::MIN);
    for (c, _) in g.cells.iter().zip(region).filter(|(_, r)| **r) {
        lo = lo.min(c.min);
        hi = hi.max(c.max);
    }
    if lo.x > hi.x {
        return None;
    }
    let pad = (CLOSE + margin + 2) as f32 * RES;
    lo -= Vec2::splat(pad);
    hi += Vec2::splat(pad);
    let w = ((hi.x - lo.x) / RES).ceil() as i64;
    let h = ((hi.y - lo.y) / RES).ceil() as i64;
    let idx = |x: i64, z: i64| (z * w + x) as usize;
    let mut img = vec![false; (w * h) as usize];
    for (c, _) in g.cells.iter().zip(region).filter(|(_, r)| **r) {
        let x0 = ((c.min.x - lo.x) / RES).round() as i64;
        let x1 = ((c.max.x - lo.x) / RES).round() as i64;
        let z0 = ((c.min.y - lo.y) / RES).round() as i64;
        let z1 = ((c.max.y - lo.y) / RES).round() as i64;
        for z in z0..z1.max(z0 + 1) {
            for x in x0..x1.max(x0 + 1) {
                img[idx(x, z)] = true;
            }
        }
    }
    // Close: dilate then erode (square kernel, separable).
    let morph = |img: &[bool], grow: bool, r: i64| -> Vec<bool> {
        let mut a = vec![!grow; img.len()];
        for z in 0..h {
            for x in 0..w {
                let lo_x = (x - r).max(0);
                let hi_x = (x + r).min(w - 1);
                let v = (lo_x..=hi_x).map(|k| img[idx(k, z)]);
                a[idx(x, z)] = if grow { v.into_iter().any(|b| b) } else { v.into_iter().all(|b| b) };
            }
        }
        let mut b = vec![!grow; img.len()];
        for z in 0..h {
            for x in 0..w {
                let lo_z = (z - r).max(0);
                let hi_z = (z + r).min(h - 1);
                let v = (lo_z..=hi_z).map(|k| a[idx(x, k)]);
                b[idx(x, z)] = if grow { v.into_iter().any(|b| b) } else { v.into_iter().all(|b| b) };
            }
        }
        b
    };
    let img = morph(&morph(&morph(&img, true, CLOSE), false, CLOSE), true, margin);
    // Largest 4-connected piece.
    let mut label = vec![0u32; img.len()];
    let mut best = (0usize, 0u32);
    let mut next = 0u32;
    for start in 0..img.len() {
        if !img[start] || label[start] != 0 {
            continue;
        }
        next += 1;
        let mut size = 0;
        let mut stack = vec![start];
        label[start] = next;
        while let Some(p) = stack.pop() {
            size += 1;
            let (x, z) = (p as i64 % w, p as i64 / w);
            for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                let (nx, nz) = (x + dx, z + dz);
                if nx >= 0 && nz >= 0 && nx < w && nz < h && img[idx(nx, nz)] && label[idx(nx, nz)] == 0 {
                    label[idx(nx, nz)] = next;
                    stack.push(idx(nx, nz));
                }
            }
        }
        if size > best.0 {
            best = (size, next);
        }
    }
    let mut img: Vec<bool> = label.iter().map(|l| *l == best.1 && best.1 != 0).collect();
    // Holes filled: what the border's empty pixels don't reach.
    let mut outside = vec![false; img.len()];
    let mut stack: Vec<usize> = (0..w).flat_map(|x| [idx(x, 0), idx(x, h - 1)]).chain((0..h).flat_map(|z| [idx(0, z), idx(w - 1, z)])).collect();
    stack.retain(|p| !img[*p]);
    for p in &stack {
        outside[*p] = true;
    }
    while let Some(p) = stack.pop() {
        let (x, z) = (p as i64 % w, p as i64 / w);
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, nz) = (x + dx, z + dz);
            if nx >= 0 && nz >= 0 && nx < w && nz < h && !img[idx(nx, nz)] && !outside[idx(nx, nz)] {
                outside[idx(nx, nz)] = true;
                stack.push(idx(nx, nz));
            }
        }
    }
    for (p, o) in img.iter_mut().zip(&outside) {
        *p = !*o;
    }
    // No two pixels touching at a corner only (the trace would pinch).
    loop {
        let mut changed = false;
        for z in 0..h - 1 {
            for x in 0..w - 1 {
                let (a, b, c, d) = (img[idx(x, z)], img[idx(x + 1, z)], img[idx(x, z + 1)], img[idx(x + 1, z + 1)]);
                if (a && d && !b && !c) || (b && c && !a && !d) {
                    img[idx(x, z)] = true;
                    img[idx(x + 1, z)] = true;
                    img[idx(x, z + 1)] = true;
                    img[idx(x + 1, z + 1)] = true;
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let filled = |x: i64, z: i64| x >= 0 && z >= 0 && x < w && z < h && img[idx(x, z)];
    // The boundary's edges, the filled side on one hand; one loop.
    let mut next_of: BTreeMap<(i64, i64), (i64, i64)> = BTreeMap::new();
    for z in 0..h {
        for x in 0..w {
            if !filled(x, z) {
                continue;
            }
            if !filled(x, z - 1) {
                next_of.insert((x, z), (x + 1, z));
            }
            if !filled(x + 1, z) {
                next_of.insert((x + 1, z), (x + 1, z + 1));
            }
            if !filled(x, z + 1) {
                next_of.insert((x + 1, z + 1), (x, z + 1));
            }
            if !filled(x - 1, z) {
                next_of.insert((x, z + 1), (x, z));
            }
        }
    }
    let start = *next_of.keys().next()?;
    let mut ring = vec![start];
    let mut at = next_of[&start];
    while at != start {
        ring.push(at);
        at = *next_of.get(&at)?;
        if ring.len() > next_of.len() + 1 {
            return None;
        }
    }
    let pts: Vec<Vec2> = ring.iter().map(|(x, z)| lo + Vec2::new(*x as f32, *z as f32) * RES).collect();
    let mut tol = 1.5;
    while tol <= 12.0 {
        let s = simplify_closed(&pts, tol);
        if s.len() >= 3 && s.len() <= max_corners && PlanPolygon::new(&s).is_ok() {
            return Some(PlanPolygon::new(&s).ok()?.points().to_vec());
        }
        tol *= 1.3;
    }
    None
}

/// Douglas-Peucker on a closed ring: split at the first point and the one
/// farthest from it.
fn simplify_closed(pts: &[Vec2], tol: f32) -> Vec<Vec2> {
    if pts.len() < 4 {
        return pts.to_vec();
    }
    let far = (1..pts.len()).max_by(|a, b| pts[*a].distance(pts[0]).total_cmp(&pts[*b].distance(pts[0]))).unwrap_or(1);
    let mut first: Vec<Vec2> = pts[..=far].to_vec();
    let mut second: Vec<Vec2> = pts[far..].to_vec();
    second.push(pts[0]);
    first = dp(&first, tol);
    second = dp(&second, tol);
    first.pop();
    second.pop();
    first.extend(second);
    first
}

fn dp(pts: &[Vec2], tol: f32) -> Vec<Vec2> {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; n];
    keep[0] = true;
    keep[n - 1] = true;
    let mut stack = vec![(0, n - 1)];
    while let Some((a, b)) = stack.pop() {
        let (p, q) = (pts[a], pts[b]);
        let d = |x: Vec2| {
            let e = q - p;
            if e.length_squared() < 1e-9 { x.distance(p) } else { (x - p).perp_dot(e).abs() / e.length() }
        };
        if let Some(k) = (a + 1..b).max_by(|i, j| d(pts[*i]).total_cmp(&d(pts[*j]))) {
            if d(pts[k]) > tol {
                keep[k] = true;
                stack.push((a, k));
                stack.push((k, b));
            }
        }
    }
    pts.iter().zip(&keep).filter(|(_, k)| **k).map(|(p, _)| *p).collect()
}

/// The cells whose centre `poly` holds.
pub fn members(g: &Graph, poly: &PlanPolygon) -> Vec<bool> {
    g.cells.iter().map(|c| poly.contains(c.centre())).collect()
}

/// Whether the segment a-b crosses an edge of `poly` (the wall stands in
/// the way).
pub fn crosses(poly: &PlanPolygon, a: Vec2, b: Vec2) -> bool {
    let side = |o: Vec2, x: Vec2, y: Vec2| (x - o).perp_dot(y - o);
    poly.edges().any(|(c, d)| (side(c, d, a) > 0.0) != (side(c, d, b) > 0.0) && (side(a, b, c) > 0.0) != (side(a, b, d) > 0.0))
}

/// Along a level ray from `from` (inside) in `dir`, the distance to the
/// polygon's edge.
pub fn to_wall(poly: &PlanPolygon, from: Vec2, dir: Vec2) -> Option<f32> {
    if dir.length_squared() < 1e-6 {
        return None;
    }
    let d = dir.normalize();
    poly.edges()
        .filter_map(|(a, b)| {
            let e = b - a;
            let den = d.perp_dot(e);
            if den.abs() < 1e-9 {
                return None;
            }
            let t = (a - from).perp_dot(e) / den;
            let u = (a - from).perp_dot(d) / den;
            (t >= 0.0 && (0.0..=1.0).contains(&u)).then_some(t)
        })
        .reduce(f32::min)
}

/// The wall's cost (module doc).
#[derive(Clone, Debug, Default)]
pub struct Wall {
    pub open_m: f32,
    pub along_m: f32,
    pub indoor_m: f32,
    /// Where it stands over a floor above another (a ledge over the ground,
    /// a bridge): told, not refused.
    pub stacked_m: f32,
    /// Where it is open (sampled every 0.5 m).
    pub open_at: Vec<Vec2>,
    /// Where it stands under a roof.
    pub roofed_at: Vec<Vec2>,
}

impl Wall {
    pub fn open_share(&self) -> f32 {
        self.open_m / (self.open_m + self.along_m).max(1e-3)
    }
}

/// `poly`'s wall over the graph's floor; `ray(from, dir, max)` the world's
/// (a roof over an open sample: a ray up from 0.3 m over its floor, 6 m).
pub fn wall(g: &Graph, index: &CellIndex, poly: &PlanPolygon, ray: &impl Fn(Vec3, Vec3, f32) -> Option<f32>) -> Wall {
    let mut w = Wall::default();
    for (a, b) in poly.edges() {
        let n = (a.distance(b) / 0.5).ceil().max(1.0) as usize;
        for k in 0..n {
            let p = a.lerp(b, (k as f32 + 0.5) / n as f32);
            let step = a.distance(b) / n as f32;
            let under: Vec<usize> = index.at(g, p).collect();
            if under.is_empty() {
                w.along_m += step;
                continue;
            }
            w.open_m += step;
            w.open_at.push(p);
            let lo = under.iter().map(|i| g.cells[*i].height_at(p)).fold(f32::MAX, f32::min);
            let hi = under.iter().map(|i| g.cells[*i].height_at(p)).fold(f32::MIN, f32::max);
            // A roof over a floor: from 1 m over it (a slope's own ground
            // never counts), 5 m up.
            let roofed = under.iter().any(|i| ray(Vec3::new(p.x, g.cells[*i].height_at(p) + 1.0, p.y), Vec3::Y, 5.0).is_some());
            if roofed {
                w.indoor_m += step;
                w.roofed_at.push(p);
            }
            if hi - lo > 2.5 {
                w.stacked_m += step;
            }
        }
    }
    w
}

/// A player band the generator aims at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Band {
    pub lo: usize,
    pub hi: usize,
}

impl Band {
    pub const ALL: [Band; 2] = [Band { lo: 2, hi: 4 }, Band { lo: 4, hi: 8 }];

    pub fn parse(s: &str) -> Option<Self> {
        let (a, b) = s.split_once('-')?;
        let (lo, hi) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
        (2 <= lo && lo <= hi).then_some(Self { lo, hi })
    }

    /// The area (m2) at which a slice of mean sight `sight` meets in 8 s
    /// at the band's low end (the plan's section 4: `8 * (N - 1) * s *
    /// 16.17`).
    pub fn target(&self, sight: f32, fit: &FitSettings) -> f32 {
        8.0 * (self.lo as f32 - 1.0) * sight.min(fit.sight_cap_m) * 2.0 * 4.0 / std::f32::consts::PI * fit.speed_mps
    }

    pub fn label(&self) -> String {
        format!("{}-{}", self.lo, self.hi)
    }
}

/// What the generator works on (module doc).
pub struct Input<'a, S, R> {
    pub graph: &'a Graph,
    pub index: &'a CellIndex,
    /// The map's spawn spots (the 1.5 m grid) and spawn points (feet).
    pub spots: &'a [Vec3],
    pub points: &'a [Vec3],
    /// The solid near each cell, 0 to 1 (seeds' bias).
    pub score: &'a [f32],
    /// The cells of the spawn clusters' middles (seeds first).
    pub must: &'a [usize],
    /// The whole map's mean sight (m; the first growth's target).
    pub sight_m: f32,
    pub sees: S,
    pub ray: R,
}

/// The generator's settings.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub seeds: usize,
    /// The target areas tried, as shares of the band's (a smaller slice
    /// meets sooner, which the pool still takes).
    pub scales: &'static [f32],
    pub table: usize,
    pub max_corners: usize,
    /// Least `area / convex hull area` (decision 5: not so concave that its
    /// wall hides one part from another).
    pub solidity_min: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            seeds: 40,
            scales: &[1.0, 0.65, 0.4],
            table: 800,
            max_corners: 16,
            solidity_min: 0.6,
        }
    }
}

/// One candidate slice.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub band: Band,
    /// Its seed's number among the seeds (its provisional key).
    pub seed_no: usize,
    pub seed: Vec3,
    pub poly: PlanPolygon,
    pub members: Vec<bool>,
    /// The cut's area and width.
    pub cut_area_m2: f32,
    pub cut_m: f32,
    pub wall: Wall,
    pub solidity: f32,
    pub report: Report,
    pub verdict: SliceVerdict,
    /// The map's points inside (and the seed's), and the ones it makes.
    pub points: Vec<Vec3>,
    pub made: Vec<Vec3>,
    pub table: SeeTable,
    /// Every reason it fails the band (empty: it passes).
    pub fails: Vec<String>,
    pub rank: f32,
    pub vertical_m: f32,
}

impl Candidate {
    pub fn passes(&self) -> bool {
        self.fails.is_empty()
    }
}

/// The polygon's area over its convex hull's.
pub fn solidity(poly: &PlanPolygon) -> f32 {
    let mut p: Vec<Vec2> = poly.points().to_vec();
    p.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    let cross = |o: Vec2, a: Vec2, b: Vec2| (a - o).perp_dot(b - o);
    let mut hull: Vec<Vec2> = Vec::new();
    for pass in 0..2 {
        let start = hull.len();
        let it: Box<dyn Iterator<Item = &Vec2>> = if pass == 0 { Box::new(p.iter()) } else { Box::new(p.iter().rev()) };
        for q in it {
            while hull.len() >= start + 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], *q) <= 0.0 {
                hull.pop();
            }
            hull.push(*q);
        }
        hull.pop();
    }
    let area = |v: &[Vec2]| (0..v.len()).map(|i| v[i].perp_dot(v[(i + 1) % v.len()])).sum::<f32>().abs() / 2.0;
    poly.area() / area(&hull).max(1e-3)
}

/// The mean of 16 level rays (capped at the pool's sight cap) from up to
/// 200 of `spots`, stopped by `poly`.
fn sight_in(spots: &[Vec3], poly: &PlanPolygon, ray: &impl Fn(Vec3, Vec3, f32) -> Option<f32>, cap: f32) -> f32 {
    let s = slice_safety::sample(spots, 200);
    let mut sum = 0.0;
    for p in &s {
        let eye = *p + Vec3::Y * slice_safety::EYE;
        for k in 0..16 {
            let t = k as f32 * std::f32::consts::TAU / 16.0;
            let d = Vec3::new(t.cos(), 0.0, t.sin());
            let hit = ray(eye, d, cap).unwrap_or(cap);
            sum += hit.min(to_wall(poly, eye.xz(), d.xz()).unwrap_or(cap));
        }
    }
    sum / (s.len() * 16).max(1) as f32
}

/// How far the polygon's area may move from the cut's (the plan's 15%, and
/// 10% more for the outline's 1 m margin, which a small slice feels).
const AREA_SLACK: f32 = 0.25;

/// Every candidate for `band` (module doc), best first; `key` names the
/// map (`game:map`) for the spawn points' seeds.
pub fn generate<S, R>(input: &Input<S, R>, band: Band, settings: &Settings, fit: &FitSettings, key: &str) -> Vec<Candidate>
where
    S: Fn(Vec3, Vec3) -> bool,
    R: Fn(Vec3, Vec3, f32) -> Option<f32>,
{
    let g = input.graph;
    let seeds = pick_seeds(g, input.score, input.must, settings.seeds);
    let mut shapes: Vec<(usize, usize, Grown, PlanPolygon, Vec<bool>, f32)> = Vec::new();
    let tries: Vec<(usize, usize, f32)> = settings
        .scales
        .iter()
        .enumerate()
        .flat_map(|(k, sc)| seeds.iter().enumerate().map(move |(n, s)| (k, n, *s, *sc)))
        .map(|(k, n, s, sc)| (k * seeds.len() + n, s, sc))
        .collect();
    for (no, seed, scale) in tries {
        let seed = &seed;
        let Some(mut grown) = grow(g, *seed, scale * band.target(input.sight_m, fit)) else { continue };
        let Some(mut pts) = outline(g, &grown.region, settings.max_corners, MARGINS[0]) else { continue };
        // Again at the first cut's own sight.
        if let Ok(poly) = PlanPolygon::new(&pts) {
            let inside: Vec<Vec3> = input.spots.iter().copied().filter(|s| poly.contains3(*s)).collect();
            let sight = sight_in(&inside, &poly, &input.ray, fit.sight_cap_m);
            if sight > 1.0 && (sight - input.sight_m).abs() > 0.1 * input.sight_m {
                if let Some(g2) = grow(g, *seed, scale * band.target(sight, fit))
                    && let Some(p2) = outline(g, &g2.region, settings.max_corners, MARGINS[0])
                {
                    grown = g2;
                    pts = p2;
                }
            }
        }
        // The margin whose wall stands under no roof (a wider one walks a
        // wall out of a cave's mouth), else the one under the least.
        let mut best: Option<(f32, PlanPolygon, Vec<bool>, f32)> = None;
        for (k, m) in MARGINS.iter().enumerate() {
            let p = if k == 0 { Some(pts.clone()) } else { outline(g, &grown.region, settings.max_corners, *m) };
            let Some(poly) = p.and_then(|p| PlanPolygon::new(&p).ok()) else { continue };
            let members = members(g, &poly);
            let area = g.area(&members);
            if (area - grown.area_m2).abs() > AREA_SLACK * grown.area_m2 {
                continue;
            }
            let roofed = wall(g, input.index, &poly, &input.ray).indoor_m;
            if best.as_ref().is_none_or(|b| roofed < b.0) {
                best = Some((roofed, poly, members, area));
            }
            if roofed <= 1.0 {
                break;
            }
        }
        let Some((_, poly, members, area)) = best else { continue };
        shapes.push((no, *seed, grown, poly, members, area));
    }
    let mut out: Vec<Candidate> = shapes
        .into_iter()
        .map(|(no, seed, grown, poly, members, area)| measure(input, band, no, seed, grown, poly, members, area, settings, fit, key))
        .collect();
    out.sort_by(|a, b| b.passes().cmp(&a.passes()).then(b.rank.total_cmp(&a.rank)).then(a.seed_no.cmp(&b.seed_no)));
    // Near-duplicates: over 80% of the smaller shared, the better kept.
    let mut kept: Vec<Candidate> = Vec::new();
    for c in out {
        let dup = kept.iter().any(|k| {
            let shared: f32 = g.cells.iter().enumerate().filter(|(i, _)| c.members[*i] && k.members[*i]).map(|(_, x)| x.area()).sum();
            shared > 0.8 * c.report.area_m2.min(k.report.area_m2)
        });
        if !dup {
            kept.push(c);
        }
    }
    kept
}

#[allow(clippy::too_many_arguments)]
fn measure<S, R>(
    input: &Input<S, R>,
    band: Band,
    no: usize,
    seed: usize,
    grown: Grown,
    poly: PlanPolygon,
    members: Vec<bool>,
    area: f32,
    settings: &Settings,
    fit: &FitSettings,
    key: &str,
) -> Candidate
where
    S: Fn(Vec3, Vec3) -> bool,
    R: Fn(Vec3, Vec3, f32) -> Option<f32>,
{
    let g = input.graph;
    // The line of sight stops at the wall.
    let sees = |a: Vec3, b: Vec3| !crosses(&poly, a.xz(), b.xz()) && (input.sees)(a, b);
    let ray = |from: Vec3, dir: Vec3, max: f32| {
        let hit = (input.ray)(from, dir, max);
        match to_wall(&poly, from.xz(), dir.xz()) {
            Some(w) if w <= max => Some(hit.map_or(w, |h| h.min(w))),
            _ => hit,
        }
    };
    let spots: Vec<Vec3> = input.spots.iter().copied().filter(|s| poly.contains3(*s)).collect();
    let seed_at = if poly.contains(g.cells[seed].centre()) {
        g.cells[seed].floor()
    } else {
        // The member cell farthest from the wall.
        (0..g.len())
            .filter(|i| members[*i])
            .max_by(|a, b| poly.edge_distance(g.cells[*a].centre()).total_cmp(&poly.edge_distance(g.cells[*b].centre())).then(b.cmp(a)))
            .map_or(g.cells[seed].floor(), |i| g.cells[i].floor())
    };
    let mut points: Vec<Vec3> = input.points.iter().copied().filter(|p| poly.contains3(*p)).collect();
    points.push(seed_at);
    let rules: PointRules = fit.slice.point_rules();
    let id = format!("cand_{}_{}_{no}", band.lo, band.hi);
    let hash = chimera_sim::weapon::random::crc32(format!("{key}@{id}").as_bytes());
    let made = slice_safety::choose_points(&spots[..spots.len().min(chimera_sim::minigame::spawns::MAX_SPOTS)], &points, &poly, &sees, rules, hash);
    let all: Vec<Vec3> = points.iter().chain(&made).copied().collect();
    let table = SeeTable::build(&slice_safety::sample(&spots, settings.table), &sees);
    let (cover, long_sight_p95_m) = slice_safety::level_rays(&table.spots, &ray);
    let (pts, floor_sees_point) = slice_safety::point_safety(&all, &table.spots, &sees);
    let mut sight = 0.0;
    for p in &table.spots {
        let eye = *p + Vec3::Y * slice_safety::EYE;
        for k in 0..16 {
            let t = k as f32 * std::f32::consts::TAU / 16.0;
            sight += ray(eye, Vec3::new(t.cos(), 0.0, t.sin()), fit.sight_cap_m).unwrap_or(fit.sight_cap_m);
        }
    }
    let sight_m = sight / (table.len() * 16).max(1) as f32;
    let mut safety = slice_safety::Safety::of(&table, hash);
    // The spots the game would draw from, not the table's sample.
    safety.spots = spots.len();
    let report = Report {
        area_m2: area,
        sight_m,
        safety,
        cover,
        long_sight_p95_m,
        points: pts,
        floor_sees_point,
    };
    let verdict = fit.slice.judge(&report, fit);
    let wall = wall(g, input.index, &poly, &input.ray);
    let solidity = solidity(&poly);
    let mut ys: Vec<f32> = spots.iter().map(|s| s.y).collect();
    ys.sort_by(f32::total_cmp);
    let q = |f: f32| if ys.is_empty() { 0.0 } else { ys[((ys.len() - 1) as f32 * f).round() as usize] };
    let vertical_m = q(0.95) - q(0.05);
    let mut fails = verdict.failed.clone();
    match verdict.players {
        // At least two counts of the band.
        Some([lo, hi]) if hi.min(band.hi) < lo.max(band.lo) + 1 => fails.push(format!("suits {lo}-{hi}, under two counts of the band {}", band.label())),
        _ => {}
    }
    if wall.indoor_m > 1.0 {
        fails.push(format!("the wall cuts under a roof for {:.1} m (a building, a doorway)", wall.indoor_m));
    }
    if solidity < settings.solidity_min {
        fails.push(format!("solidity {solidity:.2} under {:.2} (too concave: its wall hides a part)", settings.solidity_min));
    }
    let fitness = Fitness {
        loads: true,
        area_m2: area,
        sight_m,
        spot_area_m2: 0.0,
        spawns: 0,
        spots: 0,
        spots_off: 0,
        stuck: 0,
        wander_s: BTreeMap::new(),
    };
    let mid = (band.lo + band.hi).div_ceil(2);
    let rank = fit.curve(fitness.wander(mid, fit)) * (1.0 - report.safety.seen_at(band.hi)) * (1.0 - wall.open_share() / 2.0);
    Candidate {
        band,
        seed_no: no,
        seed: seed_at,
        poly,
        members,
        cut_area_m2: grown.area_m2,
        cut_m: grown.cut_m,
        wall,
        solidity,
        report,
        verdict,
        points,
        made,
        table,
        fails,
        rank,
        vertical_m,
    }
}

/// A compass word for `at` from the middle of `lo`..`hi` (north is -z, east
/// +x: up and right in the tool's pictures); "Centre" near the middle.
pub fn compass(at: Vec2, lo: Vec2, hi: Vec2) -> &'static str {
    let mid = (lo + hi) / 2.0;
    let half = ((hi - lo) / 2.0).max(Vec2::splat(1.0));
    let d = (at - mid) / half;
    if d.length() < 0.2 {
        return "Centre";
    }
    let a = (-d.y).atan2(d.x).to_degrees();
    const WORDS: [&str; 8] = ["East", "North East", "North", "North West", "West", "South West", "South", "South East"];
    WORDS[(((a + 360.0 + 22.5) % 360.0) / 45.0) as usize % 8]
}
