//! Address spaces and views (specs/halo_ce/cache_file.md, "Address space"
//! and "Tag data conventions"): a `Space` maps 32-bit Xbox addresses to
//! offsets of the decompressed image (the tag data's, or one structure
//! BSP's); a `View` is a bounds-checked window of known size at an address
//! in one space, with a read for every primitive type and for the three
//! compound ones (`Block`, `TagRef`, `DataRef`).
//!
//! Every read is checked twice: against the view's own size (a class
//! reader's offset past its struct is a transcription error, reported, not
//! a panic) and, when it follows a pointer, against the space. Errors are
//! `io::Error` (`InvalidData`) naming the map, the tag, the address, the
//! offset and what was expected.

use std::{fmt, io};

use super::types::{
    Argb8, Bounds, Euler2d, Euler3d, Plane2d, Plane3d, RealAhsv, RealArgb, RealHsv, RealRect2d, RealRect3d, RealRgb,
    Rect2d, Rgb8, TagStruct,
};
use super::{ClassCode, MapFile, TagEntry, TagId};

/// The largest element count a block may have unless the reader passes its
/// own maximum (`View::block_max`). Far above any count on the disc; with
/// the space check it keeps corrupt counts from allocating much.
pub const MAX_BLOCK_COUNT: usize = 1 << 20;

/// Which address space a pointer is in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SpaceId {
    /// The tag data, loaded at `TAG_BASE`.
    Tags,
    /// Structure BSP `i` of the scenario's BSP list, loaded so that it ends
    /// at `TAG_CACHE_END`.
    Bsp(u16),
}

/// An address space: `len` bytes at file offset `file_offset` of the
/// decompressed image, seen at addresses `base..base + len`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Space {
    pub id: SpaceId,
    pub base: u32,
    pub file_offset: usize,
    pub len: usize,
}

impl Space {
    /// The first address past the space.
    pub fn end(&self) -> u64 {
        self.base as u64 + self.len as u64
    }

    /// The image offset of `len` bytes at `addr`, if all of them lie in the
    /// space.
    pub fn offset_of(&self, addr: u32, len: usize) -> Option<usize> {
        let rel = addr.checked_sub(self.base)? as usize;
        (rel.checked_add(len)? <= self.len).then_some(self.file_offset + rel)
    }
}

impl fmt::Display for SpaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SpaceId::Tags => write!(f, "tag data"),
            SpaceId::Bsp(i) => write!(f, "bsp {i}"),
        }
    }
}

/// A tag reference field (16 bytes).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagRef {
    /// The referenced tag's class; on a null reference, the field's
    /// expected class (possibly an abstract parent such as `shdr`).
    pub class: ClassCode,
    /// Address of the path string (the target entry's own), or 0.
    pub path_address: u32,
    /// The referenced tag, or NONE.
    pub id: TagId,
}

impl TagRef {
    /// Id NONE: no tag. (An id that is not NONE may still fail to resolve:
    /// `MapFile::tag` says.)
    pub fn is_null(&self) -> bool {
        self.id.is_none()
    }
}

/// A data reference field (20 bytes), as stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataRef {
    pub size: i32,
    pub flags: u32,
    /// Meaningful only for external fields (sound samples); a leftover
    /// elsewhere.
    pub file_offset: i32,
    /// Address of the bytes in the view's space, or 0.
    pub address: u32,
}

/// A tag block field, as stored (before any check).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RawBlock {
    pub count: i32,
    pub address: u32,
}

/// A window of `len` bytes at `addr` in one space of a map. `Copy`; reads
/// take offsets from the window's start. The struct's pointers (blocks,
/// data) are followed in the same space.
#[derive(Clone, Copy)]
pub struct View<'a> {
    map: &'a MapFile,
    space: Space,
    tag: Option<u16>,
    addr: u32,
    bytes: &'a [u8],
}

impl fmt::Debug for View<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "View({}, {:#010x}, {} bytes)",
            self.place(),
            self.addr,
            self.bytes.len()
        )
    }
}

