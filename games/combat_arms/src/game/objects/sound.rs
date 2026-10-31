//! The world objects' sounds as each machine plays them (specs/
//! combat_arms/world_objects.md 14; Corrections (2026-10-22) 3): the
//! object world keeps every object's current sound (`runtime::ObjSound`:
//! an active world model's transition sound, a keyframer's `ActiveSound`,
//! a trigger's or a key's one-shot), the server's reaches clients with
//! the objects (`net`), and `play` turns each start and stop into
//! `CaLoopSound`s (`effects::sounds::loops` reads the file the first
//! time):
//!
//! - a looping sound plays under one key per object until the object's
//!   next transition stops it or starts another; a one-shot under a key
//!   of its own, so it plays to its end (only loops are stopped);
//! - an attached sound (an active world model's `SoundPos` exactly
//!   (0, 0, 0), a keyframer's first object) follows its object: the
//!   world model's node, or an anchor entity (`ObjectSoundAnchor`) kept at
//!   the object's place for an object without a node (a keyframed
//!   `SoundFX`); a `SoundFX` with `PlayAttached` gets an anchor too, for
//!   the ambience's player (`ui::music`);
//! - 3D at their radii (inner `SoundRadius` × 0.25 for world models,
//!   keyframers and triggers), volume 100, radius 0 inaudible.
//!
//! A client that joins later gets the loops still playing from the start
//! of their file (ours: world_objects.md Open question 10 leaves it open;
//! map `SoundFX` loops reach joiners: music_ambience.md 6), and none of
//! the one-shots already played (`CaObjects::net_sound_floor`).

use bevy::{platform::collections::HashMap, prelude::*};

use super::{CaObjects, commands::ObjId, geometry::to_engine, runtime::ObjSound};
use super::super::{effects::sounds::CaSound, view_model::attached::CaLoopSound};
use crate::map::SoundKey;

/// The objects' sound keys: this bit, the object id from bit 24, the
/// slot (bit 23: a placed one-shot) and a one-shot's serial below.
pub const OBJECT_SOUND_KEYS: u64 = 1 << 56;

/// The key a sound of object `id` plays under: a loop's is the object's
/// slot's (a new start replaces it, a stop ends it), a one-shot's its own.
pub fn key(id: ObjId, s: &ObjSound, shot: bool) -> SoundKey {
    let base = OBJECT_SOUND_KEYS | (id as u64) << 24 | (shot as u64) << 23;
    if s.looping {
        SoundKey(base)
    } else {
        SoundKey(base | ((s.serial as u64) & 0x7f_ffff).max(1))
    }
}

/// An entity kept at an object's place for the sounds that follow it.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectSoundAnchor(pub ObjId);

/// What this machine has started: per object and slot, the serial and
/// whether it loops; the anchors.
#[derive(Resource, Default, Debug)]
pub struct CaObjectSounds {
    pub started: HashMap<(ObjId, bool), (u32, bool)>,
    pub anchors: HashMap<ObjId, Entity>,
    /// The object world they belong to (its entities' address).
    from: usize,
}

impl CaObjectSounds {
    /// The entity sounds of object `id` follow: its node, else its anchor.
    pub fn follow(&self, objects: &CaObjects, id: ObjId) -> Option<Entity> {
        objects.nodes.get(&id).or_else(|| self.anchors.get(&id)).copied()
    }
}

/// The sound file as the install's mount reads it (lower case, `/`).
pub fn file_path(file: &str) -> String {
    file.trim().to_lowercase().replace('\\', "/")
}

/// The `CaSound` an object sound plays as (`scale` metres per unit).
pub fn ca_sound(s: &ObjSound, scale: f32) -> CaSound {
    CaSound {
        radii: (s.inner.min(s.radius), s.radius),
        ..CaSound::at(file_path(&s.file), to_engine(s.at, scale), s.radius)
    }
}

/// Start and stop the objects' sounds as their state changed, and keep
/// the anchors at their objects.
pub fn play(
    objects: Res<CaObjects>,
    mut state: ResMut<CaObjectSounds>,
    mut anchors: Query<&mut Transform, With<ObjectSoundAnchor>>,
    mut out: MessageWriter<CaLoopSound>,
    mut commands: Commands,
) {
    let from = objects
        .from
        .as_ref()
        .map_or(0, |f| std::sync::Arc::as_ptr(f) as *const u8 as usize);
    if state.from != from {
        // Another map: stop what plays, drop the anchors.
        for ((id, shot), (_, looping)) in std::mem::take(&mut state.started) {
            if looping {
                out.write(stop(id, shot));
            }
        }
        for (_, e) in std::mem::take(&mut state.anchors) {
            commands.entity(e).try_despawn();
        }
        state.from = from;
    }
    if from == 0 {
        return;
    }
    let scale = if objects.scale > 0.0 { objects.scale } else { 1.0 };
    // The objects sounds follow that have no node: an anchor each.
    let mut wanted: Vec<ObjId> = Vec::new();
    for (i, o) in objects.world.objects.iter().enumerate() {
        let attached = [&o.sound, &o.shot].into_iter().flatten().filter_map(|s| s.attach);
        wanted.extend(attached.filter(|a| !objects.nodes.contains_key(a)));
        if o.class == "SoundFX" && o.props.bool_or("PlayAttached", false) && !objects.nodes.contains_key(&(i as ObjId)) {
            wanted.push(i as ObjId);
        }
    }
    for id in wanted {
        let at = objects.world.objects.get(id as usize).map_or(Vec3::ZERO, |o| to_engine(o.pose.pos, scale));
        if let Some(&e) = state.anchors.get(&id) {
            if let Ok(mut t) = anchors.get_mut(e)
                && t.translation != at
            {
                t.translation = at;
            }
            continue;
        }
        let e = commands
            .spawn((Name::new(format!("Object sound {id}")), ObjectSoundAnchor(id), Transform::from_translation(at)))
            .id();
        state.anchors.insert(id, e);
    }
    for (i, o) in objects.world.objects.iter().enumerate() {
        let id = i as ObjId;
        for (shot, now) in [(false, &o.sound), (true, &o.shot)] {
            let had = state.started.get(&(id, shot)).copied();
            match (now, had) {
                (Some(s), Some((serial, _))) if serial == s.serial => {}
                (Some(s), had) => {
                    // A loop that ends without another taking its key.
                    if had.is_some_and(|(_, l)| l) && !s.looping {
                        out.write(stop(id, shot));
                    }
                    state.started.insert((id, shot), (s.serial, s.looping));
                    if s.radius <= 0.0 || s.file.trim().is_empty() {
                        continue;
                    }
                    let follow = s.attach.and_then(|a| state.follow(&objects, a));
                    out.write(CaLoopSound {
                        key: key(id, s, shot),
                        sound: Some(ca_sound(s, scale)),
                        follow,
                        once: !s.looping,
                    });
                }
                (None, Some((_, looping))) => {
                    state.started.remove(&(id, shot));
                    if looping {
                        out.write(stop(id, shot));
                    }
                }
                (None, None) => {}
            }
        }
    }
}

/// Stop object `id`'s loop in a slot.
fn stop(id: ObjId, shot: bool) -> CaLoopSound {
    let probe = ObjSound {
        file: String::new(),
        at: Vec3::ZERO,
        attach: None,
        radius: 0.0,
        inner: 0.0,
        looping: true,
        serial: 0,
    };
    CaLoopSound {
        key: key(id, &probe, shot),
        sound: None,
        follow: None,
        once: false,
    }
}
