//! HL2:DM's install mounted from its `hl2mp/gameinfo.txt`
//! (`game::mount`, `source_engine::search_path`; specs/hl2dm/
//! source_deltas.md, Part 2, and its test cases 14-16). Each test passes
//! with a note without an install (Steam, `[games.hl2dm] path`, or
//! `$CHIMERA_HL2DM_ROOT`); those that check the install's own folders
//! skip when the asset store stands in for it.

use std::path::{Path, PathBuf};

use chimera_core::mount::{
    Mount,
    assets::{AssetSettings, Manifest, Store, pack},
    served,
};
use hl2dm::game::{GAME, INFO, STOCK_MAPS, mount};
use source_engine::search_path::Kind;

/// The install (or the store standing in for it), if usable.
fn install() -> Option<PathBuf> {
    let Some(path) = INFO.install() else {
        eprintln!("skipping: no HL2:DM install (Steam, [games.hl2dm] path, or $CHIMERA_HL2DM_ROOT)");
        return None;
    };
    if !served::usable(GAME, &path, "hl2mp") {
        eprintln!("skipping: no HL2:DM install at {}", path.display());
        return None;
    }
    Some(path)
}

/// The install's own folders, for checks of its layers.
fn own_install() -> Option<PathBuf> {
    let path = install()?;
    if !path.join("hl2mp").is_dir() {
        eprintln!("skipping: checks the install's own folders (served from the asset store here)");
        return None;
    }
    Some(path)
}

fn rel(install: &Path, p: &Path) -> String {
    p.strip_prefix(install)
        .unwrap_or(p)
        .to_string_lossy()
        .replace('\\', "/")
}

/// The name of the layer `path` is read from.
fn layer_of(mount: &Mount, install: &Path, path: &str) -> String {
    let (_, layer) = mount
        .layer_of(path)
        .unwrap_or_else(|| panic!("{path}: not in the mount"));
    rel(install, Path::new(&layer.name()))
}

#[test]
fn search_order_is_the_gameinfo_files() {
    let Some(install) = own_install() else { return };
    let path = mount::search_path(&install).unwrap();
    let all: Vec<(String, Kind)> = path.layers.iter().map(|l| (rel(&install, &l.path), l.kind)).collect();
    // Every line the game reads through, in order; `gamebin` left out and
    // `hl2mp` (named twice) once.
    let expected_tail = [
        ("hl2mp/hl2mp_english_dir.vpk", Kind::Vpk),
        ("hl2mp/hl2mp_pak_dir.vpk", Kind::Vpk),
        ("hl2_complete/hl2_complete_textures_dir.vpk", Kind::Vpk),
        ("hl2_complete/hl2_complete_sound_vo_english_dir.vpk", Kind::Vpk),
        ("hl2_complete/hl2_complete_sound_misc_dir.vpk", Kind::Vpk),
        ("hl2_complete/hl2_complete_misc_dir.vpk", Kind::Vpk),
        ("hl2/hl2_textures_dir.vpk", Kind::Vpk),
        ("hl2/hl2_sound_vo_english_dir.vpk", Kind::Vpk),
        ("hl2/hl2_sound_misc_dir.vpk", Kind::Vpk),
        ("hl2/hl2_misc_dir.vpk", Kind::Vpk),
        ("platform/platform_misc_dir.vpk", Kind::Vpk),
        ("hl2mp", Kind::Loose),
        ("hl2_complete", Kind::Loose),
        ("hl2", Kind::Loose),
        ("platform", Kind::Loose),
        ("hl2mp/download", Kind::Loose),
    ];
    let expected: Vec<(String, Kind)> = expected_tail.iter().map(|(p, k)| (p.to_string(), *k)).collect();
    // `hl2mp/custom/*` first: whatever the user put there (nothing stock).
    let custom = all.len() - expected.len();
    assert!(
        all[..custom].iter().all(|(p, _)| p.starts_with("hl2mp/custom/")),
        "{all:#?}"
    );
    assert_eq!(all[custom..], expected[..], "{all:#?}");
    // Every archive before any loose folder (after custom/*).
    let first_loose = all[custom..].iter().position(|(_, k)| *k == Kind::Loose).unwrap();
    assert!(all[custom + first_loose..].iter().all(|(_, k)| *k == Kind::Loose));
    // On the stock install the English and four of the hl2_complete
    // archives are missing and skipped (source_deltas.md's table).
    let mounted: Vec<String> = path.mounted().map(|l| rel(&install, &l.path)).collect();
    eprintln!("search path as mounted:\n  {}", mounted.join("\n  "));
    for missing in [
        "hl2mp/hl2mp_english_dir.vpk",
        "hl2_complete/hl2_complete_textures_dir.vpk",
        "hl2_complete/hl2_complete_sound_vo_english_dir.vpk",
        "hl2_complete/hl2_complete_sound_misc_dir.vpk",
    ] {
        assert!(!mounted.iter().any(|m| m == missing), "{missing} mounted");
    }
    for present in [
        "hl2mp/hl2mp_pak_dir.vpk",
        "hl2/hl2_misc_dir.vpk",
        "hl2",
        "hl2mp/download",
    ] {
        assert!(mounted.iter().any(|m| m == present), "{present} not mounted");
    }
    // The mount: those layers and Chimera's content cache.
    let m = mount::open_install(&install).unwrap();
    assert_eq!(m.layers().count(), mounted.len() + 1);
}

