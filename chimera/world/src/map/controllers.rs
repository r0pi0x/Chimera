//! Controllers map logic attaches to physics bodies (specs/source/
//! physics_brushes.md 4-5): phys_thruster's constant push and turn,
//! phys_keepupright's turn back toward a goal up axis. Each physics step,
//! before the solver, they change the body's velocities; the logic says
//! which are on (`BodyControllers`). Engine space, SI units.
//!
//! Physics controllers the logic switches (`BodyControllers`): phys_thruster's
//! push and turn worked out at each turn-on (`thrust_rates`), keepupright's
//! correction (`upright_change`), phys_motor's spin (specs/hl2dm/
//! maps_entities.md 10: its target ramped by `motor_ramp`, the drive
//! `motor_drive` toward it with the inertia factor's gain and cap) and a
//! hinge motor's (`motor_change`), added to the body's velocities before
//! each physics step; map constraints as avian
//! joints (`BodyJoints`: fixed, ball, hinge, slide, length; to the world
//! through a static anchor body; off as `JointDisabled`). A joint to the
//! world with "No Collision" (physics_constraints.md 1.4: the world is the
//! other body) keeps its body out of the world's collision (its brushes and
//! static props, `WorldCollider`) while it holds (`NoWorldContacts`, dropped in the app's collision hooks:
//! `WorldContacts`), so dm_lockdown's fan, hinged inside its housing,
//! spins free.

use avian3d::prelude::{
    AngularVelocity, ComputedAngularInertia, ComputedCenterOfMass, ComputedMass, LinearVelocity, PhysicsSystems,
    Rotation, Sleeping,
};
use bevy::prelude::*;

/// One controller on a body.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyController {
    /// Who it is (the logic entity), so its turn-on values are kept.
    pub key: u64,
    pub body: Entity,
    pub kind: ControlKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ControlKind {
    /// phys_thruster: `force` (kg·m/s², world, from the thruster's
    /// angles; with "Orient Locally" `force_local`, in the body's frame)
    /// applied at `offset` (the body's frame) from the body's origin,
    /// worked out once per turn-on (`serial`) and then scaled by `scale`.
    Thrust {
        force: Vec3,
        force_local: Vec3,
        offset: Vec3,
        /// "Orient Locally": the push turns with the body.
        local: bool,
        linear: bool,
        angular: bool,
        ignore_mass: bool,
        ignore_pos: bool,
        scale: f32,
        serial: u32,
    },
    /// phys_keepupright: turn the body's up (its entity +Z, engine local
    /// +Y) toward `goal` at most `limit` rad/s per step.
    Upright { goal: Vec3, limit: f32 },
    /// phys_motor (specs/hl2dm/maps_entities.md 10): spin about `axis`
    /// (world, unit) at `speed` rad/s. Its target ramps toward `speed` over
    /// `spinup` s (0: `speed` per second, `motor_ramp`); the drive moves the
    /// spin toward the target by `inertia_factor` × the step of the gap,
    /// at most `inertia_factor` × (ramp + `add_accel` rad/s²) × the step
    /// (`motor_drive`).
    Motor {
        axis: Vec3,
        speed: f32,
        spinup: f32,
        inertia_factor: f32,
        add_accel: f32,
    },
    /// A hinge's motor: the spin about `axis` (world, unit) set to `speed`
    /// rad/s at once (`motor_change` with no spin-up).
    Spin { axis: Vec3, speed: f32 },
    /// A slider's motor: move along `axis` (world, unit) at `speed` m/s,
    /// at once.
    Slide { axis: Vec3, speed: f32 },
}

/// What a joint holds (`BodyJoint`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum JointKind {
    Fixed,
    Ball,
    Hinge {
        axis: Vec3,
    },
    Slide {
        axis: Vec3,
    },
    /// The bodies at most `max` (at least `min`) meters apart, from
    /// `other` on the first to the anchor on the second.
    Length {
        min: f32,
        max: f32,
        other: Vec3,
    },
}

