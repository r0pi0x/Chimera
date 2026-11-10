//! File formats no one game engine owns, as plain data (no Bevy, no
//! loader, no game): `wav` (sound files decoded to 16-bit samples: RIFF
//! WAVE, MP3; every game's sounds become `map::MapSoundClip` through it)
//! and `keyvalues` (KeyValues text: Steam's `libraryfolders.vdf` and
//! `appmanifest_*.acf`, which `mount::install` finds any Steam game's
//! install by, and Source's script files).
//!
//! Under Chimera's base (`chimera_core::mount`, `chimera_world::map::sound`)
//! and every loader layer, so neither Chimera nor a game of another engine
//! depends on Source's format crate for them
//! (docs/plans/active/architecture-pass.md, step 1). `source_formats`
//! re-exports both at their old paths.

pub mod keyvalues;
pub mod wav;
