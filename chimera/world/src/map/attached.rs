//! Sprites and beams that stay on a character's weapon model (any game):
//! a game puts `AttachedFx` on the character each frame, saying what to
//! draw on which of the model's attachments (by index or name), and
//! `draw_attached` draws it this frame as one-frame particles
//! (`map::particles`): on the first-person view model when this
//! character's is the one drawn (the attachment posed by the view model's
//! animation and offset, mirrored with it; on the view-model layer, so it
//! is drawn over the world as the model is; against the model itself by
//! the material's own `$ignorez`: `ParticleGroup::view_model`,
//! `ParticleMaterial::no_depth`),
//! else on the weapon its body holds (`HeldAttachment`s, the held world
//! model's attachments, `MapHeldModel::attachments`). A beam's end may be a
//! point in the world instead (a gravity gun's punt): then the view
//! model's end is re-projected into the world view (`reproject`), as
//! muzzle flashes are, and the beam is drawn in the world pass.
//!
//! Locked to the attachment, the same frame: `draw_attached` runs after the
//! frame's eye (`EyePlaced`) and the view model's animation and sway
//! (`DriveAnimation`; both `order_sets`), and reads the eye from the
//! `Transform`s, as `GlobalTransform`s are last frame's until `PostUpdate`.
//!
//! Beams follow the beam primitive of specs/source/visual_entities.md 1
//! with the caller's segment count: fractal noise new every frame,
//! across the beam and the view, `noise` × the length / 100; a strip
//! twice the given width; the texture's v along it, one repeat per 100
//! units, scrolled by `scroll` lengths a second.
//!
//! Sprites and beams on a weapon's model attachments (`AttachedFx`,
//! `AttachedPiece`, `AttachPoint`, `BeamEnd`, `draw_attached` in
//! `AttachedSet`; `first_person_shown`, `attachment_point`), drawn on the
//! view model's layer in first person and at `HeldAttachment`s otherwise.
//!
//! Known shortcuts (docs/tech-debt.md): beams are drawn as separate quads
//! per segment (no mitred joints) and without shading or a halo; sprites
//! show their material's first frame; on a held world model (others' guns)
//! the attachments are last frame's `GlobalTransform`s; sprites face the
//! camera of `render::particles`, one frame old. Why it's OK for now: the
//! gravity gun's (the first user's) beams are short and thin and its
//! sprites single-frame glows; a frame's walk or turn is small on someone
//! else's gun, and a round glow's facing doesn't show.

use bevy::{prelude::*, transform::helper::TransformHelper};

use super::{
    DriveAnimation, ParticleRng, ShowLocalBody, anim,
    emitters::{BEAM_NOISE_SAMPLES, fractal_noise},
    particles::{Motion, Particle, ParticleGroup, Particles, Shape},
    view_model::*,
};

const UNIT: f32 = 0.0254;

/// A particle drawn in one frame only.
const ONE_FRAME: f32 = 0.0005;

/// An attachment of a weapon model: by its index in the model's list
/// (Source's attachment n is index n − 1) or its name (any case).
#[derive(Clone, Debug, PartialEq)]
pub enum AttachPoint {
    Index(usize),
    Named(String),
}

impl AttachPoint {
    pub fn named(name: &str) -> Self {
        Self::Named(name.to_string())
    }

    /// Whether the attachment `index` called `name` is this one.
    pub fn is(&self, index: usize, name: &str) -> bool {
        match self {
            Self::Index(i) => *i == index,
            Self::Named(n) => n.eq_ignore_ascii_case(name),
        }
    }

    /// Its index in a model's attachment list.
    pub fn index_in(&self, attachments: &[MapAttachment]) -> Option<usize> {
        match self {
            Self::Index(i) => (*i < attachments.len()).then_some(*i),
            Self::Named(n) => attachments.iter().position(|a| a.name.eq_ignore_ascii_case(n)),
        }
    }
}

/// Where a beam ends.
#[derive(Clone, Debug, PartialEq)]
pub enum BeamEnd {
    Attachment(AttachPoint),
    /// A point in the world (engine space).
    World(Vec3),
}

/// One thing drawn on a weapon's attachments this frame.
#[derive(Clone, Debug, PartialEq)]
pub enum AttachedPiece {
    /// A camera-facing sprite `width` across (meters, full width).
    Sprite {
        at: AttachPoint,
        /// Index into `MapParticles::materials`.
        material: usize,
        width: f32,
        /// sRGB 0-1 and alpha 0-1 (what the material takes of them is its
        /// own: `vertex_color`, `vertex_alpha`).
        color: Vec3,
        alpha: f32,
    },
    /// A beam from `from` to `to`, `width` at each end (meters; the strip
    /// is twice as wide), through `segments` segments.
    Beam {
        from: AttachPoint,
        to: BeamEnd,
        material: usize,
        width: [f32; 2],
        color: Vec3,
        alpha: f32,
        /// Noise amplitude (visual_entities.md 1 step 5; 0: straight).
        noise: f32,
        segments: usize,
        /// Texture lengths a second the texture slides along the beam.
        scroll: f32,
    },
}

