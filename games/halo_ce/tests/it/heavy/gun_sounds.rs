//! Every Halo gun heard (playtest 2026-11-18: the shotgun's fire and the
//! plasma pistol's charge were missing): with the disc's effects library
//! (`fx::load::compile` of the donor map), a Spartan fires each of the
//! nine guns (taps, then a hold through a charge) and every sound its
//! firing effects name (effects.md, sounds.md "The four first weapons")
//! is raised as the shooter's; the impacts' sounds are nobody's (a
//! shotgun blast's pellets' impacts, counted as the shooter's, took its
//! voices from the fire sound). Passes with a note without an install.

use std::{collections::BTreeSet, sync::Arc};

use bevy::prelude::*;
use blam_formats::cache::MapFile;
use chimera_core::core::Intent;
use chimera_sim::weapon::give;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::PlaySound,
};
use halo_ce::{
    game::{
        fx::{
            ActiveFx,
            load::{FxPartKind, compile, gun_tags},
        },
        movement,
        weapons::{GUNS, guns},
    },
    harness::HaloSim,
};

#[test]
fn every_halo_gun_raises_its_firing_sounds() {
    let Some(mount) = super::maps::mount() else { return };
    let bytes = mount.read("maps/bloodgulch.map").unwrap();
    let file = MapFile::load(&bytes).unwrap();
    let tags = gun_tags();
    let set = compile(&file, &tags.iter().map(String::as_str).collect::<Vec<_>>(), false);
    let lib = Arc::new(set.library);
    let numbers = guns();
    let mut missing = Vec::new();
    for id in GUNS {
        let tag = &numbers.iter().find(|(g, _)| g == id).unwrap().1.tag;
        let fx = lib.gun(tag).unwrap_or_else(|| panic!("{tag}: no effects"));
        let want: BTreeSet<String> = fx
            .firing
            .iter()
            .flatten()
            .flat_map(|&e| lib.effects[e].events.iter().flat_map(|ev| ev.parts.iter()))
            .filter_map(|p| match &p.kind {
                FxPartKind::Sound(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
        let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
        sim.app.insert_resource(ActiveFx(Some(lib.clone())));
        let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
        sim.ticks(2);
        give(sim.app.world_mut(), p, id).unwrap();
        sim.seconds(5.0);
        let mut cursor = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
        let mut heard = BTreeSet::new();
        let mut voices = 0usize;
        // Taps for two seconds, then a held trigger for two (the plasma
        // pistol's charge and its charged bolt on the release).
        let ticks = (2.0 * sim.tick_hz()) as u32;
        for t in 0..3 * ticks {
            sim.app.world_mut().get_mut::<Intent>(p).unwrap().fire = if t < ticks { t % 8 < 4 } else { t < 2 * ticks };
            sim.ticks(1);
            for s in cursor.read(sim.app.world().resource::<Messages<PlaySound>>()) {
                heard.insert(s.entry.clone());
                // The shooter's own voices: its gun's sounds alone (an
                // impact's is nobody's, a sound at a point).
                if s.source == Some(p) {
                    voices += 1;
                    assert!(
                        want.contains(&s.entry) || !s.entry.contains("impact"),
                        "{id}: {} is the shooter's",
                        s.entry
                    );
                } else if want.contains(&s.entry) {
                    missing.push(format!("{id}: {} not the shooter's", s.entry));
                }
            }
        }
        assert!(voices > 0 || want.is_empty(), "{id}: no sound of the shooter's");
        eprintln!("{id}: want {want:?}, heard {heard:?}");
        let lacking: Vec<&String> = want.difference(&heard).collect();
        if !lacking.is_empty() {
            missing.push(format!("{id}: {lacking:?}"));
        }
    }
    assert!(missing.is_empty(), "{missing:#?}");
}
