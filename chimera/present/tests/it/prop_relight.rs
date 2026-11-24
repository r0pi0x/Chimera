//! A map's physics prop is lit where it is once it moves (the playtest's
//! "moved props keep their lighting"): at rest it wears the light baked into
//! its vertices, shared materials and all; moved or turned, its meshes take
//! own materials with the light field's probe at its centre (shaders.md 4,
//! `probe_lit`). Static props stay baked. A moving model that comes in
//! play (a body built late, a held model) is lit where it stands in the
//! frame it comes, not from the origin or the black of no sample.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_present::{
    MapPlugin,
    render::{
        probe_lit::{ProbeLit, ProbeTable},
        prop_material::PropMaterial, rope_material::RopeMaterial, shadows::ShadowMaterial, sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{LightProbe, MapData, MapLightField, MapMesh, MapModel, MapPhysics, MapProp, PropSolid, PushAway},
};

fn mesh() -> MapMesh {
    MapMesh {
        material: "test/pot".into(),
        positions: vec![[-0.5, 0.0, 0.0], [0.5, 0.0, 0.0], [0.0, 1.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        color: [128, 128, 128],
        ..default()
    }
}

/// A prop at the origin (`physics`: a physics prop, else a static one);
/// the light field is brighter the higher it is sampled.
fn map(physics: bool) -> MapData {
    let mut data = MapData { name: "test:prop_relight".into(), entity_scale: 0.0254, ..default() };
    data.models.push(MapModel {
        meshes: vec![mesh()],
        bounds: (Vec3::new(-0.5, 0.0, -0.1), Vec3::new(0.5, 1.0, 0.1)),
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
        lighting: Some(LightProbe { cube: [Vec3::splat(0.25); 6], lights: Vec::new() }),
        vertex_light: None,
        casts_shadow: false,
        physics: physics.then_some(MapPhysics {
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
        fade: None,
        parent: None,
        entity: None,
        skin: 0,
        body: 0,
        detail: false,
    });
    data.light_field = Some(MapLightField(Arc::new(|p: Vec3| LightProbe {
        cube: [Vec3::splat(p.y.max(0.0)); 6],
        lights: Vec::new(),
    })));
    data
}

fn start(physics: bool) -> Sim {
    Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(map(physics)));
    })
}

/// The prop's node and the material on its mesh.
fn prop_material(sim: &mut Sim) -> (Entity, Handle<PropMaterial>) {
    let world = sim.app.world_mut();
    let mut q = world.query::<(&ChildOf, &MeshMaterial3d<PropMaterial>)>();
    let (parent, m) = q.iter(world).next().expect("the prop's mesh");
    (parent.0, m.0.clone())
}

fn step(sim: &mut Sim) {
    sim.ticks(1);
    sim.app.update();
}

/// Whether the material is probe lit, and its ambient cube's +X red as
/// drawn (its probe table slot's, `ProbeTable::drawn`).
fn lit(sim: &Sim, h: &Handle<PropMaterial>) -> (f32, f32) {
    let world = sim.app.world();
    let m = world.resource::<Assets<PropMaterial>>().get(h).expect("the material");
    (m.params.probe, world.resource::<ProbeTable>().drawn(m)[0][0])
}

#[test]
fn a_physics_prop_is_lit_where_it_goes() {
    let mut sim = start(true);
    sim.ticks(2);
    let (node, baked) = prop_material(&mut sim);
    step(&mut sim);
    assert_eq!(prop_material(&mut sim).1, baked, "at rest it keeps the baked light");
    let move_to = |sim: &mut Sim, y: f32| sim.app.world_mut().get_mut::<Transform>(node).unwrap().translation.y = y;
    move_to(&mut sim, 2.0);
    step(&mut sim);
    step(&mut sim);
    let (_, own) = prop_material(&mut sim);
    assert_ne!(own, baked, "moved: its own material");
    let (probe, light) = lit(&sim, &own);
    assert_eq!(probe, 1.0);
    // Sampled at its centre, half a metre above its origin.
    assert!(light > 2.0 * 0.9, "the light where it went: {light}");
    let was = light;
    move_to(&mut sim, 5.0);
    step(&mut sim);
    step(&mut sim);
    let (_, light) = lit(&sim, &own);
    assert!(light > was + 2.0, "and again where it goes next: {light}");
    assert_eq!(lit(&sim, &baked).0, 0.0, "the shared material isn't touched");
}

/// A moved prop's light changes in the probe table, not in its material:
/// the material (and so its GPU copy, prepared again on every change: a
/// uniform buffer and a bind group) stays unchanged while the prop moves
/// on (docs/performance.md, "Render performance").
#[test]
fn moving_on_changes_the_probe_table_not_the_material() {
    let mut sim = start(true);
    sim.ticks(2);
    let (node, _) = prop_material(&mut sim);
    step(&mut sim);
    let move_to = |sim: &mut Sim, y: f32| sim.app.world_mut().get_mut::<Transform>(node).unwrap().translation.y = y;
    move_to(&mut sim, 2.0);
    step(&mut sim);
    step(&mut sim);
    let (_, own) = prop_material(&mut sim);
    let slot = sim.app.world().resource::<Assets<PropMaterial>>().get(&own).unwrap().params.probe_slot;
    assert_ne!(slot, 0, "lit from a table slot");
    #[derive(Resource, Default)]
    struct Modified(usize);
    sim.app.init_resource::<Modified>().add_systems(
        Last,
        |mut events: MessageReader<AssetEvent<PropMaterial>>, mut n: ResMut<Modified>| {
            n.0 += events.read().filter(|e| matches!(e, AssetEvent::Modified { .. })).count();
        },
    );
    step(&mut sim);
    sim.app.world_mut().resource_mut::<Modified>().0 = 0;
    let mut last = lit(&sim, &own).1;
    for i in 0..8 {
        move_to(&mut sim, 3.0 + i as f32);
        step(&mut sim);
        let now = lit(&sim, &own).1;
        assert!(now > last, "step {i}: relit as it goes: {last} -> {now}");
        last = now;
    }
    let changes = sim.app.world().resource::<Modified>().0;
    assert_eq!(changes, 0, "no prop material changed while it moved");
}

/// A body built late (a class downloaded on demand) or a held model comes
/// with its own materials under a character that already stands somewhere:
/// in the frame it comes it is lit by the light where it stands (its
/// transform propagated first), not by the origin's light from a
/// transform not yet propagated, nor the black of a material not yet
/// pointed at its sample.
#[test]
fn a_model_that_comes_is_lit_where_it_stands_that_frame() {
    let mut sim = start(true);
    sim.ticks(2);
    step(&mut sim);
    let world = sim.app.world_mut();
    let material = world
        .resource_mut::<Assets<PropMaterial>>()
        .add(PropMaterial { params: chimera_present::render::prop_material::PropParams { probe: 1.0, ..default() }, ..default() });
    let character = world.spawn((Transform::from_xyz(3.0, 4.0, 0.0), Visibility::default())).id();
    step(&mut sim);
    // The body arrives under the character in a frame's `Update`, by
    // commands (as `attach_bodies` adds it).
    #[derive(Resource)]
    struct Arrives(Option<(Entity, Handle<PropMaterial>)>);
    sim.app.insert_resource(Arrives(Some((character, material.clone())))).add_systems(
        Update,
        |mut arrives: ResMut<Arrives>, mut commands: Commands| {
            if let Some((character, material)) = arrives.0.take() {
                commands.spawn((
                    Transform::default(),
                    Visibility::Inherited,
                    ChildOf(character),
                    ProbeLit::new(vec![material], Vec3::Y * 0.9),
                ));
            }
        },
    );
    sim.app.update();
    let (probe, light) = lit(&sim, &material);
    assert_eq!(probe, 1.0);
    assert!((light - 4.9).abs() < 1e-3, "the light at its centre, 4.9 m up, this frame: {light}");
}

#[test]
fn turning_counts_as_moving() {
    let mut sim = start(true);
    sim.ticks(2);
    let (node, baked) = prop_material(&mut sim);
    step(&mut sim);
    sim.app.world_mut().get_mut::<Transform>(node).unwrap().rotation = Quat::from_rotation_x(1.0);
    step(&mut sim);
    step(&mut sim);
    assert_ne!(prop_material(&mut sim).1, baked);
}

#[test]
fn a_static_prop_stays_baked() {
    let mut sim = start(false);
    sim.ticks(2);
    let (node, baked) = prop_material(&mut sim);
    step(&mut sim);
    sim.app.world_mut().get_mut::<Transform>(node).unwrap().translation.y = 3.0;
    step(&mut sim);
    step(&mut sim);
    assert_eq!(prop_material(&mut sim).1, baked);
}

/// dm_lockdown's plug and its cable (the playtest's "rope looks broken"): a
/// rope's end parented to a physics prop goes with it, the other stays.
#[test]
fn a_rope_end_follows_its_parent() {
    use chimera_present::render::rope_sway::RopeMotion;
    use chimera_world::map::{MapRope, rope_sway::RopeSway};

    let mut data = map(true);
    data.props[0].entity = Some(0);
    data.entities.push(chimera_world::map::MapEntity {
        keyvalues: vec![("classname".into(), "prop_physics".into()), ("targetname".into(), "plug".into())],
        hulls: Vec::new(),
        mover: false,
        physics: None,
    });
    let nodes: Vec<Vec3> = (0..4).map(|i| Vec3::new(i as f32, 1.0, 0.0)).collect();
    data.ropes.push(MapRope {
        points: chimera_world::map::rope_sway::spline(&nodes, 1).into_iter().map(|(p, ..)| p).collect(),
        v: vec![0.0; 7],
        light: vec![Vec3::ONE; 7],
        width: 0.05,
        sway: Some(RopeSway {
            previous: nodes.clone(),
            nodes: nodes.clone(),
            link: 2.0,
            gravity: Vec3::NEG_Y,
            subdiv: 1,
            wind: false,
        }),
        follow: [Some(0), None],
        ..default()
    });
    let mut sim = Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(data));
    });
    sim.app.world_mut().spawn((Camera3d::default(), Transform::from_xyz(0.0, 0.0, 100.0)));
    sim.ticks(2);
    step(&mut sim);
    let (node, _) = prop_material(&mut sim);
    sim.app.world_mut().get_mut::<Transform>(node).unwrap().translation += Vec3::new(0.0, 2.0, 0.0);
    step(&mut sim);
    step(&mut sim);
    let world = sim.app.world_mut();
    let r = world.query::<&RopeMotion>().iter(world).next().expect("the rope");
    assert!(r.sway.nodes[0].distance(Vec3::new(0.0, 3.0, 0.0)) < 1e-3, "{}", r.sway.nodes[0]);
    assert!(r.sway.nodes[3].distance(nodes[3]) < 1e-3, "{}", r.sway.nodes[3]);
    assert!(r.look.points[0].y > 1.5, "it's redrawn: {}", r.look.points[0]);
}

