//! The stand-ins' meshes (`render::stand_in`), built here in code: our
//! own low-poly shapes, no file and nothing of another game. A humanoid
//! body (`body`), a generic gun (`gun`, held, dropped and in first
//! person) and a crate (`crate_box`, any prop). Each is a few dozen flat
//! faces: cheap to keep loaded for the whole session, faceted on purpose
//! (they read as placeholders, not as a model that failed).
//!
//! Units and axes: metres, +Y up, facing -Z (a character's yaw 0 looks
//! down -Z, as the eye does). The body stands on its feet at the origin,
//! `STAND_HEIGHT` tall, holding a gun in front of its chest: its right
//! hand at `RIGHT_HAND`, where `gun`'s grip (its origin) goes. The gun's
//! barrel points down -Z. The crate is a unit cube (half size 0.5) about
//! the origin, scaled to what it stands for.
//!
//! Every vertex carries what the loading material reads
//! (`stand_in.wesl`): its UV's x is the distance along one diagonal of
//! the model (`STRIPE_AXIS`, metres: the scrolling stripes' coordinate,
//! so they cross every face at one slant and one width whatever its
//! size), y its height; its colour's alpha marks the accent parts (1: the
//! visor, the chest plate, the gun's sight rail, the crate's frame) and
//! its RGB a tone the face is multiplied by. Faces are flat (each face
//! has its own vertices and normal).
//!
//! `MeshBuilder` is the small kit they are made with (convex faces,
//! boxes, tapered prisms between two points), there for any other
//! stand-in shape.

use bevy::{
    asset::RenderAssetUsages,
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
};

/// The body's height (m): a standing character's (`character::CAPSULE_HEIGHT`).
pub const STAND_HEIGHT: f32 = 1.8;

/// Where the body's right hand holds a gun's grip (m, body space).
pub const RIGHT_HAND: Vec3 = Vec3::new(0.13, 1.2, -0.27);

/// The stripes' direction in model space: one diagonal, so they slant
/// across every face.
pub const STRIPE_AXIS: Vec3 = Vec3::new(0.62, 0.70, 0.35);

/// A mesh built from flat faces (module doc).
#[derive(Default)]
pub struct MeshBuilder {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    uvs: Vec<[f32; 2]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
    /// The tone and accent of what is added next.
    paint: [f32; 4],
}

impl MeshBuilder {
    pub fn new() -> Self {
        Self { paint: [1.0, 1.0, 1.0, 0.0], ..default() }
    }

    /// What is added next: its tone (multiplies the surface) and whether
    /// it is an accent part.
    pub fn paint(&mut self, tone: f32, accent: bool) -> &mut Self {
        self.paint = [tone, tone, tone, if accent { 1.0 } else { 0.0 }];
        self
    }

    /// A convex face, its corners counter-clockwise seen from outside.
    pub fn face(&mut self, corners: &[Vec3]) -> &mut Self {
        if corners.len() < 3 {
            return self;
        }
        let normal = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize_or_zero();
        let axis = STRIPE_AXIS.normalize();
        let first = self.positions.len() as u32;
        for c in corners {
            self.positions.push(c.to_array());
            self.normals.push(normal.to_array());
            self.uvs.push([c.dot(axis), c.y]);
            self.colors.push(self.paint);
        }
        for i in 1..corners.len() as u32 - 1 {
            self.indices.extend([first, first + i, first + i + 1]);
        }
        self
    }

    /// An axis-aligned box from `min` to `max`.
    pub fn cuboid(&mut self, min: Vec3, max: Vec3) -> &mut Self {
        let c = |x: bool, y: bool, z: bool| {
            Vec3::new(if x { max.x } else { min.x }, if y { max.y } else { min.y }, if z { max.z } else { min.z })
        };
        let (f, t) = (false, true);
        self.face(&[c(t, f, f), c(t, t, f), c(t, t, t), c(t, f, t)]) // +X
            .face(&[c(f, f, f), c(f, f, t), c(f, t, t), c(f, t, f)]) // -X
            .face(&[c(f, t, f), c(f, t, t), c(t, t, t), c(t, t, f)]) // +Y
            .face(&[c(f, f, f), c(t, f, f), c(t, f, t), c(f, f, t)]) // -Y
            .face(&[c(f, f, t), c(t, f, t), c(t, t, t), c(f, t, t)]) // +Z
            .face(&[c(f, f, f), c(f, t, f), c(t, t, f), c(t, f, f)]) // -Z
    }

