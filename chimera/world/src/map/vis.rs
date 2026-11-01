//! Visibility sets for any game whose maps carry precomputed visibility
//! (Source's PVS, Quake-family maps), as data and queries: the map is
//! divided into clusters, a point query finds a camera's cluster, and each
//! cluster lists the clusters potentially visible from anywhere inside it.
//! Map parts are tagged with the clusters they touch (`VisClusters`);
//! areaportals divide the map into areas (`MapAreas::flood`); occluders
//! hide what lies behind them. `render::vis` culls with them (bots
//! and hearing use them too).
//!
//! Visibility for any game with precomputed visibility: `MapVisibility` (a
//! point-to-cluster tree, per-cluster potentially visible and audible
//! sets, `MapVisibility::hears` for who is sent a shot), `split_mesh`
//! (world meshes cut into grid chunks with their clusters), `MapAreas` (areas,
//! areaportals with their openings, `flood`), `WindowFade`, `Occluder`,
//! `Occludee`, `screen_occluders`, `occluded`, `FadeDistance`, `fade_band`,
//! `box_clusters`, `camera_clusters`, `ActiveVisibility`.

use bevy::prelude::*;

use super::{AreaPortalStates, MapMesh};

/// A map's visibility description, engine space (meters).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MapVisibility {
    /// Splitting planes: normal and distance (`n . p = d`).
    pub planes: Vec<(Vec3, f32)>,
    /// Plane index and children (front, back); a negative child `c` is
    /// leaf `-c - 1`.
    pub nodes: Vec<(usize, [i32; 2])>,
    /// Each leaf's cluster; negative for solid leaves and leaves outside
    /// every cluster.
    pub leaf_clusters: Vec<i32>,
    pub cluster_count: usize,
    /// Per cluster: the potentially visible clusters, as a bit set of
    /// `cluster_count` bits in 64-bit words. A cluster always sees itself.
    pub visible: Vec<Vec<u64>>,
    /// Per cluster: the potentially audible clusters (Source's PAS: the
    /// clusters any cluster of its PVS sees), the same bit sets. Empty for
    /// maps without one: everything is audible (`hears`).
    pub audible: Vec<Vec<u64>>,
    /// Areas and the portals between them; empty for maps without them.
    pub areas: MapAreas,
    /// Occluders (`Occluder`); empty for maps without them.
    pub occluders: Vec<Occluder>,
}

