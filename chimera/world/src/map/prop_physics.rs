//! Physics props' motion for the logic layer (specs/source/prop_damage.md
//! 4 step 13, 8; specs/cs_source/physics_props.md 3.1, 3.5): a frozen prop
//! ("motion disabled") stays put like a static prop until its motion is
//! enabled, then becomes a simulated body; bodies can be put to sleep and
//! woken; a prop that started asleep reports its first wake
//! (`PropAwakened`, Source OnAwakened). A round restart puts each back as
//! it spawned.
//!
//! Physics props' motion for the logic: frozen props (`FrozenBody`, static with
//! their brushes) set moving by `enable_motion`, `disable_motion`,
//! `wake`/`sleep`, props that start asleep (`StartAsleep`: put to sleep once in
//! the simulation, `PropAwakened` on their first wake), `reset` at a round
//! restart; players' physics shadows (`PhysicsShadow` on the shadow body,
//! `Shadowed` on its character; `ShadowContacts` keeps a shadow's pairs to
//! `prop_physics` bodies it can push (up to 350 kg) and keeps a shadowed
//! character's own pairs only to `prop_physics` bodies too heavy to push; the
//! shadow is on `core::SHADOW_LAYER`, which no spatial query sees);
//! `settle_props`: a moving prop that keeps within 1 in and 5 degrees of
//! one pose for 0.5 s is stopped and put to sleep (`Settling`), as Source's
//! physics sleeps nearly still bodies; one sunk more than 1 in into the
//! world or a static prop (`WEDGED_DEPTH`) sleeps at once when the map
//! placed it so (`WedgeCheck`) and after 0.5 s wedged otherwise (bodies with
//! `SettlesLikeProp`, broken props' pieces, too); `BodyCounts`:
//! dynamic bodies and how many are awake (the perf readout, `--bench`).
//!
//! Props' surfaces (`dynamic_body`): friction multiplied with the other
//! surface's (`WORLD_FRICTION` for the world), the measured rebound
//! (`rebound`), air drag (`AirDrag`, `air_drag`) and the collision model's
//! stored inertia (`StoredInertia`).
//!
//! Known shortcuts (docs/tech-debt.md): The settle thresholds (1 in, 5
//! degrees) are ours; the 0.5 s matches vphysics, which sleeps a body
//! about 0.4 to 0.55 s after its speed falls under about 1 in/s
//! (physics_props.md Q5, measured). `rebound` is a fit to crate drops on
//! concrete, the world's friction and elasticity one value for every
//! surface, and drag uses the model's bounds for its cross-sections (no
//! angular drag).
//! So is putting wedged props to sleep (cs_italy's jugs inside a cabinet's
//! base were pushed out every tick and crawled or fell through the floor;
//! what Source does with props placed in solid is unknown).
//! They stand in for a fix in the solver, which on uneven ground (a convex
//! hull bridging two displacement triangles) puts back the energy its
//! contacts take out, so such a prop rocks at speeds above avian's sleep
//! threshold until `settle_props` stops it (up to about 3 s after a bullet
//! push for de_dust2's oil drum 390). With avian's warm starting off it
//! settles, but other props then misbehave (one flew off at 50 m/s). Why
//! it's OK for now: the rocking is about a degree, and every de_dust2 prop
//! sleeps within a few seconds of a push (games/cs_source/tests/it/heavy/map_prop_settle.rs).

use avian3d::{
    dynamics::solver::islands::{BodyIslandNode, SleepBody},
    prelude::*,
};
use bevy::{ecs::system::SystemParam, prelude::*};

use super::{MapBrushCollider, MapPhysics, MapPropCollider, PhysicsProp};
use crate::core::MovingSolid;

