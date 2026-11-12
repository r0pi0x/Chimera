//! `RecordAudioPlugin`: Bevy's audio played into a WAV file instead of a
//! sound device, timed by the frames, for recordings (the client's
//! `--offscreen` with `--dump`, docs/OBSERVABILITY.md section 2). It
//! takes Bevy's `AudioPlugin`'s place: the same components
//! (`AudioPlayer`, `PlaybackSettings`, `AudioSink`) and assets (`AudioSource` with its loader, `Pitch`), so everything that
//! plays or steers sounds works unchanged, but every player is connected
//! to a mixer of ours (rodio's) that nothing drains in real time.
//!
//! Each frame, in `First` once the clock has moved (`mix_frame`), the
//! mixer is pulled for exactly the real time since the last frame
//! (`Time<Real>`, rounded on the running total so nothing drifts) and
//! appended to the file, 16-bit stereo at `RATE`. Sounds started in a
//! frame's `PostUpdate` thus begin at the sample position that frame had
//! (`AudioRecording::position`), which the frame dumps write next to each
//! frame (`frames.tsv`): a frame and its sounds line up exactly however
//! slowly frames render, since the audio is produced by frames, not by a
//! device's clock. When the app exits, one more frame's worth is mixed so
//! the last frame has its sound too: N frames of 1/fps give N x RATE/fps
//! samples. The header's sizes are rewritten after every frame, so a
//! killed run still leaves a valid file.
//!
//! `plays_audio` and `add_audio_source` are what the audio modules use to
//! register their own sources with whichever of the two is in the app.
//!
//! Known shortcuts:
//! - Spatial players (`PlaybackSettings::spatial`) play unpanned; nothing
//!   of ours uses Bevy's spatial audio.
//! - Voice chat (the client's `voice`) has its own cpal output and is not
//!   recorded.

use std::{
    fs::File,
    io::{BufWriter, Seek, SeekFrom, Write},
    num::NonZero,
    path::PathBuf,
    sync::Mutex,
};

use bevy::{
    audio::{
        AddAudioSource, AudioLoader, AudioPlayer, AudioPlugin, AudioSink, AudioSinkPlayback, AudioSource, Decodable,
        DefaultSpatialScale, GlobalVolume, PlaybackMode,
        PlaybackSettings, Pitch, Source,
    },
    prelude::*,
    time::TimeSystems,
    transform::TransformSystems,
};
use rodio::{
    Player,
    mixer::{Mixer, MixerSource},
};

/// The recording's sample rate.
pub const RATE: u32 = 48_000;
/// The recording's channels (stereo).
pub const CHANNELS: u16 = 2;

/// Bevy's audio into a WAV file at `path` (see the module doc).
pub struct RecordAudioPlugin {
    pub path: PathBuf,
}

impl Plugin for RecordAudioPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GlobalVolume>()
            .init_resource::<DefaultSpatialScale>()
            .insert_resource(AudioRecording::create(self.path.clone()))
            .configure_sets(PostUpdate, RecordPlaybackSystems.after(TransformSystems::Propagate))
            .add_systems(First, mix_frame.after(TimeSystems))
            .add_systems(Last, finish_on_exit);
        app.init_asset::<AudioSource>().init_asset_loader::<AudioLoader>();
        add_source::<AudioSource>(app);
        add_source::<Pitch>(app);
    }
}

/// Where `RecordAudioPlugin` plays and cleans up players.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
struct RecordPlaybackSystems;

/// Whether the app plays sound: Bevy's `AudioPlugin` or `RecordAudioPlugin`.
pub fn plays_audio(app: &App) -> bool {
    app.is_plugin_added::<AudioPlugin>() || app.is_plugin_added::<RecordAudioPlugin>()
}

/// Play `T`'s players with whichever audio the app has (none: nothing).
/// For a plugin's `finish`, once the audio plugin is in.
pub fn add_audio_source<T: Asset + Decodable>(app: &mut App) {
    if app.is_plugin_added::<AudioPlugin>() {
        app.add_audio_source::<T>();
    } else if app.is_plugin_added::<RecordAudioPlugin>() {
        add_source::<T>(app);
    }
}

