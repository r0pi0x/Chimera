//! Halo's looping sounds played on the engine's sound seams
//! (specs/halo_ce/sounds.md, "Looping sounds", "Gain", "Distance",
//! "Channels, limits, preemption"): owners ask for a loop each frame
//! (`LoopRequest`: a key of theirs, the looping sound's tag path, where,
//! the scale, the alternate state); `play` keeps each loop's state
//! (`Loops`) and turns it into the engine's long-lived sounds
//! (`SoundControl` under keys of its own, `LOOP_KEYS`) and one-shots
//! (`PlaySound`: start, end and detail sounds).
//!
//! The rules, per loop and track:
//! - **Start**: the track's start sound plays as a one-shot; with "fade in
//!   at start" the loop sound starts at once fading in linearly over the
//!   fade in duration, else it follows the start sound (it starts when the
//!   start sound's mean duration has passed; at once without one).
//! - **Loop**: the loop sound's loop entry (every chain, `library`) plays
//!   looping; with the looping sound's "fake impulse" flag it plays once
//!   and the loop ends.
//! - **Alternate**: a change of the request's alternate flag switches a
//!   track with an alternate loop to it: with "fade in alternate" the new
//!   sound fades in while the old fades out, linearly over the fade out
//!   duration; else the voice switches at once.
//! - **Stop**: a loop not asked for this frame stops: with "fade out at
//!   stop", or no end sound, the loop fades out linearly over the fade out
//!   duration (and the end sound, if any, plays as a one-shot); with an end
//!   sound and no fade, the end sound plays and the loop stops.
//! - **Gain** per voice: `track gain x lerp(zero gain, one gain, scale) x
//!   gain modifier x mean permutation gain x fade x audibility`, clamped to
//!   1 (class gain and duck are 1: no scripted dialogue in multiplayer);
//!   **pitch** `lerp(zero pitch, one pitch, scale) x the voice's draw from
//!   the random pitch bounds x the range's playback rate`.
//! - **Distance**: a mono loop with a position is positional on the
//!   engine's inverse law (`fx::sound::level` of its minimum distance);
//!   its maximum distance is the audibility cut-off: beyond it the loop
//!   fades to 0 over 2 s and its voices end, back within it they start
//!   again fading in over 0.5 s (a loop is created only within its range).
//!   A stereo loop is never panned: unspatialized, with the linear fade
//!   `1 - (d - min) / (max - min)` when it has a position. An
//!   unspatialized loop (the background sound) has no attenuation.
//! - **Details**: each detail's first play is `period x uniform(period
//!   bounds)` after the loop is created (the period multiplier `lerp(lower,
//!   upper, scale)`), the next `that + the detail sound's stored longest
//!   length` after each play, while the loop is not stopping and its flags
//!   allow (alternate): a one-shot at the loop's position plus an offset
//!   drawn from the distance, yaw and pitch bounds, or, for an
//!   unspatialized loop, at that offset from the listener (no offset:
//!   unspatialized), with the detail's gain.
//! - **Limits**: at most the class's per-definition limit of one looping
//!   sound's loops are heard, the nearest; the others are silent as if out
//!   of range.
//! - A class whose wet gain is 1 (ambience, music) plays dry (no room
//!   send).
//!
//! All of it is presentation: real time (`Time`) and a generator of its
//! own, never the simulation's; every machine plays what its own owners
//! ask for. The listener is the local player (`LocalPlayer`); with none
//! (a dedicated server) every loop counts as at the listener.
//!
//! Known shortcuts: the start sound's chaining is by its mean duration,
//! an end sound without a fade stops the loop at once (not at the end of
//! its current chunk), and an alternate switch without a fade is at once;
//! the maximum bend per second and the pitch ranges of engines are not
//! applied (vehicles); a loop's continuous damage effect is not played
//! (none on the multiplayer maps); preemption by age and distance is the
//! nearest-first rule above.

use std::collections::BTreeMap;

use bevy::prelude::*;

