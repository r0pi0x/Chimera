//! The implementer test cases of specs/hl2dm/suit.md (H2), numbered as
//! in the spec ("test N"): the armour split and the health remainder on
//! the suit's coverage (`suit_armour`, through Chimera's shared armour
//! step: lift C2), and on `Hl2Sim` the aux power, breath and drowning,
//! the flashlight, the suit zoom, pickups and wall chargers, and spawning.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{BodyPart, Damage, DamageKind, DamageRemainder, Dead, Health, Hitgroup, Intent, MovementState, Velocity};
use chimera_sim::weapon::{Armor, ViewZoom, hits};
use chimera_world::{
    harness::Sim,
    map::{MapBrush, MapBrushes, MapEntities, MapEntity, MapWater, MapWaterVolume, PlaySound},
};
use hl2dm::{
    game::{
        movement::ID,
        suit::{self, Flashlight, FlashlightAllowed, FlashlightOn, Suit, ZoomMove},
    },
    harness::Hl2Sim,
};
use source_engine::source_movement::{SourceMovement, to_engine, to_source};

const UNIT: f32 = 0.0254;

/// One hit of `raw` points of `class` on the suit: (health lost, armour
/// left), armour and the remainder carried in `armour` and `r`.
fn hit(raw: f32, class: hits::DamageClass, armour: &mut f32, r: &mut DamageRemainder) -> f32 {
    let pieces = suit::suit_armour();
    let entries: Vec<&hits::Coverage> = pieces.pieces.iter().flat_map(|p| p.entries.iter()).collect();
    let a = hits::absorb_quanta(
        raw / 100.0,
        BodyPart::Torso,
        class,
        Some(1.0),
        (0.0, Some(0.01)),
        *armour / 100.0,
        entries,
        hits::Depletion::Cost,
    );
    *armour = ((*armour / 100.0 - a.taken) * 100.0).round();
    r.take(a.to_health * 100.0).unwrap_or(0.0)
}

/// Tests 1-5: one hit on armour.
#[test]
fn the_armour_split() {
    for (armour, raw, class, health_lost, left) in [
        (100.0, 30.0, hits::DamageClass::BULLET, 6.0, 76.0),
        (100.0, 30.0, hits::DamageClass::BLAST, 6.0, 52.0),
        (10.0, 30.0, hits::DamageClass::BULLET, 20.0, 0.0),
        (10.0, 30.0, hits::DamageClass::BLAST, 25.0, 0.0),
        (24.0, 30.0, hits::DamageClass::BULLET, 6.0, 0.0),
    ] {
        let (mut a, mut r) = (armour, DamageRemainder::default());
        let lost = hit(raw, class, &mut a, &mut r);
        assert_eq!((lost, a), (health_lost, left), "{raw} {class:?} on {armour}");
    }
}

/// Tests 6-9, 14: whole armour points lost, the health's fraction carried.
/// Armour pays its cost truncated to whole points (the real server, probe
/// 2026-11-03: 8 bullet damage costs 6 of 100, not the spec's 7).
#[test]
fn uneven_health_steps() {
    // Test 6: four 8-damage bullets on 100 armour.
    let (mut a, mut r, mut health) = (100.0, DamageRemainder::default(), 100.0);
    let mut seen = Vec::new();
    for _ in 0..4 {
        health -= hit(8.0, hits::DamageClass::BULLET, &mut a, &mut r);
        seen.push((health, a, (r.0 * 10.0).round() / 10.0));
    }
    assert_eq!(seen, [(99.0, 94.0, 0.6), (97.0, 88.0, 0.2), (96.0, 82.0, 0.8), (94.0, 76.0, 0.4)]);

    // Test 7: five 1-damage hits on 50 armour.
    let (mut a, mut r, mut health) = (50.0, DamageRemainder::default(), 100.0);
    let mut seen = Vec::new();
    for _ in 0..5 {
        health -= hit(1.0, hits::DamageClass::BULLET, &mut a, &mut r);
        seen.push((health, a));
    }
    assert_eq!(seen, [(100.0, 49.0), (100.0, 48.0), (100.0, 47.0), (100.0, 46.0), (99.0, 45.0)]);

    // Test 8: a pistol head shot (24): 19.2 paid as 19 (probe).
    let (mut a, mut r) = (100.0, DamageRemainder::default());
    assert_eq!((hit(24.0, hits::DamageClass::BULLET, &mut a, &mut r), a), (4.0, 81.0));
    // Test 9: seven 9-damage pellets as one hit of 63: 50.4 paid as 50 (probe).
    let (mut a, mut r) = (100.0, DamageRemainder::default());
    assert_eq!((hit(63.0, hits::DamageClass::BULLET, &mut a, &mut r), a), (12.0, 50.0));
    // Test 14: 0.8 carried from the last life completes a point.
    let (mut a, mut r) = (50.0, DamageRemainder(0.8));
    assert_eq!(hit(1.0, hits::DamageClass::BULLET, &mut a, &mut r), 1.0);
    assert!(r.0 < 1e-3);
}

