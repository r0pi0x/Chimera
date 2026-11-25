//! GoldSrc's map logic in a network game, on Chimera's shared mover
//! replication (`net::movers`, the network part `map_movers`, which
//! Source's logic uses too): clients run no GoldSrc logic; what it does to
//! brush entities and sprites reaches them here.
//!
//! - **Server** (`write`, after each tick): every logic node's state into
//!   its `net::movers::NetMover`: origin, angles, velocities, the mover's
//!   own clock, when its move or wait ends (`Motion::next`) and, for a
//!   move, its goal; shown, solid, shootable, or gone (killed, broken).
//!   Its look (`map::BrushTint`: render colour, alpha, mode and the `+a`
//!   frame) replicates beside it, as the bridge writes it. Each
//!   `env_sprite`'s on/off goes as a `NetEntityPart` on an entity kept
//!   for it. The light styles the logic changed (a switchable light's
//!   pattern, `a` while dark) go as `NetLightStyles`; the presets 0-12
//!   every client has from the map (`content::lightmap`).
//! - **Client.** GoldSrc runs the players' movement first and the movers
//!   after (specs/goldsrc/entities.md, "Per-tick order"), so in each
//!   predicted and replayed tick the client places every node where the
//!   server had it for that tick's movement (`place`, in
//!   `core::Predict::PlaceMovers`: the mover stepped from its snapshots
//!   to the tick before, `net::movers::stepped`), then after movement
//!   steps it through the tick, carrying or shoving its own player as the
//!   logic does (`carry`, in `core::Predict::CarryRiders`: the same
//!   `movers::push_player`, in entity order), and places the nodes there.
//!   A broken breakable or a passable door is not solid on the client as
//!   on the server; a named `func_water` moves.
//!
//! Known shortcuts:
//! - What the client can't know ahead (a mover starting, reversing after
//!   its wait, blocked by someone else, a train turning at a corner, a
//!   pendulum's swing, a move whose end is more than 1/32 unit off its
//!   goal) mispredicts for a round trip and is corrected, as with
//!   Source's movers.
//! - Shots on the client test its own copies of the nodes (cosmetic: the
//!   server's hits count); `core::Damageable` isn't replicated.
//! - A client plays the replicated light styles by its own clock, not the
//!   server's: a flicker's phase differs between players (the original's
//!   clients do the same, entities.md "light styles").

use std::sync::Arc;

use bevy::prelude::*;

use super::{
    bridge::{CharacterQuery, GoldSrcLogic, WorldSolid, place_node, player_of, write_back},
    movers::{Step, push_player},
    world::{Kind, Volume, volumes_of},
};
use crate::{
    core::{MapBrushes, NetRole, Predict, SimClock},
    map::{MapEntities, MapEntity, entities::entity_to_engine},
    net::{
        NetAppExt,
        movers::{self, NetEntityPart, NetMover, SteppedMover, mover_flags},
        predict::{NetGraph, predicted_player},
    },
};

/// GoldSrc's logic in the network (`GoldSrcLogicPlugin` adds it): the
/// shared `map_movers` part and this one, `goldsrc_logic` (no protocol of
/// its own: systems only).
pub(super) fn plugin(app: &mut App) {
    app.net_part("map_movers", movers::register).net_part("goldsrc_logic", register);
}

fn register(app: &mut App) {
    let client = || resource_equals(NetRole::Client);
    app.init_resource::<ClientMovers>()
        .add_systems(FixedLast, write.run_if(resource_equals(NetRole::Server)))
        .add_systems(Predict::PlaceMovers, place.after(crate::core::PushFooting).run_if(client()))
        .add_systems(Predict::CarryRiders, carry.run_if(client()))
        .add_systems(PreUpdate, drop_logic.run_if(client()));
}

/// A client runs no GoldSrc logic: one built before it joined (as
/// standalone) goes.
fn drop_logic(logic: Option<Res<GoldSrcLogic>>, mut commands: Commands) {
    if logic.is_some() {
        commands.remove_resource::<GoldSrcLogic>();
    }
}

/// The server's sprite holders, for the logic they were made for.
#[derive(Resource)]
struct SpriteHolders {
    source: Arc<Vec<MapEntity>>,
    holders: Vec<(usize, Entity)>,
}

