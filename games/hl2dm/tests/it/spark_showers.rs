//! Explosion sparks under water and the spark shower
//! (specs/hl2dm/effects_sounds.md, "Explosion sparks in detail",
//! "`spark_shower`", tests 63-66).

use bevy::prelude::*;
use chimera_core::core::{BlastFlags, DamageKind, Explosion, SimClock};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{ParticleRng, SparkBurst},
};
use hl2dm::{
    game::spark_shower::{SparkShower, new_shower, touch},
    harness::Hl2Sim,
};

const U: f32 = 0.0254;

fn sim() -> Sim {
    let mut sim = Sim::new(Hl2Sim, GreyboxMapPlugin);
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    sim.seconds(0.5);
    sim
}

fn blast(origin: Vec3, flags: u16) -> Explosion {
    Explosion {
        origin,
        damage: 0.0,
        radius: 100.0 * U,
        attacker: None,
        inflictor: None,
        sound: None,
        weapon: None,
        round_scorch: false,
        bell_sigma: None,
        armor_ratio: None,
        through_walls: false,
        flags: BlastFlags(flags),
        kind: DamageKind::Blast,
    }
}

fn showers(sim: &mut Sim) -> Vec<(SparkShower, Vec3)> {
    let mut q = sim.app.world_mut().query::<(&SparkShower, &Transform)>();
    q.iter(sim.app.world()).map(|(s, t)| (*s, t.translation)).collect()
}

/// Under the greybox tank's surface, `depth` units down.
fn under(depth: f32) -> Vec3 {
    let c = (greybox::TANK_MIN + greybox::TANK_MAX) / 2.0;
    Vec3::new(c.x, greybox::TANK_WATER_DEPTH - depth * U, c.y)
}

/// Tests 63, 64: a map env_explosion without "no sparks" under water makes
/// 0-3 showers at the water's surface + 24 up, each going 400-500 up; with
/// flag 32, or out of the water, none.
#[test]
fn explosions_under_water_make_spark_showers_63_64() {
    let mut sim = sim();
    // 5 under: the line from 8 above meets the surface.
    let mut seen = std::collections::HashSet::new();
    let top = greybox::TANK_WATER_DEPTH + 24.0 * U;
    for _ in 0..12 {
        sim.app.world_mut().write_message(blast(under(5.0), 0));
        sim.ticks(1);
        let mut q = sim.app.world_mut().query::<(Entity, &SparkShower, &Transform)>();
        for (e, s, t) in q.iter(sim.app.world()) {
            if seen.insert(e) {
                // One tick of flight from the drawn point.
                let v = s.velocity / U;
                assert!((360.0..=500.0).contains(&v.y), "{v}");
                assert!((t.translation.y - top) / U > 0.0 && (t.translation.y - top) / U < 10.0, "{}", t.translation);
            }
        }
    }
    let all = showers(&mut sim);
    assert!(!all.is_empty(), "twelve explosions, some showers");
    assert!(all.len() <= 36);
    // Each is removed at the end of its life: all gone after 1.6 s.
    sim.seconds(1.7);
    assert!(showers(&mut sim).is_empty());

    // 64: "no sparks", or out of the water: none.
    for (at, flags) in [(under(5.0), BlastFlags::NO_SPARKS), (under(-100.0), 0)] {
        for _ in 0..12 {
            sim.app.world_mut().write_message(blast(at, flags));
            sim.ticks(1);
        }
        assert!(showers(&mut sim).is_empty(), "{at} {flags}");
    }
}

/// A new shower aimed straight up: 400-500 up, ±100 sideways (units/s);
/// its life 0.5-1.5 s.
#[test]
fn a_shower_starts_up_and_sideways() {
    let mut rng = ParticleRng::new(7);
    for _ in 0..200 {
        let s = new_shower(&mut rng, Vec3::Y, 0.0);
        let v = s.velocity / U;
        assert!((400.0..=500.0).contains(&v.y), "{v}");
        assert!(v.x.abs() <= 100.0 && v.z.abs() <= 100.0, "{v}");
        assert!((0.5..=1.5).contains(&s.life));
    }
    // Aimed down (a ceiling): 400-500 down.
    let v = new_shower(&mut rng, Vec3::NEG_Y, 0.0).velocity / U;
    assert!((-500.0..=-400.0).contains(&v.y), "{v}");
}

/// Test 65: L = 0.53 and nothing touched: bursts at 0.1, 0.2 ... 0.6 s,
/// removed at 0.6.
#[test]
fn a_shower_bursts_every_tenth_until_its_life_is_out_65() {
    let mut sim = sim();
    let now = sim.app.world().resource::<SimClock>().now;
    let e = sim
        .app
        .world_mut()
        .spawn((
            SparkShower { velocity: Vec3::ZERO, life: 0.53, next_think: now + 0.1, on_ground: false, in_water: None },
            Transform::from_xyz(0.0, 2000.0 * U, 0.0),
        ))
        .id();
    let mut cursor = sim.app.world().resource::<Messages<SparkBurst>>().get_cursor_current();
    let mut at = Vec::new();
    for _ in 0..60 {
        sim.ticks(1);
        let t = sim.app.world().resource::<SimClock>().now - now;
        let bursts = sim.app.world().resource::<Messages<SparkBurst>>();
        at.extend(cursor.read(bursts).map(|_| t));
    }
    assert_eq!(at.len(), 6, "{at:?}");
    for (i, t) in at.iter().enumerate() {
        assert!((t - 0.1 * (i + 1) as f64).abs() < 0.02, "{at:?}");
    }
    assert!(sim.app.world().get_entity(e).is_err(), "removed after the last");
}

/// Test 66: falling at (100, 0, -300) onto a default-surface floor: × 0.6,
/// reflected, × 0.25: (15, 0, 45); 15² ≥ 10, so it keeps its life.
#[test]
fn a_touch_slows_and_bounces_66() {
    // Source (100, 0, -300) in engine space.
    let mut s = SparkShower { velocity: Vec3::new(100.0, -300.0, 0.0) * U, life: 1.0, next_think: 0.0, on_ground: false, in_water: None };
    touch(&mut s, Vec3::Y);
    let v = s.velocity / U;
    assert!((v - Vec3::new(15.0, 45.0, 0.0)).length() < 1e-3, "{v}");
    assert_eq!(s.life, 1.0);
    assert!(!s.on_ground);
    // Slow sideways: its life ends.
    let mut s = SparkShower { velocity: Vec3::new(3.0, -300.0, 0.0) * U, ..s };
    touch(&mut s, Vec3::Y);
    assert_eq!(s.life, 0.0);
}
