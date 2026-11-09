//! HL2:DM's guns' kill icons for every app's kill feed (the shell's
//! `client::weapon_icons::WeaponIcons`, specs/hl2dm/hud.md 10): the
//! `death_<weapon>` glyphs of `scripts/mod_textures.txt` in the HL2MP font.
//! Registered at start from the install, not from the HUD's look: a gun game
//! kills with these guns on any game's map, under any game's HUD (without
//! them the feed writes the gun's name). A gun with no icon in the file
//! (the gravity gun, in the original too) keeps its name there. The SMG1's
//! grenade and the AR2's ball carry their gun's id: their kills (a blast, a
//! crush) take their own glyphs (`kill_icon_of`, `WeaponIcons::register_kill`).

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
use source_formats::keyvalues::{self, Kv};

use chimera_core::core::DamageKind;

use crate::game::{
    GAME, INFO, mount,
    rules::deaths::{kill_notice_name, weapon_notice_name},
};

/// The file of the death notices' glyphs.
const TEXTURES: &str = "scripts/mod_textures.txt";
/// Their font's file (`HL2MP`).
const FONT: &str = "resource/hl2mp.ttf";
/// The font's height on a 480-line screen (hud.md's `HL2MPTypeDeath`).
const TALL: f32 = 32.0;

/// The `death_<weapon>` glyphs of a `mod_textures.txt`, by notice name.
pub fn death_glyphs(text: &str) -> BTreeMap<String, char> {
    fn find(kv: &Kv) -> Option<&Kv> {
        kv.get("TextureData").or_else(|| kv.items().iter().find_map(|(_, v)| find(v)))
    }
    let parsed = keyvalues::parse_res(text);
    let Some(data) = find(&parsed) else { return BTreeMap::new() };
    data.items()
        .iter()
        .filter_map(|(name, v)| {
            let weapon = name.to_lowercase().strip_prefix("death_")?.to_string();
            Some((weapon, v.str("character")?.chars().next()?))
        })
        .collect()
}

/// The icon of weapon `id` among `glyphs`, in `font`.
pub fn icon_of(id: &str, glyphs: &BTreeMap<String, char>, font: &Handle<Font>) -> Option<WeaponIcon> {
    let ch = *glyphs.get(&weapon_notice_name(id))?;
    Some(WeaponIcon::Glyph { font: font.clone(), ch, em: TALL * 0.5 })
}

/// The icon of a kill by weapon `id` dealt as `kind` when it isn't the
/// gun's own (the SMG1's grenade, the AR2's ball: `kill_notice_name`).
pub fn kill_icon_of(id: &str, kind: DamageKind, glyphs: &BTreeMap<String, char>, font: &Handle<Font>) -> Option<WeaponIcon> {
    let name = kill_notice_name(id, kind);
    if name == weapon_notice_name(id) {
        return None;
    }
    let ch = *glyphs.get(&name)?;
    Some(WeaponIcon::Glyph { font: font.clone(), ch, em: TALL * 0.5 })
}

/// Reads the glyphs and the font (none without an install: the feed writes
/// the guns' names).
pub(super) fn register(mut icons: ResMut<WeaponIcons>, fonts: Option<ResMut<Assets<Font>>>) {
    let Some(mut fonts) = fonts else { return };
    let Some(mount) = INFO.install().and_then(|i| mount::open(&i).ok()) else { return };
    let (Ok(text), Ok(file)) = (mount.read(TEXTURES), mount.read(FONT)) else {
        warn!("HL2:DM: no kill icons ({TEXTURES} or {FONT} doesn't read)");
        return;
    };
    let glyphs = death_glyphs(&String::from_utf8_lossy(&text));
    let font = fonts.add(Font::from_bytes(file));
    let (kill_glyphs, kill_font) = (glyphs.clone(), font.clone());
    icons.register(GAME, move |id| icon_of(id, &glyphs, &font));
    icons.register_kill(GAME, move |id, kind| kill_icon_of(id, kind, &kill_glyphs, &kill_font));
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The characters are hud.md 10's, the file's shape ours.
    const FILE: &str = r#""sprites/640_hud"
    {
        TextureData
        {
            "death_crowbar" { "font" "HL2MPTypeDeath" "character" "6" }
            "death_rpg_missile" { "font" "HL2MPTypeDeath" "character" "3" }
            "death_grenade_frag" { "font" "HL2MPTypeDeath" "character" "4" }
            "death_crossbow_bolt" { "font" "HL2MPTypeDeath" "character" "1" }
            "death_smg1" { "font" "HL2MPTypeDeath" "character" "/" }
            "death_smg1_grenade" { "font" "HL2MPTypeDeath" "character" "7" }
            "death_combine_ball" { "font" "HL2MPTypeDeath" "character" "8" }
            "d_skull" { "file" "sprites/640hud1" "x" "192" "y" "224" "width" "32" "height" "16" }
        }
    }"#;

    /// Each gun finds its icon by its notice's name: the launchers' and
    /// the grenade's are their projectiles' (a weapon id's own short name
    /// has no icon); the gravity gun has none in the file.
    #[test]
    fn every_gun_finds_its_glyph() {
        let g = death_glyphs(FILE);
        assert_eq!(g.get("crowbar"), Some(&'6'));
        assert!(!g.contains_key("skull"), "a sprite is no glyph: {g:?}");
        let font = Handle::<Font>::default();
        let ch = |id| match icon_of(id, &g, &font) {
            Some(WeaponIcon::Glyph { ch, .. }) => Some(ch),
            _ => None,
        };
        assert_eq!(ch("hl2dm:weapon_crowbar"), Some('6'));
        assert_eq!(ch("hl2dm:weapon_rpg"), Some('3'));
        assert_eq!(ch("hl2dm:weapon_frag"), Some('4'));
        assert_eq!(ch("hl2dm:weapon_crossbow"), Some('1'));
        assert_eq!(ch("hl2dm:weapon_smg1"), Some('/'));
        assert_eq!(ch("hl2dm:weapon_physcannon"), None);
        // The launchers' kills: their projectiles' glyphs; their guns'
        // own kills go on to the gun's.
        let kill = |id, kind| match kill_icon_of(id, kind, &g, &font) {
            Some(WeaponIcon::Glyph { ch, .. }) => Some(ch),
            _ => None,
        };
        assert_eq!(kill("hl2dm:weapon_smg1", DamageKind::Blast), Some('7'));
        assert_eq!(kill("hl2dm:weapon_ar2", DamageKind::Crush), Some('8'));
        assert_eq!(kill("hl2dm:weapon_smg1", DamageKind::Bullet), None);
        assert_eq!(kill("hl2dm:weapon_rpg", DamageKind::Blast), None);
    }
}
