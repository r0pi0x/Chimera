//! The Xbox map cache file (specs/halo_ce/cache_file.md): `MapFile` loads
//! a `.map` (a 2048-byte header and one zlib stream) into its decompressed
//! image, checks it as the spec's load order says, and reads the tag
//! header, the tag index and the scenario's structure BSP list. It is the
//! foundation every tag-class reader is written on.
//!
//! - `MapFile::load` (the file as the disc stores it) or
//!   `MapFile::from_image` (an already decompressed image). Cheap to clone
//!   (an `Arc`), `Send + Sync`.
//! - The index: `tags`, `tag` (by id, with the salt check), `find` (by
//!   class and path, ignoring case), `of_class`, `of_kind` (the class or a
//!   class deriving from it), `scenario`.
//! - Spaces and views (`view.rs`): `tag_view`/`read_tag` for a tag's main
//!   struct, `bsp_view` for a BSP's, `view` for any address; a `View`
//!   reads every primitive type of the spec, blocks, tag references and
//!   data references, all bounds-checked.
//! - Raw bytes: `file_bytes` (bitmap pixels, sound samples: absolute file
//!   offsets), `model_vertex_buffers`/`model_index_buffers` (the tag
//!   header's arrays).
//! - Class readers: `types.rs` (`Field`, `TagStruct`, `tag_struct!`) and
//!   the class codes in `class.rs`.
//!
//! The whole image is held in memory (the largest disc map is 267 MiB);
//! loading inflates straight into it, so the peak is the compressed file
//! plus the image.
//!
//! Known shortcuts:
//! - The scenario's BSP list is read at load but a failure there does not
//!   refuse the map: `bsps` returns the error instead (the spec's
//!   synthetic test map has a scenario too small to hold the list).
//! - Unknown classes are refused (the PC releases' classes); parents are
//!   taken as stored, not checked against the class table.

mod bsp;
pub mod class;
mod types;
mod view;

use std::{fmt, io, sync::Arc};

use chimera_vfs::bytes::Le;
use flate2::{Decompress, FlushDecompress, Status};

pub use bsp::{BSP_LIST_ELEMENT, BSP_LIST_OFFSET, BSP_SUB_HEADER, Bsp, BspHeader, MAX_BSPS};
pub use class::{ClassCode, ClassInfo};
pub use types::{
    Argb8, Bounds, Euler2d, Euler3d, Field, Plane2d, Plane3d, RealAhsv, RealArgb, RealHsv, RealRect2d, RealRect3d,
    RealRgb, Rect2d, Rgb8, TagStruct,
};
pub use view::{Block, DataRef, MAX_BLOCK_COUNT, RawBlock, Space, SpaceId, TagRef, View};

use view::invalid;

/// The header at the start of the file and of the decompressed image.
pub const HEADER_SIZE: usize = 0x800;
/// `head`, header offset 0.
pub const HEAD_MAGIC: u32 = 0x6865_6164;
/// `foot`, header offset 0x7FC.
pub const FOOT_MAGIC: u32 = 0x666F_6F74;
/// The cache version of every Xbox map; the only one accepted.
pub const XBOX_CACHE_VERSION: i32 = 5;
/// `tags`, the tag header's last field.
pub const TAGS_MAGIC: u32 = 0x7461_6773;
/// `sbsp`, a BSP sub-header's last field.
pub const SBSP_MAGIC: u32 = 0x7362_7370;
/// Where the tag data's first byte is loaded.
pub const TAG_BASE: u32 = 0x803A_6000;
/// The memory window the tag data and the loaded BSP share.
pub const TAG_CACHE_SIZE: u32 = 0x0160_0000;
/// `TAG_BASE + TAG_CACHE_SIZE`: every BSP ends here.
pub const TAG_CACHE_END: u32 = TAG_BASE + TAG_CACHE_SIZE;
pub const TAG_HEADER_SIZE: usize = 0x24;
pub const TAG_ENTRY_SIZE: usize = 0x20;
/// One model buffer record (junk, address, junk).
pub const BUFFER_RECORD_SIZE: usize = 12;
/// The engine's upper limit on the decompressed size (a sanity bound).
pub const MAX_DECOMPRESSED_SIZE: u32 = 0x1160_0000;
/// The salt of tag 0's id on every disc map (informative; never predicted).
pub const FIRST_TAG_SALT: u16 = 0xE174;
/// Game ticks per second.
pub const TICK_RATE: u32 = 30;
/// One world unit in metres (10 ft).
pub const WORLD_UNIT_M: f32 = 3.048;
/// The engine's default gravity, world units per tick squared.
pub const GRAVITY: f32 = 0.003_565_179_2;