/// What a character's weapon shows on its model's attachments this frame
/// (games set it every frame; empty lists draw nothing).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct AttachedFx {
    /// On the first-person view model, when this character's is drawn.
    pub view: Vec<AttachedPiece>,
    /// On the weapon its body holds otherwise (others see this one).
    pub world: Vec<AttachedPiece>,
}

/// `draw_attached`: games set `AttachedFx` before it.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct AttachedSet;

/// What places the frame's first-person eye (the client's camera): sprites
/// and beams on the view model are drawn after it, so they turn with it.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct EyePlaced;

/// `AttachedSet` after this frame's eye and view-model motion.
pub fn order_sets(app: &mut App) {
    app.configure_sets(Update, AttachedSet.after(EyePlaced).after(DriveAnimation));
}

/// The view model drawn for `owner` at an anchor, if one is (the drawing's
/// own rule, `render::view_model`: not in third person, not hidden, not
/// put away, `ViewModelSettings::draw`): the anchor's transform and
/// projection, the model and whether it is mirrored.
pub struct FirstPerson<'a> {
    pub anchor: Entity,
    pub eye: GlobalTransform,
    /// The world camera's and the view model camera's vertical fields of
    /// view (degrees); None without cameras (headless).
    pub fovs: Option<(f32, f32)>,
    pub model: &'a MapViewModel,
    pub placed: Transform,
    pub pose: Vec<anim::BonePose>,
}

impl FirstPerson<'_> {
    /// Attachment `at` in camera space.
    pub fn camera_point(&self, at: &AttachPoint) -> Option<Vec3> {
        let i = at.index_in(&self.model.attachments)?;
        attachment_transform(self.model, &self.pose, i, &self.placed).map(|m| m.transform_point3(Vec3::ZERO))
    }

    /// Attachment `at` where the view model is drawn (the view-model
    /// layer's world space).
    pub fn point(&self, at: &AttachPoint) -> Option<Vec3> {
        self.camera_point(at).map(|p| self.eye.transform_point(p))
    }

    /// Attachment `at` where it appears in the world view (re-projected).
    pub fn world_point(&self, at: &AttachPoint) -> Option<Vec3> {
        let p = self.camera_point(at)?;
        let p = self.fovs.map_or(p, |(w, v)| reproject(p, w, v));
        Some(self.eye.transform_point(p))
    }
}


/// Anchors: their parent (the owner unless `ViewModelSource`), transform,
/// projection and children (the view model camera).
pub type AnchorQuery<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static ChildOf, &'static GlobalTransform, Option<&'static Projection>, Option<&'static Children>, Option<&'static ViewModelSource>),
    (With<ViewModelAnchor>, Without<ViewModelCamera>),
>;

/// The first-person view model shown for `owner`, as the drawing decides.
#[allow(clippy::too_many_arguments)]
pub fn first_person_shown<'a>(
    owner: Entity,
    view: Option<&ViewAnimator>,
    offset: Option<&ViewModelOffset>,
    controls: Option<&crate::core::MapControls>,
    anchors: &AnchorQuery,
    vm_cameras: &Query<&Projection, With<ViewModelCamera>>,
    models: Option<&'a ViewModels>,
    settings: ViewModelSettings,
    third_person: bool,
    now: f64,
) -> Option<FirstPerson<'a>> {
    let view = view?;
    if third_person || settings.draw == 0 || view.hidden || controls.is_some_and(|c| c.weapon_hidden != 0) {
        return None;
    }
    let (anchor, _, eye, projection, children, _) = anchors.iter().find(|(_, p, _, _, _, s)| anchor_owner(p, *s) == owner)?;
    let model = models?.get(view.key.as_deref()?)?;
    let mirror = mirrored(model.right_handed, model.allow_flipping, settings.right_hand != 0);
    let placed = placement(&offset.copied().unwrap_or_default(), mirror);
    let pose = view.animator.as_ref().map(|a| a.pose(now)).unwrap_or_default();
    let vm = children.into_iter().flatten().find_map(|c| vm_cameras.get(*c).ok());
    let fovs = match (projection, vm) {
        (Some(Projection::Perspective(w)), Some(Projection::Perspective(v))) => {
            Some((w.fov.to_degrees(), v.fov.to_degrees()))
        }
        _ => None,
    };
    Some(FirstPerson { anchor, eye: *eye, fovs, model, placed, pose })
}

