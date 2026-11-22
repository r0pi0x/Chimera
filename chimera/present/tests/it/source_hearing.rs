//! Source's hearing in playback (specs/source/sound_hearing.md; `audio::
//! sound::Ear::hear`): a TF2 Rocket Launcher shot (94 dB, mixer 0.72)
//! 4000 u straight ahead plays at its distance gain (~0.10) times the
//! mixer and the straight-ahead pan (0.64 per ear); the server sends it no
//! farther than 4400 u (`map::sound::send_range`); the same shot behind a
//! wall (a static collider across the line from the ear) plays at 0.393 of
//! that; the listener's own shot plays centred at full level.

use std::sync::Arc;

use avian3d::prelude::{Collider, RigidBody};
use bevy::prelude::*;
use chimera_present::{
    MapPlugin,
    audio::{
        live_sound::LiveClip,
        sound::{Playing, SoundListener},
    },
    render::{
        prop_material::PropMaterial, rope_material::RopeMaterial, shadows::ShadowMaterial,
        sprite_material::SpriteMaterial, world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        MapData, MapSoundClip, MapSoundEntry, MapSounds, PlaySound, SoundLevel,
        hearing::{EntryHearing, OBSCURED_DB, obscured_gain, source_gain},
        sound::{Interval, SOURCE_UNIT, send_range},
    },
};

use crate::prop_shadows::base;

const ROCKET: &str = "weapon_rpg.single";

fn sim() -> Sim {
    let mut sounds = MapSounds::default();
    sounds.clips.push(MapSoundClip {
        rate: 11025,
        channels: 1,
        samples: vec![1000; 11025].into(),
        loop_start: None,
    });
    sounds.entries.insert(
        ROCKET.into(),
        MapSoundEntry {
            waves: vec![0],
            volume: Interval::fixed(1.0),
            pitch: Interval::fixed(100.0),
            level: SoundLevel::Db(94.0),
            channel: 1,
            dry: false,
            hearing: Some(EntryHearing {
                mix_player: 0.72,
                mix_world: 0.72,
                obscured: obscured_gain(OBSCURED_DB),
                near_far: false,
            }),
        },
    );
    let data = MapData {
        name: "test:source_hearing".into(),
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

/// The gains of the newest sound playing `ROCKET` from `source`.
fn newest(sim: &mut Sim, source: Option<Entity>) -> (f32, f32) {
    let w = sim.app.world_mut();
    let mut q = w.query::<&Playing>();
    q.iter(w)
        .filter(|p| p.entry() == ROCKET && p.source() == source)
        .max_by(|a, b| a.started().total_cmp(&b.started()))
        .map(|p| p.gains())
        .expect("playing")
}

fn shot(sim: &mut Sim, at: Vec3, source: Option<Entity>) -> (f32, f32) {
    sim.app.world_mut().write_message(PlaySound {
        entry: ROCKET.into(),
        at: Some(at),
        volume: None,
        pitch: None,
        source,
        channel: None,
    });
    sim.app.update();
    newest(sim, source)
}

#[test]
fn a_far_rocket_shot_by_distance_mixer_pan_and_wall() {
    let mut sim = sim();
    let me = sim.app.world_mut().spawn(Transform::default()).id();
    let ear = sim.app.world_mut().spawn((SoundListener, Transform::default(), ChildOf(me))).id();
    sim.ticks(2);
    let _ = ear;
    let far = 4000.0 * SOURCE_UNIT;
    // Straight ahead (−Z), 4000 u: the distance gain ~0.10.
    let (l, r) = shot(&mut sim, Vec3::new(0.0, 0.0, -far), None);
    let distance = source_gain(94.0, 4000.0);
    assert!((distance - 0.097).abs() < 1e-3, "{distance}");
    assert!((l - r).abs() < 1e-5, "centred ahead: {l} {r}");
    assert!((l / (0.72 * 0.64) - distance).abs() < 1e-3, "{l}");
    // The server's send radius: 4400 u for 94 dB.
    let range = send_range(SoundLevel::Db(94.0)).unwrap();
    assert!((range / SOURCE_UNIT - 4400.0).abs() < 0.1);
    assert!(far <= range && 4500.0 * SOURCE_UNIT > range);
    // A wall across the line: × 0.393.
    sim.app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(20.0, 20.0, 0.2),
        Transform::from_xyz(0.0, 0.0, -far * 0.5),
    ));
    sim.ticks(3);
    let (bl, br) = shot(&mut sim, Vec3::new(0.0, 0.0, -far), None);
    assert!((bl / l - 0.393).abs() < 2e-3, "behind the wall: {}", bl / l);
    assert!((br / r - 0.393).abs() < 2e-3);
    // The listener's own shot behind the same wall: centred, full level.
    let (ol, or) = shot(&mut sim, Vec3::new(0.0, 0.0, -far), Some(me));
    assert_eq!((ol, or), (1.0, 1.0));
    // To the left past 44°: the right ear hears nothing.
    let (sl, sr) = shot(&mut sim, Vec3::new(-10.0, 0.0, -2.0), None);
    assert!(sl > 0.0 && sr == 0.0, "{sl} {sr}");
}
