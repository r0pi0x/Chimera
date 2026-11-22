//! Sound playback for any game (docs/plans/active/sound.md): playback
//! resolves a `map::PlaySound`'s entry (random wave, drawn volume, pitch
//! and level), applies the distance model (and a `Radius` level's
//! distance low-pass, `Gains::with_cutoff`) and left/right panning toward
//! the listener, and plays it, pitch as playback speed. A new sound on the
//! same (source entity, channel) replaces the one playing there; a
//! `map::StopSound` fades out a source's sound by its entry, and a
//! `map::FadeSound` ramps its volume in a straight line (`VolumeRamp`,
//! stepped each frame by `ramp_volumes`; one asked for a sound not yet
//! playing waits up to `FADE_WAIT` for it to start). A clip with
//! a loop point of its own (its WAV's `cue ` or `smpl` chunk: `loops`)
//! plays to its end, then from that point again, seamlessly, until
//! stopped or replaced; any other plays once (specs/source/
//! sound_looping.md: nothing but the file makes a sound loop).
//!
//! Voice limits (ours, for the mix: every voice is summed and the output
//! clips past full scale): a source sounds at most
//! `MAX_VOICES_PER_SOURCE` voices at once and a sound without a source
//! at most `MAX_VOICES_PER_ENTRY` copies of one entry; a new one past the
//! limit fades out the oldest.
//!
//! The mix's limit (`limit_mix`): when the one-shot voices' peaks (each
//! clip's own, times its gains, times the master volume) would sum past
//! full scale, every clip is turned down to keep them under it, coming
//! back over a quarter second (`live_sound::MixLimit`). TF2's rocket and
//! its blast (full-scale stereo waves at gain 1) clipped the output for
//! 1-2 s at `volume` 1, a moment at 0.5; ours.
//!
//! An entry with Source's hearing (`map::hearing::EntryHearing`, every
//! Source game's: specs/source/sound_hearing.md) is heard by Source's
//! rules (`Ear::hear`): the measured distance curve, one wall trace from
//! the ear against the static map when it starts (`obscured`, × 0.393
//! when blocked; never for the listener's own), the mixer's value (a
//! player's or the world's), the stereo-speaker pan law, a stereo wave
//! folded to mono (a distance-variant one blended near to far) and the
//! room's wet share by distance (`room_mix`); the listener's own sounds
//! play centred, as stereo, at full level. Other games' entries keep the
//! even pan (`pan`) and no wall loss.
//!
//! Known shortcuts (docs/tech-debt.md): a one-shot's wall trace is taken
//! once, when it starts (Source defers it a frame and redoes it, at a rate
//! the spec leaves open, Q2); only the static map blocks (Q3: brush
//! entities, props and water untested); the dry signal keeps its level
//! whatever the wet share (the room's presets' levels are ours,
//! unmeasured). Why it's OK for now: one-shots are short; long-lived
//! sounds are re-traced (`live_sound::RETRACE`).
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
    FadeSound, MapHooks, MapSoundClip, PlaySound, SoundLevel, StopSound,
    hearing as hearing_rules,
    particles::{WorldTrace, WorldTracer},
    sound::{EntryHearing, SOURCE_UNIT, SoundBank, distance_cutoff, distance_gain, ramp_volume},
};

/// A screen stands in for the match (the client's `staging::WorldCover`:
/// the pre-match lobby, a loading screen): the world is not heard, as it is
/// not seen (`held`).
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct WorldUnheard;

