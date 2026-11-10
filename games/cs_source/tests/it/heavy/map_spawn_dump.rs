//! A tool for refactors of map spawning (`map::spawn_map`), not a check:
//! with `CHIMERA_SPAWN_DUMP=<dir>` it loads de_dust2, de_nuke and a few
//! community maps headless with every render asset store the map spawns
//! into, runs one update and writes each map's world to `<dir>/<map>.txt`:
//! every entity in id order with its name, parent, components and their
//! reflected values, then the resources and the asset counts. Dump before
//! and after a change and `diff` the folders: a move-only change gives
//! identical files. Without the variable it does nothing (maps missing
//! from the install or the content cache are skipped). Each schedule's
//! systems in the order they run (the order Bevy's topological sort gives,
//! which follows registration order where systems aren't ordered) go to
//! `<dir>/<map>.systems.txt`, for refactors of the map's registration
//! (`map::build`). `CHIMERA_SPAWN_DUMP_PREPARED=1` spawns each map with its
//! parts prepared ahead (`map::prepare`, as the client's background load
//! does): the dumps must not differ. `CHIMERA_SPAWN_DUMP_VIA=change` swaps
//! each map in after the start instead (`map::prepare::change_map_prepared`,
//! all at once), `=stepped` the same a step a frame
//! (`map::prepare::begin_map_change`, the simulation paused between, as the
//! client's loading screen does): those two dumps must not differ either.
//!
//! `cargo test --features dev --test it heavy::map_spawn_dump -- --nocapture`

use std::{fmt::Write as _, path::PathBuf};

use bevy::{ecs::reflect::ReflectComponent, prelude::*};
use crate::stack::{
    games,
    harness::{CsSim, Sim},
    map::MapPlugin,
    render::{
        beams::BeamMaterial, detail::DetailMaterial, prop_material::PropMaterial, rope_material::RopeMaterial,
        shadows::ShadowMaterial, sprite_material::SpriteMaterial, water::WaterMaterial, world_material::WorldMaterial,
    },
};

const MAPS: &[&str] = &["de_dust2", "de_nuke", "mg_lego_multigames_v2", "gg_tbr_water_basin", "surf_demise"];

/// Every asset store `spawn_map` spawns into (headless `Sim` has meshes only).
fn render_assets(app: &mut App) {
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
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
}

fn dump(world: &mut World) -> String {
    let registry = world.resource::<AppTypeRegistry>().clone();
    let registry = registry.read();
    let mut entities: Vec<Entity> = world.query::<Entity>().iter(world).collect();
    entities.sort();
    let mut out = String::new();
    for e in entities {
        let entity = world.entity(e);
        let name = entity.get::<Name>().map_or(String::new(), |n| n.as_str().to_string());
        let parent = entity.get::<ChildOf>().map(|p| p.parent());
        let _ = writeln!(out, "{e} {name:?} parent {parent:?}");
        let mut lines: Vec<String> = entity
            .archetype()
            .iter_components()
            .map(|id| {
                let info = world.components().get_info(id).expect("component info");
                let name = info.name().to_string();
                // Timings differ from run to run.
                if name.contains("::diagnostics::") || name.contains("bevy_time::real::") {
                    return format!("  {name}");
                }
                let value = info
                    .type_id()
                    .and_then(|t| registry.get(t))
                    .and_then(|r| r.data::<ReflectComponent>())
                    .and_then(|r| r.reflect(entity))
                    .map(|v| format!("{v:?}"))
                    .unwrap_or_default();
                format!("  {name} {value}")
            })
            .collect();
        lines.sort();
        for l in lines {
            let _ = writeln!(out, "{l}");
        }
    }
    let mut resources: Vec<String> = world.iter_resources().map(|(_, info, _)| format!("{}", info.name())).collect();
    resources.sort();
    let _ = writeln!(out, "resources:");
    for r in resources {
        let _ = writeln!(out, "  {r}");
    }
    let _ = writeln!(out, "assets:");
    let _ = writeln!(out, "  meshes {}", world.resource::<Assets<Mesh>>().len());
    let _ = writeln!(out, "  images {}", world.resource::<Assets<Image>>().len());
    let _ = writeln!(out, "  standard {}", world.resource::<Assets<StandardMaterial>>().len());
    let _ = writeln!(out, "  world {}", world.resource::<Assets<WorldMaterial>>().len());
    let _ = writeln!(out, "  prop {}", world.resource::<Assets<PropMaterial>>().len());
    let _ = writeln!(out, "  sprite {}", world.resource::<Assets<SpriteMaterial>>().len());
    let _ = writeln!(out, "  rope {}", world.resource::<Assets<RopeMaterial>>().len());
    let _ = writeln!(out, "  water {}", world.resource::<Assets<WaterMaterial>>().len());
    let _ = writeln!(out, "  beam {}", world.resource::<Assets<BeamMaterial>>().len());
    let _ = writeln!(out, "  detail {}", world.resource::<Assets<DetailMaterial>>().len());
    out
}