/// A map's kind (header 0x60, the scenario's own type).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapType {
    Campaign,
    Multiplayer,
    UserInterface,
    /// A value the disc does not use.
    Other(i16),
}

impl MapType {
    fn from_i16(v: i16) -> Self {
        match v {
            0 => MapType::Campaign,
            1 => MapType::Multiplayer,
            2 => MapType::UserInterface,
            v => MapType::Other(v),
        }
    }
}

/// The file header's fields.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub version: i32,
    /// Header plus inflated body, in bytes.
    pub decompressed_size: u32,
    /// Zero bytes after the zlib stream, to the end of the file.
    pub compressed_padding: u32,
    /// File offset of the tag data in the decompressed image.
    pub tag_data_offset: u32,
    pub tag_data_size: u32,
    /// The map's name (`bloodgulch`).
    pub name: String,
    /// The build string (`01.10.12.2276`).
    pub build: String,
    pub map_type: MapType,
    /// Opaque; stored, never recomputed.
    pub checksum: u32,
}

/// The tag header's fields (at the tag data's start).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagHeader {
    pub index_address: u32,
    pub scenario_id: TagId,
    /// Opaque; stored, never recomputed.
    pub checksum: u32,
    pub tag_count: u32,
    pub vertex_buffer_count: u32,
    pub vertex_buffer_address: u32,
    pub index_buffer_count: u32,
    pub index_buffer_address: u32,
}

/// A tag id: low 16 bits the index entry, high 16 a salt. NONE is no tag.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TagId(pub u32);

impl TagId {
    pub const NONE: TagId = TagId(0xFFFF_FFFF);

    pub fn is_none(self) -> bool {
        self == Self::NONE
    }

    /// The index entry it names (unchecked).
    pub fn index(self) -> u16 {
        self.0 as u16
    }

    pub fn salt(self) -> u16 {
        (self.0 >> 16) as u16
    }
}

impl fmt::Debug for TagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TagId({:#010x})", self.0)
    }
}

impl fmt::Display for TagId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:#010x}", self.0)
    }
}

/// One tag index entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagEntry {
    pub class: ClassCode,
    pub parent: Option<ClassCode>,
    pub grandparent: Option<ClassCode>,
    pub id: TagId,
    /// As stored: lower case, backslashes, no extension.
    pub path: String,
    /// The path string's address.
    pub path_address: u32,
    /// The main struct's address; 0 for `sbsp` tags (their struct is in
    /// the BSP's own space: `MapFile::bsp_view`).
    pub address: u32,
}

impl TagEntry {
    /// Its position in the index.
    pub fn index(&self) -> u16 {
        self.id.index()
    }

    /// The tag is of class `c` or derives from it (by the parent slots).
    pub fn is_a(&self, c: ClassCode) -> bool {
        self.class == c || self.parent == Some(c) || self.grandparent == Some(c)
    }
}

/// A loaded map: the decompressed image and what was read from it. Clones
/// share the image.
#[derive(Clone)]
pub struct MapFile(Arc<Inner>);

struct Inner {
    image: Vec<u8>,
    header: Header,
    tag_header: TagHeader,
    tags: Vec<TagEntry>,
    /// Entry indices sorted by (class, path folded to lower case).
    by_path: Vec<u16>,
    vertex_buffers: Vec<u32>,
    index_buffers: Vec<u32>,
    bsps: Result<Vec<Bsp>, String>,
}

impl fmt::Debug for MapFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "MapFile({}, {} tags)", self.0.header.name, self.0.tags.len())
    }
}

fn fold_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    a.bytes()
        .map(|c| c.to_ascii_lowercase())
        .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
}

