//! Map effects drawn through the particle pool, for any game, as data:
//! sprite trails (Source's env_spritetrail, specs/source/
//! visual_entities.md 5), smoke stacks (env_smokestack,
//! specs/source/particles_and_smoke.md 3), tesla arcs (point_tesla,
//! public entity docs), embers (env_embers), muzzle flashes
//! (env_muzzleflash) and strike-generator env_beams' short-lived beams
//! (visual_entities.md 1, 4.2). `render::emitters` runs and draws
//! them. Engine space: meters, seconds.
//!
//! Map effects for any game as data: `MapTrail`, `MapSmokeStack` (a puff's
//! life, size and opacity), `MapTesla`, `MapEmbers`, `MapMuzzleFlash`,
//! `MapStrike` (with the beam's segment count and fractal noise, which
//! `render::beams` uses too).

use bevy::prelude::*;

use super::ParticleRng;

/// Points a trail keeps (trail_max_points).
pub const TRAIL_MAX_POINTS: usize = 256;
/// A new trail point needs the entity to have moved more than this (2
/// units), meters.
pub const TRAIL_MIN_STEP: f32 = 2.0 * 0.0254;
/// Seconds a smoke stack simulates and emits unseen after it appears.
pub const SMOKESTACK_FULL_SIM: f32 = 5.0;

/// A sprite trail (env_spritetrail).
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapTrail {
    /// The entity (index into `MapData::entities`).
    pub entity: Option<usize>,
    /// Where it starts, engine space.
    pub position: Vec3,
    /// Index into `MapData::particles::materials`.
    pub material: usize,
    /// Seconds a point lives.
    pub life: f32,
    /// Full widths at the newest and oldest end, meters; no end width:
    /// the start width all along.
    pub start_width: f32,
    pub end_width: Option<f32>,
    /// rendercolor / 255 (sRGB) and renderamt / 255.
    pub color: Vec3,
    pub alpha: f32,
}

/// A smoke stack (env_smokestack). Directions are unit vectors in engine
/// space; lengths meters; speeds m/s.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapSmokeStack {
    pub entity: Option<usize>,
    pub position: Vec3,
    /// The entity's angles (the column rises along their up).
    pub rotation: Quat,
    pub material: usize,
    /// The ring puffs start on, and the sideways speed range.
    pub base_spread: f32,
    pub spread_speed: f32,
    /// Rising speed (along the entity's up).
    pub speed: f32,
    /// Half-widths at birth and at the end of life.
    pub start_size: f32,
    pub end_size: f32,
    /// Puffs per second.
    pub rate: f32,
    /// How far the puffs rise: their life is length / speed.
    pub length: f32,
    /// Acceleration (wind), m/s².
    pub wind: Vec3,
    /// Twist about the vertical axis through the origin, degrees per
    /// second (positive: clockwise seen from above).
    pub twist: f32,
    /// Roll speed up to this either way, degrees per second.
    pub roll: f32,
    /// Colour (sRGB, 0-1) and opacity scale (renderamt / 255).
    pub color: Vec3,
    pub alpha: f32,
    pub start_on: bool,
}

impl MapSmokeStack {
    /// A puff's life, seconds (none: the stack makes no puffs).
    pub fn life(&self) -> Option<f32> {
        (self.speed > 0.0 && self.length > 0.0).then(|| self.length / self.speed)
    }

    /// Half-width at life fraction `t`.
    pub fn size(&self, t: f32) -> f32 {
        self.start_size + (self.end_size - self.start_size) * t
    }

    /// Opacity (0-1) at life fraction `t`: ½ − ½cos 2πt, squared past the
    /// middle, times renderamt.
    pub fn opacity(&self, t: f32) -> f32 {
        let a = 0.5 - 0.5 * (std::f32::consts::TAU * t).cos();
        self.alpha * if t <= 0.5 { a } else { a * a }
    }
}

