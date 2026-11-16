//! TF2's HUD look from the install (specs/tf2/hud.md), as the neutral
//! `map::hud::GameHud` in the map's kit (`maps::body_kit`) and on other
//! games' maps (`maps::dress_guest_with`), read at run time by Source's
//! shared reader (`source_engine::hud::look`: `resource/clientscheme.res`'s
//! colours and fonts with their files, `scripts/hudlayout.res`'s panels,
//! `scripts/mod_textures.txt`'s icons: the kill icons' cells of
//! `hud/d_images` and `hud/dneg_images`), with TF2's pictures on top
//! (`PICTURES`: the health crosses, the crosshair sheet). Nothing of it is
//! in the repository; a map without the install has no TF2 look, and the
//! shell's HUD draws.

use source_engine::{
    content::material::MaterialLoader,
    hud::look::{self, LookOptions},
};
use source_formats::keyvalues;

use crate::map::hud::{GameHud, HudSprite};

/// Whole pictures the HUD draws, by our name: their materials (hud.md
/// 3.2, 3.4).
pub const PICTURES: &[(&str, &str)] = &[
    ("health_bg", "hud/health_bg"),
    ("health_over_bg", "hud/health_over_bg"),
    ("health_color", "hud/health_color"),
    ("health_dead", "hud/health_dead"),
    ("crosshairs", "sprites/crosshairs"),
];

/// The HUD look from the install (None when its files are missing).
pub fn load(materials: &mut MaterialLoader) -> Option<GameHud> {
    let mut hud = look::load(
        materials,
        &LookOptions { base_icons: &[], layout: keyvalues::parse_pc, font_files: true, first_wins: true },
    )?;
    for (name, material) in PICTURES {
        if let Some(texture) = materials.resolve(material).texture {
            let t = &materials.textures[texture];
            hud.sprites.insert((*name).to_string(), HudSprite { texture, rect: [0.0, 0.0, t.width as f32, t.height as f32] });
        }
    }
    hud.game = super::GAME.to_string();
    Some(hud)
}
