//! Steam jets (`MapSteam`, Source's env_steam: specs/source/visual_entities.md
//! 7) as data: a jet's puffs' life, size, opacity and colour (the light
//! sampled along the jet, `light_ramp`). `render::steam` emits and
//! draws the puffs. Engine space: meters, seconds.
//!
//! Steam jets for any game (`MapSteam`, Source's env_steam): puffs growing per
//! second, sin fade over their life, coloured from light samples along the jet.

use bevy::prelude::*;

/// Lighting samples along a jet (steam_ramps).
pub const STEAM_RAMPS: usize = 5;

/// A steam jet. Engine space, meters, seconds.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct MapSteam {
    pub origin: Vec3,
    /// Unit vectors: where the jet points, and the two it spreads along.
    pub forward: Vec3,
    pub right: Vec3,
    pub up: Vec3,
    /// Sideways speed up to this along `right` and `up` (m/s).
    pub spread: f32,
    /// Speed along `forward` (m/s).
    pub speed: f32,
    /// Half-size at birth, and the size it reaches after one second (m).
    pub start_size: f32,
    pub end_size: f32,
    /// Puffs per second (read once: the Rate input changes nothing).
    pub rate: f32,
    /// How far the puffs get (m): their life is jet length / speed.
    pub length: f32,
    /// Roll speed up to this either way (rad/s).
    pub roll_speed: f32,
    /// Opacity, 0-1 (renderamt).
    pub alpha: f32,
    /// Linear colour at `STEAM_RAMPS` points from the origin to the jet's
    /// end (the world's light there × rendercolor, scaled to at most 1).
    pub ramp: [Vec3; STEAM_RAMPS],
    /// Index into `MapData::textures`.
    pub texture: Option<usize>,
    /// The entity it comes from (index into `MapData::entities`), which
    /// the logic turns on and off (`EntityPart`).
    pub entity: Option<usize>,
    /// Emitting at map start.
    pub start_on: bool,
    /// Type 1 (heat wave): the puffs are quads in the view plane that
    /// refract the scene, not smoke (`texture` and `ramp` are unused).
    pub heat_wave: bool,
}

impl MapSteam {
    /// Seconds a puff lives (JetLength / Speed); None when the jet makes
    /// none (speed or rate 0).
    pub fn life(&self) -> Option<f32> {
        (self.speed > 0.0 && self.rate > 0.0 && self.length > 0.0).then(|| self.length / self.speed)
    }

    /// A puff's half-size at `age` seconds: grows by `end - start` per
    /// second, not per life (the spec's quirk).
    pub fn size(&self, age: f32) -> f32 {
        self.start_size + (self.end_size - self.start_size) * age
    }

    /// A puff's opacity at `age` of `life`: sin(π·age/life) × alpha.
    pub fn opacity(&self, age: f32, life: f32) -> f32 {
        (std::f32::consts::PI * age / life).sin().max(0.0) * self.alpha
    }

    /// A puff's colour at `age` of `life`: the ramp at age/life.
    pub fn color(&self, age: f32, life: f32) -> Vec3 {
        let t = (age / life).clamp(0.0, 1.0) * (STEAM_RAMPS - 1) as f32;
        let i = (t as usize).min(STEAM_RAMPS - 2);
        self.ramp[i].lerp(self.ramp[i + 1], t - i as f32)
    }
}

/// The colour samples of a jet: the world's light at evenly spaced points
/// from `origin` to `end` × `color` (+ `color` when emissive, capped at
/// 1), each scaled so its largest channel is at most 1.
pub fn light_ramp(
    light: impl Fn(Vec3) -> Vec3,
    origin: Vec3,
    end: Vec3,
    color: Vec3,
    emissive: bool,
) -> [Vec3; STEAM_RAMPS] {
    std::array::from_fn(|i| {
        let p = origin.lerp(end, i as f32 / (STEAM_RAMPS - 1) as f32);
        let mut c = light(p) * color;
        if emissive {
            c = (c + color).min(Vec3::ONE);
        }
        let max = c.max_element();
        if max > 1.0 { c / max } else { c }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIT: f32 = 0.0254;

    fn jet(speed: f32, length: f32, rate: f32) -> MapSteam {
        MapSteam {
            origin: Vec3::ZERO,
            forward: Vec3::Y,
            right: Vec3::X,
            up: Vec3::Z,
            spread: 15.0 * UNIT,
            speed: speed * UNIT,
            start_size: 10.0 * UNIT,
            end_size: 25.0 * UNIT,
            rate,
            length: length * UNIT,
            roll_speed: 8f32.to_radians(),
            alpha: 1.0,
            ramp: [Vec3::ONE; STEAM_RAMPS],
            texture: None,
            entity: None,
            start_on: true,
            heat_wave: false,
        }
    }

    #[test]
    fn spec_sizes_and_alpha() {
        // Speed 120, JetLength 80: T = 0.667; at 1/3 s size 15, alpha 1;
        // at 0.6 s size 19, alpha sin(0.9π).
        let s = jet(120.0, 80.0, 26.0);
        let t = s.life().unwrap();
        assert!((t - 2.0 / 3.0).abs() < 1e-5);
        assert!((s.size(1.0 / 3.0) / UNIT - 15.0).abs() < 1e-3);
        assert!((s.opacity(1.0 / 3.0, t) - 1.0).abs() < 1e-5);
        assert!((s.size(0.6) / UNIT - 19.0).abs() < 1e-3);
        assert!((s.opacity(0.6, t) - 0.309).abs() < 1e-3);
        // Speed 60, JetLength 120 (T = 2): at 1.5 s, 32.5 (past EndSize).
        let slow = jet(60.0, 120.0, 26.0);
        assert!((slow.life().unwrap() - 2.0).abs() < 1e-5);
        assert!((slow.size(1.5) / UNIT - 32.5).abs() < 1e-3);
        // Speed or rate 0: no puffs.
        assert_eq!(jet(0.0, 80.0, 26.0).life(), None);
        assert_eq!(jet(120.0, 80.0, 0.0).life(), None);
    }

    #[test]
    fn colour_ramp_scales_to_one() {
        let ramp = light_ramp(
            |p| Vec3::splat(p.x * 4.0),
            Vec3::ZERO,
            Vec3::X,
            Vec3::new(1.0, 0.5, 0.25),
            false,
        );
        assert_eq!(ramp[0], Vec3::ZERO);
        // At x = 1: light 4 × colour = (4, 2, 1) → (1, 0.5, 0.25).
        assert!((ramp[4] - Vec3::new(1.0, 0.5, 0.25)).length() < 1e-5);
        // Emissive adds the colour (capped at 1).
        let lit = light_ramp(|_| Vec3::ZERO, Vec3::ZERO, Vec3::X, Vec3::splat(0.5), true);
        assert_eq!(lit[2], Vec3::splat(0.5));
        let s = MapSteam {
            ramp: [Vec3::ZERO, Vec3::ONE, Vec3::ONE, Vec3::ONE, Vec3::ONE],
            ..jet(120.0, 80.0, 26.0)
        };
        let t = s.life().unwrap();
        assert!((s.color(t / 8.0, t) - Vec3::splat(0.5)).length() < 1e-5);
    }
}
