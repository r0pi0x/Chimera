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
//! - `falls`: a Halo map's too-fast-fall rule (`globals\distance`, 105 a
//!   30 Hz tick) for every other game's body on it; Halo's own movement
//!   deals it in its tick, one hit a 30 Hz tick under any engine tick.
//!
//! A campaign body (`campaign::BodyKind`, the party apps' Elite) moves by
//! its own table (`campaign::movement_of`, used by `step`, the collider,
//! the footing and the seat): its pill, camera and blended speeds, and
//! whatever else data/campaign.toml sets; the mover is this one for every
//! body (the owner's rule: `campaign` module doc).
//!
//! Another game's push (`core::ExternalPush`: TF2's knockback in a party
//! match) is honoured as every movement does: a lift makes the biped
//! airborne (as `push`); its ground limit applies on the first tick it
//! starts on the ground, after the ground step (`tick::Controls::
//! push_limit`), as a share of the forward run speed at its crouch blend
//! (x stun and m_p) - Halo's "max speed"; in the air, Halo's own air
//! control.
//!
//! Teleporters are the engine's (`map::teleport`, from the map's netgame
//! flags: `maps::teleporters`), which move a Halo player as any other.
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
//! On another game's map (the party apps) the pill is the map's own
//! character's size where that is smaller (`body`, the engine's movement
//! envelope rule, `chimera_core::envelope`; Halo states its own with
//! `envelope`): narrower, lower standing and crouched, a lowered end's eye
//! `EYE_MARGIN` under its top or at the map's character's eye (whichever
//! is higher, never above Halo's own), and a step-up (`mover::step_up`) to the
//! map's character's step where that is higher than the 0.375 m Halo rides
//! up (a landing on a stair's nosing stands on its top, `Space::ledge`, so
//! stairs too steep to ride are walked up). The model, hit boxes, jump and gravity stay Halo's. On Halo's maps
//! and without a map envelope the numbers are the table's, to the bit.
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
//!   the player rests as the spec's blocked rows (from their tick, speed
//!   0 forward and up), but the cast's edge normal is not exactly level:
//!   it drifts along the edge by up to 2.4e-4 WU per tick, and once in a
//!   while the cast loses the floor under it for a tick.
//! - The in-air kill damage is dealt once per engine tick (per Halo tick at
//!   30 Hz); a fall's hit is written only on a first-time prediction.
//! - Stun is read as its value s_b alone: the stunned moving animations
//!   (above 0.2) have no body to play on yet.
//! - Climbable surfaces: a contact inside any ladder volume the engine
//!   publishes (`core::MapLadders`: a Halo map's climbable sheets,
//!   `maps::collision`; another game's map's ladders, its ladder lines
//!   as volumes `LINE_REACH` around them; games'
//!   `LadderZone`s) is climbable (`space::EngineSpace::ladders`, within
//!   3 cm), and counts as ground by the spec's rule (`tick`). The edge
//!   neighbour's hold (`mover::hold_on_neighbour`) knows only ladder
//!   sheets as support surfaces (`space::ladder_support`, from the
//!   ladder brushes' planes): a floor whose edge neighbour is climbable
//!   (no multiplayer map has one) does not hold.
//! - Not modelled: body facing and the animation states (cosmetic: no
//!   bodies yet), slipping (an impulse, `push`, is added to the velocity
//!   and makes the player airborne, but does not slip), elevators (no
//!   multiplayer map has one),
//!   volatile contacts and the 16-contact splatter flag, the pitch clamp
//!   (the input layer's), the airborne tick counter (animation only).

pub mod data;
pub mod falls;
pub mod mover;
pub mod space;
pub mod tick;

use std::borrow::Cow;

use avian3d::{character_controller::move_and_slide::DepenetrationConfig, prelude::*};
use bevy::prelude::*;
use chimera_movement::props::PassedProps;
use chimera_core::{
    character::{CAPSULE_HEIGHT, CAPSULE_RADIUS},
    core::MapGame,
    envelope::{MovementEnvelope, MovementEnvelopeAppExt, MovementEnvelopes, map_envelope},
    slots::{Footing, Loadout, RegisterSlots},
};

