//! The sound tag (`snd!`) reader and the Xbox ADPCM decoder, from
//! specs/halo_ce/sounds.md: `Sound::read` gives a sound's fields, pitch
//! ranges and permutations as plain structs with typed enums; a
//! permutation's samples (outside the tag data, at an absolute file offset)
//! decode to 16-bit PCM exactly by the spec.
//!
//! - `Sound::read(map, tag)`; `Sound::distances` resolves the minimum and
//!   maximum distance against the class defaults (`SoundClass::defaults`).
//! - `Sound::decode_permutation(map, range, permutation)` decodes one
//!   permutation to a `Pcm` (rate, channels, interleaved `i16` samples);
//!   `Sound::decode_chain(map, range, head)` follows a split sound's
//!   next-permutation links from a head and concatenates the chunks.
//!   `adpcm` has the block decoders themselves.
//! - `gain.rs`: the spec's distance attenuation (mono inverse distance and
//!   its audibility cutoff, stereo linear fade), the impulse gain product
//!   and the console's hundredths-of-a-decibel volume, as pure functions.
//! - `looping::LoopingSound`: the looping sound tag (tracks, details).
//!
//! The decoded `Pcm` is plain `Vec<i16>` rather than `chimera_formats`'
//! `SoundClip`: the game crate wraps it (rate, channels, `Arc<[i16]>`).
//!
//! Known shortcuts:
//! - Only what the disc has is read: Xbox ADPCM (compression 1), mono at
//!   22 050 Hz or stereo at either rate; another compression or mono at
//!   44 100 Hz refuses the tag, as the spec says.
//! - A sound class that is an unnamed slot refuses the tag (the spec calls
//!   it damage); a number past 50 is clamped to 50 and one below 0 to 0.
//! - Playback (pitch range choice, permutation choice, admission, the
//!   duck, obstruction, loops) is not here; the gain and distance rules
//!   are.
//! - A permutation's mouth data is copied; subtitle data is not read.

pub mod adpcm;
mod class;
pub mod gain;
pub mod looping;

use std::io;

use crate::cache::{Bounds, MapFile, TagEntry, TagRef, View, class as tag_class};

pub use class::{ClassDefaults, SOUND_CLASS_COUNT, SoundClass};

/// The sound tag's main struct.
pub const SOUND_SIZE: usize = 0xA4;
pub const PITCH_RANGE_SIZE: usize = 0x48;
pub const PERMUTATION_SIZE: usize = 0x7C;
pub const MAX_PITCH_RANGES: usize = 8;
pub const MAX_PERMUTATIONS_PER_RANGE: usize = 256;
/// The tool's limit on one permutation's bytes.
pub const MAX_PERMUTATION_BYTES: usize = 0x40_0000;
/// The size a long sound is cut into.
pub const CHUNK_BYTES: usize = 65_520;
/// Header flag: the build padded every permutation to whole blocks.
pub const FLAG_FIT_TO_ADPCM_BLOCK_SIZE: u32 = 1;
/// Header flag: the permutations are chunks of long sounds linked by
/// next-permutation indices.
pub const FLAG_SPLIT_LONG_SOUND: u32 = 2;

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// The sample rate enumeration of the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SampleRate {
    Hz22050,
    Hz44100,
}

