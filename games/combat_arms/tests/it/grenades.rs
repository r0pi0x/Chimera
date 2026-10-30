//! specs/combat_arms/grenades.md's test cases, one test each (`t01_` is
//! test case 1), on `CaSim` with the game's grenades
//! (data/weapons/m67.toml and the others): an open world of our own (a
//! flat floor at y = 0 of hardness 0.65, walls, sky and water where a
//! case asks), and Combat Arms players (100 HP, armour points 0, no
//! gear) where a case has victims. Server frames are the spec's 20 Hz
//! (`CaServerFrames`); "after step n" reads the grenade (`CaGrenade`, world
//! units) after its nth physics step.
//!
//! The spec's +Z forward is ours too (the flight doesn't care which way it
//! goes); world units are the data's (`inventory::UNIT`). Tolerances: the spec's ±0.05 u and u/s for
//! flight, 0.01 HP for damage, 1e-3 s for flash times and 1e-4 for the
//! screen's whiteness; "about" (over-time damage) ±1 HP. Times: a tick
//! (15 ms) beyond the server frame the spec names.
//!
//! Cases 6-8 are the bounce rule itself (`grenades::bounce`), the velocity
//! at contact being their premise. Case 25's bot and name tag: nothing in
//! the world blocks sight (no `SightBlocker`), which is what both read.
//! The last test is the network's: a thrown grenade seen and damaging on
//! both clients.

use std::time::Duration;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{
    LocalPlayer, Blinded, Damage, DamageKind, Damageable, FriendlyFire, Health, Hitgroup, Intent, MovementState,
    PLAYER_CLIP_LAYER, PropSurface, RoundRestarts, SightBlocker, TRACE_ONLY_LAYER, Team,
};
use chimera_net::{harness::NetSim, net::memory::LinkConditions, net::predict::NetGraph};
use chimera_sim::weapon::{
    self, Inventory, Weapon,
    data::WeaponDataAppExt,
    grenade::{Detonated, GrenadeKind, Throwable},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::loose::ShownItem};
use lithtech_formats::attributes::Document;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        grenades::{
            self, CaFlashProtection, CaFlashed, CaGrenade, CaServerFrames, G7A, M18, M23F, M67, XM84, bounce,
            character_touches, flash_white, throw_perturb,
        },
        effects::{
            self, CaFxRequest, FxKind,
            data::{CaFxTables, CaWeaponFx, EFFECTS},
            rules,
            sounds::CaSound,
        },
        grenades::{callout, tumble},
        view_model::CaViewKey,
        movement,
        volumes::CaVolume,
        weapons::{DEFAULT_BODY_GEAR, MALE, body_geometry, gear},
    },
    harness::CaSim,
};

/// Metres per world unit (the data's).
const U: f32 = combat_arms::game::inventory::UNIT;
/// Health per hit point.
const HP: f32 = 0.01;

/// The M67 with no spread (case 26: the throw's direction exactly the
/// view's).
const M67_EXACT: &str = r#"
id = "test:m67_exact"
base = "combat_arms:m67"
[hooks."combat_arms:grenade"]
perturb = [0, 0]
"#;

/// Detonations seen: (server frame, weapon, kind, where in u).
#[derive(Resource, Default)]
struct Booms(Vec<(u64, &'static str, GrenadeKind, Vec3)>);

fn log_booms(mut d: MessageReader<Detonated>, frames: Res<CaServerFrames>, mut log: ResMut<Booms>) {
    for d in d.read() {
        log.0.push((frames.frame, d.weapon, d.kind, d.at / U));
    }
}

/// The explosion looks and sounds the effects made (`game::effects`).
#[derive(Resource, Default)]
struct Looks(Vec<CaFxRequest>);

#[derive(Resource, Default)]
struct Heard(Vec<CaSound>);

/// An open world: a floor at y = 0 (the grenade surfaces' default
/// hardness), nothing else.
fn sim() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.init_resource::<Booms>()
            .init_resource::<Looks>()
            .init_resource::<Heard>()
            .add_systems(FixedPostUpdate, log_booms)
            .add_systems(
                Update,
                (
                    |mut r: MessageReader<CaFxRequest>, mut l: ResMut<Looks>| l.0.extend(r.read().cloned()),
                    |mut r: MessageReader<CaSound>, mut h: ResMut<Heard>| h.0.extend(r.read().cloned()),
                )
                    .after(effects::CaEffectsSet),
            );
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.add_weapon_data(&[("m67_exact", M67_EXACT)]).unwrap();
    solid(&mut sim, Vec3::new(-200.0, -1.0, -200.0), Vec3::new(200.0, 0.0, 200.0), None);
    sim.ticks(1);
    sim
}

/// A static box from `min` to `max` (m), with a surface name.
fn solid(sim: &mut Sim, min: Vec3, max: Vec3, surface: Option<&str>) -> Entity {
    let size = max - min;
    let mut e = sim.app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(size.x, size.y, size.z),
        Transform::from_translation((min + max) / 2.0),
    ));
    if let Some(s) = surface {
        e.insert(PropSurface(s.into()));
    }
    let e = e.id();
    sim.ticks(1);
    e
}

/// A static box only some things meet, as a Combat Arms map's collision
/// sets are (`layer`: the traces-only or the players-only layer).
fn layered(sim: &mut Sim, min: Vec3, max: Vec3, layer: avian3d::prelude::LayerMask) -> Entity {
    let e = solid(sim, min, max, None);
    sim.app
        .world_mut()
        .entity_mut(e)
        .insert(CollisionLayers::new(layer, LayerMask::NONE));
    sim.ticks(1);
    e
}

/// A weapon of `id` to make grenades from, owned by `owner` (a placeholder
/// when none).
fn weapon_of(sim: &mut Sim, id: &str, owner: Option<Entity>) -> Entity {
    weapon::spawn_weapon(sim.app.world_mut(), id, owner.unwrap_or(Entity::PLACEHOLDER)).unwrap()
}

/// A live grenade of `id` at `at` (u) moving at `v` (u/s), thrown by
/// `thrower`.
fn grenade(sim: &mut Sim, id: &str, thrower: Option<Entity>, at: Vec3, v: Vec3) -> Entity {
    let w = weapon_of(sim, id, thrower);
    grenades::spawn_grenade(sim.app.world_mut(), w, thrower, at * U, v * U).unwrap()
}

fn state(sim: &Sim, g: Entity) -> CaGrenade {
    sim.app.world().get::<CaGrenade>(g).expect("grenade gone").clone()
}

/// Run until `g` has run `n` physics steps.
fn to_step(sim: &mut Sim, g: Entity, n: u32) -> CaGrenade {
    for _ in 0..2000 {
        let s = state(sim, g);
        if s.steps >= n {
            assert_eq!(s.steps, n, "stepped past {n}");
            return s;
        }
        sim.ticks(1);
    }
    panic!("no step {n}");
}

fn near(a: Vec3, b: Vec3, eps: f32) -> bool {
    (a - b).abs().max_element() <= eps
}

#[track_caller]
fn assert_near(a: Vec3, b: Vec3, eps: f32, what: &str) {
    assert!(near(a, b, eps), "{what}: {a} vs {b}");
}

fn booms(sim: &Sim) -> Vec<(u64, &'static str, GrenadeKind, Vec3)> {
    sim.app.world().resource::<Booms>().0.clone()
}

/// Run until something detonates (or `limit` seconds).
fn until_boom(sim: &mut Sim, limit: f64) -> (u64, &'static str, GrenadeKind, Vec3) {
    let start = booms(sim).len();
    for _ in 0..((limit / TICK_INTERVAL) as u64 + 2) {
        sim.ticks(1);
        if let Some(b) = booms(sim).get(start) {
            return *b;
        }
    }
    panic!("no detonation in {limit} s");
}

// --- Players

/// A Combat Arms player standing at `x`, `z` (u) facing `yaw`, 100 HP,
/// no armour, on `team`.
fn player(sim: &mut Sim, x: f32, z: f32, yaw: f32, team: u8) -> Entity {
    let p = sim.spawn_character(Vec3::new(x * U, 1.0, z * U), movement::ID);
    sim.intent(p).yaw = yaw;
    sim.app.world_mut().entity_mut(p).insert(Team(team));
    sim.seconds(0.5);
    sim.app.world_mut().entity_mut(p).insert((
        Health { current: 1.0, max: 1.0 },
        combat_arms::game::weapons::gear_armour(0.0, &Vec::new()),
    ));
    p
}

/// A player's centre and head top (m), as blasts see them.
fn body(sim: &mut Sim, p: Entity) -> (Vec3, f32) {
    let world = sim.app.world_mut();
    let mut q = world.query::<(&Transform, &Intent, &MovementState, Option<&ColliderAabb>)>();
    let (t, i, s, a) = q.get(world, p).unwrap();
    let (min, max, spheres) = body_geometry(t, i, s, a, *MALE).unwrap();
    let top = MALE
        .iter()
        .zip(&spheres)
        .filter(|(s, _)| s.location == combat_arms::game::weapons::Location::Head)
        .map(|(_, (c, r))| c.y + r)
        .fold(f32::MIN, f32::max);
    ((min + max) / 2.0, top)
}

fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current / HP
}

/// A grenade of `id` that goes off at `at` (m) on the next server frame
/// but one, without moving (its fuse set to 0).
fn explode_at(sim: &mut Sim, id: &str, thrower: Option<Entity>, at: Vec3) {
    let g = grenade(sim, id, thrower, at / U, Vec3::ZERO);
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 0.0;
    until_boom(sim, 1.0);
    // Damage lands on the tick it is dealt; flash timers run on.
    sim.ticks(1);
}

/// The point `d` u from the victim's centre in the direction it faces
/// (`ahead`) or behind it, at its centre's height.
fn in_front(sim: &mut Sim, p: Entity, d: f32, ahead: bool) -> Vec3 {
    let (c, _) = body(sim, p);
    let yaw = sim.app.world().get::<Intent>(p).unwrap().yaw;
    let f = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    c + f * d * U * if ahead { 1.0 } else { -1.0 }
}

/// A thrower out of every blast's reach, on `team`.
fn far_thrower(sim: &mut Sim, team: u8) -> Entity {
    player(sim, 5000.0, 5000.0, 0.0, team)
}

// ---------------------------------------------------------------------------
// Flight (1-8)

#[test]
fn t01_level_throw() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let s = to_step(&mut sim, g, 1);
    assert_near(s.pos, Vec3::new(0.0, 59.0, 77.5), 0.05, "step 1");
    assert_near(s.velocity, Vec3::new(0.0, -80.0, 1550.0), 0.05, "step 1 v");
    let s = to_step(&mut sim, g, 5);
    assert_near(s.pos, Vec3::new(0.0, 15.0, 387.5), 0.05, "step 5");
    let s = to_step(&mut sim, g, 6);
    let c = s.contacts[0];
    assert_eq!(c.step, 6);
    assert_near(c.at, Vec3::new(0.0, 0.0, 442.86), 0.05, "contact");
    assert_near(c.velocity, Vec3::new(0.0, 311.94, 1007.5), 0.05, "bounce");
}

#[test]
fn t02_same_throw_later() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let s = to_step(&mut sim, g, 49);
    let floors: Vec<(u32, f32)> = s.contacts.iter().map(|c| (c.step, c.at.z)).collect();
    let want = [
        (6, 442.86),
        (15, 859.68),
        (23, 1092.65),
        (30, 1232.09),
        (36, 1309.53),
        (41, 1354.01),
        (46, 1377.87),
    ];
    assert_eq!(floors.len(), want.len(), "{floors:?}");
    for ((step, z), (ws, wz)) in floors.iter().zip(want) {
        assert_eq!(*step, ws, "{floors:?}");
        assert!((z - wz).abs() <= 0.05, "step {step}: z {z} vs {wz}");
    }
    assert!(s.contacts.iter().all(|c| !c.rest) && !s.resting, "never at rest");
    let created = s.created.unwrap();
    let (at_frame, weapon, kind, at) = until_boom(&mut sim, 1.0);
    assert_eq!((weapon, kind), (M67, GrenadeKind::Blast));
    assert_eq!(at_frame - created, 50, "the start of step 50");
    assert_near(at, Vec3::new(0.0, 9.95, 1389.27), 0.05, "where");
}

