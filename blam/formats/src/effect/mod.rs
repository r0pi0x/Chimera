//! The effect family of tags (specs/halo_ce/effects.md) as plain data,
//! limited to the classes and fields the first four weapons' effects use:
//! the effect `effe` (`Effect`: locations, events, their parts and
//! particle entries), the particle `part` (`particle::Particle` with its
//! effect shader part), point physics `pphy` (`particle::PointPhysics`),
//! the light `ligh`, the decal `deca`, material effects `foot` and the
//! contrail `cont` (`light`). Values are the tags' own (world units,
//! seconds, radians; a light's duration in ticks), with no conversion.
//!
//! - `Effect::read(map, tag)` and the others' `read`; each checks the
//!   tag's class and caps its blocks at the spec's maxima (more is refused
//!   as damage).
//! - `Part::base_class` is the build-time class the engine dispatches on.
//!
//! Known shortcuts: particle systems (`pctl`), lens flares (`lens`) and
//! glows (`glw!`) are not read (none of the four weapons' firing or impact
//! effects makes a particle system; lens flares are named by reference
//! only); a light's gel maps and periodic animation functions, a decal's
//! animation fields and its shader header, the effect shader's secondary
//! map and a contrail's point physics flags beyond the reference are left
//! out; padding is not checked (the disc sweep in the heavy tests is).

pub mod light;
pub mod particle;

use std::io;

use crate::cache::{Bounds, ClassCode, MapFile, RealArgb, TagEntry, TagRef, TagStruct, View, class};
use crate::object::check_class;

pub use light::{Contrail, Decal, Light, MaterialEffects, PointState};
pub use particle::{EffectShader, Particle, PointPhysics};

/// Blocks' maxima (Constants).
pub const MAX_LOCATIONS: usize = 32;
pub const MAX_EVENTS: usize = 32;
pub const MAX_PARTS: usize = 32;
pub const MAX_PARTICLES: usize = 32;

/// A block longer than `max` is damage.
pub(crate) fn cap<T>(v: &View<'_>, offset: usize, items: Vec<T>, max: usize, what: &str) -> io::Result<Vec<T>> {
    if items.len() > max {
        return Err(v.error(offset, format_args!("{} {what} over the maximum {max}", items.len())));
    }
    Ok(items)
}

crate::tag_struct! {
    /// A location (element 0x20): a marker name ("" is the origin or the
    /// caller's first marker).
    pub struct Location: 0x20 {
        0x00 marker: String,
    }
}

crate::tag_struct! {
    /// A part (element 0x68): something made at once when its event starts.
    pub struct Part: 0x68 {
        /// 0 any environment, 1 air only, 2 water only, 3 space (never).
        0x00 environment: i16,
        /// 0 either, 1 violent only, 2 nonviolent only.
        0x02 mode: i16,
        /// Element of the locations block (outside it: skipped).
        0x04 location: i16,
        /// Bit 0: face down regardless of location (decals).
        0x06 flags: u16,
        /// Build-time copy of the type's base class (`snd!`, `deca`, `ligh`,
        /// `jpt!`, `pctl`, `obje`).
        0x14 base_class: ClassCode,
        0x18 kind: TagRef,
        /// Objects: WU per tick; decals: the trace length (WU); systems: WU/s.
        0x40 velocity: Bounds<f32>,
        0x48 velocity_cone: f32,
        0x4C angular_velocity: Bounds<f32>,
        /// Decals: the radius multiplier (0 means 1).
        0x54 radius_modifier: Bounds<f32>,
        0x60 a_scales: u32,
        0x64 b_scales: u32,
    }
}

