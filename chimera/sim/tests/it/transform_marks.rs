//! Changed transform trees marked on the schedule's thread
//! (`chimera_world::map::transform_marks`; feedback b33ceded: Bevy's
//! `mark_dirty_trees` in nearly every frame spike on dod_flash, 183
//! dynamic bodies). A map-like scene: a static root with 2,000 parts, 200
//! dynamic bodies at rest on a floor, a few kept moving, and a body with a
//! child. The child follows its parent every tick (the trees are still
//! marked, the propagation still runs), and Bevy's marking, with its
//! tasks, does nothing (`Sim::work`: a few hundred instructions a run, its
//! setting read and nothing else). What the marking costs is measured
//! optimized, with the renderer (`frame_bench`; docs/performance.md,
//! "Per-frame work in a match when nothing changed"): a test build has
//! Bevy at opt-level 3 and ours at 1.

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_world::harness::{Sim, own_process};

const PARTS: usize = 2000;
const BODIES: usize = 200;
const MOVING: usize = 5;
const TICKS: u64 = 300;

/// Bevy's marking's instructions a run, held (its setting read, a return):
/// about 800 measured; 137k a run when it marks.
const HELD: u64 = 5_000;

#[derive(Component)]
struct Mover(f32);

#[derive(Component)]
struct Rider;

fn start() -> (Sim, Entity, Entity) {
    let mut sim = Sim::with(chimera_core::core::CorePlugin, |_| {});
    let w = sim.app.world_mut();
    let root = w.spawn((Transform::default(), Visibility::default())).id();
    for i in 0..PARTS {
        w.spawn((Transform::from_xyz(i as f32, 0.0, 0.0), ChildOf(root)));
    }
    w.spawn((RigidBody::Static, Collider::cuboid(400.0, 1.0, 400.0), Transform::from_xyz(0.0, -0.5, 0.0)));
    for i in 0..BODIES {
        let (x, z) = ((i % 20) as f32 * 2.0, (i / 20) as f32 * 2.0);
        w.spawn((RigidBody::Dynamic, Collider::cuboid(0.5, 0.5, 0.5), Transform::from_xyz(x, 0.25, z)));
    }
    for i in 0..MOVING {
        w.spawn((RigidBody::Kinematic, Collider::cuboid(0.5, 0.5, 0.5), Transform::from_xyz(-10.0 - i as f32 * 2.0, 2.0, 0.0), Mover(i as f32)));
    }
    let parent = w.spawn((Transform::from_xyz(0.0, 5.0, -10.0), Mover(9.0))).id();
    let rider = w.spawn((Transform::from_xyz(0.0, 1.0, 0.0), Rider, ChildOf(parent))).id();
    sim.app.add_systems(FixedUpdate, |mut q: Query<(&mut Transform, &Mover)>| {
        for (mut t, m) in &mut q {
            t.translation.x += 0.01 * (1.0 + m.0);
        }
    });
    (sim, parent, rider)
}

#[test]
fn a_child_follows_its_moving_parent_every_tick() {
    let (mut sim, parent, rider) = start();
    for _ in 0..30 {
        sim.ticks(1);
        let w = sim.app.world();
        let p = w.get::<GlobalTransform>(parent).unwrap().translation();
        let r = w.get::<GlobalTransform>(rider).unwrap().translation();
        assert!((r - (p + Vec3::Y)).length() < 1e-4, "the rider at {r}, its parent at {p}");
        assert!(p.x > 0.0, "the parent moved: {p}");
    }
}

#[test]
fn bevy_s_marking_does_nothing() {
    if !own_process("transform_marks::bevy_s_marking_does_nothing") {
        return;
    }
    let (mut sim, _, _) = start();
    sim.repeatable();
    sim.ticks(120);
    let Some(w) = sim.work(TICKS) else {
        eprintln!("skipping: no hardware counters");
        return;
    };
    let rows: Vec<(&str, u64, u64)> = w
        .systems
        .0
        .iter()
        .filter(|(k, _)| k.contains("mark_dirty_trees") || k.contains("mark_changed_trees"))
        .map(|(k, v)| (k.as_str(), v.0 / v.1.max(1), v.1))
        .collect();
    eprintln!("marking, instructions a run ({PARTS} parts, {BODIES} bodies, {MOVING} moving): {rows:?}");
    let ours = rows.iter().filter(|r| r.0.contains("mark_changed_trees")).count();
    assert_eq!(ours, 2, "ours runs in PostUpdate and FixedPostUpdate: {rows:?}");
    for (name, per_run, _) in rows.iter().filter(|r| r.0.contains("mark_dirty_trees")) {
        assert!(*per_run < HELD, "{name} costs {per_run} instructions a run (held: under {HELD}): {rows:?}");
    }
}