/// Whether the game's sounds are held: while this machine's map loads
/// (`map::LocalMapLoad`: its spawn and settling behind the loading screen
/// or the lobby) or a screen stands in for the match (`WorldUnheard`).
/// Meanwhile a sound asked for (`PlaySound`) is dropped and the long-lived
/// ones (`live_sound`: ambience, soundscapes, loops) sound at nothing (but
/// music), going on where they are once it ends. Interface sounds and the
/// announcer play on their own way and are never held.
pub fn held(local: Option<&crate::map::LocalMapLoad>, unheard: Option<&WorldUnheard>) -> bool {
    local.is_some() || unheard.is_some()
}

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
    /// centred at full gain, as Source plays the local player's
    /// own sounds: they are emitted from the simulation's latest tick
    /// while the camera is drawn between the last two ticks
    /// (`map::interp`), so while strafing a shot sat a few centimetres to
    /// the side of the ear, and `pan` of that direction is a hard pan.
    /// Other sounds within `NEAR_PAN` of the ear blend toward the
    /// centre for the same reason (a spectated player's eye).
    pub fn gains(&self, at: Vec3, source: Option<Entity>, level: SoundLevel) -> (f32, f32, f32) {
        if source.is_some() && source == self.owner {
            return (1.0, 1.0, distance_send(0.0));
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

/// What the listener hears of a sound by Source's rules (`Ear::hear`):
/// per-ear gains, the room's share and how a stereo clip folds
/// (`live_sound::Gains::set_fold`; None: played as stereo).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Heard {
    pub left: f32,
    pub right: f32,
    pub send: f32,
    pub fold: Option<f32>,
}

impl Ear {
    /// A sound at `at` made by `source` with an entry heard by Source's
    /// rules (specs/source/sound_hearing.md, "Per-frame order"): the
    /// listener's own centred at full level, as stereo, with no distance,
    /// wall or mixer; a level-0 one centred at 0.89 per ear; any other
    /// by the distance curve, times the wall loss when `blocked`, times the
    /// mixer's value (a player's or the world's), panned by the
    /// stereo-speaker law (a stereo wave folded to mono, a distance-variant
    /// one blended near to far). The room gets the wet share of `room_mix`
    /// (the room's range, `hearing::dsp_wet`).
    pub fn hear(&self, at: Vec3, source: Option<Entity>, level: SoundLevel, rules: &EntryHearing, player: bool, blocked: bool, room_mix: (f32, f32)) -> Heard {
        let l = hearing_rules::level_of(level);
        if source.is_some() && source == self.owner {
            return Heard { left: 1.0, right: 1.0, send: hearing_rules::dsp_wet(room_mix, 0.0, l), fold: None };
        }
        let to = at - self.at.translation();
        let units = to.length() / SOURCE_UNIT;
        let mix = rules.mix(player);
        let send = hearing_rules::dsp_wet(room_mix, units, l);
        let fold = Some(if rules.near_far { hearing_rules::far_weight(units) } else { 0.5 });
        if matches!(level, SoundLevel::Db(l) if l <= 0.0) {
            let g = hearing_rules::UNSPATIAL_LEVEL * mix;
            return Heard { left: g, right: g, send, fold };
        }
        let wall = if blocked { rules.obscured } else { 1.0 };
        let g = distance_gain(level, to.length()) * wall * mix;
        let local = self.at.rotation().inverse() * to.normalize_or_zero();
        let (pl, pr) = hearing_rules::speaker_pan(local.to_array());
        // Within a head's width of the ear, toward centred.
        let near = (to.length() / NEAR_PAN).clamp(0.0, 1.0);
        let centre = (pl + pr) * 0.5;
        let (pl, pr) = (centre + (pl - centre) * near, centre + (pr - centre) * near);
        Heard { left: g * pl, right: g * pr, send, fold }
    }
}

/// Whether the world blocks the line from the ear at `ear` to a sound at
/// `at` (the static map only, `map::particles::WorldTracer`): a hit short
/// of the sound by more than `OBSCURE_SLACK` (a sound on a surface, a
/// footstep on the floor, isn't behind it).
pub fn obscured(world: &impl WorldTrace, ear: Vec3, at: Vec3) -> bool {
    let len = ear.distance(at);
    len > OBSCURE_SLACK && world.trace(ear, at).is_some_and(|(f, _)| f * len < len - OBSCURE_SLACK)
}

/// How far short of a sound a wall must stop the line to block it
/// (metres, 2 units).
pub const OBSCURE_SLACK: f32 = 2.0 * SOURCE_UNIT;

/// The room's wet-share range for preset `index` (sound_hearing.md 7: the
/// automatic room, 1, as measured on TF2's itemtest; the fixed ones
/// 0.2-0.7).
pub fn room_mix(index: u16) -> (f32, f32) {
    if index == 1 { hearing_rules::AUTO_ROOM_MIX } else { hearing_rules::FIXED_ROOM_MIX }
}

/// A playing sound: who made it on which channel, and its live gains
/// (to fade it out when another replaces it). Public to read (tests
/// count the voices a source has sounding and their gains).
#[derive(Component)]
pub struct Playing {
    source: Option<Entity>,
    channel: Option<u8>,
    gains: Arc<Gains>,
    /// Its per-ear gains at volume 1 (distance, panning, the mix's gain)
    /// and the volume it started at: `gains` is their product, or the
    /// ramp's (`VolumeRamp`).
    base: (f32, f32),
    volume: f32,
    entry: String,
    /// When it started and when its clip ends (`Time` elapsed, s).
    started: f64,
    ends: f64,
    /// Its clip's peak per `PEAK_BLOCK` of the clip (0-1, `clip_peaks`)
    /// and its playback speed, for the mix's limit (`limit_mix`).
    peaks: Arc<[f32]>,
    speed: f32,
}

impl Playing {
    pub fn source(&self) -> Option<Entity> {
        self.source
    }
    pub fn channel(&self) -> Option<u8> {
        self.channel
    }
    /// Its per-ear gains now (as set when it started, or ramped:
    /// `VolumeRamp`).
    pub fn gains(&self) -> (f32, f32) {
        self.gains.get()
    }
    pub fn entry(&self) -> &str {
        &self.entry
    }
    /// When it started (`Time` elapsed, s).
    pub fn started(&self) -> f64 {
        self.started
    }
    /// Still sounding at `now` (`Time` elapsed, s): its clip hasn't
    /// ended (a loop never does). Headless, nothing despawns a finished
    /// one.
    pub fn sounding(&self, now: f64) -> bool {
        self.ends > now
    }

    /// The loudest it plays from `now` to `ahead` seconds later: its
    /// clip's peak there times its louder ear's gain.
    pub fn peak(&self, now: f64, ahead: f64) -> f32 {
        if !self.sounding(now) {
            return 0.0;
        }
        let block = |t: f64| ((t - self.started).max(0.0) * self.speed as f64 / PEAK_BLOCK) as usize;
        let (a, b) = (block(now), block(now + ahead) + 1);
        let clip = self.peaks.get(a..b.min(self.peaks.len())).map_or(0.0, |p| p.iter().copied().fold(0.0, f32::max));
        let (l, r) = self.gains.get();
        clip * l.max(r)
    }
}

/// The span of a clip one entry of its peaks covers (s).
pub const PEAK_BLOCK: f64 = 0.01;

/// A playing sound's volume ramp (`map::FadeSound`): from `from` at
/// `start` (`Time` elapsed, s) to `to` over `over` seconds.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct VolumeRamp {
    pub from: f32,
    pub to: f32,
    pub start: f64,
    pub over: f32,
}

