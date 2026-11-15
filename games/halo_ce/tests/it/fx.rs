//! Halo's weapon sounds and effects (`game::fx`) without the disc: a
//! small library made here (a firing effect with a sound and a flash, the
//! stone impact with its sound, decal and particles) on `HaloSim` and the
//! greybox. The pure pieces: an event's sound and slot, the effect's
//! timing (specs/halo_ce/effects.md's synthetic effect: particles at
//! updates 5, 7 and 9 of a 0.1 s delay and a 0.2 s constant spread), a
//! particle's fade and radius at the spec's sample ticks, the impact
//! markers and the surface classes' material types; the scenarios: the
//! assault rifle's every shot raises its fire sound, a flash and the muzzle
//! light, a tracer every fourth round (rounds between tracers 3); a shot at
//! the floor raises the stone impact's sound, particles and decal; and over
//! the network the plasma bolt exists on another client within a tick or
//! two of firing and lives as long as the server's.

use std::{collections::BTreeMap, sync::Arc, time::Duration};

use bevy::prelude::*;
use chimera_core::core::Intent;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::weapon::{WeaponEventKind, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{PlaceDecal, PlaySound, impact::SurfaceClass, particles::Particles},
};
use halo_ce::{
    game::{
        fx::{
            ActiveFx, HaloLight,
            load::{FxEffect, FxEntry, FxEvent, FxGun, FxLight, FxPart, FxPartKind, FxParticle, FxTracer, HaloFx},
            place::Frame,
            play::{Effects, Instance, Made, Running, distribution, look_at},
            shots::{HaloShown, impact_frames, material_of},
            sound::{event_slot, event_sound},
        },
        movement,
        weapons::{ASSAULT_RIFLE, HaloFired, HaloProjectile, PLASMA_RIFLE},
    },
    harness::HaloSim,
};

fn particle() -> FxParticle {
    FxParticle {
        path: "flash".into(),
        material: Some(0),
        flags: 0,
        lifespan: (0.5, 0.5),
        fade_in: 0.1,
        fade_out: 0.2,
        radius_animation: (1.0, 2.0),
        animation_rate: (0.0, 0.0),
        first_sequence: 0,
        initial: 1,
        looping: 0,
        additive: true,
        physics: None,
        orientation: 0,
        aspect: 1.0,
    }
}

pub(crate) fn entry(location: i16, count: i16, distribution: i16) -> FxEntry {
    FxEntry {
        location,
        environment: 0,
        camera_mode: 0,
        offset: [0.0; 3],
        direction: [1.0, 0.0, 0.0],
        particle: Some(0),
        flags: 0,
        distribution,
        count: (count, count),
        distribution_radius: (0.0, 0.0),
        velocity: (1.0, 1.0),
        cone: 0.0,
        angular_velocity: (0.0, 0.0),
        radius: (0.1, 0.2),
        tint_lower: [1.0; 4],
        tint_upper: [1.0, 0.0, 0.0, 0.0],
    }
}

fn part(location: i16, kind: FxPartKind, trace: f32) -> FxPart {
    FxPart { location, environment: 0, mode: 0, face_down: false, kind, velocity: (trace, trace), radius_modifier: (0.0, 0.0) }
}

fn event(delay: f32, duration: f32, parts: Vec<FxPart>, entries: Vec<FxEntry>) -> FxEvent {
    FxEvent { skip: 0.0, delay: (delay, delay), duration: (duration, duration), parts, entries }
}

const FIRE: &str = "halo_ce:fire";
const GRANHIT: &str = "halo_ce:granhit";
const STONE: &str = "halo_ce:stone";

