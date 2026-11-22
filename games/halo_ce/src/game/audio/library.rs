//! Halo's sound data as the looping sound player needs it
//! (`AudioLibrary`), compiled from a map file's tags (`Compiler`;
//! specs/halo_ce/sounds.md "The sound tag", "Looping sounds"), and how it
//! joins a map (`add`): its decoded clips and sound entries appended to the
//! map's sounds, the library itself as one map entity (`ENTITY_CLASS`, its
//! `data` key the library as TOML), so a cached conversion, a guest's
//! dressing and a network client all carry it as they carry the map's
//! entities; `follow` keeps `ActiveAudio` with the loaded map's entities.
//!
//! Per sound (`SoundInfo`): its impulse entry (`halo_ce:<path>`: each head
//! permutation's first chunk, the spec's "an impulse plays only the head
//! it picked"), and when a looping sound plays it as a loop its loop entry
//! (`halo_ce:<path>#loop`: one clip, every head's whole chain in head
//! order, played looping), its class, resolved distances, encoding, the
//! mean permutation gain times the gain modifier, the scale modifiers,
//! the random pitch bounds, range 0's playback rate, the stored longest
//! length and a head's mean duration. Per looping sound (`HaloLoop`): its
//! flags, the detail period multipliers, the maximum distance, the tracks
//! and the detail sounds by sound path.
//!
//! Known shortcuts: a loop clip is every chain once in head order, so a
//! loop of several recordings repeats the same order each pass (the
//! spec's fresh random head per pass is not drawn); only pitch range 0 is
//! compiled (the engines' other ranges are the vehicles'); a sound's gain
//! is its permutations' mean (no deck without repeats).

use std::collections::BTreeMap;

use bevy::prelude::*;
use blam_formats::{
    cache::{MapFile, TagRef, class},
    sound::{Encoding, Sound, SoundClass, looping::LoopingSound},
};
use serde::{Deserialize, Serialize};

use crate::map::{
    MapData, MapEntities, MapEntity,
    sound::{Interval, MapSoundClip, MapSoundEntry},
};

use super::super::fx::{name_of, sound::level};

/// The map entity's class name.
pub const ENTITY_CLASS: &str = "halo_ce_audio";
/// The key holding the library.
pub const KEY: &str = "data";

/// A sound's numbers (module doc).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SoundInfo {
    /// The impulse entry.
    pub entry: String,
    /// The loop entry, when a looping sound loops it.
    #[serde(default)]
    pub loop_entry: String,
    pub class: u8,
    /// Resolved minimum and maximum distance, WU.
    pub min: f32,
    pub max: f32,
    pub stereo: bool,
    /// Mean permutation gain x gain modifier.
    pub gain: f32,
    /// Gain and pitch modifiers at scale 0 and 1.
    pub zero_gain: f32,
    pub one_gain: f32,
    pub zero_pitch: f32,
    pub one_pitch: f32,
    /// Random pitch bounds.
    pub pitch: [f32; 2],
    /// Range 0's playback rate (1 / natural pitch).
    pub rate: f32,
    /// The stored longest permutation length, ms.
    pub longest_ms: i32,
    /// A head's mean duration (its first chunk), s.
    pub seconds: f32,
    /// The loop clip's duration, s.
    #[serde(default)]
    pub loop_seconds: f32,
}

impl SoundInfo {
    /// The class's row, if the class is a named one.
    pub fn class(&self) -> Option<SoundClass> {
        SoundClass::from_index(i32::from(self.class))
    }

    /// The gain factor of `scale` (`lerp(zero, one, scale)`).
    pub fn scale_gain(&self, scale: f32) -> f32 {
        blam_formats::sound::gain::lerp(self.zero_gain, self.one_gain, scale)
    }

    /// The pitch factor of `scale`.
    pub fn scale_pitch(&self, scale: f32) -> f32 {
        blam_formats::sound::gain::lerp(self.zero_pitch, self.one_pitch, scale)
    }

    /// Whether a sound plays with no room send (the class's wet gain is 1:
    /// music, ambience; sounds.md "Obstruction, occlusion, reverb").
    pub fn dry(&self) -> bool {
        self.class().is_some_and(|c| c.defaults().wet_gain >= 1.0)
    }
}

/// A looping sound's track (sound paths; empty: none).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LoopTrack {
    pub flags: u32,
    pub gain: f32,
    pub fade_in: f32,
    pub fade_out: f32,
    pub start: String,
    #[serde(rename = "loop")]
    pub looped: String,
    pub end: String,
    pub alternate_loop: String,
    pub alternate_end: String,
}

