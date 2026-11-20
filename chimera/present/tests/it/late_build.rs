//! A model added in play stops no frame (docs/performance.md, "Nothing blocks
//! the main thread"; `render::late_build`): a body, a held model and a view
//! model far larger than any game's are built on tasks and go into `Assets` over
//! frames, and the worst frame's main-thread instructions stay under a limit.
//! Instructions, because the dev box's load makes a limit in time flaky; where
//! the hardware counters aren't open (Windows) only the behaviour is checked.
//! That test runs in a process of its own (`harness::own_process`): Bevy's
//! transform propagation waits in a compute pool scope every frame, and a
//! thread waiting there runs whatever tasks the shared pool holds, so under
//! `cargo test` on a loaded box other tests' tasks went into this frame's
//! count (11-19 M in `mark_dirty_trees` or `sync_simple_transforms`).
//! A late body some character wears is built before those nobody wears, and
//! a body not built yet is never drawn; a late view model a view shows is
//! built before the others. A body built again (its late files arrived:
//! `replace_character_model`) takes over its name, and its wearer changes
//! to it once it is built, wearing the old one until then.

use std::sync::Arc;

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::{core::Intent, metrics::ThreadCounters};
use chimera_present::{
    MapPlugin,
    render::{
        bodies::{CharacterBody, HeldAssets, LATE_AHEAD, LateWanted, PlayModels},
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
        BodyName, Corpse, GameKit, MapBone, MapBrush, MapCharacterModel, MapData, MapHeldModel, MapMesh, MapModel, MapTexture, MapViewModel,
        REPLACED, CharacterModels, add_character_model, add_held_model, add_view_model, replace_character_model,
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
    w.resource::<PlayModels>().built().count() == 2
        && w.resource::<HeldAssets>().0.contains_key(GUN)
        && w.resource::<ViewModelAssets>().has(GUN)
}

#[test]
fn large_models_come_in_without_a_frame_over_the_limit() {
    // The frame's count is this test's work only with the compute pool to
    // itself (the module's doc).
    if !chimera_world::harness::own_process("late_build::large_models_come_in_without_a_frame_over_the_limit") {
        return;
    }
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
            nose: None,
            turned: Vec::new(),
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
        chimera_world::harness::let_tasks_run();
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
    let (mesh, material) = &assets.body(1).expect("the late body").parts[0];
    assert!(w.resource::<Assets<Mesh>>().contains(mesh.id()));
    // Its materials wait outside the store until something draws them.
    assert!(w.resource::<DormantProps>().get(material.id()).is_some());
    assert!(!w.resource::<Assets<PropMaterial>>().contains(material.id()));
}

/// Two meshes of 2,000 vertices, on a texture each.
fn small_model() -> MapModel {
    MapModel {
        meshes: (0..2).map(|i| mesh(2_000, i)).collect(),
        ..default()
    }
}

/// Two 1024 x 1024 textures: a body takes a few frames to go in.
fn small_textures() -> Arc<Vec<MapTexture>> {
    Arc::new(textures().iter().take(2).cloned().collect())
}

/// The model a character's drawn body shows, if any.
fn drawn(sim: &mut Sim) -> Option<usize> {
    let w = sim.app.world_mut();
    w.query::<&CharacterBody>().iter(w).next().map(|b| b.model)
}

/// Many late bodies queued, and a character changing to the last of them:
/// that one is built before the bodies nobody wears (it doesn't wait for
/// every lower index), the character keeps the body it wore meanwhile (a
/// body not built yet is a hole, never read as built), and wears the new
/// one the frame after it is built.
#[test]
fn a_worn_late_body_is_built_before_the_unworn() {
    const LATE: usize = 16;
    let mut sim = sim();
    let e = sim
        .app
        .world_mut()
        .spawn((Intent::default(), Corpse { feet: 0.0 }, Transform::default()))
        .id();
    sim.ticks(2);
    assert_eq!(drawn(&mut sim), Some(0), "the kit's body first");
    let w = sim.app.world_mut();
    for i in 0..LATE {
        add_character_model(w, character(Some(&format!("test:body{i}")), small_model()), small_textures());
    }
    // The last one: index LATE in the map's models (after the kit's own).
    w.entity_mut(e).insert(BodyName(format!("test:body{}", LATE - 1)));
    let built = |sim: &Sim| {
        let p = sim.app.world().resource::<PlayModels>();
        (1..=LATE).filter(|i| p.body(*i).is_some()).collect::<Vec<_>>()
    };
    let mut frames = 0;
    while !built(&sim).contains(&LATE) {
        assert_eq!(drawn(&mut sim), Some(0), "the body it wore stays while the new one is a hole");
        sim.app.update();
        frames += 1;
        assert!(frames < 20_000, "the worn body never came");
        chimera_world::harness::let_tasks_run();
    }
    // At worst behind those already on their way when it was first worn
    // (their tasks run first), not behind every lower index.
    let before = built(&sim);
    assert!(
        before.len() <= LATE_AHEAD + 1,
        "the worn body came after {} unworn ones ({before:?}): it waited behind lower indices",
        before.len() - 1
    );
    sim.app.update();
    assert_eq!(drawn(&mut sim), Some(LATE), "worn once built");
    // The rest still come, in their own time.
    while built(&sim).len() < LATE {
        sim.app.update();
        frames += 1;
        assert!(frames < 20_000, "the unworn bodies never came");
        chimera_world::harness::let_tasks_run();
    }
    assert_eq!(drawn(&mut sim), Some(LATE));
}

/// A worn body replaced by one built again: the old entry renamed out of
/// the way, the wearer on the old one until the new one is built, then on
/// the new one.
#[test]
fn a_replaced_body_takes_over_its_name_once_built() {
    let mut sim = sim();
    let e = sim
        .app
        .world_mut()
        .spawn((Intent::default(), Corpse { feet: 0.0 }, Transform::default(), BodyName(BODY.into())))
        .id();
    let w = sim.app.world_mut();
    add_character_model(w, character(Some(BODY), small_model()), small_textures());
    let mut frames = 0;
    while drawn(&mut sim) != Some(1) {
        sim.app.update();
        frames += 1;
        assert!(frames < 20_000, "the late body never came");
        chimera_world::harness::let_tasks_run();
    }
    replace_character_model(sim.app.world_mut(), character(Some(BODY), small_model()), small_textures());
    sim.app.update();
    let names: Vec<_> = sim.app.world().resource::<CharacterModels>().0.iter().map(|m| m.name.clone()).collect();
    assert_eq!(names[1].as_deref(), Some(format!("{BODY}{REPLACED}").as_str()), "{names:?}");
    assert_eq!(names[2].as_deref(), Some(BODY), "{names:?}");
    while drawn(&mut sim) != Some(2) {
        assert_eq!(drawn(&mut sim), Some(1), "the old body stays until the new one is built");
        sim.app.update();
        frames += 1;
        assert!(frames < 20_000, "the new body never came");
        chimera_world::harness::let_tasks_run();
    }
    assert!(sim.app.world().get::<BodyName>(e).is_some());
}

fn view_model(key: &str) -> MapViewModel {
    MapViewModel {
        key: key.into(),
        model: small_model(),
        bones: bones(),
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    }
}

/// Many late view models queued, and a view showing the last of them (as
/// `draw_view_models` says, `LateWanted::view`): that one is built before
/// the others, not after every one added before it.
#[test]
fn a_shown_late_view_model_is_built_before_the_rest() {
    const LATE: usize = 16;
    let mut sim = sim();
    let key = |i: usize| format!("test:gun{i}");
    let last = key(LATE - 1);
    let wanted = last.clone();
    // After the frame's drawing, which has no view showing it here.
    sim.app.add_systems(Last, move |mut w: ResMut<LateWanted>| {
        w.view.entry(wanted.clone()).or_insert(0.0);
    });
    let w = sim.app.world_mut();
    for i in 0..LATE {
        add_view_model(w, view_model(&key(i)), small_textures());
    }
    let built = |sim: &Sim| {
        let a = sim.app.world().resource::<ViewModelAssets>();
        (0..LATE).filter(|i| a.has(&key(*i))).count()
    };
    let mut frames = 0;
    while !sim.app.world().resource::<ViewModelAssets>().has(&last) {
        sim.app.update();
        frames += 1;
        assert!(frames < 20_000, "the shown view model never came");
        chimera_world::harness::let_tasks_run();
    }
    let before = built(&sim) - 1;
    assert!(before <= LATE_AHEAD, "the shown view model came after {before} others: it waited behind them");
    while built(&sim) < LATE {
        sim.app.update();
        frames += 1;
        assert!(frames < 20_000, "the others never came");
        chimera_world::harness::let_tasks_run();
    }
}