crate::tag_struct! {
    /// A particle entry (element 0xE8): particles spread over the event.
    pub struct ParticleEntry: 0xE8 {
        0x00 environment: i16,
        0x02 mode: i16,
        /// 0 independent, 1 first person only, 2 third person only, 3 both.
        0x04 camera_mode: i16,
        0x08 location: i16,
        /// Yaw, pitch (rad) in the marker's frame.
        0x0C direction: [f32; 2],
        /// Forward, left, up (WU) in the marker's frame.
        0x14 offset: [f32; 3],
        /// Build-time unit vector of `direction`; used as stored.
        0x20 direction_vector: [f32; 3],
        0x54 particle: TagRef,
        /// Bit 0 stay attached; 1 random initial angle; 2 tint from object
        /// colour; 3 HSV; 4 long hue path.
        0x64 flags: u32,
        /// 0 start, 1 end, 2 constant, 3 buildup, 4 falloff, 5 both.
        0x68 distribution: i16,
        0x6C count: Bounds<i16>,
        0x70 distribution_radius: Bounds<f32>,
        /// WU per second.
        0x84 velocity: Bounds<f32>,
        0x8C velocity_cone: f32,
        /// Radians per second (sprite spin).
        0x90 angular_velocity: Bounds<f32>,
        /// WU.
        0xA0 radius: Bounds<f32>,
        0xB0 tint_lower: RealArgb,
        0xC0 tint_upper: RealArgb,
        0xE0 a_scales: u32,
        0xE4 b_scales: u32,
    }
}

impl ParticleEntry {
    pub const STAY_ATTACHED: u32 = 1;
    pub const RANDOM_ANGLE: u32 = 2;
}

crate::tag_struct! {
    /// The raw event (element 0x44), blocks unchecked.
    struct RawEvent: 0x44 {
        0x04 skip_fraction: f32,
        0x08 delay: Bounds<f32>,
        0x10 duration: Bounds<f32>,
        0x2C parts: Vec<Part>,
        0x38 particles: Vec<ParticleEntry>,
    }
}

/// An event: a delay, then its parts at once and its particles spread over
/// its duration (seconds).
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub skip_fraction: f32,
    pub delay: Bounds<f32>,
    pub duration: Bounds<f32>,
    pub parts: Vec<Part>,
    pub particles: Vec<ParticleEntry>,
}

crate::tag_struct! {
    struct RawEffect: 0x40 {
        0x00 flags: u32,
        0x04 loop_start: i16,
        0x06 loop_stop: i16,
        0x08 danger_radius: f32,
        0x28 locations: Vec<Location>,
        0x34 events: Vec<RawEvent>,
    }
}

/// An effect tag (`effe`, 0x40 bytes).
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    /// Bit 0 deleted when the attachment deactivates; bit 1 required for
    /// gameplay.
    pub flags: u32,
    /// -1: none.
    pub loop_start: i16,
    pub loop_stop: i16,
    pub danger_radius: f32,
    pub locations: Vec<String>,
    pub events: Vec<Event>,
}

impl Effect {
    pub const REQUIRED_FOR_GAMEPLAY: u32 = 2;

    /// Read an `effe` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Effect> {
        let v = check_class(map, tag, class::EFFECT, "effect", 0x40)?;
        Self::read_view(&v)
    }

    /// Read from a view of the tag's main struct.
    pub fn read_view(v: &View<'_>) -> io::Result<Effect> {
        // Counts first, so a damaged block is refused before it is read whole.
        for (off, max, what) in [(0x28, MAX_LOCATIONS, "locations"), (0x34, MAX_EVENTS, "events")] {
            let n = v.i32(off)?;
            if n < 0 || n as usize > max {
                return Err(v.error(off, format_args!("{n} {what} outside 0..={max}")));
            }
        }
        let raw = <RawEffect as TagStruct>::read(v)?;
        let events = raw
            .events
            .into_iter()
            .map(|e| {
                Ok(Event {
                    skip_fraction: e.skip_fraction,
                    delay: e.delay,
                    duration: e.duration,
                    parts: cap(v, 0x34, e.parts, MAX_PARTS, "parts")?,
                    particles: cap(v, 0x34, e.particles, MAX_PARTICLES, "particle entries")?,
                })
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Effect {
            flags: raw.flags,
            loop_start: raw.loop_start,
            loop_stop: raw.loop_stop,
            danger_radius: raw.danger_radius,
            locations: raw.locations.into_iter().map(|l| l.marker).collect(),
            events,
        })
    }

    /// Every tag the effect names (parts' types and particle entries'
    /// particles), in block order, nulls left out.
    pub fn references(&self) -> Vec<TagRef> {
        self.events
            .iter()
            .flat_map(|e| e.parts.iter().map(|p| p.kind).chain(e.particles.iter().map(|p| p.particle)))
            .filter(|r| !r.id.is_none())
            .collect()
    }
}
