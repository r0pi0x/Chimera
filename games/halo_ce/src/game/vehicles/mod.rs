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
//!   disc), its centre of mass at its placement raised by half its
//!   bounding radius.
//! - `seat_input` (authoritative): the use key (pressed) on foot near a
//!   seat marker, by any game's character (anything with a movement slot:
//!   a party app's CS:S or Combat Arms player too), enters by the spec's
//!   seat search (`seats::best_seat`:
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
//!   `PhysicsProp`; no new message). Other vehicles and characters are not
//!   ground to its points; vehicles meet each other by the pair rule
//!   (`physics::pair_contact`, into the next step's accumulators).
//! - `sit` and `stand`: a rider takes `Seated`, the engine's `Carried`
//!   (every game's movement skips it: its own movement suspended whatever
//!   game's it is) and the replicated `NetSeat` in one move, and leaves
//!   them in one (entering is rare: a component move, not a flag kept from
//!   spawn); each counts a change of the server's (`core::rebase`).
//! - `carry` (authoritative): riders sit at their seat's marker, upright,
//!   their centre over it by their body's (Halo's biped's, or another
//!   game's collider's half height), moving with the vehicle;
//!   `camera::view` (authoritative): a third-person seat's camera (the
//!   spec's focus, track point and pull-in) as the character's `MapView`.
//! - `tidy` (every tick, every role): no stale seat: a rider that is gone
//!   (a disconnect, a death that took the character away) or sits
//!   elsewhere leaves its seat empty; a rider of a vehicle that is gone (a
//!   map change) stands. `round_restart` (the engine's `RoundRestart`):
//!   every rider stands and every placed vehicle is back at its placement,
//!   at rest. Dying in a seat (marked dead, or no health left) puts the
//!   rider out beside it (`seat_input`); a driver gone leaves the vehicle
//!   coasting.
//! - `look` (every machine): the wheels' spin, the steering and the
//!   suspension on the drawn model's joints.
//! - `splatter`: a character touching a moving vehicle is pushed and takes
//!   the globals' vehicle collision damage by the spec's rule (`HaloHit`,
//!   area, owned by the driver), with the 90-tick grace after leaving it.
//! - `rider_damage`: a hit on a vehicle that is not area damage passes to
//!   each rider at (1 − passthrough penalty) × rider damage fraction
//!   (0.18).
//!
//! **Network.** The vehicle is the server's: simulated there and sent as a
//! moving prop; who sits where goes to every client as `NetSeat` (the
//! vehicle by its map prop index, the seat) on the character (the
//! `halo_ce_vehicles` network part). A client seats its copies by it
//! (`seat_from_server`: `Seated` and `Carried`, its own player's movement
//! suspended), poses its copy of each vehicle where it draws it
//! (`follow_drawn`, `pose_from`), and each frame puts its own seated
//! player in its seat of that drawn vehicle with the seat's camera
//! (`camera::seat_view`) at its own mouse look (`ride_drawn`, before the
//! eye is placed); others are
//! drawn as the server sends them (in their seats in its snapshots). The
//! seated player isn't predicted: its client takes the server's state as
//! it comes (`chimera_net`'s prediction skips the comparison for a
//! `Carried` player; its weapons are still replayed), and its controls
//! reach the vehicle on the server. So driving shows the input after the
//! round trip, the command buffer (about two ticks) and the interpolation
//! delay (`cl_interp`): about 100 ms plus the ping at the defaults; the
//! look is at once.
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
//!   the camera's auto-level are not applied (the mouse look is free); a
//!   first-person seat's eye is the rider's own, not the seat's camera
//!   marker.
//! - The vehicle is not predicted on the driver's client (module doc,
//!   "Network": its input shows after the round trip and the
//!   interpolation delay).
//! - Another game's rider sits upright with its standing hull, its feet at
//!   the seat marker; its weapon stays in hand and works.

pub mod camera;
pub mod data;
pub mod drive;
pub mod level;
pub mod look;
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
use crate::core::{Carried, Dead, Intent, LocalPlayer, MapView, NetRole, SimClock, SimSet, Velocity, authoritative};
use crate::slots::MovementSlot;
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

