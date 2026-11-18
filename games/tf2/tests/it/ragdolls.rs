//! TF2's ragdoll lifetime (specs/tf2/ragdolls.md, its test cases): the
//! timer alone frame by frame as the spec counts it (frames of 1/64 s,
//! `RagdollTimer::step` with data/ragdolls.toml's numbers), then on the
//! harness with a small made-up skeleton of TF2's (no install), TF2's
//! 15 ms tick: kept past the respawn, gone at the next death, a round
//! restart or the owner leaving, no cap on the count, and each machine's
//! timer by its view (`RagdollInView`, set here by hand as the drawing
//! layer's `render::ragdoll_view` sets it from the camera).
//!
//! Not here: T11 (feign bodies), T12 (a team switch is T10's death), T13
//! (death animations), T18 (decapitation), T20 (late arrival), T21 (ice):
//! TF2's game has none of them (`game::bodies`' known shortcuts).

use bevy::prelude::*;
use chimera_core::core::{BodyRegion, Damage, DamageKind, Health, Hitbox, Hitgroup, RoundRestarts};
use chimera_world::{
    harness::Sim,
    map::{
        BoneBox, MapBone, MapBrush, MapCharacterModel, MapData, MapModel, MapRagdoll, MapRagdollBody, MapRagdollJoint,
        MapSimPlugin, Ragdoll,
        ragdoll::{RagdollFade, RagdollInView, RagdollLifetime, RagdollSettings, RagdollTimer, Ragdolled},
    },
};
use tf2::{
    game::{bodies::ragdoll_rules, movement},
    harness::Tf2Sim,
};

const SCALE: f32 = 0.0254;
const FRAME: f64 = 1.0 / 64.0;

fn fade() -> RagdollFade {
    match ragdoll_rules().lifetime {
        RagdollLifetime::Faded(f) => f,
        other => panic!("TF2's ragdolls are timed, not {other:?}"),
    }
}

/// The spec's clock: a body made at 0, evaluated on frames k/64 from
/// k = 0, `in_view(t)` saying whether frame t sees it. The time it is
/// removed at and the alphas of every frame (forced fade).
fn run(fade: &RagdollFade, forced: bool, dt: f64, in_view: impl Fn(f64) -> bool, until: f64) -> (Option<f64>, Vec<u8>) {
    let mut t = RagdollTimer::new(fade, forced, 0.0);
    let mut alphas = Vec::new();
    let mut k = 0u32;
    loop {
        let now = k as f64 * dt;
        if now > until {
            return (None, alphas);
        }
        let gone = t.step(fade, forced, in_view(now), now, dt);
        alphas.push(t.alpha.unwrap_or(255));
        if gone {
            return (Some(now), alphas);
        }
        k += 1;
    }
}

#[test]
fn the_data_holds_the_specs_numbers() {
    let f = fade();
    assert_eq!((f.fade_time, f.view_fraction, f.forced_fade_speed), (15.0, 0.33, 600.0));
    let r = ragdoll_rules();
    assert!(r.outlives_respawn(), "kept past the respawn (spec 2)");
}

#[test]
fn t1_never_seen_goes_just_after_fifteen_seconds() {
    let (gone, _) = run(&fade(), false, FRAME, |_| false, 100.0);
    assert_eq!(gone, Some(15.015625), "present at 15.0 (R < now is strict), removed the next frame");
}

#[test]
fn t2_t3_t19_seen_it_goes_a_third_of_the_time_after_the_last_look() {
    // T2: seen to 2.0: R = 6.95, present at 6.9375.
    assert_eq!(run(&fade(), false, FRAME, |t| t <= 2.0, 100.0).0, Some(6.953125));
    // T3: seen to 10.0: R = 14.95, before the creation timer's 15.
    assert_eq!(run(&fade(), false, FRAME, |t| t <= 10.0, 100.0).0, Some(14.953125));
    // T19: F = 5, seen to 1.0: R = 1 + 1.65.
    let short = RagdollFade {
        fade_time: 5.0,
        ..fade()
    };
    assert_eq!(run(&short, false, FRAME, |t| t <= 1.0, 100.0).0, Some(2.65625));
}