/// Tests 10-11: what bypasses the suit, and generic damage.
#[test]
fn what_the_suit_absorbs() {
    for class in [
        hits::DamageClass::FALL,
        hits::DamageClass::DROWN,
        hits::DamageClass::RADIATION,
        hits::DamageClass::POISON,
    ] {
        let (mut a, mut r) = (50.0, DamageRemainder::default());
        assert_eq!((hit(10.0, class, &mut a, &mut r), a), (10.0, 50.0), "{class:?}");
    }
    let (mut a, mut r) = (100.0, DamageRemainder::default());
    assert_eq!((hit(10.0, hits::DamageClass::GENERIC, &mut a, &mut r), a), (2.0, 92.0), "test 11");
    // Chimera's damage kinds: falls and drowning are their own classes.
    assert_eq!(hits::DamageClass::of(DamageKind::Fall), hits::DamageClass::FALL);
    assert_eq!(hits::DamageClass::of(DamageKind::Drown), hits::DamageClass::DROWN);
}

/// Tests 32-34: the zoom's smooth steps.
#[test]
fn zoom_times() {
    let z = ZoomMove {
        from: 75.0,
        to: 25.0,
        since: 0.0,
        time: suit::ZOOM_IN,
    };
    for (t, fov) in [(0.1, 67.1875), (0.2, 50.0), (0.3, 32.8125), (0.4, 25.0)] {
        assert!((z.fov(t) - fov).abs() < 1e-3, "test 32 at {t}");
    }
    let out = ZoomMove {
        from: 25.0,
        to: 75.0,
        since: 1.0,
        time: suit::ZOOM_OUT,
    };
    assert!((out.fov(1.1) - 50.0).abs() < 1e-3 && (out.fov(1.2) - 75.0).abs() < 1e-3, "test 33");
    let half = ZoomMove {
        from: 50.0,
        to: 75.0,
        since: 0.0,
        time: suit::ZOOM_OUT,
    };
    assert!((half.fov(0.1) - 62.5).abs() < 1e-3, "test 34");
}

// The simulation's tests: a floor at z = 0, a deep pool at x -1500 (water
// up to 200 over a floor at -200), items and chargers from `entities`.

const POOL: Vec3 = Vec3::new(-1500.0, 0.0, 0.0);
const FIELD: Vec3 = Vec3::new(2000.0, 0.0, 0.0);

fn brush(lo: Vec3, hi: Vec3) -> MapBrush {
    let (a, b) = (to_engine(lo), to_engine(hi));
    MapBrush {
        surface: Some("concrete".into()),
        ..MapBrush::from_box(a.min(b), a.max(b))
    }
}

#[derive(Resource, Clone)]
struct Entities(Vec<MapEntity>);

fn test_map(mut commands: Commands, entities: Res<Entities>) {
    commands.insert_resource(MapBrushes(vec![
        brush(Vec3::new(-8192.0, -8192.0, -64.0), Vec3::new(-2000.0, 8192.0, 0.0)),
        brush(Vec3::new(-1000.0, -8192.0, -64.0), Vec3::new(8192.0, 8192.0, 0.0)),
        brush(Vec3::new(-2000.0, -8192.0, -264.0), Vec3::new(-1000.0, 8192.0, -200.0)),
    ]));
    commands.insert_resource(MapWater(vec![MapWaterVolume {
        brush: brush(Vec3::new(-2000.0, -8192.0, -200.0), Vec3::new(-1000.0, 8192.0, 200.0)),
        slime: false,
    }]));
    commands.insert_resource(MapEntities {
        entities: Arc::new(entities.0.clone()),
        scale: UNIT,
    });
}

struct TestMap(Vec<MapEntity>);
impl Plugin for TestMap {
    fn build(&self, app: &mut App) {
        app.insert_resource(Entities(self.0.clone())).add_systems(Startup, test_map);
    }
}

