//! The bitmap tag (`bitm`) reader and pixel decoder, from
//! specs/halo_ce/bitmaps.md: `Bitmap::read` gives a tag's fields, its
//! sequences with their sprites and its images as plain structs with typed
//! enums; `Image` fetches an image's stored pixel bytes (outside the tag
//! data, at the absolute file offset the element gives) and decodes them to
//! RGBA8 or hands over the block compressed data untouched.
//!
//! - `Bitmap::read(map, tag)`, `read_image(map, tag, i)` (one element
//!   only) and `decode_image(map, tag, i)` (read and decode, for a level
//!   converter that wants one image on demand).
//! - `Image::layout` (`layout.rs`): which levels are stored, their sizes,
//!   the cube face stride and the pixels size the spec's rules predict
//!   (the reader refuses an image whose layout is invalid).
//! - `Image::pixels` (the stored bytes), `Image::decode` (every stored
//!   level of every face as RGBA8, top row first; a 3D level's slices one
//!   after another), `Image::decode_level` (one level of one face),
//!   `Image::blocks` (DXT1/3/5 levels as stored, borrowed from the map, for
//!   a GPU's BC1/BC2/BC3).
//! - `decode.rs`: the format decoders as plain functions, the swizzle
//!   rule and the bump palette.
//!
//! Everything is plain data and functions of their arguments: no global
//! state, so images decode on as many threads as the caller likes
//! (`MapFile` is a shared handle).
//!
//! Known shortcuts:
//! - Every enum value the spec lists is typed; a value outside them (on
//!   the tag's format hint or usage as well) refuses the tag.
//! - The linear storage assumes rows without padding (the spec's open
//!   question on pitch); the one disc example has none.
//! - A bitmap's pixel offset's 512-byte alignment and the exact equality
//!   of the pixels size with the layout's prediction are not enforced
//!   (`Layout::size` gives the prediction; `pixels` only needs the size to
//!   cover the stored levels).

mod decode;
mod layout;

use std::io;

use crate::cache::{MapFile, TagEntry, TagId, View, class};

pub use decode::{bump_palette, decode_blocks, decode_uncompressed, swizzled_index};
pub use layout::{Layout, LevelLayout, MAX_DIMENSION, MAX_LEVEL_TEXELS, SIZE_ROUNDING, Storage};

/// The tag's main struct size.
pub const FIXED_SIZE: usize = 0x6C;
/// One element of the bitmaps block.
pub const IMAGE_SIZE: usize = 0x30;
/// One element of the sequences block.
pub const SEQUENCE_SIZE: usize = 0x40;
/// One element of a sequence's sprites block.
pub const SPRITE_SIZE: usize = 0x20;
/// Caps on the block counts (far above the disc's), against corrupt tags.
pub const MAX_IMAGES: usize = 16384;
pub const MAX_SEQUENCES: usize = 16384;
pub const MAX_SPRITES: usize = 65536;

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