impl MapVisibility {
    /// The leaf containing `p`, or None outside the tree.
    pub fn leaf_at(&self, p: Vec3) -> Option<usize> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut node = 0i32;
        while node >= 0 {
            let &(plane, children) = self.nodes.get(node as usize)?;
            let (n, d) = self.planes[plane];
            node = if n.dot(p) - d >= 0.0 { children[0] } else { children[1] };
        }
        Some((-node - 1) as usize)
    }

    /// The cluster containing `p`, or None in solid or outside the map.
    pub fn cluster_at(&self, p: Vec3) -> Option<u32> {
        let c = *self.leaf_clusters.get(self.leaf_at(p)?)?;
        (c >= 0).then_some(c as u32)
    }

    /// The area containing `p` (0: none, or a map without areas).
    pub fn area_at(&self, p: Vec3) -> u16 {
        self.leaf_at(p)
            .and_then(|l| self.areas.leaf_areas.get(l).copied())
            .unwrap_or(0)
    }

    /// Every cluster whose leaves an axis-aligned box touches (a superset:
    /// a box straddling a plane, within `EPSILON`, visits both sides).
    pub fn clusters_in_box(&self, min: Vec3, max: Vec3, out: &mut Vec<u32>) {
        const EPSILON: f32 = 0.01;
        if self.nodes.is_empty() {
            return;
        }
        let centre = (min + max) * 0.5;
        let half = (max - min) * 0.5;
        let mut stack = vec![0i32];
        while let Some(node) = stack.pop() {
            if node < 0 {
                if let Some(&c) = self.leaf_clusters.get((-node - 1) as usize)
                    && c >= 0
                    && !out.contains(&(c as u32))
                {
                    out.push(c as u32);
                }
                continue;
            }
            let Some(&(plane, children)) = self.nodes.get(node as usize) else {
                continue;
            };
            let (n, d) = self.planes[plane];
            let dist = n.dot(centre) - d;
            let radius = n.abs().dot(half) + EPSILON;
            if dist > -radius {
                stack.push(children[0]);
            }
            if dist < radius {
                stack.push(children[1]);
            }
        }
    }

    /// Whether `to` is potentially visible from `from`.
    pub fn sees(&self, from: u32, to: u32) -> bool {
        self.visible
            .get(from as usize)
            .and_then(|row| row.get(to as usize / 64))
            .is_some_and(|w| w >> (to % 64) & 1 != 0)
    }

    /// Whether a sound or a shot at `source` is potentially audible at
    /// `listener` (the listener's cluster is in the source cluster's PAS):
    /// who Source sends "PAS" messages to. True when the map has no PAS or
    /// either point is outside every cluster (in solid, out of the map):
    /// unknown is sent.
    pub fn hears(&self, source: Vec3, listener: Vec3) -> bool {
        if self.audible.is_empty() {
            return true;
        }
        let (Some(from), Some(to)) = (self.cluster_at(source), self.cluster_at(listener)) else {
            return true;
        };
        self.audible
            .get(from as usize)
            .and_then(|row| row.get(to as usize / 64))
            .is_none_or(|w| w >> (to % 64) & 1 != 0)
    }

    /// Whether the segment from `a` to `b` stays out of solid leaves
    /// (Source's trace against world brushes; detail brushes and
    /// displacements aren't in the tree). Without a tree: clear.
    pub fn segment_clear(&self, a: Vec3, b: Vec3) -> bool {
        if self.nodes.is_empty() {
            return true;
        }
        // (node, from, to) left to walk.
        let mut stack = vec![(0i32, a, b)];
        while let Some((node, p, q)) = stack.pop() {
            if node < 0 {
                if self.leaf_clusters.get((-node - 1) as usize).is_none_or(|&c| c < 0) {
                    return false;
                }
                continue;
            }
            let Some(&(plane, [front, back])) = self.nodes.get(node as usize) else {
                return false;
            };
            let (n, d) = self.planes[plane];
            let (dp, dq) = (n.dot(p) - d, n.dot(q) - d);
            // Same side as `leaf_at` picks (>= 0 is the front).
            match (dp >= 0.0, dq >= 0.0) {
                (true, true) => stack.push((front, p, q)),
                (false, false) => stack.push((back, p, q)),
                (p_front, _) => {
                    let m = p + (q - p) * (dp / (dp - dq));
                    let (near, far) = if p_front { (front, back) } else { (back, front) };
                    stack.push((far, m, q));
                    stack.push((near, p, m));
                }
            }
        }
        true
    }

    /// Whether any of `clusters` is potentially visible from `from` (no
    /// clusters: always).
    pub fn sees_any(&self, from: u32, clusters: &[u32]) -> bool {
        if clusters.is_empty() {
            return true;
        }
        clusters.iter().any(|&c| self.sees(from, c))
    }
}

/// A map's areas: groups of leaves that areaportals separate (Source's
/// areas lump; area 0 is "none"). Each portal joins two areas through a
/// convex opening.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MapAreas {
    /// Each leaf's area, indexed like `MapVisibility::leaf_clusters`.
    pub leaf_areas: Vec<u16>,
    pub portals: Vec<AreaPortal>,
    /// Per area: (portal, the area on its other side).
    pub links: Vec<Vec<(u32, u16)>>,
    /// Per cluster: the areas its leaves are in, sorted.
    pub cluster_areas: Vec<Vec<u16>>,
}