/// A joint between two bodies (`body1` None: the world) at `anchor`
/// (world, engine space), from a map constraint entity; the logic layer
/// writes them (`BodyJoints`).
#[derive(Clone, Debug, PartialEq)]
pub struct BodyJoint {
    pub key: u64,
    pub kind: JointKind,
    pub body1: Option<Entity>,
    pub body2: Entity,
    pub anchor: Vec3,
    pub on: bool,
    /// The two bodies don't collide while it holds.
    pub no_collide: bool,
    /// It breaks when its force (N) or torque (N·m) passes these
    /// (physics_constraints.md 1.5; 0: never): `JointBroke`.
    pub force_limit: f32,
    pub torque_limit: f32,
}

/// A joint passed its break limit this step (`BodyJoint::key`): the logic
/// breaks the constraint (OnBreak, removed).
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct JointBroke(pub u64);

/// A made joint's key and break limits.
#[derive(Component, Clone, Copy, Debug)]
struct JointLimits {
    key: u64,
    force: f32,
    torque: f32,
}

/// Joints whose force or torque passed their limit this step go off and
/// are reported once (the logic removes them).
fn break_joints(
    joints: Query<(Entity, &JointLimits, &avian3d::prelude::JointForces), Without<avian3d::prelude::JointDisabled>>,
    mut broke: MessageWriter<JointBroke>,
    mut commands: Commands,
) {
    for (e, l, f) in &joints {
        let force = l.force > 0.0 && f.force().length() > l.force;
        let torque = l.torque > 0.0 && f.torque().length() > l.torque;
        if force || torque {
            commands.entity(e).insert(avian3d::prelude::JointDisabled);
            broke.write(JointBroke(l.key));
        }
    }
}

/// Marks the world's own colliders (its brushes and terrain), which a
/// body hinged to the world with "No Collision" doesn't touch.
#[derive(Component, Debug, Clone, Copy)]
pub struct WorldCollider;

/// On a body joined to the world by a joint with "No Collision" that is
/// on: its colliders don't touch the world's (`WorldContacts`).
#[derive(Component, Debug, Clone, Copy)]
pub struct NoWorldContacts;

/// For the app's collision hooks: a `NoWorldContacts` body never touches
/// a `WorldCollider`.
#[derive(bevy::ecs::system::SystemParam)]
pub struct WorldContacts<'w, 's> {
    marked: Query<'w, 's, (), With<NoWorldContacts>>,
    world: Query<'w, 's, (), With<WorldCollider>>,
    of: Query<'w, 's, &'static avian3d::prelude::ColliderOf>,
}

impl WorldContacts<'_, '_> {
    /// Some(false) when colliders `a` and `b` are such a body's and the
    /// world's; None otherwise.
    pub fn filter(&self, a: Entity, b: Entity) -> Option<bool> {
        if self.marked.is_empty() {
            return None;
        }
        let body = |e: Entity| self.of.get(e).map_or(e, |o| o.body);
        for (me, other) in [(a, b), (b, a)] {
            if self.marked.contains(body(me)) && self.world.contains(other) {
                return Some(false);
            }
        }
        None
    }
}

/// Put `NoWorldContacts` on the bodies whose world joints with "No
/// Collision" are on, and take it off the others. The body's colliders get
/// the pair filter, and are switched off and on once so that pairs the
/// broad phase already made with the world go and the filter decides anew.
/// (Part of `sync_joints`: a system of its own reorders the schedule.)
fn sync_world_contacts(
    joints: &BodyJoints,
    marked: &Query<Entity, With<NoWorldContacts>>,
    bodies: &Query<Option<&avian3d::prelude::RigidBodyColliders>>,
    colliders: &Query<(Option<&avian3d::prelude::ActiveCollisionHooks>, Has<avian3d::prelude::ColliderDisabled>)>,
    commands: &mut Commands,
) {
    use avian3d::prelude::{ActiveCollisionHooks, ColliderDisabled};
    let wanted: Vec<Entity> = joints
        .0
        .iter()
        .filter(|j| j.body1.is_none() && j.no_collide && j.on)
        .map(|j| j.body2)
        .filter(|b| bodies.contains(*b))
        .collect();
    let renew = |body: Entity, commands: &mut Commands| {
        let Ok(list) = bodies.get(body) else { return };
        let list: Vec<Entity> = match list {
            Some(l) => l.iter().collect(),
            None => vec![body],
        };
        for c in list {
            let Ok((hooks, disabled)) = colliders.get(c) else { continue };
            let hooks = hooks.copied().unwrap_or_default() | ActiveCollisionHooks::FILTER_PAIRS;
            let mut e = commands.entity(c);
            e.try_insert(hooks);
            if !disabled {
                e.try_insert(ColliderDisabled).try_remove::<ColliderDisabled>();
            }
        }
    };
    for b in &wanted {
        if !marked.contains(*b) {
            commands.entity(*b).try_insert(NoWorldContacts);
            renew(*b, commands);
        }
    }
    for b in marked {
        if !wanted.contains(&b) {
            commands.entity(b).try_remove::<NoWorldContacts>();
            renew(b, commands);
        }
    }
}