fn entity(class: &str, at: Vec3, more: &[(&str, &str)]) -> MapEntity {
    let mut keyvalues = vec![
        ("classname".to_string(), class.to_string()),
        ("origin".to_string(), format!("{} {} {}", at.x, at.y, at.z)),
    ];
    keyvalues.extend(more.iter().map(|(k, v)| (k.to_string(), v.to_string())));
    MapEntity {
        keyvalues,
        ..Default::default()
    }
}

#[derive(Resource, Default)]
struct Heard {
    tick: u64,
    sounds: Vec<(u64, String)>,
}

/// One-shot sounds and the long ones started (a charger's loop).
fn listen(
    mut heard: ResMut<Heard>,
    mut sounds: MessageReader<PlaySound>,
    mut long: MessageReader<chimera_world::map::SoundControl>,
) {
    let tick = heard.tick;
    heard.sounds.extend(sounds.read().map(|s| (tick, s.entry.clone())));
    for c in long.read() {
        if let chimera_world::map::SoundControl::Start(s) = c {
            heard.sounds.push((tick, s.entry.clone()));
        }
    }
}

fn count(mut heard: ResMut<Heard>) {
    heard.tick += 1;
}

struct Player {
    sim: Sim,
    p: Entity,
}

impl Player {
    fn at(feet: Vec3, entities: Vec<MapEntity>) -> Self {
        let mut sim = Sim::new(Hl2Sim, TestMap(entities));
        // One fixed tick per update, at HL2:DM's 0.015 s.
        sim.set_tick_interval(0.015);
        sim.app.init_resource::<Heard>().add_systems(FixedFirst, count).add_systems(FixedLast, listen);
        let p = sim.spawn_character(to_engine(feet + Vec3::Z * 36.0), ID);
        let mut me = Self { sim, p };
        me.look(0.0, 0.0);
        me.sim.ticks(1);
        me.place(feet);
        me.sim.ticks(2);
        me.place(feet);
        me.clear();
        me
    }

    fn place(&mut self, feet: Vec3) {
        let at = to_engine(feet + Vec3::Z * 36.0);
        let w = self.sim.app.world_mut();
        w.get_mut::<Transform>(self.p).unwrap().translation = at;
        if let Some(mut pos) = w.get_mut::<Position>(self.p) {
            pos.0 = at;
        }
        w.get_mut::<Velocity>(self.p).unwrap().0 = Vec3::ZERO;
    }

    fn look(&mut self, yaw: f32, pitch: f32) {
        let mut i = self.sim.intent(self.p);
        i.yaw = yaw.to_radians() - std::f32::consts::FRAC_PI_2;
        i.pitch = -pitch.to_radians();
    }

    fn intent(&mut self) -> Mut<'_, Intent> {
        self.sim.intent(self.p)
    }

    fn suit(&self) -> Suit {
        self.sim.app.world().get::<Suit>(self.p).unwrap().clone()
    }

    fn suit_mut(&mut self) -> Mut<'_, Suit> {
        self.sim.app.world_mut().get_mut::<Suit>(self.p).unwrap()
    }

    fn health(&self) -> f32 {
        (self.sim.app.world().get::<Health>(self.p).unwrap().current * 100.0).round()
    }

    fn set_health(&mut self, points: f32) {
        self.sim.app.world_mut().get_mut::<Health>(self.p).unwrap().current = points / 100.0;
    }

    fn armour(&self) -> f32 {
        (self.sim.app.world().get::<Armor>(self.p).unwrap().amount * 100.0).round()
    }

    fn set_armour(&mut self, points: f32) {
        self.sim.app.world_mut().get_mut::<Armor>(self.p).unwrap().amount = points / 100.0;
    }

    fn tick(&mut self) {
        self.sim.ticks(1);
    }

    fn now(&self) -> u64 {
        self.sim.app.world().resource::<Heard>().tick
    }

    fn clear(&mut self) {
        self.sim.app.world_mut().resource_mut::<Heard>().sounds.clear();
    }

    /// When `entry` played, in ticks from `since`.
    fn heard(&self, entry: &str, since: u64) -> Vec<u64> {
        self.sim
            .app
            .world()
            .resource::<Heard>()
            .sounds
            .iter()
            .filter(|(t, e)| e == entry && *t >= since)
            .map(|(t, _)| t - since)
            .collect()
    }
}

fn close(actual: f32, expected: f32, what: &str) {
    assert!((actual - expected).abs() <= 0.01, "{what}: got {actual}, expected {expected}");
}

