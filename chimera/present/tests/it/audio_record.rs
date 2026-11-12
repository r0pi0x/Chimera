//! `audio::record`: the sound of a run recorded into a WAV file, timed by
//! the frames (the client's `--offscreen` with `--dump`).

use std::time::Duration;

use bevy::{
    audio::{AudioPlayer, Pitch, PlaybackSettings},
    prelude::*,
    time::TimeUpdateStrategy,
};
use chimera_formats::wav;
use chimera_present::audio::record::{AudioRecording, RATE, RecordAudioPlugin};

const FPS: u32 = 30;

/// N frames of 1/30 s make a WAV of N x RATE/30 sample frames, silent
/// until a sound starts and that sound from the sample position its frame
/// had, however long the frames really took.
#[test]
fn frames_time_the_recording() {
    let dir = std::env::temp_dir().join(format!("chimera-audio-record-{}", std::process::id()));
    let path = dir.join("audio.wav");
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), RecordAudioPlugin { path: path.clone() }))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(1) / FPS));
    let frames = 30u64;
    let per_frame = u64::from(RATE / FPS);
    let mut started = None;
    for frame in 0..frames {
        if frame == 10 {
            // A 440 Hz tone for 0.2 s, queued for frame 10: it plays from there.
            let tone = app.world_mut().resource_mut::<Assets<Pitch>>().add(Pitch::new(440.0, Duration::from_millis(200)));
            app.world_mut().spawn((AudioPlayer(tone), PlaybackSettings::DESPAWN));
        }
        app.update();
        if frame == 10 {
            started = Some(app.world().resource::<AudioRecording>().position());
        }
    }
    let started = started.unwrap();
    assert_eq!(started, 10 * per_frame, "frame 10's sounds start at its position");
    app.world_mut().write_message(AppExit::Success);
    app.update();
    drop(app);

    let clip = wav::decode(&std::fs::read(&path).unwrap()).unwrap();
    std::fs::remove_dir_all(&dir).ok();
    assert_eq!((clip.rate, clip.channels), (RATE, 2));
    let len = clip.samples.len() as u64 / 2;
    // N frames (the exit's one of them) make N frames' sound: the exit
    // mixes one more frame's length for the last frame.
    assert_eq!(len, (frames + 1) * per_frame, "one frame's sound per frame");
    let peak = |from: u64, to: u64| {
        clip.samples[(from * 2) as usize..(to * 2) as usize].iter().map(|s| s.unsigned_abs()).max().unwrap_or(0)
    };
    assert_eq!(peak(0, started), 0, "silent before the sound");
    assert!(peak(started, started + RATE as u64 / 10) > 3000, "the tone plays from its frame");
    // Its first sample frames: the tone starts right at `started`.
    assert!(peak(started, started + 100) > 0, "no gap before the tone");
    assert_eq!(peak(started + RATE as u64 / 4, len), 0, "silent after the tone's 0.2 s");
}
