//! Halo's movement on a converted Halo map (`game::maps` and
//! `game::movement` on `harness::HaloSim`): on Hang 'Em High a player
//! dropped at a spawn comes to rest on the floor with its eye 0.62 WU
//! above it (does not fall through the world, does not stick in the
//! ceiling); one spawned at a spawn point runs forward for three seconds
//! and jumps: it moves, lands on a floor and never leaves the level's
//! bounds (docs/OBSERVABILITY.md, section 1). Passes with a note without
//! an install.

use bevy::math::{Vec3, Vec3Swizzles};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::{
    game::{
        METERS_PER_UNIT,
        movement::{self, HaloMovement, MovementData},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

/// The pill's centre (the character's `Transform`) above its feet, metres.
fn centre_above_feet() -> f32 {
    MovementData::builtin().centre_above_origin(0.0) * METERS_PER_UNIT
}

#[test]
fn a_player_comes_to_rest_on_hang_em_highs_floor() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "hangemhigh", false);
    let spawns: Vec<Vec3> = map.spawns.iter().map(|(p, _)| *p).take(4).collect();
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.ticks(3);
    for feet in spawns {
        // Dropped from 0.3 m above standing height.
        let p = sim.spawn_character(feet + Vec3::Y * (centre_above_feet() + 0.3), movement::ID);
        sim.seconds(1.5);
        let at = sim.position(p);
        let v = sim.velocity(p);
        let standing = at.y - centre_above_feet();
        let state = sim.state(p);
        let eye = (at.y + state.eye_offset.y - standing) / METERS_PER_UNIT;
        eprintln!("spawn {feet}: at rest at {at}, feet {standing:.3}, eye {eye:.4} WU, speed {:.3}", v.length());
        assert!(state.on_ground, "spawn {feet}: on the ground");
        assert!(
            (standing - feet.y).abs() < 0.2,
            "spawn {feet}: feet at {standing:.3} m, not on the floor ({:.3})",
            feet.y
        );
        assert!((eye - 0.62).abs() < 1e-4, "spawn {feet}: eye {eye} WU above the feet");
        assert!(v.length() < 0.01, "spawn {feet}: still moving at {v}");
        assert!((at.xz() - feet.xz()).length() < 0.3, "spawn {feet}: slid to {at}");
    }
}

#[test]
fn a_player_runs_and_jumps_on_hang_em_high() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "hangemhigh", false);
    let (feet, _) = map.spawns[0];
    let yaw = map.spawn_yaws.first().copied().unwrap_or(0.0);
    // The level's bounds: its collision's box.
    let (lo, hi) = map
        .collision_positions
        .iter()
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        });
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let p = sim.spawn_character(feet + Vec3::Y * centre_above_feet(), movement::ID);
    sim.ticks(15);
    let start = sim.position(p);
    sim.intent(p).yaw = yaw;
    sim.intent(p).move_axis = bevy::math::Vec2::Y;
    let mut floor_min = f32::MAX;
    let mut jumped = false;
    let mut landed = false;
    // Three seconds of running, then jump held until it jumps (it may be
    // falling off a ledge then), then until it lands.
    for n in 0..400 {
        sim.intent(p).jump = n >= 90 && !jumped;
        sim.ticks(1);
        let at = sim.position(p);
        floor_min = floor_min.min(at.y - centre_above_feet());
        jumped |= sim.app.world().get::<HaloMovement>(p).unwrap().jumped;
        assert!(
            at.cmpge(lo).all() && at.cmple(hi).all(),
            "tick {n}: at {at}, outside the level's bounds {lo} .. {hi}"
        );
        if jumped && n > 100 && sim.state(p).on_ground {
            landed = true;
            break;
        }
    }
    let end = sim.position(p);
    let moved = (end - start).xz().length();
    eprintln!("ran {moved:.2} m from {start} to {end}; feet down to {floor_min:.3} m; jumped {jumped}, landed {landed}");
    assert!(moved > 5.0, "runs: moved {moved} m");
    assert!(jumped, "jumped");
    assert!(landed, "lands again, on the floor");
    assert!(floor_min > lo.y, "never falls through the floor: feet down to {floor_min}, the level's bottom {}", lo.y);
}