/// A looping sound's detail sound.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LoopDetail {
    pub sound: String,
    /// Seconds between plays.
    pub period: [f32; 2],
    pub gain: f32,
    pub flags: u32,
    pub yaw: [f32; 2],
    pub pitch: [f32; 2],
    pub distance: [f32; 2],
}

/// A looping sound (module doc).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct HaloLoop {
    pub flags: u32,
    /// The detail period multiplier at scale 0 and 1.
    pub period_scale: [f32; 2],
    /// WU: the largest maximum distance of its sounds.
    pub max_distance: f32,
    pub tracks: Vec<LoopTrack>,
    pub details: Vec<LoopDetail>,
}

/// Everything the player and its owners read (module doc), plus the
/// level's own part (`level`, `maps::audio`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AudioLibrary {
    /// By sound tag path (lower case).
    pub sounds: BTreeMap<String, SoundInfo>,
    /// By looping sound tag path (lower case).
    pub loops: BTreeMap<String, HaloLoop>,
    /// A projectile's looping sound attachments, by projectile tag path.
    pub projectiles: BTreeMap<String, Vec<String>>,
    /// A weapon's looping sound attachments, by weapon tag path (lower
    /// case): the looping sound and its primary scale's function
    /// reference (0 none: full scale; 1 to 4 the weapon's outgoing
    /// function A to D: the plasma pistol's charge hum by its charge).
    pub weapons: BTreeMap<String, Vec<WeaponLoop>>,
    /// The Spartan's footstep sounds (its biped's material effects): by
    /// kind (0 walk ... 5 jump land), by material type code; empty: none.
    pub footsteps: Vec<Vec<String>>,
    /// The level's background sounds, environments, clusters and sound
    /// scenery (`maps::audio`); absent on another game's map.
    #[serde(default)]
    pub level: Option<super::level::LevelAudio>,
    /// The Spartan's unit HUD warning sounds (`warnings`), in the tag's
    /// order.
    #[serde(default)]
    pub warnings: Vec<Warning>,
}

/// One warning sound element of the unit HUD interface (`warnings`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Warning {
    /// The sound's tag path (lower case): a looping sound when `looped`,
    /// else a one-shot sound.
    pub sound: String,
    pub looped: bool,
    /// The state word's bits that play it.
    pub flags: u32,
    /// The scale a loop plays at.
    pub scale: f32,
}

impl AudioLibrary {
    /// The library as a map entity's value.
    pub fn encode(&self) -> String {
        toml::to_string(self).unwrap_or_default()
    }

    /// Read back from a map entity's value.
    pub fn decode(s: &str) -> Option<AudioLibrary> {
        toml::from_str(s).ok()
    }
}

/// A looping sound attached to a weapon (`AudioLibrary::weapons`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WeaponLoop {
    /// The looping sound's tag path (lower case).
    pub sound: String,
    /// The attachment's primary scale (objects.md "Attachments").
    pub scale: i16,
}

/// The library with what it brings to a map (clips and entries, indices
/// into its own clips).
#[derive(Clone, Debug, Default)]
pub struct AudioSet {
    pub library: AudioLibrary,
    pub clips: Vec<MapSoundClip>,
    pub entries: BTreeMap<String, MapSoundEntry>,
    pub warnings: Vec<String>,
}

/// Compiles tags of one map file into an `AudioSet`.
pub struct Compiler<'a> {
    pub file: &'a MapFile,
    pub set: AudioSet,
}

fn path(file: &MapFile, r: &TagRef) -> Option<String> {
    (!r.id.is_none()).then(|| file.tag(r.id).map(|t| t.path.to_ascii_lowercase())).flatten()
}

impl<'a> Compiler<'a> {
    pub fn new(file: &'a MapFile) -> Self {
        Self { file, set: AudioSet::default() }
    }

    fn warn(&mut self, w: String) {
        self.set.warnings.push(w);
    }

    fn read(&mut self, r: &TagRef) -> Option<(String, Sound)> {
        let tag = self.file.tag(r.id).filter(|t| t.class == class::SOUND)?;
        match Sound::read(self.file, tag) {
            Ok(s) => Some((tag.path.to_ascii_lowercase(), s)),
            Err(e) => {
                self.warn(format!("{}: {e}", tag.path));
                None
            }
        }
    }