/// The map's game decides how every probe-lit model on it is lit
/// (`MapLook::model_light`, e.g. Combat Arms' fixed-function rule): the
/// probe table's slot 0 carries the rule to the prop shader, for any
/// model whatever its own game; a map without one leaves it zero (the
/// probe's light as it is).
#[test]
fn the_maps_model_light_reaches_the_probe_table() {
    let mut sim = start(true);
    sim.ticks(2);
    step(&mut sim);
    let header = |sim: &Sim| sim.app.world().resource::<ProbeTable>().get(0).to_vec();
    assert!(header(&sim).iter().all(|r| *r == [0.0; 4]), "no rule on this map");
    let rule = chimera_world::map::ModelLight { emissive: 0.275, ambient: 0.294, diffuse: 0.761, factor: 2.0 };
    sim.app
        .world_mut()
        .insert_resource(chimera_present::render::probe_lit::ModelLightRule(Some(rule)));
    step(&mut sim);
    let rows = header(&sim);
    assert_eq!(rows[0], [1.0, 0.275, 0.294, 0.761]);
    assert_eq!(rows[1], [2.0, 0.0, 0.0, 0.0]);
    sim.app.world_mut().remove_resource::<chimera_present::render::probe_lit::ModelLightRule>();
    step(&mut sim);
    assert!(header(&sim).iter().all(|r| *r == [0.0; 4]), "gone with the map's rule");
}
