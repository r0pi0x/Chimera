//! Halo's vehicles (specs/halo_ce/vehicles.md): the data (`data`), the
//! mass-point physics (`physics`; its doc gives the design: Halo's own
//! integrator on the engine's collision queries, the result a kinematic
//! avian body), the driving rules around it (`drive`), the level as the
//! points see it (`level`), the scenario's placements (`place`), and here
//! the vehicles in the world:
//!
//! - `attach`: a placed vehicle's prop node (`place::Placed` in the map's
//!   entities) becomes a `HaloVehicle`: a kinematic body with the convex
//!   hull of its collision model as collider (characters stand on it and
//!   bump into it, shots stop on it and do nothing to it: no vitality on the
//!   disc), at its placement raised by half its bounding radius.
//! - `seat_input` (authoritative): the use key (pressed) on foot near a
//!   seat marker enters by the spec's seat search (`seats::best_seat`:
//!   within 1 WU, slower than 0.1 WU/t, the vehicle turning slower than
//!   0.1 rad/t and upright; an empty seat; a driver seat preferred by the
//!   1.5 factor; seat flag 9 needs a driver); pressed again seated, the
//!   rider gets out beside its seat with the vehicle's velocity; on an
//!   overturned driverless vehicle it starts the flip; riders of a grounded
//!   vehicle with up.z < 0.2 are thrown out.
//! - `simulate` (authoritative): each Halo tick of the engine tick
//!   (`ticks::halo_ticks`), the driver's `Intent` as the spec's controls
//!   (t_i forward, t_j left, the look as the desired facing, jump the
//!   brake), `drive::Drive::tick` on `level::EngineLevel`, the pose written
//!   to the prop's `Transform` (replicated to clients as the engine's other
//!   moving props: `chimera_net`'s `NetProp`, the vehicle carrying
//!   `PhysicsProp`; no new message).
//! - `carry`: riders sit at their seat's marker (their movement skips them:
//!   `Seated`), moving with the vehicle; the driver's camera is the seat's
//!   camera track (`seats::track_offset`) as the character's `MapView`.
//! - `splatter`: a character touching a moving vehicle is pushed and takes
//!   the globals' vehicle collision damage by the spec's rule (`HaloHit`,
//!   area, owned by the driver), with the 90-tick grace after leaving it.
//! - `rider_damage`: a hit on a vehicle passes to each rider at (1 −
//!   passthrough penalty) × rider damage fraction (0.18).
//!
//! Known shortcuts:
//! - Entering and exiting are instant (no enter or exit animation, so no
//!   entry-point search, animation lock or root-node exit position: the
//!   rider leaves 1 WU to its seat's side); the use key is edge-triggered.
//! - The rider's body is not posed by the seat's animation, the driver's
//!   own weapon is not put away, and the seat's look rates and limits and
//!   the camera's auto-level are not applied (the mouse look is free; the
//!   camera's focus is the seat marker raised 0.5 WU).
//! - Wheels, steering and suspension are not shown on the model's nodes
//!   (the model is drawn rigid, as the scenery is).
//! - The seated state is not replicated: a networked client's own player
//!   is not shown seated (the party apps come with stage 2).
//! - Other vehicles' contacts (mass point against mass point) and
//!   vehicle-vehicle collision accumulators are not gathered; vehicles meet
//!   each other's hulls through the ground query.

pub mod data;
pub mod drive;
pub mod level;
pub mod physics;
pub mod place;
pub mod seats;

use std::collections::BTreeMap;

use avian3d::prelude::*;
use bevy::prelude::*;

use self::data::{VehicleDef, seat_flags};
use self::drive::{Controls, Drive};
use self::level::{EngineLevel, to_engine, to_halo, to_halo_dir};
use self::physics::{Body, GroundScales, PhysicsDef};
use self::place::Placed;
use super::damage::HaloHit;
use super::movement::{HaloMovement, MovementData};
use super::vitality::{DamageEffects, ids};
use crate::core::{Dead, Intent, MapView, SimClock, SimSet, Velocity, authoritative};
use crate::map::{MapEntities, PhysicsProp, PropIndex, PushAway, contact_filter};