/// The simulated body of a physics prop with these settings: its friction
/// multiplies the other surface's (the world's is `WORLD_FRICTION`), it
/// rebounds as `rebound` says, air drags it (`AirDrag`), and its
/// rotational inertia is the collision model's own when it has one.
pub fn dynamic_body(p: &MapPhysics, bounds: (Vec3, Vec3)) -> impl Bundle {
    (
        RigidBody::Dynamic,
        Mass(p.mass),
        Friction::new(p.friction).with_combine_rule(CoefficientCombine::Multiply),
        Restitution::new(rebound(p.elasticity)).with_combine_rule(CoefficientCombine::Max),
        LinearDamping(p.damping),
        AngularDamping(p.rotdamping),
        // Source clamps every body to 2000 units/s and 3600 deg/s.
        MaxLinearSpeed(2000.0 * 0.0254),
        MaxAngularSpeed(3600f32.to_radians()),
        PhysicsProp {
            push: p.push,
            mass: p.mass,
            bounds,
        },
        MapPropCollider,
        AirDrag::of_box(bounds, p.mass),
        StoredInertia(p.inertia),
        // Impact sounds and damage listen for its contacts.
        CollisionEventsEnabled,
    )
}

/// The friction of the world's surfaces for physics bodies: Source's
/// default surface property (and concrete, tile, dirt and most of what
/// maps are built of), which a body's own multiplies (physics_props.md
/// 3.4, measured 2026-10-18: a crate sliding on de_dust2's concrete
/// slowed at 0.086 g with ice's 0.1, 0.24 g with 0.3, 0.61 g with 0.8).
pub const WORLD_FRICTION: f32 = 0.8;

/// The elasticity of the world's surfaces in `rebound` (concrete's).
pub const WORLD_ELASTICITY: f32 = 0.2;

/// The share of its landing speed a physics prop keeps bouncing off the
/// world with surface elasticity `elasticity` (physics_props.md 3.4,
/// measured 2026-10-18: a 40-unit crate dropped flat on de_dust2's
/// concrete rose back at 0.13 to 0.15 of its landing speed for every
/// elasticity up to 0.3, 0.28 with 0.99, 0.34 with 2.0, 0.85 with 1000;
/// the same from 25, 100 and 400 units). A fit, not vphysics' law.
pub fn rebound(elasticity: f32) -> f32 {
    (0.6 * (elasticity.max(0.0) * WORLD_ELASTICITY).sqrt()).clamp(0.14, 0.9)
}

/// Source's `air_density` (the probe server's reads 2.00), kg/m³.
pub const AIR_DENSITY: f32 = 2.0;

/// Air drag on a physics body: along each of its own axes it slows by
/// `k · |v| · v` (m/s²), with `k` = half the air density times the
/// body's cross-section across that axis over its mass (physics_props.md
/// 3.3, measured 2026-10-18: a 30 kg 40-unit crate falling at 1000 in/s
/// is held back about 950 in/s², 360 at 600; vphysics' debug print gives
/// the same per-axis `k`).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct AirDrag(pub Vec3);

impl AirDrag {
    /// For a box `bounds` (meters, body axes) of `mass` kg.
    pub fn of_box(bounds: (Vec3, Vec3), mass: f32) -> Self {
        let s = (bounds.1 - bounds.0).abs();
        let area = Vec3::new(s.y * s.z, s.x * s.z, s.x * s.y);
        Self(0.5 * AIR_DENSITY * area / mass.max(0.1))
    }
}

/// Before each physics step: awake bodies with `AirDrag` slow down.
pub fn air_drag(
    time: Res<Time>,
    mut bodies: Query<(&RigidBody, &Rotation, &AirDrag, &mut LinearVelocity), Without<Sleeping>>,
) {
    let dt = time.delta_secs();
    for (rb, rot, drag, mut v) in &mut bodies {
        let speed = v.0.length();
        if !rb.is_dynamic() || speed < 1e-4 {
            continue;
        }
        let local = rot.0.inverse() * v.0;
        // Never past a stop along an axis.
        let slow = (drag.0 * speed * dt).min(Vec3::ONE);
        v.0 -= rot.0 * (local * slow);
    }
}

/// A physics body's rotational inertia from its collision model (kg m²,
/// body axes), set once its collider is in place: vphysics uses the
/// model's stored inertia times the mass, 70 to 80% of a solid of the
/// same shape (physics_props.md Q1, measured 2026-10-18), so props spin
/// faster from the same off-centre hit than a solid would.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct StoredInertia(pub Option<Vec3>);

