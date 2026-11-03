//! HL2:DM's ragdolls (specs/hl2dm/player_models.md 10; its test cases
//! 39-42): no death pose; the killing force goes into body 0, the pelvis
//! (Δv = F/m_pelvis), every other body taking its mass share at the
//! pelvis (Δv = F/M) whatever was hit; one ragdoll per player, kept after
//! the respawn and removed at the next death. On a small made-up skeleton
//! (no install), on the harness.
//!
//! Cases 40 (.357 on a Combine) and 41 (crowbar on a woman) are the same
//! rule with the weapons' forces (`weapons.rs`) and the models' masses
//! (heavy `player_models.rs`).

use avian3d::prelude::LinearVelocity;
use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Damage, DamageKind, Health, Hitbox, Hitgroup};
use chimera_world::{
    harness::Sim,
    map::{
        BoneBox, MapBone, MapBrush, MapCharacterModel, MapData, MapModel, MapRagdoll, MapRagdollBody, MapRagdollJoint,
        MapSimPlugin, Ragdoll, RagdollBody,
        ragdoll::{Outlived, RagdollRules, Ragdolled, death_push},
    },
};
use hl2dm::{
    game::{movement, sounds::Dissolving, weapons},
    harness::Hl2Sim,
};

const SCALE: f32 = 0.0254;

fn cube(lo: Vec3, hi: Vec3) -> Vec<Vec3> {
    (0..8)
        .map(|i| {
            Vec3::new(
                if i & 1 == 0 { lo.x } else { hi.x },
                if i & 2 == 0 { lo.y } else { hi.y },
                if i & 4 == 0 { lo.z } else { hi.z },
            )
        })
        .collect()
}

fn body(bone: usize, lo: Vec3, hi: Vec3, mass: f32) -> MapRagdollBody {
    MapRagdollBody {
        bone,
        pieces: vec![cube(lo, hi)],
        mass,
        damping: 0.0,
        rotdamping: 0.0,
        inertia: 10.0,
        surfaceprop: "flesh".into(),
        friction: 0.8,
        elasticity: 0.0,
    }
}

/// Pelvis (12.7749 kg, the male citizen's), spine and head: 90 kg in all.
fn model() -> MapCharacterModel {
    let bone = |name: &str, parent: Option<usize>, at: Vec3| MapBone {
        name: name.into(),
        parent,
        position: at,
        rotation: Quat::IDENTITY,
    };
    let joint = |parent, child| MapRagdollJoint {
        parent,
        child,
        limits: [(-0.3, 0.3); 3],
    };
    let face = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let axes = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    MapCharacterModel {
        game: Some(hl2dm::game::GAME),
        team: None,
        name: None,
        class: None,
        model: MapModel::default(),
        hitboxes: vec![Hitbox::cuboid(Vec3::ZERO, Vec3::splat(0.1), Quat::IDENTITY, BodyRegion::from_hitgroup(Hitgroup::Head))],
        bones: vec![
            bone("ValveBiped.Bip01_Pelvis", None, Vec3::new(0.0, 0.0, 38.0)),
            bone("ValveBiped.Bip01_Spine2", Some(0), Vec3::new(0.0, 0.0, 6.0)),
            bone("ValveBiped.Bip01_Head1", Some(1), Vec3::new(0.0, 0.0, 20.0)),
        ],
        root: Transform::from_rotation(face * axes).with_scale(Vec3::splat(SCALE)),
        animations: None,
        boxes: vec![BoneBox {
            bone: 2,
            center: Vec3::new(0.0, 0.0, 4.5),
            half: Vec3::new(4.0, 3.5, 4.5),
            group: Hitgroup::Head,
        }],
        ragdoll: Some(MapRagdoll {
            bodies: vec![
                body(0, Vec3::new(-4.0, -6.0, -3.0), Vec3::new(4.0, 6.0, 6.0), 12.7749),
                body(1, Vec3::new(-4.0, -7.0, 0.0), Vec3::new(4.0, 7.0, 20.0), 72.0562),
                body(2, Vec3::new(-4.0, -3.5, 0.0), Vec3::new(4.0, 3.5, 9.0), 5.1689),
            ],
            joints: vec![joint(0, 1), joint(1, 2)],
            collision_pairs: Some(Vec::new()),
        }),
    }
}

fn sim() -> (Sim, Entity) {
    let (lo, hi) = (Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0));
    let map = MapData {
        name: "test:ragdoll".into(),
        collision_brushes: vec![MapBrush::from_box(lo, hi)],
        collision_hulls: vec![cube(lo, hi).iter().map(|v| v.to_array()).collect()],
        kit: chimera_world::map::GameKit {
            characters: vec![model()],
            ..default()
        },
        ..default()
    };
    let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(map));
    sim.set_tick_interval(hl2dm::game::TICK_INTERVAL);
    let p = sim.spawn_character(Vec3::new(0.0, 0.95, 0.0), movement::ID);
    sim.ticks(30);
    (sim, p)
}

/// A pistol-like head shot along -Z with `force` kg·in/s.
fn kill(sim: &mut Sim, p: Entity, force: f32) {
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.01;
    let point = sim.position(p) + Vec3::Y * 0.7;
    sim.app.world_mut().write_message(Damage {
        force: Vec3::NEG_Z * force * SCALE,
        target: p,
        attacker: None,
        amount: 5.0,
        point,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Head,
        kind: DamageKind::Bullet,
        weapon: None,
    });
}

