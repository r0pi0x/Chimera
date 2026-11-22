//! TF2's weapon effects besides the per-shot flash (specs/tf2/effects.md;
//! `weapons::looks`, `weapons::blast`), on the greybox
//! (`harness::Tf2Sim`):
//!
//! - the Minigun (3): no per-shot flash; its constant muzzle and brass
//!   systems start once when it fires and stop (`StopParticleSystems`) when
//!   it stops; a tracer every 4th bullet;
//! - tracers (6): a Scattergun blast from a count of 0 draws 5;
//! - explosions (5): a rocket on the floor sends its surface and is no
//!   player hit, leaves a `Scorch` and starts the wall effect; the rocket
//!   trails `rockettrail` (1);
//! - the backblast (4): others see it at the launcher's `backblast`, the
//!   shooter (this machine's player) never;
//! - the sticky (1, 11.3): its trail as it appears, its armed pulse
//!   (`stickybomb_pulse_red`) once, its arm time (0.8 s, 0.6 for the
//!   Quickiebomb) after it appeared on that machine.
//!
//! A made-up first-person Minigun with `muzzle` and `eject_brass`
//! attachments, and a made-up held `backblast` attachment.

use bevy::prelude::*;
use chimera_core::core::LocalPlayer;
use chimera_sim::weapon::{WeaponEvent, WeaponEventKind};
use chimera_world::map::{
    DecalGroup, MapAttachment, MapBone, MapModel, MapViewModel, PlaceDecal, ViewAnimator, ViewModelAnchor, ViewModelCamera, ViewModels,
    psys::{StartParticleSystem, StopParticleSystems},
    view_model::HeldAttachment,
};
use tf2::game::weapons::{GRENADE_LAUNCHER, MINIGUN, ROCKET_LAUNCHER, SCATTERGUN, STICKY_LAUNCHER, blast::Tf2Explosion, looks::TracerCount};

use super::weapons::{holding, sim};

fn view_model(key: &str) -> MapViewModel {
    let at = |name: &str, p: Vec3| MapAttachment { name: name.into(), bone: 0, local: Transform::from_translation(p) };
    MapViewModel {
        key: key.into(),
        model: MapModel::default(),
        bones: vec![MapBone { name: "root".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: Transform::IDENTITY,
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: vec![at("muzzle", Vec3::new(0.1, -0.3, -1.0)), at("eject_brass", Vec3::new(0.1, -0.2, -0.4))],
        light_origin: Vec3::ZERO,
    }
}

/// Everything the looks start and stop over `ticks`, with `input` each tick.
fn collect(
    sim: &mut chimera_world::harness::Sim,
    ticks: u32,
    mut input: impl FnMut(&mut chimera_world::harness::Sim, u32),
) -> (Vec<StartParticleSystem>, Vec<StopParticleSystems>, usize) {
    let mut starts = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut stops = sim.app.world().resource::<Messages<StopParticleSystems>>().get_cursor_current();
    let mut events = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let (mut a, mut b, mut bullets) = (Vec::new(), Vec::new(), 0);
    for t in 0..ticks {
        input(sim, t);
        sim.ticks(1);
        let w = sim.app.world();
        a.extend(starts.read(w.resource::<Messages<StartParticleSystem>>()).cloned());
        b.extend(stops.read(w.resource::<Messages<StopParticleSystems>>()).copied());
        bullets += events
            .read(w.resource::<Messages<WeaponEvent>>())
            .filter(|e| e.shown() && matches!(e.kind, WeaponEventKind::Shot { .. }))
            .count();
    }
    (a, b, bullets)
}

#[test]
fn the_minigun_runs_its_constant_systems_while_firing_and_no_shot_flash() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), MINIGUN);
    sim.app.world_mut().insert_resource(ViewModels::of([view_model(MINIGUN)]));
    let perspective = |deg: f32| Projection::Perspective(PerspectiveProjection { fov: deg.to_radians(), ..default() });
    let anchor = sim
        .app
        .world_mut()
        .spawn((ViewModelAnchor, perspective(90.0), Transform::default(), Visibility::default(), ChildOf(p)))
        .id();
    sim.app.world_mut().spawn((ViewModelCamera, perspective(54.0), Transform::default(), ChildOf(anchor)));
    sim.app.world_mut().resource_mut::<TracerCount>().0 = 0;
    let (starts, stops, bullets) = collect(&mut sim, 200, |s, t| {
        s.app.world_mut().get_mut::<ViewAnimator>(p).expect("a view animator").key = Some(MINIGUN.into());
        s.intent(p).fire = t < 150;
    });
    assert!(bullets >= 8, "it fired: {bullets}");
    let named = |n: &str| starts.iter().filter(|s| s.name == n).collect::<Vec<_>>();
    assert!(starts.iter().all(|s| !s.name.starts_with("muzzle_minigun") || s.name == "muzzle_minigun_constant"), "{starts:?}");
    for n in ["muzzle_minigun_constant", "eject_minigunbrass"] {
        let s = named(n);
        assert_eq!(s.len(), 1, "{n} once: {starts:?}");
        assert_eq!(s[0].tag, Some(w), "{n} tagged with the gun");
        assert_eq!(s[0].follow.map(|f| f.entity), Some(anchor), "{n} follows the eye");
    }
    assert_eq!(stops, vec![StopParticleSystems { tag: w }], "stopped when it stopped firing");
    let tracers = starts.iter().filter(|s| s.name.starts_with("bullet_tracer01_")).count();
    assert_eq!(tracers, bullets.div_ceil(4), "a tracer every 4th bullet of {bullets}");
}