/// The joints that exist now.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BodyJoints(pub Vec<BodyJoint>);

/// A spinning motor's change of angular velocity for one step of `h`
/// seconds: the rate about `axis` moves toward `speed`, by at most
/// speed/spinup per second (at once with no spin-up).
pub fn motor_change(omega: Vec3, axis: Vec3, speed: f32, spinup: f32, h: f32) -> Vec3 {
    let now = omega.dot(axis);
    let step = if spinup > 0.0 {
        speed.abs() / spinup * h
    } else {
        f32::INFINITY
    };
    let next = now + (speed - now).clamp(-step, step);
    axis * (next - now)
}

/// phys_motor's ramp rate (rad/s²): speed/spinup, or speed per second
/// with no spin-up.
pub fn motor_accel(speed: f32, spinup: f32) -> f32 {
    if spinup > 0.0 { speed.abs() / spinup } else { speed.abs() }
}

/// phys_motor's target one step of `h` seconds later: toward `speed` by at
/// most `accel`·h (spec 10's ramp, every tick while ramping, the TurnOn
/// tick too).
pub fn motor_ramp(target: f32, speed: f32, accel: f32, h: f32) -> f32 {
    let step = accel * h;
    target + (speed - target).clamp(-step, step)
}

/// phys_motor's change of angular velocity for one step of `h` seconds
/// (spec 10's drive): the rate about `axis` moves toward `target` by
/// k·h of the gap (all of it when k·h ≥ 1), at most k·`cap`·h, where k is
/// the inertia factor and `cap` the ramp rate plus the extra headroom.
/// Nothing when the target is 0 (the body coasts).
pub fn motor_drive(omega: Vec3, axis: Vec3, target: f32, k: f32, cap: f32, h: f32) -> Vec3 {
    if target == 0.0 {
        return Vec3::ZERO;
    }
    let gap = target - omega.dot(axis);
    let limit = k * cap * h;
    axis * (gap * (k * h).min(1.0)).clamp(-limit, limit)
}

/// The controllers on now (the logic layer writes it).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BodyControllers(pub Vec<BodyController>);

/// A thruster's acceleration and angular acceleration from its turn-on
/// (local or world frame).
#[derive(Clone, Copy, Debug)]
struct Started {
    serial: u32,
    linear: Vec3,
    angular: Vec3,
    local: bool,
}

pub struct ControllersPlugin;

impl Plugin for ControllersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BodyControllers>()
            .init_resource::<BodyJoints>()
            .add_message::<JointBroke>()
            .add_systems(
                FixedPostUpdate,
                (sync_joints, apply).chain().before(PhysicsSystems::StepSimulation),
            )
            .add_systems(FixedPostUpdate, break_joints.after(PhysicsSystems::Writeback));
    }
}

/// Velocity changes one thruster turn-on works out (spec 4): a = F/m
/// (or the force itself as an acceleration with "Ignore Mass"), α =
/// I⁻¹(r × F) about the body's centre of mass, `r` from it (none with
/// "Ignore Pos"); in the
/// body's frame when local.
pub fn thrust_rates(
    force: Vec3,
    r: Vec3,
    rotation: Quat,
    mass: f32,
    inverse_inertia_world: impl Fn(Vec3) -> Vec3,
    flags: (bool, bool, bool, bool, bool),
) -> (Vec3, Vec3) {
    let (local, linear, angular, ignore_mass, ignore_pos) = flags;
    let force = if ignore_mass { force * mass } else { force };
    let a = if linear && mass > 0.0 { force / mass } else { Vec3::ZERO };
    let r = if ignore_pos { Vec3::ZERO } else { r };
    let alpha = if angular { inverse_inertia_world(r.cross(force)) } else { Vec3::ZERO };
    if local {
        (rotation.inverse() * a, rotation.inverse() * alpha)
    } else {
        (a, alpha)
    }
}