fn ragdolls(sim: &mut Sim) -> Vec<(Entity, Ragdoll)> {
    let world = sim.app.world_mut();
    world.query::<(Entity, &Ragdoll)>().iter(world).map(|(e, r)| (e, r.clone())).collect()
}

fn velocities(sim: &mut Sim) -> Vec<Vec3> {
    let world = sim.app.world_mut();
    let mut out: Vec<(usize, Vec3)> =
        world.query::<(&RagdollBody, &LinearVelocity)>().iter(world).map(|(b, v)| (b.index, v.0)).collect();
    out.sort_by_key(|b| b.0);
    out.into_iter().map(|(_, v)| v).collect()
}

#[test]
fn the_killing_force_goes_into_the_pelvis() {
    // Case 39: a pistol kill (F = 666.57) on a male citizen: the pelvis
    // 52.18 in/s, every other body 666.57/90 = 7.406 in/s (here only the
    // push's line; bone velocities are ~0 standing still), even shot in
    // the head; no death pose.
    let (mut sim, p) = sim();
    kill(&mut sim, p, 666.57);
    sim.ticks(1);
    let r = ragdolls(&mut sim);
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].1.death_pose, None);
    // The joints share it out within the first step, but not its
    // momentum along the shot: the pelvis's F plus every other body's
    // (m_j/M) F, F (1 + (M - m_pelvis)/M).
    let masses = [12.7749, 72.0562, 5.1689];
    let v = velocities(&mut sim);
    let momentum: f32 = v.iter().zip(masses).map(|(v, m)| -v.z / SCALE * m).sum();
    let want = 666.57 * (1.0 + (90.0 - 12.7749) / 90.0);
    assert!((momentum - want).abs() < want * 0.02, "momentum {momentum} kg in/s, want {want}");
    // As CS:S's rule would push it (into the head, 2400 kg·in/s): far more.
    let hit = Damage {
        force: Vec3::NEG_Z * 666.57 * SCALE,
        target: p,
        attacker: None,
        amount: 1.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Head,
        kind: DamageKind::Bullet,
        weapon: None,
    };
    let (k, f) = death_push(hl2dm::game::bodies::RAGDOLL, Some(2), &hit);
    assert_eq!(k, Some(0));
    assert!((f.length() / SCALE - 666.57).abs() < 0.01);
    let (k, f) = death_push(RagdollRules::default(), Some(2), &hit);
    assert_eq!(k, Some(2));
    assert!((f.length() / SCALE - 2400.0).abs() < 0.01);
}

#[test]
fn one_ragdoll_kept_until_the_next_death() {
    // Case 42.
    let (mut sim, p) = sim();
    kill(&mut sim, p, 666.57);
    sim.ticks(5);
    let first = ragdolls(&mut sim);
    assert_eq!(first.len(), 1);
    // It lives again: the ragdoll stays, the player has its body back.
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 1.0;
    sim.ticks(60);
    let kept = ragdolls(&mut sim);
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].0, first[0].0);
    assert!(sim.app.world().get::<Outlived>(kept[0].0).is_some());
    assert!(sim.app.world().get::<Ragdolled>(p).is_none());
    // Dies again: the first goes, a new one comes.
    kill(&mut sim, p, 666.57);
    sim.ticks(3);
    let second = ragdolls(&mut sim);
    assert_eq!(second.len(), 1);
    assert_ne!(second[0].0, first[0].0);
}

/// The energy ball's kill: the same hit as `kill`, by the ball (an AR2's
/// flyer, crush).
fn kill_by_ball(sim: &mut Sim, p: Entity) {
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.01;
    let point = sim.position(p) + Vec3::Y * 0.7;
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: p,
        attacker: None,
        amount: 10.0,
        point,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Crush,
        weapon: Some(weapons::AR2),
    });
}

#[test]
fn the_balls_victim_dissolves_and_is_gone_134_ticks_on_9_11() {
    // H9 cases 9 and 11: no death voice (the victim is marked), the ragdoll
    // is removed at the first tick at or after 2.0 s, floating up at
    // 0.02 g meanwhile.
    let (mut sim, p) = sim();
    let mut sounds = sim.app.world().resource::<Messages<chimera_world::map::PlaySound>>().get_cursor_current();
    kill_by_ball(&mut sim, p);
    sim.ticks(2);
    assert!(sim.app.world().get::<Dissolving>(p).is_some());
    let r = ragdolls(&mut sim);
    assert_eq!(r.len(), 1);
    let heard: Vec<String> =
        sounds.read(sim.app.world().resource::<Messages<chimera_world::map::PlaySound>>()).map(|s| s.entry.clone()).collect();
    assert!(heard.iter().all(|e| !e.to_lowercase().ends_with(".die")), "{heard:?}");
    // 130 ticks later (132 after the kill) it is still there, at 134 gone.
    sim.ticks(129);
    assert_eq!(ragdolls(&mut sim).len(), 1);
    sim.ticks(5);
    assert!(ragdolls(&mut sim).is_empty(), "removed 134 ticks after the kill");
    // A bullet's victim is not marked.
    let (mut sim, p) = self::sim();
    kill(&mut sim, p, 666.57);
    sim.ticks(2);
    assert!(sim.app.world().get::<Dissolving>(p).is_none());
    // And one that lives again can die with a voice.
    let (mut sim, p) = self::sim();
    kill_by_ball(&mut sim, p);
    sim.ticks(2);
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 1.0;
    sim.ticks(2);
    assert!(sim.app.world().get::<Dissolving>(p).is_none());
}