use super::library::{ActiveAudio, AudioLibrary, HaloLoop, SoundInfo};
use crate::map::{PlaySound, SoundControl, SoundKey, StartSound};

/// The engine keys of this player's voices (bit 61; the logic's are bit
/// 63, the soundscape's 62).
pub const LOOP_KEYS: u64 = 1 << 61;

/// The looping sound flag: the sounds play once (`HaloLoop::flags`).
pub const FAKE_IMPULSE: u32 = blam_formats::sound::looping::FLAG_FAKE_IMPULSE;
use blam_formats::sound::looping::{TRACK_FADE_IN_ALTERNATE, TRACK_FADE_IN_AT_START, TRACK_FADE_OUT_AT_STOP};

/// The fade out when a loop leaves its range, and back in, seconds.
pub const INAUDIBLE_FADE_OUT: f32 = 2.0;
pub const INAUDIBLE_FADE_IN: f32 = 0.5;

/// Where a loop plays.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LoopSource {
    /// No position (the background sound).
    Unspatialized,
    /// A point, engine space.
    At(Vec3),
    /// An entity's position (its `GlobalTransform`, else `Transform`), and
    /// where it is now.
    Follow(Entity, Vec3),
}

/// An owner's request for one loop this frame (module doc).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct LoopRequest {
    /// The owner's key for this loop (unique among owners: see `owner_key`).
    pub key: u64,
    /// The looping sound's tag path (lower case).
    pub sound: String,
    pub source: LoopSource,
    /// 0..1.
    pub scale: f32,
    pub alternate: bool,
}

/// A key for an owner kind (a small number) and an index of its own.
pub fn owner_key(kind: u8, index: u64) -> u64 {
    (u64::from(kind) << 48) | (index & ((1 << 48) - 1))
}

/// One voice of a track.
#[derive(Clone, Debug, PartialEq)]
pub struct Voice {
    pub track: usize,
    /// The sound path it loops.
    pub sound: String,
    /// Its engine key, once started.
    pub key: Option<u64>,
    /// Seconds before it starts (following a start sound).
    pub wait: f32,
    pub fade: f32,
    /// Fade change per second (+ in, - out).
    pub rate: f32,
    /// The draw from the random pitch bounds.
    pub draw: f32,
    /// Seconds left of a fake impulse's one pass.
    pub once: Option<f32>,
    /// Ends when its fade reaches 0.
    pub ending: bool,
}

/// One loop's state.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopState {
    pub sound: String,
    pub source: LoopSource,
    pub scale: f32,
    pub alternate: bool,
    pub asked: bool,
    pub stopping: bool,
    /// Started (its start sounds played).
    pub started: bool,
    pub voices: Vec<Voice>,
    /// Seconds until each detail plays.
    pub details: Vec<f32>,
    /// The audibility fade (out of range: toward 0).
    pub audible: f32,
    /// Within range and among the nearest of its sound.
    pub heard: bool,
    pub distance: f32,
}

/// Every loop by its owner's key, and the generator.
#[derive(Resource, Clone, Debug)]
pub struct Loops {
    pub loops: BTreeMap<u64, LoopState>,
    pub rng: u64,
    next_key: u64,
}

impl Default for Loops {
    fn default() -> Self {
        Self { loops: BTreeMap::new(), rng: 0x9E37_79B9_7F4A_7C15, next_key: 0 }
    }
}

impl Loops {
    fn key(&mut self) -> u64 {
        self.next_key += 1;
        LOOP_KEYS | (self.next_key & ((1 << 48) - 1))
    }
}

/// The listener's position (engine space), if any.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct Listener(pub Option<Vec3>);

/// The listener: the local player's eye (its transform; a test may set
/// it).
pub fn listen(players: Query<&Transform, With<crate::core::LocalPlayer>>, mut listener: ResMut<Listener>) {
    if let Some(t) = players.iter().next() {
        listener.0 = Some(t.translation);
    }
}

