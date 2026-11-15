//! A player on a converted TF2 map (`game::maps` on `harness::Tf2Sim`):
//! ctf_2fort loads into the headless simulation, and players dropped at
//! RED's and BLU's spawns come to rest on the floor under the marker
//! (not through the world, not stuck), then walk forward on `tf2:movement` (the stand-in
//! with the Soldier's 240 units/s). Passes with a note without an
//! install.

use bevy::math::{Vec2, Vec3, Vec3Swizzles};
use chimera_core::{character::CAPSULE_HEIGHT, core::Team};
use chimera_world::{
    harness::Sim,
    map::{MapLoadOptions, MapSimPlugin},
};
use tf2::{game::movement, harness::Tf2Sim};

use super::maps::{convert, install_mount};

#[test]
fn players_stand_and_walk_on_ctf_2fort() {
    let Some(mount) = install_mount() else { return };
    let map = convert(&mount, "ctf_2fort", &MapLoadOptions::lean());
    // Two of each team's spawns.
    let spawns: Vec<Vec3> = [Team(1), Team(2)]
        .iter()
        .flat_map(|t| map.spawns.iter().filter(move |(_, team)| *team == Some(*t)).map(|(p, _)| *p).take(2))
        .collect();
    assert_eq!(spawns.len(), 4);
    let mut sim = Sim::new(Tf2Sim, MapSimPlugin::new(map));
    sim.ticks(3);
    for feet in spawns {
        // Dropped from 0.3 m above standing height.
        let p = sim.spawn_character(feet + Vec3::Y * (CAPSULE_HEIGHT / 2.0 + 0.3), movement::ID);
        sim.seconds(1.5);
        let at = sim.position(p);
        let v = sim.velocity(p);
        let standing = at.y - CAPSULE_HEIGHT / 2.0;
        eprintln!("spawn {feet}: at rest at {at}, feet {standing:.3}, speed {:.3}", v.length());
        // TF2's spawn markers stand a little above the floor (2fort's
        // about 20 units): the feet come to rest at most a metre under it.
        assert!(
            standing <= feet.y + 0.05 && standing > feet.y - 1.0,
            "spawn {feet}: feet at {standing:.3} m, not on the floor"
        );
        assert!(v.length() < 0.2, "spawn {feet}: still moving at {v}");
        // It walks: half a second forward, then stops.
        sim.intent(p).move_axis = Vec2::Y;
        sim.seconds(0.5);
        sim.intent(p).move_axis = Vec2::ZERO;
        sim.seconds(0.5);
        let moved = (sim.position(p).xz() - at.xz()).length();
        let fell = standing - (sim.position(p).y - CAPSULE_HEIGHT / 2.0);
        eprintln!("  walked {moved:.2} m, fell {fell:.2} m");
        assert!(moved > 0.5, "spawn {feet}: walked only {moved} m");
        assert!(fell < 1.0, "spawn {feet}: fell {fell} m walking");
        sim.app.world_mut().despawn(p);
    }
}