/// Every vehicle document with its physics as the step reads it.
#[derive(Resource, Clone, Debug)]
pub struct VehicleDefs(pub Vec<(VehicleDef, PhysicsDef)>);

impl Default for VehicleDefs {
    fn default() -> Self {
        Self(
            VehicleDef::builtin()
                .into_iter()
                .map(|d| {
                    let p = d.physics_def();
                    (d, p)
                })
                .collect(),
        )
    }
}

/// The materials' vehicle terrain scales by material type (globals.md
/// "Materials"; on the disc only ice, type 31, scales the ground friction,
/// by 0.125; the heavy test `vehicle_install` checks the disc).
pub fn ground_scales() -> Vec<GroundScales> {
    let mut s = vec![GroundScales::default(); 32];
    s[31].friction = 0.125;
    s
}

/// A vehicle in the world.
#[derive(Component, Clone, Debug)]
pub struct HaloVehicle {
    /// Index into `VehicleDefs`.
    pub def: usize,
    pub drive: Drive,
    /// Who sits in each seat.
    pub riders: Vec<Option<Entity>>,
    /// Seat and camera markers, model space (WU).
    pub markers: BTreeMap<String, Vec3>,
}

impl HaloVehicle {
    /// The vehicle's driver, if any.
    pub fn driver(&self, def: &VehicleDef) -> Option<Entity> {
        def.seats
            .iter()
            .zip(&self.riders)
            .find(|(s, r)| s.flags & seat_flags::DRIVER != 0 && r.is_some())
            .and_then(|(_, r)| *r)
    }

    /// A model-space point's world position (WU).
    pub fn world(&self, def: &PhysicsDef, q: Vec3) -> Vec3 {
        self.drive.body.point(q, def.center_of_mass)
    }

    /// A seat's marker in the model (WU); the seat's own marker, or a
    /// fallback beside the centre when the model has none (a synthetic
    /// map).
    pub fn seat_marker(&self, def: &VehicleDef, seat: usize) -> Vec3 {
        let name = def.seats[seat].marker.to_lowercase();
        self.markers
            .get(&name)
            .copied()
            .unwrap_or_else(|| seats::fallback_marker(seat))
    }
}

/// A character sitting in a vehicle's seat: its movement is suspended.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Seated {
    pub vehicle: Entity,
    pub seat: usize,
}

/// The vehicle a character last left and when (engine seconds), for the
/// splatter grace.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LastVehicle {
    pub vehicle: Entity,
    pub at: f64,
}

/// The use key's state last tick, per character (edge detection).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct UseHeld(pub bool);

/// Halo's vehicles.
pub struct HaloVehiclesPlugin;

impl Plugin for HaloVehiclesPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::console::ConsoleAppExt;
        app.console_command(
            "halo_vehicle_enter",
            "Seat the local player in the nearest vehicle's empty driver seat (screenshots)",
            |w, _| match enter_nearest(w) {
                Ok(m) => Ok(Some(m)),
                // Not yet (a launch's `+halo_vehicle_enter` runs before
                // the map loads): retried each tick until it works.
                Err(_) => {
                    w.insert_resource(EnterWhenReady);
                    Ok(Some("will seat the local player once a vehicle is near".into()))
                }
            },
        );
        app.console_command(
            "halo_vehicle_throttle",
            "Hold every driver's throttle at <t> (-1 to 1; no argument: release) (screenshots, frame dumps)",
            |w, args| {
                match args.first().map(|a| a.parse::<f32>()) {
                    Some(Ok(t)) => w.insert_resource(HeldThrottle(t.clamp(-1.0, 1.0))),
                    Some(Err(_)) => return Err("expected a number".into()),
                    None => {
                        w.remove_resource::<HeldThrottle>();
                    }
                }
                Ok(None)
            },
        );
        app.init_resource::<VehicleDefs>().add_systems(
            FixedUpdate,
            (
                attach,
                (seat_input, simulate, splatter, rider_damage)
                    .chain()
                    .run_if(authoritative),
                carry,
                enter_when_ready.run_if(resource_exists::<EnterWhenReady>),
            )
                .chain()
                .after(SimSet::Movement)
                .before(SimSet::Weapons),
        );
    }
}

