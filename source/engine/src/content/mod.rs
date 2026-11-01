//! Source's content converted into Chimera's types (docs/plans/active/
//! hl2dm.md, section 3): what CS:S's loaders did, lifted for every Source
//! game. Materials (lift S3): `material` (VMT/VTF, the map's pak first,
//! then the mount) and `water` (Water materials). Models (lift S4):
//! `props` (MDL through `vmdl`: static props and prop entities, character
//! bodies of the game that asks, held, view and shell models, LODs,
//! ragdolls), `anim` (sequences, animations, events, attachments),
//! `propdata` and `surfaceprops` (the install's prop and surface data),
//! `pcf` (particle systems); with what they read of the map: `ambient`
//! (ambient cubes, world lights, the light probes), `breakables` (gib
//! lists, windows' broken looks) and `trails` (sprite trails, smoke
//! stacks, particle materials). Here: Source's units and axes
//! (`METERS_PER_UNIT`, `to_engine`), the playable box (`playable_bounds`)
//! and entities' vectors (`parse_vector`).
//!
//! Source's pure-data readers (`source_formats`) are re-exported here at
//! the paths these modules name them by (`super::pak`, ...).

pub mod ambient;
pub mod anim;
pub mod breakables;
pub mod material;
pub mod pcf;
pub mod propdata;
pub mod props;
pub mod surfaceprops;
pub mod trails;
pub mod water;

pub use source_formats::{lumps, pak, phy, vhv, vpk, wav};

use bevy::prelude::*;
use vbsp::Bsp;

/// One Hammer unit is one inch.
pub const METERS_PER_UNIT: f32 = 0.0254;

/// Source (x forward, y left, z up) to engine (y up, -z forward), in meters.
/// A rotation, so handedness and winding are preserved.
pub fn to_engine(v: vbsp::Vector) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y) * METERS_PER_UNIT
}

/// The playable world's bounds (Source units), from the world's
/// `world_mins`/`world_maxs`. The map compiler computes these without the
/// 3D skybox, so content outside them (with a margin) is skybox content.
pub struct Bounds {
    lo: Vec3,
    hi: Vec3,
}

impl Bounds {
    const MARGIN: f32 = 64.0;

    /// Engine-space box (meters), margin included.
    pub fn engine(&self) -> (Vec3, Vec3) {
        let a = to_engine(vbsp::Vector {
            x: self.lo.x - Self::MARGIN,
            y: self.lo.y - Self::MARGIN,
            z: self.lo.z - Self::MARGIN,
        });
        let b = to_engine(vbsp::Vector {
            x: self.hi.x + Self::MARGIN,
            y: self.hi.y + Self::MARGIN,
            z: self.hi.z + Self::MARGIN,
        });
        (a.min(b), a.max(b))
    }

    pub fn contains(&self, p: vbsp::Vector) -> bool {
        self.contains_point(Vec3::new(p.x, p.y, p.z))
    }

    /// `contains` for a point in Source units held as a `Vec3`.
    pub fn contains_point(&self, p: Vec3) -> bool {
        p.cmpge(self.lo - Self::MARGIN).all() && p.cmple(self.hi + Self::MARGIN).all()
    }
}

pub fn playable_bounds(bsp: &Bsp) -> Option<Bounds> {
    let world = bsp
        .entities
        .iter()
        .find(|e| e.prop("classname") == Some("worldspawn"))?;
    let v = |key: &'static str| world.prop(key).and_then(parse_vector).map(|v| Vec3::new(v.x, v.y, v.z));
    // Only meaningful when there is a 3D skybox.
    bsp.entities
        .iter()
        .find(|e| e.prop("classname") == Some("sky_camera"))?;
    Some(Bounds {
        lo: v("world_mins")?,
        hi: v("world_maxs")?,
    })
}

/// A keyvalue's "x y z" (Source units), as entities write vectors.
pub fn parse_vector(s: &str) -> Option<vbsp::Vector> {
    let mut it = s.split_whitespace().map(|p| p.parse::<f32>());
    let (x, y, z) = (it.next()?.ok()?, it.next()?.ok()?, it.next()?.ok()?);
    Some(vbsp::Vector { x, y, z })
}