/// The stereo linear fade (sounds.md "Distance"; a maximum not above the
/// minimum: 1 within it, 0 beyond).
pub fn stereo_fade(min: f32, max: f32, d: f32) -> f32 {
    if max <= min {
        return if d <= min { 1.0 } else { 0.0 };
    }
    (1.0 - (d - min) / (max - min)).clamp(0.0, 1.0)
}

/// Whether a loop of `sound` plays positionally (mono with a position).
fn positional(info: &SoundInfo, source: LoopSource) -> bool {
    !info.stereo && source != LoopSource::Unspatialized
}

fn position(source: LoopSource) -> Option<Vec3> {
    match source {
        LoopSource::Unspatialized => None,
        LoopSource::At(p) | LoopSource::Follow(_, p) => Some(p),
    }
}

/// A one-shot of `sound` (an impulse entry) with `gain` x its own gain at
/// `scale`, at `at`: None when the scale test drops it.
pub fn one_shot(lib: &AudioLibrary, sound: &str, gain: f32, scale: f32, at: Option<Vec3>, draw: f32) -> Option<PlaySound> {
    let info = lib.sounds.get(sound)?;
    if scale <= 0.0 && info.zero_gain <= 0.0 {
        return None;
    }
    let g = (gain * info.gain * info.scale_gain(scale)).clamp(0.0, 1.0);
    let pitch = info.scale_pitch(scale) * (info.pitch[0] + (info.pitch[1] - info.pitch[0]) * draw) * info.rate * 100.0;
    let at = if info.stereo { None } else { at };
    Some(PlaySound {
        entry: info.entry.clone(),
        at,
        volume: Some(g),
        pitch: Some(pitch),
        source: None,
        channel: None,
    })
}

/// A voice's gain and pitch (percent) now (module doc).
pub fn voice_gain(lib: &AudioLibrary, def: &HaloLoop, l: &LoopState, v: &Voice) -> Option<(f32, f32)> {
    let track = def.tracks.get(v.track)?;
    let info = lib.sounds.get(&v.sound)?;
    let mut g = track.gain * info.scale_gain(l.scale) * info.gain * v.fade * l.audible;
    if info.stereo && position(l.source).is_some() {
        g *= stereo_fade(info.min, info.max, l.distance);
    }
    let pitch = info.scale_pitch(l.scale) * v.draw * info.rate * 100.0;
    Some((g.clamp(0.0, 1.0), pitch))
}

