//! The install's view models drawn on the greybox: the local player's
//! M4A1 and knife load on demand (`game::view_model::on_demand`, as the
//! front end turns on) and their meshes are drawn, visible, by the
//! anchor's view-model camera, with no map's drawing set up
//! (`render::view_model::assets_without_map`). The playtest's bug: on the
//! greybox the models loaded but nothing drew them (only a map's spawn
//! built the drawing's assets) while the placeholder box had stepped
//! aside. Both archives are V2-keyed and their paths come from the
//! install's weapon data (V1): skipped without the install or the keys,
//! or in a process another test pinned to the built-in data (`CaSim`).

use bevy::{camera::visibility::RenderLayers, prelude::*};
use chimera_core::{core::LocalPlayer, mount::config::LocalConfig};
use chimera_present::{
    MapPlugin,
    render::{
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        view_model::{ViewModelAssets, ViewModelBody},
        world_material::WorldMaterial,
    },
};
use chimera_sim::weapon::{Inventory, Weapon};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{ViewAnimator, ViewModelAnchor, ViewModelCamera, ViewModels, view_model::VIEW_MODEL_LAYER},
};
use combat_arms::{
    game::{GAME, TICK_INTERVAL, data, inventory::KNIFE, mount, movement, rules::CaRuleSettings, view_model::on_demand::{self, LoadOnDemand}, weapons::RIFLE},
};

fn keyed_install() -> bool {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return false;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return false;
    }
    if !mount::can_read("V2") {
        eprintln!("skipping: no V2 key in chimera.local.toml");
        return false;
    }
    true
}

/// The view model drawn for the local player: its name's key, whether it
/// is shown, and its meshes on the view-model layer.
fn drawn(sim: &mut Sim) -> Option<(String, bool, usize)> {
    let world = sim.app.world_mut();
    let (body, name, visibility) = world
        .query_filtered::<(Entity, &Name, &Visibility), With<ViewModelBody>>()
        .iter(world)
        .map(|(e, n, v)| (e, n.as_str().to_string(), *v))
        .next()?;
    let layer = RenderLayers::layer(VIEW_MODEL_LAYER);
    let meshes = world
        .query::<(&Mesh3d, &RenderLayers, &ChildOf)>()
        .iter(world)
        .filter(|(_, l, c)| c.parent() == body && l.intersects(&layer))
        .count();
    Some((name, visibility != Visibility::Hidden, meshes))
}

/// Run until weapon `id`'s view model is drawn and shown (its background
/// load and first pose), at most 20 s of ticks.
fn wait_for(sim: &mut Sim, id: &str) -> (bool, usize) {
    let want = format!("View model {id}");
    for _ in 0..(20 * 66) {
        sim.ticks(1);
        if let Some((name, shown, meshes)) = drawn(sim)
            && name == want
            && shown
        {
            return (shown, meshes);
        }
        // The load runs on the task pool's threads.
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let world = sim.app.world_mut();
    let models: Option<Vec<String>> =
        world.get_resource::<ViewModels>().map(|m| m.0.iter().map(|v| v.key.clone()).collect());
    let assets = world.get_resource::<ViewModelAssets>().is_some();
    let shown: Vec<Option<String>> = world.query::<&ViewAnimator>().iter(world).map(|v| v.key.clone()).collect();
    let cameras = world.query::<&ViewModelCamera>().iter(world).count();
    panic!(
        "{id}: no view model drawn on the greybox: {:?}; models {models:?}, assets {assets}, shown {shown:?}, \
         view-model cameras {cameras}, cached {:?}",
        drawn(sim),
        on_demand::cached(id).map(|r| r.map(|_| ()))
    );
}

#[test]
fn rifle_and_knife_view_models_draw_on_the_greybox() {
    if !keyed_install() {
        return;
    }
    // As the app composes it, on the install's data (`CaSim` keeps to the
    // built-in data, which names no M4A1 or knife view model).
    let mut sim = Sim::with((combat_arms::SimPlugins, combat_arms::game::CombatArmsPlugins), |app| {
        app.insert_resource(Time::<Fixed>::from_seconds(TICK_INTERVAL));
        app.world_mut().resource_mut::<CaRuleSettings>().mode = "none".into();
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins((GreyboxMapPlugin, MapPlugin::empty()))
            .init_resource::<LoadOnDemand>();
    });
    if data::CaData::install().attributes().is_none() {
        eprintln!("skipping: the process runs on the built-in data (another test's CaSim)");
        return;
    }
    let me = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    // The first-person eye (the client shell's camera carries the anchor).
    sim.app.world_mut().spawn((
        Camera3d::default(),
        ViewModelAnchor,
        Transform::from_xyz(0.0, 0.6, 0.0),
        Visibility::default(),
        ChildOf(me),
    ));
    let (shown, meshes) = wait_for(&mut sim, RIFLE);
    eprintln!("{RIFLE}: shown {shown}, {meshes} meshes");
    assert!(meshes > 0, "{RIFLE}: no meshes");
    // To the knife.
    let knife = {
        let world = sim.app.world();
        world
            .get::<Inventory>(me)
            .unwrap()
            .weapons
            .iter()
            .map(|w| world.get::<Weapon>(*w).unwrap())
            .find(|w| w.id == KNIFE)
            .expect("the knife")
            .slot
    };
    sim.intent(me).select = Some(knife);
    sim.ticks(2);
    sim.intent(me).select = None;
    let (shown, meshes) = wait_for(&mut sim, KNIFE);
    eprintln!("{KNIFE}: shown {shown}, {meshes} meshes");
    assert!(meshes > 0, "{KNIFE}: no meshes");
}
