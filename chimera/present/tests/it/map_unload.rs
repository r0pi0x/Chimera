//! A map's lifetime (docs/plans/active/architecture-review.md, A2): what
//! `map::change_map` and `map::unload_map` leave of the map they replace,
//! and what spawning frees. Headless, with every render asset store the
//! map spawns into.
//!
//! - Unloading removes everything the map made: its entities, its
//!   resources (`MapScopedApp::map_scoped`) and its assets. Any resource
//!   present after an unload that wasn't there before the first load must
//!   be registered as kept (`KEPT`), or it is a leak into the next map
//!   (and, in Lucker Party's rotation, the next game).
//! - Once spawned, the pending data is gone (`PendingMap`), and the
//!   decoded textures with it, but for the few texels the dynamic shadows
//!   sample (`render::shadows::shed_unread`), and the world's meshes and the kit.

use std::{
    any::TypeId,
    collections::{HashMap, HashSet},
};

use bevy::prelude::*;
use chimera_present::{
    MapPlugin,
    render::{
        beams::BeamMaterial,
        detail::DetailMaterial,
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::{ShadowMaterial, ShadowSource},
        sprite_material::SpriteMaterial,
        water::WaterMaterial,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapDebugView, MapPart, MapTexture, PendingMap, change_map, unload_map},
};

use crate::prop_shadows::{base, map};

/// Resources that outlive a map by design: they are the world's, not the
/// map's (what the map hands over to the next one, or lives in the app).
const KEPT: &[&str] = &[
    // The look the cameras follow: the next map replaces it, and a world
    // without a map keeps the last (`ActiveMapLook`).
    "ActiveMapLook",
    // Per-frame bookkeeping, reset in place.
    "MonitorCamera",
    "Gravity",
    // Bevy's cache of the spawner system `change_map` runs.
    "CachedSystemId",
];

fn texture(name: &str, size: u32) -> MapTexture {
    MapTexture {
        name: name.into(),
        srgb: true,
        mips: Vec::new(),
        width: size,
        height: size,
        rgba8: vec![200; (size * size * 4) as usize],
        bc: None,
    }
}

/// `prop_shadows`' floor with a crate casting a shadow, with textures: a
/// 128 x 128 one on the floor (read by nobody after the spawn) and a
/// small alpha-tested one on the crate (sampled by its shadow).
fn textured(name: &str) -> MapData {
    let mut data = map();
    data.name = name.into();
    data.textures = vec![texture("floor", 128), texture("crate", 8)];
    data.meshes[0].texture = Some(0);
    data.models[0].meshes[0].texture = Some(1);
    data.models[0].meshes[0].alpha = chimera_world::map::MapAlpha::Mask(0.5);
    data
}

fn sim() -> Sim {
    Sim::with(base(), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<WaterMaterial>()
            .init_asset::<BeamMaterial>()
            .init_asset::<DetailMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::empty());
    })
}

/// Every asset store's size, by name.
fn assets(world: &World) -> Vec<(&'static str, usize)> {
    vec![
        ("meshes", world.resource::<Assets<Mesh>>().len()),
        ("images", world.resource::<Assets<Image>>().len()),
        ("standard", world.resource::<Assets<StandardMaterial>>().len()),
        ("world", world.resource::<Assets<WorldMaterial>>().len()),
        ("prop", world.resource::<Assets<PropMaterial>>().len()),
        ("sprite", world.resource::<Assets<SpriteMaterial>>().len()),
        ("rope", world.resource::<Assets<RopeMaterial>>().len()),
        ("shadow", world.resource::<Assets<ShadowMaterial>>().len()),
        ("water", world.resource::<Assets<WaterMaterial>>().len()),
        ("beam", world.resource::<Assets<BeamMaterial>>().len()),
        ("detail", world.resource::<Assets<DetailMaterial>>().len()),
    ]
}

fn resources(world: &World) -> HashMap<TypeId, String> {
    world.iter_resources().filter_map(|(_, info, _)| Some((info.type_id()?, info.name().to_string()))).collect()
}

fn parts(world: &mut World) -> Vec<Entity> {
    world.query_filtered::<Entity, With<MapPart>>().iter(world).collect()
}

