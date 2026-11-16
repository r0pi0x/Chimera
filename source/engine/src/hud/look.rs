//! A Source game's HUD look from its install (`GameHud`): the named and
//! base colours of the client scheme (`resource/clientscheme.res`), the
//! font files its `CustomFontFiles` list (by family name, and with
//! `LookOptions::font_files` by file name, `file:<stem>`, and `family:<name>` too) and each scheme font's
//! first size, every size entry as `text_fonts`, icons drawn with font
//! glyphs or cut from sprite sheets (`scripts/hud_textures.txt`,
//! `scripts/mod_textures.txt`; a sheet as `file`, or TF2's `dfile` with its
//! negative `dnegfile` as `<name>/neg`), and each panel of `scripts/hudlayout.res`
//! (position, size, background and offsets, every key as written).
//! `glyph_icons` reads only the icon script's glyphs and their fonts (an
//! app's kill icons at start, with no map or HUD).
//!
//! Moved from CS:S's `game::hud` (the shared part; CS:S's scopes, scoreboard
//! pictures, screens and menus stay its own).

use std::sync::Arc;

use bevy::{platform::collections::HashMap, prelude::*};
use chimera_world::map::hud::{GameHud, HudCoord, HudFont, HudPanel, HudSprite, UiFontSize};
use source_formats::keyvalues::Kv;

use crate::content::material::MaterialLoader;

/// RGB or RGBA written as numbers (`255 220 0 100`).
pub fn colour(text: &str) -> Option<[u8; 4]> {
    let v: Vec<u8> = text.split_whitespace().filter_map(|x| x.parse().ok()).collect();
    match v.as_slice() {
        [r, g, b] => Some([*r, *g, *b, 255]),
        [r, g, b, a] => Some([*r, *g, *b, *a]),
        _ => None,
    }
}

/// A TrueType file's family name (name table, ID 1).
pub fn family_name(ttf: &[u8]) -> Option<String> {
    let u16_at = |o: usize| ttf.get(o..o + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let u32_at = |o: usize| ttf.get(o..o + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let tables = u16_at(4)? as usize;
    let name = (0..tables)
        .map(|i| 12 + i * 16)
        .find(|&o| ttf.get(o..o + 4) == Some(b"name"))
        .and_then(|o| u32_at(o + 8))? as usize;
    let count = u16_at(name + 2)? as usize;
    let strings = name + u16_at(name + 4)? as usize;
    let mut best: Option<String> = None;
    for r in 0..count {
        let rec = name + 6 + r * 12;
        let (platform, id) = (u16_at(rec)?, u16_at(rec + 6)?);
        if id != 1 {
            continue;
        }
        let (len, off) = (u16_at(rec + 8)? as usize, u16_at(rec + 10)? as usize);
        let bytes = ttf.get(strings + off..strings + off + len)?;
        let text = if platform == 0 || platform == 3 {
            let units: Vec<u16> = bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect();
            String::from_utf16_lossy(&units)
        } else {
            bytes.iter().map(|&b| b as char).collect()
        };
        if best.is_none() || platform == 3 {
            best = Some(text);
        }
    }
    best
}

/// The scheme's fonts: every size entry of every font, in order.
pub fn scheme_fonts(scheme: &Kv) -> Vec<(String, Vec<UiFontSize>)> {
    let mut out = Vec::new();
    for (name, f) in scheme.get("Fonts").map(Kv::items).unwrap_or_default() {
        let sizes: Vec<UiFontSize> = f
            .items()
            .iter()
            .filter_map(|(_, e)| {
                let yres = e.str("yres").and_then(|y| {
                    let v: Vec<u32> = y.split_whitespace().filter_map(|n| n.parse().ok()).collect();
                    (v.len() == 2).then(|| (v[0], v[1]))
                });
                Some(UiFontSize {
                    family: e.str("name")?.to_string(),
                    tall: e.str("tall")?.trim().parse().ok()?,
                    weight: e.str("weight").and_then(|w| w.trim().parse().ok()).unwrap_or(400),
                    yres,
                    antialias: e.str("antialias").is_some_and(|a| a.trim() != "0"),
                    additive: e.str("additive").is_some_and(|a| a.trim() != "0"),
                })
            })
            .collect();
        if !sizes.is_empty() {
            out.push((name.clone(), sizes));
        }
    }
    out
}

/// What a game's look takes from the shared reader.
pub struct LookOptions<'a> {
    /// The base HUD's icons (`scripts/hud_textures.txt`) the game draws;
    /// the rest of that file is another game's. The game's own file
    /// (`scripts/mod_textures.txt`) is read whole.
    pub base_icons: &'a [&'a str],
    /// Reads the layout file (`parse_res`: every conditional entry kept,
    /// the first found; `parse_pc`: those that hold on the Windows PC).
    pub layout: fn(&str) -> Kv,
    /// Also keep each font file under `file:<lower-case file stem>` in
    /// `GameHud::fonts` (a game that picks a face by weight).
    pub font_files: bool,
    /// A `BaseSettings` name listed twice keeps its first value (the last,
    /// CS:S's reading, otherwise).
    pub first_wins: bool,
}