fn add_source<T: Asset + Decodable>(app: &mut App) {
    app.init_asset::<T>().add_systems(
        PostUpdate,
        (play_queued::<T>, cleanup_finished::<T>).in_set(RecordPlaybackSystems),
    );
}

/// The mixer every player is connected to, and the file it is drained
/// into, a frame at a time.
#[derive(Resource)]
pub struct AudioRecording {
    mixer: Mixer,
    out: Mutex<MixerSource>,
    file: Option<BufWriter<File>>,
    path: PathBuf,
    /// `Time<Real>`'s elapsed time at the first frame.
    start: Option<f64>,
    /// Sample frames (one per channel each) written.
    written: u64,
    /// The last frame's length, in sample frames.
    last: u64,
    finished: bool,
    bytes: Vec<u8>,
}

impl AudioRecording {
    fn create(path: PathBuf) -> Self {
        let (mixer, out) = rodio::mixer::mixer(NonZero::new(CHANNELS).unwrap(), NonZero::new(RATE).unwrap());
        let file = path
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| File::create(&path))
            .map(BufWriter::new)
            .and_then(|mut f| {
                write_header(&mut f, 0)?;
                Ok(f)
            });
        let file = match file {
            Ok(f) => {
                info!("audio: recording into {}", path.display());
                Some(f)
            }
            Err(e) => {
                error!("audio: can't record into {}: {e}", path.display());
                None
            }
        };
        Self {
            mixer,
            out: Mutex::new(out),
            file,
            path,
            start: None,
            written: 0,
            last: 0,
            finished: false,
            bytes: Vec::new(),
        }
    }

    /// Where the next sound starts, in sample frames from the file's
    /// start: this frame's position once `mix_frame` has run.
    pub fn position(&self) -> u64 {
        self.written
    }

    /// The file being written.
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }

    /// Mix `frames` sample frames and append them to the file.
    fn mix(&mut self, frames: u64) {
        if frames == 0 {
            return;
        }
        self.bytes.clear();
        if let Ok(mut out) = self.out.lock() {
            for _ in 0..frames * u64::from(CHANNELS) {
                // An empty mixer ends (`None`) until a sound is added: silence.
                let s = out.next().unwrap_or(0.0);
                let s = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)).round() as i16;
                self.bytes.extend_from_slice(&s.to_le_bytes());
            }
        }
        self.written += frames;
        if let Some(f) = &mut self.file {
            let data = (self.written * u64::from(CHANNELS) * 2).min(u64::from(u32::MAX - 36)) as u32;
            let ok = f
                .write_all(&self.bytes)
                .and_then(|()| write_header(f, data))
                .and_then(|()| f.seek(SeekFrom::End(0)).map(drop))
                .and_then(|()| f.flush());
            if let Err(e) = ok {
                error!("audio: writing {}: {e}", self.path.display());
                self.file = None;
            }
        }
    }

    /// One more frame's length (the last frame's sound), once.
    fn finish(&mut self) {
        if !self.finished {
            self.finished = true;
            self.mix(self.last);
        }
    }
}

impl Drop for AudioRecording {
    fn drop(&mut self) {
        self.finish();
    }
}

/// A 16-bit PCM WAV header for `data` bytes of samples, at the file's start.
fn write_header(f: &mut BufWriter<File>, data: u32) -> std::io::Result<()> {
    let block = CHANNELS * 2;
    f.seek(SeekFrom::Start(0))?;
    let mut h = Vec::with_capacity(44);
    h.extend_from_slice(b"RIFF");
    h.extend_from_slice(&(36 + data).to_le_bytes());
    h.extend_from_slice(b"WAVEfmt ");
    h.extend_from_slice(&16u32.to_le_bytes());
    h.extend_from_slice(&1u16.to_le_bytes());
    h.extend_from_slice(&CHANNELS.to_le_bytes());
    h.extend_from_slice(&RATE.to_le_bytes());
    h.extend_from_slice(&(RATE * u32::from(block)).to_le_bytes());
    h.extend_from_slice(&block.to_le_bytes());
    h.extend_from_slice(&16u16.to_le_bytes());
    h.extend_from_slice(b"data");
    h.extend_from_slice(&data.to_le_bytes());
    f.write_all(&h)
}