/// `halo_vehicle_throttle`: every driver's forward throttle held.
#[derive(Resource, Clone, Copy, Debug)]
pub struct HeldThrottle(pub f32);

/// `halo_vehicle_enter` waiting for a local player and a vehicle.
#[derive(Resource, Clone, Copy, Debug)]
pub struct EnterWhenReady;

fn enter_when_ready(
    players: Query<(Entity, &Transform), (With<crate::core::LocalPlayer>, Without<Seated>)>,
    mut vehicles: Query<(Entity, &mut HaloVehicle)>,
    defs: Res<VehicleDefs>,
    mut commands: Commands,
) {
    let Some((p, at)) = players.iter().next() else { return };
    let best = vehicles
        .iter()
        .filter_map(|(e, v)| {
            let seat = defs.0[v.def].0.driver_seat().filter(|&s| v.riders[s].is_none())?;
            Some(((to_engine(v.drive.body.position) - at.translation).length(), e, seat))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, ve, seat)) = best else { return };
    if let Ok((_, mut v)) = vehicles.get_mut(ve) {
        v.riders[seat] = Some(p);
        commands.entity(p).insert(Seated { vehicle: ve, seat });
        commands.remove_resource::<EnterWhenReady>();
    }
}

/// The local player into the nearest vehicle's empty driver seat.
fn enter_nearest(w: &mut World) -> Result<String, String> {
    let mut players = w.query_filtered::<(Entity, &Transform), (With<crate::core::LocalPlayer>, Without<Seated>)>();
    let (p, at) = players
        .iter(w)
        .next()
        .map(|(e, t)| (e, t.translation))
        .ok_or("no local player on foot")?;
    let defs = w.get_resource_or_init::<VehicleDefs>().clone();
    let mut vehicles = w.query::<(Entity, &HaloVehicle)>();
    let (ve, seat) = vehicles
        .iter(w)
        .filter_map(|(e, v)| {
            let d = &defs.0[v.def].0;
            let seat = d.driver_seat().filter(|&s| v.riders[s].is_none())?;
            Some(((to_engine(v.drive.body.position) - at).length(), e, seat))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, e, s)| (e, s))
        .ok_or("no vehicle with an empty driver seat")?;
    w.get_mut::<HaloVehicle>(ve).ok_or("gone")?.riders[seat] = Some(p);
    w.entity_mut(p).insert(Seated { vehicle: ve, seat });
    Ok(format!("seated in {ve}"))
}

/// Engine rotation of a body's frame (`objects::placement_rotation`).
pub fn engine_rotation(b: &Body) -> Quat {
    super::maps::objects::placement_rotation(&blam_formats::scenario::Orientation {
        forward: b.forward.to_array(),
        left: b.left().to_array(),
        up: b.up.to_array(),
    })
}

#[allow(clippy::type_complexity)]
fn attach(
    mut commands: Commands,
    nodes: Query<(Entity, &PropIndex), Without<HaloVehicle>>,
    entities: Option<Res<MapEntities>>,
    defs: Res<VehicleDefs>,
    role: Option<Res<crate::core::NetRole>>,
) {
    let Some(entities) = entities else { return };
    if nodes.is_empty() {
        return;
    }
    let placed: Vec<Placed> = entities.entities.iter().filter_map(Placed::from_entity).collect();
    let server = role.is_none_or(|r| *r != crate::core::NetRole::Client);
    for (e, index) in &nodes {
        let Some(p) = placed.iter().find(|p| p.prop == index.0) else {
            continue;
        };
        let Some(def) = defs.0.iter().position(|(d, _)| d.name == p.vehicle) else {
            continue;
        };
        let (d, phys) = &defs.0[def];
        let vehicle = spawn_state(d, phys, def, p);
        let hull: Vec<Vec3> = p.hull.iter().map(|q| crate::game::maps::space::point(*q)).collect();
        let collider = Collider::convex_hull(&hull).unwrap_or_else(|| Collider::cuboid(4.0, 2.0, 5.0));
        let t = Transform::from_translation(to_engine(vehicle.drive.body.origin(phys.center_of_mass)))
            .with_rotation(engine_rotation(&vehicle.drive.body));
        let mut ec = commands.entity(e);
        ec.insert((vehicle, t, RigidBody::Kinematic, collider, contact_filter::hooks()));
        if server {
            ec.insert(PhysicsProp {
                push: PushAway::Collide,
                mass: phys.mass,
                bounds: (Vec3::splat(-1.0), Vec3::splat(1.0)),
            });
        }
    }
}

