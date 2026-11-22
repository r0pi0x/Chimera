//! Bomb and hostage objectives (specs/cs_source/objectives.md, test cases
//! O1-O42) on the greybox map with CS:S's weapons, at CS:S's tick.

use avian3d::prelude::Position;
use bevy::prelude::*;
use crate::stack::{
    console::Console,
    core::{Damage, Health, Hitgroup, SimTick, Team},
    games::cs_source::{TICK_INTERVAL, objectives::C4, weapons::CsWeaponsPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
    movement::placeholder,
    objectives::{
        HostageSpawn, MapObjectives, ObjectiveEvent, Zone,
        bomb::{BombOutcome, BombState, C4 as C4Part, LooseKit, PlantedBomb, carried_bomb},
        hostages::{Hostage, HostageTally},
    },
    rules::rounds::{Mvp, Phase, RoundEndReason, RoundEnded, RoundState},
    weapon::{
        Inventory, Weapon,
        drop::Loose,
        economy::{BuyWindow, DefuseKit, Money, buy},
        give,
    },
};

/// Every objective event with the tick it happened on.
#[derive(Resource, Default)]
struct Log(Vec<(u64, ObjectiveEvent)>, Vec<RoundEnded>);

fn record(
    mut events: MessageReader<ObjectiveEvent>,
    mut ended: MessageReader<RoundEnded>,
    tick: Res<SimTick>,
    mut log: ResMut<Log>,
) {
    for e in events.read() {
        log.0.push((tick.0, e.clone()));
    }
    log.1.extend(ended.read().cloned());
}

/// The bomb target: a box around the greybox's first spawn.
const ZONE: (Vec3, Vec3) = (Vec3::new(-3.0, -1.0, 8.0), Vec3::new(3.0, 3.0, 16.0));

fn sim(objectives: MapObjectives) -> Sim {
    sim_with(objectives, ())
}

fn sim_with<M>(objectives: MapObjectives, more: impl bevy::app::Plugins<M>) -> Sim {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin, more));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.init_resource::<Log>();
    sim.app.add_systems(FixedLast, record);
    sim.app.insert_resource(objectives);
    sim
}

fn bomb_map() -> MapObjectives {
    MapObjectives {
        bomb_targets: vec![Zone::engine_box(ZONE.0, ZONE.1)],
        ..default()
    }
}

fn character(sim: &mut Sim, at: Vec3, team: u8) -> Entity {
    let e = sim.spawn_character(at, placeholder::ID);
    sim.app.world_mut().entity_mut(e).insert(Team(team));
    e
}

fn teleport(sim: &mut Sim, e: Entity, at: Vec3) {
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(e).unwrap().translation = at;
    if let Some(mut p) = w.get_mut::<Position>(e) {
        p.0 = at;
    }
}

fn events(sim: &Sim) -> Vec<(u64, ObjectiveEvent)> {
    sim.app.world().resource::<Log>().0.clone()
}

fn first(sim: &Sim, f: impl Fn(&ObjectiveEvent) -> bool) -> Option<u64> {
    events(sim).into_iter().find(|(_, e)| f(e)).map(|(t, _)| t)
}

fn bombs(sim: &mut Sim) -> Vec<(Entity, PlantedBomb, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &PlantedBomb, &Transform)>()
        .iter(w)
        .map(|(e, b, t)| (e, b.clone(), t.translation))
        .collect()
}

fn money(sim: &Sim, e: Entity) -> u32 {
    sim.app.world().get::<Money>(e).unwrap().0
}

/// What the round's end pays `e` as the next round starts (spec 8:
/// CS:S's accounts change then, measured).
fn paid(sim: &Sim, e: Entity) -> Option<u32> {
    let state = sim.app.world().resource::<RoundState>();
    state.payouts.iter().find(|(p, _)| *p == e).map(|(_, m)| *m)
}

/// A terrorist in the target with the bomb drawn.
fn carrier(sim: &mut Sim) -> Entity {
    let t = character(sim, Vec3::new(0.0, 1.0, 12.0), 1);
    sim.ticks(2);
    give(sim.app.world_mut(), t, C4).unwrap();
    sim.seconds(1.2);
    let active = sim.app.world().get::<Inventory>(t).unwrap().active.unwrap();
    assert!(sim.app.world().get::<C4Part>(active).is_some(), "the bomb is drawn");
    t
}

/// Hold fire until the bomb is planted; returns the tick it began.
fn plant(sim: &mut Sim, t: Entity) -> u64 {
    sim.intent(t).fire = true;
    sim.ticks(1);
    let begun = first(sim, |e| matches!(e, ObjectiveEvent::BeginPlant { .. })).expect("arming began");
    sim.ticks(205);
    sim.intent(t).fire = false;
    sim.ticks(1);
    begun
}

/// Aim `who` at a point from where it stands (its eye).
fn look_at(sim: &mut Sim, who: Entity, at: Vec3) {
    let eye = sim.position(who) + sim.state(who).eye_offset;
    let d = at - eye;
    let mut i = sim.intent(who);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(d.xz().length());
}

#[test]
fn arming_takes_three_seconds_and_starts_over_when_let_go() {
    let mut sim = sim(bomb_map());
    let t = carrier(&mut sim);
    // O5: outside the target nothing starts.
    teleport(&mut sim, t, Vec3::new(10.0, 1.0, 12.0));
    sim.ticks(3);
    sim.intent(t).fire = true;
    sim.ticks(2);
    sim.intent(t).fire = false;
    sim.ticks(2);
    let refused = events(&sim).into_iter().any(|(_, e)| {
        matches!(e, ObjectiveEvent::PlantRefused { reason, .. } if reason == "C4 must be planted at a bomb site.")
    });
    assert!(refused);
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::BeginPlant { .. })).is_none());
    // O7: let go at tick 190: no bomb.
    teleport(&mut sim, t, Vec3::new(0.0, 1.0, 12.0));
    sim.ticks(3);
    sim.intent(t).fire = true;
    sim.ticks(190);
    sim.intent(t).fire = false;
    sim.ticks(10);
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::AbortPlant { .. })).is_some());
    assert!(bombs(&mut sim).is_empty());
    // O6: hold it: planted 200 ticks (3.000 s) after it began.
    sim.app.world_mut().resource_mut::<Log>().0.clear();
    let begun = plant(&mut sim, t);
    let planted = first(&sim, |e| matches!(e, ObjectiveEvent::Planted { .. })).expect("planted");
    assert_eq!(planted - begun, 200);
    assert!(carried_bomb(sim.app.world(), t).is_none(), "the bomb left his hands");
    let (_, b, at) = bombs(&mut sim).pop().unwrap();
    assert!((b.explode_at - b.planted_at - 45.0).abs() < 1e-6);
    assert!((at - sim.position(t)).xz().length() < 0.01, "at his feet");
    assert!(sim.app.world().resource::<BombState>().planted.is_some());
}

