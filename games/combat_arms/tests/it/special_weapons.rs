//! specs/combat_arms/special_weapons.md's test cases, one test each (`t01_`
//! is test case 1), on `CaSim` with the game's special weapons
//! (data/weapons/law.toml and the rest): an open world of our own (a flat
//! floor at y = 0, walls, sky and platforms where a case asks) and Combat
//! Arms players (100 HP, armour points 0, no gear) as victims. Server
//! frames are the spec's 20 Hz (`CaServerFrames`); "step n" reads the
//! object (`CaObject`, world units) after its nth physics step.
//!
//! The spec's +Z forward is ours too for objects (a player's yaw pi looks
//! down +Z). Tolerances: the spec's 0.05 u for flight, 0.01 HP for
//! damage; times are frames of 50 ms (a tick of 15 ms beyond); "about"
//! (over-time damage) 1.5 HP. A case that names a victim "centre (x, y, z)"
//! is run with the player standing where the case's distance holds on the
//! floor (a player's centre is about 40 u up) and the expected number
//! computed from the distance actually measured (the same formula).
//! Where a case's text contradicts the spec's own section the section is
//! followed and the test says so.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Damage, FriendlyFire, Health, Intent, MovementState, PropSurface, SimClock, Team};
use chimera_sim::weapon::{
    self, Inventory, Magazine, Weapon, WeaponEvent, WeaponEventKind,
    data::WeaponDataAppExt,
    grenade::{Detonated, GrenadeKind},
};
use chimera_world::harness::Sim;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        grenades::{self, CaDamageSphere, CaGrenadeEffects, CaServerFrames, OverTimeKind},
        movement,
        special::{self, CaDeviceData, CaObject, CaProjectileData, Kind, ProjectileParams},
        weapons::{Location, MALE, body_geometry},
    },
    harness::CaSim,
};

/// Metres per world unit (the data's).
pub(super) const U: f32 = combat_arms::game::inventory::UNIT;
/// Health per hit point.
pub(super) const HP: f32 = 0.01;

pub(super) const LAW: &str = "combat_arms:law";
pub(super) const RPG: &str = "combat_arms:rpg-7";
pub(super) const AT4: &str = "combat_arms:m136_at4";
pub(super) const M32: &str = "combat_arms:m32";
pub(super) const M32_INCENDIARY: &str = "combat_arms:m32_incendiarybomb";
pub(super) const MZP1: &str = "combat_arms:mzp_1";
pub(super) const XM25: &str = "combat_arms:xm_25";
pub(super) const FLAME: &str = "combat_arms:flamethrower";
pub(super) const MINE: &str = "combat_arms:m16a1_mine";
pub(super) const HORNET: &str = "combat_arms:hornet";
pub(super) const CLAYMORE: &str = "combat_arms:claymore";
pub(super) const CLAYMORE_CTL: &str = "combat_arms:claymore_controller";
pub(super) const REMOTE: &str = "combat_arms:mobile_grenade";
pub(super) const REMOTE_CTL: &str = "combat_arms:mobile_grenade_controller";
pub(super) const CALTROPS: &str = "combat_arms:caltrops";
pub(super) const NERVE: &str = "combat_arms:nerve_confuse_mine";
pub(super) const KIT: &str = "combat_arms:firstaidkit";
pub(super) const MEDKIT: &str = "combat_arms:medkit";
pub(super) const LGB: &str = "combat_arms:lgb";
pub(super) const NAPALM: &str = "combat_arms:airstrike_napalm";
pub(super) const BOW: &str = "combat_arms:assassin_bowgun_nor";
pub(super) const BOW_EXPL: &str = "combat_arms:assassin_bowgun_expl";
pub(super) const BOW_CTL: &str = "combat_arms:assassin_bowgun_expl_controller";
pub(super) const RIFLE: &str = combat_arms::game::weapons::RIFLE;

/// The turret with no aim error and no roll, so its rays are countable.
pub(super) const TURRET_EXACT: &str = r#"
id = "test:turret_exact"
base = "combat_arms:auto-turret"
[hooks."combat_arms:device".turret]
perturb = 0.0
[hooks."combat_arms:device".turret.gun]
roll = 0.0
"#;

/// Melee weapons with no damage roll ("roll 1.0": the base damage).
pub(super) const MACHETE_EXACT: &str = r#"
id = "test:machete_exact"
base = "combat_arms:machete"
[hooks."combat_arms:knife"]
damage_spread = 0.0
"#;
pub(super) const CHAINSAW_EXACT: &str = r#"
id = "test:chainsaw_exact"
base = "combat_arms:chainsaw"
[hooks."combat_arms:knife"]
damage_spread = 0.0
"#;
pub(super) const SWORD_EXACT: &str = r#"
id = "test:sword_exact"
base = "combat_arms:dompedrosword"
[hooks."combat_arms:knife"]
damage_spread = 0.0
"#;

/// The crossbow with no roll (the band is what the case measures).
pub(super) const BOW_EXACT: &str = r#"
id = "test:bow_exact"
base = "combat_arms:assassin_bowgun_nor"
[hooks."combat_arms:bolt".damage]
roll = 0.0
"#;

