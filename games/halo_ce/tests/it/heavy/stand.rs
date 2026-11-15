//! Every multiplayer map stands a player (`game::maps`, `game::movement`
//! and `game::vitality` on `harness::HaloSim` with the map's own entities,
//! its items among them): on each of the 13 maps a player put at its first
//! spawns, standing, is after three seconds alive, on the ground, near
//! where it was put and inside the level's bounds (its collision's box).
//! Blood Gulch first, by name, as the app's default map that once showed
//! only its fog colour in a short `--frames` run (the load's pipelines,
//! not the simulation: docs/OBSERVABILITY.md, section 2). Passes with a
//! note without an install.

use bevy::math::{Vec3, Vec3Swizzles};
use chimera_core::core::{Dead, Health};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::{
    game::{
        METERS_PER_UNIT, MULTIPLAYER_MAPS,
        movement::{self, MovementData},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

/// Spawns tried on each map, one player at a time.
const SPAWNS_PER_MAP: usize = 4;

/// The pill's centre (the character's `Transform`) above its feet, metres.
fn centre_above_feet() -> f32 {
    MovementData::builtin().centre_above_origin(0.0) * METERS_PER_UNIT
}

/// What is wrong with a player put at each of `name`'s first spawns after
/// three seconds (empty: nothing).
fn faults(mount: &chimera_core::mount::Mount, name: &str) -> Vec<String> {
    let map = load(mount, name, false);
    let (lo, hi) = map
        .collision_positions
        .iter()
        .fold((Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)), |(lo, hi), p| {
            (lo.min(Vec3::from(*p)), hi.max(Vec3::from(*p)))
        });
    let spawns: Vec<Vec3> = map.spawns.iter().map(|(p, _)| *p).take(SPAWNS_PER_MAP).collect();
    assert!(!spawns.is_empty(), "{name}: no spawns");
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let mut out = Vec::new();
    for feet in spawns {
        let p = sim.spawn_character(feet + Vec3::Y * centre_above_feet(), movement::ID);
        sim.seconds(3.0);
        let at = sim.position(p);
        let standing = at.y - centre_above_feet();
        let world = sim.app.world();
        let health = world.get::<Health>(p).map_or(0.0, |h| h.current);
        let alive = health > 0.0 && world.get::<Dead>(p).is_none();
        let on_ground = sim.state(p).on_ground;
        let inside = at.cmpge(lo).all() && at.cmple(hi).all();
        let near = (at.xz() - feet.xz()).length() < 1.0 && (standing - feet.y).abs() < 0.5;
        eprintln!("{name} spawn {feet}: at {at}, health {health}, alive {alive}, on the ground {on_ground}, inside {inside}");
        if !(alive && on_ground && inside && near) {
            out.push(format!(
                "{name} spawn {feet}: at {at} (feet {standing:.2}), health {health}, alive {alive}, \
                 on the ground {on_ground}, inside {lo} .. {hi}: {inside}"
            ));
        }
        sim.app.world_mut().despawn(p);
        sim.ticks(1);
    }
    out
}

#[test]
fn a_player_stands_on_blood_gulch() {
    let Some(mount) = mount() else { return };
    let faults = faults(&mount, "bloodgulch");
    assert!(faults.is_empty(), "{faults:#?}");
}

#[test]
fn a_player_stands_on_every_multiplayer_map() {
    let Some(mount) = mount() else { return };
    let faults: Vec<String> = MULTIPLAYER_MAPS.iter().flat_map(|name| faults(&mount, name)).collect();
    assert!(faults.is_empty(), "{faults:#?}");
}
