//! The world objects over the network (specs/combat_arms/world_objects.md
//! 17): the server runs every state machine, timer, trigger, counter,
//! command and hit point; clients get the objects' poses and flags. One
//! replicated entity carries every world model and volume that differs
//! from the map's start (`NetCaObjects`: position, rotation, visible,
//! solid, ray-hit, removed; a `SoundFX` a keyframer moved) and the
//! objects' sounds (`NetVoice`: a joiner gets the loops still playing,
//! not the one-shots made before it heard of them), so a late joiner gets
//! open doors, risen water and broken breakables as they are; a client writes them into its
//! own object world (built from the same map) and onto its nodes and
//! volumes. Registered as the network part `combat_arms_objects`.

use bevy::prelude::*;
use bevy_replicon::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    CaObjects, apply_effects,
    movers::Pose,
    runtime::{Kind, ObjSound},
};
use crate::{
    core::NetRole,
    map::MapFile,
    net::{NetAppExt, ordered::OrderedAppExt},
};

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
    app.replicate::<NetCaObjects>()
        .ordered::<NetCaObjects>()
        .add_systems(FixedLast, write.run_if(resource_equals(NetRole::Server)))
        .add_systems(
            PreUpdate,
            read.after(ClientSystems::Receive).run_if(resource_equals(NetRole::Client)),
        );
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
    let now = NetCaObjects {
        map,
        objects,
        sounds,
        voices,
        lights,
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
    for (i, wanted) in want.into_iter().enumerate() {
        let Some(start) = o.start.get(i).copied() else { continue };
        let (pose, flags) = wanted.unwrap_or(start);
        let ob = &mut o.world.objects[i];
        if ob.vol.speed != pads[i] {
            ob.vol.speed = pads[i];
            ob.dirty = true;
        }
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
