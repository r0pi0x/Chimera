//! How long a Combat Arms map takes to load from the install and how much
//! memory it takes (docs/performance.md, "Combat Arms map loads"): the
//! session's first load (the weapon data and character models, converted
//! once, beside the world) and a second map after it, each under a budget
//! with room for a busy box; and a weapon's view model loading on demand
//! (`view_model::on_demand`), once per session. Passes with a note
//! without `[games.combat_arms] path` in chimera.local.toml (or an asset
//! store serving Combat Arms); served by the store alone, a map or model
//! the packed sets lack is skipped with a note.

use std::time::{Duration, Instant};

use chimera_core::mount::config::LocalConfig;
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin, nav::NavMesh},
};
use combat_arms::{
    game::{GAME, INFO, bots, view_model::on_demand, weapons, world},
    harness::CaSim,
};

fn mount() -> Option<chimera_core::mount::Mount> {
    // The install, or the asset store's records of it (`mount::served`).
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    match (INFO.open)(&install) {
        Ok(mount) => Some(mount),
        Err(e) => {
            eprintln!("skipping: no Combat Arms install at {} ({e})", install.display());
            None
        }
    }
}

/// Whether `path` is missing because the mount is the asset store alone
/// (only the sets packed so far): the check is skipped with a note.
fn not_served(mount: &chimera_core::mount::Mount, path: Option<&str>, what: &str) -> bool {
    if INFO.served_partial() && path.is_none_or(|p| !mount.contains(p)) {
        eprintln!("skipping: {what} is not in the asset store's sets (the store alone, no install)");
        return true;
    }
    false
}

/// The process's peak resident memory (MB; Linux only, else 0).
fn peak_mb() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            let line = s.lines().find(|l| l.starts_with("VmHWM:"))?;
            line.split_whitespace().nth(1)?.parse::<u64>().ok()
        })
        .map_or(0, |kb| kb / 1024)
}

/// Map `name`, then `next` in the same session: each within `budget`
/// (wall time, measured at a few seconds on the dev box: the budget is a
/// regression check with room for load), the process's peak memory under
/// `peak_budget` MB.
fn loads_within(name: &str, next: &str, budget: Duration, peak_budget: u64) {
    let Some(mount) = mount() else { return };
    for map in [name, next] {
        if not_served(&mount, world::find(&mount, map).as_deref(), map) {
            return;
        }
    }
    for (k, map) in [name, next].into_iter().enumerate() {
        let started = Instant::now();
        let data = world::load_from(&mount, map).unwrap_or_else(|e| panic!("{map}: {e}"));
        let took = started.elapsed();
        eprintln!(
            "{map} ({}): {:.2} s, peak {} MB; {} meshes, {} textures, {} bodies, {} held, {} view models",
            if k == 0 { "first load" } else { "second" },
            took.as_secs_f32(),
            peak_mb(),
            data.meshes.len(),
            data.textures.len(),
            data.kit.characters.len(),
            data.kit.held.len(),
            data.kit.view_models.len(),
        );
        // View models load on demand, not with the map.
        assert!(data.kit.view_models.is_empty(), "{map}");
        assert!(took < budget, "{map}: {took:?} (budget {budget:?})");
        if k == 0 {
            spawn_and_navigate(map, data, budget);
        }
    }
    let peak = peak_mb();
    assert!(peak < peak_budget, "peak {peak} MB (budget {peak_budget} MB)");
}

/// The map spawned into a simulation (the main thread's part of a load:
/// the collision world), then a bot added: the navigation mesh generated
/// from the collision on the frame it wants one. Each within `budget`.
fn spawn_and_navigate(name: &str, map: MapData, budget: Duration) {
    let started = Instant::now();
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
    sim.ticks(2);
    let spawned = started.elapsed();
    bots::add_bot(sim.app.world_mut(), Some(1)).unwrap();
    let started = Instant::now();
    let mut longest = Duration::ZERO;
    for _ in 0..20 {
        let tick = Instant::now();
        sim.ticks(1);
        longest = longest.max(tick.elapsed());
        if sim.app.world().contains_resource::<NavMesh>() {
            break;
        }
    }
    let areas = sim.app.world().get_resource::<NavMesh>().map_or(0, |m| m.areas.len());
    eprintln!(
        "{name}: spawned in {:.2} s; navigation ({areas} areas) after {:.2} s, the longest tick {:.2} s",
        spawned.as_secs_f32(),
        started.elapsed().as_secs_f32(),
        longest.as_secs_f32()
    );
    assert!(areas > 0, "{name}: no navigation mesh");
    assert!(spawned < budget && longest < budget, "{name}");
}

