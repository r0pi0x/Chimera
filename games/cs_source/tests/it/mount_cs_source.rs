//! CS:S mount against a real install. Skipped (passes with a note) when
//! chimera.local.toml has no usable `[games.cs_source]` path.

use std::path::PathBuf;

use ::cs_source::game::{self as cs_source, vpk::Vpk};
use chimera_core::mount::{FileSource, config::LocalConfig};

fn install() -> Option<PathBuf> {
    let path = LocalConfig::load().ok()?.game_path_in(cs_source::GAME, cs_source::INFO.steam.as_ref())?;
    if chimera_core::mount::served::usable("cs_source", &path, "cstrike") {
        Some(path)
    } else {
        eprintln!("skipping: no CS:S install at {}", path.display());
        None
    }
}

/// The install itself, for checks of its own archives and layers (the
/// asset store standing in for it has neither): None, with a note,
/// otherwise.
fn own_install() -> Option<PathBuf> {
    let path = install()?;
    if !path.join("cstrike").is_dir() {
        eprintln!("skipping: checks the install's own archives (served from the asset store here)");
        return None;
    }
    Some(path)
}

#[test]
fn reads_de_dust2_through_the_mount() {
    let Some(install) = install() else { return };
    let mount = cs_source::mount::open(&install).unwrap();
    let bsp = mount.read("maps/de_dust2.bsp").unwrap();
    assert_eq!(&bsp[0..4], b"VBSP");
    assert_eq!(i32::from_le_bytes(bsp[4..8].try_into().unwrap()), 20, "BSP version");
    // Case-insensitive, either slash, like the game.
    assert_eq!(mount.read("MAPS\\DE_DUST2.BSP").unwrap().len(), bsp.len());
}

#[test]
fn vpk_contents_match_their_crcs() {
    let Some(install) = own_install() else { return };
    let vpk = Vpk::open(install.join("cstrike/cstrike_pak_dir.vpk")).unwrap();
    assert!(vpk.len() > 10_000, "only {} entries", vpk.len());
    let mut entries = vpk.entries();
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    // A spread sample across archives, including preload and in-dir data.
    for e in entries.iter().step_by(entries.len() / 300) {
        let data = vpk.read(&e.path).unwrap().unwrap();
        assert_eq!(data.len() as u64, e.size, "{}: size", e.path);
        assert_eq!(
            crc32fast::hash(&data),
            vpk.crc(&e.path).unwrap(),
            "{}: CRC mismatch",
            e.path
        );
    }
}

#[test]
fn mount_layers_shared_hl2_content() {
    let Some(install) = own_install() else { return };
    let mount = cs_source::mount::open(&install).unwrap();
    let entries = mount.entries();
    let from_hl2 = entries
        .iter()
        .filter(|(e, layer)| *layer >= 3 && e.path.ends_with(".vtf"))
        .count();
    assert!(
        from_hl2 > 1000,
        "expected HL2 textures behind CS:S content, got {from_hl2}"
    );
}

#[test]
fn lists_maps_from_the_bsp_layout() {
    struct Fake;
    impl FileSource for Fake {
        fn name(&self) -> String {
            "fake".into()
        }
        fn entries(&self) -> Vec<chimera_core::mount::Entry> {
            ["maps/de_dust2.bsp", "maps/cs_office.bsp", "maps/de_dust2.nav", "maps/sub/x.bsp", "worlds/a.dat"]
                .iter()
                .map(|p| chimera_core::mount::Entry { path: p.to_string(), size: 1 })
                .collect()
        }
        fn read(&self, _: &str) -> Option<std::io::Result<Vec<u8>>> {
            None
        }
    }
    let mut mount = chimera_core::mount::Mount::default();
    mount.push(Fake);
    assert_eq!((cs_source::INFO.map_names)(&mount), ["cs_office", "de_dust2"]);
    assert_eq!((cs_source::INFO.map_file)("de_dust2").as_deref(), Some("maps/de_dust2.bsp"));
}

/// Every file of CS:S's minigame cue pack (data/cues.toml) is a stock
/// sound of the install.
#[test]
fn the_cue_packs_files_are_in_the_install() {
    let Some(install) = install() else { return };
    let mount = cs_source::mount::open(&install).unwrap();
    let pack = chimera_core::cues::CuePack::parse(cs_source::parts::CUES).unwrap();
    assert_eq!(pack.game, cs_source::GAME);
    for (cue, file) in &pack.sounds {
        assert!(mount.read(file).is_ok(), "{cue}: {file} not in the install");
    }
}
