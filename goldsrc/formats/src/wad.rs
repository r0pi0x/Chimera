//! WAD3 texture archives (specs/goldsrc/wad3.md): `Wad::parse` reads the
//! header and the directory at the end; entries are found by name
//! ignoring case (`Wad::find`) and decoded by type: mip textures (`0x43`,
//! `miptex::MipTex`), pictures (`0x42`, `Picture`) and font sheets
//! (`0x46`, `Font`). Every image carries its own 256-colour palette.

use chimera_vfs::bytes::Le;

use crate::{Result, err, fixed_name, miptex::MipTex};

pub const MAGIC: &[u8; 4] = b"WAD3";
pub const TYPE_QPIC: u8 = 0x42;
pub const TYPE_MIPTEX: u8 = 0x43;
pub const TYPE_FONT: u8 = 0x46;

/// One directory record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WadEntry {
    pub offset: u32,
    pub disk_size: u32,
    pub size: u32,
    pub kind: u8,
    pub compression: u8,
    pub name: String,
}

/// A WAD3 file: its bytes and directory.
#[derive(Clone, Debug)]
pub struct Wad {
    pub bytes: Vec<u8>,
    pub dir_offset: u32,
    pub entries: Vec<WadEntry>,
}

/// A plain picture (`0x42`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub palette: Vec<[u8; 3]>,
}

/// One character of a font sheet: its offset into the sheet (y × 256 + x)
/// and its width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Glyph {
    pub offset: u16,
    pub width: u16,
}

/// A font sheet (`0x46`): always 256 pixels wide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font {
    pub nominal_width: u32,
    pub height: u32,
    pub rows: u32,
    pub row_height: u32,
    pub glyphs: Vec<Glyph>,
    pub pixels: Vec<u8>,
    pub palette: Vec<[u8; 3]>,
}

fn palette(r: &mut Le) -> Result<Vec<[u8; 3]>> {
    Ok(r.take(768)?.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect())
}

impl Wad {
    pub fn parse(bytes: Vec<u8>) -> Result<Wad> {
        let mut r = Le::new(&bytes);
        if r.take(4)? != MAGIC {
            return err("not a WAD3 file");
        }
        let count = r.i32()?;
        let dir_offset = r.i32()?;
        if count < 0 || dir_offset < 0 {
            return err(format!("WAD3 header: {count} entries at {dir_offset}"));
        }
        r.pos = dir_offset as usize;
        let mut entries = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let e = WadEntry {
                offset: r.u32()?,
                disk_size: r.u32()?,
                size: r.u32()?,
                kind: r.u8()?,
                compression: r.u8()?,
                name: {
                    r.take(2)?;
                    fixed_name(r.take(16)?)
                },
            };
            if e.offset as usize + e.disk_size as usize > bytes.len() {
                return err(format!("WAD3 entry {}: outside the file", e.name));
            }
            entries.push(e);
        }
        Ok(Wad {
            bytes,
            dir_offset: dir_offset as u32,
            entries,
        })
    }

    /// The entry named `name`, ignoring case.
    pub fn find(&self, name: &str) -> Option<&WadEntry> {
        self.entries.iter().find(|e| e.name.eq_ignore_ascii_case(name))
    }

    /// An entry's bytes.
    pub fn data(&self, e: &WadEntry) -> Result<&[u8]> {
        if e.compression != 0 {
            return err(format!(
                "WAD3 entry {}: compression {} is not defined",
                e.name, e.compression
            ));
        }
        Ok(&self.bytes[e.offset as usize..(e.offset + e.disk_size) as usize])
    }

    /// The mip texture named `name`, ignoring case.
    pub fn miptex(&self, name: &str) -> Option<Result<MipTex>> {
        let e = self.find(name).filter(|e| e.kind == TYPE_MIPTEX)?;
        Some(self.data(e).and_then(MipTex::parse))
    }

    /// A picture entry.
    pub fn picture(&self, e: &WadEntry) -> Result<Picture> {
        let mut r = Le::new(self.data(e)?);
        let (width, height) = (r.u32()?, r.u32()?);
        let pixels = r.take((width * height) as usize)?.to_vec();
        r.u16()?;
        Ok(Picture {
            width,
            height,
            pixels,
            palette: palette(&mut r)?,
        })
    }

    /// A font entry (the colour count field is not trusted: the palette is
    /// always 256 colours).
    pub fn font(&self, e: &WadEntry) -> Result<Font> {
        let mut r = Le::new(self.data(e)?);
        let (nominal_width, height, rows, row_height) = (r.u32()?, r.u32()?, r.u32()?, r.u32()?);
        let glyphs = (0..256)
            .map(|_| {
                Ok(Glyph {
                    offset: r.u16()?,
                    width: r.u16()?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let pixels = r.take(256 * height as usize)?.to_vec();
        r.u16()?;
        Ok(Font {
            nominal_width,
            height,
            rows,
            row_height,
            glyphs,
            pixels,
            palette: palette(&mut r)?,
        })
    }
}