/// Detonations seen: (server frame, weapon, thrower, where in u).
#[derive(Resource, Default)]
pub(super) struct Booms(pub Vec<(u64, &'static str, Option<Entity>, Vec3)>);

pub(super) fn log_booms(mut d: MessageReader<Detonated>, frames: Res<CaServerFrames>, mut log: ResMut<Booms>) {
    for d in d.read() {
        if d.kind == GrenadeKind::Blast {
            log.0.push((frames.frame, d.weapon, d.thrower, d.at / U));
        }
    }
}

/// Damage dealt: (time, target, attacker, hit points, weapon).
#[derive(Resource, Default)]
pub(super) struct Hits(pub Vec<(f64, Entity, Option<Entity>, f32, Option<&'static str>)>);

pub(super) fn log_hits(mut damage: MessageReader<Damage>, clock: Res<SimClock>, mut log: ResMut<Hits>) {
    for d in damage.read() {
        log.0.push((clock.now, d.target, d.attacker, d.amount / HP, d.weapon));
    }
}

/// What weapons did: (time, owner, weapon, kind).
#[derive(Resource, Default)]
pub(super) struct Rounds(pub Vec<(f64, Entity, Entity, WeaponEventKind)>);

pub(super) fn log_rounds(mut events: MessageReader<WeaponEvent>, clock: Res<SimClock>, mut log: ResMut<Rounds>) {
    for e in events.read() {
        log.0.push((clock.now, e.owner, e.weapon, e.kind.clone()));
    }
}

/// An open world: a floor at y = 0 and nothing else.
pub(super) fn sim() -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.init_resource::<Booms>()
            .init_resource::<Hits>()
            .init_resource::<Rounds>()
            .add_systems(FixedPostUpdate, (log_booms, log_hits, log_rounds));
    });
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app
        .add_weapon_data(&[
            ("turret_exact", TURRET_EXACT),
            ("machete_exact", MACHETE_EXACT),
            ("chainsaw_exact", CHAINSAW_EXACT),
            ("sword_exact", SWORD_EXACT),
            ("bow_exact", BOW_EXACT),
        ])
        .unwrap();
    solid(&mut sim, Vec3::new(-200.0, -1.0, -200.0), Vec3::new(200.0, 0.0, 200.0), None);
    sim.ticks(1);
    sim
}

/// A static box from `min` to `max` (m), with a surface name.
pub(super) fn solid(sim: &mut Sim, min: Vec3, max: Vec3, surface: Option<&str>) -> Entity {
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

pub(super) fn near(a: Vec3, b: Vec3, eps: f32) -> bool {
    (a - b).abs().max_element() <= eps
}

#[track_caller]
pub(super) fn assert_near(a: Vec3, b: Vec3, eps: f32, what: &str) {
    assert!(near(a, b, eps), "{what}: {a} vs {b}");
}

#[track_caller]
pub(super) fn assert_hp(got: f32, want: f32, eps: f32, what: &str) {
    assert!((got - want).abs() <= eps, "{what}: {got} HP, expected {want}");
}

pub(super) fn booms(sim: &Sim) -> Vec<(u64, &'static str, Option<Entity>, Vec3)> {
    sim.app.world().resource::<Booms>().0.clone()
}

/// Run until something detonates (or `limit` seconds).
pub(super) fn until_boom(sim: &mut Sim, limit: f64) -> (u64, &'static str, Option<Entity>, Vec3) {
    let start = booms(sim).len();
    for _ in 0..((limit / TICK_INTERVAL) as u64 + 2) {
        sim.ticks(1);
        if let Some(b) = booms(sim).get(start) {
            return *b;
        }
    }
    panic!("no detonation in {limit} s");
}

pub(super) fn now(sim: &Sim) -> f64 {
    sim.app.world().resource::<SimClock>().now
}

pub(super) fn frame(sim: &Sim) -> u64 {
    sim.app.world().resource::<CaServerFrames>().frame
}

// --- Players

/// A Combat Arms player standing at `x`, `z` (u) facing `yaw` (pi looks
/// down +Z), 100 HP, no armour, on `team`.
pub(super) fn player(sim: &mut Sim, x: f32, z: f32, yaw: f32, team: u8) -> Entity {
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
pub(super) fn body(sim: &mut Sim, p: Entity) -> (Vec3, f32) {
    let world = sim.app.world_mut();
    let mut q = world.query::<(&Transform, &Intent, &MovementState, Option<&ColliderAabb>)>();
    let (t, i, s, a) = q.get(world, p).unwrap();
    let (min, max, spheres) = body_geometry(t, i, s, a, *MALE).unwrap();
    let top = MALE
        .iter()
        .zip(&spheres)
        .filter(|(s, _)| s.location == Location::Head)
        .map(|(_, (c, r))| c.y + r)
        .fold(f32::MIN, f32::max);
    ((min + max) / 2.0, top)
}

/// A player's hit spheres (m): centre, radius, location.
pub(super) fn spheres(sim: &mut Sim, p: Entity) -> Vec<(Vec3, f32, Location)> {
    let world = sim.app.world_mut();
    let mut q = world.query::<(&Transform, &Intent, &MovementState, Option<&ColliderAabb>)>();
    let (t, i, s, a) = q.get(world, p).unwrap();
    let (_, _, spheres) = body_geometry(t, i, s, a, *MALE).unwrap();
    MALE.iter().zip(spheres).map(|(d, (c, r))| (c, r, d.location)).collect()
}

pub(super) fn health(sim: &Sim, p: Entity) -> f32 {
    sim.app.world().get::<Health>(p).unwrap().current / HP
}

pub(super) fn set_health(sim: &mut Sim, p: Entity, hp: f32) {
    let mut h = sim.app.world_mut().get_mut::<Health>(p).unwrap();
    h.current = hp * HP;
    h.max = h.max.max(hp * HP);
}

/// A player out of every blast's reach, on `team`.
pub(super) fn far_player(sim: &mut Sim, team: u8) -> Entity {
    player(sim, 5000.0, 5000.0, 0.0, team)
}

/// Move a player (feet, u) and let the movement settle.
pub(super) fn put(sim: &mut Sim, p: Entity, x: f32, z: f32) {
    sim.app.world_mut().get_mut::<Transform>(p).unwrap().translation = Vec3::new(x * U, 1.0, z * U);
    sim.ticks(3);
}

// --- Objects

pub(super) fn weapon_of(sim: &mut Sim, id: &str, owner: Option<Entity>) -> Entity {
    weapon::spawn_weapon(sim.app.world_mut(), id, owner.unwrap_or(Entity::PLACEHOLDER)).unwrap()
}

/// An impact projectile of weapon `id` at `at` (u) with `v` (u/s).
pub(super) fn projectile(sim: &mut Sim, id: &str, owner: Option<Entity>, at: Vec3, v: Vec3) -> Entity {
    let w = weapon_of(sim, id, owner);
    let d = sim.app.world().get::<CaProjectileData>(w).unwrap().clone();
    let name = sim.app.world().get::<Weapon>(w).unwrap().id;
    let o = CaObject::new(name, owner, d.unit, d.hp, at, v, Kind::Impact(d.params));
    special::spawn_object(sim.app.world_mut(), o)
}

/// A projectile of `id` that goes off where it is on the next frame but
/// one (its lifetime 0).
pub(super) fn explode_at(sim: &mut Sim, id: &str, owner: Option<Entity>, at_m: Vec3) {
    let w = weapon_of(sim, id, owner);
    let d = sim.app.world().get::<CaProjectileData>(w).unwrap().clone();
    let name = sim.app.world().get::<Weapon>(w).unwrap().id;
    let mut p: ProjectileParams = (*d.params).clone();
    p.lifetime = 0.0;
    let o = CaObject::new(name, owner, d.unit, d.hp, at_m / U, Vec3::ZERO, Kind::Impact(Arc::new(p)));
    special::spawn_object(sim.app.world_mut(), o);
    until_boom(sim, 1.0);
    // Damage lands on the tick it is dealt.
    sim.ticks(1);
}

/// A device of weapon `id` (thrown by `owner`) at `at` (u) with `v` (u/s).
pub(super) fn device(sim: &mut Sim, id: &str, owner: Option<Entity>, at: Vec3, v: Vec3) -> Entity {
    let w = weapon_of(sim, id, owner);
    let d = sim.app.world().get::<CaDeviceData>(w).unwrap().clone();
    let name = sim.app.world().get::<Weapon>(w).unwrap().id;
    let mut o = CaObject::new(name, owner, d.unit, d.hp, at, v, Kind::Device(d.params));
    o.s.origin = at;
    special::spawn_object(sim.app.world_mut(), o)
}

pub(super) fn obj(sim: &Sim, e: Entity) -> CaObject {
    sim.app.world().get::<CaObject>(e).expect("object gone").clone()
}

pub(super) fn alive(sim: &Sim, e: Entity) -> bool {
    sim.app.world().get::<CaObject>(e).is_some()
}

/// Run until `e` has run `n` physics steps.
pub(super) fn to_step(sim: &mut Sim, e: Entity, n: u32) -> CaObject {
    for _ in 0..2000 {
        let s = obj(sim, e);
        if s.steps >= n {
            assert_eq!(s.steps, n, "stepped past {n}");
            return s;
        }
        sim.ticks(1);
    }
    panic!("no step {n}");
}

/// A device that has landed: dropped from just above `x`, `z` (u).
pub(super) fn landed(sim: &mut Sim, id: &str, owner: Option<Entity>, x: f32, z: f32) -> Entity {
    let e = device(sim, id, owner, Vec3::new(x, 12.0, z), Vec3::ZERO);
    for _ in 0..200 {
        sim.ticks(1);
        if obj(sim, e).s.landed {
            return e;
        }
    }
    panic!("never landed");
}

/// Wait `secs` of server time.
pub(super) fn wait(sim: &mut Sim, secs: f64) {
    sim.seconds(secs);
}

/// The point `d` u from the victim's centre in the direction it faces
/// (`ahead`) or behind it, at its centre's height.
pub(super) fn in_front(sim: &mut Sim, p: Entity, d: f32, ahead: bool) -> Vec3 {
    let (c, _) = body(sim, p);
    let yaw = sim.app.world().get::<Intent>(p).unwrap().yaw;
    let f = Quat::from_rotation_y(yaw) * Vec3::NEG_Z;
    c + f * d * U * if ahead { 1.0 } else { -1.0 }
}

pub(super) fn dist_u(a: Vec3, b: Vec3) -> f32 {
    a.distance(b) / U
}

/// The falloff of grenades.md 9.
pub(super) fn m(r: f32, d: f32) -> f32 {
    grenades::falloff(r, d)
}

/// A player holding `id`, settled.
pub(super) fn armed(sim: &mut Sim, p: Entity, id: &str) -> Entity {
    let w = weapon::give(sim.app.world_mut(), p, id).expect("unknown weapon");
    sim.seconds(3.0);
    assert_eq!(sim.app.world().get::<Inventory>(p).unwrap().active, Some(w), "{id} drawn");
    w
}

pub(super) fn eye(sim: &Sim, p: Entity) -> Vec3 {
    sim.position(p) + sim.state(p).eye_offset
}

pub(super) fn mag(sim: &Sim, w: Entity) -> Magazine {
    sim.app.world().get::<Magazine>(w).unwrap().clone()
}

/// Times (s) the weapon `w` fired a round at or after `from`.
pub(super) fn fired(sim: &Sim, w: Entity, from: f64) -> Vec<f64> {
    sim.app
        .world()
        .resource::<Rounds>()
        .0
        .iter()
        .filter(|(t, _, e, k)| *e == w && *t >= from - 1e-9 && matches!(k, WeaponEventKind::Fired { .. }))
        .map(|(t, ..)| *t)
        .collect()
}

pub(super) fn hits(sim: &Sim) -> Vec<(f64, Entity, Option<Entity>, f32, Option<&'static str>)> {
    sim.app.world().resource::<Hits>().0.clone()
}

// ---------------------------------------------------------------------------
// Rockets (2)

/// Case 1: a LAW fired level flies 65 u a step; a wall at z = 500 is met in
/// step 8 and the blast is 1 u back.
#[test]
fn t01_rocket_flight() {
    let mut sim = sim();
    let r = projectile(&mut sim, LAW, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1300.0));
    for k in [1u32, 2, 10, 50, 199] {
        let s = to_step(&mut sim, r, k);
        assert_near(s.pos, Vec3::new(0.0, 60.0, 65.0 * k as f32), 0.05, &format!("step {k}"));
    }
    let mut sim = self::sim();
    solid(&mut sim, Vec3::new(-10.0, -1.0, 500.0 * U), Vec3::new(10.0, 10.0, 510.0 * U), None);
    let r = projectile(&mut sim, LAW, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1300.0));
    let created = to_step(&mut sim, r, 1).created.unwrap();
    let (at_frame, weapon, _, at) = until_boom(&mut sim, 1.0);
    assert_eq!(weapon, LAW);
    assert_eq!(at_frame - created, 8, "contact in step 8");
    assert_near(at, Vec3::new(0.0, 60.0, 499.0), 0.05, "1 u back");
}

/// Case 2: the rocket's blast on a player is the double application of
/// 40 (LAW) or 43 (RPG-7), full to 137.5 u, linear to 550.
#[test]
fn t02_rocket_blast() {
    for (id, d, want) in [
        (LAW, 100.0, 80.0),
        (LAW, 137.5, 80.0),
        (LAW, 300.0, 48.48),
        (LAW, 500.0, 9.70),
        (LAW, 550.0, 0.0),
        (RPG, 100.0, 86.0),
        (RPG, 300.0, 52.12),
    ] {
        let mut sim = sim();
        let thrower = far_player(&mut sim, 0);
        let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
        let at = in_front(&mut sim, victim, d, true);
        explode_at(&mut sim, id, Some(thrower), at);
        assert_hp(100.0 - health(&sim, victim), want, 0.01, &format!("{id} at {d}"));
    }
}

/// Case 3: a LAW fired at a standing victim hits his hit sphere: the blast
/// is at the hit point, he takes 80 and survives with 20, and the cover
/// test is skipped for him (a clip slab through his body does not spare
/// him, while the same blast in the air next to him is stopped by it).
#[test]
fn t03_rocket_direct_hit() {
    let mut sim = sim();
    let shooter = far_player(&mut sim, 1);
    let victim = player(&mut sim, 0.0, 400.0, 0.0, 2);
    let sph = spheres(&mut sim, victim);
    let (c, r, _) = sph
        .iter()
        .filter(|s| s.2 == Location::Torso)
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .copied()
        .unwrap();
    // A clip slab through the chest sphere, 3 u in front of its centre.
    solid(
        &mut sim,
        Vec3::new(c.x - 3.0, 0.0, c.z - 3.0 * U - 0.5 * U),
        Vec3::new(c.x + 3.0, 4.0, c.z - 3.0 * U),
        Some("invisible"),
    );
    let start = Vec3::new(c.x / U, c.y / U, c.z / U - 400.0);
    projectile(&mut sim, LAW, Some(shooter), start, Vec3::new(0.0, 0.0, 1300.0));
    let (_, _, _, at) = until_boom(&mut sim, 2.0);
    sim.ticks(2);
    // On the chest sphere's surface, as the body stood a moment before
    // (it sways): within 10 cm of it.
    let d = (at * U).distance(c);
    assert!((d - r).abs() < 0.1, "the blast is at the hit point: {d} m from the chest centre, r {r}");
    assert_hp(health(&sim, victim), 20.0, 0.01, "80 and survives with 20");
    // The same blast in the air beside him, with the slab between: stopped.
    let mut sim = self::sim();
    let shooter = far_player(&mut sim, 1);
    let victim = player(&mut sim, 0.0, 400.0, 0.0, 2);
    let c = spheres(&mut sim, victim).iter().filter(|s| s.2 == Location::Torso).map(|s| s.0).next().unwrap();
    solid(
        &mut sim,
        Vec3::new(c.x - 3.0, 0.0, c.z - 3.0 * U - 0.5 * U),
        Vec3::new(c.x + 3.0, 4.0, c.z - 3.0 * U),
        Some("invisible"),
    );
    explode_at(&mut sim, LAW, Some(shooter), c - Vec3::new(0.0, 0.0, 8.0 * U));
    assert_eq!(health(&sim, victim), 100.0, "the cover test applies to a near miss");
}

/// Case 4: the M136_AT4's ammo has AreaDamage 0: no explosion, no damage,
/// directly or 50 u away; the impact effect is still sent.
#[test]
fn t04_at4_does_nothing() {
    let mut sim = sim();
    let shooter = far_player(&mut sim, 1);
    let victim = player(&mut sim, 0.0, 400.0, 0.0, 2);
    let c = spheres(&mut sim, victim).iter().filter(|s| s.2 == Location::Torso).map(|s| s.0).next().unwrap();
    projectile(
        &mut sim,
        AT4,
        Some(shooter),
        Vec3::new(c.x / U, c.y / U, c.z / U - 400.0),
        Vec3::new(0.0, 0.0, 1300.0),
    );
    let (_, weapon, _, _) = until_boom(&mut sim, 2.0);
    assert_eq!(weapon, AT4, "the impact effect");
    sim.ticks(3);
    assert_eq!(health(&sim, victim), 100.0, "a direct hit");
    let at = in_front(&mut sim, victim, 50.0, true);
    explode_at(&mut sim, AT4, Some(shooter), at);
    assert_eq!(health(&sim, victim), 100.0, "50 u away");
}

/// Case 5: a rocket at a sky polygon is removed in step 5: no blast, no
/// effect.
#[test]
fn t05_rocket_at_the_sky() {
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, -1.0, 300.0 * U), Vec3::new(10.0, 10.0, 310.0 * U), Some("sky"));
    let r = projectile(&mut sim, LAW, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1300.0));
    let mut steps = 0;
    for _ in 0..200 {
        sim.ticks(1);
        if !alive(&sim, r) {
            break;
        }
        steps = obj(&sim, r).steps;
    }
    assert!(!alive(&sim, r), "removed");
    assert_eq!(steps, 4, "it was in step 4 before the fifth removed it");
    sim.seconds(1.0);
    assert!(booms(&sim).is_empty(), "no effect");
}

