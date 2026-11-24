//! Source's map logic in a network game: its part of the network
//! (`net::NetAppExt::net_part`, named `source_logic`, registered by
//! `LogicPlugin`, with the shared `map_movers`): moving brushes
//! (`NetMover`, `Stepper` and the client's stepping are Chimera's
//! `net::movers`, shared with GoldSrc's logic) and what the logic shows
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

pub use crate::net::movers::{NetMover, Stepper, mover_flags};
use crate::{
    core::{MovingSolid, NetRole, Predict, SimClock},
    logic::{
        CarriedMover, HudEvent, HudShow, Logic, brush_to_engine, carry_player, classes::Class, movers::pusher,
        mover_brushes_at,
    },
    map::{
        MapBrushEntity, MapEntities,
        entities::{entity_rotation, entity_to_engine, rotation_to_engine},
    },
    net::{
        NetAppExt, movers,
        predict::{NetGraph, predicted_player},
        server::Player,
    },
};

/// The logic's part of the network (`register`), named `source_logic`
/// (`LogicPlugin` adds it).
pub(super) fn plugin(app: &mut App) {
    app.net_part("source_logic", register).net_part("map_movers", movers::register);
}

/// The protocol (in this order on every side), the systems and the hooks.
fn register(app: &mut App) {
    let client = || resource_equals(NetRole::Client);
    // The movers' protocol is the shared `map_movers` part (`net::movers`).
    app.add_server_message::<MapHud>(Channel::Ordered)
        .add_message::<HudEvent>()
        .add_systems(FixedLast, write_movers.run_if(resource_equals(NetRole::Server)))
        // Each predicted and each replayed tick (`core::Predict`).
        .add_systems(Predict::PlaceMovers, place.after(crate::core::PushFooting).run_if(client()))
        .add_systems(
            PostUpdate,
            forward_hud.before(ServerSystems::Send).run_if(resource_equals(NetRole::Server)),
        )
        .add_systems(PreUpdate, receive_hud.after(ClientSystems::Receive).run_if(client()));
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
    for (node, m) in out {
        movers::write_mover(world, node, m);
    }
    // Gone (killed, broken) until a round restart.
    movers::mark_gone(world, &live);
}

/// A client's movers at the tick it runs (`SimClock::tick`): stepped
/// from their snapshots, its own player carried as the server's logic
/// does, then the nodes placed (transform, brushes for movement, shown,
/// solid). Runs before movement in each predicted tick and each replayed
/// one (`core::Predict::PlaceMovers`).
pub fn place(world: &mut World) {
    let Some(entities) = world.get_resource::<MapEntities>().cloned() else {
        return;
    };
    let clock = *world.resource::<SimClock>();
    let tick = clock.tick;
    // The logic's tick length (an f32 there), as it steps.
    let dt = clock.delta.as_secs_f32() as f64;
    let Some(placed) = movers::stepped(world, dt) else {
        return;
    };
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
    movers::prune(world, tick);
}