/// Spawn the vehicle `name` with its model origin at `origin` (WU, Halo
/// axes) facing `heading` (radians), raised as created: a vehicle without a
/// map (tests, a game's own spawns). Its collider is a box around its mass
/// points (no collision model). Returns the entity.
pub fn spawn_vehicle(world: &mut World, name: &str, origin: Vec3, heading: f32) -> Option<Entity> {
    let defs = world.get_resource_or_init::<VehicleDefs>().clone();
    let def = defs.0.iter().position(|(d, _)| d.name == name)?;
    let (d, phys) = &defs.0[def];
    let placed = Placed {
        prop: usize::MAX,
        vehicle: name.to_string(),
        position: origin.to_array(),
        forward: [heading.cos(), heading.sin(), 0.0],
        up: [0.0, 0.0, 1.0],
        markers: Default::default(),
        hull: Vec::new(),
    };
    let v = spawn_state(d, phys, def, &placed);
    let (lo, hi) = phys.points.iter().fold((Vec3::MAX, Vec3::MIN), |(lo, hi), p| {
        (lo.min(p.position - p.radius * 0.6), hi.max(p.position + p.radius * 0.6))
    });
    let centre = crate::game::maps::space::point(((lo + hi) / 2.0).to_array());
    let size = crate::game::maps::space::direction((hi - lo).to_array()).abs() * crate::game::METERS_PER_UNIT;
    let collider = Collider::compound(vec![(centre, Quat::IDENTITY, Collider::cuboid(size.x, size.y, size.z))]);
    let t = Transform::from_translation(to_engine(v.drive.body.origin(phys.center_of_mass)))
        .with_rotation(engine_rotation(&v.drive.body));
    Some(
        world
            .spawn((v, t, RigidBody::Kinematic, collider, contact_filter::hooks()))
            .id(),
    )
}

/// A placed vehicle's starting state: its centre of mass where its origin
/// at the placement raised by half its bounding radius puts it.
pub fn spawn_state(d: &VehicleDef, phys: &PhysicsDef, def: usize, p: &Placed) -> HaloVehicle {
    let mut body = Body::at(Vec3::ZERO, 0.0);
    body.forward = Vec3::from(p.forward).normalize_or(Vec3::X);
    body.up = Vec3::from(p.up).normalize_or(Vec3::Z);
    let origin = Vec3::from(p.position) + Vec3::Z * (d.bounding_radius / 2.0);
    body.position = origin + body.frame() * phys.center_of_mass;
    HaloVehicle {
        def,
        drive: Drive::new(body),
        riders: vec![None; d.seats.len()],
        markers: p.markers.iter().map(|(k, v)| (k.clone(), Vec3::from(*v))).collect(),
    }
}

/// The controls a driver's intent gives (vehicles.md "Keyboard and
/// mouse"): forward and strafe as t_i and t_j, the look as the facing.
pub fn controls(intent: &Intent) -> Controls {
    let look = Quat::from_rotation_y(intent.yaw) * Quat::from_rotation_x(intent.pitch) * Vec3::NEG_Z;
    Controls {
        driver: true,
        throttle: [intent.move_axis.y, -intent.move_axis.x],
        facing: to_halo_dir(look).normalize_or(Vec3::X),
        jump: intent.jump,
        crouch: intent.crouch,
    }
}

