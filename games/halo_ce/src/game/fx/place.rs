//! Where a gun's effects go (specs/halo_ce/effects.md "Locations and
//! instances"): a location is a marker of the gun's model; an instance is
//! its frame in the world (`Frame`).
//!
//! - **First person** (`Placer::first`, the shooter's own view): the view
//!   model's attachment of the marker's name (`view::load`: attachment 0
//!   `primary trigger`, the muzzle; `primary ejection` where casings leave),
//!   posed by the view machine's pose of this tick (`ViewAnimator`'s fixed
//!   pose) and placed by the view model's root at the camera, so effects
//!   sit on the gun through its recoil, reload and walk. Where the view
//!   model is drawn with its own camera, the point is moved to where it
//!   shows in the world's view (`view_model::reproject` by the two cameras'
//!   fields of view; Halo's follow the player's, so this is the identity in
//!   the apps). A marker with several instances (the plasma rifle's vents)
//!   is all of them.
//! - **Everyone else** (`Placer::third`): the held model's attachment of the
//!   name on the holder's body (`view_model::HeldAttachment`, the drawn
//!   held model's: `primary trigger` is the third-person muzzle), last
//!   frame's.
//! - **Neither there** (a headless server, a gun whose models did not
//!   load): the first-person model's bind-pose marker relative to the
//!   camera (`FxGun::fp_markers`; the model is authored at the camera, node
//!   0 at its frame), the camera itself without one.
//!
//! A marker's frame: its +X forward, +Y left, +Z up (Halo's axes), as the
//! attachment's axes in the world.

use bevy::{ecs::system::SystemParam, prelude::*};

use super::load::FxGun;
use super::super::{METERS_PER_UNIT, maps::space};
use crate::map::{
    ViewAnimator, ViewModels,
    anim::BonePose,
    attached::AnchorQuery,
    view_model::{HeldAttachment, MapViewModel, ViewModelCamera, ViewModelOffset, anchor_owner, attachment_transform, reproject},
};

/// A location instance in the world (engine space, metres).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub at: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

impl Frame {
    /// A frame whose forward is `forward` (any up square to it).
    pub fn facing(at: Vec3, forward: Vec3) -> Frame {
        let forward = forward.normalize_or(Vec3::NEG_Z);
        let side = if forward.y.abs() < 0.9 { Vec3::Y } else { Vec3::X };
        let up = (side - forward * side.dot(forward)).normalize();
        Frame { at, forward, up }
    }

    pub fn left(&self) -> Vec3 {
        self.up.cross(self.forward)
    }

    /// A point given in the frame as Halo's (forward, left, up) in WU.
    pub fn point(&self, p: [f32; 3]) -> Vec3 {
        self.at + self.vector(p) * METERS_PER_UNIT
    }

    /// A direction given as (forward, left, up).
    pub fn vector(&self, v: [f32; 3]) -> Vec3 {
        self.forward * v[0] + self.left() * v[1] + self.up * v[2]
    }

    /// A point of the eye's space (camera space: -Z forward, +Y up, +X
    /// right, metres) in the world, this frame the eye.
    pub fn from_eye(&self, p: Vec3) -> Vec3 {
        self.at + self.eye_vector(p)
    }

    /// A direction of the eye's space in the world.
    pub fn eye_vector(&self, v: Vec3) -> Vec3 {
        -self.left() * v.x + self.up * v.y - self.forward * v.z
    }

    /// A frame from a transform whose +X is forward and +Z up (a Halo
    /// marker's axes), its point the transform's origin.
    pub fn of_marker(m: &Mat4) -> Frame {
        let forward = m.transform_vector3(Vec3::X).normalize_or(Vec3::NEG_Z);
        let up = m.transform_vector3(Vec3::Z);
        let up = (up - forward * up.dot(forward)).normalize_or(Frame::facing(Vec3::ZERO, forward).up);
        Frame { at: m.transform_point3(Vec3::ZERO), forward, up }
    }
}

/// The camera's frame: the eye and the view's rotation (the engine's yaw
/// and pitch).
pub fn camera(eye: Vec3, view: Quat) -> Frame {
    Frame { at: eye, forward: view * Vec3::NEG_Z, up: view * Vec3::Y }
}

