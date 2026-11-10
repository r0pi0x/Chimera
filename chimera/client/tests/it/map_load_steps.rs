//! A `map` command's load spawns the map a step at a time behind the
//! loading screen (`console::finish_map_load`, `map::prepare::
//! begin_map_change`; docs/performance.md, "A map's swap"): the simulation
//! waits while it spawns (no tick on a part of a map), the world gets the
//! whole map at once and the same map as a spawn in one go, and until the
//! game is entered a frame advances the simulation by at most one tick (no
//! catch-up ticks owed for the loading screen), given back after. The
//! map's colliders keep the mass properties Avian's insert hook gave them:
//! the first tick computes none again, and the physics props weigh and fall
//! as when it did.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_client::client::{
    AppHooks,
    console::{self, MapLoadStage},
};
use chimera_core::core::SimClock;
use chimera_sim::SimBasePlugins;
use chimera_world::{
    harness::Sim,
    map::{
        MapBrush, MapCollision, MapConvex, MapData, MapDebugView, MapMesh, MapModel, MapPart, MapPhysics, MapProp,
        MapSimPlugin, PropSolid, PushAway, change_map,
    },
};

/// Props in the test map: several of the spawn's steps' worth.
const PROPS: usize = 300;

/// Of them, physics props (the first ones), placed above the floor to fall.
const PHYSICS_PROPS: usize = 8;

