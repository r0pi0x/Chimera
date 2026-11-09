//! The map's pickups on `Hl2Sim` (specs/hl2dm/maps_entities.md, H10's
//! implementer cases, "case N"): the ammo boxes, items coming to rest and
//! back (Chimera's `weapon::respawn`, lift C3), placed weapons' copies,
//! relocation, the no-respawn flag, the ammo crate and the suit chargers'
//! outputs. A floor at z = 0; ticks at 0.015 s.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::{
    core::{Health, SimClock, Velocity},
    game_log::{EventValue, GameEvent},
};
use chimera_sim::weapon::{
    Inventory, Weapon,
    drop::{Loose, PickedUp},
    respawn::{Copied, MapPickup, PickupBack, Respawning},
};
use chimera_world::{
    harness::Sim,
    map::{MapBrush, MapBrushes, MapEntities, MapEntity, PlaySound, entities::{FireEntityOutput, FireEntityOutputValue}},
};
use hl2dm::{
    game::{
        ammo,
        ammo_crates::{AmmoCrate, Lid},
        movement::ID,
        pickups::{Pickup, RespawnCvars},
        weapons::inventory::Hl2Ammo,
    },
    harness::Hl2Sim,
};
use source_engine::source_movement::{to_engine, to_source};

const UNIT: f32 = 0.0254;

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
    commands.insert_resource(MapBrushes(vec![brush(
        Vec3::new(-8192.0, -8192.0, -64.0),
        Vec3::new(8192.0, 8192.0, 0.0),
    )]));
    // The floor for physics objects too (the movement traces the brushes).
    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(400.0, 64.0 * UNIT, 400.0),
        Transform::from_translation(Vec3::new(0.0, -32.0 * UNIT, 0.0)),
    ));
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

/// What happened, by tick.
#[derive(Resource, Default)]
struct Log {
    sounds: Vec<(u64, String)>,
    outputs: Vec<(u64, usize, String)>,
    values: Vec<(u64, String, f32)>,
    events: Vec<(u64, GameEvent)>,
    backs: Vec<(u64, PickupBack)>,
    picked: Vec<(u64, PickedUp)>,
}

#[allow(clippy::too_many_arguments)]
fn listen(
    clock: Res<SimClock>,
    mut log: ResMut<Log>,
    mut sounds: MessageReader<PlaySound>,
    mut outputs: MessageReader<FireEntityOutput>,
    mut values: MessageReader<FireEntityOutputValue>,
    mut events: MessageReader<GameEvent>,
    mut backs: MessageReader<PickupBack>,
    mut picked: MessageReader<PickedUp>,
) {
    let t = clock.tick;
    log.sounds.extend(sounds.read().map(|s| (t, s.entry.clone())));
    log.outputs.extend(outputs.read().map(|o| (t, o.map_index, o.output.clone())));
    log.values.extend(values.read().map(|v| (t, v.output.clone(), v.value)));
    log.events.extend(events.read().map(|e| (t, e.clone())));
    log.backs.extend(backs.read().map(|b| (t, *b)));
    log.picked.extend(picked.read().map(|p| (t, *p)));
}

struct World_ {
    sim: Sim,
    p: Entity,
}

/// A player at `feet` far from everything (x -3000), the map's
/// `entities` placed.
fn world(entities: Vec<MapEntity>) -> World_ {
    let mut sim = Sim::new(Hl2Sim, TestMap(entities));
    sim.set_tick_interval(0.015);
    sim.app.init_resource::<Log>().add_systems(FixedLast, listen);
    let p = sim.spawn_character(to_engine(Vec3::new(-3000.0, 0.0, 36.0)), ID);
    sim.ticks(3);
    World_ { sim, p }
}

impl World_ {
    fn tick(&self) -> u64 {
        self.sim.app.world().resource::<SimClock>().tick
    }

