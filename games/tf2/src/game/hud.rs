//! TF2's HUD look from the install (specs/tf2/hud.md), as the neutral
//! `map::hud::GameHud` in the map's kit (`maps::body_kit`) and on other
//! games' maps (`maps::dress_guest_with`), read at run time by Source's
//! shared reader (`source_engine::hud::look`: `resource/clientscheme.res`'s
//! colours and fonts with their files, `scripts/hudlayout.res`'s panels,
//! `scripts/mod_textures.txt`'s icons: the kill icons' cells of
//! `hud/d_images` and `hud/dneg_images`), with TF2's pictures on top
//! (`PICTURES`: the health crosses, the crosshair sheet). Nothing of it is
//! in the repository; a map without the install has no TF2 look, and the
//! shell's HUD draws. The guns' kill icons as pictures
//! (`kill_icon_pictures`: each gun's `hud.icon` cell of `mod_textures.txt`
//! cut from its sheet) for the kill feed (`ui::systems::
//! register_kill_icons`), only once their files are here (`Mount::held`).

use std::collections::BTreeMap;

use source_engine::{
    content::material::MaterialLoader,
    hud::look::{self, LookOptions},
};
use source_formats::keyvalues;

use crate::{
    map::hud::{GameHud, HudSprite, UiImage},
    mount::Mount,
};

/// Whole pictures the HUD draws, by our name: their materials (hud.md
/// 3.2, 3.4).
pub const PICTURES: &[(&str, &str)] = &[
    ("health_bg", "hud/health_bg"),
    ("health_over_bg", "hud/health_over_bg"),
    ("health_color", "hud/health_color"),
    ("health_dead", "hud/health_dead"),
    ("crosshairs", "sprites/crosshairs"),
];

/// The class portrait's pictures (hud.md 3.10, the 2D image): each
/// class's by team (`hud/class_<short><red|blue>`) and the team's
/// background, by our name: (sprite, material).
pub const PORTRAITS: &[(&str, &str)] = &[
    ("class_scoutred", "hud/class_scoutred"),
    ("class_scoutblue", "hud/class_scoutblue"),
    ("class_sniperred", "hud/class_sniperred"),
    ("class_sniperblue", "hud/class_sniperblue"),
    ("class_soldierred", "hud/class_soldierred"),
    ("class_soldierblue", "hud/class_soldierblue"),
    ("class_demored", "hud/class_demored"),
    ("class_demoblue", "hud/class_demoblue"),
    ("class_medicred", "hud/class_medicred"),
    ("class_medicblue", "hud/class_medicblue"),
    ("class_heavyred", "hud/class_heavyred"),
    ("class_heavyblue", "hud/class_heavyblue"),
    ("class_pyrored", "hud/class_pyrored"),
    ("class_pyroblue", "hud/class_pyroblue"),
    ("class_spyred", "hud/class_spyred"),
    ("class_spyblue", "hud/class_spyblue"),
    ("class_engired", "hud/class_engired"),
    ("class_engiblue", "hud/class_engiblue"),
    ("character_red_bg", "hud/character_red_bg"),
    ("character_blue_bg", "hud/character_blue_bg"),
];

/// The portrait's sprite (`PORTRAITS`) of class `class` (data/classes.toml's
/// name: `demoman`, `engineer`, ...) on RED or BLU (`blu`).
pub fn portrait(class: &str, blu: bool) -> Option<&'static str> {
    let short = match class {
        "demoman" => "demo",
        "engineer" => "engi",
        c => c,
    };
    let team = if blu { "blue" } else { "red" };
    PORTRAITS.iter().map(|(n, _)| *n).find(|n| n.strip_prefix("class_").and_then(|r| r.strip_prefix(short)) == Some(team))
}

/// The portrait's background sprite for RED or BLU.
pub fn portrait_bg(blu: bool) -> &'static str {
    if blu { "character_blue_bg" } else { "character_red_bg" }
}

