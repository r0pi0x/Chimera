//! The front end's music and ambience (`ui::music`, headless): what the
//! client decided (`game::music`) is played as long-lived sounds
//! (`map::SoundControl`), the volume following the fade; a stream that
//! leaves the player stops; the loading screen and a window with no
//! pixels are told to the music (specs/combat_arms/music_ambience.md 2,
//! 3, 8; cases 1, 5, 18 as the front end sees them); a map's ambient
//! loops end with it (no old loop starts again across a swap).

use std::{path::PathBuf, time::Duration};

use bevy::{
    prelude::*,
    time::TimeUpdateStrategy,
    window::{PrimaryWindow, WindowResolution},
};
use chimera_world::{
    greybox::GreyboxMapPlugin,
    harness::Sim,
    map::{
        MapData, MapDebugView, MapSimPlugin, SoundControl, SoundKey,
        prepare::{begin_map_change, spawn_some},
    },
};
use combat_arms::{
    game::{
        data::CaData,
        movement,
        music::{
            CaMusicSet, MusicClock, MusicLoading,
            ambient::SoundFx,
            data::MusicData,
            director::{CaMusic, MusicFacts, Screen},
            mix::CaVolumes,
        },
        world,
    },
    harness::{CaSim, CaSimWith},
    ui::{
        loading::CaLoading,
        music::{CaMusicPlayPlugin, Playback, ambient_volume},
    },
};
use lithtech_formats::world::{Object, Property, Value, World as LtWorld};

#[derive(Resource, Default)]
struct Seen(Vec<SoundControl>);

fn collect(mut r: MessageReader<SoundControl>, mut seen: ResMut<Seen>) {
    seen.0.extend(r.read().cloned());
}

fn sim() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaMusicPlayPlugin))
            .add_message::<SoundControl>()
            .init_resource::<Seen>()
            .add_systems(Update, collect.after(CaMusicSet));
    });
    sim.app.world_mut().resource_mut::<MusicClock>().manual = true;
    sim.ticks(3);
    sim
}

/// A one-second 16-bit mono WAV in a folder of ours.
fn wav(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("ca_music_ui_test");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(name);
    std::fs::write(&path, wav_bytes()).unwrap();
    path
}

/// A one-second 16-bit mono WAV.
fn wav_bytes() -> Vec<u8> {
    let samples = 8000usize;
    let mut b = Vec::new();
    b.extend(b"RIFF");
    b.extend((36 + samples as u32 * 2).to_le_bytes());
    b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(1u16.to_le_bytes());
    b.extend(8000u32.to_le_bytes());
    b.extend(16000u32.to_le_bytes());
    b.extend(2u16.to_le_bytes());
    b.extend(16u16.to_le_bytes());
    b.extend(b"data");
    b.extend((samples as u32 * 2).to_le_bytes());
    b.extend((0..samples).flat_map(|i| ((i as i16 % 100) * 50).to_le_bytes()));
    b
}

fn set_now(sim: &mut Sim, ms: f64) {
    sim.app.world_mut().resource_mut::<MusicClock>().now = ms;
}

fn starts(sim: &Sim) -> Vec<(SoundKey, f32, bool)> {
    sim.app
        .world()
        .resource::<Seen>()
        .0
        .iter()
        .filter_map(|c| match c {
            SoundControl::Start(s) => Some((s.key, s.volume.unwrap_or(1.0), s.looping)),
            _ => None,
        })
        .collect()
}

fn changes(sim: &Sim) -> Vec<(SoundKey, f32)> {
    sim.app
        .world()
        .resource::<Seen>()
        .0
        .iter()
        .filter_map(|c| match c {
            SoundControl::Change {
                key, volume: Some(v), ..
            } => Some((*key, *v)),
            _ => None,
        })
        .collect()
}

