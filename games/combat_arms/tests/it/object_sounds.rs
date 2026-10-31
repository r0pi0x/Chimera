//! specs/combat_arms/world_objects.md 14 (object sounds) and Corrections
//! (2026-10-22) 3, on the object world's sound state (`runtime::
//! ObjSound`, `Object::sound` and `Object::shot`) and in play (`objects::
//! sound::play`: `CaLoopSound`s that follow the object, stopped at the
//! next transition), over the network too (a client hears the server's
//! loop on its own node; a late joiner gets loops, not past one-shots).
//! Map `SoundFX` that follow a keyframed object: `music::ambient`'s
//! place follows it, and a placed one gets an anchor.

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        music::ambient::{self, CaAmbient},
        objects::{
            CaObjectMessage, CaObjects,
            movers::State,
            runtime::{Kind, ObjSound, keys},
            sound::{CaObjectSounds, OBJECT_SOUND_KEYS, ObjectSoundAnchor},
        },
        view_model::attached::CaLoopSound,
        world,
    },
    harness::CaSim,
};
use lithtech_formats::world::{BlindChunk, Object, Value, World};

use crate::{
    world_objects::{T, close, obj, v, wm},
    world_objects_play::{cuboid, prop, world_with},
};

fn door(props: &[(&str, String)]) -> T {
    let mut all = vec![("OpeningTime", "1".to_string()), ("ClosingTime", "1".to_string())];
    all.extend(props.iter().cloned());
    T::new(vec![wm("SlidingDoor", "Door", Vec3::new(10.0, 20.0, 30.0), Vec3::new(30.0, 60.0, 4.0), &all)])
}

fn sound(t: &T, name: &str) -> Option<ObjSound> {
    t.w.objects[t.id(name) as usize].sound.clone()
}

fn shot(t: &T, name: &str) -> Option<ObjSound> {
    t.w.objects[t.id(name) as usize].shot.clone()
}

/// 14, Corrections 3: a looping transition sound is the door's sound
/// while it moves, attached (`SoundPos` (0, 0, 0)), radius 1000, inner
/// 250; gone from the frame it reaches On (no `OpenSound`); the next
/// transition starts the closing loop; a locked use silences a loop
/// and plays the locked sound once.
#[test]
fn a_looping_transition_sound_lasts_its_transition() {
    let mut t = door(&[
        ("LoopSounds", "1".into()),
        ("OpeningSound", "open.wav".into()),
        ("ClosingSound", "close.wav".into()),
        ("LockedSound", "locked.wav".into()),
    ]);
    assert_eq!(sound(&t, "Door"), None, "silent at load");
    t.send("Door", "TRIGGER");
    let s = sound(&t, "Door").expect("the opening loop");
    assert_eq!(
        (s.file.as_str(), s.looping, s.attach, s.radius, s.inner),
        ("open.wav", true, Some(t.id("Door")), 1000.0, 250.0)
    );
    t.frames(19);
    assert_eq!(sound(&t, "Door").map(|s| s.serial), Some(s.serial), "the same start while it moves");
    t.frame();
    assert_eq!(t.state("Door"), State::On);
    assert_eq!(sound(&t, "Door"), None, "silent from the frame it reaches On");
    t.send("Door", "TRIGGER");
    assert_eq!(sound(&t, "Door").map(|s| (s.file, s.looping)), Some(("close.wav".into(), true)));
    t.frames(3);
    t.send("Door", "LOCK");
    t.send("Door", "TRIGGER");
    let s = sound(&t, "Door").expect("the locked sound");
    assert_eq!((s.file.as_str(), s.looping), ("locked.wav", false), "the locked sound never loops");
}

/// 14: without `LoopSounds` each transition's sound is a one-shot; the
/// next transition doesn't stop it (it plays to its end) but a new sound
/// replaces it as the door's.
#[test]
fn one_shot_transition_sounds() {
    let mut t = door(&[("OpeningSound", "open.wav".into()), ("OpenSound", "opened.wav".into())]);
    t.send("Door", "TRIGGER");
    let open = sound(&t, "Door").expect("opening");
    assert!(!open.looping);
    t.frames(20);
    let opened = sound(&t, "Door").expect("open");
    assert_eq!((opened.file.as_str(), opened.looping), ("opened.wav", false));
    assert!(opened.serial > open.serial);
    // Closing has no sound: the one-shot stays the door's last.
    t.send("Door", "TRIGGER");
    assert_eq!(sound(&t, "Door").map(|s| s.serial), Some(opened.serial));
}

