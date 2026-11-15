//! Halo's player movement, from specs/halo_ce/movement.md: the movement
//! slot's implementation `halo_ce:movement` (docs/DESIGN.md, "Slots"), the
//! loadout's default. A vertical pill (radius 0.2 WU, 0.7 WU tall standing)
//! moved once a tick: on the ground its velocity steps toward the wanted
//! one (stick times speed, lifted onto the ground plane, scaled by the
//! slope) by a fixed acceleration, with a push into the ground that the
//! collision move takes out again; in the air gravity and a weak air
//! control; a jump raises the vertical speed to 0.07 WU/t; crouching blends
//! speeds, pill and eye over 6 ticks and lifts the feet in the air; the
//! ground test, landings and falling damage come from the move's contacts.
//!
//! - `data`: the numbers (data/movement.toml, `MovementData`), in world
//!   units and ticks as the spec's constants table states them.
//! - `tick`: one tick in the spec's per-tick order (`tick::tick`), and the
//!   state it keeps (`HaloMovement`, the slot's marker, predicted with
//!   `predicted_net`).
//! - `mover`: the collision move (the slide, the clip set, the stored
//!   velocity, the virtual crease contact) on a `mover::Space`.
//! - `space`: that space on the engine's collision (avian shape casts).
//!
//! Here: the system (`Predict::Movement`) that reads a player's `Intent`,
//! `Transform` (the pill's centre, metres) and `Velocity` (m/s), runs the
//! tick in world units and writes them back with `MovementState` (eye
//! height 0.62 WU standing, 0.35 crouched, linear in the crouch fraction);
//! the falling damage as a `damage::HaloHit` (`fall` at the landing's scale,
//! `lethal_fall` in the air: shields, then body, by the vitality's rule); the entity's collider following the pill; the
//! footing and seating that let a party app switch a player to this
//! movement and back (`slots::set_movement`). A spawn starts airborne with
//! the jump timer full and the pill pushed out of anything it overlaps;
//! what is under it catches it on the first tick (a settle).
//!
//! Keyboard and mouse: `Intent::move_axis` is the throttle (forward, left)
//! as the spec maps keys, full per axis, normalised on diagonals; crouch is
//! a level and always counts (the spec's keyboard choice). A
//! `ControllerStick` on the player applies the controller's crouch rule
//! instead. The vitality's `Stun` (s_b: the damage event sets it) and
//! `SpeedMultiplier` (the game type's m_p; nothing sets it yet,
//! multiplayer_rules.md will) are read when present.
//!
//! The tick's order (health_shields.md "Per-tick order", movement.md): the
//! stun's counter step (`vitality::UnitStep`, the unit update) comes first,
//! `step` runs `.after(UnitStep)` and reads the stepped stun for the wanted
//! speed, scaled by (1 - 0.8 s_b), and for the jump, scaled by the turning
//! penalty (1 - 0.8 s_b) as the spec's quirk keeps (the tag's jumping
//! penalty of 1.0 is unused); the fall's hit is written in the movement set
//! and applied by the seam after `SimSet::Weapons` and before the engine's
//! damage, so the same tick's damage update (shield delay and recharge) sees
//! it, and a stun a hit sets in tick n first slows tick n + 1's movement.
//!
//! The tick rate: the spec's numbers are per 30 Hz tick and the movement
//! is exact when the simulation ticks at 30 Hz (Halo's own apps, its
//! tests): one call per engine tick, k = 1. Under another tick (a party
//! app on another game's tick, 64 Hz or 66.7 Hz), it does what the other
//! games' movements do off their own tick, Combat Arms' and Source's: one
//! step per engine tick scaled by the tick's length, k = engine tick / Halo
//! tick (`SimClock`): the displacement is the velocity times k, and every
//! per-tick step (the acceleration limits, gravity, the crouch rate, the
//! jump timer and the landing counter) is k times the spec's; velocities
//! and thresholds stay per Halo tick. Speeds, the stop distance's order and
//! the timings follow; so that the jump's arc keeps its 30 Hz height (the
//! semi-implicit step's offset grows with the step), the airborne
//! displacement also gets -g k (1 - k) / 2, zero at k = 1. Sub-stepping at
//! 30 Hz inside the engine's tick was the other choice: exact but the body
//! would move on only some engine ticks (a 30 Hz stutter on a 64 Hz
//! client) and inputs between Halo ticks would be lost.
//!
//! Known shortcuts:
//! - The collision move uses the engine's capsule cast (collision.md allows
//!   it for a first playable) in place of the spec's sweep of a point
//!   against the level's feature shapes: concave edges get no cylinder;
//!   contact normals at edges are the cast's (GJK), so the steps' hop
//!   details (tick counts, heights, the edge normals of the steps' rows)
//!   may differ from the spec's simulation (the tests list what is
//!   checked how). A sweep that starts touching a surface it slides along
//!   restarts 2e-5 WU off it (`mover::LIFT`). The casts are 1e-3 WU
//!   thinner than the pill and the contact's time comes from its plane
//!   (`space::SKIN`): a touch shallower than that is missed. Against a
//!   box collider tens of metres across (the greybox's) the engine's cast
//!   can miss a shallow touch outright, so a player there may drop 1/128
//!   WU for a tick; a map's triangle mesh (and the tests' levels, built as
//!   triangle meshes) are met exactly. Against an edge too high to climb
//!   the player rocks by a few thousandths of a WU instead of resting
//!   (the spec's blocked rows have speed 0).
//! - The in-air kill damage is dealt once per engine tick (per Halo tick at
//!   30 Hz); a fall's hit is written only on a first-time prediction.
//! - Stun is read as its value s_b alone: the stunned moving animations
//!   (above 0.2) have no body to play on yet.
//! - Not modelled: body facing and the animation states (cosmetic: no
//!   bodies yet), climbable surfaces (ladders: Boarding Action only),
//!   impulses and slipping (no damage effects yet), elevators, teleporters,
//!   volatile contacts and the 16-contact splatter flag, the pitch clamp
//!   (the input layer's), the airborne tick counter (animation only).
//! - The virtual contact's normal for two planes, one facing down, is of
//!   e x N_s and N_s x e the one pointing out of the crease (the spec's
//!   "keeping the steepest plane's orientation" does not say).

