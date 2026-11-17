//! Stand-ins (`render::stand_in`), for any game, nothing game-specific
//! here: a character with no body drawn yet shows the stand-in body in
//! the loading material, holding the stand-in gun, until its model is
//! built, then the real body takes its place and the stand-in's time on
//! screen is counted; a view whose view model is wanted and not built
//! shows the stand-in gun until it is; `chimera_standins 0` shows none,
//! 2 shows them over the real ones. In a test run the stripes hold still.

use std::sync::Arc;

use bevy::{camera::visibility::RenderLayers, prelude::*};
use chimera_core::{console::Console, core::{Intent, Team}};
use chimera_present::{
    MapPlugin,
    render::{
        bodies::CharacterBody,
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        stand_in::{
            COBALT, StandIn, StandInHeldGun, StandInKind, StandInMaterial, StandInScroll, StandInStats, TEAM_ACCENTS,
            ViewStandIn,
        },
        view_model::ViewModelBody,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::{Sim, let_tasks_run},
    map::{
        BodyName, Held, MapBone, MapCharacterModel, MapData, MapMesh, MapModel, MapViewModel, ViewAnimator,
        ViewModelAnchor, add_character_model, add_view_model, view_model::VIEW_MODEL_LAYER,
    },
};

fn mesh(material: &str) -> MapMesh {
    MapMesh {
        material: material.into(),
        positions: vec![[-0.2, 0.0, 0.0], [0.2, 0.0, 0.0], [0.0, 0.4, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        color: [128; 3],
        ..default()
    }
}

fn body(name: &str) -> MapCharacterModel {
    MapCharacterModel {
        // Late bodies join a map that has bodies of their game.
        game: Some("test"),
        team: None,
        name: Some(name.into()),
        class: None,
        model: MapModel {
            meshes: vec![MapMesh { joints: vec![[0; 4]; 3], joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 3], ..mesh("test/body") }],
            ..default()
        },
        hitboxes: Vec::new(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    }
}

/// A map whose kit has one body, "kit".
fn sim() -> Sim {
    let mut data = MapData { name: "test:stand_ins".into(), entity_scale: 0.0254, ..default() };
    data.kit.characters.push(body("kit"));
    Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), move |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<StandInMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(data.clone()));
    })
}

fn stand_ins(sim: &mut Sim, kind: StandInKind) -> Vec<(Entity, Entity)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &StandIn, &ChildOf)>()
        .iter(w)
        .filter(|(_, s, _)| s.kind == kind)
        .map(|(e, _, c)| (e, c.parent()))
        .collect()
}

fn accent(sim: &Sim, e: Entity) -> Vec4 {
    let w = sim.app.world();
    let handle = &w.get::<MeshMaterial3d<StandInMaterial>>(e).expect("the loading material").0;
    w.resource::<Assets<StandInMaterial>>().get(handle).unwrap().params.accent
}

fn linear(c: Color) -> Vec4 {
    let l = c.to_linear();
    Vec4::new(l.red, l.green, l.blue, 1.0)
}

fn console(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(1);
}

