//! A player on a converted Halo map (`game::maps` on `harness::HaloSim`):
//! Hang 'Em High loads into the headless simulation, and a player
//! dropped at a spawn comes to rest on the floor (does not fall through
//! the world, does not stick in the ceiling). Halo has no movement of
//! its own yet: the engine's placeholder walks (docs/OBSERVABILITY.md,
//! section 1). Passes with a note without an install.

use bevy::math::{Vec3, Vec3Swizzles};
use chimera_core::character::CAPSULE_HEIGHT;
use chimera_sim::movement::placeholder;
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::harness::HaloSim;

use super::maps::{load, mount};

#[test]
fn a_player_comes_to_rest_on_hang_em_highs_floor() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "hangemhigh", false);
    let spawns: Vec<Vec3> = map.spawns.iter().map(|(p, _)| *p).take(4).collect();
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.ticks(3);
    for feet in spawns {
        // Dropped from 0.3 m above standing height.
        let p = sim.spawn_character(feet + Vec3::Y * (CAPSULE_HEIGHT / 2.0 + 0.3), placeholder::ID);
        sim.seconds(1.5);
        let at = sim.position(p);
        let v = sim.velocity(p);
        let standing = at.y - CAPSULE_HEIGHT / 2.0;
        eprintln!("spawn {feet}: at rest at {at}, feet {standing:.3}, speed {:.3}", v.length());
        assert!(
            (standing - feet.y).abs() < 0.2,
            "spawn {feet}: feet at {standing:.3} m, not on the floor ({:.3})",
            feet.y
        );
        assert!(v.length() < 0.2, "spawn {feet}: still moving at {v}");
        assert!((at.xz() - feet.xz()).length() < 0.3, "spawn {feet}: slid to {at}");
    }
}