/// Tests 15-18: the sprint's drain from standing and moving, the tap.
#[test]
fn sprint_aux() {
    let mut pl = Player::at(FIELD, vec![]);
    pl.intent().move_axis = Vec2::Y;
    pl.intent().sprint = true;
    pl.tick();
    close(pl.suit().aux, 100.0, "test 15 tick 1");
    pl.tick();
    close(pl.suit().aux, 99.625, "test 15 tick 2");

    // Test 18: moving, +speed on one tick only.
    let mut pl = Player::at(FIELD, vec![]);
    pl.intent().move_axis = Vec2::Y;
    pl.sim.ticks(20);
    pl.intent().sprint = true;
    pl.tick();
    close(pl.suit().aux, 99.625, "test 18 tick 1");
    pl.intent().sprint = false;
    pl.tick();
    close(pl.suit().aux, 97.125, "tick 2");
    pl.sim.ticks(33);
    close(pl.suit().aux, 97.125, "unchanged to tick 35");
    pl.tick();
    close(pl.suit().aux, 97.3125, "tick 36");
    pl.sim.ticks(15);
    close(pl.suit().aux, 100.0, "full at tick 51");
}

/// Tests 25-26: drowning after the aux power, and giving it back.
#[test]
fn drowning_after_the_aux_power() {
    let under = POOL + Vec3::Z * 20.0;
    let mut pl = Player::at(under, vec![]);
    pl.suit_mut().aux = 100.0;
    let start = pl.now();
    let mut aux = Vec::new();
    for _ in 0..1700 {
        pl.place(under);
        pl.tick();
        aux.push(pl.suit().aux);
        if pl.now() - start == 1648 {
            break;
        }
    }
    close(aux[0], 99.8995, "test 25 tick 1");
    assert!(aux[993] > 0.0 && aux[994] == 0.0, "aux 0 after tick 995: {} {}", aux[993], aux[994]);
    assert_eq!(pl.heard("Player.DrownStart", start), [1462], "first hit");
    assert_eq!(pl.heard("Player.DrownContinue", start), [1529, 1596]);
    close(pl.health(), 70.0, "three hits");
    // Test 26: surface on tick 1650.
    pl.place(FIELD);
    pl.sim.ticks(300);
    assert_eq!(pl.heard("Player.DrownStart", start), [1462, 1650], "the gasp");
    close(pl.health(), 100.0, "given back");
    let suit = pl.suit();
    assert_eq!(suit.devices, 0);
}

/// Test 26's steps and test 27's breather cost.
#[test]
fn surfacing() {
    let under = POOL + Vec3::Z * 20.0;
    let mut pl = Player::at(under, vec![]);
    pl.suit_mut().aux = 100.0;
    for _ in 0..100 {
        pl.place(under);
        pl.tick();
    }
    close(pl.suit().aux, 100.0 - 100.0 * 0.1005, "test 27 under");
    // Teleported out, the first tick still starts with the eyes under.
    pl.place(FIELD);
    pl.tick();
    let before = pl.suit().aux;
    pl.tick();
    close(before - pl.suit().aux, 0.67, "test 27: the breather's cost");
}

/// Tests 29-31: the flashlight.
#[test]
fn flashlight() {
    let mut pl = Player::at(FIELD, vec![]);
    pl.sim.app.world_mut().write_message(Flashlight(pl.p));
    pl.tick();
    assert!(pl.sim.app.world().get::<FlashlightOn>(pl.p).is_none(), "test 29: off by default");
    assert!(pl.heard("HL2Player.FlashlightOn", 0).is_empty());

    pl.sim.app.world_mut().insert_resource(FlashlightAllowed(true));
    pl.sim.app.world_mut().write_message(Flashlight(pl.p));
    pl.tick();
    assert!(pl.sim.app.world().get::<FlashlightOn>(pl.p).is_some(), "test 30: on");
    assert_eq!(pl.heard("HL2Player.FlashlightOn", 0).len(), 1);
    pl.sim.ticks(100);
    close(pl.suit().aux, 100.0, "no drain");
    // Test 31: death turns it off silently.
    pl.set_health(0.0);
    pl.tick();
    assert!(pl.sim.app.world().get::<FlashlightOn>(pl.p).is_none(), "test 31");
    assert!(pl.heard("HL2Player.FlashlightOff", 0).is_empty());
}

