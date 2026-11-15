//! The Spartan's body from the disc (`game::body`, specs/halo_ce/models.md
//! "Multiplayer Spartan", animations.md, collision.md): loaded from
//! bloodgulch with the spec's node, vertex and triangle counts and its
//! height in metres; the idle pose keeps the bind pose's bone lengths; a
//! hit volume per collision node with a BSP, the head's above the chest's
//! and flagged the head; on a flat floor with `HaloSim`, the kit's pistol
//! aimed at a standing Spartan's head reports a head hit and at its chest
//! a body hit; a moving Spartan's animation goes from idle to move-front.
//! Passes with a note without an install.

use std::sync::{Arc, Mutex};

use bevy::prelude::*;
use blam_formats::cache::MapFile;
use chimera_core::core::{Damage, Hitboxes, Hitgroup};
use chimera_sim::testkit::{self, PISTOL};
use chimera_world::{
    harness::Sim,
    map::{MapData, MapSimPlugin},
};
use halo_ce::{
    game::{
        METERS_PER_UNIT,
        body::{
            HaloBody,
            anims::{State, key},
            load::{self, DONOR_MAP, Spartan},
            skeleton::globals,
        },
        maps, movement,
    },
    harness::HaloSim,
};

use super::maps::mount;

fn spartan() -> Option<Spartan> {
    let mount = mount()?;
    let bytes = mount.read(&maps::map_path(DONOR_MAP)).unwrap();
    let file = MapFile::load(&bytes).unwrap();
    let start = std::time::Instant::now();
    let body = load::spartan(&file, false).unwrap();
    eprintln!(
        "the Spartan made in {:.2} s; warnings: {:?}",
        start.elapsed().as_secs_f32(),
        body.warnings
    );
    Some(body)
}

#[test]
fn the_spartan_loads_from_bloodgulch() {
    let Some(body) = spartan() else { return };
    let c = &body.character;
    assert_eq!(c.bones.len(), 19, "models.md: 19 nodes");
    assert_eq!(c.bones[0].name, "bip01 pelvis");
    assert_eq!(c.bones[18].name, "bip01 r hand");
    let vertices: usize = c.model.meshes.iter().map(|m| m.positions.len()).sum();
    let triangles: usize = c.model.meshes.iter().map(|m| m.indices.len() / 3).sum();
    assert_eq!(vertices, 2256, "the highest detail's vertices");
    assert_eq!(triangles, 2016, "the highest detail's triangles");
    assert_eq!(c.model.meshes.len(), 4, "one mesh per shader");
    for m in &c.model.meshes {
        assert_eq!(m.joints.len(), m.positions.len(), "skinned");
        assert!(
            m.joint_weights
                .iter()
                .all(|w| (w.iter().sum::<f32>() - 1.0).abs() < 1e-4)
        );
    }
    // 0.7036 WU tall, feet at the origin (models.md, the bounding box).
    let (lo, hi) = c.model.bounds;
    eprintln!("bounds {lo} .. {hi}");
    assert!((hi.y - 0.7036 * METERS_PER_UNIT).abs() < 0.01, "height {} m", hi.y);
    assert!(lo.y.abs() < 0.01, "feet at {} m", lo.y);
    // Facing -Z: the model's +X is forward; the hands (y +-0.133 WU) left
    // and right on the engine's X.
    let set = c.animations.as_ref().unwrap();
    assert!(set.names.contains_key(&key("stand", "unarmed", "idle")));
    assert!(set.names.contains_key(&key("stand", "ar", "move-front")));
    assert!(set.names.contains_key(&key("crouch", "ar", "idle")));
    assert!(set.names.contains_key(&key("stand", "ar", "aim-still")));
    eprintln!("{} sequences, {} animations", set.sequences.len(), set.animations.len());
    // The armour is tinted per colour.
    assert_eq!(c.model.skins.len(), halo_ce::game::body::COLOURS.len());
    assert!(
        c.model.skins[halo_ce::game::body::RED]
            .iter()
            .any(|l| l.tint == Some([1.0, 0.0, 0.0]))
    );
    let ok = c.model.meshes.iter().filter(|m| m.texture.is_some()).count();
    assert!(ok >= 2, "textured: {ok} of {}", c.model.meshes.len());
}

#[test]
fn the_idle_pose_keeps_the_bones_lengths() {
    let Some(body) = spartan() else { return };
    let c = &body.character;
    let set = c.animations.as_ref().unwrap();
    let mut anim = halo_ce::game::body::anims::HaloAnim::default();
    let land = halo_ce::game::body::anims::Landing {
        soft_min: 1.5,
        hard_min: 5.0,
        soft_ticks: 18,
        hard_ticks: 30,
    };
    let i = halo_ce::game::body::anims::Inputs {
        grounded: true,
        ..Default::default()
    };
    let mut pose = Vec::new();
    for _ in 0..12 {
        pose = anim.tick(set, "ar", &i, &land);
    }
    assert_eq!(anim.state, State::Idle);
    let bind = globals(&c.bones, &set.defaults);
    let idle = globals(&c.bones, &pose);
    for (n, b) in c.bones.iter().enumerate() {
        let Some(p) = b.parent else { continue };
        let a = bind[n].1.distance(bind[p].1);
        let z = idle[n].1.distance(idle[p].1);
        assert!((a - z).abs() < 2e-3, "{}: bind {a} idle {z}", b.name);
    }
    // Upright: the head above the pelvis, the feet near the ground.
    assert!(
        idle[12].1.z > idle[0].1.z + 0.15,
        "head {} pelvis {}",
        idle[12].1,
        idle[0].1
    );
    assert!(
        idle[8].1.z < 0.12 && idle[11].1.z < 0.12,
        "feet {} {}",
        idle[8].1,
        idle[11].1
    );
}

