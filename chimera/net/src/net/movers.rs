//! Map logic's moving brushes in a network game, for every logic family
//! (Source's `source_logic`, GoldSrc's `goldsrc_engine::logic`): the
//! network part named `map_movers` (`register`; each logic adds it with
//! `NetAppExt::net_part`, so the protocol is the same whichever logics
//! an app runs). Clients run no map logic; what it does to brush entities
//! reaches them here.
//!
//! - **Server.** A logic writes each mover node's motion state into its
//!   `NetMover` after its tick (`write_mover`; replicated from the first
//!   one on, only changes sent): where it is, its velocity and angular
//!   velocity, its own clock, when its move ends and where, and whether
//!   it is shown, solid, shootable or gone (`mark_gone` for nodes whose
//!   entity is gone). Its look (`map::BrushTint`: render colour, alpha,
//!   mode and texture frame) replicates beside it (the network's own
//!   component). A switched part of the map (`map::EntityPart`: a sprite
//!   on or off) goes as a `NetEntityPart` the logic writes on an entity of
//!   its own (`write_part`). Light styles a logic plays (`map::LightStyles`:
//!   each style's pattern letters) go as one `NetLightStyles` on an entity
//!   of its own (`write_light_styles`); a client plays them by its own clock
//!   (`copy_light_styles`, the writer `net` in `LightStyles`).
//! - **Client.** Each predicted and replayed tick a logic's client part
//!   steps every mover from its newest snapshot at or before the tick to
//!   the tick (`stepped`, by `Stepper`: a pusher as both logics step one,
//!   by its velocities up to its move's end, snapped to its goal there)
//!   and places its own copy of the node, carrying its own player the way
//!   its logic does (Source before movement, GoldSrc after). What the
//!   client can't know ahead (a mover starting, reversing, blocked by
//!   someone else) shows up as a prediction error when the server's state
//!   arrives, corrected and eased like any other. `copy_tints` and
//!   `copy_parts` put the server's looks and parts on the client's own
//!   nodes and parts.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::{
    core::{NetRole, SimClock},
    map::{BrushTint, EntityPart, LightStyles, MapBrushEntity, style_levels},
    net::{
        NetHooks,
        interp::{HearTicks, Snapshots, hold_heard, remove_snapshots, write_snapshot},
        ordered::OrderedAppExt,
        predict::CommandClock,
    },
};

/// Ticks a client steps a mover past its newest snapshot, at most.
pub const MAX_STEPS: u64 = 512;

/// The protocol of map logic's movers and parts, the client's copies and
/// the hooks: a logic registers it as the network part `map_movers`
/// (`app.net_part("map_movers", movers::register)`).
pub fn register(app: &mut App) {
    let client = || resource_equals(NetRole::Client);
    // Every value a client receives also goes into its snapshot buffer, by
    // the server tick it is from.
    app.replicate::<NetMover>()
        .set_receive_fns::<NetMover>(write_snapshot::<NetMover>, remove_snapshots::<NetMover>)
        .ordered_with::<NetMover>(write_snapshot::<NetMover>, remove_snapshots::<NetMover>)
        .replicate::<NetEntityPart>()
        .ordered::<NetEntityPart>()
        .replicate::<NetLightStyles>()
        .ordered::<NetLightStyles>()
        .add_systems(Update, (copy_tints, copy_parts, copy_light_styles).run_if(client()))
        .add_systems(PreUpdate, hold_heard::<NetMover>.after(HearTicks).run_if(client()));
    app.world_mut().resource_mut::<NetHooks>().stopped_serving.push(stopped_serving);
}

/// Serving stopped: movers and parts stop replicating.
fn stopped_serving(world: &mut World) {
    let movers: Vec<Entity> = world.query_filtered::<Entity, With<NetMover>>().iter(world).collect();
    for e in movers {
        world.entity_mut(e).remove::<NetMover>();
    }
    let parts: Vec<Entity> = world.query_filtered::<Entity, With<NetEntityPart>>().iter(world).collect();
    for e in parts {
        world.entity_mut(e).remove::<NetEntityPart>();
    }
    let styles: Vec<Entity> = world.query_filtered::<Entity, With<NetLightStyles>>().iter(world).collect();
    for e in styles {
        world.entity_mut(e).remove::<NetLightStyles>();
    }
}

