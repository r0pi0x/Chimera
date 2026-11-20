//! Playback's voice limits (`audio::sound`): one source sounds at most
//! `MAX_VOICES_PER_SOURCE` voices at once, a sound without a source at
//! most `MAX_VOICES_PER_ENTRY` copies of one entry; past the limit the
//! oldest fades (`FadingOut`). Headless: a voice counts while its clip
//! lasts (`Playing::sounding`). A `StopSound` fades a source's sound
//! by its entry, nothing else.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_present::{
    MapPlugin,
    audio::{
        live_sound::LiveClip,
        sound::{MAX_VOICES_PER_ENTRY, MAX_VOICES_PER_SOURCE, Playing},
    },
    render::{
        prop_material::PropMaterial, rope_material::RopeMaterial, shadows::ShadowMaterial,
        sprite_material::SpriteMaterial, world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        MapData, MapSoundClip, MapSoundEntry, MapSounds, PlaySound, SoundLevel, StopSound,
        sound::Interval,
    },
};

use crate::prop_shadows::base;

/// Two entries, "test.shot" and "test.loop": a one-second clip.
fn sim() -> Sim {
    let mut sounds = MapSounds::default();
    sounds.clips.push(MapSoundClip {
        rate: 11025,
        channels: 1,
        samples: vec![1000i16; 11025].into(),
        loop_start: None,
    });
    sounds.entries.insert(
        "test.shot".into(),
        MapSoundEntry {
            waves: vec![0],
            volume: Interval::fixed(1.0),
            pitch: Interval::fixed(100.0),
            level: SoundLevel::Db(0.0),
            channel: 0,
            dry: false,
        },
    );
    let shot = sounds.entries["test.shot"].clone();
    sounds.entries.insert("test.loop".into(), shot);
    let data = MapData {
        name: "test:voice_limits".into(),
        sounds: Arc::new(sounds),
        ..default()
    };
    let mut sim = Sim::with(base(), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .init_asset::<LiveClip>()
            .add_plugins(MapPlugin::new(data));
    });
    sim.app.set_error_handler(bevy::ecs::error::panic);
    sim.ticks(2);
    sim
}

fn shot(source: Option<Entity>, channel: Option<u8>) -> PlaySound {
    PlaySound {
        entry: "test.shot".into(),
        at: None,
        volume: None,
        pitch: None,
        source,
        channel,
    }
}

/// Voices sounding (not faded) now, by source.
fn sounding(sim: &mut Sim, source: Option<Entity>) -> usize {
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    let w = sim.app.world_mut();
    w.query::<&Playing>()
        .iter(w)
        .filter(|p| p.source() == source && p.sounding(now))
        .count()
}

/// A source starting a sound on every channel (and on none) each frame
/// keeps at most the limit sounding; so does an entry played without a
/// source, many at once and frame after frame.
#[test]
fn voices_are_limited_per_source_and_per_entry() {
    let mut sim = sim();
    let source = sim.app.world_mut().spawn(Transform::default()).id();
    for frame in 0..6u8 {
        for channel in 0..4u8 {
            sim.app.world_mut().write_message(shot(Some(source), Some(frame * 4 + channel)));
        }
        sim.app.world_mut().write_message(shot(Some(source), None));
        for _ in 0..3 {
            sim.app.world_mut().write_message(shot(None, None));
        }
        sim.app.update();
        assert!(sounding(&mut sim, Some(source)) <= MAX_VOICES_PER_SOURCE, "frame {frame}");
        assert!(sounding(&mut sim, None) <= MAX_VOICES_PER_ENTRY, "frame {frame}");
    }
    assert_eq!(sounding(&mut sim, Some(source)), MAX_VOICES_PER_SOURCE);
    assert_eq!(sounding(&mut sim, None), MAX_VOICES_PER_ENTRY);
    // Under the limit nothing is stopped: two sources' sounds.
    let other = sim.app.world_mut().spawn(Transform::default()).id();
    sim.app.world_mut().write_message(shot(Some(other), Some(1)));
    sim.app.world_mut().write_message(shot(Some(other), Some(2)));
    sim.app.update();
    assert_eq!(sounding(&mut sim, Some(other)), 2);
    assert_eq!(sounding(&mut sim, Some(source)), MAX_VOICES_PER_SOURCE);
}

/// Nothing of a map sounds while this machine loads one (`map::LocalMapLoad`:
/// its spawn and settling behind the loading screen or the lobby) or a
/// screen stands in for the match (`sound::WorldUnheard`): a sound asked
/// for then is dropped, not played late (`sound::held`).
#[test]
fn no_sound_plays_while_a_map_loads_or_the_world_is_covered() {
    use chimera_present::audio::sound::WorldUnheard;
    use chimera_world::map::LocalMapLoad;
    let mut sim = sim();
    let source = sim.app.world_mut().spawn(Transform::default()).id();
    for hold in [0, 1] {
        match hold {
            0 => sim.app.insert_resource(LocalMapLoad { map: "test:next".into() }),
            _ => sim.app.insert_resource(WorldUnheard),
        };
        sim.app.world_mut().write_message(shot(Some(source), Some(1)));
        sim.app.update();
        assert_eq!(sounding(&mut sim, Some(source)), 0, "held ({hold})");
        sim.app.world_mut().remove_resource::<LocalMapLoad>();
        sim.app.world_mut().remove_resource::<WorldUnheard>();
        sim.app.update();
        assert_eq!(sounding(&mut sim, Some(source)), 0, "not played late ({hold})");
    }
    sim.app.world_mut().write_message(shot(Some(source), Some(1)));
    sim.app.update();
    assert_eq!(sounding(&mut sim, Some(source)), 1, "plays once the hold is over");
}

/// A gun's loop out of its holder's hand (`map::StopSound`): that
/// source's sound of that entry fades, its other sounds and another
/// source's same entry go on.
#[test]
fn a_stop_fades_one_sources_entry_only() {
    let mut sim = sim();
    let (a, b) = (sim.app.world_mut().spawn(Transform::default()).id(), sim.app.world_mut().spawn(Transform::default()).id());
    for (source, channel) in [(a, 1), (b, 1)] {
        sim.app.world_mut().write_message(PlaySound { entry: "test.loop".into(), ..shot(Some(source), Some(channel)) });
    }
    sim.app.world_mut().write_message(shot(Some(a), Some(2)));
    sim.app.update();
    assert_eq!((sounding(&mut sim, Some(a)), sounding(&mut sim, Some(b))), (2, 1));
    sim.app.world_mut().write_message(StopSound { by: None, source: a, entry: "TEST.LOOP".into(), local: false });
    sim.app.update();
    assert_eq!((sounding(&mut sim, Some(a)), sounding(&mut sim, Some(b))), (1, 1));
    let w = sim.app.world_mut();
    let left: Vec<String> = w.query::<&Playing>().iter(w).filter(|p| p.source() == Some(a)).map(|p| p.entry().to_string()).collect();
    assert_eq!(left, ["test.shot"]);
}
