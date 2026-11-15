//! A Halo body's flinch (`game::body::flinch`, health_shields.md "Stun and
//! pings", animations.md "Damage and death"): on synthetic data, a hit
//! starts the s-ping overlay at frame 0 and no hit nothing, it is posed for
//! 8 ticks (frames 0 to 7) and then gone, the body back on its base; a
//! second hit does not restart it; the permutation by the draw against the
//! cumulative weights; what counts as a hit in the replicated vitality
//! (the spec's pistol case: soft, soft, none at the shield's break, soft).
//! Over the network (`NetSim` on `HaloSim`, the greybox with a synthetic
//! Spartan): the server's copy of a player and another client's copy of it
//! both flinch from a hit the server applies.

use std::{sync::Arc, time::Duration};

use bevy::prelude::*;
use chimera_core::core::BodyRegion;
use chimera_world::map::{
    CharacterModels, MapCharacterModel,
    anim::{AnimEvent, AnimSet, Animation, Sequence, Track},
};
use halo_ce::game::{
    body::{
        HaloBody,
        anims::{HaloAnim, Inputs, Landing, damage_key, key},
        flinch::{self, Ping, Seen, WEIGHT_EVENT, hit_between, ping_slot, variants},
    },
    vitality::{DamageEffects, Event, Spartan, Vitality, ids},
};

const LAND: Landing = Landing { soft_min: 1.0, hard_min: 2.0, soft_ticks: 3, hard_ticks: 6 };

/// One bone; `stand ar idle` holds it still; the ping slot's three
/// permutations each turn it about X by 0.1, 0.2, 0.3 rad (9 frames,
/// overlays), cumulative weights 1/3, 2/3, 1.
fn set() -> AnimSet {
    let mut s = AnimSet {
        defaults: vec![(Quat::IDENTITY, Vec3::ZERO)],
        bases: vec![vec![(Quat::IDENTITY, Vec3::ZERO)]],
        parents: vec![None],
        ..Default::default()
    };
    let add = |s: &mut AnimSet, name: String, anim: Animation, events: Vec<AnimEvent>| {
        s.animations.push(anim);
        s.sequences.push(Sequence { name: name.clone(), anims: vec![s.animations.len() - 1], grid: (1, 1), events, ..Default::default() });
        s.names.insert(name, s.sequences.len() - 1);
    };
    let still = Animation {
        name: "idle".into(),
        fps: 30.0,
        frames: 10,
        tracks: vec![Track { bone: 0, rotation: Some(vec![Quat::IDENTITY; 10]), position: None }],
        ..Default::default()
    };
    add(&mut s, key("stand", "ar", "idle"), still, Vec::new());
    for k in 0..3 {
        let turn = Animation {
            name: format!("ping {k}"),
            fps: 30.0,
            frames: 9,
            delta: true,
            tracks: vec![Track { bone: 0, rotation: Some(vec![Quat::from_rotation_x(0.1 * (k + 1) as f32); 9]), position: None }],
            ..Default::default()
        };
        let weight = AnimEvent { cycle: (k + 1) as f32 / 3.0, event: 0, name: WEIGHT_EVENT.into(), options: String::new() };
        add(&mut s, flinch::permutation_key(&damage_key(ping_slot()), k), turn, vec![weight]);
    }
    s
}

fn still() -> Inputs {
    Inputs { grounded: true, ..Default::default() }
}

fn turn_x(q: Quat) -> f32 {
    let (axis, angle) = q.to_axis_angle();
    angle * axis.x.signum()
}

#[test]
fn a_hit_flinches_and_no_hit_does_not() {
    let s = set();
    let mut a = HaloAnim::default();
    for _ in 0..3 {
        let p = a.tick(&s, "ar", &still(), &LAND);
        assert!(turn_x(p[0].0).abs() < 1e-5);
    }
    assert_eq!(a.ping, None, "no hit, no flinch");
    let p = a.tick(&s, "ar", &Inputs { hit: Some(0.1), ..still() }, &LAND);
    assert_eq!(a.ping.map(|p| p.frame), Some(0), "the hit's tick shows frame 0");
    assert!((turn_x(p[0].0) - 0.1).abs() < 1e-4, "the overlay on the base: {}", turn_x(p[0].0));
}