/// particles.md 8.3: each burst of firing makes a new
/// `muzzle_minigun_constant` (its clock from 0, its own 0.5 s of
/// starflash and flare), the last one stopped as it left firing.
#[test]
fn each_minigun_burst_starts_a_new_constant_system() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), MINIGUN);
    let (starts, stops, _) = collect(&mut sim, 500, |s, t| {
        s.intent(p).fire = t < 150 || (250..400).contains(&t);
    });
    let constant = starts.iter().filter(|s| s.name == "muzzle_minigun_constant").count();
    assert_eq!(constant, 2, "one per burst: {starts:?}");
    assert_eq!(stops, vec![StopParticleSystems { tag: w }; 2], "each stopped as it left firing");
}

#[test]
fn a_scattergun_blast_from_zero_draws_five_tracers() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), SCATTERGUN);
    sim.app.world_mut().resource_mut::<TracerCount>().0 = 0;
    let (starts, _, bullets) = collect(&mut sim, 3, |s, t| s.intent(p).fire = t == 0);
    assert_eq!(bullets, 10, "one blast of 10 pellets");
    let tracers: Vec<_> = starts.iter().filter(|s| s.name.starts_with("bullet_scattergun_tracer01_")).collect();
    assert_eq!(tracers.len(), 5, "{starts:?}");
    assert!(tracers.iter().all(|t| t.end.is_some() && !t.name.ends_with("_crit")));
    assert_eq!(starts.iter().filter(|s| s.name == "muzzle_scattergun").count(), 1, "one flash a shot");
}

#[test]
fn a_rocket_on_the_floor_leaves_a_scorch_and_shows_the_wall_effect() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), ROCKET_LAUNCHER);
    let mut blasts = sim.app.world().resource::<Messages<Tf2Explosion>>().get_cursor_current();
    let mut decals = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
    let (mut b, mut scorch) = (Vec::<Tf2Explosion>::new(), Vec::<PlaceDecal>::new());
    let (starts, ..) = collect(&mut sim, 60, |s, t| {
        let w = s.app.world();
        b.extend(blasts.read(w.resource::<Messages<Tf2Explosion>>()).copied());
        scorch.extend(
            decals
                .read(w.resource::<Messages<PlaceDecal>>())
                .filter(|d| matches!(&d.group, DecalGroup::Named(n) if n == "Scorch"))
                .cloned(),
        );
        s.intent(p).fire = t == 0;
        s.intent(p).pitch = -89f32.to_radians();
    });
    assert_eq!(b.len(), 1, "one blast");
    assert!(!b[0].on_player && b[0].normal.y > 0.9, "on the floor: {:?}", b[0]);
    assert_eq!(b[0].weapon, ROCKET_LAUNCHER);
    assert_eq!(scorch.len(), 1, "a scorch");
    assert!(scorch[0].point.y < b[0].at.y && scorch[0].normal.y > 0.9, "on the floor under it: {:?}", scorch[0]);
    let e: Vec<_> = starts.iter().filter(|s| s.name.starts_with("ExplosionCore")).collect();
    assert_eq!(e.len(), 1, "{starts:?}");
    assert_eq!(e[0].name, "ExplosionCore_Wall");
    assert!(e[0].forward.y > 0.9, "along the floor's normal");
    // Its trail (effects.md 1): `rockettrail` once, following the flyer
    // without carrying what it left (psys::Follow::carry).
    let trails: Vec<_> = starts.iter().filter(|s| s.name == "rockettrail").collect();
    assert_eq!(trails.len(), 1, "{starts:?}");
    let f = trails[0].follow.expect("follows the rocket");
    assert!(!f.carry);
}