pub mod data;
pub mod mover;
pub mod space;
pub mod tick;

use avian3d::{character_controller::move_and_slide::DepenetrationConfig, prelude::*};
use bevy::prelude::*;
use chimera_core::{
    character::{CAPSULE_HEIGHT, CAPSULE_RADIUS},
    slots::{Footing, Loadout, RegisterSlots},
};

pub use data::MovementData;
pub use tick::{Controls, Fall, HaloMovement, Landing};

use super::{
    damage::HaloHit,
    vitality::{DamageEffects, Stun, UnitStep},
};
use crate::core::{
    CHARACTER_FILTER, Dead, FirstTimePredicted, Intent, MovementState, Predict, PredictedAppExt, SimClock, Velocity,
};

/// The movement's id.
pub const ID: &str = "halo_ce:movement";

/// The player's speed multiplier m_p (the game type's,
/// multiplayer_rules.md): scales the wanted speed only.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[component(storage = "SparseSet")]
pub struct SpeedMultiplier(pub f32);

/// The player's throttle comes from a stick: the controller's crouch rule
/// applies (spec section 1). Keyboard players have none.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
#[component(storage = "SparseSet")]
pub struct ControllerStick;

/// `halo_ce:movement`, and the match's movement.
pub struct HaloMovementPlugin;

impl Plugin for HaloMovementPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MovementData::builtin())
            .insert_resource(Loadout { movement: ID })
            .register_movement::<HaloMovement>(ID)
            .seat_movement(ID, footing, seat)
            .add_systems(Predict::Movement, (step.after(UnitStep), sync_collider).chain())
            .predicted_net::<HaloMovement>();
    }
}

/// The engine tick's length in Halo ticks: exactly 1 at Halo's own tick.
pub fn tick_scale(clock: &SimClock) -> f32 {
    let k = clock.delta.as_secs_f64() / crate::game::TICK_INTERVAL;
    if (k - 1.0).abs() < 1e-4 { 1.0 } else { k as f32 }
}

/// Where a Halo player stands (`slots::Footing`): its origin.
fn footing(world: &World, e: Entity) -> Option<Footing> {
    let me = world.get::<HaloMovement>(e)?;
    let t = world.get::<Transform>(e)?;
    let d = world.get_resource::<MovementData>()?;
    let s = crate::game::METERS_PER_UNIT;
    Some(Footing {
        feet: t.translation - Vec3::Y * d.centre_above_origin(me.crouch) * s,
        crouched: me.crouch >= 0.5,
        on_ground: !me.airborne,
    })
}

