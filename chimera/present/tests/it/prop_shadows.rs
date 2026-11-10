//! Physics props' shadows on the world (Source's render-to-texture
//! shadows, specs/cs_source/shadows_sky.md) while the prop moves, headless:
//! the shadow follows the prop every frame through its frame in the frame
//! table (`ShadowFrames`, projected by the shader), over receiver geometry
//! in one mesh asset rewritten in place only when the frame leaves the
//! region it covers. A new mesh handle each frame left a moving prop's
//! shadow undrawn until it came to rest (Bevy re-specializes an entity
//! whose `Mesh3d` changed, and skipped it while the new mesh wasn't
//! prepared on the GPU yet). The render world's extraction is emulated
//! (`harness::emulate_mesh_extraction`): every rewrite must reach it once,
//! in the frame it was made.

use avian3d::prelude::LinearVelocity;
use bevy::{camera::primitives::MeshAabb, prelude::*};
use chimera_present::MapPlugin;
use chimera_world::{
    harness::{MeshExtraction, Sim},
    map::{
        MapCollision, MapConvex, MapData, MapMesh, MapModel, MapPhysics, MapProp, MapShadows, PropIndex,
        PropSolid, PushAway,
    },
};
use chimera_present::{
    render::{
        prop_material::PropMaterial,
        rope_material::RopeMaterial,
        sprite_material::SpriteMaterial,
        world_material::WorldMaterial,
        shadows::{Projected, PropShadow, ShadowFrames, ShadowMaterial},
    },
};

/// A box's 12 triangles (corners `lo`..`hi`).
fn box_mesh(lo: Vec3, hi: Vec3) -> MapMesh {
    let p = |k: usize| [[lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]];
    let positions: Vec<[f32; 3]> = (0..8).map(p).collect();
    let indices = vec![
        0, 1, 3, 0, 3, 2, 4, 6, 7, 4, 7, 5, 0, 4, 5, 0, 5, 1, 2, 3, 7, 2, 7, 6, 0, 2, 6, 0, 6, 4, 1, 5, 7, 1, 7, 3,
    ];
    MapMesh {
        material: "test/box".into(),
        normals: vec![[0.0, 1.0, 0.0]; 8],
        uvs: vec![[0.0, 0.0]; 8],
        positions,
        indices,
        color: [128, 128, 128],
        ..default()
    }
}

/// A 20 m floor (world mesh and collision) with a 0.6 m physics crate
/// standing on it, casting a shadow straight down and a little sideways.
pub(crate) fn map() -> MapData {
    let mut data = MapData {
        name: "test:prop_shadows".into(),
        entity_scale: 0.0254,
        shadows: Some(MapShadows {
            direction: Vec3::new(0.2, -1.0, 0.1).normalize(),
            color: [100, 100, 100],
            distance: 2.0,
        }),
        ..default()
    };
    let (a, b) = (Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 0.0, 10.0));
    data.collision_brushes.push(chimera_world::map::MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
    // The floor's top face, facing up.
    data.meshes.push(MapMesh {
        material: "test/floor".into(),
        positions: vec![[-10.0, 0.0, -10.0], [-10.0, 0.0, 10.0], [10.0, 0.0, 10.0], [10.0, 0.0, -10.0]],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        uvs: vec![[0.0, 0.0]; 4],
        lightmap_uvs: vec![[0.0, 0.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        color: [200, 180, 150],
        ..default()
    });
    let half = 0.3;
    let (lo, hi) = (Vec3::splat(-half), Vec3::splat(half));
    let corners = (0..8)
        .map(|k| Vec3::new([lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]))
        .collect();
    data.models.push(MapModel {
        meshes: vec![box_mesh(lo, hi)],
        bounds: (lo, hi),
        collision: Some(MapCollision {
            pieces: vec![MapConvex {
                points: corners,
                planes: Vec::new(),
            }],
            mass: 20.0,
            ..default()
        }),
        ..default()
    });
    data.props.push(MapProp {
        pose: None,
        ragdoll: None,
        model: 0,
        translation: Vec3::new(0.0, half + 0.01, 0.0),
        rotation: Quat::IDENTITY,
        solid: PropSolid::Mesh,
        skybox: false,
        lighting: None,
        vertex_light: None,
        casts_shadow: true,
        physics: Some(MapPhysics {
            mass: 20.0,
            // A surface's (0 would let it skate and turn on the floor
            // forever: friction multiplies the world's).
            friction: 0.8,
            elasticity: 0.0,
            damping: 0.0,
            rotdamping: 0.0,
            inertia: None,
            push: PushAway::Collide,
            frozen: false,
            asleep: false,
        }),
        fade: None,
        parent: None,
        entity: None,
        skin: 0,
        body: 0,
        detail: false,
    });
    data
}

/// The map's render assets (a headless `Sim` has meshes only).
fn sim() -> Sim {
    Sim::with(base(), |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(map()));
        chimera_world::harness::emulate_mesh_extraction(app);
    })
}