    /// A sound as an impulse (its entry made once); its path.
    pub fn sound(&mut self, r: &TagRef) -> Option<String> {
        let path = path(self.file, r)?;
        if self.set.library.sounds.contains_key(&path) {
            return Some(path);
        }
        let (path, s) = self.read(r)?;
        let range = s.pitch_ranges.first()?;
        let heads = range.actual_permutation_count.min(range.permutations.len());
        let mut waves = Vec::new();
        let (mut gain, mut seconds) = (0.0, 0.0);
        for head in 0..heads {
            match s.decode_permutation(self.file, 0, head) {
                Ok(pcm) => {
                    seconds += pcm.seconds();
                    self.set.clips.push(MapSoundClip { rate: pcm.rate, channels: pcm.channels, samples: pcm.samples.into(), loop_start: None });
                    waves.push(self.set.clips.len() - 1);
                    gain += range.permutations[head].gain;
                }
                Err(e) => self.warn(format!("{path}: permutation {head}: {e}")),
            }
        }
        if waves.is_empty() {
            return None;
        }
        let n = waves.len() as f32;
        let (min, max) = s.distances();
        let bounds = s.random_pitch_bounds;
        let pitch = if bounds.upper > 0.0 { [bounds.lower, bounds.upper] } else { [1.0, 1.0] };
        let info = SoundInfo {
            entry: name_of(&path),
            loop_entry: String::new(),
            class: s.class.index(),
            min,
            max,
            stereo: s.encoding == Encoding::Stereo,
            gain: gain / n * s.gain_modifier,
            zero_gain: s.scale_zero.gain,
            one_gain: s.scale_one.gain,
            zero_pitch: s.scale_zero.pitch,
            one_pitch: s.scale_one.pitch,
            pitch,
            rate: range.playback_rate.max(0.01),
            longest_ms: s.longest_permutation_length_ms,
            seconds: seconds / n,
            loop_seconds: 0.0,
        };
        self.set.entries.insert(info.entry.clone(), MapSoundEntry {
            waves,
            volume: Interval::fixed(info.gain.clamp(0.0, 1.0)),
            pitch: Interval { start: pitch[0] * 100.0 * info.rate, range: (pitch[1] - pitch[0]) * 100.0 * info.rate },
            level: level(min),
            channel: 0,
            dry: info.dry(),
            hearing: None,
        });
        self.set.library.sounds.insert(path.clone(), info);
        Some(path)
    }

    /// A sound a looping sound loops: its impulse entry and its loop clip
    /// (every head's chain, module doc); its path.
    pub fn loop_sound(&mut self, r: &TagRef) -> Option<String> {
        let path = self.sound(r)?;
        if !self.set.library.sounds[&path].loop_entry.is_empty() {
            return Some(path);
        }
        let (_, s) = self.read(r)?;
        let range = s.pitch_ranges.first()?;
        let heads = range.actual_permutation_count.min(range.permutations.len());
        let mut samples: Vec<i16> = Vec::new();
        let (mut rate, mut channels) = (0, 1);
        for head in 0..heads {
            match s.decode_chain(self.file, 0, head) {
                Ok(pcm) => {
                    (rate, channels) = (pcm.rate, pcm.channels);
                    samples.extend_from_slice(&pcm.samples);
                }
                Err(e) => self.warn(format!("{path}: chain {head}: {e}")),
            }
        }
        if samples.is_empty() || rate == 0 {
            return Some(path);
        }
        let seconds = samples.len() as f32 / f32::from(channels.max(1)) / rate as f32;
        self.set.clips.push(MapSoundClip { rate, channels, samples: samples.into(), loop_start: Some(0) });
        let clip = self.set.clips.len() - 1;
        let info = self.set.library.sounds.get_mut(&path)?;
        info.loop_entry = format!("{}#loop", info.entry);
        info.loop_seconds = seconds;
        let mut entry = self.set.entries[&info.entry].clone();
        entry.waves = vec![clip];
        self.set.entries.insert(info.loop_entry.clone(), entry);
        Some(path)
    }