impl SampleRate {
    pub fn hz(self) -> u32 {
        match self {
            SampleRate::Hz22050 => 22_050,
            SampleRate::Hz44100 => 44_100,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    Mono,
    Stereo,
}

impl Encoding {
    pub fn channels(self) -> usize {
        match self {
            Encoding::Mono => 1,
            Encoding::Stereo => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compression {
    /// 16-bit PCM; never on the disc.
    None,
    XboxAdpcm,
    /// The PC format; never on the disc.
    ImaAdpcm,
    /// The PC format; never on the disc.
    OggVorbis,
}

impl Compression {
    fn from_code(c: i16) -> Option<Compression> {
        Some(match c {
            0 => Compression::None,
            1 => Compression::XboxAdpcm,
            2 => Compression::ImaAdpcm,
            3 => Compression::OggVorbis,
            _ => return None,
        })
    }
}

/// The skip fraction, gain and pitch multipliers a sound applies at play
/// scale 0 or 1 (`gain::lerp` between the two by the scale).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScaleModifiers {
    pub skip_fraction: f32,
    pub gain: f32,
    pub pitch: f32,
}

/// A sound.
#[derive(Clone, Debug, PartialEq)]
pub struct Sound {
    /// The tag's path (for messages).
    pub path: String,
    /// Bit 0 fit to ADPCM block size, bit 1 split long sound.
    pub flags: u32,
    pub class: SoundClass,
    pub sample_rate: SampleRate,
    /// World units; 0 takes the class's (see `distances`).
    pub minimum_distance: f32,
    /// World units; 0 takes the class's.
    pub maximum_distance: f32,
    /// Chance a play request is dropped.
    pub skip_fraction: f32,
    /// The pitch of a play is drawn uniformly from this.
    pub random_pitch_bounds: Bounds<f32>,
    /// Radians (2 pi on the disc).
    pub inner_cone_angle: f32,
    pub outer_cone_angle: f32,
    pub outer_cone_gain: f32,
    pub gain_modifier: f32,
    /// 0 = no limit.
    pub maximum_bend_per_second: f32,
    pub scale_zero: ScaleModifiers,
    pub scale_one: ScaleModifiers,
    pub encoding: Encoding,
    pub compression: Compression,
    /// A `snd!` reference, or null.
    pub promotion_sound: TagRef,
    pub promotion_count: i16,
    /// Milliseconds, the tool's estimate (1.125 times the true duration).
    pub longest_permutation_length_ms: i32,
    pub pitch_ranges: Vec<PitchRange>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PitchRange {
    pub name: String,
    /// The pitch at which the recordings play unchanged.
    pub natural_pitch: f32,
    /// The pitches this range is used for.
    pub bend_bounds: Bounds<f32>,
    /// How many of the range's permutations a play may pick from (the
    /// chain heads, first in the list), clamped into 1..=count.
    pub actual_permutation_count: usize,
    /// Stored as `1 / natural pitch`.
    pub playback_rate: f32,
    pub permutations: Vec<Permutation>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Permutation {
    pub name: String,
    pub skip_fraction: f32,
    pub gain: f32,
    pub compression: Compression,
    /// The next chunk of a split sound (an index in the same range).
    pub next_permutation: Option<usize>,
    /// Bytes of ADPCM at `sample_offset`.
    pub sample_size: usize,
    /// Absolute offset of the samples in the decompressed image.
    pub sample_offset: usize,
    /// One byte per tick of mouth openness (dialogue only).
    pub mouth_data: Vec<u8>,
}

/// Decoded audio: `samples` interleaved (left, right for stereo).
#[derive(Clone, Debug, PartialEq)]
pub struct Pcm {
    pub rate: u32,
    pub channels: u16,
    pub samples: Vec<i16>,
}

impl Pcm {
    /// Samples per channel.
    pub fn frames(&self) -> usize {
        self.samples.len() / self.channels.max(1) as usize
    }

    /// Duration in seconds.
    pub fn seconds(&self) -> f32 {
        self.frames() as f32 / self.rate as f32
    }
}

fn bounds(v: &View<'_>, at: usize) -> io::Result<Bounds<f32>> {
    v.real_bounds(at)
}

impl Sound {
    /// Read the sound tag `tag` (class `snd!`) of `map`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Sound> {
        if tag.class != tag_class::SOUND {
            return Err(err(format!(
                "{}: {} `{}` is not a sound",
                map.header().name,
                tag.class,
                tag.path
            )));
        }
        let v = map.tag_view(tag, SOUND_SIZE)?;
        let class_number = i32::from(v.i16(0x04)?).clamp(0, SOUND_CLASS_COUNT as i32 - 1);
        let class = SoundClass::from_index(class_number)
            .ok_or_else(|| v.error(0x04, format_args!("sound class {class_number} is an unnamed slot")))?;
        let sample_rate = match v.enum16(0x06)? {
            0 => SampleRate::Hz22050,
            1 => SampleRate::Hz44100,
            r => return Err(v.error(0x06, format_args!("sample rate enum {r}"))),
        };
        let encoding = match v.enum16(0x6C)? {
            0 => Encoding::Mono,
            1 => Encoding::Stereo,
            e => return Err(v.error(0x6C, format_args!("encoding enum {e}"))),
        };
        let compression = Compression::from_code(v.enum16(0x6E)?)
            .ok_or_else(|| v.error(0x6E, format_args!("compression enum {}", v.enum16(0x6E).unwrap_or(0))))?;
        if compression != Compression::XboxAdpcm {
            return Err(v.error(
                0x6E,
                format_args!("compression {compression:?}: only Xbox ADPCM sounds are read"),
            ));
        }
        if encoding == Encoding::Mono && sample_rate == SampleRate::Hz44100 {
            return Err(v.error(0x06, "mono sound at 44100 Hz: not playable"));
        }
        let modifiers = |at: usize| -> io::Result<ScaleModifiers> {
            Ok(ScaleModifiers {
                skip_fraction: v.f32(at)?,
                gain: v.f32(at + 4)?,
                pitch: v.f32(at + 8)?,
            })
        };
        let pitch_ranges = v
            .block_max(0x98, PITCH_RANGE_SIZE, MAX_PITCH_RANGES)?
            .iter()
            .map(|r| read_pitch_range(&r))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Sound {
            path: tag.path.clone(),
            flags: v.flags32(0x00)?,
            class,
            sample_rate,
            minimum_distance: v.f32(0x08)?,
            maximum_distance: v.f32(0x0C)?,
            skip_fraction: v.f32(0x10)?,
            random_pitch_bounds: bounds(&v, 0x14)?,
            inner_cone_angle: v.f32(0x1C)?,
            outer_cone_angle: v.f32(0x20)?,
            outer_cone_gain: v.f32(0x24)?,
            gain_modifier: v.f32(0x28)?,
            maximum_bend_per_second: v.f32(0x2C)?,
            scale_zero: modifiers(0x3C)?,
            scale_one: modifiers(0x54)?,
            encoding,
            compression,
            promotion_sound: v.tag_ref(0x70)?,
            promotion_count: v.i16(0x80)?,
            longest_permutation_length_ms: v.i32(0x84)?,
            pitch_ranges,
        })
    }

    /// The minimum and maximum distance in world units: each the tag's
    /// own, or the class's when the tag's is zero.
    pub fn distances(&self) -> (f32, f32) {
        let d = self.class.defaults();
        (
            if self.minimum_distance == 0.0 {
                d.minimum_distance
            } else {
                self.minimum_distance
            },
            if self.maximum_distance == 0.0 {
                d.maximum_distance
            } else {
                self.maximum_distance
            },
        )
    }

    /// The gain at distance `d` (world units) from the listener before the
    /// other factors: the mono inverse-distance law (without the audibility
    /// cutoff: see `gain::mono_audible`) or the stereo linear fade, with
    /// the resolved distances.
    pub fn distance_gain(&self, d: f32) -> f32 {
        let (min, max) = self.distances();
        match self.encoding {
            Encoding::Mono => gain::mono_gain(min, d),
            Encoding::Stereo => gain::stereo_gain(min, max, d),
        }
    }

    /// Whether the sound is split into chunks (`FLAG_SPLIT_LONG_SOUND`).
    pub fn is_split(&self) -> bool {
        self.flags & FLAG_SPLIT_LONG_SOUND != 0
    }

    fn permutation(&self, range: usize, permutation: usize) -> io::Result<&Permutation> {
        self.pitch_ranges
            .get(range)
            .and_then(|r| r.permutations.get(permutation))
            .ok_or_else(|| {
                err(format!(
                    "sound `{}`: no permutation {permutation} in pitch range {range}",
                    self.path
                ))
            })
    }

    /// The frames a permutation decodes to (whole blocks: `floor(bytes /
    /// block size) x 64`).
    pub fn permutation_frames(&self, range: usize, permutation: usize) -> io::Result<usize> {
        let p = self.permutation(range, permutation)?;
        Ok(adpcm::frame_count(p.sample_size, self.encoding.channels()))
    }

    fn append_permutation(&self, map: &MapFile, p: &Permutation, out: &mut Vec<i16>) -> io::Result<()> {
        let bytes = map
            .file_bytes(p.sample_offset, p.sample_size)
            .map_err(|e| err(format!("sound `{}` permutation `{}`: {e}", self.path, p.name)))?;
        out.extend(adpcm::decode(bytes, self.encoding.channels()));
        Ok(())
    }

    fn pcm(&self, samples: Vec<i16>) -> Pcm {
        Pcm {
            rate: self.sample_rate.hz(),
            channels: self.encoding.channels() as u16,
            samples,
        }
    }

    /// Decode one permutation of pitch range `range` to PCM (whole blocks;
    /// a size of 0 gives an empty clip).
    pub fn decode_permutation(&self, map: &MapFile, range: usize, permutation: usize) -> io::Result<Pcm> {
        let p = self.permutation(range, permutation)?;
        let mut samples = Vec::new();
        self.append_permutation(map, p, &mut samples)?;
        Ok(self.pcm(samples))
    }

    /// The permutation indices of the chain starting at `head`, following
    /// next links (stopping at the end or when a link leaves the range or
    /// repeats an index).
    pub fn chain(&self, range: usize, head: usize) -> io::Result<Vec<usize>> {
        let r = self
            .pitch_ranges
            .get(range)
            .ok_or_else(|| err(format!("sound `{}`: no pitch range {range}", self.path)))?;
        self.permutation(range, head)?;
        let mut out = vec![head];
        let mut at = head;
        while let Some(next) = r.permutations[at].next_permutation {
            if next >= r.permutations.len() || out.contains(&next) {
                break;
            }
            out.push(next);
            at = next;
        }
        Ok(out)
    }

    /// Decode the chain starting at `head` of pitch range `range` into one
    /// clip, the chunks' samples concatenated (each chunk begins with a
    /// fresh block header, so the chunks decode alone).
    pub fn decode_chain(&self, map: &MapFile, range: usize, head: usize) -> io::Result<Pcm> {
        let mut samples = Vec::new();
        for i in self.chain(range, head)? {
            self.append_permutation(map, &self.pitch_ranges[range].permutations[i], &mut samples)?;
        }
        Ok(self.pcm(samples))
    }
}

fn read_pitch_range(r: &View<'_>) -> io::Result<PitchRange> {
    let block = r.block_max(0x3C, PERMUTATION_SIZE, MAX_PERMUTATIONS_PER_RANGE)?;
    let permutations = block
        .iter()
        .map(|p| {
            let compression = Compression::from_code(p.enum16(0x28)?)
                .ok_or_else(|| p.error(0x28, format_args!("compression enum {}", p.enum16(0x28).unwrap_or(0))))?;
            let size = p.data_ref(0x40)?;
            let sample_size = usize::try_from(size.size)
                .map_err(|_| p.error(0x40, format_args!("negative sample size {}", size.size)))?;
            let sample_offset = if sample_size == 0 {
                0
            } else {
                usize::try_from(size.file_offset)
                    .map_err(|_| p.error(0x40, format_args!("negative file offset {}", size.file_offset)))?
            };
            let next = match p.index16(0x2A)? {
                Some(n) if n >= block.len() => None,
                n => n,
            };
            Ok(Permutation {
                name: p.tag_string(0x00)?.to_owned(),
                skip_fraction: p.f32(0x20)?,
                gain: p.f32(0x24)?,
                compression,
                next_permutation: next,
                sample_size,
                sample_offset,
                mouth_data: p.data(0x54)?.to_vec(),
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let count = i32::from(r.i16(0x2C)?).clamp(1, permutations.len().max(1) as i32) as usize;
    Ok(PitchRange {
        name: r.tag_string(0x00)?.to_owned(),
        natural_pitch: r.f32(0x20)?,
        bend_bounds: bounds(r, 0x24)?,
        actual_permutation_count: count,
        playback_rate: r.f32(0x30)?,
        permutations,
    })
}