/// effects.md 5 step 5, 11: the blast's sound plays with its look, on the
/// machine that draws it, when the drawn time reaches the blast (held
/// `LateDelay` after the server's blast, as the look), not when the
/// server makes it; one of each.
#[test]
fn a_rockets_blast_sounds_with_its_late_drawn_look() {
    use chimera_world::map::{GameSound, PlaySound, loose::LateDelay};
    let mut sim = sim();
    sim.app.insert_resource(LateDelay(0.1));
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), ROCKET_LAUNCHER);
    let mut blasts = sim.app.world().resource::<Messages<Tf2Explosion>>().get_cursor_current();
    let mut starts = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut game = sim.app.world().resource::<Messages<GameSound>>().get_cursor_current();
    let (mut blast, mut look, mut sound) = (Vec::new(), Vec::new(), Vec::new());
    let mut game_sounds = Vec::new();
    for t in 0..60u32 {
        sim.intent(p).fire = t == 0;
        sim.intent(p).pitch = -89f32.to_radians();
        sim.ticks(1);
        let w = sim.app.world();
        blast.extend(blasts.read(w.resource::<Messages<Tf2Explosion>>()).map(|_| t));
        look.extend(starts.read(w.resource::<Messages<StartParticleSystem>>()).filter(|s| s.name.starts_with("ExplosionCore")).map(|_| t));
        sound.extend(sounds.read(w.resource::<Messages<PlaySound>>()).filter(|s| s.entry.contains("BaseExplosionEffect")).map(|_| t));
        game_sounds.extend(game.read(w.resource::<Messages<GameSound>>()).map(|s| s.0.entry.clone()));
    }
    assert_eq!(blast.len(), 1, "one blast");
    assert_eq!(look.len(), 1, "one look");
    assert_eq!(sound, look, "the sound with the look");
    let dt = sim.app.world().resource::<chimera_core::core::SimClock>().delta.as_secs_f32();
    let held = (look[0] - blast[0]) as f32 * dt;
    assert!((0.09..0.13).contains(&held), "held the late delay: {held} s");
    assert!(!game_sounds.iter().any(|s| s.contains("BaseExplosionEffect")), "not the server's: {game_sounds:?}");
}

/// effects.md 11.2: a rocket drawn late trails from its copy
/// (`chimera_net`'s `late` makes it; one stands in here), the trail made
/// where the copy is drawn as it arrives (its first received place).
#[test]
fn a_late_rockets_trail_is_made_on_its_copy_at_its_first_place_11_2() {
    use chimera_world::map::loose::{LateCopy, ShownItem};
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), ROCKET_LAUNCHER);
    let (starts, ..) = collect(&mut sim, 2, |s, t| s.intent(p).fire = t == 0);
    let trail = starts.iter().find(|s| s.name == "rockettrail").expect("a trail").clone();
    let rocket = trail.follow.unwrap().entity;
    let key = sim.app.world().get::<ShownItem>(rocket).unwrap().0.clone();
    let first = Vec3::new(0.0, 1.5, -1.0);
    let copy = sim.app.world_mut().spawn((ShownItem(key), LateCopy { of: rocket, owner: Some(p), first }, Transform::from_translation(first))).id();
    let (starts, ..) = collect(&mut sim, 1, |_, _| {});
    let trail = starts.iter().find(|s| s.name == "rockettrail").expect("the copy's trail");
    assert_eq!(trail.follow.unwrap().entity, copy);
    assert_eq!(trail.position, first, "made at the first place");
}

/// effects.md 1: the rocket's trail starts on and follows its model's
/// `trail` attachment (the data's `at`), in the rocket's frame; a pipe's
/// on its origin (no attachment named).
#[test]
fn a_rockets_trail_rides_its_models_trail_attachment_1() {
    use chimera_world::map::loose::{LooseAsset, LooseAssets, ShownItem};
    let mut sim = sim();
    let tail = Vec3::new(-0.3, 0.02, 0.0);
    let model = |attachments: Vec<(String, Vec3)>| LooseAsset {
        frame: Transform::IDENTITY,
        half: Vec3::splat(0.1),
        surface: None,
        nose: None,
        attachments,
    };
    let mut assets = LooseAssets::default();
    assets.0.insert("tf2:tf_weapon_rocketlauncher/rocket".into(), model(vec![("trail".into(), tail)]));
    assets.0.insert("tf2:tf_weapon_grenadelauncher/grenade".into(), model(vec![("trail".into(), tail)]));
    sim.app.insert_resource(assets);
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), ROCKET_LAUNCHER);
    let (starts, ..) = collect(&mut sim, 2, |s, t| s.intent(p).fire = t == 0);
    let trail = starts.iter().find(|s| s.name == "rockettrail").expect("a trail").clone();
    let follow = trail.follow.expect("follows the rocket");
    assert_eq!(follow.local, tail, "on the `trail` attachment");
    assert!(sim.app.world().get::<ShownItem>(follow.entity).is_some(), "on the rocket");
    let (p, _) = holding(&mut sim, Vec3::new(3.0, 1.0, 0.0), GRENADE_LAUNCHER);
    let (starts, ..) = collect(&mut sim, 2, |s, t| s.intent(p).fire = t == 0);
    let pipe = starts.iter().find(|s| s.name.starts_with("pipebombtrail_")).expect("the pipe's trail");
    assert_eq!(pipe.follow.unwrap().local, Vec3::ZERO, "a pipe's from its origin");
}

