//! Synthetic structure BSPs for the structure BSP and collision readers'
//! tests (built in code from specs/halo_ce/structure_bsp.md and
//! collision.md, no game data): `map_with_bsp` wraps a BSP built by a
//! closure into a map; `spec_structure` is structure_bsp.md's minimal BSP
//! (one lightmap group, one material, a two-triangle quad); `box_room` is
//! collision.md's closed box room as a collision BSP struct.

#![allow(dead_code)]

use blam_formats::cache::MapFile;

use crate::map_fixture::{Bsp, CacheFile, Heap, Struct, TagData, bsp_element, null_ref, tag_ref_bytes};

pub const MAIN_SIZE: usize = 0x288;

/// A map whose scenario lists one structure BSP of `size` bytes, built by
/// `build`: it appends structs to the BSP's heap bottom-up and returns the
/// 0x288-byte main struct.
pub fn map_with_bsp(size: usize, build: impl FnOnce(&mut Heap) -> Struct) -> MapFile {
    let mut td = TagData::new(&[("scnr", "levels\t\t"), ("sbsp", "levels\t\t")]);
    let mut bsp = Bsp::new(size);
    let main = build(&mut bsp.heap);
    bsp.set_struct(&main.0);
    bsp.buffers(&[], &[]);
    let list = td.block(&[bsp_element(0x800, size as u32, td.tag_ref(1))]);
    td.set_struct(0, &Struct::new(0x5B0).put(0x5A4, &list).0);
    let region = bsp.finish();
    MapFile::from_image(CacheFile::with_bsps("synthetic", 1, region, td.finish()).image()).unwrap()
}

/// What `spec_structure` puts in the material, so tests can break one
/// thing at a time.
#[derive(Clone, Debug)]
pub struct SpecParts {
    /// Compressed vertex data (4 x 32 + 4 x 8 bytes) or any other data.
    pub data: Vec<u8>,
    pub rendered_form: i16,
    pub vertices: i32,
    pub lightmap_form: i16,
    pub lightmap_vertices: i32,
    pub surfaces: Vec<[u16; 3]>,
    /// First surface and count of the material.
    pub material_surfaces: (i32, i32),
    /// Which data reference holds `data` (0xEC compressed, 0xD8 not).
    pub data_ref_at: usize,
}

pub fn packed_vertex(pos: [f32; 3], uv: [f32; 2]) -> Vec<u8> {
    Struct::new(32)
        .f32s(0, &pos)
        .u32(0x0C, 0x7FC0_0000)
        .u32(0x10, 0x0020_0000)
        .u32(0x14, 0x0000_03FF)
        .f32s(0x18, &uv)
        .0
}

impl SpecParts {
    /// structure_bsp.md's synthetic structure.
    pub fn spec() -> SpecParts {
        let quad = [
            ([0., 0., 0.], [0., 1.]),
            ([1., 0., 0.], [1., 1.]),
            ([1., 1., 0.], [1., 0.]),
            ([0., 1., 0.], [0., 0.]),
        ];
        let lm = [(0i16, 32767i16), (32767, 32767), (32767, 0), (0, 0)];
        let mut data: Vec<u8> = quad.iter().flat_map(|(p, uv)| packed_vertex(*p, *uv)).collect();
        for (u, v) in lm {
            data.extend(0x7FC0_0000u32.to_le_bytes());
            data.extend(u.to_le_bytes());
            data.extend(v.to_le_bytes());
        }
        SpecParts {
            data,
            rendered_form: 1,
            vertices: 4,
            lightmap_form: 3,
            lightmap_vertices: 4,
            surfaces: vec![[0, 3, 2], [0, 2, 1]],
            material_surfaces: (0, 2),
            data_ref_at: 0xEC,
        }
    }
}

/// The structure BSP of `p` as a map (a 0x2000-byte BSP).
pub fn structure_map(p: &SpecParts) -> MapFile {
    map_with_bsp(0x2000, |h| {
        let data_ref = h.data(&p.data);
        let surfaces: Vec<Vec<u8>> = p
            .surfaces
            .iter()
            .map(|t| Struct::new(6).u16(0, t[0]).u16(2, t[1]).u16(4, t[2]).0)
            .collect();
        let surface_block = h.block(&surfaces);
        let mut m = Struct::new(256)
            .put(0, &null_ref("senv"))
            .i32(0x14, p.material_surfaces.0)
            .i32(0x18, p.material_surfaces.1)
            .i16(0xAC, -1)
            .i16(0xB0, p.rendered_form)
            .i32(0xB4, p.vertices)
            .i16(0xC4, p.lightmap_form)
            .i32(0xC8, p.lightmap_vertices);
        m.put_mut(p.data_ref_at, &data_ref);
        let mats = h.block(&[m.0]);
        let group = h.block(&[Struct::new(32).i16(0, 0).put(0x14, &mats).0]);
        Struct::new(MAIN_SIZE)
            .put(0, &null_ref("bitm"))
            .f32s(0xC8, &[0.0, 1.0, 0.0, 1.0, 0.0, 0.0])
            .put(0xF8, &surface_block)
            .put(0x104, &group)
    })
}