#[test]
fn t03_dropped_straight_down() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::ZERO);
    for (n, y) in [(1, 59.0), (2, 54.0), (3, 45.0), (4, 32.0), (5, 15.0)] {
        let s = to_step(&mut sim, g, n);
        assert_near(s.pos, Vec3::new(0.0, y, 0.0), 0.05, &format!("step {n}"));
    }
    let s = to_step(&mut sim, g, 6);
    let c = s.contacts[0];
    assert!(c.rest && s.resting && s.velocity == Vec3::ZERO);
    assert_near(c.at, Vec3::ZERO, 0.05, "contact");
    // Then only the one-time nudge (half-height 6 x 0.65, up: case 43).
    let created = s.created.unwrap();
    sim.seconds(1.0);
    let s = state(&sim, g);
    assert_eq!(s.steps, 6, "no more steps at rest");
    assert_near(s.pos, Vec3::new(0.0, 3.9, 0.0), 0.05, "nudged");
    let (f, ..) = until_boom(&mut sim, 2.0);
    assert_eq!(f - created, 50, "at 2.5 s");
}

#[test]
fn t04_point_blank_wall() {
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, -1.0, 50.0 * U), Vec3::new(10.0, 10.0, 2.0), None);
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let s = to_step(&mut sim, g, 1);
    let c = s.contacts[0];
    assert_near(c.at, Vec3::new(0.0, 59.35, 50.0), 0.05, "wall");
    assert_near(c.velocity, Vec3::new(0.0, -52.0, -1007.44), 0.05, "off the wall");
    let s = to_step(&mut sim, g, 7);
    let c = s.contacts[1];
    assert_eq!(c.step, 7);
    assert!((c.at.z - -204.75).abs() <= 0.05 && c.at.y.abs() <= 0.05, "{c:?}");
    let s = to_step(&mut sim, g, 48);
    let last = s.contacts.last().unwrap();
    assert!(last.rest && last.step == 48, "{last:?}");
    assert_near(last.at, Vec3::new(0.0, 0.0, -852.27), 0.05, "rest");
}

#[test]
fn t05_45_degree_throw() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 1096.02, 1096.02));
    let s = to_step(&mut sim, g, 1);
    assert_near(s.pos, Vec3::new(0.0, 113.8, 54.8), 0.05, "step 1");
    let s = to_step(&mut sim, g, 10);
    assert_near(s.pos, Vec3::new(0.0, 418.01, 548.01), 0.05, "step 10");
    let s = to_step(&mut sim, g, 29);
    let c = s.contacts[0];
    assert_eq!(c.step, 29);
    assert!((c.at.z - 1585.67).abs() <= 0.05, "{c:?}");
    assert_near(c.velocity, Vec3::new(0.0, 795.52, 712.41), 0.05, "bounce");
}

#[test]
fn t06_rest_threshold() {
    let (v, rest) = bounce(Vec3::new(76.0, -480.0, 0.0), Vec3::Y, 0.65);
    assert!(rest && v == Vec3::ZERO, "49.4 < 50");
    let (v, rest) = bounce(Vec3::new(78.0, -480.0, 0.0), Vec3::Y, 0.65);
    assert!(!rest);
    assert_near(v, Vec3::new(50.7, 311.94, 0.0), 0.05, "bounces");
}

/// 6, step 4: the bounce sound's loudness min(1, |v''| h / 200), v'' the
/// damped velocity before the rest test (a resting bounce still sounds).
#[test]
fn bounce_sound_loudness() {
    use grenades::bounce_loudness;
    assert_eq!(bounce_loudness(Vec3::new(76.0, -480.0, 0.0), Vec3::Y, 0.65), 1.0, "loud, though it rests");
    let sand = grenades::CaGrenadeSurfaces::built_in().hardness(Some("desert sand"));
    // |v''| = |(60, 47.99, 0)| = 76.83: × 0.1 / 200.
    let l = bounce_loudness(Vec3::new(600.0, -480.0, 0.0), Vec3::Y, sand);
    assert!((l - 0.038416).abs() < 1e-4, "{l}");
    // A slow drop on stone: |v''| = 0.65 × 99.9, × 0.65 / 200.
    let l = bounce_loudness(Vec3::new(0.0, -100.0, 0.0), Vec3::Y, 0.65);
    assert!((l - 0.65 * 99.9 * 0.65 / 200.0).abs() < 1e-4, "{l}");
}

#[test]
fn t07_no_rest_on_walls() {
    let (v, rest) = bounce(Vec3::new(-20.0, -30.0, 0.0), Vec3::X, 0.65);
    assert!(!rest);
    assert_near(v, Vec3::new(12.94, -19.5, 0.0), 0.05, "wall");
}

#[test]
fn t08_soft_surface() {
    let sand = grenades::CaGrenadeSurfaces::built_in().hardness(Some("desert sand"));
    let (v, rest) = bounce(Vec3::new(600.0, -480.0, 0.0), Vec3::Y, sand);
    assert!(!rest);
    assert_near(v, Vec3::new(60.0, 47.99, 0.0), 0.05, "sand");
    // And on a sand floor in the world: a tenth of the speed kept.
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, -0.5, -10.0), Vec3::new(10.0, 0.001, 10.0), Some("desert sand"));
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(600.0, 0.0, 0.0));
    let s = to_step(&mut sim, g, 6);
    let c = s.contacts[0];
    assert!((c.velocity.x - 60.0).abs() <= 0.05, "{c:?}");
}

// ---------------------------------------------------------------------------
// Blasts (9-14)

#[test]
fn t09_frag_close() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, victim, 100.0, true);
    explode_at(&mut sim, M67, Some(thrower), at);
    assert_eq!(health(&sim, victim), 0.0, "120 >= 99: dead");
    // The thrower itself.
    let mut sim = self::sim();
    let thrower = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, thrower, 100.0, true);
    explode_at(&mut sim, M67, Some(thrower), at);
    assert_eq!(health(&sim, thrower), 0.0, "the thrower dies");
}

#[test]
fn t10_frag_mid() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, victim, 300.0, true);
    explode_at(&mut sim, M67, Some(thrower), at);
    assert!((100.0 - health(&sim, victim) - 53.33).abs() < 0.01, "{}", health(&sim, victim));
}

#[test]
fn t11_frag_edge() {
    for d in [450.0, 460.0] {
        let mut sim = sim();
        let thrower = far_thrower(&mut sim, 0);
        let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
        let at = in_front(&mut sim, victim, d, true);
        explode_at(&mut sim, M67, Some(thrower), at);
        assert_eq!(health(&sim, victim), 100.0, "{d} u");
    }
}

#[test]
fn t12_frag_cover() {
    // Floor to ceiling: nothing.
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, victim, 100.0, true);
    let mid = (at + body(&mut sim, victim).0) / 2.0;
    solid(&mut sim, Vec3::new(-2.0, 0.0, mid.z - 0.05), Vec3::new(2.0, 4.0, mid.z + 0.05), None);
    explode_at(&mut sim, M67, Some(thrower), at);
    assert_eq!(health(&sim, victim), 100.0, "covered");
    // Waist high: the centre hidden, the head top seen.
    let mut sim = self::sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, victim, 100.0, true);
    let (centre, top) = body(&mut sim, victim);
    let mid = (at + centre) / 2.0;
    let wall_top = centre.y + 0.25 * (top - centre.y);
    solid(&mut sim, Vec3::new(-2.0, 0.0, mid.z - 0.05), Vec3::new(2.0, wall_top, mid.z + 0.05), None);
    explode_at(&mut sim, M67, Some(thrower), at);
    assert_eq!(health(&sim, victim), 0.0, "120: dead");
}

#[test]
fn t13_frag_vs_armour() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    sim.app.world_mut().entity_mut(victim).insert(combat_arms::game::weapons::gear_armour(100.0, &vec![DEFAULT_BODY_GEAR]));
    let at = in_front(&mut sim, victim, 300.0, true);
    explode_at(&mut sim, M67, Some(thrower), at);
    assert!((health(&sim, victim) - 46.67).abs() < 0.01, "{}", health(&sim, victim));
    assert_eq!(sim.app.world().get::<chimera_sim::weapon::Armor>(victim).map(combat_arms::game::weapons::points).unwrap(), 100.0);
}

#[test]
fn t14_friendly_fire() {
    for (ff, want) in [(0, 100.0), (1, 0.0)] {
        let mut sim = sim();
        sim.app.insert_resource(FriendlyFire(ff));
        let thrower = far_thrower(&mut sim, 1);
        let mate = player(&mut sim, 0.0, 0.0, 0.0, 1);
        let at = in_front(&mut sim, mate, 100.0, true);
        explode_at(&mut sim, M67, Some(thrower), at);
        assert_eq!(health(&sim, mate), want, "friendly fire {ff}");
    }
}

// ---------------------------------------------------------------------------
// Flashes (15-23)

/// Flash `victim` with an XM84 at `d` u ahead of or behind it, thrown by
/// `thrower`; run 0.5 s. The flash it got: (time T, delay from the blast).
fn flashed(sim: &mut Sim, victim: Entity, thrower: Entity, d: f32, ahead: bool) -> Option<(f32, f64)> {
    sim.app.world_mut().entity_mut(victim).remove::<CaFlashed>();
    let at = in_front(sim, victim, d, ahead);
    let g = grenade(sim, XM84, Some(thrower), at / U, Vec3::ZERO);
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 0.0;
    until_boom(sim, 1.0);
    let blast = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.seconds(0.5);
    sim.app.world().get::<CaFlashed>(victim).map(|f| (f.time, f.at - blast))
}

#[test]
fn t15_flash_facing_close() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let (time, delay) = flashed(&mut sim, victim, thrower, 200.0, true).expect("flashed");
    assert!((time - 25.0).abs() < 1e-3, "{time}");
    // On the server frame at or after 0.25 s (ticks of 15 ms on frames of
    // 50 ms: a tick either way).
    assert!((0.25 - 0.016..0.25 + 0.05 + 0.016).contains(&delay), "sent {delay} s after");
    // The client's screen.
    for (t, w) in [(0.0, 1.0), (6.25, 0.7071), (8.333, 0.5), (12.5, 0.0)] {
        assert!((flash_white(t, time) - w).abs() < 1e-3, "w({t})");
    }
    let b = *sim.app.world().get::<Blinded>(victim).unwrap();
    assert!((b.end - b.fade_start - 12.5).abs() < 1e-3, "gone at 12.5 s");
    assert!((grenades::blinded_white(&b, b.fade_start + 6.25) - 0.7071).abs() < 1e-3);
    assert_eq!(health(&sim, victim), 100.0, "22: no damage");
}

#[test]
fn t16_flash_facing_mid() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let (time, _) = flashed(&mut sim, victim, thrower, 625.0, true).expect("flashed");
    assert!((time - 12.5).abs() < 1e-3, "{time}");
    assert!((flash_white(3.125, time) - 0.7071).abs() < 1e-4);
    assert_eq!(flash_white(6.25, time), 0.0);
    assert_eq!(health(&sim, victim), 100.0, "22: no damage");
}

#[test]
fn t17_flash_facing_away() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let (time, delay) = flashed(&mut sim, victim, thrower, 200.0, false).expect("flashed");
    assert!((time - 6.25).abs() < 1e-3, "{time}");
    assert!((0.0833 - 0.016..0.0833 + 0.05 + 0.016).contains(&delay), "sent {delay} s after");
    assert!((flash_white(1.5625, time) - 0.7071).abs() < 1e-4);
    assert_eq!(flash_white(3.125, time), 0.0);
    assert_eq!(health(&sim, victim), 100.0, "22: no damage");
}

