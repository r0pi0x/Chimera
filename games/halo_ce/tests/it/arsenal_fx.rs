//! The whole arsenal seen and heard (`game::view`, `game::fx::shots`,
//! `game::fx::objects`; specs/halo_ce/animations.md "First person",
//! effects.md "The rest of the arsenal", "Grenades", projectiles.md 15 to
//! 17, multiplayer_rules.md 5) without the disc. The tables: the arsenal's
//! weapon events as the view's cues; the plasma pistol's charged state with
//! its jitter, its secondary fire, the grenade throw (and thrown
//! overheated, back to overheated), the needler's ammunition overlay and
//! its frame through a reload; a detonation's location frames, a shown
//! projectile's look (its projectile's, else its gun's for the first
//! trigger). The scenarios on `HaloSim` and the greybox with a small
//! library: a thrown frag is a drawn object (`ShownItem`) once out of the
//! hand and its detonation raises the explosion's sound and particles; an
//! item lying on the map has a drawn look until taken, and its taker hears
//! the pickup sound. On `NetSim`: a client sees another player's thrown
//! grenade within two ticks of the server and its detonation's sound, and
//! an item appear and disappear.

use std::{sync::Arc, time::Duration};

use bevy::prelude::*;
use blam_formats::animation::FirstPersonSlot as S;
use chimera_core::core::{ActionRegistry, Intent};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::weapon::{WeaponEventKind, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{PlaySound, loose::ShownItem, particles::Particles},
};
use halo_ce::{
    game::{
        fx::{
            ActiveFx,
            load::{FxEffect, FxEvent, FxGun, FxPart, FxPartKind, FxProjectile, HaloFx},
            objects::{HaloItemLook, blast_frames},
            shots::{look_of, trigger_projectile},
        },
        movement,
        view::{
            cue,
            load::{item_key, object_key},
            machine::{Cue, HaloView, Tick, ViewState, ammunition_frame},
        },
        weapons::{
            ASSAULT_RIFLE, NEEDLER, PLASMA_PISTOL, gun,
            grenades::{GRENADE_ACTION, GrenadeData},
            pickups::{HaloItem, Item, ItemClock, Powerup},
            systems::{HaloDetonation, HaloProjectile},
        },
    },
    harness::HaloSim,
};

use super::view::{lasts, set_of};

#[test]
fn the_arsenals_events_are_cues() {
    let a = |n: &'static str| cue(&WeaponEventKind::Action(n), None);
    assert_eq!(a("fire_charged"), Some(Cue::FireSecondary));
    assert_eq!(a("charged"), Some(Cue::Charged));
    assert_eq!(a("throw_grenade"), Some(Cue::Throw));
    assert_eq!(a("charging"), None, "the charge's start plays nothing (no disc trigger has a charging effect)");
}

/// The plasma pistol's slots (animations.md "First-person graphs").
fn pp_set() -> chimera_world::map::anim::AnimSet {
    set_of(&[
        (S::Idle, 60, 0),
        (S::Ready, 14, 0),
        (S::Fire1, 13, 0),
        (S::Fire2, 24, 0),
        (S::Overcharged, 20, 20),
        (S::OverchargedJitter, 16, 0),
        (S::ThrowGrenade, 37, 0),
        (S::ThrowOverheated, 33, 0),
        (S::Overheating, 34, 0),
        (S::Overheated, 9, 34),
    ])
}

#[test]
fn the_plasma_pistol_holds_its_charge_and_fires_the_charged_bolt() {
    let set = pp_set();
    let mut v = HaloView::new(1);
    v.tick(&set, &Tick::default());
    v.tick(&set, &Tick { cues: vec![Cue::Charged], ..Default::default() });
    assert_eq!(v.state, ViewState::Charged);
    assert_eq!(v.jitter, Some(0.0), "the jitter starts at frame 0");
    // Held: it stays charged; the jitter runs 2 (overcharge + 1) a tick.
    let held = Tick { overcharge: 0.5, ..Default::default() };
    v.tick(&set, &held);
    assert_eq!(v.jitter, Some(3.0));
    for _ in 0..60 {
        v.tick(&set, &held);
    }
    assert_eq!(v.state, ViewState::Charged, "held for ever (its loop frame is its last)");
    // The release: fire-2 from charged, 24 ticks, then idle; no jitter.
    v.tick(&set, &Tick { cues: vec![Cue::FireSecondary], ..Default::default() });
    assert_eq!((v.state, v.jitter), (ViewState::Fire2, None));
    assert_eq!(lasts(&mut v, &set), 24);
    assert_eq!(v.state, ViewState::Idle);
}

