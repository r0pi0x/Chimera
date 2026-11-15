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
//! - `model`: the render model tag (skeleton, markers, regions, parts with
//!   decoded vertices and triangles).
//! - `sound`: the sound and looping sound tags, the Xbox ADPCM decoder and
//!   the distance attenuation rules.
//! - `animation`: the animation graph tag (index tables as typed slots, unit
//!   selection, uncompressed and compressed node animations sampled to
//!   poses, overlay, replacement and aim blending).
//! - `bitmap`: the bitmap tag reader and pixel decoder (every format on the
//!   disc to RGBA8; block compressed levels also as stored, for a GPU).
//! - `structure_bsp`: a level's render geometry, clusters, portals, fog and
//!   the rest of its structure BSP, vertices decoded on demand.
//! - `collision`: the collision BSP of levels and collision models: polygons,
//!   triangles, the point test and the line trace.
//! - `scenario`: the scenario tag (spawns, netgame flags and equipment,
//!   object placements), the sky and item collection tags and the
//!   rotation convention.
//! - `shader`: the eight shader tag classes and the periodic animation
//!   functions.
//! - `object`: the object tag family (biped, vehicle, equipment, scenery,
//!   devices ...: the shared object base, the unit, item and device layers,
//!   the object functions and their evaluation).
//! - `projectile`: the projectile tag (the object base, flight numbers and the
//!   material responses).
//! - `damage_effect`: the damage effect tag and the damage formulas on its
//!   fields.
//! - `vitality`: a collision model's body and shield vitality, materials and
//!   regions (health and shields).
//! - `globals`: the globals tag's player information, falling damage,
//!   multiplayer information, grenades and weapon list.
//! - `game_type`: the saved game variant record and the 26 stock game types.
//! - `weapon`: the weapon tag (the item and object layers, magazines,
//!   triggers) and the pure formulas on its fields.

pub mod animation;
pub mod bitmap;
pub mod cache;
pub mod collision;
pub mod disc;
pub mod model;
pub mod scenario;
pub mod shader;
pub mod object;
pub mod projectile;
pub mod damage_effect;
pub mod vitality;
pub mod globals;
pub mod game_type;
pub mod weapon;
pub mod sound;
pub mod structure_bsp;