/// The shadow's entity, mesh asset and its centre.
fn shadow(sim: &mut Sim) -> (Entity, AssetId<Mesh>, Vec3) {
    let world = sim.app.world_mut();
    let (e, mesh) = world
        .query::<(Entity, &PropShadow, &Mesh3d)>()
        .iter(world)
        .find(|(_, s, _)| s.prop == 0)
        .map(|(e, _, m)| (e, m.id()))
        .expect("the crate casts a shadow");
    // As the render world holds it (the shadow mesh lives there only).
    let aabb = world
        .resource::<MeshExtraction>()
        .extracted
        .get(&mesh)
        .and_then(|m| m.get_aabb())
        .expect("a shadow mesh with positions");
    (e, mesh, Vec3::from(aabb.center))
}

fn crate_at(sim: &mut Sim) -> (Entity, Vec3) {
    let world = sim.app.world_mut();
    world
        .query::<(Entity, &PropIndex, &Transform)>()
        .iter(world)
        .find(|(_, i, _)| i.0 == 0)
        .map(|(e, _, t)| (e, t.translation))
        .expect("the crate")
}

/// The shadow's direction, from the frame table (a projected shadow).
fn frame_direction(sim: &mut Sim, e: Entity) -> Vec3 {
    let world = sim.app.world();
    let slot = world.get::<Projected>(e).expect("projected").slot;
    let row = world.resource::<ShadowFrames>().get(slot)[3];
    Vec3::new(row[0], row[1], row[2])
}

/// The shadow's frame origin, from the frame table (a projected shadow).
fn frame_origin(sim: &mut Sim, e: Entity) -> Vec3 {
    let world = sim.app.world();
    let slot = world.get::<Projected>(e).expect("projected once it moves").slot;
    let row = world.resource::<ShadowFrames>().get(slot)[0];
    Vec3::new(row[0], row[1], row[2])
}