/// The bomb's blast has no attacker (measured: objectives.md Q3, every
/// `player_hurt` from it with attacker 0), so with `mp_friendlyfire 0` it
/// still hurts the planter's teammates (and the planter).
#[test]
fn the_bomb_hurts_terrorists_with_friendly_fire_off() {
    let mut sim = sim(bomb_map());
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_friendlyfire 0; mp_c4timer 10");
    sim.ticks(2);
    let t = carrier(&mut sim);
    let mate = character(&mut sim, Vec3::new(3.0, 1.0, 12.0), 1);
    plant(&mut sim, t);
    sim.seconds(10.5);
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::Exploded { .. })).is_some(), "exploded");
    let hurt = |e: Entity| sim.app.world().get::<Health>(e).is_none_or(|h| h.current < h.max);
    assert!(hurt(mate), "a teammate near the bomb is hurt with friendly fire off");
    assert!(hurt(t), "and the planter");
}

/// CS:S's bomb blast traces nothing (measured 2026-10-18, objectives.md
/// Q3: a bot behind a 200-unit stone wall took the bell's whole 418): a
/// wall between saves nobody.
#[test]
fn the_bomb_blast_goes_through_walls() {
    use avian3d::prelude::{Collider, RigidBody};
    let mut sim = sim(bomb_map());
    sim.app.world_mut().resource_mut::<Console>().submit("mp_c4timer 10");
    sim.ticks(2);
    let t = carrier(&mut sim);
    let ct = character(&mut sim, Vec3::new(0.0, 1.0, 7.0), 2);
    // A 3 m high, 6 m wide wall between, 2.5 m from the bomb.
    sim.app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(6.0, 3.0, 0.5),
        Transform::from_xyz(0.0, 1.5, 9.5),
    ));
    plant(&mut sim, t);
    sim.seconds(10.5);
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::Exploded { .. })).is_some(), "exploded");
    let hurt = sim.app.world().get::<Health>(ct).is_none_or(|h| h.current < h.max);
    assert!(hurt, "the wall stopped none of it");
}

/// A planted bomb and a defender beside it, looking at it.
fn planted_with_defender(kit: bool) -> (Sim, Entity, Entity, u64) {
    let mut sim = sim(bomb_map());
    let t = carrier(&mut sim);
    plant(&mut sim, t);
    let planted = first(&sim, |e| matches!(e, ObjectiveEvent::Planted { .. })).unwrap();
    teleport(&mut sim, t, Vec3::new(20.0, 1.0, 30.0));
    let (_, _, at) = bombs(&mut sim).pop().unwrap();
    let ct = character(&mut sim, at + Vec3::new(0.0, 1.0, 1.0), 2);
    if kit {
        sim.app.world_mut().entity_mut(ct).insert(DefuseKit);
    }
    sim.ticks(2);
    look_at(&mut sim, ct, at + Vec3::Y * 0.05);
    (sim, t, ct, planted)
}

fn tick(sim: &Sim) -> u64 {
    sim.app.world().resource::<SimTick>().0
}

fn wait_until(sim: &mut Sim, tick_no: u64) {
    let now = tick(sim);
    assert!(tick_no >= now, "already past tick {tick_no}");
    sim.ticks(tick_no - now);
}

#[test]
fn defusing_with_a_kit_takes_five_seconds() {
    // O14: start at 30.0 s, defused on the bomb's first think at or after
    // 5 s (it thinks 7 ticks after the plant and every 8 after: CS:S,
    // measured 2026-10-18, objectives.md Q14).
    let (mut sim, _, ct, planted) = planted_with_defender(true);
    wait_until(&mut sim, planted + 2000 - 1);
    sim.intent(ct).use_key = true;
    sim.ticks(1);
    let begun = first(&sim, |e| matches!(e, ObjectiveEvent::BeginDefuse { kit: true, .. })).expect("defuse began");
    assert_eq!(begun - planted, 2000);
    sim.ticks(340);
    let done = first(&sim, |e| matches!(e, ObjectiveEvent::Defused { .. })).expect("defused");
    assert_eq!(done - planted, 2335, "the think at 7 + 8 x 291 ticks");
    assert_eq!(done - begun, 335);
    assert_eq!(
        sim.app.world().resource::<BombState>().outcome,
        Some(BombOutcome::Defused)
    );
    // It no longer explodes.
    sim.seconds(12.0);
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::Exploded { .. })).is_none());
}

#[test]
fn defusing_without_a_kit_takes_ten_and_letting_go_starts_over() {
    // O15.
    let (mut sim, _, ct, planted) = planted_with_defender(false);
    wait_until(&mut sim, planted + 2000 - 1);
    sim.intent(ct).use_key = true;
    // O17's interrupt: let go after 4 s, again 0.5 s later.
    sim.seconds(4.0);
    sim.intent(ct).use_key = false;
    sim.seconds(0.5);
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::AbortDefuse { .. })).is_some());
    let again = tick(&sim);
    sim.intent(ct).use_key = true;
    sim.seconds(10.2);
    let done = first(&sim, |e| matches!(e, ObjectiveEvent::Defused { .. })).expect("defused");
    let started = again + 1 - planted;
    let think = (started * 3 + 2000).div_ceil(3).max(7);
    let think = 7 + (think - 7).div_ceil(8) * 8;
    assert_eq!(done - planted, think, "the full 10 s again, to the bomb's next think");
    assert!((667..675).contains(&(done - (again + 1))));
}

#[test]
fn a_defuse_ending_after_the_timer_loses_to_the_explosion() {
    // O16: no kit, starting 36 s in: it would end at 46.
    let (mut sim, _, ct, planted) = planted_with_defender(false);
    #[derive(Resource, Default)]
    struct Scorches(Vec<bool>);
    sim.app.init_resource::<Scorches>().add_systems(
        FixedLast,
        |mut asks: MessageReader<crate::stack::map::PlaceDecal>, mut seen: ResMut<Scorches>| {
            seen.0.extend(asks.read().map(|a| a.round));
        },
    );
    let far = character(&mut sim, Vec3::new(30.0, 1.0, -30.0), 2);
    wait_until(&mut sim, planted + 2400 - 1);
    sim.intent(ct).use_key = true;
    sim.seconds(9.5);
    let exploded = first(&sim, |e| matches!(e, ObjectiveEvent::Exploded { .. })).expect("exploded");
    assert_eq!(exploded - planted, 3007, "on the bomb's first think 45 s after the plant (45.105 s, as CS:S)");
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::Defused { .. })).is_none());
    sim.ticks(3);
    // The blast: the defender beside it dies; one 2000 units off (past
    // 3.5 x 500) takes nothing.
    assert!(sim.app.world().get::<Health>(ct).unwrap().current <= 0.0);
    let far_health = sim.app.world().get::<Health>(far).unwrap().current;
    assert!(far_health > 0.8, "far defender took {far_health}");
    assert!(bombs(&mut sim).is_empty(), "the bomb is gone");
    // Its scorch goes at the round restart (playtest 2026-10-14, as CS:S).
    assert_eq!(sim.app.world().resource::<Scorches>().0, [true], "one scorch, the round's");
}

