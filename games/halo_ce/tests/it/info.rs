//! What Halo tells the shell (`game::INFO`), with no install.

use chimera_core::mount::{Entry, FileSource, Mount, install};
use halo_ce::game::{GAME, INFO, MULTIPLAYER_MAPS};

#[test]
fn names_its_game_world_and_namespaces() {
    assert_eq!(INFO.id, GAME);
    assert_eq!(INFO.short_name, "Halo");
    assert_eq!(INFO.content_dir, "");
    assert!(INFO.steam.is_none());
    assert_eq!(INFO.namespaces(), ["halo_ce", "halo"]);
    assert_eq!(INFO.map_id("bloodgulch"), "halo_ce:bloodgulch");
    assert_eq!(install::root_env(GAME), "CHIMERA_HALO_CE_ROOT");
    // specs/halo_ce/cache_file.md, "The world": 30 ticks a second, 10 feet.
    assert_eq!(INFO.tick_interval, 1.0 / 30.0);
    assert_eq!(INFO.meters_per_unit, 3.048);
    assert!(!INFO.default_binds.is_empty());
}

#[test]
fn lists_the_thirteen_multiplayer_maps() {
    assert_eq!(MULTIPLAYER_MAPS.len(), 13);
    for name in ["bloodgulch", "sidewinder", "hangemhigh", "putput", "wizard"] {
        assert!(MULTIPLAYER_MAPS.contains(&name), "{name}");
    }
    // The campaign maps and the menu's are not multiplayer maps.
    assert!(!MULTIPLAYER_MAPS.contains(&"a10") && !MULTIPLAYER_MAPS.contains(&"ui"));
}

#[test]
fn lists_the_maps_the_mount_holds() {
    struct Fake;
    impl FileSource for Fake {
        fn name(&self) -> String {
            "fake".into()
        }
        fn entries(&self) -> Vec<Entry> {
            ["maps/wizard.map", "maps/bloodgulch.map", "maps/a10.map", "maps/ui.map", "maps/readme.txt"]
                .iter()
                .map(|p| Entry { path: p.to_string(), size: 1 })
                .collect()
        }
        fn read(&self, _: &str) -> Option<std::io::Result<Vec<u8>>> {
            None
        }
        fn contains(&self, path: &str) -> bool {
            self.entries().iter().any(|e| e.path == path)
        }
    }
    let mut mount = Mount::default();
    mount.push(Fake);
    assert_eq!((INFO.map_names)(&mount), ["bloodgulch", "wizard"]);
    // The cycle keeps the spec's order.
    assert_eq!((INFO.map_cycle)(&mount), ["bloodgulch", "wizard"]);
}

/// The map loader is Halo's, and a map the mount lacks fails with its
/// file's path (no panic, no install needed).
#[test]
fn the_map_loader_names_a_missing_maps_file() {
    assert_eq!(halo_ce::game::MAP_LOADER.game, halo_ce::game::GAME);
    let err = halo_ce::game::maps::load_in(&Mount::default(), "nowhere", &Default::default(), None).unwrap_err();
    assert!(err.contains("maps/nowhere.map"), "{err}");
}

/// Halo's pool data (data/map_pool.toml, data/map_fitness.toml): every
/// multiplayer map measured, and with teleporters working (M4) the rules
/// let in every map but Sidewinder, a vehicle map (the reasons are in the
/// file).
#[test]
fn its_pool_lets_in_every_map_but_the_vehicle_one() {
    use chimera_sim::minigame::map_fit::{MapPool, PoolRules, parse_fitness};
    use halo_ce::game::parts::{MAP_FITNESS, POOL_RULES};
    let measured = parse_fitness(MAP_FITNESS).unwrap();
    let mut names: Vec<&str> = measured.keys().map(String::as_str).collect();
    names.sort();
    assert_eq!(names, MULTIPLAYER_MAPS);
    let mut pool = MapPool::default();
    pool.add_rules(GAME, PoolRules::parse(POOL_RULES).unwrap());
    let listed: Vec<&str> = MULTIPLAYER_MAPS.iter().copied().filter(|m| pool.listed(GAME, m).is_ok()).collect();
    let all_but: Vec<&str> = MULTIPLAYER_MAPS.iter().copied().filter(|m| *m != "sidewinder").collect();
    assert_eq!(listed, all_but);
    assert!(pool.listed(GAME, "sidewinder").unwrap_err().contains("vehicle"));
}
