//! Colliders and brushes built from map models: convex hulls as brush
//! planes, smoothed terrain for loose items, a model's collider, and
//! `contact_hulls` (a convex hull as pieces whose faces have four corners
//! at most, for props', the world's, brush entities' and broken props'
//! pieces' colliders; ragdolls keep their hulls whole).
//!
//! Known shortcuts (docs/tech-debt.md): `contact_hulls` works around parry
//! clipping only the first four corners of a face in contact manifolds (a
//! TODO in parry) by cutting hulls into more pieces. A fix in parry would
//! make it unneeded.

#[allow(unused_imports)]
use super::*;
use avian3d::parry::shape::ConvexPolyhedron;

/// Share of a prop's surface that must lie on its convex hull for the hull
/// to stand in for it in exact (brush) collision. Crates and boxes with
/// shallow panel insets pass; arches and other concave props don't.
pub(super) const CONVEX_SURFACE_SHARE: f32 = 0.75;
pub(super) const MAX_HULL_PLANES: usize = 48;
/// How close (meters) a triangle must be to a hull face to count as on it.
pub(super) const ON_HULL: f32 = 0.01;

/// A model's convex hull as planes (model space), if at least `share` of
/// its surface area lies on the hull.
pub(super) fn convex_planes(model: &MapModel, share: f32) -> Option<Vec<(Vec3, f32)>> {
    let points: Vec<Vec3> = model
        .meshes
        .iter()
        .flat_map(|m| m.positions.iter().map(|p| Vec3::from(*p)))
        .collect();
    if points.len() < 4 {
        return None;
    }
    let (verts, tris) = avian3d::parry::transformation::convex_hull(&points);
    let mut planes: Vec<(Vec3, f32)> = Vec::new();
    for t in &tris {
        let [a, b, c] = t.map(|i| verts[i as usize]);
        let n = (b - a).cross(c - a);
        if n.length_squared() < 1e-12 {
            continue;
        }
        let n = n.normalize();
        let d = n.dot(a);
        if !planes.iter().any(|(m, e)| m.dot(n) > 0.9999 && (e - d).abs() < 1e-4) {
            planes.push((n, d));
        }
    }
    // Rounded props (many faces) stay meshes: little to gain, slow to place.
    if !(4..=MAX_HULL_PLANES).contains(&planes.len()) {
        return None;
    }
    let (mut on, mut total) = (0.0f32, 0.0f32);
    for m in &model.meshes {
        for t in m.indices.as_chunks::<3>().0 {
            let [a, b, c] = t.map(|i| Vec3::from(m.positions[i as usize]));
            let area = (b - a).cross(c - a).length() / 2.0;
            total += area;
            if planes
                .iter()
                .any(|(n, d)| [a, b, c].iter().all(|p| (n.dot(*p) - d).abs() < ON_HULL))
            {
                on += area;
            }
        }
    }
    (total > 0.0 && on >= share * total).then_some(planes)
}

/// Model-space planes placed in the world, with bevel planes added (so box
/// sweeps stop at edges and corners like Source's brushes:
/// `MapBrush::from_planes`).
pub(super) fn place_brush(
    planes: &[(Vec3, f32)],
    translation: Vec3,
    rotation: Quat,
    surface: Option<String>,
) -> MapBrush {
    let world: Vec<(Vec3, f32)> = planes
        .iter()
        .map(|(n, d)| {
            let n2 = rotation * *n;
            (n2, d + n2.dot(translation))
        })
        .collect();
    MapBrush {
        surface,
        ..MapBrush::from_planes(world)
    }
}

/// The collision triangles (displacements) as loose items see them: each
/// triangle comes with its own corners, so they are merged and the mesh's
/// internal edges fixed. On the plain mesh a small box sliding over it (a
/// shot weapon) catches on every edge between two triangles as on a step
/// and stops dead. Only loose items use it: physics props resting on it
/// can sink through (a de_dust2 oil drum did), so they keep the plain one.
pub(super) fn smooth_terrain(positions: Vec<Vec3>, indices: &[[u32; 3]]) -> Option<Collider> {
    let flags = TrimeshFlags::FIX_INTERNAL_EDGES
        | TrimeshFlags::DELETE_DEGENERATE_TRIANGLES
        | TrimeshFlags::DELETE_DUPLICATE_TRIANGLES;
    Collider::try_trimesh_with_config(positions, indices.to_vec(), flags)
        .inspect_err(|e| warn!("no smooth terrain for loose items: {e:?}"))
        .ok()
}