pub use data::MovementData;
pub use tick::{Controls, Fall, HaloMovement, Landing};

use super::{
    campaign::{BodyKind, BodyTables, movement_of},
    damage::HaloHit,
    vitality::{DamageEffects, Stun, UnitStep},
};
use crate::core::{
    CHARACTER_FILTER, Dead, ExternalPush, LadderZone, MapBrush, MapLadders, FirstTimePredicted, Intent, MovementState, Predict, PredictedAppExt, SimClock, Velocity,
};

/// The movement's id.
pub const ID: &str = "halo_ce:movement";

/// How far around another game's ladder line (HL2's, 18 Source units in
/// front of its ladder's geometry) its climbable volume reaches (m; ours:
/// 24 units), so the wall behind it is a climbable contact.
const LINE_REACH: f32 = 24.0 * 0.0254;

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
        register(app);
        app.insert_resource(Loadout { movement: ID });
    }
}

/// Halo's movement registered under its id without making it the match's:
/// what an app of several games adds for Halo's guns, whose holders move
/// as Halo's players (`parts::add_guns`). Once per app.
pub fn register(app: &mut App) {
    if app.world().contains_resource::<MovementData>() {
        return;
    }
    let d = MovementData::builtin();
    app.add_movement_envelope(crate::game::GAME, envelope(&d));
    app.insert_resource(d)
        .register_movement::<HaloMovement>(ID)
        .seat_movement(ID, footing, seat)
        .add_systems(Predict::Movement, (step.after(UnitStep), sync_collider).chain().before(crate::core::AfterMovements))
        .add_systems(
            FixedUpdate,
            falls::other_bodies
                .after(crate::core::SimSet::Movement)
                .before(crate::core::SimSet::Weapons)
                .run_if(crate::core::authoritative)
                .run_if(falls::on_halo_map),
        )
        .predicted_net::<HaloMovement>();
}

/// An impulse on a living player (movement.md 8, projectiles.md 11: the
/// explosions' push, already halved for a living biped): `dv` (metres per
/// second) added to its velocity, and airborne. The one seam the
/// movement offers its pushers.
pub fn push(me: Option<&mut HaloMovement>, velocity: &mut Velocity, dv: Vec3) {
    velocity.0 += dv;
    if let Some(m) = me {
        m.airborne = true;
    }
}

/// How far (m) the eye stays under the top of a body the map's envelope
/// made lower than Halo's own (`body`): under a lower ceiling the eye
/// stays inside it. Halo's own eye is 0.24 m under its top (0.15 WU
/// crouched), so on its own maps this changes nothing.
pub const EYE_MARGIN: f32 = chimera_core::envelope::EYE_MARGIN;

/// Halo's own movement envelope (`chimera_core::envelope`; metres): the
/// pill's radius and heights, the edge it rides up at a run as its step
/// (`MovementData::ride_step`), and its cameras as the eyes.
pub fn envelope(d: &MovementData) -> MovementEnvelope {
    let s = crate::game::METERS_PER_UNIT;
    MovementEnvelope {
        radius: d.radius * s,
        height: d.height_stand * s,
        crouch_height: d.height_crouch * s,
        step: d.ride_step * s,
        eye: d.camera_stand * s,
        crouch_eye: d.camera_crouch * s,
    }
}