#[test]
fn warehouse_loads_within_budget() {
    loads_within("warehouse", "junk_flea", Duration::from_secs(20), 2500);
}

/// A lean load (`MapLoadOptions::lean`, the dedicated server's): the same
/// world textures by name and size, no texels or mip levels; the whole
/// load has both. The lean one first, so its peak memory shows alone.
#[test]
fn a_lean_load_keeps_the_textures_sizes_and_drops_their_texels() {
    use chimera_world::map::MapLoadOptions;
    let Some(mount) = mount() else { return };
    let started = Instant::now();
    let lean = world::load_from_with(&mount, "warehouse", &MapLoadOptions::lean()).unwrap();
    let lean_took = started.elapsed();
    let lean_peak = peak_mb();
    let started = Instant::now();
    let full = world::load_from(&mount, "warehouse").unwrap();
    eprintln!(
        "warehouse: lean {:.2} s, peak {lean_peak} MB; whole {:.2} s, peak {} MB",
        lean_took.as_secs_f32(),
        started.elapsed().as_secs_f32(),
        peak_mb()
    );
    assert_eq!(lean.textures.len(), full.textures.len());
    let world_textures = lean.textures.iter().zip(&full.textures).filter(|(l, _)| l.rgba8.is_empty()).count();
    assert!(world_textures > 10, "{world_textures} placeholders");
    for (l, f) in lean.textures.iter().zip(&full.textures) {
        assert_eq!((&l.name, l.width, l.height), (&f.name, f.width, f.height));
        if l.rgba8.is_empty() {
            assert!(l.mips.is_empty() && f.rgba8.len() == (f.width * f.height * 4) as usize, "{}", l.name);
        }
    }
    let texture_of = |m: &MapData| m.meshes.iter().map(|m| (m.texture, m.alpha)).collect::<Vec<_>>();
    assert_eq!(texture_of(&lean), texture_of(&full), "the meshes find the same textures, alpha tests included");
}

/// TERRORDOME: the biggest lightmaps (4096 x 19428, laid out at half
/// resolution).
#[test]
fn terrordome_loads_within_budget() {
    loads_within("terrordome", "warehouse", Duration::from_secs(30), 3500);
}

/// A weapon's view model on demand: with its hands and own textures (the
/// meshes' indices into them), the second time from the session's cache.
#[test]
fn view_model_loads_on_demand_once() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let started = Instant::now();
    let first = match on_demand::load_cached(weapons::RIFLE, &file) {
        Ok(first) => first,
        Err(e) if not_served(&mount, None, &format!("{} ({e})", weapons::RIFLE)) => return,
        Err(e) => panic!("{e}"),
    };
    let took = started.elapsed();
    eprintln!(
        "{}: {:.2} s, {} meshes, {} textures, {} sequences, peak {} MB",
        weapons::RIFLE,
        took.as_secs_f32(),
        first.model.model.meshes.len(),
        first.textures.len(),
        first.model.animations.as_ref().map_or(0, |a| a.sequences.len()),
        peak_mb()
    );
    for w in &first.warnings {
        eprintln!("warning: {w}");
    }
    assert_eq!(first.model.key, weapons::RIFLE);
    assert!(!first.model.model.meshes.is_empty());
    for m in &first.model.model.meshes {
        if let Some(t) = m.texture {
            assert!(t < first.textures.len(), "{}: texture {t} of {}", m.material, first.textures.len());
        }
    }
    let started = Instant::now();
    let again = on_demand::load_cached(weapons::RIFLE, &file).unwrap();
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    assert!(started.elapsed() < Duration::from_millis(50));
}
