//! Synthetic map cache files built in code from
//! specs/halo_ce/cache_file.md, for the cache reader's tests and every
//! tag-class reader's after it: `spec_a_tag_data` (the spec's test case
//! (a), byte for byte), `TagData` (a tag header, index and paths, then
//! structs, blocks, references and data appended at known addresses),
//! `Bsp` (a structure BSP sub-file in its own space), `Struct` (a struct's
//! bytes by offset) and `CacheFile` (header, image, compressed file).
//!
//! A class reader's test: declare the tags, append the structs bottom-up
//! (elements before the struct holding their block), then build the file.
//!
//! ```ignore
//! let mut td = TagData::new(&[("scnr", "levels\\x"), ("weap", "weapons\\gun")]);
//! let elems = td.block(&[Struct::new(8).i16(0, 1).f32(4, 2.0).into()]);
//! td.set_struct(1, &Struct::new(0x20).put(0, &elems).into());
//! td.set_struct(0, &Struct::new(0x10).put(0, &td.tag_ref(1)).into());
//! let map = MapFile::from_image(CacheFile::new("x", 1, td.finish()).image()).unwrap();
//! ```

#![allow(dead_code)]

use std::io::Write as _;

use blam_formats::cache::{ClassCode, TAG_BASE, TAG_CACHE_END, class};
use flate2::{Compression, write::ZlibEncoder};

pub const HEADER: usize = 0x800;

/// A class code's stored u32 (`c("weap")`).
pub fn c(code: &str) -> u32 {
    ClassCode::from_text(code).as_u32()
}

/// A growable byte buffer written at offsets (zero-filled).
#[derive(Clone, Debug, Default)]
pub struct Struct(pub Vec<u8>);

impl Struct {
    pub fn new(size: usize) -> Self {
        Struct(vec![0; size])
    }

    pub fn put(mut self, off: usize, b: &[u8]) -> Self {
        self.put_mut(off, b);
        self
    }

    pub fn put_mut(&mut self, off: usize, b: &[u8]) {
        if self.0.len() < off + b.len() {
            self.0.resize(off + b.len(), 0);
        }
        self.0[off..off + b.len()].copy_from_slice(b);
    }

    pub fn u8(self, off: usize, v: u8) -> Self {
        self.put(off, &[v])
    }

    pub fn u16(self, off: usize, v: u16) -> Self {
        self.put(off, &v.to_le_bytes())
    }

    pub fn i16(self, off: usize, v: i16) -> Self {
        self.put(off, &v.to_le_bytes())
    }

    pub fn u32(self, off: usize, v: u32) -> Self {
        self.put(off, &v.to_le_bytes())
    }

    pub fn i32(self, off: usize, v: i32) -> Self {
        self.put(off, &v.to_le_bytes())
    }

    pub fn f32(self, off: usize, v: f32) -> Self {
        self.put(off, &v.to_le_bytes())
    }

    pub fn f32s(mut self, off: usize, vs: &[f32]) -> Self {
        for (i, v) in vs.iter().enumerate() {
            self = self.f32(off + 4 * i, *v);
        }
        self
    }

    pub fn str(self, off: usize, s: &str) -> Self {
        self.put(off, s.as_bytes())
    }
}

impl From<Struct> for Vec<u8> {
    fn from(s: Struct) -> Vec<u8> {
        s.0
    }
}

/// A tag reference field's 16 bytes.
pub fn tag_ref_bytes(class: u32, path_address: u32, id: u32) -> [u8; 16] {
    let mut b = [0u8; 16];
    b[0..4].copy_from_slice(&class.to_le_bytes());
    b[4..8].copy_from_slice(&path_address.to_le_bytes());
    b[12..16].copy_from_slice(&id.to_le_bytes());
    b
}

/// A null tag reference expecting `class`.
pub fn null_ref(class: &str) -> [u8; 16] {
    tag_ref_bytes(c(class), 0, 0xFFFF_FFFF)
}

/// A block field's 12 bytes.
pub fn block_bytes(count: i32, address: u32) -> [u8; 12] {
    let mut b = [0u8; 12];
    b[0..4].copy_from_slice(&count.to_le_bytes());
    b[4..8].copy_from_slice(&address.to_le_bytes());
    b
}

