//! The world objects over the network (specs/combat_arms/world_objects.md
//! 17): the server runs every state machine, timer, trigger, counter,
//! command and hit point; clients get the objects' poses and flags. One
//! replicated entity carries every world model and volume that differs
//! from the map's start (`NetCaObjects`: position, rotation, visible,
//! solid, ray-hit, removed; a `SoundFX` a keyframer moved), the
//! `TargetTrigger` markers that are on and exist in the mode playing, the
//! cameras live or with a changed field of view, and the
//! objects' sounds (`NetVoice`: a joiner gets the loops still playing,
//! not the one-shots made before it heard of them), so a late joiner gets
//! open doors, risen water and broken breakables as they are; a client writes them into its
//! own object world (built from the same map) and onto its nodes and
//! volumes. Registered as the network part `combat_arms_objects`.
//!
//! Riding and meeting movers on a client (10, Client). After each tick
//! the server writes every updating active world model (doors, sliding,
//! rotating and spinning world models whose countdown runs) with what
//! changes as it runs (`movers::AwmMotion`), every running keyframer
//! with its own (`keyframer::KfMotion`) and the poses of the objects it
//! moves, and the next object frame's time on an entity of its own
//! (`NetCaMotion`, kept by server tick in the client's `Snapshots`). A
//! client runs its own player ahead of the server, so in each predicted
//! and each replayed tick it puts its own copies of those machines where
//! the server had them after the tick before (`place_motion`,
//! `core::Predict::PlaceMovers`: from the newest state at or before it,
//! stepped through the server frames since with the server's own code,
//! `movers::update_when_due`, `Keyframer::frame` and `keyframer::
//! placements`, in the server's object order, and our player's box as
//! its predicted ticks had it); its movement then sweeps the nodes where
//! the server's did. After movement (`step_motion`,
//! `core::Predict::CarryRiders`), when the server's 20 Hz frame falls on
//! the tick, it steps them once more with our player's box: a step into
//! the player is refused as on the server (6), and a player standing on
//! one (a keyframed solid world model too) is carried by its move
//! (`movers::carried`), as the server's frame carries riders. The stepped
//! poses go onto the client's object world, nodes and volumes
//! (attachments follow: HGOILRIG's rising sea carries its water and gas
//! volumes, its keyframers the water sounds); the server's poses
//! (`read`) don't overwrite them (`CaObjects::predicted`). What the
//! client can't know ahead (a door or keyframer starting, a key's
//! command, a trigger, another player in the way) shows up as a
//! prediction error when the server's state arrives, corrected and eased
//! like any other.
//!
//! Teleports (`INIT`, `GOTO`) aren't stepped: a client takes the new pose
//! from the server and never carries its own player with it
//! (world_objects.md Follow-up (2026-10-31b) 2.2, 2.3), and the server
//! carries only the other riders (bots) on a teleport, so the player's
//! place stays its own client's.
//!
//! Known shortcuts (a parity decision left to review, plan item 23): the
//! original draws sliding movers, attached volumes and keyframed objects
//! with `PushObjects` interpolated 100 ms in the past (the server's
//! `MaxInterpolateTime`), applies rotating movers' poses on arrival
//! (20 Hz steps), jumps keyframed objects without `PushObjects` to each
//! pose, and pushes or carries only its own player by each drawn step
//! (cut back when wedged, 0.1 u lower when riding, a rotating one's push
//! out in 1 u steps), never on a teleport (Follow-up (2026-10-31b) 2.1 to
//! 2.3, cases 40 to 46); ours follows the engine's prediction as for
//! Source's movers instead: movers stepped to the predicted tick, the
//! player carried and blocked there, rotating ones too, and a keyframer
//! without `PushObjects` carries the player on each frame's move (the
//! original's per-frame teleport doesn't, so its rider is in effect left
//! behind). A stepped
//! keyframer faces its target where the client has the target when the
//! stepping starts (no stock keyframer has one). Only our own player
//! meets the stepped movers on the client (others are drawn in the
//! past).

use bevy::prelude::*;
use bevy_replicon::{prelude::*, shared::server_entity_map::ServerEntityMap};
use serde::{Deserialize, Serialize};

