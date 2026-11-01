//! Source's content converted into Chimera's types (docs/plans/active/
//! hl2dm.md, section 3): what CS:S's loaders did, lifted for every Source
//! game. So far materials (lift S3): `material` (VMT/VTF, the map's pak
//! first, then the mount) and `water` (Water materials).
//!
//! Source's pure-data readers (`source_formats`) are re-exported here at
//! the paths these modules name them by (`super::pak`, ...).

pub mod material;
pub mod water;

pub use source_formats::{lumps, pak, phy, vhv, vpk, wav};

/// One Hammer unit is one inch.
pub const METERS_PER_UNIT: f32 = 0.0254;