#[test]
fn a_moving_prop_keeps_its_shadow_mesh_and_moves_it() {
    let mut sim = sim();
    sim.ticks(5);
    let (entity, mesh, start) = shadow(&mut sim);
    let (node, at) = crate_at(&mut sim);
    assert!(start.xz().distance(at.xz()) < 0.5, "under the crate: {start} {at}");
    #[derive(Resource, Default)]
    struct Rewrites(usize);
    sim.app.init_resource::<Rewrites>().add_systems(
        Last,
        |mut events: MessageReader<AssetEvent<Mesh>>, mut n: ResMut<Rewrites>| {
            n.0 += events.read().filter(|e| matches!(e, AssetEvent::Modified { .. })).count();
        },
    );
    sim.app.world_mut().entity_mut(node).insert(LinearVelocity(Vec3::X * 2.0));
    let mut first = None;
    let mut last = (start, at);
    for tick in 0..20 {
        sim.ticks(1);
        let (e, m, _) = shadow(&mut sim);
        let (_, at) = crate_at(&mut sim);
        assert_eq!((e, m), (entity, mesh), "tick {tick}: the same shadow entity and mesh asset");
        let origin = frame_origin(&mut sim, e);
        assert!(
            origin.xz().distance(at.xz()) < 0.5,
            "tick {tick}: the shadow follows the crate: {origin} {at}"
        );
        first.get_or_insert((origin, at));
        last = (origin, at);
    }
    let ((o0, c0), (o1, c1)) = (first.unwrap(), last);
    assert!(c1.x - c0.x > 0.2, "the crate moved: {c0} -> {c1}");
    assert!(
        ((o1.x - o0.x) - (c1.x - c0.x)).abs() < 0.02,
        "the shadow moved with the crate: {o0} -> {o1}, the crate {c0} -> {c1}"
    );
    let rewrites = sim.app.world().resource::<Rewrites>().0;
    assert!(
        rewrites <= 3,
        "the receiver geometry is rewritten only when the shadow leaves its region, not each frame: {rewrites}"
    );
    // Coming to rest: no shadow update is announced after the render
    // world took its data (a rewrite announced a frame late, after
    // `AssetEventSystems`, was extracted twice: Bevy logged "RenderMesh
    // with RenderAssetUsages == RENDER_WORLD cannot be extracted" every
    // time a moving prop stopped).
    sim.app.world_mut().entity_mut(node).insert(LinearVelocity(Vec3::ZERO));
    for _ in 0..30 {
        sim.ticks(1);
        // Stop and go: moves on alternate stretches of frames.
        let v = if sim.tick() % 6 < 3 { Vec3::X } else { Vec3::ZERO };
        sim.app.world_mut().entity_mut(node).insert(LinearVelocity(v));
    }
    let failures = &sim.app.world().resource::<MeshExtraction>().failures;
    assert!(failures.is_empty(), "meshes announced changed after extraction: {failures:?}");
}

/// The prop shadows there are now, and their materials' atlas sizes
/// (render-to-texture: the packed atlas; blobs: the 32-texel picture).
fn shadows_now(sim: &mut Sim) -> Vec<(Entity, u32)> {
    let world = sim.app.world_mut();
    let found: Vec<(Entity, Handle<ShadowMaterial>)> = world
        .query::<(Entity, &PropShadow, &MeshMaterial3d<ShadowMaterial>)>()
        .iter(world)
        .map(|(e, _, m)| (e, m.0.clone()))
        .collect();
    found
        .into_iter()
        .map(|(e, m)| {
            let atlas = world.resource::<Assets<ShadowMaterial>>().get(&m).unwrap().atlas.clone();
            (e, world.resource::<Assets<Image>>().get(&atlas).unwrap().width())
        })
        .collect()
}

/// Video > Advanced's shadow detail, live (CS:S's cvars): Low
/// (`r_shadowrendertotexture 0`) swaps the crate's silhouette for a blob
/// straight under it, `r_shadows 0` removes it, Medium brings the
/// silhouette back.
#[test]
fn shadow_detail_switches_between_silhouettes_and_blobs_at_once() {
    use chimera_present::render::shadows::{BLOB_TEX, ShadowSettings};
    let mut sim = sim();
    sim.ticks(5);
    let rtt = shadows_now(&mut sim);
    assert_eq!(rtt.len(), 1);
    assert_ne!(rtt[0].1, BLOB_TEX, "render-to-texture by default");
    let set = |sim: &mut Sim, shadows: u8, rtt: u8| {
        sim.app.insert_resource(ShadowSettings {
            shadows,
            render_to_texture: rtt,
            flashlight_depth: 0,
        });
        sim.ticks(2);
    };
    set(&mut sim, 1, 0);
    let blob = shadows_now(&mut sim);
    assert_eq!(blob.len(), 1, "one shadow, rebuilt");
    assert_eq!(blob[0].1, BLOB_TEX, "the blob picture");
    let (_, _, centre) = shadow(&mut sim);
    let (_, at) = crate_at(&mut sim);
    // Straight down (the map's direction leans 0.2 sideways).
    assert!(centre.xz().distance(at.xz()) < 0.05, "under the crate: {centre} {at}");
    set(&mut sim, 0, 0);
    assert!(shadows_now(&mut sim).is_empty(), "r_shadows 0");
    set(&mut sim, 1, 1);
    let back = shadows_now(&mut sim);
    assert_eq!(back.len(), 1);
    assert_ne!(back[0].1, BLOB_TEX);
}

