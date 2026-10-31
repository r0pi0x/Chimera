//! LithTech's file formats as plain data (the `lithtech/` loader layer,
//! beside `source/`; docs/ARCHITECTURE.md, "Crates"): bytes in, Rust
//! values out, no Bevy and no game code. A game crate (Combat Arms)
//! converts what these read into Chimera's neutral types, as `cs_source`
//! does with `source_formats`.
//!
//! - `anim`: skeletal animation maths and playback on a model's
//!   animation data: the combined list with child models, sampling, the
//!   millisecond tracker (keys, looping, transitions), key strings,
//!   layering, node evaluation and skinning matrices.
//! - `attributes`: attribute files ("butes"), the INI-like text documents
//!   with a game's numbers: sections of typed values, strict typed reads.
//! - `dtx`: textures (`.dtx`, version -5): header, mip layout, 32-bit
//!   BGRA and DXT1/3/5 decoded to RGBA8.
//! - `fxf`: the client effect file (`CLIENTFX.FXF`, text form): named
//!   effect groups of timed keys (particle systems, sprites, lights,
//!   camera jitter...) with typed properties.
//! - `ltb`: models (`.ltb`, versions 23 and 25): pieces and levels of
//!   detail with rigid, skinned and vertex-animated meshes, the node
//!   tree, weight sets, child models, animations, sockets and bindings,
//!   bounds-checked.
//! - `rez`: `.rez` resource archives (header, directory, file data) as a
//!   mount layer (`chimera_vfs::FileSource`), and a folder of them
//!   as a `Mount`.
//! - `rez_cipher`: the payload ciphers, chosen by the archive's title (V1
//!   Twofish-CBC, V2 AES-256-CBC) with keys supplied at run time; keys
//!   never live in the repo.
//! - `spr`: sprites (`.spr`): a frame rate and a list of texture paths
//!   (animated textures and model skins).
//! - `world`: world files (the compiled map, `.dat`, version 85): BSPs,
//!   objects, the light grid, blockers and the render data with its
//!   lightmaps, bounds-checked.
//!
//! Each format follows its spec under specs/ (specs/README.md); comments
//! name the spec's sections.

pub mod anim;
pub mod attributes;
pub mod dtx;
pub mod fxf;
pub mod ltb;
pub mod rez;
pub mod rez_cipher;
pub mod spr;
pub mod world;
