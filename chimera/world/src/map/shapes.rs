//! Code-built shapes the simulation asks to be drawn (a drawn-part request,
//! docs/ARCHITECTURE.md): a box or a cylinder of one colour, centred on the
//! entity's transform (`CodeShape`), drawn by `render::shapes` with no
//! game asset, so on any map, the greybox too. The sandbox's props
//! (`chimera_sim::minigame::targets`: bottles, tin cans, a target board)
//! are made of them; their collision is the simulation's own (a collider
//! beside the shape, `Hittable` for what shots strike as objects). Nothing
//! here draws; without `render` (a dedicated server) they stay plain
//! entities.

use bevy::prelude::*;

/// A code-built shape that is a thing shots strike (the sandbox's prop
/// pieces and target board), not part of the level: on its collider's
/// body, so a game whose projectiles trace the level apart from objects
/// (Halo's) reports it as an object it struck (a `core::Damage` on it,
/// with the point) instead of a wall.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hittable;

/// A shape's form, in its entity's frame (m).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShapeKind {
    /// A box of these half extents.
    Box(Vec3),
    /// A cylinder along the local Y axis.
    Cylinder { radius: f32, half_height: f32 },
}

/// Draw this entity as `kind` in `colour` (render: a plain matte
/// material).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
#[require(Transform, Visibility)]
pub struct CodeShape {
    pub kind: ShapeKind,
    pub colour: Color,
}