/// A plane element.
fn plane(i: f32, j: f32, k: f32, d: f32) -> Vec<u8> {
    Struct::new(16).f32s(0, &[i, j, k, d]).0
}

fn node3(plane: i32, back: i32, front: i32) -> Vec<u8> {
    Struct::new(12).i32(0, plane).i32(4, back).i32(8, front).0
}

/// collision.md's box room (interior x, y in 0..4, z in 0..3, one material
/// of type 2) appended to `h`: the 0x60-byte collision BSP struct.
pub fn box_room(h: &mut Heap) -> Struct {
    let verts: [([f32; 3], i32); 8] = [
        ([0., 0., 0.], 0),
        ([4., 0., 0.], 0),
        ([4., 4., 0.], 1),
        ([0., 4., 0.], 2),
        ([0., 0., 3.], 4),
        ([4., 0., 3.], 6),
        ([4., 4., 3.], 5),
        ([0., 4., 3.], 4),
    ];
    let vertices: Vec<Vec<u8>> = verts
        .iter()
        .map(|(p, e)| Struct::new(16).f32s(0, p).i32(12, *e).0)
        .collect();
    let planes = [
        plane(0., 0., 1., 0.),
        plane(0., 0., -1., -3.),
        plane(1., 0., 0., 0.),
        plane(-1., 0., 0., -4.),
        plane(0., 1., 0., 0.),
        plane(0., -1., 0., -4.),
    ];
    let mut nodes: Vec<Vec<u8>> = (0..5).map(|i| node3(i, -1, i + 1)).collect();
    nodes.push(node3(5, -1, 0x8000_0000u32 as i32));
    let leaf = Struct::new(8).u16(0, 0).i16(2, 6).i32(4, 0).0;
    let refs: Vec<Vec<u8>> = (0..6)
        .map(|i| Struct::new(8).i32(0, i).i32(4, 0x8000_0000u32 as i32 | i).0)
        .collect();
    let first_edges = [0, 4, 3, 10, 9, 2];
    let surfaces: Vec<Vec<u8>> = first_edges
        .iter()
        .enumerate()
        .map(|(i, f)| Struct::new(12).i32(0, i as i32).i32(4, *f).i16(10, 0).0)
        .collect();
    let edges: [[i32; 6]; 12] = [
        [0, 1, 1, 9, 0, 4],
        [1, 2, 2, 10, 0, 3],
        [2, 3, 3, 11, 0, 5],
        [3, 0, 0, 8, 0, 2],
        [4, 7, 5, 9, 1, 2],
        [7, 6, 6, 8, 1, 5],
        [6, 5, 7, 11, 1, 3],
        [5, 4, 4, 10, 1, 4],
        [3, 7, 4, 2, 2, 5],
        [4, 0, 3, 7, 2, 4],
        [1, 5, 6, 0, 3, 4],
        [6, 2, 1, 5, 3, 5],
    ];
    let edges: Vec<Vec<u8>> = edges
        .iter()
        .map(|e| {
            let mut s = Struct::new(24);
            for (k, v) in e.iter().enumerate() {
                s = s.i32(4 * k, *v);
            }
            s.0
        })
        .collect();
    Struct::new(0x60)
        .put(0x00, &h.block(&nodes))
        .put(0x0C, &h.block(&planes))
        .put(0x18, &h.block(&[leaf]))
        .put(0x24, &h.block(&refs))
        .put(0x30, &h.block(&[]))
        .put(0x3C, &h.block(&surfaces))
        .put(0x48, &h.block(&edges))
        .put(0x54, &h.block(&vertices))
}

/// A map whose BSP holds the box room as its level collision (one collision
/// material of type 2).
pub fn box_room_map() -> MapFile {
    map_with_bsp(0x2000, |h| {
        let bsp = box_room(h);
        let coll = h.block(&[bsp.0]);
        let mat = h.block(&[Struct::new(0x14)
            .put(0, &tag_ref_bytes(crate::map_fixture::c("senv"), 0, 0xFFFF_FFFF))
            .i16(0x12, 2)
            .0]);
        Struct::new(MAIN_SIZE).put(0xA4, &mat).put(0xB0, &coll)
    })
}