fn map() -> MapData {
    let mut data = MapData {
        name: "test:steps".into(),
        entity_scale: 0.0254,
        ..default()
    };
    let (a, b) = (Vec3::new(-50.0, -1.0, -50.0), Vec3::new(50.0, 0.0, 50.0));
    data.collision_brushes.push(MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
    data.meshes.push(MapMesh {
        material: "test/floor".into(),
        positions: vec![[-50.0, 0.0, -50.0], [-50.0, 0.0, 50.0], [50.0, 0.0, 50.0], [50.0, 0.0, -50.0]],
        normals: vec![[0.0, 1.0, 0.0]; 4],
        uvs: vec![[0.0, 0.0]; 4],
        indices: vec![0, 1, 2, 0, 2, 3],
        ..default()
    });
    // The floor's triangles too (the world's trimesh collider).
    data.collision_positions = vec![[a.x, b.y, a.z], [a.x, b.y, b.z], [b.x, b.y, b.z], [b.x, b.y, a.z]];
    data.collision_indices = vec![[0, 1, 2], [0, 2, 3]];
    let (lo, hi) = (Vec3::splat(-0.3), Vec3::splat(0.3));
    data.models.push(MapModel {
        bounds: (lo, hi),
        ..default()
    });
    // The physics props' model: a convex piece off its origin (a centre of
    // mass and inertia from the shape).
    let (plo, phi) = (Vec3::new(-0.3, -0.1, -0.2), Vec3::new(0.5, 0.2, 0.2));
    let pick = |lo: f32, hi: f32, bit: usize| if bit == 0 { lo } else { hi };
    let corner = |k: usize| Vec3::new(pick(plo.x, phi.x, k & 1), pick(plo.y, phi.y, k & 2), pick(plo.z, phi.z, k & 4));
    data.models.push(MapModel {
        bounds: (plo, phi),
        collision: Some(MapCollision {
            pieces: vec![MapConvex {
                points: (0..8).map(corner).collect(),
                planes: Vec::new(),
            }],
            mass: 20.0,
            damping: 0.0,
            rotdamping: 0.0,
            inertia: 1.0,
            unit_inertia: None,
            surfaceprop: String::new(),
        }),
        ..default()
    });
    for i in 0..PROPS {
        data.props.push(MapProp {
            pose: None,
            ragdoll: None,
            model: usize::from(i < PHYSICS_PROPS),
            translation: Vec3::new(
                (i % 20) as f32 * 2.0 - 20.0,
                if i < PHYSICS_PROPS { 1.5 } else { 0.31 },
                (i / 20) as f32 * 2.0 - 15.0,
            ),
            rotation: Quat::IDENTITY,
            solid: if i < PHYSICS_PROPS { PropSolid::Mesh } else { PropSolid::Box },
            skybox: false,
            lighting: None,
            vertex_light: None,
            casts_shadow: false,
            physics: (i < PHYSICS_PROPS).then(|| MapPhysics {
                mass: 20.0 + i as f32,
                friction: 0.8,
                elasticity: 0.2,
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
    }
    data.spawns.push((Vec3::new(0.0, 0.1, 0.0), None));
    data
}

fn sim() -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins(MapSimPlugin::empty())
            .insert_resource(AppHooks {
                build: "test",
                load_map: |_, _| Ok(map()),
                load_map_file: |id, _, _| Err(format!("{id}: not in tests")),
                map_file_bytes: |_| None,
                unload_greybox: |_| {},
                respawn_greybox: |_| {},
            })
            .add_systems(Update, console::finish_map_load);
    });
    sim.ticks(2);
    sim
}

/// The map's parts: each entity's name, sorted.
fn parts(sim: &mut Sim) -> Vec<String> {
    let w = sim.app.world_mut();
    let mut names: Vec<String> = w
        .query_filtered::<Option<&Name>, With<MapPart>>()
        .iter(w)
        .map(|n| n.map_or(String::new(), |n| n.as_str().to_string()))
        .collect();
    names.sort();
    names
}

#[test]
fn a_map_spawns_in_steps_behind_the_screen_with_the_simulation_waiting() {
    let mut sim = sim();
    let default_max = sim.app.world().resource::<Time<Virtual>>().max_delta();
    console::start_map_load(sim.app.world_mut(), "test:steps", None);
    let mut stages = Vec::new();
    let mut spawning_ticks = Vec::new();
    let mut seen_parts = Vec::new();
    for _ in 0..600 {
        let before = console::map_load_stage(sim.app.world());
        if before == MapLoadStage::None && !stages.is_empty() {
            break;
        }
        sim.app.update();
        let after = console::map_load_stage(sim.app.world());
        stages.push(after);
        let w = sim.app.world_mut();
        let n = w.query_filtered::<(), With<MapPart>>().iter(w).count();
        if after == MapLoadStage::Spawning {
            spawning_ticks.push(w.resource::<SimClock>().tick);
            seen_parts.push(n);
        }
        if after == MapLoadStage::Settling {
            let tick = w.resource::<Time<Fixed>>().timestep();
            assert_eq!(
                w.resource::<Time<Virtual>>().max_delta(),
                tick,
                "settling: a frame advances at most one tick"
            );
            assert!(!w.resource::<Time<Virtual>>().is_paused(), "settling: the simulation runs");
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(stages.contains(&MapLoadStage::Spawning), "spawned in steps: {stages:?}");
    assert!(stages.contains(&MapLoadStage::Settling), "settled: {stages:?}");
    assert_eq!(console::map_load_stage(sim.app.world()), MapLoadStage::None, "in the game");
    assert!(
        spawning_ticks.windows(2).all(|t| t[0] == t[1]),
        "no tick while the map spawns: {spawning_ticks:?}"
    );
    assert!(seen_parts.iter().all(|n| *n == 0), "no part of the map before the whole: {seen_parts:?}");
    let w = sim.app.world();
    assert_eq!(w.resource::<Time<Virtual>>().max_delta(), default_max, "given back in the game");
    assert!(!w.resource::<Time<Virtual>>().is_paused());
    let stepped = parts(&mut sim);
    assert!(stepped.iter().filter(|n| n.starts_with("Prop ")).count() == PROPS, "every prop");
    // The same map spawned in one go.
    let mut once = self::sim();
    change_map(once.app.world_mut(), map(), MapDebugView::Normal);
    once.app.update();
    assert_eq!(stepped, parts(&mut once), "the same parts as a spawn in one go");
}

/// Map colliders whose shape or density changed since this system last ran,
/// as Avian's `update_collider_mass_properties` sees them (`Changed`).
#[derive(Resource, Default)]
struct Recounted(usize);

fn count_recounted(
    mut n: ResMut<Recounted>,
    q: Query<
        (Option<&Name>, Ref<Collider>, Ref<ColliderDensity>),
        (
            With<MapPart>,
            Or<(Changed<Collider>, Changed<ColliderDensity>)>,
            Without<Sensor>,
        ),
    >,
) {
    for (name, c, d) in &q {
        eprintln!(
            "computed again: {name:?} (collider added at {:?}, changed at {:?}; density {:?}, {:?})",
            c.added(),
            c.last_changed(),
            d.added(),
            d.last_changed()
        );
    }
    n.0 += q.iter().count();
}

/// Each physics prop by name: its mass, angular inertia, centre of mass
/// and place.
fn bodies(sim: &mut Sim) -> Vec<(String, f32, Vec3, Vec3, Vec3)> {
    let w = sim.app.world_mut();
    let mut out: Vec<_> = w
        .query_filtered::<(
            &Name,
            &RigidBody,
            &ComputedMass,
            &ComputedAngularInertia,
            &ComputedCenterOfMass,
            &Transform,
        ), With<MapPart>>()
        .iter(w)
        .filter(|(n, b, ..)| n.as_str().starts_with("Prop ") && **b == RigidBody::Dynamic)
        .map(|(n, _, m, i, c, t)| {
            (
                n.to_string(),
                m.value(),
                i.principal_angular_inertia_with_local_frame().0,
                c.0,
                t.translation,
            )
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

#[test]
fn the_first_tick_on_a_map_computes_no_collider_mass_again() {
    let mut sim = sim();
    sim.app
        .init_resource::<Recounted>()
        .add_systems(FixedPostUpdate, count_recounted);
    console::start_map_load(sim.app.world_mut(), "test:steps", None);
    for _ in 0..600 {
        // Counted from the frames that spawn (the old map's colliders went
        // with it).
        if console::map_load_stage(sim.app.world()) == MapLoadStage::Spawning {
            sim.app.world_mut().resource_mut::<Recounted>().0 = 0;
        }
        sim.app.update();
        if console::map_load_stage(sim.app.world()) == MapLoadStage::None {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    sim.ticks(5);
    assert_eq!(
        sim.app.world().resource::<Recounted>().0,
        0,
        "no map collider's mass computed again"
    );
    // Every collider holds what Avian computes from its shape and density.
    let w = sim.app.world_mut();
    let mut n = 0;
    for (c, d, m) in w
        .query_filtered::<(&Collider, &ColliderDensity, &ColliderMassProperties), (With<MapPart>, Without<Sensor>)>()
        .iter(w)
    {
        assert_eq!(*m, ColliderMassProperties::from(c.mass_properties(d.0)));
        n += 1;
    }
    assert!(n > PHYSICS_PROPS, "the world's colliders and the physics props': {n}");
}

#[test]
fn physics_props_weigh_and_fall_as_with_their_mass_computed_again() {
    // The map spawned in one go; then again with every collider marked
    // changed after its spawn, as the first tick saw them before.
    let mut now = sim();
    change_map(now.app.world_mut(), map(), MapDebugView::Normal);
    let mut before = sim();
    change_map(before.app.world_mut(), map(), MapDebugView::Normal);
    let w = before.app.world_mut();
    for (mut c, mut d) in w.query::<(Mut<Collider>, Mut<ColliderDensity>)>().iter_mut(w) {
        c.set_changed();
        d.set_changed();
    }
    let start = bodies(&mut now);
    now.ticks(40);
    before.ticks(40);
    let (a, b) = (bodies(&mut now), bodies(&mut before));
    assert_eq!(a.len(), PHYSICS_PROPS, "every physics prop has a body");
    for (i, (x, y)) in a.iter().zip(&b).enumerate() {
        assert_eq!(x.0, y.0);
        assert_eq!((x.1, x.2, x.3), (y.1, y.2, y.3), "{}: the same mass properties", x.0);
        assert!(x.4.distance(y.4) < 1e-4, "{}: the same fall ({} and {})", x.0, x.4, y.4);
        assert!(
            x.4.y < start[i].4.y - 0.5,
            "{}: falls ({} from {})",
            x.0,
            x.4,
            start[i].4
        );
    }
}
