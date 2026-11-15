//! The level as Halo's mover sees it (`mover::Space`), on the engine's
//! collision: avian shape casts of the pill (a capsule of the movement's
//! radius and segment) and of its bottom sphere against every collider a
//! character's movement meets (`core::CHARACTER_FILTER`: the map, its
//! player clips, other characters; not the player itself), in world units
//! (the engine's metres divided by `METERS_PER_UNIT`).
//!
//! The cast shapes are `SKIN` thinner than the pill and sweep `REACH`
//! further (see there): a hit can lie beyond the motion (`Hit::t` above 1),
//! where the mover's plane test decides.
//!
//! The contact's normal comes from the geometry: from the contact point to
//! the nearest point of the pill's segment where the cast touched (at an
//! edge or a vertex, the spec's rounded shapes' normal; the cast's own is
//! off by degrees there), replaced by the face's own normal (a ray from the
//! segment into it) when the two agree to within a few degrees, so a
//! floor's normal is exactly its plane's. A touch on an edge between two
//! triangles of one plane (a map's polygons are triangulated; the cast
//! reports such edges) is a touch of that plane, as collision.md gives
//! those edges no shape.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::MapBrush;

use super::{
    data::MovementData,
    mover::{Hit, Neighbour, Space, Support},
};

/// How close (cosine) a face's normal must be to the cast's to replace it.
const SAME_FACE: f32 = 0.999;

/// How much thinner (WU) the cast shapes are than the pill: a pill resting
/// exactly on a surface (where the mover puts it) would start every cast
/// touching it, where the engine's casts are unreliable (no hit, or a
/// wrong normal); a cast `SKIN` thinner starts clear of it. The mover takes
/// the contact's time from the plane (`mover::sweep`), so only grazes
/// shallower than this are missed.
pub const SKIN: f32 = 1e-3;

/// How far (m) from a ladder volume a contact still counts as on it.
const LADDER_REACH: f32 = 0.03;

/// How much further (WU) than asked a cast sweeps (`SKIN` over the
/// shallowest approach it covers: a tenth of the motion into the surface).
const REACH: f32 = 10.0 * SKIN;

/// The engine's collision in world units, for one player.
pub struct EngineSpace<'a, 'w, 's> {
    pub mover: &'a MoveAndSlide<'w, 's>,
    pub filter: SpatialQueryFilter,
    /// Metres per world unit.
    pub scale: f32,
    pub radius: f32,
    /// Climbable volumes (engine space): a contact inside one is on a
    /// ladder (`Hit::climbable`): the map's ladders (`core::MapLadders`,
    /// Halo's climbable sheets among them) and games' `LadderZone`s.
    pub ladders: &'a [MapBrush],
}

impl<'a, 'w, 's> EngineSpace<'a, 'w, 's> {
    pub fn new(mover: &'a MoveAndSlide<'w, 's>, filter: SpatialQueryFilter, d: &MovementData) -> Self {
        Self {
            mover,
            filter,
            scale: crate::game::METERS_PER_UNIT,
            radius: d.radius,
            ladders: &[],
        }
    }

    /// The space with the climbable volumes `ladders`.
    pub fn with_ladders(self, ladders: &'a [MapBrush]) -> Self {
        Self { ladders, ..self }
    }

    /// Whether a contact at `point` (WU) is on a ladder.
    fn on_ladder(&self, point: Vec3) -> bool {
        let at = point * self.scale;
        self.ladders.iter().any(|b| b.overlaps_box(at, Vec3::splat(LADDER_REACH), 0.0))
    }

    /// The pill's cast shape (metres): a capsule of segment `h` (WU),
    /// `SKIN` thinner than the pill.
    pub fn pill(&self, h: f32) -> Collider {
        Collider::capsule((self.radius - SKIN) * self.scale, h * self.scale)
    }

    /// Sweep `shape` (a segment of `h` WU up from `p`, swept round) by
    /// `motion` (WU).
    fn cast_shape(&self, shape: &Collider, p: Vec3, h: f32, motion: Vec3) -> Option<Hit> {
        let s = self.scale;
        let len = motion.length();
        if len <= 0.0 {
            return None;
        }
        // Swept `REACH` further: a pill that reaches a surface by less than
        // `SKIN` touches it where the thinner shape would not.
        let reach = motion * (1.0 + REACH / len);
        let centre = p + Vec3::Y * (h / 2.0);
        let hit = self.mover.cast_move(shape, centre * s, Quat::IDENTITY, reach * s, 0.0, &self.filter)?;
        let t = (hit.distance / (len * s)).max(0.0);
        let point = hit.point1 / s;
        // The normal from the geometry (module doc).
        let at = p + motion * t;
        let q = at + Vec3::Y * (point.y - at.y).clamp(0.0, h);
        let normal = self.contact_normal(q, point, hit.normal1.normalize_or_zero());
        Some(Hit {
            t,
            point,
            normal,
            climbable: self.on_ladder(point),
        })
    }

    /// The first face a ray from `from` (WU) along `dir` meets within
    /// `len`: its point and normal.
    fn ray(&self, from: Vec3, dir: Vec3, len: f32) -> Option<(Vec3, Vec3)> {
        let s = self.scale;
        let dir = Dir3::new(dir).ok()?;
        self.mover
            .spatial_query
            .cast_ray(from * s, dir, len * s, true, &self.filter)
            .map(|h| (from + dir * (h.distance / s), h.normal.normalize_or_zero()))
    }

