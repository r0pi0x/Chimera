//! Sound playback for any game (docs/plans/active/sound.md): playback
//! resolves a `map::PlaySound`'s entry (random wave, drawn volume, pitch
//! and level), applies the distance model (and a `Radius` level's
//! distance low-pass, `Gains::with_cutoff`) and left/right panning toward
//! the listener, and plays it, pitch as playback speed. A new sound on the
//! same (source entity, channel) replaces the one playing there.
//!
//! Voice limits (ours, for the mix: every voice is summed and the output
//! clips past full scale): a source sounds at most
//! `MAX_VOICES_PER_SOURCE` voices at once and a sound without a source
//! at most `MAX_VOICES_PER_ENTRY` copies of one entry; a new one past the
//! limit fades out the oldest.
//!
//! The listener: `SoundListener` (the client has one) and its `Ear`s.

use std::sync::Arc;

use bevy::{
    audio::{AudioPlayer, PlaybackSettings, Volume},
    prelude::*,
};

use super::{
    hearing::Hearing,
    live_sound::{Gains, LiveClip},
    room::{RoomDsp, distance_send},
};
use crate::map::{
    MapHooks, MapSoundClip, PlaySound, SoundLevel,
    sound::{SoundBank, distance_cutoff, distance_gain},
};

/// Who's listening (the camera the player sees through). The entity it
/// is a child of (the player whose eyes it is) hears its own sounds
/// centred, not panned (`Ear`).
#[derive(Component)]
pub struct SoundListener;

/// The listener as sounds are mixed for it: where it is and whose ears
/// they are (the `SoundListener`'s parent).
#[derive(Clone, Copy, Debug)]
pub struct Ear {
    pub at: GlobalTransform,
    pub owner: Option<Entity>,
}

impl Ear {
    /// The first `SoundListener` (the client has one).
    pub fn find<'a>(mut listeners: impl Iterator<Item = (&'a GlobalTransform, Option<&'a ChildOf>)>) -> Option<Self> {
        listeners.next().map(|(at, parent)| Self {
            at: *at,
            owner: parent.map(ChildOf::parent),
        })
    }

    /// Per-ear gains (distance and panning) and the room send for a
    /// sound at `at` (engine space) made by `source`.
    ///
    /// The listener's own sounds (its gunfire, reloads, footsteps) play
    /// centred at full distance gain, as Source plays the local player's
    /// own sounds: they are emitted from the simulation's latest tick
    /// while the camera is drawn between the last two ticks
    /// (`map::interp`), so while strafing a shot sat a few centimetres to
    /// the side of the ear, and `pan` of that direction is a hard pan.
    /// Other sounds within `NEAR_PAN` of the ear blend toward the
    /// centre for the same reason (a spectated player's eye).
    pub fn gains(&self, at: Vec3, source: Option<Entity>, level: SoundLevel) -> (f32, f32, f32) {
        if source.is_some() && source == self.owner {
            let g = distance_gain(level, 0.0);
            return (g, g, distance_send(0.0));
        }
        let to = at - self.at.translation();
        let d = to.length();
        let g = distance_gain(level, d);
        let (pl, pr) = pan(to.normalize_or_zero(), self.at.right().as_vec3());
        let near = (d / NEAR_PAN).clamp(0.0, 1.0);
        let (pl, pr) = (1.0 + (pl - 1.0) * near, 1.0 + (pr - 1.0) * near);
        (g * pl, g * pr, distance_send(d))
    }
}

/// Sounds this close to the listener (metres, about a head's width)
/// pan less, down to centred at the ear.
pub const NEAR_PAN: f32 = 0.4;

/// A playing sound: who made it on which channel, and its live gains
/// (to fade it out when another replaces it). Public to read (tests
/// count the voices a source has sounding and their gains).
#[derive(Component)]
pub struct Playing {
    source: Option<Entity>,
    channel: Option<u8>,
    gains: Arc<Gains>,
    entry: String,
    /// When it started and when its clip ends (`Time` elapsed, s).
    started: f64,
    ends: f64,
}

impl Playing {
    pub fn source(&self) -> Option<Entity> {
        self.source
    }
    pub fn channel(&self) -> Option<u8> {
        self.channel
    }
    /// Its per-ear gains as set when it started.
    pub fn gains(&self) -> (f32, f32) {
        self.gains.get()
    }
    pub fn entry(&self) -> &str {
        &self.entry
    }
    /// Still sounding at `now` (`Time` elapsed, s): its clip hasn't
    /// ended. Headless, nothing despawns a finished one.
    pub fn sounding(&self, now: f64) -> bool {
        self.ends > now
    }
}

