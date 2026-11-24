//! The mip texture record a BSP's texture lump and a WAD3's `0x43` entries
//! share (specs/goldsrc/wad3.md, "Mip textures"; bsp.md, "Textures lump"):
//! a 16-byte name, the size, four mip offsets and, when embedded, the four
//! levels of palette indices and the texture's own 256-colour palette.
//! `MipTex::rgba` converts level 0: index 255 is transparent only in `{`
//! textures; decals (`decal_rgba`) take the palette's last colour as the
//! tint and the index as coverage.

use chimera_vfs::bytes::Le;

use crate::{Result, err, fixed_name};

/// Mip levels stored: full size, ½, ¼, ⅛.
pub const MIP_LEVELS: usize = 4;
/// The index that is a hole in `{` textures.
pub const TRANSPARENT_INDEX: u8 = 255;

/// A mip texture record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MipTex {
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Offsets of the levels relative to the record's start (all 0: not
    /// embedded; the pixels are in a WAD).
    pub mip_offsets: [u32; 4],
    /// The four levels' indices (empty when not embedded).
    pub levels: Vec<Vec<u8>>,
    /// 256 RGB colours (empty when not embedded).
    pub palette: Vec<[u8; 3]>,
    /// The colour count field as stored (256; a few font-like files say
    /// otherwise, ignored).
    pub colour_count: u16,
}

impl MipTex {
    /// Read a record from the start of `rec` (the rest of a lump or a WAD
    /// entry's bytes).
    pub fn parse(rec: &[u8]) -> Result<MipTex> {
        let mut r = Le::new(rec);
        let name = fixed_name(r.take(16)?);
        let (width, height) = (r.u32()?, r.u32()?);
        let mip_offsets = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
        if width == 0 || height == 0 || width > 8192 || height > 8192 {
            return err(format!("mip texture {name}: size {width} x {height}"));
        }
        if mip_offsets == [0; 4] {
            return Ok(MipTex {
                name,
                width,
                height,
                mip_offsets,
                levels: Vec::new(),
                palette: Vec::new(),
                colour_count: 0,
            });
        }
        let mut levels = Vec::with_capacity(MIP_LEVELS);
        let mut end = 0usize;
        for (i, &o) in mip_offsets.iter().enumerate() {
            let len = ((width >> i) * (height >> i)) as usize;
            let level = rec
                .get(o as usize..o as usize + len)
                .ok_or_else(|| crate::Error(format!("mip texture {name}: level {i} at {o} outside the record")))?;
            levels.push(level.to_vec());
            end = end.max(o as usize + len);
        }
        let mut r = Le::new(rec);
        r.pos = end;
        let colour_count = r.u16()?;
        let pal = r.take(768)?;
        let palette = pal.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();
        Ok(MipTex {
            name,
            width,
            height,
            mip_offsets,
            levels,
            palette,
            colour_count,
        })
    }

    /// Whether the pixels are in the record (else in a WAD, by name).
    pub fn embedded(&self) -> bool {
        !self.levels.is_empty()
    }

    /// Whether index 255 is a hole (a `{` name).
    pub fn masked(&self) -> bool {
        self.name.starts_with('{')
    }

    /// Mip `level` as RGBA8, row by row from the top (`None` when not
    /// embedded). Holes of a `{` texture are black with alpha 0.
    pub fn rgba(&self, level: usize) -> Option<Vec<u8>> {
        let px = self.levels.get(level)?;
        let masked = self.masked();
        let mut out = Vec::with_capacity(px.len() * 4);
        for &i in px {
            if masked && i == TRANSPARENT_INDEX {
                out.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                let c = self.palette[i as usize];
                out.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        Some(out)
    }

    /// A decal (`decals.wad`) as RGBA8: the palette's last colour as the
    /// tint, the index as coverage (0 clear, 255 covered).
    pub fn decal_rgba(&self, level: usize) -> Option<Vec<u8>> {
        let px = self.levels.get(level)?;
        let tint = *self.palette.last()?;
        Some(px.iter().flat_map(|&i| [tint[0], tint[1], tint[2], i]).collect())
    }
}