/// phys_keepupright's change of angular velocity for one step of `h`
/// seconds (spec 5): in the body's frame, drive the rate about the
/// correcting axis to θ/h, capped at `limit`, damping the rate about it.
/// `omega` and the result are world rad/s.
pub fn upright_change(rotation: Quat, omega: Vec3, goal: Vec3, limit: f32, h: f32) -> Vec3 {
    let up = Vec3::Y;
    let g = (rotation.inverse() * goal).normalize_or_zero();
    let cross = up.cross(g);
    let len = cross.length();
    if len < 1e-6 {
        // Upright, or upside down (axis undefined): nothing.
        return Vec3::ZERO;
    }
    let k = cross / len;
    let theta = len.atan2(up.dot(g));
    let w = rotation.inverse() * omega;
    let mut want = k * (theta / h) - k * w.dot(k);
    let m = want.length();
    if m > limit && m > 0.0 {
        want *= limit / m;
    }
    rotation * want
}

#[allow(clippy::type_complexity)]
fn apply(
    controllers: Res<BodyControllers>,
    mut started: Local<std::collections::HashMap<u64, Started>>,
    mut targets: Local<std::collections::BTreeMap<u64, f32>>,
    mut bodies: Query<(
        &mut LinearVelocity,
        &mut AngularVelocity,
        &Rotation,
        Option<&ComputedMass>,
        Option<&ComputedAngularInertia>,
        Option<&ComputedCenterOfMass>,
        Has<Sleeping>,
    )>,
    time: Res<Time<Fixed>>,
    mut commands: Commands,
) {
    let h = time.timestep().as_secs_f32();
    started.retain(|k, _| controllers.0.iter().any(|c| c.key == *k));
    // A motor turned off (gone from the list) drops its target to 0.
    targets.retain(|k, _| controllers.0.iter().any(|c| c.key == *k));
    for c in &controllers.0 {
        let Ok((mut v, mut w, rot, mass, inertia, centre, sleeping)) = bodies.get_mut(c.body) else {
            continue;
        };
        let rotation = rot.0;
        let (dv, dw) = match &c.kind {
            ControlKind::Thrust {
                force,
                force_local,
                offset,
                local,
                linear,
                angular,
                ignore_mass,
                ignore_pos,
                scale,
                serial,
            } => {
                let s = match started.get(&c.key) {
                    Some(s) if s.serial == *serial => *s,
                    _ => {
                        let m = mass.map_or(1.0, |m| m.value());
                        let inv = |t: Vec3| match inertia {
                            Some(i) => rotation * (i.inverse() * (rotation.inverse() * t)),
                            None => Vec3::ZERO,
                        };
                        let f = if *local { rotation * *force_local } else { *force };
                        // The arm is from the body's centre of mass, not its
                        // origin (CS:S, measured 2026-10-18, spec Q3: a
                        // thruster level with an oil drum's centre spins it
                        // in place, one level with its origin at the base
                        // tips it).
                        let com = centre.map_or(Vec3::ZERO, |c| c.0);
                        let (linear, angular) = thrust_rates(
                            f,
                            rotation * (*offset - com),
                            rotation,
                            m,
                            inv,
                            (*local, *linear, *angular, *ignore_mass, *ignore_pos),
                        );
                        let s = Started {
                            serial: *serial,
                            linear,
                            angular,
                            local: *local,
                        };
                        started.insert(c.key, s);
                        // Turning on wakes the body (one asleep since it
                        // spawned too).
                        let body = c.body;
                        commands.queue(move |w: &mut World| super::prop_physics::wake(w, body));
                        s
                    }
                };
                let (a, alpha) = if s.local {
                    (rotation * s.linear, rotation * s.angular)
                } else {
                    (s.linear, s.angular)
                };
                (a * *scale * h, alpha * *scale * h)
            }
            ControlKind::Upright { goal, limit } => (Vec3::ZERO, upright_change(rotation, w.0, *goal, *limit, h)),
            ControlKind::Motor {
                axis,
                speed,
                spinup,
                inertia_factor,
                add_accel,
            } => {
                let accel = motor_accel(*speed, *spinup);
                let target = targets.entry(c.key).or_insert(0.0);
                *target = motor_ramp(*target, *speed, accel, h);
                (
                    Vec3::ZERO,
                    motor_drive(w.0, *axis, *target, *inertia_factor, accel + *add_accel, h),
                )
            }
            ControlKind::Spin { axis, speed } => (Vec3::ZERO, motor_change(w.0, *axis, *speed, 0.0, h)),
            ControlKind::Slide { axis, speed } => (*axis * (*speed - v.0.dot(*axis)), Vec3::ZERO),
        };
        if dv == Vec3::ZERO && dw == Vec3::ZERO {
            continue;
        }
        v.0 += dv;
        w.0 += dw;
        if sleeping {
            commands.entity(c.body).remove::<Sleeping>();
        }
    }
}