    /// The contact normal of a touch at `point` by the segment's nearest
    /// point `q` (WU; `approx`: the cast's): a face's own normal when the
    /// touch is on a face or on an edge between two faces of one plane,
    /// else the rounded shape's, from `point` toward `q`.
    fn contact_normal(&self, q: Vec3, point: Vec3, approx: Vec3) -> Vec3 {
        let gap = q - point;
        let Some(n) = gap.try_normalize() else { return approx };
        let reach = gap.length() + 4.0 * SKIN;
        let Some((_, face)) = self.ray(q, -n, reach) else { return n };
        if face.dot(n) > SAME_FACE {
            return face;
        }
        // An edge between triangles of one plane is no edge (collision.md:
        // such edges get no shape): the face's plane under the segment.
        match self.ray(q, -face, reach) {
            Some((foot, under)) if under.dot(face) > 0.99999 && (point - foot).dot(face).abs() < 1e-4 => face,
            _ => n,
        }
    }
}

impl Space for EngineSpace<'_, '_, '_> {
    fn cast(&self, p: Vec3, h: f32, motion: Vec3) -> Option<Hit> {
        self.cast_shape(&self.pill(h), p, h, motion)
    }

    fn sphere_blocked(&self, p: Vec3, motion: Vec3) -> bool {
        self.cast_shape(&Collider::sphere((self.radius - SKIN) * self.scale), p, 0.0, motion)
            .is_some_and(|hit| hit.t <= 1.0)
    }

    fn support(&self, p: Vec3, n: Vec3) -> Option<Support> {
        ladder_support(self.ladders, self.scale, self.radius, p, n)
    }
}

/// A ladder brush's sheet (`maps::collision::ladder_brush`'s shape: the
/// slab's two planes, then one plane per edge in the polygon's order):
/// its plane (normal, offset; metres) and its corners in order. None for
/// a brush of another shape (a box: no two edge planes in a row meet).
fn sheet(b: &MapBrush) -> Option<(Vec3, f32, Vec<Vec3>)> {
    let skin = crate::game::maps::collision::LADDER_SKIN;
    let [(n, front), (back_n, back), edges @ ..] = b.planes.as_slice() else { return None };
    if edges.len() < 3 || n.dot(*back_n) > -0.9999 || (front + back - 2.0 * skin).abs() > 1e-4 {
        return None;
    }
    let d = front - skin;
    let corners = (0..edges.len())
        .map(|i| {
            let (a, da) = edges[(i + edges.len() - 1) % edges.len()];
            let (c, dc) = edges[i];
            let m = Mat3::from_cols(*n, a, c).transpose();
            (m.determinant().abs() > 1e-6).then(|| m.inverse() * Vec3::new(d, da, dc))
        })
        .collect::<Option<Vec<_>>>()?;
    Some((*n, d, corners))
}

/// The ladder sheet a pill of radius `r` (WU) whose point is `p` (WU)
/// rested on with contact normal `n` (`Space::support`), among the
/// climbable brushes `ladders` (engine space, `scale` metres per WU), with
/// the sheets that share an edge with it (the edge neighbour's hold), in
/// world units. Each neighbour's face is the one on the support's side
/// of the fold (the faces of a folded sheet join consistently).
pub fn ladder_support(ladders: &[MapBrush], scale: f32, r: f32, p: Vec3, n: Vec3) -> Option<Support> {
    let at = p * scale;
    let sheets: Vec<(Vec3, f32, Vec<Vec3>)> = ladders.iter().filter_map(sheet).collect();
    let (i, sense) = sheets
        .iter()
        .enumerate()
        .filter_map(|(i, (sn, d, corners))| {
            let sense = sn.dot(n).signum();
            let off = sense * (sn.dot(at) - d) - r * scale;
            let inside = corners.iter().enumerate().all(|(j, a)| {
                let b = corners[(j + 1) % corners.len()];
                let centre = corners.iter().copied().sum::<Vec3>() / corners.len() as f32;
                let mut out = (b - *a).cross(*sn).normalize_or_zero();
                if out.dot(centre - *a) > 0.0 {
                    out = -out;
                }
                out.dot(at - *a) <= r * scale
            });
            (sn.dot(n).abs() > 0.999 && off.abs() < 0.05 * scale && inside).then_some((i, sense, off.abs()))
        })
        .min_by(|a, b| a.2.total_cmp(&b.2))
        .map(|(i, sense, _)| (i, sense))?;
    let (sn, d, corners) = &sheets[i];
    let normal = *sn * sense;
    let centre_of = |c: &[Vec3]| c.iter().copied().sum::<Vec3>() / c.len() as f32;
    let tol = 1e-3;
    let mut neighbours = Vec::new();
    for (j, (on, od, oc)) in sheets.iter().enumerate() {
        if j == i {
            continue;
        }
        for (k, a) in corners.iter().enumerate() {
            let b = corners[(k + 1) % corners.len()];
            let shared = oc.iter().any(|x| x.distance(*a) < tol) && oc.iter().any(|x| x.distance(b) < tol);
            if !shared {
                continue;
            }
            // Into each sheet from the edge, in its plane.
            let mid = (*a + b) / 2.0;
            let into = |c: &[Vec3], sn: Vec3| {
                let u = (b - *a).cross(sn).normalize_or_zero();
                if u.dot(centre_of(c) - mid) < 0.0 { -u } else { u }
            };
            let (u_a, u_b) = (into(corners, *sn), into(oc, *on));
            let fold = normal.dot(u_b);
            let s = if fold.abs() < 1e-4 {
                on.dot(normal).signum()
            } else {
                (on.dot(u_a) * fold).signum()
            };
            neighbours.push(Neighbour {
                normal: *on * s,
                offset: s * od / scale,
                edge: [*a / scale, b / scale],
            });
        }
    }
    Some(Support {
        normal,
        offset: sense * d / scale,
        neighbours,
    })
}
