//! Source's map logic in a network game: its part of the network
//! (`net::NetAppExt::net_part`, named `source_logic`, registered by
//! `LogicPlugin`): moving brushes (`NetMover`) and what the logic shows
//! on HUDs (`MapHud`). Clients run no map logic; what it does reaches them
//! through the map's state (`map::state`: render looks, tints, one-shot
//! sounds, `say` lines, effects: the network's own) or here.
//!
//! Moving brushes (docs/plans/active/multiplayer.md, slice 3): doors,
//! platforms, trains, rotating and parented brushes.
//!
//! - **Server.** After each tick, every mover node's motion state goes
//!   into its `NetMover` (`write_movers`): the logic's pusher (where it
//!   is, its velocity and angular velocity, its local time, when its move
//!   ends and where), and whether it is shown, solid or gone. Only changes
//!   are sent.
//! - **Client.** A client runs its own player ahead of the server (the
//!   tick it predicts, `predict`), and the server ran that tick with the
//!   movers where they were then. So each predicted tick (and each
//!   replayed one) the client places every mover node at that tick
//!   (`place`): from the newest snapshot at or before it, stepped forward
//!   the way the logic steps a pusher (`Stepper`, the same arithmetic in
//!   the same order, so the result is the server's bit for bit while the
//!   mover keeps doing what it did); then pushes its own player with
//!   those steps as the logic does before movement
//!   (`logic::carry_player`: riders carried, players in the way shoved).
//!   Its movement then sweeps the movers' brushes where the server's did.
//!   What the client can't know ahead (a door starting to open, a train
//!   turning at a corner, a mover blocked by someone else, reversing at
//!   the end of its wait) shows up as a prediction error when the
//!   server's state arrives, corrected and eased like any other.
//! - Movers are drawn at the predicted tick too (eased between ticks as
//!   in single player, `map::interp`), so the client's own player stands
//!   on what it is drawn on; others riding them are drawn in the past
//!   (`interp`), as in Source.
//!
//! Moving brushes: the server's pushers into `NetMover` (`write_movers`); a
//! client steps each from its snapshots to the tick it predicts as the logic
//! does (`Stepper`), carries its own player with those steps
//! (`logic::carry_player`) and places the nodes (`Transform`, `MovingSolid`,
//! shown, collider) before movement, in each predicted and each replayed tick
//! (`place`, in `core::Predict::PlaceMovers`); the network puts
//! the player back on the server's mover after a correction
//! (`net::predict::restore_ground`).
//!
//! HUD messages: what map logic shows on HUDs (`logic::HudEvent`: game_text,
//! env_fade, env_hudhint) as `MapHud` to the player it is for or everyone;
//! a client shows it (`hud::show_on_hud`).

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    core::{MovingSolid, NetRole, Predict, SimClock},
    logic::{
        CarriedMover, HudEvent, HudShow, Logic, brush_to_engine, carry_player, classes::Class, movers::pusher,
        mover_brushes_at,
    },
    map::{
        BrushTint, MapBrushEntity, MapEntities,
        entities::{entity_rotation, entity_to_engine, rotation_to_engine},
    },
    net::{
        NetAppExt, NetHooks,
        interp::{HearTicks, Snapshots, hold_heard, remove_snapshots, write_snapshot},
        ordered::OrderedAppExt,
        predict::{CommandClock, NetGraph, predicted_player},
        server::Player,
    },
};

/// Ticks a client steps a mover past its newest snapshot, at most.
const MAX_STEPS: u64 = 512;

/// The logic's part of the network (`register`), named `source_logic`
/// (`LogicPlugin` adds it).
pub(super) fn plugin(app: &mut App) {
    app.net_part("source_logic", register);
}