#[allow(clippy::type_complexity)]
fn seat_input(
    mut vehicles: Query<(Entity, &mut HaloVehicle)>,
    mut people: Query<
        (
            Entity,
            &Intent,
            &Transform,
            &Velocity,
            Option<&Seated>,
            Option<&mut UseHeld>,
            Has<Dead>,
        ),
        With<HaloMovement>,
    >,
    defs: Res<VehicleDefs>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let s = crate::game::METERS_PER_UNIT;
    let now = clock.now;
    for (e, intent, t, vel, seated, held, dead) in &mut people {
        let was = held.as_ref().is_some_and(|h| h.0);
        match held {
            Some(mut h) => {
                if h.0 != intent.use_key {
                    h.0 = intent.use_key;
                }
            }
            None => {
                commands.entity(e).insert(UseHeld(intent.use_key));
            }
        }
        let pressed = intent.use_key && !was;
        if let Some(seat) = seated.copied() {
            let Ok((ve, mut v)) = vehicles.get_mut(seat.vehicle) else {
                commands.entity(e).remove::<Seated>();
                continue;
            };
            let (d, phys) = &defs.0[v.def];
            let thrown = v.drive.on_ground && v.drive.body.up.z < drive::EJECT_UP;
            if pressed || thrown || dead {
                exit(&mut commands, e, ve, &mut v, d, phys, seat.seat, now);
            }
            continue;
        }
        if !pressed || dead {
            continue;
        }
        let centre = to_halo(t.translation);
        let speed2 = (vel.0 / (s * 30.0)).length_squared();
        let mut best: Option<(f32, Entity, seats::Offer)> = None;
        for (ve, v) in &vehicles {
            let (d, phys) = &defs.0[v.def];
            if let Some((dist, offer)) = seats::offer(d, phys, v, centre, speed2)
                && best.as_ref().is_none_or(|b| dist < b.0)
            {
                best = Some((dist, ve, offer));
            }
        }
        let Some((_, ve, offer)) = best else { continue };
        let Ok((_, mut v)) = vehicles.get_mut(ve) else { continue };
        match offer {
            seats::Offer::Enter(seat) => {
                v.riders[seat] = Some(e);
                commands.entity(e).insert(Seated { vehicle: ve, seat });
            }
            seats::Offer::Flip => {
                v.drive.start_flip(centre);
                commands.entity(e).insert(LastVehicle { vehicle: ve, at: now });
            }
        }
    }
}