/// A defuse and the blast due on the same think of the bomb: it blows
/// (CS:S, measured 2026-10-18, objectives.md Q14: a kit's defuse started
/// 330-337 ticks after a 10 s plant, both due on the think 671 ticks
/// after it, exploded; one started at 329 was done on the think before).
#[test]
fn a_defuse_due_on_the_blasts_think_loses_to_it() {
    for (start, defused) in [(2665, true), (2667, false), (2674, false)] {
        let (mut sim, _, ct, planted) = planted_with_defender(true);
        wait_until(&mut sim, planted + start - 1);
        sim.intent(ct).use_key = true;
        sim.seconds(6.0);
        let begun = first(&sim, |e| matches!(e, ObjectiveEvent::BeginDefuse { .. })).unwrap();
        assert_eq!(begun - planted, start);
        let done = first(&sim, |e| matches!(e, ObjectiveEvent::Defused { .. }));
        let exploded = first(&sim, |e| matches!(e, ObjectiveEvent::Exploded { .. }));
        if defused {
            assert_eq!(done.map(|d| d - planted), Some(2999), "start {start}");
            assert!(exploded.is_none(), "start {start}");
        } else {
            assert!(done.is_none(), "start {start}");
            assert_eq!(exploded.map(|d| d - planted), Some(3007), "start {start}");
        }
    }
}

#[test]
fn another_defender_is_told_it_is_taken() {
    // O18.
    let (mut sim, _, ct, _) = planted_with_defender(true);
    let (_, _, at) = bombs(&mut sim).pop().unwrap();
    let ct2 = character(&mut sim, at + Vec3::new(1.0, 1.0, 0.0), 2);
    sim.ticks(2);
    look_at(&mut sim, ct2, at + Vec3::Y * 0.05);
    sim.intent(ct).use_key = true;
    sim.ticks(5);
    sim.intent(ct2).use_key = true;
    sim.ticks(5);
    let told = events(&sim).into_iter().any(|(_, e)| {
        matches!(e, ObjectiveEvent::DefuseRefused { who, reason } if who == ct2 && reason == "The bomb is already being defused.")
    });
    assert!(told);
    sim.ticks(340);
    assert!(
        events(&sim)
            .iter()
            .any(|(_, e)| *e == ObjectiveEvent::Defused { who: ct })
    );
}

fn rounds(sim: &mut Sim) {
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_freezetime 1; mp_roundtime 1; chimera_rounds 1");
    sim.ticks(3);
}

fn phase(sim: &Sim) -> Phase {
    sim.app.world().resource::<RoundState>().phase
}

fn ended(sim: &Sim) -> Option<RoundEnded> {
    sim.app.world().resource::<Log>().1.last().cloned()
}

fn bomb_carriers(sim: &mut Sim) -> Vec<Entity> {
    let w = sim.app.world_mut();
    w.query::<(&Weapon, &C4Part)>()
        .iter(w)
        .filter_map(|(w, _)| w.owner)
        .collect()
}

#[test]
fn a_round_gives_one_terrorist_the_bomb_and_the_explosion_wins() {
    let mut sim = sim(bomb_map());
    let ts: Vec<Entity> = (0..3)
        .map(|i| character(&mut sim, Vec3::new(i as f32, 1.0, 12.0), 1))
        .collect();
    let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    // O1: exactly one terrorist, no CT.
    let carriers = bomb_carriers(&mut sim);
    assert_eq!(carriers.len(), 1);
    let t = carriers[0];
    assert!(ts.contains(&t));
    assert!(first(&sim, |e| *e == ObjectiveEvent::GotBomb { who: t }).is_some());
    // Not drawn: a pistol stays in hand.
    let active = sim.app.world().get::<Inventory>(t).unwrap().active;
    sim.seconds(1.3);
    assert!(matches!(phase(&sim), Phase::Live { .. }));
    let active_now = sim.app.world().get::<Inventory>(t).unwrap().active;
    assert!(
        active_now.is_none_or(|a| sim.app.world().get::<C4Part>(a).is_none()),
        "{active:?}"
    );
    let start = money(&sim, t);
    // Into the target, draw it (slot 5), plant.
    teleport(&mut sim, t, Vec3::new(0.0, 1.0, 12.0));
    for &o in ts.iter().filter(|o| **o != t) {
        teleport(&mut sim, o, Vec3::new(20.0, 1.0, -30.0));
    }
    teleport(&mut sim, ct, Vec3::new(-30.0, 1.0, -30.0));
    sim.intent(t).select = Some(4);
    sim.seconds(1.2);
    sim.intent(t).select = None;
    plant(&mut sim, t);
    assert_eq!(money(&sim, t), start, "no planter's reward (measured)");
    // O19: every terrorist dead: the round goes on.
    for &o in &ts {
        sim.app.world_mut().write_message(Damage {
            force: Vec3::ZERO,
            target: o,
            attacker: Some(ct),
            amount: 10.0,
            point: Vec3::ZERO,
            dir: Vec3::X,
            hitgroup: Hitgroup::Chest,
            kind: default(),
            weapon: None,
        });
    }
    // O23: and the clock (1 min) runs out: still going.
    sim.seconds(44.0);
    assert!(matches!(phase(&sim), Phase::Live { .. }), "{:?}", phase(&sim));
    let ct_before = money(&sim, ct);
    let t_before = money(&sim, t);
    sim.seconds(1.5);
    let end = ended(&sim).expect("round over");
    assert_eq!(end.winner, Some(Team(1)));
    assert_eq!(end.reason, RoundEndReason::TargetBombed);
    // The win panel: the planter is the MVP; the terrorists won with
    // every one of them dead, killed by the CT (3 kills).
    assert_eq!(end.summary.mvp, Some((t, Mvp::BombPlant)));
    assert_eq!(
        end.summary.fun_fact.as_ref().map(|f| (f.token.as_str(), f.player, f.values.clone())),
        Some(("funfact_t_win_no_kills", None, vec![]))
    );
    // O13: terrorists +3500; the CT the loss bonus (and +300 per kill),
    // paid as the next round starts.
    assert_eq!((money(&sim, t), money(&sim, ct)), (t_before, ct_before));
    assert_eq!(paid(&sim, t), Some(3500));
    assert_eq!(paid(&sim, ct), Some(1400));
}