use super::super::grenades::SERVER_FRAME;
use super::{
    CaObjects, apply_effects,
    commands::{ObjId, Who},
    keyframer::{self, KfMotion, Keyframer},
    movers::{Awm, AwmMotion, Blocking, CharBox, Pose},
    runtime::{Kind, ObjSound},
};
use crate::{
    core::{Dead, Health, MovementState, NetRole, Predict, SimClock},
    map::MapFile,
    net::{
        NetAppExt, NetHooks,
        interp::{HearTicks, Snapshots, hold_heard, remove_snapshots, write_snapshot},
        ordered::OrderedAppExt,
        predict::{CommandClock, NetGraph, predicted_player},
    },
};

/// Server frames a client steps past the newest motion state, at most.
const MAX_FRAMES: u32 = 64;

/// The active world models that are updating (moving, waiting to move or
/// to close: their countdown runs) with what changes as they run, and
/// when the server's next object frame is (`CaObjects::next_frame`),
/// written by the server after every tick on an entity of its own. A
/// client steps its own copies of them to the tick it predicts
/// (`place_motion`, `step_motion`).
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetCaMotion {
    /// The map they belong to (`MapFile::name`).
    pub map: String,
    /// None while nothing updates.
    pub next_frame: Option<f64>,
    pub movers: Vec<NetMover>,
    /// The running keyframers (built, active, not paused).
    pub keyframers: Vec<NetKeyframer>,
}

/// One running keyframer: what changes as it runs and the objects it
/// moves as the server has them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NetKeyframer {
    pub id: u32,
    pub motion: KfMotion,
    pub objects: Vec<NetPlaced>,
}

/// An object a keyframer moves.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NetPlaced {
    pub id: u32,
    pub pos: Vec3,
    pub rot: Quat,
    /// A solid world model (characters stand on it and are carried).
    pub carries: bool,
}

/// One updating active world model.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NetMover {
    pub id: u32,
    pub pos: Vec3,
    pub rot: Quat,
    pub solid: bool,
    pub motion: AwmMotion,
}

/// A client's movers as stepped to the end of `tick` from the motion
/// state of server tick `base`.
#[derive(Resource, Default, Debug, Clone)]
pub struct CaMotionPredict {
    pub tick: u64,
    pub base: u64,
    pub next_frame: f64,
    movers: Vec<Stepped>,
    /// Our player's centre after movement (before any carry) at each
    /// recent predicted tick: where the server's frame at that tick met
    /// it.
    own: std::collections::VecDeque<(u64, Vec3)>,
}

/// Ticks of our player's positions kept (`CaMotionPredict::own`).
const OWN_KEPT: usize = 256;

impl CaMotionPredict {
    /// The stepped objects' poses (active world models and what
    /// keyframers move).
    pub fn poses(&self) -> Vec<(ObjId, Pose)> {
        poses(&self.movers)
    }
}

/// One machine a client steps, by object id (the server's update order).
#[derive(Debug, Clone)]
struct Stepped {
    id: ObjId,
    run: Run,
}

#[derive(Debug, Clone)]
enum Run {
    /// An active world model.
    Awm { awm: Box<Awm>, pose: Pose, solid: bool },
    /// A keyframer with the objects it moves (id, pose, carries riders)
    /// and where its target is (taken when the stepping starts).
    Kf {
        kf: Box<Keyframer>,
        objects: Vec<(ObjId, Pose, bool)>,
        target: Option<Vec3>,
    },
}

/// The objects that differ from the map's start, on one replicated
/// entity.
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct NetCaObjects {
    /// The map they belong to (`MapFile::name`).
    pub map: String,
    pub objects: Vec<NetObject>,
    /// The map sounds (`SoundFX`) switched or restarted since the start
    /// (music_ambience.md 6: a joiner gets every sound still running).
    pub sounds: Vec<NetSound>,
    /// The objects' own sounds (world_objects.md 14: transition, active
    /// and placed one-shot sounds; `sound`).
    pub voices: Vec<NetVoice>,
    /// The light groups switched or recoloured since the start (world_dat.md
    /// "Light groups": the object sends every client its group's colour).
    pub lights: Vec<NetLight>,
    /// The `TargetTrigger` markers that are on and exist in the mode
    /// playing (world_objects.md Follow-up (2026-10-31b) 1.6).
    pub markers: Vec<u32>,
    /// The cameras that are live or changed their field of view (1.7).
    pub cameras: Vec<NetCamera>,
}

