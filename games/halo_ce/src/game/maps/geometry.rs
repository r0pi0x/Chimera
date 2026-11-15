//! The structure BSP's render geometry as the engine's meshes
//! (specs/halo_ce/structure_bsp.md, "Render geometry", "Surfaces and
//! triangle lists", "Lightmaps", "Drawing order and transparency").
//!
//! One mesh per material, materials merged when nothing tells them apart:
//! the same lightmap group (so the same page), shader, shader permutation,
//! breakable surface and lit-ness (Hang 'Em High's 176 light cards of one
//! shader are one mesh). Meshes come in the game's drawing order (groups
//! in order, their materials in order, a merged mesh where its first
//! material is). Each vertex: its position (`space::point`), its normal
//! (`space::direction`), its base texture coordinates as stored (the
//! shader's map scales are the mesh's transform: `materials`), and, for a
//! lit material, its lightmap coordinates remapped into the atlas
//! (`lightmap::Packed::uv`); a material without lightmap vertices (the
//! last, page-less group's transparent ones) has none. Triangles are the
//! stored ones with the second and third index swapped
//! (`space::render_triangle`: clockwise-front to counter-clockwise-front).
//! Each material's vertices decode on threads (`StructureBsp::decode`).

use std::{collections::BTreeMap, io};

use blam_formats::{cache::TagRef, structure_bsp::StructureBsp};

use super::{lightmap::Packed, space};
use crate::map::MapMesh;

/// What tells merged materials apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    group: u32,
    shader: u32,
    permutation: i16,
    breakable: Option<u16>,
    lit: bool,
}

/// A mesh of the level and the shader it is drawn with.
pub struct LevelMesh {
    pub mesh: MapMesh,
    pub shader: TagRef,
    /// Whether its materials have lightmap coordinates on a page.
    pub lit: bool,
}

/// The level's meshes (module doc). `pages[g]` is group g's page in
/// `packed` (None: no page).
pub fn meshes(bsp: &StructureBsp, pages: &[Option<usize>], packed: Option<&Packed>) -> io::Result<Vec<LevelMesh>> {
    let materials: Vec<_> = bsp.materials().collect();
    let decoded = crate::map::parallel::par_map(materials.clone(), |m| bsp.decode(m));
    let mut out: Vec<LevelMesh> = Vec::new();
    let mut index: BTreeMap<Key, usize> = BTreeMap::new();
    for (m, geometry) in materials.into_iter().zip(decoded) {
        let geometry = geometry?;
        let page = pages.get(m.group as usize).copied().flatten();
        let lit = m.has_lightmap() && page.is_some() && packed.is_some();
        let key = Key {
            group: m.group,
            shader: m.shader.id.0,
            permutation: m.shader_permutation,
            breakable: m.breakable_surface,
            lit,
        };
        let shader_path = bsp.shader_tag(m).map_or_else(String::new, |t| t.path.replace('\\', "/"));
        let i = *index.entry(key).or_insert_with(|| {
            out.push(LevelMesh {
                mesh: MapMesh {
                    material: shader_path,
                    ..Default::default()
                },
                shader: m.shader,
                lit,
            });
            out.len() - 1
        });
        let mesh = &mut out[i].mesh;
        let base = mesh.positions.len() as u32;
        for v in &geometry.vertices {
            mesh.positions.push(space::point(v.position).to_array());
            mesh.normals.push(space::direction(v.normal).normalize_or_zero().to_array());
            mesh.uvs.push(v.uv);
        }
        if let (true, Some(page), Some(packed)) = (lit, page, packed) {
            mesh.lightmap_uvs
                .extend(geometry.lightmap_vertices.iter().map(|l| packed.uv(page, l.uv)));
        }
        for t in &geometry.triangles {
            mesh.indices.extend(space::render_triangle(t.map(|i| base + i)));
        }
    }
    Ok(out)
}