#[test]
fn dump_spawned_maps() {
    let Some(dir) = chimera_core::env::var_os("CHIMERA_SPAWN_DUMP").map(PathBuf::from) else {
        eprintln!("CHIMERA_SPAWN_DUMP not set: nothing to dump");
        return;
    };
    std::fs::create_dir_all(&dir).unwrap();
    // `CHIMERA_SPAWN_DUMP_ONLY=<map>`: just that one (its own process, so
    // the RSS line below is that map's).
    let only = chimera_core::env::var_os("CHIMERA_SPAWN_DUMP_ONLY").map(|v| v.to_string_lossy().into_owned());
    for name in MAPS.iter().filter(|n| only.as_deref().is_none_or(|o| o == **n)) {
        let map = match games::load_map(&format!("cs_source:{name}")) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("skipping {name}: {e}");
                continue;
            }
        };
        let via = chimera_core::env::var("CHIMERA_SPAWN_DUMP_VIA").ok();
        if let Some(via) = via.as_deref() {
            let mut sim = Sim::with(CsSim, |app| {
                app.add_plugins((render_assets, MapPlugin::empty()));
            });
            let w = sim.app.world_mut();
            let prepared = crate::stack::map::prepare::preparation(w).run(&map);
            let view = crate::stack::map::MapDebugView::Normal;
            if via == "stepped" {
                crate::stack::map::prepare::begin_map_change(w, map, prepared, view);
                w.resource_mut::<Time<Virtual>>().pause();
                let mut frames = 0;
                while !crate::stack::map::prepare::spawn_some(sim.app.world_mut(), std::time::Duration::ZERO) {
                    sim.app.update();
                    frames += 1;
                }
                sim.app.world_mut().resource_mut::<Time<Virtual>>().unpause();
                eprintln!("{name}: spawned over {frames} frames");
            } else {
                crate::stack::map::prepare::change_map_prepared(w, map, prepared, view);
            }
            sim.app.update();
            let text = dump(sim.app.world_mut());
            let path = dir.join(format!("{name}.txt"));
            std::fs::write(&path, &text).unwrap();
            eprintln!("{name}: {} lines to {}", text.lines().count(), path.display());
            continue;
        }
        let mut sim = Sim::with(CsSim, |app| {
            app.add_plugins((render_assets, MapPlugin::new(map)));
            // `CHIMERA_SPAWN_DUMP_PREPARED=1`: its spawn's parts computed
            // ahead, as a background load does (`map::prepare`).
            if chimera_core::env::var("CHIMERA_SPAWN_DUMP_PREPARED").is_ok_and(|v| v == "1") {
                let parts = crate::stack::map::prepare::prepare_pending(app.world_mut());
                eprintln!("{name}: {parts} parts prepared ahead");
            }
        });
        sim.app.update();
        eprintln!("{name}: resident {} MB after load", rss_mb());
        let text = dump(sim.app.world_mut());
        let path = dir.join(format!("{name}.txt"));
        std::fs::write(&path, &text).unwrap();
        eprintln!("{name}: {} lines to {}", text.lines().count(), path.display());
        // After the world's dump: building a schedule initializes its
        // systems (which may add resources).
        let order = crate::stack::harness::system_order(sim.app.world_mut());
        std::fs::write(dir.join(format!("{name}.systems.txt")), order).unwrap();
    }
}

/// This process's resident memory in MB (Linux; 0 elsewhere).
fn rss_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find_map(|l| l.strip_prefix("VmRSS:"))
                .and_then(|l| l.split_whitespace().next()?.parse::<u64>().ok())
        })
        .map_or(0, |kb| kb / 1024)
}
