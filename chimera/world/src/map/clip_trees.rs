//! A map's clip node trees as plain data (`MapClipTrees`, one of
//! `MapData::extras`): solid space precompiled per box size, the way
//! Quake's and GoldSrc's maps hold their collision (specs/goldsrc/bsp.md,
//! "Hulls and collision"). A loader whose maps have them fills it; it is
//! carried in the converted map, so a machine given only the conversion
//! (the cache, the asset service) holds the same trees as one that read the
//! file; the map's spawn inserts it as a resource. What traces them is the
//! loader's (`goldsrc_engine::hulls`, which offers them to the movement as
//! `chimera_movement::MapClipHulls`): nothing here walks a tree.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// A brush model placed in the map: the world, or a brush entity that
/// stays where it is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MapClipModel {
    /// Each hull's root node (`MapClipTrees::nodes`; negative: a contents
    /// value, the whole model is that).
    pub heads: [i32; 3],
    /// Where the model's own origin is, the map's units.
    pub origin: [f32; 3],
}

/// The loaded map's clip node trees (module doc). The map's own units
/// and axes (GoldSrc: inches, Z up).
#[derive(Resource, Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MapClipTrees {
    /// Each hull's box (minimum, maximum) around the traced point.
    pub boxes: [([f32; 3], [f32; 3]); 3],
    /// Planes: normal x, y, z and distance.
    #[serde(with = "super::pod_serde::pod")]
    pub planes: Vec<[f32; 4]>,
    /// Nodes: plane index, front child, back child (a child of 0 or more
    /// is a node, a negative one a contents value).
    #[serde(with = "super::pod_serde::pod")]
    pub nodes: Vec<[i32; 3]>,
    /// The placed models, the world first.
    pub models: Vec<MapClipModel>,
}
