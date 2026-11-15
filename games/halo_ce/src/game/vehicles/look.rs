//! What shows on a vehicle's model (specs/halo_ce/vehicles.md, "Exported
//! values and animation", "Collisions": suspension): the wheels' spin, the
//! steering and the suspension's travel as overlays of the vehicle's
//! animation graph on its bind pose, posed on the drawn prop's skeleton.
//!
//! - At load (`super::place`): the vehicle's model is a rigged prop
//!   (`MapModel::rig`: its nodes as bones, skinned meshes) whose `AnimSet`
//!   holds the graph's vehicle overlays as sequences: `STEERING` (a screen
//!   over the block's steering bounds), `GROUND_SPEED` and `suspension k`
//!   (`suspension_key`), one per suspension element of the graph, whose
//!   mass point and depths ride in the placement's map entity.
//! - Each tick, on every machine (`track`): the values the overlays play
//!   at, in `VehicleLook`: the wheel position W and turn τ, the tread
//!   positions, and each suspension element's stored value (a trace from
//!   the mass point's place with the centre of mass as origin along its up,
//!   target k = clamp(2 (1 − t), 0, 1), stored (k + previous)/2 quantised to
//!   a byte). Where the vehicle is simulated (the server, a listen host)
//!   W, τ and the treads are the drive's own; a networked client derives
//!   them from the replicated motion (no network message carries them).
//! - Each frame, where the prop is drawn (`pose`, the client side's
//!   `PropRig` joints): the bind pose, the steering screen at yaw τ and
//!   pitch 0, the ground-speed overlay at frame (W/C) × N (continuous,
//!   wrapping), each suspension overlay at frame value × (N − 1);
//!   overlay rotations compose on the right, translations add
//!   (`body::anims`' convention).
//!
//! Known shortcuts:
//! - On a networked client the speed variable s is taken as the replicated
//!   ground speed along the forward (so stuck wheels do not spin, and a
//!   sliding Warthog's do), and τ as the four-wheel turn that would give
//!   the replicated yaw rate at that speed (tan τ = ω (L/2)/v, L the
//!   wheelbase of the front and back channels' points), clamped to the
//!   turn limits and held below 0.01 WU/t; the tracks of a tank as s ∓ τ
//!   from the same estimate.
//! - The roll, throttle and velocity overlays (none on the disc), the
//!   Warthog's `firing` object overlay and the turret's aiming screen
//!   (the gun work), the `~blur` permutation and the suspension sound
//!   are not played.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::data::Kind;
use super::level::{EngineLevel, to_halo, to_halo_dir};
use super::physics::{Body, Level, PhysicsDef};
use super::{HaloVehicle, VehicleDefs};
use crate::core::SimClock;
use blam_formats::animation::{AnimationGraph, ScreenBounds, VehicleSlot};

use crate::game::body::{anims, skeleton};
use crate::map::anim::{AnimSet, BonePose, Sequence, nlerp};
use crate::map::{MapBone, PropRig};

/// The steering overlay's sequence name.
pub const STEERING: &str = "vehicle steering";
/// The ground-speed overlay's sequence name.
pub const GROUND_SPEED: &str = "vehicle ground-speed";

/// Suspension element `k`'s sequence name.
pub fn suspension_key(k: usize) -> String {
    format!("vehicle suspension {k}")
}

/// A suspension element of the vehicle's graph.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Suspension {
    /// The mass point it follows.
    pub point: usize,
    /// Full extension and full compression ground depths (WU, negative).
    pub extension: f32,
    pub compression: f32,
}

/// The values a vehicle's overlays play at (module doc).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct VehicleLook {
    /// The wheel position W in [0, C).
    pub wheel: f32,
    /// The turn variable τ (radians).
    pub turn: f32,
    /// The left and right tread positions in [0, C).
    pub treads: [f32; 2],
    /// The suspension elements and their stored values (0..1).
    pub elements: Vec<Suspension>,
    pub suspension: Vec<f32>,
    /// The last tick's body (a client's estimate).
    pub last: Option<Body>,
}

