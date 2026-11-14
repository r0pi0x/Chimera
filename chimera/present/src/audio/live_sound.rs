//! Long-lived sounds for any game (docs/plans/active/sound.md): started,
//! changed (volume, pitch) and stopped by their owner through
//! `SoundControl` messages, each under its own `SoundKey`. Map ambience
//! (Source's ambient_generic, from the logic layer) and soundscape loops
//! (`audio::soundscape`) use them; every clip sends to the room
//! (`audio::room`).
//!
//! A clip with a loop point plays from its start, then loops from that
//! point forever; any other clip plays once and ends. A sound sits at a
//! point or follows an entity; its distance falloff and panning follow
//! the listener while it plays. The state (`LiveSounds`) is kept without
//! audio too, so headless tests read what plays.
//!
//! Long-lived sounds started, changed and stopped by `map::SoundControl`
//! (`LiveSounds`, kept headless too; `LiveClip`, loop points, per-ear gains).

use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, Ordering},
    },
};

use bevy::{
    audio::{
        AudioPlayer, AudioSink, AudioSinkPlayback, ChannelCount, Decodable, PlaybackSettings,
        SampleRate, Source,
    },
    prelude::*,
    reflect::TypePath,
};

use super::hearing::{Hearing, HearingMix, Muffle};
use super::room::{AMBIENT_SEND, RoomBus, RoomDsp, RoomSend};
use super::sound::{Ear, SoundListener};
use crate::map::{MapSoundClip, SoundControl, SoundKey, SoundLevel, sound::SoundBank};

/// A playing long-lived sound.
#[derive(Clone, Debug)]
pub struct LiveSound {
    pub entry: String,
    /// Index into the bank's clips.
    pub clip: usize,
    pub at: Option<Vec3>,
    pub follow: Option<Entity>,
    pub volume: f32,
    /// Percent of the recorded rate.
    pub pitch: f32,
    pub level: SoundLevel,
    /// The clip loops (it has a loop point).
    pub looping: bool,
    /// Seconds of the clip left to play at pitch 100 (one-shots).
    pub left: f32,
    /// Bypasses the room.
    pub dry: bool,
    audio: Option<(Entity, Arc<Gains>)>,
}

/// Volume by kind of sound: music (`snd_musicvolume`, set by the
/// client's options) and an app's interface sounds (`snd_uivolume`, the
/// `ui` bus: `chimera_client::client::ui_sound`), times the master
/// volume.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SoundMix {
    pub music: f32,
    pub ui: f32,
}

/// Sound entries this client doesn't play (an app's own sound stands in:
/// Ultimate Gun Game's air-strike alert for Combat Arms' siren under its
/// Signal look). Matched by the whole entry.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct MutedSounds(pub Vec<String>);

/// `snd_uivolume`'s default.
pub const DEFAULT_UI_VOLUME: f32 = 0.8;

impl Default for SoundMix {
    fn default() -> Self {
        Self { music: 1.0, ui: DEFAULT_UI_VOLUME }
    }
}

impl SoundMix {
    /// The gain for a sound entry (a sound script's name or a wave path).
    pub fn gain(&self, entry: &str) -> f32 {
        if is_music(entry) { self.music.clamp(0.0, 1.0) } else { 1.0 }
    }
}

/// Whether a sound is music: a wave under `music/` (after Source's sound
/// characters) or an MP3 (CS:S ships none; community maps play their
/// music as MP3s).
pub fn is_music(entry: &str) -> bool {
    let e = entry
        .trim()
        .trim_start_matches(|c: char| "*#@<>^)}$!?&".contains(c))
        .replace('\\', "/")
        .to_ascii_lowercase();
    let e = e.strip_prefix("sound/").unwrap_or(&e);
    e.starts_with("music/") || e.ends_with(".mp3")
}

/// The long-lived sounds playing now.
#[derive(Resource, Default)]
pub struct LiveSounds {
    pub sounds: HashMap<SoundKey, LiveSound>,
}

/// Per-ear gains and the room send a playing clip reads (f32 bits), set
/// each frame; and whether it is fading out (`fade_out`).
#[derive(Debug, Default)]
pub struct Gains {
    left: AtomicU32,
    right: AtomicU32,
    send: AtomicU32,
    fading: AtomicBool,
    /// A low-pass cut-off as a fraction of the clip's bandwidth (its
    /// Nyquist frequency); 0 or 1 and above: none.
    cutoff: AtomicU32,
}

