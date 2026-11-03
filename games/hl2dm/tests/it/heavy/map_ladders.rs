//! The stock maps' `func_useableladder`s (specs/hl2dm/movement.md "The
//! ladder entity"; H0's census: 26 ladders and 131 dismount points on six
//! maps) as Source movement reads them (`useable_ladders::ladders_of`):
//! every ladder found with its ends, and dm_resistance's `ladder` (the
//! probe's, movement.md P10: bottom (-450, 299, -1215), top (-450, 299,
//! -947)) with its four dismount points. Skips without an install.

use chimera_world::map::MapLoadOptions;
use hl2dm::game::STOCK_MAPS;
use source_engine::source_movement::useable_ladders::ladders_of;

use super::map_stock::{convert, install_mount};

#[test]
fn the_stock_maps_ladders() {
    let Some(mount) = install_mount() else { return };
    let (mut ladders, mut nodes) = (0, 0);
    for name in STOCK_MAPS {
        let map = convert(&mount, name, &MapLoadOptions::default());
        let found = ladders_of(&map.entities);
        let placed = map.entities.iter().filter(|e| e.classname() == "info_ladder_dismount").count();
        eprintln!("{name}: {} ladders, {placed} dismount points", found.len());
        ladders += found.len();
        nodes += placed;
        assert!(found.iter().all(|l| l.top.z >= l.bottom.z && l.enabled));
        if *name == "dm_resistance" {
            let l = found
                .iter()
                .find(|l| (l.bottom - bevy::math::Vec3::new(-450.0, 299.0, -1215.0)).length() < 1.0)
                .expect("dm_resistance's ladder");
            assert!((l.top.z - -947.0).abs() < 1.0);
            assert_eq!(l.dismounts.len(), 4, "{:?}", l.dismounts);
        }
    }
    assert_eq!((ladders, nodes), (26, 131));
}
