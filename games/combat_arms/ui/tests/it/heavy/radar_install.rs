//! The install checks of specs/combat_arms/radar_minimap.md (47, 52, 53)
//! that need the front end: the radar's art and icons (sizes and
//! formats), the large-map attribute file's world size and legend lists,
//! and the radar's misspelt flag badge key. (The rest of the radar's
//! install checks are `combat_arms`' own, tests/it/heavy/radar.rs.) Pass
//! with a note without an install; the attribute files need the V1 key.

use chimera_core::mount::{Mount, config::LocalConfig};
use combat_arms::game::{
    GAME, INFO, mount,
    radar::{
        legend,
        look::{self, ARROW, FRAME, GUIDE, MASK, ZOOM_KNOB, ZOOM_TRACK},
    },
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

/// Test 47: the radar art and the icons, with the sizes the spec lists.
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
    for path in combat_arms_ui::hud::radar::art_names()
        .into_iter()
        .filter(|p| p.starts_with(look::ICON_DIR))
    {
        let (_, d) = dtx(&m, &path);
        assert_eq!(d.format, Format::Bgra32, "{path}");
        let want = if path.ends_with("area_death.dtx") {
            (64, 64)
        } else {
            (64, 32)
        };
        assert_eq!((d.width, d.height), want, "{path}");
    }
}

/// Test 52: `[RadarInfo]`'s `DefaultWorldSize`, and the legend lists.
#[test]
fn t52_the_large_map_attribute_file() {
    let Some(m) = install() else { return };
    if !v1(true) {
        return;
    }
    let bytes = m
        .read(combat_arms_ui::install::LARGE_MAP_ATTRIBUTES)
        .expect("the large-map attribute file");
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
#[test]
fn t53_the_radars_misspelt_flag_badge_key() {
    let Some(m) = install() else { return };
    if !v1(true) {
        return;
    }
    let layout = m
        .read(combat_arms_ui::install::LAYOUT_ATTRIBUTES)
        .expect("the layout attribute file");
    let doc = Document::parse(&layout).unwrap();
    let section = doc.section("HUDLargeMap").expect("[HUDLargeMap]");
    let right: Vec<char> = "largemapbravoflagmove".chars().collect();
    let swapped = |k: &str| {
        let a: Vec<char> = k.to_lowercase().chars().collect();
        a.len() == right.len() && a.iter().zip(&right).filter(|(x, y)| x != y).count() == 2
    };
    assert!(section.get("LargeMapBravoFlagMove").is_some(), "the correct key");
    assert!(
        !section.iter().any(|(k, _)| swapped(k)),
        "no misspelt key: the radar's lookup finds nothing"
    );
}

