//! What the bots move on (specs/combat_arms/bots.md 3): the map's
//! navigation areas (Chimera's `NavMesh`: the map's hand-placed AI
//! volumes, `map_ai`, or where it has none generated from its collision,
//! `chimera_world::map::nav_gen`, bounded to the clusters a walking player
//! reaches on a map with visibility data, `brain::ensure_nav`), the volume path
//! search with its squared-distance costs, and the straight walk test the
//! attack moves and the sight shortcut use.
//!
//! - Links: an area link whose shared edge is shorter than `MIN_LINK`
//!   (48 u) is not made between the map's volumes (`map_ai`); generated
//!   areas have no minimum (their rectangles meet along ragged edges,
//!   often one 0.5 m cell wide in the middle of a wide floor, so the rule
//!   would cut open floors apart).
//! - The search: Dijkstra over areas, the cost of entering the next area
//!   w(V) ‖e(V) − c(V, V')‖² (u²), e(V) where the path entered V (the
//!   start position in the first), c the link's connection point (the
//!   middle of the shared edge, on the floor), w 1, plus `RESERVED` when
//!   the area is reserved by another bot and the request diverges; it
//!   stops expanding areas costlier than the goal's best.
//! - Following: the path's points are the crossings of each link (nearest
//!   to the walker on the shared edge, as Chimera's `crossing`), then the
//!   destination.

use bevy::prelude::*;

use crate::map::nav::{NavMesh, Side, Via};

/// The shortest link kept, u.
pub const MIN_LINK: f32 = 48.0;
/// The weight added to a reserved area when paths diverge.
pub const RESERVED: f32 = 500.0;

/// Where link `from` → `to` across `side` lies: its two ends on the
/// shared edge (metres, on `from`'s floor).
pub fn link_ends(mesh: &NavMesh, from: usize, to: usize, side: Side) -> (Vec3, Vec3) {
    let (a, b) = (&mesh.areas[from], &mesh.areas[to]);
    let along_x = matches!(side, Side::MinZ | Side::MaxZ);
    let (lo, hi, fixed) = if along_x {
        let z = if side == Side::MinZ { a.min.y } else { a.max.y };
        (a.min.x.max(b.min.x), a.max.x.min(b.max.x), z)
    } else {
        let x = if side == Side::MinX { a.min.x } else { a.max.x };
        (a.min.y.max(b.min.y), a.max.y.min(b.max.y), x)
    };
    let at = |t: f32| {
        let (x, z) = if along_x { (t, fixed) } else { (fixed, t) };
        Vec3::new(x, a.height_at(x, z).max(b.height_at(x, z)), z)
    };
    (at(lo), at(hi.max(lo)))
}

/// The link's connection point: the middle of its shared edge.
pub fn connection(mesh: &NavMesh, from: usize, to: usize, side: Side) -> Vec3 {
    let (a, b) = link_ends(mesh, from, to, side);
    (a + b) * 0.5
}

/// A found path: the areas from the start's to the goal's, each with the
/// side it was entered across, and the total cost (u²).
#[derive(Clone, Debug, PartialEq)]
pub struct Path {
    pub areas: Vec<(usize, Option<Side>)>,
    pub cost: f32,
}

/// The volume path search (3) from `start` area (position `from`, m) to
/// `goal` area. `unit`: metres per world unit; `weight(area)`: its path
/// weight (1, plus `RESERVED` for an area reserved when diverging).
pub fn find(
    mesh: &NavMesh,
    start: usize,
    from: Vec3,
    goal: usize,
    unit: f32,
    min_link: f32,
    weight: impl Fn(usize) -> f32,
) -> Option<Path> {
    let n = mesh.areas.len();
    if start >= n || goal >= n {
        return None;
    }
    let mut cost = vec![f32::INFINITY; n];
    let mut entry = vec![Vec3::ZERO; n];
    let mut parent: Vec<Option<(usize, Side)>> = vec![None; n];
    let mut done = vec![false; n];
    cost[start] = 0.0;
    entry[start] = from / unit;
    // Costs are never negative: their bits order as the numbers do.
    let mut open = std::collections::BinaryHeap::new();
    open.push(std::cmp::Reverse((0u32, start)));
    while let Some(std::cmp::Reverse((bits, cur))) = open.pop() {
        if done[cur] || f32::from_bits(bits) > cost[cur] {
            continue;
        }
        if cost[cur] > cost[goal] {
            break;
        }
        done[cur] = true;
        if cur == goal {
            break;
        }
        for &(next, via) in &mesh.areas[cur].links {
            let Via::Walk(side) = via else { continue };
            if done[next] {
                continue;
            }
            let (lo, hi) = link_ends(mesh, cur, next, side);
            if lo.distance(hi) / unit < min_link {
                continue;
            }
            let c = (lo + hi) * 0.5 / unit;
            let step = weight(cur) * entry[cur].distance_squared(c);
            let total = cost[cur] + step;
            if total < cost[next] {
                cost[next] = total;
                entry[next] = c;
                parent[next] = Some((cur, side));
                open.push(std::cmp::Reverse((total.to_bits(), next)));
            }
        }
    }
    if !cost[goal].is_finite() {
        return None;
    }
    let mut areas = vec![(goal, parent[goal].map(|p| p.1))];
    let mut at = goal;
    while let Some((p, _)) = parent[at] {
        areas.push((p, parent[p].map(|q| q.1)));
        at = p;
    }
    areas.reverse();
    Some(Path { areas, cost: cost[goal] })
}

