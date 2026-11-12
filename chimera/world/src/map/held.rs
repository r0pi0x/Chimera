//! Physics bodies a player holds (lift C6, docs/plans/active/hl2dm.md;
//! specs/hl2dm/gravity_gun.md 4-6, 14): what any game's gravity gun or
//! `+use` carry needs of the engine, the game's rules (what may be held,
//! where it goes) staying the game's.
//!
//! - `Held` on the body: its holder, its target pose (the game writes it
//!   each tick), the controller's limits, and the target relative to the
//!   holder's view (`view_offset`: what the holder's client draws it at,
//!   `chimera_net`'s `NetHold`). `hold` and `release` put it on and take
//!   it off, keeping the body's mass, damping and gravity (`HeldSaved`)
//!   while it weighs `mass` and has `angular_damping`, no linear damping
//!   and no gravity.
//! - **The controller** (`drive`, each physics step before the solver;
//!   specs/hl2dm/gravity_gun.md 5): first the contact factor (`contact`,
//!   0 at the grab) moves toward 0.1 while the body touches anything
//!   immovable or heavier than the load (`load`, kg), else toward 1, at 2
//!   a second (`CONTACT_RATE`); the body's velocity becomes what reaches
//!   the target in `arrive` seconds, cut to `max_speed`; its spin likewise
//!   toward the target rotation, cut to `max_angular` × contact³; then,
//!   against each such contact, the velocity's part into its normal is
//!   taken off (`slide`): it slides along walls instead of pressing into
//!   them. Contacts are the last step's (the contact graph, points no
//!   farther apart than `CONTACT_GAP`).
//! - **Holder and held never touch** (`HeldContacts`, in the app's
//!   collision hooks: the body against its holder's character or physics
//!   shadow); the holder's movement and shots pass it (`source_engine`'s
//!   movement, `chimera_sim`'s shots read `Held`).
//! - `PhysicsInfluence`: who last moved a body by hand and when (a punt, a
//!   grab, a throw), for the game's damage credit.
//! - `LaunchedFromHold`: thrown from a hold and not yet touched anything
//!   (the logic's impacts read it: `source_logic`).
//!
//! Known shortcuts: the controller is a model of Source's shadow control
//! (gravity_gun.md Open questions 1): one-tick arrival, capped, with no
//! damping limit (the spec's 2000 in/s).

use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};

use super::prop_physics::PhysicsShadow;

/// A body a player holds.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Held {
    pub holder: Entity,
    /// Where its origin and rotation should be (engine space); None: no
    /// target yet (the body only floats).
    pub target: Option<(Vec3, Quat)>,
    /// The target relative to the holder's eye and view rotation (position
    /// in the view's frame, rotation after the view's): what the holder's
    /// network client draws it at, following its own predicted view.
    pub view_offset: Option<(Vec3, Quat)>,
    /// Seconds to reach the target.
    pub arrive: f32,
    /// m/s and rad/s (the angular one before the contact factor).
    pub max_speed: f32,
    pub max_angular: f32,
    /// The load weight (kg): the held object's masses before the hold,
    /// summed. What touches it and is heavier slows its spin and takes
    /// its push.
    pub load: f32,
    /// The contact factor (0 to 1; 0 at the grab): the angular limit is
    /// `max_angular` × contact³ (`angular_limit`).
    pub contact: f32,
}

/// How fast the contact factor moves (per second).
pub const CONTACT_RATE: f32 = 2.0;
/// The contact factor's goal while the body touches something immovable
/// or heavier than the load (else 1).
pub const CONTACT_LOW: f32 = 0.1;
/// How far apart (m) a contact's points may be and still count as
/// touching (the contact graph keeps speculative ones farther out).
pub const CONTACT_GAP: f32 = 0.0254;

impl Held {
    /// A hold by `holder` that reaches its target in `arrive` seconds within
    /// `max_speed` (m/s) and `max_angular` (rad/s), of a load of `load` kg;
    /// no target yet, the contact factor 0.
    pub fn new(holder: Entity, arrive: f32, max_speed: f32, max_angular: f32, load: f32) -> Self {
        Self {
            holder,
            target: None,
            view_offset: None,
            arrive,
            max_speed,
            max_angular,
            load,
            contact: 0.0,
        }
    }