#[test]
fn a_defused_bomb_wins_for_the_defenders_and_pays_the_planters_too() {
    let mut sim = sim(bomb_map());
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    sim.seconds(1.3);
    teleport(&mut sim, t, Vec3::new(0.0, 1.0, 12.0));
    teleport(&mut sim, ct, Vec3::new(-30.0, 1.0, -30.0));
    sim.intent(t).select = Some(4);
    sim.seconds(1.2);
    sim.intent(t).select = None;
    plant(&mut sim, t);
    teleport(&mut sim, t, Vec3::new(20.0, 1.0, -30.0));
    let (_, _, at) = bombs(&mut sim).pop().unwrap();
    teleport(&mut sim, ct, at + Vec3::new(0.0, 1.0, 1.0));
    sim.ticks(2);
    look_at(&mut sim, ct, at + Vec3::Y * 0.05);
    let (t0, c0) = (money(&sim, t), money(&sim, ct));
    sim.intent(ct).use_key = true;
    sim.seconds(10.2);
    let end = ended(&sim).expect("round over");
    assert_eq!((end.winner, end.reason), (Some(Team(2)), RoundEndReason::BombDefused));
    assert_eq!(end.summary.mvp, Some((ct, Mvp::BombDefuse)), "the defuser is the MVP");
    // O14's money: CTs +3250 (measured; was 3500), terrorists the loss
    // bonus + 800; nothing to the defuser himself.
    assert_eq!((money(&sim, ct), money(&sim, t)), (c0, t0));
    assert_eq!(paid(&sim, ct), Some(3250));
    assert_eq!(paid(&sim, t), Some(1400 + 800));
}

#[test]
fn bomb_maps_end_on_time_for_the_defenders_and_on_elimination() {
    // O21.
    let mut sim = sim(bomb_map());
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    sim.seconds(62.0);
    let end = ended(&sim).expect("round over");
    assert_eq!((end.winner, end.reason), (Some(Team(2)), RoundEndReason::TargetSaved));
    assert_eq!(end.summary.mvp, None, "no kills, no MVP");
    assert_eq!(paid(&sim, ct), Some(3250));
    // The terrorist, alive when time ran out, gets no loss bonus.
    assert_eq!(paid(&sim, t), None);
    // O20: all CTs dead after a plant: terrorists at once.
    sim.seconds(5.5);
    sim.seconds(1.3);
    teleport(&mut sim, t, Vec3::new(0.0, 1.0, 12.0));
    teleport(&mut sim, ct, Vec3::new(-30.0, 1.0, -30.0));
    sim.intent(t).select = Some(4);
    sim.seconds(1.2);
    sim.intent(t).select = None;
    plant(&mut sim, t);
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: ct,
        attacker: Some(t),
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: default(),
        weapon: None,
    });
    sim.ticks(3);
    let end = ended(&sim).expect("round over");
    assert_eq!((end.winner, end.reason), (Some(Team(1)), RoundEndReason::Eliminated));
}

#[test]
fn the_carrier_drops_the_bomb_and_only_terrorists_take_it() {
    let mut sim = sim(bomb_map());
    let t = carrier(&mut sim);
    // O10: knife (or anything) in hand, killed: the bomb lies there.
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: t,
        attacker: None,
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: default(),
        weapon: None,
    });
    sim.ticks(3);
    let loose: Vec<(Entity, Vec3)> = {
        let w = sim.app.world_mut();
        w.query::<(&Loose, &Transform)>()
            .iter(w)
            .filter(|(l, _)| w.get::<C4Part>(l.weapon).is_some())
            .map(|(l, t)| (l.weapon, t.translation))
            .collect()
    };
    assert_eq!(loose.len(), 1, "the bomb on the ground");
    assert!(first(&sim, |e| *e == ObjectiveEvent::DroppedBomb { who: t }).is_some());
    let (bomb, at) = loose[0];
    // O11: a CT walks over it: nothing (also once it is touchable, 1 s
    // after the drop: specs/cs_source/weapons.md 3.9).
    let ct = character(&mut sim, at + Vec3::Y * 0.5, 2);
    sim.ticks(80);
    assert!(sim.app.world().get::<Weapon>(bomb).unwrap().owner.is_none());
    teleport(&mut sim, ct, Vec3::new(-20.0, 1.0, 0.0));
    // O12: a terrorist takes it, into its slot, his hand unchanged.
    let t2 = character(&mut sim, Vec3::new(-20.0, 1.0, 20.0), 1);
    sim.ticks(3);
    let held = sim.app.world().get::<Inventory>(t2).unwrap().active;
    teleport(&mut sim, t2, at + Vec3::Y * 0.5);
    sim.ticks(20);
    assert_eq!(sim.app.world().get::<Weapon>(bomb).unwrap().owner, Some(t2));
    assert_eq!(sim.app.world().get::<Inventory>(t2).unwrap().active, held);
    assert!(first(&sim, |e| *e == ObjectiveEvent::PickedUpBomb { who: t2 }).is_some());
}

#[test]
fn defusal_kits_are_bought_by_defenders_and_dropped_on_death() {
    let mut sim = sim(bomb_map());
    let ct = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 2);
    let t = character(&mut sim, Vec3::new(4.0, 1.0, 12.0), 1);
    sim.ticks(2);
    sim.app.world_mut().insert_resource(BuyWindow::default());
    // O32.
    sim.app.world_mut().entity_mut(ct).insert(Money(800));
    sim.app.world_mut().entity_mut(t).insert(Money(800));
    buy(sim.app.world_mut(), ct, "defuser").unwrap();
    assert_eq!(money(&sim, ct), 600);
    assert!(sim.app.world().get::<DefuseKit>(ct).is_some());
    assert!(buy(sim.app.world_mut(), ct, "defuser").is_err());
    // O33.
    assert!(buy(sim.app.world_mut(), t, "defuser").is_err());
    assert_eq!(money(&sim, t), 800);
    // O34: dies, the kit lies there; a CT with a kit leaves it, one
    // without takes it.
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: ct,
        attacker: None,
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: default(),
        weapon: None,
    });
    sim.ticks(3);
    let kit_at = {
        let w = sim.app.world_mut();
        let v: Vec<Vec3> = w
            .query_filtered::<&Transform, With<LooseKit>>()
            .iter(w)
            .map(|t| t.translation)
            .collect();
        assert_eq!(v.len(), 1);
        v[0]
    };
    assert!(sim.app.world().get::<DefuseKit>(ct).is_none());
    let with = character(&mut sim, Vec3::new(10.0, 1.0, 0.0), 2);
    sim.app.world_mut().entity_mut(with).insert(DefuseKit);
    let without = character(&mut sim, Vec3::new(14.0, 1.0, 0.0), 2);
    teleport(&mut sim, with, kit_at);
    sim.ticks(5);
    assert!(sim.app.world_mut().query::<&LooseKit>().iter(sim.app.world()).count() == 1);
    teleport(&mut sim, with, Vec3::new(10.0, 1.0, 0.0));
    teleport(&mut sim, without, kit_at);
    sim.ticks(5);
    assert!(sim.app.world().get::<DefuseKit>(without).is_some());
    assert_eq!(
        sim.app.world_mut().query::<&LooseKit>().iter(sim.app.world()).count(),
        0
    );
}

