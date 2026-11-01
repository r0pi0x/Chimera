//! Data-driven particle systems (Source's `.pcf` effects, run by
//! info_particle_system: specs/source/particles_and_smoke.md 1-2) as data,
//! for any game whose loader fills `MapParticleSystems`. A system
//! definition is a list of emitters, initializers, operators, forces and
//! renderers; their meanings follow specs/cs_source/impact_effects.md 14
//! and specs/source/fire.md 7.3 (the library that runs them is not public:
//! interpretations, listed in docs/tech-debt.md). `render::psys`
//! runs and draws them.
//!
//! Data-driven particle systems for any game (Source `.pcf` effects) as
//! data: `SystemDef` (emitters, initializers, operators, forces, renderer,
//! children with delays), `MapParticleSystems` (the definitions and the
//! placed info_particle_systems), `Pick` (a biased random pick),
//! `StartParticleSystem` (a one-shot effect a game starts at a point, e.g.
//! a splash; the presentation runs it).

use bevy::prelude::*;

use super::ParticleRng;

/// Control points a system can use.
pub const MAX_CONTROL_POINTS: usize = 8;

/// One particle system definition.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SystemDef {
    pub name: String,
    pub max_particles: usize,
    pub initial_particles: usize,
    /// Index into `MapData::particles::materials`.
    pub material: Option<usize>,
    /// Defaults every particle starts with (radius in units, colour and
    /// alpha 0-1, roll and roll speed in degrees).
    pub radius: f32,
    pub color: Vec3,
    pub alpha: f32,
    pub rotation: f32,
    pub rotation_speed: f32,
    pub sequence: u32,
    /// Largest step, seconds.
    pub max_step: f32,
    /// Not drawn (nor run) farther than this from the eye, units.
    pub max_draw_distance: f32,
    pub emitters: Vec<Emitter>,
    pub initializers: Vec<Init>,
    pub operators: Vec<Op>,
    pub renderer: Render,
    /// Child systems (index into the definitions) and their delay, s.
    pub children: Vec<(usize, f32)>,
}