/// Bodies whose `StoredInertia` is known take it in place of the one
/// avian computes from their colliders.
pub fn apply_stored_inertia(
    bodies: Query<(Entity, &StoredInertia), Added<StoredInertia>>,
    mut commands: Commands,
) {
    for (e, inertia) in &bodies {
        if let Some(i) = inertia.0 {
            commands
                .entity(e)
                .insert((AngularInertia::new(i.max(Vec3::splat(1e-5))), NoAutoAngularInertia));
        }
    }
}

/// A physics prop placed frozen: the body it becomes when its motion is
/// enabled (`enabled` once it has), and what it was while frozen.
#[derive(Component, Clone, Debug)]
pub struct FrozenBody {
    pub physics: MapPhysics,
    pub bounds: (Vec3, Vec3),
}

/// A frozen prop now moving: its still brushes, kept for a round restart.
#[derive(Component, Clone, Debug)]
pub struct Unfrozen {
    solid: Option<MovingSolid>,
    brush_collider: bool,
}

/// A prop that spawned asleep: held still (a static body) until something
/// wakes it (the Wake input, a push from damage, a moving body running
/// into it), which reports `PropAwakened`. (Avian's own sleep can't be
/// set before a body joins the simulation, and props placed touching
/// each other would wake each other on the first step.)
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct StartAsleep;

/// A prop that started asleep woke up (its node).
#[derive(Message, Clone, Copy, Debug)]
pub struct PropAwakened(pub Entity);

/// Set a frozen prop moving; false if it isn't one (or already moves).
pub fn enable_motion(world: &mut World, node: Entity) -> bool {
    if super::placed_ragdoll::set_motion(world, node, true) {
        return true;
    }
    let Ok(mut e) = world.get_entity_mut(node) else {
        return false;
    };
    if e.contains::<Unfrozen>() {
        // Disabled again by DisableMotion: just let it move.
        if e.get::<RigidBody>().is_some_and(|b| b.is_static()) {
            e.insert(RigidBody::Dynamic);
            return true;
        }
        return false;
    }
    let Some(frozen) = e.get::<FrozenBody>().cloned() else {
        if e.get::<RigidBody>().is_some_and(|b| b.is_static()) && e.contains::<PhysicsProp>() {
            e.insert(RigidBody::Dynamic);
            return true;
        }
        return false;
    };
    let solid = e.take::<MovingSolid>();
    let brush_collider = e.take::<MapBrushCollider>().is_some();
    // Its visibility clusters follow it as it moves; its occluder bounds
    // were where it stood.
    let (min, max) = frozen.bounds;
    e.remove::<super::vis::Occludee>();
    e.insert(super::BrushEntityBounds { min, max });
    if !e.contains::<super::VisClusters>() {
        e.insert(super::VisClusters::new(Vec::new()));
    }
    e.insert((
        Unfrozen { solid, brush_collider },
        dynamic_body(&frozen.physics, frozen.bounds),
        Visibility::Inherited,
    ));
    true
}

/// Freeze a moving prop where it is.
pub fn disable_motion(world: &mut World, node: Entity) {
    if super::placed_ragdoll::set_motion(world, node, false) {
        return;
    }
    let now = world.get_resource::<crate::core::SimClock>().map_or(0.0, |c| c.now);
    if let Ok(mut e) = world.get_entity_mut(node)
        && e.get::<RigidBody>().is_some_and(|b| b.is_dynamic())
    {
        // Teleported into a slot and pinned there (dm_lockdown's plug): the
        // step between them pushed it out of what it overlaps (our solver,
        // 2 units and a turn in one step); it stays where it was sent.
        if let Some(sent) = e.take::<JustTeleported>().filter(|s| now - s.at <= TELEPORT_PIN_WINDOW) {
            if let Some(mut t) = e.get_mut::<Transform>() {
                t.translation = sent.position;
                t.rotation = sent.rotation;
            }
            if let Some(mut p) = e.get_mut::<Position>() {
                p.0 = sent.position;
            }
            if let Some(mut r) = e.get_mut::<Rotation>() {
                *r = Rotation::from(sent.rotation);
            }
        }
        e.insert(RigidBody::Static);
    }
}

/// How long after a teleport a body pinned stays exactly where it was sent
/// (s): the map's "teleport, DisableMotion 0.01 s later".
const TELEPORT_PIN_WINDOW: f64 = 0.05;