impl VolumeRamp {
    /// Its volume at `now`.
    pub fn at(&self, now: f64) -> f32 {
        ramp_volume(self.from, self.to, self.over, (now - self.start) as f32)
    }
}

/// How long a `FadeSound` waits for its sound to start (s): a network
/// client gets a sound and its fade on different channels.
pub const FADE_WAIT: f64 = 0.5;

/// Each ramping sound's gains from its ramp (`VolumeRamp`).
fn ramp_volumes(playing: Query<(&Playing, &VolumeRamp)>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    for (p, r) in &playing {
        let v = r.at(now);
        p.gains.set(p.base.0 * v, p.base.1 * v);
    }
}

/// A clip's peak (0-1, either channel) in each `PEAK_BLOCK`.
pub fn clip_peaks(clip: &MapSoundClip) -> Arc<[f32]> {
    let channels = clip.channels.max(1) as usize;
    let per = ((clip.rate.max(1) as f64 * PEAK_BLOCK) as usize).max(1) * channels;
    clip.samples.chunks(per).map(|c| c.iter().map(|s| (*s as f32 / 32768.0).abs()).fold(0.0, f32::max)).collect()
}

/// The highest the summed one-shot voices may reach (of full scale),
/// leaving room for what isn't counted: the long-lived loops (2fort's
/// ambience peaks near 0.25 at `volume` 1) and the room's echo. At 0.98
/// a rocket and its blast still clipped a tenth as long.
pub const MIX_CEILING: f32 = 0.75;