/// The first block under the file's root key (`"Layout" { ... }`).
fn root(kv: Kv) -> Kv {
    kv.items().first().map(|(_, v)| v.clone()).unwrap_or(kv)
}

/// The scheme's font files (`CustomFontFiles`, by family name; with
/// `font_files` also by file and family name) and each scheme font's first
/// size, into `hud.fonts`; `read` reads a file of the game's search path.
fn scheme_hud_fonts(hud: &mut GameHud, scheme: &Kv, read: &dyn Fn(&str) -> Option<Vec<u8>>, font_files: bool) {
    let mut files: HashMap<String, Arc<Vec<u8>>> = HashMap::new();
    for (_, entry) in scheme.get("CustomFontFiles").map(Kv::items).unwrap_or_default() {
        // A file as a value, or a block naming it (`"font" "resource/tf.ttf"`).
        let path = match entry {
            Kv::Value(path) => path.as_str(),
            Kv::Block(_) => match entry.str("font") {
                Some(p) => p,
                None => continue,
            },
        };
        let Some(bytes) = read(path) else { continue };
        let bytes = Arc::new(bytes);
        if font_files {
            let stem = path.rsplit(['/', '\\']).next().unwrap_or(path);
            let stem = stem.rsplit_once('.').map_or(stem, |(s, _)| s).to_lowercase();
            hud.fonts.insert(format!("file:{stem}"), HudFont { data: bytes.clone(), tall: 0.0, additive: false });
        }
        if let Some(family) = family_name(&bytes) {
            if font_files {
                hud.fonts
                    .entry(format!("family:{}", family.to_lowercase()))
                    .or_insert(HudFont { data: bytes.clone(), tall: 0.0, additive: false });
            }
            files.entry(family.to_lowercase()).or_insert(bytes);
        }
    }
    for (name, f) in scheme.get("Fonts").map(Kv::items).unwrap_or_default() {
        // The first size entry (the 480-line one).
        let Some((_, first)) = f.items().first() else { continue };
        let (Some(family), Some(tall)) = (first.str("name"), first.str("tall").and_then(|t| t.parse().ok())) else {
            continue;
        };
        if let Some(data) = files.get(&family.to_lowercase()) {
            hud.fonts.insert(
                name.clone(),
                HudFont { data: data.clone(), tall, additive: first.str("additive").is_some_and(|a| a.trim() != "0") },
            );
        }
    }
}

/// An icon script's `TextureData` block (`scripts/mod_textures.txt`).
fn texture_data(text: &str) -> Kv {
    // A glyph that is a brace (`"character" "{"`) would open a block.
    let t = source_formats::keyvalues::parse_res(&text.replace("\"{\"", "\"<open>\"").replace("\"}\"", "\"<close>\""));
    // Entries sit in a "TextureData" block (under the file's root key).
    fn find(kv: &Kv) -> Option<&Kv> {
        kv.get("TextureData").or_else(|| kv.items().iter().find_map(|(_, v)| find(v)))
    }
    find(&t).cloned().unwrap_or(t)
}

/// An icon entry's font and glyph, when it is drawn with one.
fn glyph_of(v: &Kv) -> Option<(String, char)> {
    let ch = match v.str("character")? {
        "<open>" => '{',
        "<close>" => '}',
        c => c.chars().next()?,
    };
    Some((v.str("font")?.to_string(), ch))
}

/// Only the glyph icons of the game's own icon script
/// (`scripts/mod_textures.txt`: its guns' death notices) and the scheme's
/// fonts they are drawn in, as a `GameHud` holding just those: what an app
/// registers its kill icons from at start, without a map or the HUD
/// (`read` reads a file of the game's search path). None when the scheme
/// or the script doesn't read.
pub fn glyph_icons(read: &dyn Fn(&str) -> Option<Vec<u8>>) -> Option<GameHud> {
    let text = |p: &str| read(p).map(|b| String::from_utf8_lossy(&b).into_owned());
    let scheme = root(source_formats::keyvalues::parse_pc(&text("resource/clientscheme.res")?));
    let mut hud = GameHud::default();
    scheme_hud_fonts(&mut hud, &scheme, read, false);
    for (name, v) in texture_data(&text("scripts/mod_textures.txt")?).items() {
        if let Some(g) = glyph_of(v) {
            hud.icons.insert(name.clone(), g);
        }
    }
    let used: Vec<String> = hud.icons.values().map(|(f, _)| f.clone()).collect();
    hud.fonts.retain(|name, _| used.contains(name));
    Some(hud)
}

