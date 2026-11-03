// Adapted from hl2-rs (MIT, see THIRD_PARTY.md): crates/source-assets/src/eyes.rs
//! Faces of character models, for any game whose models have them: an
//! eyeball mesh (`MapEye`: drawn with its iris projected on it and lit
//! along the eyeball's own normal, as Source's Eyes shader:
//! specs/hl2dm/player_models.md 11.3-11.4) and a mouth's teeth
//! (`MapTeeth`: lit toward the mouth's forward, 11.5), set on a model's
//! meshes by its loader; where a character looks (`EyeTarget`, set by its
//! game's animation on clients) and the iris projection the drawn eye
//! takes from it (`MapEye::projection`), which `render::bodies` writes
//! into the eye's material each frame.
//!
//! The eye basis and the planar iris projection follow hl2-rs's
//! `eyes::basis` and `eyes::projection`, in world space here.
//!
//! Known shortcuts (docs/tech-debt.md): the 30 degree cone and the 6 unit
//! minimum (spec 11.3) are taken about the eyeball's own rest forward, not
//! the model's `eyes` attachment; no glint (the engine's procedural glint
//! texture, 11.4); the teeth's darkening is the mouth descriptor's weight
//! (spec open question 5), 0 with no facial animation. Why it's OK for
//! now: the eyes sit in the head bone, so the two forwards differ by the
//! eyes' own small rest angle; a close capture of a citizen settles the
//! rest (spec open question 4).

use bevy::prelude::*;

/// An eyeball (Source's Eyes shader), in the frame of its bone and the
/// skeleton's units (Source: inches).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapEye {
    /// The bone it sits on (index into the model's bones).
    pub bone: usize,
    /// The eyeball's centre.
    pub origin: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
    pub radius: f32,
    /// Iris texture repeats per unit (1 / the iris's width).
    pub iris_scale: f32,
    /// Turns the gaze slightly outward (Source's `z_offset`).
    pub z_offset: f32,
    /// The iris texture (index into `MapData::textures`).
    pub iris: Option<usize>,
}

/// A mouth whose teeth darken as it closes (Source's Teeth shader), in
/// its bone's frame.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapTeeth {
    pub bone: usize,
    /// The mouth's forward, in the bone's frame.
    pub forward: Vec3,
    /// The flex descriptor of the mouth's opening (-1: none).
    pub descriptor: i32,
}

/// Where a character's eyes look (world space, meters): its game sets it
/// on clients; without it a drawn eye looks straight ahead.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct EyeTarget(pub Vec3);

/// How far ahead an eye with no target looks, in the skeleton's units
/// (spec 10.2: a ragdoll's eyes look 1000 units ahead).
pub const AHEAD: f32 = 1000.0;

/// Most an eye turns off its rest forward, as a cosine (spec 11.3: 30
/// degrees).
pub const MAX_DEFLECTION: f32 = 0.866;

/// The nearest a target in front may be, skeleton units (spec 11.3).
pub const MIN_FORWARD: f32 = 6.0;

impl MapEye {
    /// The eye's centre, up and rest forward in world space, given its
    /// bone's world transform (scale included).
    pub fn world(&self, bone: &GlobalTransform) -> (Vec3, Vec3, Vec3) {
        let a = bone.affine();
        let origin = a.transform_point3(self.origin);
        let up = a.transform_vector3(self.up).normalize_or(Vec3::Y);
        // The stored forward is the gaze at rest (the citizens': out of
        // the face, from the eyes' centres; hl2-rs takes its negation in its
        // own bone frames).
        let ahead = a.transform_vector3(self.forward).normalize_or(Vec3::NEG_Z);
        (origin, up, ahead)
    }

