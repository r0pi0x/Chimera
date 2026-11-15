//! The M3 arsenal in the game (`game::weapons`) on `HaloSim` at 30 Hz,
//! held by a Spartan on the greybox (its surfaces stone, as a level
//! without Halo's material table): shots to kill a full Spartan
//! (health_shields.md "Shots to kill": the sniper rifle's headshot and two
//! body shots, a rocket, explosions at the centre: a frag and an unstuck
//! plasma grenade leave 30 body points, a stuck one and a rocket kill; a
//! charged plasma pistol bolt strips the shield, then one pistol headshot
//! kills), a rocket at a wall beside its shooter (self damage and the
//! push), a frag's throw and detonation tick (projectiles.md (c), released
//! on the throw's key frame), and the pickups (multiplayer_rules.md "Test
//! cases": the item clock's timeline, ammunition from a duplicate weapon,
//! a health pack and an overshield).

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{ActionRegistry, Died, Health};
use chimera_sim::weapon::{Inventory, Magazine, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{
    game::{
        movement,
        vitality::Vitality,
        weapons::{
            HaloFired, METERS_PER_WU, PISTOL, PLASMA_PISTOL, ROCKET_LAUNCHER, SNIPER_RIFLE, ASSAULT_RIFLE,
            flight::{Flight, Request},
            grenades::{GRENADE_ACTION, GrenadeData},
            pickups::{HaloItem, Item, ItemClock, NetgameEntry, Powerup, spawn_entry},
            systems::{HaloDetonation, HaloHands, HaloProjectile, Holder},
        },
    },
    harness::HaloSim,
};

use super::weapons::armed;

/// A second Spartan `metres` ahead of `p` (along -Z).
fn target(sim: &mut Sim, p: Entity, metres: f32) -> Entity {
    let at = sim.position(p) - Vec3::Z * metres;
    let t = sim.spawn_character(at, movement::ID);
    sim.ticks(2);
    t
}

/// Aim `p` at `height` of `t`'s box.
fn aim_at(sim: &mut Sim, p: Entity, t: Entity, height: f32) {
    let eye = sim.position(p) + sim.state(p).eye_offset;
    let b = *sim.app.world().get::<avian3d::prelude::ColliderAabb>(t).expect("the target's box");
    let aim = sim.position(t).with_y(b.min.y + (b.max.y - b.min.y) * height) - eye;
    let i = &mut *sim.intent(p);
    i.yaw = (-aim.x).atan2(-aim.z);
    i.pitch = (aim.y / aim.xz().length()).atan();
}

/// Shots `id` takes to kill a full Spartan 3 m ahead, aimed at `height`
/// of its box, a tap every 20 ticks.
fn shots_to_kill(id: &str, height: f32) -> u32 {
    let (mut sim, p, g) = armed(id);
    if let Some(mut m) = sim.app.world_mut().get_mut::<Magazine>(g) {
        m.clip = m.size;
        m.reserve = 20;
    }
    let t = target(&mut sim, p, 3.0);
    aim_at(&mut sim, p, t, height);
    let mut died = sim.app.world().resource::<Messages<Died>>().get_cursor_current();
    let mut fired = sim.app.world().resource::<Messages<HaloFired>>().get_cursor_current();
    let mut n = 0;
    for k in 0..400 {
        sim.intent(p).fire = k % 20 == 0;
        sim.ticks(1);
        n += fired.read(sim.app.world().resource::<Messages<HaloFired>>()).filter(|f| f.owner == p).count() as u32;
        if died.read(sim.app.world().resource::<Messages<Died>>()).any(|d| d.entity == t) {
            return n;
        }
    }
    panic!("{id}: no kill ({n} shots, health {})", sim.app.world().get::<Health>(t).unwrap().current);
}

#[test]
fn the_sniper_rifle_kills_with_a_headshot_or_two_body_shots() {
    assert_eq!(shots_to_kill(SNIPER_RIFLE, 0.93), 1);
    assert_eq!(shots_to_kill(SNIPER_RIFLE, 0.6), 2);
}

#[test]
fn a_rocket_kills_a_full_spartan() {
    assert_eq!(shots_to_kill(ROCKET_LAUNCHER, 0.5), 1);
}

/// A projectile of `p`'s owned by `owner` set to detonate where `at` is
/// (metres) on the next tick, stuck in `stuck` if any.
fn detonate_at(sim: &mut Sim, owner: Entity, proj: halo_ce::game::weapons::params::Projectile, id: &'static str, at: Vec3, stuck: Option<Entity>) {
    let proj = Arc::new(proj);
    let mut flight = Flight::new(&proj, at / METERS_PER_WU, Vec3::X, 0.0);
    flight.request = Request::Detonate;
    flight.arming = 1.0;
    flight.arming_step = 0.0;
    flight.velocity = Vec3::ZERO;
    let holder = match stuck {
        Some(t) => Holder::Stuck { target: t, offset: Vec3::ZERO, seq: 0 },
        None => Holder::None,
    };
    sim.app.world_mut().spawn(HaloProjectile {
        proj,
        id,
        owner,
        flight,
        ignored: None,
        rng: 1,
        tracer: false,
        target: None,
        k: 0,
        holder,
        consumed: false,
        ended: None,
    });
}

/// A full Spartan after an explosion of `proj` at its centre: (alive,
/// shield, body).
fn after_blast(proj: halo_ce::game::weapons::params::Projectile, stuck: bool) -> (bool, f32, f32) {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    let owner = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    let t = sim.spawn_character(greybox::SPAWNS[0] - Vec3::Z * 10.0, movement::ID);
    sim.seconds(2.0);
    let centre = sim.position(t);
    detonate_at(&mut sim, owner, proj, "test", centre, stuck.then_some(t));
    sim.ticks(3);
    let v = sim.app.world().get::<Vitality>(t).unwrap();
    (!v.dead, v.shield, v.body)
}

#[test]
fn explosions_at_the_centre_hurt_as_the_spec_tabulates() {
    let g = GrenadeData::load();
    // A frag: shield 0, body 0.4 (30 points), alive.
    let (alive, shield, body) = after_blast(g.frag.clone(), false);
    assert!(alive && shield == 0.0 && (body - 0.4).abs() < 1e-3, "frag: {alive} {shield} {body}");
    // An unstuck plasma grenade: the same.
    let (alive, shield, body) = after_blast(g.plasma.clone(), false);
    assert!(alive && shield == 0.0 && (body - 0.4).abs() < 1e-3, "plasma: {alive} {shield} {body}");
    // A stuck one (120 attached, emp, then 120 area): dead.
    let (alive, ..) = after_blast(g.plasma.clone(), true);
    assert!(!alive, "a stuck plasma grenade kills");
    // A rocket: dead.
    let (alive, ..) = after_blast(halo_ce::game::weapons::gun(ROCKET_LAUNCHER).projectile, false);
    assert!(!alive, "a rocket at the centre kills");
}

#[test]
fn a_rocket_at_a_wall_beside_its_shooter_hurts_and_pushes_him() {
    let (mut sim, p, _) = armed(ROCKET_LAUNCHER);
    // Straight down at the floor below the eye: the blast 0.6 WU under
    // the centre.
    sim.intent(p).pitch = -1.5;
    let mut det = sim.app.world().resource::<Messages<HaloDetonation>>().get_cursor_current();
    let before = sim.app.world().get::<Vitality>(p).unwrap().shield;
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    let mut up = 0.0f32;
    let mut seen = false;
    for _ in 0..5 {
        sim.ticks(1);
        seen |= det.read(sim.app.world().resource::<Messages<HaloDetonation>>()).any(|d| d.owner == p);
        up = up.max(sim.app.world().get::<chimera_core::core::Velocity>(p).unwrap().0.y);
    }
    assert!(seen, "the rocket detonated");
    let v = sim.app.world().get::<Vitality>(p).unwrap();
    assert!(v.shield < before || v.dead, "self damage: {} -> {}", before, v.shield);
    // A living biped's push: 0.035 WU a tick, straight up, at least.
    assert!(v.dead || up > 0.03 * METERS_PER_WU * 30.0, "pushed up at {up} m/s");
}

#[test]
fn a_thrown_frag_detonates_on_its_tick() {
    let (mut sim, p, _) = armed(ASSAULT_RIFLE);
    let bit = sim.app.world().resource::<ActionRegistry>().bit(GRENADE_ACTION);
    assert_ne!(bit, 0, "the grenade action is registered");
    assert_eq!(sim.app.world().get::<HaloHands>(p).unwrap().throw.counts, [4, 0]);
    let mut det = sim.app.world().resource::<Messages<HaloDetonation>>().get_cursor_current();
    let mut when = None;
    for k in 0..90u32 {
        sim.intent(p).actions = if k == 0 { bit } else { 0 };
        sim.ticks(1);
        if det.read(sim.app.world().resource::<Messages<HaloDetonation>>()).any(|d| d.owner == p) {
            when = Some(k);
            break;
        }
    }
    // Released on frame 8; thrown level from the eye 0.62 WU over a stone
    // floor it rests on its 34th update and detonates on its 49th: tick
    // 8 + 48. The player's feet and eye are the movement's (a tick's
    // rounding allowed).
    let k = when.expect("the frag detonated");
    assert!((55..=57).contains(&k), "detonated on tick {k}");
    assert_eq!(sim.app.world().get::<HaloHands>(p).unwrap().throw.counts, [3, 0]);
}

#[test]
fn a_charged_plasma_pistol_bolt_strips_the_shield_then_a_pistol_headshot_kills() {
    let (mut sim, p, _) = armed(PLASMA_PISTOL);
    let t = target(&mut sim, p, 3.0);
    aim_at(&mut sim, p, t, 0.6);
    for k in 0..25 {
        sim.intent(p).fire = k < 22;
        sim.ticks(1);
    }
    sim.ticks(20);
    let v = sim.app.world().get::<Vitality>(t).unwrap().clone();
    assert!(v.shield == 0.0 && (v.body - 1.0).abs() < 1e-6, "shield {} body {}", v.shield, v.body);
    let g = give(sim.app.world_mut(), p, PISTOL).unwrap();
    sim.seconds(2.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(g));
    aim_at(&mut sim, p, t, 0.93);
    let mut died = sim.app.world().resource::<Messages<Died>>().get_cursor_current();
    sim.intent(p).fire = true;
    let mut dead = false;
    for _ in 0..3 {
        sim.ticks(1);
        dead |= died.read(sim.app.world().resource::<Messages<Died>>()).any(|d| d.entity == t);
    }
    // Body 1 - 1.5 (37.5 points on the head, no shield): -0.5.
    let v = sim.app.world().get::<Vitality>(t).unwrap();
    assert!(dead && v.dead && (v.body + 0.5).abs() < 1e-5, "one headshot after the bolt: {}", v.body);
}

// ---------------------------------------------------------------------------
// Pickups.

fn items_of(sim: &mut Sim, entry: usize) -> usize {
    let w = sim.app.world_mut();
    let mut q = w.query::<&HaloItem>();
    q.iter(w).filter(|i| i.entry == Some(entry)).count()
}

#[test]
fn the_item_clock_makes_an_item_every_period_taken_or_not() {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.ticks(1);
    sim.app.world_mut().resource_mut::<ItemClock>().time = 0;
    // Hang 'Em High's entry 0: an assault rifle every 30 s, far from anyone.
    let entry = NetgameEntry {
        index: 0,
        at: Vec3::new(500.0, 0.0, 500.0),
        period: NetgameEntry::period_of(0, 0),
        items: vec![(100.0, "weapons\\assault rifle\\assault rifle".into())],
        levitate: false,
    };
    assert_eq!(entry.period, 900);
    spawn_entry(sim.app.world_mut(), entry);
    // Never taken: the item of game time 0 lives to 900, two stand at 900,
    // one at 901 (deleted when now - 900 > last owned).
    let at = |sim: &mut Sim, t: i64| {
        while sim.app.world().resource::<ItemClock>().time <= t {
            sim.ticks(1);
        }
        items_of(sim, 0)
    };
    assert_eq!(at(&mut sim, 0), 1, "game time 0");
    assert_eq!(at(&mut sim, 899), 1, "game time 899");
    assert_eq!(at(&mut sim, 900), 2, "game time 900");
    assert_eq!(at(&mut sim, 901), 1, "game time 901");
    // Taken at 1000 (removed): the next at 1800, not 30 s after.
    at(&mut sim, 1000);
    let w = sim.app.world_mut();
    let mut q = w.query_filtered::<Entity, With<HaloItem>>();
    for e in q.iter(w).collect::<Vec<_>>() {
        w.despawn(e);
    }
    assert_eq!(at(&mut sim, 1799), 0, "game time 1799");
    assert_eq!(at(&mut sim, 1800), 1, "game time 1800");
    // The periods by the entry's and the collection's spawn times: Blood
    // Gulch's rocket launcher (own 90, collection 120) and super shield
    // (own 0, collection 60).
    assert_eq!((NetgameEntry::period_of(90, 120), NetgameEntry::period_of(0, 60)), (2700, 1800));
}

/// An item put at `p`'s centre.
fn drop_item(sim: &mut Sim, p: Entity, item: Item) -> Entity {
    let at = sim.position(p);
    sim.app.world_mut().spawn((Transform::from_translation(at), HaloItem { item, last_owned: 1 << 30, entry: None })).id()
}

#[test]
fn walking_over_a_duplicate_weapon_takes_its_spare_rounds() {
    let (mut sim, p, g) = armed(ASSAULT_RIFLE);
    sim.app.world_mut().get_mut::<Magazine>(g).unwrap().reserve = 100;
    let i = drop_item(&mut sim, p, Item::Weapon { id: ASSAULT_RIFLE, loaded: 60, reserve: 180, age: 0.0 });
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<Magazine>(g).unwrap().reserve, 280);
    assert!(sim.app.world().get_entity(i).is_err(), "the ground weapon is gone");
    // Held 500: takes 100, the ground one keeps 80.
    sim.app.world_mut().get_mut::<Magazine>(g).unwrap().reserve = 500;
    let i = drop_item(&mut sim, p, Item::Weapon { id: ASSAULT_RIFLE, loaded: 60, reserve: 180, age: 0.0 });
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<Magazine>(g).unwrap().reserve, 600);
    let left = sim.app.world().get::<HaloItem>(i).map(|x| x.item.clone());
    assert_eq!(left, Some(Item::Weapon { id: ASSAULT_RIFLE, loaded: 60, reserve: 80, age: 0.0 }));
}