/// A data reference field's 20 bytes.
pub fn data_ref_bytes(size: i32, file_offset: i32, address: u32) -> [u8; 20] {
    let mut b = [0u8; 20];
    b[0..4].copy_from_slice(&size.to_le_bytes());
    b[8..12].copy_from_slice(&file_offset.to_le_bytes());
    b[12..16].copy_from_slice(&address.to_le_bytes());
    b
}

/// A heap of structs at known addresses from `base` (4-aligned appends).
#[derive(Clone, Debug)]
pub struct Heap {
    pub base: u32,
    pub bytes: Vec<u8>,
}

impl Heap {
    /// The next append's address.
    pub fn next(&self) -> u32 {
        self.base + self.bytes.len().next_multiple_of(4) as u32
    }

    /// Append `b` (4-aligned); its address.
    pub fn append(&mut self, b: &[u8]) -> u32 {
        let at = self.next();
        self.bytes.resize((at - self.base) as usize, 0);
        self.bytes.extend_from_slice(b);
        at
    }

    /// Append the elements back to back; the block field pointing at them.
    pub fn block(&mut self, elements: &[Vec<u8>]) -> [u8; 12] {
        if elements.is_empty() {
            return block_bytes(0, 0);
        }
        let at = self.next();
        self.bytes.resize((at - self.base) as usize, 0);
        for e in elements {
            self.bytes.extend_from_slice(e);
        }
        block_bytes(elements.len() as i32, at)
    }

    /// Append `b`; a data reference (in this space) to it.
    pub fn data(&mut self, b: &[u8]) -> [u8; 20] {
        if b.is_empty() {
            return data_ref_bytes(0, 0, 0);
        }
        data_ref_bytes(b.len() as i32, 0, self.append(b))
    }

    /// Overwrite bytes at an address already appended.
    pub fn patch(&mut self, addr: u32, b: &[u8]) {
        let at = (addr - self.base) as usize;
        self.bytes[at..at + b.len()].copy_from_slice(b);
    }
}