/// Tests 32 and 35: `+zoom` narrows the view and blocks attack while held.
#[test]
fn suit_zoom_held() {
    let mut pl = Player::at(FIELD, vec![]);
    let bit = pl.sim.app.world().resource::<chimera_core::core::ActionRegistry>().bit(suit::ZOOM_ACTION);
    pl.intent().actions = bit;
    pl.intent().fire = true;
    pl.sim.ticks(30);
    let fov = pl.sim.app.world().get::<ViewZoom>(pl.p).map(|v| v.0);
    assert!(fov.is_some_and(|f| (f - 25.0).abs() < 0.01), "test 32: {fov:?}");
    assert!(!pl.sim.app.world().get::<Intent>(pl.p).unwrap().fire, "test 35: attack cleared");
    pl.intent().actions = 0;
    pl.sim.ticks(20);
    assert!(pl.sim.app.world().get::<ViewZoom>(pl.p).is_none(), "test 33: back to 75");
}

fn kit_at(class: &str, at: Vec3) -> MapEntity {
    entity(class, at, &[])
}

/// Tests 40-43: health kits, vials and batteries.
#[test]
fn pickups() {
    let item = FIELD + Vec3::new(20.0, 0.0, 0.0);
    for (class, health, armour, health_after, armour_after, taken) in [
        ("item_healthkit", 100.0, 0.0, 100.0, 0.0, false),
        ("item_healthkit", 90.0, 0.0, 100.0, 0.0, true),
        ("item_healthkit", 60.0, 0.0, 85.0, 0.0, true),
        ("item_healthvial", 95.0, 0.0, 100.0, 0.0, true),
        ("item_battery", 100.0, 95.0, 100.0, 100.0, true),
        ("item_battery", 100.0, 100.0, 100.0, 100.0, false),
        ("item_battery", 100.0, 150.0, 100.0, 150.0, false),
    ] {
        let mut pl = Player::at(FIELD + Vec3::new(-400.0, 0.0, 0.0), vec![kit_at(class, item)]);
        pl.set_health(health);
        pl.set_armour(armour);
        pl.place(FIELD);
        pl.tick();
        assert_eq!((pl.health(), pl.armour()), (health_after, armour_after), "{class} at {health}/{armour}");
        let entry = match class {
            "item_healthkit" => "HealthKit.Touch",
            "item_healthvial" => "HealthVial.Touch",
            _ => "ItemBattery.Touch",
        };
        assert_eq!(!pl.heard(entry, 0).is_empty(), taken, "{class} taken");
        if class == "item_healthkit" && taken && health == 60.0 {
            // Test 43: back 30 s later.
            pl.place(FIELD + Vec3::new(-400.0, 0.0, 0.0));
            let t = pl.now();
            pl.sim.ticks(2010);
            assert_eq!(pl.heard("AlyxEMP.Charge", t), [2000]);
        }
    }
    // Test 42: a wall between the item and the eye.
    let mut pl = Player::at(FIELD + Vec3::new(-400.0, 0.0, 0.0), vec![kit_at("item_healthkit", item)]);
    let wall = to_engine(FIELD + Vec3::new(10.0, 0.0, 40.0));
    pl.sim.app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(4.0 * UNIT, 120.0 * UNIT, 200.0 * UNIT),
        Transform::from_translation(wall),
    ));
    pl.sim.ticks(2);
    pl.set_health(60.0);
    pl.place(FIELD);
    pl.tick();
    assert_eq!(pl.health(), 60.0, "test 42: refused");
}

/// A charger 40 units ahead at eye height, the player looking at it.
fn at_charger(class: &str, flags: &str) -> Player {
    let charger = entity(class, FIELD + Vec3::new(40.0, 0.0, 64.0), &[("spawnflags", flags)]);
    let mut pl = Player::at(FIELD, vec![charger]);
    pl.sim.ticks(2);
    pl.clear();
    pl
}

