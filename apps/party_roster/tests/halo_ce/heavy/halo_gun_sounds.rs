//! Every Halo gun heard in the party (playtest 2026-11-18: the shotgun's
//! fire and the plasma pistol's charge were missing in a party app): on
//! another game's map (de_dust2, Halo's guest dressing) and on a Halo
//! map, a CS:S player and a Spartan fire each of Halo's nine guns (taps,
//! then a held trigger), and each sound the gun's firing effects name is
//! raised and is in the map's sound table with clips. Passes with a note
//! without the installs.

use std::collections::BTreeSet;

use bevy::prelude::*;
use chimera_sim::weapon::give;
use chimera_world::{
    harness::Sim,
    map::{MapSimPlugin, PlaySound},
};
use combat_arms::harness::CaSim;
use halo_ce::game::{
    fx::{ActiveFx, load::FxPartKind},
    weapons::{GUNS, guns},
};
use party_roster::PartyPartsPlugin;

#[test]
fn every_halo_gun_sounds_in_the_party() {
    if halo_ce::game::INFO.mount().is_none() {
        eprintln!("skipping: no Halo CE disc image or store");
        return;
    }
    let numbers = guns();
    let mut missing = Vec::new();
    for id in ["cs_source:de_dust2", "halo_ce:bloodgulch"] {
        let Ok(map) = party_roster::load_map(id, 0) else {
            eprintln!("skipping {id}: not installed");
            continue;
        };
        let mut sim = Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin));
        sim.ticks(2);
        let Some(lib) = sim.app.world().resource::<ActiveFx>().0.clone() else {
            missing.push(format!("{id}: no Halo effects library"));
            continue;
        };
        let spawn = sim
            .app
            .world_mut()
            .query_filtered::<&Transform, With<chimera_core::core::SpawnPoint>>()
            .iter(sim.app.world())
            .next()
            .map(|t| t.translation)
            .unwrap();
        for movement in ["cs_source:movement", halo_ce::game::movement::ID] {
            for gun in GUNS {
                let tag = &numbers.iter().find(|(g, _)| g == gun).unwrap().1.tag;
                let want: BTreeSet<String> = lib
                    .gun(tag)
                    .map(|fx| {
                        fx.firing
                            .iter()
                            .flatten()
                            .flat_map(|&e| lib.effects[e].events.iter().flat_map(|ev| ev.parts.iter()))
                            .filter_map(|p| match &p.kind {
                                FxPartKind::Sound(s) => Some(s.clone()),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let p = sim.spawn_character(spawn + Vec3::Y * 1.0, movement);
                sim.ticks(2);
                give(sim.app.world_mut(), p, gun).unwrap();
                sim.seconds(3.0);
                let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
                let mut heard = BTreeSet::new();
                let ticks = (2.0 * sim.tick_hz()) as u32;
                for t in 0..3 * ticks {
                    sim.intent(p).fire = if t < ticks { t % 8 < 4 } else { t < 2 * ticks };
                    sim.ticks(1);
                    heard.extend(
                        cursor
                            .read(sim.app.world().resource::<Messages<PlaySound>>())
                            .map(|s| s.entry.clone()),
                    );
                }
                let table = sim.app.world().resource::<chimera_world::map::sound::SoundBank>();
                let lacking: Vec<String> = want
                    .iter()
                    .filter(|s| !heard.contains(*s) || table.0.entry(s).is_none_or(|e| e.waves.is_empty()))
                    .map(|s| {
                        format!(
                            "{s} (raised {}, in the table {})",
                            heard.contains(s),
                            table.0.entry(s).is_some()
                        )
                    })
                    .collect();
                eprintln!("{id} {movement} {gun}: want {want:?}, heard {heard:?}");
                if !lacking.is_empty() {
                    missing.push(format!("{id} {movement} {gun}: {lacking:?}"));
                }
                sim.app.world_mut().despawn(p);
                sim.ticks(1);
            }
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}