    /// A prism of `sides` sides from `a` to `b`, closed at both ends, its
    /// cross-section an ellipse's polygon of radii `ra` at `a` and `rb`
    /// at `b` (x: across, toward +X where it can; y: the other way),
    /// turned so a flat side faces -Z where it can.
    pub fn prism(&mut self, a: Vec3, b: Vec3, ra: Vec2, rb: Vec2, sides: u32) -> &mut Self {
        let along = (b - a).normalize_or(Vec3::Y);
        let across = if along.x.abs() > 0.9 { Vec3::Z } else { Vec3::X };
        let across = (across - along * across.dot(along)).normalize();
        let other = along.cross(across);
        let sides = sides.max(3);
        let ring = |at: Vec3, r: Vec2| -> Vec<Vec3> {
            (0..sides)
                .map(|i| {
                    let t = (i as f32 + 0.5) / sides as f32 * std::f32::consts::TAU;
                    at + across * (t.cos() * r.x) + other * (t.sin() * r.y)
                })
                .collect()
        };
        let (ra_ring, rb_ring) = (ring(a, ra), ring(b, rb));
        for i in 0..sides as usize {
            let j = (i + 1) % sides as usize;
            self.face(&[ra_ring[i], ra_ring[j], rb_ring[j], rb_ring[i]]);
        }
        let mut cap_a = ra_ring.clone();
        cap_a.reverse();
        self.face(&cap_a).face(&rb_ring)
    }

    /// A four-sided limb from `a` to `b`, `wa` wide and `da` deep at `a`
    /// (half sizes), `wb` and `db` at `b`.
    pub fn limb(&mut self, a: Vec3, b: Vec3, wa: f32, da: f32, wb: f32, db: f32) -> &mut Self {
        // A square prism's corners sit at 45 degrees: scaled by √2 its
        // flat sides are at the half sizes asked.
        let s = std::f32::consts::SQRT_2;
        self.prism(a, b, Vec2::new(wa, da) * s, Vec2::new(wb, db) * s, 4)
    }

    /// The triangle count so far.
    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    pub fn build(self) -> Mesh {
        Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, self.uvs)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices))
    }
}

/// The humanoid body (module doc): legs, pelvis, a torso widening to the
/// shoulders, a neck and an eight-sided head with a visor, arms raised
/// to hold a gun at `RIGHT_HAND`; the visor and chest plate are accents.
pub fn body() -> Mesh {
    let mut m = MeshBuilder::new();
    let v = Vec3::new;
    for s in [-1.0f32, 1.0] {
        // Thigh, shin, foot.
        m.paint(0.92, false)
            .limb(v(s * 0.1, 0.94, 0.0), v(s * 0.105, 0.5, -0.02), 0.08, 0.09, 0.065, 0.075)
            .limb(v(s * 0.105, 0.5, -0.02), v(s * 0.11, 0.09, 0.02), 0.062, 0.07, 0.05, 0.06)
            .paint(0.8, false)
            .cuboid(v(s * 0.11 - 0.06, 0.0, -0.17), v(s * 0.11 + 0.06, 0.09, 0.07));
    }
    // Pelvis, torso, neck.
    m.paint(0.85, false)
        .limb(v(0.0, 0.86, 0.0), v(0.0, 1.0, 0.0), 0.17, 0.11, 0.17, 0.11)
        .paint(1.0, false)
        .limb(v(0.0, 1.0, 0.0), v(0.0, 1.47, 0.0), 0.16, 0.11, 0.23, 0.13)
        .paint(0.85, false)
        .limb(v(0.0, 1.47, 0.0), v(0.0, 1.54, 0.0), 0.05, 0.05, 0.05, 0.05);
    // The chest plate, standing off the torso's front.
    m.paint(1.0, true).cuboid(v(-0.12, 1.24, -0.145), v(0.12, 1.38, -0.115));
    // The head and its visor.
    m.paint(1.0, false).prism(v(0.0, 1.54, 0.0), v(0.0, 1.8, 0.0), Vec2::splat(0.115), Vec2::splat(0.1), 8);
    m.paint(1.0, true).cuboid(v(-0.085, 1.645, -0.122), v(0.085, 1.7, -0.095));
    // Arms: the right hand on the grip, the left under the barrel.
    let (r_shoulder, r_elbow) = (v(0.22, 1.43, 0.02), v(0.25, 1.2, -0.06));
    let (l_shoulder, l_elbow) = (v(-0.22, 1.43, 0.02), v(-0.2, 1.22, -0.24));
    let left_hand = v(-0.02, 1.24, -0.5);
    m.paint(0.9, false)
        .limb(r_shoulder, r_elbow, 0.055, 0.055, 0.045, 0.045)
        .limb(r_elbow, RIGHT_HAND, 0.045, 0.045, 0.038, 0.038)
        .limb(l_shoulder, l_elbow, 0.055, 0.055, 0.045, 0.045)
        .limb(l_elbow, left_hand, 0.045, 0.045, 0.038, 0.038)
        // Shoulder pads.
        .paint(0.95, false)
        .cuboid(v(0.17, 1.38, -0.08), v(0.3, 1.49, 0.1))
        .cuboid(v(-0.3, 1.38, -0.08), v(-0.17, 1.49, 0.1));
    m.paint(0.8, false)
        .cuboid(RIGHT_HAND - Vec3::splat(0.04), RIGHT_HAND + Vec3::splat(0.04))
        .cuboid(left_hand - Vec3::splat(0.04), left_hand + Vec3::splat(0.04));
    m.build()
}

