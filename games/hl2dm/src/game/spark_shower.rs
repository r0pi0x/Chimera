//! Explosion sparks under water and the `spark_shower` they make
//! (specs/hl2dm/effects_sounds.md, "Explosion sparks in detail" and
//! "`spark_shower`"; the server's).
//!
//! - **The sparks** (`explosion_sparks`): a map explosion (`core::Explosion`)
//!   without "no sparks" (`BlastFlags::NO_SPARKS`, spawnflag 32; every
//!   explosion made by code carries it) whose own origin is in water makes
//!   I(0, 3) showers at its drawn point: the line from 8 above the origin to
//!   40 below meets the world or a water surface; the point is that hit +
//!   24 along its normal (the origin when it meets nothing), the showers
//!   aimed along that normal. A line that starts in water has no usable
//!   normal: zero (the showers go straight down, Open questions 12).
//! - **A shower** (`SparkShower`): v = U(200, 300)·F plus U(-100, 100) on
//!   each horizontal axis, then 200 more up when v_z ≥ 0 else 200 more
//!   down; life L = U(0.5, 1.5); tossed under 400 units/s² with bounce
//!   against the world and solid things (players too). Every 0.1 s from its
//!   creation an electric-spark burst where it is (`map::SparkBurst`,
//!   impact_effects.md 8; networked as map effects are), L −= 0.1, gone
//!   when L ≤ 0: ⌈L / 0.1⌉ bursts. A touch: v × 0.1 if it rests on the
//!   ground, else × 0.6, and L := 0 when then v_x² + v_y² < 10; then the
//!   bounce: v reflected off the plane and scaled by min(0.9, 0.25) (the
//!   default surface's elasticity); on a floor (normal z > 0.7) below 30
//!   units/s it stops on the ground. Water: its first move counts as
//!   entering (`BaseEntity.EnterWater`, v_z halved when it is in water);
//!   leaving plays `BaseEntity.ExitWater`, going in again the enter sound
//!   and halves v_z again.
//!
//! Known shortcuts: every surface bounces as `default` (0.25; the spec
//! names only that one); the spark bursts' magnitude and trail length (1)
//! are the burst's own.

use avian3d::prelude::{SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use crate::{
    core::{BlastFlags, Explosion, MapWater, SOLID_LAYERS, SimClock},
    map::{GameSound, ParticleRng, PlaySound, SparkBurst},
};

/// Metres per Source unit.
const U: f32 = 0.0254;
/// The shower's gravity (units/s²: 400 / `sv_gravity` of the player's).
pub const GRAVITY: f32 = 400.0;
/// Seconds between bursts, and the life each one takes.
pub const THINK: f64 = 0.1;
/// The default surface's elasticity, under the 0.9 cap.
pub const ELASTICITY: f32 = 0.25;

/// A spark shower in flight (engine space, m/s).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct SparkShower {
    pub velocity: Vec3,
    /// Seconds of bursts left.
    pub life: f32,
    pub next_think: f64,
    pub on_ground: bool,
    /// Its contents at the last move (None before the first).
    pub in_water: Option<bool>,
}

/// The showers' random numbers (the server's own).
#[derive(Resource)]
struct ShowerRng(ParticleRng);

impl Default for ShowerRng {
    fn default() -> Self {
        Self(ParticleRng::new(0x5BA2_C5))
    }
}

/// The server's explosion sparks and spark showers.
pub struct SparkShowerPlugin;

impl Plugin for SparkShowerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ShowerRng>()
            .add_message::<Explosion>()
            .add_message::<SparkBurst>()
            .add_message::<GameSound>()
            .add_systems(
                FixedUpdate,
                (explosion_sparks, fly).chain().after(crate::core::SimSet::Weapons).run_if(crate::core::authoritative),
            );
    }
}

fn in_water(water: Option<&MapWater>, p: Vec3) -> bool {
    water.is_some_and(|w| w.0.iter().any(|v| v.brush.planes.iter().all(|(n, d)| n.dot(p) - d <= 0.0)))
}

/// Where the line from `from` along `dir` (unit) first enters a water
/// volume within `len`: its distance and the entered face's normal.
fn water_entry(water: Option<&MapWater>, from: Vec3, dir: Vec3, len: f32) -> Option<(f32, Vec3)> {
    let mut best: Option<(f32, Vec3)> = None;
    for v in water.map_or(&[][..], |w| &w.0[..]) {
        let (mut enter, mut exit, mut normal) = (0.0f32, len, Vec3::ZERO);
        let mut miss = false;
        for (n, d) in &v.brush.planes {
            let dist = n.dot(from) - d;
            let along = n.dot(dir);
            if along.abs() < 1e-9 {
                if dist > 0.0 {
                    miss = true;
                    break;
                }
                continue;
            }
            let t = -dist / along;
            if along < 0.0 {
                if t > enter {
                    enter = t;
                    normal = *n;
                }
            } else if t < exit {
                exit = t;
            }
        }
        if miss || enter > exit || normal == Vec3::ZERO {
            continue;
        }
        if best.is_none_or(|(b, _)| enter < b) {
            best = Some((enter, normal));
        }
    }
    best
}