/// Case 6: a LAW fired into empty space detonates in the air at the start
/// of the 200th frame at z = 12935.
#[test]
fn t06_rocket_lifetime() {
    let mut sim = sim();
    let r = projectile(&mut sim, LAW, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1300.0));
    let created = to_step(&mut sim, r, 1).created.unwrap();
    let (at_frame, _, _, at) = until_boom(&mut sim, 12.0);
    assert_eq!(at_frame - created, 200, "the 200th frame");
    assert_near(at, Vec3::new(0.0, 60.0, 12935.0), 0.05, "in the air");
}

/// Case 7: a rocket takes bullets (50 HP): the second 35 detonates it at
/// its position, credited to the rocket's shooter.
#[test]
fn t07_shooting_a_rocket() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 2);
    armed(&mut sim, shooter, RIFLE);
    let e = eye(&sim, shooter);
    let at = Vec3::new(e.x / U, e.y / U, e.z / U + 300.0);
    let r = projectile(&mut sim, LAW, Some(owner), at, Vec3::ZERO);
    sim.ticks(2);
    sim.intent(shooter).fire = true;
    let mut seen = 0;
    for _ in 0..200 {
        sim.ticks(1);
        let n = hits(&sim).iter().filter(|h| h.1 == r).count();
        if n > seen {
            seen = n;
            if n == 1 {
                // 35 of 50: still there.
                sim.ticks(2);
                assert!(alive(&sim, r), "one round does not destroy it");
            }
        }
        if n >= 2 {
            break;
        }
    }
    sim.intent(shooter).fire = false;
    assert!(seen >= 2, "two rounds hit it");
    let (_, weapon, thrower, boom) = until_boom(&mut sim, 1.0);
    assert_eq!((weapon, thrower), (LAW, Some(owner)), "credited to the rocket's shooter");
    assert_near(boom, at, 0.05, "where it was");
}