/// The mix's limit for voices whose peaks (each already times its
/// gains) sum to `sum`, at the master `volume`: 1 while that stays under
/// `MIX_CEILING`, else what brings it there (a sum of peaks, so it never
/// clips however the waves line up).
pub fn mix_limit_for(sum: f32, volume: f32) -> f32 {
    let total = sum * volume;
    if total > MIX_CEILING { MIX_CEILING / total } else { 1.0 }
}

/// How far ahead of this frame the limit looks (s): a frame or so, so
/// a sound starting now is limited from its first sample.
const LIMIT_AHEAD: f64 = 0.05;

/// The mix's limit this frame (`live_sound::MixLimit`), from every voice
/// playing (one-shot sounds; long-lived loops and the room's echo are
/// left out).
fn limit_mix(playing: Query<&Playing>, volume: Option<Res<GlobalVolume>>, time: Res<Time>, limit: Res<super::live_sound::MixLimit>) {
    let now = time.elapsed_secs_f64();
    let sum: f32 = playing.iter().map(|p| p.peak(now, LIMIT_AHEAD)).sum();
    let volume = volume.map_or(1.0, |v| v.volume.to_linear());
    limit.set(mix_limit_for(sum, volume));
}

/// Whether a clip loops: its own loop point (from its WAV's `cue ` or
/// `smpl` chunk: `chimera_formats::wav`) lies inside its data. Nothing
/// else makes a game sound loop (specs/source/sound_looping.md 1, 4).
pub fn loops(clip: &MapSoundClip) -> bool {
    clip.loop_start.is_some_and(|l| l < clip.samples.len() / clip.channels.max(1) as usize)
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
            .add_message::<StopSound>()
            .add_message::<FadeSound>()
            .add_systems(PostUpdate, (play_sounds, ramp_volumes, limit_mix).chain().run_if(resource_exists::<Assets<LiveClip>>))
            .add_plugins((
                super::soundscape::SoundscapePlugin,
                super::live_sound::LiveSoundPlugin,
                super::hearing::HearingPlugin,
                super::room::RoomPlugin,
            ));
    }
}

/// What a sound asked for but not heard says (once per entry and map):
/// its entry isn't in the map's sound table (nothing loaded it: a gun's
/// or view model's sound missing from its game's sound lists), or it is
/// but none of its waves read (their files missing from the install or
/// the asset store: a pack published before the entry was added).
pub fn unheard_warning(entry: &str, known: bool) -> String {
    if known {
        format!("sound {entry}: no wave of it could be read (its files are missing from the install or the asset store); not heard")
    } else {
        format!("sound {entry}: not in this map's sound table (nothing loaded it); not heard")
    }
}