/// The spec's test case (a): the 0x100 bytes of tag data.
pub fn spec_a_tag_data() -> Vec<u8> {
    let mut s = Struct::new(0x100)
        .u32(0x00, 0x803A_6024)
        .u32(0x04, 0xE174_0000)
        .u32(0x08, 0x1234_5678)
        .u32(0x0C, 2)
        .u32(0x20, 0x7461_6773);
    let entries: [[u32; 8]; 2] = [
        [c("scnr"), !0, !0, 0xE174_0000, 0x803A_6064, 0x803A_6090, 0, 0],
        [
            c("weap"),
            c("item"),
            c("obje"),
            0xE175_0001,
            0x803A_6073,
            0x803A_60C0,
            0,
            0,
        ],
    ];
    for (i, e) in entries.iter().enumerate() {
        for (k, w) in e.iter().enumerate() {
            s = s.u32(0x24 + i * 0x20 + k * 4, *w);
        }
    }
    s = s.str(0x64, "scenarios\\tiny").str(0x73, "weapons\\gun");
    s = s
        .put(0x90, &block_bytes(2, 0x803A_60D8))
        .put(0x9C, &tag_ref_bytes(c("weap"), 0x803A_6073, 0xE175_0001))
        .put(0xAC, &data_ref_bytes(8, 0, 0x803A_60E8))
        .f32(0xC0, 1.5)
        .put(0xC4, &null_ref("proj"))
        .i16(0xD8, 1)
        .i16(0xDA, -1)
        .f32(0xDC, 2.0)
        .i16(0xE0, 2)
        .i16(0xE2, 0)
        .f32(0xE4, -0.5)
        .put(0xE8, &[1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(s.0.len(), 0x100);
    s.0
}

/// Tag data: the tag header, the index and the paths for the tags given
/// up front, then a heap for structs. Parents come from the class table;
/// ids are `0xE174 + i` salts over index `i`; tag 0 should be the scenario.
#[derive(Clone, Debug)]
pub struct TagData {
    pub heap: Heap,
    pub ids: Vec<u32>,
    pub path_addresses: Vec<u32>,
}

impl TagData {
    pub fn new(tags: &[(&str, &str)]) -> Self {
        let n = tags.len();
        let index_end = 0x24 + n * 0x20;
        let mut hi = Struct::new(index_end)
            .u32(0x00, TAG_BASE + 0x24)
            .u32(0x04, 0xE174_0000)
            .u32(0x0C, n as u32)
            .u32(0x20, 0x7461_6773);
        let mut ids = Vec::new();
        let mut paths = Vec::new();
        let mut path_at = index_end;
        for (i, (class_code, path)) in tags.iter().enumerate() {
            let code = ClassCode::from_text(class_code);
            let info = class::class_info(code);
            let parent = info.and_then(|i| i.parent).map_or(!0, |p| p.as_u32());
            let grand = info.and_then(|i| i.grandparent).map_or(!0, |p| p.as_u32());
            let id = ((0xE174 + i as u32) << 16) | i as u32;
            let e = 0x24 + i * 0x20;
            hi = hi
                .u32(e, code.as_u32())
                .u32(e + 4, parent)
                .u32(e + 8, grand)
                .u32(e + 12, id)
                .u32(e + 16, TAG_BASE + path_at as u32)
                .str(path_at, path)
                .u8(path_at + path.len(), 0);
            ids.push(id);
            paths.push(TAG_BASE + path_at as u32);
            path_at += path.len() + 1;
        }
        let heap_start = path_at.next_multiple_of(16);
        hi.0.resize(heap_start, 0);
        TagData {
            heap: Heap {
                base: TAG_BASE,
                bytes: hi.0,
            },
            ids,
            path_addresses: paths,
        }
    }

    pub fn id(&self, i: usize) -> u32 {
        self.ids[i]
    }

    pub fn append(&mut self, b: &[u8]) -> u32 {
        self.heap.append(b)
    }

    pub fn block(&mut self, elements: &[Vec<u8>]) -> [u8; 12] {
        self.heap.block(elements)
    }

    pub fn data(&mut self, b: &[u8]) -> [u8; 20] {
        self.heap.data(b)
    }

    /// Append tag `i`'s main struct and point its entry at it.
    pub fn set_struct(&mut self, i: usize, b: &[u8]) -> u32 {
        let at = self.append(b);
        self.heap
            .patch(TAG_BASE + 0x24 + i as u32 * 0x20 + 0x14, &at.to_le_bytes());
        at
    }

    /// A reference to tag `i` as the disc writes it (own class, shared path).
    pub fn tag_ref(&self, i: usize) -> [u8; 16] {
        let class = &self.heap.bytes[0x24 + i * 0x20..0x24 + i * 0x20 + 4];
        let class = u32::from_le_bytes(class.try_into().unwrap());
        tag_ref_bytes(class, self.path_addresses[i], self.ids[i])
    }

    /// Point the tag header's model buffer arrays at records whose middle
    /// words are `vertex` and `index`.
    pub fn model_buffers(&mut self, vertex: &[u32], index: &[u32]) {
        for (k, list) in [vertex, index].into_iter().enumerate() {
            let recs: Vec<u8> = list
                .iter()
                .flat_map(|a| [0xCAu8; 4].into_iter().chain(a.to_le_bytes()).chain([0; 4]))
                .collect();
            let at = if list.is_empty() { 0 } else { self.append(&recs) };
            self.heap
                .patch(TAG_BASE + 0x10 + 8 * k as u32, &(list.len() as u32).to_le_bytes());
            self.heap.patch(TAG_BASE + 0x14 + 8 * k as u32, &at.to_le_bytes());
        }
    }

    pub fn finish(self) -> Vec<u8> {
        self.heap.bytes
    }
}

/// A structure BSP sub-file of `size` bytes in its own space (loaded at
/// `TAG_CACHE_END - size`): the sub-header, then a heap.
#[derive(Clone, Debug)]
pub struct Bsp {
    pub size: usize,
    pub heap: Heap,
}

impl Bsp {
    pub fn new(size: usize) -> Self {
        let base = TAG_CACHE_END - size as u32;
        let mut heap = Heap {
            base,
            bytes: Vec::new(),
        };
        heap.append(&[0; 0x18]);
        Bsp { size, heap }
    }

    pub fn load_address(&self) -> u32 {
        self.heap.base
    }

    /// Append the BSP's main struct and point the sub-header at it.
    pub fn set_struct(&mut self, b: &[u8]) -> u32 {
        let at = self.heap.append(b);
        self.heap.patch(self.heap.base, &at.to_le_bytes());
        at
    }

    /// Buffer lists A and B (middle words), each right after the other.
    pub fn buffers(&mut self, a: &[u32], b: &[u32]) {
        for (k, list) in [a, b].into_iter().enumerate() {
            let recs: Vec<u8> = list
                .iter()
                .flat_map(|x| [0u8; 4].into_iter().chain(x.to_le_bytes()).chain([0; 4]))
                .collect();
            let at = if list.is_empty() { 0 } else { self.heap.append(&recs) };
            let base = self.heap.base;
            self.heap
                .patch(base + 4 + 8 * k as u32, &(list.len() as u32).to_le_bytes());
            self.heap.patch(base + 8 + 8 * k as u32, &at.to_le_bytes());
        }
    }

    /// The sub-file's bytes (`sbsp` magic, zero-filled to `size`).
    pub fn finish(mut self) -> Vec<u8> {
        let base = self.heap.base;
        self.heap.patch(base + 0x14, &0x7362_7370u32.to_le_bytes());
        assert!(self.heap.bytes.len() <= self.size, "BSP heap over its size");
        self.heap.bytes.resize(self.size, 0);
        self.heap.bytes
    }
}

/// A BSP list element (0x20 bytes): file offset, size, load address, the
/// `sbsp` tag reference.
pub fn bsp_element(file_offset: u32, size: u32, sbsp_ref: [u8; 16]) -> Vec<u8> {
    Struct::new(0x20)
        .u32(0, file_offset)
        .u32(4, size)
        .u32(8, TAG_CACHE_END - size)
        .put(0x10, &sbsp_ref)
        .into()
}

/// A whole map: a header and the image's body (the BSP region, then the
/// tag data, then tail padding).
#[derive(Clone, Debug)]
pub struct CacheFile {
    pub header: Struct,
    /// Everything after the header: BSPs, tag data, tail.
    pub body: Vec<u8>,
}

impl CacheFile {
    /// `tag_data` right after the header.
    pub fn new(name: &str, map_type: i16, tag_data: Vec<u8>) -> Self {
        Self::with_bsps(name, map_type, Vec::new(), tag_data)
    }

    /// The BSP region (the sub-files back to back) at 0x800, then the tag
    /// data.
    pub fn with_bsps(name: &str, map_type: i16, bsps: Vec<u8>, tag_data: Vec<u8>) -> Self {
        let tag_off = HEADER + bsps.len();
        let tag_size = tag_data.len();
        let mut body = bsps;
        body.extend_from_slice(&tag_data);
        let header = Struct::new(HEADER)
            .u32(0x000, 0x6865_6164)
            .u32(0x004, 5)
            .u32(0x008, (HEADER + body.len()) as u32)
            .u32(0x010, tag_off as u32)
            .u32(0x014, tag_size as u32)
            .str(0x020, name)
            .str(0x040, "01.10.12.2276")
            .i16(0x060, map_type)
            .u32(0x064, 0xDEAD_BEEF)
            .u32(0x7FC, 0x666F_6F74);
        CacheFile { header, body }
    }

    /// The decompressed image.
    pub fn image(&self) -> Vec<u8> {
        let mut v = self.header.0.clone();
        v.extend_from_slice(&self.body);
        v
    }

    /// The body as one zlib stream.
    pub fn stream(&self) -> Vec<u8> {
        let mut z = ZlibEncoder::new(Vec::new(), Compression::default());
        z.write_all(&self.body).unwrap();
        z.finish().unwrap()
    }

    /// The file as the disc stores it: header (with the padding P filled
    /// in), stream, P zeros; a multiple of 2048 bytes.
    pub fn file(&self) -> Vec<u8> {
        let stream = self.stream();
        let p = (2048 - (HEADER + stream.len()) % 2048) % 2048;
        file_from(&self.header.clone().u32(0x00C, p as u32), &stream, p)
    }
}

/// A file from a header (taken as is), a stream and `padding` zero bytes.
pub fn file_from(header: &Struct, stream: &[u8], padding: usize) -> Vec<u8> {
    let mut v = header.0.clone();
    v.extend_from_slice(stream);
    v.resize(v.len() + padding, 0);
    v
}