/// One hostage near the first spawn; a rescue zone off to the east.
fn hostage_map() -> MapObjectives {
    MapObjectives {
        rescue_zones: vec![Zone::engine_box(Vec3::new(8.0, -1.0, -2.0), Vec3::new(18.0, 3.0, 8.0))],
        hostages: vec![HostageSpawn {
            feet: Vec3::new(0.0, 0.0, 5.0),
            yaw: 0.0,
            kind: None,
            ..Default::default()
        }],
        ..default()
    }
}

fn hostages(sim: &mut Sim) -> Vec<(Entity, Hostage, Vec3)> {
    let w = sim.app.world_mut();
    w.query::<(Entity, &Hostage, &Transform)>()
        .iter(w)
        .map(|(e, h, t)| (e, h.clone(), t.translation))
        .collect()
}

/// Spec 9 "Spawn" (measured): a hostage placed above the floor falls onto
/// it and rests 1/32 unit above it, whatever the local player moves with
/// (one flying with noclip gave them noclip: they hung in the air).
#[test]
fn hostages_fall_onto_the_floor_while_the_player_flies() {
    use crate::stack::{
        core::LocalPlayer,
        games::cs_source::movement::{self, SourceMovementPlugin},
        movement::noclip,
        slots::MovementSlot,
    };
    let mut map = hostage_map();
    map.hostages[0].feet.y = 40.0 * 0.0254;
    let mut sim = sim_with(map, SourceMovementPlugin);
    let flyer = sim.spawn_character(Vec3::new(0.0, 3.0, 12.0), noclip::ID);
    sim.app.world_mut().entity_mut(flyer).insert((Team(2), LocalPlayer));
    character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 1);
    rounds(&mut sim);
    sim.seconds(1.5);
    let (h, _, _) = hostages(&mut sim).pop().expect("hostage spawned");
    assert_eq!(sim.app.world().get::<MovementSlot>(h).map(|m| m.0), Some(movement::ID));
    assert!(sim.state(h).on_ground);
    let feet = (sim.position(h).y - sim.state(h).hull_min.y.abs()) / 0.0254;
    assert!((feet - 0.031_25).abs() < 0.01, "feet {feet} units: on the floor");
}

/// A hostage gives up a leader more than 2000 units away (CS:S, measured
/// 2026-10-18, objectives.md Q10: the leader teleported 1980 units off is
/// still followed, 2060 off is given up at once).
#[test]
fn a_hostage_gives_up_a_leader_2000_units_away() {
    for (off, kept) in [(1950.0, true), (2050.0, false)] {
        let mut sim = sim(hostage_map());
        let _t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
        let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
        rounds(&mut sim);
        sim.seconds(1.3);
        let (_, _, at) = hostages(&mut sim).pop().expect("hostage spawned");
        teleport(&mut sim, ct, Vec3::new(0.0, 1.0, 6.5));
        sim.ticks(2);
        look_at(&mut sim, ct, at);
        sim.intent(ct).use_key = true;
        sim.ticks(2);
        sim.intent(ct).use_key = false;
        sim.ticks(1);
        assert_eq!(hostages(&mut sim)[0].1.leader, Some(ct));
        teleport(&mut sim, ct, at + Vec3::new(off * 0.0254, 1.0, 0.0));
        sim.ticks(3);
        let leader = hostages(&mut sim)[0].1.leader;
        assert_eq!(leader.is_some(), kept, "{off} units off: {leader:?}");
    }
}

#[test]
fn a_hostage_follows_its_leader_to_the_rescue_zone() {
    let mut sim = sim(hostage_map());
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    sim.seconds(1.3);
    let (h, _, at) = hostages(&mut sim).pop().expect("hostage spawned");
    assert!(sim.app.world().get::<Team>(h).is_none(), "on no team");
    // O37: a terrorist can't move it.
    teleport(&mut sim, t, Vec3::new(0.0, 1.0, 6.5));
    teleport(&mut sim, ct, Vec3::new(-20.0, 1.0, 20.0));
    sim.ticks(2);
    look_at(&mut sim, t, at);
    sim.intent(t).use_key = true;
    sim.ticks(2);
    sim.intent(t).use_key = false;
    assert!(first(&sim, |e| *e == ObjectiveEvent::HostageRefused { who: t }).is_some());
    teleport(&mut sim, t, Vec3::new(-30.0, 1.0, -30.0));
    // O35: a CT's use: it follows him, +150 once.
    teleport(&mut sim, ct, Vec3::new(0.0, 1.0, 6.5));
    sim.ticks(2);
    look_at(&mut sim, ct, at);
    let c0 = money(&sim, ct);
    sim.intent(ct).use_key = true;
    sim.ticks(2);
    sim.intent(ct).use_key = false;
    sim.ticks(1);
    assert_eq!(hostages(&mut sim)[0].1.leader, Some(ct));
    assert_eq!(money(&sim, ct), c0 + 150);
    // O36: again: it stays.
    sim.intent(ct).use_key = true;
    sim.ticks(2);
    sim.intent(ct).use_key = false;
    sim.ticks(1);
    assert_eq!(hostages(&mut sim)[0].1.leader, None);
    sim.intent(ct).use_key = true;
    sim.ticks(2);
    sim.intent(ct).use_key = false;
    sim.ticks(1);
    assert_eq!(money(&sim, ct), c0 + 150, "only the first touch pays");
    // The CT walks into the zone; the hostage comes after him.
    let c1 = money(&sim, ct);
    teleport(&mut sim, ct, Vec3::new(13.0, 1.0, 3.0));
    let mut moved = false;
    for _ in 0..400 {
        sim.ticks(1);
        match hostages(&mut sim).pop() {
            Some((_, _, p)) => moved |= (p - at).xz().length() > 1.0,
            None => break,
        }
    }
    assert!(moved, "it walked");
    // O38/O39: rescued: gone, +1000, and with no hostage left: CT win.
    assert!(hostages(&mut sim).is_empty(), "rescued");
    assert_eq!(sim.app.world().resource::<HostageTally>().rescued, 1);
    sim.ticks(2);
    let end = ended(&sim).expect("round over");
    assert_eq!(
        (end.winner, end.reason),
        (Some(Team(2)), RoundEndReason::HostagesRescued)
    );
    // The rescue's 1000 at once; the win's 2500 and 850 for the rescued
    // hostage as the next round starts.
    assert_eq!(money(&sim, ct), c1 + 1000);
    assert_eq!(paid(&sim, ct), Some(2500 + 850));
}

#[test]
fn hostage_maps_go_to_the_terrorists_on_time() {
    // O40, and O2's shape: nobody gets a bomb on a hostage map.
    let mut sim = sim(hostage_map());
    let _t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let _ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    assert!(bomb_carriers(&mut sim).is_empty());
    assert_eq!(hostages(&mut sim).len(), 1);
    sim.seconds(62.0);
    let end = ended(&sim).expect("round over");
    assert_eq!(
        (end.winner, end.reason),
        (Some(Team(1)), RoundEndReason::HostagesNotRescued)
    );
    // The next round puts it back.
    sim.seconds(5.5);
    assert_eq!(hostages(&mut sim).len(), 1);
}

