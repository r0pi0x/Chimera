//! Hitboxes follow the body as drawn whatever the look did earlier in the
//! tick (playtest 2026-11-17: a TF2 practice target's hitboxes turned to
//! face away from a player walking round it, so its head couldn't be
//! shot). A bot's brain sets the look before the rules (toward the
//! player); the target's rule puts it back on its line after them, and
//! the body faces that line. Hitboxes are kept in the look's frame (the
//! trace turns them by `Intent::yaw`), so they are posed once the intents
//! are final (`SimSet::Commands`): the head box stands where the drawn
//! head is, for every look the brain took; and the schedule orders
//! `pose_hitboxes` after `SimSet::Commands` (the scenario alone passed with
//! the old, unordered schedule when the executor happened to pose after
//! the target's rule).

use std::sync::Arc;

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use chimera_core::core::{BodyRegion, Hitbox, Hitboxes, Hitgroup, Intent, SimSet};
use chimera_present::{
    MapPlugin,
    render::{prop_material::PropMaterial, rope_material::RopeMaterial, shadows::ShadowMaterial, sprite_material::SpriteMaterial, world_material::WorldMaterial},
};
use chimera_world::{
    harness::Sim,
    map::{BoneBox, BodyName, MapBone, MapCharacterModel, MapData, MapMesh, MapModel, anim::{AnimSet, Animator}},
};

/// The head, ahead of the feet along the body's forward (-Z at yaw 0).
const HEAD: Vec3 = Vec3::new(0.0, 1.6, -0.3);
/// Where the body faces (the target's line) and where the brain looks
/// (the player, round the side).
const LINE: f32 = 0.4;

#[derive(Resource)]
struct Brain(f32);

fn model() -> MapCharacterModel {
    let mesh = MapMesh {
        material: "test/body".into(),
        positions: vec![[-0.2, 0.0, 0.0], [0.2, 0.0, 0.0], [0.0, 0.4, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        uvs: vec![[0.0, 0.0]; 3],
        indices: vec![0, 1, 2],
        joints: vec![[1, 0, 0, 0]; 3],
        joint_weights: vec![[1.0, 0.0, 0.0, 0.0]; 3],
        color: [128; 3],
        ..default()
    };
    let bone = |name: &str, parent, position| MapBone { name: name.into(), parent, position, rotation: Quat::IDENTITY };
    MapCharacterModel {
        game: None,
        team: None,
        name: Some("body".into()),
        class: None,
        model: MapModel { meshes: vec![mesh], ..default() },
        hitboxes: vec![Hitbox::cuboid(HEAD, Vec3::splat(0.1), Quat::IDENTITY, BodyRegion::Head)],
        bones: vec![bone("root", None, Vec3::ZERO), bone("head", Some(0), HEAD)],
        root: Transform::IDENTITY,
        animations: Some(Arc::new(AnimSet { defaults: vec![(Quat::IDENTITY, Vec3::ZERO), (Quat::IDENTITY, HEAD)], parents: vec![None, Some(0)], ..default() })),
        boxes: vec![BoneBox { bone: 1, center: Vec3::ZERO, half: Vec3::splat(0.1), group: Hitgroup::Head }],
        ragdoll: None,
    }
}

/// A bot's brain: the look toward the player, before the rules.
fn brain(b: Res<Brain>, mut q: Query<&mut Intent>) {
    for mut i in &mut q {
        i.yaw = b.0;
    }
}

/// The target's rule: the look back on its line after the rules, and the
/// body drawn facing it (as a game's animation sets `Animator::yaw`).
fn line(mut q: Query<(&mut Intent, &mut Animator)>) {
    for (mut i, mut a) in &mut q {
        i.yaw = LINE;
        a.yaw = Some(LINE);
        a.main = Some(0);
        a.fixed = Some(chimera_core::later::Later::ready(vec![(Quat::IDENTITY, Vec3::ZERO), (Quat::IDENTITY, HEAD)]));
    }
}

#[test]
fn hitboxes_follow_the_bodys_facing_not_an_earlier_look() {
    let mut data = MapData { name: "test:hitboxes_facing".into(), entity_scale: 0.0254, ..default() };
    data.kit.characters.push(model());
    let mut sim = Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), move |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(data.clone()))
            .insert_resource(Brain(0.0))
            .add_systems(FixedUpdate, (brain.before(SimSet::Rules), line.after(SimSet::Rules).before(SimSet::Commands)));
    });
    let e = sim
        .app
        .world_mut()
        .spawn((Intent::default(), BodyName("body".into()), avian3d::prelude::ColliderAabb::new(Vec3::ZERO, Vec3::splat(0.3)), Transform::default()))
        .id();
    sim.ticks(3);
    let drawn = Quat::from_rotation_y(LINE) * HEAD;
    // The player walks round the target: the brain looks every way.
    for k in 0..8 {
        let look = k as f32 * std::f32::consts::FRAC_PI_4;
        sim.app.world_mut().resource_mut::<Brain>().0 = look;
        sim.ticks(2);
        let w = sim.app.world();
        let boxes = w.get::<Hitboxes>(e).expect("hitboxes");
        let yaw = w.get::<Intent>(e).unwrap().yaw;
        // Where the trace sees the head: the box turned by the look.
        let head = Quat::from_rotation_y(yaw) * boxes.0[0].center;
        assert!((head - drawn).length() < 1e-3, "brain looking {look:.2}: the head box at {head}, the drawn head at {drawn}");
    }
}