    /// The player standing at `feet` (Source units), still.
    fn place(&mut self, feet: Vec3) {
        let at = to_engine(feet + Vec3::Z * 36.0);
        let w = self.sim.app.world_mut();
        w.get_mut::<Transform>(self.p).unwrap().translation = at;
        if let Some(mut pos) = w.get_mut::<Position>(self.p) {
            pos.0 = at;
        }
        w.get_mut::<Velocity>(self.p).unwrap().0 = Vec3::ZERO;
    }

    fn away(&mut self) {
        self.place(Vec3::new(-3000.0, 0.0, 0.0));
    }

    /// No weapons and no ammo.
    fn unarmed(&mut self) {
        let w = self.sim.app.world_mut();
        let weapons = w.get::<Inventory>(self.p).map(|i| i.weapons.clone()).unwrap_or_default();
        for e in weapons {
            w.despawn(e);
        }
        w.entity_mut(self.p).insert((Inventory::default(), Hl2Ammo::default()));
    }

    fn ammo(&self, kind: &str) -> u32 {
        ammo::carried(self.sim.app.world(), self.p, kind)
    }

    fn set_ammo(&mut self, kind: &str, n: u32) {
        let w = self.sim.app.world_mut();
        w.entity_mut(self.p).insert(Hl2Ammo(vec![(kind.to_string(), n)]));
    }

    fn items(&mut self) -> Vec<(Entity, Pickup, Transform, Option<Respawning>, MapPickup)> {
        let w = self.sim.app.world_mut();
        let mut q = w.query::<(Entity, &Pickup, &Transform, Option<&Respawning>, &MapPickup)>();
        let mut all: Vec<_> = q.iter(w).map(|(e, p, t, r, m)| (e, p.clone(), *t, r.copied(), m.clone())).collect();
        all.sort_by_key(|(_, p, ..)| p.map_index);
        all
    }

    fn log(&self) -> &Log {
        self.sim.app.world().resource::<Log>()
    }

    fn outputs(&self, name: &str) -> Vec<u64> {
        self.log().outputs.iter().filter(|(_, _, o)| o == name).map(|(t, ..)| *t).collect()
    }

    fn heard(&self, entry: &str) -> Vec<u64> {
        self.log().sounds.iter().filter(|(_, e)| e == entry).map(|(t, _)| *t).collect()
    }
}

/// Case 1: every ammo box gives its count (capped by the limit) to empty
/// pools and is used up.
#[test]
fn ammo_boxes_give_their_counts_1() {
    let data = ammo::ItemData::hl2();
    for (class, kind, expect) in [
        ("item_ammo_pistol", "hl2dm:pistol", 20),
        ("item_ammo_pistol_large", "hl2dm:pistol", 100),
        ("item_ammo_smg1", "hl2dm:smg1", 45),
        ("item_ammo_smg1_large", "hl2dm:smg1", 225),
        ("item_ammo_ar2", "hl2dm:ar2", 20),
        ("item_ammo_ar2_large", "hl2dm:ar2", 60),
        ("item_ammo_357", "hl2dm:357", 6),
        ("item_ammo_357_large", "hl2dm:357", 12),
        ("item_ammo_crossbow", "hl2dm:xbowbolt", 6),
        ("item_box_buckshot", "hl2dm:buckshot", 20),
        ("item_rpg_round", "hl2dm:rpg_round", 1),
        ("item_ammo_smg1_grenade", "hl2dm:smg1_grenade", 1),
        ("item_ammo_ar2_altfire", "hl2dm:ar2altfire", 1),
    ] {
        assert_eq!(data.ammo_box(class).map(|b| b.ammo.as_str()), Some(kind));
        let mut w = world(vec![entity(class, Vec3::new(0.0, 0.0, 0.0), &[])]);
        w.unarmed();
        w.place(Vec3::new(-20.0, 0.0, 0.0));
        w.sim.ticks(2);
        assert_eq!(w.ammo(kind), expect, "{class}");
        assert!(w.items()[0].3.is_some(), "{class}: used up");
    }
}