#[test]
fn a_throw_plays_and_an_overheated_one_returns_to_overheated() {
    let set = pp_set();
    let mut v = HaloView::new(1);
    v.tick(&set, &Tick { cues: vec![Cue::Throw], ..Default::default() });
    assert_eq!(v.state, ViewState::ThrowGrenade);
    assert_eq!(lasts(&mut v, &set), 37);
    assert_eq!(v.state, ViewState::Idle);
    // Overheated: throw-overheated, then overheated while the gun is.
    let hot = Tick { overheated: true, ..Default::default() };
    v.tick(&set, &Tick { cues: vec![Cue::Throw], ..hot.clone() });
    assert_eq!(v.state, ViewState::ThrowOverheated);
    for _ in 0..33 {
        v.tick(&set, &hot);
    }
    assert_eq!(v.state, ViewState::Overheated);
    // An overheating that ends on a cooled gun goes to idle (the plasma
    // pistol's rule).
    v.tick(&set, &Tick { cues: vec![Cue::Overheat], ..Default::default() });
    assert_eq!(v.state, ViewState::Overheating);
    assert_eq!(lasts(&mut v, &set), 34);
    assert_eq!(v.state, ViewState::Idle);
}

#[test]
fn the_needlers_needles_are_its_ammunition_overlay() {
    // Rounds loaded, unchanged early in a reload, then toward the count
    // the reload leaves (animations.md "The pose each drawn frame", 2).
    assert_eq!(ammunition_frame(7, None, 60, 20), 7);
    assert_eq!(ammunition_frame(7, Some((70, 70 - 40)), 60, 20), 7);
    assert_eq!(ammunition_frame(0, Some((70, 70 - 46)), 60, 20), 8);
    assert_eq!(ammunition_frame(0, Some((70, 70 - 60)), 60, 20), 20);
    assert_eq!(ammunition_frame(0, Some((70, 70 - 60)), 5, 20), 5, "no more than the rounds there are");
    let set = set_of(&[(S::Idle, 60, 0), (S::Ammunition, 21, 0)]);
    let mut v = HaloView::new(1);
    let empty = v.tick(&set, &Tick { ammunition: Some(0), ..Default::default() });
    let full = v.tick(&set, &Tick { ammunition: Some(20), ..Default::default() });
    assert_ne!(empty, full, "the overlay's frame moves the gun's needles");
    // The document's magazine (20) fits the overlay's 21 frames.
    assert_eq!(gun(NEEDLER).magazine.unwrap().rounds_loaded_maximum, 20);
}

#[test]
fn a_detonations_locations() {
    // A rocket into a wall: "" along the flight, "normal" back out of it.
    let f = blast_frames(Vec3::ZERO, Vec3::NEG_Z);
    assert!(f[""].forward.abs_diff_eq(Vec3::NEG_Z, 1e-6));
    assert!(f["normal"].forward.abs_diff_eq(Vec3::Z, 1e-6));
    assert!(f["gravity"].forward.abs_diff_eq(Vec3::NEG_Y, 1e-6));
    // A grenade at rest (falling, or no flight): "normal" up.
    let f = blast_frames(Vec3::ZERO, Vec3::NEG_Y);
    assert!(f["normal"].forward.abs_diff_eq(Vec3::Y, 1e-6));
}

#[test]
fn a_shown_projectile_looks_like_its_projectile() {
    let pp = gun(PLASMA_PISTOL);
    assert_eq!(trigger_projectile(&pp, 0).tag, pp.projectile.tag);
    assert_eq!(trigger_projectile(&pp, 1).tag, pp.secondary.as_ref().unwrap().projectile.tag);
    let mut lib = HaloFx::default();
    lib.guns.insert(pp.tag.to_ascii_lowercase(), FxGun { impacts: vec![Some(1)], ..Default::default() });
    // Only the gun's look: the bolt has it, the charged bolt has none.
    assert_eq!(look_of(&lib, &pp, trigger_projectile(&pp, 0)).map(|l| l.impacts.to_vec()), Some(vec![Some(1)]));
    assert!(look_of(&lib, &pp, trigger_projectile(&pp, 1)).is_none());
    // Its own look first.
    let charged = trigger_projectile(&pp, 1).tag.to_ascii_lowercase();
    lib.projectiles.insert(charged, FxProjectile { potential: vec![Some(2)], model: true, ..Default::default() });
    let l = look_of(&lib, &pp, trigger_projectile(&pp, 1)).unwrap();
    assert!(l.model && l.potential == [Some(2)]);
}

const EXPL: &str = "halo_ce:expl";
const PICKUP: &str = "halo_ce:pickup";