impl VehicleLook {
    /// A look for a vehicle with these suspension elements.
    pub fn new(elements: Vec<Suspension>) -> Self {
        Self {
            suspension: vec![0.0; elements.len()],
            elements,
            ..Default::default()
        }
    }
}

/// The trace of a suspension element at `q` (the mass point's model
/// position) with point up `u` on `body` (centre of mass `c`): its start
/// and vector (module doc; the start sits c_x ahead of the wheel, the
/// spec's quirk).
pub fn suspension_trace(body: &Body, q: Vec3, u: Vec3, c: Vec3, s: &Suspension) -> (Vec3, Vec3) {
    let p = body.position + body.frame() * q;
    let (e, k) = (s.extension, s.compression);
    let start = p + u * ((k - c.z) - (e - k));
    (start, u * (2.0 * (e - k)))
}

/// The target compression from the trace's hit fraction (none: 1).
pub fn suspension_target(t: Option<f32>) -> f32 {
    (2.0 * (1.0 - t.unwrap_or(1.0))).clamp(0.0, 1.0)
}

/// The stored value after a tick with target `k`: (k + previous)/2,
/// quantised to a byte.
pub fn suspension_store(k: f32, previous: f32) -> f32 {
    (((k + previous) / 2.0).clamp(0.0, 1.0) * 255.0).round() / 255.0
}

/// A body from a vehicle's drawn transform (the model origin) and its
/// centre of mass `c`.
pub fn body_from(t: &Transform, c: Vec3) -> Body {
    let axis = |a: [f32; 3]| to_halo_dir(t.rotation * crate::game::maps::space::direction(a)).normalize_or_zero();
    let mut b = Body::at(Vec3::ZERO, 0.0);
    b.forward = axis([1.0, 0.0, 0.0]);
    b.up = axis([0.0, 0.0, 1.0]);
    b.position = to_halo(t.translation) + b.frame() * c;
    b
}

/// The four-wheel turn that gives yaw rate `yaw_rate` (rad/t) at speed
/// `speed` (WU/t) with half wheelbase `half_base`, clamped to ±`limit`;
/// `None` below 0.01 WU/t.
pub fn turn_from_motion(speed: f32, yaw_rate: f32, half_base: f32, limit: f32) -> Option<f32> {
    (speed.abs() >= 0.01).then(|| (yaw_rate * half_base / speed).atan().clamp(-limit, limit))
}

/// Half the distance between the front (channel 0) and back (channel 1)
/// points along x.
fn half_base(phys: &PhysicsDef) -> f32 {
    let mean = |ch: usize| {
        let xs: Vec<f32> = phys
            .points
            .iter()
            .filter(|p| p.channel == Some(ch))
            .map(|p| p.position.x)
            .collect();
        (!xs.is_empty()).then(|| xs.iter().sum::<f32>() / xs.len() as f32)
    };
    match (mean(0), mean(1)) {
        (Some(a), Some(b)) => ((a - b) / 2.0).abs().max(0.1),
        _ => 0.65,
    }
}

