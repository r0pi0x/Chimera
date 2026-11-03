//! HL2:DM's rules on `Hl2Sim` (specs/hl2dm/rules.md, H6's implementer
//! cases, "test N"): DM and TDM on the mode runtime, joining and teams,
//! the models, `jointeam` and `spectate`, the respawn, spawn spots and the
//! telefrag, the blast grace, scoring and team points, the limits, the
//! intermission and the next map, the restart and the ready-up. A floor at
//! z = 0, spawn spots along X; ticks at 0.015 s.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::{
    console::{CommandPlayer, execute},
    core::{Damage, DamageKind, Dead, Health, Hitgroup, Intent, MapCycle, Score, SimClock, Spectating, Team, UserInfo},
    game_log::{EventValue, GameEvent},
};
use chimera_sim::mode::Match;
use chimera_world::{
    harness::Sim,
    map::{MapBrush, MapBrushes, MapEntities, MapEntity},
};
use hl2dm::{
    game::{
        movement::ID,
        rules::{self, COMBINE, Hl2Match, Hl2Player, Hl2Rules, REBELS, SPECTATOR, UNASSIGNED, limits},
        weapons::inventory::Hl2Model,
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

fn entity(class: &str, at: Vec3, angles: Vec3) -> MapEntity {
    MapEntity {
        keyvalues: vec![
            ("classname".to_string(), class.to_string()),
            ("origin".to_string(), format!("{} {} {}", at.x, at.y, at.z)),
            ("angles".to_string(), format!("{} {} {}", angles.x, angles.y, angles.z)),
        ],
        ..Default::default()
    }
}

/// Three deathmatch spots 1000 units apart.
fn dm_spots() -> Vec<MapEntity> {
    (0..3)
        .map(|i| entity("info_player_deathmatch", Vec3::new(i as f32 * 1000.0, 0.0, 0.0), Vec3::ZERO))
        .collect()
}

#[derive(Resource, Default)]
struct Events(Vec<(u64, GameEvent)>);

fn listen(clock: Res<SimClock>, mut log: ResMut<Events>, mut events: MessageReader<GameEvent>) {
    log.0.extend(events.read().map(|e| (clock.tick, e.clone())));
}

struct Game {
    sim: Sim,
}

/// The rules on a map of `entities`, `teamplay`, `setup` before the start.
fn game(entities: Vec<MapEntity>, setup: impl FnOnce(&mut Hl2Rules)) -> Game {
    let mut sim = Sim::new(Hl2Sim, TestMap(entities));
    sim.set_tick_interval(0.015);
    sim.app.init_resource::<Events>().add_systems(FixedLast, listen);
    sim.ticks(1);
    setup(&mut sim.app.world_mut().resource_mut::<Hl2Rules>());
    rules::start(sim.app.world_mut());
    sim.ticks(1);
    Game { sim }
}

impl Game {
    fn world(&mut self) -> &mut World {
        self.sim.app.world_mut()
    }

    fn tick(&self) -> u64 {
        self.sim.app.world().resource::<SimClock>().tick
    }

    /// A player joining, with `cl_playermodel` `model`.
    fn join(&mut self, model: &str) -> Entity {
        let p = self.sim.spawn_character(to_engine(Vec3::new(-3000.0, 0.0, 36.0)), ID);
        let mut info = UserInfo::default();
        info.0.insert("cl_playermodel".into(), model.into());
        self.world().entity_mut(p).insert(info);
        self.sim.ticks(2);
        p
    }

    fn player(&self, p: Entity) -> Hl2Player {
        self.sim.app.world().get::<Hl2Player>(p).unwrap().clone()
    }

    fn score(&self, p: Entity) -> (i32, u32) {
        let s = self.sim.app.world().get::<Score>(p).unwrap();
        (s.kills, s.deaths)
    }

    fn teams(&self) -> [i32; 2] {
        self.sim.app.world().resource::<Match>().score
    }

    fn dead(&self, p: Entity) -> bool {
        self.sim.app.world().get::<Dead>(p).is_some()
    }

    /// `amount` points of `kind` to `victim` from `attacker`.
    fn hurt(&mut self, victim: Entity, attacker: Option<Entity>, amount: f32, kind: DamageKind) {
        self.world().write_message(Damage {
            target: victim,
            attacker,
            amount: amount / 100.0,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Y,
            hitgroup: Hitgroup::Generic,
            kind,
            weapon: None,
            force: Vec3::ZERO,
        });
    }

    fn kill(&mut self, victim: Entity, attacker: Option<Entity>) {
        self.hurt(victim, attacker, 1000.0, DamageKind::Bullet);
        self.sim.ticks(1);
    }

    /// Run a console line as `p`'s.
    fn command(&mut self, p: Entity, line: &str) {
        let w = self.world();
        w.insert_resource(CommandPlayer(p));
        let words: Vec<String> = line.split_whitespace().map(str::to_string).collect();
        execute(w, &words, 0);
        w.remove_resource::<CommandPlayer>();
    }

    fn events(&self, name: &str) -> Vec<(u64, GameEvent)> {
        self.sim.app.world().resource::<Events>().0.iter().filter(|(_, e)| e.name == name).cloned().collect()
    }

    fn feet(&self, p: Entity) -> Vec3 {
        to_source(self.sim.position(p)) - Vec3::Z * 36.0
    }

    fn over(&self) -> bool {
        self.sim.app.world().resource::<Hl2Match>().over
    }
}

/// Test 1: `mp_teamplay` is read at the map's load only.
#[test]
fn teamplay_is_read_at_load_1() {
    let mut g = game(dm_spots(), |_| {});
    let a = g.join("none");
    g.world().resource_mut::<Hl2Rules>().teamplay = 1;
    g.sim.seconds(10.0);
    assert!(!g.world().resource::<Hl2Match>().teamplay);
    assert_eq!(g.player(a).team, UNASSIGNED);
    let b = g.join("none");
    assert_eq!(g.player(b).team, UNASSIGNED, "a joiner too");
    // Every DM player is its own team to the engine.
    let (ta, tb) = (g.world().get::<Team>(a).copied(), g.world().get::<Team>(b).copied());
    assert_ne!(ta, tb);
}

/// Test 2: TDM joins the smaller team; `player_team` from 0.
#[test]
fn tdm_joins_the_smaller_team_2() {
    let mut g = game(dm_spots(), |r| r.teamplay = 1);
    let ps: Vec<Entity> = (0..3).map(|_| g.join("none")).collect();
    let combine = ps.iter().filter(|p| g.player(**p).team == COMBINE).count();
    let rebels = ps.iter().filter(|p| g.player(**p).team == REBELS).count();
    assert_eq!(combine + rebels, 3);
    assert!(combine.abs_diff(rebels) == 1, "{combine} {rebels}");
    let n = g.join("none");
    let team = g.player(n).team;
    assert_eq!(team, if combine < rebels { COMBINE } else { REBELS });
    let e = g.events("player_team").into_iter().map(|(_, e)| e).find(|e| e.player("userid") == Some(n)).unwrap();
    assert_eq!(e.get("team"), Some(&EventValue::Int(team as i64)));
    assert_eq!(e.get("oldteam"), Some(&EventValue::Int(0)));
    assert_eq!(e.get("autoteam"), Some(&EventValue::Bool(false)));
}

/// Tests 4, 5, 7: the models: the round robin (from the second), the
/// melee weapon by kind, DM's fallback.
#[test]
fn models_4_5_7() {
    let mut g = game(dm_spots(), |r| r.teamplay = 1);
    let mut combine = Vec::new();
    for _ in 0..6 {
        let p = g.join("models/humans/group03/male_01.mdl");
        if g.player(p).team == COMBINE {
            combine.push(g.player(p).model);
        }
    }
    let list = [
        "models/combine_soldier_prisonguard.mdl",
        "models/combine_super_soldier.mdl",
        "models/police.mdl",
        "models/combine_soldier.mdl",
    ];
    assert_eq!(combine, list[..combine.len()].to_vec(), "test 4: in turn from the second");
    let mut g = game(dm_spots(), |r| r.teamplay = 1);
    let mut rebel = None;
    for _ in 0..40 {
        if rebel.is_some() {
            break;
        }
        let p = g.join("models/police.mdl");
        if g.player(p).team == REBELS {
            rebel = Some(p);
        }
    }
    assert_eq!(g.player(rebel.unwrap()).model, "models/humans/group03/male_02.mdl", "test 5");
    g.sim.ticks(5);
    assert_eq!(g.world().get::<Hl2Model>(rebel.unwrap()).unwrap().0, "models/humans/group03/male_02.mdl");

    let mut g = game(dm_spots(), |_| {});
    let p = g.join("none");
    assert_eq!(g.player(p).model, "models/combine_soldier.mdl", "test 7");
}

/// Tests 8, 9: a DM model change within 5 s is refused; after, it is at
/// once.
#[test]
fn model_changes_8_9() {
    let mut g = game(dm_spots(), |_| {});
    let p = g.join("models/combine_soldier.mdl");
    g.sim.seconds(2.0);
    g.world().get_mut::<UserInfo>(p).unwrap().0.insert("cl_playermodel".into(), "models/humans/group03/male_01.mdl".into());
    g.sim.ticks(1);
    assert_eq!(g.player(p).model, "models/combine_soldier.mdl", "test 8: refused");
    assert_eq!(g.world().get::<UserInfo>(p).unwrap().0["cl_playermodel"], "models/combine_soldier.mdl");
    g.sim.seconds(4.0);
    g.world().get_mut::<UserInfo>(p).unwrap().0.insert("cl_playermodel".into(), "models/police.mdl".into());
    g.sim.ticks(1);
    assert_eq!(g.player(p).model, "models/police.mdl", "test 9");
    assert!(!g.dead(p));
}

/// Tests 10-12: TDM team changes: by model, by `jointeam` (0.3 s rate),
/// a suicide charged to the new team, alive within the suicide limit.
#[test]
fn team_changes_10_11_12() {
    let mut g = game(dm_spots(), |r| r.teamplay = 1);
    let mut p = g.join("models/combine_soldier.mdl");
    for _ in 0..40 {
        if g.player(p).team == COMBINE {
            break;
        }
        p = g.join("models/combine_soldier.mdl");
    }
    g.sim.seconds(6.0);
    let before = g.score(p);
    let teams = g.teams();
    g.world().get_mut::<UserInfo>(p).unwrap().0.insert("cl_playermodel".into(), "models/humans/group03/female_01.mdl".into());
    g.sim.ticks(2);
    assert_eq!(g.player(p).team, REBELS, "test 10");
    assert!(g.dead(p));
    assert_eq!(g.score(p), (before.0 - 1, before.1 + 1));
    assert_eq!(g.teams(), [teams[0], teams[1] - 1], "the Rebels' point");

    // 11: two jointeams 0.15 s apart: the second ignored.
    let mut g = game(dm_spots(), |r| r.teamplay = 1);
    let mut p = g.join("none");
    for _ in 0..40 {
        if g.player(p).team == COMBINE {
            break;
        }
        p = g.join("none");
    }
    g.sim.seconds(6.0);
    g.command(p, "hl2dm:jointeam 3");
    g.sim.seconds(0.15);
    g.command(p, "hl2dm:jointeam 2");
    g.sim.ticks(1);
    assert_eq!(g.player(p).team, REBELS, "test 11");
    // 12: back within the suicide limit after respawning: alive.
    g.sim.seconds(3.5);
    g.sim.intent(p).fire = true;
    g.sim.ticks(2);
    g.sim.intent(p).fire = false;
    assert!(!g.dead(p), "respawned");
    g.sim.seconds(0.5);
    g.command(p, "hl2dm:jointeam 2");
    g.sim.ticks(1);
    assert_eq!(g.player(p).team, COMBINE);
    assert!(!g.dead(p), "test 12: alive");
    assert!(!rules::players::PlayerModels::hl2().citizens.contains(&g.player(p).model));
}

/// Tests 13-16: `spectate` costs a TDM death, nothing in DM; refused
/// without `mp_allowspectators`; `jointeam 0`/`5` change nothing.
#[test]
fn spectating_13_to_16() {
    let mut g = game(dm_spots(), |r| r.teamplay = 1);
    let p = g.join("none");
    g.world().get_mut::<Score>(p).unwrap().kills = 4;
    let team = g.player(p).team;
    let teams = g.teams();
    g.command(p, "hl2dm:spectate");
    g.sim.ticks(1);
    assert_eq!(g.player(p).team, SPECTATOR);
    assert!(g.world().get::<Spectating>(p).is_some());
    assert_eq!(g.score(p), (4, 1), "test 13");
    let i = (team - COMBINE) as usize;
    assert_eq!(g.teams()[i], teams[i] - 1);

    let mut g = game(dm_spots(), |_| {});
    let p = g.join("none");
    *g.world().get_mut::<Score>(p).unwrap() = Score { kills: 4, deaths: 2 };
    g.command(p, "hl2dm:spectate");
    g.sim.ticks(1);
    assert_eq!(g.score(p), (4, 2), "test 14");
    assert!(g.events("player_death").is_empty());

    let mut g = game(dm_spots(), |r| r.allowspectators = 0);
    let p = g.join("none");
    g.command(p, "hl2dm:jointeam 1");
    g.command(p, "hl2dm:jointeam 0");
    g.command(p, "hl2dm:jointeam 5");
    g.sim.ticks(1);
    assert_eq!(g.player(p).team, UNASSIGNED, "tests 15, 16");
}

/// Tests 17-21: the respawn: a press before 3 s does nothing; held from
/// tick 100, tick 201; buttons held through death count from their
/// release; the force at tick 334; none without it.
#[test]
fn respawns_17_to_21() {
    let mut g = game(dm_spots(), |_| {});
    let p = g.join("none");
    let k = g.join("none");
    g.kill(p, Some(k));
    let died = g.player(p).death_tick.unwrap();
    // Up to tick 99 nothing; a press at 100 (one tick).
    while g.tick() < died + 99 {
        g.sim.ticks(1);
    }
    g.sim.intent(p).fire = true;
    g.sim.ticks(1);
    assert!(g.dead(p), "test 17");
    // Held from then on (the dead's intent is cleared every tick).
    let mut back = None;
    for _ in 0..200 {
        g.sim.intent(p).fire = true;
        g.sim.ticks(1);
        if !g.dead(p) {
            back = Some(g.tick() - died);
            break;
        }
    }
    assert_eq!(back, Some(201), "test 18");
    g.sim.intent(p).fire = false;

    // 19: held through death until 300, pressed at 305.
    g.sim.seconds(1.0);
    g.sim.intent(p).fire = true;
    g.kill(p, Some(k));
    let died = g.player(p).death_tick.unwrap();
    let mut back = None;
    for _ in 0..400 {
        let t = g.tick() + 1 - died;
        g.sim.intent(p).fire = t <= 300 || t >= 305;
        g.sim.ticks(1);
        if !g.dead(p) {
            back = Some(g.tick() - died);
            break;
        }
    }
    assert_eq!(back, Some(305), "test 19");
    g.sim.intent(p).fire = false;

    // 20: forced at 334.
    g.world().resource_mut::<Hl2Rules>().forcerespawn = 1;
    g.sim.seconds(1.0);
    g.kill(p, Some(k));
    let died = g.player(p).death_tick.unwrap();
    while g.dead(p) && g.tick() < died + 400 {
        g.sim.ticks(1);
    }
    assert_eq!(g.tick() - died, 334, "test 20");

    // 21: without it, still dead after 60 s.
    g.world().resource_mut::<Hl2Rules>().forcerespawn = 0;
    g.sim.seconds(1.0);
    g.kill(p, Some(k));
    g.sim.seconds(60.0);
    assert!(g.dead(p), "test 21");
}

/// Tests 26-29, 32: spots: an occupied spot is skipped; with all taken the
/// telefrag kills within 48 of the first candidate; TDM falls back to the
/// deathmatch class; `info_player_start` last; the spawn's place.
#[test]
fn spawn_spots_26_to_29_32() {
    // 32: one spot.
    let spot = entity("info_player_deathmatch", Vec3::new(100.0, 200.0, 0.0), Vec3::new(10.0, 90.0, 0.0));
    let mut g = game(vec![spot], |_| {});
    let p = g.join("none");
    let feet = g.feet(p);
    assert!((feet - Vec3::new(100.0, 200.0, 1.0)).length() < 0.6, "test 32: {feet}");
    let i = g.world().get::<Intent>(p).unwrap().clone();
    assert!((i.yaw - 0.0).abs() < 1e-3 && (i.pitch + 10f32.to_radians()).abs() < 1e-3, "view (10, 90)");
    assert_eq!(g.world().get::<Health>(p).unwrap().current, 1.0);

    // 26: a spot with a player within 128 is skipped.
    let mut g = game(dm_spots(), |_| {});
    let a = g.join("none");
    let b = g.join("none");
    let c = g.join("none");
    let mut spots: Vec<f32> = [a, b, c].iter().map(|p| (g.feet(*p).x / 1000.0).round()).collect();
    spots.sort_by(f32::total_cmp);
    assert_eq!(spots, [0.0, 1.0, 2.0], "three free spots, one each");

    // 27: two spots, all taken: the one 30 from the first candidate dies.
    let two = vec![
        entity("info_player_deathmatch", Vec3::new(0.0, 0.0, 0.0), Vec3::ZERO),
        entity("info_player_deathmatch", Vec3::new(1000.0, 0.0, 0.0), Vec3::ZERO),
    ];
    let mut g = game(two, |_| {});
    let a = g.join("none");
    let b = g.join("none");
    let telefrags_before = g.events("player_death").len();
    let _c = g.join("none");
    g.sim.ticks(2);
    let deaths: Vec<GameEvent> = g.events("player_death").into_iter().skip(telefrags_before).map(|(_, e)| e).collect();
    assert_eq!(deaths.len(), 1, "one telefrag");
    assert_eq!(deaths[0].get("weapon"), Some(&EventValue::Str("worldspawn".into())));
    assert_eq!(deaths[0].get("attacker"), Some(&EventValue::Int(0)));
    let victim = deaths[0].player("userid").unwrap();
    assert!(victim == a || victim == b);
    assert_eq!(g.score(victim), (-1, 1), "a world kill");

    // 28: TDM with no rebel spots: the deathmatch class.
    let mut g = game(
        vec![
            entity("info_player_combine", Vec3::new(0.0, 0.0, 0.0), Vec3::ZERO),
            entity("info_player_deathmatch", Vec3::new(2000.0, 0.0, 0.0), Vec3::ZERO),
        ],
        |r| r.teamplay = 1,
    );
    for _ in 0..4 {
        let p = g.join("none");
        let x = g.feet(p).x;
        if g.player(p).team == REBELS {
            assert!((x - 2000.0).abs() < 200.0, "test 28: {x}");
        } else {
            assert!(x.abs() < 200.0, "combine at its own: {x}");
        }
    }
    // 29: no class spot: info_player_start.
    let mut g = game(vec![entity("info_player_start", Vec3::new(300.0, 0.0, 0.0), Vec3::ZERO)], |_| {});
    let p = g.join("none");
    assert!((g.feet(p).x - 300.0).abs() < 1.0, "test 29");
}

/// Tests 30, 31: pure blast within 0.5 s of the spawn is refused.
#[test]
fn spawn_blast_grace_30_31() {
    let mut g = game(dm_spots(), |_| {});
    let p = g.join("none");
    let k = g.join("none");
    g.kill(p, Some(k));
    g.sim.seconds(3.5);
    g.sim.intent(p).fire = true;
    g.sim.ticks(1);
    g.sim.intent(p).fire = false;
    assert!(!g.dead(p));
    let spawned = g.player(p).spawned_at;
    // Up to the first tick 0.5 s on: refused.
    g.hurt(p, Some(k), 50.0, DamageKind::Blast);
    g.sim.ticks(1);
    assert_eq!(g.world().get::<Health>(p).unwrap().current, 1.0, "test 30: refused");
    for _ in 0..100 {
        if g.world().resource::<SimClock>().now >= spawned + 0.5 {
            break;
        }
        g.sim.ticks(1);
    }
    g.hurt(p, Some(k), 50.0, DamageKind::Blast);
    g.sim.ticks(1);
    assert!(g.world().get::<Health>(p).unwrap().current < 1.0, "test 30: after");
    // 31: a burn (not pure blast) lands at once.
    g.kill(p, Some(k));
    g.sim.seconds(3.5);
    g.sim.intent(p).fire = true;
    g.sim.ticks(1);
    g.sim.intent(p).fire = false;
    g.hurt(p, Some(k), 10.0, DamageKind::Burn);
    g.sim.ticks(1);
    assert!(g.world().get::<Health>(p).unwrap().current < 1.0, "test 31");
}

/// Tests 33, 34, 56: the frag limits; team kills; intermission frozen.
#[test]
fn frag_limits_33_34_56() {
    let mut g = game(dm_spots(), |r| r.fraglimit = 3.0);
    let a = g.join("none");
    let b = g.join("none");
    g.world().get_mut::<Score>(a).unwrap().kills = 2;
    g.kill(b, Some(a));
    g.sim.ticks(1);
    assert!(g.over(), "test 33");
    assert!(g.world().resource::<chimera_core::core::FreezeTime>().0);
    g.sim.intent(a).move_axis = Vec2::Y;
    g.sim.ticks(1);
    assert_eq!(g.world().get::<Intent>(a).unwrap().move_axis, Vec2::ZERO, "frozen");

    // 34: TDM, a team kill brings the killer's team to the limit.
    let mut g = game(dm_spots(), |r| {
        r.teamplay = 1;
        r.fraglimit = 3.0;
        r.friendlyfire = 1;
    });
    let mut ps = Vec::new();
    for _ in 0..40 {
        if ps.iter().filter(|p| g.player(**p).team == COMBINE).count() >= 2 {
            break;
        }
        ps.push(g.join("none"));
    }
    let combine: Vec<Entity> = ps.iter().copied().filter(|p| g.player(*p).team == COMBINE).collect();
    g.world().resource_mut::<Match>().score[0] = 2;
    g.kill(combine[1], Some(combine[0]));
    g.sim.ticks(1);
    assert_eq!(g.score(combine[0]).0, -1, "test 34: the killer loses a frag");
    assert_eq!(g.teams()[0], 3, "its team +1");
    assert!(g.over());

    // 56: a negative limit is met at 0 frags.
    let mut g = game(dm_spots(), |r| r.fraglimit = -1.0);
    g.join("none");
    g.sim.ticks(1);
    assert!(g.over(), "test 56");
}

/// Tests 35-40: the time limit; the intermission's 10 s; the next map;
/// the dead stay dead.
#[test]
fn time_limit_and_map_change_35_to_40() {
    let mut g = game(dm_spots(), |r| r.timelimit = 1.0);
    let start = g.world().resource::<Hl2Match>().game_start;
    let a = g.join("none");
    let b = g.join("none");
    g.world().insert_resource(MapCycle {
        maps: rules::DEFAULT_CYCLE.iter().map(|m| m.to_string()).collect(),
        current: "dm_steamlab".into(),
    });
    let mut ended = None;
    for _ in 0..5000 {
        if ended.is_some() {
            break;
        }
        g.sim.ticks(1);
        if g.over() {
            ended = Some(g.world().resource::<SimClock>().now);
        }
    }
    let t = ended.expect("the time limit ended the game") - start;
    assert!(t > 60.0 && t < 60.0 + 0.016, "test 35: {t}");
    g.kill(b, Some(a));
    assert_eq!(g.score(a), (0, 0), "test 41: no frags in the intermission");
    let over_at = ended.unwrap();
    for _ in 0..1000 {
        if g.world().resource::<Hl2Match>().next_map.is_some() {
            break;
        }
        g.sim.intent(b).fire = true;
        g.sim.ticks(1);
    }
    let t = g.world().resource::<SimClock>().now - over_at;
    assert!(t > 10.0 && t < 10.0 + 0.016, "test 36: {t}");
    assert!(g.dead(b), "test 40");
    assert_eq!(g.world().resource::<Hl2Match>().next_map.as_deref(), Some("dm_underpass"), "test 37");
    let cycle = |current: &str| {
        let mut w = World::new();
        w.insert_resource(Hl2Rules::default());
        w.insert_resource(MapCycle {
            maps: rules::DEFAULT_CYCLE.iter().map(|m| m.to_string()).collect(),
            current: current.into(),
        });
        limits::next_map(&w)
    };
    assert_eq!(cycle("dm_underpass").as_deref(), Some("dm_lockdown"), "test 37");
    assert_eq!(cycle("halls3").as_deref(), Some("dm_lockdown"), "test 38");
    let mut w = World::new();
    w.insert_resource(Hl2Rules {
        nextlevel: "dm_runoff".into(),
        ..default()
    });
    assert_eq!(limits::next_map(&w).as_deref(), Some("dm_runoff"), "test 39");
}

/// Tests 42, 43: `mp_restartgame`.
#[test]
fn restart_42_43() {
    let mut g = game(dm_spots(), |_| {});
    let a = g.join("none");
    let b = g.join("none");
    *g.world().get_mut::<Score>(a).unwrap() = Score { kills: 5, deaths: 3 };
    g.kill(b, Some(a));
    g.sim.seconds(3.5);
    g.sim.intent(b).fire = true;
    g.sim.ticks(1);
    g.sim.intent(b).fire = false;
    assert!(!g.dead(b));
    g.world().resource_mut::<Hl2Rules>().restartgame = 2.0;
    let asked = g.world().resource::<SimClock>().now;
    g.sim.ticks(1);
    // b dies again just before the restart.
    for _ in 0..400 {
        if !g.events("round_start").is_empty() {
            break;
        }
        if g.world().resource::<Hl2Match>().restart_at.is_some_and(|r| g.sim.app.world().resource::<SimClock>().now > r - 1.0) && !g.dead(b) {
            g.kill(b, Some(a));
        }
        g.sim.ticks(1);
    }
    let t = g.world().resource::<SimClock>().now - asked;
    assert!(t <= 3.1, "test 42: {t}");
    assert_eq!(g.score(a), (0, 0));
    assert_eq!(g.score(b), (0, 0));
    assert!(g.dead(b), "dead 1 s: not respawned");
    assert_eq!(g.events("round_start")[0].1.get("objective"), Some(&EventValue::Str("DEATHMATCH".into())));

    let mut g = game(dm_spots(), |_| {});
    g.world().resource_mut::<Hl2Rules>().restartgame = 90.0;
    g.sim.seconds(1.1);
    let at = g.world().resource::<Hl2Match>().restart_at.unwrap() - g.world().resource::<SimClock>().now;
    assert!(at > 58.0 && at <= 60.0, "test 43: {at}");
}

/// Tests 44-46: the ready-up (case sensitive, lost on a spawn).
#[test]
fn ready_up_44_45_46() {
    let say = |g: &mut Game, p: Entity, text: &str| {
        g.world()
            .write_message(GameEvent::of("player_say", p).with("text", EventValue::Str(text.into())));
    };
    let mut g = game(dm_spots(), |_| {});
    let a = g.join("none");
    let b = g.join("none");
    g.world().resource_mut::<Hl2Rules>().readyrestart = 1;
    g.sim.seconds(1.1);
    say(&mut g, a, "ready");
    say(&mut g, b, "Ready");
    g.sim.seconds(2.0);
    assert!(g.world().resource::<Hl2Match>().restart_at.is_none(), "test 45");
    say(&mut g, b, "ready");
    g.sim.seconds(1.1);
    assert!(g.world().resource::<Hl2Match>().restart_at.is_some(), "test 44");

    let mut g = game(dm_spots(), |_| {});
    let a = g.join("none");
    let b = g.join("none");
    g.world().resource_mut::<Hl2Rules>().readyrestart = 1;
    g.sim.seconds(1.1);
    say(&mut g, a, "ready");
    g.sim.ticks(2);
    g.kill(a, Some(b));
    g.sim.seconds(3.5);
    g.sim.intent(a).fire = true;
    g.sim.ticks(1);
    g.sim.intent(a).fire = false;
    say(&mut g, b, "ready");
    g.sim.seconds(2.0);
    assert!(g.world().resource::<Hl2Match>().restart_at.is_none(), "test 46");
}

/// Tests 53, 54: a fall is a world kill; `kill` twice 3 s apart.
#[test]
fn falls_and_suicides_53_54() {
    let mut g = game(dm_spots(), |_| {});
    let a = g.join("none");
    g.hurt(a, Some(a), 1000.0, DamageKind::Fall);
    g.sim.ticks(1);
    assert_eq!(g.score(a), (-1, 1));
    let e = g.events("player_death").pop().unwrap().1;
    assert_eq!(e.get("weapon"), Some(&EventValue::Str("worldspawn".into())));
    assert_eq!(e.get("attacker"), Some(&EventValue::Int(0)));

    // `kill`: generic damage from the player itself (the engine's command).
    let mut g = game(dm_spots(), |_| {});
    let a = g.join("none");
    g.hurt(a, Some(a), 1000.0, DamageKind::Generic);
    g.sim.ticks(2);
    assert!(g.dead(a));
    g.sim.seconds(3.5);
    g.sim.intent(a).fire = true;
    g.sim.ticks(1);
    g.sim.intent(a).fire = false;
    assert!(!g.dead(a));
    g.hurt(a, Some(a), 1000.0, DamageKind::Generic);
    g.sim.ticks(2);
    assert!(!g.dead(a), "test 54: refused within 5 s");
}

/// Test 55: `timeleft` in the last second says there is none.
#[test]
fn timeleft_55() {
    let mut g = game(dm_spots(), |r| r.timelimit = 1.0);
    let start = g.world().resource::<Hl2Match>().game_start;
    assert!(limits::time_left(g.world()).is_some());
    for _ in 0..1000 {
        if g.world().resource::<SimClock>().now >= start + 59.4 {
            break;
        }
        g.sim.ticks(10);
    }
    assert_eq!(limits::time_left(g.world()), None, "test 55");
}