/// The most voices one source sounds at once (every channel and none):
/// a player's gun, steps, voice and reload together stay under it.
pub const MAX_VOICES_PER_SOURCE: usize = 8;

/// The most copies of one entry sounding at once among sounds without a
/// source (impacts, debris).
pub const MAX_VOICES_PER_ENTRY: usize = 4;

/// Fade out a playing sound (replaced, or over a voice limit).
fn fade(commands: &mut Commands, stopped: &mut Vec<Entity>, e: Entity, p: &Playing) {
    p.gains.fade_out();
    stopped.push(e);
    // The audio backend may despawn a finished one first.
    if let Ok(mut e) = commands.get_entity(e) {
        e.try_remove::<Playing>().try_insert(FadingOut);
    }
}

/// A sound another replaced on its channel: fading out, then ended
/// (`live_sound::Gains::fade_out`).
#[derive(Component)]
pub struct FadingOut;

/// Per-ear gains for a source in `dir` (unit, listener to source) given the
/// listener's right vector: full in the near ear, fading in the far one.
pub fn pan(dir: Vec3, right: Vec3) -> (f32, f32) {
    let d = dir.dot(right).clamp(-1.0, 1.0);
    ((1.0 - d).min(1.0), (1.0 + d).min(1.0))
}

pub struct SoundPlugin;

impl Plugin for SoundPlugin {
    fn build(&self, app: &mut App) {
        // A map unloaded: the soundscape and the long-lived sounds stop.
        app.init_resource::<MapHooks>();
        app.world_mut()
            .resource_mut::<MapHooks>()
            .unload
            .extend([super::soundscape::reset as fn(&mut World), super::live_sound::reset]);
        app.add_message::<PlaySound>()
            .add_systems(PostUpdate, play_sounds.run_if(resource_exists::<Assets<LiveClip>>))
            .add_plugins((
                super::soundscape::SoundscapePlugin,
                super::live_sound::LiveSoundPlugin,
                super::hearing::HearingPlugin,
                super::room::RoomPlugin,
            ));
    }
}