/// A model's collider in model space: its collision model's convex pieces,
/// else all its triangles.
pub(super) fn model_collider(model: &MapModel) -> Option<Collider> {
    if let Some(c) = &model.collision {
        let parts: Vec<(Vec3, Quat, Collider)> = c
            .pieces
            .iter()
            .flat_map(|p| contact_hulls(&p.points))
            .map(|hull| (Vec3::ZERO, Quat::IDENTITY, hull))
            .collect();
        if !parts.is_empty() {
            return Some(Collider::compound(parts));
        }
    }
    let mut positions = Vec::new();
    let mut indices = Vec::new();
    for m in &model.meshes {
        let base = positions.len() as u32;
        positions.extend(m.positions.iter().map(|p| Vec3::from(*p)));
        indices.extend(
            m.indices
                .as_chunks::<3>()
                .0
                .iter()
                .map(|t| [base + t[0], base + t[1], base + t[2]]),
        );
    }
    (!indices.is_empty()).then(|| Collider::trimesh(positions, indices))
}

/// A convex hull (of `points`) as colliders whose faces have at most four
/// corners each, for resting contacts; none for a degenerate hull.
///
/// Parry's contact manifolds between convex polyhedra clip only the first
/// four corners of each touching face (a TODO in its
/// `ConvexPolyhedron::local_support_feature`). A prop's octagonal base
/// then rests on a strip of itself, and on a curved floor brush (a face
/// of six or nine corners) the strip of the floor clipped can miss the
/// prop altogether: one contact point is left, which hops around the rim
/// every tick, and the prop wobbles by up to ten degrees and never sleeps
/// (de_inferno's potted plants: games/cs_source/tests/it/heavy/
/// map_prop_settle.rs). So the hull is cut along diagonals of its wider
/// faces, by planes across those faces, into pieces whose faces have four
/// corners at most: together they are the same solid, and every corner
/// of a face takes part in the contact. Corners on a straight edge (within
/// `STRAIGHT`) are dropped first. A hull that would need more than
/// `MAX_PIECES` stays whole.
pub(super) fn contact_hulls(points: &[Vec3]) -> Vec<Collider> {
    match contact_pieces(points) {
        Some(pieces) => pieces
            .into_iter()
            .map(|p| Collider::from(avian3d::parry::shape::SharedShape::new(p)))
            .collect(),
        None => Collider::convex_hull(points).into_iter().collect(),
    }
}

/// Most pieces `contact_hulls` cuts a hull into.
const MAX_PIECES: usize = 32;
/// Faces narrower than this (meters) are not cut: too small to matter.
const MIN_CUT: f32 = 0.01;
/// A face corner this close to the line through its neighbours (meters)
/// is dropped: a corner without area.
const STRAIGHT: f32 = 1e-4;

/// `contact_hulls`' pieces; None: keep the hull whole (no wide face, too
/// many pieces, or a piece parry couldn't build safely).
fn contact_pieces(points: &[Vec3]) -> Option<Vec<ConvexPolyhedron>> {
    let mut hull = safe_polyhedron(points)?;
    for _ in 0..4 {
        let Some(fewer) = without_straight_corners(&hull) else { break };
        hull = safe_polyhedron(&fewer)?;
    }
    // The planes of the wide faces: only they are cut (a cut's own faces
    // are inside the solid and touch nothing).
    let wide: Vec<(Vec3, f32)> = hull_faces(&hull)
        .filter(|(_, corners)| corners.len() > 4)
        .map(|(n, corners)| (n, n.dot(corners[0])))
        .collect();
    if wide.is_empty() {
        return None;
    }
    let mut done = Vec::new();
    let mut todo = vec![hull];
    while let Some(piece) = todo.pop() {
        if done.len() + todo.len() >= MAX_PIECES {
            return None;
        }
        let Some((m, off)) = wide_face_cut(&piece, &wide) else {
            done.push(piece);
            continue;
        };
        for side in [1.0, -1.0] {
            todo.push(safe_polyhedron(&half(&piece, m * side, off * side))?);
        }
    }
    Some(done)
}