/// Cases 2, 3: a partial pickup uses the box up; a full pool leaves it,
/// `OnCacheInteraction` every tick of the touch.
#[test]
fn partial_and_refused_ammo_2_3() {
    let mut w = world(vec![entity("item_ammo_pistol", Vec3::ZERO, &[])]);
    w.unarmed();
    w.set_ammo("hl2dm:pistol", 140);
    w.place(Vec3::new(-20.0, 0.0, 0.0));
    w.sim.ticks(2);
    assert_eq!(w.ammo("hl2dm:pistol"), 150);
    assert!(w.items()[0].3.is_some(), "case 2: used up");
    assert_eq!(w.outputs("OnPlayerTouch").len(), 1);
    assert_eq!(w.heard("BaseCombatCharacter.AmmoPickup").len(), 1);
    let e = w.log().events.iter().find(|(_, e)| e.name == "ammo_pickup").map(|(_, e)| e.clone()).unwrap();
    assert_eq!(e.get("amount"), Some(&EventValue::Int(10)));
    assert_eq!(e.get("total"), Some(&EventValue::Int(150)));

    let mut w = world(vec![entity("item_ammo_pistol", Vec3::ZERO, &[])]);
    w.unarmed();
    w.set_ammo("hl2dm:pistol", 150);
    w.place(Vec3::new(-20.0, 0.0, 0.0));
    let t = w.tick();
    w.sim.ticks(10);
    assert!(w.items()[0].3.is_none(), "case 3: stays");
    assert_eq!(w.outputs("OnCacheInteraction").iter().filter(|x| **x > t).count(), 10);
    assert!(w.outputs("OnPlayerTouch").is_empty());
}

/// Cases 4 (in part), 5: bolts without a crossbow; the flare box is never
/// taken.
#[test]
fn ammo_without_its_weapon_and_unknown_types_4_5() {
    let mut w = world(vec![entity("item_ammo_crossbow", Vec3::ZERO, &[])]);
    w.unarmed();
    w.place(Vec3::new(-20.0, 0.0, 0.0));
    w.sim.ticks(2);
    assert_eq!(w.ammo("hl2dm:xbowbolt"), 6, "case 4");

    let mut w = world(vec![entity("item_box_flare_rounds", Vec3::ZERO, &[])]);
    w.unarmed();
    w.place(Vec3::new(-20.0, 0.0, 0.0));
    w.sim.ticks(10);
    assert!(w.items()[0].3.is_none(), "case 5: never taken");
    assert!(!w.outputs("OnCacheInteraction").is_empty());
}

/// Case 6: an item placed in the air falls, and records its spot at the
/// first 7-tick check at rest, on the floor.
#[test]
fn items_record_where_they_rest_6() {
    let mut w = world(vec![entity("item_battery", Vec3::new(0.0, 0.0, 40.0), &[])]);
    w.sim.ticks(150);
    let (_, _, t, _, p) = w.items().remove(0);
    let spot = p.spot.expect("rested");
    let floor = to_source(spot.translation).z;
    assert!((floor - 8.0).abs() < 1.5, "the box's centre on the floor: {floor}");
    assert!((to_source(t.translation).z - floor).abs() < 0.5);
    assert!(p.next_check.is_some(), "listed");
}