/// Spec 9 "Spawn" and Q9 (measured on the probe, cs_office and
/// cs_compound): every round start re-creates the hostages, after a round
/// ends (on time, by elimination, a draw) and after `mp_restartgame` alike:
/// new entities (CS:S's references change, same edict slots), back at the
/// map's origins, the model turn going on (a 3-hostage map one model later
/// each round; cs_office's four look unchanged only because there are four
/// models).
#[test]
fn hostages_are_new_at_every_round_start() {
    use crate::stack::{map::BodyName, objectives::hostages::HostageRules};
    let mut map = hostage_map();
    map.hostages = (0..3)
        .map(|i| HostageSpawn {
            feet: Vec3::new(-2.0 + 2.0 * i as f32, 0.0, 5.0),
            ..Default::default()
        })
        .collect();
    let mut sim = sim(map);
    sim.app.world_mut().resource_mut::<HostageRules>().models = ["a", "b", "c", "d"].map(String::from).to_vec();
    let _t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let _ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    let snapshot = |sim: &mut Sim| -> Vec<(Entity, usize, String)> {
        let w = sim.app.world_mut();
        let mut v: Vec<(Entity, usize, String)> = w
            .query::<(Entity, &Hostage, &BodyName)>()
            .iter(w)
            .map(|(e, h, b)| (e, h.index, b.0.clone()))
            .collect();
        v.sort_by_key(|x| x.1);
        v
    };
    let models = |s: &[(Entity, usize, String)]| s.iter().map(|x| x.2.clone()).collect::<Vec<_>>();
    let first = snapshot(&mut sim);
    assert_eq!(models(&first), ["a", "b", "c"]);
    // One wanders off; the next round puts a new one at the origin.
    teleport(&mut sim, first[0].0, Vec3::new(10.0, 1.0, 12.0));
    sim.seconds(62.0);
    assert!(ended(&sim).is_some(), "round over on time");
    sim.seconds(5.5);
    let second = snapshot(&mut sim);
    assert_eq!(models(&second), ["d", "a", "b"], "the turn goes on");
    for (e, ..) in &first {
        assert!(sim.app.world().get_entity(*e).is_err(), "last round's hostage went");
    }
    assert!(sim.position(second[0].0).x.abs() < 2.5, "back at its origin");
    // mp_restartgame: the same again.
    sim.app.world_mut().resource_mut::<Console>().submit("mp_restartgame 1");
    sim.seconds(1.5);
    let third = snapshot(&mut sim);
    assert_eq!(models(&third), ["c", "d", "a"]);
    for (e, ..) in &second {
        assert!(sim.app.world().get_entity(*e).is_err(), "restart re-created them");
    }
}

#[test]
fn killing_a_hostage_costs_money_and_too_many_get_you_removed() {
    // O41 with mp_hostagepenalty 3.
    let mut sim = sim(MapObjectives {
        hostages: (0..4)
            .map(|i| HostageSpawn {
                feet: Vec3::new(i as f32 * 2.0, 0.0, 5.0),
                yaw: 0.0,
                kind: None,
                ..Default::default()
            })
            .collect(),
        ..hostage_map()
    });
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let _ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_hostagepenalty 3");
    rounds(&mut sim);
    sim.seconds(1.3);
    let m0 = money(&sim, t);
    let all: Vec<Entity> = hostages(&mut sim).iter().map(|h| h.0).collect();
    for (n, h) in all[..3].iter().enumerate() {
        if n == 2 {
            // Each killing hit (1000 points): 20 a point twice, and 500.
            assert_eq!(money(&sim, t), m0.saturating_sub(2 * (2 * 20_000 + 500)));
        }
        sim.app.world_mut().write_message(Damage {
            force: Vec3::ZERO,
            target: *h,
            attacker: Some(t),
            amount: 10.0,
            point: Vec3::ZERO,
            dir: Vec3::X,
            hitgroup: Hitgroup::Chest,
            kind: default(),
            weapon: None,
        });
        sim.ticks(3);
    }
    assert_eq!(sim.app.world().resource::<HostageTally>().killed, 3);
    let kills = events(&sim)
        .iter()
        .filter(|(_, e)| matches!(e, ObjectiveEvent::HostageKilled { attacker: Some(a), .. } if *a == t))
        .count();
    assert_eq!(kills, 3);
    // A computer player would be gone; this one is (no LocalPlayer).
    assert!(sim.app.world().get_entity(t).is_err(), "removed after the third");
}

#[test]
fn hostages_hurt_report_their_attacker() {
    let mut sim = sim(hostage_map());
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let _ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    let (h, ..) = hostages(&mut sim).pop().unwrap();
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: h,
        attacker: Some(t),
        amount: 0.2,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: default(),
        weapon: None,
    });
    sim.ticks(2);
    assert!(
        events(&sim)
            .iter()
            .any(|(_, e)| matches!(e, ObjectiveEvent::HostageHurt { attacker: Some(a), .. } if *a == t))
    );
    assert!((sim.app.world().get::<Health>(h).unwrap().current - 0.8).abs() < 1e-4);
}