/// The protocol (in this order on every side), the systems and the hooks.
fn register(app: &mut App) {
    let client = || resource_equals(NetRole::Client);
    // Every value a client receives also goes into its snapshot buffer, by
    // the server tick it is from.
    app.replicate::<NetMover>()
        .set_receive_fns::<NetMover>(write_snapshot::<NetMover>, remove_snapshots::<NetMover>)
        .ordered_with::<NetMover>(write_snapshot::<NetMover>, remove_snapshots::<NetMover>)
        .add_server_message::<MapHud>(Channel::Ordered)
        .add_message::<HudEvent>()
        .add_systems(FixedLast, write_movers.run_if(resource_equals(NetRole::Server)))
        // Each predicted and each replayed tick (`core::Predict`).
        .add_systems(Predict::PlaceMovers, place.run_if(client()))
        .add_systems(Update, copy_tints.run_if(client()))
        .add_systems(PreUpdate, hold_heard::<NetMover>.after(HearTicks).run_if(client()))
        .add_systems(
            PostUpdate,
            forward_hud.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(PreUpdate, receive_hud.after(ClientSystems::Receive).run_if(client()));
    app.world_mut().resource_mut::<NetHooks>().stopped_serving.push(stopped_serving);
}

/// Serving stopped: movers stop replicating.
fn stopped_serving(world: &mut World) {
    let movers: Vec<Entity> = world.query_filtered::<Entity, With<NetMover>>().iter(world).collect();
    for e in movers {
        world.entity_mut(e).remove::<NetMover>();
    }
}

/// A moving brush's motion state (doors, platforms, trains, rotating and
/// parented brushes; `logic::movers::Pusher`), entity space (angles in
/// degrees), written by the server on the mover's node after each tick
/// (`write_movers`). A client steps its own copy of the node
/// (`map::MapBrushEntity` with this `index`) to the tick it predicts the
/// way the logic does (`place`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetMover {
    /// The map entity index (`map::MapBrushEntity`).
    pub index: u32,
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub velocity: [f32; 3],
    pub avelocity: [f32; 3],
    /// The pusher's local time, its move's (or wait's) end, and where the
    /// move ends.
    pub ltime: f64,
    pub move_done: Option<f64>,
    pub goal_origin: Option<[f32; 3]>,
    pub goal_angles: Option<[f32; 3]>,
    /// `mover_flags::*` bits.
    pub flags: u16,
}

/// `NetMover::flags` bits.
pub mod mover_flags {
    pub const VISIBLE: u16 = 1;
    pub const SOLID: u16 = 1 << 1;
    /// Steps land on whole ticks (func_rotating's spin).
    pub const INEXACT: u16 = 1 << 2;
    /// Angles kept in [0, 360) (func_rotating).
    pub const SPIN: u16 = 1 << 3;
    /// Rotations push by the box's leading corner (model doors).
    pub const PHYSICS_SOLID: u16 = 1 << 4;
    /// Players are moved through (trains flag 512).
    pub const UNBLOCKABLE: u16 = 1 << 5;
    /// Shots hit it (a broken breakable's aren't).
    pub const SHOOTABLE: u16 = 1 << 6;
    /// Its logic entity is gone (killed, broken) until a round restart.
    pub const GONE: u16 = 1 << 7;
    /// Moves with a parent (parented brushes, breakables): not stepped on
    /// its own.
    pub const ATTACHED: u16 = 1 << 8;
}

/// Server -> a client: what map logic shows on its player's HUD (a
/// game_text, an env_fade, an env_hudhint: `logic::HudEvent`), to the
/// player it is for or to everyone.
#[derive(Message, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapHud(pub HudShow);

/// What map logic shows on HUDs (`logic::HudEvent`) to the client of the
/// player it is for, or to every client.
fn forward_hud(
    mut events: MessageReader<HudEvent>,
    players: Query<(Entity, &Player)>,
    mut out: MessageWriter<ToClients<MapHud>>,
) {
    for ev in events.read() {
        let targets = match ev.to {
            None => SendTargets::CLIENTS_ONLY,
            Some(c) => match players.iter().find(|(_, p)| p.character == c) {
                Some((client, _)) => SendTargets::Single(ClientId::Client(client)),
                None => continue,
            },
        };
        out.write(ToClients {
            targets,
            message: MapHud(ev.what.clone()),
        });
    }
}

/// The server's HUD messages for this client's player.
fn receive_hud(mut shows: MessageReader<MapHud>, time: Res<Time>, mut commands: Commands) {
    let now = time.elapsed_secs_f64();
    for s in shows.read() {
        let what = s.0.clone();
        commands.queue(move |w: &mut World| crate::logic::hud::show_on_hud(w, what, now));
    }
}


