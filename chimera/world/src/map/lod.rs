//! Models' levels of detail as map data (`MapModel::lods`); drawing the
//! level a prop's size on screen asks for is `render::lod`'s.

use super::MapMesh;

/// One lower level of detail of a model: the metric it takes over at (the
/// `.vtx` switch point) and its meshes, by the slot of the LOD 0 mesh each
/// stands in for (None: none at this level; the finer level's is drawn).
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct MapLod {
    pub switch: f32,
    pub meshes: Vec<Option<MapMesh>>,
}