/// A camera's state.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NetCamera {
    pub id: u32,
    pub live: bool,
    pub fov: [f32; 2],
    pub fov_from: [f32; 2],
    pub fov_since: f64,
    pub fov_over: f32,
}

/// A `LightGroup`'s state.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NetLight {
    pub id: u32,
    pub on: bool,
    pub color: [f32; 3],
}

/// One object's sound (`runtime::ObjSound`) in a slot.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct NetVoice {
    pub id: u32,
    /// A placed one-shot (`Object::shot`), else the object's sound.
    pub shot: bool,
    pub file: String,
    pub at: [f32; 3],
    pub attach: Option<u32>,
    pub radius: f32,
    pub inner: f32,
    pub looping: bool,
    pub serial: u32,
}

impl NetVoice {
    fn of(id: u32, shot: bool, s: &ObjSound) -> Self {
        Self {
            id,
            shot,
            file: s.file.clone(),
            at: s.at.to_array(),
            attach: s.attach,
            radius: s.radius,
            inner: s.inner,
            looping: s.looping,
            serial: s.serial,
        }
    }

    fn sound(&self) -> ObjSound {
        ObjSound {
            file: self.file.clone(),
            at: Vec3::from(self.at),
            attach: self.attach,
            radius: self.radius,
            inner: self.inner,
            looping: self.looping,
            serial: self.serial,
        }
    }
}

/// One map sound's state.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NetSound {
    pub id: u32,
    pub on: bool,
    pub plays: u32,
}

/// One object's state (the world file's frame, u).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct NetObject {
    pub id: u32,
    pub pos: [f32; 3],
    pub rot: [f32; 4],
    /// `VISIBLE` | `SOLID` | `RAYHIT` | `REMOVED`.
    pub flags: u8,
    /// A jump pad's launch speed after a `VELOCITY` message (the client's
    /// prediction launches with it).
    pub pad: Option<f32>,
}

pub const VISIBLE: u8 = 1;
pub const SOLID: u8 = 2;
pub const RAYHIT: u8 = 4;
pub const REMOVED: u8 = 8;

pub(super) fn register(app: &mut App) {
    app.net_part("combat_arms_objects", part);
}

fn part(app: &mut App) {
    let client = || resource_equals(NetRole::Client);
    app.init_resource::<CaMotionPredict>()
        .replicate::<NetCaObjects>()
        .ordered::<NetCaObjects>()
        .replicate::<NetCaMotion>()
        .set_receive_fns::<NetCaMotion>(write_snapshot::<NetCaMotion>, remove_snapshots::<NetCaMotion>)
        .ordered_with::<NetCaMotion>(write_snapshot::<NetCaMotion>, remove_snapshots::<NetCaMotion>)
        .add_systems(FixedLast, write.run_if(resource_equals(NetRole::Server)))
        .add_systems(PreUpdate, read.after(ClientSystems::Receive).run_if(client()))
        .add_systems(PreUpdate, hold_heard::<NetCaMotion>.after(HearTicks).run_if(client()))
        // Each predicted and each replayed tick (`core::Predict`).
        .add_systems(Predict::PlaceMovers, place_motion.run_if(client()))
        .add_systems(Predict::CarryRiders, step_motion.run_if(client()));
    app.world_mut().resource_mut::<NetHooks>().stopped_serving.push(stopped_serving);
}

/// Serving stopped: the motion state stops replicating.
fn stopped_serving(world: &mut World) {
    let gone: Vec<Entity> = world.query_filtered::<Entity, With<NetCaMotion>>().iter(world).collect();
    for e in gone {
        world.entity_mut(e).despawn();
    }
}