/// A moving brush's motion state (doors, platforms, trains, rotating and
/// parented brushes, walls and breakables), entity space (angles in
/// degrees), written by the server on the mover's node after each tick
/// (`write_mover`). A client steps its own copy of the node
/// (`map::MapBrushEntity` with this `index`) to the tick it predicts.
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
    /// Steps land on whole ticks (Source's func_rotating spin).
    pub const INEXACT: u16 = 1 << 2;
    /// Angles kept in [0, 360) (Source's func_rotating).
    pub const SPIN: u16 = 1 << 3;
    /// Rotations push by the box's leading corner (Source's model doors).
    pub const PHYSICS_SOLID: u16 = 1 << 4;
    /// Players are moved through (Source's trains flag 512).
    pub const UNBLOCKABLE: u16 = 1 << 5;
    /// Shots hit it (a broken breakable's aren't).
    pub const SHOOTABLE: u16 = 1 << 6;
    /// Its logic entity is gone (killed, broken) until a round restart.
    pub const GONE: u16 = 1 << 7;
    /// Moves with a parent (parented brushes, breakables): not stepped on
    /// its own.
    pub const ATTACHED: u16 = 1 << 8;
}

/// A switched part of the map (`map::EntityPart`: a sprite on or off, or
/// killed) as the server's logic has it, on an entity the logic spawns to
/// replicate it (`write_part`). A client puts it on its own part of the
/// same map entity (`copy_parts`).
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NetEntityPart {
    /// Index into `MapData::entities`.
    pub entity: u32,
    pub on: bool,
    pub exists: bool,
}

/// The light styles a logic has changed from the map's own, each style's
/// pattern letters now (`a` dark, `m` normal; one letter is steady), on an
/// entity of its own (`write_light_styles`). A client plays them
/// (`copy_light_styles`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct NetLightStyles {
    pub styles: Vec<(u8, String)>,
}

/// The server: the logic's changed light styles, replicated from the first
/// non-empty list on (only a change is sent).
pub fn write_light_styles(world: &mut World, styles: Vec<(u8, String)>) {
    let holder = world.query_filtered::<Entity, With<NetLightStyles>>().iter(world).next();
    match holder {
        Some(h) => {
            if let Some(mut have) = world.entity_mut(h).get_mut::<NetLightStyles>() {
                have.set_if_neq(NetLightStyles { styles });
            }
        }
        None if !styles.is_empty() => {
            world.spawn((Name::new("light styles"), Replicated, NetLightStyles { styles }));
        }
        None => {}
    }
}

/// The server: `m` on mover node `node` (replicated from the first one on;
/// only a change is sent).
pub fn write_mover(world: &mut World, node: Entity, m: NetMover) {
    let Ok(mut e) = world.get_entity_mut(node) else { return };
    match e.get_mut::<NetMover>() {
        Some(mut have) => {
            have.set_if_neq(m);
        }
        None => {
            e.insert((Replicated, m));
        }
    }
}

/// The server: every replicated mover not in `live` is gone (its entity
/// killed or broken) until a round restart.
pub fn mark_gone(world: &mut World, live: &[Entity]) {
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
    for (node, m) in gone {
        write_mover(world, node, m);
    }
}

/// The server: part `p` on `holder` (an entity the logic keeps for it),
/// replicated from the first one on.
pub fn write_part(world: &mut World, holder: Entity, p: NetEntityPart) {
    let Ok(mut e) = world.get_entity_mut(holder) else { return };
    match e.get_mut::<NetEntityPart>() {
        Some(mut have) => {
            have.set_if_neq(p);
        }
        None => {
            e.insert((Replicated, p));
        }
    }
}