/// The movement's numbers for a Halo body on a map whose envelope is `map`
/// (the envelope's rule, `MovementEnvelope::within`): a narrower pill, a
/// lower one standing and crouched (a lowered end's eye `EYE_MARGIN` under
/// its top or at the map's character's eye, whichever is higher, never
/// above its own; both ends of the crouch blend, so it moves smoothly), and the step-up
/// (`MovementData::step_up`) when the map's character steps higher. The
/// table itself, untouched, where the rule changes nothing (Halo's maps,
/// no map envelope): the same numbers to the bit.
pub fn body<'a>(d: &'a MovementData, map: Option<&MovementEnvelope>) -> Cow<'a, MovementData> {
    let own = envelope(d);
    let b = own.within(map);
    if b == own {
        return Cow::Borrowed(d);
    }
    let s = crate::game::METERS_PER_UNIT;
    let mut out = d.clone();
    if b.height < own.height {
        out.height_stand = b.height / s;
    }
    if b.crouch_height < own.crouch_height {
        out.height_crouch = (b.crouch_height / s).min(out.height_stand);
    }
    if b.radius < own.radius {
        out.radius = b.radius / s;
    }
    out.radius = out.radius.min(out.height_crouch / 2.0);
    if b.step > own.step {
        out.step_up = b.step / s;
    }
    if out.height_stand < d.height_stand {
        out.camera_stand = d.camera_stand.min(b.eye / s);
    }
    if out.height_crouch < d.height_crouch {
        out.camera_crouch = d.camera_crouch.min(b.crouch_eye / s);
    }
    Cow::Owned(out)
}

/// The loaded map's envelope as the movement's systems read it.
fn map_body_envelope(envelopes: Option<&MovementEnvelopes>, game: Option<&MapGame>) -> Option<MovementEnvelope> {
    envelopes?.for_map(game)
}

/// The engine tick's length in Halo ticks: exactly 1 at Halo's own tick.
pub fn tick_scale(clock: &SimClock) -> f32 {
    let k = clock.delta.as_secs_f64() / crate::game::TICK_INTERVAL;
    if (k - 1.0).abs() < 1e-4 { 1.0 } else { k as f32 }
}

/// The movement table of body `e` (`campaign::movement_of`: its kind's,
/// else the Spartan's `MovementData`).
fn table_of(world: &World, e: Entity) -> Option<&MovementData> {
    let d = world.get_resource::<MovementData>()?;
    Some(crate::game::campaign::movement_of(d, world.get_resource(), world.get(e)))
}

