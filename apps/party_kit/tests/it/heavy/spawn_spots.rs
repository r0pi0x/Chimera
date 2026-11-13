//! The gun game's spawn spots on every map of the pool (`auto::map_pool`
//! over the installed games' maps), loaded as the dedicated server loads
//! them with the party's parts (the map logic) and the gun game started:
//! `party_kit::spot_checks`, what `map_fitness` reports
//! (docs/plans/active/map-pool-fitness.md). No spot off the map's
//! navigation file (Source maps that ship one: cs_militia's hills,
//! havana's clipped yard, de_port's roofs), none a standing player is
//! stuck in (the box, not a capsule, and the player clips), and at most
//! `OFF_REGION_SHARE` of them (or `OFF_REGION_FEW`) off the main region,
//! or off the main region of the bots' generated mesh (a bot spawned off
//! it has no path to or from the others: cs_militia's high floors).
//! Prints each map's counts (`--nocapture`); `CHIMERA_SPAWN_SPOTS=<text>`
//! checks only the maps whose id has the text. Passes with a note without
//! the CS:S and Combat Arms installs; a game not installed is skipped.
//!
//! The pool (about 138 maps, 656 s one after another) is split into
//! `SHARDS` tests by a stable hash of the map id, so the heavy slots run
//! them in parallel and none nears the timeout; each map is in exactly one
//! shard (`shards_cover_the_pool_once`).

use bevy::prelude::*;
use chimera_core::mount::config::LocalConfig;
use chimera_sim::minigame::auto;
use chimera_world::{
    harness::Sim,
    map::{KitLoad, LoadContext, MapLoadOptions, MapSimPlugin},
};
use combat_arms::harness::CaSim;
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin, spot_checks};

/// Off the main region allowed: this share of a map's spots, or this
/// many, whichever is more (the region's slack misses a stair's edge).
const OFF_REGION_SHARE: f32 = 0.01;
const OFF_REGION_FEW: usize = 10;

/// The number of shards the pool is split into (one test each, below).
const SHARDS: u64 = 6;

/// The shard a map is in: FNV-1a of its id, stable across runs and builds.
fn shard_of(id: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in id.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0000_0100_0000_01b3);
    }
    h % SHARDS
}

fn installed() -> bool {
    let Ok(config) = LocalConfig::load() else { return false };
    let css = config
        .game_path_in(cs_source::game::GAME, cs_source::game::INFO.steam.as_ref())
        .filter(|p| chimera_core::mount::served::usable("cs_source", p, "cstrike"));
    let ca = config.game_path(combat_arms::game::GAME).filter(|p| chimera_core::mount::served::usable("combat_arms", p, "Game"));
    css.is_some() && ca.is_some()
}

/// The pool: every game's claimed maps and the installed ones its
/// patterns take, the games with an install.
fn pool() -> Vec<String> {
    let mut sim = Sim::new(CaSim, PartyPartsPlugin);
    let w = sim.app.world_mut();
    auto::find_installed_maps(w);
    let caps = auto::report(w);
    let mounted = |game: &str| {
        party_kit::BUNDLES.iter().any(|b| b.info.id == game && b.info.mount().is_some())
    };
    auto::map_pool(&caps, &mounted).into_iter().filter(|m| m != "greybox").collect()
}

/// The pool maps passing `CHIMERA_SPAWN_SPOTS` (all when unset).
fn filtered_pool() -> (Option<String>, Vec<String>) {
    // `CHIMERA_SPAWN_SPOTS=<text>`: only the pool's maps whose id has it.
    let only = std::env::var("CHIMERA_SPAWN_SPOTS").ok();
    let pool = pool().into_iter().filter(|m| only.as_ref().is_none_or(|o| m.contains(o.as_str()))).collect();
    (only, pool)
}

macro_rules! shard_tests {
    ($($name:ident = $n:expr),* $(,)?) => {
        $(
            #[test]
            fn $name() {
                check_shard($n);
            }
        )*
    };
}