/// The server: the updating active world models' motion (`NetCaMotion`).
fn write_motion(world: &mut World) {
    let Some(map) = world.get_resource::<MapFile>().map(|m| m.name.clone()) else { return };
    let o = world.resource::<CaObjects>();
    let movers: Vec<NetMover> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter(|(_, ob)| !ob.removed)
        .filter_map(|(i, ob)| match &ob.kind {
            Kind::Active(a) if a.countdown.is_some() => Some(NetMover {
                id: i as u32,
                pos: ob.pose.pos,
                rot: ob.pose.rot,
                solid: ob.solid,
                motion: a.motion(),
            }),
            _ => None,
        })
        .collect();
    let keyframers: Vec<NetKeyframer> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter(|(_, ob)| !ob.removed)
        .filter_map(|(i, ob)| match &ob.kind {
            Kind::KeyFramer(k) if k.running() => Some(NetKeyframer {
                id: i as u32,
                motion: k.motion(),
                objects: o
                    .world
                    .controlled(k)
                    .into_iter()
                    .map(|c| {
                        let p = &o.world.objects[c as usize];
                        NetPlaced {
                            id: c,
                            pos: p.pose.pos,
                            rot: p.pose.rot,
                            carries: p.solid && p.is_world_model(),
                        }
                    })
                    .collect(),
            }),
            _ => None,
        })
        .collect();
    let now = NetCaMotion {
        map,
        next_frame: (!movers.is_empty() || !keyframers.is_empty()).then_some(o.next_frame),
        movers,
        keyframers,
    };
    let have = world.query::<(Entity, &NetCaMotion)>().iter(world).next().map(|(e, n)| (e, n == &now));
    match have {
        Some((e, false)) => {
            world.entity_mut(e).insert(now);
        }
        Some(_) => {}
        None => {
            world.spawn((Name::new("World objects' motion"), Replicated, now));
        }
    }
}

/// Whether the server's object frame runs at a tick at `now` (s) with the
/// next one due at `next`; moves `next` on as the server does.
fn frame_due(now: f64, next: &mut f64) -> Option<f64> {
    if now + 1e-9 < *next {
        return None;
    }
    let at = *next;
    *next = at + SERVER_FRAME;
    if *next < now {
        *next = now + SERVER_FRAME;
    }
    Some(at)
}

/// Our player's box for the objects, centred at `at` (engine m).
fn own_box(world: &World, player: Entity, at: Vec3, scale: f32) -> Option<CharBox> {
    let s = world.get::<MovementState>(player)?;
    Some(super::char_box(
        player,
        at,
        s,
        world.get::<Health>(player),
        true,
        world.get::<Dead>(player).is_some(),
        false,
        scale,
    ))
}

/// The stepped objects' poses.
fn poses(movers: &[Stepped]) -> Vec<(ObjId, Pose)> {
    let mut out = Vec::new();
    for m in movers {
        match &m.run {
            Run::Awm { pose, .. } => out.push((m.id, *pose)),
            Run::Kf { objects, .. } => out.extend(objects.iter().map(|(o, p, _)| (*o, *p))),
        }
    }
    out
}

/// Characters standing on a world model moved from `from` to `to` move
/// with it (10); returns how far each was carried into `carried`.
fn carry(chars: &mut [CharBox], riders: &[usize], from: &Pose, to: &Pose, carried: &mut [Vec3]) {
    for &i in riders {
        let d = super::movers::carried(&chars[i], from, to);
        chars[i].min += d;
        chars[i].max += d;
        carried[i] += d;
    }
}