/// The library: the assault rifle's (and the plasma rifle's) firing effect
/// (its sound and one flash), the stone impact (sound at "reflection", a
/// decal along "negative incident", three particles at "normal").
pub(crate) fn library() -> HaloFx {
    let fire = FxEffect {
        path: "fire".into(),
        locations: vec!["primary trigger".into()],
        events: vec![event(0.0, 0.0, vec![part(0, FxPartKind::Sound(FIRE.into()), 0.0)], vec![entry(0, 1, 0)])],
    };
    let stone = FxEffect {
        path: "impact stone".into(),
        locations: vec!["reflection".into(), "negative incident".into(), "normal".into()],
        events: vec![event(
            0.0,
            0.0,
            vec![part(0, FxPartKind::Sound(GRANHIT.into()), 0.0), part(1, FxPartKind::Decal(STONE.into()), 0.1)],
            vec![entry(2, 3, 0)],
        )],
    };
    let mut impacts = vec![None; 33];
    impacts[2] = Some(1);
    let gun = FxGun {
        firing: vec![Some(0), Some(0)],
        light: Some((FxLight { color: [0.961, 0.910, 0.686], radius: 2.0, ticks: 0.0, falloff: 0 }, "primary trigger".into())),
        impacts,
        tracer: Some(FxTracer { width: 0.05, color: [1.0, 0.89, 0.28, 1.0], material: Some(0), additive: true, ..Default::default() }),
        fp_markers: BTreeMap::from([("primary trigger".to_string(), [0.2, 0.0, 0.05])]),
        ..Default::default()
    };
    let mut plasma = gun.clone();
    plasma.bolt_light = Some(FxLight { color: [0.5, 1.0, 1.0], radius: 1.0, ticks: 0.0, falloff: 0 });
    HaloFx {
        effects: vec![fire, stone],
        by_path: BTreeMap::from([("fire".into(), 0), ("impact stone".into(), 1)]),
        particles: vec![particle()],
        guns: BTreeMap::from([
            ("weapons\\assault rifle\\assault rifle".into(), gun),
            ("weapons\\plasma rifle\\plasma rifle".into(), plasma),
        ]),
        ..Default::default()
    }
}

#[test]
fn an_events_sound_and_slot() {
    let mut gun = FxGun::default();
    gun.anim_sounds.insert("ready".into(), ("halo_ce:ready".into(), 3));
    gun.anim_sounds.insert("enter".into(), ("halo_ce:shell".into(), 0));
    assert_eq!(event_slot(&WeaponEventKind::Deployed, 60, false), Some("ready"));
    assert_eq!(event_slot(&WeaponEventKind::ReloadStarted, 0, false), Some("reload-empty"));
    assert_eq!(event_slot(&WeaponEventKind::ReloadStarted, 5, false), Some("reload-full"));
    assert_eq!(event_slot(&WeaponEventKind::ReloadStarted, 5, true), Some("enter"));
    assert_eq!(event_slot(&WeaponEventKind::Action("overheat_cleared"), 0, false), Some("o-h-exit"));
    assert_eq!(event_slot(&WeaponEventKind::DryFire, 0, false), None);
    assert_eq!(event_sound(&gun, "ready"), Some(("halo_ce:ready", 3)));
    assert_eq!(event_sound(&gun, "melee"), None);
}

/// The spec's synthetic effect: delay 0.1 s, duration 0.2 s, 3 particles
/// spread constantly: the parts on update 3, particles on 5, 7 and 9, the
/// effect over on 9.
#[test]
fn the_synthetic_effects_timing() {
    let mut fx = library();
    fx.effects[0].events = vec![event(0.1, 0.2, vec![part(0, FxPartKind::Sound(FIRE.into()), 0.0)], vec![entry(0, 3, 2)])];
    let mut rng = chimera_world::map::ParticleRng::new(1);
    let f = Frame::facing(Vec3::ZERO, Vec3::NEG_Z);
    let mut r = Running::new(&fx, 0, vec![vec![Instance::both(f)]], &mut rng);
    let none = |_: Vec3, _: Vec3| None;
    let mut made = Made::default();
    r.step(&fx, 0.0, &mut rng, &none, &mut made);
    let mut log = Vec::new();
    for update in 1..=10 {
        let mut made = Made::default();
        r.step(&fx, 1.0 / 30.0, &mut rng, &none, &mut made);
        let n: usize = made.groups.iter().map(|g| g.particles.len()).sum();
        log.push((update, made.sounds.len(), n, r.done));
    }
    assert_eq!(log[2], (3, 1, 0, false), "{log:?}");
    let particles: Vec<u32> = log.iter().filter(|l| l.2 > 0).map(|l| l.0).collect();
    assert_eq!(particles, [5, 7, 9], "{log:?}");
    assert!(log[8].3 && !log[7].3, "deleted at update 9: {log:?}");
    assert_eq!((distribution(4, 0.5), distribution(5, 0.5), distribution(1, 0.99)), (0.75, 0.5, 0.0));
}