/// Tests 44-45, 49-50: suit chargers.
#[test]
fn suit_chargers() {
    // Test 44.
    let mut pl = at_charger("item_suitcharger", "0");
    let start = pl.now();
    pl.intent().use_key = true;
    let mut armour = Vec::new();
    for _ in 0..520 {
        pl.tick();
        armour.push(pl.armour());
    }
    assert_eq!((armour[0], armour[7], armour[518]), (1.0, 2.0, 75.0), "test 44");
    assert_eq!(pl.heard("SuitRecharge.Start", start), [1]);
    assert_eq!(pl.heard("SuitRecharge.ChargingLoop", start), [43]);
    assert_eq!(pl.heard("SuitRecharge.Deny", start), [520]);
    pl.intent().use_key = false;
    pl.sim.ticks(2010);
    assert_eq!(pl.heard("SuitRecharge.Start", start), [1, 2520], "refilled 30 s later");

    // Test 45.
    let mut pl = at_charger("item_suitcharger", "0");
    let start = pl.now();
    pl.set_armour(90.0);
    pl.intent().use_key = true;
    pl.sim.ticks(64);
    assert_eq!(pl.armour(), 100.0, "test 45 at 64");
    pl.sim.ticks(40);
    assert_eq!(pl.heard("SuitRecharge.Deny", start), [65], "one deny");

    // Test 49: Kleiner's.
    let mut pl = at_charger("item_suitcharger", "16384");
    pl.intent().use_key = true;
    pl.sim.ticks(200);
    assert_eq!(pl.armour(), 25.0, "test 49");

    // Test 50: the citadel's.
    let mut pl = at_charger("item_suitcharger", "8192");
    pl.set_armour(100.0);
    pl.set_health(50.0);
    let start = pl.now();
    pl.intent().use_key = true;
    pl.sim.ticks(64);
    assert_eq!(pl.health(), 100.0, "test 50 health at 64");
    pl.sim.ticks(280);
    assert_eq!(pl.armour(), 200.0, "test 50 armour at 344");
    pl.tick();
    assert_eq!(pl.heard("SuitRecharge.Deny", start), [345]);
}

/// Tests 46-48: health chargers and denying.
#[test]
fn health_chargers() {
    let mut pl = at_charger("item_healthcharger", "0");
    pl.set_health(40.0);
    let start = pl.now();
    pl.intent().use_key = true;
    pl.sim.ticks(344);
    assert_eq!(pl.health(), 90.0, "test 46");
    pl.tick();
    assert_eq!(pl.heard("WallHealth.Deny", start), [345]);
    pl.intent().use_key = false;
    pl.sim.ticks(4020);
    assert_eq!(pl.heard("WallHealth.Recharge", start).len(), 1, "refilled 60 s later");

    // Test 47: full health, three presses.
    let mut pl = at_charger("item_healthcharger", "0");
    let start = pl.now();
    for _ in 0..3 {
        pl.intent().use_key = true;
        pl.sim.ticks(5);
        pl.intent().use_key = false;
        pl.sim.ticks(5);
    }
    assert_eq!(pl.heard("WallHealth.Deny", start).len(), 3, "test 47");

    // Test 48: an empty one denies every 0.62 s while held.
    let mut pl = at_charger("item_healthcharger", "0");
    pl.set_health(1.0);
    pl.intent().use_key = true;
    pl.sim.ticks(400);
    pl.intent().use_key = false;
    pl.sim.ticks(70);
    pl.set_health(1.0);
    let start = pl.now();
    pl.intent().use_key = true;
    pl.sim.ticks(133);
    assert_eq!(pl.heard("WallHealth.Deny", start), [1, 43, 85, 127], "test 48");
}

/// Test 52: what spawning resets.
#[test]
fn spawning_resets_the_suit() {
    let mut pl = Player::at(FIELD, vec![]);
    pl.set_armour(60.0);
    pl.suit_mut().aux = 40.0;
    pl.sim.app.world_mut().get_mut::<DamageRemainder>(pl.p).unwrap().0 = 0.8;
    pl.set_health(0.0);
    pl.sim.app.world_mut().entity_mut(pl.p).insert((Dead { since: 0.0 }, FlashlightOn));
    pl.tick();
    chimera_sim::spawning::revive(pl.sim.app.world_mut(), pl.p, None);
    pl.tick();
    assert_eq!((pl.health(), pl.armour()), (100.0, 0.0));
    close(pl.suit().aux, 100.0, "aux");
    assert!(pl.sim.app.world().get::<FlashlightOn>(pl.p).is_none());
    close(pl.sim.app.world().get::<DamageRemainder>(pl.p).unwrap().0, 0.8, "the remainder is kept");
}

/// suit.md Quirks, as the real server measured (probe P9, the plan's
/// Progress 2026-11-03 "probe 2"): killed sprinting, `+speed` let go while
/// dead, the respawned player pays the sprint's 2.5 detach on the first
/// tick (aux 97.5), full again 14 ticks after the half-second delay.
#[test]
fn a_sprinter_killed_pays_the_detach_at_the_respawn() {
    let mut pl = Player::at(FIELD, vec![]);
    pl.intent().move_axis = Vec2::Y;
    pl.intent().sprint = true;
    pl.sim.ticks(10);
    assert!(pl.suit().sprinting);
    pl.set_health(0.0);
    pl.sim.app.world_mut().entity_mut(pl.p).insert(Dead { since: 0.0 });
    pl.intent().move_axis = Vec2::ZERO;
    pl.intent().sprint = false;
    pl.sim.ticks(5);
    chimera_sim::spawning::revive(pl.sim.app.world_mut(), pl.p, None);
    pl.tick();
    pl.tick();
    close(pl.suit().aux, 97.5, "the first tick after the respawn");
    pl.sim.ticks(33 + 14);
    close(pl.suit().aux, 100.0, "full again");
}