/// Whether the schedule `label` runs every system of `later` after `set`:
/// a path of ordering edges from `set` (or a set holding it) to `later`
/// (or a set holding it).
fn ordered_after(app: &App, label: impl bevy::ecs::schedule::ScheduleLabel, set: impl SystemSet, later: impl SystemSet) -> bool {
    use bevy::ecs::schedule::{NodeId, graph::Direction};
    let g = app.world().resource::<Schedules>().get(label).expect("the schedule").graph();
    let key = |s: bevy::ecs::schedule::InternedSystemSet| g.system_sets.get_key(s).map(NodeId::Set);
    let (Some(from), Some(to)) = (key(set.intern()), key(later.intern())) else { return false };
    let (hierarchy, dependency) = (g.hierarchy().graph(), g.dependency().graph());
    // A node and the sets holding it.
    let up = |n: NodeId| {
        let mut out = vec![n];
        let mut i = 0;
        while i < out.len() {
            let parents: Vec<NodeId> = hierarchy.neighbors_directed(out[i], Direction::Incoming).collect();
            for p in parents {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
            i += 1;
        }
        out
    };
    // `later`'s set, its systems (a system's type set holds it) and the
    // sets holding those (a chain's anonymous set, which takes the order).
    let mut targets = up(to);
    for member in hierarchy.neighbors_directed(to, Direction::Outgoing).collect::<Vec<_>>() {
        for n in up(member) {
            if !targets.contains(&n) {
                targets.push(n);
            }
        }
    }
    // `set` and its systems (each runs before what follows the set).
    let mut seen = vec![from];
    seen.extend(hierarchy.neighbors_directed(from, Direction::Outgoing));
    let mut i = 0;
    while i < seen.len() {
        for a in up(seen[i]) {
            for s in dependency.neighbors_directed(a, Direction::Outgoing) {
                if targets.contains(&s) {
                    return true;
                }
                if !seen.contains(&s) {
                    seen.push(s);
                }
            }
        }
        i += 1;
    }
    false
}

/// The order itself: the hitboxes are posed after the intents are final
/// (a deterministic check: the scenario above passed by chance on the
/// unordered schedule).
#[test]
fn hitboxes_are_posed_after_the_intents_are_final() {
    let mut data = MapData { name: "test:hitboxes_order".into(), entity_scale: 0.0254, ..default() };
    data.kit.characters.push(model());
    let mut sim = Sim::with((chimera_core::core::CorePlugin, chimera_core::console::ConsolePlugin), move |app| {
        app.init_asset::<Image>()
            .init_asset::<StandardMaterial>()
            .init_asset::<WorldMaterial>()
            .init_asset::<PropMaterial>()
            .init_asset::<SpriteMaterial>()
            .init_asset::<RopeMaterial>()
            .init_asset::<ShadowMaterial>()
            .init_asset::<SkinnedMeshInverseBindposes>()
            .add_plugins(MapPlugin::new(data.clone()));
    });
    sim.ticks(1);
    let pose = chimera_world::map::pose_hitboxes.into_system_set();
    assert!(ordered_after(&sim.app, FixedUpdate, SimSet::Commands, pose), "pose_hitboxes after SimSet::Commands");
    assert!(ordered_after(&sim.app, FixedUpdate, pose, SimSet::Movement), "and before SimSet::Movement");
    assert!(!ordered_after(&sim.app, FixedUpdate, SimSet::Movement, pose), "the check sees no order where there is none");
}