impl MapFile {
    /// Load a map from the file's bytes as the disc stores them: check the
    /// header, inflate the body, check the padding, then parse the image
    /// (`from_image`).
    pub fn load(file: &[u8]) -> io::Result<MapFile> {
        let header = parse_header(file)?;
        let body = &file[HEADER_SIZE..];
        let padding = header.compressed_padding as usize;
        if padding > body.len() {
            return Err(invalid(format!(
                "compressed padding {padding} is larger than the body ({} bytes)",
                body.len()
            )));
        }
        let mut image = vec![0u8; header.decompressed_size as usize];
        image[..HEADER_SIZE].copy_from_slice(&file[..HEADER_SIZE]);
        let used = inflate(body, &mut image[HEADER_SIZE..])?;
        let rest = &body[used..];
        if rest.len() != padding {
            return Err(invalid(format!(
                "{} bytes follow the zlib stream, the header says the padding is {padding}",
                rest.len()
            )));
        }
        if rest.iter().any(|&b| b != 0) {
            return Err(invalid("the padding after the zlib stream is not zero"));
        }
        Self::parse(image, header)
    }

    /// Parse a decompressed image (header followed by the inflated body).
    pub fn from_image(image: Vec<u8>) -> io::Result<MapFile> {
        let header = parse_header(&image)?;
        if image.len() != header.decompressed_size as usize {
            return Err(invalid(format!(
                "the image is {} bytes, the header says {}",
                image.len(),
                header.decompressed_size
            )));
        }
        Self::parse(image, header)
    }

    fn parse(image: Vec<u8>, header: Header) -> io::Result<MapFile> {
        let name = header.name.clone();
        let ctx = |e: io::Error| invalid(format!("{name}: {e}"));
        let start = header.tag_data_offset as usize;
        let tag_bytes = &image[start..start + header.tag_data_size as usize];
        let tag_header = parse_tag_header(tag_bytes).map_err(ctx)?;
        let tags = parse_index(tag_bytes, &tag_header).map_err(ctx)?;
        let mut by_path: Vec<u16> = (0..tags.len() as u16).collect();
        by_path.sort_by(|&a, &b| {
            let (a, b) = (&tags[a as usize], &tags[b as usize]);
            a.class.cmp(&b.class).then_with(|| fold_cmp(&a.path, &b.path))
        });
        let vertex_buffers = buffer_list(
            tag_bytes,
            TAG_BASE,
            tag_header.vertex_buffer_count,
            tag_header.vertex_buffer_address,
        )
        .map_err(|e| ctx(invalid(format!("model vertex buffers: {e}"))))?;
        let index_buffers = buffer_list(
            tag_bytes,
            TAG_BASE,
            tag_header.index_buffer_count,
            tag_header.index_buffer_address,
        )
        .map_err(|e| ctx(invalid(format!("model index buffers: {e}"))))?;
        let mut map = MapFile(Arc::new(Inner {
            image,
            header,
            tag_header,
            tags,
            by_path,
            vertex_buffers,
            index_buffers,
            bsps: Ok(Vec::new()),
        }));
        let bsps = bsp::read_list(&map).map_err(|e| e.to_string());
        Arc::get_mut(&mut map.0).expect("not shared yet").bsps = bsps;
        Ok(map)
    }

    pub fn header(&self) -> &Header {
        &self.0.header
    }

    pub fn tag_header(&self) -> &TagHeader {
        &self.0.tag_header
    }

    /// The decompressed image, header included.
    pub fn image(&self) -> &[u8] {
        &self.0.image
    }

    /// `len` bytes at an absolute file offset of the decompressed image
    /// (bitmap pixels, sound samples).
    pub fn file_bytes(&self, offset: usize, len: usize) -> io::Result<&[u8]> {
        offset
            .checked_add(len)
            .and_then(|end| self.0.image.get(offset..end))
            .ok_or_else(|| {
                invalid(format!(
                    "{}: {len} bytes at file offset {offset:#x} past the image ({:#x} bytes)",
                    self.0.header.name,
                    self.0.image.len()
                ))
            })
    }

    // The index.

    /// Every index entry, in index order (entry i has index i).
    pub fn tags(&self) -> &[TagEntry] {
        &self.0.tags
    }

    /// The tag an id names: `None` for NONE, an index past the count, or a
    /// salt that does not match the entry's (an invalid id: no tag).
    pub fn tag(&self, id: TagId) -> Option<&TagEntry> {
        let e = self.0.tags.get(id.index() as usize)?;
        (e.id == id).then_some(e)
    }

    /// The tag of class `class` (exactly) at `path`, ignoring ASCII case.
    pub fn find(&self, class: ClassCode, path: &str) -> Option<&TagEntry> {
        let tags = &self.0.tags;
        let i = self
            .0
            .by_path
            .binary_search_by(|&i| {
                let t = &tags[i as usize];
                t.class.cmp(&class).then_with(|| fold_cmp(&t.path, path))
            })
            .ok()?;
        Some(&tags[self.0.by_path[i] as usize])
    }