/// A drowning hit bypasses the suit (test 10 in the simulation).
#[test]
fn damage_lands_in_whole_points() {
    let mut pl = Player::at(FIELD, vec![]);
    pl.set_armour(50.0);
    let p = pl.p;
    let at = pl.sim.position(p);
    for _ in 0..5 {
        pl.sim.app.world_mut().write_message(Damage {
            target: p,
            attacker: None,
            amount: 0.002,
            point: at,
            dir: Vec3::ZERO,
            hitgroup: Hitgroup::Generic,
            kind: DamageKind::Generic,
            weapon: None,
            force: Vec3::ZERO,
        });
        pl.tick();
    }
    assert_eq!(pl.health(), 99.0, "five 0.2 hits take one point");
    let _ = (to_source(at), pl.sim.app.world().get::<MovementState>(p).is_some(), pl.sim.app.world().get::<SourceMovement>(p).is_some());
}

/// Edge cases (suit.md): a generic trigger_hurt is absorbed by armour,
/// a radiation or poison one (dm_underpass's) bypasses it. 40 a second:
/// 20 a pass on 100 armour.
#[test]
fn trigger_hurt_radiation_and_poison_bypass_the_suit() {
    use chimera_world::map::{MapData, MapHull, MapSimPlugin};
    let pass = |kind: &str| {
        let (lo, hi) = (Vec3::new(-64.0, -64.0, 0.0), Vec3::new(64.0, 64.0, 128.0));
        let b = MapBrush::from_box(lo, hi);
        let points = (0..8)
            .map(|i| Vec3::new([lo.x, hi.x][i & 1], [lo.y, hi.y][(i >> 1) & 1], [lo.z, hi.z][(i >> 2) & 1]))
            .collect();
        let hurt = MapEntity {
            keyvalues: [("classname", "trigger_hurt"), ("spawnflags", "1"), ("damage", "40"), ("damagetype", kind), ("origin", "0 0 0")]
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            hulls: vec![MapHull { planes: b.planes, points }],
            ..Default::default()
        };
        let (a, b) = (to_engine(Vec3::new(-4096.0, -4096.0, -64.0)), to_engine(Vec3::new(4096.0, 4096.0, 0.0)));
        let data = MapData {
            name: "test:hurt".into(),
            collision_brushes: vec![MapBrush::from_box(a.min(b), a.max(b))],
            entities: vec![hurt],
            entity_scale: UNIT,
            ..Default::default()
        };
        let mut sim = Sim::new(Hl2Sim, MapSimPlugin::new(data));
        sim.set_tick_interval(0.015);
        let p = sim.spawn_character(to_engine(Vec3::Z * 37.0), ID);
        sim.ticks(2);
        sim.app.world_mut().get_mut::<Armor>(p).unwrap().amount = 1.0;
        // The first pass only (the next is 33 ticks on).
        sim.ticks(20);
        let w = sim.app.world();
        ((w.get::<Health>(p).unwrap().current * 100.0).round(), (w.get::<Armor>(p).unwrap().amount * 100.0).round())
    };
    assert_eq!(pass("0"), (96.0, 84.0), "generic: 4 to health, 16 armour");
    assert_eq!(pass("262144"), (80.0, 100.0), "radiation bypasses the suit");
    assert_eq!(pass("131072"), (80.0, 100.0), "poison bypasses the suit");
}

/// Flashlight: `impulse 100` (`weapon::PlayerImpulse`, the console's and a
/// network client's) toggles it.
#[test]
fn impulse_100_toggles_the_flashlight() {
    use chimera_sim::weapon::PlayerImpulse;
    let mut pl = Player::at(FIELD, vec![]);
    pl.sim.app.world_mut().insert_resource(FlashlightAllowed(true));
    pl.sim.app.world_mut().write_message(PlayerImpulse { player: pl.p, impulse: 100 });
    pl.tick();
    assert!(pl.sim.app.world().get::<FlashlightOn>(pl.p).is_some(), "on");
    pl.sim.app.world_mut().write_message(PlayerImpulse { player: pl.p, impulse: 100 });
    pl.tick();
    assert!(pl.sim.app.world().get::<FlashlightOn>(pl.p).is_none(), "off");
    assert_eq!(pl.heard("HL2Player.FlashlightOff", 0).len(), 1);
}