/// The convex hull of `points` as parry builds it, or None where its
/// `ConvexPolyhedron::from_convex_mesh` would fail or never return: it
/// merges the hull's triangles within about 1.5 degrees of each other into
/// faces and walks each face's rim, which loops forever when a merged
/// face's rim isn't one simple loop (near-coplanar corners left by a cut
/// did that on de_inferno). The same merge is done here first and each
/// face's rim checked.
fn safe_polyhedron(points: &[Vec3]) -> Option<ConvexPolyhedron> {
    let (verts, tris) = avian3d::parry::transformation::try_convex_hull(points).ok()?;
    if tris.is_empty() {
        return None;
    }
    let eps = f32::EPSILON.sqrt();
    let normals: Vec<Vec3> = tris
        .iter()
        .map(|t| {
            let [a, b, c] = t.map(|i| verts[i as usize]);
            (b - a).cross(c - a).try_normalize().unwrap_or(Vec3::ZERO)
        })
        .collect();
    // Each edge's two triangles (a third, or a missing one, fails parry).
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for (k, t) in tris.iter().enumerate() {
        for i in 0..3 {
            let (a, b) = (t[i], t[(i + 1) % 3]);
            edges.entry((a.min(b), a.max(b))).or_default().push(k);
        }
    }
    // Faces: triangles joined across merged edges (union-find).
    let mut parent: Vec<usize> = (0..tris.len()).collect();
    fn root(parent: &mut [usize], mut k: usize) -> usize {
        while parent[k] != k {
            parent[k] = parent[parent[k]];
            k = parent[k];
        }
        k
    }
    let mut rims: Vec<(usize, (u32, u32))> = Vec::new();
    for (edge, ts) in &edges {
        let &[t1, t2] = ts.as_slice() else { return None };
        if normals[t1].dot(normals[t2]) > 1.0 - eps {
            let (r1, r2) = (root(&mut parent, t1), root(&mut parent, t2));
            parent[r1] = r2;
        } else {
            rims.push((t1, *edge));
            rims.push((t2, *edge));
        }
    }
    // Each face's rim must be one loop through each of its corners once.
    let mut by_face: HashMap<usize, Vec<(u32, u32)>> = HashMap::new();
    for (t, edge) in rims {
        let r = root(&mut parent, t);
        by_face.entry(r).or_default().push(edge);
    }
    for rim in by_face.values() {
        let mut degree: HashMap<u32, u32> = HashMap::new();
        for (a, b) in rim {
            *degree.entry(*a).or_default() += 1;
            *degree.entry(*b).or_default() += 1;
        }
        if rim.len() < 3 || degree.len() != rim.len() || degree.values().any(|d| *d != 2) {
            return None;
        }
        // Connected: walk the loop from one corner.
        let start = rim[0].0;
        let (mut prev, mut at, mut steps) = (rim[0].0, rim[0].1, 1);
        while at != start {
            let next = rim
                .iter()
                .find_map(|(a, b)| {
                    if *a == at && *b != prev {
                        Some(*b)
                    } else if *b == at && *a != prev {
                        Some(*a)
                    } else {
                        None
                    }
                })?;
            (prev, at) = (at, next);
            steps += 1;
            if steps > rim.len() {
                return None;
            }
        }
        if steps != rim.len() {
            return None;
        }
    }
    ConvexPolyhedron::from_convex_mesh(verts, &tris)
}

/// Each face of a hull: its outward normal and its corners in order.
fn hull_faces(hull: &ConvexPolyhedron) -> impl Iterator<Item = (Vec3, Vec<Vec3>)> + '_ {
    hull.faces().iter().map(|f| {
        let ids = &hull.vertices_adj_to_face()[f.first_vertex_or_edge as usize..][..f.num_vertices_or_edges as usize];
        (f.normal, ids.iter().map(|i| hull.points()[*i as usize]).collect())
    })
}

/// The hull's points less the face corners on a straight edge, if any.
fn without_straight_corners(hull: &ConvexPolyhedron) -> Option<Vec<Vec3>> {
    let mut straight = Vec::new();
    for (_, corners) in hull_faces(hull) {
        let n = corners.len();
        for k in 0..n {
            let (a, p, b) = (corners[(k + n - 1) % n], corners[k], corners[(k + 1) % n]);
            let along = (b - a).normalize_or_zero();
            let off = p - a;
            if (off - along * off.dot(along)).length() < STRAIGHT {
                straight.push(p);
            }
        }
    }
    (!straight.is_empty()).then(|| {
        hull.points()
            .iter()
            .copied()
            .filter(|p| !straight.contains(p))
            .collect()
    })
}