/// The per-frame step (module doc): requests in, the engine's controls and
/// one-shots out.
pub fn step(
    loops: &mut Loops,
    lib: &AudioLibrary,
    requests: impl IntoIterator<Item = LoopRequest>,
    listener: Option<Vec3>,
    dt: f32,
    controls: &mut Vec<SoundControl>,
    shots: &mut Vec<PlaySound>,
) {
    for l in loops.loops.values_mut() {
        l.asked = false;
    }
    for r in requests {
        let Some(def) = lib.loops.get(&r.sound) else { continue };
        let fresh = !loops.loops.get(&r.key).is_some_and(|l| l.sound == r.sound && !l.stopping);
        if fresh {
            if let Some(old) = loops.loops.remove(&r.key) {
                // Another sound under the same key: the old one stops.
                let mut old = old;
                let empty = HaloLoop::default();
                stop_loop(lib, lib.loops.get(&old.sound).unwrap_or(&empty), &mut old, shots);
                if !old.voices.is_empty() {
                    loops.loops.insert(r.key | (1 << 47), old);
                }
            }
            let mut details = Vec::new();
            for d in &def.details {
                let k = blam_formats::sound::gain::lerp(def.period_scale[0], def.period_scale[1], r.scale);
                details.push(k * uniform(&mut loops.rng, d.period));
            }
            loops.loops.insert(r.key, LoopState {
                sound: r.sound.clone(),
                source: r.source,
                scale: r.scale,
                alternate: r.alternate,
                asked: true,
                stopping: false,
                started: false,
                voices: Vec::new(),
                details,
                audible: 1.0,
                heard: true,
                distance: 0.0,
            });
            continue;
        }
        let l = loops.loops.get_mut(&r.key).expect("present");
        l.asked = true;
        l.source = r.source;
        l.scale = r.scale;
        if l.alternate != r.alternate {
            l.alternate = r.alternate;
            let changes: Vec<(usize, String, bool)> = def
                .tracks
                .iter()
                .enumerate()
                .filter(|(_, t)| !t.alternate_loop.is_empty())
                .map(|(i, t)| (i, if r.alternate { t.alternate_loop.clone() } else { t.looped.clone() }, t.flags & TRACK_FADE_IN_ALTERNATE != 0))
                .collect();
            let mut new = Vec::new();
            for (track, sound, fade) in changes {
                let d = def.tracks[track].fade_out.max(1e-3);
                for v in l.voices.iter_mut().filter(|v| v.track == track && !v.ending) {
                    v.ending = true;
                    v.rate = if fade { -1.0 / d } else { -1e9 };
                }
                new.push((track, sound, fade, d));
            }
            for (track, sound, fade, d) in new {
                let v = new_voice_for(&mut loops.rng, lib, track, &sound, if fade { 0.0 } else { 1.0 }, if fade { 1.0 / d } else { 0.0 });
                loops.loops.get_mut(&r.key).expect("present").voices.push(v);
            }
        }
    }
    // Distances, and who is heard: within range, the nearest of each sound
    // up to its class's per-definition limit.
    for l in loops.loops.values_mut() {
        if let LoopSource::Follow(_, p) | LoopSource::At(p) = l.source {
            l.distance = listener.map_or(0.0, |e| e.distance(p) / super::super::METERS_PER_UNIT);
        } else {
            l.distance = 0.0;
        }
    }
    let mut by_sound: BTreeMap<&str, Vec<(f32, u64)>> = BTreeMap::new();
    for (k, l) in &loops.loops {
        by_sound.entry(l.sound.as_str()).or_default().push((l.distance, *k));
    }
    let mut heard: BTreeMap<u64, bool> = BTreeMap::new();
    for (sound, mut list) in by_sound {
        let Some(def) = lib.loops.get(sound) else { continue };
        let limit = def
            .tracks
            .iter()
            .filter_map(|t| lib.sounds.get(&t.looped))
            .filter_map(|i| i.class())
            .map(|c| usize::from(c.defaults().per_definition_limit))
            .min()
            .unwrap_or(4);
        list.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (i, (d, k)) in list.into_iter().enumerate() {
            let in_range = def.max_distance <= 0.0 || d <= def.max_distance || loops.loops[&k].source == LoopSource::Unspatialized;
            heard.insert(k, in_range && i < limit);
        }
    }
    let keys: Vec<u64> = loops.loops.keys().copied().collect();
    for k in keys {
        let mut l = loops.loops.remove(&k).expect("present");
        let Some(def) = lib.loops.get(&l.sound) else { continue };
        l.heard = heard.get(&k).copied().unwrap_or(false);
        if !l.asked && !l.stopping {
            stop_loop(lib, def, &mut l, shots);
        }
        // Audibility.
        if l.heard {
            l.audible = (l.audible + dt / INAUDIBLE_FADE_IN).min(1.0);
        } else {
            l.audible = (l.audible - dt / INAUDIBLE_FADE_OUT).max(0.0);
        }
        // Created only when heard; started once.
        if !l.started && l.heard && !l.stopping {
            l.started = true;
            l.audible = 1.0;
            start_loop(&mut loops.rng, lib, def, &mut l, shots);
        } else if l.started && l.heard && !l.stopping && l.voices.is_empty() && def.flags & FAKE_IMPULSE == 0 {
            // Back in range: the loops again, fading in.
            l.audible = 0.0;
            for (i, t) in def.tracks.iter().enumerate() {
                let sound = if l.alternate && !t.alternate_loop.is_empty() { &t.alternate_loop } else { &t.looped };
                if !sound.is_empty() {
                    let v = new_voice_for(&mut loops.rng, lib, i, sound, 1.0, 0.0);
                    l.voices.push(v);
                }
            }
        }
        // Details.
        if !l.stopping && l.heard {
            for (i, d) in def.details.iter().enumerate() {
                l.details[i] -= dt;
                if l.details[i] > 0.0 {
                    continue;
                }
                let per = blam_formats::sound::gain::lerp(def.period_scale[0], def.period_scale[1], l.scale);
                let longest = lib.sounds.get(&d.sound).map_or(0.0, |s| s.longest_ms as f32 / 1000.0);
                l.details[i] = per * uniform(&mut loops.rng, d.period) + longest;
                let allowed = !(d.flags & 1 != 0 && l.alternate) && !(d.flags & 2 != 0 && !l.alternate);
                if !allowed {
                    continue;
                }
                let dist = uniform(&mut loops.rng, d.distance);
                let offset = if dist > 0.0 {
                    let (yaw, pitch) = (uniform(&mut loops.rng, d.yaw), uniform(&mut loops.rng, d.pitch));
                    let h = super::super::METERS_PER_UNIT * dist;
                    Some(Vec3::new(yaw.cos() * pitch.cos(), pitch.sin(), -yaw.sin() * pitch.cos()) * h)
                } else {
                    None
                };
                let at = match (position(l.source), offset) {
                    (Some(p), o) => Some(p + o.unwrap_or(Vec3::ZERO)),
                    (None, Some(o)) => listener.map(|e| e + o),
                    (None, None) => None,
                };
                let draw = unit(&mut loops.rng);
                if let Some(s) = one_shot(lib, &d.sound, d.gain, l.scale, at, draw) {
                    shots.push(s);
                }
            }
        }
        // Voices.
        let mut keep = Vec::new();
        for mut v in std::mem::take(&mut l.voices) {
            v.fade = (v.fade + v.rate * dt).clamp(0.0, 1.0);
            if v.wait > 0.0 {
                v.wait -= dt;
                if v.wait > 0.0 {
                    if !(v.ending && v.fade <= 0.0) {
                        keep.push(v);
                    }
                    continue;
                }
            }
            if let Some(left) = &mut v.once {
                *left -= dt;
                if *left <= 0.0 {
                    v.ending = true;
                    v.fade = 0.0;
                }
            }
            let gone = (v.ending && v.fade <= 0.0) || (l.audible <= 0.0 && !l.heard);
            if gone {
                if let Some(key) = v.key {
                    controls.push(SoundControl::Stop(SoundKey(key)));
                }
                continue;
            }
            let Some((gain, pitch)) = voice_gain(lib, def, &l, &v) else { continue };
            match v.key {
                Some(key) => controls.push(SoundControl::Change { key: SoundKey(key), volume: Some(gain), pitch: Some(pitch) }),
                None => {
                    let Some(info) = lib.sounds.get(&v.sound) else { continue };
                    let entry = if info.loop_entry.is_empty() { info.entry.clone() } else { info.loop_entry.clone() };
                    let key = loops.key();
                    v.key = Some(key);
                    let once = def.flags & FAKE_IMPULSE != 0;
                    if once {
                        v.once = Some(info.loop_seconds.max(info.seconds));
                    }
                    let spatial = positional(info, l.source);
                    controls.push(SoundControl::Start(StartSound {
                        key: SoundKey(key),
                        entry,
                        at: if spatial { position(l.source) } else { None },
                        follow: match l.source {
                            LoopSource::Follow(e, _) if spatial => Some(e),
                            _ => None,
                        },
                        volume: Some(gain),
                        pitch: Some(pitch),
                        level: Some(super::super::fx::sound::level(info.min)),
                        clip: None,
                        looping: !once,
                        dry: info.dry(),
                    }));
                }
            }
            keep.push(v);
        }
        l.voices = keep;
        // A fake impulse that played stays, silent, while it is asked for.
        let done = l.stopping && l.voices.is_empty();
        if !done {
            loops.loops.insert(k, l);
        }
    }
}