/// A pusher stepped as the logics step one (Source's
/// `LogicWorld::step_movers` and `settle`, GoldSrc's `World::run_mover`),
/// without the logic: what it does once a move or wait ends is the
/// server's to say.
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

    pub fn has(&self, bit: u16) -> bool {
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
#[derive(Clone, Debug)]
pub struct SteppedMover {
    pub node: Entity,
    pub index: u32,
    pub before: Stepper,
    pub after: Stepper,
    pub d: Vec3,
    pub da: Vec3,
}

/// A client's movers at the tick it runs (`SimClock::tick`), in map
/// order: each stepped from its newest snapshot before the tick to the
/// tick before (`before`), then through the tick (`after`; a snapshot
/// for this very tick is the server's word for where it ends up, the step
/// staying as stepped). `dt`: the logic's tick length as it steps. None
/// outside a network client's predicted or replayed tick.
pub fn stepped(world: &mut World, dt: f64) -> Option<Vec<SteppedMover>> {
    if world.get_resource::<NetRole>() != Some(&NetRole::Client)
        || world.get_resource::<CommandClock>().is_none_or(|c| c.tick.is_none())
    {
        return None;
    }
    let tick = world.resource::<SimClock>().tick;
    let nodes: bevy::platform::collections::HashMap<u32, Entity> =
        world.query::<(Entity, &MapBrushEntity)>().iter(world).map(|(e, n)| (n.0 as u32, e)).collect();
    let mut out: Vec<SteppedMover> = Vec::new();
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
        let after = match buf.at_or_before(tick) {
            Some((t, m)) if *t == tick => Stepper::of(m),
            _ => s,
        };
        out.push(SteppedMover { node, index: base.index, before, after, d, da });
    }
    out.sort_by_key(|p| p.index);
    Some(out)
}

/// A client: snapshots the replays of tick `tick` on can no longer need.
pub fn prune(world: &mut World, tick: u64) {
    let keep = tick.saturating_sub(crate::net::interp::MAX_SAMPLES as u64) as f64;
    for mut buf in world.query::<&mut Snapshots<NetMover>>().iter_mut(world) {
        buf.prune(keep);
    }
}

/// A client: a mover's render colour, alpha, mode and texture frame
/// (`map::BrushTint`, replicated with its `NetMover`) onto its own copy
/// of the node, which draws it.
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

/// A client: the server's switched parts (`NetEntityPart`) onto its own
/// parts of the same map entity (`map::EntityPart`), or, where the map
/// drew none, onto the replicated entity itself (so what the logic says
/// is there for whatever draws it).
fn copy_parts(
    parts: Query<(Entity, &NetEntityPart, Option<&EntityPart>), Changed<NetEntityPart>>,
    mut own: Query<&mut EntityPart, Without<NetEntityPart>>,
    mut commands: Commands,
) {
    for (holder, p, have) in &parts {
        let want = EntityPart { entity: p.entity as usize, on: p.on, exists: p.exists };
        let mut found = false;
        for mut e in own.iter_mut().filter(|e| e.entity == want.entity) {
            found = true;
            e.set_if_neq(want);
        }
        if !found && have != Some(&want) {
            commands.entity(holder).insert(want);
        }
    }
}

/// A client: the server's light styles into `map::LightStyles` (writer
/// `net`): lit unless the pattern is dark, and a pattern of several
/// letters played.
fn copy_light_styles(
    changed: Query<(), Changed<NetLightStyles>>,
    all: Query<&NetLightStyles>,
    current: Option<Res<LightStyles>>,
    mut commands: Commands,
) {
    if changed.is_empty() {
        return;
    }
    let (mut on, mut played) = (Vec::new(), Vec::new());
    for s in all.iter().flat_map(|n| &n.styles) {
        on.push((s.0, s.1 != "a"));
        if s.1.len() > 1 {
            played.push((s.0, style_levels(&s.1)));
        }
    }
    if let Some(next) = current.as_deref().cloned().unwrap_or_default().with_patterns("net", on, played) {
        commands.insert_resource(next);
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