/// The generic gun (module doc): a rifle-like receiver with a barrel, a
/// handguard, a magazine, a grip at the origin, a stock and an accent
/// sight rail; about 0.7 m long.
pub fn gun() -> Mesh {
    let mut m = MeshBuilder::new();
    let v = Vec3::new;
    m.paint(1.0, false)
        // Receiver, above and ahead of the hand.
        .cuboid(v(-0.026, 0.0, -0.24), v(0.026, 0.075, 0.08))
        // Handguard and barrel.
        .paint(0.9, false)
        .cuboid(v(-0.03, 0.008, -0.4), v(0.03, 0.068, -0.24))
        .paint(0.75, false)
        .prism(v(0.0, 0.045, -0.4), v(0.0, 0.045, -0.5), Vec2::splat(0.014), Vec2::splat(0.014), 6)
        // Magazine, grip, stock.
        .paint(0.8, false)
        .cuboid(v(-0.018, -0.14, -0.15), v(0.018, 0.0, -0.08))
        .cuboid(v(-0.02, -0.1, -0.02), v(0.02, 0.0, 0.035))
        .paint(0.9, false)
        .cuboid(v(-0.022, -0.045, 0.08), v(0.022, 0.06, 0.27))
        // The sight rail.
        .paint(1.0, true)
        .cuboid(v(-0.013, 0.075, -0.2), v(0.013, 0.1, 0.02));
    m.build()
}

/// The crate (module doc): a unit cube with a raised accent frame on its
/// twelve edges.
pub fn crate_box() -> Mesh {
    let mut m = MeshBuilder::new();
    m.paint(1.0, false).cuboid(Vec3::splat(-0.47), Vec3::splat(0.47));
    let (t, e) = (0.045, 0.5);
    m.paint(0.85, true);
    for axis in 0..3 {
        for (a, b) in [(-1.0f32, -1.0f32), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)] {
            let mut min = Vec3::ZERO;
            let mut max = Vec3::ZERO;
            min[axis] = -e;
            max[axis] = e;
            let (i, j) = ((axis + 1) % 3, (axis + 2) % 3);
            min[i] = a * e - if a > 0.0 { t } else { 0.0 };
            max[i] = a * e + if a < 0.0 { t } else { 0.0 };
            min[j] = b * e - if b > 0.0 { t } else { 0.0 };
            max[j] = b * e + if b < 0.0 { t } else { 0.0 };
            m.cuboid(min, max);
        }
    }
    m.build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds(mesh: &Mesh) -> (Vec3, Vec3) {
        let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = mesh.attribute(Mesh::ATTRIBUTE_POSITION) else {
            panic!("positions")
        };
        p.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p))))
    }

    /// The shapes are the size the module doc says, and cheap.
    #[test]
    fn the_shapes_have_their_sizes() {
        let (lo, hi) = bounds(&body());
        assert!(lo.y.abs() < 1e-4 && (hi.y - STAND_HEIGHT).abs() < 1e-4, "feet at 0, {STAND_HEIGHT} m tall: {lo} {hi}");
        assert!(hi.x - lo.x < 0.7 && lo.z < -0.5, "shoulder-wide, the arms forward: {lo} {hi}");
        let (lo, hi) = bounds(&gun());
        assert!((hi.z - lo.z - 0.77).abs() < 0.01 && lo.z < -0.45, "about 0.7 m, barrel down -Z: {lo} {hi}");
        let (lo, hi) = bounds(&crate_box());
        assert_eq!((lo, hi), (Vec3::splat(-0.5), Vec3::splat(0.5)));
        for (name, mesh) in [("body", body()), ("gun", gun()), ("crate", crate_box())] {
            let tris = mesh.indices().map_or(0, |i| i.len() / 3);
            assert!(tris < 600, "{name}: {tris} triangles");
            assert!(mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_some(), "{name}: colours for the material");
        }
    }

    /// Faces point outward: a box's normals point away from its centre.
    #[test]
    fn faces_point_outward() {
        let mut m = MeshBuilder::new();
        m.cuboid(Vec3::splat(-1.0), Vec3::splat(1.0)).prism(Vec3::new(5.0, 0.0, 0.0), Vec3::new(5.0, 2.0, 0.0), Vec2::ONE, Vec2::ONE, 6);
        let mesh = m.build();
        let (Some(bevy::mesh::VertexAttributeValues::Float32x3(p)), Some(bevy::mesh::VertexAttributeValues::Float32x3(n))) =
            (mesh.attribute(Mesh::ATTRIBUTE_POSITION), mesh.attribute(Mesh::ATTRIBUTE_NORMAL))
        else {
            panic!()
        };
        for (p, n) in p.iter().zip(n) {
            let (p, n) = (Vec3::from(*p), Vec3::from(*n));
            let centre = if p.x > 3.0 { Vec3::new(5.0, 1.0, 0.0) } else { Vec3::ZERO };
            assert!(n.dot(p - centre) > 0.0, "{p}: {n}");
        }
    }
}
