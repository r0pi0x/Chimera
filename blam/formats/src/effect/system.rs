//! The particle system tag `pctl` (0x68 bytes; specs/halo_ce/effects.md
//! "Particle systems"): its particle types, each with its creation physics
//! and constants, its type states (durations, transitions, the state
//! variables: multipliers, the minimum count and the creation rate) and
//! its particle states (durations, transitions, bitmap and sequence, scale,
//! animation and rotation rates, colours, radius multiplier, point physics
//! and effect shader). Values are the tag's own (world units, seconds).
//!
//! Known shortcuts: the system's own physics constants and the type
//! states' physics blocks (empty on the disc) are not read.

use std::io;

use crate::cache::{Bounds, MapFile, RealArgb, TagEntry, TagRef, TagStruct, View, class};
use crate::object::check_class;

use super::{EffectShader, particle::shader_at};

/// Blocks' maxima: the spec's 8 states of each kind; types are not
/// bounded by the spec (4 used on the disc), 32 here.
pub const MAX_TYPES: usize = 32;
pub const MAX_STATES: usize = 8;
pub const MAX_CONSTANTS: usize = 16;

crate::tag_struct! {
    /// One physics constant (element 0x04).
    pub struct Constant: 0x04 {
        0x00 value: f32,
    }
}

crate::tag_struct! {
    /// A type state (element 0xC0): how long, the transition to the next,
    /// the state variables.
    pub struct TypeState: 0xC0 {
        0x20 duration: Bounds<f32>,
        0x28 transition: Bounds<f32>,
        0x34 scale_multiplier: f32,
        0x38 animation_rate_multiplier: f32,
        0x3C rotation_rate_multiplier: f32,
        0x40 color_multiplier: RealArgb,
        0x50 radius_multiplier: f32,
        0x54 minimum_count: f32,
        /// Particles per second.
        0x58 creation_rate: f32,
        0xB0 creation_physics: i16,
        0xB2 update_physics: i16,
    }
}

crate::tag_struct! {
    struct RawParticleState: 0x178 {
        0x20 duration: Bounds<f32>,
        0x28 transition: Bounds<f32>,
        0x30 bitmap: TagRef,
        0x40 sequence: i16,
        /// WU per bitmap pixel.
        0x48 scale: Bounds<f32>,
        0x50 animation_rate: Bounds<f32>,
        0x58 rotation_rate: Bounds<f32>,
        0x60 color_lower: RealArgb,
        0x70 color_upper: RealArgb,
        0x80 radius_multiplier: f32,
        0x84 physics: TagRef,
    }
}

/// A particle state (element 0x178).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleState {
    pub duration: Bounds<f32>,
    pub transition: Bounds<f32>,
    pub bitmap: TagRef,
    pub sequence: i16,
    pub scale: Bounds<f32>,
    pub animation_rate: Bounds<f32>,
    pub rotation_rate: Bounds<f32>,
    pub color_lower: RealArgb,
    pub color_upper: RealArgb,
    pub radius_multiplier: f32,
    pub physics: TagRef,
    pub shader: EffectShader,
}

crate::tag_struct! {
    struct RawType: 0x80 {
        0x20 flags: u32,
        0x24 initial_count: i16,
        0x28 render_mode: i16,
        0x2A orientation: i16,
        0x2C radius: f32,
        0x54 creation_physics: i16,
        0x58 physics_flags: u32,
    }
}

/// A particle type (element 0x80).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleType {
    /// The spec's table (bit 0 type states loop, 2 particle states loop, 8
    /// disabled, 16 / 17 not drawn in first / third person, ...).
    pub flags: u32,
    pub initial_count: i16,
    pub render_mode: i16,
    pub orientation: i16,
    /// WU.
    pub radius: f32,
    /// 0 default, 1 explosion, 2 jet.
    pub creation_physics: i16,
    /// Explosion: xy spread, z spread (WU), intensity (1/s); jet:
    /// velocity (WU/s), spread fraction, rotates up.
    pub constants: Vec<f32>,
    pub type_states: Vec<TypeState>,
    pub particle_states: Vec<ParticleState>,
}

impl ParticleType {
    pub const DISABLED: u32 = 1 << 8;
    pub const PARTICLE_STATES_LOOP: u32 = 1 << 2;
    pub const EXPLOSION: i16 = 1;
    pub const JET: i16 = 2;
}

crate::tag_struct! {
    struct RawSystem: 0x68 {
        0x38 physics: TagRef,
        0x48 update_physics: i16,
        0x4C physics_flags: u32,
    }
}

/// A particle system (`pctl`).
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSystem {
    /// `pphy`: moves a free system's own position.
    pub physics: TagRef,
    pub update_physics: i16,
    pub types: Vec<ParticleType>,
}

impl ParticleSystem {
    /// Read a `pctl` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<ParticleSystem> {
        let v = check_class(map, tag, class::PARTICLE_SYSTEM, "particle system", 0x68)?;
        Self::read_view(&v)
    }

    pub fn read_view(v: &View<'_>) -> io::Result<ParticleSystem> {
        let raw = <RawSystem as TagStruct>::read(v)?;
        let mut types = Vec::new();
        for tv in v.block_max(0x5C, 0x80, MAX_TYPES)?.iter() {
            let t = <RawType as TagStruct>::read(&tv)?;
            let constants = tv.block_max(0x5C, 0x04, MAX_CONSTANTS)?.read_all::<Constant>()?.into_iter().map(|c| c.value).collect();
            let type_states = tv.block_max(0x68, 0xC0, MAX_STATES)?.read_all::<TypeState>()?;
            let mut particle_states = Vec::new();
            for sv in tv.block_max(0x74, 0x178, MAX_STATES)?.iter() {
                let s = <RawParticleState as TagStruct>::read(&sv)?;
                particle_states.push(ParticleState {
                    duration: s.duration,
                    transition: s.transition,
                    bitmap: s.bitmap,
                    sequence: s.sequence,
                    scale: s.scale,
                    animation_rate: s.animation_rate,
                    rotation_rate: s.rotation_rate,
                    color_lower: s.color_lower,
                    color_upper: s.color_upper,
                    radius_multiplier: s.radius_multiplier,
                    physics: s.physics,
                    shader: shader_at(&sv, 0xB8)?,
                });
            }
            types.push(ParticleType {
                flags: t.flags,
                initial_count: t.initial_count,
                render_mode: t.render_mode,
                orientation: t.orientation,
                radius: t.radius,
                creation_physics: t.creation_physics,
                constants,
                type_states,
                particle_states,
            });
        }
        Ok(ParticleSystem { physics: raw.physics, update_physics: raw.update_physics, types })
    }
}