    /// The tags whose own class is `class`, in index order.
    pub fn of_class(&self, class: ClassCode) -> impl Iterator<Item = &TagEntry> {
        self.0.tags.iter().filter(move |t| t.class == class)
    }

    /// The tags that are `class` or derive from it (`of_kind(class::OBJECT)`
    /// gives every weapon, biped, scenery ...), in index order.
    pub fn of_kind(&self, class: ClassCode) -> impl Iterator<Item = &TagEntry> {
        self.0.tags.iter().filter(move |t| t.is_a(class))
    }

    /// The scenario tag (entry 0; checked at load).
    pub fn scenario(&self) -> &TagEntry {
        &self.0.tags[0]
    }

    // Spaces and views.

    /// The tag data's address space.
    pub fn tag_space(&self) -> Space {
        Space {
            id: SpaceId::Tags,
            base: TAG_BASE,
            file_offset: self.0.header.tag_data_offset as usize,
            len: self.0.header.tag_data_size as usize,
        }
    }

    /// The scenario's structure BSPs (its BSP list), or why the list could
    /// not be read.
    pub fn bsps(&self) -> io::Result<&[Bsp]> {
        self.0.bsps.as_deref().map_err(|e| invalid(e.clone()))
    }

    /// The address space `id`.
    pub fn space(&self, id: SpaceId) -> io::Result<Space> {
        match id {
            SpaceId::Tags => Ok(self.tag_space()),
            SpaceId::Bsp(i) => self
                .bsps()?
                .get(i as usize)
                .map(Bsp::space)
                .ok_or_else(|| invalid(format!("{}: no bsp {i}", self.0.header.name))),
        }
    }

    /// A view of `len` bytes at `addr` in space `id` (not tied to a tag).
    pub fn view(&self, id: SpaceId, addr: u32, len: usize) -> io::Result<View<'_>> {
        View::new(self, self.space(id)?, None, addr, len)
    }

    /// A view of a tag's main struct, `size` bytes (the class spec's).
    /// `sbsp` tags have none here: use `bsp_view`.
    pub fn tag_view(&self, tag: &TagEntry, size: usize) -> io::Result<View<'_>> {
        View::new(self, self.tag_space(), Some(tag.index()), tag.address, size)
    }

    /// A tag's main struct read as `T` (checking nothing about the class:
    /// pick the tag with `find`, `of_class` or `View::reference`).
    pub fn read_tag<T: TagStruct>(&self, tag: &TagEntry) -> io::Result<T> {
        T::read(&self.tag_view(tag, T::SIZE)?)
    }

    /// A view of BSP `i`'s main struct (`size` bytes), in the BSP's space.
    pub fn bsp_view(&self, i: usize, size: usize) -> io::Result<View<'_>> {
        let bsp = self
            .bsps()?
            .get(i)
            .ok_or_else(|| invalid(format!("{}: no bsp {i}", self.0.header.name)))?;
        let tag = self.tag(bsp.tag.id).map(TagEntry::index);
        View::new(self, bsp.space(), tag, bsp.header.struct_address, size)
    }

    /// The addresses (middle words) of the tag header's model vertex
    /// buffer records, in order.
    pub fn model_vertex_buffers(&self) -> &[u32] {
        &self.0.vertex_buffers
    }

    /// The addresses of the tag header's model index buffer records.
    pub fn model_index_buffers(&self) -> &[u32] {
        &self.0.index_buffers
    }
}

/// Inflate `body` into `out` exactly: the stream must end having filled
/// `out`. Returns the bytes of `body` the stream took.
fn inflate(body: &[u8], out: &mut [u8]) -> io::Result<usize> {
    let mut d = Decompress::new(true);
    loop {
        let (i, o) = (d.total_in() as usize, d.total_out() as usize);
        let status = d
            .decompress(&body[i..], &mut out[o..], FlushDecompress::Finish)
            .map_err(|e| invalid(format!("zlib stream: {e}")))?;
        let (i2, o2) = (d.total_in() as usize, d.total_out() as usize);
        if status == Status::StreamEnd {
            if o2 != out.len() {
                return Err(invalid(format!(
                    "the zlib stream inflates to {o2} bytes, the header says {}",
                    out.len()
                )));
            }
            return Ok(i2);
        }
        if o2 == out.len() {
            // Full: the stream must end without another byte.
            let mut spare = [0u8; 1];
            let status = d
                .decompress(&body[i2..], &mut spare, FlushDecompress::Finish)
                .map_err(|e| invalid(format!("zlib stream: {e}")))?;
            return match status {
                Status::StreamEnd if d.total_out() as usize == out.len() => Ok(d.total_in() as usize),
                _ if d.total_out() as usize > out.len() => Err(invalid(format!(
                    "the zlib stream inflates to more than the header's {} bytes",
                    out.len()
                ))),
                _ => Err(invalid("the zlib stream is truncated")),
            };
        }
        if i2 == i && o2 == o {
            return Err(invalid("the zlib stream is truncated"));
        }
    }
}