    /// The spin limit now (rad/s): `max_angular` × contact³.
    pub fn angular_limit(&self) -> f32 {
        self.max_angular * self.contact.powi(3)
    }

    /// The contact factor after `dt` seconds touching something immovable
    /// or heavier than the load (`blocked`) or not.
    pub fn ramp(&mut self, blocked: bool, dt: f32) {
        let goal = if blocked { CONTACT_LOW } else { 1.0 };
        let step = CONTACT_RATE * dt;
        self.contact += (goal - self.contact).clamp(-step, step);
    }
}

/// What a held body was before it was held, put back by `release`.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HeldSaved {
    pub mass: f32,
    pub inertia: Option<AngularInertia>,
    pub linear_damping: f32,
    pub angular_damping: f32,
    pub gravity: f32,
}

/// Who last moved a body by hand (a gravity gun's punt or grab, a
/// throw), and when (game seconds).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PhysicsInfluence {
    pub by: Entity,
    pub at: f64,
}

/// On a network client's copy of the prop its own player holds: where the
/// last snapshots have it (it is drawn at the view instead): what the
/// client's prediction of the hold's rules reads.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ServerPose(pub Vec3, pub Quat);

/// A body launched from a hold, until its next new contact: that contact
/// counts more (specs/hl2dm/gravity_gun.md 9, the game's own rule, read by
/// the logic's impact damage).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct LaunchedFromHold;

/// `holder` holds `body` from now: it weighs `mass` kg (its inertia
/// scaled with it), spins down with `angular_damping`, has no linear
/// damping or gravity, and follows `held`'s target.
pub fn hold(world: &mut World, body: Entity, held: Held, mass: f32, angular_damping: f32) {
    let Ok(mut e) = world.get_entity_mut(body) else { return };
    let old_mass = e.get::<Mass>().map_or(1.0, |m| m.0);
    let saved = HeldSaved {
        mass: old_mass,
        inertia: e.get::<AngularInertia>().copied(),
        linear_damping: e.get::<LinearDamping>().map_or(0.0, |d| d.0),
        angular_damping: e.get::<AngularDamping>().map_or(0.0, |d| d.0),
        gravity: e.get::<GravityScale>().map_or(1.0, |g| g.0),
    };
    let k = mass / old_mass.max(1e-3);
    if let Some(i) = saved.inertia {
        e.insert(AngularInertia {
            principal: i.principal * k,
            local_frame: i.local_frame,
        });
    }
    e.insert((
        held,
        saved,
        Mass(mass),
        LinearDamping(0.0),
        AngularDamping(angular_damping),
        GravityScale(0.0),
    ));
    e.remove::<Sleeping>();
}

/// `body` is let go: its mass, inertia, damping and gravity back.
pub fn release(world: &mut World, body: Entity) {
    let Ok(mut e) = world.get_entity_mut(body) else { return };
    let saved = e.take::<HeldSaved>();
    e.remove::<Held>();
    if let Some(s) = saved {
        e.insert((
            Mass(s.mass),
            LinearDamping(s.linear_damping),
            AngularDamping(s.angular_damping),
            GravityScale(s.gravity),
        ));
        if let Some(i) = s.inertia {
            e.insert(i);
        }
    }
    e.remove::<Sleeping>();
}