// ---------------------------------------------------------------------------
// Launcher shells (3)

/// Case 8: an M32 shell fired level: steps 1-5 as grenades.md case 1; step
/// 6 touches the floor at z = 442.857 with post-gravity velocity (0, -480,
/// 1550); the blast is 1 u back along it; a victim 163 u away takes 48.71.
#[test]
fn t08_launcher_shell_level() {
    let mut sim = sim();
    let shell = projectile(&mut sim, M32, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let s = to_step(&mut sim, shell, 1);
    assert_near(s.pos, Vec3::new(0.0, 59.0, 77.5), 0.05, "step 1");
    let s = to_step(&mut sim, shell, 5);
    assert_near(s.pos, Vec3::new(0.0, 15.0, 387.5), 0.05, "step 5");
    // Step 6 touches the floor at (0, 0, 442.857) with the post-gravity
    // velocity (0, -480, 1550): the blast is 1 u back along it.
    let created = obj(&sim, shell).created.unwrap();
    let (f, _, _, at) = until_boom(&mut sim, 1.0);
    assert_eq!(f - created, 6, "step 6");
    assert_near(at, Vec3::new(0.0, 0.296, 441.902), 0.05, "1 u back");

    // A victim: the number from the distance measured.
    let mut sim = self::sim();
    let thrower = far_player(&mut sim, 0);
    let victim = player(&mut sim, 0.0, 600.0, 0.0, 0);
    let (c, _) = body(&mut sim, victim);
    projectile(&mut sim, M32, Some(thrower), Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 1550.0));
    let (_, _, _, at) = until_boom(&mut sim, 2.0);
    sim.ticks(2);
    let d = dist_u(c, at * U);
    assert!((d - 163.0).abs() < 4.0, "the case's 163 u from the blast: {d}");
    assert_hp(100.0 - health(&sim, victim), 2.0 * 40.0 * m(300.0, d), 0.01, "the double blast");
}