/// An opening between two areas.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AreaPortal {
    /// What map logic calls it by (Source: the portal entity's
    /// `portalnumber`).
    pub key: u16,
    pub areas: [u16; 2],
    /// The opening, a convex polygon (engine space, meters); empty when
    /// unknown (then looked through whole).
    pub polygon: Vec<Vec3>,
    /// A window (func_areaportalwindow): its brush fades in with the
    /// view's distance and the portal closes once the brush is opaque.
    pub fade: Option<WindowFade>,
    /// What closes it can be seen through (a door with a window): never
    /// closed, so the glass doesn't show a hole.
    pub see_through: bool,
}

impl AreaPortal {
    /// The distance from `eye` to the opening's bounds (0 inside them, or
    /// with no opening known).
    pub fn distance(&self, eye: Vec3) -> f32 {
        let Some(first) = self.polygon.first() else { return 0.0 };
        let (lo, hi) = self.polygon.iter().fold((*first, *first), |(a, b), p| (a.min(*p), b.max(*p)));
        (eye.clamp(lo, hi) - eye).length()
    }

    /// Whether a window is closed for a view at `eye` (past its fade's
    /// end, with a brush drawn opaque there).
    pub fn window_closed(&self, eye: Vec3) -> bool {
        self.fade.is_some_and(|f| f.closes(self.distance(eye)))
    }
}

/// A func_areaportalwindow's fade (public entity documentation): its
/// brush (`target`) draws at `limit` alpha nearer than `start` from the
/// opening, rising to opaque at `end`; beyond `end` the portal closes and
/// the opaque brush stands in for what lies behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WindowFade {
    /// Meters from the opening (`FadeStartDist`, `FadeDist`).
    pub start: f32,
    pub end: f32,
    /// The brush's alpha up close (`TranslucencyLimit`).
    pub limit: f32,
    /// The brush's entity (index in the map's entities); without one the
    /// window never closes (nothing would stand in for the far side).
    pub brush: Option<usize>,
}

impl WindowFade {
    /// The brush's alpha for a view `distance` from the opening.
    pub fn alpha(&self, distance: f32) -> f32 {
        let t = if self.end > self.start {
            ((distance - self.start) / (self.end - self.start)).clamp(0.0, 1.0)
        } else if distance < self.end {
            0.0
        } else {
            1.0
        };
        let limit = self.limit.clamp(0.0, 1.0);
        limit + (1.0 - limit) * t
    }

    /// Whether the portal is closed at `distance`: past the end, with a
    /// brush to draw.
    pub fn closes(&self, distance: f32) -> bool {
        self.brush.is_some() && distance > self.end
    }
}

/// A rectangle in normalized device coordinates: min x, min y, max x,
/// max y.
pub type ScreenRect = [f32; 4];

const FULL_SCREEN: ScreenRect = [-1.0, -1.0, 1.0, 1.0];

/// Slack around a portal's projected rectangle (NDC), for rasterization.
const PORTAL_MARGIN: f32 = 0.01;

impl MapAreas {
    /// Build the links and each cluster's areas. `leaf_clusters` as in
    /// `MapVisibility`.
    pub fn new(leaf_areas: Vec<u16>, leaf_clusters: &[i32], cluster_count: usize, portals: Vec<AreaPortal>) -> Self {
        let area_count = portals
            .iter()
            .flat_map(|p| p.areas)
            .chain(leaf_areas.iter().copied())
            .max()
            .map_or(0, |a| a as usize + 1);
        let mut links = vec![Vec::new(); area_count];
        for (i, p) in portals.iter().enumerate() {
            let [a, b] = p.areas;
            links[a as usize].push((i as u32, b));
            links[b as usize].push((i as u32, a));
        }
        let mut cluster_areas: Vec<Vec<u16>> = vec![Vec::new(); cluster_count];
        for (leaf, &c) in leaf_clusters.iter().enumerate() {
            if let (Ok(c), Some(&a)) = (usize::try_from(c), leaf_areas.get(leaf))
                && let Some(areas) = cluster_areas.get_mut(c)
                && !areas.contains(&a)
            {
                areas.push(a);
            }
        }
        for a in &mut cluster_areas {
            a.sort_unstable();
        }
        Self {
            leaf_areas,
            portals,
            links,
            cluster_areas,
        }
    }