/// Who sits where, as every machine hears it (replicated by the
/// `halo_ce_vehicles` network part): the vehicle by its map prop index
/// (`map::PropIndex`: the same node on every machine) and the seat. On
/// the server it goes on and off with `Seated`; a client seats its own
/// copies of the characters by it (`seat_from_server`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NetSeat {
    pub prop: u32,
    pub seat: u8,
}

/// The vehicle a character last left and when (engine seconds), for the
/// splatter grace.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct LastVehicle {
    pub vehicle: Entity,
    pub at: f64,
}

/// Halo's vehicles.
pub struct HaloVehiclesPlugin;

impl Plugin for HaloVehiclesPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::console::ConsoleAppExt;
        app.console_command(
            "halo_vehicle_enter",
            "Seat the local player in the nearest vehicle's empty driver seat; <name>: of that vehicle (screenshots)",
            |w, args| match enter_nearest(w, args.first().map(String::as_str)) {
                Ok(m) => Ok(Some(m)),
                // Not yet (a launch's `+halo_vehicle_enter` runs before
                // the map loads): retried each tick until it works.
                Err(_) => {
                    w.insert_resource(EnterWhenReady(args.first().cloned()));
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
        use chimera_net::net::parts::NetAppExt;
        app.add_systems(Update, look::pose);
        app.init_resource::<VehicleDefs>()
            .net_part("halo_ce_vehicles", |app| {
                use bevy_replicon::prelude::AppRuleExt;
                app.replicate::<NetSeat>();
            })
            .add_observer(round_restart)
            .add_systems(
                FixedUpdate,
                (
                    attach,
                    tidy,
                    seat_from_server.run_if(on_client),
                    (seat_input, simulate, splatter, rider_damage, carry, camera::view)
                        .chain()
                        .run_if(authoritative),
                    look::track,
                    enter_when_ready.run_if(resource_exists::<EnterWhenReady>),
                )
                    .chain()
                    .after(SimSet::Movement)
                    .before(SimSet::Weapons),
            )
            .add_systems(
                Update,
                (follow_drawn, ride_drawn)
                    .chain()
                    .run_if(on_client)
                    .before(crate::map::attached::EyePlaced),
            );
    }
}

fn on_client(role: Option<Res<NetRole>>) -> bool {
    role.is_some_and(|r| *r == NetRole::Client)
}

/// `halo_vehicle_throttle`: every driver's forward throttle held.
#[derive(Resource, Clone, Copy, Debug)]
pub struct HeldThrottle(pub f32);

/// `halo_vehicle_enter` waiting for a local player and a vehicle (of
/// that name).
#[derive(Resource, Clone, Debug)]
pub struct EnterWhenReady(pub Option<String>);

fn enter_when_ready(
    players: Query<(Entity, &Transform), (With<crate::core::LocalPlayer>, Without<Seated>)>,
    mut vehicles: Query<(Entity, &mut HaloVehicle, Option<&PropIndex>)>,
    defs: Res<VehicleDefs>,
    wanted: Res<EnterWhenReady>,
    mut commands: Commands,
) {
    let Some((p, at)) = players.iter().next() else { return };
    let best = vehicles
        .iter()
        .filter(|(_, v, _)| wanted.0.as_ref().is_none_or(|n| *n == defs.0[v.def].0.name))
        .filter_map(|(e, v, _)| {
            let seat = defs.0[v.def].0.driver_seat().filter(|&s| v.riders[s].is_none())?;
            Some(((to_engine(v.drive.body.position) - at.translation).length(), e, seat))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0));
    let Some((_, ve, seat)) = best else { return };
    if let Ok((_, mut v, prop)) = vehicles.get_mut(ve) {
        v.riders[seat] = Some(p);
        sit(&mut commands, p, ve, seat, prop);
        commands.remove_resource::<EnterWhenReady>();
    }
}

/// The local player into the nearest vehicle's empty driver seat.
fn enter_nearest(w: &mut World, name: Option<&str>) -> Result<String, String> {
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
        .filter(|(_, v)| name.is_none_or(|n| n == defs.0[v.def].0.name))
        .filter_map(|(e, v)| {
            let d = &defs.0[v.def].0;
            let seat = d.driver_seat().filter(|&s| v.riders[s].is_none())?;
            Some(((to_engine(v.drive.body.position) - at).length(), e, seat))
        })
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, e, s)| (e, s))
        .ok_or("no vehicle with an empty driver seat")?;
    w.get_mut::<HaloVehicle>(ve).ok_or("gone")?.riders[seat] = Some(p);
    let prop = w.get::<PropIndex>(ve).copied();
    let mut commands = w.commands();
    sit(&mut commands, p, ve, seat, prop.as_ref());
    w.flush();
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
        ec.insert((
            vehicle,
            look::VehicleLook::new(p.suspension.clone()),
            t,
            RigidBody::Kinematic,
            collider,
            contact_filter::hooks(),
        ));
        if server {
            ec.insert(PhysicsProp {
                push: PushAway::Collide,
                mass: phys.mass,
                bounds: (Vec3::splat(-1.0), Vec3::splat(1.0)),
            });
        }
    }
}

