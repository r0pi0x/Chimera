//! The disc image reader on the user's Halo disc image, against section
//! (b) of specs/halo_ce/disc_image.md. Each test passes with a note when
//! there is no image (`$CHIMERA_HALO_CE_ROOT`, else `[games.halo_ce] path`
//! in chimera.local.toml).

use std::path::PathBuf;

use blam_formats::disc::{DiscImage, fold_cmp, open_install};
use chimera_vfs::FileSource;

const SKIP: &str = "skipping: no Halo CE disc image ($CHIMERA_HALO_CE_ROOT or [games.halo_ce] path)";

/// The local config's `[games.halo_ce]` table: `$CHIMERA_CONFIG` (or
/// `$MASHUP_CONFIG`), else chimera.local.toml at the repository root.
fn config() -> Option<toml::Table> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = [std::env::var_os("CHIMERA_CONFIG"), std::env::var_os("MASHUP_CONFIG")]
        .into_iter()
        .flatten()
        .map(PathBuf::from)
        .chain([root.join("chimera.local.toml"), root.join("mashup.local.toml")])
        .find(|p| p.is_file())?;
    let table: toml::Table = std::fs::read_to_string(&file).ok()?.parse().ok()?;
    table.get("games")?.get("halo_ce")?.as_table().cloned()
}

/// The disc image file: the environment's path, else the config's; a
/// folder holding an image (`*.iso`) counts too.
fn image_path() -> Option<PathBuf> {
    let path = std::env::var_os("CHIMERA_HALO_CE_ROOT")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            let p = config()?.get("path")?.as_str()?.to_string();
            Some(match p.strip_prefix("~/") {
                Some(rest) => {
                    PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?).join(rest)
                }
                None => PathBuf::from(p),
            })
        })?;
    if path.is_file() {
        return Some(path);
    }
    let mut isos: Vec<_> = std::fs::read_dir(&path)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("iso")))
        .collect();
    isos.sort();
    isos.into_iter().next()
}

fn disc() -> Option<DiscImage> {
    let Some(path) = image_path() else {
        eprintln!("{SKIP}");
        return None;
    };
    Some(DiscImage::open(&path).unwrap())
}

#[test]
fn counts_and_base() {
    let Some(d) = disc() else { return };
    assert_eq!(d.base(), 0);
    assert_eq!(d.file_count(), 1166);
    assert_eq!(d.dir_count(), 57);
    let entries = d.entries();
    assert_eq!(entries.iter().map(|e| e.size).sum::<u64>(), 3_726_832_117);
    assert_eq!(d.len(), 3_728_867_328);
    // 2001-10-15 08:21:50 UTC (the spec's byte list for the field omits its
    // lowest zero byte: the u64 is 00 e0 87 77 05 52 c1 01).
    assert_eq!(d.volume_time(), 126_476_077_105_580_000);
}

#[test]
fn top_level() {
    let Some(d) = disc() else { return };
    let top: Vec<_> = d
        .list("")
        .unwrap()
        .iter()
        .map(|e| (e.name.as_str(), e.is_dir))
        .collect();
    assert_eq!(
        top,
        [("bink", true), ("default.xbe", false), ("maps", true), ("XDemos", true)]
    );
    assert_eq!(d.list("bink").unwrap().len(), 5);
    assert_eq!(d.list("maps").unwrap().len(), 24);
    // The 1,136 files of XDemos are counted through its subdirectories.
    assert_eq!(entries_under(&d, "xdemos/"), 1136);
}

#[test]
fn every_directory_is_in_the_trees_order() {
    let Some(d) = disc() else { return };
    let mut dirs = vec![String::new()];
    while let Some(dir) = dirs.pop() {
        let list = d.list(&dir).unwrap();
        for pair in list.windows(2) {
            assert!(
                fold_cmp(pair[0].name.as_bytes(), pair[1].name.as_bytes()).is_lt(),
                "{dir}: {} before {}",
                pair[0].name,
                pair[1].name
            );
        }
        dirs.extend(list.iter().filter(|e| e.is_dir).map(|e| format!("{dir}/{}", e.name)));
    }
}

#[test]
fn default_xbe() {
    let Some(d) = disc() else { return };
    let (offset, size) = d.extent("default.xbe").unwrap();
    assert_eq!((offset, size), (265 * 2048, 2_535_424));
    let bytes = d.read_file("default.xbe").unwrap();
    assert_eq!(bytes.len(), 2_535_424);
    assert_eq!(&bytes[..4], [0x58, 0x42, 0x45, 0x48]);
}

#[test]
fn bloodgulch() {
    let Some(d) = disc() else { return };
    let (offset, size) = d.extent("maps/bloodgulch.map").unwrap();
    assert_eq!((offset, size), (835_909 * 2048, 22_571_008));
    let bytes = d.read_file("maps/bloodgulch.map").unwrap();
    assert_eq!(bytes.len(), 22_571_008);
    assert_eq!(
        &bytes[..16],
        [0x64, 0x61, 0x65, 0x68, 5, 0, 0, 0, 0, 0x68, 0xa4, 2, 0x31, 6, 0, 0]
    );
    assert_eq!(d.extent("MAPS/BLOODGULCH.MAP"), d.extent("maps/bloodgulch.map"));
    assert_eq!(d.read_file("MAPS\\BloodGulch.Map").unwrap(), bytes);
}

#[test]
fn a_file_deep_in_the_tree() {
    let Some(d) = disc() else { return };
    let path = "XDemos/MunchTrial/Layout/AI/Mechanics/doautosave.txt";
    let bytes = d.read_file(path).unwrap();
    assert_eq!(bytes.len(), 53);
    assert_eq!(&bytes[..16], b"script doautosav");
    assert!(d.read_file(&path.to_uppercase()).is_ok());
    assert!(d.contains("xdemos/munchtrial/effects/health/maps/clouds_dry_128_nobands.bmp"));
}

#[test]
fn the_largest_file() {
    let Some(d) = disc() else { return };
    let biggest = d.entries().into_iter().max_by_key(|e| e.size).unwrap();
    assert_eq!((biggest.path.as_str(), biggest.size), ("bink/credits.bik", 200_614_260));
}

#[test]
fn twenty_four_maps() {
    let Some(d) = disc() else { return };
    let maps = d.files_in("maps");
    assert_eq!(maps.len(), 24);
    assert!(maps.iter().all(|m| m.ends_with(".map")));
    assert!(maps.contains(&"maps/ui.map".to_string()));
}

#[test]
fn opens_as_an_install_and_has_a_fingerprint() {
    let Some(path) = image_path() else {
        eprintln!("{SKIP}");
        return;
    };
    let mount = open_install(&path).unwrap();
    assert_eq!(mount.read("default.xbe").unwrap().len(), 2_535_424);
    let (_, layer) = mount.layer_of("maps/ui.map").unwrap();
    let fp = layer.fingerprint("maps/ui.map").unwrap();
    assert!(fp.starts_with("xiso "), "{fp}");
}

fn entries_under(d: &DiscImage, prefix: &str) -> usize {
    d.entries().iter().filter(|e| e.path.starts_with(prefix)).count()
}