/// effects.md 1: a sticky trails `stickybombtrail_<team>` from its launch
/// and pulses (`stickybomb_pulse_<team>`) once, its arm time (0.8 s)
/// after it appeared here (effects.md 11.3: the launch, on this machine),
/// both following it.
#[test]
fn a_sticky_trails_from_its_launch_and_pulses_once_when_armed() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), STICKY_LAUNCHER);
    let mut first: Vec<(String, u32)> = Vec::new();
    let mut cursor = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let (starts, ..) = collect(&mut sim, 120, |s, t| {
        // A tap: pressed one tick, let go (the launch on release).
        s.intent(p).fire = t == 0;
        let w = s.app.world();
        first.extend(cursor.read(w.resource::<Messages<StartParticleSystem>>()).map(|m| (m.name.clone(), t)));
    });
    let trail: Vec<_> = starts.iter().filter(|s| s.name.starts_with("stickybombtrail_")).collect();
    assert_eq!(trail.len(), 1, "{starts:?}");
    let pulse: Vec<_> = starts.iter().filter(|s| s.name.starts_with("stickybomb_pulse_")).collect();
    assert_eq!(pulse.len(), 1, "one pulse: {starts:?}");
    assert_eq!(pulse[0].name, trail[0].name.replace("stickybombtrail", "stickybomb_pulse"), "the trail's team");
    let follows = |s: &StartParticleSystem| s.follow.expect("follows the sticky").entity;
    assert_eq!(follows(pulse[0]), follows(trail[0]), "on the same sticky");
    let tick = |n: &str| first.iter().find(|(m, _)| m.starts_with(n)).map(|(_, t)| *t).expect(n);
    let (launch, armed) = (tick("stickybombtrail_"), tick("stickybomb_pulse_"));
    let dt = sim.app.world().resource::<chimera_core::core::SimClock>().delta.as_secs_f32();
    let after = (armed - launch) as f32 * dt;
    assert!((after - 0.8).abs() < 0.02, "pulsed 0.8 s after the launch: {after}");
}

/// effects.md 11.3 (its test rows): each machine starts a sticky's pulse
/// on its first frame the arm time L or more after it first received the
/// sticky (here: a sticky appearing at a client's 5.0 s, as a snapshot
/// brings it), not after the launch: L = 0.8 stock, 0.6 with the
/// Quickiebomb's `sticky_arm_time` −0.2.
#[test]
fn a_received_stickys_pulse_starts_its_arm_time_after_it_arrived() {
    use chimera_world::map::loose::ShownItem;
    use tf2::game::weapons::{looks::GunLooks, sticky::SHOWN};
    for (l, quickie) in [(0.8f64, false), (0.6, true)] {
        let mut sim = sim();
        if quickie {
            sim.app.world_mut().resource_mut::<GunLooks>().set_armed_after(STICKY_LAUNCHER, 0.6);
        }
        let now = |sim: &chimera_world::harness::Sim| sim.app.world().resource::<Time>().elapsed_secs_f64();
        while now(&sim) < 5.0 {
            sim.ticks(1);
        }
        // As a snapshot brings it: it is there in the next frame (a
        // client's receive runs at that frame's start).
        sim.app.world_mut().spawn((ShownItem(SHOWN.into()), Transform::from_xyz(0.0, 2.0, 0.0)));
        let mut cursor = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
        let mut started = Vec::new();
        sim.ticks(1);
        let arrived = now(&sim);
        for _ in 0..80 {
            sim.ticks(1);
            let t = now(&sim);
            let w = sim.app.world();
            started.extend(cursor.read(w.resource::<Messages<StartParticleSystem>>()).filter(|s| s.name.starts_with("stickybomb_pulse_")).map(|_| t));
        }
        assert_eq!(started.len(), 1, "once: {started:?}");
        let dt = sim.app.world().resource::<Time>().delta_secs_f64();
        let after = started[0] - arrived;
        assert!(after >= l - 1e-6 && after < l + dt + 1e-6, "the first frame at or after {l} s: {after}");
    }
}

