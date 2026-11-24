//! Physics props' motion against CS:S's, measured on the probe server
//! (2026-10-18, specs/cs_source/physics_props.md 3.3-3.5 and open
//! questions 1, 3-5): a 40-unit 30 kg crate on a concrete floor slides
//! with the product of the two frictions, rebounds at about 0.14 of its
//! landing speed, is held back by air drag growing with the square of its
//! speed, sleeps about half a second after it stops, and spins with its
//! collision model's stored inertia; a static prop's bbox solid blocks as
//! the box along the world's axes around its turned bounds.

use avian3d::prelude::{
    ComputedAngularInertia, LinearVelocity, Position, RigidBody, Sleeping,
};
use bevy::prelude::*;

use crate::stack::{
    games::cs_source::{
        self,
        movement::{SourceMovementPlugin, to_engine, to_source},
        weapons::CsWeaponsPlugin,
    },
    harness::{CsSim, Sim},
    map::{
        MapBrush, MapCollision, MapConvex, MapData, MapEntity, MapModel, MapPhysics, MapPlugin, MapProp, PropEntity,
        PropSolid, PushAway,
    },
};

const SCALE: f32 = 0.0254;
/// The crate's half size (Source units): wood_crate001a's 40 units.
const HALF: f32 = 20.0;

