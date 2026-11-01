//! What HL2:DM tells the shell (`game::INFO`), with no install.

use chimera_core::mount::{FileSource, Mount, install};
use hl2dm::game::{GAME, INFO, STOCK_MAPS};

#[test]
fn names_its_game_folder_app_and_namespaces() {
    assert_eq!(INFO.id, GAME);
    assert_eq!(INFO.content_dir, "hl2mp");
    assert_eq!(INFO.steam.map(|s| s.app_id), Some(320));
    assert_eq!(INFO.namespaces(), ["hl2dm", "hl2mp"]);
    assert_eq!(INFO.map_id("dm_lockdown"), "hl2dm:dm_lockdown");
    assert_eq!((INFO.map_file)("halls3").as_deref(), Some("maps/halls3.bsp"));
    assert_eq!(install::root_env(GAME), "CHIMERA_HL2DM_ROOT");
    assert_eq!(STOCK_MAPS.len(), 8);
}

#[test]
fn lists_maps_and_the_cycle_from_the_mount() {
    struct Fake;
    impl FileSource for Fake {
        fn name(&self) -> String {
            "fake".into()
        }
        fn entries(&self) -> Vec<chimera_core::mount::Entry> {
            [
                "maps/dm_lockdown.bsp",
                "maps/halls3.bsp",
                "maps/dm_lockdown.nav",
                "cfg/mapcycle_default.txt",
            ]
            .iter()
            .map(|p| chimera_core::mount::Entry {
                path: p.to_string(),
                size: 1,
            })
            .collect()
        }
        fn read(&self, path: &str) -> Option<std::io::Result<Vec<u8>>> {
            (path == "cfg/mapcycle_default.txt").then(|| Ok(b"// cycle\ndm_lockdown\ndm_missing\n".to_vec()))
        }
    }
    let mut mount = Mount::default();
    mount.push(Fake);
    assert_eq!((INFO.map_names)(&mount), ["dm_lockdown", "halls3"]);
    assert_eq!(
        (INFO.map_cycle)(&mount),
        ["dm_lockdown"],
        "only the cycle's maps the install has"
    );
}