/// particles.md Open questions 6 (measured): another player's Minigun
/// ejects its brass to his right, square to the aim and ~23° up (the
/// system's x, along which its 100-200 units/s go: 150-180 seen), born
/// 28 right of and 41 below his eye, not at the held model's attachment.
#[test]
fn others_minigun_brass_leaves_right_and_up_from_beside_the_eye() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), MINIGUN);
    for name in ["eject_brass", "muzzle"] {
        sim.app.world_mut().spawn((HeldAttachment { owner: p, index: 0, name: name.into() }, GlobalTransform::from_translation(Vec3::new(0.3, 1.2, -0.5))));
    }
    let yaw = 0.7f32;
    let (starts, ..) = collect(&mut sim, 120, |s, _| {
        let mut i = s.intent(p);
        (i.fire, i.yaw, i.pitch) = (true, yaw, 0.0);
    });
    let brass: Vec<_> = starts.iter().filter(|s| s.name == "eject_minigunbrass").collect();
    assert_eq!(brass.len(), 1, "{starts:?}");
    let b = brass[0];
    let aim = chimera_sim::weapon::aim_of(yaw, 0.0);
    let (right, fwd) = (aim * Vec3::X, aim * Vec3::NEG_Z);
    let f = b.forward.normalize();
    let up = f.y.asin().to_degrees();
    assert!((20.0..=25.0).contains(&up), "{up}° up");
    let flat = Vec3::new(f.x, 0.0, f.z).normalize();
    assert!(flat.angle_between(right).to_degrees() < 5.0, "square to the aim, to the right: {f}");
    assert!(flat.dot(fwd).abs() < 0.09);
    let w = sim.app.world();
    let eye = w.get::<Transform>(p).unwrap().translation + w.get::<chimera_core::core::MovementState>(p).unwrap().eye_offset;
    let d = (b.position - eye) / 0.0254;
    assert!((d.dot(right) - 28.0).abs() < 0.5 && (d.y + 41.0).abs() < 0.5 && d.dot(fwd).abs() < 0.5, "from the eye: {d}");
    // The muzzle's system stays at its attachment.
    let m = starts.iter().find(|s| s.name == "muzzle_minigun_constant").expect("the constant flash");
    assert!((m.position - Vec3::new(0.3, 1.2, -0.5)).length() < 1e-4, "{m:?}");
}

#[test]
fn others_see_the_backblast_and_the_shooter_never() {
    let run = |local: bool| {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), ROCKET_LAUNCHER);
        if local {
            sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
        }
        sim.app.world_mut().spawn((
            HeldAttachment { owner: p, index: 0, name: "backblast".into() },
            GlobalTransform::from_translation(Vec3::new(0.0, 1.5, 0.3)),
        ));
        let (starts, ..) = collect(&mut sim, 3, |s, t| s.intent(p).fire = t == 0);
        starts.iter().filter(|s| s.name == "rocketbackblast").count()
    };
    assert_eq!(run(false), 1, "a watcher sees it");
    assert_eq!(run(true), 0, "the shooter never does");
}