impl Gains {
    pub fn new(left: f32, right: f32) -> Self {
        let g = Self::default();
        g.set(left, right);
        g
    }
    /// With a share sent to the room (`room::distance_send`).
    pub fn with_send(self, send: f32) -> Self {
        self.set_send(send);
        self
    }
    /// Low-passed at `fraction` of the clip's bandwidth (a one-pole
    /// filter; 1: unfiltered; `map::sound::distance_cutoff`).
    pub fn with_cutoff(self, fraction: f32) -> Self {
        self.cutoff.store(fraction.to_bits(), Ordering::Relaxed);
        self
    }
    fn cutoff(&self) -> f32 {
        f32::from_bits(self.cutoff.load(Ordering::Relaxed))
    }
    /// Set the per-ear gains (a playing clip glides to them over a few
    /// ms: `GAIN_SMOOTHING`).
    pub fn set(&self, left: f32, right: f32) {
        self.left.store(left.to_bits(), Ordering::Relaxed);
        self.right.store(right.to_bits(), Ordering::Relaxed);
    }
    fn set_send(&self, send: f32) {
        self.send.store(send.to_bits(), Ordering::Relaxed);
    }
    fn send(&self) -> f32 {
        f32::from_bits(self.send.load(Ordering::Relaxed))
    }
    /// The per-ear gains.
    pub fn get(&self) -> (f32, f32) {
        (
            f32::from_bits(self.left.load(Ordering::Relaxed)),
            f32::from_bits(self.right.load(Ordering::Relaxed)),
        )
    }
    /// End the clip with a short fade (`FADE_OUT_SECONDS`) instead of a
    /// hard stop, which clicks: a sound replaced on its channel (a
    /// full-auto gun's next shot).
    pub fn fade_out(&self) {
        self.fading.store(true, Ordering::Relaxed);
    }
    fn fading(&self) -> bool {
        self.fading.load(Ordering::Relaxed)
    }
}

/// How long a clip takes to fade out once `Gains::fade_out` is called:
/// a few milliseconds, too short to hear as a fade, long enough not to
/// click (a waveform cut mid-swing is a step).
pub const FADE_OUT_SECONDS: f32 = 0.005;

/// A clip played with live gains, looping from its loop point, through
/// the listener's hearing (`hearing::HearingMix`), sending to the room
/// (`room::RoomBus`). Every sound plays as one, so the hearing and the
/// room reach them all.
#[derive(Asset, TypePath)]
pub struct LiveClip {
    clip: MapSoundClip,
    gains: Arc<Gains>,
    hearing: Arc<HearingMix>,
    room: Option<Arc<RoomBus>>,
}

impl LiveClip {
    pub fn new(clip: MapSoundClip, gains: Arc<Gains>, hearing: Arc<HearingMix>) -> Self {
        Self {
            clip,
            gains,
            hearing,
            room: None,
        }
    }

    /// Send `Gains`' share of it to the room.
    pub fn with_room(mut self, room: &RoomDsp) -> Self {
        self.room = Some(room.bus.clone());
        self
    }
}

/// Stereo samples of a `LiveClip`.
pub struct LiveDecoder {
    samples: Arc<[i16]>,
    channels: usize,
    rate: SampleRate,
    loop_start: Option<usize>,
    frame: usize,
    right: Option<f32>,
    gains: Arc<Gains>,
    current: (f32, f32),
    hearing: Arc<HearingMix>,
    muffle: Muffle,
    room: Option<RoomSend>,
    /// Fading out (`Gains::fade_out`): the gain left, down to 0, where
    /// the clip ends.
    fade: f32,
    /// The distance low-pass (`Gains::with_cutoff`): its coefficient (0:
    /// off) and each ear's last output.
    low_alpha: f32,
    low: (f32, f32),
}

/// How fast the applied gains follow a change, per frame of audio
/// (about 5 ms at 44 kHz), so changes don't click.
const GAIN_SMOOTHING: f32 = 0.005;