/// A cut through a face of `piece` on one of the `wide` planes that still
/// has more than four corners: the plane (normal, offset) through its
/// longest diagonal splitting its corners in halves, across the face.
fn wide_face_cut(piece: &ConvexPolyhedron, wide: &[(Vec3, f32)]) -> Option<(Vec3, f32)> {
    hull_faces(piece).find_map(|(normal, corners)| {
        let n = corners.len();
        let on_wide = wide
            .iter()
            .any(|(w, d)| w.dot(normal) > 0.9999 && (w.dot(corners[0]) - d).abs() < 1e-3);
        if n <= 4 || !on_wide {
            return None;
        }
        let (a, b) = (0..n)
            .map(|i| (corners[i], corners[(i + n / 2) % n]))
            .max_by(|x, y| x.0.distance_squared(x.1).total_cmp(&y.0.distance_squared(y.1)))?;
        if a.distance(b) < MIN_CUT {
            return None;
        }
        let m = (b - a).cross(normal).try_normalize()?;
        Some((m, m.dot(a)))
    })
}

/// The part of `piece` on the side `m . p <= off`: its corners there and
/// where its edges cross the plane.
fn half(piece: &ConvexPolyhedron, m: Vec3, off: f32) -> Vec<Vec3> {
    // Corners within `ON` of the plane are on it; every point is snapped
    // to a `ON` grid and kept once (near-duplicate corners made parry's
    // hull loop forever on a de_dust2 brush).
    const ON: f32 = 1e-4;
    let snap = |p: Vec3| (p / ON).round() * ON;
    let mut out: Vec<Vec3> = Vec::new();
    let mut add = |p: Vec3| {
        let p = snap(p);
        if !out.contains(&p) {
            out.push(p);
        }
    };
    for p in piece.points() {
        if m.dot(*p) <= off + ON {
            add(*p);
        }
    }
    for (_, corners) in hull_faces(piece) {
        for k in 0..corners.len() {
            let (p, q) = (corners[k], corners[(k + 1) % corners.len()]);
            let (dp, dq) = (m.dot(p) - off, m.dot(q) - off);
            if (dp > ON && dq < -ON) || (dp < -ON && dq > ON) {
                add(p + (q - p) * (dp / (dp - dq)));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::parry::shape::Shape;

    fn volume(points: &[Vec3]) -> f32 {
        ConvexPolyhedron::from_convex_hull(points).unwrap().mass_properties(1.0).mass()
    }

    /// A prism of a regular `n`-gon (radius `r`) from y 0 to `h`, plus a
    /// corner halfway along each of the bottom's edges when `straight`.
    fn prism(n: usize, r: f32, h: f32, straight: bool) -> Vec<Vec3> {
        let ring: Vec<Vec3> = (0..n)
            .map(|k| {
                let a = k as f32 * std::f32::consts::TAU / n as f32;
                Vec3::new(a.cos() * r, 0.0, a.sin() * r)
            })
            .collect();
        let mut pts: Vec<Vec3> = ring.iter().flat_map(|p| [*p, *p + Vec3::Y * h]).collect();
        if straight {
            pts.extend((0..n).map(|k| (ring[k] + ring[(k + 1) % n]) / 2.0));
        }
        pts
    }

    fn widest_face(points: &[Vec3]) -> usize {
        let hull = ConvexPolyhedron::from_convex_hull(points).unwrap();
        hull_faces(&hull).map(|(_, c)| c.len()).max().unwrap()
    }

    #[test]
    fn wide_faces_are_cut_into_pieces_of_four_corners_or_fewer() {
        for (n, straight) in [(8, false), (6, false), (9, false), (16, false), (5, true)] {
            let pts = prism(n, 0.127, 0.3, straight);
            let pieces = contact_pieces(&pts).expect("cut");
            for p in &pieces {
                assert!(widest_face(p.points()) <= 4, "{n}-gon: a piece keeps a face of {} corners", widest_face(p.points()));
            }
            let sum: f32 = pieces.iter().map(|p| volume(p.points())).sum();
            let whole = volume(&pts);
            assert!((sum - whole).abs() < whole * 1e-3, "{n}-gon: pieces {sum} vs whole {whole}");
        }
        // A box (with straight corners on its edges) stays whole.
        assert!(contact_pieces(&prism(4, 1.0, 1.0, true)).is_none());
        assert_eq!(contact_hulls(&prism(4, 1.0, 1.0, true)).len(), 1);
    }
}
