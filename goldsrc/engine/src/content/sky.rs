//! The sky box (specs/goldsrc/mount.md, "Where things live"; bsp.md,
//! "Entities"): `worldspawn`'s `skyname` (`desert` when absent), six TGA
//! faces `gfx/env/<skyname><face>.tga` looked up ignoring case
//! (de_dust2's are `Des*.tga`), as the map's sky cube. The faces are
//! named and laid out as Source's sky faces are (`rt`, `lf`, `up`, `dn`,
//! `bk`, `ft`), so they take the same places and turns on the engine's
//! cube as Source's do.
//!
//! Known shortcuts:
//! - A sky with a face missing is not drawn at all (the original draws
//!   that face black; none is missing in the install).

use crate::{
    map::{MapData, MapSky, MapTexture, MapUvTransform},
    mount::Mount,
};

/// Each face's suffix, its engine cube face (+X, −X, +Y, −Y, +Z, −Z) and
/// its quarter turns, as Source's.
const FACES: [(&str, usize, u8); 6] = [("rt", 0, 0), ("lf", 1, 0), ("up", 2, 3), ("dn", 3, 1), ("bk", 4, 0), ("ft", 5, 0)];

/// The sky named `skyname` from `mount` into `data` (its textures appended).
pub fn add(mount: &Mount, skyname: &str, decode: bool, data: &mut MapData) {
    let mut faces = [(0usize, 0u8); 6];
    for (suffix, face, turns) in FACES {
        let path = format!("gfx/env/{skyname}{suffix}.tga");
        let bytes = match mount.read(&path) {
            Ok(b) => b,
            Err(e) => {
                data.warnings.push(format!("sky {path}: {e}"));
                return;
            }
        };
        let texture = match image::load_from_memory_with_format(&bytes, image::ImageFormat::Tga) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                MapTexture {
                    name: path.clone(),
                    srgb: true,
                    mips: Vec::new(),
                    width: rgba.width(),
                    height: rgba.height(),
                    rgba8: if decode { rgba.into_raw() } else { Vec::new() },
                    bc: None,
                }
            }
            Err(e) => {
                data.warnings.push(format!("sky {path}: {e}"));
                return;
            }
        };
        data.textures.push(texture);
        faces[face] = (data.textures.len() - 1, turns);
    }
    data.sky = Some(MapSky {
        faces,
        transforms: [MapUvTransform::IDENTITY; 6],
        hdr: None,
    });
}
