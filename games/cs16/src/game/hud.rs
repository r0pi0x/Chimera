//! Counter-Strike 1.6's HUD pictures from the install (specs/cs16/hud.md,
//! "Sprites"; specs/goldsrc/spr.md, "The HUD's sprite lists"), read at run
//! time for its HUD (`cs16_ui::hud`) and every app's kill feed:
//!
//! - the shared pictures (`Pictures::shared`, `SHARED`): `sprites/hud.txt`'s
//!   640 entries for the digits, the cross, the suits, the divider and the
//!   kill marks (`d_headshot`, `d_skull`, `d_grenade`);
//! - each weapon's (`WeaponPictures`): its `sprites/weapon_<name>.txt`'s
//!   640 `weapon`, `weapon_s` and `ammo`;
//! - the kill icons by weapon id (`Pictures::kill_icons`): `d_<name>`
//!   (`kill_icon_name`: the HE grenade's is `d_grenade`), none for a weapon
//!   without one (the feed then shows `d_skull`);
//! - the damage direction arcs, `sprites/640_pain.spr`'s four frames.
//!
//! Each rectangle is cut out of frame 0 of its atlas
//! (`sprites/<file>.spr`) as RGBA: the HUD atlases are additive and grey,
//! kept as they are (opaque grey), for the HUD to tint and add.
//! `pictures` gives None while a file the mount names isn't here yet (the
//! asset store before CS 1.6's `players` set arrived); callers try again
//! later. Nothing is fetched. `read_all` reads every file the HUD may use
//! (the `players` set, `sets`).
//!
//! The two layouts (hud.md, "Units and conventions": 640 at screens 640 wide
//! or more, else 320): `pictures(mount, resolution)` reads the entries of
//! one of them, each from its own atlases (`640hud*`, `320hud*`) and pain
//! sprite (`640_pain`, `320_pain`); the two are read apart, so a set that
//! has only the 640 files still gives its 640 pictures.
//!
//! Known shortcuts:
//! - A 320 entry the install's list lacks (the list ends before its
//!   count) is absent: the HUD then leaves that picture out.

use std::collections::BTreeMap;

use goldsrc_formats::spr::{Sprite, parse_sprite_list};

use crate::{map::hud::UiImage, mount::Mount};

/// The shared HUD list.
pub const HUD_LIST: &str = "sprites/hud.txt";

/// The layouts, the first the default one (hud.md, "Units and
/// conventions").
pub const RESOLUTIONS: [u32; 2] = [640, 320];

/// The layout at and above which the screen's width uses the 640 entries.
pub const SWITCH_WIDTH: u32 = 640;

/// The layout a screen `width` pixels wide uses.
pub fn resolution_for(width: u32) -> u32 {
    if width >= SWITCH_WIDTH { 640 } else { 320 }
}

/// The damage direction arcs of layout `resolution` (four frames: front,
/// right, behind, left).
pub fn pain_path(resolution: u32) -> String {
    format!("sprites/{resolution}_pain.spr")
}

/// The shared entries read from `HUD_LIST`.
pub const SHARED: &[&str] = &[
    "number_0",
    "number_1",
    "number_2",
    "number_3",
    "number_4",
    "number_5",
    "number_6",
    "number_7",
    "number_8",
    "number_9",
    "divider",
    "stopwatch",
    "selection",
    "bucket0",
    "bucket1",
    "bucket2",
    "bucket3",
    "bucket4",
    "bucket5",
    "cross",
    "suit_empty",
    "suit_full",
    "suithelmet_empty",
    "suithelmet_full",
    "d_headshot",
    "d_skull",
    "d_grenade",
];

/// A weapon's HUD pictures (hud.md, "Per weapon").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WeaponPictures {
    /// `weapon`: the weapon menu's picture.
    pub weapon: Option<UiImage>,
    /// `weapon_s`: the same, lit.
    pub weapon_s: Option<UiImage>,
    /// `ammo`: its ammunition's icon.
    pub ammo: Option<UiImage>,
}

