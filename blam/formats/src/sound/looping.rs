//! The looping sound tag (`lsnd`) reader, from specs/halo_ce/sounds.md,
//! "Looping sounds": a header, one to four tracks (start, loop and end
//! sounds, an alternate loop and end, fades and a gain) and up to 32
//! detail sounds (random one-shots). Plain data: how a loop plays is the
//! game crate's.

use std::io;

use crate::cache::{Bounds, MapFile, TagEntry, TagRef, View, class};

/// The looping sound's main struct.
pub const LOOPING_SIZE: usize = 0x54;
pub const TRACK_SIZE: usize = 0xA0;
pub const DETAIL_SIZE: usize = 0x68;
pub const MAX_TRACKS: usize = 4;
pub const MAX_DETAILS: usize = 32;

/// Track flag: fade in at start.
pub const TRACK_FADE_IN_AT_START: u32 = 1;
/// Track flag: fade out at stop.
pub const TRACK_FADE_OUT_AT_STOP: u32 = 2;
/// Track flag: fade in alternate.
pub const TRACK_FADE_IN_ALTERNATE: u32 = 4;
/// Looping sound flag: the sounds play once and the loop ends.
pub const FLAG_FAKE_IMPULSE: u32 = 2;

/// A looping sound.
#[derive(Clone, Debug, PartialEq)]
pub struct LoopingSound {
    /// Bit 0 deafening, bit 1 fake impulse, bit 2 stops music.
    pub flags: u32,
    /// The detail period multiplier at scale 0 first, then two reals
    /// nothing uses (1.0 on the disc).
    pub scale_lower_bound: [f32; 3],
    /// The same at scale 1.
    pub scale_upper_bound: [f32; 3],
    /// The tool-computed largest maximum distance of its sounds, WU.
    pub maximum_distance: f32,
    /// A `cdmg` reference, or null.
    pub continuous_damage_effect: TagRef,
    pub tracks: Vec<Track>,
    pub detail_sounds: Vec<DetailSound>,
}

/// One track of a looping sound. Every sound reference is a `snd!` tag
/// reference (null when absent).
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    /// Bit 0 fade in at start, bit 1 fade out at stop, bit 2 fade in
    /// alternate.
    pub flags: u32,
    pub gain: f32,
    /// Seconds.
    pub fade_in_duration: f32,
    /// Seconds.
    pub fade_out_duration: f32,
    pub start_sound: TagRef,
    pub loop_sound: TagRef,
    pub end_sound: TagRef,
    pub alternate_loop_sound: TagRef,
    pub alternate_end_sound: TagRef,
}

/// A random one-shot played while the loop runs.
#[derive(Clone, Debug, PartialEq)]
pub struct DetailSound {
    pub sound: TagRef,
    /// Seconds between plays.
    pub random_period: Bounds<f32>,
    pub gain: f32,
    /// Bit 0 don't play with alternate, bit 1 don't play without it.
    pub flags: u32,
    /// Radians.
    pub yaw_bounds: Bounds<f32>,
    /// Radians.
    pub pitch_bounds: Bounds<f32>,
    /// World units.
    pub distance_bounds: Bounds<f32>,
}

impl LoopingSound {
    /// Read the looping sound tag `tag` (class `lsnd`) of `map`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<LoopingSound> {
        if tag.class != class::SOUND_LOOPING {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {} `{}` is not a looping sound",
                    map.header().name,
                    tag.class,
                    tag.path
                ),
            ));
        }
        let v = map.tag_view(tag, LOOPING_SIZE)?;
        let three = |v: &View<'_>, at: usize| v.point3(at);
        let tracks = v
            .block_max(0x3C, TRACK_SIZE, MAX_TRACKS)?
            .iter()
            .map(|t| {
                Ok(Track {
                    flags: t.flags32(0x00)?,
                    gain: t.f32(0x04)?,
                    fade_in_duration: t.f32(0x08)?,
                    fade_out_duration: t.f32(0x0C)?,
                    start_sound: t.tag_ref(0x30)?,
                    loop_sound: t.tag_ref(0x40)?,
                    end_sound: t.tag_ref(0x50)?,
                    alternate_loop_sound: t.tag_ref(0x80)?,
                    alternate_end_sound: t.tag_ref(0x90)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        let detail_sounds = v
            .block_max(0x48, DETAIL_SIZE, MAX_DETAILS)?
            .iter()
            .map(|d| {
                Ok(DetailSound {
                    sound: d.tag_ref(0x00)?,
                    random_period: d.real_bounds(0x10)?,
                    gain: d.f32(0x18)?,
                    flags: d.flags32(0x1C)?,
                    yaw_bounds: d.real_bounds(0x50)?,
                    pitch_bounds: d.real_bounds(0x58)?,
                    distance_bounds: d.real_bounds(0x60)?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(LoopingSound {
            flags: v.flags32(0x00)?,
            scale_lower_bound: three(&v, 0x04)?,
            scale_upper_bound: three(&v, 0x10)?,
            maximum_distance: v.f32(0x20)?,
            continuous_damage_effect: v.tag_ref(0x2C)?,
            tracks,
            detail_sounds,
        })
    }
}