/// A library with the frag's look: its detonation effect (a sound and a
/// particle entry) and a light in flight; the health pack's pickup sound.
fn library() -> HaloFx {
    let mut lib = super::fx::library();
    let expl = FxEffect {
        path: "explosion".into(),
        locations: vec![String::new()],
        events: vec![FxEvent {
            skip: 0.0,
            delay: (0.0, 0.0),
            duration: (0.0, 0.0),
            parts: vec![FxPart {
                location: 0,
                environment: 0,
                mode: 0,
                face_down: false,
                kind: FxPartKind::Sound(EXPL.into()),
                velocity: (0.0, 0.0),
                radius_modifier: (0.0, 0.0),
            }],
            entries: vec![super::fx::entry(0, 5, 0)],
        }],
    };
    lib.effects.push(expl);
    let e = lib.effects.len() - 1;
    lib.by_path.insert("explosion".into(), e);
    let g = GrenadeData::load();
    lib.projectiles.insert(
        g.frag.tag.to_ascii_lowercase(),
        FxProjectile { detonation: Some(e), model: true, ..Default::default() },
    );
    lib.pickups.insert("powerups\\health pack".into(), PICKUP.into());
    lib
}

#[test]
fn a_thrown_frag_is_drawn_and_its_detonation_heard_and_seen() {
    let (mut sim, p, _) = super::weapons::armed(ASSAULT_RIFLE);
    sim.app.insert_resource(ActiveFx(Some(Arc::new(library()))));
    let bit = sim.app.world().resource::<ActionRegistry>().bit(GRENADE_ACTION);
    let key = object_key(&GrenadeData::load().frag.tag);
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut det = sim.app.world().resource::<Messages<HaloDetonation>>().get_cursor_current();
    let (mut drawn, mut heard, mut particles, mut detonated) = (None, 0, 0, None);
    for k in 0..90u32 {
        sim.intent(p).actions = if k == 0 { bit } else { 0 };
        let before = sim.app.world().resource::<Particles>().count();
        sim.ticks(1);
        let w = sim.app.world_mut();
        let shown = w.query::<(&HaloProjectile, &ShownItem, &Transform)>().iter(w).any(|(_, s, _)| s.0 == key);
        if shown && drawn.is_none() {
            drawn = Some(k);
        }
        let w = sim.app.world();
        heard += sounds.read(w.resource::<Messages<PlaySound>>()).filter(|s| s.entry == EXPL).count();
        if det.read(w.resource::<Messages<HaloDetonation>>()).next().is_some() {
            detonated = Some(k);
            particles += w.resource::<Particles>().count().saturating_sub(before);
        }
        if detonated.is_some_and(|d| k > d + 2) {
            break;
        }
    }
    // Released on the throw's frame 8 (the tick after the press starts
    // it): drawn from then.
    let drawn = drawn.expect("the frag drawn");
    assert!((7..=10).contains(&drawn), "drawn from tick {drawn}");
    assert!(detonated.is_some(), "it detonated");
    assert_eq!(heard, 1, "the explosion's sound");
    assert!(particles >= 5, "the explosion's particles: {particles}");
}

/// A health pack lying at `at` (as the item clock or a drop makes one).
fn health_pack(w: &mut World, at: Vec3) {
    let now = w.resource::<ItemClock>().time;
    w.spawn((Transform::from_translation(at), HaloItem { item: Item::Powerup(Powerup::Health), last_owned: now, entry: None }));
}

fn looks(w: &mut World) -> Vec<(Entity, String, Vec3)> {
    w.query::<(Entity, &HaloItemLook, &ShownItem, &Transform)>().iter(w).map(|(e, _, s, t)| (e, s.0.clone(), t.translation)).collect()
}

#[test]
fn an_item_is_seen_until_taken_and_its_taker_hears_it() {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.app.insert_resource(ActiveFx(Some(Arc::new(library()))));
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    // A hurt Spartan, so the health pack is taken.
    sim.app.world_mut().get_mut::<halo_ce::game::vitality::Vitality>(p).unwrap().body = 0.5;
    let floor = sim.position(p);
    let at = floor - Vec3::Z * 3.0 + Vec3::Y * 0.5;
    health_pack(sim.app.world_mut(), at);
    sim.ticks(3);
    let seen = looks(sim.app.world_mut());
    assert_eq!(seen.len(), 1, "one look");
    assert_eq!(seen[0].1, item_key("powerups\\health pack"));
    assert!(seen[0].2.y < at.y, "resting below its entry: {} {}", seen[0].2, at);
    // Walk onto it.
    let mut sounds = sim.app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut heard = 0;
    for _ in 0..120 {
        sim.intent(p).move_axis = Vec2::Y;
        sim.ticks(1);
        let w = sim.app.world();
        heard += sounds.read(w.resource::<Messages<PlaySound>>()).filter(|s| s.entry == PICKUP).count();
        let w = sim.app.world_mut();
        if w.query::<&HaloItem>().iter(w).count() == 0 {
            sim.ticks(2);
            break;
        }
    }
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&HaloItem>().iter(w).count(), 0, "taken");
    assert!(looks(w).is_empty(), "its look gone with it");
    assert_eq!(heard, 1, "the pickup sound for its taker");
}