macro_rules! coded_enum {
    ($(#[$m:meta])* $name:ident { $($var:ident = $code:literal,)* }) => {
        $(#[$m])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum $name { $($var,)* }
        impl $name {
            /// The variant for a stored code, if listed.
            pub fn from_code(code: i16) -> Option<Self> {
                match code { $($code => Some(Self::$var),)* _ => None }
            }
            /// The stored code.
            pub fn code(self) -> i16 {
                match self { $(Self::$var => $code,)* }
            }
        }
    };
}

coded_enum! {
    /// What the tag's images are (tag field at 0x00).
    BitmapType {
        Textures2d = 0,
        Textures3d = 1,
        CubeMaps = 2,
        Sprites = 3,
        Interface = 4,
    }
}

coded_enum! {
    /// The author's format choice (tag field at 0x02; an import hint).
    FormatHint {
        Dxt1 = 0,
        Dxt3 = 1,
        Dxt5 = 2,
        Color16 = 3,
        Color32 = 4,
        Monochrome = 5,
    }
}

coded_enum! {
    /// How the importer built mip levels (tag field at 0x04; build time).
    Usage {
        AlphaBlend = 0,
        Default = 1,
        HeightMap = 2,
        DetailMap = 3,
        LightMap = 4,
        VectorMap = 5,
    }
}

coded_enum! {
    /// An image's own type (element field at 0x0A).
    ImageType {
        Texture2d = 0,
        Texture3d = 1,
        CubeMap = 2,
    }
}

coded_enum! {
    /// An image's pixel format code (element field at 0x0C). Codes 4, 5, 7,
    /// 12 and 13 are invalid.
    PixelFormat {
        A8 = 0,
        Y8 = 1,
        Ay8 = 2,
        A8y8 = 3,
        R5g6b5 = 6,
        A1r5g5b5 = 8,
        A4r4g4b4 = 9,
        X8r8g8b8 = 10,
        A8r8g8b8 = 11,
        Dxt1 = 14,
        Dxt3 = 15,
        Dxt5 = 16,
        P8Bump = 17,
    }
}

impl PixelFormat {
    /// DXT1, DXT3 or DXT5.
    pub fn is_block_compressed(self) -> bool {
        matches!(self, Self::Dxt1 | Self::Dxt3 | Self::Dxt5)
    }

    /// Bytes of one texel; 0 for the block compressed formats.
    pub fn texel_bytes(self) -> usize {
        match self {
            Self::A8 | Self::Y8 | Self::Ay8 | Self::P8Bump => 1,
            Self::A8y8 | Self::R5g6b5 | Self::A1r5g5b5 | Self::A4r4g4b4 => 2,
            Self::X8r8g8b8 | Self::A8r8g8b8 => 4,
            Self::Dxt1 | Self::Dxt3 | Self::Dxt5 => 0,
        }
    }

    /// Bytes of one 4x4 block; 0 for the uncompressed formats.
    pub fn block_bytes(self) -> usize {
        match self {
            Self::Dxt1 => 8,
            Self::Dxt3 | Self::Dxt5 => 16,
            _ => 0,
        }
    }
}

/// An image's flags (element field at 0x0E).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ImageFlags(pub u16);

impl ImageFlags {
    pub const POWER_OF_TWO: u16 = 0x01;
    pub const COMPRESSED: u16 = 0x02;
    pub const PALETTIZED: u16 = 0x04;
    pub const SWIZZLED: u16 = 0x08;
    pub const LINEAR: u16 = 0x10;
    pub const V16U16: u16 = 0x20;
    /// Run time: set on every shipped bitmap.
    pub const CACHED: u16 = 0x80;

    pub fn has(self, mask: u16) -> bool {
        self.0 & mask != 0
    }
}

/// One sprite of a sequence: a rectangle on a page.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    /// The page: an index into the tag's images.
    pub bitmap_index: u16,
    /// Fractions of the page (v grows downward: `top < bottom`).
    pub left: f32,
    pub right: f32,
    pub top: f32,
    pub bottom: f32,
    /// The anchor, from the rectangle's top-left, in page fractions.
    pub registration: [f32; 2],
}

/// A sequence: a run of images, or a group of sprites.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    pub name: String,
    /// Index of the first image (0 for a sprite sequence).
    pub first_bitmap: u16,
    /// Number of images (0 for a sprite sequence).
    pub bitmap_count: u16,
    pub sprites: Vec<Sprite>,
}

/// One element of the bitmaps block: an image and where its pixels are.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    /// Pixels of level 0.
    pub width: u16,
    pub height: u16,
    /// Slices of level 0 (1 unless 3D).
    pub depth: u16,
    pub image_type: ImageType,
    pub format: PixelFormat,
    pub flags: ImageFlags,
    pub registration: [i16; 2],
    /// Levels after level 0, as the element states it (a logical count: a
    /// block compressed image stores fewer, see `Layout::stored_levels`).
    pub mipmap_count: u16,
    /// Absolute offset of the pixel data in the decompressed map image.
    pub pixels_offset: u32,
    /// Bytes the pixel data occupies, padding included.
    pub pixels_size: u32,
    /// The tag's own id as the element stores it (a run-time leftover).
    pub owner: TagId,
}