/// Another player's Minigun (whose spin this client isn't sent) runs its
/// constant systems here while its rounds arrive, and stops after.
#[test]
fn others_minigun_runs_its_constant_systems_while_its_rounds_arrive() {
    use chimera_core::core::Intent;
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use chimera_sim::weapon::{self, Inventory};
    use chimera_world::{greybox::GreyboxMapPlugin, map::psys::{StartParticleSystem, StopParticleSystems}};
    let mut sim = NetSim::new(tf2::harness::Tf2Sim, LinkConditions::default(), 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.add_client(|_| {});
    sim.add_client(|_| {});
    sim.set_frame(tf2::game::TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(600);
    let theirs = sim.character_of(0).unwrap();
    let sw = weapon::give(sim.server.app.world_mut(), theirs, MINIGUN).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(sw);
    sim.ticks(150);
    let mut starts = sim.clients[1].app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    let mut stops = sim.clients[1].app.world().resource::<Messages<StopParticleSystems>>().get_cursor_current();
    let (mut started, mut stopped, mut stopped_while_firing) = (Vec::new(), 0, false);
    for k in 0..300 {
        let me = sim.local_player(0).unwrap();
        sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = k < 150;
        sim.step();
        let w = sim.clients[1].app.world();
        started.extend(starts.read(w.resource::<Messages<StartParticleSystem>>()).map(|s| s.name.clone()));
        let n = stops.read(w.resource::<Messages<StopParticleSystems>>()).count();
        if n > 0 && k < 150 && started.iter().any(|s| s == "muzzle_minigun_constant") {
            stopped_while_firing = true;
        }
        stopped += n;
    }
    // (Headless, no gun is drawn: the brass has no attachment to start
    // at, the constant flash starts ahead of the eye.)
    assert_eq!(started.iter().filter(|s| *s == "muzzle_minigun_constant").count(), 1, "started once on the other client: {started:?}");
    assert!(!stopped_while_firing, "kept while its rounds came");
    assert!(stopped >= 1, "stopped after");
}

/// A needle's mark on the world (`projectile::FlyerMark`, the server's)
/// reaches every client (`NetMark`) and shows there as a `Marked` impact
/// on what that client's world has at the touch (effects.md, "Needles on a
/// wall"): the shooter's and another's.
#[test]
fn every_client_sees_a_needles_mark() {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use chimera_sim::weapon::{self, Inventory};
    use chimera_world::greybox::GreyboxMapPlugin;
    use tf2::game::weapons::SYRINGE_GUN;
    let mut sim = NetSim::new(tf2::harness::Tf2Sim, LinkConditions::default(), 5, 0, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.add_client(|_| {});
    sim.add_client(|_| {});
    sim.set_frame(tf2::game::TICK_INTERVAL);
    sim.until_joined(900);
    sim.until_looks_taken(600);
    let theirs = sim.character_of(0).unwrap();
    let gun = weapon::give(sim.server.app.world_mut(), theirs, SYRINGE_GUN).unwrap();
    sim.server.app.world_mut().get_mut::<Inventory>(theirs).unwrap().wanted = Some(gun);
    sim.ticks(150);
    let mut cursors: Vec<_> =
        sim.clients.iter().map(|c| c.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current()).collect();
    let mut marks = vec![Vec::new(); 2];
    for k in 0..120 {
        let me = sim.local_player(0).unwrap();
        let mut intent = sim.clients[0].app.world_mut().get_mut::<chimera_core::core::Intent>(me).unwrap();
        intent.fire = k < 2;
        intent.pitch = -80f32.to_radians();
        sim.step();
        for (i, c) in sim.clients.iter().enumerate() {
            let w = c.app.world();
            marks[i].extend(
                cursors[i]
                    .read(w.resource::<Messages<WeaponEvent>>())
                    .filter_map(|e| match e.kind {
                        WeaponEventKind::Marked { id, normal, .. } => Some((id, normal)),
                        _ => None,
                    }),
            );
        }
    }
    for (i, m) in marks.iter().enumerate() {
        assert!(!m.is_empty(), "client {i} saw the mark");
        assert!(m.iter().all(|(id, n)| *id == SYRINGE_GUN && n.y > 0.9), "client {i}: on the floor, the gun's: {m:?}");
    }
}

/// effects.md 13's placements on the greybox (floor top at y 0, the north
/// wall's face at z −40): 1 unit out along the normal of what the blast's
/// own trace met, the scorch on that hit.
#[test]
fn blasts_are_centred_one_unit_off_what_their_trace_met_13() {
    use bevy::ecs::system::RunSystemOnce;
    use chimera_world::greybox::NORTH_WALL_Z;
    use tf2::game::weapons::blast::{BlastWorld, Placement};
    const U: f32 = 0.0254;
    let mut sim = sim();
    sim.ticks(2);
    let place = |sim: &mut chimera_world::harness::Sim, f: fn(&BlastWorld) -> Placement| {
        sim.app.world_mut().run_system_once(move |w: BlastWorld| f(&w)).unwrap()
    };
    let near = |a: Vec3, b: Vec3| a.distance(b) < 0.05 * U;
    // A rocket touching the floor at the origin.
    let p = place(&mut sim, |w| w.touched(Vec3::ZERO, Vec3::Y, None));
    assert!(near(p.centre, Vec3::Y * U) && p.normal == Vec3::Y, "{p:?}");
    assert_eq!(p.scorch, Some((Vec3::ZERO, Vec3::Y)));
    // Fused pipes: 2 and 20 units above the floor snap to 1 above it, the
    // floor's normal, a scorch on the floor; 30 above stays, in the air.
    for h in [2.0, 20.0] {
        let p = sim.app.world_mut().run_system_once(move |w: BlastWorld| w.fused(Vec3::new(1.0, h * U, 2.0), None)).unwrap();
        assert!(near(p.centre, Vec3::new(1.0, U, 2.0)), "{h}: {p:?}");
        assert!(p.normal.y > 0.99, "{h}: {p:?}");
        let (at, n) = p.scorch.expect("a scorch on the floor");
        assert!(at.y.abs() < 1e-3 && n.y > 0.99, "{p:?}");
    }
    let p = place(&mut sim, |w| w.fused(Vec3::new(1.0, 30.0 * U, 2.0), None));
    assert_eq!((p.centre, p.normal, p.scorch), (Vec3::new(1.0, 30.0 * U, 2.0), Vec3::ZERO, None), "in the air");
    // A sticky on the wall, the floor 100 units below: its own centre, the
    // wall's outward normal, no scorch.
    let wall = Vec3::new(0.0, 100.0 * U, NORTH_WALL_Z + 2.0 * U);
    let p = sim.app.world_mut().run_system_once(move |w: BlastWorld| w.fused(wall, Some(Vec3::Z))).unwrap();
    assert_eq!((p.centre, p.normal, p.scorch), (wall, Vec3::Z, None), "on the wall");
    // A pipe's direct line: into the wall ahead (its centre 2 units off),
    // 1 unit off the wall with its normal; a line meeting nothing leaves
    // it in the air.
    let p = sim.app.world_mut().run_system_once(move |w: BlastWorld| w.pipe_hit(wall, Vec3::NEG_Z)).unwrap();
    assert!(near(p.centre, Vec3::new(0.0, 100.0 * U, NORTH_WALL_Z + U)) && p.normal.z > 0.99 && p.scorch.is_some(), "{p:?}");
    let open = Vec3::new(0.0, 100.0 * U, 0.0);
    let p = sim.app.world_mut().run_system_once(move |w: BlastWorld| w.pipe_hit(open, Vec3::X)).unwrap();
    assert_eq!((p.centre, p.normal, p.target, p.scorch), (open, Vec3::ZERO, None, None));
}

/// effects.md 13 end to end: a sticky stuck on the floor and detonated
/// goes off 1 unit above it with the floor's normal (the wall effect) and
/// scorches the floor under it, its owner stepped away.
#[test]
fn a_detonated_sticky_on_the_floor_uses_the_wall_effect_and_scorches_13() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), STICKY_LAUNCHER);
    let mut blasts = sim.app.world().resource::<Messages<Tf2Explosion>>().get_cursor_current();
    let mut decals = sim.app.world().resource::<Messages<PlaceDecal>>().get_cursor_current();
    let (mut b, mut scorch) = (Vec::<Tf2Explosion>::new(), Vec::<PlaceDecal>::new());
    let (starts, ..) = collect(&mut sim, 140, |s, t| {
        let w = s.app.world();
        b.extend(blasts.read(w.resource::<Messages<Tf2Explosion>>()).copied());
        scorch.extend(
            decals
                .read(w.resource::<Messages<PlaceDecal>>())
                .filter(|d| matches!(&d.group, DecalGroup::Named(n) if n == "Scorch"))
                .cloned(),
        );
        s.intent(p).fire = t == 0;
        s.intent(p).pitch = -89f32.to_radians();
        s.intent(p).secondary = t >= 100;
        // Stepped away from it first: a line starting inside his box
        // meets nothing there (13).
        if t == 90 {
            s.app.world_mut().get_mut::<Transform>(p).unwrap().translation.x += 3.0;
        }
    });
    assert_eq!(b.len(), 1, "one blast");
    assert!(b[0].normal.y > 0.99 && !b[0].on_player, "the floor's normal: {:?}", b[0]);
    assert!((b[0].at.y - 0.0254).abs() < 0.002, "1 unit above the floor: {:?}", b[0]);
    assert_eq!(scorch.len(), 1, "a scorch");
    assert!(scorch[0].point.y.abs() < 1e-3, "on the floor: {:?}", scorch[0]);
    assert!(starts.iter().any(|s| s.name == "ExplosionCore_Wall"), "{:?}", starts.iter().map(|s| &s.name).collect::<Vec<_>>());
}

