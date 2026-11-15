//! Halo's HUD from a map file (`HudData::load`; the front end reads the
//! donor map, `game::body::load::DONOR_MAP`, so the HUD is the same on
//! every map): the Spartan's unit HUD (the biped's first HUD interface:
//! one local player), every weapon's HUD chain by its weapon tag, the
//! empty hands' HUD, the two grenade HUDs, the HUD globals, the number
//! tag, and every bitmap they draw, decoded to RGBA8 (`Page`), with the
//! sequence rule that picks a picture (`HudData::picture`).
//!
//! Plain data: no Bevy (the nodes make textures of the pages).

use std::collections::BTreeMap;

use blam_formats::{
    bitmap::Bitmap,
    cache::{MapFile, TagRef, class},
    hud::{
        UnitHud,
        globals::{HudGlobals, HudNumber},
        grenade::GrenadeHud,
        weapon::{WeaponHud, chain, resolve},
    },
    object::Biped,
    weapon::Weapon,
};

use crate::game::body::load::BIPED;

/// A bitmap tag's pictures: its sequences and its images as RGBA8.
#[derive(Clone, Debug)]
pub struct Page {
    pub path: String,
    pub bitmap: Bitmap,
    /// (width, height, RGBA8, top row first) per image.
    pub images: Vec<(u32, u32, Vec<u8>)>,
    /// Drawn as a meter (point-sampled).
    pub meter: bool,
}

/// A picture of a page: which image, its texture rectangle (fractions:
/// left, top, right, bottom) and its pixel size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Picture {
    pub page: u32,
    pub image: usize,
    pub uv: [f32; 4],
    pub size: [f32; 2],
}

/// Everything Halo's HUD draws, from one map.
#[derive(Clone, Debug)]
pub struct HudData {
    pub unit: UnitHud,
    /// Each weapon's HUD chain (itself first, then its parents) by its
    /// weapon tag's path, lower case.
    pub weapons: BTreeMap<String, Vec<WeaponHud>>,
    /// The chain shown with empty hands (`ui\hud\empty`).
    pub empty: Vec<WeaponHud>,
    /// Frag (`ui\hud\default`), then plasma.
    pub grenades: Vec<GrenadeHud>,
    pub globals: HudGlobals,
    pub number: HudNumber,
    /// The pages by tag id.
    pub pages: BTreeMap<u32, Page>,
    /// The motion tracker's own pictures (not in any tag field): the
    /// sweep, its mask and the blip.
    pub sweeper: Option<u32>,
    pub sweeper_mask: Option<u32>,
    pub blip: Option<u32>,
}

fn id(r: &TagRef) -> Option<u32> {
    (!r.is_null()).then_some(r.id.0)
}