/// Everything `pictures` reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pictures {
    /// `SHARED`'s pictures by entry name.
    pub shared: BTreeMap<String, UiImage>,
    /// Weapons' pictures by weapon id.
    pub weapons: BTreeMap<String, WeaponPictures>,
    /// Kill icons by weapon id.
    pub kill_icons: BTreeMap<String, UiImage>,
    /// The pain arcs: front, right, behind, left.
    pub pain: Vec<UiImage>,
}

/// A weapon id's script name (`cs16:weapon_ak47` -> `ak47`).
pub fn script_name(id: &str) -> &str {
    id.rsplit(':').next().unwrap_or(id).trim_start_matches("weapon_")
}

/// The kill icon's entry of weapon `id` (`d_ak47`; the HE grenade's
/// `d_grenade`).
pub fn kill_icon_name(id: &str) -> String {
    match script_name(id) {
        "hegrenade" => "d_grenade".into(),
        name => format!("d_{name}"),
    }
}

/// A weapon's list file (`sprites/weapon_ak47.txt`).
pub fn weapon_list(id: &str) -> String {
    format!("sprites/weapon_{}.txt", script_name(id))
}

/// Every weapon id the game has (its data).
fn weapon_ids() -> Vec<String> {
    super::weapons::weapons().weapons.into_iter().map(|w| w.id).collect()
}

/// The file of `mount` at `path` if it is here (`Some(None)`: the mount
/// doesn't have it); None when the mount names it but it isn't here yet
/// (reading would fetch it).
fn here(mount: &Mount, path: &str) -> Option<Option<Vec<u8>>> {
    if !mount.contains(path) {
        return Some(None);
    }
    mount.held(path).then(|| mount.read(path).ok())
}

/// A sprite list's entries at `resolution` by name: (atlas, rectangle).
pub fn list_entries(text: &str, resolution: u32) -> BTreeMap<String, (String, [i32; 4])> {
    parse_sprite_list(text)
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.resolution == resolution)
        .map(|e| (e.name, (e.sprite, e.rect)))
        .collect()
}

/// Atlases read once each, rectangles cut from their frame 0.
#[derive(Default)]
struct Atlases(BTreeMap<String, Option<UiImage>>);

impl Atlases {
    /// Rectangle `[x, y, w, h]` of atlas `name` (`640hud7`): `Some(None)`
    /// when the atlas is missing or the rectangle past its edge; None while
    /// the atlas isn't here yet.
    fn cut(&mut self, mount: &Mount, name: &str, [x, y, w, h]: [i32; 4]) -> Option<Option<UiImage>> {
        let key = name.to_lowercase();
        if !self.0.contains_key(&key) {
            let bytes = here(mount, &format!("sprites/{key}.spr"))?;
            let frame0 = bytes.and_then(|b| Sprite::parse(&b).ok()).and_then(|s| {
                let f = s.all_frames().first().copied()?.clone();
                Some(UiImage { width: f.width, height: f.height, rgba8: s.rgba(&f) })
            });
            self.0.insert(key.clone(), frame0);
        }
        Some(self.0[&key].as_ref().and_then(|atlas| crop(atlas, x, y, w, h)))
    }
}

/// `w` x `h` at (`x`, `y`) of `image` (None past its edge or empty).
pub fn crop(image: &UiImage, x: i32, y: i32, w: i32, h: i32) -> Option<UiImage> {
    if x < 0 || y < 0 || w <= 0 || h <= 0 {
        return None;
    }
    let (x, y, w, h) = (x as u32, y as u32, w as u32, h as u32);
    if x + w > image.width || y + h > image.height {
        return None;
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = ((row * image.width + x) * 4) as usize;
        out.extend_from_slice(&image.rgba8[start..start + (w * 4) as usize]);
    }
    Some(UiImage { width: w, height: h, rgba8: out })
}

/// The frames of sprite `path` as pictures (`Some(empty)`: missing).
fn frames(mount: &Mount, path: &str) -> Option<Vec<UiImage>> {
    let Some(bytes) = here(mount, path)? else { return Some(Vec::new()) };
    let Ok(s) = Sprite::parse(&bytes) else { return Some(Vec::new()) };
    Some(s.all_frames().into_iter().map(|f| UiImage { width: f.width, height: f.height, rgba8: s.rgba(f) }).collect())
}

