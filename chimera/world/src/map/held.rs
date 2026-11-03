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
//! - **The controller** (`drive`, each physics step before the solver):
//!   the body's velocity becomes what reaches the target in `arrive`
//!   seconds, cut to `max_speed`; its spin likewise toward the target
//!   rotation, cut to `max_angular`.
//! - **Holder and held never touch** (`HeldContacts`, in the app's
//!   collision hooks: the body against its holder's character or physics
//!   shadow); the holder's movement and shots pass it (`source_engine`'s
//!   movement, `chimera_sim`'s shots read `Held`).
//! - `PhysicsInfluence`: who last moved a body by hand and when (a punt, a
//!   grab, a throw), for the game's damage credit.
//!
//! Known shortcuts: the controller is a model of Source's shadow control
//! (gravity_gun.md Open questions 1): one-tick arrival, capped, with no
//! damping limit and no contact factor (the angular limit is constant); a
//! held body's velocity into what it touches isn't removed (it presses
//! against walls and is pushed back by the solver instead of sliding).

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
    /// m/s and rad/s.
    pub max_speed: f32,
    pub max_angular: f32,
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

/// Each physics step: held bodies' velocities toward their targets.
pub fn drive(
    time: Res<Time>,
    mut bodies: Query<(&Held, &Position, &Rotation, &mut LinearVelocity, &mut AngularVelocity)>,
) {
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    for (held, pos, rot, mut v, mut w) in &mut bodies {
        let Some((to, turn)) = held.target else { continue };
        let (lin, ang) = toward(held, pos.0, rot.0, to, turn);
        v.0 = lin;
        w.0 = ang;
    }
}

/// The velocities that take a body at `at`, `rot` to `to`, `turn` in
/// `held.arrive` seconds, within its limits.
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
        (axis * angle / t).clamp_length_max(held.max_angular)
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
        app.add_systems(FixedPostUpdate, drive.before(PhysicsSystems::StepSimulation));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn held() -> Held {
        Held {
            holder: Entity::PLACEHOLDER,
            target: None,
            view_offset: None,
            arrive: 0.015,
            max_speed: 1000.0 * 0.0254,
            max_angular: 3600f32.to_radians(),
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
}
