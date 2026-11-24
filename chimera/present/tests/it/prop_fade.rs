//! Moving props' fade distances (Source's `fademindist`/`fademaxdist` on
//! prop_physics and prop_dynamic): a physics prop is tagged with its far
//! distance (`FadeDistance`, hidden beyond it by the visibility pass) and
//! its meshes get a dither band (`VisibilityRange`), like a static prop.

use bevy::{camera::visibility::VisibilityRange, prelude::*};
use chimera_present::MapPlugin;
use chimera_world::{
    harness::Sim,
    map::{
        MapData, MapMesh, MapModel, MapProp, PropSolid,
        MapPhysics, PushAway,
        vis::{FadeDistance, MapVisibility},
    },
};
use chimera_present::{
    render::{
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
        shadows::ShadowMaterial,
    },
};

fn quad(material: &str, half: f32) -> MapMesh {
    MapMesh {
        material: material.into(),
        positions: vec![
            [-half, 0.0, 0.0],
            [half, 0.0, 0.0],
            [half, 2.0 * half, 0.0],
            [-half, 2.0 * half, 0.0],
        ],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 0.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        color: [128, 128, 128],
        ..default()
    }
}

fn map() -> MapData {
    let mut data = MapData {
        name: "test:prop_fade".into(),
        entity_scale: 0.0254,
        visibility: Some(std::sync::Arc::new(MapVisibility::default())),
        ..default()
    };
    let half = 0.5;
    data.models.push(MapModel {
        meshes: vec![quad("test/pot", half)],
        bounds: (Vec3::new(-half, 0.0, -half), Vec3::new(half, 2.0 * half, half)),
        ..default()
    });
    data.props.push(MapProp {
        pose: None,
        ragdoll: None,
        model: 0,
        translation: Vec3::ZERO,
        rotation: Quat::IDENTITY,
        solid: PropSolid::None,
        skybox: false,
        lighting: None,
        vertex_light: None,
        casts_shadow: false,
        physics: Some(MapPhysics {
            mass: 10.0,
            friction: 1.0,
            elasticity: 0.1,
            damping: 0.0,
            rotdamping: 0.0,
            inertia: None,
            push: PushAway::Collide,
            frozen: true,
            asleep: true,
            debris: false,
        }),
        fade: Some((10.0, 20.0)),
        parent: None,
        entity: None,
        skin: 0,
        body: 0,
        detail: false,
    });
    data
}

#[test]
fn physics_props_carry_their_fade_distance_and_band() {
    let mut sim = Sim::with(base(), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(map()));
    });
    sim.ticks(2);
    let world = sim.app.world_mut();
    let far: Vec<f32> = world.query::<&FadeDistance>().iter(world).map(|f| f.0).collect();
    assert_eq!(far, [20.0], "the physics prop is tagged with fademaxdist");
    let bands = world.query::<&VisibilityRange>().iter(world).count();
    assert_eq!(bands, 1, "its mesh has a fade band");
}

fn base() -> (chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin) {
    (chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin)
}