/// The HUD look from the install (None when its files are missing).
pub fn load(materials: &mut MaterialLoader) -> Option<GameHud> {
    let mut hud = look::load(
        materials,
        &LookOptions { base_icons: &[], layout: keyvalues::parse_pc, font_files: true, first_wins: true },
    )?;
    for (name, material) in PICTURES.iter().chain(PORTRAITS) {
        if let Some(texture) = materials.resolve(material).texture {
            let t = &materials.textures[texture];
            hud.sprites.insert((*name).to_string(), HudSprite { texture, rect: [0.0, 0.0, t.width as f32, t.height as f32] });
        }
    }
    hud.game = super::GAME.to_string();
    Some(hud)
}

/// The kill-icon cells of a `mod_textures.txt` (hud.md 3.6): name ->
/// (file, x, y, w, h).
pub fn icon_cells(text: &str) -> BTreeMap<String, (String, [u32; 4])> {
    use keyvalues::Kv;
    fn find(kv: &Kv) -> Option<&Kv> {
        kv.get("TextureData").or_else(|| kv.items().iter().find_map(|(_, v)| find(v)))
    }
    let parsed = keyvalues::parse_res(text);
    let Some(data) = find(&parsed) else { return BTreeMap::new() };
    data.items()
        .iter()
        .filter_map(|(name, v)| {
            let n = |k: &str| v.str(k)?.trim().parse::<u32>().ok();
            let file = v.str("dfile").or_else(|| v.str("file"))?;
            Some((name.to_lowercase(), (file.to_string(), [n("x")?, n("y")?, n("width")?, n("height")?])))
        })
        .collect()
}

/// The RGBA8 pixels of `w` x `h` at (`x`, `y`) of `image` (None past its
/// edge).
pub fn crop(image: &UiImage, x: u32, y: u32, w: u32, h: u32) -> Option<Vec<u8>> {
    if x + w > image.width || y + h > image.height {
        return None;
    }
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = ((row * image.width + x) * 4) as usize;
        out.extend_from_slice(&image.rgba8[start..start + (w * 4) as usize]);
    }
    Some(out)
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

/// TF2's guns' kill icons by gun id (each gun's `hud.icon` cell of
/// `scripts/mod_textures.txt`, cut from its sheet, `materials/<file>.vtf`
/// or its `.vmt`'s `$basetexture`): None while a file they need is named
/// by the mount but not here (the asset store before TF2's `players`
/// arrived), or when `mod_textures.txt` isn't in the mount at all (TF2's
/// files may come later); empty when the files don't make any. Reads
/// only what is here: never fetches.
pub fn kill_icon_pictures(mount: &Mount) -> Option<BTreeMap<String, UiImage>> {
    let text = here(mount, "scripts/mod_textures.txt")??;
    let cells = icon_cells(&String::from_utf8_lossy(&text));
    let mut sheets: BTreeMap<String, Option<UiImage>> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for w in super::weapons::tf2_weapons().weapons {
        let Some(icon) = w.hud.icon.as_deref() else { continue };
        let Some((file, [x, y, cw, ch])) = cells.get(&icon.to_lowercase()) else { continue };
        let file = file.to_lowercase();
        if !sheets.contains_key(&file) {
            let vtf = match here(mount, &format!("materials/{file}.vtf"))? {
                Some(v) => Some(v),
                None => match here(mount, &format!("materials/{file}.vmt"))? {
                    None => None,
                    Some(vmt) => {
                        let base = keyvalues::parse_res(&String::from_utf8_lossy(&vmt))
                            .items()
                            .first()
                            .and_then(|(_, b)| b.str("$basetexture").map(str::to_string));
                        match base {
                            Some(b) => here(mount, &format!("materials/{}.vtf", b.trim_end_matches(".vtf")))?,
                            None => None,
                        }
                    }
                },
            };
            sheets.insert(file.clone(), vtf.and_then(|v| source_engine::hud::gameui::decode_vtf(&v)));
        }
        let Some(sheet) = sheets[&file].as_ref() else { continue };
        let Some(rgba8) = crop(sheet, *x, *y, *cw, *ch) else { continue };
        out.insert(w.id, UiImage { width: *cw, height: *ch, rgba8 });
    }
    Some(out)
}