/// A bitmap tag.
#[derive(Clone, Debug, PartialEq)]
pub struct Bitmap {
    /// The tag's path in the index.
    pub path: String,
    pub bitmap_type: BitmapType,
    pub format_hint: FormatHint,
    pub usage: Usage,
    /// Import options (build time).
    pub flags: u16,
    pub detail_fade_factor: f32,
    pub sharpen_amount: f32,
    /// In texture repeats; a bump map's height scale.
    pub bump_height: f32,
    pub sprite_budget_size: u16,
    pub sprite_budget_count: u16,
    pub color_plate_size: [i16; 2],
    pub blur_filter_size: f32,
    pub alpha_bias: f32,
    pub author_mipmap_count: i16,
    pub sprite_usage: u16,
    pub sprite_spacing: i16,
    pub sequences: Vec<Sequence>,
    pub images: Vec<Image>,
}

fn enum_field<T>(v: &View<'_>, offset: usize, what: &str, from: fn(i16) -> Option<T>) -> io::Result<T> {
    let code = v.enum16(offset)?;
    from(code).ok_or_else(|| v.error(offset, format_args!("{what} {code}: not a value the spec lists")))
}

fn u16_field(v: &View<'_>, offset: usize, what: &str) -> io::Result<u16> {
    let n = v.i16(offset)?;
    u16::try_from(n).map_err(|_| v.error(offset, format_args!("negative {what} {n}")))
}

impl Sprite {
    fn read(v: &View<'_>) -> io::Result<Sprite> {
        Ok(Sprite {
            bitmap_index: u16_field(v, 0x00, "sprite bitmap index")?,
            left: v.f32(0x08)?,
            right: v.f32(0x0C)?,
            top: v.f32(0x10)?,
            bottom: v.f32(0x14)?,
            registration: [v.f32(0x18)?, v.f32(0x1C)?],
        })
    }
}

impl Sequence {
    fn read(v: &View<'_>) -> io::Result<Sequence> {
        let sprites = v
            .block_max(0x34, SPRITE_SIZE, MAX_SPRITES)?
            .iter()
            .map(|e| Sprite::read(&e))
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Sequence {
            name: v.string(0x00, 32)?.to_owned(),
            first_bitmap: u16_field(v, 0x20, "first bitmap index")?,
            bitmap_count: u16_field(v, 0x22, "bitmap count")?,
            sprites,
        })
    }
}

impl Image {
    /// Read an element of the bitmaps block (`IMAGE_SIZE` bytes) and check
    /// that its layout is valid.
    pub fn read(v: &View<'_>) -> io::Result<Image> {
        let sig = v.u32(0x00)?;
        if sig != class::BITMAP.as_u32() {
            return Err(v.error(0, format_args!("bitmap signature {sig:#010x}, not `bitm`")));
        }
        let dim = |off: usize, what: &str| -> io::Result<u16> {
            let n = v.i16(off)?;
            u16::try_from(n)
                .ok()
                .filter(|&n| n > 0)
                .ok_or_else(|| v.error(off, format_args!("{what} {n}")))
        };
        let image = Image {
            width: dim(0x04, "width")?,
            height: dim(0x06, "height")?,
            depth: dim(0x08, "depth")?,
            image_type: enum_field(v, 0x0A, "image type", ImageType::from_code)?,
            format: enum_field(v, 0x0C, "pixel format", PixelFormat::from_code)?,
            flags: ImageFlags(v.flags16(0x0E)?),
            registration: [v.i16(0x10)?, v.i16(0x12)?],
            mipmap_count: u16_field(v, 0x14, "mipmap count")?,
            pixels_offset: v.u32(0x18)?,
            pixels_size: v.u32(0x1C)?,
            owner: v.tag_id(0x20)?,
        };
        image.layout().map_err(|e| v.error(0, e))?;
        Ok(image)
    }

