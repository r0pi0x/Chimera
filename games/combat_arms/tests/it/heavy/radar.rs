//! The install checks of specs/combat_arms/radar_minimap.md (46 to 52),
//! on the user's Combat Arms install: the map images' sizes and formats,
//! the radar art, the mission files' corners on Warehouse (Showdown),
//! Junk Flea 2, WarHead and Junk Flea, the large-map attribute file's world
//! size, and the server's whole path from the world and mode to the image
//! data (`mission::find`). Passes with a note without an install; the
//! mission files need the V1 key. Nothing read here is written anywhere.

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::game::{
    GAME, INFO, mount,
    radar::mission::{self, MapImage},
};
use lithtech_formats::{
    attributes::Document,
    dtx::{Dtx, Format},
};

fn install() -> Option<Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

fn v1(mount_keys: bool) -> bool {
    let ok = mount::can_read("V1");
    if !ok && mount_keys {
        eprintln!("skipping: no V1 key in chimera.local.toml");
    }
    ok
}

fn dtx(m: &Mount, path: &str) -> (usize, Dtx) {
    let bytes = m.read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let n = bytes.len();
    (n, Dtx::parse(bytes).unwrap_or_else(|e| panic!("{path}: {e}")))
}

/// Test 46: the HUD map archive's images.
#[test]
fn t46_the_map_images() {
    let Some(m) = install() else { return };
    let dir = "ui_hud_map/mapimage/";
    let all: Vec<String> = m
        .entries()
        .into_iter()
        .map(|(e, _)| e.path)
        .filter(|p| p.starts_with(dir))
        .collect();
    assert_eq!(all.len(), 78, "78 map images");
    for name in ["map_showdown", "map_junkflea2", "map_warhead"] {
        let (bytes, d) = dtx(&m, &format!("{dir}{name}.dtx"));
        assert_eq!(
            (bytes, d.width, d.height, d.mip_count, d.format),
            (1_048_740, 1024, 1024, 1, Format::Dxt5),
            "{name}"
        );
    }
    let (bytes, d) = dtx(&m, &format!("{dir}map_cargoship.dtx"));
    assert_eq!(
        (bytes, d.width, d.height, d.format),
        (4_194_468, 1024, 1024, Format::Bgra32)
    );
    let (bytes, d) = dtx(&m, &format!("{dir}map_deathdesert.dtx"));
    assert_eq!(
        (bytes, d.width, d.height, d.format),
        (524_452, 1024, 1024, Format::Dxt1)
    );
}

/// Tests 48 to 51: the corners from the mission files.
fn corners_of(m: &Mount, file: &str) -> (MapImage, Document) {
    let bytes = m.read(&format!("gms/{file}")).unwrap_or_else(|e| panic!("{file}: {e}"));
    let doc = Document::parse(&bytes).unwrap();
    (MapImage::of_document(&doc, 0), doc)
}

fn check(img: &MapImage, name: &str, lt: (f32, f32), rb: (f32, f32)) {
    assert_eq!(img.size, (1024, 1024));
    assert!(img.name.to_lowercase().ends_with(name), "{}", img.name);
    let c = img.corners.expect("corners");
    for (got, want) in [(c.lt.0, lt.0), (c.lt.1, lt.1), (c.rb.0, rb.0), (c.rb.1, rb.1)] {
        assert!((got - want).abs() < 0.01, "{c:?} != {lt:?} {rb:?}");
    }
}

#[test]
fn t48_to_51_the_corners_of_four_maps() {
    let Some(m) = install() else { return };
    if !v1(true) {
        return;
    }
    let (img, _) = corners_of(&m, "showdown_t.gms");
    check(&img, "map_showdown.dtx", (-2646.112, 2902.112), (2138.785, -1882.785));
    let (img, _) = corners_of(&m, "junkflea2_t.gms");
    check(&img, "map_junkflea2.dtx", (-3414.446, 3414.446), (3421.121, -3421.121));
    let (img, _) = corners_of(&m, "warhead_t.gms");
    check(&img, "map_warhead.dtx", (-3073.335, 4097.335), (5129.346, -4105.346));
    let (img, _) = corners_of(&m, "junkflea_t.gms");
    check(&img, "map_junkflea.dtx", (-2091.557, 2091.557), (3376.897, -3376.897));
}

/// The server's path: the world and the mode find the mission file.
#[test]
fn the_mission_file_of_a_world_and_mode() {
    let Some(m) = install() else { return };
    if !v1(true) {
        return;
    }
    let (path, _) = mission::find(&m, "warehouse", "TDM").expect("Warehouse's team deathmatch file");
    assert_eq!(path, "gms/showdown_t.gms");
    let img = mission::image_of(&m, "junk_flea2", "TDM").expect("Junk Flea 2");
    assert!(img.corners.is_some());
}

