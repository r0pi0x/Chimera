//! Decals on character bodies (`render::body_decals`, specs/tf2/effects.md
//! 8b, the engine's rules for every game): a hit puts one on the drawn
//! body, skinned by the body's joints (it follows the pose); at most
//! `r_maxmodeldecal` per body, the oldest replaced; none on a character
//! with `NoBodyDecals` (whose arrival clears them), on the shooter, or on
//! a teammate without friendly fire; `ClearBodyDecals` and living again
//! (`Dead` removed) clear them.

use bevy::{mesh::skinning::SkinnedMesh, prelude::*};
use chimera_core::core::{Dead, FriendlyFire, Team};
use chimera_present::{
    MapPlugin,
    render::{
        body_decals::{BodyDecalLayer, BodyDecalSettings, BodyDecals, pose_part, project_on_body},
        decal::{DecalAssets, DecalMaterial},
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        shadows::ShadowMaterial,
        sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
    },
};
use chimera_world::{
    harness::Sim,
    map::{
        BodyName, ClearBodyDecals, DecalGroup, MapBone, MapCharacterModel, MapData, MapMesh, MapModel, NoBodyDecals,
        PlaceBodyDecal,
        decal::{DecalBlend, MapDecal, MapDecals},
    },
};

/// A 0.4 m chest quad facing +Z (z = 0.1), 0.9 to 1.3 m up, on bone 1.
fn chest() -> MapMesh {
    MapMesh {
        material: "test/body".into(),
        positions: vec![[-0.2, 0.9, 0.1], [0.2, 0.9, 0.1], [0.2, 1.3, 0.1], [-0.2, 1.3, 0.1]],
        normals: vec![[0.0, 0.0, 1.0]; 4],
        uvs: vec![[0.0, 0.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        joints: vec![[1, 0, 0, 0]; 4],
        joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 4],
        color: [128; 3],
        ..default()
    }
}

fn body() -> MapData {
    let mut data = MapData { name: "test:body_decals".into(), entity_scale: 0.0254, ..default() };
    let bone = |name: &str, parent: Option<usize>, y: f32| MapBone {
        name: name.into(),
        parent,
        position: Vec3::Y * y,
        rotation: Quat::IDENTITY,
    };
    data.kit.characters.push(MapCharacterModel {
        game: None,
        team: None,
        name: Some("tester".into()),
        class: None,
        model: MapModel { meshes: vec![chest()], ..default() },
        hitboxes: Vec::new(),
        bones: vec![bone("root", None, 0.0), bone("spine", Some(0), 1.0)],
        root: Transform::IDENTITY,
        animations: None,
        boxes: Vec::new(),
        ragdoll: None,
    });
    data
}

fn decal() -> MapDecal {
    MapDecal {
        texture: 0,
        uv_min: Vec2::ZERO,
        uv_max: Vec2::ONE,
        size: Vec2::splat(0.1),
        blend: DecalBlend::Alpha,
    }
}

fn sim() -> (Sim, Entity, Entity) {
    let mut sim = Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<DecalMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(body()));
    });
    let victim = sim
        .app
        .world_mut()
        .spawn((
            chimera_core::core::Intent::default(),
            BodyName("tester".into()),
            Team(2),
            avian3d::prelude::ColliderAabb::new(Vec3::new(0.0, 0.9, 0.0), Vec3::new(0.3, 0.9, 0.3)),
            Transform::default(),
        ))
        .id();
    let shooter = sim.app.world_mut().spawn((chimera_core::core::Intent::default(), Team(3))).id();
    let mut groups = std::collections::BTreeMap::new();
    groups.insert("impact.flesh".to_string(), vec![(1.0, 0)]);
    let data = MapDecals { decals: vec![decal()], groups, ..default() };
    sim.app.insert_resource(DecalAssets::new(data, vec![Handle::default()]));
    sim.ticks(3);
    (sim, victim, shooter)
}

fn shoot(sim: &mut Sim, target: Entity, owner: Entity) {
    let (from, point) = (Vec3::new(0.0, 1.1, 2.0), Vec3::new(0.0, 1.1, 0.15));
    sim.app.world_mut().write_message(PlaceBodyDecal {
        target,
        owner,
        group: DecalGroup::Named("impact.flesh".into()),
        from,
        point,
        dir: (point - from).normalize(),
    });
}

fn decals(sim: &mut Sim) -> usize {
    let world = sim.app.world_mut();
    world.query::<&BodyDecals>().iter(world).map(BodyDecals::len).sum()
}

fn layers(sim: &mut Sim) -> Vec<(Entity, SkinnedMesh)> {
    let world = sim.app.world_mut();
    world
        .query_filtered::<(Entity, &SkinnedMesh), With<BodyDecalLayer>>()
        .iter(world)
        .map(|(e, s)| (e, s.clone()))
        .collect()
}