/// The spec's particle: fade 0, 1/3, 1, 5/6 and radius factor 1, 1.0667,
/// 1.333, 1.667 after 0, 1, 5 and 10 ticks of a 0.5 s life (fade in 0.1,
/// out 0.2, radius 1 to 2).
#[test]
fn a_particles_fade_and_radius() {
    for (ticks, fade, radius) in [(0, 0.0, 1.0), (1, 0.3333, 1.0667), (5, 1.0, 1.3333), (10, 0.8333, 1.6667)] {
        let (a, r) = look_at(&particle(), 0.5, ticks as f32 / 30.0);
        assert!((a - fade).abs() < 1e-3 && (r - radius).abs() < 1e-3, "{ticks}: {a} {r}");
    }
}

#[test]
fn impact_markers_and_materials() {
    let m = impact_frames(Vec3::ZERO, Vec3::Y, Vec3::new(1.0, -1.0, 0.0));
    assert!(m["normal"].forward.abs_diff_eq(Vec3::Y, 1e-6));
    assert!(m["reflection"].forward.abs_diff_eq(Vec3::new(1.0, 1.0, 0.0).normalize(), 1e-6));
    assert!(m["incident"].forward.abs_diff_eq(Vec3::new(-1.0, 1.0, 0.0).normalize(), 1e-6));
    assert!(m["gravity"].forward.abs_diff_eq(Vec3::NEG_Y, 1e-6));
    assert_eq!(material_of(SurfaceClass::Concrete), 2);
    assert_eq!(material_of(SurfaceClass::Metal), 6);
    assert_eq!(material_of(SurfaceClass::Grass), 29);
}

/// A Spartan on the greybox with the library, holding `id`, looking down
/// at the floor `pitch`.
fn armed(id: &str, pitch: f32) -> (Sim, Entity) {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.app.insert_resource(ActiveFx(Some(Arc::new(library()))));
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    give(sim.app.world_mut(), p, id).unwrap();
    sim.seconds(5.0);
    sim.app.world_mut().get_mut::<Intent>(p).unwrap().pitch = pitch;
    sim.ticks(1);
    (sim, p)
}

/// What a run of ticks raised: sounds by entry, decals, lights, particles
/// added, and the shots' tracer flags.
#[derive(Default, Debug)]
struct Raised {
    sounds: BTreeMap<String, usize>,
    decals: usize,
    lights: usize,
    particles: usize,
    shots: Vec<bool>,
}

fn run(sim: &mut Sim, ticks: u32, mut each: impl FnMut(&mut Sim, u32)) -> Raised {
    let w = sim.app.world();
    let mut c_sound = w.resource::<Messages<PlaySound>>().get_cursor_current();
    let mut c_decal = w.resource::<Messages<PlaceDecal>>().get_cursor_current();
    let mut c_light = w.resource::<Messages<HaloLight>>().get_cursor_current();
    let mut c_fired = w.resource::<Messages<HaloFired>>().get_cursor_current();
    let mut out = Raised::default();
    for t in 0..ticks {
        each(sim, t);
        let before = sim.app.world().resource::<Particles>().count();
        sim.ticks(1);
        let w = sim.app.world();
        out.particles += w.resource::<Particles>().count().saturating_sub(before);
        for s in c_sound.read(w.resource::<Messages<PlaySound>>()) {
            *out.sounds.entry(s.entry.clone()).or_default() += 1;
        }
        out.decals += c_decal.read(w.resource::<Messages<PlaceDecal>>()).count();
        out.lights += c_light.read(w.resource::<Messages<HaloLight>>()).count();
        for f in c_fired.read(w.resource::<Messages<HaloFired>>()) {
            out.shots.extend(f.projectiles.iter().map(|p| p.1));
        }
    }
    out
}

