//! A view model added while no map's drawing is set up (the greybox, or
//! before the first map: `map::add_view_model`, as Combat Arms' on-demand
//! loads do) is drawn: `render::view_model::assets_without_map` gives the
//! drawing assets of its own, `build_late` builds the model (on a task), and the
//! anchor's view-model camera shows its meshes. Before, only a map's spawn
//! built those assets, so on the greybox the model was in `ViewModels`
//! (and the game's placeholder stepped aside) but nothing drew it. The
//! meshes its `ViewAnimator` hides aren't drawn.

use std::sync::Arc;

use bevy::{camera::visibility::RenderLayers, prelude::*};
use chimera_present::{
    MapPlugin,
    render::{
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        view_model::{ViewModelAssets, ViewModelBody},
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        MapMesh, MapModel, MapViewModel, ViewAnimator, ViewModelAnchor, ViewModelCamera, add_view_model,
        view_model::VIEW_MODEL_LAYER,
    },
};

const KEY: &str = "test:gun";

fn gun() -> MapViewModel {
    let mesh = MapMesh {
        material: "test/gun".into(),
        positions: vec![[0.1, -0.1, -0.3], [0.2, -0.1, -0.3], [0.2, 0.0, -0.4]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        color: [128, 128, 128],
        ..default()
    };
    MapViewModel {
        key: KEY.into(),
        model: MapModel {
            meshes: vec![mesh.clone(), mesh],
            ..default()
        },
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    }
}

#[test]
fn a_view_model_added_without_a_map_is_drawn() {
    let mut sim = Sim::with(
        (chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin),
        |app| {
            app.init_asset::<Image>()
                .init_asset::<StandardMaterial>()
                .init_asset::<WorldMaterial>()
                .init_asset::<PropMaterial>()
                .init_asset::<SpriteMaterial>()
                .init_asset::<RopeMaterial>()
                .init_asset::<ShadowMaterial>()
                .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
                .add_plugins(MapPlugin::empty());
        },
    );
    // The local player's eye.
    let owner = sim
        .app
        .world_mut()
        .spawn((Transform::default(), Visibility::default(), ViewAnimator::default()))
        .id();
    sim.app.world_mut().spawn((
        Camera3d::default(),
        ViewModelAnchor,
        Transform::default(),
        Visibility::default(),
        ChildOf(owner),
    ));
    sim.ticks(2);
    assert!(sim.app.world().get_resource::<ViewModelAssets>().is_none(), "no map, no view model yet");
    // Loaded on demand, then shown.
    add_view_model(sim.app.world_mut(), gun(), Arc::new(Vec::new()));
    sim.app.world_mut().get_mut::<ViewAnimator>(owner).unwrap().key = Some(KEY.into());
    sim.ticks(1);
    assert!(sim.app.world().get_resource::<ViewModelAssets>().is_some(), "assets without a map");
    // Built on a task, over frames (`render::late_build`).
    let mut frames = 0;
    while !sim.app.world().resource::<ViewModelAssets>().has(KEY) {
        sim.ticks(1);
        std::thread::sleep(std::time::Duration::from_millis(1));
        frames += 1;
        assert!(frames < 5000, "never built");
    }
    sim.ticks(2);
    let world = sim.app.world_mut();
    let bodies: Vec<(Entity, Visibility, Entity)> = world
        .query::<(Entity, &ViewModelBody, &Visibility, &ChildOf)>()
        .iter(world)
        .map(|(e, _, v, c)| (e, *v, c.parent()))
        .collect();
    assert_eq!(bodies.len(), 1, "one view model drawn");
    let (body, visibility, camera) = bodies[0];
    assert!(world.get::<ViewModelCamera>(camera).is_some(), "under the view-model camera");
    assert_ne!(visibility, Visibility::Hidden, "shown");
    let layer = RenderLayers::layer(VIEW_MODEL_LAYER);
    let meshes = world
        .query::<(&Mesh3d, &RenderLayers, &ChildOf)>()
        .iter(world)
        .filter(|(_, l, c)| c.parent() == body && l.intersects(&layer))
        .count();
    assert_eq!(meshes, 2, "its meshes on the view-model layer");
    // A mesh the game hides (`ViewAnimator::hidden_meshes`: a launcher's
    // fired rocket) isn't drawn, the rest are; shown again once cleared.
    let hidden = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        world
            .query::<(&Mesh3d, &Visibility, &ChildOf)>()
            .iter(world)
            .filter(|(_, v, c)| c.parent() == body && **v == Visibility::Hidden)
            .count()
    };
    sim.app.world_mut().get_mut::<ViewAnimator>(owner).unwrap().hidden_meshes = vec![1];
    sim.ticks(2);
    assert_eq!(hidden(&mut sim), 1, "one mesh hidden");
    sim.app.world_mut().get_mut::<ViewAnimator>(owner).unwrap().hidden_meshes.clear();
    sim.ticks(2);
    assert_eq!(hidden(&mut sim), 0, "both drawn again");
}
