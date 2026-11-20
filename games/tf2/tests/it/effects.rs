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
//! - the sticky (1): its trail as it appears, its armed pulse
//!   (`stickybomb_pulse_red`) once, when it arms 0.8 s later.
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
use tf2::game::weapons::{MINIGUN, ROCKET_LAUNCHER, SCATTERGUN, STICKY_LAUNCHER, blast::Tf2Explosion, looks::TracerCount};

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

/// effects.md 1: a sticky trails `stickybombtrail_<team>` from its launch
/// and pulses (`stickybomb_pulse_<team>`) once, when it arms (0.8 s), both
/// following it.
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
    assert!((after - 0.8).abs() < 0.05, "armed 0.8 s after the launch: {after}");
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