fn unit(rng: &mut u64) -> f32 {
    *rng ^= *rng << 13;
    *rng ^= *rng >> 7;
    *rng ^= *rng << 17;
    (*rng >> 40) as f32 / (1u64 << 24) as f32
}

fn uniform(rng: &mut u64, b: [f32; 2]) -> f32 {
    b[0] + (b[1] - b[0]) * unit(rng)
}

fn new_voice_for(rng: &mut u64, lib: &AudioLibrary, track: usize, sound: &str, fade: f32, rate: f32) -> Voice {
    let bounds = lib.sounds.get(sound).map_or([1.0, 1.0], |i| i.pitch);
    Voice { track, sound: sound.into(), key: None, wait: 0.0, fade, rate, draw: uniform(rng, bounds), once: None, ending: false }
}

/// A loop's start (module doc, "Start").
fn start_loop(rng: &mut u64, lib: &AudioLibrary, def: &HaloLoop, l: &mut LoopState, shots: &mut Vec<PlaySound>) {
    for (i, t) in def.tracks.iter().enumerate() {
        let mut wait = 0.0;
        if !t.start.is_empty() {
            let draw = unit(rng);
            if let Some(s) = one_shot(lib, &t.start, t.gain, l.scale, position(l.source), draw) {
                shots.push(s);
            }
            if t.flags & TRACK_FADE_IN_AT_START == 0 {
                wait = lib.sounds.get(&t.start).map_or(0.0, |s| s.seconds);
            }
        }
        let sound = if l.alternate && !t.alternate_loop.is_empty() { &t.alternate_loop } else { &t.looped };
        if sound.is_empty() {
            continue;
        }
        let (fade, rate) = if t.flags & TRACK_FADE_IN_AT_START != 0 && t.fade_in > 0.0 { (0.0, 1.0 / t.fade_in) } else { (1.0, 0.0) };
        let mut v = new_voice_for(rng, lib, i, sound, fade, rate);
        v.wait = wait;
        l.voices.push(v);
    }
}