/// A floor at z = 0 (and, `ceiling`, nothing above) and one crate of
/// surface `friction` and `elasticity` at `at`; `inertia` its stored
/// inertia (kg m², engine axes).
fn sim(at: Vec3, friction: f32, elasticity: f32, inertia: Option<Vec3>) -> Sim {
    let mut data = MapData {
        name: "test:prop_motion".into(),
        entities: vec![MapEntity {
            keyvalues: vec![
                ("classname".into(), "prop_physics".into()),
                ("origin".into(), format!("{} {} {}", at.x, at.y, at.z)),
            ],
            hulls: Vec::new(),
            mover: false,
            physics: None,
        }],
        entity_scale: SCALE,
        // CS:S's gravity for physics (800 units/s²), as maps get it.
        gravity: Some(800.0 * SCALE),
        ..default()
    };
    let (a, b) = (to_engine(Vec3::new(-8192.0, -8192.0, -64.0)), to_engine(Vec3::new(8192.0, 8192.0, 0.0)));
    let (a, b) = (a.min(b), a.max(b));
    data.collision_brushes.push(MapBrush::from_box(a, b));
    data.collision_hulls.push(
        (0..8)
            .map(|k| [[a.x, b.x][k & 1], [a.y, b.y][(k >> 1) & 1], [a.z, b.z][(k >> 2) & 1]])
            .collect(),
    );
    let (lo, hi) = (to_engine(Vec3::splat(-HALF)), to_engine(Vec3::splat(HALF)));
    let (lo, hi) = (lo.min(hi), lo.max(hi));
    let corners = (0..8)
        .map(|k| Vec3::new([lo.x, hi.x][k & 1], [lo.y, hi.y][(k >> 1) & 1], [lo.z, hi.z][(k >> 2) & 1]))
        .collect();
    data.models.push(MapModel {
        bounds: (lo, hi),
        collision: Some(MapCollision {
            pieces: vec![MapConvex { points: corners, planes: Vec::new() }],
            mass: 30.0,
            ..default()
        }),
        ..default()
    });
    data.props.push(MapProp {
        pose: None,
        ragdoll: None,
        model: 0,
        translation: to_engine(at),
        rotation: Quat::IDENTITY,
        solid: PropSolid::Mesh,
        skybox: false,
        lighting: None,
        vertex_light: None,
        casts_shadow: false,
        physics: Some(MapPhysics {
            mass: 30.0,
            friction,
            elasticity,
            damping: 0.0,
            rotdamping: 0.0,
            inertia,
            push: PushAway::Collide,
            frozen: false,
            asleep: false,
            debris: false,
        }),
        fade: None,
        parent: None,
        entity: Some(0),
        skin: 0,
        body: 0,
        detail: false,
    });
    let mut sim = Sim::new(CsSim, (MapPlugin::new(data), SourceMovementPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(cs_source::TICK_INTERVAL);
    sim
}

fn node(sim: &mut Sim) -> Entity {
    let world = sim.app.world_mut();
    world.query::<(Entity, &PropEntity)>().iter(world).next().unwrap().0
}

/// The crate's origin and velocity (Source units).
fn state(sim: &mut Sim) -> (Vec3, Vec3) {
    let e = node(sim);
    let w = sim.app.world();
    (
        to_source(w.get::<Position>(e).unwrap().0),
        to_source(w.get::<LinearVelocity>(e).unwrap().0),
    )
}

fn set_velocity(sim: &mut Sim, v: Vec3) {
    let e = node(sim);
    sim.app.world_mut().get_mut::<LinearVelocity>(e).unwrap().0 = to_engine(v);
}

/// A crate resting on the floor shoved at 150 units/s slows at the
/// product of its friction and the world's 0.8 times gravity: CS:S's
/// slowed at 0.61 g with 0.8 (0.64 the product) and 0.086 g with ice's
/// 0.1.
#[test]
fn a_shoved_crate_slides_with_the_product_of_the_frictions() {
    for (friction, lo, hi) in [(0.8, 0.55, 0.7), (0.1, 0.06, 0.11)] {
        let mut sim = sim(Vec3::new(0.0, 0.0, HALF + 0.5), friction, 0.1, None);
        sim.seconds(1.0);
        set_velocity(&mut sim, Vec3::new(150.0, 0.0, 0.0));
        sim.ticks(3);
        let (_, v0) = state(&mut sim);
        sim.ticks(6);
        let (_, v1) = state(&mut sim);
        let g = (v0.x - v1.x) / (6.0 * cs_source::TICK_INTERVAL as f32) / 800.0;
        assert!((lo..hi).contains(&g), "friction {friction}: slowed at {g} g ({v0} -> {v1})");
    }
}

/// A crate falling at 1000 units/s high above the floor is held back by
/// air drag: CS:S's slowed its fall by about 950 units/s² beyond gravity
/// (the debug print's k: half the air density 2 times the 1.03 m²
/// face over 30 kg, 874 at this speed).
#[test]
fn a_fast_crate_feels_the_air() {
    let mut sim = sim(Vec3::new(0.0, 0.0, 3000.0), 0.8, 0.1, None);
    sim.ticks(1);
    set_velocity(&mut sim, Vec3::new(0.0, 0.0, -1000.0));
    sim.ticks(1);
    let (_, v0) = state(&mut sim);
    sim.ticks(4);
    let (_, v1) = state(&mut sim);
    let drag = (v1.z - v0.z) / (4.0 * cs_source::TICK_INTERVAL as f32) + 800.0;
    assert!((750.0..1050.0).contains(&drag), "drag {drag} units/s² ({v0} -> {v1})");
    // And slower, a quarter of it at half the speed.
    set_velocity(&mut sim, Vec3::new(0.0, 0.0, 0.0));
    sim.ticks(1);
    set_velocity(&mut sim, Vec3::new(500.0, 0.0, 0.0));
    sim.ticks(1);
    let (_, v0) = state(&mut sim);
    sim.ticks(4);
    let (_, v1) = state(&mut sim);
    let drag = (v0.x - v1.x) / (4.0 * cs_source::TICK_INTERVAL as f32);
    assert!((170.0..270.0).contains(&drag), "drag {drag} at 500 ({v0} -> {v1})");
}

/// Dropped flat from 100 units onto the floor, the crate (elasticity 0.1)
/// rises back about 2 units (CS:S: 1.84 with wood, 2.01 with concrete,
/// 2.17 with plastic's 0.01), then sleeps: CS:S's was asleep 0.4 to
/// 0.55 s after its speed fell under about 1 unit/s.
#[test]
fn a_dropped_crate_rebounds_a_little_then_sleeps() {
    let mut sim = sim(Vec3::new(0.0, 0.0, HALF + 100.0), 0.8, 0.1, None);
    let mut landed = None;
    let mut peak = f32::MIN;
    let mut still_at = None;
    let mut slept_at = None;
    for tick in 0..300 {
        sim.ticks(1);
        let (p, v) = state(&mut sim);
        if landed.is_none() && v.z > 0.0 {
            landed = Some(tick);
        }
        if landed.is_some() && tick < landed.unwrap() + 30 {
            peak = peak.max(p.z - HALF);
        }
        if landed.is_some() && still_at.is_none() && v.length() < 1.0 {
            still_at = Some(tick);
        }
        let e = node(&mut sim);
        if slept_at.is_none() && sim.app.world().get::<Sleeping>(e).is_some() {
            slept_at = Some(tick);
        }
    }
    assert!(landed.is_some(), "it landed");
    assert!((1.0..3.5).contains(&peak), "rebound to {peak} units");
    let (still, slept) = (still_at.expect("came to rest"), slept_at.expect("slept"));
    let after = (slept - still) as f64 * cs_source::TICK_INTERVAL;
    assert!((0.2..0.9).contains(&after), "slept {after} s after it stopped");
}

/// A prop whose collision model stores its inertia spins with that (the
/// plastic crate: 0.56, 0.33, 0.62 kg m² about Source x, y, z in CS:S),
/// not the solid box's.
#[test]
fn a_prop_spins_with_its_stored_inertia() {
    let stored = Vec3::new(3.59, 3.61, 3.62);
    let mut sim = sim(Vec3::new(0.0, 0.0, HALF + 0.5), 0.8, 0.1, Some(stored));
    sim.ticks(2);
    let e = node(&mut sim);
    let w = sim.app.world();
    assert!(w.get::<RigidBody>(e).unwrap().is_dynamic());
    let (i, _) = w.get::<ComputedAngularInertia>(e).unwrap().principal_angular_inertia_with_local_frame();
    let sorted = |v: Vec3| {
        let mut a = v.to_array();
        a.sort_by(f32::total_cmp);
        Vec3::from_array(a)
    };
    // The solid 40-unit box's would be 5.16 about every axis.
    assert!((sorted(i) - sorted(stored)).abs().max_element() < 1e-2, "{i}");
}

/// A static prop with the bbox solid turned 45° blocks as the box along
/// the world's axes around its turned bounds (CS:S, measured 2026-10-18:
/// de_dust's windows turned 120° stopped traces on that box,
/// physics_props.md Q9); an oriented one (`Box`) as its turned bounds.
#[test]
fn a_static_bbox_prop_blocks_as_a_world_box() {
    use crate::stack::map::MapBrushes;
    for (solid, axis_aligned) in [(PropSolid::WorldBox, true), (PropSolid::Box, false)] {
        let mut data = MapData {
            name: "test:prop_motion_box".into(),
            entity_scale: SCALE,
            ..default()
        };
        let (a, b) = (to_engine(Vec3::new(-512.0, -512.0, -64.0)), to_engine(Vec3::new(512.0, 512.0, 0.0)));
        data.collision_brushes.push(MapBrush::from_box(a.min(b), a.max(b)));
        let (lo, hi) = (to_engine(Vec3::new(-2.0, -16.0, 0.0)), to_engine(Vec3::new(2.0, 16.0, 64.0)));
        data.models.push(MapModel {
            bounds: (lo.min(hi), lo.max(hi)),
            ..default()
        });
        data.props.push(MapProp {
            pose: None,
            ragdoll: None,
            model: 0,
            translation: to_engine(Vec3::new(100.0, 0.0, 0.0)),
            rotation: Quat::from_rotation_y(45f32.to_radians()),
            solid,
            skybox: false,
            lighting: None,
            vertex_light: None,
            casts_shadow: false,
            physics: None,
            fade: None,
            parent: None,
            entity: None,
            skin: 0,
            body: 0,
            detail: false,
        });
        let mut sim = Sim::new(CsSim, (MapPlugin::new(data), SourceMovementPlugin, CsWeaponsPlugin));
        sim.ticks(1);
        let brushes = &sim.app.world().resource::<MapBrushes>().0;
        let prop = brushes.iter().find(|b| to_source(b.min).x.min(to_source(b.max).x) > 50.0).expect("the prop's brush");
        let aligned = prop.planes.iter().all(|(n, _)| n.abs().max_element() > 0.999);
        assert_eq!(aligned, axis_aligned, "{solid:?}: {:?}", prop.planes);
        if axis_aligned {
            // The 4 x 32 footprint turned 45°: 12.7 each way (√2 × 18 / 2).
            let (lo, hi) = (to_source(prop.min), to_source(prop.max));
            let (lo, hi) = (lo.min(hi), lo.max(hi));
            assert!((hi.x - 100.0 - 12.73).abs() < 0.1 && (lo.y + 12.73).abs() < 0.1, "{lo} {hi}");
        }
    }
}