    pub fn area_count(&self) -> usize {
        self.links.len()
    }

    /// Mark portals with a see-through surface in their opening (glass,
    /// grates: any mesh not opaque with a triangle within `SEE_THROUGH_REACH`
    /// of the opening's box) as `see_through`: closing them would show a
    /// hole through it.
    pub fn mark_see_through(&mut self, meshes: &[MapMesh]) {
        const SEE_THROUGH_REACH: f32 = 0.25;
        for portal in &mut self.portals {
            let Some(first) = portal.polygon.first() else { continue };
            let (lo, hi) = portal.polygon.iter().fold((*first, *first), |(a, b), p| (a.min(*p), b.max(*p)));
            let (lo, hi) = (lo - SEE_THROUGH_REACH, hi + SEE_THROUGH_REACH);
            portal.see_through = meshes
                .iter()
                .filter(|m| !m.skybox && m.alpha != super::MapAlpha::Opaque)
                .any(|m| {
                    m.indices.chunks_exact(3).any(|t| {
                        let p = [0, 1, 2].map(|k| Vec3::from(m.positions[t[k] as usize]));
                        let (a, b) = (p[0].min(p[1]).min(p[2]), p[0].max(p[1]).max(p[2]));
                        a.cmple(hi).all() && b.cmpge(lo).all()
                    })
                });
        }
    }

    /// The areas a camera in area `from` can see into: those reached
    /// through portals `open` passes. With `clip_from_world` (the camera's
    /// view projection), a portal is looked through only where its screen
    /// rectangle overlaps the rectangle it was reached by, starting from
    /// the whole screen. Area 0, or one out of range, reaches everything;
    /// area 0 (leaves in no area) is always reached.
    pub fn flood(&self, from: u16, open: &dyn Fn(&AreaPortal) -> bool, clip_from_world: Option<Mat4>) -> Vec<bool> {
        let n = self.area_count();
        if from == 0 || from as usize >= n {
            return vec![true; n];
        }
        let mut rects: Vec<Option<ScreenRect>> = vec![None; n];
        rects[from as usize] = Some(FULL_SCREEN);
        let mut stack = vec![from];
        // Rectangles only grow, so this ends; the budget is a guard.
        let mut budget = 64 * n + 64;
        while let Some(a) = stack.pop() {
            if budget == 0 {
                warn!("areaportal flood gave up; drawing every area");
                return vec![true; n];
            }
            budget -= 1;
            let Some(rect) = rects[a as usize] else { continue };
            for &(portal, to) in &self.links[a as usize] {
                let portal = &self.portals[portal as usize];
                if !open(portal) {
                    continue;
                }
                let through = match clip_from_world {
                    Some(m) => portal_rect(&portal.polygon, m).and_then(|r| intersect(rect, r)),
                    None => Some(rect),
                };
                let Some(through) = through else { continue };
                let grown = match rects[to as usize] {
                    Some(old) => union(old, through),
                    None => through,
                };
                if rects[to as usize] != Some(grown) {
                    rects[to as usize] = Some(grown);
                    stack.push(to);
                }
            }
        }
        let mut seen: Vec<bool> = rects.iter().map(Option::is_some).collect();
        seen[0] = true;
        seen
    }

    /// The clusters with a leaf in a `reached` area (or in area 0, or with
    /// no known area), as a bit set of `cluster_count` bits.
    pub fn cluster_mask(&self, reached: &[bool], cluster_count: usize) -> Vec<u64> {
        let mut mask = vec![0u64; cluster_count.div_ceil(64)];
        for c in 0..cluster_count {
            let on = match self.cluster_areas.get(c) {
                Some(areas) if !areas.is_empty() => {
                    areas.iter().any(|&a| a == 0 || reached.get(a as usize).is_none_or(|r| *r))
                }
                _ => true,
            };
            if on {
                mask[c / 64] |= 1 << (c % 64);
            }
        }
        mask
    }
}