/// The joint entities made for `BodyJoints`: by key, the joint, the
/// static body standing in for the world, and what it was made from.
#[derive(Default)]
struct MadeJoints(std::collections::HashMap<u64, (Entity, Option<Entity>, BodyJoint)>);

/// Make, switch and remove joint entities to match `BodyJoints`. A joint
/// to the world holds its body to a static body at the anchor.
fn sync_joints(
    joints: Res<BodyJoints>,
    mut made: Local<MadeJoints>,
    marked: Query<Entity, With<NoWorldContacts>>,
    bodies: Query<Option<&avian3d::prelude::RigidBodyColliders>>,
    colliders: Query<(Option<&avian3d::prelude::ActiveCollisionHooks>, Has<avian3d::prelude::ColliderDisabled>)>,
    mut commands: Commands,
) {
    sync_world_contacts(&joints, &marked, &bodies, &colliders, &mut commands);
    use avian3d::prelude::{
        DistanceJoint, FixedJoint, JointCollisionDisabled, JointDisabled, PrismaticJoint, RevoluteJoint, RigidBody,
        SphericalJoint,
    };
    if !joints.is_changed() && joints.0.len() == made.0.len() {
        return;
    }
    let wanted: std::collections::HashSet<u64> = joints.0.iter().map(|j| j.key).collect();
    made.0.retain(|key, (joint, anchor, _)| {
        let keep = wanted.contains(key);
        if !keep {
            commands.entity(*joint).try_despawn();
            if let Some(a) = anchor {
                commands.entity(*a).try_despawn();
            }
        }
        keep
    });
    for j in &joints.0 {
        if let Some((joint, _, was)) = made.0.get_mut(&j.key) {
            if was.on != j.on {
                if j.on {
                    commands.entity(*joint).try_remove::<JointDisabled>();
                } else {
                    commands.entity(*joint).try_insert(JointDisabled);
                }
                was.on = j.on;
            }
            continue;
        }
        let world_body = j.body1.is_none().then(|| {
            commands
                .spawn((
                    Name::new("Joint anchor"),
                    super::MapPart,
                    RigidBody::Static,
                    Transform::from_translation(j.anchor),
                ))
                .id()
        });
        let Some(b1) = j.body1.or(world_body) else { continue };
        let b2 = j.body2;
        let basis = |from: Vec3, to: Vec3| Quat::from_rotation_arc(from, to.try_normalize().unwrap_or(from));
        let mut e = commands.spawn((Name::new("Joint"), super::MapPart));
        match j.kind {
            JointKind::Fixed => {
                e.insert(FixedJoint::new(b1, b2).with_anchor(j.anchor));
            }
            JointKind::Ball => {
                e.insert(SphericalJoint::new(b1, b2).with_anchor(j.anchor));
            }
            JointKind::Hinge { axis } => {
                e.insert(
                    RevoluteJoint::new(b1, b2)
                        .with_anchor(j.anchor)
                        .with_basis(basis(Vec3::Z, axis))
                        .with_hinge_axis(Vec3::Z),
                );
            }
            JointKind::Slide { axis } => {
                let mut p = PrismaticJoint::new(b1, b2)
                    .with_anchor(j.anchor)
                    .with_basis(basis(Vec3::X, axis));
                p.slider_axis = Vec3::X;
                e.insert(p);
            }
            JointKind::Length { min, max, other } => {
                let mut d = DistanceJoint::new(b1, b2).with_limits(min.min(max), max);
                d.anchor1 = avian3d::prelude::JointAnchor::FromGlobal(other);
                d.anchor2 = avian3d::prelude::JointAnchor::FromGlobal(j.anchor);
                e.insert(d);
            }
        }
        if j.no_collide {
            e.insert(JointCollisionDisabled);
        }
        if j.force_limit > 0.0 || j.torque_limit > 0.0 {
            e.insert((
                avian3d::prelude::JointForces::new(),
                JointLimits {
                    key: j.key,
                    force: j.force_limit,
                    torque: j.torque_limit,
                },
            ));
        }
        if !j.on {
            e.insert(JointDisabled);
        }
        let id = e.id();
        made.0.insert(j.key, (id, world_body, j.clone()));
    }
}