impl HudData {
    /// Read the HUD of `map` (a multiplayer map).
    pub fn load(map: &MapFile) -> Result<HudData, String> {
        let e = |e: std::io::Error| e.to_string();
        let biped = Biped::read(map, map.find(class::BIPED, BIPED).ok_or_else(|| format!("no biped `{BIPED}`"))?).map_err(e)?;
        let unit_ref = biped.unit.new_hud_interfaces.first().map(|h| h.hud).ok_or("the biped has no HUD interface")?;
        let unit = UnitHud::read(map, resolve(map, &unit_ref).ok_or("the biped's HUD interface is missing")?).map_err(e)?;
        let chain_of = |tag| chain(map, tag).map(|c| c.into_iter().map(|(_, h)| h).collect::<Vec<_>>());
        let mut weapons = BTreeMap::new();
        for t in map.of_class(class::WEAPON) {
            let Ok(w) = Weapon::read(map, t) else { continue };
            if let Some(hud) = resolve(map, &w.own.hud_interface) {
                weapons.insert(t.path.to_lowercase(), chain_of(hud).map_err(e)?);
            }
        }
        let globals = HudGlobals::read(map, map.of_class(class::HUD_GLOBALS).next().ok_or("no HUD globals")?).map_err(e)?;
        let empty = match resolve(map, &globals.defaults.weapon_hud) {
            Some(t) => chain_of(t).map_err(e)?,
            None => Vec::new(),
        };
        let mut grenades = Vec::new();
        for path in ["ui\\hud\\default", "ui\\hud\\plasma_flare"] {
            if let Some(t) = map.find(class::GRENADE_HUD_INTERFACE, path) {
                grenades.push(GrenadeHud::read(map, t).map_err(e)?);
            }
        }
        let number = HudNumber::read(map, map.of_class(class::HUD_NUMBER).next().ok_or("no HUD number tag")?).map_err(e)?;
        // Every picture the HUD draws, and which pages are meters.
        let mut wanted: BTreeMap<u32, bool> = BTreeMap::new();
        let mut want = |r: &TagRef, meter: bool| {
            if let Some(i) = id(r) {
                *wanted.entry(i).or_default() |= meter;
            }
        };
        for p in [&unit.shield, &unit.health] {
            want(&p.background.bitmap, false);
            want(&p.meter.bitmap, true);
        }
        want(&unit.sensor.background.bitmap, false);
        want(&unit.sensor.foreground.bitmap, false);
        for hud in weapons.values().chain(std::iter::once(&empty)).flatten() {
            hud.statics.iter().for_each(|s| want(&s.element.bitmap, false));
            hud.meters.iter().for_each(|m| want(&m.element.bitmap, true));
            hud.crosshairs.iter().for_each(|c| want(&c.bitmap, false));
            hud.overlays.iter().for_each(|o| want(&o.bitmap, false));
            hud.screen_effects.iter().for_each(|s| want(&s.mask_full, false));
        }
        for g in &grenades {
            want(&g.background.bitmap, false);
            want(&g.count_background.bitmap, false);
            want(&g.overlay_bitmap, false);
        }
        want(&number.bitmap, false);
        want(&globals.damage.bitmap, false);
        let named = |name: &str| {
            map.of_class(class::BITMAP)
                .find(|t| t.path.rsplit('\\').next().is_some_and(|n| n.eq_ignore_ascii_case(name)))
                .map(|t| t.id.0)
        };
        let (sweeper, sweeper_mask, blip) = (named("hud_sweeper"), named("hud_sweeper_mask"), named("hud_sensor_blip"));
        for i in [sweeper, sweeper_mask, blip].into_iter().flatten() {
            wanted.entry(i).or_default();
        }
        let mut pages = BTreeMap::new();
        for (tag_id, meter) in wanted {
            let Some(tag) = map.tags().iter().find(|t| t.id.0 == tag_id) else { continue };
            let bitmap = Bitmap::read(map, tag).map_err(e)?;
            let images = (0..bitmap.images.len())
                .map(|i| {
                    let d = bitmap.decode_image(map, i).map_err(e)?;
                    let level = d.faces.first().and_then(|f| f.first()).ok_or_else(|| format!("{}: no level", tag.path))?;
                    Ok((level.width, level.height, level.rgba.clone()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            pages.insert(tag_id, Page { path: tag.path.clone(), bitmap, images, meter });
        }
        Ok(HudData { unit, weapons, empty, grenades, globals, number, pages, sweeper, sweeper_mask, blip })
    }

    /// The HUD chain of the weapon whose tag is `path` (any case).
    pub fn weapon(&self, path: &str) -> Option<&[WeaponHud]> {
        self.weapons.get(&path.to_lowercase()).map(Vec::as_slice)
    }

    /// The picture `sequence`, `frame` of the page `r` (bitmaps.md's
    /// sequence rule; a sprite frame wraps over the sequence's sprites):
    /// None when the page, the sequence or the image is missing.
    pub fn picture(&self, r: &TagRef, sequence: i16, frame: i64) -> Option<Picture> {
        self.picture_of(id(r)?, sequence, frame)
    }

    /// `picture` by the page's tag id.
    pub fn picture_of(&self, page_id: u32, sequence: i16, frame: i64) -> Option<Picture> {
        let page = self.pages.get(&page_id)?;
        let seq = page.bitmap.sequences.get(usize::try_from(sequence).ok()?)?;
        if !seq.sprites.is_empty() {
            let s = &seq.sprites[frame.rem_euclid(seq.sprites.len() as i64) as usize];
            let image = usize::from(s.bitmap_index);
            let (w, h, _) = page.images.get(image)?;
            return Some(Picture {
                page: page_id,
                image,
                uv: [s.left, s.top, s.right, s.bottom],
                // Not rounded: the rectangle's bounds are truncated (a 104.99994
                // pixel sprite is 104 wide on screen, as the spec's heat bar).
                size: [(s.right - s.left) * *w as f32, (s.bottom - s.top) * *h as f32],
            });
        }
        if seq.bitmap_count == 0 {
            return None;
        }
        let image = usize::from(seq.first_bitmap) + frame.rem_euclid(i64::from(seq.bitmap_count)) as usize;
        self.whole(page_id, image)
    }

    /// Image `index` of page `r` drawn whole (a crosshair item "not a
    /// sprite").
    pub fn whole_image(&self, r: &TagRef, index: i16) -> Option<Picture> {
        self.whole(id(r)?, usize::try_from(index).ok()?)
    }

    fn whole(&self, page_id: u32, image: usize) -> Option<Picture> {
        let (w, h, _) = self.pages.get(&page_id)?.images.get(image)?;
        Some(Picture { page: page_id, image, uv: [0.0, 0.0, 1.0, 1.0], size: [*w as f32, *h as f32] })
    }

    /// Image `image` of page `page_id`'s alpha at texture coordinates
    /// (u, v) (nearest, clamped to the edge).
    pub fn alpha_at(&self, page_id: u32, image: usize, u: f32, v: f32) -> f32 {
        let Some((w, h, rgba)) = self.pages.get(&page_id).and_then(|p| p.images.get(image)) else { return 1.0 };
        let x = ((u.clamp(0.0, 1.0) * *w as f32) as u32).min(w.saturating_sub(1));
        let y = ((v.clamp(0.0, 1.0) * *h as f32) as u32).min(h.saturating_sub(1));
        rgba.get(((y * w + x) * 4 + 3) as usize).map_or(1.0, |a| f32::from(*a) / 255.0)
    }
}