#[test]
fn a_flinch_is_posed_for_eight_ticks_then_the_base_returns() {
    let s = set();
    let mut a = HaloAnim::default();
    a.tick(&s, "ar", &still(), &LAND);
    let mut frames = vec![];
    a.tick(&s, "ar", &Inputs { hit: Some(0.9), ..still() }, &LAND);
    frames.push(a.ping.unwrap().frame);
    let mut posed = 1;
    loop {
        let p = a.tick(&s, "ar", &still(), &LAND);
        match a.ping {
            Some(ping) => {
                frames.push(ping.frame);
                posed += 1;
                assert!((turn_x(p[0].0) - 0.3).abs() < 1e-4, "the third permutation");
            }
            None => {
                assert!(turn_x(p[0].0).abs() < 1e-5, "back on the base");
                break;
            }
        }
    }
    assert_eq!(frames, (0..8).collect::<Vec<_>>(), "frames 0 to 7, removed on reaching the last (8)");
    assert_eq!(posed, 8);
}

#[test]
fn a_second_hit_does_not_restart_a_running_flinch() {
    let s = set();
    let mut a = HaloAnim::default();
    a.tick(&s, "ar", &Inputs { hit: Some(0.1), ..still() }, &LAND);
    for _ in 0..3 {
        a.tick(&s, "ar", &still(), &LAND);
    }
    let running = a.ping.unwrap();
    assert_eq!(running.frame, 3);
    a.tick(&s, "ar", &Inputs { hit: Some(0.9), ..still() }, &LAND);
    assert_eq!(a.ping, Some(Ping { frame: 4, ..running }), "frame 3 <= 9: kept, not restarted");
}

#[test]
fn the_permutation_follows_the_cumulative_weights() {
    let s = set();
    let v = variants(&s);
    assert_eq!(v.len(), 3);
    let name = damage_key(ping_slot());
    assert_eq!(name, "damage s-ping front gut");
    let pick = |r: f32| flinch::choose(&v, r).map(|q| s.sequences[q].name.clone()).unwrap();
    assert_eq!(pick(0.0), name);
    assert_eq!(pick(0.3), name);
    assert_eq!(pick(0.5), format!("{name}%1"));
    assert_eq!(pick(0.99), format!("{name}%2"));
    // Every direction and part plays this one slot: a set without it, none.
    assert!(variants(&AnimSet::default()).is_empty());
    let mut a = HaloAnim::default();
    a.tick(&AnimSet::default(), "ar", &Inputs { hit: Some(0.5), ..still() }, &LAND);
    assert_eq!(a.ping, None);
}

/// The spec's case: pistol body shots 1 to 4 on a full Spartan (each after
/// the last flinch is over) flinch soft, soft, none (the shield breaks:
/// forced hard, no h-ping), soft; ticks without a hit and the shield's
/// recharge do not.
#[test]
fn what_the_replicated_vitality_says_of_a_hit() {
    let spartan = Spartan::load();
    let effects = DamageEffects::load();
    let pistol = effects.get(ids::PISTOL_BULLET).unwrap();
    let mut v = Vitality::default();
    let mut seen = Seen::of(&v);
    let mut flinches = Vec::new();
    for _ in 0..4 {
        let e = Event {
            effect: pistol,
            amount: pistol.amount(1.0, 0.0),
            scale: 1.0,
            material: Some(spartan.material_of(Some(BodyRegion::Chest))),
            friendly: false,
            unsuspecting: false,
            bypass_shields: false,
            game_multiplier: 1.0,
        };
        v.take(&spartan, &e);
        v.tick(&spartan);
        let now = Seen::of(&v);
        flinches.push(hit_between(&seen, &now));
        seen = now;
        for _ in 0..12 {
            v.tick(&spartan);
            let now = Seen::of(&v);
            assert!(!hit_between(&seen, &now), "no hit, no flinch: {seen:?} {now:?}");
            seen = now;
        }
    }
    assert_eq!(flinches, [true, true, false, true]);
    // The overshield drains without a hit; a dead body never flinches.
    let over = Seen { shield: 2.0, ..Seen::default() };
    assert!(!hit_between(&over, &Seen { shield: 1.99, ..over }));
    assert!(!hit_between(&Seen::default(), &Seen { body: 0.5, delay: 180, dead: true, ..Seen::default() }));
    assert!(hit_between(&Seen::default(), &Seen { delay: 180, ..Seen::default() }), "another game's hit: the delay");
    // The draw is the values', the same on every machine.
    let s = Seen { shield: 0.4, body: 0.9, delay: 179, ..Seen::default() };
    assert_eq!(flinch::draw(&s), flinch::draw(&Seen { ..s }));
    assert!((0.0..1.0).contains(&flinch::draw(&s)));
}