/// 14: `SoundPos` not (0, 0, 0) plays at position + `SoundPos` (a world
/// offset, not rotated) and stays there; `SoundRadius` sets both radii;
/// radius 0 is kept (inaudible: the playback skips it).
#[test]
fn placed_transition_sound_and_radius() {
    let mut t = door(&[
        ("OpeningSound", "open.wav".into()),
        ("SoundPos", v(0.0, 50.0, 0.0)),
        ("SoundRadius", "400".into()),
        ("Rotation", v(0.0, 1.0, 0.0)),
    ]);
    t.send("Door", "TRIGGER");
    let s = sound(&t, "Door").unwrap();
    assert_eq!(s.attach, None);
    close(s.at.distance(Vec3::new(10.0, 70.0, 30.0)), 0.0, 1e-4, "at + SoundPos");
    assert_eq!((s.radius, s.inner), (400.0, 100.0));
}

/// Corrections 15: `REMEMBER` restarts the remembered state's looping
/// transition sound; `INIT` starts and stops nothing.
#[test]
fn remember_restarts_a_loop_init_doesnt() {
    let mut t = door(&[("LoopSounds", "1".into()), ("OpeningSound", "open.wav".into())]);
    t.send("Door", "TRIGGER");
    t.frames(5);
    t.send("Door", "MEMORIZE");
    let first = sound(&t, "Door").unwrap();
    t.frames(20);
    assert_eq!(sound(&t, "Door"), None);
    t.send("Door", "REMEMBER");
    let again = sound(&t, "Door").expect("restarted");
    assert!(again.looping && again.serial > first.serial);
    t.send("Door", "INIT");
    assert_eq!(sound(&t, "Door").map(|s| s.serial), Some(again.serial), "INIT leaves it");
}

/// 12, 14: a trigger's `ActivationSound` is a one-shot at the trigger,
/// radius `SoundRadius` (200 by default), on each activation that runs
/// its commands.
#[test]
fn trigger_activation_sound() {
    let mut t = T::new(vec![{
        let (mut kv, c) = obj("Trigger", "Trig", &[("ActivationSound", "beep.wav".into()), ("NumberOfActivations", "0".into())]);
        kv.push(("origin".into(), v(5.0, 6.0, 7.0)));
        (kv, c)
    }]);
    assert_eq!(shot(&t, "Trig"), None);
    t.send("Trig", "TRIGGER");
    let s = shot(&t, "Trig").expect("the activation sound");
    assert_eq!((s.file.as_str(), s.looping, s.attach), ("beep.wav", false, None));
    assert_eq!((s.at, s.radius, s.inner), (Vec3::new(5.0, 6.0, 7.0), 200.0, 50.0));
    t.frame();
    t.send("Trig", "TRIGGER");
    assert!(shot(&t, "Trig").unwrap().serial > s.serial, "again");
}

