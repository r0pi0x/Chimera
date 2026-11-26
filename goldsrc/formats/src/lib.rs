//! GoldSrc's file formats as plain data (the `goldsrc/` loader layer,
//! beside `source/`, `lithtech/` and `blam/`; docs/ARCHITECTURE.md,
//! "Crates"; the plan is docs/plans/active/goldsrc.md): bytes in, Rust
//! values out, no Bevy and no game code. A game crate (`cs16`) converts
//! what these read into Chimera's types. Every reader follows its spec in
//! specs/goldsrc/.
//!
//! - `bsp`: BSP version 30 maps (specs/goldsrc/bsp.md): the lumps and
//!   records, lightmap sizing, visibility, entities, point contents and
//!   hull traces on the four hulls (`bsp::hull`), and the hulls' solid
//!   space as convex regions (`bsp::solids`). The record shapes, the leaf
//!   lookup and the visibility decoding are adapted from `qbsp`
//!   (THIRD_PARTY.md); its Quake palette is not.
//! - `miptex`: the mip texture record BSP and WAD3 share, to RGBA.
//! - `holes`: the colour under alpha-tested images' holes (from their
//!   opaque neighbours, so filtering and mip levels don't bleed the key
//!   colour).
//! - `wad`: WAD3 texture archives (specs/goldsrc/wad3.md): mip textures,
//!   pictures, fonts, decals.
//! - `mdl`: studio models, MDL version 10 (specs/goldsrc/mdl.md): bones,
//!   controllers, hitboxes, sequences and their animation streams, events,
//!   body parts, meshes and triangle commands, textures and skins.
//! - `spr`: sprites, SPR version 2 (specs/goldsrc/spr.md), and the HUD's
//!   sprite lists (`hud.txt`, `weapon_*.txt`).
//! - `liblist`: `liblist.gam` and the content folder chain
//!   (specs/goldsrc/mount.md).
//! - `materials`: `sound/materials.txt`, texture names to surface letters
//!   (specs/goldsrc/mount.md).

pub mod bsp;
pub mod holes;
pub mod liblist;
pub mod materials;
pub mod mdl;
pub mod miptex;
pub mod spr;
pub mod wad;

use std::fmt;

/// A file that could not be read: what was wrong, in words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error(pub String);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Error {}

impl From<chimera_vfs::bytes::Truncated> for Error {
    fn from(t: chimera_vfs::bytes::Truncated) -> Error {
        Error(format!("truncated: {t}"))
    }
}

impl From<Error> for std::io::Error {
    fn from(e: Error) -> std::io::Error {
        std::io::Error::new(std::io::ErrorKind::InvalidData, e.0)
    }
}

/// What every reader here returns.
pub type Result<T> = std::result::Result<T, Error>;

pub(crate) fn err<T>(what: impl Into<String>) -> Result<T> {
    Err(Error(what.into()))
}

/// A fixed-size, NUL-padded name: the text before the first NUL (Latin-1:
/// every byte one character), the rest ignored.
pub fn fixed_name(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    bytes[..end].iter().map(|&b| b as char).collect()
}