impl<'a> View<'a> {
    /// A view of `len` bytes at `addr` in `space`; `tag` (an index entry)
    /// names the tag in errors.
    pub(crate) fn new(map: &'a MapFile, space: Space, tag: Option<u16>, addr: u32, len: usize) -> io::Result<Self> {
        let at = |what: String| {
            let place = Place {
                map,
                space: space.id,
                tag,
            };
            invalid(format!("{place}: {what}"))
        };
        if addr == 0 {
            return Err(at(format!("null address for {len} bytes")));
        }
        let off = space.offset_of(addr, len).ok_or_else(|| {
            at(format!(
                "{len} bytes at {addr:#010x} are outside the {} ({:#010x}..{:#010x})",
                space.id,
                space.base,
                space.end()
            ))
        })?;
        Ok(View {
            map,
            space,
            tag,
            addr,
            bytes: &map.image()[off..off + len],
        })
    }

    pub fn map(&self) -> &'a MapFile {
        self.map
    }

    /// The space the view and its pointers are in.
    pub fn space(&self) -> Space {
        self.space
    }

    /// The address of the view's first byte.
    pub fn addr(&self) -> u32 {
        self.addr
    }

    /// The view's size in bytes.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The file offset of the view's first byte in the decompressed image.
    pub fn file_offset(&self) -> usize {
        self.space.file_offset + (self.addr - self.space.base) as usize
    }

    /// The view's bytes.
    pub fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    /// The index entry of the tag the view belongs to, if known.
    pub fn tag(&self) -> Option<&'a TagEntry> {
        self.tag.map(|i| &self.map.tags()[i as usize])
    }

    fn place(&self) -> Place<'a> {
        Place {
            map: self.map,
            space: self.space.id,
            tag: self.tag,
        }
    }

    /// An `InvalidData` error at `offset` of this view: the map, the tag,
    /// the address and offset, and `what`. For class readers' own checks
    /// (an enum out of range, an index past its block).
    pub fn error(&self, offset: usize, what: impl fmt::Display) -> io::Error {
        invalid(format!("{} at {:#010x}+{offset:#x}: {what}", self.place(), self.addr))
    }

    /// `n` bytes at `offset`.
    pub fn slice(&self, offset: usize, n: usize) -> io::Result<&'a [u8]> {
        offset
            .checked_add(n)
            .and_then(|end| self.bytes.get(offset..end))
            .ok_or_else(|| self.error(offset, format_args!("{n} bytes past the view's {} bytes", self.len())))
    }

    fn arr<const N: usize>(&self, offset: usize) -> io::Result<[u8; N]> {
        Ok(self.slice(offset, N)?.try_into().expect("slice of N bytes"))
    }

    /// A view of `len` bytes at `offset` of this one (a struct inside a
    /// struct).
    pub fn sub(&self, offset: usize, len: usize) -> io::Result<View<'a>> {
        self.slice(offset, len)?;
        Ok(View {
            addr: self.addr + offset as u32,
            bytes: &self.bytes[offset..offset + len],
            ..*self
        })
    }

    /// A view of `len` bytes at `addr` in this view's space (for a pointer
    /// a class spec names outside the block and data reference fields).
    pub fn follow(&self, addr: u32, len: usize) -> io::Result<View<'a>> {
        View::new(self.map, self.space, self.tag, addr, len)
    }

    /// Read a `T` at `offset` (a struct inline in this one).
    pub fn read<T: TagStruct>(&self, offset: usize) -> io::Result<T> {
        T::read(&self.sub(offset, T::SIZE)?)
    }

    // Integers and reals.

    pub fn u8(&self, offset: usize) -> io::Result<u8> {
        Ok(self.arr::<1>(offset)?[0])
    }

    pub fn i8(&self, offset: usize) -> io::Result<i8> {
        Ok(self.arr::<1>(offset)?[0] as i8)
    }

    pub fn u16(&self, offset: usize) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.arr(offset)?))
    }

    pub fn i16(&self, offset: usize) -> io::Result<i16> {
        Ok(i16::from_le_bytes(self.arr(offset)?))
    }

    pub fn u32(&self, offset: usize) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.arr(offset)?))
    }

    pub fn i32(&self, offset: usize) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.arr(offset)?))
    }

    /// A real, fraction or angle (radians).
    pub fn f32(&self, offset: usize) -> io::Result<f32> {
        Ok(f32::from_le_bytes(self.arr(offset)?))
    }

    fn f32s<const N: usize>(&self, offset: usize) -> io::Result<[f32; N]> {
        let mut out = [0.0; N];
        for (i, v) in out.iter_mut().enumerate() {
            *v = self.f32(offset + 4 * i)?;
        }
        Ok(out)
    }

    // Enums, flags, indices.

    /// An `enum16`: the stored i16 (the class spec lists the values).
    pub fn enum16(&self, offset: usize) -> io::Result<i16> {
        self.i16(offset)
    }

    /// An `enum16` that must lie in `0..count`.
    pub fn enum16_below(&self, offset: usize, count: i16) -> io::Result<i16> {
        let v = self.i16(offset)?;
        if (0..count).contains(&v) {
            Ok(v)
        } else {
            Err(self.error(offset, format_args!("enum value {v} not in 0..{count}")))
        }
    }

    pub fn flags8(&self, offset: usize) -> io::Result<u8> {
        self.u8(offset)
    }

    pub fn flags16(&self, offset: usize) -> io::Result<u16> {
        self.u16(offset)
    }

    pub fn flags32(&self, offset: usize) -> io::Result<u32> {
        self.u32(offset)
    }

    fn index(&self, offset: usize, v: i64) -> io::Result<Option<usize>> {
        match v {
            -1 => Ok(None),
            v if v >= 0 => Ok(Some(v as usize)),
            v => Err(self.error(offset, format_args!("index {v}: negative and not NONE"))),
        }
    }

    /// An `index8`: `None` for NONE (-1); another negative value is an
    /// error. Check it against its block's count (`Block::get` does).
    pub fn index8(&self, offset: usize) -> io::Result<Option<usize>> {
        self.index(offset, self.i8(offset)? as i64)
    }

    /// An `index16`, as `index8`.
    pub fn index16(&self, offset: usize) -> io::Result<Option<usize>> {
        self.index(offset, self.i16(offset)? as i64)
    }

    /// An `index32`, as `index8`.
    pub fn index32(&self, offset: usize) -> io::Result<Option<usize>> {
        self.index(offset, self.i32(offset)? as i64)
    }

    // Strings and codes.

    /// A 32-byte tag string: up to 31 characters, NUL-terminated. No NUL,
    /// or bytes that are not UTF-8, is an error.
    pub fn tag_string(&self, offset: usize) -> io::Result<&'a str> {
        let b = self.slice(offset, 32)?;
        let end = b
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| self.error(offset, "tag string without a NUL in its 32 bytes"))?;
        std::str::from_utf8(&b[..end]).map_err(|_| self.error(offset, "tag string is not text"))
    }

    /// A fixed-length string of `len` bytes (a length a class spec gives):
    /// up to the first NUL, or all `len` bytes without one.
    pub fn string(&self, offset: usize, len: usize) -> io::Result<&'a str> {
        let b = self.slice(offset, len)?;
        let end = b.iter().position(|&c| c == 0).unwrap_or(len);
        std::str::from_utf8(&b[..end]).map_err(|_| self.error(offset, "string is not text"))
    }

    /// A class code (NONE reads as the code 0xFFFFFFFF; see
    /// `ClassCode::from_slot`).
    pub fn class(&self, offset: usize) -> io::Result<ClassCode> {
        Ok(ClassCode::from_u32(self.u32(offset)?))
    }

    pub fn tag_id(&self, offset: usize) -> io::Result<TagId> {
        Ok(TagId(self.u32(offset)?))
    }

    // Geometry.

    /// `point2d`: i16 x, y.
    pub fn point2d(&self, offset: usize) -> io::Result<[i16; 2]> {
        Ok([self.i16(offset)?, self.i16(offset + 2)?])
    }

    /// `real_point2d` or `real_vector2d`.
    pub fn point2(&self, offset: usize) -> io::Result<[f32; 2]> {
        self.f32s(offset)
    }

    /// `real_point3d` or `real_vector3d`.
    pub fn point3(&self, offset: usize) -> io::Result<[f32; 3]> {
        self.f32s(offset)
    }

    /// `real_vector2d` (same layout as a point).
    pub fn vector2(&self, offset: usize) -> io::Result<[f32; 2]> {
        self.f32s(offset)
    }

    /// `real_vector3d` (same layout as a point).
    pub fn vector3(&self, offset: usize) -> io::Result<[f32; 3]> {
        self.f32s(offset)
    }

    /// `real_quaternion`: `[i, j, k, w]`.
    pub fn quaternion(&self, offset: usize) -> io::Result<[f32; 4]> {
        self.f32s(offset)
    }

    pub fn euler2d(&self, offset: usize) -> io::Result<Euler2d> {
        let [yaw, pitch] = self.f32s(offset)?;
        Ok(Euler2d { yaw, pitch })
    }

    pub fn euler3d(&self, offset: usize) -> io::Result<Euler3d> {
        let [yaw, pitch, roll] = self.f32s(offset)?;
        Ok(Euler3d { yaw, pitch, roll })
    }

    pub fn plane2d(&self, offset: usize) -> io::Result<Plane2d> {
        let [i, j, d] = self.f32s(offset)?;
        Ok(Plane2d { normal: [i, j], d })
    }

    pub fn plane3d(&self, offset: usize) -> io::Result<Plane3d> {
        let [i, j, k, d] = self.f32s(offset)?;
        Ok(Plane3d { normal: [i, j, k], d })
    }

    /// `rectangle2d`: top, left, bottom, right.
    pub fn rect2d(&self, offset: usize) -> io::Result<Rect2d> {
        Ok(Rect2d {
            top: self.i16(offset)?,
            left: self.i16(offset + 2)?,
            bottom: self.i16(offset + 4)?,
            right: self.i16(offset + 6)?,
        })
    }

    /// `real_rectangle2d`: x0, x1, y0, y1.
    pub fn real_rect2d(&self, offset: usize) -> io::Result<RealRect2d> {
        Ok(RealRect2d {
            x: self.real_bounds(offset)?,
            y: self.real_bounds(offset + 8)?,
        })
    }

    /// `real_rectangle3d`: x0, x1, y0, y1, z0, z1.
    pub fn real_rect3d(&self, offset: usize) -> io::Result<RealRect3d> {
        Ok(RealRect3d {
            x: self.real_bounds(offset)?,
            y: self.real_bounds(offset + 8)?,
            z: self.real_bounds(offset + 16)?,
        })
    }

    // Colours.

    pub fn real_rgb(&self, offset: usize) -> io::Result<RealRgb> {
        let [r, g, b] = self.f32s(offset)?;
        Ok(RealRgb { r, g, b })
    }

    /// `real_argb_color`: alpha first.
    pub fn real_argb(&self, offset: usize) -> io::Result<RealArgb> {
        let [a, r, g, b] = self.f32s(offset)?;
        Ok(RealArgb { a, r, g, b })
    }

    pub fn real_hsv(&self, offset: usize) -> io::Result<RealHsv> {
        let [h, s, v] = self.f32s(offset)?;
        Ok(RealHsv { h, s, v })
    }

    pub fn real_ahsv(&self, offset: usize) -> io::Result<RealAhsv> {
        let [a, h, s, v] = self.f32s(offset)?;
        Ok(RealAhsv { a, h, s, v })
    }

    /// `rgb_color`: bytes B, G, R, ignored.
    pub fn rgb8(&self, offset: usize) -> io::Result<Rgb8> {
        let [b, g, r, _] = self.arr(offset)?;
        Ok(Rgb8 { r, g, b })
    }

    /// `argb_color`: bytes B, G, R, A.
    pub fn argb8(&self, offset: usize) -> io::Result<Argb8> {
        let [b, g, r, a] = self.arr(offset)?;
        Ok(Argb8 { a, r, g, b })
    }

    // Bounds.

    /// `short_bounds`.
    pub fn short_bounds(&self, offset: usize) -> io::Result<Bounds<i16>> {
        Ok(Bounds {
            lower: self.i16(offset)?,
            upper: self.i16(offset + 2)?,
        })
    }

    /// `real_bounds`, `angle_bounds` or `fraction_bounds`.
    pub fn real_bounds(&self, offset: usize) -> io::Result<Bounds<f32>> {
        let [lower, upper] = self.f32s(offset)?;
        Ok(Bounds { lower, upper })
    }

    // Compound fields.

    /// A tag block field as stored, unchecked (for a count alone).
    pub fn raw_block(&self, offset: usize) -> io::Result<RawBlock> {
        Ok(RawBlock {
            count: self.i32(offset)?,
            address: self.u32(offset + 4)?,
        })
    }

    /// The tag block at `offset` whose elements are `elem_size` bytes, its
    /// count at most `MAX_BLOCK_COUNT`; checked to lie in the space.
    pub fn block(&self, offset: usize, elem_size: usize) -> io::Result<Block<'a>> {
        self.block_max(offset, elem_size, MAX_BLOCK_COUNT)
    }

    /// `block` with the block definition's own maximum count (the class
    /// spec's); a larger count is damage.
    pub fn block_max(&self, offset: usize, elem_size: usize, max: usize) -> io::Result<Block<'a>> {
        let raw = self.raw_block(offset)?;
        if elem_size == 0 {
            return Err(self.error(offset, "block element size 0"));
        }
        if raw.count < 0 {
            return Err(self.error(offset, format_args!("negative block count {}", raw.count)));
        }
        let count = raw.count as usize;
        if count > max {
            return Err(self.error(offset, format_args!("block count {count} over the maximum {max}")));
        }
        let empty = Block {
            view: *self,
            count: 0,
            elem_size,
            first: None,
        };
        if count == 0 {
            return Ok(empty);
        }
        let len = count * elem_size;
        let first = View::new(self.map, self.space, self.tag, raw.address, len).map_err(|_| {
            self.error(
                offset,
                format_args!(
                    "block of {count} x {elem_size} bytes at {:#010x} is outside the {} ({:#010x}..{:#010x})",
                    raw.address,
                    self.space.id,
                    self.space.base,
                    self.space.end()
                ),
            )
        })?;
        Ok(Block {
            count,
            first: Some(first),
            ..empty
        })
    }

    /// The tag reference at `offset`, as stored.
    pub fn tag_ref(&self, offset: usize) -> io::Result<TagRef> {
        Ok(TagRef {
            class: self.class(offset)?,
            path_address: self.u32(offset + 4)?,
            id: self.tag_id(offset + 12)?,
        })
    }

    /// The tag reference at `offset`, resolved: `None` when null or when the
    /// id is invalid (the spec's "treat as null"); an error when it names a
    /// tag that is not `accepts` and does not derive from it.
    pub fn reference(&self, offset: usize, accepts: ClassCode) -> io::Result<Option<&'a TagEntry>> {
        self.reference_any(offset, &[accepts])
    }

    /// `reference` for a field that accepts several classes (any of them,
    /// or a class deriving from one).
    pub fn reference_any(&self, offset: usize, accepts: &[ClassCode]) -> io::Result<Option<&'a TagEntry>> {
        let r = self.tag_ref(offset)?;
        let Some(entry) = self.map.tag(r.id) else {
            return Ok(None);
        };
        if accepts.iter().any(|&c| entry.is_a(c)) {
            Ok(Some(entry))
        } else {
            Err(self.error(
                offset,
                format_args!(
                    "reference to {} `{}`, expected {}",
                    entry.class,
                    entry.path,
                    accepts
                        .iter()
                        .map(|c| format!("`{c}`"))
                        .collect::<Vec<_>>()
                        .join(" or ")
                ),
            ))
        }
    }

    /// The data reference at `offset`, as stored.
    pub fn data_ref(&self, offset: usize) -> io::Result<DataRef> {
        Ok(DataRef {
            size: self.i32(offset)?,
            flags: self.u32(offset + 4)?,
            file_offset: self.i32(offset + 8)?,
            address: self.u32(offset + 12)?,
        })
    }

    fn data_size(&self, offset: usize, d: &DataRef) -> io::Result<usize> {
        usize::try_from(d.size).map_err(|_| self.error(offset, format_args!("negative data size {}", d.size)))
    }

    /// The bytes of the data reference at `offset`, in this view's space
    /// (the default case): empty for size 0, and for address 0 (data the
    /// build stripped, whatever the size). The file-offset word is ignored.
    pub fn data(&self, offset: usize) -> io::Result<&'a [u8]> {
        let d = self.data_ref(offset)?;
        let size = self.data_size(offset, &d)?;
        if size == 0 || d.address == 0 {
            return Ok(&[]);
        }
        let at = self.space.offset_of(d.address, size).ok_or_else(|| {
            self.error(
                offset,
                format_args!(
                    "{size} bytes of data at {:#010x} are outside the {}",
                    d.address, self.space.id
                ),
            )
        })?;
        Ok(&self.map.image()[at..at + size])
    }

    /// The bytes of an external data reference at `offset` (sound
    /// permutation samples): `size` bytes at the file offset in the
    /// decompressed image. The address word is ignored.
    pub fn external_data(&self, offset: usize) -> io::Result<&'a [u8]> {
        let d = self.data_ref(offset)?;
        let size = self.data_size(offset, &d)?;
        if size == 0 {
            return Ok(&[]);
        }
        let start = usize::try_from(d.file_offset)
            .map_err(|_| self.error(offset, format_args!("negative file offset {}", d.file_offset)))?;
        self.map.file_bytes(start, size).map_err(|_| {
            self.error(
                offset,
                format_args!("{size} bytes at file offset {start:#x} past the image"),
            )
        })
    }
}