/// Spawn the vehicle `name` as placed at `position` (its centre of mass,
/// WU, Halo axes) facing `heading` (radians), raised as created: a vehicle
/// without a map (tests, a game's own spawns). Its collider is a box
/// around its mass points (no collision model). Returns the entity.
pub fn spawn_vehicle(world: &mut World, name: &str, position: Vec3, heading: f32) -> Option<Entity> {
    let defs = world.get_resource_or_init::<VehicleDefs>().clone();
    let def = defs.0.iter().position(|(d, _)| d.name == name)?;
    let (d, phys) = &defs.0[def];
    let placed = Placed {
        prop: usize::MAX,
        vehicle: name.to_string(),
        position: position.to_array(),
        forward: [heading.cos(), heading.sin(), 0.0],
        up: [0.0, 0.0, 1.0],
        markers: Default::default(),
        hull: Vec::new(),
        suspension: Vec::new(),
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
            .spawn((
                v,
                look::VehicleLook::default(),
                t,
                RigidBody::Kinematic,
                collider,
                contact_filter::hooks(),
            ))
            .id(),
    )
}

/// A placed vehicle's starting state: its centre of mass at the placement
/// raised by half its bounding radius (vehicles.md "Units and
/// conventions").
pub fn spawn_state(d: &VehicleDef, phys: &PhysicsDef, def: usize, p: &Placed) -> HaloVehicle {
    let mut body = Body::at(Vec3::ZERO, 0.0);
    body.forward = Vec3::from(p.forward).normalize_or(Vec3::X);
    body.up = Vec3::from(p.up).normalize_or(Vec3::Z);
    // The placement's position is the centre of mass, raised by half the
    // bounding radius.
    body.position = Vec3::from(p.position) + Vec3::Z * (d.bounding_radius / 2.0);
    let _ = phys;
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
            (Has<Dead>, Option<&crate::core::Health>),
        ),
        With<MovementSlot>,
    >,
    props: Query<&PropIndex>,
    defs: Res<VehicleDefs>,
    clock: Res<SimClock>,
    mut commands: Commands,
    // The use key's state last tick, per character (edge detection): kept
    // here, not on the characters (no part put on every character of
    // every game: a party's archetypes stay as they are).
    mut held: Local<BTreeMap<Entity, bool>>,
) {
    let s = crate::game::METERS_PER_UNIT;
    let now = clock.now;
    if vehicles.is_empty() {
        held.clear();
        return;
    }
    held.retain(|e, _| people.contains(*e));
    for (e, intent, t, vel, seated, (dead, health)) in &mut people {
        // Dead by the rules' mark or by its health (a game whose rules
        // mark it later).
        let dead = dead || health.is_some_and(|h| h.current <= 0.0);
        let was = held.insert(e, intent.use_key).unwrap_or(false);
        let pressed = intent.use_key && !was;
        if let Some(seat) = seated.copied() {
            let Ok((ve, mut v)) = vehicles.get_mut(seat.vehicle) else {
                stand(&mut commands, e);
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
                sit(&mut commands, e, ve, seat, props.get(ve).ok());
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
    stand(commands, e);
    commands.entity(e).insert(LastVehicle { vehicle: ve, at: now });
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
    characters: Query<(), With<Intent>>,
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
    // Other vehicles and characters are never ground to a vehicle (they
    // meet it through the pair rule and the splatter rule).
    let all: Vec<Entity> = vehicles.iter().map(|(e, ..)| e).collect();
    let skip = |e: Entity| characters.contains(e) || all.contains(&e);
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
    // Step 7: vehicle against vehicle, the pair rule, into each one's
    // accumulators for its next step.
    let mut pairs = vehicles.iter_combinations_mut::<2>();
    while let Some([(_, mut a, _), (_, mut b, _)]) = pairs.fetch_next() {
        let ((da, pa), (db, pb)) = (&defs.0[a.def], &defs.0[b.def]);
        let reach = da.bounding_radius + db.bounding_radius + 1.0;
        if (a.drive.body.position - b.drive.body.position).length() > reach {
            continue;
        }
        if let Some([(fa, ta), (fb, tb)]) = physics::pair_contact(&a.drive.body, pa, &b.drive.body, pb) {
            a.drive.collision_force += fa;
            a.drive.collision_torque += ta;
            b.drive.collision_force += fb;
            b.drive.collision_torque += tb;
            a.drive.wake();
            b.drive.wake();
        }
    }
}

/// A character into a seat: `Seated` with the engine's `Carried` (its
/// movement suspended, whatever game's it is) and, for a map's vehicle, the
/// replicated `NetSeat`, in one move; counted a change of the server's
/// (`core::rebase`), so the client predicting it takes the new state as a
/// restart, not an error.
pub fn sit(commands: &mut Commands, e: Entity, vehicle: Entity, seat: usize, prop: Option<&PropIndex>) {
    let seated = (Seated { vehicle, seat }, Carried(vehicle));
    match prop {
        Some(p) => commands.entity(e).insert((
            seated,
            NetSeat {
                prop: p.0 as u32,
                seat: seat as u8,
            },
        )),
        None => commands.entity(e).insert(seated),
    };
    commands.queue(move |w: &mut World| crate::core::rebase(w, e));
}

/// A character out of its seat (`sit` undone, in one move).
pub fn stand(commands: &mut Commands, e: Entity) {
    if let Ok(mut ec) = commands.get_entity(e) {
        ec.try_remove::<(Seated, Carried, NetSeat)>();
    }
    commands.queue(move |w: &mut World| crate::core::rebase(w, e));
}

/// Where a rider sits: its centre (engine metres) upright over its seat's
/// marker, for a body whose centre is `above` metres over its feet.
fn ride(v: &HaloVehicle, d: &VehicleDef, phys: &PhysicsDef, seat: usize, above: f32) -> Vec3 {
    to_engine(v.world(phys, v.seat_marker(d, seat))) + Vec3::Y * above
}

/// How far a rider's centre is over its feet (m): Halo's biped standing,
/// or another game's character by its collider's height.
fn centre_above(halo: bool, collider: Option<&Collider>, data: &MovementData) -> f32 {
    match collider {
        Some(c) if !halo => c.aabb(Vec3::ZERO, Quat::IDENTITY, 0.0).size().y / 2.0,
        _ => data.centre_above_origin(0.0) * crate::game::METERS_PER_UNIT,
    }
}

/// A rider's `MapView` made `want` (None: none).
fn set_view(commands: &mut Commands, e: Entity, want: Option<MapView>, have: Option<&MapView>) {
    match (want, have) {
        (Some(w), Some(h)) if *h == w => {}
        (Some(w), _) => {
            commands.entity(e).insert(w);
        }
        (None, Some(_)) => {
            commands.entity(e).remove::<MapView>();
        }
        (None, None) => {}
    }
}

/// Riders follow their seats (authoritative; their camera:
/// `camera::view`; a network client draws its own player with
/// `ride_drawn`, the others as the server sends them).
#[allow(clippy::type_complexity)]
fn carry(
    vehicles: Query<&HaloVehicle>,
    mut riders: Query<
        (
            &Seated,
            &mut Transform,
            &mut Velocity,
            Option<&mut Position>,
            (Has<HaloMovement>, Option<&Collider>),
        ),
        Without<HaloVehicle>,
    >,
    defs: Res<VehicleDefs>,
    data: Res<MovementData>,
    mut commands: Commands,
    mut gone: RemovedComponents<Seated>,
) {
    for e in gone.read() {
        if let Ok(mut ec) = commands.get_entity(e) {
            ec.try_remove::<MapView>();
        }
    }
    for (seat, mut t, mut vel, pos, (halo, collider)) in &mut riders {
        let Ok(v) = vehicles.get(seat.vehicle) else { continue };
        let (d, phys) = &defs.0[v.def];
        let centre = ride(v, d, phys, seat.seat, centre_above(halo, collider, &data));
        t.translation = centre;
        if let Some(mut p) = pos {
            p.0 = centre;
        }
        vel.0 = to_engine(v.drive.body.velocity) * 30.0;
    }
}

/// No stale seat (every role): a vehicle's rider that is gone or no longer
/// sits there (a disconnect, a death that took the character away, a map
/// change) leaves its seat empty, and a character seated in a vehicle that
/// is gone (a map change) stands.
fn tidy(
    mut vehicles: Query<(Entity, &mut HaloVehicle)>,
    seated: Query<(Entity, &Seated)>,
    role: Option<Res<NetRole>>,
    mut commands: Commands,
) {
    for (ve, mut v) in &mut vehicles {
        let stale: Vec<usize> = v
            .riders
            .iter()
            .enumerate()
            .filter(|(i, r)| r.is_some_and(|e| !seated.get(e).is_ok_and(|(_, s)| s.vehicle == ve && s.seat == *i)))
            .map(|(i, _)| i)
            .collect();
        for i in stale {
            v.riders[i] = None;
        }
    }
    let client = role.is_some_and(|r| *r == NetRole::Client);
    for (e, s) in &seated {
        if vehicles.get(s.vehicle).is_err() {
            if client {
                commands.entity(e).try_remove::<(Seated, Carried)>();
            } else {
                stand(&mut commands, e);
            }
        }
    }
}

/// A network client seats its copies of the characters as the server's
/// `NetSeat` says (its own player too: its movement suspended).
#[allow(clippy::type_complexity)]
fn seat_from_server(
    people: Query<(Entity, Option<&NetSeat>, Option<&Seated>), Or<(With<NetSeat>, With<Seated>)>>,
    mut vehicles: Query<(Entity, &PropIndex, &mut HaloVehicle)>,
    mut commands: Commands,
) {
    for (e, net, seated) in &people {
        let want = net.and_then(|n| {
            vehicles
                .iter()
                .find(|(_, p, _)| p.0 == n.prop as usize)
                .map(|(ve, ..)| Seated {
                    vehicle: ve,
                    seat: n.seat as usize,
                })
        });
        if want.as_ref() == seated {
            continue;
        }
        if let Some(old) = seated
            && let Ok((_, _, mut v)) = vehicles.get_mut(old.vehicle)
            && v.riders.get(old.seat).copied().flatten() == Some(e)
        {
            v.riders[old.seat] = None;
        }
        match want {
            Some(s) => {
                if let Ok((_, _, mut v)) = vehicles.get_mut(s.vehicle)
                    && s.seat < v.riders.len()
                {
                    v.riders[s.seat] = Some(e);
                }
                commands.entity(e).insert((s, Carried(s.vehicle)));
            }
            None => {
                commands.entity(e).try_remove::<(Seated, Carried, MapView)>();
            }
        }
    }
}

/// A network client's vehicles: their state is where they are drawn (the
/// server's snapshots, `chimera_net`'s props), so seats and cameras are
/// worked out from what the player sees.
fn follow_drawn(mut vehicles: Query<(&mut HaloVehicle, &Transform)>, defs: Res<VehicleDefs>) {
    for (mut v, t) in &mut vehicles {
        let phys = &defs.0[v.def].1;
        let mut b = v.drive.body;
        pose_from(&mut b, t, phys.center_of_mass);
        if b.position != v.drive.body.position || b.forward != v.drive.body.forward || b.up != v.drive.body.up {
            v.drive.body = b;
        }
    }
}

/// A body posed where its prop's `Transform` (the model origin, engine
/// space) puts it (`look::body_from`), its motion kept.
pub fn pose_from(b: &mut Body, t: &Transform, center_of_mass: Vec3) {
    let drawn = look::body_from(t, center_of_mass);
    b.forward = drawn.forward;
    b.up = drawn.up;
    b.position = drawn.position;
}

/// A network client's own seated player, each frame: in its seat of the
/// vehicle as drawn, viewing through the seat's camera (`camera::seat_view`)
/// at its own look (not predicted: the vehicle is the server's; module doc
/// "Network").
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn ride_drawn(
    vehicles: Query<&HaloVehicle>,
    mut me: Query<
        (Entity, &Seated, &Intent, &mut Transform, Option<&MapView>, Has<HaloMovement>, Option<&Collider>),
        (With<LocalPlayer>, Without<HaloVehicle>),
    >,
    characters: Query<(), With<Intent>>,
    defs: Res<VehicleDefs>,
    data: Res<MovementData>,
    query: SpatialQuery,
    mut commands: Commands,
) {
    for (e, seat, intent, mut t, view, halo, collider) in &mut me {
        let Ok(v) = vehicles.get(seat.vehicle) else { continue };
        let (d, phys) = &defs.0[v.def];
        t.translation = ride(v, d, phys, seat.seat, centre_above(halo, collider, &data));
        let skip = |x: Entity| characters.contains(x) || vehicles.contains(x);
        let want = camera::seat_view(v, d, phys, seat.seat, intent, &query, &skip);
        set_view(&mut commands, e, want, view);
    }
}

/// A round restart (`core::RoundRestart`) on the authority: every rider
/// stands and every map vehicle is back at its placement, at rest.
#[allow(clippy::type_complexity)]
fn round_restart(
    _restart: On<crate::core::RoundRestart>,
    mut vehicles: Query<(&mut HaloVehicle, &PropIndex, &mut Transform)>,
    seated: Query<Entity, With<Seated>>,
    entities: Option<Res<MapEntities>>,
    defs: Res<VehicleDefs>,
    role: Option<Res<NetRole>>,
    mut commands: Commands,
) {
    if role.is_some_and(|r| *r == NetRole::Client) {
        return;
    }
    for e in &seated {
        stand(&mut commands, e);
    }
    let placed: Vec<Placed> = entities
        .map(|m| m.entities.iter().filter_map(Placed::from_entity).collect())
        .unwrap_or_default();
    for (mut v, index, mut t) in &mut vehicles {
        let Some(p) = placed.iter().find(|p| p.prop == index.0) else { continue };
        let def = v.def;
        let (d, phys) = &defs.0[def];
        *v = spawn_state(d, phys, def, p);
        t.translation = to_engine(v.drive.body.origin(phys.center_of_mass));
        t.rotation = engine_rotation(&v.drive.body);
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
            Option<&mut HaloMovement>,
            Option<&LastVehicle>,
        ),
        (With<Intent>, Without<Seated>, Without<Dead>, Without<HaloVehicle>),
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
            super::movement::push(me.as_deref_mut(), &mut pvel, to_engine(dv / 2.0) * 30.0);
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
        // Area damage reaches riders, if at all, through their own bodies.
        if hit.flags & super::damage::hit_flags::AREA != 0 {
            continue;
        }
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