/// Where a teleport sent a physics body, and when (`disable_motion`).
#[derive(Component, Clone, Copy, Debug)]
pub struct JustTeleported {
    pub position: Vec3,
    pub rotation: Quat,
    pub at: f64,
}

/// Wake a sleeping body (one that started asleep reports it).
pub fn wake(world: &mut World, node: Entity) {
    if super::placed_ragdoll::wake(world, node) {
        return;
    }
    let Ok(mut e) = world.get_entity_mut(node) else { return };
    if e.take::<StartAsleep>().is_some() {
        e.insert(RigidBody::Dynamic);
        world.write_message(PropAwakened(node));
    } else if e.contains::<Sleeping>() {
        e.remove::<Sleeping>();
    }
}

/// Whether a prop node is still asleep since it spawned.
pub fn asleep(world: &World, node: Entity) -> bool {
    world.get::<StartAsleep>(node).is_some()
}

/// Put a body to sleep.
pub fn sleep(world: &mut World, node: Entity) {
    if let Ok(mut e) = world.get_entity_mut(node)
        && e.get::<RigidBody>().is_some_and(|b| b.is_dynamic())
        && !e.contains::<Sleeping>()
    {
        e.insert(Sleeping);
    }
}

/// A round restart: a prop that was frozen is frozen again (its still
/// brushes back), one that started asleep sleeps again and will report
/// its next wake.
pub fn reset(world: &mut World, node: Entity, asleep: bool) {
    let Ok(mut e) = world.get_entity_mut(node) else { return };
    if let Some(u) = e.take::<Unfrozen>() {
        e.remove::<PhysicsProp>().insert(RigidBody::Static);
        if let Some(s) = u.solid {
            e.insert(s);
        }
        if u.brush_collider {
            e.insert(MapBrushCollider);
        }
    } else if e.contains::<FrozenBody>() && e.get::<RigidBody>().is_some_and(|b| b.is_dynamic()) {
        e.insert(RigidBody::Static);
    }
    if asleep && e.contains::<PhysicsProp>() {
        e.insert((StartAsleep, RigidBody::Static));
    } else if e.get::<RigidBody>().is_some_and(|b| b.is_dynamic()) && e.contains::<PhysicsProp>() {
        e.insert(WedgeCheck::default());
    }
}

/// A moving body running into a prop that sleeps since it spawned wakes
/// it.
pub fn wake_on_contact(
    mut started: MessageReader<CollisionStart>,
    asleep: Query<(), With<StartAsleep>>,
    bodies: Query<(&RigidBody, &LinearVelocity)>,
    mut commands: Commands,
    mut awakened: MessageWriter<PropAwakened>,
) {
    for s in started.read() {
        let (b1, b2) = (s.body1.unwrap_or(s.collider1), s.body2.unwrap_or(s.collider2));
        for (me, other) in [(b1, b2), (b2, b1)] {
            let moving = bodies
                .get(other)
                .is_ok_and(|(rb, v)| rb.is_dynamic() && v.0.length_squared() > WAKE_SPEED * WAKE_SPEED);
            if moving && asleep.contains(me) {
                commands.entity(me).try_remove::<StartAsleep>().try_insert(RigidBody::Dynamic);
                awakened.write(PropAwakened(me));
            }
        }
    }
}

/// A body slower than this (m/s) doesn't wake what it touches.
const WAKE_SPEED: f32 = 0.05;

/// A player's physics shadow (specs/cs_source/physics_props.md 4.1 step 3):
/// an invisible 85 kg body the size of the player's box that the game
/// drives to the player each tick. It is what touches physics props for
/// the player: it pushes the light ones and holds up the ones resting on
/// or against the player. It is on its own layer (`core::SHADOW_LAYER`),
/// which no spatial query sees. Inactive (a dead player's), it touches
/// nothing.
#[derive(Component, Clone, Copy, Debug)]
pub struct PhysicsShadow {
    pub owner: Entity,
    pub active: bool,
}