/// One server frame of the stepped machines (in object order, as the
/// server's frame updates them), with `chars` as the characters they
/// meet: active world models' updates (`movers::update_when_due`) and
/// running keyframers' (`Keyframer::frame`, `keyframer::placements`;
/// their keys' sounds and commands are the server's); each move carries
/// the riders' boxes along. Returns how far each character was carried.
fn step_frame(movers: &mut [Stepped], chars: &mut [CharBox], at: f64) -> Vec<Vec3> {
    let mut carried = vec![Vec3::ZERO; chars.len()];
    let mut out = Vec::new();
    for m in movers.iter_mut() {
        match &mut m.run {
            Run::Awm { awm, pose, solid } => {
                out.clear();
                let Some((next, riders)) =
                    super::movers::update_when_due(awm, pose, *solid, chars, at, SERVER_FRAME as f32, &mut out)
                else {
                    continue;
                };
                carry(chars, &riders, pose, &next, &mut carried);
                *pose = next;
            }
            Run::Kf { kf, objects, target } => {
                if !kf.running() {
                    continue;
                }
                let f = kf.frame(SERVER_FRAME as f32);
                kf.snap = false;
                let Some((pos, rot)) = f.place else { continue };
                let now: Vec<(ObjId, Pose)> = objects.iter().map(|(o, p, _)| (*o, *p)).collect();
                let places = keyframer::placements(kf, pos, rot, &now, *target);
                for ((_, pose, carries), new) in objects.iter_mut().zip(places) {
                    if new != *pose {
                        let riders = Blocking {
                            chars: &*chars,
                            solid: *carries,
                        }
                        .riders(pose);
                        carry(chars, &riders, pose, &new, &mut carried);
                    }
                    *pose = new;
                }
            }
        }
    }
    carried
}

/// The stepped objects onto the client's own object world, their nodes and
/// volumes (attachments follow, `ObjectWorld::set_pose`); those the
/// prediction places aren't taken from the server's poses (`read`).
fn show_motion(world: &mut World, movers: &[Stepped]) {
    {
        let mut o = world.resource_mut::<CaObjects>();
        let mut placed: Vec<ObjId> = Vec::new();
        for (id, pose) in poses(movers) {
            o.world.set_pose(id, pose);
            let mut stack = vec![id];
            while let Some(id) = stack.pop() {
                if !placed.contains(&id) {
                    placed.push(id);
                    stack.extend(o.world.objects[id as usize].attached.iter().map(|a| a.id));
                }
            }
        }
        if o.predicted != placed {
            o.predicted = placed;
        }
    }
    apply_effects(world);
}

/// A client, before movement in each predicted and each replayed tick
/// (`core::Predict::PlaceMovers`):
/// the active world models where the server had them after the tick
/// before (`CaMotionPredict`): from the newest motion state at or before
/// it (`NetCaMotion`), its machines stepped through the server frames
/// since with the same code (`movers::update_when_due`), our player as
/// the predicted ticks had it; then the nodes placed there.
fn place_motion(world: &mut World) {
    if world.get_resource::<NetRole>() != Some(&NetRole::Client) {
        return;
    }
    let Some(clock) = world.get_resource::<CommandClock>().cloned() else { return };
    if clock.tick.is_none() || world.resource::<CaObjects>().from.is_none() {
        return;
    }
    let Some(map) = world.get_resource::<MapFile>().map(|m| m.name.clone()) else { return };
    let tick = world.resource::<SimClock>().tick;
    let before = tick.saturating_sub(1);
    let base = world
        .query::<&Snapshots<NetCaMotion>>()
        .iter(world)
        .next()
        .and_then(|b| b.at_or_before(before).cloned());
    let Some((base_tick, net)) = base.filter(|(_, n)| n.map == map) else {
        if !world.resource::<CaObjects>().predicted.is_empty() {
            world.resource_mut::<CaObjects>().predicted.clear();
        }
        return;
    };
    let cache = world.resource::<CaMotionPredict>();
    if cache.tick == before && cache.base == base_tick && base_tick <= before {
        let movers = cache.movers.clone();
        show_motion(world, &movers);
        return;
    }
    // From the server's motion state at `base_tick`.
    let scale = world.resource::<CaObjects>().scale;
    let to_client = |w: &World, who: Who| match who {
        Who::Character(bits) => Entity::try_from_bits(bits)
            .and_then(|e| w.get_resource::<ServerEntityMap>()?.to_client().get(&e).copied())
            .map_or(Who::Nobody, |e| Who::Character(e.to_bits())),
        other => other,
    };
    let mut movers: Vec<Stepped> = {
        let o = world.resource::<CaObjects>();
        let awms = net.movers.iter().filter_map(|m| {
            let ob = o.world.objects.get(m.id as usize)?;
            let mut awm = Box::new(ob.awm()?.clone());
            let mut motion = m.motion.clone();
            motion.activator = to_client(world, motion.activator);
            awm.set_motion(&motion);
            Some(Stepped {
                id: m.id,
                run: Run::Awm {
                    awm,
                    pose: Pose {
                        pos: m.pos,
                        rot: m.rot,
                        half: ob.pose.half,
                    },
                    solid: m.solid,
                },
            })
        });
        let kfs = net.keyframers.iter().filter_map(|n| {
            let Kind::KeyFramer(k) = &o.world.objects.get(n.id as usize)?.kind else { return None };
            let mut kf = k.clone();
            kf.set_motion(&n.motion);
            let objects = n
                .objects
                .iter()
                .filter_map(|p| {
                    let ob = o.world.objects.get(p.id as usize)?;
                    Some((
                        p.id,
                        Pose {
                            pos: p.pos,
                            rot: p.rot,
                            half: ob.pose.half,
                        },
                        p.carries,
                    ))
                })
                .collect();
            let target = o.world.keyframer_target(&kf);
            Some(Stepped {
                id: n.id,
                run: Run::Kf { kf, objects, target },
            })
        });
        awms.chain(kfs).collect()
    };
    movers.sort_by_key(|m| m.id);
    let mut next = net.next_frame.unwrap_or(f64::INFINITY);
    // The server frames since, our player where its predicted tick had
    // it after movement.
    let player = predicted_player(world);
    let own = std::mem::take(&mut world.resource_mut::<CaMotionPredict>().own);
    let mut t = base_tick + 1;
    let mut frames = 0;
    while t <= before && frames < MAX_FRAMES && !movers.is_empty() {
        let Some(now) = clock.clock_for(t).map(|c| c.now) else { break };
        if let Some(at) = frame_due(now, &mut next) {
            frames += 1;
            let at_t = player.and_then(|p| {
                let pos = own.iter().rev().find(|(k, _)| *k == t)?.1;
                own_box(world, p, pos, scale)
            });
            let mut chars: Vec<CharBox> = at_t.into_iter().collect();
            step_frame(&mut movers, &mut chars, at);
        }
        t += 1;
    }
    *world.resource_mut::<CaMotionPredict>() = CaMotionPredict {
        tick: before,
        base: base_tick,
        next_frame: next,
        movers: movers.clone(),
        own,
    };
    show_motion(world, &movers);
}

