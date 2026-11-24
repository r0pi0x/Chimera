//! What CS 1.6 tells the shell (`game::INFO`): its id, folder, tick, unit,
//! Steam app with its required files and console namespaces
//! (docs/plans/active/goldsrc.md §2, §5).

use cs16::game::INFO;

#[test]
fn info_is_the_plan_s() {
    assert_eq!(INFO.id, "cs16");
    assert_eq!(INFO.content_dir, "cstrike");
    assert_eq!(INFO.tick_interval, 0.01);
    assert_eq!(INFO.meters_per_unit, 0.0254);
    let steam = INFO.steam.unwrap();
    assert_eq!((steam.app_id, steam.name), (10, "Half-Life"));
    assert_eq!(
        steam.required,
        &["cstrike/liblist.gam", "cstrike/maps/de_dust2.bsp", "valve/halflife.wad"]
    );
    assert_eq!(INFO.console_namespaces, &["cs16", "cstrike"]);
}