/// A checked tag block: `count` elements of `elem_size` bytes, all in the
/// space. `get`, `iter` and `read_all` hand out element views.
#[derive(Clone, Copy, Debug)]
pub struct Block<'a> {
    /// The struct holding the block field (for errors and the space).
    view: View<'a>,
    count: usize,
    elem_size: usize,
    /// The elements' bytes as one view; `None` when empty.
    first: Option<View<'a>>,
}

impl<'a> Block<'a> {
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The first element's address (0 when empty).
    pub fn addr(&self) -> u32 {
        self.first.map_or(0, |v| v.addr)
    }

    pub fn elem_size(&self) -> usize {
        self.elem_size
    }

    fn at(&self, i: usize) -> View<'a> {
        let all = self.first.expect("non-empty block");
        let off = i * self.elem_size;
        View {
            addr: all.addr + off as u32,
            bytes: &all.bytes[off..off + self.elem_size],
            ..all
        }
    }

    /// Element `i`; an index past the count is an error (an index field
    /// pointing past its block).
    pub fn get(&self, i: usize) -> io::Result<View<'a>> {
        if i < self.count {
            Ok(self.at(i))
        } else {
            Err(self.view.error(
                0,
                format_args!("element {i} of a block of {} at {:#010x}", self.count, self.addr()),
            ))
        }
    }

    /// Every element, in order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = View<'a>> + use<'a> {
        let b = *self;
        (0..b.count).map(move |i| b.at(i))
    }

    /// Every element read as `T` (whose `SIZE` must be the element size).
    pub fn read_all<T: TagStruct>(&self) -> io::Result<Vec<T>> {
        if T::SIZE != self.elem_size {
            return Err(self.view.error(
                0,
                format_args!("reading {}-byte elements as {} bytes", self.elem_size, T::SIZE),
            ));
        }
        self.iter().map(|v| T::read(&v)).collect()
    }
}

/// Where a view is, for messages: `bloodgulch: weap `weapons\gun``.
struct Place<'a> {
    map: &'a MapFile,
    space: SpaceId,
    tag: Option<u16>,
}

impl fmt::Display for Place<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: ", self.map.header().name)?;
        match self.tag.and_then(|i| self.map.tags().get(i as usize)) {
            Some(t) => write!(f, "{} `{}`", t.class, t.path)?,
            None => write!(f, "{}", self.space)?,
        }
        if let (SpaceId::Bsp(i), Some(_)) = (self.space, self.tag) {
            write!(f, " (bsp {i})")?;
        }
        Ok(())
    }
}

pub(crate) fn invalid(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}