/// Each physics step: held bodies' contact factors, then their velocities
/// toward their targets, slid along what blocks them.
pub fn drive(
    time: Res<Time>,
    collisions: Collisions,
    others: Query<(&RigidBody, Option<&ComputedMass>)>,
    mut bodies: Query<(
        Entity,
        &mut Held,
        &Position,
        &Rotation,
        &mut LinearVelocity,
        &mut AngularVelocity,
        Option<&RigidBodyColliders>,
    )>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let mut normals: Vec<Vec3> = Vec::new();
    for (e, mut held, pos, rot, mut v, mut w, colliders) in &mut bodies {
        // What it touches that it can't move: the normals into them.
        normals.clear();
        let load = held.load;
        let blocks = |body: Option<Entity>| match body.and_then(|b| others.get(b).ok()) {
            None => true,
            Some((rb, mass)) => !rb.is_dynamic() || mass.is_some_and(|m| m.value() > load),
        };
        let own: Vec<Entity> = match colliders {
            Some(c) => c.iter().collect(),
            None => vec![e],
        };
        for c in own.iter().copied() {
            for pair in collisions.collisions_with(c) {
                if !pair.generates_constraints() {
                    continue;
                }
                let first = pair.collider1 == c;
                let other = if first { pair.body2 } else { pair.body1 };
                if other == Some(e) || own.contains(if first { &pair.collider2 } else { &pair.collider1 }) || !blocks(other) {
                    continue;
                }
                for m in &pair.manifolds {
                    if m.points.iter().any(|p| p.penetration >= -CONTACT_GAP) {
                        normals.push(if first { m.normal } else { -m.normal });
                    }
                }
            }
        }
        held.ramp(!normals.is_empty(), dt);
        let Some((to, turn)) = held.target else { continue };
        let (lin, ang) = toward(&held, pos.0, rot.0, to, turn);
        v.0 = slide(lin, &normals);
        w.0 = ang;
    }
}

/// `v` with its part into each of `normals` (unit, pointing from the body
/// into what it touches) taken off.
pub fn slide(mut v: Vec3, normals: &[Vec3]) -> Vec3 {
    for n in normals {
        let into = v.dot(*n);
        if into > 0.0 {
            v -= *n * into;
        }
    }
    v
}

/// The velocities that take a body at `at`, `rot` to `to`, `turn` in
/// `held.arrive` seconds, within its limits (the spin's after the contact
/// factor).
pub fn toward(held: &Held, at: Vec3, rot: Quat, to: Vec3, turn: Quat) -> (Vec3, Vec3) {
    let t = held.arrive.max(1e-4);
    let lin = ((to - at) / t).clamp_length_max(held.max_speed);
    let mut d = turn * rot.inverse();
    if d.w < 0.0 {
        d = -d;
    }
    let (axis, angle) = d.to_axis_angle();
    let ang = if angle.abs() < 1e-6 || !axis.is_finite() {
        Vec3::ZERO
    } else {
        (axis * angle / t).clamp_length_max(held.angular_limit())
    };
    (lin, ang)
}

/// For the app's collision hooks: a held body never touches its holder
/// (its character or its physics shadow).
#[derive(SystemParam)]
pub struct HeldContacts<'w, 's> {
    held: Query<'w, 's, &'static Held>,
    shadows: Query<'w, 's, &'static PhysicsShadow>,
    of: Query<'w, 's, &'static ColliderOf>,
}

impl HeldContacts<'_, '_> {
    /// Some(false) when colliders `a` and `b` are a held body and its
    /// holder; None otherwise.
    pub fn filter(&self, a: Entity, b: Entity) -> Option<bool> {
        if self.held.is_empty() {
            return None;
        }
        let body = |e: Entity| self.of.get(e).map_or(e, |o| o.body);
        let player = |e: Entity| self.shadows.get(e).map_or(e, |s| s.owner);
        let (a, b) = (body(a), body(b));
        for (me, other) in [(a, b), (b, a)] {
            if let Ok(h) = self.held.get(me)
                && player(other) == h.holder
            {
                return Some(false);
            }
        }
        None
    }
}

/// The bodies `holder` holds and their colliders (for its traces to
/// leave out).
pub fn held_by<'a>(
    holder: Entity,
    held: impl Iterator<Item = (Entity, &'a Held, Option<&'a RigidBodyColliders>)> + 'a,
) -> impl Iterator<Item = Entity> + 'a {
    held.filter(move |(_, h, _)| h.holder == holder)
        .flat_map(|(e, _, c)| std::iter::once(e).chain(c.into_iter().flat_map(|c| c.into_iter())))
}

/// The held bodies' controller, before each physics step.
pub struct HeldPlugin;

