//! Source HUDs' picture sheets: the cells a `TextureData` block names
//! (`scripts/mod_textures.txt`, a weapon script's `TextureData`: `file`
//! or `dfile`, `x`, `y`, `width`, `height` in texels of
//! `materials/<file>.vtf`), cut from their sheets (`Sheets`), and whole
//! VGUI materials' pictures (`material_picture`), read from a mount only
//! once their files are here (`here`: a mount that names a file it hasn't
//! fetched yet gives None, so a caller tries again later rather than
//! fetching). DoD:S's HUD and kill icons (`dods::game::hud`) read them.
//!
//! Known shortcuts:
//! - TF2's kill icons (`tf2::game::hud`) keep their own copy of these
//!   helpers: moving them changes TF2's converter version (its `hud.rs` is
//!   among what shapes the converted output), so it moves with its next
//!   change there.

use std::collections::BTreeMap;

use chimera_core::mount::Mount;
use chimera_world::map::hud::UiImage;
use source_formats::keyvalues::{self, Kv};

/// The cells of a `TextureData` block in `text` (the first anywhere in
/// it), by lower-case name: (sheet file, `[x, y, width, height]`). Glyph
/// entries (`font`, `character`) have no cell.
pub fn icon_cells(text: &str) -> BTreeMap<String, (String, [u32; 4])> {
    fn find(kv: &Kv) -> Option<&Kv> {
        kv.get("TextureData").or_else(|| kv.items().iter().find_map(|(_, v)| find(v)))
    }
    let parsed = keyvalues::parse_res(text);
    find(&parsed).map(block_cells).unwrap_or_default()
}

/// The cells of `block`'s entries (as `icon_cells`).
pub fn block_cells(block: &Kv) -> BTreeMap<String, (String, [u32; 4])> {
    block
        .items()
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
pub fn here(mount: &Mount, path: &str) -> Option<Option<Vec<u8>>> {
    if !mount.contains(path) {
        return Some(None);
    }
    mount.held(path).then(|| mount.read(path).ok())
}

/// The picture of material `name` (under `materials/`, no extension):
/// its `.vtf`, else its `.vmt`'s `$basetexture` (back slashes read as
/// forward). `Some(None)`: the mount lacks it; None: not here yet.
pub fn material_picture(mount: &Mount, name: &str) -> Option<Option<UiImage>> {
    let name = name.replace('\\', "/").to_lowercase();
    let name = name.trim_end_matches(".vtf").trim_end_matches(".vmt");
    let vtf = match here(mount, &format!("materials/{name}.vtf"))? {
        Some(v) => Some(v),
        None => match here(mount, &format!("materials/{name}.vmt"))? {
            None => None,
            Some(vmt) => {
                let base = keyvalues::parse_res(&String::from_utf8_lossy(&vmt))
                    .items()
                    .first()
                    .and_then(|(_, b)| b.str("$basetexture").map(|s| s.replace('\\', "/").to_lowercase()));
                match base {
                    Some(b) if b.trim_end_matches(".vtf") != name => {
                        here(mount, &format!("materials/{}.vtf", b.trim_end_matches(".vtf")))?
                    }
                    _ => None,
                }
            }
        },
    };
    Some(vtf.and_then(|v| super::gameui::decode_vtf(&v)))
}

/// Sheets read once each, cells cut from them.
#[derive(Default)]
pub struct Sheets(BTreeMap<String, Option<UiImage>>);

impl Sheets {
    /// The cell `[x, y, w, h]` of sheet `file` (`material_picture`'s
    /// name): `Some(None)` when the sheet is missing or the cell is past
    /// its edge; None while the sheet isn't here yet.
    pub fn cut(&mut self, mount: &Mount, file: &str, [x, y, w, h]: [u32; 4]) -> Option<Option<UiImage>> {
        let key = file.replace('\\', "/").to_lowercase();
        if !self.0.contains_key(&key) {
            let picture = material_picture(mount, &key)?;
            self.0.insert(key.clone(), picture);
        }
        Some(self.0[&key].as_ref().and_then(|sheet| Some(UiImage { width: w, height: h, rgba8: crop(sheet, x, y, w, h)? })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cells_read_and_glyphs_have_none() {
        let file = r#""sprites/640_hud"
        {
            TextureData
            {
                "d_garand" { "file" "sprites/hud/death_icons" "x" "326" "y" "96" "width" "186" "height" "39" }
                "scattergun" { "dfile" "hud/d_images" "x" "96" "y" "192" "width" "96" "height" "32" }
                "d_skull_dod" { "font" "ClientTitleFontPlayerIcons" "character" "S" }
            }
        }"#;
        let c = icon_cells(file);
        assert_eq!(c.get("d_garand"), Some(&("sprites/hud/death_icons".to_string(), [326, 96, 186, 39])));
        assert_eq!(c.get("scattergun").map(|x| x.0.as_str()), Some("hud/d_images"));
        assert!(!c.contains_key("d_skull_dod"));
    }

    #[test]
    fn a_cell_is_cut_row_by_row() {
        let image = UiImage { width: 4, height: 2, rgba8: (0..32).collect() };
        assert_eq!(crop(&image, 1, 1, 2, 1), Some((20..28).collect()));
        assert_eq!(crop(&image, 3, 0, 2, 1), None);
    }
}