/// Two keys, (0, 0, 0) at 0 s and (100, 0, 0) 1 s later; key 1 has a
/// sound of radius 300.
fn keys_hex() -> String {
    let mut b = Vec::new();
    b.extend(2u32.to_le_bytes());
    for (x, stamp, snd) in [(0.0f32, 0.0f32, ""), (100.0, 1.0, "key.wav")] {
        b.extend(0u16.to_le_bytes());
        b.extend([snd.len() as u8, 0u8]);
        for f in [x, 0.0, 0.0, 0.0, 0.0, 0.0, stamp, 300.0] {
            b.extend(f.to_le_bytes());
        }
        b.extend(snd.as_bytes());
    }
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 9, 14: a keyframer's `ActiveSound` loops on its first object while
/// it is active (from `ON`, once the key list is built) and stops when
/// the list ends; a key's sound is a one-shot at the key with the key's
/// radius.
#[test]
fn keyframer_sounds() {
    let mut t = T::new(vec![
        obj(
            "KeyFramer",
            "KF",
            &[
                ("ObjectName", "Snd;Other".into()),
                ("BaseKeyName", "K".into()),
                ("ActiveSound", "hum.wav".into()),
                ("SoundRadius", "800".into()),
                (keys::KEYS, keys_hex()),
            ],
        ),
        {
            let (mut kv, _) = obj("SoundFX", "Snd", &[]);
            kv.push(("origin".into(), v(0.0, 10.0, 0.0)));
            (kv, None)
        },
        obj("SoundFX", "Other", &[]),
    ]);
    t.frame();
    assert_eq!(sound(&t, "KF"), None, "inactive: silent");
    t.send("KF", "ON");
    let s = sound(&t, "KF").expect("the active sound");
    assert_eq!(
        (s.file.as_str(), s.looping, s.attach, s.radius, s.inner),
        ("hum.wav", true, Some(t.id("Snd")), 800.0, 200.0)
    );
    t.frames(10);
    assert_eq!(sound(&t, "KF").map(|x| x.serial), Some(s.serial), "one start while active");
    t.frames(15);
    let Kind::KeyFramer(k) = &t.w.objects[t.id("KF") as usize].kind else { unreachable!() };
    assert!(!k.active, "the list ended");
    assert_eq!(sound(&t, "KF"), None, "stopped when inactive");
    let key = shot(&t, "KF").expect("key 1's sound");
    assert_eq!((key.file.as_str(), key.looping, key.at, key.radius), ("key.wav", false, Vec3::new(100.0, 0.0, 0.0), 300.0));
    // OFF while active stops it too.
    t.send("KF", "ON");
    assert!(sound(&t, "KF").is_some());
    t.send("KF", "OFF");
    assert_eq!(sound(&t, "KF"), None);
}

/// A door across +Z at z 300..310 opening 160 u up in `time` s with a
/// looping opening sound and a one-shot open sound.
fn sound_door(time: f32) -> World {
    world_with(
        vec![cuboid("Door", [-100.0, 0.0, 300.0], [100.0, 150.0, 310.0], false)],
        vec![Object {
            class: "SlidingDoor".into(),
            properties: vec![
                prop("Name", Value::String("Door".into())),
                prop("Pos", Value::Vector([0.0, 75.0, 305.0])),
                prop("MoveDir", Value::Vector([0.0, 1.0, 0.0])),
                prop("MoveDist", Value::Real(160.0)),
                prop("OpeningTime", Value::Real(time)),
                prop("LoopSounds", Value::Bool(true)),
                prop("OpeningSound", Value::String("SND\\Doors\\Slide.wav".into())),
                prop("Solid", Value::Bool(true)),
                prop("Rayhit", Value::Bool(true)),
            ],
        }],
    )
}

/// The `CaLoopSound`s written so far.
#[derive(Resource, Default)]
struct Heard(Vec<CaLoopSound>);

fn listen(app: &mut App) {
    app.init_resource::<Heard>().add_systems(
        Last,
        |mut l: MessageReader<CaLoopSound>, mut h: ResMut<Heard>| {
            h.0.extend(l.read().filter(|l| l.key.0 & OBJECT_SOUND_KEYS != 0).cloned());
        },
    );
}

fn node(app: &App, name: &str) -> Entity {
    let o = app.world().resource::<CaObjects>();
    *o.nodes.get(&o.named(name).unwrap()).expect("a node")
}

/// In play: the opening loop starts on the trigger, follows the door's
/// node and loops; it stops when the door is open.
#[test]
fn a_door_loop_plays_on_its_node_and_stops() {
    let s = combat_arms::game::movement::movement_data().meters_per_unit;
    let map = world::convert(&sound_door(1.0), "door", s, &|_| None);
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    listen(&mut sim.app);
    sim.ticks(3);
    sim.app.world_mut().write_message(CaObjectMessage {
        target: "Door".into(),
        message: "trigger".into(),
    });
    sim.ticks(6);
    let n = node(&sim.app, "Door");
    let heard = std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0);
    assert_eq!(heard.len(), 1, "{heard:?}");
    let start = &heard[0];
    let snd = start.sound.as_ref().expect("a start");
    assert_eq!(snd.file, "snd/doors/slide.wav");
    assert_eq!((start.follow, start.once), (Some(n), false), "follows the node, loops");
    assert_eq!(snd.radii, (250.0, 1000.0));
    sim.seconds(1.5);
    let heard = std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0);
    assert_eq!(heard.len(), 1, "{heard:?}");
    assert!(heard[0].sound.is_none() && heard[0].key == start.key, "stopped under its key");
}

