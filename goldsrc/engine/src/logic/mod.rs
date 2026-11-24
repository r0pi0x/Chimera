//! GoldSrc's map logic (specs/goldsrc/entities.md), for every GoldSrc
//! game: named entities firing one another with delays, kills and use
//! types; logic classes; moving brushes; triggers; walls that switch to
//! their `+a` textures; breakables; light styles, sprites, render changes
//! and ambient sounds; the CS objective entities as data; `+use`.
//!
//! - `world`: the logic world as plain data, stepped tick by tick (its
//!   tests drive it directly), with the firing model, triggers,
//!   breakables, looks and sounds;
//! - `movers`: the moving brushes' shared rules and classes;
//! - `tables`: the spec's sound tables and light style patterns;
//! - `bridge`: the world in the ECS (`GoldSrcLogicPlugin`): built from a
//!   map whose entities name this family (`FAMILY`, set by
//!   `content::entities`), stepped after the characters' movement, its
//!   movers' nodes placed, shown, hidden, made solid or not and their
//!   frames written (the `+a` set through `map::BrushTint` and
//!   `MapMesh::frames`), weapons' damage to breakables, sounds, damage,
//!   explosions and pushes applied.

pub mod bridge;
pub mod movers;
pub mod tables;
pub mod world;

#[cfg(test)]
mod tests;

pub use bridge::{GoldSrcLogic, GoldSrcLogicPlugin, LogicFire};
pub use world::{Effect, Id, Player, UseType, Who, World};

/// The value of `map::entities::LOGIC_FAMILY_KEY` on a GoldSrc map: its
/// logic is this module's.
pub const FAMILY: &str = "goldsrc";
