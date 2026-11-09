//! The stock maps' set pieces on `Hl2Sim` (specs/hl2dm/maps_entities.md 8,
//! 12, 16): dm_runoff's headcrab canisters (made by a template, fired by
//! an input, landing after the flight time with the blast's falloff), the
//! ragdoll manager and the microphone hearing blasts and shots. A floor at
//! z = 0; ticks at 0.015 s.

use std::sync::Arc;

use avian3d::prelude::*;
use bevy::prelude::*;
use chimera_core::core::{Explosion, Health, SimClock, Velocity};
use chimera_world::{
    harness::Sim,
    map::{MapBrush, MapBrushes, MapEntities, MapEntity, PlaySound, entities::FireKeyedOutput},
};
use hl2dm::{
    game::{movement::ID, set_pieces::RagdollLimit},
    harness::Hl2Sim,
};
use source_logic::logic::{Logic, Value};
use source_engine::source_movement::to_engine;

const UNIT: f32 = 0.0254;

#[derive(Resource, Clone)]
struct Entities(Vec<MapEntity>);

fn test_map(mut commands: Commands, entities: Res<Entities>) {
    let (a, b) = (to_engine(Vec3::new(-8192.0, -8192.0, -64.0)), to_engine(Vec3::new(8192.0, 8192.0, 0.0)));
    commands.insert_resource(MapBrushes(vec![MapBrush {
        surface: Some("concrete".into()),
        ..MapBrush::from_box(a.min(b), a.max(b))
    }]));
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

#[derive(Resource, Default)]
struct Log {
    sounds: Vec<(u64, String)>,
    keyed: Vec<(u64, String)>,
}

fn listen(clock: Res<SimClock>, mut log: ResMut<Log>, mut sounds: MessageReader<PlaySound>, mut keyed: MessageReader<FireKeyedOutput>) {
    let t = clock.tick;
    log.sounds.extend(sounds.read().map(|s| (t, s.entry.clone())));
    log.keyed.extend(keyed.read().map(|k| (t, k.output.clone())));
}

struct Case {
    sim: Sim,
    p: Entity,
}

fn case(entities: Vec<MapEntity>) -> Case {
    let mut sim = Sim::new(Hl2Sim, TestMap(entities));
    sim.set_tick_interval(0.015);
    sim.app.init_resource::<Log>().add_systems(FixedLast, listen);
    let p = sim.spawn_character(to_engine(Vec3::new(-3000.0, 0.0, 36.0)), ID);
    sim.ticks(3);
    sim.app.world_mut().resource_mut::<Logic>().world.record = true;
    Case { sim, p }
}

impl Case {
    fn input(&mut self, target: &str, input: &str) {
        self.sim
            .app
            .world_mut()
            .resource_mut::<Logic>()
            .world
            .queue_input(target, input, Value::Void, 0.0, None);
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

    fn health(&self) -> f32 {
        self.sim.app.world().get::<Health>(self.p).unwrap().current
    }

    fn fired(&self, output: &str) -> usize {
        self.sim.app.world().resource::<Logic>().world.fired.iter().filter(|(_, _, o)| o == output).count()
    }

    fn heard(&self, entry: &str) -> Vec<u64> {
        self.sim.app.world().resource::<Log>().sounds.iter().filter(|(_, e)| e == entry).map(|(t, _)| *t).collect()
    }
}

/// dm_runoff's canister, as the stock map has it.
fn canister(at: Vec3, extra: &[(&str, &str)]) -> MapEntity {
    let mut kv = vec![
        ("targetname", "can1"),
        ("angles", "-71 0 0"),
        ("FlightSpeed", "3000"),
        ("FlightTime", "4"),
        ("Damage", "150"),
        ("DamageRadius", "350"),
        ("SmokeLifetime", "20"),
        ("spawnflags", "565250"),
    ];
    kv.retain(|(k, _)| !extra.iter().any(|(e, _)| e == k));
    kv.extend_from_slice(extra);
    entity("env_headcrabcanister", at, &kv)
}

/// A canister fired once lands after its flight time: `OnLaunched` at
/// once, the incoming sound, the explosion and `OnImpacted` 4 s later,
/// the blast hurting by its falloff.
#[test]
fn a_canister_lands_after_its_flight_and_blasts() {
    let mut c = case(vec![canister(Vec3::new(0.0, 0.0, 8.0), &[("spawnflags", "565248")])]);
    c.place(Vec3::new(300.0, 0.0, 0.0));
    c.input("can1", "FireCanister");
    c.input("can1", "FireCanister");
    c.sim.ticks(10);
    assert_eq!(c.fired("OnLaunched"), 1, "once only");
    assert_eq!(c.health(), 1.0);
    assert!(!c.heard("HeadcrabCanister.LaunchSound").is_empty());
    c.sim.seconds(3.0);
    assert_eq!(c.fired("OnImpacted"), 0, "still in the air at 3 s");
    assert!(!c.heard("HeadcrabCanister.IncomingSound").is_empty(), "the incoming sound within a second");
    assert_eq!(c.health(), 1.0);
    let mut shoved = 0.0f32;
    for _ in 0..120 {
        c.sim.ticks(1);
        shoved = shoved.max(c.sim.app.world().get::<Velocity>(c.p).unwrap().0.length());
    }
    assert_eq!(c.fired("OnImpacted"), 1);
    assert!(shoved > 1.0, "shoved at {shoved} m/s");
    assert!(!c.heard("HeadcrabCanister.Explosion").is_empty());
    // 300 units out of 350: s = ((300 - 350) / 262.5)^2 = 0.036, 5 points.
    let hurt = 1.0 - c.health();
    assert!((0.03..0.08).contains(&hurt), "hurt {hurt}");
}

/// The shown keys and where (the canister's models as `ShownItem`s).
fn shown(sim: &mut Sim) -> Vec<(String, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(&chimera_world::map::loose::ShownItem, &Transform)>()
        .iter(w)
        .filter(|(s, _)| s.0.starts_with("hl2dm:headcrabcanister"))
        .map(|(s, t)| (s.0.clone(), t.translation))
        .collect()
}

/// The canister is drawn (maps_entities.md 8): unseen until fired, the
/// flight model on its line in the world (2000 units out a third of the
/// way down at 3000 units/s), then the landed model at the impact, its
/// `open` frames through the opening, smoking for `SmokeLifetime`, then
/// the same without smoke; `StopSmoke` ends it early. dm_runoff's flags:
/// no impact effects (`CanisterBlast`).
#[test]
fn a_canister_is_shown_in_flight_and_landed() {
    use hl2dm::game::set_pieces::{CANISTER_MODEL, CanisterBlast, OPEN_FRAMES, landed_key};
    let mut c = case(vec![canister(Vec3::new(0.0, 0.0, 8.0), &[("SmokeLifetime", "2")])]);
    let impact = to_engine(Vec3::new(0.0, 0.0, 8.0));
    assert!(shown(&mut c.sim).is_empty(), "nothing before it is fired");
    c.input("can1", "FireCanister");
    c.sim.seconds(3.0);
    let s = shown(&mut c.sim);
    assert_eq!(s.len(), 1, "{s:?}");
    assert_eq!(s[0].0, CANISTER_MODEL);
    let out = s[0].1.distance(impact) / UNIT;
    assert!((2700.0..3300.0).contains(&out), "1 s out: {out} units");
    assert!(s[0].1.y > impact.y, "coming down");
    c.sim.seconds(1.1);
    let s = shown(&mut c.sim);
    assert_eq!(s.len(), 1, "{s:?}");
    assert!(s[0].0.starts_with("hl2dm:headcrabcanister_open"), "landed, smoking: {s:?}");
    assert!(s[0].1.distance(impact) < 0.01);
    c.sim.seconds(1.2);
    assert_eq!(shown(&mut c.sim)[0].0, landed_key(OPEN_FRAMES - 1, true), "opened");
    c.sim.seconds(1.0);
    assert_eq!(shown(&mut c.sim)[0].0, landed_key(OPEN_FRAMES - 1, false), "the smoke ran out");
    let blasts = c.sim.app.world().resource::<bevy::ecs::message::Messages<CanisterBlast>>();
    assert_eq!(blasts.iter_current_update_messages().count() + blasts.len(), 0, "no impact effects (524288)");
    // Killed: gone.
    c.input("can1", "Kill");
    c.sim.ticks(3);
    assert!(shown(&mut c.sim).is_empty());
}

/// Without flag 524288 the impact's effects are drawn (`CanisterBlast`
/// at the impact); `StopSmoke` ends the smoke.
#[test]
fn a_canister_with_impact_effects_blasts_and_stops_smoking() {
    use hl2dm::game::set_pieces::CanisterBlast;
    #[derive(Resource, Default)]
    struct Seen(Vec<Vec3>);
    let mut c = case(vec![canister(Vec3::new(0.0, 0.0, 8.0), &[("spawnflags", "40962")])]);
    c.sim.app.init_resource::<Seen>().add_systems(
        FixedLast,
        |mut r: MessageReader<CanisterBlast>, mut seen: ResMut<Seen>| seen.0.extend(r.read().map(|b| b.at)),
    );
    c.input("can1", "FireCanister");
    c.sim.seconds(4.3);
    let seen = &c.sim.app.world().resource::<Seen>().0;
    assert_eq!(seen.len(), 1, "one blast");
    assert!(seen[0].distance(to_engine(Vec3::new(0.0, 0.0, 8.0))) < 0.01);
    assert!(shown(&mut c.sim)[0].0.starts_with("hl2dm:headcrabcanister_open"));
    c.input("can1", "StopSmoke");
    c.sim.ticks(3);
    assert!(shown(&mut c.sim)[0].0.starts_with("hl2dm:headcrabcanister_spent"));
}

#[test]
fn a_canister_beside_the_player_kills() {
    let mut c = case(vec![canister(Vec3::new(0.0, 0.0, 8.0), &[])]);
    c.place(Vec3::new(40.0, 0.0, 0.0));
    c.input("can1", "FireCanister");
    c.sim.seconds(4.6);
    assert!(c.health() <= 0.0, "full 150 damage inside a quarter of the radius: {}", c.health());
}

/// dm_runoff's quirk: the template removes the map's canisters at load, so
/// the first strike fires nothing; the template's copies fire.
#[test]
fn only_the_templates_copies_fire() {
    let mut c = case(vec![
        canister(Vec3::new(0.0, 0.0, 8.0), &[]),
        entity("point_template", Vec3::new(0.0, 900.0, 0.0), &[("targetname", "can_temp"), ("Template01", "can1"), ("spawnflags", "2")]),
    ]);
    c.input("can1", "FireCanister");
    c.sim.ticks(5);
    assert_eq!(c.fired("OnLaunched"), 0, "the map's own was removed at load");
    c.input("can_temp", "ForceSpawn");
    c.sim.ticks(5);
    c.input("can1", "FireCanister");
    c.sim.ticks(5);
    assert_eq!(c.fired("OnLaunched"), 1);
    c.input("can1", "Kill");
    c.sim.ticks(5);
}

#[test]
fn the_ragdoll_manager_sets_the_limit() {
    let mut c = case(vec![entity("game_ragdoll_manager", Vec3::ZERO, &[("targetname", "rm"), ("MaxRagdollCount", "4")])]);
    assert_eq!(c.sim.app.world().resource::<RagdollLimit>().current(), 4);
    c.sim
        .app
        .world_mut()
        .resource_mut::<Logic>()
        .world
        .queue_input("rm", "SetMaxRagdollCount", Value::Str("2".into()), 0.0, None);
    c.sim.ticks(3);
    assert_eq!(c.sim.app.world().resource::<RagdollLimit>().current(), 2);
}

/// dm_steamlab's gas trap: the microphone (impacts and blasts, 256 range)
/// hears a blast and a shot's impact within reach, not one beyond.
#[test]
fn the_microphone_hears_blasts() {
    let mut c = case(vec![entity(
        "env_microphone",
        Vec3::new(0.0, 0.0, 0.0),
        &[("targetname", "mic"), ("spawnflags", "40"), ("MaxRange", "256"), ("Sensitivity", "1")],
    )]);
    let blast = |at: Vec3| Explosion {
        origin: to_engine(at),
        damage: 0.5,
        radius: 100.0 * UNIT,
        attacker: None,
        inflictor: None,
        sound: None,
        weapon: None,
        round_scorch: false,
        bell_sigma: None,
        armor_ratio: None,
        through_walls: false,
        flags: Default::default(),
    };
    c.sim.app.world_mut().write_message(blast(Vec3::new(1000.0, 0.0, 0.0)));
    c.sim.seconds(1.0);
    assert_eq!(c.fired("OnHeardSound"), 0, "a blast 1000 away");
    // A blast of radius 100 carries 150 units: heard 120 away.
    c.sim.app.world_mut().write_message(blast(Vec3::new(120.0, 0.0, 0.0)));
    c.sim.seconds(0.5);
    assert!(c.fired("OnHeardSound") >= 1);
}