/// The drawn point and normal of an explosion at `origin` (engine space):
/// the line from 8 above to 40 below against the world and water.
pub fn drawn_point(spatial: &SpatialQuery, water: Option<&MapWater>, origin: Vec3) -> (Vec3, Vec3) {
    let from = origin + Vec3::Y * 8.0 * U;
    let len = 48.0 * U;
    if in_water(water, from) {
        // No usable surface (Open questions 12).
        return (from, Vec3::ZERO);
    }
    let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
    let solid = spatial.cast_ray(from, Dir3::NEG_Y, len, true, &filter).map(|h| (h.distance, h.normal));
    let wet = water_entry(water, from, Vec3::NEG_Y, len);
    let hit = match (solid, wet) {
        (Some(a), Some(b)) => Some(if b.0 < a.0 { b } else { a }),
        (a, b) => a.or(b),
    };
    match hit {
        Some((t, n)) => (from + Vec3::NEG_Y * t + n * 24.0 * U, n),
        None => (origin, Vec3::ZERO),
    }
}

/// A new shower's state, aimed along `forward` (unit or zero).
pub fn new_shower(rng: &mut ParticleRng, forward: Vec3, now: f64) -> SparkShower {
    // In Source units, z up.
    let f = Vec3::new(forward.x, -forward.z, forward.y);
    let mut v = f * rng.float(200.0, 300.0);
    v.x += rng.float(-100.0, 100.0);
    v.y += rng.float(-100.0, 100.0);
    if v.z >= 0.0 {
        v.z += 200.0;
    } else {
        v.z -= 200.0;
    }
    SparkShower {
        velocity: Vec3::new(v.x, v.z, -v.y) * U,
        life: rng.float(0.5, 1.5),
        next_think: now + THINK,
        on_ground: false,
        in_water: None,
    }
}

/// Map explosions under water without "no sparks" make 0-3 showers.
fn explosion_sparks(
    mut blasts: MessageReader<Explosion>,
    spatial: SpatialQuery,
    water: Option<Res<MapWater>>,
    clock: Res<SimClock>,
    mut rng: ResMut<ShowerRng>,
    mut commands: Commands,
) {
    for e in blasts.read() {
        if e.flags.has(BlastFlags::NO_SPARKS) || !in_water(water.as_deref(), e.origin) {
            continue;
        }
        let (at, normal) = drawn_point(&spatial, water.as_deref(), e.origin);
        for _ in 0..rng.0.int(0, 3) {
            commands.spawn((Name::new("spark_shower"), new_shower(&mut rng.0, normal, clock.now), Transform::from_translation(at)));
        }
    }
}

/// The showers' moves, touches and bursts.
fn fly(
    mut showers: Query<(Entity, &mut SparkShower, &mut Transform)>,
    spatial: SpatialQuery,
    water: Option<Res<MapWater>>,
    clock: Res<SimClock>,
    mut bursts: MessageWriter<SparkBurst>,
    mut sounds: MessageWriter<GameSound>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
    for (e, mut s, mut t) in &mut showers {
        // The move: tossed with bounce.
        if !s.on_ground {
            s.velocity.y -= GRAVITY * U * dt;
            let step = s.velocity * dt;
            let len = step.length();
            if let Ok(dir) = Dir3::new(step) {
                match spatial.cast_ray(t.translation, dir, len, true, &filter) {
                    Some(hit) => {
                        t.translation += *dir * hit.distance + hit.normal * 0.01 * U;
                        touch(&mut s, hit.normal);
                    }
                    None => t.translation += step,
                }
            }
        }
        // Water: the first move counts as entering.
        let wet = in_water(water.as_deref(), t.translation);
        let was = s.in_water.unwrap_or(false);
        if wet != was {
            let entry = if wet { "BaseEntity.EnterWater" } else { "BaseEntity.ExitWater" };
            if wet {
                s.velocity.y *= 0.5;
            }
            sounds.write(GameSound(PlaySound {
                pitch: None,
                entry: entry.into(),
                at: Some(t.translation),
                volume: None,
                source: None,
                channel: None,
            }));
        }
        s.in_water = Some(wet);
        // The think: a burst, then its life on.
        if clock.now + 1e-6 >= s.next_think {
            bursts.write(SparkBurst { at: t.translation, dir: Vec3::ZERO, magnitude: 1.0 });
            s.life -= THINK as f32;
            if s.life > 1e-6 {
                s.next_think += THINK;
            } else {
                commands.entity(e).despawn();
            }
        }
    }
}

/// A touch during the move (`normal` the hit plane's, engine space).
pub fn touch(s: &mut SparkShower, normal: Vec3) {
    s.velocity *= if s.on_ground { 0.1 } else { 0.6 };
    let (vx, vy) = (s.velocity.x / U, s.velocity.z / U);
    if vx * vx + vy * vy < 10.0 {
        s.life = 0.0;
    }
    // The bounce: reflected off the plane, scaled by the elasticity.
    let v = s.velocity - 2.0 * s.velocity.dot(normal) * normal;
    s.velocity = v * ELASTICITY.min(0.9);
    if normal.y > 0.7 && s.velocity.length() / U < 30.0 {
        s.velocity = Vec3::ZERO;
        s.on_ground = true;
    }
}