fn net() -> NetSim {
    let link = LinkConditions { latency: Duration::from_millis(30), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 5, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
        app.insert_resource(ActiveFx(Some(Arc::new(library()))));
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    sim
}

/// The shown items of key `key` on a world.
fn shown(w: &mut World, key: &str) -> usize {
    w.query::<&ShownItem>().iter(w).filter(|s| s.0 == key).count()
}

#[test]
fn a_client_sees_a_thrown_grenade_and_hears_it_go_off() {
    let mut sim = net();
    let thrower = sim.character_of(0).expect("client 0's player on the server");
    give(sim.server.app.world_mut(), thrower, ASSAULT_RIFLE).unwrap();
    sim.ticks(90);
    let mine = sim.local_player(0).unwrap();
    let bit = sim.clients[0].app.world().resource::<ActionRegistry>().bit(GRENADE_ACTION);
    let key = object_key(&GrenadeData::load().frag.tag);
    let mut heard = sim.clients[1].app.world().resource::<Messages<PlaySound>>().get_cursor_current();
    let mut log: Vec<(usize, usize, usize)> = Vec::new();
    for t in 0..100 {
        sim.clients[0].app.world_mut().get_mut::<Intent>(mine).unwrap().actions = if t == 0 { bit } else { 0 };
        sim.ticks(1);
        let s = shown(sim.server.app.world_mut(), &key);
        let c = shown(sim.clients[1].app.world_mut(), &key);
        let w = sim.clients[1].app.world();
        let h = heard.read(w.resource::<Messages<PlaySound>>()).filter(|s| s.entry == EXPL).count();
        log.push((s, c, h));
    }
    let first = |f: &dyn Fn(&(usize, usize, usize)) -> bool| log.iter().position(f);
    let s0 = first(&|l| l.0 > 0).expect("the server's grenade drawn");
    let c0 = first(&|l| l.1 > 0).unwrap_or_else(|| panic!("the client draws it: {log:?}"));
    assert!(c0 <= s0 + 2 + 2, "within a couple of ticks (and the link's 30 ms): server {s0}, client {c0}");
    let s1 = log.iter().rposition(|l| l.0 > 0).unwrap();
    let c1 = log.iter().rposition(|l| l.1 > 0).unwrap();
    assert!(c1 + 1 >= s1 && c1 <= s1 + 4, "gone with the server's: {s1} {c1}");
    assert_eq!(log.iter().map(|l| l.2).sum::<usize>(), 1, "the client hears the explosion once: {log:?}");
}

#[test]
fn a_client_sees_an_item_appear_and_go() {
    let mut sim = net();
    let taker = sim.character_of(0).expect("client 0's player on the server");
    sim.ticks(30);
    let key = item_key("powerups\\health pack");
    let at = sim.server.app.world().get::<Transform>(taker).unwrap().translation - Vec3::Z * 3.0;
    sim.server.app.world_mut().get_mut::<halo_ce::game::vitality::Vitality>(taker).unwrap().body = 0.5;
    health_pack(sim.server.app.world_mut(), at);
    let mut seen = Vec::new();
    for _ in 0..20 {
        sim.ticks(1);
        seen.push(shown(sim.clients[1].app.world_mut(), &key));
    }
    assert!(seen.contains(&1), "the client sees the item: {seen:?}");
    // The server's player takes it.
    let mine = sim.local_player(0).unwrap();
    let mut gone = false;
    for _ in 0..150 {
        sim.clients[0].app.world_mut().get_mut::<Intent>(mine).unwrap().move_axis = Vec2::Y;
        sim.ticks(1);
        if shown(sim.clients[1].app.world_mut(), &key) == 0 {
            gone = true;
            break;
        }
    }
    let w = sim.server.app.world_mut();
    assert_eq!(w.query::<&HaloItem>().iter(w).count(), 0, "taken on the server");
    assert!(gone, "and gone on the client");
}

/// A free particle system (effects.md "Particle systems"): an explosion's
/// initial count spread by its constants, each particle once per state on
/// one path, the later state hidden until its time.
#[test]
fn a_particle_systems_explosion() {
    use halo_ce::game::fx::{
        load::{FxSysState, FxSysType, FxSystem},
        place::Frame,
        play::{Made, system},
    };
    let state = |additive: bool, d: f32| FxSysState {
        duration: (d, d),
        transition: (0.1, 0.1),
        rotation_rate: (0.0, 0.0),
        radius_multiplier: 1.0,
        physics: None,
        material: Some(0),
        sequence: 0,
        radius: (0.1, 0.1),
        color_lower: [1.0; 4],
        color_upper: [1.0; 4],
        additive,
        animation_rate: (0.0, 0.0),
        texture: Some(0),
        sprites: 1,
    };
    let sys = FxSystem {
        path: "explosion med".into(),
        types: vec![FxSysType { flags: 0, initial: 35, creation: 1, constants: vec![0.012, 0.08, 200.0], birth: (1.0, 1.0), rate: 0.0, states: vec![state(true, 0.2), state(false, 0.5)], radius: 1.5, type_states: Vec::new(), render_mode: 0 }],
    };
    let mut rng = chimera_world::map::ParticleRng::new(3);
    let mut made = Made::default();
    system(&sys, &Frame::facing(Vec3::ZERO, Vec3::NEG_Y), &mut rng, &mut made);
    assert_eq!(made.groups.len(), 1);
    let p = &made.groups[0].particles;
    assert_eq!(p.len(), 70, "35 particles, one per state");
    let m = halo_ce::game::METERS_PER_UNIT;
    for pair in p.chunks(2) {
        let (fire, smoke) = (&pair[0], &pair[1]);
        assert_eq!(fire.position, smoke.position, "one path");
        let o = fire.position / m;
        assert!(o.x.abs() <= 0.012 + 1e-5 && o.z.abs() <= 0.012 + 1e-5 && o.y.abs() <= 0.08 + 1e-5, "{o}");
        assert!(fire.velocity.abs_diff_eq(fire.position * 200.0, 1e-3), "moving at the spread times the intensity");
        // 0.2 s of fire, 0.1 s of transition, 0.5 s of smoke: one life.
        assert!((fire.life - 0.8).abs() < 1e-5 && (smoke.life - 0.8).abs() < 1e-5);
        let (mut f, mut s) = (fire.clone(), smoke.clone());
        f.age = 0.1;
        s.age = 0.1;
        assert_eq!((f.current_alpha(), s.current_alpha()), (1.0, 0.0), "the smoke hidden while the fire burns");
        f.age = 0.25;
        s.age = 0.25;
        assert!((f.current_alpha() - 0.5).abs() < 1e-4 && (s.current_alpha() - 0.5).abs() < 1e-4, "crossing: the fire's weight falls as the smoke's rises");
        s.age = 0.5;
        f.age = 0.5;
        assert_eq!((f.current_alpha(), s.current_alpha()), (0.0, 1.0));
    }
}

/// multiplayer_rules.md 5.4: a dead player's weapon lies where it died, as
/// an item (on a map whose pickups play).
#[test]
fn a_dead_players_weapon_is_dropped() {
    use halo_ce::game::weapons::{PISTOL, ROCKET_LAUNCHER, pickups::{NetgameEntry, spawn_entry}};
    let (mut sim, p, _) = super::weapons::armed(ROCKET_LAUNCHER);
    spawn_entry(sim.app.world_mut(), NetgameEntry { index: 0, at: Vec3::new(500.0, 0.0, 500.0), period: 900, items: Vec::new(), levitate: false });
    let at = sim.position(p) - Vec3::Z * 4.0;
    let t = sim.spawn_character(at, movement::ID);
    sim.ticks(2);
    give(sim.app.world_mut(), t, PISTOL).unwrap();
    sim.seconds(2.0);
    let mut died = sim.app.world().resource::<Messages<chimera_core::core::Died>>().get_cursor_current();
    let mut dead = false;
    for k in 0..60 {
        sim.intent(p).fire = k == 0;
        sim.ticks(1);
        dead |= died.read(sim.app.world().resource::<Messages<chimera_core::core::Died>>()).any(|d| d.entity == t);
        if dead {
            sim.ticks(3);
            break;
        }
    }
    assert!(dead, "the rocket killed");
    let w = sim.app.world_mut();
    let items: Vec<(Item, Vec3)> = w.query::<(&HaloItem, &Transform)>().iter(w).map(|(i, t)| (i.item.clone(), t.translation)).collect();
    assert!(
        items.iter().any(|(i, x)| matches!(i, Item::Weapon { id, .. } if *id == PISTOL) && x.distance(at) < 2.0),
        "the pistol lies where it died: {items:?}"
    );
}

/// effects.md "Point physics", "Particle systems": a system's particles
/// move by their state's physics at the type's radius (the frag
/// explosion's fire: friction 2000, density 0.0005, radius 1.5: a drag of
/// 7.025 a second, rising by 3.2086613 x 0.375 WU/s^2), live through the
/// states and their transitions, and an attached system makes `rate x dt`
/// particles.
#[test]
fn a_particle_systems_physics_lifetime_and_rate() {
    use blam_formats::effect::PointPhysics;
    use halo_ce::game::fx::{
        load::{FxSysState, FxSysType, FxSystem, FxTypeState},
        place::Frame,
        play::{Made, emit, system},
    };
    let physics = PointPhysics {
        flags: 0,
        mass_per_r3: 59.306675,
        water_buoyancy: 0.9990005,
        air_buoyancy: 0.375,
        density: 0.0005,
        air_friction: 2000.0,
        water_friction: 2000.0,
        surface_friction: 0.01,
        elasticity: 0.0,
    };
    let state = |d: (f32, f32), t: (f32, f32), alpha: f32| FxSysState {
        duration: d,
        transition: t,
        material: Some(0),
        sequence: 0,
        radius: (0.2, 0.4),
        color_lower: [alpha, 1.0, 1.0, 1.0],
        color_upper: [alpha, 1.0, 1.0, 1.0],
        additive: false,
        animation_rate: (0.0, 0.0),
        rotation_rate: (1.0, 2.0),
        radius_multiplier: 1.0,
        physics: Some(physics.clone()),
        texture: Some(0),
        sprites: 1,
    };
    let sys = FxSystem {
        path: "s".into(),
        types: vec![FxSysType {
            flags: 0,
            initial: 5,
            creation: 0,
            constants: Vec::new(),
            birth: (1.0, 1.0),
            rate: 30.0,
            states: vec![state((0.1, 0.1), (0.2, 0.2), 1.0), state((0.3, 0.3), (0.3, 0.3), 0.5), state((0.0, 0.0), (0.0, 0.0), 0.0)],
            radius: 1.5,
            // Full colour for 1 s, then to nothing over 4 s.
            type_states: vec![
                FxTypeState { duration: (1.0, 1.0), transition: (4.0, 4.0), color: [1.0; 4], radius_multiplier: 1.0, scale_multiplier: 1.0 },
                FxTypeState { duration: (0.0, 0.0), transition: (0.0, 0.0), color: [0.0; 4], radius_multiplier: 1.0, scale_multiplier: 1.0 },
            ],
            render_mode: 0,
        }],
    };
    let m = halo_ce::game::METERS_PER_UNIT;
    let mut rng = chimera_world::map::ParticleRng::new(5);
    let mut made = Made::default();
    system(&sys, &Frame::facing(Vec3::ZERO, Vec3::NEG_Y), &mut rng, &mut made);
    let g = &made.groups[0];
    let damping = 2000.0 / ((59.306675 + 130.47474) * 1.5);
    // Each particle carries its state's physics as its own motion keys.
    let keys = &g.particles[0].curves.as_ref().unwrap().motion;
    assert!((keys[0].1[1] - damping).abs() < 1e-3, "{:?}", keys[0]);
    assert!((keys[0].1[0] + 3.2086613 * 0.375 * m).abs() < 1e-4, "{:?}", keys[0]);
    // The visible states (the last, of alpha 0, is not drawn): fire, smoke.
    assert_eq!(g.particles.len(), 70, "5 at once and 30 for the birth second, two sprites each");
    // Life: 0.1 + 0.2 + 0.3 + 0.3 = 0.9 s; the smoke's alpha through its
    // state is its 0.5 (the type's multiplier still 1 before 1 s), then it
    // falls to 0 over the last transition.
    let smoke = &g.particles[1];
    assert!((smoke.life - 0.9).abs() < 1e-5, "{}", smoke.life);
    let mut s = smoke.clone();
    s.age = 0.45;
    assert!((s.current_alpha() - 0.5).abs() < 1e-4, "{}", s.current_alpha());
    s.age = 0.75;
    assert!((s.current_alpha() - 0.25).abs() < 1e-4, "halfway through the fade out: {}", s.current_alpha());
    assert!(s.current_alpha() < smoke.clone().current_alpha() + 1.0);
    // An attached system: its rate over a tick (30 a second: one a tick).
    let mut count = 0;
    for _ in 0..30 {
        let mut made = Made::default();
        emit(&sys, &Frame::facing(Vec3::ZERO, Vec3::NEG_Y), 1.0 / 30.0, &mut rng, &mut made);
        count += made.groups.iter().map(|g| g.particles.len() / 2).sum::<usize>();
    }
    assert_eq!(count, 30);
}

/// effects.md "Contrails": a point of a long-lived trail lives through the
/// contrail's states (the rocket's: bright and thin, then grey smoke that
/// widens and fades to nothing).
#[test]
fn a_rockets_trail_follows_its_contrails_states() {
    use chimera_world::map::particles::Particles;
    use halo_ce::game::fx::{
        load::{FxTracer, FxTracerState},
        shots::draw_segment,
    };
    let state = |duration: f32, transition: f32, width: f32, color: [f32; 4]| FxTracerState { duration, transition, width, color, duration_bounds: (duration, duration), transition_bounds: (transition, transition), scale_flags: 0 };
    let tracer = FxTracer {
        width: 0.15,
        color: [1.0, 1.0, 0.7, 0.0],
        material: Some(0),
        additive: false,
        life: 1.0,
        end_width: 0.5,
        states: vec![
            state(0.1, 0.1, 0.15, [1.0, 1.0, 0.7, 0.0]),
            state(0.5, 0.3, 0.3, [0.25, 0.4, 0.4, 0.4]),
            state(0.0, 0.0, 0.5, [0.0, 0.3, 0.3, 0.3]),
        ],
    };
    let mut particles = Particles::default();
    draw_segment(&mut particles, Some(&tracer), None, Vec3::ZERO, Vec3::X, true, true);
    let p = &particles.groups[0].particles[0];
    // 0.1 + 0.1 + 0.5 + 0.3 s, each as the updates that pass it at 30 a
    // second (4, 4, 16 and 10: the progress must pass 1 strictly): 34 / 30.
    assert!((p.life - 34.0 / 30.0).abs() < 1e-4, "{}", p.life);
    let mut q = p.clone();
    assert!((q.current_alpha() - 1.0).abs() < 1e-5 && q.size_scale() == 1.0);
    q.age = 0.4;
    assert!((q.current_alpha() - 0.25).abs() < 1e-5 && (q.size_scale() - 2.0).abs() < 1e-5, "smoke: {} {}", q.current_alpha(), q.size_scale());
    assert!(q.current_color().x < 0.5, "grey, not the flame's");
    q.age = 29.0 / 30.0;
    assert!((q.current_alpha() - 0.125).abs() < 1e-4, "halfway to nothing: {}", q.current_alpha());
}

/// effects.md "Effect lights": intensity = 1 - the fall-off function of the
/// elapsed fraction (the frag's linear, the plasma grenade's late, the
/// tank shell's early).
#[test]
fn an_effect_lights_intensity_follows_its_falloff_function() {
    use halo_ce::ui::fx_lights::intensity;
    assert!((intensity(0, 0.5) - 0.5).abs() < 1e-6);
    assert!((intensity(3, 0.5) - 0.75).abs() < 1e-6, "late: stays bright");
    assert!((intensity(1, 0.25) - 0.5).abs() < 1e-6, "early: drops fast");
    assert_eq!((intensity(0, 0.0), intensity(0, 1.0)), (1.0, 0.0));
}

/// effects.md test cases, "Particle systems and contrails (fourth errata
/// batch)": each state's own physics, blended in reverse through a
/// transition; the radius the physics uses.
#[test]
fn errata4_a_particle_states_physics() {
    use blam_formats::effect::PointPhysics;
    use halo_ce::game::fx::play::{blended_physics, physics_radius};
    let p = |friction: f32, flags: u32| PointPhysics {
        flags,
        mass_per_r3: 59.3,
        water_buoyancy: 1.0,
        air_buoyancy: 0.375,
        density: 0.0005,
        air_friction: friction,
        water_friction: 0.0,
        surface_friction: 0.0,
        elasticity: 0.0,
    };
    let (p0, p1) = (p(2000.0, 0), p(200.0, 0x18));
    // In state 0 (w = 1 outside a transition the code uses the state's as
    // stored); in transition at w = 0.75: 0.25 x 2000 + 0.75 x 200 = 650.
    let b = blended_physics(Some(&p0), Some(&p1), 0.75).unwrap();
    assert!((b.air_friction - 650.0).abs() < 1e-3, "{}", b.air_friction);
    assert_eq!(b.flags, 0, "the current state's flags");
    assert_eq!(blended_physics(Some(&p0), Some(&p1), 0.0).unwrap().air_friction, 2000.0, "the transition's end: the current state's");
    assert!((physics_radius(1.5, 1.0, 1.0, 2.0, 0.75) - 1.875).abs() < 1e-6);
}

/// Same batch: a particle system's drawn size is the scale in WU per sprite
/// pixel (a 36 pixel sprite at k = 0.02: 0.72 WU; the type's scale
/// multiplier 0.5: 0.36 WU, nothing else multiplying it); a plain
/// particle's is 2R (radius 1.5, sprite size field 1/33, 33 pixels: 3.0 WU).
#[test]
fn errata4_drawing_sizes() {
    use halo_ce::game::fx::{
        load::{FxSysState, FxSysType, FxSystem, FxTypeState},
        place::Frame,
        play::{Made, particle, system},
    };
    let m = halo_ce::game::METERS_PER_UNIT;
    // As compiled: half the sprite's pixels times k (WU).
    let state = FxSysState {
        duration: (1.0, 1.0),
        transition: (0.0, 0.0),
        rotation_rate: (0.0, 0.0),
        radius_multiplier: 1.0,
        physics: None,
        material: Some(0),
        sequence: 0,
        radius: (0.02 * 36.0 / 2.0, 0.02 * 36.0 / 2.0),
        color_lower: [1.0; 4],
        color_upper: [1.0; 4],
        additive: true,
        animation_rate: (0.0, 0.0),
        texture: Some(0),
        sprites: 1,
    };
    let ts = |scale: f32| FxTypeState { duration: (10.0, 10.0), transition: (0.0, 0.0), color: [1.0; 4], radius_multiplier: 1.0, scale_multiplier: scale };
    for (scale, quad) in [(1.0, 0.72), (0.5, 0.36)] {
        let sys = FxSystem {
            path: "s".into(),
            types: vec![FxSysType { flags: 0, initial: 1, creation: 0, constants: Vec::new(), birth: (0.0, 0.0), rate: 0.0, states: vec![state.clone()], radius: 1.5, type_states: vec![ts(scale), ts(scale)], render_mode: 0 }],
        };
        let mut made = Made::default();
        system(&sys, &Frame::facing(Vec3::ZERO, Vec3::NEG_Y), &mut chimera_world::map::ParticleRng::new(1), &mut made);
        let size = made.groups[0].particles[0].current_size();
        assert!((2.0 * size / m - quad).abs() < 1e-4, "{} WU", 2.0 * size / m);
    }
    let mut def = super::fx::particle();
    def.radius_animation = (1.0, 1.0);
    def.sprite_scale = 33.0 / 33.0;
    let mut e = super::fx::entry(0, 1, 0);
    e.radius = (1.5, 1.5);
    let p = particle(&def, &e, &Frame::facing(Vec3::ZERO, Vec3::NEG_Y), &mut chimera_world::map::ParticleRng::new(1)).unwrap();
    assert!((2.0 * p.current_size() / m - 3.0).abs() < 1e-4, "2R: {}", 2.0 * p.current_size() / m);
}

/// Same batch: a contrail point's lengths, each its own draw, quantised to
/// the updates that pass a progress of 1 strictly (the overshoot dropped);
/// a last state of 0 expires with the transition into it, one of 0.5 s
/// lives that long more.
#[test]
fn errata4_contrail_point_lengths() {
    use halo_ce::game::fx::{
        load::FxTracerState,
        shots::{point_length, point_lengths, updates_in},
    };
    let dt = 1.0 / 30.0;
    let l = point_length((0.1, 0.25), 0.5);
    assert!((l - 0.175).abs() < 1e-6);
    assert!((5.0 * dt / l - 0.952).abs() < 1e-3 && 6.0 * dt / l > 1.0);
    assert_eq!(updates_in(l, dt), 6, "it leaves state 0 on the 6th update");
    assert_eq!((point_length((0.1, 0.3), 0.0), point_length((0.1, 0.3), 1.0)), (0.1, 0.3), "each point its own draw");
    let st = |d: f32, t: f32| FxTracerState { duration_bounds: (d, d), transition_bounds: (t, t), ..Default::default() };
    let zero_last = point_lengths(&[st(0.1, 0.2), st(0.0, 0.0)], dt, || 0.0);
    assert_eq!(zero_last[1], (0.0, 0.0), "expires the update the transition into it ends");
    let half_last = point_lengths(&[st(0.1, 0.2), st(0.5, 0.0)], dt, || 0.0);
    assert!((half_last[1].0 - 0.5).abs() <= dt + 1e-6 && half_last[1].1 == 0.0, "{:?}", half_last);
}