/// A polygon's screen rectangle under `clip_from_world` (with a margin),
/// clamped to the screen; None when it is wholly behind the eye or off
/// screen. A polygon crossing the eye's plane (or an empty one) covers the
/// whole screen.
pub fn portal_rect(polygon: &[Vec3], clip_from_world: Mat4) -> Option<ScreenRect> {
    if polygon.is_empty() {
        return Some(FULL_SCREEN);
    }
    let (mut lo, mut hi) = (Vec2::MAX, Vec2::MIN);
    let mut behind = 0;
    for p in polygon {
        let c = clip_from_world * p.extend(1.0);
        if c.w <= 1e-4 {
            behind += 1;
            continue;
        }
        let ndc = c.truncate().truncate() / c.w;
        lo = lo.min(ndc);
        hi = hi.max(ndc);
    }
    if behind == polygon.len() {
        return None;
    }
    if behind > 0 {
        return Some(FULL_SCREEN);
    }
    intersect(
        FULL_SCREEN,
        [lo.x - PORTAL_MARGIN, lo.y - PORTAL_MARGIN, hi.x + PORTAL_MARGIN, hi.y + PORTAL_MARGIN],
    )
}

fn intersect(a: ScreenRect, b: ScreenRect) -> Option<ScreenRect> {
    let r = [a[0].max(b[0]), a[1].max(b[1]), a[2].min(b[2]), a[3].min(b[3])];
    (r[0] <= r[2] && r[1] <= r[3]).then_some(r)
}

fn union(a: ScreenRect, b: ScreenRect) -> ScreenRect {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].max(b[2]), a[3].max(b[3])]
}

/// An occluder (Source's func_occluder): polygons that hide props lying
/// fully behind them, while it is active (even where the
/// polygon sticks out of the wall it sits in: world surfaces aren't
/// hidden, so a window there still shows the room, without its props).
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Occluder {
    /// What map logic calls it by (Source: `occludernumber`).
    pub key: u16,
    /// Planar convex polygons (engine space, meters).
    pub polygons: Vec<Vec<Vec3>>,
    /// Active at map start (Source: `StartActive`).
    pub start_active: bool,
}

/// A prop's bounds (engine space, under the map's root): hidden while
/// they lie fully behind an active occluder. World chunks don't get one.
#[derive(Component, Clone, Copy, Debug)]
pub struct Occludee {
    pub min: Vec3,
    pub max: Vec3,
}

/// One occluder polygon as a view sees it.
#[derive(Clone, Debug)]
pub struct ScreenOccluder {
    /// Its plane, the normal facing the eye.
    normal: Vec3,
    dist: f32,
    /// Its corners in normalized device coordinates, counter-clockwise.
    ndc: Vec<Vec2>,
    rect: ScreenRect,
}

/// How far behind an occluder's plane a box must lie (meters), and how
/// far inside its projected outline (NDC): rounding never hides an edge.
const OCCLUDER_PLANE_EPSILON: f32 = 0.01;
const OCCLUDER_SCREEN_EPSILON: f32 = 1e-4;

