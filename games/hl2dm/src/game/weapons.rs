//! HL2:DM's weapons as data (games/hl2dm/data/weapons, docs/weapon-data.md):
//! M1's one test weapon, the pistol with the spec's plain numbers
//! (`PISTOL`), which every player starts with. The guns, their hooks and
//! the spawn loadout come with M4 (specs/hl2dm/weapons.md).

use bevy::prelude::*;
use chimera_sim::weapon::{StartingWeapons, data::WeaponDataAppExt};

/// The test weapon.
pub const PISTOL: &str = "hl2dm:weapon_pistol";

/// The weapon documents, built into the game.
pub const DATA: &[(&str, &str)] = &[
    ("_base.toml", include_str!("../../data/weapons/_base.toml")),
    ("pistol.toml", include_str!("../../data/weapons/pistol.toml")),
];

/// HL2:DM's weapons, and the pistol for every new player.
pub struct Hl2WeaponsPlugin;

impl Plugin for Hl2WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_weapon_data(DATA).expect("HL2:DM's weapon data loads");
        app.insert_resource(StartingWeapons {
            team: Vec::new(),
            all: vec![PISTOL],
        });
    }
}

#[cfg(test)]
mod tests {
    /// Every file in data/weapons is built in.
    #[test]
    fn every_data_file_is_built_in() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weapons");
        for entry in std::fs::read_dir(dir).unwrap() {
            let name = entry.unwrap().file_name().to_string_lossy().to_string();
            assert!(super::DATA.iter().any(|(n, _)| *n == name), "{name} not in DATA");
        }
    }
}