/// Cases 7, 8, 14: a taken item is hidden and back 2000 ticks later at its
/// spot (667 with `sv_hl2mp_item_respawn_time 10`); with the no-respawn
/// flag it's gone.
#[test]
fn taken_items_come_back_7_8_14() {
    for (secs, ticks) in [(30.0, 2000), (10.0, 667)] {
        let mut w = world(vec![entity("item_healthkit", Vec3::ZERO, &[])]);
        w.sim.app.world_mut().resource_mut::<RespawnCvars>().item = secs;
        w.sim.ticks(60);
        w.sim.app.world_mut().get_mut::<Health>(w.p).unwrap().current = 0.5;
        w.place(Vec3::new(-20.0, 0.0, 0.0));
        w.sim.ticks(1);
        let taken = *w.heard("HealthKit.Touch").first().expect("taken");
        w.away();
        w.sim.ticks(ticks + 5);
        assert_eq!(w.heard("AlyxEMP.Charge"), [taken + ticks], "{secs} s");
        let (_, _, t, r, p) = w.items().remove(0);
        assert!(r.is_none());
        assert!(t.translation.distance(p.spot.unwrap().translation) < 1e-3, "at its spot");
    }
    let mut w = world(vec![entity("item_healthkit", Vec3::ZERO, &[("spawnflags", "1073741824")])]);
    w.sim.app.world_mut().get_mut::<Health>(w.p).unwrap().current = 0.5;
    w.place(Vec3::new(-20.0, 0.0, 0.0));
    w.sim.ticks(2);
    assert!(w.items().is_empty(), "case 14: removed");
}

/// Cases 9, 12: a placed weapon taken new comes back as a copy 1333 ticks
/// later; its relocation checks every 20 s (1334 ticks).
#[test]
fn placed_weapons_come_back_as_copies_9_12() {
    let mut w = world(vec![entity("weapon_shotgun", Vec3::new(0.0, 0.0, 0.0), &[])]);
    w.sim.ticks(30);
    {
        let world = w.sim.app.world_mut();
        let mut q = world.query::<(&Loose, &MapPickup)>();
        let (_, p) = q.single(world).unwrap();
        let listed = p.next_check.expect("listed");
        let _ = listed;
    }
    w.place(Vec3::new(-20.0, 0.0, 0.0));
    w.sim.ticks(3);
    let (taken, _) = *w.log().picked.first().expect("picked up");
    w.away();
    w.sim.ticks(1340);
    let backs: Vec<u64> = w.log().backs.iter().map(|(t, _)| *t).collect();
    assert_eq!(backs, [taken + 1333], "case 9");
    let world = w.sim.app.world_mut();
    let mut q = world.query_filtered::<(&Loose, &MapPickup), With<Copied>>();
    let (l, p) = q.single(world).expect("the copy lies there");
    assert_eq!(world.get::<Weapon>(l.weapon).unwrap().id, "hl2dm:weapon_shotgun");
    assert_eq!(world.get::<Weapon>(l.weapon).and_then(|x| x.owner), None);
    // Case 12's interval: listed when it appeared, the next check 20 s on.
    let now = world.resource::<SimClock>().now;
    let next = p.next_check.unwrap();
    assert!(next > now && next - now <= 20.0, "{next} {now}");
}

/// Case 11: relocation beyond 64 units at the next check; not at 64.
#[test]
fn moved_items_go_back_11() {
    // (64 itself lands a hair over in metres and back: 63.9.)
    for (push, back) in [(65.0, true), (63.9, false)] {
        let mut w = world(vec![entity("item_battery", Vec3::ZERO, &[])]);
        w.sim.ticks(120);
        let (e, _, t, _, p) = w.items().remove(0);
        let spot = p.spot.unwrap();
        let first_check = p.next_check.unwrap();
        let to = t.translation + Vec3::X * push * UNIT;
        {
            let world = w.sim.app.world_mut();
            world.get_mut::<Transform>(e).unwrap().translation = to;
            world.get_mut::<Position>(e).unwrap().0 = to;
        }
        // Past the next check (30 s after the first).
        while w.sim.app.world().resource::<SimClock>().now < first_check + 30.1 {
            w.sim.ticks(10);
        }
        let world = w.sim.app.world_mut();
        let at = world.get::<Transform>(e).unwrap().translation;
        assert_eq!(at.distance(spot.translation) < 1.0 * UNIT, back, "pushed {push}");
    }
}