/// A character wearing a body the map doesn't have yet ("late", added
/// and built later, as a game's late bodies are).
#[test]
fn a_missing_body_stands_in_until_its_model_is_built() {
    let mut sim = sim();
    let e = sim
        .app
        .world_mut()
        .spawn((
            Intent::default(),
            Team(1),
            BodyName("late".into()),
            Held(Some("test:rifle".into())),
            avian3d::prelude::ColliderAabb::new(Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.3, 0.9, 0.3)),
            Transform::default(),
        ))
        .id();
    sim.ticks(2);
    let shown = stand_ins(&mut sim, StandInKind::Body);
    assert_eq!(shown.len(), 1, "one stand-in body");
    let (stand_in, parent) = shown[0];
    assert_eq!(parent, e, "on the character");
    assert_eq!(accent(&sim, stand_in), linear(TEAM_ACCENTS[1]), "its team's accent");
    assert!(sim.app.world().get::<Mesh3d>(stand_in).is_some());
    let w = sim.app.world_mut();
    let guns: Vec<Entity> = w.query_filtered::<&ChildOf, With<StandInHeldGun>>().iter(w).map(|c| c.parent()).collect();
    assert_eq!(guns, [stand_in], "holding the stand-in gun");
    assert_eq!(sim.app.world().resource::<StandInScroll>().phase, 0.0, "the stripes hold still in a test");
    // Its model comes (late: built on a task over frames).
    add_character_model(sim.app.world_mut(), body("late"), Arc::new(Vec::new()));
    let mut frames = 0;
    loop {
        sim.ticks(1);
        let_tasks_run();
        let w = sim.app.world_mut();
        if w.query::<&CharacterBody>().iter(w).count() > 0 {
            break;
        }
        frames += 1;
        assert!(frames < 5000, "never built");
    }
    sim.ticks(1);
    assert!(stand_ins(&mut sim, StandInKind::Body).is_empty(), "gone once the real body is drawn");
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&StandInHeldGun>().iter(w).count(), 0, "its gun with it");
    let stats = sim.app.world().resource::<StandInStats>();
    assert_eq!((stats.showing, stats.session[&StandInKind::Body].count), (0, 1), "counted: {stats:?}");
    assert_eq!(stats.runs.len(), 1, "one run of stand-ins ended");
    // `chimera_standins 2` shows it over the real body (hidden), 1 takes it away.
    console(&mut sim, "chimera_standins 2");
    sim.ticks(1);
    assert_eq!(stand_ins(&mut sim, StandInKind::Body).len(), 1);
    let w = sim.app.world_mut();
    let real: Vec<Visibility> = w.query_filtered::<&Visibility, With<CharacterBody>>().iter(w).copied().collect();
    assert_eq!(real, [Visibility::Hidden], "the real body hidden");
    console(&mut sim, "chimera_standins 1");
    sim.ticks(1);
    assert!(stand_ins(&mut sim, StandInKind::Body).is_empty());
    let w = sim.app.world_mut();
    let real: Vec<Visibility> = w.query_filtered::<&Visibility, With<CharacterBody>>().iter(w).copied().collect();
    assert_eq!(real, [Visibility::Inherited], "shown again");
}

#[test]
fn chimera_standins_0_shows_none() {
    let mut sim = sim();
    console(&mut sim, "chimera_standins 0");
    sim.app.world_mut().spawn((
        Intent::default(),
        avian3d::prelude::ColliderAabb::new(Vec3::ZERO, Vec3::splat(0.3)),
        Transform::default(),
    ));
    sim.ticks(3);
    assert!(stand_ins(&mut sim, StandInKind::Body).is_empty());
}

#[test]
fn a_view_model_still_coming_shows_the_stand_in_gun() {
    let mut sim = sim();
    let owner = sim
        .app
        .world_mut()
        .spawn((Transform::default(), Visibility::default(), ViewAnimator { key: Some("test:gun".into()), ..default() }))
        .id();
    sim.app.world_mut().spawn((
        Camera3d::default(),
        ViewModelAnchor,
        Transform::default(),
        Visibility::default(),
        ChildOf(owner),
    ));
    sim.ticks(3);
    let shown = stand_ins(&mut sim, StandInKind::ViewGun);
    assert_eq!(shown.len(), 1, "the stand-in gun while nothing is added");
    let (gun, _) = shown[0];
    assert_eq!(accent(&sim, gun), linear(COBALT));
    let w = sim.app.world();
    assert!(w.get::<ViewStandIn>(gun).is_some());
    assert!(w.get::<RenderLayers>(gun).is_some_and(|l| l.intersects(&RenderLayers::layer(VIEW_MODEL_LAYER))));
    // The model comes: built on a task, then drawn instead.
    let model = MapViewModel {
        key: "test:gun".into(),
        model: MapModel { meshes: vec![mesh("test/gun")], ..default() },
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    };
    add_view_model(sim.app.world_mut(), model, Arc::new(Vec::new()));
    let mut frames = 0;
    loop {
        sim.ticks(1);
        let_tasks_run();
        let w = sim.app.world_mut();
        if w.query::<&ViewModelBody>().iter(w).count() > 0 {
            break;
        }
        assert_eq!(stand_ins(&mut sim, StandInKind::ViewGun).len(), 1, "the stand-in until then");
        frames += 1;
        assert!(frames < 5000, "never built");
    }
    sim.ticks(2);
    assert!(stand_ins(&mut sim, StandInKind::ViewGun).is_empty(), "replaced by the real one");
    assert_eq!(sim.app.world().resource::<StandInStats>().session[&StandInKind::ViewGun].count, 1);
}