    /// How the pixel data is stored: from the format, and for the
    /// uncompressed ones the swizzled and linear flags (neither set: plain
    /// row-major).
    pub fn storage(&self) -> Storage {
        if self.format.is_block_compressed() {
            Storage::Blocks
        } else if self.flags.has(ImageFlags::SWIZZLED) && !self.flags.has(ImageFlags::LINEAR) {
            Storage::Swizzled
        } else {
            Storage::Linear
        }
    }

    /// The stored layout (levels, face stride, predicted pixels size).
    pub fn layout(&self) -> Result<Layout, String> {
        Layout::new(
            self.image_type,
            self.format,
            self.storage(),
            u32::from(self.width),
            u32::from(self.height),
            u32::from(self.depth),
            u32::from(self.mipmap_count),
        )
    }

    /// Six for a cube map, else one.
    pub fn face_count(&self) -> usize {
        if self.image_type == ImageType::CubeMap { 6 } else { 1 }
    }

    /// The stored pixel bytes: `pixels_size` bytes at `pixels_offset` of
    /// the decompressed image; refused if they lie outside it or do not
    /// cover the stored levels.
    pub fn pixels<'a>(&self, map: &'a MapFile) -> io::Result<&'a [u8]> {
        let layout = self.layout().map_err(err)?;
        let size = self.pixels_size as usize;
        if size < stored_bytes(&layout) {
            return Err(err(format!(
                "{}: bitmap {}x{} {:?}: pixels size {size:#x} is less than the stored levels' {:#x}",
                map.header().name,
                self.width,
                self.height,
                self.format,
                stored_bytes(&layout)
            )));
        }
        map.file_bytes(self.pixels_offset as usize, size)
    }

    /// One stored level of one face as RGBA8: `width * height * depth * 4`
    /// bytes, rows top first, a volume's slices one after another.
    pub fn decode_level(&self, map: &MapFile, face: usize, level: usize) -> io::Result<DecodedLevel> {
        let layout = self.layout().map_err(err)?;
        let pixels = self.pixels(map)?;
        decode_one(self, &layout, pixels, face, level)
    }

    /// Every stored level of every face as RGBA8.
    pub fn decode(&self, map: &MapFile) -> io::Result<DecodedImage> {
        let layout = self.layout().map_err(err)?;
        let pixels = self.pixels(map)?;
        let faces = (0..layout.faces)
            .map(|f| {
                (0..layout.stored_levels())
                    .map(|l| decode_one(self, &layout, pixels, f, l))
                    .collect::<io::Result<Vec<_>>>()
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(DecodedImage {
            image_type: self.image_type,
            format: self.format,
            mipmap_count: self.mipmap_count,
            faces,
        })
    }

    /// The stored blocks of a block compressed image, untouched and borrowed
    /// from the map (`None` for the other formats), level 0 first, for a
    /// GPU's BC1 (DXT1), BC2 (DXT3) or BC3 (DXT5).
    pub fn blocks<'a>(&self, map: &'a MapFile) -> io::Result<Option<BlockImage<'a>>> {
        if !self.format.is_block_compressed() {
            return Ok(None);
        }
        let layout = self.layout().map_err(err)?;
        let pixels = self.pixels(map)?;
        let faces = (0..layout.faces)
            .map(|f| {
                layout
                    .levels
                    .iter()
                    .map(|lv| {
                        let start = f * layout.face_stride + lv.offset;
                        let data = pixels
                            .get(start..start + lv.size)
                            .ok_or_else(|| err(format!("level of {} bytes at {start:#x} past the pixels", lv.size)))?;
                        Ok(BlockLevel {
                            width: lv.width,
                            height: lv.height,
                            blocks_x: lv.width.div_ceil(4),
                            blocks_y: lv.height.div_ceil(4),
                            data,
                        })
                    })
                    .collect::<io::Result<Vec<_>>>()
            })
            .collect::<io::Result<Vec<_>>>()?;
        Ok(Some(BlockImage {
            format: self.format,
            faces,
        }))
    }
}

