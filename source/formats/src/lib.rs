//! Source engine file formats as plain data (docs/plans/active/engine-split.md,
//! "Crates and compile time": `source/formats`): readers that turn bytes or
//! text into Rust values, with no Bevy, no ECS and no game rules. Turning
//! those values into the engine's types (units, axes, components) is the
//! game's side (`mashup::games::cs_source`).
//!
//! Vectors are `glam` types, the same ones as `bevy::math` (one version
//! across the workspace), in the file's own units and axes (Source:
//! inches, Z up) unless a reader says otherwise.

// Formats Source shares with every engine live in `chimera_formats`
// (sound files, KeyValues text); here at their old paths.
pub use chimera_formats::{keyvalues, wav};

pub mod dem;
pub mod detail;
pub mod localization;
pub mod lumps;
pub mod nav;
pub mod pak;
pub mod pcf;
pub mod phy;
pub mod vhv;
pub mod vpk;
