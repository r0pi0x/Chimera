//! Halo's material types as the engine's generic surface names, for
//! footsteps and impacts (`MapMesh::surface`, `MapCollisionSet::surface`):
//! the table in `data/surfaces.toml` (specs/halo_ce/collision.md,
//! "Materials", to chimera_world's `map::impact::SurfaceClass` names).
//! `SURFACES` says so to other games' impacts (`map::impact::
//! SurfaceClasses`): a name of Halo's map is the class of that name.

use std::collections::BTreeMap;

use blam_formats::collision::MaterialType;

/// The table's text, compiled in.
pub const TABLE: chimera_core::game_data::GameData = chimera_core::game_data!("data/surfaces.toml");

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
    let file: File = toml::from_str(&TABLE).expect("data/surfaces.toml parses (a test checks it)");
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

/// The engine's surface class named `name` (the table's names: lower-case
/// class names), None for another name.
pub fn class_of(name: &str) -> Option<crate::map::impact::SurfaceClass> {
    use crate::map::impact::SurfaceClass;
    SurfaceClass::ALL.into_iter().find(|c| class_name(*c).eq_ignore_ascii_case(name))
}

/// The name a Halo map gives surface class `class` (the table's).
pub fn class_name(class: crate::map::impact::SurfaceClass) -> &'static str {
    use crate::map::impact::SurfaceClass::*;
    match class {
        Concrete => "concrete",
        Metal => "metal",
        Wood => "wood",
        Dirt => "dirt",
        Sand => "sand",
        Grass => "grass",
        Snow => "snow",
        Glass => "glass",
        Tile => "tile",
        Plastic => "plastic",
        Cloth => "cloth",
        Flesh => "flesh",
        Water => "water",
        Default => "default",
    }
}

/// Halo's maps' surface names as classes, for other games' impacts on
/// them (`map::impact::SurfaceClassesAppExt`, from the bundle's `sim`).
pub const SURFACES: crate::map::impact::GameSurfaces = crate::map::impact::GameSurfaces {
    game: super::super::GAME,
    classify: class_of,
    name: |c| Some(class_name(c)),
};
