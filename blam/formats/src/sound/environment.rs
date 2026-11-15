//! The sound environment tag (`snde`) reader, from specs/halo_ce/sounds.md,
//! "Sound environment, background sound, sound scenery": the reverb
//! parameters of the room the listener is in (I3DL2 style), as stored.
//! Plain data: what a port does with them is the game crate's.

use std::io;

use crate::cache::{MapFile, TagEntry, class};

/// The sound environment's main struct.
pub const ENVIRONMENT_SIZE: usize = 0x48;

/// A sound environment, its fields in the tag's units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SoundEnvironment {
    pub priority: i16,
    /// Linear gain.
    pub room_intensity: f32,
    /// Linear gain.
    pub room_intensity_hf: f32,
    pub room_rolloff: f32,
    /// Seconds.
    pub decay_time: f32,
    pub decay_hf_ratio: f32,
    /// Linear gain.
    pub reflections_intensity: f32,
    /// Seconds.
    pub reflections_delay: f32,
    /// Linear gain.
    pub reverb_intensity: f32,
    /// Seconds.
    pub reverb_delay: f32,
    /// 0..1.
    pub diffusion: f32,
    /// 0..1.
    pub density: f32,
    /// Hz.
    pub hf_reference: f32,
}

impl SoundEnvironment {
    /// Read the sound environment tag `tag` (class `snde`) of `map`.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<SoundEnvironment> {
        if tag.class != class::SOUND_ENVIRONMENT {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "{}: {} `{}` is not a sound environment",
                    map.header().name,
                    tag.class,
                    tag.path
                ),
            ));
        }
        let v = map.tag_view(tag, ENVIRONMENT_SIZE)?;
        Ok(SoundEnvironment {
            priority: v.i16(0x04)?,
            room_intensity: v.f32(0x08)?,
            room_intensity_hf: v.f32(0x0C)?,
            room_rolloff: v.f32(0x10)?,
            decay_time: v.f32(0x14)?,
            decay_hf_ratio: v.f32(0x18)?,
            reflections_intensity: v.f32(0x1C)?,
            reflections_delay: v.f32(0x20)?,
            reverb_intensity: v.f32(0x24)?,
            reverb_delay: v.f32(0x28)?,
            diffusion: v.f32(0x2C)?,
            density: v.f32(0x30)?,
            hf_reference: v.f32(0x34)?,
        })
    }
}
