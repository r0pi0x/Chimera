//! The Combat Arms view models of another game's map on a client (docs/
//! performance.md, "A client's memory"): a front end's map loads
//! (`on_demand::ViewModelsOnDemand` in the app's `LoadContext`) take no
//! view models with the dressing (`world::DRESSING`), which converted all
//! of ours again for each map beside the ones loaded on demand; and one
//! model file's animations are converted once while something holds them
//! (`view_model::ViewAnimations`): the dressing's and the on-demand load's
//! of one gun, and two guns on the same file. Bytes, not times. Passes
//! with a note without the install.

use std::sync::Arc;

use chimera_world::map::{LoadContext, MapData, MapLoadOptions, MapViewModel, anim::AnimSet};
use combat_arms::game::{
    GAME, INFO,
    data::CaData,
    view_model::{self, on_demand},
    world,
};

fn mount() -> Option<chimera_core::mount::Mount> {
    let install = chimera_core::mount::config::LocalConfig::load().ok().and_then(|c| c.game_path(GAME));
    let Some(install) = install else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    (INFO.open)(&install).ok()
}

/// The bytes of a set's sampled tracks.
fn track_bytes(set: &AnimSet) -> usize {
    let track = |t: &chimera_world::map::anim::Track| {
        t.rotation.as_ref().map_or(0, |r| r.len() * 16) + t.position.as_ref().map_or(0, |p| p.len() * 12)
    };
    set.animations.iter().flat_map(|a| &a.tracks).map(track).sum()
}

fn anims(v: &MapViewModel) -> &Arc<AnimSet> {
    v.animations.as_ref().expect("a view model's animations")
}

#[test]
fn a_front_ends_dressing_leaves_the_view_models_to_on_demand() {
    if mount().is_none() {
        return;
    }
    let dressed = |context: LoadContext| {
        let mut map = MapData::default();
        (world::DRESSING.dress)(&mut map, &MapLoadOptions { context, ..Default::default() }).expect("dressed");
        map
    };
    let plain = dressed(LoadContext::default());
    if plain.kit.view_models.is_empty() {
        eprintln!("skipping: no view model loads ({:?})", plain.warnings.iter().find(|w| w.contains("view model")));
        return;
    }
    let bytes: usize = plain.kit.view_models.iter().map(|v| track_bytes(anims(v))).sum();
    eprintln!("the dressing without on-demand loads: {} view models, {} MB of animations", plain.kit.view_models.len(), bytes >> 20);
    let front_end = LoadContext::default();
    front_end.insert(on_demand::ViewModelsOnDemand);
    let map = dressed(front_end);
    assert!(
        map.kit.view_models.is_empty(),
        "a front end's view models load on demand, not with the dressing: {:?}",
        map.kit.view_models.iter().map(|v| &v.key).collect::<Vec<_>>()
    );
    assert!(!map.kit.characters.is_empty() || plain.kit.characters.is_empty(), "the rest of the dressing stays");
    assert_eq!(map.kit.held.len(), plain.kit.held.len(), "the held models stay");
}

#[test]
fn one_view_model_files_animations_are_converted_once() {
    const M92FS: &str = "combat_arms:beretta_m92fs";
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let ca = CaData::install();
    // The dressing's (an app without on-demand loads) and the on-demand
    // load's of the same gun share one set while both are held.
    let mut map = MapData::default();
    view_model::add_view_models(&mut map, &ca, &file);
    let Some(dressed) = map.kit.view_models.iter().find(|v| v.key == M92FS) else {
        eprintln!("skipping: no M92FS view model ({:?})", map.warnings);
        return;
    };
    let loaded = on_demand::load_weapon(&ca, M92FS, &file).expect("the M92FS's view model");
    assert!(Arc::ptr_eq(anims(dressed), anims(&loaded.model)), "one conversion of the M92FS's animations");
    eprintln!("the M92FS's animations: {} MB", track_bytes(anims(dressed)) >> 20);
    // Two guns on one file (a gun's other skins) share theirs too.
    let set = ca.weapon_set();
    let twins = set.weapons.iter().filter_map(|w| Some((w.id.as_str(), w.models.view.as_deref()?))).find_map(|(a, path)| {
        let b = set.weapons.iter().find(|w| w.id != a && w.models.view.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(path)))?;
        Some((a.to_string(), b.id.to_string()))
    });
    let Some((a, b)) = twins else {
        eprintln!("no two guns on one view-model file in this data");
        return;
    };
    let (Ok(one), Ok(two)) = (on_demand::load_weapon(&ca, &a, &file), on_demand::load_weapon(&ca, &b, &file)) else {
        eprintln!("skipping the twins: {a} or {b} doesn't load (the V2 key)");
        return;
    };
    assert!(Arc::ptr_eq(anims(&one.model), anims(&two.model)), "{a} and {b} share one file's animations");
    // Nothing holds them: the next load converts again (`Weak`).
    let held = ca.memo(view_model::ViewAnimations::default).held();
    drop((one, two, loaded, map));
    let after = ca.memo(view_model::ViewAnimations::default).held();
    assert!(after < held, "sets nothing holds are let go: {held} -> {after}");
}
