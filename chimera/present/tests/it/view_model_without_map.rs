//! A view model added while no map's drawing is set up (the greybox, or
//! before the first map: `map::add_view_model`, as Combat Arms' on-demand
//! loads do) is drawn: `render::view_model::assets_without_map` gives the
//! drawing assets of its own, `build_late` builds the model (on a task), and the
//! anchor's view-model camera shows its meshes. Before, only a map's spawn
//! built those assets, so on the greybox the model was in `ViewModels`
//! (and the game's placeholder stepped aside) but nothing drew it. The
//! meshes its `ViewAnimator` hides aren't drawn, and a mesh whose frame
//! the game picks (`ViewAnimator::mesh_frames`) shows that frame.

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
        MapMesh, MapModel, MapTexture, MapViewModel, ViewAnimator, ViewModelAnchor, ViewModelCamera, add_view_model,
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
        chimera_world::harness::let_tasks_run();
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

/// One plain 1 x 1 texture.
fn texel(name: &str, rgba: [u8; 4]) -> MapTexture {
    MapTexture {
        name: name.into(),
        srgb: true,
        mips: Vec::new(),
        width: 1,
        height: 1,
        rgba8: rgba.to_vec(),
        bc: None,
    }
}

/// `ViewAnimator::mesh_frames`: the second mesh has three frames (its
/// texture is frame 0); the game picks frame 2, then 0 again: its own
/// material's base texture follows, the other mesh's never changes.
#[test]
fn a_mesh_shows_the_frame_the_game_picks() {
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
    let mut model = gun();
    model.model.meshes[0].texture = Some(0);
    model.model.meshes[1].texture = Some(0);
    model.model.meshes[1].frames = vec![0, 1, 2];
    let textures = vec![texel("f0", [0, 0, 0, 255]), texel("f1", [255, 0, 0, 255]), texel("f2", [0, 255, 0, 255])];
    add_view_model(sim.app.world_mut(), model, Arc::new(textures));
    sim.app.world_mut().get_mut::<ViewAnimator>(owner).unwrap().key = Some(KEY.into());
    let mut frames = 0;
    while sim.app.world().get_resource::<ViewModelAssets>().is_none_or(|a| !a.has(KEY)) {
        sim.ticks(1);
        chimera_world::harness::let_tasks_run();
        frames += 1;
        assert!(frames < 5000, "never built");
    }
    sim.ticks(2);
    // Each mesh's base texture, in the model's order.
    let bases = |sim: &mut Sim| -> Vec<Option<Handle<Image>>> {
        let world = sim.app.world_mut();
        let body = world.query_filtered::<Entity, With<ViewModelBody>>().single(world).unwrap();
        let parts: Vec<Handle<PropMaterial>> = world
            .query::<(&MeshMaterial3d<PropMaterial>, &ChildOf)>()
            .iter(world)
            .filter(|(_, c)| c.parent() == body)
            .map(|(m, _)| m.0.clone())
            .collect();
        let materials = world.resource::<Assets<PropMaterial>>();
        parts.iter().map(|m| materials.get(m).and_then(|m| m.base.clone())).collect()
    };
    let before = bases(&mut sim);
    assert_eq!(before.len(), 2);
    assert!(before[0].is_some() && before[0] == before[1], "both show texture 0");
    sim.app.world_mut().get_mut::<ViewAnimator>(owner).unwrap().mesh_frames = vec![(1, 2)];
    sim.ticks(2);
    // (The parts' entities need not be in the model's order.)
    let picked = bases(&mut sim);
    assert_eq!(picked.iter().filter(|b| **b == before[0]).count(), 1, "the other mesh keeps its texture");
    assert!(picked.iter().all(Option::is_some), "frame 2 shown on the picked mesh");
    sim.app.world_mut().get_mut::<ViewAnimator>(owner).unwrap().mesh_frames = vec![(1, 0)];
    sim.ticks(2);
    assert_eq!(bases(&mut sim), before, "frame 0 again");
}
