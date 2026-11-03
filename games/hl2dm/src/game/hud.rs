//! HL2:DM's HUD look from the install (specs/hl2dm/hud.md, H8), as the
//! neutral `map::hud::GameHud` in the map's kit (`maps::HL2DM`'s `kit`
//! hook), read at run time: the shared reading is
//! `source_engine::hud::look` (lift S7: colours with the first of a name
//! listed twice winning, the scheme's fonts and font files by family and
//! by file, the layout as the Windows PC reads it, the texture lists),
//! with HL2:DM's own on top: its weapon scripts' icons (`weapon_<class>/
//! <icon>` in `icons` and `sprites`: glyphs of the HUD fonts or sprite
//! rectangles), the pictures the HUD draws (`white_additive`, `zoom`),
//! the localised strings (`resource/valve_english.txt`, `hl2_english.txt`,
//! `hl2mp_english.txt`, as `GameMenus::strings`) and the chat box's
//! place (`resource/ui/basechat.res`, Half-Life 2's loose file; the
//! layout's own `HudChat` is an unused 4 by 4).
//!
//! `animations`: `scripts/hudanimations.txt` (kept in the strings under
//! `ANIMATIONS`) as `source_engine::hud::anim` reads it. Nothing here is read from anywhere but the install; a map
//! without one has no HUD look and the shell's draws.

use source_engine::{
    content::material::MaterialLoader,
    hud::{
        anim::Script,
        look::{self, LookOptions},
    },
    weapon_script::{ScriptIcon, WeaponScript},
};
use source_formats::{keyvalues, localization};

use crate::map::hud::{GameHud, GameMenus, HudCoord, HudPanel, HudSprite};

/// The base HUD's icons (`scripts/hud_textures.txt`) HL2:DM draws: the
/// default crosshair and the quick info's centre, the battery and kit in
/// the pickup history (no vial icon exists), voice chat's.
pub const BASE_ICONS: &[&str] = &["crosshair_default", "crosshair", "item_battery", "item_healthkit", "voice_self", "voice_player"];

/// Weapon scripts whose icons the HUD takes (the classes of HL2:DM's
/// weapons, M5's and M6's too).
pub const WEAPON_CLASSES: &[&str] = &[
    "weapon_crowbar",
    "weapon_stunstick",
    "weapon_pistol",
    "weapon_357",
    "weapon_smg1",
    "weapon_ar2",
    "weapon_shotgun",
    "weapon_crossbow",
    "weapon_rpg",
    "weapon_frag",
    "weapon_slam",
    "weapon_physcannon",
];

/// Whole pictures the HUD draws, by our name: their materials.
/// (`white_additive`'s material names no `$basetexture`: Source draws it
/// with the white 1 x 1 texture, `vgui/white`.)
const PICTURES: &[(&str, &str, &str)] =
    &[("white_additive", "vgui/white_additive", "vgui/white"), ("zoom", "vgui/zoom", "vgui/zoom")];

/// The localization files, later ones under earlier ones.
const STRINGS: &[&str] = &["resource/hl2mp_english.txt", "resource/hl2_english.txt", "resource/valve_english.txt"];

/// The key of a weapon's icon in `GameHud::icons` and `sprites`.
pub fn icon_key(class: &str, icon: &str) -> String {
    format!("{}/{icon}", class.to_lowercase())
}

/// The HUD look from the install (None when its files are missing).
pub fn load(materials: &mut MaterialLoader) -> Option<GameHud> {
    let mut hud = look::load(
        materials,
        &LookOptions { base_icons: BASE_ICONS, layout: keyvalues::parse_pc, font_files: true, first_wins: true },
    )?;
    for class in WEAPON_CLASSES {
        let Some(text) = materials.read(&format!("scripts/{class}.txt")) else { continue };
        let Some(script) = WeaponScript::parse(&String::from_utf8_lossy(&text)) else { continue };
        for (name, icon) in &script.icons {
            let key = icon_key(class, name);
            match icon {
                ScriptIcon::Glyph { font, character } => {
                    hud.icons.insert(key, (font.clone(), *character));
                }
                ScriptIcon::Sprite { file, rect } => {
                    if let Some(texture) = materials.resolve(file).texture {
                        hud.sprites.insert(key, HudSprite { texture, rect: *rect });
                    }
                }
            }
        }
    }
    for (name, material, fallback) in PICTURES {
        if materials.read(&format!("materials/{material}.vmt")).is_none() {
            continue;
        }
        let texture = materials.resolve(material).texture.or_else(|| materials.texture(fallback, true));
        if let Some(texture) = texture {
            let t = &materials.textures[texture];
            hud.sprites.insert((*name).to_string(), HudSprite { texture, rect: [0.0, 0.0, t.width as f32, t.height as f32] });
        }
    }
    // The chat box (Half-Life 2's `basechat.res`): its frame.
    if let Some(text) = materials.read("resource/ui/basechat.res") {
        let kv = keyvalues::parse_pc(&String::from_utf8_lossy(&text));
        let root = kv.items().first().map(|(_, v)| v.clone()).unwrap_or(kv);
        if let Some(chat) = root.get("HudChat") {
            let n = |k: &str| chat.str(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
            hud.panels.insert(
                "HudChat".into(),
                HudPanel {
                    x: HudCoord::Start(n("xpos")),
                    y: HudCoord::Start(n("ypos")),
                    wide: n("wide"),
                    tall: n("tall"),
                    background: None,
                    icon: Default::default(),
                    digit: Default::default(),
                    digit2: Default::default(),
                    keys: Default::default(),
                },
            );
        }
    }
    let mut menus = GameMenus::default();
    for file in STRINGS {
        if let Some(bytes) = materials.read(file) {
            for (k, v) in localization::localization(&localization::decode(&bytes)) {
                menus.strings.entry(k).or_insert(v);
            }
        }
    }
    // The animation script travels in the strings (read here with the
    // rest, through the mount, so it is the map's, cached with it).
    if let Some(bytes) = materials.read("scripts/hudanimations.txt") {
        menus.strings.insert(ANIMATIONS.to_string(), String::from_utf8_lossy(&bytes).into_owned());
    }
    hud.menus = Some(menus);
    (!hud.panels.is_empty()).then_some(hud)
}

/// The key of `scripts/hudanimations.txt` in `GameMenus::strings`.
pub const ANIMATIONS: &str = "@hudanimations";

/// The HUD animation script with the scheme's colour names resolved.
pub fn animations(hud: &GameHud) -> Script {
    let text = hud.menus.as_ref().and_then(|m| m.strings.get(ANIMATIONS)).map_or("", String::as_str);
    Script::parse(text, &|name| hud.colors.get(name).copied())
}