fn parse_header(file: &[u8]) -> io::Result<Header> {
    if file.len() < HEADER_SIZE {
        return Err(invalid(format!("{} bytes: shorter than the map header", file.len())));
    }
    let h = &file[..HEADER_SIZE];
    let mut r = Le::new(h);
    let magic = r.u32()?;
    if magic != HEAD_MAGIC {
        return Err(invalid(format!("not a map: header magic {magic:#010x}")));
    }
    let foot = u32::from_le_bytes(h[0x7FC..].try_into().expect("4 bytes"));
    if foot != FOOT_MAGIC {
        return Err(invalid(format!("not a map: footer magic {foot:#010x}")));
    }
    let version = r.i32()?;
    if version != XBOX_CACHE_VERSION {
        return Err(invalid(format!(
            "cache version {version}: only the Xbox's {XBOX_CACHE_VERSION} is read"
        )));
    }
    let fields = [r.i32()?, r.i32()?, r.i32()?, r.i32()?];
    let names = [
        "decompressed size",
        "compressed padding",
        "tag data offset",
        "tag data size",
    ];
    let mut v = [0u32; 4];
    for ((out, f), n) in v.iter_mut().zip(fields).zip(names) {
        *out = u32::try_from(f).map_err(|_| invalid(format!("negative {n} {f}")))?;
    }
    let [decompressed_size, compressed_padding, tag_data_offset, tag_data_size] = v;
    let text = |at: usize, what: &str| -> io::Result<String> {
        let b = &h[at..at + 32];
        let end = b
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| invalid(format!("the map's {what} has no NUL in its 32 bytes")))?;
        Ok(String::from_utf8_lossy(&b[..end]).into_owned())
    };
    let name = text(0x20, "name")?;
    let build = text(0x40, "build string")?;
    let map_type = MapType::from_i16(i16::from_le_bytes([h[0x60], h[0x61]]));
    let checksum = u32::from_le_bytes(h[0x64..0x68].try_into().expect("4 bytes"));

    let ctx = |what: String| invalid(format!("{name}: {what}"));
    if !(HEADER_SIZE as u32..=MAX_DECOMPRESSED_SIZE).contains(&decompressed_size) {
        return Err(ctx(format!("decompressed size {decompressed_size:#x} out of range")));
    }
    if tag_data_offset < HEADER_SIZE as u32 {
        return Err(ctx(format!("tag data offset {tag_data_offset:#x} inside the header")));
    }
    if tag_data_offset as u64 + tag_data_size as u64 > decompressed_size as u64 {
        return Err(ctx(format!(
            "tag data {tag_data_offset:#x}+{tag_data_size:#x} beyond the image ({decompressed_size:#x} bytes)"
        )));
    }
    if tag_data_size > TAG_CACHE_SIZE {
        return Err(ctx(format!("tag data size {tag_data_size:#x} over the tag cache")));
    }
    Ok(Header {
        version,
        decompressed_size,
        compressed_padding,
        tag_data_offset,
        tag_data_size,
        name,
        build,
        map_type,
        checksum,
    })
}

