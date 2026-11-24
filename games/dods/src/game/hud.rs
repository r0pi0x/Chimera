//! DoD:S's HUD pictures from the install (specs/dods/hud.md), read at run
//! time through Source's sheets (`source_engine::hud::sheets`), for its
//! HUD (`dods_ui::hud`) and every app's kill feed:
//!
//! - the kill icons (`Pictures::kill_icons`): `scripts/mod_textures.txt`'s
//!   `d_<weapon>` cells of `sprites/hud/death_icons`, by our weapon id
//!   (`dods:weapon_<weapon>`; hud.md, "Kill icons");
//! - each weapon's pictures (`WeaponPictures`): its script's `TextureData`
//!   `weapon_s` and `weapon_g` (the current weapon, its outline), `ammo`
//!   and `ammo2` (the magazine full and empty) and the clip fields
//!   (`HudClipHeight`, `HudClipBaseHeight`, `HudClipBulletHeight`,
//!   `clip_size`; hud.md, "Ammo");
//! - each class's silhouette (`ClassPictures`): its class script's
//!   `healthimage` and `healthimagebg` (`materials/vgui/<name>`), the class
//!   found by its body (`playermodel`, `bodies::CLASSES`).
//!
//! `pictures` gives None while a file the mount names isn't here yet (the
//! asset store before DoD:S's `players` set arrived); callers try again
//! later. Nothing is fetched.

use std::collections::BTreeMap;

use chimera_core::mount::keyvalues::{self, Escapes, Kv};
use source_engine::hud::sheets::{self, Sheets, here};

use crate::{map::hud::UiImage, mount::Mount};

/// The file of the kill icons' and the HUD's shared cells.
pub const TEXTURES: &str = "scripts/mod_textures.txt";

/// How a weapon's magazine picture empties (hud.md, "Ammo"): the
/// script's `HudClipHeight`, `HudClipBaseHeight`, `HudClipBulletHeight`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipFill {
    pub height: f32,
    pub base: f32,
    pub bullet: f32,
}

impl ClipFill {
    /// The share of the full picture shown, from the bottom, with
    /// `rounds` in the magazine (0-1).
    pub fn shown(&self, rounds: u32) -> f32 {
        if self.height <= 0.0 {
            return 1.0;
        }
        ((self.base + rounds as f32 * self.bullet) / self.height).clamp(0.0, 1.0)
    }
}

/// A weapon's HUD pictures and numbers (hud.md, "Current weapon", "Ammo").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponPictures {
    /// `weapon_s`: the current weapon's picture.
    pub weapon: Option<UiImage>,
    /// `weapon_g`: its outline.
    pub glow: Option<UiImage>,
    /// `ammo`: the magazine, full.
    pub ammo: Option<UiImage>,
    /// `ammo2`: the magazine, empty.
    pub ammo_empty: Option<UiImage>,
    /// How the magazine empties (none: the full picture always).
    pub clip: Option<ClipFill>,
    /// The script's `clip_size` (the spare magazines' count divides by it).
    pub clip_size: u32,
}

/// A class's health silhouette and its outline (hud.md, "Health").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClassPictures {
    pub image: Option<UiImage>,
    pub outline: Option<UiImage>,
}

/// Everything `pictures` reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pictures {
    /// Kill icons by weapon id.
    pub kill_icons: BTreeMap<String, UiImage>,
    /// Weapons' pictures by weapon id.
    pub weapons: BTreeMap<String, WeaponPictures>,
    /// Classes' silhouettes by class id (`dods:us_rifleman`).
    pub classes: BTreeMap<String, ClassPictures>,
}

/// A weapon id's script name (`dods:weapon_garand` -> `garand`).
pub fn script_name(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id).trim_start_matches("weapon_")
}

/// The kill icon's name of weapon `id` (`d_garand`).
pub fn kill_icon_name(id: &str) -> String {
    format!("d_{}", script_name(id))
}

/// Every weapon DoD:S has (guns, melee, explosives).
fn weapon_ids() -> impl Iterator<Item = &'static str> {
    super::weapons::GUNS.iter().chain(super::weapons::MELEE).chain(super::weapons::EXPLOSIVES).copied()
}

fn parse(bytes: &[u8]) -> Kv {
    keyvalues::parse(&String::from_utf8_lossy(bytes), Escapes::Off)
}

/// A weapon script's HUD numbers and cells (`WeaponData`'s fields and its
/// `TextureData`): (clip fill, clip size, cells).
pub fn weapon_hud_data(text: &str) -> (Option<ClipFill>, u32, BTreeMap<String, (String, [u32; 4])>) {
    let parsed = keyvalues::parse(text, Escapes::Off);
    let Some(data) = parsed.get("WeaponData") else { return (None, 0, BTreeMap::new()) };
    let num = |k: &str| data.str(k).and_then(|v| v.trim().parse::<f32>().ok());
    let clip = match (num("HudClipHeight"), num("HudClipBaseHeight"), num("HudClipBulletHeight")) {
        (Some(height), Some(base), Some(bullet)) => Some(ClipFill { height, base, bullet }),
        _ => None,
    };
    let cells = data.get("TextureData").map(sheets::block_cells).unwrap_or_default();
    (clip, num("clip_size").unwrap_or(0.0).max(0.0) as u32, cells)
}