/// A loop's stop (module doc, "Stop").
fn stop_loop(lib: &AudioLibrary, def: &HaloLoop, l: &mut LoopState, shots: &mut Vec<PlaySound>) {
    l.stopping = true;
    for (i, t) in def.tracks.iter().enumerate() {
        let end = if l.alternate && !t.alternate_end.is_empty() { &t.alternate_end } else { &t.end };
        let has_voice = l.voices.iter().any(|v| v.track == i && v.key.is_some());
        if !end.is_empty()
            && has_voice
            && l.heard
            && let Some(s) = one_shot(lib, end, t.gain, l.scale, position(l.source), 0.5)
        {
            shots.push(s);
        }
        let fade = t.flags & TRACK_FADE_OUT_AT_STOP != 0 || end.is_empty();
        for v in l.voices.iter_mut().filter(|v| v.track == i) {
            v.ending = true;
            v.rate = if fade && t.fade_out > 0.0 && v.key.is_some() { -1.0 / t.fade_out } else { -1e9 };
        }
    }
}

/// The system: this frame's requests and the time since the last frame.
pub fn play(
    mut requests: MessageReader<LoopRequest>,
    mut loops: ResMut<Loops>,
    active: Res<ActiveAudio>,
    listener: Res<Listener>,
    time: Res<Time>,
    mut controls: MessageWriter<SoundControl>,
    mut sounds: MessageWriter<PlaySound>,
) {
    let asked: Vec<LoopRequest> = requests.read().cloned().collect();
    let Some(lib) = active.0.clone() else {
        // No library (no map yet): every voice stops.
        for (_, l) in std::mem::take(&mut loops.loops) {
            for v in l.voices.into_iter().filter_map(|v| v.key) {
                controls.write(SoundControl::Stop(SoundKey(v)));
            }
        }
        return;
    };
    let (mut c, mut s) = (Vec::new(), Vec::new());
    step(&mut loops, &lib, asked, listener.0, time.delta_secs(), &mut c, &mut s);
    controls.write_batch(c);
    sounds.write_batch(s);
}
