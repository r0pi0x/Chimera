//! Every game's characters on a Halo map's ladders (the one ladder rule,
//! docs/DESIGN.md "Ladders across games"; a CS:S character couldn't climb
//! Halo's ladders, 2026-11-18): on Prisoner (4 climbable sheets, Halo's
//! `climbable` surface flag, made ladder brushes by `halo_ce`'s
//! `maps::collision`) each movement the party registers, put down in
//! front of a sheet's face with floor at its foot and walking into it
//! looking 45 degrees up, climbs most of the sheet's height. Each face
//! Halo's own movement climbs, every other movement climbs too. Passes
//! with a note without the disc image.

use bevy::prelude::*;
use chimera_world::{
    harness::Sim,
    map::{MapLoadOptions, MapSimPlugin},
};
use combat_arms::harness::CaSim;
use party_roster::PartyPartsPlugin;

#[test]
fn every_movement_climbs_prisoners_ladders() {
    if halo_ce::game::INFO.mount().is_none() {
        eprintln!("skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)");
        return;
    }
    let map = party_roster::load_map_with("halo_ce:prisoner", &MapLoadOptions::lean()).expect("prisoner");
    let sheets: Vec<(Vec3, Vec3, Vec3)> = map.collision_brushes.iter().filter(|b| b.ladder).map(|b| (b.planes[0].0, b.min, b.max)).collect();
    assert_eq!(sheets.len(), 4, "Prisoner's climbable sheets");
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
    sim.ticks(3);
    let ids: Vec<&'static str> = {
        let reg = sim.app.world().resource::<chimera_core::slots::MovementRegistry>();
        reg.0.iter().map(|m| m.id).filter(|id| !id.starts_with("chimera:")).collect()
    };
    let s = halo_ce::game::METERS_PER_UNIT;
    let mut failed = Vec::new();
    let mut tried = 0;
    for (i, &(n, lo, hi)) in sheets.iter().enumerate() {
        for sense in [1.0, -1.0] {
            let out = (n * sense).with_y(0.0).normalize_or_zero();
            if out == Vec3::ZERO {
                continue;
            }
            let mid = (lo + hi) / 2.0;
            let foot = Vec3::new(mid.x, lo.y, mid.z) + out * 0.4 * s;
            // Halo's climb decides which faces have floor at their foot.
            let mut results = Vec::new();
            for id in &ids {
                let p = sim.spawn_character(foot + Vec3::Y * 1.0, id);
                sim.seconds(0.5);
                let start = sim.position(p).y;
                let grounded = sim.state(p).on_ground;
                let f = -out;
                let ticks = (6.0 * sim.tick_hz()) as u32;
                let mut top = start;
                for _ in 0..ticks {
                    let mut it = sim.intent(p);
                    it.yaw = (-f.x).atan2(-f.z);
                    it.pitch = 45f32.to_radians();
                    it.move_axis = Vec2::Y;
                    sim.ticks(1);
                    top = top.max(sim.position(p).y);
                }
                results.push((*id, grounded, top - start));
                sim.app.world_mut().despawn(p);
                sim.ticks(1);
            }
            let need = 0.6 * (hi.y - lo.y);
            let halo = results.iter().find(|r| r.0 == halo_ce::game::movement::ID).copied();
            eprintln!("sheet {i} face {sense}: height {:.2} m: {results:?}", hi.y - lo.y);
            let Some((_, true, halo_rose)) = halo else { continue };
            if halo_rose < need {
                continue;
            }
            tried += 1;
            for (id, grounded, rose) in &results {
                if *grounded && *rose < need {
                    failed.push(format!("{id}: sheet {i} face {sense}: rose {rose:.2} of {:.2} m", hi.y - lo.y));
                }
            }
        }
    }
    assert!(tried >= 2, "faces Halo's movement climbs: {tried}");
    assert!(failed.is_empty(), "every movement climbs Halo's ladders:\n{}", failed.join("\n"));
}