/// The HUD look from the install (None when its files are missing).
pub fn load(materials: &mut MaterialLoader, opts: &LookOptions) -> Option<GameHud> {
    let text = |m: &MaterialLoader, p: &str| m.read(p).map(|b| String::from_utf8_lossy(&b).into_owned());
    // As the Windows game reads it: its platform-conditional entries
    // (font names and sizes) are the Windows ones.
    let scheme = root(source_formats::keyvalues::parse_pc(&text(materials, "resource/clientscheme.res")?));
    let layout = root((opts.layout)(&text(materials, "scripts/hudlayout.res")?));
    let mut hud = GameHud::default();

    // Colours: named ones, then base settings that refer to them by name.
    if let Some(c) = scheme.get("Colors") {
        for (k, v) in c.items() {
            if let Kv::Value(v) = v
                && let Some(rgba) = colour(v)
            {
                hud.colors.insert(k.clone(), rgba);
            }
        }
    }
    if let Some(b) = scheme.get("BaseSettings") {
        let mut seen = bevy::platform::collections::HashSet::new();
        for (k, v) in b.items() {
            if let Kv::Value(v) = v {
                let rgba = colour(v).or_else(|| hud.colors.get(v).copied());
                // A name listed twice keeps its first value when asked.
                if let Some(rgba) = rgba
                    && (!opts.first_wins || seen.insert(k.clone()))
                {
                    hud.colors.insert(k.clone(), rgba);
                }
            }
        }
    }

    scheme_hud_fonts(&mut hud, &scheme, &|p| materials.read(p), opts.font_files);

    // Icons drawn with font glyphs or cut from sprite sheets: the base
    // HUD's, then the game's own over them.
    for script in ["scripts/hud_textures.txt", "scripts/mod_textures.txt"] {
        let Some(t) = text(materials, script) else { continue };
        let base = script.starts_with("scripts/hud_");
        for (name, v) in texture_data(&t).items() {
            if base && !opts.base_icons.iter().any(|k| name.eq_ignore_ascii_case(k)) {
                continue;
            }
            let num = |v: &Kv, k: &str| v.str(k).and_then(|x| x.trim().parse::<f32>().ok());
            if let Some((font, ch)) = glyph_of(v) {
                hud.icons.insert(name.clone(), (font, ch));
            } else if let Some(file) = v.str("file").or_else(|| v.str("dfile")) {
                // TF2's kill icons name their sheet `dfile`, and the sheet
                // for a row with the local player `dnegfile` (kept as
                // `<name>/neg`).
                let rect = [num(v, "x"), num(v, "y"), num(v, "width"), num(v, "height")];
                if let [Some(x), Some(y), Some(w), Some(h)] = rect {
                    if let Some(texture) = materials.resolve(file).texture {
                        hud.sprites.insert(name.clone(), HudSprite { texture, rect: [x, y, w, h] });
                    }
                    if let Some(texture) = v.str("dnegfile").and_then(|f| materials.resolve(f).texture) {
                        hud.sprites.insert(format!("{name}/neg"), HudSprite { texture, rect: [x, y, w, h] });
                    }
                }
            }
        }
    }

    // Panels.
    let num = |p: &Kv, k: &str| p.str(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
    for (name, p) in layout.items() {
        // A panel needs a position; one it leaves out is 0 (VGUI's
        // default: `HudHistoryResource` gives no `ypos`).
        let (x, y) = (p.str("xpos").and_then(HudCoord::parse), p.str("ypos").and_then(HudCoord::parse));
        // The damage indicator places itself by its own keys (`dmg_xpos`).
        if x.is_none() && y.is_none() && p.str("dmg_xpos").is_none() {
            continue;
        }
        let (x, y) = (x.unwrap_or(HudCoord::Start(0.0)), y.unwrap_or(HudCoord::Start(0.0)));
        hud.panels.insert(
            name.clone(),
            HudPanel {
                x,
                y,
                wide: num(p, "wide"),
                tall: num(p, "tall"),
                background: p.str("bgcolor_override").and_then(colour),
                icon: Vec2::new(num(p, "icon_xpos"), num(p, "icon_ypos")),
                digit: Vec2::new(num(p, "digit_xpos"), num(p, "digit_ypos")),
                digit2: Vec2::new(num(p, "digit2_xpos"), num(p, "digit2_ypos")),
                keys: p
                    .items()
                    .iter()
                    .filter_map(|(k, v)| match v {
                        Kv::Value(v) => Some((k.to_lowercase(), v.clone())),
                        Kv::Block(_) => None,
                    })
                    .collect(),
            },
        );
    }
    hud.text_fonts = scheme_fonts(&scheme).into_iter().collect();
    Some(hud)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_read_rgb_and_rgba() {
        assert_eq!(colour("255 220 0 100"), Some([255, 220, 0, 100]));
        assert_eq!(colour("1 2 3"), Some([1, 2, 3, 255]));
        assert_eq!(colour("Orange"), None);
    }
}