#[test]
fn a_health_pack_heals_a_hurt_body_and_an_overshield_needs_a_shield_at_most_full() {
    let (mut sim, p, _) = armed(ASSAULT_RIFLE);
    // Full body: the health pack stays.
    let pack = drop_item(&mut sim, p, Item::Powerup(Powerup::Health));
    sim.ticks(2);
    assert!(sim.app.world().get_entity(pack).is_ok());
    sim.app.world_mut().get_mut::<Vitality>(p).unwrap().body = 0.4;
    sim.ticks(2);
    assert!(sim.app.world().get_entity(pack).is_err());
    assert_eq!(sim.app.world().get::<Vitality>(p).unwrap().body, 1.0);
    // An overshield at shield 1: taken, charging up.
    let over = drop_item(&mut sim, p, Item::Powerup(Powerup::Overshield));
    sim.ticks(2);
    assert!(sim.app.world().get_entity(over).is_err());
    sim.ticks(70);
    assert!(sim.app.world().get::<Vitality>(p).unwrap().shield > 2.9);
    // Another at 3: refused, it stays.
    let again = drop_item(&mut sim, p, Item::Powerup(Powerup::Overshield));
    sim.ticks(2);
    assert!(sim.app.world().get_entity(again).is_ok());
    // A frag grenade: one more, up to 4.
    let (mut sim, p, _) = armed(ASSAULT_RIFLE);
    sim.app.world_mut().get_mut::<HaloHands>(p).unwrap().throw.counts = [3, 0];
    drop_item(&mut sim, p, Item::Grenade(0));
    drop_item(&mut sim, p, Item::Grenade(0));
    sim.ticks(2);
    assert_eq!(sim.app.world().get::<HaloHands>(p).unwrap().throw.counts, [4, 0]);
}