/// A character whose contacts with physics props go through its
/// `PhysicsShadow` (this entity; None: none, its own collider touches
/// them): its own collider never touches them. On every character of a
/// movement with shadows from the spawn, a shadow or none.
#[derive(Component, Clone, Copy, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct Shadowed(pub Option<Entity>);

/// The shadow's mass, kg (spec 4.1 step 3).
pub const SHADOW_MASS: f32 = 85.0;
/// The heaviest prop a shadow pushes, kg (spec: the player push mass
/// limit). Heavier props never touch the shadow: the player's movement
/// treats them as walls, and the character's own collider (kinematic)
/// stops them when they move into the player.
pub const SHADOW_PUSH_MASS: f32 = 350.0;

/// Which pairs players' shadows and shadowed characters keep.
#[derive(SystemParam)]
pub struct ShadowContacts<'w, 's> {
    shadows: Query<'w, 's, &'static PhysicsShadow>,
    shadowed: Query<'w, 's, &'static Shadowed>,
    of: Query<'w, 's, &'static ColliderOf>,
    props: Query<'w, 's, &'static PhysicsProp>,
}

impl ShadowContacts<'_, '_> {
    /// Some(keep) when collider `a` or `b` is a shadow or a shadowed
    /// character: an active shadow touches only the `prop_physics` bodies
    /// it can push (multiplayer props are in a group players never touch,
    /// spec 4.2.1); a shadowed character touches only the heavier ones.
    /// None: neither is one.
    pub fn filter(&self, a: Entity, b: Entity) -> Option<bool> {
        let body = |e: Entity| self.of.get(e).map_or(e, |o| o.body);
        let (a, b) = (body(a), body(b));
        for (me, other) in [(a, b), (b, a)] {
            if let Ok(s) = self.shadows.get(me) {
                return Some(s.active && self.props.get(other).is_ok_and(pushable));
            }
        }
        for (me, other) in [(a, b), (b, a)] {
            if self.shadowed.get(me).is_ok_and(|s| s.0.is_some())
                && let Ok(p) = self.props.get(other)
            {
                return Some(p.push == super::PushAway::Collide && !pushable(p));
            }
        }
        None
    }
}

/// Whether players' shadows push this prop: a `prop_physics` (not a
/// multiplayer one) up to `SHADOW_PUSH_MASS`.
pub fn pushable(p: &PhysicsProp) -> bool {
    p.push == super::PushAway::Collide && p.mass <= SHADOW_PUSH_MASS
}

/// How many simulated bodies there are and how many of them are awake
/// (the perf readout, `--bench`'s `bodies_awake`, tests): dynamic,
/// enabled bodies; a sleeping one costs the solver nothing.
#[derive(SystemParam)]
pub struct BodyCounts<'w, 's> {
    bodies: Query<'w, 's, (Entity, &'static RigidBody, Has<Sleeping>), Without<RigidBodyDisabled>>,
}

impl BodyCounts<'_, '_> {
    /// (dynamic bodies, awake ones)
    pub fn counts(&self) -> (usize, usize) {
        let mut n = (0, 0);
        for (_, rb, asleep) in &self.bodies {
            if rb.is_dynamic() {
                n.0 += 1;
                n.1 += usize::from(!asleep);
            }
        }
        n
    }

    /// The dynamic bodies awake.
    pub fn awake(&self) -> impl Iterator<Item = Entity> + '_ {
        self.bodies
            .iter()
            .filter(|(_, rb, asleep)| rb.is_dynamic() && !asleep)
            .map(|(e, ..)| e)
    }
}

/// A moving physics prop that stays within `SETTLE_DISTANCE` and
/// `SETTLE_ANGLE` of one pose for `SETTLE_TIME` is stopped and put to
/// sleep (with its island), as Source's physics puts nearly still bodies
/// to sleep (spec 3.5; its thresholds are inside vphysics, an open
/// question, so these are ours). Avian's own sleep needs the speed below
/// its threshold for half a second, which a prop rocking on uneven ground
/// never gets: a de_dust2 oil drum knocked onto the crease between two
/// displacement triangles (a flat one and one 3.7 degrees steeper) rocked
/// by about 1.5 degrees at up to 0.5 rad/s forever, the solver putting
/// back each tick the energy its contacts take out
/// (games/cs_source/tests/it/heavy/map_prop_settle.rs).
pub const SETTLE_DISTANCE: f32 = 0.0254;
/// See `SETTLE_DISTANCE`, radians.
pub const SETTLE_ANGLE: f32 = 0.087;
/// See `SETTLE_DISTANCE`, seconds.
pub const SETTLE_TIME: f64 = 0.5;