/// Case 9: launcher blasts by weapon: M32 and M79 40, MZP_1 42, XM_25 22,
/// radius 300, applied twice.
#[test]
fn t09_launcher_blast() {
    for (id, d, want) in [
        (M32, 75.0, 80.0),
        (M32, 100.0, 71.11),
        (M32, 200.0, 35.56),
        (M32, 300.0, 0.0),
        (XM25, 100.0, 39.11),
        (MZP1, 50.0, 84.0),
    ] {
        let mut sim = sim();
        let thrower = far_player(&mut sim, 0);
        let victim = player(&mut sim, 0.0, 0.0, 0.0, 0);
        let at = in_front(&mut sim, victim, d, true);
        explode_at(&mut sim, id, Some(thrower), at);
        assert_hp(100.0 - health(&sim, victim), want, 0.01, &format!("{id} at {d}"));
    }
}

/// Case 10: no arming distance: a shell with a wall 30 u ahead goes off in
/// step 1, 1 u back along the flight, and the shooter takes 80.
#[test]
fn t10_no_arming_distance() {
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, -1.0, 30.0 * U), Vec3::new(10.0, 10.0, 40.0 * U), None);
    let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    let e = eye(&sim, shooter);
    let at = Vec3::new(e.x / U, e.y / U, e.z / U);
    let _shell = projectile(&mut sim, M32, Some(shooter), at, Vec3::new(0.0, 0.0, 1550.0));
    let (f, _, _, boom) = until_boom(&mut sim, 1.0);
    sim.ticks(2);
    // Contact (0, y - 0.39, 30), the post-gravity direction (0, -80, 1550).
    let v = Vec3::new(0.0, -80.0, 1550.0).normalize();
    let q = Vec3::new(at.x, at.y - 30.0 / 77.5, 30.0 + at.z - at.z);
    assert_near(boom, q - v, 0.1, "1 u back");
    let _ = f;
    assert_hp(100.0 - health(&sim, shooter), 80.0, 0.01, "the shooter takes the double blast");
}