/// The real time since the last frame, mixed into the file (see the
/// module doc).
fn mix_frame(time: Res<Time<Real>>, mut rec: ResMut<AudioRecording>) {
    if rec.finished {
        return;
    }
    let now = time.elapsed_secs_f64();
    let start = *rec.start.get_or_insert(now);
    let due = ((now - start) * f64::from(RATE)).round().max(0.0) as u64;
    let frames = due.saturating_sub(rec.written);
    if frames > 0 {
        rec.last = frames;
    }
    rec.mix(frames);
}

/// The last frame's sound, when the app is asked to exit.
fn finish_on_exit(mut exits: MessageReader<AppExit>, mut rec: ResMut<AudioRecording>) {
    if exits.read().next().is_some() {
        rec.finish();
    }
}

/// What to do with a player once it has played (`PlaybackMode`): despawn
/// it, or remove its playback components.
#[derive(Component)]
struct DespawnWhenDone;

#[derive(Component)]
struct RemoveWhenDone;

/// Each player whose source has loaded gets an `AudioSink` playing into
/// our mixer, its `PlaybackSettings` applied as Bevy's audio applies them.
fn play_queued<T: Asset + Decodable>(
    rec: Res<AudioRecording>,
    sources: Res<Assets<T>>,
    global: Res<GlobalVolume>,
    queued: Query<(Entity, &AudioPlayer<T>, &PlaybackSettings), Without<AudioSink>>,
    mut commands: Commands,
) {
    for (entity, player, settings) in &queued {
        let Some(source) = sources.get(&player.0) else { continue };
        if settings.spatial {
            warn_once!("audio: spatial players are recorded unpanned");
        }
        let sink = Player::connect_new(&rec.mixer);
        // The clip from its start position, for its duration, looped or not.
        let mut clip: Box<dyn Source + Send> = Box::new(source.decoder());
        if let Some(start) = settings.start_position {
            clip = Box::new(clip.skip_duration(start));
        }
        if let Some(length) = settings.duration {
            clip = Box::new(clip.take_duration(length));
        }
        match settings.mode {
            PlaybackMode::Loop => sink.append(clip.repeat_infinite()),
            _ => sink.append(clip),
        }
        let mut sink = AudioSink::new(sink);
        if settings.muted {
            sink.mute();
        }
        sink.set_speed(settings.speed);
        sink.set_volume(settings.volume * global.volume);
        if settings.paused {
            sink.pause();
        }
        let mut e = commands.entity(entity);
        match settings.mode {
            PlaybackMode::Loop | PlaybackMode::Once => e.insert(sink),
            PlaybackMode::Despawn => e.insert((sink, DespawnWhenDone)),
            PlaybackMode::Remove => e.insert((sink, RemoveWhenDone)),
        };
    }
}

/// Finished players despawned, or their playback components removed, as
/// their mode asks.
fn cleanup_finished<T: Asset + Decodable>(
    mut commands: Commands,
    despawn: Query<(Entity, &AudioSink), (With<DespawnWhenDone>, With<AudioPlayer<T>>)>,
    remove: Query<(Entity, &AudioSink), (With<RemoveWhenDone>, With<AudioPlayer<T>>)>,
) {
    for (entity, sink) in &despawn {
        if sink.empty() {
            commands.entity(entity).despawn();
        }
    }
    for (entity, sink) in &remove {
        if sink.empty() {
            commands
                .entity(entity)
                .remove::<(AudioPlayer<T>, AudioSink, PlaybackSettings, RemoveWhenDone)>();
        }
    }
}