/// Suit zoom: `toggle_zoom` starts and stops it and, not holding `+zoom`,
/// lets the player fire (test 35's other half).
#[test]
fn toggle_zoom_lets_the_player_fire() {
    use hl2dm::game::suit::ZoomToggle;
    let mut pl = Player::at(FIELD, vec![]);
    pl.sim.app.world_mut().entity_mut(pl.p).insert(ZoomToggle);
    pl.intent().fire = true;
    pl.sim.ticks(30);
    let fov = pl.sim.app.world().get::<ViewZoom>(pl.p).map(|v| v.0);
    assert!(fov.is_some_and(|f| (f - 25.0).abs() < 0.01), "zoomed: {fov:?}");
    assert!(pl.sim.app.world().get::<Intent>(pl.p).unwrap().fire, "attack kept");
    pl.sim.app.world_mut().entity_mut(pl.p).insert(ZoomToggle);
    pl.sim.ticks(20);
    assert!(pl.sim.app.world().get::<ViewZoom>(pl.p).is_none(), "toggled back");
}

/// One zoom owner: with the crossbow's scope up the suit zoom doesn't
/// start; with the suit zoom up the crossbow's zoom toggle (attack2) is
/// refused.
#[test]
fn one_zoom_owner() {
    use chimera_sim::weapon::{Inventory, Zoomed};
    use hl2dm::game::suit::ZoomToggle;
    let mut pl = Player::at(FIELD, vec![]);
    let bow = chimera_sim::weapon::give(pl.sim.app.world_mut(), pl.p, hl2dm::game::weapons::CROSSBOW).unwrap();
    pl.sim.app.world_mut().get_mut::<Inventory>(pl.p).unwrap().active = Some(bow);
    pl.sim.ticks(70);
    // The scope up: +zoom does nothing.
    pl.sim.app.world_mut().entity_mut(pl.p).insert(Zoomed { fov: 20.0, scope: true, sensitivity: None });
    let bit = pl.sim.app.world().resource::<chimera_core::core::ActionRegistry>().bit(suit::ZOOM_ACTION);
    pl.intent().actions = bit;
    pl.sim.ticks(10);
    assert!(pl.suit().zoom.is_none(), "refused while the scope holds");
    pl.intent().actions = 0;
    pl.sim.ticks(2);
    pl.sim.app.world_mut().entity_mut(pl.p).remove::<Zoomed>();
    pl.sim.ticks(2);
    // The suit zoom up: attack2 cleared, the scope stays down.
    pl.sim.app.world_mut().entity_mut(pl.p).insert(ZoomToggle);
    pl.sim.ticks(2);
    pl.intent().secondary = true;
    pl.sim.ticks(3);
    assert!(!pl.sim.app.world().get::<Intent>(pl.p).unwrap().secondary, "the crossbow's toggle refused");
    assert!(pl.sim.app.world().get::<Zoomed>(pl.p).is_none_or(|z| z.fov != 20.0), "no scope");
}

/// `impulse 50` (bound to C): HL2's squad command does nothing here; the
/// console takes it without an error.
#[test]
fn impulse_50_does_nothing() {
    let mut pl = Player::at(FIELD, vec![]);
    pl.sim.app.world_mut().insert_resource(FlashlightAllowed(true));
    chimera_core::console::execute(pl.sim.app.world_mut(), &["impulse".to_string(), "50".to_string()], 0);
    pl.tick();
    assert!(pl.sim.app.world().get::<FlashlightOn>(pl.p).is_none());
}

/// The flashlight's light: one spot light while on, gone when off.
#[test]
fn the_flashlight_lights_the_view() {
    use chimera_sim::weapon::PlayerImpulse;
    use hl2dm::game::suit::FlashlightBeam;
    let mut pl = Player::at(FIELD, vec![]);
    pl.sim.app.world_mut().insert_resource(FlashlightAllowed(true));
    pl.sim.app.world_mut().write_message(PlayerImpulse { player: pl.p, impulse: 100 });
    pl.sim.ticks(3);
    let beams = |pl: &mut Player| pl.sim.app.world_mut().query::<&FlashlightBeam>().iter(pl.sim.app.world()).count();
    assert_eq!(beams(&mut pl), 1);
    pl.sim.app.world_mut().write_message(PlayerImpulse { player: pl.p, impulse: 100 });
    pl.sim.ticks(3);
    assert_eq!(beams(&mut pl), 0);
}