#[test]
fn t18_flash_gear() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    sim.app.world_mut().entity_mut(victim).insert(CaFlashProtection(1.0));
    assert_eq!(flashed(&mut sim, victim, thrower, 200.0, true), None, "full goggles");
    sim.app.world_mut().entity_mut(victim).insert(CaFlashProtection(0.3));
    let (time, _) = flashed(&mut sim, victim, thrower, 200.0, true).expect("flashed");
    assert!((time - 17.5).abs() < 1e-3, "{time}");
    assert_eq!(flash_white(8.75, time), 0.0, "ends at 8.75 s");
    assert_eq!(health(&sim, victim), 100.0, "22: no damage");
}

#[test]
fn t19_flash_range() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let (time, _) = flashed(&mut sim, victim, thrower, 990.0, true).expect("flashed");
    assert!((time - 0.3333).abs() < 1e-3, "{time}");
    assert_eq!(flash_white(0.1667, time), 0.0, "ends at 0.167 s");
    assert_eq!(flashed(&mut sim, victim, thrower, 998.0, true), None, "0.067 s: not sent");
    assert_eq!(flashed(&mut sim, victim, thrower, 1000.0, true), None);
    assert_eq!(health(&sim, victim), 100.0, "22: no damage");
}

#[test]
fn t20_flash_cover() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, victim, 200.0, true);
    let mid = (at + body(&mut sim, victim).0) / 2.0;
    solid(&mut sim, Vec3::new(-2.0, 0.0, mid.z - 0.05), Vec3::new(2.0, 4.0, mid.z + 0.05), None);
    assert_eq!(flashed(&mut sim, victim, thrower, 200.0, true), None);
    assert_eq!(health(&sim, victim), 100.0, "22: no damage");
}

#[test]
fn t21_flash_self_and_team() {
    let mut sim = sim();
    let thrower = player(&mut sim, 0.0, 0.0, 0.0, 1);
    let (time, _) = flashed(&mut sim, thrower, thrower, 200.0, true).expect("the thrower");
    assert!((time - 25.0).abs() < 1e-3);
    for (ff, want) in [(0, None), (1, Some(25.0))] {
        let mut sim = self::sim();
        sim.app.insert_resource(FriendlyFire(ff));
        let thrower = far_thrower(&mut sim, 1);
        let mate = player(&mut sim, 0.0, 0.0, 0.0, 1);
        let got = flashed(&mut sim, mate, thrower, 200.0, true).map(|(t, _)| (t * 1000.0).round() / 1000.0);
        assert_eq!(got, want, "friendly fire {ff}");
        assert_eq!(health(&sim, mate), 100.0, "22: no damage");
    }
}

#[test]
fn t22_flash_no_damage() {
    // Checked in each of 15-21: hit points unchanged; and no damage at all.
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    flashed(&mut sim, victim, thrower, 50.0, true).expect("flashed");
    assert_eq!(health(&sim, victim), 100.0);
}

#[test]
fn t23_two_flashes() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let (first, _) = flashed(&mut sim, victim, thrower, 200.0, true).expect("first");
    assert!((first - 25.0).abs() < 1e-3);
    sim.seconds(0.5);
    let (second, _) = flashed(&mut sim, victim, thrower, 625.0, true).expect("second");
    assert!((second - 12.5).abs() < 1e-3, "replaced by the weaker one");
    let b = *sim.app.world().get::<Blinded>(victim).unwrap();
    let f = *sim.app.world().get::<CaFlashed>(victim).unwrap();
    assert_eq!(b.fade_start, f.at, "white again from the second");
    assert!((b.end - b.fade_start - 6.25).abs() < 1e-3, "ends 6.25 s later");
}

// ---------------------------------------------------------------------------
// Fuses, smoke, the throw (24-28)

#[test]
fn t24_fuses() {
    for (id, fuse) in [(M67, 2.5), (XM84, 1.5), (M18, 3.0)] {
        let mut sim = sim();
        let g = grenade(&mut sim, id, None, Vec3::new(0.0, 20000.0, 0.0), Vec3::ZERO);
        sim.ticks(5);
        let created = state(&sim, g).created.expect("started");
        let (f, weapon, ..) = until_boom(&mut sim, 4.0);
        assert_eq!(weapon, id);
        assert_eq!(f - created, (fuse / 0.05f64).round() as u64, "{id}");
    }
}

#[test]
fn t25_smoke() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let near_by = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, near_by, 100.0, true);
    explode_at(&mut sim, M18, Some(thrower), at);
    sim.seconds(1.0);
    assert_eq!(health(&sim, near_by), 100.0, "no damage");
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&SightBlocker>().iter(world).count(), 0, "nothing blocks sight (bots, name tags)");
    assert_eq!(world.query::<&chimera_sim::weapon::grenade::SmokeCloud>().iter(world).count(), 1, "the clients' cloud");
    // A bullet through the cloud hits the target behind it.
    let shooter = player(&mut sim, 0.0, 400.0, 0.0, 0);
    let target = player(&mut sim, 0.0, -400.0, 0.0, 0);
    sim.app.world_mut().entity_mut(near_by).despawn();
    let rifle = weapon::give(sim.app.world_mut(), shooter, combat_arms::game::weapons::RIFLE).unwrap();
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<Inventory>(shooter).unwrap().active, Some(rifle));
    // At the target's chest (hull centre): its head leans to the rifle,
    // off the line of the eyes (characters.md 11).
    {
        let d = sim.position(target) - (sim.position(shooter) + sim.state(shooter).eye_offset);
        sim.intent(shooter).pitch = d.y.atan2(d.xz().length());
    }
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(5);
    assert!(health(&sim, target) < 100.0, "hit through the smoke");
}

/// A player at the origin facing +Z (yaw π: ours faces −Z at 0) holding a
/// grenade of `id`, drawn.
fn holding(sim: &mut Sim, id: &str) -> (Entity, Entity) {
    let p = player(sim, 0.0, 0.0, std::f32::consts::PI, 0);
    let w = weapon::give(sim.app.world_mut(), p, id).unwrap();
    {
        let mut inv = sim.app.world_mut().get_mut::<Inventory>(p).unwrap();
        inv.wanted = Some(w);
    }
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "in hand");
    (p, w)
}

fn live(sim: &mut Sim) -> Vec<Entity> {
    let world = sim.app.world_mut();
    world.query_filtered::<Entity, With<CaGrenade>>().iter(world).collect()
}

#[test]
fn t26_no_thrower_velocity() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, "test:m67_exact");
    sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
    sim.seconds(1.0);
    let v = sim.velocity(p);
    assert!(v.length() > 2.0, "moving: {v}");
    // A tap (held, the grenade stays in hand: case 37).
    let (g, _) = throw_timed(&mut sim, p, TICK_INTERVAL, 3.0).expect("thrown");
    let s = state(&sim, g);
    assert_near(s.velocity, Vec3::new(0.0, 0.0, 1550.0), 0.05, "the view's direction at 1550, nothing carried");
    let start = s.pos;
    let s = to_step(&mut sim, g, 1);
    assert_near(s.pos - start, Vec3::new(0.0, -1.0, 77.5), 0.05, "test 1's step");
}

/// A thrown frag (fire pressed and let go, view level) flies case 1's
/// numbers from where it left, drawn with its projectile record's model
/// (`thrown_key`), and is drawn on its path between the server's 20
/// frames a second, one frame behind: at every tick on the segment from
/// its place at the frame before to its place now, a different point each
/// tick (playtest 2026-10-27: it hopped 50 ms at a time).
#[test]
fn thrown_frag_flies_the_spec_arc_and_is_drawn_between_frames() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, "test:m67_exact");
    let (g, _) = throw_timed(&mut sim, p, TICK_INTERVAL, 3.0).expect("thrown");
    let shown = sim.app.world().get::<ShownItem>(g).unwrap().0.clone();
    assert_eq!(shown, grenades::thrown_key("test:m67_exact"), "drawn as its thrown model");
    let first = state(&sim, g);
    let (start, k0) = (first.pos, first.steps as f32);
    let mut drawn = Vec::new();
    for _ in 0..40 {
        let s = state(&sim, g);
        if s.steps >= 5 {
            break;
        }
        let at = sim.app.world().get::<Transform>(g).unwrap().translation / U;
        // On the segment from the frame before to now.
        let seg = s.pos - s.drawn_from;
        let t = if seg.length_squared() > 0.0 { (at - s.drawn_from).dot(seg) / seg.length_squared() } else { 0.0 };
        assert!((-1e-3..=1.0 + 1e-3).contains(&t), "drawn off its segment: t = {t}");
        assert_near(at, s.drawn_from + seg * t, 0.05, "drawn on the segment");
        drawn.push((s.steps, at));
        sim.ticks(1);
    }
    let inside = drawn.windows(2).filter(|w| w[0].0 == w[1].0 && w[0].1 != w[1].1).count();
    assert!(inside >= 3, "it moves between server frames: {drawn:?}");
    let s = to_step(&mut sim, g, 5);
    // Case 1: (0, 59, 77.5) after step 1 and (0, 15, 387.5) after step 5
    // from (0, 60, 0): a drop of 2n² - n u after n steps (the quarter
    // gravity), 77.5 u ahead each.
    let drop = |n: f32| 2.0 * n * n - n;
    let want = Vec3::new(0.0, -(drop(5.0) - drop(k0)), 77.5 * (5.0 - k0));
    assert_near(s.pos - start, want, 0.05, "test 1's step 5");
    assert_near(s.velocity, Vec3::new(0.0, -400.0, 1550.0), 0.05, "and its velocity");
}

/// Press fire with a grenade in hand at tick t0 + 1, hold it `hold` s
/// (at least a tick), run until a grenade leaves (or `limit` s): the
/// grenade and its time since the press (s).
fn throw_timed(sim: &mut Sim, p: Entity, hold: f64, limit: f64) -> Option<(Entity, f64)> {
    let t0 = sim.tick();
    sim.intent(p).fire = true;
    for _ in 0..((limit / TICK_INTERVAL) as u64) {
        sim.ticks(1);
        let since = (sim.tick() - t0) as f64 * TICK_INTERVAL - TICK_INTERVAL;
        if since + 1e-9 >= hold {
            sim.intent(p).fire = false;
        }
        if let Some(g) = live(sim).first() {
            return Some((*g, since));
        }
    }
    None
}

/// Case 27's fuse part (its release time is case 37's): held 3 s, the
/// grenade stays in hand; the fuse counts from the server's frame.
#[test]
fn t27_no_cooking() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, M67);
    let (g, left) = throw_timed(&mut sim, p, 3.0, 4.0).expect("thrown");
    assert!(left > 3.0, "in hand while held: left at {left}");
    sim.ticks(5);
    let created = state(&sim, g).created.unwrap();
    let (f, ..) = until_boom(&mut sim, 3.5);
    assert_eq!(f - created, 50);
}

/// Cases 36 and 38: a tap. Throw animation 700-767 ms, the post-fire from
/// 767 ms, the release at its key, 900 ms (none at the throw animation's
/// end); for the XM84 with 2, the reload 1133-1533 ms, then the second
/// pre-fire. Our ticks are 15 ms: each step waits for the first tick at or
/// after its time, and the grenade appears a tick after the release.
#[test]
fn t36_t38_tap_throw_timing() {
    let tick = TICK_INTERVAL;
    for id in [M67, XM84, M18] {
        let mut sim = sim();
        let (p, w) = holding(&mut sim, id);
        let (first, left) = throw_timed(&mut sim, p, tick, 2.0).expect("thrown");
        assert!((0.900 - 1e-6..=0.900 + 3.0 * tick).contains(&left), "{id}: left at {left}");
        if id == XM84 {
            // The second: fire held from 1.2 s to 1.6 s starts its pre-fire
            // when the reload ends (1533 ms), released at 1533 + 900.
            assert_eq!(sim.app.world().get::<Throwable>(w).unwrap().count, 1);
            let start = left + tick;
            sim.seconds(1.2 - start);
            let pressed = sim.tick();
            sim.intent(p).fire = true;
            let mut second = None;
            for _ in 0..200 {
                sim.ticks(1);
                let t = 1.2 + (sim.tick() - pressed) as f64 * tick;
                if t >= 1.6 {
                    sim.intent(p).fire = false;
                }
                // (The first may have gone off: its fuse is 1.5 s.)
                if live(&mut sim).iter().any(|g| *g != first) {
                    second = Some(t);
                    break;
                }
            }
            let second = second.expect("a second throw");
            assert!((2.433 - 1e-6..=2.433 + 8.0 * tick).contains(&second), "second at {second}");
        }
    }
}

