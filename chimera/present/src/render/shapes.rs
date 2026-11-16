//! Draws code-built shapes (`map::shapes::CodeShape`): a mesh of the box or
//! cylinder and a plain matte material of its colour, added once to each
//! new shape's entity (and again when its shape changes). No asset, so they
//! show on every map, the greybox too.

use bevy::prelude::*;

use crate::map::shapes::{CodeShape, ShapeKind};

/// Give new or changed shapes their mesh and material.
pub fn draw_shapes(
    shapes: Query<(Entity, &CodeShape), Changed<CodeShape>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut commands: Commands,
) {
    for (e, s) in &shapes {
        let mesh = match s.kind {
            ShapeKind::Box(half) => meshes.add(Cuboid::from_size(half * 2.0)),
            ShapeKind::Cylinder { radius, half_height } => meshes.add(Cylinder::new(radius, half_height * 2.0)),
        };
        let material = materials.add(StandardMaterial { base_color: s.colour, perceptual_roughness: 0.8, ..default() });
        // `try_`: a shape can be gone by the time this applies.
        commands.entity(e).try_insert((Mesh3d(mesh), MeshMaterial3d(material)));
    }
}