/// What the bomb shows the local player (spec HUD table and section 3):
/// arming types the code on the C4's screen a digit per key press (the
/// view model's `c4.click` times), masks it at 2.6 s and keeps it until
/// the bomb leaves the hands, with no progress bar (it would cover the
/// screen; `cl_c4progressbar` is "Show a progress bar when defusing the
/// C4"; spec Q16); the bar shows while defusing with `cl_c4progressbar
/// 1` and not with 0.
#[test]
fn arming_types_the_code_and_the_defuse_bar_follows_cl_c4progressbar() {
    use crate::stack::{
        games::cs_source::ui::{
            bomb_fx::C4ScreenText,
            objectives_hud::{BombHudStatePlugin, BombProgress},
        },
        core::LocalPlayer,
        objectives::bomb::BombRules,
    };
    fn shown(sim: &Sim) -> (String, Option<f32>) {
        let w = sim.app.world();
        (w.resource::<C4ScreenText>().text.clone(), w.resource::<BombProgress>().0)
    }
    let mut sim = sim_with(bomb_map(), BombHudStatePlugin);
    let t = carrier(&mut sim);
    sim.app.world_mut().entity_mut(t).insert(LocalPlayer);
    sim.ticks(2);
    assert_eq!(shown(&sim), (String::new(), None), "nothing before arming");
    let rules = sim.app.world().resource::<BombRules>().clone();
    // Letting go part way clears the typed digits at once (playtest
    // 2026-10-14, as CS:S).
    sim.intent(t).fire = true;
    sim.seconds(1.3);
    assert_eq!(shown(&sim).0, "73", "typing");
    sim.intent(t).fire = false;
    sim.ticks(1);
    assert_eq!(shown(&sim), (String::new(), None), "let go: the digits cleared");
    sim.ticks(2);
    sim.intent(t).fire = true;
    sim.ticks(1);
    let begun = tick(&sim);
    // Each tick's text, until the bomb is down.
    let mut seen: Vec<(u64, String)> = Vec::new();
    while first(&sim, |e| matches!(e, ObjectiveEvent::Planted { .. })).is_none() {
        let (text, bar) = shown(&sim);
        let w = sim.app.world();
        let since = w.get::<crate::stack::objectives::bomb::Arming>(t).expect("arming").since;
        let elapsed = (w.resource::<crate::stack::core::SimClock>().now - since) as f32;
        let by_tick = (tick(&sim) - begun) as f32 * TICK_INTERVAL as f32;
        assert!((elapsed - by_tick).abs() < 1e-4, "{elapsed} s at {by_tick} s");
        assert_eq!(text, rules.screen_text(elapsed), "at {elapsed} s");
        assert_eq!(bar, None, "no bar while arming ({elapsed} s), cl_c4progressbar 1 or not");
        if seen.last().is_none_or(|s| s.1 != text) {
            seen.push((tick(&sim) - begun, text));
        }
        sim.ticks(1);
        assert!(tick(&sim) - begun < 220, "planted");
    }
    // A digit a key press, on the tick its press's time is reached.
    let texts: Vec<&str> = seen.iter().map(|s| s.1.as_str()).collect();
    assert_eq!(texts, ["", "7", "73", "735", "7355", "73556", "735560", "7355608", "*******"]);
    for ((at, text), (press, _)) in seen[1..].iter().zip(&rules.screen[1..]) {
        let (at, press) = (*at as f64 * TICK_INTERVAL, *press as f64);
        assert!(
            at >= press - 1e-4 && at - TICK_INTERVAL < press - 1e-4,
            "{text:?} at {at} s, its key press at {press} s"
        );
    }
    sim.intent(t).fire = false;
    sim.ticks(1);
    assert_eq!(shown(&sim), (String::new(), None), "the screen goes with the bomb");

    // A defender (the local player now) with the bar off, then on.
    sim.app.world_mut().entity_mut(t).remove::<LocalPlayer>();
    let (_, _, at) = bombs(&mut sim).pop().unwrap();
    teleport(&mut sim, t, Vec3::new(20.0, 1.0, 30.0));
    let ct = character(&mut sim, at + Vec3::new(0.0, 1.0, 1.0), 2);
    sim.app.world_mut().entity_mut(ct).insert(LocalPlayer);
    sim.ticks(2);
    look_at(&mut sim, ct, at + Vec3::Y * 0.05);
    sim.app.world_mut().resource_mut::<Console>().submit("cl_c4progressbar 0");
    sim.intent(ct).use_key = true;
    sim.seconds(1.0);
    assert!(first(&sim, |e| matches!(e, ObjectiveEvent::BeginDefuse { .. })).is_some());
    assert_eq!(shown(&sim), (String::new(), None), "cl_c4progressbar 0: no defuse bar");
    sim.app.world_mut().resource_mut::<Console>().submit("cl_c4progressbar 1");
    sim.ticks(1);
    let bar = shown(&sim).1.expect("cl_c4progressbar 1: the defuse bar");
    assert!((bar - 1.0 / 10.0).abs() < 0.01, "a tenth of a 10 s defuse: {bar}");
}

/// The C4's screen follows the loaded view model, not numbers of ours:
/// its press-button sequence's screen events (model event 7001, the text
/// in their options) at their times into the sequence, which arming plays
/// from its start. A made-up model here (a 2 s sequence typing "4", "42"
/// and masking it) shows its own digits at its own times, and no bar.
#[test]
fn the_c4_screen_types_what_its_view_model_says_when_it_says_it() {
    use std::sync::Arc;

    use crate::stack::{
        core::LocalPlayer,
        games::cs_source::ui::{
            bomb_fx::C4ScreenText,
            objectives_hud::{BombHudStatePlugin, BombProgress},
        },
        map::{
            MapViewModel, ViewModels,
            anim::{AnimEvent, AnimSet, Animation, Sequence},
        },
        objectives::bomb::{Arming, BombRules, SCREEN_EVENT},
    };
    // 40 frames at 20 fps: cycle 0.25 is 0.5 s in.
    let typed = [(0.0, ""), (0.25, "4"), (0.5, "42"), (0.8, "**")];
    let defaults = vec![(Quat::IDENTITY, Vec3::ZERO)];
    let set = AnimSet {
        bases: vec![defaults.clone()],
        defaults,
        animations: vec![Animation {
            name: "a_pressbutton".into(),
            fps: 20.0,
            frames: 41,
            ..default()
        }],
        sequences: vec![Sequence {
            name: "pressbutton".into(),
            activity: "ACT_VM_PRIMARYATTACK".into(),
            activity_weight: 1,
            grid: (1, 1),
            anims: vec![0],
            bone_weights: vec![1.0],
            events: typed
                .iter()
                .map(|(cycle, text)| AnimEvent {
                    cycle: *cycle,
                    event: SCREEN_EVENT,
                    name: String::new(),
                    options: format!("{text} "),
                })
                .collect(),
            ..default()
        }],
        ..default()
    };
    let length = set.duration(0);
    assert!((length - 2.0).abs() < 1e-4, "{length}");
    let mut sim = sim_with(bomb_map(), BombHudStatePlugin);
    sim.app.insert_resource(ViewModels::of(vec![MapViewModel {
        key: C4.to_string(),
        model: default(),
        bones: Vec::new(),
        root: Transform::default(),
        animations: Some(Arc::new(set)),
        right_handed: true,
        allow_flipping: true,
        attachments: Vec::new(),
        light_origin: Vec3::ZERO,
    }]));
    let t = carrier(&mut sim);
    sim.app.world_mut().entity_mut(t).insert(LocalPlayer);
    let want: Vec<(f32, String)> = typed.iter().map(|(c, s)| (c * length, s.to_string())).collect();
    assert_eq!(sim.app.world().resource::<BombRules>().screen, want, "read from the model");
    sim.intent(t).fire = true;
    let mut seen: Vec<(f32, String)> = Vec::new();
    for _ in 0..(2.5 / TICK_INTERVAL) as usize {
        sim.ticks(1);
        let w = sim.app.world();
        let Some(a) = w.get::<Arming>(t) else { break };
        let elapsed = (w.resource::<crate::stack::core::SimClock>().now - a.since) as f32;
        assert_eq!(w.resource::<BombProgress>().0, None, "no bar while arming");
        let text = w.resource::<C4ScreenText>().text.clone();
        if seen.last().is_none_or(|s| s.1 != text) {
            seen.push((elapsed, text));
        }
    }
    let texts: Vec<&str> = seen.iter().map(|s| s.1.as_str()).collect();
    assert_eq!(texts, ["", "4", "42", "**"]);
    for ((at, text), (press, _)) in seen[1..].iter().zip(&want[1..]) {
        assert!(
            *at >= press - 1e-4 && *at < press + TICK_INTERVAL as f32,
            "{text:?} at {at} s, the model's event at {press} s"
        );
    }
}