impl Iterator for LiveDecoder {
    type Item = bevy::audio::Sample;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(r) = self.right.take() {
            return Some(r);
        }
        let frames = self.samples.len() / self.channels.max(1);
        if self.gains.fading() {
            if self.fade <= 0.0 {
                return None;
            }
            self.fade -= 1.0 / (FADE_OUT_SECONDS * self.rate.get() as f32).max(1.0);
        }
        if self.frame >= frames {
            match self.loop_start {
                Some(s) if s < frames => self.frame = s,
                _ => return None,
            }
        }
        let i = self.frame * self.channels;
        let (l, r) = if self.channels >= 2 {
            (self.samples[i], self.samples[i + 1])
        } else {
            (self.samples[i], self.samples[i])
        };
        self.frame += 1;
        let (tl, tr) = self.gains.get();
        self.current.0 += (tl - self.current.0) * GAIN_SMOOTHING;
        self.current.1 += (tr - self.current.1) * GAIN_SMOOTHING;
        let fade = self.fade.max(0.0);
        let (mut l, mut r) = (
            l as f32 / 32768.0 * self.current.0 * fade,
            r as f32 / 32768.0 * self.current.1 * fade,
        );
        if self.low_alpha > 0.0 {
            self.low.0 += (l - self.low.0) * self.low_alpha;
            self.low.1 += (r - self.low.1) * self.low_alpha;
            (l, r) = self.low;
        }
        if let Some(room) = &mut self.room {
            room.push((l + r) * 0.5 * self.gains.send());
        }
        let (l, r) = self.muffle.frame(&self.hearing, self.rate.get() as f32, l, r);
        self.right = Some(r);
        Some(l)
    }
}

impl Source for LiveDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        self.rate
    }
    fn total_duration(&self) -> Option<std::time::Duration> {
        None
    }
}

impl Decodable for LiveClip {
    type Decoder = LiveDecoder;

    fn decoder(&self) -> LiveDecoder {
        let start = self.gains.get();
        let rate = self.clip.rate.max(1) as f32;
        let cutoff = self.gains.cutoff();
        let low_alpha = if cutoff > 0.0 && cutoff < 1.0 {
            super::hearing::low_pass_alpha(cutoff * rate / 2.0, rate)
        } else {
            0.0
        };
        LiveDecoder {
            low_alpha,
            low: (0.0, 0.0),
            samples: self.clip.samples.clone(),
            channels: self.clip.channels as usize,
            rate: SampleRate::new(self.clip.rate.max(1)).unwrap(),
            loop_start: self.clip.loop_start,
            frame: 0,
            right: None,
            gains: self.gains.clone(),
            current: start,
            hearing: self.hearing.clone(),
            muffle: Muffle::default(),
            room: self.room.clone().map(RoomSend::new),
            fade: 1.0,
        }
    }
}

/// Registered by `sound::SoundPlugin`.
pub(super) struct LiveSoundPlugin;

impl Plugin for LiveSoundPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<SoundControl>()
            .init_resource::<LiveSounds>()
            .init_resource::<SoundMix>()
            .add_systems(PostUpdate, (control, drive_audio.run_if(resource_exists::<Assets<LiveClip>>)).chain());
    }

    fn finish(&self, app: &mut App) {
        // Audio is added after the map's plugins (or not at all, headless).
        super::record::add_audio_source::<LiveClip>(app);
    }
}

/// Stop every long-lived sound (map change).
pub fn reset(world: &mut World) {
    let Some(mut live) = world.get_resource_mut::<LiveSounds>() else { return };
    let audio: Vec<Entity> = live.sounds.drain().filter_map(|(_, s)| s.audio.map(|a| a.0)).collect();
    for e in audio {
        if let Some(sink) = world.get::<AudioSink>(e) {
            sink.stop();
        }
        if let Ok(e) = world.get_entity_mut(e) {
            e.despawn();
        }
    }
}

/// Stop a long-lived sound: a short fade out (`Gains::fade_out`), after
/// which the backend despawns it; despawned at once when it isn't playing.
fn stop(commands: &mut Commands, sinks: &Query<&AudioSink>, sound: LiveSound) {
    if let Some((e, gains)) = sound.audio {
        if sinks.get(e).is_ok_and(|s| !s.is_paused() && !s.empty()) {
            gains.fade_out();
        } else {
            commands.entity(e).try_despawn();
        }
    }
}

