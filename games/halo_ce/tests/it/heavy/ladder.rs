//! A real ladder climbed from the floor to the top (movement.md section 8,
//! "Climbable surfaces"): on Prisoner (4 climbable sheets, collision.md)
//! a player put down in front of each sheet's face, settled on the floor,
//! aiming 0.5 radian up and holding forward, mounts the sheet with no
//! jump (the sheet beats the floor in the ground test) and climbs until
//! it leaves over the top edge (its feet within 0.2 WU of the top, the
//! bottom sphere's centre past the edge) and stays up there. Each sheet
//! is climbed from the floor to the top from one face (the other stands
//! over a drop: no floor at its foot); every face climbed climbs at the
//! spec's 0.04875 WU per tick. Passes
//! with a note without an install.

use bevy::math::{Vec2, Vec3};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::{
    game::{
        METERS_PER_UNIT,
        movement::{self, HaloMovement, MovementData},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

#[test]
fn a_player_climbs_a_ladder_on_prisoner_from_the_floor_to_the_top() {
    let Some(mount) = mount() else { return };
    let map = load(&mount, "prisoner", false);
    let sheets: Vec<(Vec3, Vec3, Vec3)> = map
        .collision_brushes
        .iter()
        .filter(|b| b.ladder)
        .map(|b| (b.planes[0].0, b.min, b.max))
        .collect();
    assert_eq!(sheets.len(), 4, "Prisoner's climbable sheets");
    let d = MovementData::builtin();
    let s = METERS_PER_UNIT;
    let above = d.centre_above_origin(0.0) * s;
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let mut climbed = 0;
    for (i, &(n, lo, hi)) in sheets.iter().enumerate() {
        for sense in [1.0, -1.0] {
            let out = n * sense;
            let mid = (lo + hi) / 2.0;
            let feet = Vec3::new(mid.x, lo.y, mid.z) + out * 0.5 * s;
            let p = sim.spawn_character(feet + Vec3::Y * (above + 0.05 * s), movement::ID);
            sim.ticks(30);
            let me = sim.app.world().get::<HaloMovement>(p).unwrap().clone();
            let start = sim.position(p).y - above;
            if me.airborne || (start - lo.y).abs() > 0.3 * s {
                eprintln!("sheet {i} face {sense}: no floor at its foot ({start:.2} m, sheet from {:.2})", lo.y);
                sim.app.world_mut().despawn(p);
                continue;
            }
            let f = -out;
            sim.intent(p).yaw = (-f.x).atan2(-f.z);
            sim.intent(p).pitch = 0.5;
            sim.intent(p).move_axis = Vec2::Y;
            let mut mounted_at = None;
            let mut top = start;
            let mut speeds = Vec::new();
            for tick in 0..((hi.y - lo.y) / s / 0.04875) as u32 + 60 {
                sim.ticks(1);
                let me = sim.app.world().get::<HaloMovement>(p).unwrap().clone();
                let y = sim.position(p).y - above;
                top = top.max(y);
                if !me.airborne && me.ground_normal.y.abs() < 0.05 {
                    mounted_at.get_or_insert(tick);
                    speeds.push(sim.velocity(p).y / (s * 30.0));
                }
            }
            // Over the top: the bottom sphere's centre (0.2 WU above the
            // feet) left the top edge, and the player did not fall back.
            let end = sim.position(p).y - above;
            let reached = top >= hi.y - 0.2 * s && end >= hi.y - 0.2 * s;
            eprintln!(
                "sheet {i} face {sense}: from {start:.2} m mounted at tick {mounted_at:?}, top {top:.2} m, end {end:.2} m (sheet to {:.2}), reached {reached}",
                hi.y
            );
            if let Some(m) = mounted_at
                && reached
            {
                assert!(m < 30, "mounted from the floor without a jump, tick {m}");
                let full = speeds.iter().filter(|v| (**v - 0.04875).abs() < 1e-4).count();
                assert!(full > 10, "climbs at 0.04875 WU per tick: {speeds:?}");
                climbed += 1;
            }
            sim.app.world_mut().despawn(p);
        }
    }
    assert_eq!(climbed, 4, "each of Prisoner's ladders climbed from the floor to the top");
}