/// The synthetic Spartan as a map's Halo character model.
fn models() -> CharacterModels {
    CharacterModels(Arc::new(vec![MapCharacterModel {
        game: Some(halo_ce::game::GAME),
        team: None,
        name: None,
        class: None,
        model: Default::default(),
        hitboxes: Vec::new(),
        bones: Vec::new(),
        root: Transform::IDENTITY,
        animations: Some(Arc::new(set())),
        boxes: Vec::new(),
        ragdoll: None,
    }]))
}

#[test]
fn the_server_and_another_client_flinch_a_hit_player() {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use halo_ce::game::damage::HaloHit;
    let link = LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(halo_ce::harness::HaloSim, link, 7, 2, |app: &mut App| {
        app.add_plugins(chimera_world::greybox::GreyboxMapPlugin);
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(10);
    // The body as the map's would give it (the harness has no body
    // attaching: Halo's identity, the model, its animator).
    let wear = |w: &mut World| {
        w.insert_resource(models());
        let all: Vec<Entity> = w.query_filtered::<Entity, With<Vitality>>().iter(w).collect();
        for e in all {
            w.entity_mut(e).insert((
                chimera_core::slots::GameIdentity(halo_ce::game::GAME.into()),
                chimera_world::map::BodyModel(0),
                chimera_world::map::anim::Animator::new(Arc::new(set())),
            ));
        }
    };
    wear(sim.server.app.world_mut());
    for c in &mut sim.clients {
        wear(c.app.world_mut());
    }
    sim.ticks(30);
    let victim = sim.character_of(0).expect("client 0's player on the server");
    let watcher = sim.local_player(1).expect("client 1's own player");
    let pinged = |w: &World, e: Entity| w.get::<HaloBody>(e).and_then(|b| b.anim.ping);
    {
        let w = sim.server.app.world();
        let e = w.entity(victim);
        assert!(
            e.contains::<HaloBody>(),
            "the server animates its body: model {:?} animator {} vitality {} identity {:?}",
            e.get::<chimera_world::map::BodyModel>().map(|b| b.0),
            e.contains::<chimera_world::map::anim::Animator>(),
            e.contains::<Vitality>(),
            e.get::<chimera_core::slots::GameIdentity>().map(|g| g.0.clone()),
        );
    }
    assert!(pinged(sim.server.app.world(), victim).is_none(), "no hit yet");
    let fx = sim.server.app.world().resource::<DamageEffects>().clone();
    let amount = fx.get(ids::PISTOL_BULLET).unwrap().amount(1.0, 0.0);
    sim.server.app.world_mut().write_message(HaloHit::direct(
        victim,
        ids::PISTOL_BULLET,
        amount,
        1.0,
        Some(BodyRegion::Chest),
        Vec3::ZERO,
        Vec3::NEG_Z,
        None,
    ));
    // The server: on the hit's tick.
    let mut on_server = false;
    let mut on_client = false;
    for _ in 0..20 {
        sim.ticks(1);
        on_server |= pinged(sim.server.app.world(), victim).is_some();
        let w = sim.clients[1].app.world_mut();
        let mut q = w.query_filtered::<(Entity, &HaloBody), With<Vitality>>();
        on_client |= q.iter(w).any(|(e, b)| e != watcher && b.anim.ping.is_some());
    }
    assert!(on_server, "the server's copy flinched");
    assert!(on_client, "client 1's copy of client 0's player flinched");
    // And both are over (8 ticks) a while later.
    sim.ticks(20);
    assert!(pinged(sim.server.app.world(), victim).is_none());
}