/// `owner`'s held weapon's attachment `at` (world space), if its body holds
/// one with it.
pub fn attachment_point(held: &Query<(&HeldAttachment, &GlobalTransform)>, owner: Entity, at: &AttachPoint) -> Option<Vec3> {
    held.iter()
        .find(|(h, _)| h.owner == owner && at.is(h.index, &h.name))
        .map(|(_, g)| g.translation())
}

#[derive(Resource)]
pub struct AttachedRng(ParticleRng);

impl Default for AttachedRng {
    fn default() -> Self {
        Self(ParticleRng::new(0xA77A_C4ED))
    }
}

fn frame_group(view_model: bool) -> ParticleGroup {
    let mut g = ParticleGroup::new(Motion::default());
    g.skip_first = false;
    g.capped = false;
    g.view_model = view_model;
    g
}

/// A beam's quads from `a` to `b` (visual_entities.md 1 steps 5, 7, 8),
/// facing `eye`: `segments` quads through fractal noise across the beam
/// and the view, `width` the strip's full width at each end (meters).
#[allow(clippy::too_many_arguments)]
pub fn beam_quads(
    rng: &mut ParticleRng,
    a: Vec3,
    b: Vec3,
    eye: Vec3,
    material: usize,
    width: [f32; 2],
    color: Vec3,
    alpha: f32,
    noise: f32,
    segments: usize,
    scroll_offset: f32,
) -> Vec<Particle> {
    let d = b - a;
    let length = d.length() / UNIT;
    if length < 0.1 {
        return Vec::new();
    }
    let n = segments.max(1) + 1;
    let samples = if noise != 0.0 { fractal_noise(rng) } else { [0.0; BEAM_NOISE_SAMPLES] };
    let view = (a - eye).normalize_or(Vec3::NEG_Z);
    let across = view.cross(d).normalize_or(d.any_orthonormal_vector());
    let points: Vec<Vec3> = (0..n)
        .map(|j| {
            let f = j as f32 / (n - 1) as f32;
            a + d * f + across * samples[(127.0 * f) as usize] * noise * length / 100.0 * UNIT
        })
        .collect();
    let v_step = (length / 100.0) / (n - 1) as f32;
    let mut out = Vec::with_capacity(n - 1);
    for (j, pair) in points.windows(2).enumerate() {
        let (p, q) = (pair[0], pair[1]);
        let mid = (p + q) * 0.5;
        let f = |k: usize| k as f32 / (n - 1) as f32;
        let w = |t: f32| width[0] + (width[1] - width[0]) * t;
        let side = (q - p).cross(mid - eye).normalize_or_zero();
        let (sp, sq) = (side * w(f(j)) / 2.0, side * w(f(j + 1)) / 2.0);
        let mut particle = Particle::new(mid, ONE_FRAME, material, 0.0);
        particle.color = color;
        particle.alpha = alpha;
        let v0 = scroll_offset + j as f32 * v_step;
        particle.shape = Shape::Quad {
            corners: [p - sp, q - sq, q + sq, p + sp],
            rect: [0.0, v0, 1.0, v0 + v_step],
        };
        out.push(particle);
    }
    out
}