impl Default for SystemDef {
    fn default() -> Self {
        Self {
            name: String::new(),
            max_particles: 1000,
            initial_particles: 0,
            material: None,
            radius: 5.0,
            color: Vec3::ONE,
            alpha: 1.0,
            rotation: 0.0,
            rotation_speed: 0.0,
            sequence: 0,
            max_step: 0.1,
            max_draw_distance: 100_000.0,
            emitters: Vec::new(),
            initializers: Vec::new(),
            operators: Vec::new(),
            renderer: Render::None,
            children: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Emitter {
    /// `rate` a second from `start` for `duration` s (0: forever).
    Continuously { start: f32, rate: f32, duration: f32 },
    /// `count` at `start`.
    Instantaneously { start: f32, count: u32 },
}

/// A uniform pick between two values with an exponent on the fraction.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Pick {
    pub min: f32,
    pub max: f32,
    pub exp: f32,
}

impl Pick {
    pub fn new(min: f32, max: f32, exp: f32) -> Self {
        Self { min, max, exp }
    }

    pub fn draw(&self, rng: &mut ParticleRng) -> f32 {
        let f = rng.random();
        let f = if self.exp == 1.0 || self.exp <= 0.0 {
            f
        } else {
            f.powf(self.exp)
        };
        self.min + (self.max - self.min) * f
    }
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Init {
    /// Position at a random direction × distance from the control point
    /// (scaled per axis by `bias`; an axis with `bias_abs` set takes the
    /// absolute value: a half sphere), velocity outward × speed plus a
    /// random vector between the two local speeds in the point's frame.
    Sphere {
        cp: usize,
        distance: Pick,
        bias: Vec3,
        bias_abs: Vec3,
        speed: Pick,
        local_min: Vec3,
        local_max: Vec3,
    },
    /// Position in a box about the control point.
    Box {
        cp: usize,
        min: Vec3,
        max: Vec3,
    },
    /// Position moved by U(min, max) per axis (× radius when
    /// proportional, in the point's frame when local).
    Offset {
        cp: usize,
        min: Vec3,
        max: Vec3,
        local: bool,
        proportional: bool,
    },
    /// Velocity: a random direction × speed plus a random local vector.
    Velocity {
        cp: usize,
        speed: Pick,
        local_min: Vec3,
        local_max: Vec3,
    },
    /// Velocity per axis in [min, max] (from noise: here uniform).
    VelocityNoise {
        cp: usize,
        min: Vec3,
        max: Vec3,
        local: bool,
    },
    Lifetime(Pick),
    /// Life = the chosen sheet sequence's frames / fps.
    LifetimeFromSequence {
        fps: f32,
    },
    Radius(Pick),
    /// A blend between two colours (0-1), then `tint` (`tint_perc`) of the
    /// way toward the light at control point `cp` (gamma space:
    /// specs/cs_source/impact_effects.md 14).
    Color {
        a: Vec3,
        b: Vec3,
        tint: f32,
        cp: usize,
    },
    /// Alpha 0-1.
    Alpha(Pick),
    /// Roll = initial + U(min, max), degrees.
    Rotation {
        initial: f32,
        offset: Pick,
    },
    /// Mirror this fraction of particles.
    YawFlip {
        fraction: f32,
    },
    Sequence {
        min: u32,
        max: u32,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Op {
    /// Gravity (units/s²), and drag: the fraction of velocity removed per
    /// step.
    Movement {
        gravity: Vec3,
        drag: f32,
    },
    /// Dies at the end of its life.
    Decay,
    /// Alpha × 0→1 over a random time (fraction of life when
    /// proportional) from birth.
    FadeIn {
        time: Pick,
        proportional: bool,
    },
    /// Alpha × 1→0 over a random time before death.
    FadeOut {
        time: Pick,
        proportional: bool,
    },
    /// Alpha × start→1 over [fade_in], 1→end over [fade_out] (fractions
    /// of life); dies at the end of its life.
    FadeAndDecay {
        start_alpha: f32,
        end_alpha: f32,
        fade_in: (f32, f32),
        fade_out: (f32, f32),
    },
    /// Colour toward `color` between the two fractions of life.
    ColorFade {
        color: Vec3,
        start: f32,
        end: f32,
        ease: bool,
    },
    /// Radius × lerp(start_scale, end_scale, bias(x)) between the two
    /// fractions of life.
    RadiusScale {
        start_scale: f32,
        end_scale: f32,
        start: f32,
        end: f32,
        bias: f32,
        ease: bool,
    },
    /// Roll by the particle's roll speed.
    RotationBasic,
    /// Roll (or yaw) at this rate, degrees/s.
    SpinRoll {
        rate: f32,
    },
    SpinYaw {
        rate: f32,
    },
    /// Speed at most this, units/s.
    MaxVelocity {
        max: f32,
    },
    /// Moves with the control point.
    LockToControlPoint {
        cp: usize,
    },
    /// Acceleration toward the control point: force / distance^falloff.
    Pull {
        cp: usize,
        force: f32,
        falloff: f32,
    },
    /// A random acceleration per axis in [min, max] each step.
    RandomForce {
        min: Vec3,
        max: Vec3,
    },
    /// A tangential acceleration about the axis through the control point.
    Twist {
        cp: usize,
        force: f32,
        axis: Vec3,
        local: bool,
    },
}

#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum Render {
    None,
    /// Camera-facing (orientation 0) or flat (others) sprites from the
    /// material's sheet: `rate` frames a second when `as_fps`, the
    /// sequence over the particle's life when `fit_life`, else `rate`
    /// times the sequence a second.
    Sprites {
        rate: f32,
        as_fps: bool,
        fit_life: bool,
        orientation: i32,
    },
    /// A streak back along the velocity, between the two lengths (units).
    Trail {
        min_length: f32,
        max_length: f32,
        length_fade_in: f32,
    },
    /// A strip through the particles in birth order.
    Rope,
}

/// The loaded map's particle systems: definitions and the
/// info_particle_systems placed in it.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MapParticleSystems {
    pub defs: Vec<SystemDef>,
    pub placed: Vec<PlacedSystem>,
    /// Meters per game unit.
    pub scale: f32,
}

impl MapParticleSystems {
    pub fn find(&self, name: &str) -> Option<usize> {
        self.defs.iter().position(|d| d.name.eq_ignore_ascii_case(name))
    }
}

/// An info_particle_system.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlacedSystem {
    /// The entity (index into `MapData::entities`).
    pub entity: Option<usize>,
    /// Engine space.
    pub position: Vec3,
    pub rotation: Quat,
    /// Index into `MapParticleSystems::defs`.
    pub system: usize,
    pub start_on: bool,
}

/// Start a one-shot particle system by its definition's name at
/// `position` (engine space), its forward axis along `forward` (engine
/// space). The presentation runs it to its end (`render::psys`); a name
/// the map's definitions lack shows nothing. Games load the systems they
/// start with the map's.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct StartParticleSystem {
    pub name: String,
    pub position: Vec3,
    pub forward: Vec3,
}