/// A point_tesla (public entity docs): arcs from its origin to surfaces
/// within its radius, each spark a random number of them.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapTesla {
    pub entity: Option<usize>,
    pub position: Vec3,
    pub material: usize,
    /// sRGB 0-1.
    pub color: Vec3,
    /// Meters.
    pub radius: f32,
    /// Arc widths (meters), lives (seconds) and counts, each a range.
    pub width: (f32, f32),
    pub life: (f32, f32),
    pub beams: (u32, u32),
}

/// env_embers (no spec; public entity docs): embers born at random in its
/// volume, `density` a second, each living `lifetime` seconds and drifting
/// at `speed` along the entity's facing.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapEmbers {
    pub entity: Option<usize>,
    /// The entity's origin, engine space: where its node is placed.
    pub origin: Vec3,
    /// The volume's bounds, engine space (meters).
    pub min: Vec3,
    pub max: Vec3,
    pub material: usize,
    /// Per second.
    pub density: f32,
    pub life: f32,
    /// Engine space, meters per second.
    pub velocity: Vec3,
    pub color: Vec3,
    /// 0 normal, 1 smooth fade, 2 pulled (speeding up as it goes).
    pub kind: i32,
    pub start_on: bool,
}

/// env_muzzleflash (no spec; public entity docs): Fire flashes a few
/// view-facing sprites along its facing, "scale" times the size.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapMuzzleFlash {
    pub entity: Option<usize>,
    pub position: Vec3,
    pub rotation: Quat,
    pub material: usize,
    pub scale: f32,
}

/// A strike-generator env_beam's look (visual_entities.md 4.2): its
/// strikes are beams of this width, colour and noise.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct MapStrike {
    /// The entity (index into `MapData::entities`).
    pub entity: usize,
    /// Index into `MapData::particles::materials` (additive).
    pub material: usize,
    /// BoltWidth, meters: the strip is twice as wide (1 step 7).
    pub width: f32,
    /// rendercolor / 255 x renderamt / 255 (the brightness).
    pub color: Vec3,
    /// NoiseAmplitude, 0-64.
    pub noise: f32,
}

/// Samples of a beam's noise (noise_divisions + 1).
pub const BEAM_NOISE_SAMPLES: usize = 129;
/// Most segments a beam is drawn in (noise_divisions).
pub const BEAM_MAX_SEGMENTS: usize = 128;

/// How many points a beam of `length` units, widths `width`/`end_width`
/// (units) and noise amplitude `noise` is drawn through (1 step 4).
pub fn beam_segments(length: f32, width: f32, end_width: f32, noise: f32) -> usize {
    let mut n = if noise >= 0.5 {
        (length * 0.25 + 3.0) as usize
    } else {
        (length * 0.075 + 3.0) as usize
    };
    let h = width.max(end_width) / 2.0;
    if h > 0.0 && length / ((n.max(2) - 1) as f32) < 1.414 * h {
        n = ((length / (1.414 * h)) as usize + 1).max(2);
    }
    n.min(BEAM_MAX_SEGMENTS)
}

/// Fractal noise (1 step 5): the ends 0, each span's middle the mean of
/// its ends plus U(-1, 1) x k, k halving at each subdivision.
pub fn fractal_noise(rng: &mut ParticleRng) -> [f32; BEAM_NOISE_SAMPLES] {
    fn split(n: &mut [f32; BEAM_NOISE_SAMPLES], a: usize, b: usize, k: f32, rng: &mut ParticleRng) {
        if b - a <= 1 {
            return;
        }
        let mid = (a + b) / 2;
        n[mid] = (n[a] + n[b]) * 0.5 + rng.float(-1.0, 1.0) * k;
        split(n, a, mid, k * 0.5, rng);
        split(n, mid, b, k * 0.5, rng);
    }
    let mut n = [0.0; BEAM_NOISE_SAMPLES];
    split(&mut n, 0, BEAM_NOISE_SAMPLES - 1, 1.0, rng);
    n
}
