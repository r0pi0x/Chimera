//! CS:S's HUD look, read from the install: panel layout
//! (`scripts/hudlayout.res`), fonts and colours (`resource/clientscheme.res`
//! and the TTFs it lists), icons (`scripts/hud_textures.txt`,
//! `scripts/mod_textures.txt`); the
//! VGUI menus come from `super::vgui`.

use bevy::prelude::*;

use super::material::MaterialLoader;
use crate::map::hud::GameHud;

/// The KeyValues tree and the resource-file reader (platform conditionals
/// dropped), from `source_formats::keyvalues`.
pub use source_formats::keyvalues::{Kv, parse_res as parse};

/// The base HUD's icons (`scripts/hud_textures.txt`) CS:S draws: the
/// default crosshair and voice chat's (`voice_self`: the talking player's
/// own, `voice_player`: others'). The rest of the file is HL2's.
pub const BASE_ICONS: &[&str] = &["crosshair_default", "voice_self", "voice_player"];

fn color(text: &str) -> Option<[u8; 4]> {
    source_engine::hud::look::colour(text)
}

/// A TrueType file's family name (name table, ID 1), from
/// `source_engine::hud::look` (lift S7).
pub use source_engine::hud::look::family_name;

/// The HUD look from the install, or None when its files are missing; with
/// the menus (`super::vgui`, `map`: the loaded map, for its description).
/// The shared reading is `source_engine::hud::look::load`; CS:S adds its
/// scopes, scoreboard pictures, screens and menus.
pub fn load(materials: &mut MaterialLoader, map: &str) -> Option<GameHud> {
    let mut hud = source_engine::hud::look::load(
        materials,
        &source_engine::hud::look::LookOptions {
            base_icons: BASE_ICONS,
            layout: parse,
            font_files: false,
            first_wins: false,
        },
    )?;

    // The sniper scope: a quarter of the ring (transparent inside, black
    // outside; the bottom-right quarter as stored) and the lens tint; the
    // icon over a teammate's head while they speak on the radio; the
    // scoreboard's status icons and its row highlight; the planted bomb's
    // LED's glow (and `sprites/c4`, which nothing draws: CS:S has no
    // marker over the bomb).
    for (name, material) in [
        ("scope_arc", "sprites/scope_arc"),
        ("scope_lens", "overlays/scope_lens"),
        ("radio", "sprites/radio"),
        ("scoreboard_bomb", "hud/scoreboard_bomb"),
        ("scoreboard_dead", "hud/scoreboard_dead"),
        ("scoreboard_defuser", "hud/scoreboard_defuser"),
        ("scoreboard_select", "vgui/scoreboard/scoreboard-select"),
        ("c4", "sprites/c4"),
        ("ledglow", "sprites/ledglow"),
    ] {
        if materials.read(&format!("materials/{material}.vmt")).is_none() {
            continue;
        }
        if let Some(texture) = materials.resolve(material).texture {
            let t = &materials.textures[texture];
            let rect = [0.0, 0.0, t.width as f32, t.height as f32];
            hud.sprites.insert(name.to_string(), crate::map::hud::HudSprite { texture, rect });
        }
    }

    hud.screens = screens(materials).collect();
    hud.menus = super::vgui::load(materials, &mut hud, map);
    hud.game = crate::game::GAME.to_string();
    (!hud.panels.is_empty()).then_some(hud)
}

/// The screens on models (`scripts/vgui_screens.txt`: each panel's size in
/// pixels); the C4's (`c4_*`) take their font and colours from the C4
/// panel's scheme, `resource/c4panel.res`.
fn screens(materials: &MaterialLoader) -> impl Iterator<Item = (String, crate::map::hud::ModelScreen)> {
    let text = |p: &str| materials.read(p).map(|b| String::from_utf8_lossy(&b).into_owned());
    let Some(list) = text("scripts/vgui_screens.txt") else {
        return Vec::new().into_iter();
    };
    let list = parse(&list);
    let list = list.items().first().map(|(_, v)| v.clone()).unwrap_or(list);
    let c4 = text("resource/c4panel.res").map(|t| super::gameui::parse_pc(&t));
    let c4 = c4.map(|s| s.items().first().map(|(_, v)| v.clone()).unwrap_or(s));
    let num = |v: &Kv, k: &str| v.str(k).and_then(|x| x.trim().parse::<f32>().ok()).unwrap_or(0.0);
    let mut out = Vec::new();
    for (name, v) in list.items() {
        let mut screen = crate::map::hud::ModelScreen {
            pixels: Vec2::new(num(v, "pixelswide"), num(v, "pixelshigh")),
            ..Default::default()
        };
        if let Some(scheme) = c4.as_ref().filter(|_| name.to_lowercase().starts_with("c4")) {
            screen.font = super::vgui::fonts(scheme).remove("Default").and_then(|s| s.into_iter().next());
            for (k, c) in scheme.get("Colors").map(Kv::items).unwrap_or_default() {
                if let Some(rgba) = c.str_value().and_then(color) {
                    screen.colors.insert(k.clone(), rgba);
                }
            }
        }
        out.push((name.to_lowercase(), screen));
    }
    out.into_iter()
}

/// The map's radar picture: `resource/overviews/<map>.txt` (upper-left
/// world corner `pos_x`/`pos_y`, `scale` world units per pixel of a
/// 1024-wide image, `rotate`) and its material, preferring the `_radar`
/// variant the game's radar uses.
pub fn overview(materials: &mut MaterialLoader, map: &str) -> Option<crate::map::hud::MapOverview> {
    let text = materials.read(&format!("resource/overviews/{}.txt", map.to_lowercase()))?;
    let kv = parse(&String::from_utf8_lossy(&text));
    let o = &kv.items().first()?.1;
    let num = |k: &str| o.str(k).and_then(|v| v.trim().parse::<f32>().ok());
    let material = o.str("material")?.to_string();
    // The _radar variant only when it exists (resolving a missing material
    // counts as a load warning).
    let radar = format!("{material}_radar");
    let name = if materials.read(&format!("materials/{radar}.vmt")).is_some() {
        radar
    } else {
        material
    };
    let texture = materials.resolve(&name).texture?;
    let size = {
        let t = &materials.textures[texture];
        Vec2::new(t.width as f32, t.height as f32)
    };
    let (pos_x, pos_y, scale) = (num("pos_x")?, num("pos_y")?, num("scale")?);
    let unit = super::bsp::METERS_PER_UNIT;
    // Source x is engine x, Source y is engine -z; scale is for a 1024-pixel
    // image.
    Some(crate::map::hud::MapOverview {
        texture,
        origin: Vec2::new(pos_x * unit, -pos_y * unit),
        meters_per_pixel: scale * unit * 1024.0 / size.x,
        rotate: num("rotate").is_some_and(|r| r != 0.0),
        size,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyvalues_with_conditionals() {
        let kv = parse(
            r#""Layout" { HudHealth { "xpos" "8" [$WIN32] "ypos" "446" "wide" "80" } // c
               Colors { "Orange" "255 176 0 255" } }"#,
        );
        let root = &kv.items()[0].1;
        let h = root.get("hudhealth").unwrap();
        assert_eq!(h.str("xpos"), Some("8"));
        assert_eq!(h.str("ypos"), Some("446"));
        assert_eq!(
            color(root.get("Colors").unwrap().str("Orange").unwrap()),
            Some([255, 176, 0, 255])
        );
    }
}