    /// The point the eye aims at for world target `target` (None: ahead),
    /// pulled onto the deflection cone and pushed out to the minimum
    /// distance (spec 11.3). `scale`: world units per skeleton unit.
    pub fn aim(&self, bone: &GlobalTransform, target: Option<Vec3>, scale: f32) -> Vec3 {
        let (origin, up, ahead) = self.world(bone);
        let Some(target) = target else {
            return origin + ahead * AHEAD * scale;
        };
        let right = ahead.cross(up).normalize_or(Vec3::X);
        let up = right.cross(ahead);
        let d = target - origin;
        let mut local = Vec3::new(d.dot(ahead), d.dot(right), d.dot(up));
        local.x = local.x.max(MIN_FORWARD * scale);
        let dist = local.length();
        let mut l = local / dist.max(1e-6);
        if l.x < MAX_DEFLECTION {
            let side = Vec2::new(l.y, l.z).normalize_or(Vec2::X) * (1.0 - MAX_DEFLECTION * MAX_DEFLECTION).sqrt();
            l = Vec3::new(MAX_DEFLECTION, side.x, side.y);
        }
        origin + (ahead * l.x + right * l.y + up * l.z) * dist
    }

    /// The gaze basis toward `target` (world): forward and up, the
    /// forward turned outward by `z_offset`.
    pub fn basis(&self, bone: &GlobalTransform, target: Vec3) -> (Vec3, Vec3) {
        let (origin, up, ahead) = self.world(bone);
        let mut forward = (target - origin).try_normalize().unwrap_or(ahead);
        let right = forward.cross(up).try_normalize().unwrap_or_else(|| ahead.cross(up).normalize_or(Vec3::X));
        forward = (forward + right * (self.z_offset * 2.0)).normalize_or(ahead);
        let right = forward.cross(up).normalize_or(Vec3::X);
        (forward, right.cross(forward).normalize_or(up))
    }

    /// The iris's texture planes in world space: u = P_u · (p, 1), v =
    /// P_v · (p, 1) for a world point p, the eye's centre at (0.5, 0.5),
    /// one texture across 1 / `iris_scale` skeleton units. `scale`: world
    /// units per skeleton unit.
    pub fn projection(&self, bone: &GlobalTransform, target: Vec3, scale: f32) -> [Vec4; 2] {
        let (origin, _, _) = self.world(bone);
        let (forward, up) = self.basis(bone, target);
        let k = self.iris_scale / scale.max(1e-9);
        let u = -forward.cross(up).normalize_or(Vec3::X) * k;
        let v = -up * k;
        [u.extend(0.5 - u.dot(origin)), v.extend(0.5 - v.dot(origin))]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn eye() -> MapEye {
        MapEye {
            bone: 0,
            origin: Vec3::new(1.0, 2.0, 3.0),
            up: Vec3::Z,
            forward: Vec3::X,
            radius: 0.5,
            iris_scale: 1.5,
            z_offset: 0.0,
            iris: None,
        }
    }

    #[test]
    fn the_iris_centres_on_the_eye_and_spans_its_scale() {
        let e = eye();
        let bone = GlobalTransform::IDENTITY;
        let target = Vec3::new(100.0, 2.0, 3.0);
        let [u, v] = e.projection(&bone, target, 1.0);
        assert!((u.dot(e.origin.extend(1.0)) - 0.5).abs() < 1e-5);
        assert!((v.dot(e.origin.extend(1.0)) - 0.5).abs() < 1e-5);
        assert!((u.truncate().length() - 1.5).abs() < 1e-5);
        assert!(u.truncate().dot(v.truncate()).abs() < 1e-5);
        // Looking along +X the planes are the sideways and vertical axes.
        assert!(u.truncate().dot(Vec3::X).abs() < 1e-5);
    }

    #[test]
    fn targets_far_off_to_the_side_are_pulled_onto_the_cone() {
        // Spec test 37: 45 degrees to the side, 100 units away.
        let e = MapEye { origin: Vec3::ZERO, ..eye() };
        let bone = GlobalTransform::IDENTITY;
        let side = Vec3::new(1.0, 1.0, 0.0).normalize() * 100.0;
        let aimed = e.aim(&bone, Some(side), 1.0);
        assert!((aimed.length() - 100.0).abs() < 1e-3);
        assert!((aimed.normalize().x - 0.866).abs() < 1e-3, "{aimed}");
        // Spec test 38: 2 units in front pushed out to 6.
        let near = e.aim(&bone, Some(Vec3::new(2.0, 0.0, 0.0)), 1.0);
        assert!((near.x - 6.0).abs() < 1e-4);
    }
}