/// The menu playlist's one track plays as a looping sound after it
/// decodes, its volume the music master times the fade; a stopped
/// player stops the sound.
#[test]
fn the_playlist_plays_and_follows_the_fade() {
    let mut sim = sim();
    let track = MusicData::built_in().playlist[0].file.clone();
    {
        let w = sim.app.world_mut();
        w.resource_mut::<Playback>().add_file(&track, wav("loop.wav"));
        let mut m = w.resource_mut::<CaMusic>();
        m.files.insert(&track, Some(1000.0));
        m.screen = Screen::Menu;
        w.resource_mut::<CaVolumes>().set_music(60);
    }
    set_now(&mut sim, 0.0);
    for _ in 0..200 {
        sim.ticks(1);
        if !starts(&sim).is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let s = starts(&sim);
    assert_eq!(s.len(), 1, "one stream: {:?}", sim.app.world().resource::<Seen>().0);
    assert!(s[0].2, "a single track loops");
    assert!(s[0].1.abs() < 1e-3, "the 10 s fade-in starts from silence: {}", s[0].1);
    // Half way through the fade at music 60: 0.5 x 0.6.
    set_now(&mut sim, 5_000.0);
    sim.ticks(2);
    let c = changes(&sim);
    assert!((c.last().unwrap().1 - 0.3).abs() < 1e-3, "{c:?}");
    // The music switch off: silent, the sound keeps going.
    sim.app.world_mut().resource_mut::<CaVolumes>().music_on = false;
    sim.ticks(2);
    assert_eq!(changes(&sim).last().unwrap().1, 0.0);
    assert_eq!(sim.app.world().resource::<Playback>().counts().0, 1);
    // Stopping the player stops the sound.
    sim.app.world_mut().resource_mut::<CaMusic>().player.stop();
    sim.app.world_mut().resource_mut::<CaMusic>().paused = true;
    sim.ticks(2);
    assert!(
        sim.app
            .world()
            .resource::<Seen>()
            .0
            .iter()
            .any(|c| matches!(c, SoundControl::Stop(k) if *k == s[0].0)),
        "stopped"
    );
    assert_eq!(sim.app.world().resource::<Playback>().counts().0, 0);
}

/// The loading screen is told to the music, which then plays its
/// loading track and stops the playlist (case 6).
#[test]
fn the_loading_screen_reaches_the_music() {
    let mut sim = sim();
    sim.app.world_mut().resource_mut::<CaMusic>().files =
        combat_arms::game::music::director::MusicFiles::from_data(MusicData::built_in());
    set_now(&mut sim, 0.0);
    sim.ticks(3);
    assert!(
        sim.app.world().resource::<CaMusic>().player.playing(),
        "the playlist runs at the title"
    );
    sim.app.world_mut().insert_resource(CaLoading {
        label: Some("server".into()),
        ..Default::default()
    });
    sim.ticks(3);
    assert!(sim.app.world().resource::<MusicLoading>().0);
    let facts = sim.app.world().resource::<MusicFacts>().clone();
    assert_eq!(facts.screen, Screen::Loading);
    let m = sim.app.world().resource::<CaMusic>();
    assert_eq!(
        m.player.primary.as_ref().map(|s| s.file.as_str()),
        Some("bgm/map_loading.mp3")
    );
}

/// Both masters go to 0 while the window has no pixels, back when it has
/// (case 18).
#[test]
fn a_window_without_pixels_is_minimized() {
    let mut sim = sim();
    let w = sim
        .app
        .world_mut()
        .spawn((
            Window {
                resolution: WindowResolution::new(0, 0),
                ..Default::default()
            },
            PrimaryWindow,
        ))
        .id();
    sim.app.world_mut().resource_mut::<CaVolumes>().set_music(80);
    sim.ticks(2);
    let v = *sim.app.world().resource::<CaVolumes>();
    assert!(v.minimized);
    assert_eq!((v.music(), v.sound()), (0.0, 0.0));
    sim.app.world_mut().entity_mut(w).insert(Window {
        resolution: WindowResolution::new(1280, 720),
        ..Default::default()
    });
    sim.ticks(2);
    let v = *sim.app.world().resource::<CaVolumes>();
    assert!(!v.minimized);
    assert!((v.music() - 0.8).abs() < 1e-6);
}

/// An ambient sound's played volume is its distance gain times its
/// Volume and the effects master; a placed one keeps Volume and lets the
/// radius level do the rest.
#[test]
fn ambient_volumes() {
    let mut fx = SoundFx::from_props(
        0,
        "birds",
        &combat_arms::game::objects::runtime::Props(vec![
            ("OuterRadius".into(), "6000".into()),
            ("InnerRadius".into(), "4000".into()),
            ("Volume".into(), "50".into()),
        ]),
        Vec3::ZERO,
        true,
        0,
    );
    let at = |d: f32| Some(Vec3::new(d, 0.0, 0.0));
    assert!((ambient_volume(&fx, at(3000.0), 0.8) - 0.4).abs() < 1e-5);
    assert!((ambient_volume(&fx, at(4200.0), 1.0) - 0.295).abs() < 3e-3);
    assert_eq!(ambient_volume(&fx, at(6500.0), 1.0), 0.0);
    assert_eq!(ambient_volume(&fx, None, 1.0), 0.0);
    fx.ambient = false;
    assert!((ambient_volume(&fx, at(6500.0), 0.8) - 0.4).abs() < 1e-5);
}

/// Sound files of our own, as an install's archive serves them.
struct Sounds(Vec<&'static str>);

impl chimera_core::mount::FileSource for Sounds {
    fn name(&self) -> String {
        "test sounds".into()
    }
    fn entries(&self) -> Vec<chimera_core::mount::Entry> {
        self.0
            .iter()
            .map(|p| chimera_core::mount::Entry { path: p.to_string(), size: 1 })
            .collect()
    }
    fn read(&self, path: &str) -> Option<std::io::Result<Vec<u8>>> {
        self.0.contains(&path).then(|| Ok(wav_bytes()))
    }
}

fn prop(name: &str, value: Value) -> Property {
    Property {
        name: name.into(),
        flags: 0,
        len: 0,
        value,
    }
}

/// A level with a start point and one looping `SoundFX` playing `file`.
fn looping_map(name: &str, file: &str) -> MapData {
    let mut w = LtWorld::default();
    w.objects.push(Object {
        class: "GameStartPoint".into(),
        properties: vec![
            prop("Name", Value::String("GameStartPoint00".into())),
            prop("Pos", Value::Vector([0.0, 66.0, 0.0])),
            prop("Rotation", Value::Rotation([0.0; 4])),
        ],
    });
    w.objects.push(Object {
        class: "SoundFX".into(),
        properties: vec![
            prop("Name", Value::String(format!("{name}_loop"))),
            prop("Pos", Value::Vector([0.0, 66.0, 0.0])),
            prop("Sound", Value::String(file.into())),
            prop("OuterRadius", Value::Real(2000.0)),
            prop("InnerRadius", Value::Real(500.0)),
        ],
    });
    world::convert(&w, name, movement::movement_data().meters_per_unit, &|_| None)
}

/// The ambient files started since the last `Seen` clear.
fn ambient_starts(sim: &Sim) -> Vec<String> {
    sim.app
        .world()
        .resource::<Seen>()
        .0
        .iter()
        .filter_map(|c| match c {
            SoundControl::Start(s) if s.entry.starts_with("ca:amb:") => Some(s.entry.clone()),
            _ => None,
        })
        .collect()
}

/// Frames drawn between fixed ticks (a client draws faster than its
/// tick, and goes on drawing behind the loading screen).
fn frames(sim: &mut Sim, n: usize) {
    let tick = sim.tick();
    for _ in 0..n {
        sim.app.update();
    }
    assert_eq!(sim.tick(), tick, "no fixed tick in these frames");
}

/// A map's ambient loops end with it: after a swap no sound of the old
/// map starts again, neither while the next one loads (the front end's
/// stand-in sound bank) nor in the frames before its objects are built on
/// its first tick; the new map's own loop starts then.
#[test]
fn ambient_loops_end_with_their_map() {
    let mut mount = chimera_core::mount::Mount::default();
    mount.push(Sounds(vec!["snd/old/amb.wav", "snd/new/amb.wav"]));
    let mut sim = Sim::with(CaSimWith(CaData::builtin_with_mount(mount)), |app| {
        app.add_plugins((MapSimPlugin::new(looping_map("old", "SND\\Old\\amb.wav")), CaMusicPlayPlugin))
            .add_message::<SoundControl>()
            .init_resource::<Seen>()
            .add_systems(Update, collect.after(CaMusicSet));
    });
    sim.ticks(5);
    assert_eq!(ambient_starts(&sim), ["ca:amb:snd/old/amb.wav"], "the old map's loop plays");
    sim.app.world_mut().resource_mut::<Seen>().0.clear();
    // Frames shorter than a tick from here on.
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_micros(100)));
    // The swap as a client makes it: the old map unloads now, the next
    // spawns over frames, the loading screen up meanwhile.
    begin_map_change(
        sim.app.world_mut(),
        looping_map("new", "SND\\New\\amb.wav"),
        Default::default(),
        MapDebugView::Normal,
    );
    frames(&mut sim, 3);
    while !spawn_some(sim.app.world_mut(), Duration::from_secs(1)) {
        frames(&mut sim, 1);
    }
    frames(&mut sim, 3);
    let early = ambient_starts(&sim);
    assert!(
        !early.iter().any(|e| e.contains("snd/old/")),
        "the old map's loop started again after the swap: {early:?}"
    );
    // Back to whole ticks: the new map's objects are built and its loop
    // plays, the old one's still not.
    let tick = Duration::from_secs_f64(1.0 / sim.tick_hz());
    sim.app.insert_resource(TimeUpdateStrategy::ManualDuration(tick));
    sim.ticks(5);
    let all = ambient_starts(&sim);
    assert!(!all.iter().any(|e| e.contains("snd/old/")), "{all:?}");
    assert!(all.iter().any(|e| e == "ca:amb:snd/new/amb.wav"), "the new map's loop plays: {all:?}");
}
