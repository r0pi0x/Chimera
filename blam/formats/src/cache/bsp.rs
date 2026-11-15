//! The scenario's structure BSP list and each BSP's sub-header
//! (specs/halo_ce/cache_file.md, "Structure BSP sub-files"): where each
//! BSP sits in the decompressed image, its own address space (loaded so
//! that it ends at `TAG_CACHE_END`) and its buffer lists. Read once at
//! load; the BSP's contents belong to the structure BSP reader.

use std::io;

use super::view::{Space, SpaceId, TagRef};
use super::{MapFile, SBSP_MAGIC, TAG_CACHE_END, TAG_CACHE_SIZE, buffer_list, class};

/// The BSP list block's offset in the scenario's main struct.
pub const BSP_LIST_OFFSET: usize = 0x5A4;
/// One BSP list element.
pub const BSP_LIST_ELEMENT: usize = 0x20;
/// A BSP sub-file's header.
pub const BSP_SUB_HEADER: usize = 0x18;
/// The most BSPs a list may hold (the disc's most is 13; the spec gives no
/// maximum, so this is a sanity bound).
pub const MAX_BSPS: usize = 256;

/// A BSP sub-header's fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BspHeader {
    /// The `sbsp` tag's main struct, in the BSP's space.
    pub struct_address: u32,
    pub list_a_count: u32,
    pub list_a_address: u32,
    pub list_b_count: u32,
    pub list_b_address: u32,
}

/// One structure BSP of the scenario's list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bsp {
    /// Its position in the list (its `SpaceId::Bsp`).
    pub index: u16,
    /// File offset of the sub-file in the decompressed image.
    pub file_offset: u32,
    pub size: u32,
    /// Where its first byte is loaded (`TAG_CACHE_END - size`).
    pub load_address: u32,
    /// The `sbsp` tag (resolved and checked at load).
    pub tag: TagRef,
    pub header: BspHeader,
    /// The middle words of buffer list A's records.
    pub buffers_a: Vec<u32>,
    /// The middle words of buffer list B's records.
    pub buffers_b: Vec<u32>,
}

impl Bsp {
    /// The BSP's own address space.
    pub fn space(&self) -> Space {
        Space {
            id: SpaceId::Bsp(self.index),
            base: self.load_address,
            file_offset: self.file_offset as usize,
            len: self.size as usize,
        }
    }
}

/// Read and check the scenario's BSP list.
pub(super) fn read_list(map: &MapFile) -> io::Result<Vec<Bsp>> {
    let scenario = map.tag_view(map.scenario(), BSP_LIST_OFFSET + 12)?;
    let list = scenario.block_max(BSP_LIST_OFFSET, BSP_LIST_ELEMENT, MAX_BSPS)?;
    let tag_size = map.header().tag_data_size;
    let mut out = Vec::with_capacity(list.len());
    for (i, e) in list.iter().enumerate() {
        let fail = |what: String| e.error(0, format_args!("bsp {i}: {what}"));
        let file_offset = e.i32(0)?;
        let size = e.i32(4)?;
        let load_address = e.u32(8)?;
        let tag = e.tag_ref(0x10)?;
        let (Ok(file_offset), Ok(size)) = (u32::try_from(file_offset), u32::try_from(size)) else {
            return Err(fail(format!("negative offset {file_offset} or size {size}")));
        };
        if (size as usize) < BSP_SUB_HEADER {
            return Err(fail(format!("size {size:#x} smaller than its sub-header")));
        }
        let bytes = map
            .file_bytes(file_offset as usize, size as usize)
            .map_err(|_| fail(format!("{size:#x} bytes at {file_offset:#x} past the image")))?;
        if load_address as u64 + size as u64 != TAG_CACHE_END as u64 {
            return Err(fail(format!(
                "loaded at {load_address:#010x} with size {size:#x}, not ending at {TAG_CACHE_END:#010x}"
            )));
        }
        if tag_size as u64 + size as u64 > TAG_CACHE_SIZE as u64 {
            return Err(fail(format!("size {size:#x} overlaps the tag data")));
        }
        match map.tag(tag.id) {
            Some(t) if t.class == class::SCENARIO_STRUCTURE_BSP => {}
            Some(t) => return Err(fail(format!("tag reference names {} `{}`", t.class, t.path))),
            None => return Err(fail(format!("tag reference {} names no tag", tag.id))),
        }
        let w = |k: usize| u32::from_le_bytes(bytes[k * 4..k * 4 + 4].try_into().expect("4 bytes"));
        if w(5) != SBSP_MAGIC {
            return Err(fail(format!("sub-header magic {:#010x}, not `sbsp`", w(5))));
        }
        let header = BspHeader {
            struct_address: w(0),
            list_a_count: w(1),
            list_a_address: w(2),
            list_b_count: w(3),
            list_b_address: w(4),
        };
        let list = |count, addr, name| {
            buffer_list(bytes, load_address, count, addr).map_err(|e| fail(format!("buffer list {name}: {e}")))
        };
        let buffers_a = list(header.list_a_count, header.list_a_address, "A")?;
        let buffers_b = list(header.list_b_count, header.list_b_address, "B")?;
        out.push(Bsp {
            index: i as u16,
            file_offset,
            size,
            load_address,
            tag,
            header,
            buffers_a,
            buffers_b,
        });
    }
    Ok(out)
}