/// A walking character's blob shadow follows it through its frame in the
/// frame table; its receiver geometry (one mesh asset) is rewritten only
/// when the blob leaves the region it covers, not every frame (a mesh
/// rewritten every frame is freed and allocated again in Bevy's mesh slabs:
/// docs/performance.md, "Render performance").
#[test]
fn a_walking_character_s_blob_moves_by_its_frame() {
    use chimera_present::render::{bodies::CharacterBody, shadows::CharacterBlob};
    let mut sim = sim();
    sim.ticks(5);
    let w = sim.app.world_mut();
    let character = w
        .spawn((chimera_core::core::Intent::default(), Transform::from_xyz(3.0, 0.0, 3.0), Visibility::Inherited))
        .id();
    w.spawn((
        CharacterBody {
            model: 0,
            skeleton: 0,
            skin: 0,
            body: 0,
            joints: Vec::new(),
            held: None,
            held_parts: Vec::new(),
        },
        Visibility::Inherited,
        ChildOf(character),
    ));
    #[derive(Resource, Default)]
    struct Rewrites(usize);
    sim.app.init_resource::<Rewrites>().add_systems(
        Last,
        |mut events: MessageReader<AssetEvent<Mesh>>, mut n: ResMut<Rewrites>| {
            n.0 += events.read().filter(|e| matches!(e, AssetEvent::Modified { .. })).count();
        },
    );
    sim.app.update();
    let blob = |sim: &mut Sim| {
        let world = sim.app.world_mut();
        world
            .query::<(Entity, &CharacterBlob, &Mesh3d)>()
            .iter(world)
            .find(|(_, b, _)| b.owner == character)
            .map(|(e, _, m)| (e, m.id()))
            .expect("a blob under the character")
    };
    let (entity, mesh) = blob(&mut sim);
    sim.app.world_mut().resource_mut::<Rewrites>().0 = 0;
    for frame in 0..60 {
        // 6 m/s at 60 frames a second.
        let x = 3.0 + 0.1 * frame as f32;
        sim.app.world_mut().get_mut::<Transform>(character).unwrap().translation.x = x;
        sim.app.update();
        assert_eq!(blob(&mut sim), (entity, mesh), "frame {frame}: the same blob and mesh asset");
        // Where the frame's axis meets the floor (y = 0): the map's
        // direction leans 0.2 sideways a unit down.
        let origin = frame_origin(&mut sim, entity);
        let d = frame_direction(&mut sim, entity);
        let hit = origin + d * (-origin.y / d.y);
        assert!(
            (hit.x - x).abs() < 0.4 && (hit.z - 3.0).abs() < 0.4,
            "frame {frame}: the blob by the character: {hit} at x {x}"
        );
    }
    let rewrites = sim.app.world().resource::<Rewrites>().0;
    assert!(
        (1..=8).contains(&rewrites),
        "6 m walked: the blob's geometry rewritten a few times, not each of 60 frames: {rewrites}"
    );
    // Gone with its character: its table slot back.
    let live = sim.app.world().resource::<ShadowFrames>().live();
    sim.app.world_mut().entity_mut(character).despawn();
    sim.app.update();
    assert_eq!(sim.app.world().resource::<ShadowFrames>().live(), live - 1, "its slot given back");
}

/// The engine's base the props need (`core`'s sets and the console's
/// cvars); nothing of a game.
pub(crate) fn base() -> (chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin) {
    (chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin)
}
