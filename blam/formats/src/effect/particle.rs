//! The particle tag `part` (0x164 bytes) with its effect shader part, and
//! point physics `pphy` (0x40 bytes) (specs/halo_ce/effects.md, "The
//! particle tag", "Point physics"): the fields a particle's look, life and
//! motion read; the physics' runtime words as stored (the spec: use them).

use std::io;

use crate::cache::{Bounds, MapFile, TagEntry, TagRef, TagStruct, View, class};
use crate::object::check_class;

crate::tag_struct! {
    /// The effect shader part (0xB4 bytes) of particles, particle-system
    /// states and contrails: its flags, blend and fade (the secondary map
    /// is left out).
    pub struct EffectShader: 0xB4 {
        /// Bit 1 nonlinear tint (names inferred).
        0x28 flags: u16,
        /// `shaders.md`'s framebuffer blend function (0 alpha blend, 3 add, ...).
        0x2A blend: i16,
        /// 0 none, 1 fade when perpendicular, 2 fade when parallel.
        0x2C fade_mode: i16,
        0x2E map_flags: u16,
    }
}

impl EffectShader {
    pub const ALPHA_BLEND: i16 = 0;
    pub const ADD: i16 = 3;
    pub const NONLINEAR_TINT: u16 = 2;
}

/// Read an effect shader part at `offset` of `v`.
pub(crate) fn shader_at(v: &View<'_>, offset: usize) -> io::Result<EffectShader> {
    Ok(EffectShader {
        flags: v.u16(offset + 0x28)?,
        blend: v.i16(offset + 0x2A)?,
        fade_mode: v.i16(offset + 0x2C)?,
        map_flags: v.u16(offset + 0x2E)?,
    })
}

crate::tag_struct! {
    struct RawParticle: 0x164 {
        0x000 flags: u32,
        0x004 bitmap: TagRef,
        0x014 physics: TagRef,
        0x024 material_effects: TagRef,
        0x038 lifespan: Bounds<f32>,
        0x040 fade_in: f32,
        0x044 fade_out: f32,
        0x048 collision_effect: TagRef,
        0x058 death_effect: TagRef,
        0x068 minimum_size: f32,
        0x074 radius_animation: Bounds<f32>,
        0x080 animation_rate: Bounds<f32>,
        0x088 contact_deterioration: f32,
        0x08C fade_start_size: f32,
        0x090 fade_end_size: f32,
        0x098 first_sequence: i16,
        0x09A initial_sequences: i16,
        0x09C looping_sequences: i16,
        0x09E final_sequences: i16,
        0x0A8 sprite_size_inverse: f32,
        0x0AC orientation: i16,
    }
}

/// A particle tag (`part`).
#[derive(Clone, Debug, PartialEq)]
pub struct Particle {
    /// `Particle::ANIMATE_ONCE_PER_FRAME` and the others (the spec's table).
    pub flags: u32,
    /// `bitm`: a sprite group.
    pub bitmap: TagRef,
    /// `pphy`.
    pub physics: TagRef,
    /// `foot`: impact kind 8 on hitting the level.
    pub material_effects: TagRef,
    /// Seconds.
    pub lifespan: Bounds<f32>,
    pub fade_in: f32,
    pub fade_out: f32,
    pub collision_effect: TagRef,
    pub death_effect: TagRef,
    /// Pixels.
    pub minimum_size: f32,
    /// Radius multiplier at age 0 and at the end of the lifespan.
    pub radius_animation: Bounds<f32>,
    /// Frames per second (an upper bound of 0: never animates).
    pub animation_rate: Bounds<f32>,
    pub contact_deterioration: f32,
    pub fade_start_size: f32,
    pub fade_end_size: f32,
    pub first_sequence: i16,
    pub initial_sequences: i16,
    pub looping_sequences: i16,
    pub final_sequences: i16,
    /// 1 / the largest sprite dimension (pixels).
    pub sprite_size_inverse: f32,
    /// 0 screen facing, 1 parallel to direction, 2 perpendicular.
    pub orientation: i16,
    pub shader: EffectShader,
}

impl Particle {
    pub const ANIMATES_BACKWARDS: u32 = 1 << 0;
    pub const STOPS_AT_REST: u32 = 1 << 1;
    pub const RANDOM_START_FRAME: u32 = 1 << 2;
    pub const ANIMATE_ONCE_PER_FRAME: u32 = 1 << 3;
    pub const DIES_AT_REST: u32 = 1 << 4;
    pub const DIES_ON_STRUCTURE: u32 = 1 << 5;
    pub const DIES_IN_WATER: u32 = 1 << 7;
    pub const DIES_IN_AIR: u32 = 1 << 8;
    pub const SELF_ILLUMINATED: u32 = 1 << 9;