/// Apply the owners' messages, follow entities, end finished one-shots.
#[allow(clippy::too_many_arguments)]
fn control(
    mut messages: MessageReader<SoundControl>,
    bank: Option<Res<SoundBank>>,
    mut live: ResMut<LiveSounds>,
    positions: Query<&GlobalTransform>,
    sinks: Query<&AudioSink>,
    time: Res<Time>,
    mut seed: Local<u64>,
    mut commands: Commands,
) {
    let mut unit = || {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 40) as f32 / (1u64 << 24) as f32
    };
    for m in messages.read() {
        match m {
            SoundControl::Start(s) => {
                if let Some(old) = live.sounds.remove(&s.key) {
                    stop(&mut commands, &sinks, old);
                }
                let Some(bank) = &bank else { continue };
                let (clip_index, volume, pitch, level, dry) = match s.clip {
                    Some(c) if c < bank.0.clips.len() => (
                        c,
                        s.volume.unwrap_or(1.0),
                        s.pitch.unwrap_or(100.0),
                        s.level.unwrap_or(SoundLevel::Db(75.0)),
                        s.dry,
                    ),
                    Some(_) => continue,
                    None => {
                        let Some(entry) = bank.0.entry(&s.entry) else {
                            debug!("sound: no entry '{}'", s.entry);
                            continue;
                        };
                        if entry.waves.is_empty() {
                            continue;
                        }
                        let n = entry.waves.len();
                        (
                            entry.waves[((unit() * n as f32) as usize).min(n - 1)],
                            s.volume.unwrap_or_else(|| entry.volume.draw(unit())),
                            s.pitch.unwrap_or_else(|| entry.pitch.draw(unit()).trunc()),
                            s.level.unwrap_or(entry.level),
                            s.dry || entry.dry,
                        )
                    }
                };
                let volume = volume.clamp(0.0, 1.0);
                let clip = &bank.0.clips[clip_index];
                let frames = clip.samples.len() / clip.channels.max(1) as usize;
                let looping = s.looping || clip.loop_start.is_some_and(|l| l < frames);
                info!(
                    "sound: start '{}' volume {volume:.2} pitch {pitch} level {level:?}{}",
                    s.entry,
                    if looping { ", looping" } else { "" }
                );
                live.sounds.insert(
                    s.key,
                    LiveSound {
                        entry: s.entry.clone(),
                        clip: clip_index,
                        at: s.at,
                        follow: s.follow,
                        volume,
                        pitch,
                        level,
                        looping,
                        left: frames as f32 / clip.rate.max(1) as f32,
                        dry,
                        audio: None,
                    },
                );
            }
            SoundControl::Change { key, volume, pitch } => {
                if let Some(s) = live.sounds.get_mut(key) {
                    if let Some(v) = volume {
                        s.volume = v.clamp(0.0, 1.0);
                    }
                    if let Some(p) = pitch {
                        s.pitch = *p;
                    }
                }
            }
            SoundControl::Stop(key) => {
                if let Some(old) = live.sounds.remove(key) {
                    stop(&mut commands, &sinks, old);
                }
            }
            SoundControl::StopAll => {
                let keys: Vec<SoundKey> = live.sounds.keys().filter(|k| !k.soundscape()).copied().collect();
                for k in keys {
                    if let Some(old) = live.sounds.remove(&k) {
                        stop(&mut commands, &sinks, old);
                    }
                }
            }
        }
    }
    let dt = time.delta_secs();
    let ended: Vec<SoundKey> = live
        .sounds
        .iter_mut()
        .filter_map(|(k, s)| {
            if let Some(p) = s.follow.and_then(|f| positions.get(f).ok()) {
                s.at = Some(p.translation());
            }
            if !s.looping {
                s.left -= dt * s.pitch.clamp(1.0, 255.0) / 100.0;
            }
            (!s.looping && s.left <= 0.0).then_some(*k)
        })
        .collect();
    for k in ended {
        if let Some(old) = live.sounds.remove(&k) {
            stop(&mut commands, &sinks, old);
        }
    }
}

