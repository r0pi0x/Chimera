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

#[test]
fn the_map_loader_says_it_is_not_written() {
    let err = (halo_ce::game::MAP_LOADER.load)("bloodgulch", &Default::default()).unwrap_err();
    assert!(err.contains("not implemented yet"), "{err}");
}