/// effects.md 9, 13: a rocket on a river bed under water goes off 1 unit
/// above the bed, in the water: the water effect, along the bed's normal.
#[test]
fn a_rocket_on_a_river_bed_shows_the_water_effect_13() {
    use chimera_core::core::{MapBrush, MapWater, MapWaterVolume};
    let mut sim = sim();
    sim.app.insert_resource(MapWater(vec![MapWaterVolume {
        brush: MapBrush::from_box(Vec3::new(-10.0, -1.0, -10.0), Vec3::new(10.0, 1.27, 10.0)),
        slime: false,
    }]));
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), ROCKET_LAUNCHER);
    let (starts, ..) = collect(&mut sim, 60, |s, t| {
        s.intent(p).fire = t == 0;
        s.intent(p).pitch = -89f32.to_radians();
    });
    let e: Vec<_> = starts.iter().filter(|s| s.name.starts_with("ExplosionCore")).collect();
    assert_eq!(e.len(), 1, "{starts:?}");
    assert_eq!(e[0].name, "ExplosionCore_MidAir_underwater");
    assert!(e[0].forward.y > 0.9, "along the bed's normal");
}

/// effects.md 12: a needle into the floor marks it at once here (not
/// drawn late) and its model stays stuck there, still, with no trail,
/// until 6 s from its launch; RED's marks may ricochet, BLU's never.
#[test]
fn a_needle_stays_stuck_until_six_seconds_from_its_launch_12() {
    use chimera_sim::weapon::projectile::StuckMark;
    use chimera_world::map::loose::ShownItem;
    use tf2::game::weapons::{SYRINGE_GUN, looks::STYLE};
    assert!(STYLE.mark_ricochets(1) && !STYLE.mark_ricochets(2) && !STYLE.mark_ricochets(0), "RED's only");
    let mut sim = sim();
    let (p, _) = holding(&mut sim, Vec3::new(0.0, 1.0, 0.0), SYRINGE_GUN);
    let mut events = sim.app.world().resource::<Messages<WeaponEvent>>().get_cursor_current();
    let (mut marked, mut stuck_from, mut stuck_to) = (None, None, None);
    let mut trails_on_stuck = 0;
    let mut starts = sim.app.world().resource::<Messages<StartParticleSystem>>().get_cursor_current();
    for t in 0..500u32 {
        sim.intent(p).fire = t == 0;
        sim.intent(p).pitch = -80f32.to_radians();
        sim.ticks(1);
        let w = sim.app.world_mut();
        if marked.is_none() && events.read(w.resource::<Messages<WeaponEvent>>()).any(|e| matches!(e.kind, WeaponEventKind::Marked { .. })) {
            marked = Some(t);
        }
        let stuck: Vec<Entity> = w.query_filtered::<Entity, (With<StuckMark>, With<ShownItem>)>().iter(w).collect();
        for s in starts.read(w.resource::<Messages<StartParticleSystem>>()) {
            if s.follow.is_some_and(|f| stuck.contains(&f.entity)) {
                trails_on_stuck += 1;
            }
        }
        if !stuck.is_empty() {
            stuck_from.get_or_insert(t);
        } else if stuck_from.is_some() && stuck_to.is_none() {
            stuck_to = Some(t);
        }
    }
    let dt = tf2::game::TICK_INTERVAL as f32;
    let (marked, from, to) = (marked.expect("marked"), stuck_from.expect("stuck"), stuck_to.expect("gone"));
    assert!(from <= marked + 1, "stuck as it marks: {from} {marked}");
    assert!(((to as f32) * dt - 6.0).abs() < 0.05, "gone 6 s after the launch: {} s", to as f32 * dt);
    assert_eq!(trails_on_stuck, 0, "no trail on the stuck needle");
}

