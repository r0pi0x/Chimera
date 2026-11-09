//! A map's physics prop is lit where it is once it moves (the playtest's
//! "moved props keep their lighting"): at rest it wears the light baked into
//! its vertices, shared materials and all; moved or turned, its meshes take
//! own materials with the light field's probe at its centre (shaders.md 4,
//! `probe_lit`). Static props stay baked.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_present::{
    MapPlugin,
    render::{
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

fn lit(sim: &Sim, h: &Handle<PropMaterial>) -> (f32, f32) {
    let m = sim.app.world().resource::<Assets<PropMaterial>>().get(h).expect("the material");
    (m.params.probe, m.params.probe_cube[0].x)
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
