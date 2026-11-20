//! specs/hl2dm/effects_sounds.md 15.3, 15.5 and test case P6: the sound
//! events (5004) of HL2:DM's view models, as the install carries them: the
//! .357's four loader sounds (cycles 0.255, 0.355, 0.609, 0.836) and the
//! AR2's two (0.021, 0.404) on their reload sequences, and no other
//! weapon's; the guns' data names the ones the owner hears (`[sounds]
//! view`, loaded with a map). Skips without an install.

use std::collections::BTreeSet;

use hl2dm::game::{flashes, maps, weapons};
use source_engine::content::material::MaterialLoader;

use super::map_stock::install_mount;

/// (weapon, sequence, [(seconds into it, entry)]). The S.L.A.M.'s mode
/// switch carries `Weapon_SLAM.TripMineMode` (cycle 1.0 of
/// `throw_to_tripmine_ND`): our rules never play that sequence.
const EVENTS: &[(&str, &str, &[(f32, &str)])] = &[
    ("357", "reload", &[(0.9333, "Weapon_357.OpenLoader"), (1.3, "Weapon_357.RemoveLoader"), (2.2333, "Weapon_357.ReplaceLoader"), (3.0667, "Weapon_357.Spin")]),
    ("ar2", "IR_reload", &[(0.0333, "Weapon_AR2.Reload_Rotate"), (0.6333, "Weapon_AR2.Reload_Push")]),
    ("slam", "throw_to_tripmine_ND", &[(1.0, "Weapon_SLAM.TripMineMode")]),
];

#[test]
fn the_view_models_sound_events_are_the_specs() {
    let Some(mount) = install_mount() else { return };
    let mut materials = MaterialLoader::guest(&mount, 0, 0);
    let (mut models, mut warnings) = (Vec::new(), Vec::new());
    maps::add_view_models(&mut materials, chimera_world::map::KitLoad::All, &mut models, &mut warnings);
    let mut wrong = Vec::new();
    for v in &models {
        let set = v.animations.as_ref().unwrap();
        let short = v.key.trim_start_matches("hl2dm:weapon_");
        let mut named = BTreeSet::new();
        for (i, q) in set.sequences.iter().enumerate() {
            let len = set.duration(i);
            let got: Vec<(f32, String)> = q
                .events
                .iter()
                .filter_map(|e| match flashes::event_effect(e) {
                    Some(chimera_world::map::ViewModelEventKind::Sound { entry }) => Some((e.cycle * len, entry)),
                    _ => None,
                })
                .collect();
            let want = EVENTS.iter().find(|(g, s, _)| *g == short && *s == q.name).map_or(&[][..], |r| r.2);
            if got.len() != want.len() || got.iter().zip(want).any(|((t, e), (wt, we))| (t - wt).abs() > 0.006 || e != we) {
                wrong.push(format!("{short} {}: {got:?}, expected {want:?}", q.name));
            }
            named.extend(got.into_iter().map(|(_, e)| e));
        }
        // What the guns' data loads for them (the S.L.A.M.'s is never played).
        if short != "slam" {
            let listed: BTreeSet<String> = weapons::hl2_weapons()
                .weapons
                .iter()
                .find(|w| w.id == v.key)
                .and_then(|w| w.sounds.as_ref())
                .map(|s| s.view.iter().cloned().collect())
                .unwrap_or_default();
            if listed != named {
                wrong.push(format!("{short}: [sounds] view {listed:?}, the events {named:?}"));
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