/// The points to walk along `path` from `from` to `to` (m): each link's
/// crossing nearest the walker, then `to`.
pub fn points(mesh: &NavMesh, path: &Path, from: Vec3, to: Vec3) -> Vec<Vec3> {
    let mut out = Vec::new();
    let mut at = from;
    for w in path.areas.windows(2) {
        let (prev, (next, side)) = (w[0].0, w[1]);
        let Some(side) = side else { continue };
        let (lo, hi) = link_ends(mesh, prev, next, side);
        // Nearest point on the edge, kept a little inside its ends.
        let d = hi - lo;
        let len = d.length();
        let t = if len > 1e-4 {
            ((at - lo).dot(d) / (len * len)).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let margin = if len > 1e-4 { (0.35 / len).min(0.5) } else { 0.5 };
        let p = lo + d * t.clamp(margin, 1.0 - margin);
        out.push(p);
        at = p;
    }
    out.push(to);
    out
}

/// Whether a straight walk from `a` to `b` (m) stays on the areas: points
/// every `step` m along the line each over some area within a step of the
/// line's height (the attack moves' and the sight shortcut's test).
pub fn straight(mesh: &NavMesh, a: Vec3, b: Vec3, step: f32) -> bool {
    let d = b - a;
    let n = (d.xz().length() / step).ceil().max(1.0) as usize;
    let mut y = a.y;
    for k in 0..=n {
        let p = a + d * (k as f32 / n as f32);
        let Some(i) = mesh.area_at(Vec3::new(p.x, y + 0.6, p.z)) else { return false };
        let h = mesh.areas[i].height_at(p.x, p.z);
        if (h - y).abs() > 0.6 {
            return false;
        }
        y = h;
    }
    true
}

/// The patrol graph (4): the map's patrol nodes (`map_ai`), or where a
/// map has none ours (`build`): nodes are area centres at least
/// `NODE_SPACING` m apart (the biggest areas first), linked to the nodes
/// reachable within `NODE_REACH` m of path.
#[derive(Clone, Debug, Default)]
pub struct PatrolGraph {
    /// Each node's area.
    pub nodes: Vec<usize>,
    /// Each node's place (m, on its area's floor).
    pub points: Vec<Vec3>,
    /// Each node's links (node indices).
    pub links: Vec<Vec<usize>>,
    /// Each node's name in the map (empty for ours).
    pub names: Vec<String>,
}

/// Patrol nodes' least spacing and their links' reach (m).
pub const NODE_SPACING: f32 = 7.0;
pub const NODE_REACH: f32 = 16.0;

impl PatrolGraph {
    pub fn build(mesh: &NavMesh) -> Self {
        let mut order: Vec<usize> = (0..mesh.areas.len()).collect();
        let size = |i: usize| {
            let a = &mesh.areas[i];
            (a.max - a.min).x * (a.max - a.min).y
        };
        order.sort_by(|&a, &b| size(b).total_cmp(&size(a)).then(a.cmp(&b)));
        let mut nodes: Vec<usize> = Vec::new();
        for i in order {
            let c = mesh.areas[i].center;
            if nodes.iter().all(|&n| mesh.areas[n].center.distance(c) >= NODE_SPACING) {
                nodes.push(i);
            }
        }
        // Each area's node: a large mesh has hundreds of nodes and each
        // reaches hundreds of areas (cs_militia: 11,510 areas).
        let mut node_of: Vec<Option<usize>> = vec![None; mesh.areas.len()];
        for (j, &n) in nodes.iter().enumerate() {
            node_of[n] = Some(j);
        }
        let links = nodes
            .iter()
            .enumerate()
            .map(|(k, &n)| {
                let mut near: Vec<usize> = mesh
                    .within(n, NODE_REACH)
                    .into_iter()
                    .filter_map(|r| node_of[r.0])
                    .filter(|&j| j != k)
                    .collect();
                near.sort_unstable();
                near.dedup();
                near
            })
            .collect();
        let points = nodes.iter().map(|&n| mesh.areas[n].center).collect();
        Self { nodes, points, links, names: Vec::new() }
    }

    /// The node the map names `name` (any case).
    pub fn by_name(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n.eq_ignore_ascii_case(name))
    }

    /// The node nearest `p` (m).
    pub fn nearest(&self, p: Vec3) -> Option<usize> {
        (0..self.points.len())
            .min_by(|&a, &b| self.points[a].distance_squared(p).total_cmp(&self.points[b].distance_squared(p)))
    }
}
