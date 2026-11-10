//! A `map` command's load spawns the map a step at a time behind the
//! loading screen (`console::finish_map_load`, `map::prepare::
//! begin_map_change`; docs/performance.md, "A map's swap"): the simulation
//! waits while it spawns (no tick on a part of a map), the world gets the
//! whole map at once and the same map as a spawn in one go, and until the
//! game is entered a frame advances the simulation by at most one tick (no
//! catch-up ticks owed for the loading screen), given back after.

use bevy::prelude::*;
use chimera_client::client::{
    AppHooks,
    console::{self, MapLoadStage},
};
use chimera_core::core::SimClock;
use chimera_sim::SimBasePlugins;
use chimera_world::{
    harness::Sim,
    map::{
        MapBrush, MapData, MapDebugView, MapMesh, MapModel, MapPart, MapProp, MapSimPlugin, PropSolid, change_map,
    },
};

/// Props in the test map: several of the spawn's steps' worth.
const PROPS: usize = 300;

fn map() -> MapData {
    let mut data = MapData {
        name: "test:steps".into(),
        entity_scale: 0.0254,
        ..default()
    };
    let (a, b) = (Vec3::new(-50.0, -1.0, -50.0), Vec3::new(50.0, 0.0, 50.0));
    data.collision_brushes.push(MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
    data.meshes.push(MapMesh {
        material: "test/floor".into(),
        positions: vec![[-50.0, 0.0, -50.0], [-50.0, 0.0, 50.0], [50.0, 0.0, 50.0], [50.0, 0.0, -50.0]],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        uvs: vec![[0.0, 0.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..default()
    });
    let (lo, hi) = (Vec3::splat(-0.3), Vec3::splat(0.3));
    data.models.push(MapModel {
        bounds: (lo, hi),
        ..default()
    });
    for i in 0..PROPS {
        data.props.push(MapProp {
            pose: None,
            ragdoll: None,
            model: 0,
            translation: Vec3::new((i % 20) as f32 * 2.0 - 20.0, 0.31, (i / 20) as f32 * 2.0 - 15.0),
            rotation: Quat::IDENTITY,
            solid: PropSolid::Box,
            skybox: false,
            lighting: None,
            vertex_light: None,
            casts_shadow: false,
            physics: None,
            fade: None,
            parent: None,
            entity: None,
            skin: 0,
            body: 0,
            detail: false,
        });
    }
    data.spawns.push((Vec3::new(0.0, 0.1, 0.0), None));
    data
}

fn sim() -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins(MapSimPlugin::empty())
            .insert_resource(AppHooks {
                build: "test",
                load_map: |_, _| Ok(map()),
                load_map_file: |id, _, _| Err(format!("{id}: not in tests")),
                map_file_bytes: |_| None,
                unload_greybox: |_| {},
                respawn_greybox: |_| {},
            })
            .add_systems(Update, console::finish_map_load);
    });
    sim.ticks(2);
    sim
}

/// The map's parts: each entity's name, sorted.
fn parts(sim: &mut Sim) -> Vec<String> {
    let w = sim.app.world_mut();
    let mut names: Vec<String> = w
        .query_filtered::<Option<&Name>, With<MapPart>>()
        .iter(w)
        .map(|n| n.map_or(String::new(), |n| n.as_str().to_string()))
        .collect();
    names.sort();
    names
}

#[test]
fn a_map_spawns_in_steps_behind_the_screen_with_the_simulation_waiting() {
    let mut sim = sim();
    let default_max = sim.app.world().resource::<Time<Virtual>>().max_delta();
    console::start_map_load(sim.app.world_mut(), "test:steps", None);
    let mut stages = Vec::new();
    let mut spawning_ticks = Vec::new();
    let mut seen_parts = Vec::new();
    for _ in 0..600 {
        let before = console::map_load_stage(sim.app.world());
        if before == MapLoadStage::None && !stages.is_empty() {
            break;
        }
        sim.app.update();
        let after = console::map_load_stage(sim.app.world());
        stages.push(after);
        let w = sim.app.world_mut();
        let n = w.query_filtered::<(), With<MapPart>>().iter(w).count();
        if after == MapLoadStage::Spawning {
            spawning_ticks.push(w.resource::<SimClock>().tick);
            seen_parts.push(n);
        }
        if after == MapLoadStage::Settling {
            let tick = w.resource::<Time<Fixed>>().timestep();
            assert_eq!(
                w.resource::<Time<Virtual>>().max_delta(),
                tick,
                "settling: a frame advances at most one tick"
            );
            assert!(!w.resource::<Time<Virtual>>().is_paused(), "settling: the simulation runs");
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(stages.contains(&MapLoadStage::Spawning), "spawned in steps: {stages:?}");
    assert!(stages.contains(&MapLoadStage::Settling), "settled: {stages:?}");
    assert_eq!(console::map_load_stage(sim.app.world()), MapLoadStage::None, "in the game");
    assert!(
        spawning_ticks.windows(2).all(|t| t[0] == t[1]),
        "no tick while the map spawns: {spawning_ticks:?}"
    );
    assert!(seen_parts.iter().all(|n| *n == 0), "no part of the map before the whole: {seen_parts:?}");
    let w = sim.app.world();
    assert_eq!(w.resource::<Time<Virtual>>().max_delta(), default_max, "given back in the game");
    assert!(!w.resource::<Time<Virtual>>().is_paused());
    let stepped = parts(&mut sim);
    assert!(stepped.iter().filter(|n| n.starts_with("Prop ")).count() == PROPS, "every prop");
    // The same map spawned in one go.
    let mut once = self::sim();
    change_map(once.app.world_mut(), map(), MapDebugView::Normal);
    once.app.update();
    assert_eq!(stepped, parts(&mut once), "the same parts as a spawn in one go");
}
