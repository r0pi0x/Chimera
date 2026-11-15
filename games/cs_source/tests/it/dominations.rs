//! Dominations and revenge under CS:S's rules (specs/cs_source/
//! dominations.md, `chimera_sim::dominations`): the 4th unanswered kill
//! dominates (only the 4th), the dominated player's kill back is a
//! revenge, team kills count but never flag, rounds and `mp_restartgame`
//! keep it all, a team change clears the player's; `player_death` carries
//! `dominated` and `revenge` (0/1) and each player its `Relationships`.

use bevy::prelude::*;

use crate::stack::{
    console::Console,
    core::{Damage, Health, Hitgroup, Team, restart_round},
    game_log::{EventValue, GameEvent},
    games::cs_source::{TICK_INTERVAL, weapons::CsWeaponsPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
    movement::placeholder,
    rules::{Dead, join_team},
};
use chimera_sim::dominations::{Dominations, Relationships};

#[derive(Resource, Default)]
struct Events(Vec<GameEvent>);

fn record(mut events: MessageReader<GameEvent>, mut log: ResMut<Events>) {
    log.0.extend(events.read().filter(|e| e.name == "player_death").cloned());
}

/// A, B and C: A and C terrorists, B a counter-terrorist.
fn setup() -> (Sim, Entity, Entity, Entity) {
    let mut sim = Sim::new(CsSim, (GreyboxMapPlugin, CsWeaponsPlugin));
    sim.set_tick_interval(TICK_INTERVAL);
    sim.app.init_resource::<Events>();
    sim.app.add_systems(Last, record);
    // No respawns of their own: the test revives them.
    sim.app.world_mut().resource_mut::<Console>().submit("mp_respawn_delay 1000; mp_friendlyfire 1");
    let mut player = |x: f32, team: u8| {
        let e = sim.spawn_character(Vec3::new(x, 1.0, 0.0), placeholder::ID);
        sim.app.world_mut().entity_mut(e).insert(Team(team));
        e
    };
    let (a, b, c) = (player(0.0, 1), player(4.0, 2), player(-4.0, 1));
    sim.ticks(2);
    (sim, a, b, c)
}

/// `killer` kills `victim` (revived first); the death's (dominated, revenge).
fn kill(sim: &mut Sim, victim: Entity, killer: Option<Entity>) -> (i64, i64) {
    let mut e = sim.app.world_mut().entity_mut(victim);
    e.remove::<Dead>();
    e.get_mut::<Health>().unwrap().current = 100.0;
    sim.app.world_mut().write_message(Damage {
        force: Vec3::ZERO,
        target: victim,
        attacker: killer,
        amount: 1000.0,
        point: Vec3::ZERO,
        dir: Vec3::X,
        hitgroup: Hitgroup::Chest,
        kind: default(),
        weapon: None,
    });
    sim.ticks(3);
    let mut events = std::mem::take(&mut sim.app.world_mut().resource_mut::<Events>().0);
    assert_eq!(events.len(), 1, "one death: {events:?}");
    let e = events.pop().unwrap();
    assert_eq!(e.player("userid"), Some(victim));
    let int = |k| match e.get(k) {
        Some(EventValue::Int(v)) => *v,
        other => panic!("{k}: {other:?}"),
    };
    (int("dominated"), int("revenge"))
}

fn kills(sim: &mut Sim, victim: Entity, killer: Entity, n: u32) -> Vec<(i64, i64)> {
    (0..n).map(|_| kill(sim, victim, Some(killer))).collect()
}

fn dominates(sim: &Sim, a: Entity, b: Entity) -> bool {
    sim.app.world().resource::<Dominations>().dominates(a, b)
}

fn relationships(sim: &Sim, e: Entity) -> Relationships {
    sim.app.world().get::<Relationships>(e).cloned().unwrap_or_default()
}

#[test]
fn the_fourth_unanswered_kill_dominates_and_a_kill_back_is_revenge() {
    let (mut sim, a, b, _) = setup();
    assert_eq!(kills(&mut sim, b, a, 3), [(0, 0); 3]);
    assert!(!dominates(&sim, a, b));
    assert_eq!(kill(&mut sim, b, Some(a)), (1, 0), "the 4th");
    assert!(dominates(&sim, a, b));
    sim.ticks(1);
    assert_eq!(relationships(&sim, a).dominating, [b]);
    assert_eq!(relationships(&sim, b).nemeses, [a]);
    assert_eq!(kills(&mut sim, b, a, 2), [(0, 0); 2], "the 5th and 6th");
    assert!(dominates(&sim, a, b));
    // Suicides and deaths to the world change nothing.
    kill(&mut sim, a, Some(a));
    kill(&mut sim, a, None);
    assert!(dominates(&sim, a, b));
    // B's revenge.
    assert_eq!(kill(&mut sim, a, Some(b)), (0, 1));
    assert!(!dominates(&sim, a, b));
    sim.ticks(1);
    assert_eq!(relationships(&sim, b), Relationships::default());
    let t = sim.app.world().resource::<Dominations>();
    assert_eq!((t.unanswered(a, b), t.unanswered(b, a)), (1, 0));
    // Four more: dominated again.
    assert_eq!(kills(&mut sim, b, a, 4), [(0, 0), (0, 0), (0, 0), (1, 0)]);
}

#[test]
fn team_kills_count_but_never_flag() {
    let (mut sim, a, _, c) = setup();
    assert_eq!(kills(&mut sim, c, a, 4), [(0, 0); 4]);
    assert!(!dominates(&sim, a, c));
    // C moved to the other side without dying: the 4th was spent.
    sim.app.world_mut().entity_mut(c).insert(Team(2));
    assert_eq!(kill(&mut sim, c, Some(a)), (0, 0));
    // Fresh pair: three team kills and one as enemies dominate.
    let (mut sim, a, _, c) = setup();
    kills(&mut sim, c, a, 3);
    sim.app.world_mut().entity_mut(c).insert(Team(2));
    assert_eq!(kill(&mut sim, c, Some(a)), (1, 0));
    // Back on A's team: C's kill of A is no revenge and ends nothing.
    sim.app.world_mut().entity_mut(c).insert(Team(1));
    assert_eq!(kill(&mut sim, a, Some(c)), (0, 0));
    assert!(dominates(&sim, a, c));
}

#[test]
fn rounds_and_restarts_keep_dominations() {
    let (mut sim, a, b, _) = setup();
    kills(&mut sim, b, a, 3);
    sim.app
        .world_mut()
        .resource_mut::<Console>()
        .submit("mp_restartgame 1");
    sim.seconds(1.3);
    assert_eq!(kill(&mut sim, b, Some(a)), (1, 0), "3 before the restart + 1 after");
    restart_round(sim.app.world_mut());
    sim.ticks(2);
    assert!(dominates(&sim, a, b), "a new round keeps it");
    assert_eq!(kill(&mut sim, a, Some(b)), (0, 1));
}

#[test]
fn joining_a_team_clears_the_players_dominations() {
    let (mut sim, a, b, c) = setup();
    kills(&mut sim, b, a, 4);
    kills(&mut sim, c, b, 2);
    assert!(dominates(&sim, a, b));
    join_team(sim.app.world_mut(), b, Team(1)).unwrap();
    sim.ticks(1);
    assert!(!dominates(&sim, a, b));
    let t = sim.app.world().resource::<Dominations>();
    assert_eq!((t.unanswered(b, a), t.unanswered(c, b)), (0, 0));
    assert_eq!(relationships(&sim, a), Relationships::default());
    // 3 before and 1 after a team change: no domination.
    let (mut sim, a, b, _) = setup();
    kills(&mut sim, b, a, 3);
    join_team(sim.app.world_mut(), b, Team(1)).unwrap();
    sim.app.world_mut().entity_mut(b).insert(Team(2));
    assert_eq!(kill(&mut sim, b, Some(a)), (0, 0));
}

#[test]
fn a_player_who_leaves_takes_theirs_along() {
    let (mut sim, a, b, _) = setup();
    kills(&mut sim, b, a, 4);
    sim.app.world_mut().despawn(a);
    sim.ticks(2);
    assert_eq!(*sim.app.world().resource::<Dominations>(), Dominations::default());
    assert_eq!(relationships(&sim, b), Relationships::default(), "B has no nemesis");
}
