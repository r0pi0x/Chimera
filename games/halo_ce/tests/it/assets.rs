//! Halo and the asset store, with no disc: the install's build read from
//! a map's header (`mount::build`, on a folder of synthetic map headers),
//! and what a match on a Halo map offers a client without the disc
//! (`chimera_net::net::assets::offer`: Halo's `base`, `players` and
//! `common`, and the map's own `map/<name>`; `game::sets`).

use std::{fs, path::Path};

use bevy::prelude::*;
use blam_formats::cache::{FOOT_MAGIC, HEAD_MAGIC, HEADER_SIZE, XBOX_CACHE_VERSION};
use chimera_net::net::{
    assets::{self, AssetHost},
    manifest::{GameNeed, MatchManifest},
};
use halo_ce::game::{GAME, mount, sets};

/// A map file's header (cache_file.md, "The map file on disc") naming
/// `name` and `build`.
fn header(name: &str, build: &str) -> Vec<u8> {
    let mut h = vec![0u8; HEADER_SIZE];
    h[0..4].copy_from_slice(&HEAD_MAGIC.to_le_bytes());
    h[4..8].copy_from_slice(&XBOX_CACHE_VERSION.to_le_bytes());
    h[8..12].copy_from_slice(&0x900i32.to_le_bytes());
    h[0x10..0x14].copy_from_slice(&0x800i32.to_le_bytes());
    h[0x14..0x18].copy_from_slice(&0x100i32.to_le_bytes());
    h[0x20..0x20 + name.len()].copy_from_slice(name.as_bytes());
    h[0x40..0x40 + build.len()].copy_from_slice(build.as_bytes());
    h[0x60..0x62].copy_from_slice(&1i16.to_le_bytes());
    h[0x7FC..].copy_from_slice(&FOOT_MAGIC.to_le_bytes());
    h
}

fn write(dir: &Path, name: &str, build: &str) {
    fs::create_dir_all(dir.join("maps")).unwrap();
    fs::write(dir.join(format!("maps/{name}.map")), header(name, build)).unwrap();
}

#[test]
fn the_build_is_the_one_its_maps_name() {
    let dir = std::env::temp_dir().join(format!("chimera_halo_build_{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    // No map: no build (the packer then asks for --build).
    fs::create_dir_all(&dir).unwrap();
    assert!(mount::build(&dir).is_err());
    // A multiplayer map alone names it.
    write(&dir, "wizard", "01.10.12.2276");
    assert_eq!(mount::build(&dir).unwrap(), "01.10.12.2276");
    // The menu's map first (every disc has it).
    write(&dir, "ui", "01.01.14.2342");
    assert_eq!(mount::build(&dir).unwrap(), "01.01.14.2342");
    // A file that isn't a map is refused, not read as a build.
    fs::write(dir.join("maps/ui.map"), vec![0u8; HEADER_SIZE]).unwrap();
    assert!(mount::build(&dir).is_err());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn a_match_on_a_halo_map_offers_its_set_and_base() {
    let mut world = World::new();
    world.insert_resource(AssetHost::new("https://assets.invalid:1", "ab".repeat(32)));
    let mut m = MatchManifest {
        map: format!("{GAME}:hangemhigh"),
        games: vec![GameNeed { game: GAME.into(), required: true }],
        ..default()
    };
    assets::offer(&mut world, &mut m);
    let offer = m.assets.expect("an offer");
    let sets: Vec<String> = offer.now.iter().filter(|g| g.game == GAME).flat_map(|g| g.sets.clone()).collect();
    assert_eq!(sets, ["base", "players", "common", "map/hangemhigh"]);
    // `base` is the donor map (`game::sets`).
    assert_eq!(sets::DONOR_MAP, "bloodgulch");
}