/// Start audio for new sounds; set gains (volume, distance, panning) and
/// speed (pitch) of the playing ones.
fn drive_audio(
    mut live: ResMut<LiveSounds>,
    bank: Option<Res<SoundBank>>,
    listener: Query<(&GlobalTransform, Option<&ChildOf>), With<SoundListener>>,
    mut sinks: Query<&mut AudioSink>,
    mut clips: ResMut<Assets<LiveClip>>,
    global: Option<Res<bevy::audio::GlobalVolume>>,
    hearing: Res<Hearing>,
    room: Res<RoomDsp>,
    mix: Option<Res<SoundMix>>,
    mut commands: Commands,
) {
    let Some(bank) = bank else { return };
    let mix = mix.map_or_else(SoundMix::default, |m| *m);
    // Bevy applies the master volume when a sink starts; follow later
    // changes to it (the clip's own gains carry the rest).
    let master = global.map_or(1.0, |g| g.volume.to_linear());
    let ear = Ear::find(listener.iter());
    for s in live.sounds.values_mut() {
        let volume = s.volume * mix.gain(&s.entry);
        let (left, right, send) = match (s.at, ear) {
            (Some(at), Some(ear)) if !matches!(s.level, SoundLevel::Db(l) if l <= 0.0) => {
                let (l, r, send) = ear.gains(at, s.follow, s.level);
                (l * volume, r * volume, send)
            }
            _ => (volume, volume, AMBIENT_SEND),
        };
        let send = if s.dry { 0.0 } else { send };
        let speed = s.pitch.clamp(1.0, 255.0) / 100.0;
        match &s.audio {
            Some((e, gains)) => {
                gains.set(left, right);
                gains.set_send(send);
                if let Ok(mut sink) = sinks.get_mut(*e) {
                    if (sink.speed() - speed).abs() > 1e-4 {
                        sink.set_speed(speed);
                    }
                    if (sink.volume().to_linear() - master).abs() > 1e-4 {
                        sink.set_volume(bevy::audio::Volume::Linear(master));
                    }
                }
            }
            None => {
                let Some(clip) = bank.0.clips.get(s.clip) else { continue };
                let mut clip = clip.clone();
                let frames = clip.samples.len() / clip.channels.max(1) as usize;
                if s.looping && clip.loop_start.is_none_or(|l| l >= frames) {
                    clip.loop_start = Some(0);
                }
                let gains = Arc::new(Gains::new(left, right).with_send(send));
                let handle = clips.add(LiveClip::new(clip, gains.clone(), hearing.mix.clone()).with_room(&room));
                let e = commands
                    .spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN.with_speed(speed)))
                    .id();
                s.audio = Some((e, gains));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clip(frames: usize, loop_start: Option<usize>) -> MapSoundClip {
        MapSoundClip {
            rate: 11025,
            channels: 1,
            samples: (0..frames as i16).map(|i| i * 100).collect::<Vec<_>>().into(),
            loop_start,
        }
    }

    fn decoder(c: MapSoundClip) -> LiveDecoder {
        LiveClip::new(c, Arc::new(Gains::new(1.0, 0.5)), Arc::default()).decoder()
    }

    #[test]
    fn one_shot_plays_once_in_stereo() {
        let out: Vec<f32> = decoder(clip(4, None)).collect();
        assert_eq!(out.len(), 8);
        assert!((out[2] - 100.0 / 32768.0).abs() < 1e-6);
        assert!((out[3] - 50.0 / 32768.0).abs() < 1e-6);
    }

    #[test]
    fn loop_plays_intro_then_repeats_from_the_loop_point() {
        let left: Vec<f32> = decoder(clip(4, Some(2))).step_by(2).take(8).collect();
        let frames: Vec<i32> = left.iter().map(|v| (v * 32768.0 / 100.0).round() as i32).collect();
        assert_eq!(frames, [0, 1, 2, 3, 2, 3, 2, 3]);
    }

    /// A sound replaced on its channel fades out over a few milliseconds
    /// and ends, instead of stopping on a loud sample (a click).
    #[test]
    fn fade_out_ramps_to_silence_then_ends() {
        // A loud constant clip, a second long.
        let c = MapSoundClip {
            rate: 44100,
            channels: 1,
            samples: vec![20000i16; 44100].into(),
            loop_start: None,
        };
        let gains = Arc::new(Gains::new(1.0, 1.0));
        let mut d = LiveClip::new(c, gains.clone(), Arc::default()).decoder();
        let before: Vec<f32> = d.by_ref().take(200).collect();
        let full = 20000.0 / 32768.0;
        assert!((before[198] - full).abs() < 1e-3, "{}", before[198]);
        gains.fade_out();
        let after: Vec<f32> = d.step_by(2).collect();
        let fade_frames = (FADE_OUT_SECONDS * 44100.0) as usize;
        assert!(
            after.len() >= fade_frames - 2 && after.len() <= fade_frames + 2,
            "ends after the fade: {} frames, fade {fade_frames}",
            after.len()
        );
        // No step larger than the ramp's (a hard stop is a jump of `full`).
        let mut last = full;
        for v in &after {
            assert!(*v <= last + 1e-6 && last - v < 2.0 * full / fade_frames as f32, "{last} -> {v}");
            last = *v;
        }
        assert!(last < full * 0.02, "ends near silence: {last}");
    }
}
