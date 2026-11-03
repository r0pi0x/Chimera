//! The Source explosion effect (specs/cs_source/grenades.md 5.4, the
//! temp entity "explosion" every Source game draws the same way; lifted
//! from CS:S's grenades for HL2:DM, specs/hl2dm/effects_sounds.md 8): core
//! smoke puffs coloured by the world's light, embers, fireballs, spark
//! streaks and cement flecks (`explosion`), sprayed away from the nearby
//! walls and floor (`blast_direction`), plus the flash overlay's glow. On
//! `map::particles`. The sound (`BaseExplosionEffect.Sound`) is the
//! caller's.

use bevy::prelude::*;

use crate::map::{
    LightField, ParticleRng,
    particles::{
        Bounce, Fade, MapParticles, Motion, Particle, ParticleGroup, Ramp, Shape, WorldTrace, probe_planes,
    },
};

const UNIT: f32 = 0.0254;

/// Particle materials the effect uses (loaded with the map's).
pub const MATERIALS: &[&str] = &[
    "effects/fire_embers1",
    "effects/fire_embers2",
    "effects/fire_cloud2",
    "particle/particle_smokegrenade1",
    "sprites/light_glow02_add_noz",
];

/// Particle materials by role.
#[derive(Clone, Debug, Default)]
pub struct FxMaterials {
    pub noise: Option<usize>,
    pub embers: [Option<usize>; 2],
    pub fire: Option<usize>,
    pub cement: [Option<usize>; 2],
    pub glow: Option<usize>,
    pub cloud: Option<usize>,
}

impl FxMaterials {
    pub fn new(p: &MapParticles) -> Self {
        let f = |n: &str| p.find(n);
        Self {
            noise: f("particle/particle_noisesphere"),
            embers: [f("effects/fire_embers1"), f("effects/fire_embers2")],
            fire: f("effects/fire_cloud2"),
            cement: [f("effects/fleck_cement1"), f("effects/fleck_cement2")],
            glow: f("sprites/light_glow02_add_noz").or_else(|| f("particle/particle_glow_03")),
            cloud: f("particle/particle_smokegrenade1").or_else(|| f("particle/particle_smokegrenade")),
        }
    }
}

/// The blast's direction (spec 5.4): probes 100 units along each axis;
/// each hit pushes away from its surface by how close it is.
pub fn blast_direction(at: Vec3, world: &impl WorldTrace) -> Vec3 {
    let mut sum = Vec3::ZERO;
    for dir in [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z] {
        if let Some((f, _)) = world.trace(at, at + dir * 100.0 * UNIT) {
            sum -= dir * (1.0 - f);
        }
    }
    sum.try_normalize().unwrap_or(Vec3::Y)
}

/// Light at a point as a colour (0-1), from the map's light field.
pub fn light_at(field: Option<&LightField>, p: Vec3) -> Vec3 {
    let Some(field) = field else { return Vec3::splat(0.6) };
    let probe = (field.0.0)(p);
    let dirs = [Vec3::X, Vec3::NEG_X, Vec3::Y, Vec3::NEG_Y, Vec3::Z, Vec3::NEG_Z];
    (dirs.iter().map(|d| probe.eval(*d)).sum::<Vec3>() / 6.0).min(Vec3::ONE)
}

fn rand_unit_cube(rng: &mut ParticleRng, s: f32) -> Vec3 {
    rng.vec3(-s, s)
}