/// A prop sunk deeper than this (meters) into something that doesn't move
/// (the world, a static prop) is wedged: placed so, it is put to sleep
/// where it is (`WedgeCheck`); knocked awake, it sleeps once it has stayed
/// wedged for `SETTLE_TIME`, wherever it rocks.
pub const WEDGED_DEPTH: f32 = 0.0254;
/// How long after its first step a placed prop is checked for being
/// wedged, seconds (a few ticks: the step right after spawning wakes
/// what was put to sleep in it).
pub const WEDGE_CHECK_TIME: f64 = 0.1;

/// A physics prop placed by the map, checked for being wedged in its first
/// steps (`settle_props`; the time of the first, once seen): one placed
/// sunk into the world or a static prop (cs_italy's glass jugs inside a
/// cabinet's base, de_inferno's milk carton between two pieces of a junk
/// pile) is pushed out at up to avian's overlap speed every tick and
/// jitters, crawls or falls through the floor (a jug did) without ever
/// resting. It is put to sleep where it is instead; a shot, a push or a
/// moving body wakes it as any other. A round restart checks again
/// (`reset`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct WedgeCheck(Option<f64>);

/// A body that isn't a physics prop but settles as one (`settle_props`'
/// stillness and wedge rules): a broken prop's piece.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SettlesLikeProp;

/// Where an awake physics prop has stayed since when, and since when it
/// is wedged (`settle_props`).
#[derive(Component, Clone, Copy, Debug)]
pub struct Settling {
    at: Vec3,
    rotation: Quat,
    since: f64,
    wedged: Option<f64>,
}