#[test]
fn hit_volumes_cover_every_collision_node() {
    let Some(body) = spartan() else { return };
    let c = &body.character;
    // collision.md: 19 nodes, 3 with no region and no BSP.
    assert_eq!(
        c.boxes.len(),
        16,
        "{:?}",
        c.boxes.iter().map(|b| &c.bones[b.bone].name).collect::<Vec<_>>()
    );
    for (b, h) in c.boxes.iter().zip(&c.hitboxes) {
        eprintln!(
            "{:18} {:?} centre {:.3} half {:.3} (WU {:.3})",
            c.bones[b.bone].name, h.region, h.center, h.half, b.half
        );
    }
    let head: Vec<_> = c
        .hitboxes
        .iter()
        .filter(|h| h.region.hitgroup() == Hitgroup::Head)
        .collect();
    assert_eq!(head.len(), 1, "one head");
    let chest = c
        .hitboxes
        .iter()
        .find(|h| h.region.hitgroup() == Hitgroup::Chest)
        .expect("a chest");
    assert!(
        head[0].center.y > chest.center.y + 0.2,
        "head {} chest {}",
        head[0].center,
        chest.center
    );
    let head_box = c.boxes.iter().find(|b| b.group == Hitgroup::Head).unwrap();
    assert_eq!(c.bones[head_box.bone].name, "bip01 head");
}

/// A flat floor 200 m across at y 0 with the Spartan in its kit.
fn floor(body: &Spartan) -> MapData {
    let mut map = MapData {
        collision_positions: vec![
            [-100.0, 0.0, -100.0],
            [100.0, 0.0, -100.0],
            [100.0, 0.0, 100.0],
            [-100.0, 0.0, 100.0],
        ],
        collision_indices: vec![[0, 2, 1], [0, 3, 2]],
        spawns: vec![(Vec3::new(0.0, 0.1, 0.0), None)],
        ..Default::default()
    };
    load::add(&mut map, body);
    map
}

/// The hit groups of the tick's damage.
#[derive(Resource, Default, Clone)]
struct Hits(Arc<Mutex<Vec<Hitgroup>>>);

fn record(mut d: MessageReader<Damage>, hits: Res<Hits>) {
    hits.0.lock().unwrap().extend(d.read().map(|d| d.hitgroup));
}

#[test]
fn a_bullet_hits_a_spartans_head_and_chest() {
    let Some(body) = spartan() else { return };
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(floor(&body)));
    testkit::add_weapons(&mut sim.app);
    let hits = Hits::default();
    sim.app
        .insert_resource(hits.clone())
        .add_systems(FixedPostUpdate, record);
    sim.ticks(3);
    let shooter = testkit::with_player(&mut sim, Some(Vec3::new(0.0, 1.2, 0.0)), PISTOL);
    let target = sim.spawn_character(Vec3::new(0.0, 1.2, -6.0), movement::ID);
    sim.seconds(1.0);
    assert!(
        sim.app.world().get::<HaloBody>(target).is_some(),
        "the target wears the Spartan"
    );
    let boxes = sim.app.world().get::<Hitboxes>(target).expect("hitboxes").clone();
    let head = boxes
        .0
        .iter()
        .find(|h| h.region.hitgroup() == Hitgroup::Head)
        .unwrap()
        .center;
    let chest = boxes
        .0
        .iter()
        .find(|h| h.region.hitgroup() == Hitgroup::Chest)
        .unwrap()
        .center;
    eprintln!("target at {}; head box {head}, chest box {chest}", sim.position(target));
    // The boxes are in the target's frame, its feet at the origin, on
    // the floor at 0.
    let ground = 0.0;
    for (height, want) in [(head.y, Hitgroup::Head), (chest.y, Hitgroup::Chest)] {
        hits.0.lock().unwrap().clear();
        let at = Vec3::new(sim.position(target).x, ground + height, sim.position(target).z);
        testkit::aim_at(&mut sim, shooter, at);
        sim.intent(shooter).fire = true;
        sim.ticks(1);
        sim.intent(shooter).fire = false;
        sim.seconds(0.6);
        let got = hits.0.lock().unwrap().clone();
        eprintln!("aimed at {at}: {got:?}");
        assert_eq!(got.first(), Some(&want), "aimed at {height} m");
    }
}

#[test]
fn a_moving_spartan_plays_move_front() {
    let Some(body) = spartan() else { return };
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(floor(&body)));
    sim.ticks(3);
    let p = sim.spawn_character(Vec3::new(0.0, 1.2, 0.0), movement::ID);
    sim.seconds(1.0);
    let state = |sim: &Sim| sim.app.world().get::<HaloBody>(p).unwrap().anim.state;
    assert_eq!(state(&sim), State::Idle);
    sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
    sim.seconds(0.5);
    assert_eq!(state(&sim), State::MoveFront);
    sim.intent(p).move_axis = Vec2::new(-1.0, 0.0);
    sim.seconds(0.6);
    assert_eq!(state(&sim), State::MoveLeft);
}