/// The bytes the stored levels (all faces, without the last face's
/// rounding) take.
fn stored_bytes(layout: &Layout) -> usize {
    let last = layout.levels.last().map_or(0, |l| l.offset + l.size);
    (layout.faces - 1) * layout.face_stride + last
}

fn decode_one(image: &Image, layout: &Layout, pixels: &[u8], face: usize, level: usize) -> io::Result<DecodedLevel> {
    if face >= layout.faces {
        return Err(err(format!("face {face} of an image with {}", layout.faces)));
    }
    let lv = layout
        .levels
        .get(level)
        .ok_or_else(|| err(format!("level {level} of an image storing {}", layout.stored_levels())))?;
    let start = layout.level_start(face, level);
    let src = pixels
        .get(start..start + lv.size)
        .ok_or_else(|| err(format!("level {level} of face {face} runs past the pixel data")))?;
    let rgba = if image.format.is_block_compressed() {
        decode_blocks(image.format, lv.width, lv.height, src)?
    } else {
        decode_uncompressed(image.format, layout.storage, lv.width, lv.height, lv.depth, src)?
    };
    Ok(DecodedLevel {
        width: lv.width,
        height: lv.height,
        depth: lv.depth,
        rgba,
    })
}

/// One decoded level.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedLevel {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// RGBA8, `width * height * depth * 4` bytes; row 0 is the top; a
    /// volume's slices follow one another.
    pub rgba: Vec<u8>,
}

/// A decoded image: `faces[face][level]` (one face unless a cube map, in
/// the order +X, -X, +Y, -Y, +Z, -Z), only the levels the file stores.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodedImage {
    pub image_type: ImageType,
    pub format: PixelFormat,
    /// The element's logical mipmap count (may exceed the stored levels).
    pub mipmap_count: u16,
    pub faces: Vec<Vec<DecodedLevel>>,
}

impl DecodedImage {
    /// The stored levels per face.
    pub fn stored_levels(&self) -> usize {
        self.faces.first().map_or(0, Vec::len)
    }
}

/// One stored level of block compressed data, as in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockLevel<'a> {
    pub width: u32,
    pub height: u32,
    /// `ceil(width / 4)` and `ceil(height / 4)`; rows of blocks top first.
    pub blocks_x: u32,
    pub blocks_y: u32,
    pub data: &'a [u8],
}

/// A block compressed image's stored levels, `faces[face][level]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BlockImage<'a> {
    /// `Dxt1` (BC1), `Dxt3` (BC2) or `Dxt5` (BC3).
    pub format: PixelFormat,
    pub faces: Vec<Vec<BlockLevel<'a>>>,
}

impl BlockImage<'_> {
    pub fn stored_levels(&self) -> usize {
        self.faces.first().map_or(0, Vec::len)
    }
}

impl Bitmap {
    /// Read a bitmap tag (class `bitm`): its fields, sequences and images.
    pub fn read(map: &MapFile, tag: &TagEntry) -> io::Result<Bitmap> {
        let v = bitmap_view(map, tag)?;
        let images = v
            .block_max(0x60, IMAGE_SIZE, MAX_IMAGES)?
            .iter()
            .map(|e| Image::read(&e))
            .collect::<io::Result<Vec<_>>>()?;
        let sequences = v
            .block_max(0x54, SEQUENCE_SIZE, MAX_SEQUENCES)?
            .iter()
            .map(|e| Sequence::read(&e))
            .collect::<io::Result<Vec<_>>>()?;
        for (i, s) in sequences.iter().enumerate() {
            let end = usize::from(s.first_bitmap) + usize::from(s.bitmap_count);
            if end > images.len() {
                return Err(v.error(
                    0x54,
                    format_args!("sequence {i} covers images up to {end} of {}", images.len()),
                ));
            }
            if let Some(p) = s.sprites.iter().find(|p| usize::from(p.bitmap_index) >= images.len()) {
                return Err(v.error(
                    0x54,
                    format_args!(
                        "sequence {i}: sprite page {} of {} images",
                        p.bitmap_index,
                        images.len()
                    ),
                ));
            }
        }
        Ok(Bitmap {
            path: tag.path.clone(),
            bitmap_type: enum_field(&v, 0x00, "bitmap type", BitmapType::from_code)?,
            format_hint: enum_field(&v, 0x02, "format hint", FormatHint::from_code)?,
            usage: enum_field(&v, 0x04, "usage", Usage::from_code)?,
            flags: v.flags16(0x06)?,
            detail_fade_factor: v.f32(0x08)?,
            sharpen_amount: v.f32(0x0C)?,
            bump_height: v.f32(0x10)?,
            sprite_budget_size: v.u16(0x14)?,
            sprite_budget_count: v.u16(0x16)?,
            color_plate_size: [v.i16(0x18)?, v.i16(0x1A)?],
            blur_filter_size: v.f32(0x44)?,
            alpha_bias: v.f32(0x48)?,
            author_mipmap_count: v.i16(0x4C)?,
            sprite_usage: v.u16(0x4E)?,
            sprite_spacing: v.i16(0x50)?,
            sequences,
            images,
        })
    }

