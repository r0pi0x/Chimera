//! Halo's material types as the engine's generic surface names, for
//! footsteps and impacts (`MapMesh::surface`, `MapCollisionSet::surface`):
//! the table in `data/surfaces.toml` (specs/halo_ce/collision.md,
//! "Materials", to chimera_world's `map::impact::SurfaceClass` names).

use std::collections::BTreeMap;

use blam_formats::collision::MaterialType;

/// The table's text, compiled in.
pub const TABLE: &str = include_str!("../../../data/surfaces.toml");

#[derive(serde::Deserialize)]
struct File {
    types: Vec<Row>,
}

/// One row: a material type's code, Halo's name and the engine's surface.
#[derive(Clone, Debug, PartialEq, Eq, serde::Deserialize)]
pub struct Row {
    pub code: i16,
    pub name: String,
    pub surface: String,
}

/// The table's rows, by code. Parsed on each call (a few dozen lines, a
/// few times a load): no process-wide state.
pub fn table() -> BTreeMap<i16, Row> {
    let file: File = toml::from_str(TABLE).expect("data/surfaces.toml parses (a test checks it)");
    file.types.into_iter().map(|r| (r.code, r)).collect()
}

/// The engine's surface name for a material type.
pub fn surface_name(t: MaterialType) -> Option<String> {
    table().remove(&t.code()).map(|r| r.surface)
}

/// The engine's surface name for a stored material type code (`None` for
/// -1 and codes outside the enumeration).
pub fn surface_of_code(code: i16) -> Option<String> {
    MaterialType::from_code(code).and_then(surface_name)
}