/// Case 11: an M32 on hold fires a shell a second; the empty clip reloads
/// when the fire animation ends: 2100 ms, a shell a second, 1034 ms.
/// (The case lists the shells at 8100, 9100, ... and the end at 14134;
/// section 3.2 and case 12 have each shell complete a second after the
/// pre-reload, the reload ending at 2100 + 6000 + 1034: followed.)
#[test]
fn t11_m32_automatic_fire() {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    let w = armed(&mut sim, p, M32);
    assert_eq!((mag(&sim, w).clip, mag(&sim, w).reserve), (6, 6));
    let t0 = now(&sim);
    sim.intent(p).fire = true;
    let mut clips: Vec<(f64, u32)> = Vec::new();
    let mut last = 6;
    for _ in 0..1400 {
        sim.ticks(1);
        let c = mag(&sim, w).clip;
        if c != last {
            clips.push((now(&sim) - t0, c));
            last = c;
        }
        if now(&sim) - t0 > 5.5 {
            sim.intent(p).fire = false;
        }
        if now(&sim) - t0 > 17.0 {
            break;
        }
    }
    let shots: Vec<f64> = fired(&sim, w, t0).into_iter().map(|t| t - t0).collect();
    assert_eq!(shots.len(), 6, "{shots:?}");
    for (k, t) in shots.iter().enumerate() {
        assert!((t - k as f64).abs() <= 0.035, "shell {k} at {t}");
    }
    assert_eq!(clips[4].1, 1);
    assert_eq!(clips[5].1, 0);
    assert!((clips[5].0 - 5.0).abs() <= 0.035, "clip 0 at {}", clips[5].0);
    // The reload: begins at 6.0, the shells complete 3.1, 4.1, ... 8.1 s
    // into it, the last part ends at 9.134 s.
    let loads: Vec<f64> = clips[6..].iter().map(|c| c.0).collect();
    assert_eq!(loads.len(), 6, "{clips:?}");
    for (k, t) in loads.iter().enumerate() {
        let want = 6.0 + 2.1 + 1.0 * (k as f64 + 1.0);
        assert!((t - want).abs() <= 0.05, "shell {k} loaded at {t}, expected {want}");
    }
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.reserve), (6, 0), "clip 6, total 6");
}

/// Case 12: a reload interrupted by fire: clip 2, total 12; shells complete
/// at 3100, 4100; fire at 4000 ms stops the reload with clip 3 (the shell
/// under way is lost) and one shell leaves at once: clip 2, total 11.
#[test]
fn t12_m32_reload_interrupted() {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    let w = armed(&mut sim, p, M32);
    {
        let mut m = sim.app.world_mut().get_mut::<Magazine>(w).unwrap();
        m.clip = 2;
        m.reserve = 10;
    }
    let t0 = now(&sim);
    sim.intent(p).reload = true;
    sim.ticks(1);
    sim.intent(p).reload = false;
    while now(&sim) - t0 < 3.99 {
        sim.ticks(1);
    }
    assert_eq!(mag(&sim, w).clip, 3, "one shell in at 3100");
    sim.intent(p).fire = true;
    sim.ticks(2);
    sim.intent(p).fire = false;
    let shots = fired(&sim, w, t0);
    assert_eq!(shots.len(), 1, "one shell left at once");
    assert!(shots[0] - t0 < 4.1, "at 4000 ms: {}", shots[0] - t0);
    let m = mag(&sim, w);
    assert_eq!((m.clip, m.clip + m.reserve), (2, 11), "clip 2, total 11");
    sim.seconds(1.0);
    assert_eq!(mag(&sim, w).clip, 2, "the reload did not go on (the lost shell)");
}

/// Case 13: an incendiary shell on the floor 100 u from a victim: no
/// instant damage; 10 HP/s while he is inside the 200 u sphere (9 s) and 6
/// s after; the shooter inside burns too.
#[test]
fn t13_incendiary_shell() {
    let mut sim = sim();
    let shooter = player(&mut sim, 0.0, 0.0, 0.0, 1);
    let victim = player(&mut sim, 0.0, 200.0, 0.0, 2);
    let (vc, _) = body(&mut sim, victim);
    let at = Vec3::new(vc.x, 0.0, vc.z - 100.0 * U);
    explode_at(&mut sim, M32_INCENDIARY, Some(shooter), at);
    assert_eq!(health(&sim, victim), 100.0, "no instant damage");
    let t0 = now(&sim);
    let mut at3 = 0.0;
    let mut at5 = 0.0;
    let mut dead_at = None;
    let mut shooter_at3 = 0.0;
    for _ in 0..1500 {
        sim.ticks(1);
        let t = now(&sim) - t0;
        if at3 == 0.0 && t >= 3.0 {
            at3 = health(&sim, victim);
            shooter_at3 = health(&sim, shooter);
        }
        if at5 == 0.0 && t >= 5.0 {
            at5 = health(&sim, victim);
        }
        if dead_at.is_none() && health(&sim, victim) <= 0.0 {
            dead_at = Some(t);
            break;
        }
    }
    assert_hp(at3, 70.0, 1.5, "10 HP/s at 3 s");
    assert_hp(at5, 50.0, 1.5, "at 5 s");
    let dead = dead_at.expect("dead");
    assert!((dead - 10.0).abs() < 0.6, "dead at about 10 s: {dead}");
    assert!(shooter_at3 < 80.0, "the shooter in the sphere burns too: {shooter_at3}");
}

// ---------------------------------------------------------------------------
// The flamethrower (4)

/// Case 14: fire held from t = 0: puffs at 200, 300, ... 900 ms, the next
/// loop from 934 ms (934, 1034, ...); 300 puffs, then the clip is 0.
#[test]
fn t14_flamethrower_puffs() {
    let mut sim = sim();
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 0);
    let w = armed(&mut sim, p, FLAME);
    let t0 = now(&sim) + TICK_INTERVAL;
    sim.intent(p).fire = true;
    sim.seconds(30.0);
    sim.intent(p).fire = false;
    let shots: Vec<f64> = fired(&sim, w, t0 - 0.01).into_iter().map(|t| t - t0).collect();
    assert_eq!(shots.len(), 300, "300 puffs");
    let tick = TICK_INTERVAL + 1e-6;
    for (k, want) in [0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 0.934, 1.034, 1.134].iter().enumerate() {
        assert!((shots[k] - want).abs() <= tick, "puff {k} at {}, expected {want}", shots[k]);
    }
    assert_eq!(mag(&sim, w).clip, 0, "the clip is empty");
}

/// A flame puff of `FLAME` that flies `dist` u level and goes off at the
/// end of its life: `end` (u) is where, `owner` its shooter.
pub(super) fn puff_to(sim: &mut Sim, owner: Option<Entity>, end: Vec3) -> Entity {
    projectile(sim, FLAME, owner, end - Vec3::new(0.0, 0.0, 475.0), Vec3::new(0.0, 0.0, 500.0))
}