/// A client: a mover's render colour, alpha, mode and texture frame
/// (`map::BrushTint`, replicated with its `NetMover`) onto its own
/// copy of the node, which draws it.
fn copy_tints(
    movers: Query<(&NetMover, &BrushTint), Changed<BrushTint>>,
    nodes: Query<(Entity, &MapBrushEntity, Option<&BrushTint>), Without<NetMover>>,
    mut commands: Commands,
) {
    for (m, tint) in &movers {
        for (node, index, have) in &nodes {
            if index.0 as u32 == m.index && have != Some(tint) {
                commands.entity(node).insert(*tint);
            }
        }
    }
}

/// The server: each mover node's state into its `NetMover` (replicated
/// from the first one on); a node whose entity is gone says so.
fn write_movers(world: &mut World) {
    let Some(logic) = world.get_resource::<Logic>() else {
        return;
    };
    let mut out: Vec<(Entity, NetMover)> = Vec::new();
    let mut live: Vec<Entity> = Vec::new();
    for (id, node) in &logic.nodes {
        let Some(e) = logic.world.get(*id) else { continue };
        let Some(p) = pusher(&e.class) else { continue };
        let Some(index) = world.get::<MapBrushEntity>(*node).map(|n| n.0 as u32) else {
            continue;
        };
        let mut flags = 0;
        for (on, bit) in [
            (p.visible, mover_flags::VISIBLE),
            (p.solid, mover_flags::SOLID),
            (p.inexact, mover_flags::INEXACT),
            (matches!(e.class, Class::Rotating(_)), mover_flags::SPIN),
            (matches!(e.class, Class::PropDoor(_)), mover_flags::PHYSICS_SOLID),
            (p.unblockable, mover_flags::UNBLOCKABLE),
            (logic.world.shootable(*id), mover_flags::SHOOTABLE),
            (
                matches!(e.class, Class::Attached(_) | Class::Breakable(_)),
                mover_flags::ATTACHED,
            ),
        ] {
            if on {
                flags |= bit;
            }
        }
        live.push(*node);
        out.push((
            *node,
            NetMover {
                index,
                origin: p.origin.to_array(),
                angles: p.angles.to_array(),
                velocity: p.velocity.to_array(),
                avelocity: p.avelocity.to_array(),
                ltime: p.ltime,
                move_done: p.move_done,
                goal_origin: p.goal_origin.map(|g| g.to_array()),
                goal_angles: p.goal_angles.map(|g| g.to_array()),
                flags,
            },
        ));
    }
    // Gone (killed, broken) until a round restart.
    let gone: Vec<(Entity, NetMover)> = world
        .query::<(Entity, &NetMover)>()
        .iter(world)
        .filter(|(e, m)| !live.contains(e) && m.flags & mover_flags::GONE == 0)
        .map(|(e, m)| {
            (
                e,
                NetMover {
                    flags: mover_flags::GONE,
                    velocity: [0.0; 3],
                    avelocity: [0.0; 3],
                    move_done: None,
                    ..m.clone()
                },
            )
        })
        .collect();
    for (node, m) in out.into_iter().chain(gone) {
        let mut e = world.entity_mut(node);
        match e.get_mut::<NetMover>() {
            Some(mut have) => {
                have.set_if_neq(m);
            }
            None => {
                e.insert((Replicated, m));
            }
        }
    }
}

/// A pusher stepped as the logic steps it (`LogicWorld::step_movers` and
/// `settle`), without the logic: what it does once a move or wait ends
/// is the server's to say.
#[derive(Clone, Debug, PartialEq)]
pub struct Stepper {
    pub origin: Vec3,
    pub angles: Vec3,
    pub velocity: Vec3,
    pub avelocity: Vec3,
    pub ltime: f64,
    pub move_done: Option<f64>,
    pub goal_origin: Option<Vec3>,
    pub goal_angles: Option<Vec3>,
    pub flags: u16,
}

impl Stepper {
    pub fn of(m: &NetMover) -> Self {
        Self {
            origin: Vec3::from_array(m.origin),
            angles: Vec3::from_array(m.angles),
            velocity: Vec3::from_array(m.velocity),
            avelocity: Vec3::from_array(m.avelocity),
            ltime: m.ltime,
            move_done: m.move_done,
            goal_origin: m.goal_origin.map(Vec3::from_array),
            goal_angles: m.goal_angles.map(Vec3::from_array),
            flags: m.flags,
        }
    }