/// Case 37: fire held until 3000 ms keeps the grenade in hand; the throw
/// animation restarts while held, the post-fire starts when the play
/// running at the release ends, and the grenade leaves 133 ms later. Our
/// plays last whole ticks (5 of 15 ms for the 67 ms).
#[test]
fn t37_hold_throw_timing() {
    let mut sim = sim();
    let (p, _) = holding(&mut sim, M67);
    let (_, left) = throw_timed(&mut sim, p, 3.0, 4.0).expect("thrown");
    assert!((3.0 + 0.133..=3.0 + 0.075 + 0.133 + 3.0 * TICK_INTERVAL).contains(&left), "left at {left}");
}

#[test]
fn t28_switch_mid_throw() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, M67);
    let rifle = weapon::give(sim.app.world_mut(), p, combat_arms::game::weapons::RIFLE).unwrap();
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.seconds(0.3);
    {
        let mut inv = sim.app.world_mut().get_mut::<Inventory>(p).unwrap();
        inv.wanted = Some(rifle);
    }
    sim.seconds(2.0);
    assert!(live(&mut sim).is_empty(), "no grenade");
    assert!(booms(&sim).is_empty());
    assert_eq!(sim.app.world().get::<Throwable>(w).unwrap().count, 1, "total stays 1");
}

// ---------------------------------------------------------------------------
// Death (29, 30), sky (31), chains (32)

fn kill(sim: &mut Sim, p: Entity) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 10.0,
        point: at,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(1);
}

#[test]
fn t29_death_after_the_pin_pull() {
    // The release: a live M67 at the hull centre, 500 u/s along the body's
    // facing, full fuse, the ammo untouched.
    let mut sim = sim();
    let (p, w) = holding(&mut sim, M67);
    sim.intent(p).fire = true;
    sim.seconds(0.72);
    assert!(sim.app.world().get::<grenades::CaPin>(w).unwrap().on, "pin pulled");
    let (centre, _) = body(&mut sim, p);
    kill(&mut sim, p);
    let g = live(&mut sim)[0];
    let s = state(&sim, g);
    assert_near(s.pos, centre / U, 0.05, "at the hull centre");
    assert_near(s.velocity, Vec3::new(0.0, 0.0, 500.0), 0.05, "500 u/s along the facing");
    assert_eq!(s.thrower, Some(p), "credited to the dead");
    assert_eq!(sim.app.world().get::<Throwable>(w).unwrap().count, 1, "ammo unchecked");
    sim.ticks(5);
    let created = state(&sim, g).created.unwrap();
    let (f, weapon, ..) = until_boom(&mut sim, 3.0);
    assert_eq!((weapon, f - created), (M67, 50), "2.5 s fuse");
    // Its flight from the spec's (0, 40, 0).
    let mut sim = self::sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 40.0, 0.0), Vec3::new(0.0, 0.0, 500.0));
    let s = to_step(&mut sim, g, 5);
    let c = s.contacts[0];
    assert!(c.step == 5 && (c.at.z - 117.65).abs() <= 0.05, "{c:?}");
    let s = to_step(&mut sim, g, 36);
    let last = s.contacts.last().unwrap();
    assert!(last.rest && last.step == 36, "{last:?}");
    assert_near(last.at, Vec3::new(0.0, 0.0, 361.19), 0.05, "rest");
}

#[test]
fn t30_death_during_the_pin_pull() {
    let mut sim = sim();
    let (p, w) = holding(&mut sim, M67);
    sim.intent(p).fire = true;
    sim.seconds(0.1);
    kill(&mut sim, p);
    sim.seconds(1.5);
    assert!(live(&mut sim).is_empty(), "no grenade");
    assert_eq!(sim.app.world().get::<Throwable>(w).unwrap().count, 1);
}

#[test]
fn t31_sky() {
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, 300.0 * U, -10.0), Vec3::new(10.0, 400.0 * U, 10.0), Some("sky"));
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 50.0, 0.0, 0.0, 0);
    let g = grenade(&mut sim, M67, Some(thrower), Vec3::new(0.0, 100.0, 0.0), Vec3::new(0.0, 1000.0, 0.0));
    sim.ticks(30);
    assert!(sim.app.world().get_entity(g).is_err(), "removed");
    sim.seconds(3.0);
    assert!(booms(&sim).is_empty(), "no explosion");
    assert_eq!(health(&sim, victim), 100.0, "no damage");
}

#[test]
fn t32_chain() {
    for (d, goes) in [(150.0, true), (200.0, false)] {
        let mut sim = sim();
        let flash = grenade(&mut sim, XM84, None, Vec3::new(0.0, 5.0, 0.0), Vec3::ZERO);
        sim.seconds(0.6);
        assert!(state(&sim, flash).resting);
        explode_at(&mut sim, M67, None, Vec3::new(d * U, 0.05, 0.0));
        let b = booms(&sim);
        if goes {
            assert_eq!(b.len(), 2, "{b:?}");
            assert_eq!(b[0].0, b[1].0, "the same server frame");
            assert_eq!(b[1].1, XM84);
        } else {
            assert_eq!(b.len(), 1, "{b:?}");
            let left = state(&sim, flash).health;
            assert!((left - 5.56).abs() < 0.01, "{left}");
        }
    }
}

// ---------------------------------------------------------------------------
// Fire and gas (33, 34), water (35)

/// HP lost by a victim 100 u from a `id` sphere, moved out of it at
/// `leave` s (none: stays), and when the last damage landed (s).
fn over_time(id: &str, leave: Option<f64>) -> (f32, f64) {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let at = in_front(&mut sim, victim, 100.0, true);
    explode_at(&mut sim, id, Some(thrower), at);
    let t0 = sim.tick();
    let mut last = 0.0;
    let mut hp = health(&sim, victim);
    for _ in 0..((25.0 / TICK_INTERVAL) as u64) {
        sim.ticks(1);
        let t = (sim.tick() - t0) as f64 * TICK_INTERVAL;
        if leave.is_some_and(|l| (t - l).abs() < TICK_INTERVAL / 2.0) {
            sim.app.world_mut().get_mut::<Transform>(victim).unwrap().translation.x += 1000.0 * U;
        }
        let now = health(&sim, victim);
        if now < hp {
            last = t;
            hp = now;
        }
    }
    (100.0 - hp, last)
}

#[test]
fn t33_incendiary() {
    let (lost, last) = over_time(M23F, None);
    assert!((lost - 95.0).abs() <= 1.0, "{lost}");
    assert!((last - 19.0).abs() <= 0.2, "last at {last}");
    let (lost, _) = over_time(M23F, Some(4.0));
    assert!((lost - 70.0).abs() <= 1.0, "{lost}");
}

#[test]
fn t34_nerve_gas() {
    let (lost, _) = over_time(G7A, None);
    assert!((lost - 69.0).abs() <= 1.0, "{lost}");
}

#[test]
fn t35_water() {
    let mut sim = sim();
    let box_min = Vec3::new(-5.0, -1.0, -5.0);
    let box_max = Vec3::new(5.0, 5.0, 5.0);
    sim.app
        .world_mut()
        .spawn(CaVolume::liquid_box(box_min, box_max, 0.0, Vec3::ZERO));
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 200.0, 0.0), Vec3::ZERO);
    let s = to_step(&mut sim, g, 1);
    assert_near(s.velocity, Vec3::new(0.0, -10.0, 0.0), 0.05, "v");
    assert!((200.0 - s.pos.y - 0.125).abs() < 1e-3, "drop {}", 200.0 - s.pos.y);
}

// ---------------------------------------------------------------------------
// The follow-up answers (36-58)

/// Cases 39 and 40: the flash and smoke fly exactly along the view
/// (g = 0 makes the cone's top 0) whatever the thrower does; the frag's
/// cone is 1 thousandth (p is 0 in stock play), 5 at p = 0.5.
#[test]
fn t39_t40_throw_spread() {
    assert_eq!(throw_perturb([1, 1], 0.0, 0.0), 0.0);
    assert_eq!(throw_perturb([1, 1], 0.0, 1.0), 0.0);
    assert_eq!(throw_perturb([1, 10], 1.0, 0.0), 1.0);
    assert_eq!(throw_perturb([1, 10], 1.0, 0.5), 5.0);
    for (id, moving) in [(XM84, true), (M18, true), (M67, false)] {
        let mut sim = sim();
        let (p, _) = holding(&mut sim, id);
        if moving {
            sim.intent(p).move_axis = Vec2::new(0.0, 1.0);
            sim.intent(p).jump = true;
            sim.seconds(0.5);
        }
        let (g, _) = throw_timed(&mut sim, p, TICK_INTERVAL, 2.0).expect("thrown");
        // The throw's velocity: gravity's taken off for any step already run.
        let s = state(&sim, g);
        let d = (s.velocity + Vec3::Y * 80.0 * s.steps as f32).normalize();
        if id == M67 {
            // Within 0.001 on each of the right and up axes.
            assert!(d.x.abs() <= 0.001 + 1e-6 && (d.y / d.z).abs() <= 0.001 + 1e-6, "{d}");
        } else {
            assert!(d.x.abs() < 1e-6 && d.y.abs() < 1e-6, "{id}: exactly forward, {d}");
        }
    }
}

/// Case 41: the bounce's push (0.05 per contact, taken twice) sits inside
/// the hardness.
#[test]
fn t41_bounce_push() {
    let (v, rest) = bounce(Vec3::new(100.0, -300.0, 0.0), Vec3::Y, 0.65);
    assert!(!rest);
    assert!((v - Vec3::new(65.0, 194.935, 0.0)).abs().max_element() < 1e-3, "{v}");
}

/// Case 42: the first physics step is in the server frame the grenade is
/// made in; its start time is the frame before; 49 steps, and it goes off
/// at the start of the frame 2.45 s after the first.
#[test]
fn t42_first_step_in_the_creation_frame() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 20000.0, 0.0), Vec3::ZERO);
    let frame = sim.app.world().resource::<CaServerFrames>().frame;
    while sim.app.world().resource::<CaServerFrames>().frame == frame {
        assert_eq!(state(&sim, g).steps, 0);
        sim.ticks(1);
    }
    let first = sim.app.world().resource::<CaServerFrames>().frame;
    let s = state(&sim, g);
    assert_eq!((s.steps, s.created), (1, Some(first - 1)), "stepped in its first frame");
    let s = to_step(&mut sim, g, 49);
    let (boom, ..) = until_boom(&mut sim, 1.0);
    assert_eq!(s.steps, 49);
    assert_eq!(boom - first, 49, "2.45 s after the first frame");
}

/// Case 43's other grenades (the M67's is case 3): an XM84 dropped as in
/// case 3 rises 10 x 0.65 = 6.5; on sand (0.1), 1.0.
#[test]
fn t43_rest_nudge_xm84() {
    for (surface, up) in [(None, 6.5), (Some("desert sand"), 1.0)] {
        let mut sim = sim();
        if surface.is_some() {
            solid(&mut sim, Vec3::new(-10.0, -0.5, -10.0), Vec3::new(10.0, 0.001, 10.0), surface);
        }
        let g = grenade(&mut sim, XM84, None, Vec3::new(0.0, 60.0, 0.0), Vec3::ZERO);
        let s = to_step(&mut sim, g, 6);
        assert!(s.resting, "{:?}", s.contacts);
        sim.seconds(0.2);
        // From the floor's top (the sand's is 0.001 m up).
        let floor = if surface.is_some() { 0.001 / U } else { 0.0 };
        assert_near(state(&sim, g).pos, Vec3::new(0.0, floor + up, 0.0), 0.05, "nudged");
    }
}