/// Draw every character's `AttachedFx` this frame (see the module docs).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn draw_attached(
    time: Res<Time>,
    owners: Query<(
        Entity,
        &AttachedFx,
        Option<&ViewAnimator>,
        Option<&ViewModelOffset>,
        Option<&crate::core::MapControls>,
    )>,
    anchors: AnchorQuery,
    vm_cameras: Query<&Projection, With<ViewModelCamera>>,
    cameras: Query<&GlobalTransform, (With<Camera3d>, Without<ViewModelCamera>)>,
    held: Query<(&HeldAttachment, &GlobalTransform)>,
    models: Option<Res<ViewModels>>,
    settings: Option<Res<ViewModelSettings>>,
    third_person: Option<Res<ShowLocalBody>>,
    transforms: TransformHelper,
    mut rng: Local<AttachedRng>,
    mut particles: ResMut<Particles>,
) {
    let now = time.elapsed_secs_f64();
    let settings = settings.map(|s| *s).unwrap_or_default();
    let third_person = third_person.is_some_and(|t| t.0);
    // This frame's, not the last propagation's (without a `Transform`, a
    // test's fixed anchor: its `GlobalTransform`).
    let now_eye = |anchor: Entity, was: GlobalTransform| transforms.compute_global_transform(anchor).unwrap_or(was);
    let camera_eye = anchors
        .iter()
        .next()
        .map(|(e, _, g, ..)| now_eye(e, *g).translation())
        .or_else(|| cameras.iter().next().map(GlobalTransform::translation));
    for (owner, fx, view, offset, controls) in &owners {
        if fx.view.is_empty() && fx.world.is_empty() {
            continue;
        }
        let mut fp = first_person_shown(
            owner,
            view,
            offset,
            controls,
            &anchors,
            &vm_cameras,
            models.as_deref(),
            settings,
            third_person,
            now,
        );
        if let Some(f) = fp.as_mut() {
            f.eye = now_eye(f.anchor, f.eye);
        }
        let pieces = if fp.is_some() { &fx.view } else { &fx.world };
        let eye = fp.as_ref().map(|f| f.eye.translation()).or(camera_eye).unwrap_or(Vec3::ZERO);
        let mut on_model = frame_group(fp.is_some());
        let mut in_world = frame_group(false);
        for piece in pieces {
            match piece {
                AttachedPiece::Sprite { at, material, width, color, alpha } => {
                    let p = match &fp {
                        Some(f) => f.point(at),
                        None => attachment_point(&held, owner, at),
                    };
                    let Some(p) = p else { continue };
                    on_model.particles.push(Particle {
                        color: *color,
                        alpha: *alpha,
                        ..Particle::new(p, ONE_FRAME, *material, width / 2.0)
                    });
                }
                AttachedPiece::Beam { from, to, material, width, color, alpha, noise, segments, scroll } => {
                    let world_end = matches!(to, BeamEnd::World(_));
                    let point = |at: &AttachPoint| match &fp {
                        Some(f) if world_end => f.world_point(at),
                        Some(f) => f.point(at),
                        None => attachment_point(&held, owner, at),
                    };
                    let (Some(a), Some(b)) = (
                        point(from),
                        match to {
                            BeamEnd::World(p) => Some(*p),
                            BeamEnd::Attachment(at) => point(at),
                        },
                    ) else {
                        continue;
                    };
                    let offset = (now as f32 * scroll).fract();
                    let quads = beam_quads(
                        &mut rng.0, a, b, eye, *material, [width[0] * 2.0, width[1] * 2.0], *color, *alpha, *noise,
                        *segments, offset,
                    );
                    let group = if world_end { &mut in_world } else { &mut on_model };
                    group.particles.extend(quads);
                }
            }
        }
        for g in [on_model, in_world] {
            if !g.particles.is_empty() {
                particles.add(g);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attach_points_by_index_or_name() {
        let list = vec![
            MapAttachment { name: "core".into(), bone: 0, local: Transform::IDENTITY },
            MapAttachment { name: "fork1t".into(), bone: 0, local: Transform::IDENTITY },
        ];
        assert_eq!(AttachPoint::Index(0).index_in(&list), Some(0));
        assert_eq!(AttachPoint::Index(2).index_in(&list), None);
        assert_eq!(AttachPoint::named("FORK1T").index_in(&list), Some(1));
        assert!(AttachPoint::named("fork1t").is(1, "Fork1T"));
        assert!(!AttachPoint::Index(0).is(1, "core"));
    }

    #[test]
    fn a_beam_is_its_segments_wide_and_bent_by_noise() {
        let mut rng = ParticleRng::new(1);
        let (a, b) = (Vec3::ZERO, Vec3::X * 100.0 * UNIT);
        let eye = Vec3::new(0.5, 0.0, 3.0);
        let straight = beam_quads(&mut rng, a, b, eye, 0, [2.0 * UNIT, 4.0 * UNIT], Vec3::ONE, 1.0, 0.0, 8, 0.0);
        assert_eq!(straight.len(), 8);
        let Shape::Quad { corners, rect } = straight[0].shape else { panic!() };
        // Full width 2 units at the start, on the line.
        assert!(((corners[0] - corners[3]).length() / UNIT - 2.0).abs() < 1e-3);
        assert!((corners[0] + corners[3]).length() < 1e-5);
        // One texture repeat per 100 units: 1/8 a segment.
        assert!((rect[3] - rect[1] - 0.125).abs() < 1e-5);
        let Shape::Quad { corners, .. } = straight[7].shape else { panic!() };
        assert!(((corners[1] - corners[2]).length() / UNIT - 4.0).abs() < 1e-3);
        let noisy = beam_quads(&mut rng, a, b, eye, 0, [2.0 * UNIT; 2], Vec3::ONE, 1.0, 16.0, 8, 0.0);
        let off = noisy.iter().map(|p| p.position.y.abs() + p.position.z.abs()).fold(0.0, f32::max);
        // Moved across, at most 2 × 16 × 100/100 units.
        assert!(off > 0.0 && off < 32.0 * UNIT, "{off}");
    }
}