/// Each tick: the values (module doc).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn track(
    mut vehicles: Query<(Entity, &HaloVehicle, &Transform, &mut VehicleLook)>,
    all: Query<Entity, With<HaloVehicle>>,
    characters: Query<(), With<crate::core::Intent>>,
    defs: Res<VehicleDefs>,
    clock: Res<SimClock>,
    query: SpatialQuery,
    shapes: Query<super::level::Shape>,
    role: Option<Res<crate::core::NetRole>>,
) {
    let n = super::super::ticks::halo_ticks(&clock);
    if n == 0 {
        return;
    }
    let simulated = role.is_none_or(|r| *r != crate::core::NetRole::Client);
    let others: Vec<Entity> = all.iter().collect();
    let skip = |e: Entity| characters.contains(e) || others.contains(&e);
    for (e, v, t, mut look) in &mut vehicles {
        let (d, phys) = &defs.0[v.def];
        let body = if simulated { v.drive.body } else { body_from(t, phys.center_of_mass) };
        if simulated {
            look.wheel = v.drive.wheel;
            look.turn = v.drive.turn;
            look.treads = v.drive.treads;
        } else if let Some(last) = look.last {
            let ticks = n as f32;
            let speed = (body.position - last.position).dot(body.forward) / ticks;
            let heading = |b: &Body| b.forward.y.atan2(b.forward.x);
            let yaw = (heading(&body) - heading(&last) + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            let limit = d.max_left_turn.abs().max(d.max_right_turn.abs()).to_radians();
            let circ = d.wheel_circumference;
            match d.kind {
                Kind::Jeep => {
                    if let Some(turn) = turn_from_motion(speed, yaw / ticks, half_base(phys), limit) {
                        look.turn = turn;
                    }
                    if circ > 0.0 {
                        look.wheel = (look.wheel + speed * ticks).rem_euclid(circ);
                    }
                }
                Kind::Tank if circ > 0.0 => {
                    // Treads at s ∓ τ: τ from the yaw rate over the treads'
                    // half track (the points' |y|).
                    let half_track = phys.points.iter().map(|p| p.position.y.abs()).fold(0.0, f32::max).max(0.1);
                    let tau = yaw / ticks * half_track;
                    look.treads[0] = (look.treads[0] + (speed - tau) * ticks).rem_euclid(circ);
                    look.treads[1] = (look.treads[1] + (speed + tau) * ticks).rem_euclid(circ);
                }
                _ => {}
            }
        }
        look.last = Some(body);
        if look.elements.is_empty() {
            continue;
        }
        let level = EngineLevel {
            query: &query,
            filter: SpatialQueryFilter::from_excluded_entities([e]).with_mask(super::level::mask()),
            skip: &skip,
            materials: None,
            shapes: &shapes,
        };
        let look = &mut *look;
        for (s, value) in look.elements.iter().zip(look.suspension.iter_mut()) {
            let Some(point) = phys.points.get(s.point) else { continue };
            let u = body.frame() * point.up;
            let (from, by) = suspension_trace(&body, point.position, u, phys.center_of_mass, s);
            let k = suspension_target(level.sweep(from, from + by).map(|(t, _)| t));
            for _ in 0..n {
                *value = suspension_store(k, *value);
            }
        }
    }
}

/// A plain overlay sequence played at fractional frame `x` onto `pose`
/// (`wrap`: frame N − 1 blends into frame 0; else clamped to the last).
pub fn overlay_at(set: &AnimSet, seq: usize, x: f32, wrap: bool, pose: &mut [BonePose]) {
    let Some(anim) = set
        .sequences
        .get(seq)
        .and_then(|s| s.anims.first())
        .and_then(|a| set.animations.get(*a))
    else {
        return;
    };
    let n = anim.frames.max(1);
    let x = if wrap { x.rem_euclid(n as f32) } else { x.clamp(0.0, (n - 1) as f32) };
    let f0 = (x.floor() as usize).min(n - 1);
    let f1 = if wrap { (f0 + 1) % n } else { (f0 + 1).min(n - 1) };
    let u = x - f0 as f32;
    for t in &anim.tracks {
        let Some(bone) = pose.get_mut(t.bone) else { continue };
        if let Some(r) = t.rotation.as_deref()
            && let (Some(a), Some(b)) = (r.get(f0.min(r.len().saturating_sub(1))), r.get(f1.min(r.len().saturating_sub(1))))
        {
            bone.0 = (bone.0 * nlerp(*a, *b, u)).normalize();
        }
        if let Some(p) = t.position.as_deref()
            && let (Some(a), Some(b)) = (p.get(f0.min(p.len().saturating_sub(1))), p.get(f1.min(p.len().saturating_sub(1))))
        {
            bone.1 += a.lerp(*b, u);
        }
    }
}

/// The vehicle's pose from its look (module doc).
pub fn posed(set: &AnimSet, look: &VehicleLook, circumference: f32) -> Vec<BonePose> {
    let mut pose = set.defaults.clone();
    if let Some(&s) = set.names.get(STEERING) {
        anims::apply_screen(set, s, look.turn, 0.0, &mut pose);
    }
    if let Some(&s) = set.names.get(GROUND_SPEED)
        && circumference > 0.0
    {
        let n = anims::frames(set, s) as f32;
        overlay_at(set, s, look.wheel / circumference * n, true, &mut pose);
    }
    for (k, value) in look.suspension.iter().enumerate() {
        if let Some(&s) = set.names.get(&suspension_key(k)) {
            let n = anims::frames(set, s) as f32;
            overlay_at(set, s, value * (n - 1.0), false, &mut pose);
        }
    }
    pose
}

/// Each frame where the prop is drawn: its joints posed (module doc).
pub fn pose(
    vehicles: Query<(&HaloVehicle, &VehicleLook, &PropRig)>,
    defs: Res<VehicleDefs>,
    mut joints: Query<&mut Transform, Without<HaloVehicle>>,
) {
    for (v, look, rig) in &vehicles {
        let d = &defs.0[v.def].0;
        let pose = posed(&rig.animator.set, look, d.wheel_circumference);
        for (joint, (q, p)) in rig.joints.iter().zip(pose) {
            if let Ok(mut t) = joints.get_mut(*joint)
                && (t.rotation != q || t.translation != p)
            {
                t.rotation = q;
                t.translation = p;
            }
        }
    }
}


/// The vehicle's overlays as an animation set for a model of `bones`
/// (module doc), the graph's suspension elements, and what was left out.
pub fn anim_set(graph: &AnimationGraph, bones: &[MapBone], checksum: u32) -> (AnimSet, Vec<Suspension>, Vec<String>) {
    let defaults = skeleton::bind_locals(bones);
    let mut set = AnimSet {
        defaults: defaults.clone(),
        bases: vec![defaults],
        parents: bones.iter().map(|b| b.parent).collect(),
        bone_names: bones.iter().map(|b| b.name.to_ascii_lowercase()).collect(),
        ..Default::default()
    };
    let mut warnings = Vec::new();
    let mut elements = Vec::new();
    let Some(block) = graph.vehicles.first() else {
        return (set, elements, warnings);
    };
    let mut add = |set: &mut AnimSet, name: String, index: usize, screen: Option<&ScreenBounds>| {
        match anims::convert(graph, index, bones.len(), checksum) {
            Ok(a) => {
                set.animations.push(a);
                let mut s = Sequence {
                    name: name.clone(),
                    anims: vec![set.animations.len() - 1],
                    grid: (1, 1),
                    delta: true,
                    ..Default::default()
                };
                if let Some(b) = screen {
                    s.grid = (b.columns(), b.rows());
                    s.axes = anims::screen_axes(b);
                    // One animation holds the whole screen (`apply_screen`
                    // reads its frames); every cell names it, so the
                    // engine's prop animator (which a party's new round
                    // starts on a prop's first sequence) indexes the grid
                    // safely.
                    s.anims = vec![set.animations.len() - 1; (s.grid.0 * s.grid.1).max(1)];
                }
                set.sequences.push(s);
                set.names.insert(name, set.sequences.len() - 1);
                true
            }
            Err(e) => {
                warnings.push(format!("vehicle animation left out: {e}"));
                false
            }
        }
    };
    if let Some(i) = block.animations.get(VehicleSlot::Steering) {
        add(&mut set, STEERING.into(), i, Some(&block.steering_bounds));
    }
    if let Some(i) = block.animations.get(VehicleSlot::GroundSpeed) {
        add(&mut set, GROUND_SPEED.into(), i, None);
    }
    for s in &block.suspension {
        let (Some(i), Ok(point)) = (s.animation, usize::try_from(s.mass_point_index)) else {
            continue;
        };
        if add(&mut set, suspension_key(elements.len()), i, None) {
            elements.push(Suspension {
                point,
                extension: s.full_extension_ground_depth,
                compression: s.full_compression_ground_depth,
            });
        }
    }
    (set, elements, warnings)
}

/// A set with the bind pose alone (a vehicle whose graph is unreadable).
pub fn anim_set_still(bones: &[MapBone]) -> AnimSet {
    let defaults = skeleton::bind_locals(bones);
    AnimSet {
        defaults: defaults.clone(),
        bases: vec![defaults],
        parents: bones.iter().map(|b| b.parent).collect(),
        bone_names: bones.iter().map(|b| b.name.to_ascii_lowercase()).collect(),
        ..Default::default()
    }
}