/// Case 44: a slanted stop is nudged back along the velocity at the
/// start of its step (76, -400, 0), not the step's path.
#[test]
fn t44_rest_nudge_slanted() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 10.0, 0.0), Vec3::new(76.0, -400.0, 0.0));
    let s = to_step(&mut sim, g, 1);
    let c = s.contacts[0];
    assert!(c.rest, "{c:?}");
    sim.seconds(0.2);
    let moved = state(&sim, g).pos - c.at;
    assert_near(moved, Vec3::new(-0.728, 3.831, 0.0), 0.05, "nudge");
}

/// Case 45: a character's boxes report a touch, its hit spheres decide
/// it: a line of motion that misses every sphere does nothing; one through
/// the chest whose segment crosses only the character's box slows it
/// once; crossing the hit-box object's box (the spheres' bounds) too,
/// twice.
#[test]
fn t45_character_touches() {
    let (min, max) = (Vec3::new(-0.3, 0.0, -0.3), Vec3::new(0.3, 1.8, 0.3));
    let spheres = [(Vec3::new(0.0, 1.3, 0.0), 0.15), (Vec3::new(0.0, 1.7, 0.0), 0.1)];
    let v = Vec3::new(10.0, 0.0, 0.0);
    // At knee height: through the box, no sphere on the line.
    assert_eq!(character_touches(Vec3::new(-1.0, 0.4, 0.0), Vec3::new(1.0, 0.4, 0.0), v, min, max, &spheres), 0);
    // Through the chest, stopping inside the box before the spheres' box.
    assert_eq!(character_touches(Vec3::new(-1.0, 1.3, 0.0), Vec3::new(-0.25, 1.3, 0.0), v, min, max, &spheres), 1);
    // Right through.
    assert_eq!(character_touches(Vec3::new(-1.0, 1.3, 0.0), Vec3::new(1.0, 1.3, 0.0), v, min, max, &spheres), 2);
    // The line counts, not the segment: one ending short of the sphere (it
    // passes 0.14 from the chest's centre) but inside both boxes.
    assert_eq!(character_touches(Vec3::new(-1.0, 1.44, 0.0), Vec3::new(-0.10, 1.44, 0.0), v, min, max, &spheres), 2);
    // A grenade into a standing player's chest: slowed, not turned (no
    // mirror), and again on later steps while inside.
    let mut sim = sim();
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let (centre, _) = body(&mut sim, victim);
    let chest = centre / U + Vec3::new(0.0, 15.0, 0.0);
    let g = grenade(&mut sim, M67, None, chest - Vec3::new(0.0, 0.0, 100.0), Vec3::new(0.0, 0.0, 1000.0));
    let s = to_step(&mut sim, g, 4);
    assert!(s.velocity.z > 0.0 && s.velocity.z < 1000.0 * 0.65 + 1e-3, "slowed: {}", s.velocity);
    assert!(s.velocity.x.abs() < 1e-3 && s.contacts.iter().all(|c| !c.rest), "{s:?}");
}

/// Cases 46 and 47 (and world_dat.md case 10): a solid door and non-solid
/// level detail (traces only) bounce a grenade as case 4's wall does; a
/// door with its solid flag off has no collision, and player blockers
/// don't meet grenades.
#[test]
fn t46_t47_doors_detail_and_blockers() {
    for layer in [None, Some(TRACE_ONLY_LAYER)] {
        let mut sim = sim();
        let (min, max) = (Vec3::new(-10.0, -1.0, 50.0 * U), Vec3::new(10.0, 10.0, 2.0));
        match layer {
            Some(l) => layered(&mut sim, min, max, l),
            None => solid(&mut sim, min, max, None),
        };
        let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
        let c = to_step(&mut sim, g, 1).contacts[0];
        assert_near(c.at, Vec3::new(0.0, 59.35, 50.0), 0.05, "wall");
        assert_near(c.velocity, Vec3::new(0.0, -52.0, -1007.44), 0.05, "off the wall");
    }
    let mut sim = sim();
    layered(&mut sim, Vec3::new(-10.0, -1.0, 50.0 * U), Vec3::new(10.0, 10.0, 2.0), PLAYER_CLIP_LAYER);
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let s = to_step(&mut sim, g, 2);
    assert!(s.contacts.is_empty() && s.pos.z > 150.0, "through the blocker: {:?}", s.pos);
}