/// After each physics step: props (and `SettlesLikeProp` bodies) that kept still (`SETTLE_DISTANCE`) or
/// stayed wedged (`WEDGED_DEPTH`) sleep. An island sleeps as a whole, so it waits for every prop in it
/// to have kept still and every other body in it to be resting by avian's
/// own measure. Their velocities are zeroed without marking them changed
/// (a change wakes a sleeping body); a prop that sleeps (by this or by
/// avian) forgets its pose, so one woken by a push starts its wait again.
/// A prop placed wedged (`WedgeCheck`) sleeps at once, with its island.
#[allow(clippy::type_complexity)]
pub fn settle_props(
    time: Res<Time>,
    time_to_sleep: Res<TimeToSleep>,
    mut props: Query<
        (
            Entity,
            &RigidBody,
            &Position,
            &Rotation,
            &BodyIslandNode,
            &mut LinearVelocity,
            &mut AngularVelocity,
            Option<&mut Settling>,
            Option<&mut WedgeCheck>,
        ),
        (Or<(With<PhysicsProp>, With<SettlesLikeProp>)>, Without<Sleeping>),
    >,
    others: Query<
        (&RigidBody, &BodyIslandNode, &SleepTimer),
        (Without<PhysicsProp>, Without<SettlesLikeProp>, Without<Sleeping>),
    >,
    asleep: Query<
        Entity,
        (
            With<Settling>,
            Or<(With<Sleeping>, (Without<PhysicsProp>, Without<SettlesLikeProp>))>,
        ),
    >,
    (contacts, bodies): (Res<ContactGraph>, Query<&RigidBody>),
    mut commands: Commands,
    mut still: Local<Vec<(Entity, u32)>>,
    mut moving: Local<bevy::platform::collections::HashSet<u32>>,
) {
    for e in &asleep {
        commands.entity(e).try_remove::<Settling>();
    }
    let now = time.elapsed_secs_f64();
    still.clear();
    moving.clear();
    let mut wedged = Vec::new();
    for (e, rb, p, r, island, _, _, settling, check) in &mut props {
        if !rb.is_dynamic() {
            if settling.is_some() {
                commands.entity(e).try_remove::<Settling>();
            }
            continue;
        }
        let stuck = wedged_depth(&contacts, &bodies, e) > WEDGED_DEPTH;
        if let Some(mut check) = check {
            // Only in its first steps: one woken later (by a shot) moves.
            if now - *check.0.get_or_insert(now) < WEDGE_CHECK_TIME {
                if stuck {
                    wedged.push(e);
                }
            } else {
                commands.entity(e).try_remove::<WedgeCheck>();
            }
        }
        let fresh = Settling {
            at: p.0,
            rotation: r.0,
            since: now,
            wedged: stuck.then_some(now),
        };
        let Some(mut s) = settling else {
            commands.entity(e).try_insert(fresh);
            moving.insert(island.island_id().0);
            continue;
        };
        if !stuck {
            s.wedged = None;
        } else if s.wedged.is_none() {
            s.wedged = Some(now);
        }
        if s.at.distance(p.0) > SETTLE_DISTANCE || s.rotation.angle_between(r.0) > SETTLE_ANGLE {
            *s = Settling { wedged: s.wedged, ..fresh };
        }
        if now - s.since >= SETTLE_TIME || s.wedged.is_some_and(|t| now - t >= SETTLE_TIME) {
            still.push((e, island.island_id().0));
        } else {
            moving.insert(island.island_id().0);
        }
    }
    for (rb, island, timer) in &others {
        if rb.is_dynamic() && timer.0 < time_to_sleep.0 {
            moving.insert(island.island_id().0);
        }
    }
    let mut slept = bevy::platform::collections::HashSet::new();
    for e in wedged {
        if let Ok((.., island, mut v, mut w, _, _)) = props.get_mut(e) {
            v.bypass_change_detection().0 = Vec3::ZERO;
            w.bypass_change_detection().0 = Vec3::ZERO;
            if slept.insert(island.island_id().0) {
                commands.entity(e).try_remove::<Settling>();
                commands.queue(SleepBody(e));
            }
        }
    }
    for &(e, island) in still.iter() {
        if moving.contains(&island) || slept.contains(&island) {
            continue;
        }
        if let Ok((.., mut v, mut w, _, _)) = props.get_mut(e) {
            v.bypass_change_detection().0 = Vec3::ZERO;
            w.bypass_change_detection().0 = Vec3::ZERO;
        }
        // Forgotten now: a push before the next tick wakes it before this
        // runs again, and it must not count as still from before.
        commands.entity(e).try_remove::<Settling>();
        if slept.insert(island) {
            commands.queue(SleepBody(e));
        }
    }
}

/// How deep (meters) body `e` is sunk into anything that doesn't move.
fn wedged_depth(contacts: &ContactGraph, bodies: &Query<&RigidBody>, e: Entity) -> f32 {
    contacts
        .contact_pairs_with(e)
        .filter(|pair| {
            let other = if pair.body1 == Some(e) { pair.body2 } else { pair.body1 };
            other.is_none_or(|o| bodies.get(o).is_ok_and(|rb| !rb.is_dynamic()))
        })
        .flat_map(|pair| pair.manifolds.iter().flat_map(|m| m.points.iter().map(|p| p.penetration)))
        .fold(0.0, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A body despawned between `settle_props` reading it and its commands
    /// being applied (a map change or a breakage in the same frame) is no
    /// error: its commands are for a body that is gone.
    #[test]
    fn settling_a_body_that_is_despawned_before_its_commands_apply_is_no_error() {
        let mut world = World::new();
        world.init_resource::<Time>();
        world.init_resource::<TimeToSleep>();
        world.init_resource::<ContactGraph>();
        world.init_resource::<avian3d::dynamics::solver::islands::PhysicsIslands>();
        // The default handler panics on a failed command.
        let body = world
            .spawn((
                RigidBody::Dynamic,
                Position::default(),
                Rotation::default(),
                BodyIslandNode::default(),
                LinearVelocity::default(),
                AngularVelocity::default(),
                SettlesLikeProp,
            ))
            .id();
        let mut system = IntoSystem::into_system(settle_props);
        system.initialize(&mut world);
        system.run_without_applying_deferred((), &mut world).unwrap();
        world.despawn(body);
        system.apply_deferred(&mut world);
    }
}