/// A rider leaves its seat (module doc: beside it, with the vehicle's
/// velocity).
#[allow(clippy::too_many_arguments)]
fn exit(
    commands: &mut Commands,
    e: Entity,
    ve: Entity,
    v: &mut HaloVehicle,
    d: &VehicleDef,
    phys: &PhysicsDef,
    seat: usize,
    now: f64,
) {
    if v.riders.get(seat).copied().flatten() == Some(e) {
        v.riders[seat] = None;
    }
    let s = crate::game::METERS_PER_UNIT;
    let marker = v.seat_marker(d, seat);
    let side = if marker.y >= 0.0 { 1.0 } else { -1.0 };
    let out = v.world(phys, marker + Vec3::new(0.0, side * 1.0, 0.3));
    let at = to_engine(out);
    let velocity = to_engine(v.drive.body.velocity) * 30.0;
    commands
        .entity(e)
        .remove::<Seated>()
        .insert(LastVehicle { vehicle: ve, at: now });
    commands.queue(move |w: &mut World| {
        let Ok(mut ent) = w.get_entity_mut(e) else { return };
        if let Some(mut t) = ent.get_mut::<Transform>() {
            t.translation = at;
        }
        if let Some(mut p) = ent.get_mut::<Position>() {
            p.0 = at;
        }
        if let Some(mut vel) = ent.get_mut::<Velocity>() {
            vel.0 = velocity;
        }
        if let Some(mut m) = ent.get_mut::<HaloMovement>() {
            m.airborne = true;
        }
    });
    let _ = s;
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn simulate(
    mut vehicles: Query<(Entity, &mut HaloVehicle, &mut Transform)>,
    drivers: Query<&Intent>,
    characters: Query<(), With<HaloMovement>>,
    defs: Res<VehicleDefs>,
    clock: Res<SimClock>,
    query: SpatialQuery,
    materials: Option<Res<crate::game::weapons::materials::LevelMaterials>>,
    held: Option<Res<HeldThrottle>>,
    shapes: Query<(
        &'static Collider,
        Option<&'static Position>,
        Option<&'static Rotation>,
        Option<&'static Name>,
    )>,
) {
    let n = super::ticks::halo_ticks(&clock);
    if n == 0 {
        return;
    }
    let held = held.as_deref().copied();
    let scales = ground_scales();
    let skip = |e: Entity| characters.contains(e);
    for (e, mut v, mut t) in &mut vehicles {
        let (d, phys) = &defs.0[v.def];
        let level = EngineLevel {
            query: &query,
            filter: SpatialQueryFilter::from_excluded_entities([e]).with_mask(level::mask()),
            skip: &skip,
            materials: materials.as_deref(),
            shapes: &shapes,
        };
        let c = v
            .driver(d)
            .and_then(|r| drivers.get(r).ok())
            .map(|i| {
                let mut c = controls(i);
                if let Some(h) = held {
                    c.throttle[0] = h.0;
                }
                c
            })
            .unwrap_or(Controls {
                facing: v.drive.body.forward,
                ..Default::default()
            });
        for _ in 0..n {
            v.drive.tick(d, phys, &c, &level, &scales);
        }
        let origin = to_engine(v.drive.body.origin(phys.center_of_mass));
        let rotation = engine_rotation(&v.drive.body);
        if t.translation != origin || t.rotation != rotation {
            t.translation = origin;
            t.rotation = rotation;
        }
    }
}

/// Riders follow their seats; the driver's camera.
#[allow(clippy::type_complexity)]
fn carry(
    vehicles: Query<&HaloVehicle>,
    mut riders: Query<
        (
            Entity,
            &Seated,
            &Intent,
            &mut Transform,
            &mut Velocity,
            Option<&mut Position>,
            Option<&MapView>,
        ),
        Without<HaloVehicle>,
    >,
    defs: Res<VehicleDefs>,
    data: Res<MovementData>,
    mut commands: Commands,
    mut gone: RemovedComponents<Seated>,
) {
    let s = crate::game::METERS_PER_UNIT;
    for e in gone.read() {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.try_remove::<MapView>();
        }
    }
    for (e, seat, intent, mut t, mut vel, pos, view) in &mut riders {
        let Ok(v) = vehicles.get(seat.vehicle) else { continue };
        let (d, phys) = &defs.0[v.def];
        let marker = v.world(phys, v.seat_marker(d, seat.seat));
        let centre = to_engine(marker + Vec3::Z * data.centre_above_origin(0.0));
        t.translation = centre;
        if let Some(mut p) = pos {
            p.0 = centre;
        }
        vel.0 = to_engine(v.drive.body.velocity) * 30.0 / 1.0;
        let _ = s;
        let seat_def = &d.seats[seat.seat];
        let track = d.tracks.get(&seat_def.track);
        let focus = match d.seats[seat.seat].camera_marker.to_lowercase() {
            n if !n.is_empty() && v.markers.contains_key(&n) => v.world(phys, v.markers[&n]),
            _ => marker + Vec3::Z * 0.5,
        };
        let third =
            seat_def.flags & (seat_flags::DRIVER | seat_flags::THIRD_PERSON_CAMERA | seat_flags::INVISIBLE) != 0;
        let want = (third && track.is_some()).then(|| {
            let o = seats::track_offset(&track.unwrap().points, intent.pitch);
            let look = Quat::from_rotation_y(intent.yaw) * Vec3::NEG_Z;
            let horizontal = to_halo_dir(look);
            let at = focus + horizontal * o.x + Vec3::Z * o.y;
            MapView {
                origin: to_engine(at),
                rotation: Quat::from_rotation_y(intent.yaw) * Quat::from_rotation_x(intent.pitch),
            }
        });
        match (want, view) {
            (Some(w), Some(have)) if *have == w => {}
            (Some(w), _) => {
                commands.entity(e).insert(w);
            }
            (None, Some(_)) => {
                commands.entity(e).remove::<MapView>();
            }
            (None, None) => {}
        }
    }
}

