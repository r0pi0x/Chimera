//! The stored layout of a bitmap image's pixel data (specs/halo_ce/bitmaps.md,
//! "Levels and their order", "Cube maps" and "3D (volume) textures"): which
//! levels the file stores, how big each is, where each face starts, and the
//! pixels size the layout predicts (the spec's consistency check).
//!
//! `Layout::new` is pure arithmetic on the image's dimensions, type, format
//! and storage kind; it also validates them (the spec's "Edge cases"), so a
//! corrupt bitmap element is refused here and never reaches a decoder.

use super::{ImageType, PixelFormat};

/// The largest width, height or depth accepted (the disc's largest is
/// 2048); keeps a corrupt element from asking for a huge decode.
pub const MAX_DIMENSION: u32 = 4096;
/// The most texels one level may have (width x height x depth): 64 Mi, 256
/// MiB as RGBA8. The disc's largest level has 2 Mi.
pub const MAX_LEVEL_TEXELS: u64 = 1 << 26;
/// A face chain (and the pixels size) rounds up to this.
pub const SIZE_ROUNDING: usize = 128;

/// How the texels of an uncompressed level are ordered in the file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Storage {
    /// Block compressed: linear rows of 4x4 blocks.
    Blocks,
    /// Morton (bit-interleaved) order.
    Swizzled,
    /// Plain row-major.
    Linear,
}

/// One stored level of one face.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LevelLayout {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    /// Bytes from the start of the face's chain.
    pub offset: usize,
    /// The level's stored bytes.
    pub size: usize,
}

/// The stored layout of an image.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    /// How the texels are stored.
    pub storage: Storage,
    /// 1, or 6 for a cube map.
    pub faces: usize,
    /// The stored levels of one face, level 0 first. Their count is the
    /// image's number of stored levels (never more than mip count + 1; for
    /// block compressed images fewer when the tail is below one block).
    pub levels: Vec<LevelLayout>,
    /// Bytes from one face's start to the next's (the face chain rounded
    /// up to 128); for a single-face image the chain's size.
    pub face_stride: usize,
    /// The pixels size the layout predicts: `faces * face_stride` (with
    /// the chain rounded up to 128 on a 2D or 3D image).
    pub size: usize,
}

impl Layout {
    /// The stored levels' count (per face).
    pub fn stored_levels(&self) -> usize {
        self.levels.len()
    }

    /// Where level `level` of face `face` starts, from the pixels offset.
    pub fn level_start(&self, face: usize, level: usize) -> usize {
        face * self.face_stride + self.levels[level].offset
    }

    /// Work out the layout, refusing what the spec calls invalid. `mip_count`
    /// is the element's (levels after the first).
    pub fn new(
        kind: ImageType,
        format: PixelFormat,
        storage: Storage,
        width: u32,
        height: u32,
        depth: u32,
        mip_count: u32,
    ) -> Result<Layout, String> {
        let compressed = format.is_block_compressed();
        if compressed != (storage == Storage::Blocks) {
            return Err(format!(
                "{format:?} cannot be stored as {storage:?} (block compression is for codes 14 to 16 only)"
            ));
        }
        if width == 0 || height == 0 || depth == 0 {
            return Err(format!("size {width}x{height}x{depth} has a zero side"));
        }
        if width > MAX_DIMENSION || height > MAX_DIMENSION || depth > MAX_DIMENSION {
            return Err(format!("size {width}x{height}x{depth} over {MAX_DIMENSION}"));
        }
        if u64::from(width) * u64::from(height) * u64::from(depth) > MAX_LEVEL_TEXELS {
            return Err(format!("size {width}x{height}x{depth} has too many texels"));
        }
        if depth > 1 && kind != ImageType::Texture3d {
            return Err(format!("depth {depth} on a {kind:?} image"));
        }
        if kind == ImageType::CubeMap && width != height {
            return Err(format!("cube map of {width}x{height}: not square"));
        }
        if kind == ImageType::Texture3d && compressed {
            return Err("a block compressed 3D texture".into());
        }
        let largest = width.max(height).max(depth);
        let max_mips = largest.ilog2();
        if mip_count > max_mips {
            return Err(format!(
                "mipmap count {mip_count} over {max_mips} for {width}x{height}x{depth}"
            ));
        }
        if storage == Storage::Swizzled
            && !(width.is_power_of_two() && height.is_power_of_two() && depth.is_power_of_two())
        {
            return Err(format!("swizzled image of {width}x{height}x{depth}: not powers of two"));
        }

        let block_bytes = format.block_bytes();
        let texel_bytes = format.texel_bytes();
        let mut levels = Vec::with_capacity(mip_count as usize + 1);
        let mut offset = 0usize;
        for l in 0..=mip_count {
            let (w, h, d) = ((width >> l).max(1), (height >> l).max(1), (depth >> l).max(1));
            // Block compressed images do not store levels below one block.
            if compressed && l > 0 && w.max(h) < 4 {
                break;
            }
            let size = if compressed {
                w.div_ceil(4) as usize * h.div_ceil(4) as usize * block_bytes
            } else {
                w as usize * h as usize * d as usize * texel_bytes
            };
            levels.push(LevelLayout {
                width: w,
                height: h,
                depth: d,
                offset,
                size,
            });
            offset += size;
        }
        let chain = offset;
        let faces = if kind == ImageType::CubeMap { 6 } else { 1 };
        let face_stride = chain.next_multiple_of(SIZE_ROUNDING);
        Ok(Layout {
            storage,
            faces,
            levels,
            face_stride,
            size: faces * face_stride,
        })
    }
}