/// A client, after movement in each predicted and each replayed tick
/// (`core::Predict::CarryRiders`): when the server's object frame runs at this
/// tick, the movers' step, our player carried as the server carries
/// riders (world_objects.md 10), and the nodes placed.
fn step_motion(world: &mut World) {
    let tick = world.resource::<SimClock>().tick;
    let now = world.resource::<SimClock>().now;
    if world.resource::<CaObjects>().from.is_none() {
        return;
    }
    let player = predicted_player(world);
    if let Some(at) = player.and_then(|p| world.get::<Transform>(p)).map(|t| t.translation) {
        let mut c = world.resource_mut::<CaMotionPredict>();
        // (A replayed tick replaces what was predicted for it.)
        while c.own.back().is_some_and(|(k, _)| *k >= tick) {
            c.own.pop_back();
        }
        c.own.push_back((tick, at));
        while c.own.len() > OWN_KEPT {
            c.own.pop_front();
        }
    }
    let mut cache = world.resource::<CaMotionPredict>().clone();
    if cache.tick + 1 != tick || cache.movers.is_empty() {
        return;
    }
    cache.tick = tick;
    if let Some(at) = frame_due(now, &mut cache.next_frame) {
        let scale = world.resource::<CaObjects>().scale;
        let own = player.and_then(|p| {
            let at = world.get::<Transform>(p)?.translation;
            own_box(world, p, at, scale)
        });
        let mut chars: Vec<CharBox> = own.into_iter().collect();
        let carried = step_frame(&mut cache.movers, &mut chars, at);
        if let (Some(p), Some(d)) = (player, carried.first().copied())
            && d != Vec3::ZERO
        {
            let d = super::geometry::to_engine(d, scale);
            if let Ok(mut e) = world.get_entity_mut(p) {
                if let Some(mut t) = e.get_mut::<Transform>() {
                    t.translation += d;
                }
                if let Some(mut q) = e.get_mut::<avian3d::prelude::Position>() {
                    q.0 += d;
                }
            }
            world.resource_mut::<NetGraph>().carried += 1;
        }
    }
    let movers = cache.movers.clone();
    *world.resource_mut::<CaMotionPredict>() = cache;
    show_motion(world, &movers);
}