/// A player switched to Halo's movement put on the old movement's feet,
/// crouched if it crouched; the first tick pushes it out of what it
/// overlaps.
fn seat(world: &mut World, e: Entity, f: Footing) {
    let Some(d) = world.get_resource::<MovementData>().cloned() else { return };
    let s = crate::game::METERS_PER_UNIT;
    let c = if f.crouched { 1.0 } else { 0.0 };
    let at = f.feet + Vec3::Y * d.centre_above_origin(c) * s;
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if let Some(mut me) = ent.get_mut::<HaloMovement>() {
        me.crouch = c;
        me.stance_crouch = f.crouched;
        me.airborne = !f.on_ground;
    }
    if let Some(mut t) = ent.get_mut::<Transform>() {
        t.translation = at;
    }
    if let Some(mut p) = ent.get_mut::<Position>() {
        p.0 = at;
    }
}

#[allow(clippy::type_complexity)]
fn step(
    mut q: Query<(
        Entity,
        &Intent,
        &mut HaloMovement,
        &mut Transform,
        &mut Velocity,
        &mut MovementState,
        (Option<&Stun>, Option<&SpeedMultiplier>, Has<ControllerStick>, Has<Dead>),
    )>,
    move_and_slide: MoveAndSlide,
    data: Res<MovementData>,
    clock: Res<SimClock>,
    first: Option<Res<FirstTimePredicted>>,
    effects: Res<DamageEffects>,
    mut hits: MessageWriter<HaloHit>,
) {
    let d = &*data;
    let s = crate::game::METERS_PER_UNIT;
    let per_tick = s * data::TICKS_PER_SECOND;
    let k = tick_scale(&clock);
    let first = first.is_none_or(|f| f.0);
    for (e, intent, mut me, mut transform, mut vel, mut state, (stun, mult, controller, dead)) in &mut q {
        if dead {
            me.was_dead = true;
            continue;
        }
        let filter = SpatialQueryFilter::from_excluded_entities([e]).with_mask(CHARACTER_FILTER);
        let space = space::EngineSpace::new(&move_and_slide, filter.clone(), d);
        if me.was_dead {
            // A respawn: a new biped.
            *me = HaloMovement::default();
            vel.0 = Vec3::ZERO;
        }
        if !me.initialized {
            me.initialized = true;
            me.jump_timer = d.jump_timer_max;
            me.airborne = true;
            let shape = space.pill(d.segment(me.crouch));
            let out = move_and_slide.depenetrate(
                &shape,
                transform.translation,
                Quat::IDENTITY,
                &DepenetrationConfig::default(),
                &filter,
            );
            transform.translation += out;
        }
        let origin = transform.translation / s - Vec3::Y * d.centre_above_origin(me.crouch);
        let velocity = vel.0 / per_tick;
        let rot = Quat::from_rotation_y(intent.yaw);
        let controls = Controls {
            throttle: Vec2::new(intent.move_axis.y, -intent.move_axis.x),
            forward: rot * Vec3::NEG_Z,
            left: rot * Vec3::NEG_X,
            jump: intent.jump,
            crouch: intent.crouch,
            controller,
            stun: stun.map_or(0.0, |s| s.value),
            speed_multiplier: mult.map_or(1.0, |m| m.0),
        };
        let out = tick::tick(d, &mut me, origin, velocity, controls, k, &space);
        transform.translation = (out.origin + Vec3::Y * d.centre_above_origin(me.crouch)) * s;
        vel.0 = out.velocity * per_tick;
        if first && let Some(fall) = out.damage {
            hits.write(match fall {
                Fall::Landing(q_f) => HaloHit::fall(&effects, e, q_f),
                Fall::Lethal => HaloHit::lethal_fall(&effects, e),
            });
        }
        *state = MovementState {
            on_ground: !me.airborne,
            crouching: me.crouch >= 0.5,
            eye_offset: Vec3::Y * (d.eye(me.crouch) - d.centre_above_origin(me.crouch)) * s,
            ..default()
        };
    }
}

/// The entity's collider follows the pill; a player that leaves this
/// movement gets the standard capsule back.
fn sync_collider(
    mut q: Query<(&HaloMovement, &mut Collider), Changed<HaloMovement>>,
    mut removed: RemovedComponents<HaloMovement>,
    mut others: Query<&mut Collider, Without<HaloMovement>>,
    data: Res<MovementData>,
) {
    let s = crate::game::METERS_PER_UNIT;
    for e in removed.read() {
        if let Ok(mut c) = others.get_mut(e) {
            *c = Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS);
        }
    }
    for (me, mut collider) in &mut q {
        let radius = data.radius * s;
        let half = data.segment(me.crouch) * s / 2.0;
        let same = collider
            .shape_scaled()
            .as_capsule()
            .is_some_and(|c| (c.half_height() - half).abs() < 1e-6 && (c.radius - radius).abs() < 1e-6);
        if !same {
            *collider = Collider::capsule(radius, 2.0 * half);
        }
    }
}