/// Case 15: a puff fired level into empty space goes off at the start of
/// frame 20 at z = 475; a sphere of 75 u lives 2 s; a victim 50 u from it
/// for the whole time takes 12 x 3 = 36, one 80 u away nothing.
#[test]
fn t15_flame_puff_in_the_air() {
    let mut sim = sim();
    let shooter = far_player(&mut sim, 1);
    let near_v = player(&mut sim, 0.0, 0.0, 0.0, 2);
    let far_v = player(&mut sim, 400.0, 0.0, 0.0, 2);
    let (c1, _) = body(&mut sim, near_v);
    let (c2, _) = body(&mut sim, far_v);
    let end1 = Vec3::new(c1.x / U + 50.0, c1.y / U, c1.z / U);
    let end2 = Vec3::new(c2.x / U + 80.0, c2.y / U, c2.z / U);
    let a = puff_to(&mut sim, Some(shooter), end1);
    let created = to_step(&mut sim, a, 1).created.unwrap();
    puff_to(&mut sim, Some(shooter), end2);
    let (at_frame, _, _, at) = until_boom(&mut sim, 2.0);
    assert_eq!(at_frame - created, 20, "frame 20");
    assert_near(at, end1, 0.05, "475 u from the muzzle");
    wait(&mut sim, 4.0);
    assert_hp(100.0 - health(&sim, near_v), 36.0, 1.5, "12 x (2 + 1) at 50 u");
    assert_eq!(health(&sim, far_v), 100.0, "80 u: outside the 75 u sphere");
    assert_eq!(sim.app.world_mut().query::<&CaDamageSphere>().iter(sim.app.world()).count(), 0, "the spheres are gone");
}

/// The stream as a player meets it: fire held at a victim standing close
/// (a puff that touches him goes off on him, 1.3), in mid range and at the
/// puff's end (475 u, section 4's reach): each one burns. Backlog: "doesn't
/// seem to do damage to anybody".
#[test]
fn the_stream_burns_close_at_mid_range_and_at_the_puffs_end() {
    for distance in [60.0, 240.0, 470.0] {
        let mut sim = sim();
        let shooter = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
        let victim = player(&mut sim, 0.0, distance, 0.0, 2);
        armed(&mut sim, shooter, FLAME);
        // At his chest (a level line from the eye passes over a standing
        // head: a puff touches only by a hit sphere on its line, 1.2).
        let (chest, _) = body(&mut sim, victim);
        let from = eye(&sim, shooter);
        sim.intent(shooter).pitch = (chest.y - from.y).atan2(chest.z - from.z);
        sim.intent(shooter).fire = true;
        sim.seconds(1.5);
        sim.intent(shooter).fire = false;
        wait(&mut sim, 1.0);
        let lost = 100.0 - health(&sim, victim);
        assert!(lost > 10.0, "at {distance} u the victim lost {lost} HP");
        eprintln!("at {distance} u: {lost:.1} HP in 2.5 s");
    }
}

/// Case 16: spheres of three puffs at once stack: 36 HP in that second.
#[test]
fn t16_flame_stacking() {
    let mut sim = sim();
    let shooter = far_player(&mut sim, 1);
    let victim = player(&mut sim, 0.0, 0.0, 0.0, 2);
    set_health(&mut sim, victim, 1000.0);
    let (c, _) = body(&mut sim, victim);
    let end = Vec3::new(c.x / U + 20.0, c.y / U, c.z / U);
    for _ in 0..3 {
        puff_to(&mut sim, Some(shooter), end);
    }
    until_boom(&mut sim, 2.0);
    let t0 = now(&sim);
    let before = health(&sim, victim);
    while now(&sim) - t0 < 1.0 {
        sim.ticks(1);
    }
    assert_hp(before - health(&sim, victim), 36.0, 2.0, "three entries in a second");
    let entries = sim
        .app
        .world()
        .resource::<CaGrenadeEffects>()
        .over_time
        .iter()
        .filter(|o| o.victim == victim && o.kind == OverTimeKind::Burn)
        .count();
    assert_eq!(entries, 3, "one entry per sphere");
}

/// Case 17: the shooter in his own spheres takes nothing; a teammate takes
/// nothing with friendly fire off and the full burn with it on.
#[test]
fn t17_flame_self_and_team() {
    let mut sim = sim();
    let shooter = player(&mut sim, 0.0, 0.0, 0.0, 1);
    let (c, _) = body(&mut sim, shooter);
    puff_to(&mut sim, Some(shooter), Vec3::new(c.x / U + 20.0, c.y / U, c.z / U));
    wait(&mut sim, 4.0);
    assert_eq!(health(&sim, shooter), 100.0, "his own flames");
    for (ff, want) in [(0, 0.0), (1, 36.0)] {
        let mut sim = self::sim();
        sim.app.insert_resource(FriendlyFire(ff));
        let shooter = far_player(&mut sim, 1);
        let mate = player(&mut sim, 0.0, 0.0, 0.0, 1);
        let (c, _) = body(&mut sim, mate);
        puff_to(&mut sim, Some(shooter), Vec3::new(c.x / U + 20.0, c.y / U, c.z / U));
        wait(&mut sim, 4.0);
        assert_hp(100.0 - health(&sim, mate), want, 1.5, &format!("friendly fire {ff}"));
    }
}

// ---------------------------------------------------------------------------
// Mines and devices (6)