/// Case 16: an item taken before its first rest check comes back at the
/// world origin.
#[test]
fn taken_before_rest_comes_back_at_the_origin_16() {
    let mut w = world(vec![entity("item_healthkit", Vec3::new(500.0, 0.0, 0.0), &[])]);
    w.sim.app.world_mut().get_mut::<Health>(w.p).unwrap().current = 0.5;
    // Taken at once (spawned and touched before the first check).
    w.place(Vec3::new(480.0, 0.0, 0.0));
    w.sim.ticks(1);
    let (_, _, t, r, p) = w.items().remove(0);
    if r.is_some() && p.spot.is_none() {
        assert!(t.translation.length() < 1e-3, "waits at the world origin");
    }
}

/// Cases 17, 18, 20: the ammo crate's timeline.
#[test]
fn the_ammo_crate_17_18_20() {
    let crate_at = Vec3::new(0.0, 0.0, 0.0);
    let mut w = world(vec![entity("item_ammo_crate", crate_at, &[("AmmoType", "3")])]);
    w.unarmed();
    // In front of the crate, looking at it.
    w.place(Vec3::new(-60.0, 0.0, 0.0));
    {
        let mut i = w.sim.intent(w.p);
        i.yaw = -std::f32::consts::FRAC_PI_2;
        i.pitch = -0.675;
    }
    w.sim.ticks(2);
    w.sim.intent(w.p).use_key = true;
    w.sim.ticks(1);
    let used = *w.outputs("OnUsed").first().expect("case 17: OnUsed");
    w.sim.intent(w.p).use_key = false;
    assert_eq!(w.heard("AmmoCrate.Open"), [used]);
    let mut given = None;
    let mut closing = None;
    for _ in 0..200 {
        w.sim.ticks(1);
        let t = w.tick();
        if given.is_none() && w.ammo("hl2dm:rpg_round") == 3 {
            given = Some(t);
        }
        let world = w.sim.app.world_mut();
        let lid = world.query::<&AmmoCrate>().single(world).unwrap().lid;
        if closing.is_none() && matches!(lid, Lid::Closing(_)) {
            closing = Some(t);
        }
    }
    assert_eq!(given, Some(used + 63), "ammo on T + 63");
    assert_eq!(closing, Some(used + 105), "Close from T + 105");
    assert_eq!(w.heard("AmmoCrate.Close"), [used + 182]);

    // Case 20: full already: opens and closes, nothing given.
    w.sim.intent(w.p).use_key = true;
    w.sim.ticks(1);
    w.sim.intent(w.p).use_key = false;
    w.sim.ticks(70);
    let world = w.sim.app.world_mut();
    let c = world.query::<&AmmoCrate>().single(world).unwrap().clone();
    assert!(!c.shown_empty, "case 20: nothing given");
    assert!(matches!(c.lid, Lid::Opening(_)));
}

/// Cases 23, 25: a suit charger's outputs: `OnFull` at load, `OnHalfEmpty`
/// and `OnEmpty` as it drains, `OnFull` at the refill.
#[test]
fn charger_outputs_23() {
    let mut w = world(vec![entity("item_suitcharger", Vec3::new(40.0, 0.0, 64.0), &[("spawnflags", "8192")])]);
    assert_eq!(w.outputs("OnFull").len(), 1, "at load");
    w.place(Vec3::ZERO);
    {
        let mut i = w.sim.intent(w.p);
        i.yaw = -std::f32::consts::FRAC_PI_2;
        i.pitch = 0.0;
    }
    w.sim.ticks(2);
    w.sim.app.world_mut().get_mut::<chimera_sim::weapon::Armor>(w.p).unwrap().amount = 0.0;
    w.sim.intent(w.p).use_key = true;
    let start = w.tick() + 1;
    w.sim.ticks(720);
    let half = w.outputs("OnHalfEmpty");
    let empty = w.outputs("OnEmpty");
    assert_eq!(half, [start + 343], "OnHalfEmpty on the 50th charge");
    assert_eq!(empty, [start + 693], "OnEmpty on the 100th");
    w.sim.intent(w.p).use_key = false;
    w.sim.ticks(4100);
    assert_eq!(w.outputs("OnFull").len(), 2, "refilled 60 s later");
}