/// The HE explosion's particles at `b` blowing along `a` (spec 5.4: core
/// smoke, embers, fireballs, sparks, flecks), lit by `light`.
pub fn explosion(
    rng: &mut ParticleRng,
    m: &FxMaterials,
    b: Vec3,
    a: Vec3,
    light: Vec3,
    world: &impl WorldTrace,
) -> Vec<ParticleGroup> {
    let mut out = Vec::new();
    let sigma = 1.0 - 0.15 * 2.0;
    // Smoke, embers and fireballs share the core's motion: speed decaying
    // to 1/10000 in 0.5 s but not below 32 units/s, roll slowing.
    let mut core = ParticleGroup::new(Motion {
        decay: Some((0.5, 32.0 * UNIT)),
        roll_decay: Some((8.0, 0.5)),
        ..default()
    });
    // Smoke takes the world's light colour at its luminance.
    let lum = (light.dot(Vec3::new(0.299, 0.587, 0.114)) * 255.0).clamp(1.0, 255.0);
    let tint = light / light.max_element().max(1e-3);
    let smoke_color = |rng: &mut ParticleRng| tint * rng.int((lum / 2.0) as i32, lum as i32) as f32 / 255.0;
    let fade = Fade::Power(2.0);
    if let Some(noise) = m.noise {
        for _ in 0..4 {
            let dir = (rand_unit_cube(rng, sigma) + a * rng.float(1.0, 6.0)).normalize_or(Vec3::Y);
            let speed = rng.float(1.0, 750.0) * 2.0 * sigma * dir.dot(a).abs();
            let color = smoke_color(rng);
            core.particles.push(Particle {
                velocity: dir * speed * UNIT,
                color,
                fade,
                size: Ramp::linear(72.0 * UNIT, 144.0 * UNIT),
                roll: (rng.int(0, 360) as f32).to_radians(),
                roll_speed: rng.float(-2.0, 2.0),
                ..Particle::new(b, rng.float(2.0, 3.0), noise, 0.0)
            });
        }
        for _ in 0..8 {
            let at = b + rand_unit_cube(rng, 16.0 * UNIT);
            let dir = (rand_unit_cube(rng, sigma) + a * rng.float(1.0, 6.0)).normalize_or(Vec3::Y);
            let speed = rng.float(1.0, 2000.0) * 2.0 * sigma * dir.dot(a).abs();
            let size = rng.int(32, 64) as f32;
            let color = smoke_color(rng);
            core.particles.push(Particle {
                velocity: dir * speed * UNIT,
                color,
                alpha: rng.float(128.0, 255.0) / 255.0,
                fade,
                size: Ramp::linear(size * UNIT, size * 2.0 * UNIT),
                roll_speed: rng.float(-8.0, 8.0),
                ..Particle::new(at, rng.float(0.5, 1.0), noise, 0.0)
            });
        }
        for i in 0..32 {
            let yaw = (11.25 * (i + 1) as f32).to_radians();
            let h = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
            let at = b + rand_unit_cube(rng, 4.0 * UNIT) + h * rng.float(8.0, 16.0) * UNIT;
            let speed = rng.float(500.0, 2000.0) * 2.0 * sigma;
            let size = rng.int(16, 32) as f32;
            let color = smoke_color(rng);
            core.particles.push(Particle {
                velocity: h * speed * UNIT,
                color,
                alpha: rng.float(16.0, 32.0) / 255.0,
                fade,
                size: Ramp::linear(size * UNIT, size * 4.0 * UNIT),
                roll_speed: rng.float(-8.0, 8.0),
                ..Particle::new(at, rng.float(0.5, 1.5), noise, 0.0)
            });
        }
    }
    for _ in 0..16 {
        let pick = rng.int(0, 1) as usize;
        let Some(mat) = m.embers[pick].or(m.embers[1 - pick]) else {
            break;
        };
        let at = b + rand_unit_cube(rng, 32.0 * UNIT);
        let dir = (rand_unit_cube(rng, 1.4) + a).normalize_or(Vec3::Y);
        let k = sigma * dir.dot(a).abs();
        let speed = rng.float(1.0, 400.0) * 8.0 * k * k;
        let grey = rng.int(192, 255) as f32 / 255.0;
        let size = (rng.int(8, 16) as f32 * k).clamp(4.0, 32.0);
        core.particles.push(Particle {
            velocity: dir * speed * UNIT,
            color: Vec3::splat(grey),
            fade,
            roll_speed: rng.float(-8.0, 8.0),
            ..Particle::new(at, rng.float(2.0, 3.0), mat, size * UNIT)
        });
    }
    if let Some(fire) = m.fire {
        for _ in 0..32 {
            let at = b + rand_unit_cube(rng, 48.0 * UNIT);
            let dir = (rand_unit_cube(rng, 0.525) + a).normalize_or(Vec3::Y);
            let k = sigma * dir.dot(a).abs();
            let speed = rng.float(400.0, 800.0) * 8.0 * k * k;
            let grey = rng.int(128, 255) as f32 / 255.0;
            let size = (rng.int(32, 85) as f32 * k).clamp(32.0, 85.0);
            core.particles.push(Particle {
                velocity: dir * speed * UNIT,
                color: Vec3::splat(grey),
                fade,
                size: Ramp::linear(size * UNIT, size * 1.5 * UNIT),
                roll_speed: rng.float(-16.0, 16.0),
                ..Particle::new(at, rng.float(0.2, 0.4), fire, 0.0)
            });
        }
        // Debris: spark streaks.
        let mut sparks = ParticleGroup::new(Motion {
            gravity: 200.0 * UNIT,
            damping: 8.0,
            ..default()
        });
        for _ in 0..rng.int(8, 16) {
            let dir = (rand_unit_cube(rng, 1.0) + a).normalize_or(Vec3::Y);
            sparks.particles.push(Particle {
                velocity: dir * rng.float(1500.0, 2500.0) * UNIT,
                fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
                shape: Shape::Trail {
                    width: rng.float(2.0, 16.0) * UNIT,
                    length: rng.float(0.05, 0.1),
                },
                ..Particle::new(b, rng.float(0.1, 0.15), fire, 0.0)
            });
        }
        out.push(sparks);
    }
    // The flash: one glow, fading out in 0.1 s while growing 16 units/s
    // (its start size isn't in the spec: ours).
    if let Some(glow) = m.glow {
        core.particles.push(Particle {
            color: Vec3::new(1.0, 0.9, 0.7),
            fade: Fade::Ramp(Ramp::linear(1.0, 0.0)),
            size: Ramp::linear(96.0 * UNIT, (96.0 + 1.6) * UNIT),
            ..Particle::new(b, 0.1, glow, 0.0)
        });
    }
    out.push(core);
    // Cement flecks bouncing off the world.
    if m.cement.iter().any(Option::is_some) {
        let origin = b + a * 16.0 * UNIT;
        let planes = probe_planes(origin, None, 256.0 * UNIT, 800.0 * UNIT, world);
        let mut g = ParticleGroup::new(Motion {
            gravity: 800.0 * UNIT,
            bounce: Some(Bounce {
                keep: 0.3,
                land_normal_y: 0.5,
                land_speed: 48.0 * UNIT,
                planes,
            }),
            ..default()
        });
        for _ in 0..rng.int(16, 32) {
            let pick = rng.int(0, 1) as usize;
            let Some(mat) = m.cement[pick].or(m.cement[1 - pick]) else {
                break;
            };
            let at = origin + rand_unit_cube(rng, 8.0 * UNIT);
            let dir = (rand_unit_cube(rng, 1.0) + a).normalize_or(Vec3::Y);
            let size = rng.int(1, 3);
            let k = 0.8 * dir.dot(a).abs();
            let speed = rng.float(64.0, 256.0) * (4 - size) as f32 * 8.0 * k * k;
            let grey = (0.25 * rng.float(0.5, 1.5)).min(1.0);
            g.particles.push(Particle {
                velocity: dir * speed * UNIT,
                color: Vec3::splat(grey),
                fade: Fade::Tail(0.5),
                roll_speed: rng.float(0.0, 6.0),
                ..Particle::new(at, 3.0, mat, size as f32 * UNIT)
            });
        }
        out.push(g);
    }
    out
}
