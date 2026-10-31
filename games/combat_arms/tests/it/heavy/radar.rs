//! The install checks of specs/combat_arms/radar_minimap.md (46 to 52),
//! on the user's Combat Arms install: the map images' sizes and formats,
//! the radar art, the mission files' corners on Warehouse (Showdown),
//! Junk Flea 2, WarHead and Junk Flea, the large-map attribute file's world
//! size, and the server's whole path from the world and mode to the image
//! data (`mission::find`). Passes with a note without an install; the
//! mission files need the V1 key. Nothing read here is written anywhere.
#![cfg_attr(not(feature = "ui"), allow(unused_imports))]


use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::game::{
    GAME, INFO, mount,
    radar::{
        legend,
        look::{self, ARROW, FRAME, GUIDE, MASK, ZOOM_KNOB, ZOOM_TRACK},
        mission::{self, MapImage},
    },
};
use lithtech_formats::{attributes::Document, dtx::{Dtx, Format}};

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
    let all: Vec<String> = m.entries().into_iter().map(|(e, _)| e.path).filter(|p| p.starts_with(dir)).collect();
    assert_eq!(all.len(), 78, "78 map images");
    for name in ["map_showdown", "map_junkflea2", "map_warhead"] {
        let (bytes, d) = dtx(&m, &format!("{dir}{name}.dtx"));
        assert_eq!((bytes, d.width, d.height, d.mip_count, d.format), (1_048_740, 1024, 1024, 1, Format::Dxt5), "{name}");
    }
    let (bytes, d) = dtx(&m, &format!("{dir}map_cargoship.dtx"));
    assert_eq!((bytes, d.width, d.height, d.format), (4_194_468, 1024, 1024, Format::Bgra32));
    let (bytes, d) = dtx(&m, &format!("{dir}map_deathdesert.dtx"));
    assert_eq!((bytes, d.width, d.height, d.format), (524_452, 1024, 1024, Format::Dxt1));
}

/// Test 47: the radar art and the icons, with the sizes the spec lists.
#[cfg(feature = "ui")]
#[test]
fn t47_the_radar_art_and_icons() {
    let Some(m) = install() else { return };
    for (path, w, h) in [
        (FRAME, 256, 256),
        (MASK, 256, 256),
        (GUIDE, 512, 512),
        (ZOOM_TRACK, 16, 256),
        (ZOOM_KNOB, 16, 16),
        (ARROW, 64, 32),
    ] {
        let (_, d) = dtx(&m, path);
        assert_eq!((d.width, d.height, d.format), (w, h, Format::Bgra32), "{path}");
    }
    // Every icon the radar and the map name: 64 x 32 32-bit, but the
    // death mark (64 x 64).
    for path in combat_arms::ui::hud::radar::art_names().into_iter().filter(|p| p.starts_with(look::ICON_DIR)) {
        let (_, d) = dtx(&m, &path);
        assert_eq!(d.format, Format::Bgra32, "{path}");
        let want = if path.ends_with("area_death.dtx") { (64, 64) } else { (64, 32) };
        assert_eq!((d.width, d.height), want, "{path}");
    }
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

/// Test 52: `[RadarInfo]`'s `DefaultWorldSize`, and the legend lists.
#[cfg(feature = "ui")]
#[test]
fn t52_the_large_map_attribute_file() {
    let Some(m) = install() else { return };
    if !v1(true) {
        return;
    }
    let bytes = m.read(combat_arms::ui::install::LARGE_MAP_ATTRIBUTES).expect("the large-map attribute file");
    let doc = Document::parse(&bytes).unwrap();
    let size = doc.section("RadarInfo").map(|s| s.number("DefaultWorldSize", 0.0));
    assert_eq!(size, Some(combat_arms::game::radar::maths::WORLD_SIZE));
    let lists = legend::parse(&doc);
    assert!(!lists.is_empty(), "icon lists");
    assert_eq!(lists[0].items.len(), 6, "Elimination's six rows");
}

/// Corrections 2026-10-22 (5): the install's `[HUDLargeMap]` section (in
/// the HUD layout attributes) carries the correct Bravo flag badge key and
/// no misspelt one (two letters of `LargeMapBravoFlagMove` swapped), so the
/// radar's Alpha carriers have an empty badge (`look::radar_player`).
#[cfg(feature = "ui")]
#[test]
fn t53_the_radars_misspelt_flag_badge_key() {
    let Some(m) = install() else { return };
    if !v1(true) {
        return;
    }
    let layout = m.read(combat_arms::ui::install::LAYOUT_ATTRIBUTES).expect("the layout attribute file");
    let doc = Document::parse(&layout).unwrap();
    let section = doc.section("HUDLargeMap").expect("[HUDLargeMap]");
    let right: Vec<char> = "largemapbravoflagmove".chars().collect();
    let swapped = |k: &str| {
        let a: Vec<char> = k.to_lowercase().chars().collect();
        a.len() == right.len() && a.iter().zip(&right).filter(|(x, y)| x != y).count() == 2
    };
    assert!(section.get("LargeMapBravoFlagMove").is_some(), "the correct key");
    assert!(!section.iter().any(|(k, _)| swapped(k)), "no misspelt key: the radar's lookup finds nothing");
}