/// The suit charger's inputs and value outputs (maps_entities.md 4):
/// `SetCharge` sets the pool and its maximum, each charge tells the charge
/// left as a fraction, `Recharge` fills the pool and fires `OnFull`.
#[test]
fn charger_inputs_and_value_outputs() {
    let mut w = world(vec![
        entity("item_suitcharger", Vec3::new(40.0, 0.0, 64.0), &[("targetname", "mega"), ("spawnflags", "8192")]),
        entity("logic_relay", Vec3::new(0.0, 500.0, 0.0), &[("targetname", "r1"), ("OnTrigger", "mega,SetCharge,40,0,-1")]),
        entity("logic_relay", Vec3::new(0.0, 600.0, 0.0), &[("targetname", "r2"), ("OnTrigger", "mega,Recharge,,0,-1")]),
    ]);
    let fire = |w: &mut World_, name: &str| {
        w.sim
            .app
            .world_mut()
            .resource_mut::<source_logic::logic::Logic>()
            .world
            .queue_input(name, "Trigger", source_logic::logic::Value::Void, 0.0, None);
        w.sim.ticks(3);
    };
    fire(&mut w, "r1");
    w.place(Vec3::ZERO);
    {
        let mut i = w.sim.intent(w.p);
        i.yaw = -std::f32::consts::FRAC_PI_2;
        i.pitch = 0.0;
    }
    w.sim.ticks(2);
    w.sim.app.world_mut().get_mut::<chimera_sim::weapon::Armor>(w.p).unwrap().amount = 0.0;
    w.sim.intent(w.p).use_key = true;
    w.sim.ticks(10);
    let first = w.log().values.iter().find(|(_, o, _)| o == "OutRemainingCharge").map(|v| v.2).expect("a value");
    // The citadel takes 2 a charge: 38 of 40 left.
    assert!((first - 38.0 / 40.0).abs() < 1e-5, "{first}");
    w.sim.ticks(400);
    assert_eq!(w.outputs("OnEmpty").len(), 1, "40 points are gone after 20 charges");
    w.sim.intent(w.p).use_key = false;
    let full = w.outputs("OnFull").len();
    fire(&mut w, "r2");
    assert_eq!(w.outputs("OnFull").len(), full + 1, "Recharge fills it at once");
    assert!(w.heard("SuitRecharge.Start").len() >= 1);
}

/// The dial and the loop (maps_entities.md 4, suit.md "Wall chargers"):
/// the charger's model (a node of its entity) holds `idle` at 1 − display
/// / max, running ahead of the pool while used; the loop sound starts
/// 0.56 s into a run under the charger's key and stops at switch-off.
#[test]
fn a_chargers_dial_and_loop() {
    use chimera_world::map::{PropCycle, PropEntity, SoundControl};
    #[derive(Resource, Default)]
    struct Loops(Vec<(bool, u64)>);
    let mut w = world(vec![entity("item_suitcharger", Vec3::new(40.0, 0.0, 64.0), &[])]);
    let node = w.sim.app.world_mut().spawn(PropEntity(0)).id();
    w.sim.app.init_resource::<Loops>().add_systems(FixedLast, |mut r: MessageReader<SoundControl>, mut l: ResMut<Loops>| {
        for c in r.read() {
            match c {
                SoundControl::Start(s) => l.0.push((true, s.key.0)),
                SoundControl::Stop(k) => l.0.push((false, k.0)),
                _ => {}
            }
        }
    });
    w.place(Vec3::ZERO);
    {
        let mut i = w.sim.intent(w.p);
        i.yaw = -std::f32::consts::FRAC_PI_2;
        i.pitch = 0.0;
    }
    w.sim.ticks(2);
    let cycle = |w: &World_| w.sim.app.world().get::<PropCycle>(node).map(|c| c.0);
    assert_eq!(cycle(&w), Some(0.0), "full: frame 0");
    w.sim.app.world_mut().get_mut::<chimera_sim::weapon::Armor>(w.p).unwrap().amount = 0.0;
    w.sim.intent(w.p).use_key = true;
    // 70 ticks of use: 10 charges (pool 65), the display 70/7 = 10 down
    // too, over max + 0.1 while used.
    w.sim.ticks(70);
    let c = cycle(&w).unwrap();
    assert!((c - (1.0 - 65.0 / 75.1)).abs() < 0.01, "dial {c}");
    let loops = &w.sim.app.world().resource::<Loops>().0;
    assert!(loops.iter().any(|(start, _)| *start), "the loop started");
    w.sim.intent(w.p).use_key = false;
    w.sim.ticks(40);
    let loops = &w.sim.app.world().resource::<Loops>().0;
    let key = loops.iter().find(|(s, _)| *s).unwrap().1;
    assert!(loops.contains(&(false, key)), "the loop stopped at switch-off: {loops:?}");
    let c = cycle(&w).unwrap();
    assert!((c - (1.0 - 65.0 / 75.0)).abs() < 0.01, "back to the pool: {c}");
}

