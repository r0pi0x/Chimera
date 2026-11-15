//! TF2's install mounted from its `tf/gameinfo.txt` (`game::mount`,
//! `source_engine::search_path`; specs/tf2/install.md 1.4, 1.5): the
//! search order (tf's VPKs, hl2's, platform's, then the loose folders,
//! `tf/download` last), tf's files over hl2's at the same path, the stock
//! maps and textures found, and an archive listed but missing on disk
//! skipped file by file. Each test passes with a note without an install
//! (Steam, `[games.tf2] path`, or `$CHIMERA_TF2_ROOT`); those that check
//! the install's own folders skip when the asset store stands in for it.

use std::path::{Path, PathBuf};

use chimera_core::mount::{FileSource, Mount};
use source_engine::search_path::Kind;
use source_formats::vpk::Vpk;
use tf2::game::{M1_MAPS, mount};

use super::maps::install;

/// The install's own folders, for checks of its layers.
fn own_install() -> Option<PathBuf> {
    let path = install()?;
    if !path.join("tf").is_dir() {
        eprintln!("skipping: checks the install's own folders (served from the asset store here)");
        return None;
    }
    Some(path)
}

fn rel(install: &Path, p: &Path) -> String {
    p.strip_prefix(install).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

/// The name of the layer `path` is read from.
fn layer_of(mount: &Mount, install: &Path, path: &str) -> String {
    let (_, layer) = mount.layer_of(path).unwrap_or_else(|| panic!("{path}: not in the mount"));
    rel(install, Path::new(&layer.name()))
}

#[test]
fn search_order_is_the_gameinfo_files() {
    let Some(install) = own_install() else { return };
    let path = mount::search_path(&install).unwrap();
    let all: Vec<(String, Kind)> = path.layers.iter().map(|l| (rel(&install, &l.path), l.kind)).collect();
    eprintln!("search path:\n  {all:?}");
    // Every line the game reads through, in order (install.md 1.4);
    // `gamebin` left out and `tf` (named twice) once.
    let expected_tail = [
        ("tf/tf2_textures_dir.vpk", Kind::Vpk),
        ("tf/tf2_sound_vo_english_dir.vpk", Kind::Vpk),
        ("tf/tf2_sound_misc_dir.vpk", Kind::Vpk),
        ("tf/tf2_misc_dir.vpk", Kind::Vpk),
        ("hl2/hl2_textures_dir.vpk", Kind::Vpk),
        ("hl2/hl2_sound_vo_english_dir.vpk", Kind::Vpk),
        ("hl2/hl2_sound_misc_dir.vpk", Kind::Vpk),
        ("hl2/hl2_misc_dir.vpk", Kind::Vpk),
        ("platform/platform_misc_dir.vpk", Kind::Vpk),
        ("tf", Kind::Loose),
        ("hl2", Kind::Loose),
        ("platform", Kind::Loose),
        ("tf/download", Kind::Loose),
    ];
    let expected: Vec<(String, Kind)> = expected_tail.iter().map(|(p, k)| (p.to_string(), *k)).collect();
    // `tf/custom/*` (and the low-violence VPK, absent) first.
    let custom = all.len() - expected.len();
    assert!(
        all[..custom].iter().all(|(p, _)| p.starts_with("tf/custom/") || p.starts_with("tf/tf2_lv")),
        "{all:#?}"
    );
    assert_eq!(all[custom..], expected[..], "{all:#?}");
    let mounted: Vec<String> = path.mounted().map(|l| rel(&install, &l.path)).collect();
    eprintln!("search path as mounted:\n  {}", mounted.join("\n  "));
    for present in ["tf/tf2_misc_dir.vpk", "hl2/hl2_misc_dir.vpk", "platform/platform_misc_dir.vpk", "tf"] {
        assert!(mounted.iter().any(|m| m == present), "{present} not mounted");
    }
    // The mount: those layers and Chimera's content cache.
    let m = mount::open_install(&install).unwrap();
    assert_eq!(m.layers().count(), mounted.len() + 1);
}

/// A file both tf's and hl2's misc archives hold is read from tf's (tf's
/// VPKs come first, install.md 1.4).
#[test]
fn tf_wins_over_hl2() {
    let Some(install) = own_install() else { return };
    let tf = Vpk::open(install.join("tf/tf2_misc_dir.vpk")).unwrap();
    let hl2 = Vpk::open(install.join("hl2/hl2_misc_dir.vpk")).unwrap();
    let mut both: Vec<String> = hl2.entries().into_iter().map(|e| e.path).filter(|p| tf.contains(p)).collect();
    both.sort();
    assert!(!both.is_empty(), "no file in both archives");
    eprintln!("{} files in both misc archives, e.g. {}", both.len(), both[0]);
    let m = mount::open(&install).unwrap();
    for p in both.iter().take(20) {
        assert_eq!(layer_of(&m, &install, p), "tf/tf2_misc_dir.vpk", "{p}");
    }
}

/// The stock maps are loose in `tf/maps/`; the materials in tf2_misc;
/// on the client's install the textures in tf2_textures.
#[test]
fn the_maps_and_their_materials_are_found() {
    let Some(install) = own_install() else { return };
    let m = mount::open(&install).unwrap();
    for name in M1_MAPS {
        assert_eq!(layer_of(&m, &install, &format!("maps/{name}.bsp")), "tf", "{name}");
    }
    let names = (tf2::game::INFO.map_names)(&m);
    assert!(names.len() > 200, "{} maps", names.len());
    assert!(M1_MAPS.iter().all(|n| names.iter().any(|m| m == n)));
    let vtf = "materials/skybox/sky_well_01up.vtf";
    if mount::full_content(&install) {
        assert_eq!(layer_of(&m, &install, vtf), "tf/tf2_textures_dir.vpk");
    } else {
        assert!(!m.contains(vtf), "the server's layout has no stock textures");
    }
    assert_eq!(layer_of(&m, &install, "materials/skybox/sky_well_01up.vmt"), "tf/tf2_misc_dir.vpk");
}

/// An archive a `_dir` file lists but that is missing on disk (the
/// dedicated server's textures and sounds) leaves its files out, file by
/// file: the directory file copied without its archives lists none of
/// the archives' files, only what it holds itself.
#[test]
fn a_missing_archive_is_skipped_file_by_file() {
    let Some(install) = own_install() else { return };
    let dir = std::env::temp_dir().join(format!("tf2-missing-archives-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // platform's misc directory alone (its one archive left behind), then
    // with its archive.
    let source = install.join("platform/platform_misc_dir.vpk");
    let copy = dir.join("platform_misc_dir.vpk");
    std::fs::copy(&source, &copy).unwrap();
    let whole = Vpk::open(&source).unwrap();
    let alone = Vpk::open(&copy).unwrap();
    eprintln!("{} files; {} without the archive ({} skipped)", whole.len(), alone.len(), alone.skipped());
    assert!(alone.skipped() > 0 && alone.len() + alone.skipped() == whole.len() + whole.skipped());
    for e in alone.entries() {
        assert!(alone.read(&e.path).unwrap().is_ok(), "{} unreadable", e.path);
    }
    let _ = std::fs::remove_dir_all(&dir);
    // The server's layout itself, if one is named: its texture archives
    // are missing, so none of their files are listed.
    if let Some(server) = std::env::var_os("CHIMERA_TF2_SERVER_ROOT").map(PathBuf::from) {
        let textures = Vpk::open(server.join("tf/tf2_textures_dir.vpk")).unwrap();
        eprintln!("server's tf2_textures: {} listed, {} skipped", textures.len(), textures.skipped());
        assert!(textures.skipped() > 20_000, "{}", textures.skipped());
        let m = mount::open_install(&server).unwrap();
        assert!(m.read("materials/skybox/sky_well_01up.vmt").is_ok());
        assert!(!m.contains("materials/skybox/sky_well_01up.vtf"));
    }
}
