//! CS 1.6's footsteps on the user's install (specs/cs16/footsteps.md):
//! every step set's, the splash's, the bubbles' and the gasps' waves
//! resolve and decode through the chain; de_dust2 converts with its
//! surfaces named by CS 1.6's `materials.txt` rule (more kinds than
//! concrete among them) and the step surfaces in its sounds, none of
//! their files missing. Passes with a note without an install.

use goldsrc_engine::content::sounds;

use super::maps::{convert, mount};

#[test]
fn every_step_water_and_gasp_file_resolves() {
    let Some(mount) = mount() else { return };
    let mut table = Default::default();
    let mut warnings = Vec::new();
    sounds::add_steps(&mount, &mut table, &mut warnings);
    assert!(warnings.is_empty(), "{warnings:?}");
    for (name, surface) in sounds::step_surfaces() {
        assert!(table.surfaces.contains_key(&name), "{name}");
        for e in [surface.step_left, surface.step_right].into_iter().flatten() {
            let entry = table.entry(&e).unwrap_or_else(|| panic!("{e}: no entry"));
            assert!(!entry.waves.is_empty(), "{e}");
        }
    }
    for e in [sounds::SPLASH, sounds::BUBBLE, sounds::GASP_OUT, sounds::GASP_SHORT] {
        assert!(table.entry(e).is_some_and(|e| !e.waves.is_empty()), "{e}");
    }
    // The tile set plays pl_tile5 one step in five: 5 listings, 2 of one file.
    let (left, _) = sounds::step_entries("tile", "pl_tile");
    assert_eq!(table.entry(&left).unwrap().waves.len(), 5);
}

#[test]
fn de_dust2_names_its_surfaces_by_the_cs16_rule() {
    let Some(mount) = mount() else { return };
    let map = convert(&mount, "de_dust2", false);
    let mut kinds: Vec<&str> = map.meshes.iter().filter_map(|m| m.surface.as_deref()).collect();
    kinds.sort();
    kinds.dedup();
    eprintln!("de_dust2 surface kinds: {kinds:?}");
    assert!(kinds.len() >= 3, "{kinds:?}");
    assert!(kinds.iter().any(|k| *k != "concrete"));
    assert!(map.sounds.surfaces.contains_key("concrete") && map.sounds.surfaces.contains_key("ladder"));
    assert!(!map.warnings.iter().any(|w| w.starts_with("step sounds") || w.starts_with("water sound")), "{:?}", map.warnings);
}