#[test]
fn a_hit_decals_the_body_skinned_by_its_joints() {
    let (mut sim, victim, shooter) = sim();
    shoot(&mut sim, victim, shooter);
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 1);
    let layers = layers(&mut sim);
    assert_eq!(layers.len(), 1, "one mesh for the body's one decal material");
    let world = sim.app.world_mut();
    let body_joints = world
        .query_filtered::<&SkinnedMesh, Without<BodyDecalLayer>>()
        .iter(world)
        .next()
        .map(|s| s.joints.clone())
        .expect("the body's part");
    assert_eq!(layers[0].1.joints, body_joints, "skinned by the body's own joints");
    // Bind space: on the chest (z 0.1, lifted) around the hit.
    let mesh = world.query::<&Mesh3d>().get(world, layers[0].0).unwrap().0.clone();
    let meshes = world.resource::<Assets<Mesh>>();
    let Some(bevy::mesh::VertexAttributeValues::Float32x3(p)) = meshes.get(&mesh).unwrap().attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions")
    };
    assert!(p.iter().all(|v| (v[2] - 0.1).abs() < 0.01 && (v[1] - 1.1).abs() < 0.08 && v[0].abs() < 0.08), "{p:?}");
}

#[test]
fn at_most_the_cap_per_body_oldest_first() {
    let (mut sim, victim, shooter) = sim();
    sim.app.world_mut().insert_resource(BodyDecalSettings { draw: 1, cap: 3 });
    for _ in 0..5 {
        shoot(&mut sim, victim, shooter);
        sim.ticks(1);
    }
    sim.ticks(1);
    assert_eq!(decals(&mut sim), 3);
    assert_eq!(BodyDecalSettings::default().cap, 50, "r_maxmodeldecal's default");
}

#[test]
fn none_on_the_shooter_or_a_teammate_without_friendly_fire() {
    let (mut sim, victim, _) = sim();
    let mate = sim.app.world_mut().spawn((chimera_core::core::Intent::default(), Team(2))).id();
    shoot(&mut sim, victim, victim);
    shoot(&mut sim, victim, mate);
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 0);
    sim.app.world_mut().insert_resource(FriendlyFire(1));
    shoot(&mut sim, victim, mate);
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 1, "friendly fire on: a teammate's hit decals");
}

#[test]
fn no_body_decals_blocks_and_clears() {
    let (mut sim, victim, shooter) = sim();
    shoot(&mut sim, victim, shooter);
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 1);
    sim.app.world_mut().entity_mut(victim).insert(NoBodyDecals);
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 0, "cloaking (Über, a disguise) removes them");
    assert!(layers(&mut sim).is_empty());
    shoot(&mut sim, victim, shooter);
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 0, "none while cloaked");
    sim.app.world_mut().entity_mut(victim).remove::<NoBodyDecals>();
    shoot(&mut sim, victim, shooter);
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 1);
}

#[test]
fn clear_message_and_living_again_clear_them() {
    let (mut sim, victim, shooter) = sim();
    shoot(&mut sim, victim, shooter);
    sim.ticks(2);
    sim.app.world_mut().write_message(ClearBodyDecals(victim));
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 0, "healed to full (TF2)");
    shoot(&mut sim, victim, shooter);
    sim.ticks(2);
    sim.app.world_mut().entity_mut(victim).insert(Dead { since: 0.0 });
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 1, "kept while dead");
    sim.app.world_mut().entity_mut(victim).remove::<Dead>();
    sim.ticks(2);
    assert_eq!(decals(&mut sim), 0, "respawned: gone");
}

/// The projection itself: a posed body (bone 1 moved 0.5 m down) takes the
/// decal where the posed chest is, in bind space.
#[test]
fn projection_follows_the_pose_into_bind_space() {
    let mesh = chest();
    let skin = [Mat4::IDENTITY, Mat4::from_translation(Vec3::new(0.0, -0.5, 0.0))];
    let posed = pose_part(&mesh, &skin);
    assert!((posed[0].y - 0.4).abs() < 1e-5);
    let parts = [(&mesh, posed.as_slice())];
    let piece =
        project_on_body(&parts, &decal(), Vec3::new(0.0, 0.6, 2.0), Vec3::new(0.0, 0.6, 0.15), Vec3::X, Vec3::NEG_Y)
            .expect("on the posed chest");
    assert!(piece.positions.iter().all(|p| (p[1] - 1.1).abs() < 0.06), "bind space: {:?}", piece.positions);
    assert!(piece.weights.iter().all(|w| (w[0] - 1.0).abs() < 1e-5) && piece.joints.iter().all(|j| j[0] == 1));
    // Missed (a shot beside the body): nothing.
    assert!(
        project_on_body(&parts, &decal(), Vec3::new(1.0, 0.6, 2.0), Vec3::new(1.0, 0.6, 0.15), Vec3::X, Vec3::NEG_Y)
            .is_none()
    );
}