/// The tag header, checked: magic, base, a sane count.
fn parse_tag_header(tags: &[u8]) -> io::Result<TagHeader> {
    let mut r = Le::new(tags);
    let mut w = [0u32; 9];
    for v in &mut w {
        *v = r.u32().map_err(|_| invalid("tag data shorter than the tag header"))?;
    }
    let [
        index_address,
        scenario_id,
        checksum,
        count,
        vcount,
        vaddr,
        icount,
        iaddr,
        magic,
    ] = w;
    if magic != TAGS_MAGIC {
        return Err(invalid(format!("tag header magic {magic:#010x}, not `tags`")));
    }
    let base = index_address.wrapping_sub(TAG_HEADER_SIZE as u32);
    if base != TAG_BASE {
        return Err(invalid(format!(
            "tag index at {index_address:#010x}: base {base:#010x}, not {TAG_BASE:#010x}"
        )));
    }
    if !(1..=0xFFFF).contains(&count) {
        return Err(invalid(format!("tag count {} out of 1..=65535", count as i32)));
    }
    for (c, what) in [(vcount, "model vertex buffer"), (icount, "model index buffer")] {
        if c > i32::MAX as u32 {
            return Err(invalid(format!("negative {what} count {}", c as i32)));
        }
    }
    Ok(TagHeader {
        index_address,
        scenario_id: TagId(scenario_id),
        checksum,
        tag_count: count,
        vertex_buffer_count: vcount,
        vertex_buffer_address: vaddr,
        index_buffer_count: icount,
        index_buffer_address: iaddr,
    })
}

/// `n` bytes at `addr` of the tag data, if they lie in it.
fn tag_slice(tags: &[u8], addr: u32, n: usize) -> Option<&[u8]> {
    let rel = addr.checked_sub(TAG_BASE)? as usize;
    tags.get(rel..rel.checked_add(n)?)
}

fn parse_index(tags: &[u8], th: &TagHeader) -> io::Result<Vec<TagEntry>> {
    let count = th.tag_count as usize;
    let index = tag_slice(tags, th.index_address, count * TAG_ENTRY_SIZE)
        .ok_or_else(|| invalid(format!("tag index of {count} entries runs past the tag data")))?;
    let mut out = Vec::with_capacity(count);
    for (i, e) in index.as_chunks::<TAG_ENTRY_SIZE>().0.iter().enumerate() {
        let w = |k: usize| u32::from_le_bytes(e[k * 4..k * 4 + 4].try_into().expect("4 bytes"));
        let class = ClassCode::from_u32(w(0));
        let id = TagId(w(3));
        let (path_address, address) = (w(4), w(5));
        let at = |what: String| invalid(format!("tag entry {i} ({class}): {what}"));
        if !class::is_known(class) {
            return Err(at("not a class of the Xbox game".into()));
        }
        if id.index() as usize != i {
            return Err(at(format!("id {id} names entry {}", id.index())));
        }
        let rel = path_address
            .checked_sub(TAG_BASE)
            .map(|r| r as usize)
            .filter(|&r| r < tags.len())
            .ok_or_else(|| at(format!("path at {path_address:#010x} outside the tag data")))?;
        let rest = &tags[rel..];
        let end = rest
            .iter()
            .position(|&c| c == 0)
            .ok_or_else(|| at("path without a NUL before the tag data's end".into()))?;
        let path = String::from_utf8_lossy(&rest[..end]).into_owned();
        if address != 0 && tag_slice(tags, address, 1).is_none() {
            return Err(at(format!("struct at {address:#010x} outside the tag data")));
        }
        out.push(TagEntry {
            class,
            parent: ClassCode::from_slot(w(1)),
            grandparent: ClassCode::from_slot(w(2)),
            id,
            path,
            path_address,
            address,
        });
    }
    let scenario = &out[0];
    if th.scenario_id != scenario.id {
        return Err(invalid(format!(
            "the tag header's scenario id {} is not entry 0 ({})",
            th.scenario_id, scenario.id
        )));
    }
    if scenario.class != class::SCENARIO {
        return Err(invalid(format!("entry 0 is {}, not the scenario", scenario.class)));
    }
    Ok(out)
}

/// The middle words of `count` 12-byte buffer records at `addr` in a space
/// whose bytes are `space` loaded at `base`.
pub(crate) fn buffer_list(space: &[u8], base: u32, count: u32, addr: u32) -> io::Result<Vec<u32>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    let n = count as usize;
    let len = n
        .checked_mul(BUFFER_RECORD_SIZE)
        .ok_or_else(|| invalid(format!("{count} records: too many")))?;
    let rel = addr
        .checked_sub(base)
        .map(|r| r as usize)
        .filter(|&r| r.checked_add(len).is_some_and(|e| e <= space.len()))
        .ok_or_else(|| invalid(format!("{count} records at {addr:#010x} run outside their space")))?;
    Ok(space[rel..rel + len]
        .as_chunks::<BUFFER_RECORD_SIZE>()
        .0
        .iter()
        .map(|r| u32::from_le_bytes(r[4..8].try_into().expect("4 bytes")))
        .collect())
}