    /// Decode image `index` (all stored levels), with the tag's path in
    /// any error.
    pub fn decode_image(&self, map: &MapFile, index: usize) -> io::Result<DecodedImage> {
        let image = self
            .images
            .get(index)
            .ok_or_else(|| err(format!("{}: image {index} of {}", self.path, self.images.len())))?;
        image
            .decode(map)
            .map_err(|e| err(format!("{}: image {index}: {e}", self.path)))
    }

    /// The image a (sequence, frame) pair draws, by the spec's rule: no
    /// sequences or a negative sequence index: the frame is the image
    /// index; else the sequence is `sequence mod count`; with images the
    /// frame wraps over them, with sprites it picks a sprite (not wrapped)
    /// and the sprite's page is the image; otherwise the frame is the image
    /// index. `None` when the result is outside the images. Also returns
    /// the sprite chosen, if any.
    pub fn frame(&self, sequence: i32, frame: i32) -> Option<(usize, Option<&Sprite>)> {
        if frame < 0 {
            return None;
        }
        let frame = frame as usize;
        let mut chosen: Option<(usize, Option<&Sprite>)> = None;
        if sequence >= 0 && !self.sequences.is_empty() {
            let s = &self.sequences[sequence as usize % self.sequences.len()];
            if s.bitmap_count > 0 {
                chosen = Some((usize::from(s.first_bitmap) + frame % usize::from(s.bitmap_count), None));
            } else if let Some(p) = s.sprites.get(frame) {
                chosen = Some((usize::from(p.bitmap_index), Some(p)));
            }
        }
        let (index, sprite) = chosen.unwrap_or((frame, None));
        (index < self.images.len()).then_some((index, sprite))
    }
}

fn bitmap_view<'a>(map: &'a MapFile, tag: &TagEntry) -> io::Result<View<'a>> {
    if tag.class != class::BITMAP {
        return Err(err(format!(
            "{}: {} `{}` is not a bitmap tag",
            map.header().name,
            tag.class,
            tag.path
        )));
    }
    map.tag_view(tag, FIXED_SIZE)
}

/// Read image `index` of a bitmap tag without reading the rest.
pub fn read_image(map: &MapFile, tag: &TagEntry, index: usize) -> io::Result<Image> {
    let v = bitmap_view(map, tag)?;
    Image::read(&v.block_max(0x60, IMAGE_SIZE, MAX_IMAGES)?.get(index)?)
}

/// Read and decode image `index` of a bitmap tag, all stored levels: the
/// call a level converter makes per texture.
pub fn decode_image(map: &MapFile, tag: &TagEntry, index: usize) -> io::Result<DecodedImage> {
    let image = read_image(map, tag, index)?;
    image.decode(map).map_err(|e| {
        err(format!(
            "{}: {} `{}` image {index}: {e}",
            map.header().name,
            tag.class,
            tag.path
        ))
    })
}