#[allow(clippy::too_many_arguments)]
fn play_sounds(
    mut commands: Commands,
    mut messages: MessageReader<PlaySound>,
    bank: Option<Res<SoundBank>>,
    listeners: Query<(&GlobalTransform, Option<&ChildOf>), With<SoundListener>>,
    playing: Query<(Entity, &Playing)>,
    mut sources: ResMut<Assets<LiveClip>>,
    hearing: Res<Hearing>,
    room: Res<RoomDsp>,
    mix: Option<Res<super::live_sound::SoundMix>>,
    muted: Option<Res<super::live_sound::MutedSounds>>,
    time: Res<Time>,
    mut seed: Local<u64>,
) {
    let now = time.elapsed_secs_f64();
    let mix = mix.map_or_else(Default::default, |m| *m);
    let Some(bank) = bank else {
        messages.clear();
        return;
    };
    let listener = Ear::find(listeners.iter());
    let mut unit = || {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (*seed >> 40) as f32 / (1u64 << 24) as f32
    };
    // Sounds stopped this run, and the ones started this run by source and
    // channel: several messages for one source's channel in a frame (a
    // network client gets a burst after a stall) replace each other, and
    // each old sound is stopped once (it was despawned twice: a warning
    // each time, hundreds in a 17-minute live soak).
    let mut stopped: Vec<Entity> = Vec::new();
    let mut started: Vec<((Entity, u8), Entity)> = Vec::new();
    // Every sound started this run (source, entry), for the voice limits.
    let mut fresh: Vec<(Entity, Option<Entity>, String)> = Vec::new();
    for m in messages.read() {
        if muted.as_ref().is_some_and(|x| x.0.iter().any(|e| *e == m.entry)) {
            continue;
        }
        let Some(entry) = bank.0.entry(&m.entry) else {
            continue;
        };
        if entry.waves.is_empty() {
            continue;
        }
        let wave = entry.waves[((unit() * entry.waves.len() as f32) as usize).min(entry.waves.len() - 1)];
        let clip = &bank.0.clips[wave];
        let volume = m.volume.unwrap_or_else(|| entry.volume.draw(unit())).clamp(0.0, 1.0) * mix.gain(&m.entry);
        let drawn = entry.pitch.draw(unit());
        let pitch = m.pitch.unwrap_or(drawn).round().clamp(1.0, 255.0);
        let level = match entry.level {
            SoundLevel::Db(l) => SoundLevel::Db(l.round()),
            a => a,
        };
        // Unspatialized sounds (interface, announcer) stay out of the room.
        let (left, right, send) = match (m.at, listener) {
            (Some(at), Some(ear)) => {
                let (l, r, send) = ear.gains(at, m.source, level);
                (l * volume, r * volume, send)
            }
            _ => (volume, volume, 0.0),
        };
        // A level's distance low-pass (none for the listener's own sounds).
        let cutoff = match (m.at, listener) {
            (Some(at), Some(ear)) if m.source.is_none() || m.source != ear.owner => {
                distance_cutoff(level, at.distance(ear.at.translation()))
            }
            _ => 1.0,
        };
        let send = if entry.dry { 0.0 } else { send };
        if left.max(right) < 1e-3 {
            continue;
        }
        // Replace whatever this source plays on this channel: it fades out
        // over a few milliseconds and ends (a hard stop clicks: a
        // full-auto gun's shots each cut the last one off), then the
        // backend despawns it; it no longer counts as on the channel.
        if let (Some(source), Some(channel)) = (m.source, m.channel) {
            for (e, p) in &playing {
                if p.source == Some(source) && p.channel == Some(channel) && !stopped.contains(&e) {
                    fade(&mut commands, &mut stopped, e, p);
                }
            }
            if let Some(i) = started.iter().position(|(k, _)| *k == (source, channel)) {
                let e = started.swap_remove(i).1;
                fresh.retain(|f| f.0 != e);
                commands.entity(e).despawn();
            }
        }
        // The voice limits: the oldest of the source's (or of this
        // entry's, without a source) go first.
        let same = |source: Option<Entity>, entry: &str| match m.source {
            Some(s) => source == Some(s),
            None => source.is_none() && entry.eq_ignore_ascii_case(&m.entry),
        };
        let limit = if m.source.is_some() { MAX_VOICES_PER_SOURCE } else { MAX_VOICES_PER_ENTRY };
        let mut old: Vec<(Entity, &Playing)> = playing
            .iter()
            .filter(|(e, p)| !stopped.contains(e) && p.sounding(now) && same(p.source, &p.entry))
            .collect();
        old.sort_by(|a, b| a.1.started.total_cmp(&b.1.started));
        let mut over = (old.len() + fresh.iter().filter(|f| same(f.1, &f.2)).count() + 1).saturating_sub(limit);
        for (e, p) in old {
            if over == 0 {
                break;
            }
            fade(&mut commands, &mut stopped, e, p);
            over -= 1;
        }
        while over > 0 {
            let Some(i) = fresh.iter().position(|f| same(f.1, &f.2)) else { break };
            let e = fresh.remove(i).0;
            started.retain(|s| s.1 != e);
            commands.entity(e).despawn();
            over -= 1;
        }
        let once = MapSoundClip {
            loop_start: None,
            ..clip.clone()
        };
        let frames = clip.samples.len() / clip.channels.max(1) as usize;
        let length = frames as f64 / clip.rate.max(1) as f64 / (pitch as f64 / 100.0);
        let gains = Arc::new(Gains::new(left, right).with_send(send).with_cutoff(cutoff));
        let handle = sources.add(LiveClip::new(once, gains.clone(), hearing.mix.clone()).with_room(&room));
        let e = commands
            .spawn((
                AudioPlayer(handle),
                PlaybackSettings::DESPAWN
                    .with_volume(Volume::Linear(1.0))
                    .with_speed(pitch / 100.0),
                Playing {
                    source: m.source,
                    channel: m.channel,
                    gains,
                    entry: m.entry.clone(),
                    started: now,
                    ends: now + length,
                },
            ))
            .id();
        fresh.push((e, m.source, m.entry.clone()));
        if let (Some(source), Some(channel)) = (m.source, m.channel) {
            started.push(((source, channel), e));
        }
    }
}

/// A clip as a 16-bit stereo PCM WAV, each channel scaled (for panning and
/// distance), for an audio backend that reads WAV.
pub fn stereo_wav(clip: &MapSoundClip, left: f32, right: f32) -> Vec<u8> {
    let frames = clip.samples.len() / clip.channels as usize;
    let mut out = Vec::with_capacity(44 + frames * 4);
    let data_len = (frames * 4) as u32;
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(clip.rate.to_le_bytes());
    out.extend((clip.rate * 4).to_le_bytes());
    out.extend(4u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    let scale = |s: i16, g: f32| ((s as f32 * g).clamp(-32768.0, 32767.0) as i16).to_le_bytes();
    for f in 0..frames {
        let (l, r) = if clip.channels >= 2 {
            (clip.samples[f * 2], clip.samples[f * 2 + 1])
        } else {
            (clip.samples[f], clip.samples[f])
        };
        out.extend(scale(l, left));
        out.extend(scale(r, right));
    }
    out
}