    /// A looping sound (its sounds compiled); its path.
    pub fn looping(&mut self, r: &TagRef) -> Option<String> {
        let tag = self.file.tag(r.id).filter(|t| t.class == class::SOUND_LOOPING)?;
        let lpath = tag.path.to_ascii_lowercase();
        if self.set.library.loops.contains_key(&lpath) {
            return Some(lpath);
        }
        let l = match LoopingSound::read(self.file, tag) {
            Ok(l) => l,
            Err(e) => {
                self.warn(format!("{}: {e}", tag.path));
                return None;
            }
        };
        let mut out = HaloLoop {
            flags: l.flags,
            period_scale: [l.scale_lower_bound[0], l.scale_upper_bound[0]],
            max_distance: l.maximum_distance,
            ..Default::default()
        };
        for t in &l.tracks {
            let track = LoopTrack {
                flags: t.flags,
                gain: t.gain,
                fade_in: t.fade_in_duration,
                fade_out: t.fade_out_duration,
                start: self.sound(&t.start_sound).unwrap_or_default(),
                looped: self.loop_sound(&t.loop_sound).unwrap_or_default(),
                end: self.sound(&t.end_sound).unwrap_or_default(),
                alternate_loop: self.loop_sound(&t.alternate_loop_sound).unwrap_or_default(),
                alternate_end: self.sound(&t.alternate_end_sound).unwrap_or_default(),
            };
            out.tracks.push(track);
        }
        for d in &l.detail_sounds {
            let Some(sound) = self.sound(&d.sound) else { continue };
            out.details.push(LoopDetail {
                sound,
                period: [d.random_period.lower, d.random_period.upper],
                gain: d.gain,
                flags: d.flags,
                yaw: [d.yaw_bounds.lower, d.yaw_bounds.upper],
                pitch: [d.pitch_bounds.lower, d.pitch_bounds.upper],
                distance: [d.distance_bounds.lower, d.distance_bounds.upper],
            });
        }
        // The stored maximum distance, else the largest of its sounds'.
        if out.max_distance <= 0.0 {
            let lib = &self.set.library.sounds;
            out.max_distance = out
                .tracks
                .iter()
                .flat_map(|t| [&t.start, &t.looped, &t.end])
                .filter_map(|p| lib.get(p))
                .map(|s| s.max)
                .fold(0.0, f32::max);
        }
        self.set.library.loops.insert(lpath.clone(), out);
        Some(lpath)
    }

    /// An object's looping sound attachments (compiled); their paths.
    pub fn attached_loops(&mut self, attachments: &[blam_formats::object::Attachment]) -> Vec<String> {
        self.scaled_loops(attachments).into_iter().map(|l| l.sound).collect()
    }

    /// `attached_loops` with each attachment's primary scale.
    pub fn scaled_loops(&mut self, attachments: &[blam_formats::object::Attachment]) -> Vec<WeaponLoop> {
        attachments
            .iter()
            .filter(|a| a.attached.class == class::SOUND_LOOPING)
            .filter_map(|a| Some(WeaponLoop { sound: self.looping(&a.attached)?, scale: a.primary_scale }))
            .collect()
    }
}

/// `set` joined to `map` (module doc): clips and entries to its sounds,
/// the library as its entity (one already there is replaced).
pub fn add(map: &mut MapData, set: &AudioSet) {
    let sounds = std::sync::Arc::make_mut(&mut map.sounds);
    let clip = sounds.clips.len();
    sounds.clips.extend(set.clips.iter().cloned());
    for (name, e) in &set.entries {
        let mut e = e.clone();
        e.waves.iter_mut().for_each(|w| *w += clip);
        sounds.entries.insert(name.clone(), e);
    }
    map.entities.retain(|e| e.classname() != ENTITY_CLASS);
    map.entities.push(MapEntity {
        keyvalues: vec![("classname".into(), ENTITY_CLASS.into()), (KEY.into(), set.library.encode())],
        ..Default::default()
    });
    map.warnings.extend(set.warnings.iter().map(|w| format!("Halo's sounds: {w}")));
}

/// The loaded map's library (from its entity, or set by a test).
#[derive(Resource, Clone, Debug, Default)]
pub struct ActiveAudio(pub Option<std::sync::Arc<AudioLibrary>>);

/// `ActiveAudio` from the map's entities, kept with them.
pub fn follow(entities: Option<Res<MapEntities>>, mut active: ResMut<ActiveAudio>, mut seen: Local<Option<usize>>) {
    let Some(map) = entities else {
        if seen.take().is_some() {
            active.0 = None;
        }
        return;
    };
    let key = std::sync::Arc::as_ptr(&map.entities) as usize;
    if *seen == Some(key) {
        return;
    }
    *seen = Some(key);
    active.0 = map
        .entities
        .iter()
        .find(|e| e.classname() == ENTITY_CLASS)
        .and_then(|e| e.get(KEY))
        .and_then(AudioLibrary::decode)
        .map(std::sync::Arc::new);
}