#[test]
fn the_assault_rifle_raises_its_fire_sound_flash_light_and_tracers() {
    // Level: the shots fly far before they meet anything.
    let (mut sim, p) = armed(ASSAULT_RIFLE, 0.0);
    sim.app.world_mut().get_mut::<Intent>(p).unwrap().fire = true;
    let r = run(&mut sim, 20, |_, _| {});
    let shots = r.shots.len();
    assert!(shots >= 9, "{r:?}");
    assert_eq!(r.sounds.get(FIRE).copied(), Some(shots), "a fire sound a shot: {r:?}");
    assert_eq!(r.lights, shots, "a muzzle light a shot");
    assert!(r.particles >= shots, "a flash a shot: {r:?}");
    // Rounds between tracers 3: every fourth round is one.
    let tracers: Vec<usize> = r.shots.iter().enumerate().filter(|(_, t)| **t).map(|(i, _)| i).collect();
    assert!(tracers.windows(2).all(|w| w[1] - w[0] == 4), "{tracers:?}");
}

#[test]
fn a_shot_at_the_floor_raises_the_stone_impacts_sound_particles_and_decal() {
    let (mut sim, p) = armed(ASSAULT_RIFLE, -0.9);
    let r = run(&mut sim, 6, |sim, t| sim.app.world_mut().get_mut::<Intent>(p).unwrap().fire = t == 0);
    assert_eq!(r.shots.len(), 1, "{r:?}");
    assert_eq!(r.sounds.get(GRANHIT).copied(), Some(1), "{r:?}");
    assert_eq!(r.decals, 1, "{r:?}");
    // The flash and the impact's three.
    assert!(r.particles >= 4, "{r:?}");
}

/// Over the network: a plasma bolt client 0 fires is seen by client 1
/// (`HaloShown`) within two ticks of the server's projectile and for as
/// long (to a tick).
#[test]
fn the_plasma_bolt_exists_on_another_client() {
    let link = LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 5, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    let shooter = sim.character_of(0).expect("client 0's player on the server");
    give(sim.server.app.world_mut(), shooter, PLASMA_RIFLE).unwrap();
    sim.ticks(90);
    let mine = sim.local_player(0).unwrap();
    let count = |w: &mut World, server: bool| {
        if server {
            w.query::<&HaloProjectile>().iter(w).count()
        } else {
            w.query::<&HaloShown>().iter(w).count()
        }
    };
    let mut log: Vec<(usize, usize)> = Vec::new();
    for t in 0..90 {
        sim.clients[0].app.world_mut().get_mut::<Intent>(mine).unwrap().fire = t == 0;
        sim.ticks(1);
        log.push((count(sim.server.app.world_mut(), true), count(sim.clients[1].app.world_mut(), false)));
    }
    let span = |i: usize| {
        let on: Vec<usize> = log.iter().enumerate().filter(|(_, c)| if i == 0 { c.0 > 0 } else { c.1 > 0 }).map(|(t, _)| t).collect();
        (*on.first().unwrap_or(&usize::MAX), *on.last().unwrap_or(&0))
    };
    let (s0, s1) = span(0);
    let (c0, c1) = span(1);
    assert!(s0 < 90 && c0 < 90, "both see a bolt: {log:?}");
    assert!(c0 <= s0 + 2, "the client's within two ticks of the server's: {s0} {c0}");
    assert!((c1 as i64 - c0 as i64 - (s1 as i64 - s0 as i64)).abs() <= 1, "as long: server {s0}..{s1}, client {c0}..{c1}");
    let _ = Effects::default;
}