#[test]
fn t4_always_seen_stays_opaque() {
    let (gone, alphas) = run(&fade(), false, FRAME, |_| true, 60.0);
    assert_eq!(gone, None);
    assert!(alphas.iter().all(|a| *a == 255));
}

#[test]
fn t14_t15_t16_t17_a_forced_fade() {
    // T14: seen at 0; 245 a frame on (255 - 9.375 truncated), then 10 a
    // frame: 5 at 25/64, gone at 26/64.
    let (gone, alphas) = run(&fade(), true, FRAME, |_| true, 10.0);
    assert_eq!(gone, Some(0.40625));
    assert_eq!(&alphas[..4], &[255, 245, 235, 225]);
    assert_eq!(alphas[25], 5);
    assert_eq!(alphas[26], 0);
    // T15: out of view from 0.1, the fade goes on.
    assert_eq!(run(&fade(), true, FRAME, |t| t < 0.1, 10.0).0, Some(0.40625));
    // T16: never seen: gone on its first evaluation.
    assert_eq!(run(&fade(), true, FRAME, |_| false, 10.0).0, Some(0.0));
    // T17: frames of 1/128: steps of 5, 5 at 50/128, gone at 51/128.
    let (gone, alphas) = run(&fade(), true, 1.0 / 128.0, |_| true, 10.0);
    assert_eq!(gone, Some(0.3984375));
    assert_eq!(alphas[50], 5);
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

/// A pelvis and a head, TF2's (100 kg in all).
fn model() -> MapCharacterModel {
    let bone = |name: &str, parent: Option<usize>, at: Vec3| MapBone {
        name: name.into(),
        parent,
        position: at,
        rotation: Quat::IDENTITY,
    };
    let face = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
    let axes = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    MapCharacterModel {
        game: Some(tf2::game::GAME),
        team: None,
        name: None,
        class: None,
        model: MapModel::default(),
        hitboxes: vec![Hitbox::cuboid(Vec3::ZERO, Vec3::splat(0.1), Quat::IDENTITY, BodyRegion::from_hitgroup(Hitgroup::Head))],
        bones: vec![
            bone("bip_pelvis", None, Vec3::new(0.0, 0.0, 38.0)),
            bone("bip_head", Some(0), Vec3::new(0.0, 0.0, 26.0)),
        ],
        root: Transform::from_rotation(face * axes).with_scale(Vec3::splat(SCALE)),
        animations: None,
        boxes: vec![BoneBox {
            bone: 1,
            center: Vec3::new(0.0, 0.0, 4.5),
            half: Vec3::new(4.0, 3.5, 4.5),
            group: Hitgroup::Head,
        }],
        ragdoll: Some(MapRagdoll {
            bodies: vec![
                body(0, Vec3::new(-4.0, -6.0, -3.0), Vec3::new(4.0, 6.0, 20.0), 90.0),
                body(1, Vec3::new(-4.0, -3.5, 0.0), Vec3::new(4.0, 3.5, 9.0), 10.0),
            ],
            joints: vec![MapRagdollJoint {
                parent: 0,
                child: 1,
                limits: [(-0.3, 0.3); 3],
            }],
            collision_pairs: Some(Vec::new()),
        }),
    }
}

/// A flat floor with `players` TF2 players standing on it.
fn sim(players: usize) -> (Sim, Vec<Entity>) {
    let (lo, hi) = (Vec3::new(-20.0, -1.0, -20.0), Vec3::new(20.0, 0.0, 20.0));
    let map = MapData {
        name: "test:tf2_ragdolls".into(),
        collision_brushes: vec![MapBrush::from_box(lo, hi)],
        collision_hulls: vec![cube(lo, hi).iter().map(|v| v.to_array()).collect()],
        kit: chimera_world::map::GameKit {
            characters: vec![model()],
            ..default()
        },
        ..default()
    };
    let mut sim = Sim::new(Tf2Sim, MapSimPlugin::new(map));
    sim.set_tick_interval(tf2::game::TICK_INTERVAL);
    sim.app.world_mut().init_resource::<RoundRestarts>();
    let ps = (0..players)
        .map(|i| sim.spawn_character(Vec3::new(-15.0 + 3.0 * (i % 10) as f32, 0.95, -6.0 + 4.0 * (i / 10) as f32), movement::ID))
        .collect();
    sim.ticks(30);
    (sim, ps)
}

fn kill(sim: &mut Sim, p: Entity) {
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.01;
    let point = sim.position(p) + Vec3::Y * 0.7;
    sim.app.world_mut().write_message(Damage {
        force: Vec3::NEG_Z * 1000.0 * SCALE,
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

fn respawn(sim: &mut Sim, p: Entity) {
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 125.0;
}

fn ragdolls(sim: &mut Sim) -> Vec<(Entity, Ragdoll)> {
    let world = sim.app.world_mut();
    world.query::<(Entity, &Ragdoll)>().iter(world).map(|(e, r)| (e, r.clone())).collect()
}

/// Ticks of TF2's 15 ms in `s` seconds.
fn ticks(s: f64) -> u64 {
    (s / tf2::game::TICK_INTERVAL).round() as u64
}

/// Kill `p`, then tick until its ragdoll comes (the tick it is made on);
/// the ragdoll.
fn die(sim: &mut Sim, p: Entity) -> Entity {
    let before: Vec<Entity> = ragdolls(sim).into_iter().map(|(e, _)| e).collect();
    kill(sim, p);
    for _ in 0..5 {
        sim.ticks(1);
        if let Some((e, _)) = ragdolls(sim).into_iter().find(|(e, r)| r.owner == p && !before.contains(e)) {
            return e;
        }
    }
    panic!("no ragdoll");
}

#[test]
fn t5_unseen_it_goes_fifteen_seconds_on_respawn_or_not() {
    let (mut sim, ps) = sim(1);
    let p = ps[0];
    die(&mut sim, p);
    // T5: it lives again 3 s on; the ragdoll stays (spec 2).
    sim.ticks(ticks(3.0));
    respawn(&mut sim, p);
    sim.ticks(ticks(10.9));
    assert_eq!(ragdolls(&mut sim).len(), 1, "present 14 s on");
    assert!(sim.app.world().get::<Ragdolled>(p).is_none(), "the player has its body back");
    sim.ticks(ticks(1.2));
    assert!(ragdolls(&mut sim).is_empty(), "gone just past 15 s");
}

#[test]
fn still_dead_when_it_goes_the_owner_stays_ragdolled() {
    // Removed before the owner lives again: no standing body comes back.
    let (mut sim, ps) = sim(1);
    let p = ps[0];
    die(&mut sim, p);
    sim.ticks(ticks(15.2));
    assert!(ragdolls(&mut sim).is_empty());
    assert!(sim.app.world().get::<Ragdolled>(p).is_some(), "still dead: no body drawn");
    respawn(&mut sim, p);
    sim.ticks(3);
    assert!(sim.app.world().get::<Ragdolled>(p).is_none(), "alive: its own body again");
}

#[test]
fn t2_t4_the_view_keeps_it_and_shortens_it() {
    let (mut sim, ps) = sim(2);
    let (a, b) = (ps[0], ps[1]);
    let ra = die(&mut sim, a);
    let rb = die(&mut sim, b);
    // T4: `a`'s always seen: there a minute on; `b`'s seen for 2 s, then
    // not: gone 4.95 s after the last look, well before 15 s.
    sim.app.world_mut().entity_mut(ra).insert(RagdollInView);
    sim.app.world_mut().entity_mut(rb).insert(RagdollInView);
    sim.ticks(ticks(2.0));
    sim.app.world_mut().entity_mut(rb).remove::<RagdollInView>();
    sim.ticks(ticks(4.8));
    assert!(sim.app.world().get_entity(rb).is_ok(), "present 4.8 s after the last look");
    sim.ticks(ticks(0.3));
    assert!(sim.app.world().get_entity(rb).is_err(), "gone 4.95 s after the last look");
    sim.ticks(ticks(60.0));
    assert!(sim.app.world().get_entity(ra).is_ok(), "seen throughout: kept");
    assert_eq!(sim.app.world().get::<RagdollTimer>(ra).unwrap().alpha, None, "no fade by default");
}

#[test]
fn t6_t10_dying_again_replaces_it() {
    let (mut sim, ps) = sim(1);
    let p = ps[0];
    let first = die(&mut sim, p);
    sim.app.world_mut().entity_mut(first).insert(RagdollInView);
    for _ in 0..2 {
        sim.ticks(ticks(2.0));
        respawn(&mut sim, p);
        sim.ticks(ticks(2.0));
        assert_eq!(ragdolls(&mut sim).len(), 1, "kept past the respawn");
        let next = die(&mut sim, p);
        sim.ticks(2);
        let now = ragdolls(&mut sim);
        assert_eq!(now.len(), 1, "one body for the owner");
        assert_eq!(now[0].0, next);
        assert!(sim.app.world().get_entity(first).is_err());
    }
}

#[test]
fn t7_a_round_restart_removes_every_one() {
    let (mut sim, ps) = sim(3);
    for p in &ps {
        let r = die(&mut sim, *p);
        sim.app.world_mut().entity_mut(r).insert(RagdollInView);
    }
    sim.ticks(ticks(1.0));
    assert_eq!(ragdolls(&mut sim).len(), 3);
    sim.app.world_mut().resource_mut::<RoundRestarts>().0 += 1;
    sim.ticks(2);
    assert!(ragdolls(&mut sim).is_empty());
}

#[test]
fn t8_the_owner_leaving_takes_only_its_own() {
    let (mut sim, ps) = sim(2);
    for p in &ps {
        let r = die(&mut sim, *p);
        sim.app.world_mut().entity_mut(r).insert(RagdollInView);
    }
    sim.app.world_mut().entity_mut(ps[0]).despawn();
    sim.ticks(2);
    let left = ragdolls(&mut sim);
    assert_eq!(left.len(), 1);
    assert_eq!(left[0].1.owner, ps[1]);
}

#[test]
fn t9_no_cap_on_the_count() {
    // More than `g_ragdoll_maxcount`'s 8, which doesn't apply (spec 7).
    let (mut sim, ps) = sim(12);
    for p in &ps {
        kill(&mut sim, *p);
    }
    sim.ticks(ticks(2.0));
    assert_eq!(ragdolls(&mut sim).len(), 12);
}

#[test]
fn t14_t16_forced_fades_on_the_tick() {
    let (mut sim, ps) = sim(2);
    sim.app.world_mut().resource_mut::<RagdollSettings>().force_fade = 1;
    // T16: never seen: gone on its first evaluation, the tick after the
    // one it was made on (the view has looked once by then).
    kill(&mut sim, ps[1]);
    sim.ticks(3);
    assert!(ragdolls(&mut sim).iter().all(|(_, r)| r.owner != ps[1]));
    // T14: seen, it fades 9 a tick (600 x 0.015) from the tick after:
    // 255 / 9 = 28.3, so 29 ticks.
    let r = die(&mut sim, ps[0]);
    sim.app.world_mut().entity_mut(r).insert(RagdollInView);
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<RagdollTimer>(r).unwrap().alpha, Some(255), "seen: fading from the next tick");
    sim.ticks(1);
    assert_eq!(sim.app.world().get::<RagdollTimer>(r).unwrap().alpha, Some(246));
    sim.ticks(27);
    assert_eq!(sim.app.world().get::<RagdollTimer>(r).unwrap().alpha, Some(3));
    sim.ticks(1);
    assert!(sim.app.world().get_entity(r).is_err(), "alpha 0: gone");
}

/// A party app (`ragdoll::BodiesGoOnRespawn`) removes TF2's ragdolls
/// when their owner lives again, over TF2's own timer; the standalone
/// game keeps them (t5 above).
#[test]
fn in_a_party_it_goes_on_the_respawn() {
    let (mut sim, ps) = sim(1);
    sim.app.world_mut().insert_resource(chimera_world::map::ragdoll::BodiesGoOnRespawn);
    let p = ps[0];
    die(&mut sim, p);
    sim.ticks(ticks(3.0));
    assert_eq!(ragdolls(&mut sim).len(), 1, "kept while its owner is dead");
    respawn(&mut sim, p);
    sim.ticks(3);
    assert!(ragdolls(&mut sim).is_empty(), "gone at the respawn");
    assert!(sim.app.world().get::<Ragdolled>(p).is_none(), "the player has its body back");
}