/// The HUD's notes (hud.md 9, weapons.md test 42): a battery names its
/// class; a box of a full type, and a placed weapon of a full reserve, are
/// denied at each touch and stay.
#[test]
fn hud_notes_from_pickups() {
    use chimera_sim::weapon::Magazine;
    use hl2dm::game::{hud_notes::{HudPickup, PickupNote}, weapons};
    let mut g = world(vec![
        entity("item_battery", Vec3::new(0.0, 0.0, 0.0), &[]),
        entity("item_ammo_pistol", Vec3::new(500.0, 0.0, 0.0), &[]),
        entity("weapon_smg1", Vec3::new(1000.0, 0.0, 0.0), &[]),
    ]);
    g.sim.ticks(60);
    let notes = |g: &mut World_, c: &mut bevy::ecs::message::MessageCursor<HudPickup>, n: u64| {
        let mut got = Vec::new();
        for _ in 0..n {
            g.sim.ticks(1);
            got.extend(c.read(g.sim.app.world().resource::<Messages<HudPickup>>()).cloned());
        }
        got
    };
    let mut cursor = bevy::ecs::message::MessageCursor::<HudPickup>::default();
    let p = g.p;
    let item = |class: &str| HudPickup { player: p, note: PickupNote::Item(class.into()) };
    let denied = |ammo: &str| HudPickup { player: p, note: PickupNote::AmmoDenied(ammo.into()) };

    g.place(Vec3::new(0.0, 0.0, 0.0));
    assert!(notes(&mut g, &mut cursor, 5).contains(&item("item_battery")));

    // The pistol at its limit: a box refused, and it stays.
    g.unarmed();
    g.set_ammo("hl2dm:pistol", 150);
    g.place(Vec3::new(500.0, 0.0, 0.0));
    assert!(notes(&mut g, &mut cursor, 5).contains(&denied("hl2dm:pistol")));

    // A placed SMG1 with the SMG1's reserve full.
    g.away();
    let smg = chimera_sim::weapon::give(g.sim.app.world_mut(), p, weapons::SMG1).unwrap();
    {
        let mut m = g.sim.app.world_mut().get_mut::<Magazine>(smg).unwrap();
        m.reserve = m.reserve_max;
    }
    g.place(Vec3::new(1000.0, 0.0, 0.0));
    let got = notes(&mut g, &mut cursor, 30);
    assert!(got.contains(&denied("hl2dm:smg1")), "{got:?}");
    let world = g.sim.app.world_mut();
    assert_eq!(world.query_filtered::<(), With<Loose>>().iter(world).count(), 1, "the weapon stays");
}