/// Marker `name` of the first-person model in its bind pose, as the
/// shooter sees it (the fallback: module doc).
pub fn bind_pose_frame(camera: Frame, gun: &FxGun, name: &str) -> Frame {
    let p = gun.fp_markers.get(name).copied().unwrap_or([0.0; 3]);
    Frame { at: camera.point(p), ..camera }
}

/// The view model's attachments called `name` (any case) posed by `pose`
/// and placed by `placed` under the eye `camera`, in the world; with
/// `fovs` (the world camera's and the view model camera's, both vertical,
/// degrees) moved to where they show in the world's view.
pub fn view_model_frames(
    camera: Frame,
    model: &MapViewModel,
    pose: &[BonePose],
    placed: &Transform,
    fovs: Option<(f32, f32)>,
    name: &str,
) -> Vec<Frame> {
    model
        .attachments
        .iter()
        .enumerate()
        .filter(|(_, a)| a.name.eq_ignore_ascii_case(name))
        .filter_map(|(i, _)| attachment_transform(model, pose, i, placed))
        .map(|m| {
            let local = Frame::of_marker(&m);
            let at = fovs.map_or(local.at, |(w, v)| reproject(local.at, w, v));
            Frame { at: camera.from_eye(at), forward: camera.eye_vector(local.forward), up: camera.eye_vector(local.up) }
        })
        .collect()
}

/// Where a holder's gun's markers are this tick (module doc).
#[derive(SystemParam)]
pub struct Placer<'w, 's> {
    models: Option<Res<'w, ViewModels>>,
    views: Query<'w, 's, (&'static ViewAnimator, Option<&'static ViewModelOffset>)>,
    anchors: AnchorQuery<'w, 's>,
    vm_cameras: Query<'w, 's, &'static Projection, With<ViewModelCamera>>,
    held: Query<'w, 's, (&'static HeldAttachment, &'static GlobalTransform)>,
}

impl Placer<'_, '_> {
    /// The world camera's and the view model camera's vertical fields of
    /// view at `owner`'s anchor, where it has both.
    fn fovs(&self, owner: Entity) -> Option<(f32, f32)> {
        let (_, _, _, projection, children, _) = self.anchors.iter().find(|(_, p, _, _, _, s)| anchor_owner(p, *s) == owner)?;
        let vm = children.into_iter().flatten().find_map(|c| self.vm_cameras.get(*c).ok())?;
        match (projection?, vm) {
            (Projection::Perspective(w), Projection::Perspective(v)) => Some((w.fov.to_degrees(), v.fov.to_degrees())),
            _ => None,
        }
    }

    /// Marker `name` in first person: on `owner`'s view model, else the
    /// bind pose's (module doc).
    pub fn first(&self, owner: Entity, camera: Frame, gun: &FxGun, name: &str) -> Vec<Frame> {
        let on_model = self.views.get(owner).ok().and_then(|(view, offset)| {
            let model = self.models.as_deref()?.get(view.key.as_deref()?)?;
            // The view machine's pose (fixed); without one, the bind pose.
            let pose = view.animator.as_ref().map(|a| a.pose(0.0)).unwrap_or_default();
            let placed = crate::map::view_model::placement(&offset.copied().unwrap_or_default(), false);
            Some(view_model_frames(camera, model, &pose, &placed, self.fovs(owner), name))
        });
        match on_model {
            Some(f) if !f.is_empty() => f,
            _ => vec![bind_pose_frame(camera, gun, name)],
        }
    }

    /// Marker `name` as everyone else sees it: on `owner`'s held model,
    /// else the first person's bind pose (module doc).
    pub fn third(&self, owner: Entity, camera: Frame, gun: &FxGun, name: &str) -> Vec<Frame> {
        let held: Vec<Frame> = self
            .held
            .iter()
            .filter(|(h, _)| h.owner == owner && h.name.eq_ignore_ascii_case(name))
            .map(|(_, g)| Frame::of_marker(&g.to_matrix()))
            .collect();
        if held.is_empty() { vec![bind_pose_frame(camera, gun, name)] } else { held }
    }
}

/// Halo's world vector (WU per tick) as the engine's (m/s).
pub fn velocity(v: [f32; 3]) -> Vec3 {
    space::direction(v) * METERS_PER_UNIT * 30.0
}