pub(super) fn flags_of(o: &super::runtime::Object) -> u8 {
    ((o.visible as u8) * VISIBLE) | ((o.solid as u8) * SOLID) | ((o.rayhit as u8) * RAYHIT) | ((o.removed as u8) * REMOVED)
}

/// The server's objects that differ from the start.
fn write(world: &mut World) {
    let Some(map) = world.get_resource::<MapFile>().map(|m| m.name.clone()) else { return };
    let o = world.resource::<CaObjects>();
    let objects: Vec<NetObject> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter(|(i, ob)| {
            (ob.node || o.volumes.contains_key(&(*i as u32)) || ob.class == "SoundFX")
                && o.start.get(*i).is_some_and(|s| (ob.pose, flags_of(ob)) != *s || ob.vol.speed.is_some())
        })
        .map(|(i, ob)| NetObject {
            id: i as u32,
            pos: ob.pose.pos.to_array(),
            rot: ob.pose.rot.to_array(),
            flags: flags_of(ob),
            pad: ob.vol.speed,
        })
        .collect();
    let sounds: Vec<NetSound> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter(|(_, ob)| ob.class == "SoundFX")
        .filter_map(|(i, ob)| match ob.kind {
            Kind::Switchable { on, plays } if (on, plays) != (ob.props.bool_or("StartOn", true), 0) => Some(NetSound {
                id: i as u32,
                on,
                plays,
            }),
            _ => None,
        })
        .collect();
    let voices: Vec<NetVoice> = o
        .world
        .objects
        .iter()
        .enumerate()
        .flat_map(|(i, ob)| {
            [(false, &ob.sound), (true, &ob.shot)]
                .into_iter()
                .filter_map(move |(shot, s)| s.as_ref().map(|s| NetVoice::of(i as u32, shot, s)))
        })
        .collect();
    let lights: Vec<NetLight> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter_map(|(i, ob)| match ob.kind {
            Kind::LightGroup { on, color } if Some(&ob.kind) != o.world_start_light(i).as_ref() => Some(NetLight {
                id: i as u32,
                on,
                color: color.to_array(),
            }),
            _ => None,
        })
        .collect();
    let family = crate::game::rules::current_mode(world).and_then(|m| m.spawns.family);
    let o = world.resource::<CaObjects>();
    let markers: Vec<u32> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter(|(_, ob)| !ob.removed)
        .filter_map(|(i, ob)| match &ob.kind {
            Kind::Trigger(t) => t.marker.as_ref().filter(|m| m.on && m.exists_in(family)).map(|_| i as u32),
            _ => None,
        })
        .collect();
    let cameras: Vec<NetCamera> = o
        .world
        .objects
        .iter()
        .enumerate()
        .filter_map(|(i, ob)| match &ob.kind {
            Kind::Camera(c) if c.live || c.fov != super::events::Camera::of(&ob.props).fov => Some(NetCamera {
                id: i as u32,
                live: c.live,
                fov: c.fov.to_array(),
                fov_from: c.fov_from.to_array(),
                fov_since: c.fov_since,
                fov_over: c.fov_over,
            }),
            _ => None,
        })
        .collect();
    let now = NetCaObjects {
        map,
        objects,
        sounds,
        voices,
        lights,
        markers,
        cameras,
    };
    let have = world.query::<(Entity, &NetCaObjects)>().iter(world).next().map(|(e, n)| (e, n.clone()));
    match have {
        Some((e, n)) if n != now => {
            world.entity_mut(e).insert(now);
        }
        Some(_) => {}
        None => {
            world.spawn((Name::new("World objects"), Replicated, now));
        }
    }
    write_motion(world);
}

