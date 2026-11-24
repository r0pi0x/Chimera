//! A map's textures (specs/goldsrc/bsp.md, "Textures lump", "Texture name
//! prefixes"; wad3.md; mount.md, "WADs named by a map"): each texture of
//! the map's lump, embedded or looked up by name (ignoring case) in the
//! WADs its `worldspawn` `wad` key lists, in the key's order, each found
//! by its file name at the root of the mount's folders (`Wads`). What a
//! name's prefix means for drawing and collision is `Kind`.
//!
//! Texture families (bsp.md, "Texture name prefixes"; `Families`): the
//! names that differ only in a `+` or `-` prefix's character belong
//! together. `+0name`..`+9name` are an animation's frames and
//! `+aname`..`+jname` its alternate set's, each played in its character's
//! order at `FRAME_RATE` (frames a set lacks are skipped); a brush entity
//! shows the alternate set while toggled on. `-0name`..`-9name` are
//! random-tiling variants: each face shows one (`Families::variant`),
//! picked by a fixed hash of the face's index so a map converts the same
//! each time. A family's members are looked up in the map's own texture
//! lump (the compiler stores every frame and variant there; checked on
//! the stock maps), so using one texture of a family loads all of them.
//!
//! Liquids (`!`) are drawn with a sine warp of their texture coordinates
//! (`warp`, Chimera's `MapUvWarp`).
//!
//! Known shortcuts:
//! - The warp's size, wavelength and speed are our own numbers
//!   (`WARP_AMPLITUDE`, `WARP_WAVELENGTH`, `WARP_PERIOD`): bsp.md says only
//!   "a sine warp, turbulent" (docs/plans/active/goldsrc.md, bsp.md's open
//!   questions). The frame rate (`FRAME_RATE`) is the spec's.
//! - Random tiling picks a variant per face, not per texture tile of a
//!   face (the spec allows either; which one the original does is not
//!   pinned down), with our own hash.
//! - The four stored mip levels are not handed over: the renderer makes
//!   its own.

use std::collections::BTreeMap;

use goldsrc_formats::{miptex::MipTex, wad::Wad};

use crate::{
    map::{MapTexture, MapUvWarp},
    mount::Mount,
};

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

/// The frames a second of an animated texture (`+0`..`+9`, `+a`..`+j`)
/// shows (bsp.md, "Texture name prefixes").
pub const FRAME_RATE: f32 = 10.0;

/// How far a liquid's warp moves its texels (our number, module doc).
pub const WARP_AMPLITUDE: f32 = 8.0;
/// The warp's wavelength in texels (our number).
pub const WARP_WAVELENGTH: f32 = 64.0;
/// Seconds for one wave of the warp (our number).
pub const WARP_PERIOD: f32 = 4.0;

/// The warp of a liquid texture of `width` x `height` texels, in its
/// coordinates' repeats.
pub fn warp(width: u32, height: u32) -> MapUvWarp {
    let (w, h) = (width.max(1) as f32, height.max(1) as f32);
    let k = std::f32::consts::TAU / WARP_WAVELENGTH;
    MapUvWarp {
        amplitude: [WARP_AMPLITUDE / w, WARP_AMPLITUDE / h],
        frequency: [k * w, k * h],
        speed: std::f32::consts::TAU / WARP_PERIOD,
    }
}

/// What a texture name's `+` or `-` prefix says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prefix {
    /// `+0`..`+9` (`alternate` false) or `+a`..`+j` (true): frame `n` of
    /// the animation of `base` (lower case).
    Frame { alternate: bool, n: u8, base: String },
    /// `-0`..`-9`: random-tiling variant `n` of `base` (lower case).
    Tile { n: u8, base: String },
    /// Neither.
    None,
}

/// The prefix of texture `name`.
pub fn prefix(name: &str) -> Prefix {
    let n = name.to_ascii_lowercase();
    let b = n.as_bytes();
    if b.len() < 3 {
        return Prefix::None;
    }
    let base = n[2..].to_string();
    match (b[0], b[1]) {
        (b'+', c @ b'0'..=b'9') => Prefix::Frame { alternate: false, n: c - b'0', base },
        (b'+', c @ b'a'..=b'j') => Prefix::Frame { alternate: true, n: c - b'a', base },
        (b'-', c @ b'0'..=b'9') => Prefix::Tile { n: c - b'0', base },
        _ => Prefix::None,
    }
}

/// A map's texture families (module doc), by index into its texture
/// lump.
#[derive(Clone, Debug, Default)]
pub struct Families {
    /// Per texture: its animation's frames and its alternate set's (lump
    /// indices in frame order; empty when it is no frame, or a set is
    /// missing).
    pub frames: Vec<(Vec<usize>, Vec<usize>)>,
    /// Per texture: its random-tiling variants in order, itself among
    /// them (empty: not a variant).
    pub tiles: Vec<Vec<usize>>,
}