    fn has(&self, bit: u16) -> bool {
        self.flags & bit != 0
    }

    /// One tick of `dt` s: how far it moved and turned.
    pub fn step(&mut self, dt: f64) -> (Vec3, Vec3) {
        let moving = self.velocity != Vec3::ZERO || self.avelocity != Vec3::ZERO;
        if self.has(mover_flags::ATTACHED | mover_flags::GONE) || (!moving && self.move_done.is_none()) {
            return (Vec3::ZERO, Vec3::ZERO);
        }
        let step = match self.move_done {
            Some(done) if !self.has(mover_flags::INEXACT) => (done - self.ltime).clamp(0.0, dt),
            _ => dt,
        };
        let d = self.velocity * step as f32;
        let da = self.avelocity * step as f32;
        self.origin += d;
        self.angles += da;
        if self.has(mover_flags::SPIN) {
            self.angles = Vec3::new(
                self.angles.x.rem_euclid(360.0),
                self.angles.y.rem_euclid(360.0),
                self.angles.z.rem_euclid(360.0),
            );
        }
        self.ltime += step;
        if let Some(done) = self.move_done
            && self.ltime >= done - 1e-6
        {
            if let Some(g) = self.goal_origin.take() {
                self.origin = g;
                self.velocity = Vec3::ZERO;
            }
            if let Some(g) = self.goal_angles.take() {
                self.angles = g;
                self.avelocity = Vec3::ZERO;
            }
            self.move_done = None;
        }
        (d, da)
    }
}

/// A mover at a predicted tick: before and after its step, and the step.
struct Placed {
    node: Entity,
    index: u32,
    before: Stepper,
    after: Stepper,
    d: Vec3,
    da: Vec3,
}