/// A client: the server's objects into its own, and onto the nodes.
fn read(world: &mut World) {
    let Some(map) = world.get_resource::<MapFile>().map(|m| m.name.clone()) else { return };
    let Some(net) = world.query::<&NetCaObjects>().iter(world).next().cloned() else { return };
    if net.map != map {
        return;
    }
    {
    let mut o = world.resource_mut::<CaObjects>();
    if o.from.is_none() {
        return;
    }
    let n = o.world.objects.len();
    let mut want: Vec<Option<(Pose, u8)>> = vec![None; n];
    let mut pads: Vec<Option<f32>> = vec![None; n];
    for x in &net.objects {
        let Some(ob) = o.world.objects.get(x.id as usize) else { continue };
        pads[x.id as usize] = x.pad;
        want[x.id as usize] = Some((
            Pose {
                pos: Vec3::from(x.pos),
                rot: Quat::from_array(x.rot),
                half: ob.pose.half,
            },
            x.flags,
        ));
    }
    let predicted = o.predicted.clone();
    for (i, wanted) in want.into_iter().enumerate() {
        let Some(start) = o.start.get(i).copied() else { continue };
        let (pose, flags) = wanted.unwrap_or(start);
        let ob = &mut o.world.objects[i];
        if ob.vol.speed != pads[i] {
            ob.vol.speed = pads[i];
            ob.dirty = true;
        }
        // (The prediction places the movers it steps: `place_motion`.)
        let pose = if predicted.contains(&(i as ObjId)) { ob.pose } else { pose };
        if ob.pose != pose || flags_of(ob) != flags {
            ob.pose = pose;
            ob.visible = flags & VISIBLE != 0;
            ob.solid = flags & SOLID != 0;
            ob.rayhit = flags & RAYHIT != 0;
            ob.removed = flags & REMOVED != 0;
            ob.dirty = true;
        }
    }
    }
    {
        let mut o = world.resource_mut::<CaObjects>();
        for i in 0..o.world.objects.len() {
            if o.world.objects[i].class != "SoundFX" {
                continue;
            }
            let start = o.world.objects[i].props.bool_or("StartOn", true);
            let (on, plays) = net.sounds.iter().find(|s| s.id as usize == i).map_or((start, 0), |s| (s.on, s.plays));
            o.world.objects[i].kind = Kind::Switchable { on, plays };
        }
        // The light groups.
        for i in 0..o.world.objects.len() {
            let Some(start) = o.world_start_light(i) else { continue };
            o.world.objects[i].kind = net
                .lights
                .iter()
                .find(|l| l.id as usize == i)
                .map_or(start, |l| Kind::LightGroup {
                    on: l.on,
                    color: Vec3::from(l.color),
                });
        }
        // The markers (on when the server says) and the cameras.
        for i in 0..o.world.objects.len() {
            let ob = &mut o.world.objects[i];
            match &mut ob.kind {
                Kind::Trigger(t) => {
                    if let Some(m) = &mut t.marker {
                        m.on = net.markers.contains(&(i as u32));
                    }
                }
                Kind::Camera(c) => {
                    let start = super::events::Camera::of(&ob.props);
                    match net.cameras.iter().find(|n| n.id as usize == i) {
                        Some(n) => {
                            c.live = n.live;
                            c.fov = Vec2::from(n.fov);
                            c.fov_from = Vec2::from(n.fov_from);
                            c.fov_since = n.fov_since;
                            c.fov_over = n.fov_over;
                        }
                        None => {
                            c.live = false;
                            c.fov = start.fov;
                            c.fov_from = start.fov;
                            c.fov_over = 0.0;
                        }
                    }
                }
                _ => {}
            }
        }
        // The objects' sounds; the one-shots made before this client
        // first heard of them already played (17: a joiner doesn't get
        // past one-shot sounds).
        let floor = *o
            .net_sound_floor
            .get_or_insert_with(|| net.voices.iter().map(|v| v.serial).max().unwrap_or(0));
        for ob in &mut o.world.objects {
            ob.sound = None;
            ob.shot = None;
        }
        for v in &net.voices {
            if !v.looping && v.serial <= floor {
                continue;
            }
            let Some(ob) = o.world.objects.get_mut(v.id as usize) else { continue };
            if v.shot {
                ob.shot = Some(v.sound());
            } else {
                ob.sound = Some(v.sound());
            }
        }
    }
    apply_effects(world);
}