/// The splatter rule's thresholds (vehicles.md, Constants).
pub const SPLAT_MIN_SPEED: f32 = 0.066_666_67;
pub const SPLAT_MIN_RELATIVE2: f32 = 0.001_111_111_1;
/// Seconds after leaving a vehicle in which it does not hurt (90 ticks).
pub const SPLAT_EXIT_GRACE: f64 = 3.0;
pub const PUSH_LIFT: f32 = 0.8;
pub const PUSH_MIN_SPEED: f32 = 0.1;

/// Whether a touch hurts (the pill fix taken as succeeded): unless the
/// biped left this vehicle within the grace, or the vehicle is slow and
/// moves with the biped.
pub fn splatters(vehicle_velocity: Vec3, biped_velocity: Vec3, in_grace: bool) -> bool {
    let slow = vehicle_velocity.length() <= SPLAT_MIN_SPEED
        && (vehicle_velocity - biped_velocity).length_squared() <= SPLAT_MIN_RELATIVE2;
    !(in_grace || slow)
}

#[allow(clippy::type_complexity)]
fn splatter(
    vehicles: Query<(Entity, &HaloVehicle, &Collider, &Transform)>,
    mut people: Query<
        (
            Entity,
            &Transform,
            &mut Velocity,
            &mut HaloMovement,
            Option<&LastVehicle>,
        ),
        (Without<Seated>, Without<Dead>, Without<HaloVehicle>),
    >,
    defs: Res<VehicleDefs>,
    data: Res<MovementData>,
    effects: Res<DamageEffects>,
    clock: Res<SimClock>,
    mut hits: MessageWriter<HaloHit>,
) {
    let s = crate::game::METERS_PER_UNIT;
    let now = clock.now;
    for (ve, v, collider, vt) in &vehicles {
        let (d, phys) = &defs.0[v.def];
        let vv = v.drive.body.velocity;
        if vv.length_squared() < 1e-8 {
            continue;
        }
        for (pe, pt, mut pvel, mut me, last) in &mut people {
            let dist = collider.distance_to_point(vt.translation, vt.rotation, pt.translation, true) / s;
            if dist > data.radius {
                continue;
            }
            let centre = to_halo(pt.translation);
            let n = ((centre - v.drive.body.position).normalize_or_zero() + Vec3::Z * PUSH_LIFT).normalize_or(Vec3::Z);
            let dv = (n * vv.length().max(PUSH_MIN_SPEED) + vv) / 2.0;
            // A biped takes half a push (movement.md).
            super::movement::push(Some(&mut me), &mut pvel, to_engine(dv / 2.0) * 30.0);
            let biped_v = to_halo_dir(pvel.0) / (s * 30.0);
            let grace = last.is_some_and(|l| l.vehicle == ve && now - l.at < SPLAT_EXIT_GRACE);
            if splatters(vv, biped_v, grace)
                && let Some(effect) = effects.get(ids::VEHICLE_COLLISION)
            {
                let owner = v.driver(d).unwrap_or(ve);
                hits.write(HaloHit::area(
                    pe,
                    ids::VEHICLE_COLLISION,
                    effect.amount(1.0, 0.0),
                    1.0,
                    to_engine(v.drive.body.position),
                    to_engine(n).normalize_or_zero(),
                    Some(owner),
                ));
            }
            let _ = phys;
        }
    }
}

/// Hits on vehicles passed to their riders (module doc).
fn rider_damage(
    mut messages: ResMut<Messages<HaloHit>>,
    mut cursor: Local<bevy::ecs::message::MessageCursor<HaloHit>>,
    vehicles: Query<&HaloVehicle>,
    defs: Res<VehicleDefs>,
    effects: Res<DamageEffects>,
) {
    let mut out = Vec::new();
    for hit in cursor.read(&messages) {
        let Ok(v) = vehicles.get(hit.target) else { continue };
        let d = &defs.0[v.def].0;
        let penalty = effects.get(hit.effect).map_or(0.0, |e| e.passthrough);
        let k = (1.0 - penalty) * d.rider_damage_fraction;
        if k <= 0.0 {
            continue;
        }
        for r in v.riders.iter().flatten() {
            let mut h = hit.clone();
            h.target = *r;
            h.amount *= k;
            out.push(h);
        }
    }
    for h in out {
        messages.write(h);
    }
}