#[cfg(test)]
mod motor_tests {
    use super::*;

    #[test]
    fn a_hinge_motor_sets_its_spin_at_once() {
        let speed = 100f32.to_radians();
        assert!((motor_change(Vec3::ZERO, Vec3::Y, speed, 0.0, 0.1).y - speed).abs() < 1e-5);
        assert_eq!(motor_change(Vec3::Y * speed, Vec3::Y, speed, 0.0, 0.1), Vec3::ZERO);
        // Other axes are left alone.
        assert_eq!(motor_change(Vec3::X, Vec3::Y, 0.0, 1.0, 0.1), Vec3::ZERO);
    }

    /// maps_entities.md test case 35: speed 1500, spinup 0, TurnOn at tick
    /// k: target 22.5 at k, +22.5 a tick, 1485 at k + 65, 1500 from k + 66.
    #[test]
    fn phys_motor_ramps_its_target() {
        let h = 0.015;
        let a = motor_accel(1500.0, 0.0);
        assert_eq!(a, 1500.0);
        assert_eq!(motor_accel(-90.0, 2.0), 45.0);
        let mut t = 0.0;
        let mut seen = Vec::new();
        for _ in 0..=67 {
            t = motor_ramp(t, 1500.0, a, h);
            seen.push(t);
        }
        assert!((seen[0] - 22.5).abs() < 1e-3);
        assert!((seen[1] - 45.0).abs() < 1e-3);
        assert!((seen[65] - 1485.0).abs() < 1e-2, "{}", seen[65]);
        assert_eq!(seen[66], 1500.0);
        assert_eq!(seen[67], 1500.0);
    }

    /// The drive closes k·h of the gap (all of it once k·h ≥ 1), capped
    /// at k·(ramp + headroom)·h, and does nothing at target 0.
    #[test]
    fn phys_motor_drives_toward_its_target() {
        let x = Vec3::X;
        // dm_lockdown's fan: k 125, a full step closes the gap.
        let dw = motor_drive(Vec3::X * 10.0, x, 26.18, 125.0, 26.18, 0.015);
        assert!((dw.x - 16.18).abs() < 1e-3);
        // k 1: a sixty-sixth of the gap.
        let dw = motor_drive(Vec3::ZERO, x, 1.0, 1.0, 100.0, 0.015);
        assert!((dw.x - 0.015).abs() < 1e-6);
        // Capped: k·cap·h.
        let dw = motor_drive(Vec3::ZERO, x, 100.0, 10.0, 1.0, 0.015);
        assert!((dw.x - 0.15).abs() < 1e-6);
        // Off: the body coasts.
        assert_eq!(motor_drive(Vec3::X * 5.0, x, 0.0, 125.0, 26.0, 0.015), Vec3::ZERO);
        // Only the axis's part changes.
        let dw = motor_drive(Vec3::new(1.0, 3.0, 0.0), x, 2.0, 125.0, 26.0, 0.015);
        assert_eq!(dw.y, 0.0);
    }
}