/// Case 48: grenades in flight touch through their boxes (each slowed by
/// the other's metal, 0.65, no turn); grenades of one thrower only when
/// the kind's `CanHitSameKind` says so; a resting grenade never.
#[test]
fn t48_grenades_and_grenades() {
    // Two throwers' frags: A crosses B's box in A's second step.
    let mut sim = sim();
    let (ta, tb) = (far_thrower(&mut sim, 0), far_thrower(&mut sim, 0));
    let a = grenade(&mut sim, M67, Some(ta), Vec3::new(0.0, 500.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let b = grenade(&mut sim, M67, Some(tb), Vec3::new(0.0, 500.0, 100.0), Vec3::ZERO);
    let s = to_step(&mut sim, a, 2);
    assert!((s.velocity.z - 1007.5).abs() < 0.05, "A: {}", s.velocity);
    let s = state(&sim, b);
    assert!((s.velocity.y - (-80.0 * 0.65 - 80.0)).abs() < 0.05, "B: {}", s.velocity);
    // Two XM84s of one thrower: nothing.
    let mut sim = self::sim();
    let t = far_thrower(&mut sim, 0);
    let a = grenade(&mut sim, XM84, Some(t), Vec3::new(0.0, 500.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    grenade(&mut sim, XM84, Some(t), Vec3::new(0.0, 500.0, 100.0), Vec3::ZERO);
    assert!((to_step(&mut sim, a, 2).velocity.z - 1550.0).abs() < 0.05);
    // An M67 through a resting XM84's box: nothing.
    let mut sim = self::sim();
    let flash = grenade(&mut sim, XM84, None, Vec3::new(0.0, 5.0, 100.0), Vec3::ZERO);
    sim.seconds(0.6);
    assert!(state(&sim, flash).resting);
    let rest = state(&sim, flash).pos;
    let a = grenade(&mut sim, M67, None, Vec3::new(0.0, rest.y + 1.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let s = to_step(&mut sim, a, 2);
    assert!(s.pos.z > 120.0 && (s.velocity.z - 1550.0).abs() < 0.05, "{s:?}");
}

/// Case 49: stock water (`Viscosity` 0.1, `Current` (0, 1500, 0)): the
/// drag, then the current, then the liquid gravity; nothing at 1 u/s or
/// less.
#[test]
fn t49_stock_water() {
    for (v, want_v, want_d) in [
        (Vec3::new(0.0, 0.0, -1000.0), Vec3::new(0.0, 65.0, -975.0), Some(Vec3::new(0.0, 3.625, -48.75))),
        (Vec3::new(0.0, 0.0, 0.5), Vec3::new(0.0, -10.0, 0.5), None),
    ] {
        let mut sim = sim();
        sim.app.world_mut().spawn(CaVolume::liquid_box(
            Vec3::new(-50.0, -1.0, -50.0),
            Vec3::new(50.0, 50.0, 50.0),
            0.1,
            Vec3::new(0.0, 1500.0, 0.0),
        ));
        let start = Vec3::new(0.0, 200.0, 0.0);
        let g = grenade(&mut sim, M67, None, start, v);
        let s = to_step(&mut sim, g, 1);
        assert_near(s.velocity, want_v, 0.05, "v");
        if let Some(d) = want_d {
            assert_near(s.pos - start, d, 0.01, "moved");
        }
    }
}

/// Case 50: what blocks the blast's cover traces: walls of invisible
/// polygons, non-solid level polygons (traces only), closed doors; not
/// players, props or player blockers.
#[test]
fn t50_cover_blockers() {
    #[derive(Clone, Copy, Debug)]
    enum Wall {
        Invisible,
        TraceOnly,
        Door,
        PlayerBlocker,
        Prop,
        Player,
    }
    for (wall, want) in [
        (Wall::Invisible, 100.0),
        (Wall::TraceOnly, 100.0),
        (Wall::Door, 100.0),
        (Wall::PlayerBlocker, 0.0),
        (Wall::Prop, 0.0),
        (Wall::Player, 0.0),
    ] {
        let mut sim = sim();
        let thrower = far_thrower(&mut sim, 0);
        let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
        let at = in_front(&mut sim, victim, 100.0, true);
        let mid = (at + body(&mut sim, victim).0) / 2.0;
        let (min, max) = (Vec3::new(-2.0, 0.0, mid.z - 0.05), Vec3::new(2.0, 4.0, mid.z + 0.05));
        match wall {
            Wall::Invisible => {
                solid(&mut sim, min, max, Some("invisible"));
            }
            Wall::TraceOnly => {
                layered(&mut sim, min, max, TRACE_ONLY_LAYER);
            }
            Wall::Door => {
                solid(&mut sim, min, max, None);
            }
            Wall::PlayerBlocker => {
                layered(&mut sim, min, max, PLAYER_CLIP_LAYER);
            }
            Wall::Prop => {
                let e = solid(&mut sim, min, max, None);
                sim.app
                    .world_mut()
                    .entity_mut(e)
                    .insert(chimera_world::map::PropEntity(0));
            }
            Wall::Player => {
                let other = player(&mut sim, 0.0, 0.0, 0.0, 0);
                sim.app.world_mut().get_mut::<Transform>(other).unwrap().translation.z = mid.z;
                sim.ticks(1);
            }
        }
        explode_at(&mut sim, M67, Some(thrower), at);
        assert_eq!(health(&sim, victim), want, "{wall:?}");
    }
}

/// Case 51: a breakable takes the blast with no cover test.
#[test]
fn t51_breakables_skip_the_cover_test() {
    #[derive(Resource, Default)]
    struct Hits(Vec<f32>);
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let target = sim
        .app
        .world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(0.3, 0.3, 0.3),
            Transform::from_xyz(0.0, 1.0, 0.0),
            Damageable,
        ))
        .id();
    sim.app.init_resource::<Hits>().add_systems(
        FixedPostUpdate,
        move |mut d: MessageReader<Damage>, mut hits: ResMut<Hits>| {
            hits.0.extend(d.read().filter(|d| d.target == target).map(|d| d.amount / HP));
        },
    );
    let at = Vec3::new(0.0, 1.0, 100.0 * U);
    solid(&mut sim, Vec3::new(-2.0, 0.0, 50.0 * U), Vec3::new(2.0, 4.0, 52.0 * U), None);
    explode_at(&mut sim, M67, Some(thrower), at);
    let hits = &sim.app.world().resource::<Hits>().0;
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!((hits[0] - 60.0).abs() < 0.01, "{hits:?}");
}

/// Case 52: flash protection from gear (torso entries summed): the
/// ballistic mask (0.8), the eyepatch (0.15), the 50 goggles and the
/// eyepatch together (0.65).
#[test]
fn t52_flash_gear() {
    for (names, time) in [
        (&["Ballistic_Mask"][..], 5.0),
        (&["Eyepatch"][..], 21.25),
        (&["Anti-Flash_50", "Eyepatch"][..], 8.75),
    ] {
        let mut sim = sim();
        let thrower = far_thrower(&mut sim, 0);
        let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
        let worn = names.iter().map(|n| gear(n).unwrap_or_else(|| panic!("{n}"))).collect::<Vec<_>>();
        sim.app.world_mut().entity_mut(victim).insert(combat_arms::game::weapons::gear_armour(0.0, &worn));
        let (t, _) = flashed(&mut sim, victim, thrower, 200.0, true).expect("flashed");
        assert!((t - time).abs() < 1e-3, "{names:?}: {t}");
    }
}

/// Cases 53 and 54: the facing is the body's yaw only: looking 80° up
/// with the blast ahead is facing it; a blast straight below the centre
/// counts as behind (f . u = 0) whatever the pitch; the held weapon
/// doesn't matter.
#[test]
fn t53_t54_flash_facing_is_yaw_only() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    sim.intent(victim).pitch = 80f32.to_radians();
    let (t, delay) = flashed(&mut sim, victim, thrower, 200.0, true).expect("flashed");
    assert!((t - 25.0).abs() < 1e-3, "{t}");
    assert!(delay > 0.2, "{delay}");
    // Straight above (below would be under the floor here; f . u = 0
    // either way).
    sim.app.world_mut().entity_mut(victim).remove::<CaFlashed>();
    let (centre, _) = body(&mut sim, victim);
    let g = grenade(&mut sim, XM84, Some(thrower), (centre + Vec3::new(0.0, 200.0 * U, 0.0)) / U, Vec3::ZERO);
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 0.0;
    until_boom(&mut sim, 1.0);
    let blast = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    sim.seconds(0.5);
    let f = *sim.app.world().get::<CaFlashed>(victim).expect("flashed from below");
    assert!((f.time - 6.25).abs() < 1e-3, "{}", f.time);
    assert!(f.at - blast < 0.0833 + 0.05 + 0.016, "{}", f.at - blast);
    // Holding the knife: the same.
    let knife = {
        let world = sim.app.world();
        let inv = world.get::<Inventory>(victim).unwrap();
        *inv.weapons
            .iter()
            .find(|w| world.get::<Weapon>(**w).unwrap().id == combat_arms::game::inventory::KNIFE)
            .expect("the knife")
    };
    sim.app.world_mut().get_mut::<Inventory>(victim).unwrap().wanted = Some(knife);
    sim.seconds(1.0);
    sim.intent(victim).pitch = 0.0;
    let (t, _) = flashed(&mut sim, victim, thrower, 200.0, true).expect("flashed");
    assert!((t - 25.0).abs() < 1e-3, "{t}");
}

/// Case 56 (the carry-over part): a new round doesn't clear grenades in
/// flight; they go off on their fuse.
#[test]
fn t56_round_end_carries_over() {
    let mut sim = sim();
    sim.app.init_resource::<RoundRestarts>();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 20000.0, 0.0), Vec3::ZERO);
    sim.seconds(1.0);
    chimera_core::core::restart_round(sim.app.world_mut());
    sim.ticks(3);
    assert!(sim.app.world().get::<CaGrenade>(g).is_some(), "carried over");
    let (_, weapon, ..) = until_boom(&mut sim, 2.0);
    assert_eq!(weapon, M67);
}

/// Case 58: bullets hit grenades; their damage comes off its hit points
/// and at 1 or less it goes off at once, as at its fuse, credited to its
/// thrower: a resting XM84 (50) on the second 25, a flying M67 (300) on
/// the twelfth.
#[test]
fn t58_shooting_grenades() {
    for (id, rounds, z) in [(XM84, 2, 0.0), (M67, 12, 1550.0)] {
        let mut sim = sim();
        let thrower = far_thrower(&mut sim, 0);
        let g = grenade(&mut sim, id, Some(thrower), Vec3::new(0.0, 5000.0, 0.0), Vec3::new(0.0, 0.0, z));
        sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 100.0;
        sim.ticks(5);
        for i in 1..=rounds {
            sim.app.world_mut().write_message(Damage {
                target: g,
                attacker: None,
                amount: 25.0 * HP,
                point: Vec3::ZERO,
                dir: Vec3::Z,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Bullet,
                weapon: None,
                force: Vec3::ZERO,
            });
            sim.ticks(4);
            let gone = booms(&sim).len() == 1;
            assert_eq!(gone, i == rounds, "{id}: after {i}");
            if !gone {
                assert!((state(&sim, g).health - (if id == XM84 { 50.0 } else { 300.0 } - 25.0 * i as f32)).abs() < 1e-3);
            }
        }
    }
    // A rifle round at a resting XM84 hits it.
    let mut sim = sim();
    let shooter = player(&mut sim, 0.0, 300.0, 0.0, 0);
    let flash = grenade(&mut sim, XM84, None, Vec3::new(0.0, 60.0, 0.0), Vec3::ZERO);
    sim.app.world_mut().get_mut::<CaGrenade>(flash).unwrap().params.fuse = 100.0;
    sim.seconds(1.0);
    assert!(state(&sim, flash).resting);
    let rifle = weapon::give(sim.app.world_mut(), shooter, combat_arms::game::weapons::RIFLE).unwrap();
    sim.seconds(1.5);
    assert_eq!(sim.app.world().get::<Inventory>(shooter).unwrap().active, Some(rifle));
    // Aim at it from the eye.
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    let to = state(&sim, flash).pos * U - eye;
    let mut i = sim.intent(shooter);
    i.yaw = (-to.x).atan2(-to.z);
    i.pitch = (to.y / to.xz().length()).atan();
    sim.ticks(10);
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(10);
    let left = sim.app.world().get::<CaGrenade>(flash).map(|g| g.health);
    assert!(left.is_none_or(|h| h < 50.0), "hit: {left:?}");
}

// ---------------------------------------------------------------------------
// The spent grenade (inventory.md 10, its test case 40)

/// Throwing the last M67 keeps it in the inventory at total 0 (same
/// place), the auto-switch draws the rifle when the post-fire ends, and
/// respawn gives it back.
#[test]
fn spent_grenade_kept_at_total_0() {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, 0.0, 0);
    sim.seconds(1.0);
    let before = sim.app.world().get::<Inventory>(p).unwrap().weapons.clone();
    let ids: Vec<&str> = before.iter().map(|w| sim.app.world().get::<Weapon>(*w).unwrap().id).collect();
    assert_eq!(ids, [combat_arms::game::weapons::RIFLE, combat_arms::game::inventory::KNIFE, M67], "the loadout");
    let m67 = before[2];
    sim.intent(p).select = Some(3);
    sim.ticks(1);
    sim.intent(p).select = None;
    sim.seconds(0.5);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(m67));
    sim.intent(p).fire = true;
    sim.ticks(1);
    sim.intent(p).fire = false;
    sim.seconds(1.5);
    let inv = sim.app.world().get::<Inventory>(p).unwrap().clone();
    assert_eq!(inv.weapons, before, "kept, same place");
    assert_eq!(sim.app.world().get::<Throwable>(m67).unwrap().count, 0);
    assert_eq!(inv.active, Some(before[0]), "auto-switched to the rifle");
    sim.intent(p).select = Some(3);
    sim.ticks(1);
    sim.intent(p).select = None;
    sim.seconds(0.5);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(before[0]), "key 4: nothing");
    combat_arms::game::inventory::respawn_inventory(sim.app.world_mut(), p);
    sim.seconds(1.5);
    let w = sim.app.world().get::<Inventory>(p).unwrap().weapons.clone();
    let m67 = w.iter().find(|w| sim.app.world().get::<Weapon>(**w).unwrap().id == M67).unwrap();
    assert_eq!(sim.app.world().get::<Throwable>(*m67).unwrap().count, 1, "back at respawn");
}

// ---------------------------------------------------------------------------
// The network

/// Over the network: a client throws its M67; both clients draw it in
/// flight, the throw is predicted without error, and its blast's damage
/// reaches both (the victim's health replicated to each).
#[test]
fn net_thrown_grenade_seen_and_damaging() {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 7, 2, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    let me = sim.local_player(0).unwrap();
    // Key 4: the M67.
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().select = Some(3);
    sim.ticks(1);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().select = None;
    sim.ticks(40);
    let active = sim.clients[0].app.world().get::<Inventory>(me).unwrap().active.unwrap();
    assert_eq!(sim.clients[0].app.world().get::<Weapon>(active).unwrap().id, M67, "drawn");
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = true;
    sim.ticks(1);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = false;
    // Seen in flight on both clients.
    let mut seen = [false; 2];
    let ok = sim.until(300, |s| {
        for (i, c) in s.clients.iter_mut().enumerate() {
            let w = c.app.world_mut();
            if w.query::<&ShownItem>().iter(w).any(|i| i.0 == grenades::thrown_key(M67)) {
                seen[i] = true;
            }
        }
        seen == [true, true]
    });
    assert!(ok, "drawn on both clients: {seen:?}");
    for i in 0..2 {
        let g = sim.clients[i].app.world().resource::<NetGraph>().clone();
        assert_eq!(g.errors, 0, "client {i} mispredicted the throw ({:?})", g.last_differing);
    }
    // Put it at the other player's feet on the server; let it go off.
    let victim = sim.character_of(1).unwrap();
    let at = sim.server.app.world().get::<Transform>(victim).unwrap().translation;
    {
        let w = sim.server.app.world_mut();
        let g = w.query_filtered::<Entity, With<CaGrenade>>().iter(w).next().expect("on the server");
        let mut s = w.get_mut::<CaGrenade>(g).unwrap();
        s.pos = (at + Vec3::new(0.5, 0.0, 0.0)) / U;
        s.velocity = Vec3::ZERO;
        s.resting = true;
    }
    let before = sim.server.app.world().get::<Health>(victim).unwrap().current;
    sim.ticks(300);
    let after = sim.server.app.world().get::<Health>(victim).unwrap().current;
    assert!(after < before, "the server's victim hurt");
    let theirs = sim.local_player(1).unwrap();
    let on_victim = sim.clients[1].app.world().get::<Health>(theirs).unwrap().current;
    assert_eq!(on_victim, after, "the victim's client has it");
    // The thrower's client sees the victim's health too.
    let w = sim.clients[0].app.world_mut();
    let others: Vec<f32> = w
        .query_filtered::<&Health, With<Intent>>()
        .iter(w)
        .map(|h| h.current)
        .collect();
    assert!(others.contains(&after), "the thrower's client has it: {others:?}");
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-19): 59-60

/// 59. The fire offset: 20 u clear ahead of an eye at (0, 64, 0) looking
/// along +Z → the grenade starts at (0, 64, 20); a wall 10 u ahead → at
/// the eye; offset 0 → at the eye. And the data's default is 0.
#[test]
fn t59_fire_offset() {
    let mut sim = sim();
    let thrower = player(&mut sim, 0.0, -500.0, 0.0, 0);
    let eye = Vec3::new(0.0, 64.0, 0.0) * U;
    let at = grenades::fire_offset(sim.app.world_mut(), thrower, eye, Vec3::Z, 20.0 * U);
    assert_near(at / U, Vec3::new(0.0, 64.0, 20.0), 1e-3, "clear");
    let at = grenades::fire_offset(sim.app.world_mut(), thrower, eye, Vec3::Z, 0.0);
    assert_near(at / U, Vec3::new(0.0, 64.0, 0.0), 1e-6, "no offset");
    solid(&mut sim, Vec3::new(-50.0, 0.0, 10.0) * U, Vec3::new(50.0, 100.0, 12.0) * U, None);
    let at = grenades::fire_offset(sim.app.world_mut(), thrower, eye, Vec3::Z, 20.0 * U);
    assert_near(at / U, Vec3::new(0.0, 64.0, 0.0), 1e-6, "a wall 10 u ahead");
    let w = weapon_of(&mut sim, M67, None);
    let data = sim.app.world().get::<grenades::CaGrenadeData>(w).unwrap();
    assert_eq!(data.params.fire_offset, 0.0, "stock: 0 (Open question 23)");
}