    /// Read a `part` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Particle> {
        let v = check_class(map, tag, class::PARTICLE, "particle", 0x164)?;
        Self::read_view(&v)
    }

    pub fn read_view(v: &View<'_>) -> io::Result<Particle> {
        let r = <RawParticle as TagStruct>::read(v)?;
        if !(0..=2).contains(&r.orientation) {
            return Err(v.error(0xAC, format_args!("orientation {} outside 0..=2", r.orientation)));
        }
        Ok(Particle {
            flags: r.flags,
            bitmap: r.bitmap,
            physics: r.physics,
            material_effects: r.material_effects,
            lifespan: r.lifespan,
            fade_in: r.fade_in,
            fade_out: r.fade_out,
            collision_effect: r.collision_effect,
            death_effect: r.death_effect,
            minimum_size: r.minimum_size,
            radius_animation: r.radius_animation,
            animation_rate: r.animation_rate,
            contact_deterioration: r.contact_deterioration,
            fade_start_size: r.fade_start_size,
            fade_end_size: r.fade_end_size,
            first_sequence: r.first_sequence,
            initial_sequences: r.initial_sequences,
            looping_sequences: r.looping_sequences,
            final_sequences: r.final_sequences,
            sprite_size_inverse: r.sprite_size_inverse,
            orientation: r.orientation,
            shader: shader_at(v, 0xB0)?,
        })
    }

    /// Whether flag `bit` is set.
    pub fn has(&self, bit: u32) -> bool {
        self.flags & bit != 0
    }

    /// Fade (0..1) at age `a` of a particle that lives `l` seconds (the
    /// spec's "Size, alpha and drawing", without the view-angle fade).
    pub fn fade(&self, a: f32, l: f32) -> f32 {
        let mut f = 1.0;
        if self.fade_in > 0.0 && a < self.fade_in {
            f = a / self.fade_in;
        }
        if self.fade_out > 0.0 && l - a < self.fade_out {
            f *= ((l - a) / self.fade_out).max(0.0);
        }
        f
    }

    /// The radius multiplier at age `a` of a lifespan `l`.
    pub fn radius_factor(&self, a: f32, l: f32) -> f32 {
        let t = if l > 0.0 { a / l } else { 1.0 };
        self.radius_animation.lower + (self.radius_animation.upper - self.radius_animation.lower) * t
    }
}

crate::tag_struct! {
    /// Point physics (`pphy`, 0x40 bytes).
    pub struct PointPhysics: 0x40 {
        /// Bit 0 flamethrower collision; 1 collides with structures; 2
        /// with water surfaces; 3 simple wind; 4 damped wind; 5 no gravity.
        0x00 flags: u32,
        /// Runtime: density x 118613.34 (kg/WU^3).
        0x04 mass_per_r3: f32,
        0x08 water_buoyancy: f32,
        0x0C air_buoyancy: f32,
        0x20 density: f32,
        0x24 air_friction: f32,
        0x28 water_friction: f32,
        0x2C surface_friction: f32,
        0x30 elasticity: f32,
    }
}

impl PointPhysics {
    pub const COLLIDES_WITH_STRUCTURES: u32 = 1 << 1;
    pub const NO_GRAVITY: u32 = 1 << 5;
    /// The engine's gravity, WU/s^2.
    pub const GRAVITY: f32 = 3.208_661_3;
    /// Mass of the displaced air per r^3 (kg/WU^3).
    pub const AIR_MASS_PER_R3: f32 = 130.474_74;

    /// Read a `pphy` tag.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<PointPhysics> {
        let v = check_class(map, tag, class::POINT_PHYSICS, "point physics", 0x40)?;
        <PointPhysics as TagStruct>::read(&v)
    }

    /// One step in air with no wind and no collision (the spec's steps 2 to
    /// 4 then the move): radius `r` (WU), position and velocity in WU and
    /// WU/s, `dt` seconds.
    pub fn step_air(&self, r: f32, position: [f32; 3], velocity: [f32; 3], dt: f32) -> ([f32; 3], [f32; 3]) {
        if dt <= 0.0 {
            return (position, velocity);
        }
        let r3 = r * r * r;
        let m = (self.mass_per_r3 + Self::AIR_MASS_PER_R3) * r3;
        let b = if self.flags & Self::NO_GRAVITY != 0 { 0.0 } else { self.air_buoyancy };
        let c = self.air_friction * r * r;
        let mut v = velocity;
        v[2] += Self::GRAVITY * b * dt;
        let k = if m > 0.0 {
            (dt * c / m).clamp(0.0, 1.0)
        } else if c != 0.0 {
            1.0
        } else {
            0.0
        };
        for x in &mut v {
            *x -= *x * k;
        }
        let p = [position[0] + v[0] * dt, position[1] + v[1] * dt, position[2] + v[2] * dt];
        (p, v)
    }

    /// A bounce off a surface of unit normal `n`: the tangential part keeps
    /// 1 - surface friction, the normal part reverses times the elasticity.
    pub fn bounce(&self, v: [f32; 3], n: [f32; 3]) -> [f32; 3] {
        let d = v[0] * n[0] + v[1] * n[1] + v[2] * n[2];
        let vn = [n[0] * d, n[1] * d, n[2] * d];
        std::array::from_fn(|i| (1.0 - self.surface_friction) * (v[i] - vn[i]) - self.elasticity * vn[i])
    }
}
