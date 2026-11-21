//! The local player's Pistol reload on the install's own kit as another
//! game's map gets it (`maps::dress_guest_in`: the arms' view models and
//! the sound table a match plays from, as Ultimate Gun Game's): the
//! owner hears specs/tf2/viewmodel_sounds.md 3's sounds at their times,
//! each an entry of that table with waves, the others the world reload;
//! and with `ClipIn`'s waves gone (the Pistol's set published before the
//! entry joined it: a served Engineer heard no reload, 2026-11-21) the
//! Engineer hears the world reload in its place. Passes with a note
//! without the install.

use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_world::{
    greybox,
    map::{MapData, ViewModels, sound::SoundBank},
};
use tf2::game::{maps, weapons::PISTOL_SCOUT};

use super::maps::install_mount;
use crate::{
    viewmodel_sounds::{Heard, reload, ticks_of},
    weapons::{holding, sim},
};

const RELOAD_SCOUT: &str = "tf2:Weapon_Pistol.ReloadScout";
const CLIP_IN: &str = "tf2:Weapon_Pistol.ClipIn";
const WORLD_RELOAD: &str = "tf2:Weapon_Pistol.WorldReload";

/// An empty map dressed with TF2's guns and their sounds from the install.
fn dressed() -> Option<MapData> {
    let mount = install_mount()?;
    let mut map = MapData::default();
    maps::dress_guest_in(&mount, &mut map, tf2::game::BUNDLE.kept, false);
    Some(map)
}

/// This machine's player, in `class`'s arms (None: the gun's own, the
/// Scout's, as a body of another game holds it), reloads the Pistol from
/// 3 rounds on `map`'s view models and sound table.
fn pistol_reload(map: &MapData, class: Option<&str>) -> (Heard, Entity) {
    let mut sim = sim();
    sim.app.world_mut().insert_resource(ViewModels::of(map.kit.view_models.clone()));
    let (p, w) = holding(&mut sim, greybox::SPAWNS[0], PISTOL_SCOUT);
    // The table the dressing made, over the greybox's own.
    sim.app.world_mut().insert_resource(SoundBank(map.sounds.clone()));
    let mut e = sim.app.world_mut().entity_mut(p);
    e.insert(LocalPlayer);
    if let Some(c) = class {
        // The class a rung drew (`classes::RungClass`): its arms
        // (`view::view_variants`).
        e.insert(tf2::game::classes::RungClass(c.into()));
    }
    sim.seconds(1.0);
    (reload(&mut sim, p, w, 3, 80, |_, _| false), p)
}

/// Every sound the owner was asked to play is the table's, with waves.
fn all_play(map: &MapData, h: &Heard) {
    for (t, e) in &h.played {
        let entry = map.sounds.entry(e).unwrap_or_else(|| panic!("tick {t}: {e} not in the table"));
        assert!(!entry.waves.is_empty(), "tick {t}: {e} has no waves");
    }
}

/// Test cases 1 and 2: the Scout's arms (a body of another game holds
/// the Pistol in them) and the Engineer's.
#[test]
fn the_local_players_pistol_reload_plays_the_specs_sounds_1_2() {
    let Some(map) = dressed() else { return };
    let (h, p) = pistol_reload(&map, None);
    let scout = ticks_of(&h.played, RELOAD_SCOUT);
    assert!(scout.len() == 1 && (2..=3).contains(&scout[0]), "Scout: ReloadScout at {scout:?} ({:?})", h.played);
    let clip = ticks_of(&h.played, CLIP_IN);
    assert!(clip.len() == 1 && (39..=40).contains(&clip[0]), "Scout: ClipIn at {clip:?}");
    assert!(ticks_of(&h.played, WORLD_RELOAD).is_empty());
    assert_eq!(h.others, vec![(0, p, WORLD_RELOAD.to_string())]);
    all_play(&map, &h);

    let (h, p) = pistol_reload(&map, Some("engineer"));
    assert!(ticks_of(&h.played, RELOAD_SCOUT).is_empty(), "the Engineer's arms carry ClipIn alone: {:?}", h.played);
    let clip = ticks_of(&h.played, CLIP_IN);
    assert!(clip.len() == 1 && (31..=32).contains(&clip[0]), "Engineer: ClipIn at {clip:?}");
    assert_eq!(h.others, vec![(0, p, WORLD_RELOAD.to_string())]);
    all_play(&map, &h);
}

/// The served Engineer whose store lacks `ClipIn`'s wave hears the world
/// reload at its time, not nothing.
#[test]
fn a_missing_clip_in_wave_leaves_the_engineer_the_world_reload() {
    let Some(mut map) = dressed() else { return };
    std::sync::Arc::make_mut(&mut map.sounds)
        .entries
        .get_mut(&CLIP_IN.to_lowercase())
        .expect("ClipIn in the table")
        .waves
        .clear();
    let (h, _) = pistol_reload(&map, Some("engineer"));
    assert!(ticks_of(&h.played, CLIP_IN).is_empty(), "{:?}", h.played);
    let instead = ticks_of(&h.played, WORLD_RELOAD);
    assert!(instead.len() == 1 && (31..=32).contains(&instead[0]), "the world reload at ClipIn's time: {instead:?}");
    all_play(&map, &h);
}
