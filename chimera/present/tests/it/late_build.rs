//! A model added in play stops no frame (docs/performance.md, "Nothing blocks
//! the main thread"; `render::late_build`): a body, a held model and a view
//! model far larger than any game's are built on tasks and go into `Assets` over
//! frames, and the worst frame's main-thread instructions stay under a limit.
//! Instructions, because the dev box's load makes a limit in time flaky; where
//! the hardware counters aren't open (Windows) only the behaviour is checked.

use std::sync::Arc;

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::metrics::ThreadCounters;
use chimera_present::{
    MapPlugin,
    render::{
        bodies::{HeldAssets, PlayModels},
        prop_material::{DormantProps, PropMaterial},
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        view_model::ViewModelAssets,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        GameKit, MapBone, MapBrush, MapCharacterModel, MapData, MapHeldModel, MapMesh, MapModel, MapTexture, MapViewModel,
        add_character_model, add_held_model, add_view_model,
    },
};

/// The worst frame while the three models come in: little over what a frame
/// costs here. Building them in one frame was hundreds of millions.
const WORST_FRAME: u64 = 8_000_000;

fn mesh(vertices: usize, texture: usize) -> MapMesh {
    MapMesh {
        material: "test/late".into(),
        positions: (0..vertices).map(|i| [i as f32 * 0.001, 0.0, (i % 7) as f32]).collect(),
        normals: vec![[0.0, 1.0, 0.0]; vertices],
        uvs: vec![[0.0, 0.0]; vertices],
        joints: vec![[0, 1, 0, 0]; vertices],
        joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; vertices],
        indices: (0..vertices as u32).collect(),
        texture: Some(texture),
        color: [128, 128, 128],
        ..default()
    }
}

/// Twelve meshes of 40,000 vertices, on a texture each.
fn model() -> MapModel {
    MapModel {
        meshes: (0..12).map(|i| mesh(40_000, i)).collect(),
        ..default()
    }
}

/// Twelve 1024 x 1024 textures whose mip levels are to be made.
fn textures() -> Arc<Vec<MapTexture>> {
    let t = |i: usize| MapTexture {
        name: format!("test/late{i}"),
        srgb: true,
        mips: Vec::new(),
        width: 1024,
        height: 1024,
        rgba8: vec![128; 1024 * 1024 * 4],
        bc: None,
    };
    Arc::new((0..12).map(t).collect())
}

fn bones() -> Vec<MapBone> {
    let bone = |name: &str, parent| MapBone {
        name: name.into(),
        parent,
        position: Vec3::Y,
        rotation: Quat::IDENTITY,
    };
    vec![bone("pelvis", None), bone("hand", Some(0))]
}

fn character(name: Option<&str>, model: MapModel) -> MapCharacterModel {
    MapCharacterModel {
        game: Some("test"),
        team: None,
        name: name.map(String::from),
        class: None,
        model,
        hitboxes: Vec::new(),
        bones: bones(),
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    }
}

fn sim() -> Sim {
    let map = MapData {
        name: "test:late_build".into(),
        collision_brushes: vec![MapBrush::from_box(Vec3::new(-5.0, -1.0, -5.0), Vec3::new(5.0, 0.0, 5.0))],
        kit: GameKit {
            characters: vec![character(None, MapModel::default())],
            ..default()
        },
        ..default()
    };
    let mut sim = Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(map));
    });
    sim.ticks(3);
    sim
}

const GUN: &str = "test:gun";
const BODY: &str = "test:body";

fn done(sim: &Sim) -> bool {
    let w = sim.app.world();
    w.resource::<PlayModels>().bodies.len() == 2
        && w.resource::<HeldAssets>().0.contains_key(GUN)
        && w.resource::<ViewModelAssets>().has(GUN)
}

#[test]
fn large_models_come_in_without_a_frame_over_the_limit() {
    let mut sim = sim();
    assert!(!done(&sim));
    let w = sim.app.world_mut();
    add_character_model(w, character(Some(BODY), model()), textures());
    add_held_model(
        w,
        MapHeldModel {
            key: GUN.into(),
            bone: "hand".into(),
            model: model(),
            muzzle: None,
            attachments: Vec::new(),
            merged: Vec::new(),
            own: None,
        },
        textures(),
    );
    add_view_model(
        w,
        MapViewModel {
            key: GUN.into(),
            model: model(),
            bones: bones(),
            root: Transform::IDENTITY,
            animations: None,
            right_handed: true,
            allow_flipping: false,
            attachments: Vec::new(),
            light_origin: Vec3::ZERO,
        },
        textures(),
    );
    let mut counters = ThreadCounters::current();
    let (mut worst, mut frames) = (0u64, 0u32);
    while !done(&sim) {
        let before = counters.read().hw.map(|h| h.instructions);
        sim.app.update();
        let after = counters.read().hw.map(|h| h.instructions);
        if let (Some(a), Some(b)) = (before, after) {
            worst = worst.max(b - a);
        }
        frames += 1;
        assert!(frames < 20_000, "the models never came");
        // The tasks' turn, on a box with few threads to spare.
        std::thread::sleep(std::time::Duration::from_micros(200));
    }
    eprintln!("{frames} frames, the worst {:.2} M instructions", worst as f64 / 1e6);
    // 144 MB of texels with their mip levels and 12 x 2 MB of vertices, three
    // times over, at most 3 x FILL_BYTES a frame.
    assert!(frames > 10, "came in {frames} frames: not spread");
    if ThreadCounters::current().read().hw.is_some() {
        assert!(worst < WORST_FRAME, "the worst frame took {worst} instructions (limit {WORST_FRAME})");
    }
    // Every asset is in by the time the model is out.
    let w = sim.app.world();
    let assets = w.resource::<PlayModels>();
    let (mesh, material) = &assets.bodies[1].parts[0];
    assert!(w.resource::<Assets<Mesh>>().contains(mesh.id()));
    // Its materials wait outside the store until something draws them.
    assert!(w.resource::<DormantProps>().get(material.id()).is_some());
    assert!(!w.resource::<Assets<PropMaterial>>().contains(material.id()));
}
