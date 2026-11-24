//! A map's textures (specs/goldsrc/bsp.md, "Textures lump", "Texture name
//! prefixes"; wad3.md; mount.md, "WADs named by a map"): each texture of
//! the map's lump, embedded or looked up by name (ignoring case) in the
//! WADs its `worldspawn` `wad` key lists, in the key's order, each found
//! by its file name at the root of the mount's folders (`Wads`). What a
//! name's prefix means for drawing and collision is `Kind`.
//!
//! Known shortcuts:
//! - Animated (`+0`..`+9`, `+a`..`+j`) and random-tiling (`-0`..`-9`)
//!   textures show their first frame.
//! - The four stored mip levels are not handed over: the renderer makes
//!   its own.

use goldsrc_formats::{miptex::MipTex, wad::Wad};

use crate::{map::MapTexture, mount::Mount};

/// What a texture name says (bsp.md, "Texture name prefixes").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Drawn and lit as it is.
    Normal,
    /// `{`: index 255 is a hole (alpha-tested).
    Masked,
    /// `!`: a liquid surface.
    Water,
    /// `sky`: not drawn; the sky box shows through.
    Sky,
    /// `aaatrigger`, `clip`, `origin`, `null` and other tool textures: not
    /// drawn.
    Tool,
}

/// The kind of texture `name`.
pub fn kind(name: &str) -> Kind {
    let n = name.to_ascii_lowercase();
    if n == "sky" {
        Kind::Sky
    } else if matches!(n.as_str(), "aaatrigger" | "clip" | "origin" | "null" | "bevel" | "hint" | "skip") {
        Kind::Tool
    } else if n.starts_with('{') {
        Kind::Masked
    } else if n.starts_with('!') {
        Kind::Water
    } else {
        Kind::Normal
    }
}

/// The WADs a map names, opened in its key's order (those the mount
/// lacks skipped: normal for maps that embed what they used).
#[derive(Default)]
pub struct Wads {
    pub list: Vec<(String, Wad)>,
}

impl Wads {
    /// The WADs `names` lists (file names) from `mount`.
    pub fn open(mount: &Mount, names: &[String], warnings: &mut Vec<String>) -> Wads {
        let mut list = Vec::new();
        for n in names {
            let Ok(bytes) = mount.read(n) else { continue };
            match Wad::parse(bytes) {
                Ok(w) => list.push((n.clone(), w)),
                Err(e) => warnings.push(format!("{n}: {e}")),
            }
        }
        Wads { list }
    }

    /// The mip texture named `name` in the first WAD that holds it.
    pub fn miptex(&self, name: &str) -> Option<MipTex> {
        self.list
            .iter()
            .find_map(|(_, w)| w.miptex(name).and_then(Result::ok))
    }
}

/// The pixels of texture `tex` (the map's record): its own when
/// embedded, else the WADs'. None: found nowhere.
pub fn resolve(tex: &MipTex, wads: &Wads) -> Option<MipTex> {
    if tex.embedded() {
        return Some(tex.clone());
    }
    wads.miptex(&tex.name)
}

/// `tex` as a map texture (sRGB RGBA8, level 0), or with `decode` off a
/// placeholder of its size.
pub fn map_texture(tex: &MipTex, decode: bool) -> MapTexture {
    let rgba8 = if decode { tex.rgba(0).unwrap_or_default() } else { Vec::new() };
    MapTexture {
        name: tex.name.to_ascii_lowercase(),
        srgb: true,
        mips: Vec::new(),
        width: tex.width,
        height: tex.height,
        rgba8,
        bc: None,
    }
}

/// A flat placeholder of `width` x `height` for a texture found nowhere.
pub fn placeholder(name: &str, width: u32, height: u32, decode: bool) -> MapTexture {
    let (w, h) = (width.max(1), height.max(1));
    let rgba8 = if decode {
        (0..w * h)
            .flat_map(|i| if ((i % w) / 8 + (i / w) / 8) % 2 == 0 { [255, 0, 255, 255] } else { [0, 0, 0, 255] })
            .collect()
    } else {
        Vec::new()
    };
    MapTexture {
        name: name.to_ascii_lowercase(),
        srgb: true,
        mips: Vec::new(),
        width: w,
        height: h,
        rgba8,
        bc: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefixes_name_the_kind() {
        assert_eq!(kind("SKY"), Kind::Sky);
        assert_eq!(kind("AAATRIGGER"), Kind::Tool);
        assert_eq!(kind("{pi_treeb"), Kind::Masked);
        assert_eq!(kind("!waterblue"), Kind::Water);
        assert_eq!(kind("-0SandRock2"), Kind::Normal);
        assert_eq!(kind("+0~fifties_lgt2"), Kind::Normal);
    }
}