#[allow(clippy::too_many_arguments)]
fn play_sounds(
    mut commands: Commands,
    mut messages: MessageReader<PlaySound>,
    mut stops: MessageReader<StopSound>,
    bank: Option<Res<SoundBank>>,
    (listeners, world, players): (
        Query<(&GlobalTransform, Option<&ChildOf>), With<SoundListener>>,
        Option<WorldTracer>,
        Query<(), (With<crate::core::MovementState>, Without<crate::core::Npc>)>,
    ),
    playing: Query<(Entity, &Playing, Option<&VolumeRamp>)>,
    mut sources: ResMut<Assets<LiveClip>>,
    hearing: Res<Hearing>,
    room: Res<RoomDsp>,
    mix: Option<Res<super::live_sound::SoundMix>>,
    muted: Option<Res<super::live_sound::MutedSounds>>,
    time: Res<Time>,
    hold: (Option<Res<crate::map::LocalMapLoad>>, Option<Res<WorldUnheard>>),
    (mut seed, mut unheard): (Local<u64>, Local<std::collections::HashSet<String>>),
    (mut peaks, mut fades, mut pending): (
        Local<std::collections::HashMap<usize, Arc<[f32]>>>,
        MessageReader<FadeSound>,
        Local<Vec<(f64, FadeSound)>>,
    ),
    mix_limit: Res<super::live_sound::MixLimit>,
) {
    let now = time.elapsed_secs_f64();
    let mix = mix.map_or_else(Default::default, |m| *m);
    let mut stopped: Vec<Entity> = Vec::new();
    // Sounds asked to stop (`StopSound`: a gun's loop out of its holder's
    // hand) fade out first, before this run's new ones start
    // (held or not).
    for s in stops.read() {
        for (e, p, _) in &playing {
            if p.source == Some(s.source) && p.entry.eq_ignore_ascii_case(&s.entry) && !stopped.contains(&e) {
                fade(&mut commands, &mut stopped, e, p);
            }
        }
    }
    // Ramps asked for (`FadeSound`), waiting for their sounds a moment.
    pending.extend(fades.read().map(|f| (now, f.clone())));
    pending.retain(|(at, _)| now - *at <= FADE_WAIT);
    let Some(bank) = bank.filter(|_| !held(hold.0.as_deref(), hold.1.as_deref())) else {
        messages.clear();
        return;
    };
    // Wave indices are the bank's: a new bank (a map change) starts anew.
    if bank.is_changed() {
        peaks.clear();
        unheard.clear();
    }
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
    let mut started: Vec<((Entity, u8), Entity)> = Vec::new();
    // Every sound started this run (source, entry), for the voice limits.
    let mut fresh: Vec<(Entity, Option<Entity>, String)> = Vec::new();
    for m in messages.read() {
        // Every game sound asked for (`RUST_LOG=chimera_present::audio=debug`):
        // what plays when (nothing of a game under a main menu).
        debug!("game sound {}", m.entry);
        if muted.as_ref().is_some_and(|x| x.0.iter().any(|e| *e == m.entry)) {
            continue;
        }
        // An entry the map's table lacks, or whose files didn't read,
        // plays nothing: said once per entry and map (`unheard_warning`).
        let entry = bank.0.entry(&m.entry);
        if entry.is_none_or(|e| e.waves.is_empty()) {
            if unheard.insert(m.entry.to_lowercase()) {
                warn!("{}", unheard_warning(&m.entry, entry.is_some()));
            }
            continue;
        }
        let Some(entry) = entry else { continue };
        let wave = entry.waves[((unit() * entry.waves.len() as f32) as usize).min(entry.waves.len() - 1)];
        let clip = &bank.0.clips[wave];
        let volume = m.volume.unwrap_or_else(|| entry.volume.draw(unit())).clamp(0.0, 1.0);
        let drawn = entry.pitch.draw(unit());
        let pitch = m.pitch.unwrap_or(drawn).round().clamp(1.0, 255.0);
        let level = match entry.level {
            SoundLevel::Db(l) => SoundLevel::Db(l.round()),
            a => a,
        };
        // Unspatialized sounds (interface, announcer) stay out of the room.
        let base = mix.gain(&m.entry);
        let mut fold = None;
        let (left, right, send) = match (m.at, listener, &entry.hearing) {
            // Source's rules (specs/source/sound_hearing.md): one wall
            // trace from the ear when the sound starts (not for the
            // listener's own).
            (Some(at), Some(ear), Some(rules)) => {
                let own = m.source.is_some() && m.source == ear.owner;
                let blocked = !own && world.as_ref().is_some_and(|w| obscured(w, ear.at.translation(), at));
                let player = m.source.is_some_and(|s| players.contains(s));
                let h = ear.hear(at, m.source, level, rules, player, blocked, room_mix(room.preset));
                fold = h.fold;
                (h.left * base, h.right * base, h.send)
            }
            (Some(at), Some(ear), None) => {
                let (l, r, send) = ear.gains(at, m.source, level);
                (l * base, r * base, send)
            }
            _ => (base, base, 0.0),
        };
        // A ramp waiting for this sound starts with it (`FadeSound`).
        let ramp = m.source.and_then(|source| {
            let i = pending.iter().position(|(_, f)| f.source == source && f.entry.eq_ignore_ascii_case(&m.entry))?;
            let f = pending.remove(i).1;
            Some(VolumeRamp { from: volume, to: f.to.clamp(0.0, 1.0), start: now, over: f.over })
        });
        // A level's distance low-pass (none for the listener's own sounds).
        let cutoff = match (m.at, listener) {
            (Some(at), Some(ear)) if m.source.is_none() || m.source != ear.owner => {
                distance_cutoff(level, at.distance(ear.at.translation()))
            }
            _ => 1.0,
        };
        let send = if entry.dry { 0.0 } else { send };
        // Silent (out of hearing, or at volume 0 with no ramp up): not
        // played.
        if left.max(right) < 1e-3 || volume.max(ramp.map_or(0.0, |r| r.to)) < 1e-3 {
            continue;
        }
        // Replace whatever this source plays on this channel: it fades out
        // over a few milliseconds and ends (a hard stop clicks: a
        // full-auto gun's shots each cut the last one off), then the
        // backend despawns it; it no longer counts as on the channel.
        if let (Some(source), Some(channel)) = (m.source, m.channel) {
            for (e, p, _) in &playing {
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
            .filter(|(e, p, _)| !stopped.contains(e) && p.sounding(now) && same(p.source, &p.entry))
            .map(|(e, p, _)| (e, p))
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
        // A clip loops only by its own loop point (its WAV's `cue ` or
        // `smpl` chunk, specs/source/sound_looping.md): then it sounds
        // until stopped or replaced.
        let frames = clip.samples.len() / clip.channels.max(1) as usize;
        let length = if loops(clip) {
            f64::INFINITY
        } else {
            frames as f64 / clip.rate.max(1) as f64 / (pitch as f64 / 100.0)
        };
        let gains = Arc::new(Gains::new(left * volume, right * volume).with_send(send).with_cutoff(cutoff).with_fold(fold));
        let handle = sources.add(LiveClip::new(clip.clone(), gains.clone(), hearing.mix.clone()).with_room(&room).with_limit(&mix_limit));
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
                    base: (left, right),
                    volume,
                    entry: m.entry.clone(),
                    started: now,
                    ends: now + length,
                    peaks: peaks.entry(wave).or_insert_with(|| clip_peaks(clip)).clone(),
                    speed: pitch / 100.0,
                },
            ))
            .id();
        if let Some(r) = ramp {
            commands.entity(e).insert(r);
        }
        fresh.push((e, m.source, m.entry.clone()));
        if let (Some(source), Some(channel)) = (m.source, m.channel) {
            started.push(((source, channel), e));
        }
    }
    // The ramps for sounds already playing start now, from where they are.
    pending.retain(|(_, f)| {
        let mut found = false;
        for (e, p, r) in &playing {
            if p.source == Some(f.source) && p.entry.eq_ignore_ascii_case(&f.entry) && !stopped.contains(&e) {
                let from = r.map_or(p.volume, |r| r.at(now));
                commands.entity(e).try_insert(VolumeRamp { from, to: f.to.clamp(0.0, 1.0), start: now, over: f.over });
                found = true;
            }
        }
        !found
    });
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
