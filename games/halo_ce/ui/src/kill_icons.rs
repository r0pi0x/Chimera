//! Halo's weapons' kill-feed icons (`chimera_client::client::weapon_icons::
//! WeaponIcons`, the party's shared feed): each gun's picture from the
//! disc, the one the HUD's prompts show it by (specs/halo_ce/hud.md,
//! "Weapon HUD interface" 0x13C, the messaging icon: a sequence of the HUD
//! globals' messaging icon bitmap, "Hold [X] to swap for [picture]"),
//! found up the weapon's HUD chain (its own HUD first); the two grenades'
//! from their grenade HUDs' first overlay picture (the plate's grenade
//! icon: the grenade HUD's own messaging icon is junk on the disc, 0x1B8).
//! Each picture is cut out of its sprite page into an image of its own
//! (`icon_rgba`), white with its brightness times its alpha as the ink
//! (the HUD tints them; the feed's other icons are white), made when the HUD's data is read (`ui::hud::nodes::
//! HudLoad`, from the donor map) and registered under Halo's game name;
//! a weapon without one (the melee and other kinds of kill, any gun whose
//! chain names none) keeps the feed's fallback, its display name. Nothing
//! is copied: everything is read from the user's disc at run time. The
//! party's front end (`ui::party_app`) starts the HUD's read at once, so
//! the icons are there before the local player first holds a Halo gun.

use std::collections::BTreeMap;

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};

use super::hud::{
    data::{HudData, Picture},
    nodes::HudLoad,
};
use crate::game::{
    GAME,
    weapons::{
        self,
        systems::{FRAG_GRENADE, PLASMA_GRENADE},
    },
};

/// A picture's texels cut out of its page: (width, height, RGBA8).
pub fn icon_rgba(data: &HudData, p: &Picture) -> Option<(u32, u32, Vec<u8>)> {
    let (w, h, rgba) = data.pages.get(&p.page)?.images.get(p.image)?;
    let x0 = (p.uv[0] * *w as f32).round() as u32;
    let y0 = (p.uv[1] * *h as f32).round() as u32;
    let x1 = ((p.uv[2] * *w as f32).round() as u32).min(*w);
    let y1 = ((p.uv[3] * *h as f32).round() as u32).min(*h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut out = Vec::with_capacity(((x1 - x0) * (y1 - y0) * 4) as usize);
    for y in y0..y1 {
        let row = ((y * w + x0) * 4) as usize;
        out.extend_from_slice(rgba.get(row..row + ((x1 - x0) * 4) as usize)?);
    }
    Some((x1 - x0, y1 - y0, out))
}

/// Each Halo weapon id's icon picture in `data` (module doc): the guns'
/// messaging icons, the grenades' plate icons.
pub fn pictures(data: &HudData) -> BTreeMap<&'static str, Picture> {
    let mut out = BTreeMap::new();
    let page = data.globals.messaging.icons;
    for (id, gun) in weapons::guns() {
        let Some(chain) = data.weapon(&gun.tag) else { continue };
        let Some(seq) = chain.iter().map(|h| h.messaging_icon.sequence).find(|s| *s >= 0) else { continue };
        if let (Some(p), Some(id)) = (data.picture(&page, seq, 0), weapons::GUNS.iter().find(|g| **g == id)) {
            out.insert(*id, p);
        }
    }
    for (id, g) in [FRAG_GRENADE, PLASMA_GRENADE].into_iter().zip(&data.grenades) {
        if let Some(p) = g.overlays.iter().find_map(|o| data.picture(&g.overlay_bitmap, o.sequence, 0)) {
            out.insert(id, p);
        }
    }
    out
}

/// Each icon as an image (module doc), by weapon id.
pub fn images(data: &HudData, assets: &mut Assets<Image>) -> BTreeMap<&'static str, WeaponIcon> {
    let mut out = BTreeMap::new();
    for (id, p) in pictures(data) {
        let Some((w, h, mut rgba)) = icon_rgba(data, &p) else { continue };
        // As the HUD draws them: a light picture over its alpha, tinted
        // there; here white with the picture's brightness as its ink, as
        // the other games' feed icons are.
        for t in rgba.chunks_mut(4) {
            let ink = u16::from(t[0].max(t[1]).max(t[2])) * u16::from(t[3]) / 255;
            t.copy_from_slice(&[255, 255, 255, ink as u8]);
        }
        let image = Image::new(
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            TextureDimension::D2,
            rgba,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
        );
        out.insert(id, WeaponIcon::Image { image: assets.add(image), size: Vec2::new(w as f32, h as f32) });
    }
    out
}

/// Register the icons once the HUD's data is read.
pub fn register(load: Res<HudLoad>, assets: Option<ResMut<Assets<Image>>>, icons: Option<ResMut<WeaponIcons>>, mut done: Local<bool>) {
    let (Some(data), Some(mut assets), Some(mut icons)) = (load.data(), assets, icons) else { return };
    if *done {
        return;
    }
    *done = true;
    let made = images(data, &mut assets);
    icons.register(GAME, move |id| made.get(id).cloned());
}

/// The icons in an app with Halo's HUD (`ui::hud::HaloHudPlugin`).
pub fn plugin(app: &mut App) {
    app.init_resource::<WeaponIcons>().add_systems(Update, register);
}