/// The planted bomb's HUD icon blinks with the beeps (playtest
/// 2026-10-14, as CS:S): lit for the first half of each beep's interval,
/// so slowly at first (1 s) and fast at the end (0.1 s).
#[test]
fn the_planted_bomb_icon_blinks_with_the_beeps() {
    use crate::stack::{games::cs_source::ui::objectives_hud::bomb_icon_lit, objectives::bomb::beep_interval};
    for remaining in [45.0, 20.0, 1.0] {
        let interval = beep_interval(remaining, 45.0);
        assert!(bomb_icon_lit(0.0, interval));
        assert!(bomb_icon_lit(interval * 0.4, interval));
        assert!(!bomb_icon_lit(interval * 0.6, interval), "{remaining} s left: off half way");
    }
    let end = beep_interval(0.5, 45.0);
    assert!(bomb_icon_lit(0.04, end) && !bomb_icon_lit(0.07, end), "fast at the end ({end} s)");
    assert!(bomb_icon_lit(f32::INFINITY, f32::NAN), "lit before the first beep");
}

/// Hostage money (spec 8, measured on the probe server 2026-10-17): a hit
/// costs 20 a whole point, the killing hit that again and 500; a hostage
/// map's elimination pays the winners 3000 and 150 a living hostage, as
/// the next round starts.
#[test]
fn hostage_hits_cost_by_the_point_and_wins_pay_per_hostage() {
    let mut sim = sim(MapObjectives {
        hostages: (0..3)
            .map(|i| HostageSpawn {
                feet: Vec3::new(i as f32 * 2.0, 0.0, 5.0),
                yaw: 0.0,
                kind: None,
                ..Default::default()
            })
            .collect(),
        ..hostage_map()
    });
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    sim.seconds(1.3);
    sim.app.world_mut().entity_mut(ct).insert(Money(10_000));
    let hs: Vec<Entity> = hostages(&mut sim).iter().map(|h| h.0).collect();
    let hit = |sim: &mut Sim, target: Entity, points: f32| {
        sim.app.world_mut().write_message(Damage {
            force: Vec3::ZERO,
            target,
            attacker: Some(ct),
            amount: points / 100.0,
            point: Vec3::ZERO,
            dir: Vec3::X,
            hitgroup: Hitgroup::Generic,
            kind: default(),
            weapon: None,
        });
        sim.ticks(3);
    };
    hit(&mut sim, hs[0], 5.7);
    assert_eq!(money(&sim, ct), 10_000 - 100, "5 whole points");
    hit(&mut sim, hs[0], 0.5);
    assert_eq!(money(&sim, ct), 10_000 - 100, "no whole point");
    hit(&mut sim, hs[1], 100.0);
    assert_eq!(money(&sim, ct), 10_000 - 100 - (2000 + 2000 + 500), "the kill");
    // The CT kills the terrorist (+300): two hostages alive.
    hit(&mut sim, t, 200.0);
    let end = ended(&sim).expect("round over");
    assert_eq!((end.winner, end.reason), (Some(Team(2)), RoundEndReason::Eliminated));
    assert_eq!(money(&sim, ct), 10_000 - 4600 + 300);
    assert_eq!(paid(&sim, ct), Some(3000 + 2 * 150));
    assert_eq!(paid(&sim, t), Some(1400));
}

/// Objectives.md Q5's last cases (measured 2026-10-17, cs_office): a
/// hostage led and let go still pays the winning CTs its 100 (led at some
/// time this round); terrorists winning on time after a rescue get 150 for
/// each hostage left and nothing for the rescued one, and the CT, alive,
/// gets no loss bonus.
#[test]
fn a_hostage_let_go_counts_as_led_and_a_rescued_one_pays_terrorists_nothing() {
    let two = || MapObjectives {
        hostages: (0..2)
            .map(|i| HostageSpawn {
                feet: Vec3::new(i as f32 * 2.0, 0.0, 5.0),
                ..Default::default()
            })
            .collect(),
        ..hostage_map()
    };
    // `ct` uses hostage `i` from beside it: once to lead, again to let go.
    let use_hostage = |sim: &mut Sim, ct: Entity, at: Vec3| {
        teleport(sim, ct, at + Vec3::new(0.0, 1.0, 1.5));
        sim.ticks(2);
        look_at(sim, ct, at + Vec3::Y * 0.5);
        sim.intent(ct).use_key = true;
        sim.ticks(2);
        sim.intent(ct).use_key = false;
        sim.ticks(1);
    };
    let by_spawn = |sim: &mut Sim, x: f32| {
        hostages(sim)
            .into_iter()
            .find(|h| (h.2.x - x).abs() < 0.5)
            .map(|h| (h.0, h.2))
            .expect("hostage")
    };

    // Led and let go, then the CT wins by elimination.
    let mut sim = self::sim(two());
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    sim.seconds(1.3);
    let (h0, at0) = by_spawn(&mut sim, 0.0);
    use_hostage(&mut sim, ct, at0);
    assert_eq!(sim.app.world().get::<Hostage>(h0).unwrap().leader, Some(ct));
    use_hostage(&mut sim, ct, at0);
    assert_eq!(sim.app.world().get::<Hostage>(h0).unwrap().leader, None, "let go");
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: t,
        attacker: Some(ct),
        amount: 2.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Generic,
        kind: default(),
        weapon: None,
    });
    sim.ticks(3);
    let end = ended(&sim).expect("round over");
    assert_eq!((end.winner, end.reason), (Some(Team(2)), RoundEndReason::Eliminated));
    assert_eq!(paid(&sim, ct), Some(3000 + 2 * 150 + 100));

    // One rescued, then the clock runs out: the terrorists win.
    let mut sim = self::sim(two());
    let t = character(&mut sim, Vec3::new(0.0, 1.0, 12.0), 1);
    let ct = character(&mut sim, Vec3::new(5.0, 1.0, 12.0), 2);
    rounds(&mut sim);
    sim.seconds(1.3);
    let (_, at1) = by_spawn(&mut sim, 2.0);
    use_hostage(&mut sim, ct, at1);
    teleport(&mut sim, ct, Vec3::new(13.0, 1.0, 3.0));
    for _ in 0..400 {
        sim.ticks(1);
        if sim.app.world().resource::<HostageTally>().rescued == 1 {
            break;
        }
    }
    assert_eq!(sim.app.world().resource::<HostageTally>().rescued, 1, "rescued");
    teleport(&mut sim, ct, Vec3::new(-20.0, 1.0, 20.0));
    sim.seconds(62.0);
    let end = ended(&sim).expect("round over");
    assert_eq!(
        (end.winner, end.reason),
        (Some(Team(1)), RoundEndReason::HostagesNotRescued)
    );
    assert_eq!(paid(&sim, t), Some(3250 + 150), "the hostage left, not the rescued one");
    assert_eq!(paid(&sim, ct), None, "alive at the time-out");
}