/// Case 18: a mine thrown level lands in the step that crosses y = 0
/// (z = 171.43); an enemy on it sets it off: `active` 267 ms, then the blast
/// (80 within 137.5 u, falling off to 550); the owner or a teammate nothing.
#[test]
fn t18_land_mine() {
    let mut sim = sim();
    let owner = far_player(&mut sim, 1);
    let enemy = player(&mut sim, 0.0, 171.43, 0.0, 2);
    let far = player(&mut sim, 298.0, 171.43, 0.0, 2);
    set_health(&mut sim, far, 1000.0);
    let mine = device(&mut sim, MINE, Some(owner), Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 600.0));
    let mut trigger = None;
    let mut landed_at = None;
    let (frame_of_boom, _, thrower, at) = loop {
        sim.ticks(1);
        if let Some(b) = booms(&sim).first() {
            break *b;
        }
        let o = obj(&sim, mine);
        if o.s.landed && landed_at.is_none() {
            landed_at = Some(o.s.landed_at);
            assert_near(o.pos, Vec3::new(0.0, 6.6, 171.43), 0.05, "landed in step 6");
            assert_eq!(o.steps, 6);
        }
        if trigger.is_none() {
            trigger = o.s.triggered_at;
        }
    };
    let t = trigger.expect("set off by the enemy on it");
    let blast_time = frame_of_boom as f64 * 0.05;
    assert!(blast_time >= t + 0.267 - 1e-6 && blast_time <= t + 0.267 + 0.05 + 1e-6, "blast at {blast_time}, set off at {t}");
    assert_eq!(thrower, Some(owner));
    sim.ticks(2);
    assert_hp(100.0 - health(&sim, enemy), 80.0, 0.01, "on the mine");
    let (c, _) = body(&mut sim, far);
    let d = dist_u(c, at * U);
    assert!((d - 300.0).abs() < 4.0, "about 300 u: {d}");
    assert_hp(1000.0 - health(&sim, far), 2.0 * 40.0 * m(550.0, d), 0.01, "at the distance measured");

    // The owner or a teammate walking over it does nothing.
    let mut sim = self::sim();
    let owner = player(&mut sim, 0.0, 171.43, 0.0, 1);
    let mate = player(&mut sim, 30.0, 171.43, 0.0, 1);
    let mine = device(&mut sim, MINE, Some(owner), Vec3::new(15.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 600.0));
    wait(&mut sim, 2.0);
    assert!(alive(&sim, mine) && obj(&sim, mine).s.landed, "still there");
    assert!(obj(&sim, mine).s.triggered_at.is_none(), "not set off");
    assert_eq!((health(&sim, owner), health(&sim, mate)), (100.0, 100.0));
}

/// Case 19: a mine thrown at a wall 100 u away doesn't land on it; it
/// slides down and lands on the floor below.
#[test]
fn t19_mine_against_a_wall() {
    let mut sim = sim();
    solid(&mut sim, Vec3::new(-10.0, -1.0, 100.0 * U), Vec3::new(10.0, 10.0, 110.0 * U), None);
    let mine = device(&mut sim, MINE, None, Vec3::new(0.0, 60.0, 0.0), Vec3::new(0.0, 0.0, 600.0));
    let mut hit_wall = false;
    for _ in 0..400 {
        sim.ticks(1);
        let o = obj(&sim, mine);
        if o.pos.z > 99.0 && !o.s.landed {
            hit_wall = true;
        }
        if o.s.landed {
            break;
        }
    }
    let o = obj(&sim, mine);
    assert!(hit_wall, "it reached the wall without landing");
    assert!(o.s.landed, "landed on the floor");
    assert!(o.pos.z > 98.0 && o.pos.z < 100.0 + 1e-3, "at the foot of the wall: {}", o.pos.z);
    assert_near(Vec3::new(0.0, o.pos.y, 0.0), Vec3::new(0.0, 6.6, 0.0), 0.05, "on the floor");
}

// ---------------------------------------------------------------------------
// Flames and melee on a map's breakables (ours: another game's glass)

/// A wall that takes damage across `z` u ahead.
fn breakable_wall(sim: &mut Sim, z: f32) -> Entity {
    let e = solid(sim, Vec3::new(-5.0, 0.0, z * U), Vec3::new(5.0, 3.0, z * U + 0.1), None);
    sim.app.world_mut().entity_mut(e).insert(chimera_core::core::Damageable);
    sim.ticks(2);
    e
}

/// The Damage kinds `target` took.
fn hit_kinds(sim: &mut Sim, target: Entity) -> Vec<chimera_core::core::DamageKind> {
    sim.app.world().resource::<KindLog>().0.iter().filter(|(t, _)| *t == target).map(|(_, k)| *k).collect()
}

#[derive(Resource, Default)]
struct KindLog(Vec<(Entity, chimera_core::core::DamageKind)>);

fn log_kinds(mut damage: MessageReader<Damage>, mut log: ResMut<KindLog>) {
    for d in damage.read() {
        log.0.push((d.target, d.kind));
    }
}

fn with_kinds(sim: &mut Sim) {
    sim.app.init_resource::<KindLog>().add_systems(FixedPostUpdate, log_kinds);
}

/// A melee weapon breaks what a CS:S knife breaks: its swing's damage, as a
/// melee hit, reaches a map's breakable.
#[test]
fn a_swing_hits_a_breakable() {
    let mut sim = sim();
    with_kinds(&mut sim);
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
    armed(&mut sim, p, "test:machete_exact");
    let wall = breakable_wall(&mut sim, 60.0);
    sim.intent(p).fire = true;
    sim.seconds(1.0);
    sim.intent(p).fire = false;
    assert!(hit_kinds(&mut sim, wall).contains(&chimera_core::core::DamageKind::Melee), "{:?}", hit_kinds(&mut sim, wall));
}

/// A flame puff that touches a breakable is a melee-kind hit there (a pane
/// shatters: our rule), and the puff still goes off.
#[test]
fn a_flame_puff_hits_a_breakable() {
    let mut sim = sim();
    with_kinds(&mut sim);
    let p = player(&mut sim, 0.0, 0.0, std::f32::consts::PI, 1);
    armed(&mut sim, p, FLAME);
    let wall = breakable_wall(&mut sim, 200.0);
    sim.intent(p).fire = true;
    sim.seconds(1.5);
    sim.intent(p).fire = false;
    let kinds = hit_kinds(&mut sim, wall);
    assert!(kinds.contains(&chimera_core::core::DamageKind::Melee), "{kinds:?}");
    let spheres = sim.app.world_mut().query::<&CaDamageSphere>().iter(sim.app.world()).count();
    assert!(spheres > 0, "the puffs went off at the wall");
}