impl Plugin for HeldPlugin {
    fn build(&self, app: &mut App) {
        // Inside the physics step (its own schedule): before the solver,
        // after the game's targets were set in the tick.
        app.add_systems(
            PhysicsSchedule,
            drive.after(PhysicsStepSystems::First).before(PhysicsStepSystems::BroadPhase),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: f32 = 0.0254;

    fn held() -> Held {
        Held {
            contact: 1.0,
            ..Held::new(Entity::PLACEHOLDER, 0.015, 1000.0 * U, 3600f32.to_radians(), 30.0)
        }
    }

    /// gravity_gun.md 5: the target reached in one tick, at most 1000
    /// in/s (test 21's "linear correction speed limited").
    #[test]
    fn the_controller_reaches_in_one_tick_within_its_limit() {
        let h = held();
        let (v, w) = toward(&h, Vec3::ZERO, Quat::IDENTITY, Vec3::X * 0.1, Quat::IDENTITY);
        assert!((v - Vec3::X * 0.1 / 0.015).length() < 1e-4);
        assert_eq!(w, Vec3::ZERO);
        let (v, _) = toward(&h, Vec3::ZERO, Quat::IDENTITY, Vec3::X * 2.0, Quat::IDENTITY);
        assert!((v.length() - 1000.0 * 0.0254).abs() < 1e-4);
        let (_, w) = toward(&h, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Quat::from_rotation_y(0.01));
        assert!((w - Vec3::Y * 0.01 / 0.015).length() < 1e-3, "{w}");
    }

    /// gravity_gun.md 5: the contact factor starts at 0 and climbs 2 a
    /// second (0.03 a tick) to 1, so the spin limit is 3600 × (0.03 n)³
    /// deg/s after n free ticks: 0.0972 after 1, 97.2 after 10, 3600 from
    /// tick 34 on. Touching something it can't move, it falls to 0.1 (3.6
    /// deg/s) in 30 ticks.
    #[test]
    fn the_contact_factor_ramps_the_spin_limit() {
        let mut h = Held::new(Entity::PLACEHOLDER, 0.015, 1000.0 * U, 3600f32.to_radians(), 30.0);
        assert_eq!(h.angular_limit(), 0.0, "no spin at the grab");
        let deg = |h: &Held| h.angular_limit().to_degrees();
        h.ramp(false, 0.015);
        assert!((deg(&h) - 0.0972).abs() < 1e-4, "{}", deg(&h));
        for _ in 1..10 {
            h.ramp(false, 0.015);
        }
        assert!((deg(&h) - 97.2).abs() < 1e-2, "{}", deg(&h));
        let mut n = 10;
        while h.contact < 1.0 {
            h.ramp(false, 0.015);
            n += 1;
        }
        assert_eq!(n, 34, "full spin from tick 34");
        assert!((deg(&h) - 3600.0).abs() < 1e-2);
        let mut n = 0;
        while h.contact > CONTACT_LOW + 1e-6 {
            h.ramp(true, 0.015);
            n += 1;
        }
        assert_eq!(n, 30);
        assert!((deg(&h) - 3.6).abs() < 1e-3, "{}", deg(&h));
        h.ramp(true, 0.015);
        assert!((h.contact - CONTACT_LOW).abs() < 1e-6, "it stays at 0.1");
        // The spin it is driven at is cut there.
        let (_, w) = toward(&h, Vec3::ZERO, Quat::IDENTITY, Vec3::ZERO, Quat::from_rotation_z(1.0));
        assert!((w.length().to_degrees() - 3.6).abs() < 1e-3, "{w}");
    }

    /// gravity_gun.md 5: the velocity's part into a blocking contact is
    /// taken off, the part along it and away from it kept.
    #[test]
    fn the_velocity_into_a_wall_is_taken_off() {
        let wall = Vec3::X;
        assert_eq!(slide(Vec3::new(600.0, 800.0, 0.0), &[wall]), Vec3::new(0.0, 800.0, 0.0));
        assert_eq!(slide(Vec3::new(-600.0, 800.0, 0.0), &[wall]), Vec3::new(-600.0, 800.0, 0.0), "away: kept");
        // A corner (a wall and a floor below it).
        assert_eq!(slide(Vec3::new(500.0, 300.0, -400.0), &[wall, Vec3::NEG_Z]), Vec3::new(0.0, 300.0, 0.0));
    }
}
