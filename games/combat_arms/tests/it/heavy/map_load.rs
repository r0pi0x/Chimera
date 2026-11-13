//! What a Combat Arms map's load from the install costs and how much
//! memory it takes (docs/performance.md, "Combat Arms map loads"): the
//! session's first load (the weapon data and character models, converted
//! once, beside the world) and a second map after it, each under a budget
//! of instructions (the loading thread's and the helpers it starts:
//! `InstructionCounter::current_with_children`, the same on an idle box
//! and a busy one; wall seconds are printed for reading); and a weapon's
//! view model loading on demand (`view_model::on_demand`), once per
//! session. Without hardware counters (Windows) the budgets are skipped
//! with a note. Passes with a note
//! without `[games.combat_arms] path` in chimera.local.toml (or an asset
//! store serving Combat Arms); served by the store alone, a map or model
//! the packed sets lack is skipped with a note.

use std::time::Instant;

use chimera_core::{metrics::InstructionCounter, mount::config::LocalConfig};
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

/// `f`'s instructions on this thread and the threads it starts (None
/// without hardware counters), and its result.
fn counted<T>(f: impl FnOnce() -> T) -> (T, Option<u64>) {
    let mut c = InstructionCounter::current_with_children();
    let a = c.read();
    let out = f();
    let b = c.read();
    (out, a.zip(b).map(|(a, b)| b.saturating_sub(a)))
}

fn giga(n: Option<u64>) -> String {
    n.map_or("-".into(), |n| format!("{:.2} G instructions", n as f64 / 1e9))
}

/// Budgets of instructions: `load` for each map's load, `spawn` for the
/// spawn into a simulation and for its longest tick while the navigation
/// mesh is made.
struct Budget {
    load: u64,
    spawn: u64,
}