/// The server: each node's state into its `NetMover`, each sprite's into
/// its `NetEntityPart`.
fn write(world: &mut World) {
    let Some(l) = world.get_resource::<GoldSrcLogic>() else {
        if let Some(h) = world.remove_resource::<SpriteHolders>() {
            for (_, e) in h.holders {
                world.despawn(e);
            }
        }
        movers::write_light_styles(world, Vec::new());
        return;
    };
    let mut out: Vec<(Entity, NetMover)> = Vec::new();
    let mut live: Vec<Entity> = Vec::new();
    for &(id, node) in &l.nodes {
        let e = &l.world.ents[id];
        if e.removed {
            continue;
        }
        let m = &e.motion;
        let moving = m.velocity != Vec3::ZERO || m.avelocity != Vec3::ZERO;
        let damageable = match &e.kind {
            Kind::Breakable(_) | Kind::Pushable(Some(_)) => e.flags & 1 == 0,
            Kind::Button(b) => b.shootable,
            _ => false,
        };
        let mut flags = 0;
        for (on, bit) in [
            (e.visible, mover_flags::VISIBLE),
            (e.solid, mover_flags::SOLID),
            (damageable, mover_flags::SHOOTABLE),
        ] {
            if on {
                flags |= bit;
            }
        }
        let goal = (m.next.is_some() && m.step == Step::MoveDone).then_some(m.goal.to_array());
        live.push(node);
        out.push((
            node,
            NetMover {
                index: id as u32,
                origin: e.origin.to_array(),
                angles: e.angles.to_array(),
                velocity: m.velocity.to_array(),
                avelocity: m.avelocity.to_array(),
                // An idle mover's clock only matters once it moves again
                // (then sent with the move): kept still, so an idle node
                // sends nothing.
                ltime: if moving || m.next.is_some() { m.ltime } else { 0.0 },
                move_done: m.next,
                goal_origin: goal.filter(|_| !m.turn),
                goal_angles: goal.filter(|_| m.turn),
                flags,
            },
        ));
    }
    let styles: Vec<(u8, String)> = l.world.styles.iter().map(|(n, p)| (*n, p.clone())).collect();
    let source = l.source();
    let sprites: Vec<(usize, bool)> = l
        .world
        .ents
        .iter()
        .enumerate()
        .filter_map(|(i, e)| match e.kind {
            Kind::Sprite { on } => Some((i, on && !e.removed)),
            _ => None,
        })
        .collect();
    let exists: Vec<bool> = sprites.iter().map(|(i, _)| !l.world.ents[*i].removed).collect();
    movers::write_light_styles(world, styles);
    for (node, m) in out {
        movers::write_mover(world, node, m);
    }
    movers::mark_gone(world, &live);
    // The sprites' holders, made once per logic.
    if world.get_resource::<SpriteHolders>().is_none_or(|h| !Arc::ptr_eq(&h.source, &source)) {
        if let Some(h) = world.remove_resource::<SpriteHolders>() {
            for (_, e) in h.holders {
                world.despawn(e);
            }
        }
        let holders = sprites.iter().map(|(i, _)| (*i, world.spawn(Name::new("goldsrc sprite")).id())).collect();
        world.insert_resource(SpriteHolders { source, holders });
    }
    let holders = world.resource::<SpriteHolders>().holders.clone();
    for ((i, holder), ((_, on), exists)) in holders.into_iter().zip(sprites.into_iter().zip(exists)) {
        movers::write_part(world, holder, NetEntityPart { entity: i as u32, on, exists });
    }
}

/// A client's movers this tick (`place` to `carry`) and each map entity's
/// volumes, for the map they were read from.
#[derive(Resource, Default)]
struct ClientMovers {
    source: Option<Arc<Vec<MapEntity>>>,
    volumes: Vec<Vec<Volume>>,
    stepped: Vec<SteppedMover>,
}

/// The tick's length as the server's logic steps (`bridge::step`).
fn tick_dt(world: &World) -> f64 {
    world.resource::<SimClock>().delta.as_secs_f64()
}