/// 60. The big-object slack on a character: dims 130 long, 300 u away →
/// 270: 60 x (1 - (270 - 112.5) / 337.5) = 32.0 a hit. A stock body's
/// dims (15, 50, 15) change nothing.
#[test]
fn t60_slack_on_a_large_character() {
    let dims = Vec3::new(0.0, 130.0, 0.0);
    let d = grenades::blast_distance(300.0, dims);
    assert!((d - 270.0).abs() < 1e-4, "{d}");
    let per_hit = 60.0 * grenades::falloff(450.0, d);
    assert!((per_hit - 32.0).abs() < 1e-3, "{per_hit}");
    assert_eq!(grenades::blast_distance(300.0, Vec3::new(15.0, 50.0, 15.0)), 300.0);
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-22): cases 61-63

/// 61. The flash fades on a cosine on the client's screen, not Chimera's
/// straight line.
#[test]
fn t61_flash_curve() {
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
    let (time, _) = flashed(&mut sim, victim, thrower, 200.0, true).expect("flashed");
    assert!((time - 25.0).abs() < 1e-3);
    let b = *sim.app.world().get::<Blinded>(victim).unwrap();
    for (t, white, linear) in [(0.0, 1.0, 1.0), (3.125, 0.924, 0.75), (6.25, 0.7071, 0.5), (8.333, 0.5, 0.333), (12.5, 0.0, 0.0)] {
        let now = b.fade_start + t;
        assert!((grenades::blinded_alpha(&b, now) - white).abs() < 1e-3, "white at {t}: {}", grenades::blinded_alpha(&b, now));
        assert!((b.alpha_at(now) - linear).abs() < 1e-3, "Chimera's own fade stays linear");
    }
    // Before the fade starts it is full white; after the end none.
    assert_eq!(grenades::blinded_alpha(&b, b.fade_start - 1.0), 1.0);
    assert_eq!(grenades::blinded_alpha(&b, b.end + 1.0), 0.0);
    // Facing away (T = 6.25 s): 0.707 at 1.5625 s (linear: 0.5).
    let (time, _) = flashed(&mut sim, victim, thrower, 200.0, false).expect("flashed");
    let b = *sim.app.world().get::<Blinded>(victim).unwrap();
    assert!((time - 6.25).abs() < 1e-3);
    assert!((grenades::blinded_alpha(&b, b.fade_start + 1.5625) - 0.7071).abs() < 1e-3);
}

/// 62. The smoke cloud is spread by 0.5 s and nothing is left of it at
/// 10 s.
#[test]
fn t62_smoke_life() {
    use chimera_sim::weapon::grenade::SmokeCloud;
    let mut sim = sim();
    let thrower = far_thrower(&mut sim, 0);
    explode_at(&mut sim, M18, Some(thrower), Vec3::new(0.0, 1.0, 0.0));
    let world = sim.app.world_mut();
    let cloud = world.query::<&SmokeCloud>().iter(world).next().expect("the cloud").clone();
    assert!((cloud.radius(0.5) - cloud.smoke.max_radius).abs() < 1e-5, "full size by 0.5 s");
    assert!(cloud.radius(0.25) < cloud.smoke.max_radius);
    assert_eq!(cloud.alpha(10.0), 0.0, "nothing of it at 10 s");
    assert!(cloud.alpha(9.0) > 0.0);
    sim.seconds(9.4);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&SmokeCloud>().iter(world).count(), 1, "still there at about 9.5 s");
    sim.seconds(1.5);
    let world = sim.app.world_mut();
    assert_eq!(world.query::<&SmokeCloud>().iter(world).count(), 0, "gone by 11 s");
}

/// The explosion looks (`FxKind::Grenade`) and the sounds the frag made.
fn grenade_look(sim: &mut Sim) -> (Vec<String>, Vec<CaSound>) {
    sim.ticks(3);
    let looks = std::mem::take(&mut sim.app.world_mut().resource_mut::<Looks>().0);
    let heard = std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0);
    let groups = looks.iter().filter(|r| r.kind == FxKind::Grenade).map(|r| r.group.clone()).collect();
    (groups, heard)
}

/// 63. An M67 exploding in the air without having touched anything, seen
/// at 500 u: its own surface's group (metal: `Grenade_Stone`) and
/// `gren.wav`.
#[test]
fn t63_mid_air_frag_look() {
    let mut sim = sim();
    let me = player(&mut sim, 0.0, 0.0, 0.0, 0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let at = in_front(&mut sim, me, 500.0, true);
    explode_at(&mut sim, M67, None, at);
    let (groups, heard) = grenade_look(&mut sim);
    assert_eq!(groups, ["Grenade_Stone"]);
    assert!(heard.iter().any(|s| s.file == "guns_snd_impacts/explosions/gren.wav" && s.at.is_some()), "{heard:?}");
}

/// Corrections 1 (effects_sounds.md, case 58): the look is that of the
/// surface of the grenade's last bounce, even when it goes off in the air
/// far from it.
#[test]
fn t63_last_bounce_decides_the_look() {
    // Off a Wood wall, then in the air.
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, -1.0, 50.0 * U), Vec3::new(10.0, 10.0, 2.0), Some("30"));
    let me = player(&mut sim, 0.0, 545.0, 0.0, 0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let s = to_step(&mut sim, g, 1);
    assert_eq!(s.contacts.len(), 1, "the wall");
    assert_eq!(s.last_surface.as_deref(), Some("30"));
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 0.0;
    until_boom(&mut sim, 1.0);
    let (groups, _) = grenade_look(&mut sim);
    assert_eq!(groups, ["Grenade_Dirt"], "Wood's, in the air");
}

/// ... and a frag resting on Desert_Sand after bouncing there shows
/// `Grenade_Sand`.
#[test]
fn t63_resting_on_sand() {
    let mut sim = sim();
    let x = 110.0 / U;
    solid(&mut sim, Vec3::new(100.0, -1.0, -10.0), Vec3::new(120.0, 0.0, 10.0), Some("60"));
    let me = player(&mut sim, x, 500.0, 0.0, 0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let g = grenade(&mut sim, M67, None, Vec3::new(x, 60.0, 0.0), Vec3::ZERO);
    let s = to_step(&mut sim, g, 6);
    assert!(s.resting, "{s:?}");
    assert_eq!(s.last_surface.as_deref(), Some("60"));
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().params.fuse = 0.0;
    until_boom(&mut sim, 1.0);
    let (groups, _) = grenade_look(&mut sim);
    assert_eq!(groups, ["Grenade_Sand"]);
}

// ---------------------------------------------------------------------------
// Corrections (2026-10-27): sounds, spin and pose of a throw (64-72)

/// The M67 with the stock record's misc sounds (what the install's
/// `MiscSound1` and `MiscSound2` give it, `data::sound_values`).
const M67_PIN: &str = r#"
id = "test:m67_pin"
base = "combat_arms:m67"
[hooks."combat_arms:effects"]
misc_sounds = ["guns_snd_throwing/m67/fire.wav", "guns_snd_throwing/m67/firetoss.wav"]
"#;

fn pin_sim() -> Sim {
    let mut sim = sim();
    sim.app.add_weapon_data(&[("m67_pin", M67_PIN)]).unwrap();
    sim
}

fn heard_all(sim: &mut Sim) -> Vec<CaSound> {
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Heard>().0)
}

/// The weapon documents the install's records make of our grenades.
fn merged(weapons: &str) -> Vec<toml::Table> {
    let file = combat_arms::game::data::attributes::WeaponsFile::read(&Document::parse(weapons.as_bytes()).unwrap()).unwrap();
    let ours = [
        ("m67.toml", include_str!("../../data/weapons/m67.toml")),
        ("xm84.toml", include_str!("../../data/weapons/xm84.toml")),
        ("m18.toml", include_str!("../../data/weapons/m18.toml")),
    ];
    combat_arms::game::data::merge_weapon_docs(&ours, &file, 0.018)
        .unwrap()
        .iter()
        .map(|(_, t)| t.parse().unwrap())
        .collect()
}