/// DoD:S's HUD pictures from `mount` (module doc); None while a file it
/// names isn't here yet.
pub fn pictures(mount: &Mount) -> Option<Pictures> {
    let mut out = Pictures::default();
    let mut sheets = Sheets::default();
    if let Some(text) = here(mount, TEXTURES)? {
        let cells = sheets::icon_cells(&String::from_utf8_lossy(&text));
        for id in weapon_ids() {
            let Some((file, cell)) = cells.get(&kill_icon_name(id)) else { continue };
            if let Some(p) = sheets.cut(mount, file, *cell)? {
                out.kill_icons.insert(id.to_string(), p);
            }
        }
    }
    for id in weapon_ids() {
        let Some(text) = here(mount, &format!("scripts/weapon_{}.txt", script_name(id)))? else { continue };
        let (clip, clip_size, cells) = weapon_hud_data(&String::from_utf8_lossy(&text));
        let mut cut = |name: &str| -> Option<Option<UiImage>> {
            match cells.get(name) {
                Some((file, cell)) => sheets.cut(mount, file, *cell),
                None => Some(None),
            }
        };
        let w = WeaponPictures { weapon: cut("weapon_s")?, glow: cut("weapon_g")?, ammo: cut("ammo")?, ammo_empty: cut("ammo2")?, clip, clip_size };
        out.weapons.insert(id.to_string(), w);
    }
    // The class scripts name their body: each class's is the one naming
    // its body's model.
    let mut scripts = Vec::new();
    for name in CLASS_SCRIPTS {
        if let Some(bytes) = here(mount, &format!("scripts/{name}.txt"))? {
            scripts.extend(parse(&bytes).items().first().map(|(_, v)| v.clone()));
        }
    }
    for class in super::bodies::CLASSES.iter() {
        let Some(script) = scripts.iter().find(|s| s.str("playermodel").is_some_and(|m| m.eq_ignore_ascii_case(class.model))) else {
            continue;
        };
        let picture = |field: &str| -> Option<Option<UiImage>> {
            match script.str(field) {
                Some(name) => sheets::material_picture(mount, &format!("vgui/{name}")),
                None => Some(None),
            }
        };
        out.classes.insert(class.id.to_string(), ClassPictures { image: picture("healthimage")?, outline: picture("healthimagebg")? });
    }
    Some(out)
}

/// The twelve class scripts (`scripts/<name>.txt`; classes.md).
pub const CLASS_SCRIPTS: [&str; 12] = [
    "playerclass_us_garand",
    "playerclass_us_tommy",
    "playerclass_us_bar",
    "playerclass_us_spring",
    "playerclass_us_30cal",
    "playerclass_us_bazooka",
    "playerclass_axis_k98",
    "playerclass_axis_mp40",
    "playerclass_axis_mp44",
    "playerclass_axis_k98s",
    "playerclass_axis_mg42",
    "playerclass_axis_pschreck",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_thompson_magazine_empties_from_its_fields() {
        // hud.md test case 2: the Thompson's fields.
        let c = ClipFill { height: 184.0, base: 2.0, bullet: 6.0 };
        assert_eq!(c.shown(30), 182.0 / 184.0);
        assert_eq!(c.shown(15), 92.0 / 184.0);
        assert_eq!(c.shown(0), 2.0 / 184.0);
        assert_eq!(ClipFill { height: 121.0, base: 6.0, bullet: 17.0 }.shown(7), 1.0);
    }

    #[test]
    fn a_weapon_script_gives_its_cells_and_clip() {
        let text = r#"WeaponData
        {
            "clip_size" "30"
            "HudClipHeight" "184"
            "HudClipBaseHeight" "2"
            "HudClipBulletHeight" "6"
            TextureData
            {
                "weapon_s" { "file" "sprites/hud/dod_weapons01" "x" "0" "y" "0" "width" "350" "height" "122" }
                "ammo" { "file" "sprites/hud/clips_bullets" "x" "476""y" "193""width" "36""height" "184" }
            }
        }"#;
        let (clip, size, cells) = weapon_hud_data(text);
        assert_eq!(clip, Some(ClipFill { height: 184.0, base: 2.0, bullet: 6.0 }));
        assert_eq!(size, 30);
        assert_eq!(cells.get("ammo"), Some(&("sprites/hud/clips_bullets".to_string(), [476, 193, 36, 184])));
        assert_eq!(cells.get("weapon_s").map(|c| c.1), Some([0, 0, 350, 122]));
    }

    #[test]
    fn kill_icon_names_follow_the_scripts() {
        assert_eq!(kill_icon_name("dods:weapon_k98_scoped"), "d_k98_scoped");
        assert_eq!(kill_icon_name("dods:weapon_amerknife"), "d_amerknife");
    }
}
