//! Practice targets on DoD:S's maps (docs/plans/active/dods.md, M0 fix
//! (c)): the sandbox on a stock map, the player at one of its spawns,
//! seven `target add` asked for at once (the console's default
//! distance, and at 3 to 15 m): every one asked for stands, alive, apart
//! from the others. Passes with a note without the install.
//! `cargo test -p ultimate_gun_game --features dev --test dods heavy::sandbox_targets -- --nocapture`

use bevy::prelude::*;
use chimera_core::{console::Console, core::LocalPlayer};
use chimera_sim::minigame::{self, targets::Target};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{game::movement, harness::CaSim};
use party_roster::PartyPartsPlugin;
use ultimate_gun_game::{UltimateGunGamePlugin, sandbox::SANDBOX};

const MAPS: [&str; 3] = ["dods:dod_avalanche", "dods:dod_anzio", "dods:dod_flash"];

fn run(sim: &mut Sim, line: &str) -> String {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.ticks(3);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect::<Vec<_>>().join("\n")
}

fn targets(sim: &mut Sim) -> Vec<(Entity, Vec3)> {
    let mut q = sim.app.world_mut().query_filtered::<(Entity, &Transform), With<Target>>();
    q.iter(sim.app.world()).map(|(e, t)| (e, t.translation)).collect()
}

/// The sandbox on `map` with the player at its first spawn; None without
/// the map.
fn sandbox_on(map: &str) -> Option<Sim> {
    let data = match party_roster::load_map(map, 0) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("skipping {map}: {e}");
            return None;
        }
    };
    let spawn = data.spawns.first().map(|s| s.0)?;
    let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), PartyPartsPlugin, UltimateGunGamePlugin::default()));
    sim.ticks(2);
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(spawn + Vec3::Y * 0.1, movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(2.0);
    Some(sim)
}

#[test]
fn seven_targets_asked_for_all_stand() {
    for map in MAPS {
        for distances in [None, Some([3, 5, 7, 9, 11, 13, 15])] {
            let Some(mut sim) = sandbox_on(map) else { continue };
            let mut said = Vec::new();
            for i in 0..7 {
                let line = match distances {
                    None => "target add dods:us_rifleman".to_string(),
                    Some(d) => format!("target add dods:us_rifleman {}", d[i]),
                };
                said.push(run(&mut sim, &line));
            }
            sim.seconds(3.0);
            let got = targets(&mut sim);
            eprintln!("{map} {distances:?}: {} targets\n{}", got.len(), said.join("\n"));
            assert_eq!(got.len(), 7, "{map} {distances:?}: {said:?}");
            for (i, a) in got.iter().enumerate() {
                for b in &got[i + 1..] {
                    assert!(a.1.distance(b.1) > 0.5, "{map}: two targets in one place {a:?} {b:?}");
                }
            }
        }
    }
}