shard_tests! {
    spawn_spots_are_on_every_pool_maps_floors_shard_0 = 0,
    spawn_spots_are_on_every_pool_maps_floors_shard_1 = 1,
    spawn_spots_are_on_every_pool_maps_floors_shard_2 = 2,
    spawn_spots_are_on_every_pool_maps_floors_shard_3 = 3,
    spawn_spots_are_on_every_pool_maps_floors_shard_4 = 4,
    spawn_spots_are_on_every_pool_maps_floors_shard_5 = 5,
}

/// Every pool map is in exactly one of the shard tests (cheap: no map loads).
#[test]
fn shards_cover_the_pool_once() {
    if !installed() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let (_, pool) = filtered_pool();
    let mut seen = std::collections::HashSet::new();
    for id in &pool {
        assert!(shard_of(id) < SHARDS);
        assert!(seen.insert(id.clone()), "{id} twice in the pool");
    }
    let total: usize = (0..SHARDS).map(|n| pool.iter().filter(|m| shard_of(m) == n).count()).sum();
    assert_eq!(total, pool.len());
}

/// How many shard tests the macro above lists; keep equal to `SHARDS`.
const SHARDS_TESTED: u64 = 6;
const _: () = assert!(SHARDS == SHARDS_TESTED);

fn check_shard(shard: u64) {
    if !installed() {
        eprintln!("skipping: needs both the CS:S and the Combat Arms installs");
        return;
    }
    let (only, all) = filtered_pool();
    assert!(only.is_some() || all.len() >= 4 * SHARDS as usize, "{all:?}");
    let pool: Vec<String> = all.into_iter().filter(|m| shard_of(m) == shard).collect();
    eprintln!("shard {shard} of {SHARDS}: {} maps", pool.len());
    let context = LoadContext::default();
    let mut bad = Vec::new();
    for id in &pool {
        let options = MapLoadOptions {
            kit: KitLoad::Only(&[]),
            ..MapLoadOptions::lean().in_context(&context)
        };
        let data = match party_kit::load_map_with(id, &options) {
            Ok(d) => d,
            Err(e) => {
                bad.push(format!("{id}: doesn't load: {e}"));
                continue;
            }
        };
        let mut sim = Sim::new(CaSim, (MapSimPlugin::new(data), PartyPartsPlugin));
        chimera_sim::minigame::start(sim.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
        sim.ticks(3);
        let w = sim.app.world_mut();
        let feet = spot_checks::spawn_feet(w);
        let bots = spot_checks::bots_mesh(w, &feet);
        let r = spot_checks::report(w, &feet, &bots);
        let allowed = OFF_REGION_FEW.max((r.spots.len() as f32 * OFF_REGION_SHARE) as usize);
        eprintln!(
            "{id}: {} spots{}, off the file {}{}, stuck {}, off the region {}, off the bots' {} (allowed {allowed})",
            r.spots.len(),
            if r.points_only { " (the spawn points)" } else { "" },
            r.off_file.len(),
            if r.has_file { "" } else { " (no file)" },
            r.stuck.len(),
            r.off_region.len(),
            r.off_bots.len(),
        );
        if r.points_only && r.spots.is_empty() {
            bad.push(format!("{id}: no spawn spots or points"));
        }
        if !r.off_file.is_empty() {
            bad.push(format!("{id}: {} spots off its navigation file, e.g. {:?}", r.off_file.len(), &r.off_file[..r.off_file.len().min(4)]));
        }
        if !r.stuck.is_empty() {
            let at: Vec<Vec3> = r.stuck.iter().take(4).map(|(p, _)| *p).collect();
            bad.push(format!("{id}: {} spots a standing player is stuck in, e.g. {at:?}", r.stuck.len()));
        }
        if r.off_region.len() > allowed {
            bad.push(format!(
                "{id}: {} spots off the main region (allowed {allowed}), e.g. {:?}",
                r.off_region.len(),
                &r.off_region[..r.off_region.len().min(4)]
            ));
        }
        if r.off_bots.len() > allowed {
            bad.push(format!(
                "{id}: {} spots off the bots' main region (allowed {allowed}), e.g. {:?}",
                r.off_bots.len(),
                &r.off_bots[..r.off_bots.len().min(4)]
            ));
        }
    }
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