impl Families {
    /// The families of a lump whose textures are named `names` (None: a
    /// missing entry).
    pub fn of(names: &[Option<&str>]) -> Families {
        let mut anims: BTreeMap<String, [BTreeMap<u8, usize>; 2]> = BTreeMap::new();
        let mut tiles: BTreeMap<String, BTreeMap<u8, usize>> = BTreeMap::new();
        for (i, name) in names.iter().enumerate() {
            match name.map(prefix) {
                Some(Prefix::Frame { alternate, n, base }) => {
                    anims.entry(base).or_default()[alternate as usize].entry(n).or_insert(i);
                }
                Some(Prefix::Tile { n, base }) => {
                    tiles.entry(base).or_default().entry(n).or_insert(i);
                }
                _ => {}
            }
        }
        let mut out = Families {
            frames: vec![Default::default(); names.len()],
            tiles: vec![Vec::new(); names.len()],
        };
        for sets in anims.values() {
            let set = |k: usize| sets[k].values().copied().collect::<Vec<usize>>();
            let (main, alternate) = (set(0), set(1));
            for &i in main.iter().chain(&alternate) {
                out.frames[i] = (main.clone(), alternate.clone());
            }
        }
        for set in tiles.values() {
            let set: Vec<usize> = set.values().copied().collect();
            for &i in &set {
                out.tiles[i] = set.clone();
            }
        }
        out
    }

    /// The textures drawing texture `t` needs: itself, its animations'
    /// frames, its tiling variants.
    pub fn members(&self, t: usize) -> Vec<usize> {
        let mut v = vec![t];
        if let Some((a, b)) = self.frames.get(t) {
            v.extend(a.iter().chain(b));
        }
        if let Some(tiles) = self.tiles.get(t) {
            v.extend(tiles);
        }
        v.sort_unstable();
        v.dedup();
        v
    }

    /// The texture face `face` (its index in the map) shows for texture
    /// `t`: one of `t`'s tiling variants, picked by a fixed hash of the
    /// face's index; `t` itself when it has none.
    pub fn variant(&self, t: usize, face: usize) -> usize {
        match self.tiles.get(t) {
            Some(set) if set.len() > 1 => set[(mix(face as u64) % set.len() as u64) as usize],
            _ => t,
        }
    }

    /// The first frames of texture `t`'s animation and of its alternate
    /// set, when it has both (what a toggled brush entity switches
    /// between).
    pub fn toggle(&self, t: usize) -> Option<(usize, usize)> {
        let (a, b) = self.frames.get(t)?;
        Some((*a.first()?, *b.first()?))
    }
}

/// A fixed 64-bit mix (SplitMix64's finaliser): the same everywhere, so
/// a random pick converts the same each time.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
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

    /// A liquid's warp moves its texels by `WARP_AMPLITUDE`, waving once
    /// every `WARP_WAVELENGTH` texels of the other axis.
    #[test]
    fn liquid_warp_in_texels() {
        let w = warp(128, 64);
        assert!((w.amplitude[0] * 128.0 - WARP_AMPLITUDE).abs() < 1e-5);
        assert!((w.amplitude[1] * 64.0 - WARP_AMPLITUDE).abs() < 1e-5);
        // One wavelength along v is WARP_WAVELENGTH texels.
        let v = WARP_WAVELENGTH / 64.0;
        assert!((w.frequency[1] * v - std::f32::consts::TAU).abs() < 1e-4);
        assert!((w.speed * WARP_PERIOD - std::f32::consts::TAU).abs() < 1e-5);
    }

    #[test]
    fn prefixes_name_frames_and_tiles() {
        assert_eq!(prefix("+0~fifties_lgt2"), Prefix::Frame { alternate: false, n: 0, base: "~fifties_lgt2".into() });
        assert_eq!(prefix("+A~sp_lock"), Prefix::Frame { alternate: true, n: 0, base: "~sp_lock".into() });
        assert_eq!(prefix("+j~x"), Prefix::Frame { alternate: true, n: 9, base: "~x".into() });
        assert_eq!(prefix("-2SandRock2"), Prefix::Tile { n: 2, base: "sandrock2".into() });
        assert_eq!(prefix("+k~x"), Prefix::None);
        assert_eq!(prefix("!waterblue"), Prefix::None);
        assert_eq!(prefix("+0"), Prefix::None);
    }

    /// cs_office's `~lab_crt2` (four frames, listed out of order, no
    /// alternate set), `~elev2_pan` with both sets, and de_dust2's three
    /// `SandRock2` variants.
    #[test]
    fn families_group_frames_in_order_and_variants() {
        let names = [
            "+2~lab_crt2",
            "+0~lab_crt2",
            "+a~elev2_pan",
            "+3~lab_crt2",
            "+1~lab_crt2",
            "+0~elev2_pan",
            "-1SandRock2",
            "-0sandrock2",
            "plain",
            "+1~ELEV2_PAN",
            "-2SandRock2",
        ];
        let lump: Vec<Option<&str>> = names.iter().map(|n| Some(*n)).collect();
        let f = Families::of(&lump);
        assert_eq!(f.frames[3], (vec![1, 4, 0, 3], vec![]));
        assert_eq!(f.frames[2], (vec![5, 9], vec![2]));
        assert_eq!(f.toggle(9), Some((5, 2)));
        assert_eq!(f.toggle(0), None);
        assert_eq!(f.tiles[6], vec![7, 6, 10]);
        assert_eq!(f.members(8), vec![8]);
        assert_eq!(f.members(0), vec![0, 1, 3, 4]);
        assert_eq!(f.members(6), vec![6, 7, 10]);
        // Every face picks a variant, the same each time, and all show.
        let picks: Vec<usize> = (0..300).map(|face| f.variant(6, face)).collect();
        assert_eq!(picks, (0..300).map(|face| f.variant(7, face)).collect::<Vec<_>>());
        for v in [6, 7, 10] {
            let n = picks.iter().filter(|&&p| p == v).count();
            assert!(n > 60, "variant {v}: {n} of 300");
        }
        assert_eq!(f.variant(8, 5), 8);
    }
}