/// Over the network: a client hears the server's loop on its own node; a
/// client joining while the door still moves gets the loop; a one-shot
/// made before it joined isn't played to it.
#[test]
fn clients_hear_object_sounds() {
    let s = combat_arms::game::movement::movement_data().meters_per_unit;
    let mut w = sound_door(30.0);
    w.objects.push(Object {
        class: "Trigger".into(),
        properties: vec![
            prop("Name", Value::String("Beep".into())),
            prop("Pos", Value::Vector([0.0, 500.0, 0.0])),
            prop("ActivationSound", Value::String("beep.wav".into())),
            prop("NumberOfActivations", Value::Real(0.0)),
        ],
    });
    let map = world::convert(&w, "door", s, &|_| None);
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(map.clone()));
        listen(app);
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(60);
    let send = |sim: &mut NetSim, target: &str| {
        sim.server.app.world_mut().write_message(CaObjectMessage {
            target: target.into(),
            message: "trigger".into(),
        })
    };
    send(&mut sim, "Door");
    send(&mut sim, "Beep");
    sim.ticks(30);
    let heard = std::mem::take(&mut sim.clients[0].app.world_mut().resource_mut::<Heard>().0);
    let n = node(&sim.clients[0].app, "Door");
    let lp = heard.iter().find(|l| !l.once).expect("the client hears the loop");
    assert_eq!(lp.follow, Some(n), "on its own node");
    let beep = heard.iter().find(|l| l.once).expect("and the one-shot");
    assert_eq!(beep.sound.as_ref().unwrap().file, "beep.wav");
    assert_eq!(beep.follow, None);
    sim.add_client(|_| {});
    sim.until_joined(600);
    sim.ticks(30);
    let heard = &sim.clients[1].app.world().resource::<Heard>().0;
    assert!(heard.iter().any(|l| !l.once && l.sound.is_some()), "a late joiner gets the loop: {heard:?}");
    assert!(!heard.iter().any(|l| l.once), "but not the past one-shot: {heard:?}");
    // A new one-shot reaches both.
    send(&mut sim, "Beep");
    sim.ticks(30);
    assert!(sim.clients[1].app.world().resource::<Heard>().0.iter().any(|l| l.once));
}

/// 14, music_ambience.md 6: a `SoundFX` a keyframer carries: the
/// ambience's place for it follows the object (an ambient one's gain is
/// by that distance), and a placed one (`Ambient` off, `PlayAttached`)
/// gets an anchor entity at the object.
#[test]
fn keyframed_map_sounds_follow_their_object() {
    let s = combat_arms::game::movement::movement_data().meters_per_unit;
    let mut w = world_with(vec![], vec![]);
    let kf = |name: &str, object: &str| Object {
        class: "KeyFramer".into(),
        properties: vec![
            prop("Name", Value::String(name.into())),
            prop("Pos", Value::Vector([0.0, 0.0, 0.0])),
            prop("ObjectName", Value::String(object.into())),
            prop("BaseKeyName", Value::String("K".into())),
            prop("IgnoreOffsets", Value::Bool(true)),
            prop("KeyDataIndex", Value::Real(0.1)),
        ],
    };
    let fx = |name: &str, ambient: bool| Object {
        class: "SoundFX".into(),
        properties: vec![
            prop("Name", Value::String(name.into())),
            prop("Pos", Value::Vector([0.0, 0.0, 0.0])),
            prop("Sound", Value::String("water.wav".into())),
            prop("Ambient", Value::Bool(ambient)),
            prop("PlayAttached", Value::Bool(true)),
            prop("OuterRadius", Value::Real(3000.0)),
            prop("InnerRadius", Value::Real(250.0)),
        ],
    };
    w.objects.extend([kf("KF", "Amb;Placed"), fx("Amb", true), fx("Placed", false)]);
    // The keys as blind data (chunk 0): (0,0,0) at 0 s, (100,0,0) 1 s on.
    let bytes: Vec<u8> = (0..keys_hex().len() / 2)
        .map(|i| u8::from_str_radix(&keys_hex()[2 * i..2 * i + 2], 16).unwrap())
        .collect();
    w.blind_data.push(BlindChunk {
        id: combat_arms::game::objects::keyframer::KEY_CHUNK_ID,
        data: bytes,
    });
    let map = world::convert(&w, "keyed", s, &|_| None);
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(10);
    let amb_at = |sim: &Sim| {
        let a = sim.app.world().resource::<CaAmbient>();
        a.sounds.iter().find(|f| f.name == "Amb").expect("Amb").at
    };
    assert!(amb_at(&sim).x.abs() < 1e-3);
    sim.app.world_mut().write_message(CaObjectMessage {
        target: "KF".into(),
        message: "on".into(),
    });
    sim.seconds(0.5);
    let x = amb_at(&sim).x;
    assert!(x > 30.0 && x < 70.0, "half way: {x}");
    // The placed one's anchor is at its object.
    let w = sim.app.world_mut();
    let placed = w.resource::<CaObjects>().named("Placed").unwrap();
    let e = *w.resource::<CaObjectSounds>().anchors.get(&placed).expect("an anchor");
    assert_eq!(w.get::<ObjectSoundAnchor>(e), Some(&ObjectSoundAnchor(placed)));
    let o = w.resource::<CaObjects>();
    let want = combat_arms::game::objects::geometry::to_engine(o.world.objects[placed as usize].pose.pos, o.scale);
    let got = w.get::<Transform>(e).unwrap().translation;
    assert!(got.distance(want) < 0.05, "{got} vs {want}");
    let _ = ambient::gain(0.0, 1.0, 2.0);
}