/// Map `name`, then `next` in the same session: each within its budget of
/// instructions (measured, with room for the maps' and code's growth: a
/// regression check), the process's peak memory under `peak_budget` MB.
fn loads_within(name: &str, next: &str, budget: Budget, peak_budget: u64) {
    let Some(mount) = mount() else { return };
    for map in [name, next] {
        if not_served(&mount, world::find(&mount, map).as_deref(), map) {
            return;
        }
    }
    for (k, map) in [name, next].into_iter().enumerate() {
        let started = Instant::now();
        let (data, count) = counted(|| world::load_from(&mount, map).unwrap_or_else(|e| panic!("{map}: {e}")));
        let took = started.elapsed();
        eprintln!(
            "{map} ({}): {}, {:.2} s, peak {} MB; {} meshes, {} textures, {} bodies, {} held, {} view models",
            if k == 0 { "first load" } else { "second" },
            giga(count),
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
        match count {
            Some(n) => assert!(n < budget.load, "{map}: {} (budget {})", giga(Some(n)), giga(Some(budget.load))),
            None => eprintln!("skipping the budget: no hardware counters ({})", chimera_core::metrics::hw_status()),
        }
        if k == 0 {
            spawn_and_navigate(map, data, budget.spawn);
        }
    }
    let peak = peak_mb();
    assert!(peak < peak_budget, "peak {peak} MB (budget {peak_budget} MB)");
}

/// The map spawned into a simulation (the main thread's part of a load:
/// the collision world), then a bot added: the navigation mesh generated
/// from the collision on the frame it wants one. The spawn and the
/// longest tick (`Sim::work`) each within `budget` instructions.
fn spawn_and_navigate(name: &str, map: MapData, budget: u64) {
    let started = Instant::now();
    let (mut sim, spawn) = counted(|| {
        let mut sim = Sim::new(CaSim, MapSimPlugin::new(map));
        sim.ticks(2);
        sim
    });
    let spawned = started.elapsed();
    bots::add_bot(sim.app.world_mut(), Some(1)).unwrap();
    let started = Instant::now();
    let mut longest: Option<u64> = None;
    for _ in 0..20 {
        let tick = sim.work(1).map(|w| w.ticks[0]);
        longest = longest.max(tick);
        if sim.app.world().contains_resource::<NavMesh>() {
            break;
        }
    }
    let areas = sim.app.world().get_resource::<NavMesh>().map_or(0, |m| m.areas.len());
    eprintln!(
        "{name}: spawned in {} ({:.2} s); navigation ({areas} areas) after {:.2} s, the longest tick {}",
        giga(spawn),
        spawned.as_secs_f32(),
        started.elapsed().as_secs_f32(),
        giga(longest)
    );
    assert!(areas > 0, "{name}: no navigation mesh");
    if let (Some(spawn), Some(longest)) = (spawn, longest) {
        assert!(
            spawn < budget && longest < budget,
            "{name}: spawned in {}, the longest tick {} (budget {})",
            giga(Some(spawn)),
            giga(Some(longest)),
            giga(Some(budget))
        );
    }
}

/// The peak after the second map varies from run to run on the same code
/// (2,267 to 2,716 MB over eight runs on the dev box, 2026-11-03; the first
/// load's 1,663 to 1,695 MB): the budget sits above that spread.
///
/// Instructions (2026-11-11, the same on every run): warehouse 56.6 G,
/// junk_flea after it 54.7 G (3.0-3.3 s idle; 20.3 s once under a whole
/// after-push run's load, which the old 20 s wall budget failed on); the
/// spawn 1.7 G, the longest tick 1.0 G.
#[test]
fn warehouse_loads_within_budget() {
    loads_within("warehouse", "junk_flea", Budget { load: 80_000_000_000, spawn: 3_000_000_000 }, 3000);
}

/// A lean load (`MapLoadOptions::lean`, the dedicated server's): the same
/// textures by name and size, no texels or mip levels, the character, held
/// and view models' too (`world::SizesOnly`: the server held 1-2 GB of them,
/// docs/performance.md, "The dedicated server's memory"), and none of the
/// effects' (it reads no effect file: `effects::library::add_looks`); the
/// whole load has both. The lean one first, so its peak memory shows alone.
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
    // The effects' textures (their particle materials') are the whole load's
    // alone: the lean one has no effect looks.
    let effects: std::collections::HashSet<usize> = full
        .particles
        .materials
        .iter()
        .filter(|m| m.name.starts_with("ca_fx:"))
        .filter_map(|m| m.texture)
        .collect();
    assert!(!effects.is_empty(), "the whole load has the effects' textures");
    assert!(lean.particles.materials.iter().all(|m| !m.name.starts_with("ca_fx:")), "the lean load has effect looks");
    assert!(combat_arms::game::effects::library::library(&lean.memos).is_none(), "the lean load has an effect library");
    let full_textures: Vec<&chimera_world::map::MapTexture> =
        full.textures.iter().enumerate().filter(|(i, _)| !effects.contains(i)).map(|(_, t)| t).collect();
    assert_eq!(lean.textures.len(), full_textures.len());
    let world_textures = lean.textures.iter().filter(|l| l.rgba8.is_empty()).count();
    assert!(world_textures > 10, "{world_textures} placeholders");
    let decoded: Vec<&str> = lean
        .textures
        .iter()
        .filter(|t| !t.rgba8.is_empty() || !t.mips.is_empty())
        .map(|t| t.name.as_str())
        .collect();
    // (Texels only: the shared models may arrive as blocks, below.)
    assert!(decoded.len() <= 1, "the lean load decoded {} textures: {:?}", decoded.len(), &decoded[..decoded.len().min(8)]);
    for (l, f) in lean.textures.iter().zip(full_textures.iter().copied()) {
        assert_eq!((&l.name, l.width, l.height), (&f.name, f.width, f.height));
        // Blocks the lean load holds are the process's shared character
        // models' kept from an earlier load in this process (cargo test
        // runs the tests in one; nextest, one each): this load decoded
        // none, and the whole load holds those textures as blocks alone.
        if l.bc.is_some() {
            assert!(f.rgba8.is_empty() && f.mips.is_empty(), "{}: blocks in the lean load, texels in the whole", l.name);
        }
        if l.rgba8.is_empty() {
            // (The shared character models' keep their blocks only: decoded.)
            assert!(l.mips.is_empty() && f.decoded().rgba8.len() == (f.width * f.height * 4) as usize, "{}", l.name);
        }
    }
    let texture_of = |m: &MapData| {
        m.meshes
            .iter()
            .map(|x| (x.texture.map(|i| m.textures[i].name.clone()), x.alpha))
            .collect::<Vec<_>>()
    };
    assert_eq!(texture_of(&lean), texture_of(&full), "the meshes find the same textures, alpha tests included");
}

/// TERRORDOME: the biggest lightmaps (4096 x 19428, laid out at half
/// resolution).
/// Instructions (2026-11-11): terrordome 85.4 G, warehouse after it 56.6 G;
/// the spawn 3.3 G, the longest tick 1.9 G.
#[test]
fn terrordome_loads_within_budget() {
    loads_within("terrordome", "warehouse", Budget { load: 120_000_000_000, spawn: 5_000_000_000 }, 3500);
}

/// A weapon's view model on demand: with its hands and own textures (the
/// meshes' indices into them), the second time from the session's cache.
#[test]
fn view_model_loads_on_demand_once() {
    let Some(mount) = mount() else { return };
    let file = |p: &str| mount.read(p).ok();
    let started = Instant::now();
    let ca = combat_arms::game::data::CaData::install();
    let first = match on_demand::load_cached(&ca, weapons::RIFLE, &file) {
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
    let (again, count) = counted(|| on_demand::load_cached(&ca, weapons::RIFLE, &file).unwrap());
    assert!(std::sync::Arc::ptr_eq(&first, &again));
    eprintln!("{} again, from the session's cache: {}", weapons::RIFLE, giga(count));
    // A lookup, not a load (the old wall budget: 50 ms).
    if let Some(n) = count {
        assert!(n < 10_000_000, "{n} instructions from the cache");
    }
}