/// Where a Halo player stands (`slots::Footing`): its origin.
fn footing(world: &World, e: Entity) -> Option<Footing> {
    let me = world.get::<HaloMovement>(e)?;
    let t = world.get::<Transform>(e)?;
    let d = table_of(world, e)?;
    let d = body(d, map_envelope(world).as_ref());
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
    let Some(d) = table_of(world, e) else { return };
    let d = body(d, map_envelope(world).as_ref()).into_owned();
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
        (Option<&Stun>, Option<&SpeedMultiplier>, Has<ControllerStick>, Has<Dead>, Option<&BodyKind>),
        Option<&mut ExternalPush>,
    ), Without<crate::game::vehicles::Seated>>,
    move_and_slide: MoveAndSlide,
    data: Res<MovementData>,
    clock: Res<SimClock>,
    first: Option<Res<FirstTimePredicted>>,
    effects: Res<DamageEffects>,
    mut hits: MessageWriter<HaloHit>,
    (map_ladders, zones): (Option<Res<MapLadders>>, Query<&LadderZone>),
    (envelopes, map_game, tables): (Option<Res<MovementEnvelopes>>, Option<Res<MapGame>>, Option<Res<BodyTables>>),
    props: PassedProps,
) {
    let map = map_body_envelope(envelopes.as_deref(), map_game.as_deref());
    // The physics props its casts pass, by the map's rule
    // (`chimera_movement::props`): multiplayer ones, or under turbo physics
    // those in the push-away group and debris.
    let passed = props.passed(props.rule(Some(ID)));
    // Every ladder, climbed by Halo's rule (docs/DESIGN.md, "Ladders
    // across games"): the map's volumes, the games' zones, and the map's
    // ladder lines (HL2's) as volumes reaching the geometry behind them
    // (`LINE_REACH` around the line).
    let ladders: Vec<MapBrush> = map_ladders
        .iter()
        .flat_map(|l| l.volumes.iter().cloned())
        .chain(zones.iter().map(|z| z.0.clone()))
        .chain(map_ladders.iter().flat_map(|l| l.lines.iter()).map(|l| {
            let r = Vec3::new(LINE_REACH, 0.0, LINE_REACH);
            MapBrush { ladder: true, ..MapBrush::from_box(l.bottom.min(l.top) - r, l.bottom.max(l.top) + r) }
        }))
        .collect();
    let s = crate::game::METERS_PER_UNIT;
    let per_tick = s * data::TICKS_PER_SECOND;
    let k = tick_scale(&clock);
    let first = first.is_none_or(|f| f.0);
    for (e, intent, mut me, mut transform, mut vel, mut state, (stun, mult, controller, dead, kind), mut pushed) in &mut q {
        // How this body moves (`campaign::movement_of`: its kind's table),
        // in the map's envelope (`body`).
        let d = &*body(movement_of(&data, tables.as_deref(), kind), map.as_ref());
        if dead {
            me.was_dead = true;
            continue;
        }
        let filter = PassedProps::filter(e, &passed, CHARACTER_FILTER);
        let space = space::EngineSpace::new(&move_and_slide, filter.clone(), d).with_ladders(&ladders);
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
        // Another game's push (`ExternalPush`): a lift takes the biped
        // off as `push` does; on the ground its limit applies this tick.
        if let Some(p) = pushed.as_mut().filter(|p| p.lift) {
            p.lift = false;
            me.airborne = true;
        }
        let push_limit = match pushed.as_mut() {
            Some(p) if !me.airborne && p.ground_limit > 0.0 => p.take_on_ground().unwrap_or(0.0),
            _ => 0.0,
        };
        let origin = transform.translation / s - Vec3::Y * d.centre_above_origin(me.crouch);
        let velocity = vel.0 / per_tick;
        let rot = Quat::from_rotation_y(intent.yaw);
        let controls = Controls {
            throttle: Vec2::new(intent.move_axis.y, -intent.move_axis.x),
            forward: rot * Vec3::NEG_Z,
            left: rot * Vec3::NEG_X,
            aim: rot * Quat::from_rotation_x(intent.pitch) * Vec3::NEG_Z,
            jump: intent.jump,
            crouch: intent.crouch,
            controller,
            stun: stun.map_or(0.0, |s| s.value),
            speed_multiplier: mult.map_or(1.0, |m| m.0),
            push_limit,
        };
        let out = tick::tick(d, &mut me, origin, velocity, controls, k, &space);
        transform.translation = (out.origin + Vec3::Y * d.centre_above_origin(me.crouch)) * s;
        vel.0 = out.velocity * per_tick;
        if first && let Some(fall) = out.damage {
            match fall {
                Fall::Landing(q_f) => {
                    hits.write(HaloHit::fall(&effects, e, q_f));
                }
                // One a 30 Hz tick (movement.md 6), whatever the engine's
                // tick: none on an engine tick that crosses no Halo tick.
                Fall::Lethal => {
                    for _ in 0..crate::game::ticks::halo_ticks(&clock) {
                        hits.write(HaloHit::lethal_fall(&effects, e));
                    }
                }
            }
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
#[allow(clippy::type_complexity)]
fn sync_collider(
    mut q: Query<(&HaloMovement, &mut Collider, Option<&BodyKind>), Or<(Changed<HaloMovement>, Changed<BodyKind>)>>,
    mut removed: RemovedComponents<HaloMovement>,
    mut others: Query<&mut Collider, Without<HaloMovement>>,
    data: Res<MovementData>,
    (envelopes, map_game, tables): (Option<Res<MovementEnvelopes>>, Option<Res<MapGame>>, Option<Res<BodyTables>>),
) {
    let map = map_body_envelope(envelopes.as_deref(), map_game.as_deref());
    let s = crate::game::METERS_PER_UNIT;
    for e in removed.read() {
        if let Ok(mut c) = others.get_mut(e) {
            *c = Collider::capsule(CAPSULE_RADIUS, CAPSULE_HEIGHT - 2.0 * CAPSULE_RADIUS);
        }
    }
    for (me, mut collider, kind) in &mut q {
        let data = body(movement_of(&data, tables.as_deref(), kind), map.as_ref());
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