/// The map's entities if they are GoldSrc's, with the volumes read.
fn goldsrc_map(world: &mut World) -> Option<f32> {
    let m = world.get_resource::<MapEntities>()?;
    if crate::map::entities::logic_family(&m.entities) != Some(super::FAMILY) || m.scale <= 0.0 {
        return None;
    }
    let (entities, scale) = (m.entities.clone(), m.scale);
    let mut c = world.resource_mut::<ClientMovers>();
    if c.source.as_ref().is_none_or(|s| !Arc::ptr_eq(s, &entities)) {
        c.volumes = entities.iter().map(volumes_of).collect();
        c.source = Some(entities);
    }
    Some(scale)
}

/// Place each node per `which` (its state before or after the tick).
fn place_all(world: &mut World, stepped: &[SteppedMover], after: bool, scale: f32) {
    let c = world.remove_resource::<ClientMovers>().unwrap_or_default();
    for p in stepped {
        let s = if after { &p.after } else { &p.before };
        let Some(volumes) = c.volumes.get(p.index as usize) else { continue };
        let gone = s.has(mover_flags::GONE);
        let shown = s.has(mover_flags::VISIBLE) && !gone;
        let solid = s.has(mover_flags::SOLID) && !gone;
        let velocity = entity_to_engine(s.velocity, scale);
        let moved = world.get::<Transform>(p.node).is_none_or(|t| {
            t.translation != entity_to_engine(s.origin, scale)
                || t.rotation
                    != crate::map::entities::rotation_to_engine(crate::map::entities::entity_rotation(s.angles))
        });
        place_node(world, p.node, (s.origin, s.angles), shown, solid, volumes, velocity, scale, moved);
    }
    world.insert_resource(c);
    // Spatial queries find the nodes where they now are.
    crate::core::refresh_moved_colliders(world);
}

/// A client before movement in each predicted and replayed tick: the
/// nodes where the server's movement met them this tick.
fn place(world: &mut World) {
    let Some(scale) = goldsrc_map(world) else { return };
    let dt = tick_dt(world);
    let Some(stepped) = movers::stepped(world, dt) else { return };
    place_all(world, &stepped, false, scale);
    world.resource_mut::<NetGraph>().movers = stepped.len();
    world.resource_mut::<ClientMovers>().stepped = stepped;
}

/// A client after movement in each predicted and replayed tick: the
/// movers' step carrying or shoving its own player as the server's logic
/// does (a step the player would block is left out: the server's mover
/// stops or reverses, the next correction says how), then the nodes
/// where the step left them.
fn carry(world: &mut World) {
    let Some(scale) = goldsrc_map(world) else { return };
    let stepped = std::mem::take(&mut world.resource_mut::<ClientMovers>().stepped);
    if stepped.is_empty() {
        return;
    }
    if let Some(player) = predicted_player(world) {
        let mut q = world.query::<CharacterQuery>();
        let before = q.get(world, player).ok().map(|row| player_of(row, scale));
        if let Some(before) = before {
            let mut p = before.clone();
            {
                let c = world.resource::<ClientMovers>();
                let empty = Vec::new();
                let brushes = world.get_resource::<MapBrushes>().map_or(&empty, |b| &b.0);
                let solid = WorldSolid { brushes, scale };
                for m in &stepped {
                    let s = &m.before;
                    if !s.has(mover_flags::SOLID) || s.has(mover_flags::GONE) || (m.d == Vec3::ZERO && m.da == Vec3::ZERO)
                    {
                        continue;
                    }
                    let Some(volumes) = c.volumes.get(m.index as usize) else { continue };
                    let (o0, a0) = (s.origin, s.angles);
                    let (o1, a1) = (o0 + m.d, a0 + m.da);
                    let at = |o: Vec3, a: Vec3| -> Vec<crate::core::MapBrush> {
                        let rot = crate::map::entities::entity_rotation(a);
                        volumes.iter().map(|v| super::world::place(v, rot, o)).collect()
                    };
                    let r = push_player(&p, &at(o0, a0), &at(o1, a1), (o0, a0), (o1, a1), &solid);
                    if let Ok(Some(to)) = r {
                        p.origin = to;
                        p.moved = true;
                    }
                }
            }
            if p.moved {
                write_back(world, scale, std::slice::from_ref(&p));
                world.resource_mut::<NetGraph>().carried += 1;
            }
        }
    }
    place_all(world, &stepped, true, scale);
    movers::prune(world, world.resource::<SimClock>().tick);
}
