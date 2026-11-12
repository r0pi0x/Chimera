//! The map files the party's client reads for a server's map check
//! (`party_kit::client::map_file_bytes`, `net::maps::MapFiles`): a CS:S
//! map's and an HL2:DM map's from their installs (or the asset store
//! standing in for them), the same file the server hashes; Combat Arms'
//! maps and names without a game have none (loaded by id). Without the
//! HL2:DM read an HL2:DM map, offered with its hash and never sent, was
//! "missing" on every client, which asked the server for it and was
//! disconnected (rehearsal, 2026-11-12). Each game's part skips without
//! its install.

use chimera_core::{game_registry::GameInfo, mount::served};
use party_kit::client::map_file_bytes;

/// The game's install (or the store standing in for it) has its folder.
fn usable(info: &GameInfo, marker: &str) -> bool {
    info.install().is_some_and(|p| served::usable(info.id, &p, marker))
}

#[test]
fn cs_source_and_hl2dm_maps_are_read_combat_arms_maps_are_not() {
    assert_eq!(map_file_bytes("combat_arms:warehouse"), None);
    assert_eq!(map_file_bytes("de_dust2"), None, "a name without its game");
    let cases = [
        ("cs_source:de_dust2", &cs_source::game::INFO, "cstrike"),
        ("hl2dm:dm_lockdown", &hl2dm::game::INFO, "hl2mp"),
    ];
    for (map, info, marker) in cases {
        if !usable(info, marker) {
            eprintln!("skipping {map}: no {} install", info.id);
            continue;
        }
        let bytes = map_file_bytes(map).unwrap_or_else(|| panic!("{map}: no file read"));
        assert!(bytes.starts_with(b"VBSP"), "{map}: not a BSP file");
    }
}