/// hl2mp's own animation files win over hl2's at the same path (H0 test
/// case 14): `male_shared.mdl` from hl2mp's VPK, which includes the
/// multiplayer `models/player/male_anims.mdl`.
#[test]
fn hl2mp_models_win_over_hl2s() {
    let Some(install) = own_install() else { return };
    let m = mount::open_install(&install).unwrap();
    let path = "models/humans/male_shared.mdl";
    assert_eq!(layer_of(&m, &install, path), "hl2mp/hl2mp_pak_dir.vpk");
    let copies = m.read_every(path);
    assert!(
        copies.len() >= 2,
        "{path}: {} copies, expected hl2mp's and hl2's",
        copies.len()
    );
    assert_ne!(copies[0], copies[1], "hl2mp's copy differs from hl2's");
    // Include paths are written with either slash (`models/player\male_anims.mdl`).
    let has = |bytes: &[u8], s: &str| {
        let slash = |b: &u8| if *b == b'\\' { b'/' } else { b.to_ascii_lowercase() };
        bytes.windows(s.len()).any(|w| w.iter().map(slash).eq(s.bytes()))
    };
    assert!(
        has(&copies[0], "models/player/male_anims.mdl"),
        "hl2mp's includes the multiplayer animations"
    );
    assert!(!has(&copies[1], "models/player/male_anims.mdl"), "hl2's doesn't");
    assert!(m.contains("models/player/male_anims.mdl"));
}

/// VPKs before loose folders (H0 test cases 15 and 16): the weapon
/// scripts from hl2mp's VPK over hl2/scripts' loose single-player ones;
/// the sound manifest, in no VPK, from hl2/ loose.
#[test]
fn archives_win_over_loose_files() {
    let Some(install) = own_install() else { return };
    let m = mount::open_install(&install).unwrap();
    assert_eq!(
        layer_of(&m, &install, "scripts/weapon_pistol.txt"),
        "hl2mp/hl2mp_pak_dir.vpk"
    );
    assert!(
        m.read_every("scripts/weapon_pistol.txt").len() >= 2,
        "hl2/scripts' copy is behind it"
    );
    assert_eq!(layer_of(&m, &install, "scripts/game_sounds_manifest.txt"), "hl2");
}

#[test]
fn every_stock_map_is_present() {
    let Some(install) = install() else { return };
    let m = (INFO.open)(&install).unwrap();
    let names = (INFO.map_names)(&m);
    for map in STOCK_MAPS {
        assert!(names.iter().any(|n| n == map), "{map} missing from {names:?}");
        let bsp = m.read(&format!("maps/{map}.bsp")).unwrap();
        assert_eq!(&bsp[0..4], b"VBSP", "{map}");
        assert_eq!(
            i32::from_le_bytes(bsp[4..8].try_into().unwrap()),
            20,
            "{map}: BSP version"
        );
    }
    // The cycle is the seven dm_ maps; halls3 isn't in it.
    let cycle = (INFO.map_cycle)(&m);
    assert_eq!(cycle.len(), 7, "{cycle:?}");
    assert!(cycle.iter().all(|c| c.starts_with("dm_")));
}

#[test]
fn lookups_ignore_case_and_slashes() {
    let Some(install) = install() else { return };
    let m = (INFO.open)(&install).unwrap();
    let bsp = m.read("maps/dm_lockdown.bsp").unwrap();
    assert_eq!(m.read("MAPS\\DM_Lockdown.BSP").unwrap().len(), bsp.len());
    let mdl = m.read("models/humans/male_shared.mdl").unwrap();
    assert_eq!(m.read("Models\\Humans\\MALE_SHARED.mdl").unwrap(), mdl);
    assert_eq!(
        m.read("Scripts/Weapon_Pistol.TXT").unwrap(),
        m.read("scripts/weapon_pistol.txt").unwrap()
    );
}

/// The asset store standing in for a missing install (`mount::served`):
/// files packed from the install into a scratch store, the mount opened
/// with the install off at the store's stand-in path, the same bytes.
#[test]
fn the_store_stands_in_for_the_install() {
    let Some(install) = own_install() else { return };
    assert!(served::usable(GAME, &install, "hl2mp"));
    let installed = mount::open_install(&install).unwrap();
    let paths: Vec<String> = [
        "scripts/weapon_pistol.txt",
        "models/humans/male_shared.mdl",
        "scripts/game_sounds_manifest.txt",
    ]
    .iter()
    .map(|p| p.to_string())
    .collect();
    let dir = std::env::temp_dir().join(format!("chimera_hl2dm_served_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(&dir);
    let mut manifest = Manifest::new(GAME, "test", Some(320));
    let report = pack(&installed, &paths, &store, &mut manifest, None, 2).unwrap();
    assert!(report.missing.is_empty(), "{:?}", report.missing);
    manifest.save(&store.manifest_path(GAME, "test")).unwrap();
    let settings = AssetSettings {
        store: Some(store.clone()),
        use_install: false,
        builds: Default::default(),
        remote: None,
        lock: None,
    };
    let stand_in = settings.stand_in_install(GAME).unwrap();
    let from_store = served::open_with(&settings, GAME, &stand_in, mount::open_install).unwrap();
    for p in &paths {
        assert_eq!(from_store.read(p).unwrap(), installed.read(p).unwrap(), "{p}");
    }
    assert!(from_store.read("maps/dm_lockdown.bsp").is_err(), "only what was packed");
    // With nothing packed for the game, there's nothing to stand in.
    let empty = AssetSettings {
        store: Some(Store::new(dir.join("empty"))),
        ..settings
    };
    assert!(served::open_with(&empty, GAME, &stand_in, mount::open_install).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}