/// effects.md 12, 8c: a needle into an enemy leaves 2 blood traces behind
/// him (its 10 points) and no mark.
#[test]
fn a_needle_into_an_enemy_bleeds_two_traces_12() {
    use chimera_sim::testkit;
    use tf2::game::weapons::{SYRINGE_GUN, blood::Tf2Stains};
    let mut sim = sim();
    let from = chimera_world::greybox::SPAWNS[0];
    let (p, _) = holding(&mut sim, from, SYRINGE_GUN);
    let t = super::weapons::target(&mut sim, from, 150.0);
    sim.app.world_mut().entity_mut(t).insert(chimera_core::core::Team(2));
    sim.app.world_mut().entity_mut(p).insert(chimera_core::core::Team(1));
    let aim = sim.position(t) - Vec3::X * 6.0 * 0.0254;
    testkit::aim_at(&mut sim, p, aim);
    let mut stains = sim.app.world().resource::<Messages<Tf2Stains>>().get_cursor_current();
    let mut got = Vec::new();
    for k in 0..40u32 {
        sim.intent(p).fire = k == 0;
        sim.ticks(1);
        got.extend(stains.read(sim.app.world().resource::<Messages<Tf2Stains>>()).copied());
    }
    assert_eq!(got.len(), 1, "{got:?}");
    assert_eq!(source_engine::effects::impact_effects::bleed_traces(got[0].points).0, 2, "{got:?}");
}