/// 64. Pin sound source: `MiscSound1` is key 10, named by the record (no
/// fallback by file name); the other fields are loaded but their keys
/// never fire on the stock models.
#[test]
fn t64_pin_sound_source() {
    let docs = merged(
        r#"[Ammo0] Name="a"
[Weapon0] Name="M67" AmmoName0="a" Guntype=7
  MiscSound1="GUNS_SND_Throwing\M67\fire.wav" MiscSound2="GUNS_SND_Throwing\M67\firetoss.wav"
[Weapon1] Name="XM84" AmmoName0="a" Guntype=7 MiscSound1="GUNS_SND_Throwing\XM-84\fire.wav"
[Weapon2] Name="M18" AmmoName0="a" Guntype=7"#,
    );
    let fx = |d: &toml::Table| -> Option<CaWeaponFx> {
        d["hooks"].get(EFFECTS).cloned().map(|v| v.try_into().unwrap())
    };
    let m67 = fx(&docs[0]).expect("the M67 has misc sounds");
    assert_eq!(m67.key_sound(10).map(|k| k.0), Some("guns_snd_throwing/m67/fire.wav"));
    assert_eq!(m67.key_sound(11).map(|k| k.0), Some("guns_snd_throwing/m67/firetoss.wav"));
    assert_eq!(m67.key_sound(12), None);
    let xm84 = fx(&docs[1]).unwrap();
    assert_eq!(xm84.key_sound(10).map(|k| k.0), Some("guns_snd_throwing/xm-84/fire.wav"));
    assert!(fx(&docs[2]).unwrap().key_sound(10).is_none(), "a record without MiscSound1: silence, no fallback by file name");
    // The call-out switch: Guntype 7 with PlayAutoThrowSound on.
    for d in &docs {
        assert_eq!(d["hooks"]["combat_arms:grenade"]["call_out"].as_bool(), Some(true));
    }
    let off = merged(r#"[Ammo0] Name="a" [Weapon0] Name="M67" AmmoName0="a" Guntype=7 PlayAutoThrowSound=FALSE [Weapon1] Name="XM84" AmmoName0="a" Guntype=10"#);
    for d in &off[..2] {
        assert_eq!(d["hooks"]["combat_arms:grenade"]["call_out"].as_bool(), Some(false));
    }
    // Keys 10 to 14 are MiscSound1..5.
    let all = merged(
        r#"[Ammo0] Name="a" [Weapon0] Name="M67" AmmoName0="a" Guntype=7
        MiscSound1="a.wav" MiscSound2="b.wav" MiscSound3="c.wav" MiscSound4="d.wav" MiscSound5="e.wav""#,
    );
    let fx = fx(&all[0]).unwrap();
    let files: Vec<_> = (10..=14).map(|k| fx.key_sound(k).unwrap().0.to_string()).collect();
    assert_eq!(files, ["a.wav", "b.wav", "c.wav", "d.wav", "e.wav"]);
}

/// 65. Pin sound for others: the thrower's flash point at the key, fixed
/// there, radius 2000 (inner 140), full volume; gain 0.717 at 1000 u and
/// silent at 1900; the thrower hears it once, locally; none from the dead.
#[test]
fn t65_pin_sound_for_others() {
    let mut sim = pin_sim();
    let me = player(&mut sim, 0.0, 0.0, 0.0, 0);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = player(&mut sim, 0.0, 300.0, 0.0, 0);
    for p in [me, other] {
        let w = weapon::give(sim.app.world_mut(), p, "test:m67_pin").unwrap();
        sim.app.world_mut().get_mut::<Inventory>(p).unwrap().wanted = Some(w);
    }
    sim.seconds(1.0);
    heard_all(&mut sim);
    let key = |sim: &mut Sim, owner: Entity| {
        sim.app.world_mut().write_message(CaViewKey {
            owner,
            key: combat_arms::game::view_model::machine::ViewKey::Sound(10),
        });
        sim.app.update();
        heard_all(sim)
            .into_iter()
            .filter(|s| s.file.ends_with("m67/fire.wav"))
            .collect::<Vec<_>>()
    };
    let own = key(&mut sim, me);
    assert_eq!(own.len(), 1, "once");
    assert_eq!((own[0].at, own[0].volume, own[0].server), (None, 100.0, false), "local, full volume");
    let theirs = key(&mut sim, other);
    assert_eq!(theirs.len(), 1);
    let at = theirs[0].at.expect("3D");
    assert_eq!(theirs[0].radii, (140.0, 2000.0));
    assert_eq!(theirs[0].volume, 100.0);
    assert_eq!(theirs[0].source, None, "fixed where the key fired, not following");
    let eye = sim.app.world().get::<Transform>(other).unwrap().translation;
    assert!((at - eye).length() < 3.0, "at the thrower's flash point (the eye without a view model)");
    // The gain at 1000 and 1900 u.
    let gain = |d: f32| rules::gain(d, rules::inner_radius(2000.0), 2000.0, 100.0);
    assert!((gain(1000.0) - 0.717).abs() < 1e-3);
    assert_eq!(gain(1900.0), 0.0);
    // The thrower walking away doesn't move the sound that was started.
    sim.app.world_mut().get_mut::<Transform>(other).unwrap().translation += Vec3::new(50.0, 0.0, 0.0);
    sim.ticks(1);
    // None from the dead.
    sim.app.world_mut().get_mut::<Health>(other).unwrap().current = 0.0;
    assert!(key(&mut sim, other).is_empty(), "dead: none");
}

/// 66. No release sound: a throw makes no weapon sound at the release or
/// in flight before the first bounce (the call-out is a voice).
#[test]
fn t66_no_release_sound() {
    let mut sim = pin_sim();
    let (p, _) = holding(&mut sim, "test:m67_pin");
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    heard_all(&mut sim);
    let (g, _) = throw_timed(&mut sim, p, TICK_INTERVAL, 3.0).expect("thrown");
    sim.ticks(3);
    assert_eq!(state(&sim, g).contacts.len(), 0);
    let files: Vec<String> = heard_all(&mut sim).into_iter().map(|s| s.file).collect();
    assert!(files.iter().all(|f| f.contains("vma_")), "only the voice: {files:?}");
}

/// The call-out sounds heard (the male template's two files).
fn call_outs(heard: Vec<CaSound>) -> Vec<CaSound> {
    heard.into_iter().filter(|s| s.file.contains("vma_grenade") || s.file.contains("vma_fireinthehole")).collect()
}

/// 67. The throw call-out: the thrower's team hears one of two male
/// files from the thrower, radius 500 (inner 35); another team nothing;
/// a second grenade while it plays is dropped.
#[test]
fn t67_throw_call_out() {
    for (mine, hears) in [(0, true), (1, false)] {
        let mut sim = sim();
        let (thrower, _) = holding(&mut sim, M67);
        let listener = if mine == 0 { thrower } else { player(&mut sim, 400.0, 400.0, 0.0, 1) };
        sim.app.world_mut().entity_mut(listener).insert(LocalPlayer);
        heard_all(&mut sim);
        throw_timed(&mut sim, thrower, TICK_INTERVAL, 3.0).expect("thrown");
        sim.ticks(2);
        let got = call_outs(heard_all(&mut sim));
        if !hears {
            assert!(got.is_empty(), "the other team hears nothing: {got:?}");
            continue;
        }
        assert_eq!(got.len(), 1, "{got:?}");
        let c = &got[0];
        assert!(
            ["chars_snd/male/vma_grenade_ma.wav", "chars_snd/male/vma_fireinthehole_ma.wav"].contains(&c.file.as_str()),
            "{}",
            c.file
        );
        assert_eq!(c.radii, (35.0, 500.0));
        assert_eq!((c.volume, c.source, c.server), (100.0, Some(thrower), false), "follows the thrower (its source)");
        assert!(c.at.is_some());
    }
}

/// 67 (second part). A call-out is dropped while the client's previous one
/// still plays.
#[test]
fn t67_a_second_call_out_is_dropped_while_one_plays() {
    let mut l = callout::CalloutLimiter::default();
    assert!(l.try_start(10.0, 1.5));
    assert!(!l.try_start(11.0, 1.5), "still playing");
    assert!(l.try_start(11.5, 1.5), "over");
    let t = CaFxTables::built_in();
    assert_eq!(callout::files(t, false).len(), 2);
    assert!(callout::files(t, true).iter().all(|f| f.contains("female") && f.ends_with("_fa.wav")));
    assert_eq!(t.sounds.grenade_throw_radius, 500.0);
}

/// 67 (over the network). The server sends the call-out to the thrower's
/// team only, by their teams on the server; the thrower's own client plays
/// it, as does a teammate's; a client on the other team hears nothing. And
/// the client draws the flying grenade through the rotation blend.
#[test]
fn t67_net_call_out_reaches_the_thrower_s_team() {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 7, 3, |app| {
        app.add_plugins(GreyboxMapPlugin);
        app.init_resource::<Heard>().add_systems(
            Update,
            (|mut r: MessageReader<CaSound>, mut h: ResMut<Heard>| h.0.extend(r.read().cloned()))
                .after(effects::CaEffectsSet),
        );
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(900);
    sim.ticks(240);
    let me = sim.local_player(0).unwrap();
    // Clients 0 and 1 on one team, client 2 on the other (the server's).
    let characters: Vec<Entity> = (0..3).map(|i| sim.character_of(i).unwrap()).collect();
    for (i, c) in characters.iter().enumerate() {
        sim.server.app.world_mut().entity_mut(*c).insert(Team(if i == 2 { 1 } else { 0 }));
    }
    sim.ticks(30);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().select = Some(3);
    sim.ticks(1);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().select = None;
    sim.ticks(40);
    for c in &mut sim.clients {
        c.app.world_mut().resource_mut::<Heard>().0.clear();
    }
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = true;
    sim.ticks(1);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = false;
    sim.ticks(120);
    let heard: Vec<usize> = (0..3)
        .map(|i| call_outs(heard_all_of(&mut sim.clients[i].app)).len())
        .collect();
    assert_eq!(heard, [1, 1, 0], "the thrower's team only, the thrower included");
    // Client 1 draws the grenade through the blend.
    let w = sim.clients[1].app.world_mut();
    let blended = w.query::<&tumble::ThrownBlend>().iter(w).count();
    assert!(blended <= 1);
}

fn heard_all_of(app: &mut App) -> Vec<CaSound> {
    std::mem::take(&mut app.world_mut().resource_mut::<Heard>().0)
}

/// A grenade high above the floor with these spin rates, thrown along `v`.
fn tumbling(sim: &mut Sim, v: Vec3, rates: Vec3) -> Entity {
    let g = grenade(sim, M67, None, Vec3::new(0.0, 3000.0, 0.0), v);
    sim.app.world_mut().get_mut::<CaGrenade>(g).unwrap().tumble.rates = rates;
    g
}

fn axes(sim: &Sim, g: Entity) -> (Vec3, Vec3) {
    tumble::game_axes(sim.app.world().get::<Transform>(g).unwrap().rotation)
}

fn close3(a: Vec3, b: [f32; 3], what: &str) {
    assert!((a - Vec3::from(b)).abs().max_element() < 5e-4, "{what}: {a:?} vs {b:?}");
}

/// 68. Tumble numbers, and the same whichever way it was thrown.
#[test]
fn t68_tumble() {
    use std::f32::consts::{FRAC_PI_2, PI};
    let rates = Vec3::new(FRAC_PI_2, PI, -PI);
    let mut sim = sim();
    let a = tumbling(&mut sim, Vec3::new(0.0, 0.0, 1550.0), rates);
    let b = tumbling(&mut sim, Vec3::new(-900.0, 300.0, -400.0), rates);
    to_step(&mut sim, a, 1);
    for g in [a, b] {
        let s = state(&sim, g);
        close3(s.tumble.angles, [0.0785, 0.1571, -0.1571], "angles after the creation frame");
        let (f, u) = axes(&sim, g);
        close3(f, [0.1560, -0.0785, 0.9846], "forward");
        close3(u, [0.1666, 0.9846, 0.0521], "up");
    }
    to_step(&mut sim, a, 4);
    let (f, u) = axes(&sim, a);
    close3(state(&sim, a).tumble.angles, [0.3142, 0.6283, -0.6283], "angles after 4");
    close3(f, [0.5590, -0.3090, 0.7694], "forward");
    close3(u, [0.6225, 0.7694, -0.1432], "up");
    to_step(&mut sim, a, 20);
    let (f, u) = axes(&sim, a);
    close3(f, [0.0, -1.0, 0.0], "forward after 20");
    close3(u, [0.0, 0.0, 1.0], "up after 20");
}

/// 69. A bounce redraws the rates (uniform in their ranges) and leaves the
/// angles where they were.
#[test]
fn t69_tumble_across_a_bounce() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let mut last = None;
    let mut bounced = false;
    for _ in 0..80 {
        sim.ticks(1);
        let Some(s) = sim.app.world().get::<CaGrenade>(g) else { break };
        let s = s.clone();
        if let Some((rates, angles, n)) = last
            && s.contacts.len() > n
        {
            // The update that bounced still turned by the old rates.
            let moved: Vec3 = s.tumble.angles - angles;
            assert!(moved.abs_diff_eq(rates * 0.05, 1e-4), "{moved:?} vs {rates:?}");
            assert_ne!(s.tumble.rates, rates, "redrawn");
            let r = s.tumble.rates;
            assert!(r.x.abs() <= std::f32::consts::PI && r.y.abs() <= std::f32::consts::TAU && r.z.abs() <= std::f32::consts::TAU);
            bounced = true;
            break;
        }
        last = Some((s.tumble.rates, s.tumble.angles, s.contacts.len()));
    }
    assert!(bounced, "bounced");
}

/// 70. Frozen at rest: the rotation is the same on every later frame.
#[test]
fn t70_frozen_at_rest() {
    let mut sim = sim();
    let g = grenade(&mut sim, M67, None, Vec3::new(0.0, 60.0, 0.0), Vec3::ZERO);
    let s = to_step(&mut sim, g, 6);
    assert!(s.resting);
    let before = sim.app.world().get::<Transform>(g).unwrap().rotation;
    let angles = s.tumble.angles;
    sim.seconds(1.0);
    assert_eq!(sim.app.world().get::<Transform>(g).unwrap().rotation, before);
    assert_eq!(state(&sim, g).tumble.angles, angles);
}

/// 71. Client smoothing numbers (`tumble::RotationBlend`): 25.5, 19.9 and
/// 12.3 degrees left after frames 1 to 3, 0 after the fourth.
#[test]
fn t71_client_smoothing() {
    let server = Quat::from_rotation_y(30f32.to_radians());
    let mut b = tumble::RotationBlend::snapped(Quat::IDENTITY);
    b.receive(server, 0.05);
    let left: Vec<f32> = (0..4)
        .map(|_| {
            b.frame(1.0 / 60.0);
            b.drawn.angle_between(server).to_degrees()
        })
        .collect();
    for (got, want) in left.iter().zip([25.5, 19.9, 12.3, 0.0]) {
        assert!((got - want).abs() < 0.06, "{left:?}");
    }
}

/// 72. Pose: a thrown (or death-released) grenade is shown in `World`
/// from its creation, through its `thrown_key` model; its box stays the
/// `Hand` one times 2.
#[test]
fn t72_world_pose_from_creation() {
    assert_eq!(combat_arms::game::characters::models::THROWN_POSE, "World");
    let mut sim = sim();
    for id in [M67, XM84, M18] {
        let g = grenade(&mut sim, id, None, Vec3::new(0.0, 60.0, 0.0), Vec3::ZERO);
        // Before any frame has run.
        assert_eq!(sim.app.world().get::<ShownItem>(g).unwrap().0, grenades::thrown_key(id));
    }
    let w = weapon_of(&mut sim, M67, None);
    let data = sim.app.world().get::<grenades::CaGrenadeData>(w).unwrap();
    assert_eq!(data.params.box_half, [6.0, 6.0, 8.0]);
    assert_eq!(data.params.thrown_scale, [2.0; 3]);
}
