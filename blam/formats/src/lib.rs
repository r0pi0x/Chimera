//! Halo: Combat Evolved's file formats as plain data (the `blam/` loader
//! layer, beside `source/` and `lithtech/`; docs/ARCHITECTURE.md,
//! "Crates"): bytes in, Rust values out, no Bevy and no game code. A game
//! crate (`halo_ce`, later) converts what these read into Chimera's types.
//!
//! - `disc`: the Xbox disc image's file system (volume descriptor,
//!   directory trees, partition probing) as a mount layer
//!   (`chimera_vfs::FileSource`) over an image file or bytes, and
//!   `open_install`, which mounts an image or a folder of the disc's files.
//! - `cache`: the map cache file (`.map`): the decompressed image, the tag
//!   index, address spaces and bounds-checked views of tag data, the
//!   foundation every tag-class reader is written on.

pub mod cache;
pub mod disc;