/// The occluder polygons a view at `eye` with `clip_from_world` can use:
/// those wholly in front of the eye, not edge on, overlapping the screen.
pub fn screen_occluders<'a>(
    polygons: impl IntoIterator<Item = &'a [Vec3]>,
    eye: Vec3,
    clip_from_world: Mat4,
) -> Vec<ScreenOccluder> {
    let mut out = Vec::new();
    'polygons: for polygon in polygons {
        if polygon.len() < 3 {
            continue;
        }
        // Newell's normal: robust for any planar polygon.
        let mut normal = Vec3::ZERO;
        for (i, a) in polygon.iter().enumerate() {
            let b = polygon[(i + 1) % polygon.len()];
            normal += Vec3::new((a.y - b.y) * (a.z + b.z), (a.z - b.z) * (a.x + b.x), (a.x - b.x) * (a.y + b.y));
        }
        let Some(mut normal) = normal.try_normalize() else { continue };
        let mut dist = normal.dot(polygon[0]);
        let side = normal.dot(eye) - dist;
        if side.abs() < OCCLUDER_PLANE_EPSILON {
            continue;
        }
        if side < 0.0 {
            (normal, dist) = (-normal, -dist);
        }
        let mut ndc = Vec::with_capacity(polygon.len());
        for p in polygon {
            let c = clip_from_world * p.extend(1.0);
            if c.w <= 1e-4 {
                continue 'polygons;
            }
            ndc.push(c.truncate().truncate() / c.w);
        }
        let area: f32 = (0..ndc.len()).map(|i| ndc[i].perp_dot(ndc[(i + 1) % ndc.len()])).sum();
        if area.abs() < 1e-8 {
            continue;
        }
        if area < 0.0 {
            ndc.reverse();
        }
        let (lo, hi) = ndc.iter().fold((Vec2::MAX, Vec2::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
        let rect = [lo.x, lo.y, hi.x, hi.y];
        if intersect(rect, FULL_SCREEN).is_none() {
            continue;
        }
        out.push(ScreenOccluder { normal, dist, ndc, rect });
    }
    out
}

/// Whether the box `min`..`max` is hidden by one of `occluders` (from
/// `screen_occluders` with the same `clip_from_world`): wholly in front of
/// the eye, wholly behind the occluder's plane, and its screen rectangle
/// inside the occluder's outline. Conservative: a box only partly hidden
/// by each of two occluders stays drawn.
pub fn occluded(occluders: &[ScreenOccluder], min: Vec3, max: Vec3, clip_from_world: Mat4) -> bool {
    if occluders.is_empty() {
        return false;
    }
    let corners: [Vec3; 8] = std::array::from_fn(|k| {
        Vec3::new(
            if k & 1 == 0 { min.x } else { max.x },
            if k & 2 == 0 { min.y } else { max.y },
            if k & 4 == 0 { min.z } else { max.z },
        )
    });
    let (mut lo, mut hi) = (Vec2::MAX, Vec2::MIN);
    for c in corners {
        let p = clip_from_world * c.extend(1.0);
        if p.w <= 1e-4 {
            return false;
        }
        let ndc = p.truncate().truncate() / p.w;
        lo = lo.min(ndc);
        hi = hi.max(ndc);
    }
    let rect = [lo, Vec2::new(hi.x, lo.y), hi, Vec2::new(lo.x, hi.y)];
    occluders.iter().any(|o| {
        lo.x >= o.rect[0]
            && lo.y >= o.rect[1]
            && hi.x <= o.rect[2]
            && hi.y <= o.rect[3]
            && corners.iter().all(|c| o.normal.dot(*c) - o.dist < -OCCLUDER_PLANE_EPSILON)
            && (0..o.ndc.len()).all(|i| {
                let (a, b) = (o.ndc[i], o.ndc[(i + 1) % o.ndc.len()]);
                rect.iter().all(|p| (b - a).perp_dot(*p - a) >= OCCLUDER_SCREEN_EPSILON * (b - a).length())
            })
    })
}

/// Decompress one run-length encoded visibility row (Quake/Source PVS: a
/// zero byte is followed by a count of zero bytes; other bytes are eight
/// clusters' bits, lowest bit first) into `cluster_count` bits.
pub fn decompress_row(data: &[u8], cluster_count: usize) -> Vec<u64> {
    let bytes_needed = cluster_count.div_ceil(8);
    let mut row = vec![0u64; cluster_count.div_ceil(64)];
    let (mut i, mut byte) = (0, 0);
    while byte < bytes_needed && i < data.len() {
        if data[i] == 0 {
            byte += data.get(i + 1).copied().unwrap_or(0) as usize;
            i += 2;
            continue;
        }
        for bit in 0..8 {
            let c = byte * 8 + bit;
            if c < cluster_count && data[i] >> bit & 1 != 0 {
                row[c / 64] |= 1 << (c % 64);
            }
        }
        byte += 1;
        i += 1;
    }
    row
}

/// Hidden when the camera is farther than this from the entity's origin,
/// meters (Source props' `fademaxdist`).
#[derive(Component, Clone, Copy, Debug)]
pub struct FadeDistance(pub f32);

/// A prop mesh's fade band (Source's `fademindist` to `fademaxdist`,
/// meters) as a Bevy visibility range: drawn fully nearer than `near`,
/// dithered out (screen-door, in the opaque passes: no sorting) toward
/// `far`, gone beyond it. The distance is from the view to the mesh's
/// origin (the prop's), per view. None without a band (`near` negative,
/// "use fademaxdist", or not below `far`): the prop pops at `far`.
/// Prop shaders apply the dither (`prop.wesl`, `prop_prepass.wesl`).
pub fn fade_band(near: f32, far: f32) -> Option<bevy::camera::visibility::VisibilityRange> {
    (near >= 0.0 && near < far).then(|| bevy::camera::visibility::VisibilityRange {
        start_margin: 0.0..0.0,
        end_margin: near..far,
        use_aabb: false,
    })
}

/// The loaded map's visibility, when it has one.
#[derive(Resource, Clone)]
pub struct ActiveVisibility(pub std::sync::Arc<MapVisibility>);

/// The world mesh chunk size, meters (a cube's edge). Chunks are cells of
/// this grid, per material: small enough that frustum and visibility
/// culling skip most of a map, large enough to keep draws few.
pub const CHUNK_SIZE: f32 = 512.0 * 0.0254;

/// The chunk size to spawn with: `CHUNK_SIZE`, or CHIMERA_CHUNK_SIZE (in
/// Source units, for tuning measurements).
pub fn chunk_size() -> f32 {
    chimera_core::env::var("CHIMERA_CHUNK_SIZE")
        .ok()
        .and_then(|v| v.parse::<f32>().ok())
        .filter(|v| *v > 0.0)
        .map_or(CHUNK_SIZE, |units| units * 0.0254)
}

/// Split a mesh into chunks by the grid cell of each triangle's centroid,
/// each with the clusters its triangles touch. With no visibility, or a
/// mesh in the 3D skybox, the mesh comes back whole with no clusters.
pub fn split_mesh(mesh: &MapMesh, vis: Option<&MapVisibility>, size: f32) -> Vec<(MapMesh, Vec<u32>)> {
    let Some(vis) = vis.filter(|_| !mesh.skybox) else {
        return vec![(mesh.clone(), Vec::new())];
    };
    let mut cells: std::collections::BTreeMap<[i32; 3], (Vec<u32>, Vec<u32>)> = Default::default();
    for tri in mesh.indices.chunks_exact(3) {
        let p = [0, 1, 2].map(|k| Vec3::from(mesh.positions[tri[k] as usize]));
        let centroid = (p[0] + p[1] + p[2]) / 3.0;
        let cell = (centroid / size).floor().as_ivec3().to_array();
        let (tris, clusters) = cells.entry(cell).or_default();
        tris.extend_from_slice(tri);
        let min = p[0].min(p[1]).min(p[2]);
        let max = p[0].max(p[1]).max(p[2]);
        vis.clusters_in_box(min, max, clusters);
    }
    // The mesh without its per-vertex data, copied into every chunk.
    let mut template = mesh.clone();
    template.indices.clear();
    template.positions.clear();
    template.normals.clear();
    template.uvs.clear();
    template.joints.clear();
    template.joint_weights.clear();
    template.lightmap_uvs.clear();
    template.blend_weights.clear();
    template.uvs2.clear();
    cells
        .into_values()
        .map(|(tris, mut clusters)| {
            clusters.sort_unstable();
            (sub_mesh(mesh, &template, &tris), clusters)
        })
        .collect()
}

/// The triangles `indices` of `mesh` as a mesh of their own (vertices
/// renumbered, every per-vertex attribute kept). `template` is the mesh
/// with its per-vertex data emptied (everything else is copied from it).
pub fn sub_mesh(mesh: &MapMesh, template: &MapMesh, indices: &[u32]) -> MapMesh {
    let mut remap: std::collections::HashMap<u32, u32> = Default::default();
    let mut used: Vec<usize> = Vec::new();
    let new_indices = indices
        .iter()
        .map(|&i| {
            *remap.entry(i).or_insert_with(|| {
                used.push(i as usize);
                used.len() as u32 - 1
            })
        })
        .collect();
    fn pick<T: Clone>(v: &[T], used: &[usize]) -> Vec<T> {
        if v.is_empty() {
            return Vec::new();
        }
        used.iter().map(|&i| v[i].clone()).collect()
    }
    MapMesh {
        positions: pick(&mesh.positions, &used),
        normals: pick(&mesh.normals, &used),
        uvs: pick(&mesh.uvs, &used),
        joints: pick(&mesh.joints, &used),
        joint_weights: pick(&mesh.joint_weights, &used),
        indices: new_indices,
        lightmap_uvs: pick(&mesh.lightmap_uvs, &used),
        blend_weights: pick(&mesh.blend_weights, &used),
        uvs2: pick(&mesh.uvs2, &used),
        source_vertices: pick(&mesh.source_vertices, &used),
        ..template.clone()
    }
}

/// Move a mesh's vertices so its bounds' centre is the origin; returns
/// that centre (where to place it).
pub fn recentre(mesh: &mut MapMesh) -> Vec3 {
    let (lo, hi) = mesh
        .positions
        .iter()
        .fold((Vec3::MAX, Vec3::MIN), |(a, b), p| (a.min(Vec3::from(*p)), b.max(Vec3::from(*p))));
    if lo.x > hi.x {
        return Vec3::ZERO;
    }
    let centre = (lo + hi) / 2.0;
    for p in &mut mesh.positions {
        *p = (Vec3::from(*p) - centre).to_array();
    }
    centre
}

/// The clusters a box touches, for tagging map parts.
pub fn box_clusters(vis: &MapVisibility, min: Vec3, max: Vec3) -> Vec<u32> {
    let mut out = Vec::new();
    vis.clusters_in_box(min, max, &mut out);
    out.sort_unstable();
    out
}

/// What one camera draws from: the clusters potentially visible from its
/// cluster that lie in an area it reaches through open areaportals (a bit
/// set), its area and how many areas it reaches. None outside the map.
/// `clip_from_world` narrows the areas to those seen through portal
/// openings; `open_all` ignores areaportals. Windows close by their
/// distance from `window_eye` (the main view's, whose window brushes are
/// drawn), even when glass is in them: their opaque brush covers it.
pub fn camera_clusters(
    v: &MapVisibility,
    eye: Vec3,
    window_eye: Vec3,
    clip_from_world: Option<Mat4>,
    states: &AreaPortalStates,
    open_all: bool,
) -> Option<(Vec<u64>, u16, usize)> {
    let leaf = v.leaf_at(eye)?;
    let cluster = usize::try_from(*v.leaf_clusters.get(leaf)?).ok()?;
    let mut row = v.visible.get(cluster)?.clone();
    let areas = &v.areas;
    let area = areas.leaf_areas.get(leaf).copied().unwrap_or(0);
    if open_all || area == 0 || areas.portals.is_empty() {
        return Some((row, area, areas.area_count()));
    }
    let open = |p: &AreaPortal| !p.window_closed(window_eye) && (p.see_through || states.is_open(p.key));
    let reached = areas.flood(area, &open, clip_from_world);
    for (w, m) in row.iter_mut().zip(areas.cluster_mask(&reached, v.cluster_count)) {
        *w &= m;
    }
    Some((row, area, reached.iter().filter(|r| **r).count()))
}

/// An areaportal window's brush (the node of the entity its `target`
/// names): drawn with the alpha its fade gives for the main view's
/// distance from the opening (`fade_windows`).
#[derive(Component, Clone, Copy, Debug)]
pub struct WindowBrush {
    /// Index in the map's `MapAreas::portals`.
    pub portal: usize,
    /// The alpha last applied.
    pub alpha: Option<f32>,
}