/// A client's movers at the tick it runs (`SimClock::tick`): stepped
/// from their snapshots, its own player carried as the server's logic
/// does, then the nodes placed (transform, brushes for movement, shown,
/// solid). Runs before movement in each predicted tick and each replayed
/// one (`core::Predict::PlaceMovers`).
pub fn place(world: &mut World) {
    if world.get_resource::<NetRole>() != Some(&NetRole::Client)
        || world.get_resource::<CommandClock>().is_none_or(|c| c.tick.is_none())
    {
        return;
    }
    let Some(entities) = world.get_resource::<MapEntities>().cloned() else {
        return;
    };
    let clock = *world.resource::<SimClock>();
    let tick = clock.tick;
    // The logic's tick length (an f32 there), as it steps.
    let dt = clock.delta.as_secs_f32() as f64;
    let nodes: std::collections::HashMap<u32, Entity> = world
        .query::<(Entity, &MapBrushEntity)>()
        .iter(world)
        .map(|(e, n)| (n.0 as u32, e))
        .collect();
    let mut placed: Vec<Placed> = Vec::new();
    for buf in world.query::<&Snapshots<NetMover>>().iter(world) {
        let Some((base_tick, base)) = buf.at_or_before(tick.saturating_sub(1)) else {
            continue;
        };
        let Some(node) = nodes.get(&base.index).copied() else {
            continue;
        };
        let mut s = Stepper::of(base);
        let mut t = *base_tick;
        while t + 1 < tick && tick - t <= MAX_STEPS {
            s.step(dt);
            t += 1;
        }
        let before = s.clone();
        let (d, da) = if t < tick { s.step(dt) } else { (Vec3::ZERO, Vec3::ZERO) };
        // A snapshot for this very tick is the server's word for where it
        // ends up (the step's push stays as stepped).
        let after = match buf.at_or_before(tick) {
            Some((t, m)) if *t == tick => Stepper::of(m),
            _ => s,
        };
        placed.push(Placed {
            node,
            index: base.index,
            before,
            after,
            d,
            da,
        });
    }
    placed.sort_by_key(|p| p.index);
    let scale = entities.scale;
    // Our own player, carried and pushed.
    if let Some(player) = predicted_player(world) {
        let carried: Vec<CarriedMover> = placed
            .iter()
            .filter_map(|p| {
                let hulls = &entities.entities.get(p.index as usize)?.hulls;
                Some(CarriedMover {
                    node: p.node,
                    index: p.index,
                    hulls,
                    origin: p.before.origin,
                    angles: p.before.angles,
                    d: p.d,
                    da: p.da,
                    solid: p.before.has(mover_flags::SOLID) && !p.before.has(mover_flags::GONE),
                    physics_solid: p.before.has(mover_flags::PHYSICS_SOLID),
                    unblockable: p.before.has(mover_flags::UNBLOCKABLE),
                })
            })
            .collect();
        if carry_player(world, player, scale, &carried) {
            world.resource_mut::<NetGraph>().carried += 1;
        }
    }
    world.resource_mut::<NetGraph>().movers = placed.len();
    for p in &placed {
        let Some(hulls) = entities.entities.get(p.index as usize).map(|e| &e.hulls) else {
            continue;
        };
        let s = &p.after;
        let gone = s.has(mover_flags::GONE);
        let visible = s.has(mover_flags::VISIBLE) && !gone;
        let solid = s.has(mover_flags::SOLID) && !gone;
        let shootable = s.has(mover_flags::SHOOTABLE) && !gone;
        let transform = Transform::from_translation(entity_to_engine(s.origin, scale))
            .with_rotation(rotation_to_engine(entity_rotation(s.angles)));
        let velocity = entity_to_engine(s.velocity, scale);
        let Ok(mut e) = world.get_entity_mut(p.node) else {
            continue;
        };
        let moved = e.get::<Transform>().is_none_or(|t| *t != transform);
        if moved && let Some(mut t) = e.get_mut::<Transform>() {
            *t = transform;
        }
        super::set_node_shown(&mut e, visible);
        let same = !moved
            && e.get::<MovingSolid>()
                .is_some_and(|m| m.solid == solid && m.velocity == velocity);
        if !same {
            let brushes = if solid {
                mover_brushes_at(hulls, s.origin, s.angles)
                    .iter()
                    .map(|b| brush_to_engine(b, scale))
                    .collect()
            } else {
                Vec::new()
            };
            e.insert(MovingSolid {
                brushes,
                velocity,
                solid,
            });
        }
        // A broken window's collider is its unbroken panes' (`net::breakables`
        // copies the server's; `map::breakables::update_panes`).
        if !e.contains::<crate::map::BrushPanes>() && shootable == e.contains::<avian3d::prelude::ColliderDisabled>() {
            if shootable {
                e.remove::<avian3d::prelude::ColliderDisabled>();
            } else {
                e.insert(avian3d::prelude::ColliderDisabled);
            }
        }
    }
    // Snapshots the replays can no longer need.
    let keep = tick.saturating_sub(crate::net::interp::MAX_SAMPLES as u64) as f64;
    for mut buf in world.query::<&mut Snapshots<NetMover>>().iter_mut(world) {
        buf.prune(keep);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stepper_moves_as_the_logic_and_stops_at_its_goal() {
        // 100 units at 100 units/s from x = 0, 0.015 s ticks.
        let m = NetMover {
            index: 0,
            origin: [0.0; 3],
            velocity: [100.0, 0.0, 0.0],
            ltime: 0.0,
            move_done: Some(1.0),
            goal_origin: Some([100.0, 0.0, 0.0]),
            flags: mover_flags::SOLID | mover_flags::VISIBLE,
            ..default()
        };
        let mut s = Stepper::of(&m);
        let dt = 0.015f32 as f64;
        let mut x = 0.0f32;
        for _ in 0..66 {
            let (d, _) = s.step(dt);
            x += 100.0 * dt as f32;
            assert_eq!(d.x, 100.0 * dt as f32);
            assert_eq!(s.origin.x, x);
        }
        // The last partial step ends exactly at the goal and stops.
        let (d, _) = s.step(dt);
        assert!(d.x > 0.0 && d.x < 1.5, "{}", d.x);
        assert_eq!(s.origin.x, 100.0);
        assert_eq!(s.velocity, Vec3::ZERO);
        assert_eq!(s.step(dt), (Vec3::ZERO, Vec3::ZERO));
    }
}