/// CS 1.6's HUD pictures of layout `resolution` from `mount` (module
/// doc); None while a file it names isn't here yet.
pub fn pictures(mount: &Mount, resolution: u32) -> Option<Pictures> {
    let mut out = Pictures::default();
    let mut atlases = Atlases::default();
    let ids = weapon_ids();
    if let Some(text) = here(mount, HUD_LIST)? {
        let entries = list_entries(&String::from_utf8_lossy(&text), resolution);
        for name in SHARED {
            if let Some((atlas, rect)) = entries.get(*name)
                && let Some(p) = atlases.cut(mount, atlas, *rect)?
            {
                out.shared.insert(name.to_string(), p);
            }
        }
        for id in &ids {
            if let Some((atlas, rect)) = entries.get(&kill_icon_name(id))
                && let Some(p) = atlases.cut(mount, atlas, *rect)?
            {
                out.kill_icons.insert(id.clone(), p);
            }
        }
    }
    for id in &ids {
        let Some(text) = here(mount, &weapon_list(id))? else { continue };
        let entries = list_entries(&String::from_utf8_lossy(&text), resolution);
        let mut cut = |name: &str| -> Option<Option<UiImage>> {
            match entries.get(name) {
                Some((atlas, rect)) => atlases.cut(mount, atlas, *rect),
                None => Some(None),
            }
        };
        let w = WeaponPictures { weapon: cut("weapon")?, weapon_s: cut("weapon_s")?, ammo: cut("ammo")? };
        out.weapons.insert(id.clone(), w);
    }
    out.pain = frames(mount, &pain_path(resolution))?;
    Some(out)
}

/// Read every file the HUD may use, for the asset service's sets
/// (`sets::load_players`): the lists, every atlas their 640 entries name
/// and the pain sprite.
pub fn read_all(mount: &Mount) {
    let mut lists = vec![HUD_LIST.to_string()];
    lists.extend(weapon_ids().iter().map(|id| weapon_list(id)));
    let mut atlases = std::collections::BTreeSet::new();
    for list in &lists {
        if let Ok(text) = mount.read(list) {
            for resolution in RESOLUTIONS {
                atlases.extend(list_entries(&String::from_utf8_lossy(&text), resolution).into_values().map(|(a, _)| a.to_lowercase()));
            }
        }
    }
    for atlas in atlases {
        let _ = mount.read(&format!("sprites/{atlas}.spr"));
    }
    for resolution in RESOLUTIONS {
        let _ = mount.read(&pain_path(resolution));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_follow_the_spec() {
        assert_eq!(kill_icon_name("cs16:weapon_awp"), "d_awp");
        assert_eq!(kill_icon_name("cs16:weapon_hegrenade"), "d_grenade");
        assert_eq!(weapon_list("cs16:weapon_ak47"), "sprites/weapon_ak47.txt");
    }

    /// spr.md's test case: `weapon_ak47.txt`'s `ammo` at 640.
    #[test]
    fn a_list_reads_its_640_entries() {
        let text = "4\nweapon 320 320hud1 0 0 80 20\nammo 320 640hud7 72 72 24 24\nweapon 640 640hud10 0 0 170 45\nammo 640 640hud7 72 72 24 24\n";
        let e = list_entries(text, 640);
        assert_eq!(list_entries(text, 320)["weapon"], ("320hud1".to_string(), [0, 0, 80, 20]));
        assert_eq!(e["ammo"], ("640hud7".to_string(), [72, 72, 24, 24]));
        assert_eq!(e["weapon"], ("640hud10".to_string(), [0, 0, 170, 45]));
    }

    #[test]
    fn the_screen_s_width_picks_the_layout() {
        assert_eq!([resolution_for(1920), resolution_for(640), resolution_for(639), resolution_for(320)], [640, 640, 320, 320]);
        assert_eq!(pain_path(320), "sprites/320_pain.spr");
    }

    #[test]
    fn a_rectangle_is_cut_from_its_atlas() {
        let atlas = UiImage { width: 4, height: 2, rgba8: (0..32).collect() };
        let c = crop(&atlas, 1, 1, 2, 1).unwrap();
        assert_eq!((c.width, c.height), (2, 1));
        assert_eq!(c.rgba8, (20..28).collect::<Vec<u8>>());
        assert!(crop(&atlas, 3, 0, 2, 1).is_none());
    }
}
