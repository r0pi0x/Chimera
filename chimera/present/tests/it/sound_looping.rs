//! specs/source/sound_looping.md on playback (`audio::sound`): a game
//! sound loops only by its clip's own loop point (its WAV's `cue ` or
//! `smpl` chunk): played, it keeps sounding past its length, the decoder
//! wrapping to the loop start with no gap (cases 1, 2, 4-6: an intro once,
//! then the tail repeating), until a stop (case 7); a clip without one
//! plays once and ends (case 3), whatever its entry or channel.

use std::sync::Arc;

use bevy::{audio::Decodable, prelude::*};
use chimera_present::{
    MapPlugin,
    audio::{live_sound::LiveClip, sound::Playing},
    render::{
        prop_material::PropMaterial, rope_material::RopeMaterial, shadows::ShadowMaterial,
        sprite_material::SpriteMaterial, world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSoundClip, MapSoundEntry, MapSounds, PlaySound, SoundLevel, StopSound, sound::Interval},
};

use crate::prop_shadows::base;

/// Entries "once" (no loop point), "loop" (from sample 0) and "tail" (an
/// intro of 4 frames, then frames 4-7 repeating): 8-frame ramps 0..7.
fn sim() -> Sim {
    let mut sounds = MapSounds::default();
    for loop_start in [None, Some(0), Some(4)] {
        sounds.clips.push(MapSoundClip { rate: 8, channels: 1, samples: (0..8i16).map(|i| i * 1000).collect::<Vec<_>>().into(), loop_start });
    }
    for (i, name) in ["once", "loop", "tail"].iter().enumerate() {
        sounds.entries.insert(
            (*name).into(),
            MapSoundEntry { waves: vec![i], volume: Interval::fixed(1.0), pitch: Interval::fixed(100.0), level: SoundLevel::Db(0.0), channel: 0, dry: false },
        );
    }
    let data = MapData { name: "test:sound_looping".into(), sounds: Arc::new(sounds), ..default() };
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

fn play(sim: &mut Sim, source: Entity, entry: &str) {
    sim.app.world_mut().write_message(PlaySound { entry: entry.into(), at: None, volume: None, pitch: None, source: Some(source), channel: None });
    sim.app.update();
}

/// The entries `source` sounds now.
fn sounding(sim: &mut Sim, source: Entity) -> Vec<String> {
    let now = sim.app.world().resource::<Time>().elapsed_secs_f64();
    let w = sim.app.world_mut();
    let mut v: Vec<String> = w.query::<&Playing>().iter(w).filter(|p| p.source() == Some(source) && p.sounding(now)).map(|p| p.entry().to_string()).collect();
    v.sort();
    v
}

/// The left channel's frames (as 0..7) the playing `entry` gives, `n` of them.
fn frames(sim: &mut Sim, entry: &str, n: usize) -> Vec<i32> {
    let w = sim.app.world_mut();
    let handle = w.query::<(&Playing, &AudioPlayer<LiveClip>)>().iter(w).find(|(p, _)| p.entry() == entry).map(|(_, a)| a.0.clone()).expect("playing");
    let clips = sim.app.world().resource::<Assets<LiveClip>>();
    let d = clips.get(&handle).unwrap().decoder();
    let left: Vec<f32> = d.step_by(2).take(n).collect();
    let peak = left.iter().fold(0.0f32, |m, v| m.max(v.abs())).max(1e-9);
    left.iter().map(|v| (v / peak * 7.0).round() as i32).collect()
}

#[test]
fn a_clip_loops_only_by_its_own_loop_point() {
    let mut sim = sim();
    let source = sim.app.world_mut().spawn(Transform::default()).id();
    for e in ["once", "loop", "tail"] {
        play(&mut sim, source, e);
    }
    // Case 3: the one-shot ends with its data; 1, 2: the loops go on.
    assert_eq!(frames(&mut sim, "once", 20), [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(frames(&mut sim, "loop", 20), [0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3]);
    // 5, 6: the intro once, then from the loop start to the end.
    assert_eq!(frames(&mut sim, "tail", 16), [0, 1, 2, 3, 4, 5, 6, 7, 4, 5, 6, 7, 4, 5, 6, 7]);
    sim.seconds(3.0);
    assert_eq!(sounding(&mut sim, source), ["loop", "tail"], "the loops outlast their 1 s clips");
    // 7: a stop ends one.
    sim.app.world_mut().write_message(StopSound { by: None, source, entry: "loop".into(), local: true });
    sim.app.update();
    assert_eq!(sounding(&mut sim, source), ["tail"]);
}