/// Fails on anything of the map `before` left in the world.
fn assert_gone(
    sim: &mut Sim,
    what: &str,
    before: &HashMap<TypeId, String>,
    assets_before: &[(&str, usize)],
    loaded: &[Entity],
) {
    // What a map's resources hold is freed on a task (`map::drop_later`).
    let started = std::time::Instant::now();
    while assets(sim.app.world_mut()) != assets_before && started.elapsed().as_secs() < 5 {
        sim.ticks(1);
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    let world = sim.app.world_mut();
    for e in loaded {
        assert!(world.get_entity(*e).is_err(), "{what}: {e} (a map part) is still there");
    }
    assert!(parts(world).is_empty(), "{what}: map parts left");
    let leaked: Vec<String> = resources(world)
        .into_iter()
        .filter(|(id, name)| !before.contains_key(id) && !KEPT.iter().any(|k| name.contains(k)))
        .map(|(_, name)| name)
        .collect();
    assert!(
        leaked.is_empty(),
        "{what}: resources the map left behind (map_scoped them, or list them as kept): {leaked:#?}"
    );
    assert_eq!(assets(world), assets_before, "{what}: assets the map left behind");
}

#[test]
fn unloading_a_map_leaves_nothing_of_it() {
    let mut sim = sim();
    sim.ticks(3);
    let before = resources(sim.app.world());
    let assets_before = assets(sim.app.world());
    for (i, name) in ["test:unload_a", "test:unload_b"].into_iter().enumerate() {
        change_map(sim.app.world_mut(), textured(name), MapDebugView::Normal);
        sim.ticks(5);
        let world = sim.app.world_mut();
        let loaded = parts(world);
        assert!(loaded.len() > 3, "map {i}: spawned its parts");
        assert!(world.resource::<Assets<Image>>().len() > 2, "map {i}: spawned its images");
        assert!(world.contains_resource::<ShadowSource>(), "map {i}: the crate's shadow is set up");
        unload_map(world);
        sim.ticks(3);
        assert_gone(&mut sim, &format!("map {i} unloaded"), &before, &assets_before, &loaded);
    }
}

#[test]
fn changing_maps_replaces_the_old_one_whole() {
    let mut sim = sim();
    sim.ticks(3);
    let before = resources(sim.app.world());
    change_map(sim.app.world_mut(), textured("test:change_a"), MapDebugView::Normal);
    sim.ticks(5);
    let a = parts(sim.app.world_mut());
    let a_images = sim.app.world().resource::<Assets<Image>>().len();
    change_map(sim.app.world_mut(), textured("test:change_b"), MapDebugView::Normal);
    sim.ticks(5);
    let world = sim.app.world_mut();
    for e in &a {
        assert!(world.get_entity(*e).is_err(), "{e} of the first map is still there");
    }
    assert_eq!(world.resource::<Assets<Image>>().len(), a_images, "as many images as one map has");
    assert_eq!(world.resource::<chimera_world::map::MapFile>().name, "test:change_b");
    // The same set of resources as after the first map's load.
    let now: HashSet<TypeId> = resources(world).into_keys().collect();
    let first: HashSet<TypeId> = before.keys().copied().collect();
    assert!(first.is_subset(&now), "nothing of the world's went with the map");
}

/// Once spawned the pending data is gone, and the decoded textures with
/// it, but for what the shadows sample.
#[test]
fn spawning_frees_the_pending_data_and_its_textures() {
    // At startup too: the plugin gives its share up, so the pending data
    // is the only owner when it is spawned.
    let mut sim = Sim::with(base(), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(textured("test:startup")));
    });
    sim.ticks(3);
    check_freed(&mut sim);
    // And a map changed to.
    let mut sim = self::sim();
    change_map(sim.app.world_mut(), textured("test:later"), MapDebugView::Normal);
    sim.ticks(3);
    check_freed(&mut sim);
}

fn check_freed(sim: &mut Sim) {
    let world = sim.app.world();
    assert!(!world.contains_resource::<PendingMap>(), "the pending map is gone once spawned");
    let source = world.resource::<ShadowSource>();
    assert!(
        source.data.meshes.is_empty() && source.data.kit.characters.is_empty() && source.data.extras.is_empty(),
        "the shadows keep no world meshes, kit or extensions"
    );
    assert_eq!(source.data.textures.len(), 2, "the shadows keep the texture list");
    assert!(source.data.textures[0].rgba8.is_empty(), "the floor's texels are not kept");
    assert_eq!(source.data.textures[0].width, 128, "its size is");
    assert_eq!(source.data.textures[1].rgba8.len(), 8 * 8 * 4, "the crate's alpha is: its shadow samples it");
}
