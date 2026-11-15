//! King of the hill and oddball (`game::rules::king`, `game::rules::oddball`;
//! specs/halo_ce/multiplayer_rules.md 6.5, 6.6) on `HaloSim` and the
//! greybox with a pinned `Netgame` (two starts where two players stand,
//! a hill of four flags around the first, a ball spot at the second): the
//! hill's outline and volume (a lone flag's 2 WU square), time on the hill
//! scoring once a tick per team, the moving hill, the game won at the
//! minutes' score; the ball appearing 450 ticks after the start, taken by
//! touch, slowing and disarming its carrier, scoring a tick a tick, dropped
//! at a death and reset 1200 ticks after it was last held; juggernaut's
//! kills passing the ball and scoring.

use bevy::prelude::*;
use blam_formats::{
    game_type::{BallType, GameTypeOptions},
    scenario::{GameTypes, NetgameFlagType, netgame::SLOT_ALL},
};
use chimera_core::core::{Dead, Intent, Team};
use chimera_sim::mode::Match;
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{
    game::{
        damage::HaloHit,
        movement,
        rules::{
            self, HaloGameType, HaloMatch, HaloPlayer,
            king::{self, KingState, hill_of},
            netgame::{Flag, Netgame, Start, WU},
            oddball::{HaloBall, OddballState},
            spawning::{centre_above_feet, feet},
        },
        vitality::ids,
    },
    harness::HaloSim,
};

#[test]
fn a_hills_outline_and_volume() {
    // A lone flag: a 2 WU square, -0.1 to +0.8 WU.
    let h = hill_of(&[Vec3::ZERO]);
    assert!(h.contains(Vec3::new(0.9 * WU, 0.0, -0.9 * WU)));
    assert!(!h.contains(Vec3::new(1.1 * WU, 0.0, 0.0)));
    assert!(h.contains(Vec3::Y * 0.79 * WU) && !h.contains(Vec3::Y * 0.81 * WU));
    assert!(h.contains(Vec3::Y * -0.09 * WU) && !h.contains(Vec3::Y * -0.11 * WU));
    // Four corners and one inside: the hull is the square of the corners.
    let pts = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0), Vec3::new(4.0, 1.0, 4.0), Vec3::new(0.0, 0.0, 4.0), Vec3::new(2.0, 0.0, 2.0)];
    let h = hill_of(&pts);
    assert_eq!(h.outline.len(), 4);
    assert!(h.contains(Vec3::new(1.0, 0.5, 3.0)));
    assert!(!h.contains(Vec3::new(5.0, 0.5, 3.0)));
    assert!(!h.contains(Vec3::new(1.0, 1.0 + 0.81 * WU, 3.0)));
}

struct Game {
    sim: Sim,
    p: [Entity; 2],
    spots: [Vec3; 2],
}

/// `gametype` on the greybox: two players, a hill around the first's spot
/// (hill 0) and another far off (hill 1), the ball's spot at the second's.
fn game(gametype: &str, options: impl FnOnce(&mut GameTypeOptions)) -> Game {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    let a = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    let b = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.ticks(30);
    let spots = [a, b].map(|e| feet(sim.app.world().get::<Transform>(e).unwrap()));
    let all = GameTypes([SLOT_ALL, 0, 0, 0]);
    let mut flags: Vec<Flag> = [Vec3::new(-3.0, 0.0, -3.0), Vec3::new(3.0, 0.0, -3.0), Vec3::new(3.0, 0.0, 3.0), Vec3::new(-3.0, 0.0, 3.0)]
        .into_iter()
        .map(|d| Flag { at: spots[0] + d, yaw: 0.0, kind: NetgameFlagType::Hill, team: 0 })
        .collect();
    flags.push(Flag { at: spots[0] + Vec3::new(20.0, 0.0, 0.0), yaw: 0.0, kind: NetgameFlagType::Hill, team: 1 });
    flags.push(Flag { at: spots[1], yaw: 0.0, kind: NetgameFlagType::OddballBallSpawn, team: 0 });
    let n = Netgame {
        starts: (0..2).map(|i| Start { at: spots[i], yaw: 0.0, team: i as i16, game_types: all }).collect(),
        flags,
        equipment: Vec::new(),
        seen: 0,
    }
    .pinned();
    sim.app.world_mut().insert_resource(n);
    rules::start_as(sim.app.world_mut(), gametype);
    options(&mut sim.app.world_mut().resource_mut::<HaloGameType>().0.rules.options);
    sim.ticks(3);
    // Who stands where: p[0] at the hill, p[1] at the ball's spot.
    let near = |sim: &Sim, e: Entity| feet(sim.app.world().get::<Transform>(e).unwrap()).distance(spots[0]) < 1.0;
    let p = if near(&sim, a) { [a, b] } else { [b, a] };
    Game { sim, p, spots }
}

impl Game {
    fn put(&mut self, e: Entity, at: Vec3) {
        let t = Transform::from_translation(at + Vec3::Y * centre_above_feet());
        chimera_sim::spawning::revive(self.sim.app.world_mut(), e, Some(t));
        self.sim.ticks(1);
    }
    fn score(&self, e: Entity) -> i32 {
        self.sim.app.world().get::<HaloPlayer>(e).unwrap().score
    }
}

#[test]
fn time_on_the_hill_scores_and_wins() {
    let mut g = game("king", |_| {});
    assert_eq!(g.sim.app.world().resource::<KingState>().number, 0);
    let [on, off] = g.p;
    let s0 = g.score(on);
    g.sim.ticks(30);
    assert_eq!(g.score(on) - s0, 30, "a point a tick on the hill");
    assert_eq!(g.score(off), 0);
    // The score to win in minutes: 1 is 1800 ticks.
    g.sim.app.world_mut().resource_mut::<HaloGameType>().0.rules.score_to_win = 1;
    g.sim.seconds(62.0);
    assert!(g.sim.app.world().resource::<HaloMatch>().over);
    let team = g.sim.app.world().get::<Team>(on).unwrap().0;
    assert_eq!(g.sim.app.world().resource::<Match>().winner, Some(team));
}

#[test]
fn a_team_scores_once_a_tick_and_the_hill_moves() {
    let mut g = game("team_king", |_| {});
    let [a, b] = g.p;
    let spot = g.spots[0];
    // A third player away; the first two both blue, both on the hill.
    let c = g.sim.spawn_character(greybox::SPAWNS[1] + Vec3::X * 6.0, movement::ID);
    g.sim.ticks(3);
    g.sim.app.world_mut().entity_mut(c).insert(Team(rules::RED));
    g.sim.app.world_mut().entity_mut(b).insert(Team(rules::BLUE));
    g.sim.app.world_mut().entity_mut(a).insert(Team(rules::BLUE));
    g.put(c, g.spots[1] + Vec3::X * 6.0);
    g.put(a, spot);
    g.put(b, spot + Vec3::X * 1.0);
    let before = g.sim.app.world().resource::<Match>().score;
    g.sim.ticks(10);
    let after = g.sim.app.world().resource::<Match>().score;
    assert_eq!(after[1] - before[1], 10, "once a tick for the team");
    assert_eq!(after[0], before[0], "red is off the hill");
    // Moving hill: hill 1 after 1800 ticks.
    let mut g = game("crazy_king", |_| {});
    g.sim.seconds(61.0);
    assert_eq!(g.sim.app.world().resource::<KingState>().number, 1);
    let _ = king::MOVE_TICKS;
}

#[test]
fn the_ball_appears_is_taken_scores_and_resets() {
    let mut g = game("oddball", |_| {});
    let [_, at_ball] = g.p;
    let ball = |g: &mut Game| {
        let w = g.sim.app.world_mut();
        w.query::<(Entity, &HaloBall)>().iter(w).map(|(e, b)| (e, b.clone())).next()
    };
    while g.sim.app.world().resource::<HaloMatch>().time < 449 {
        assert!(ball(&mut g).is_none());
        g.sim.ticks(1);
    }
    g.sim.ticks(2);
    let (_, b) = ball(&mut g).expect("ball 0 at 450");
    assert_eq!(b.home, g.spots[1]);
    // The player standing there takes it: slowed, disarmed, scoring.
    g.put(at_ball, g.spots[1]);
    assert_eq!(ball(&mut g).unwrap().1.carrier, Some(at_ball));
    g.sim.app.world_mut().get_mut::<Intent>(at_ball).unwrap().fire = true;
    g.sim.ticks(1);
    assert!(!g.sim.app.world().get::<Intent>(at_ball).unwrap().fire);
    assert_eq!(g.sim.app.world().get::<movement::SpeedMultiplier>(at_ball).unwrap().0, 0.75);
    let s = g.score(at_ball);
    g.sim.ticks(20);
    assert_eq!(g.score(at_ball) - s, 20);
    // Dropped at its carrier's death, away from its spot; reset 1200
    // ticks after it was last held.
    g.put(at_ball, g.spots[1] + Vec3::X * 6.0);
    let at = g.sim.position(at_ball);
    g.sim.app.world_mut().write_message(HaloHit::direct(at_ball, ids::PISTOL_BULLET, 10_000.0, 1.0, None, at, Vec3::NEG_Z, None));
    g.sim.ticks(2);
    let (e, b) = ball(&mut g).unwrap();
    assert!(b.carrier.is_none());
    assert!(g.sim.app.world().get::<Transform>(e).unwrap().translation.distance(b.home) > 1.0);
    let last = b.last_held;
    while g.sim.app.world().resource::<HaloMatch>().time <= last + 1200 {
        g.sim.ticks(1);
    }
    g.sim.ticks(2);
    assert!(g.sim.app.world().get::<Transform>(e).unwrap().translation.distance(b.home) < 0.01, "reset");
}

#[test]
fn juggernaut_kills_pass_the_ball() {
    let mut g = game("juggernaut", |o| {
        if let GameTypeOptions::Oddball { ball_type, .. } = o {
            assert_eq!(*ball_type, BallType::Juggernaut);
        }
    });
    let [a, b] = g.p;
    let kill = |g: &mut Game, k: Entity, v: Entity| {
        while g.sim.app.world().get::<Dead>(v).is_some() || g.sim.app.world().get::<Dead>(k).is_some() {
            g.sim.ticks(1);
        }
        let at = g.sim.position(v);
        g.sim.app.world_mut().write_message(HaloHit::direct(v, ids::PISTOL_BULLET, 10_000.0, 1.0, None, at, Vec3::NEG_Z, Some(k)));
        g.sim.ticks(1);
    };
    // A kill while the ball is free: a is "it", +1.
    kill(&mut g, a, b);
    assert_eq!(g.sim.app.world().resource::<OddballState>().holders, [Some(a)]);
    assert_eq!(g.score(a), 1);
    // A kill by the holder: +1.
    kill(&mut g, a, b);
    assert_eq!(g.score(a), 2);
    // A kill of the holder: the ball to b, +1.
    kill(&mut g, b, a);
    assert_eq!(g.sim.app.world().resource::<OddballState>().holders, [Some(b)]);
    assert_eq!(g.score(b), 1);
}

/// Over the network: a king game's view on both clients, its time scores
/// rising with the server's.
#[test]
fn king_is_seen_by_every_client() {
    use chimera_net::{harness::NetSim, net::memory::LinkConditions};
    use halo_ce::game::rules::view::HaloMatchView;
    let link = LinkConditions { latency: std::time::Duration::from_millis(20), jitter: std::time::Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 17, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    rules::start_as(sim.server.app.world_mut(), "king");
    sim.ticks(10);
    // The hill under the first client's player.
    let a = sim.character_of(0).unwrap();
    let at = feet(sim.server.app.world().get::<Transform>(a).unwrap());
    let all = GameTypes([SLOT_ALL, 0, 0, 0]);
    let n = Netgame {
        starts: vec![Start { at, yaw: 0.0, team: 0, game_types: all }],
        flags: vec![Flag { at, yaw: 0.0, kind: NetgameFlagType::Hill, team: 0 }],
        equipment: Vec::new(),
        seen: 0,
    }
    .pinned();
    sim.server.app.world_mut().insert_resource(n);
    king::setup(sim.server.app.world_mut());
    sim.ticks(40);
    let server = sim.server.app.world().get::<HaloPlayer>(a).unwrap().score;
    assert!(server >= 30, "{server}");
    for c in 0..2 {
        let w = sim.clients[c].app.world_mut();
        let view: Vec<HaloMatchView> = w.query::<&HaloMatchView>().iter(w).cloned().collect();
        assert_eq!((view[0].base, view[0].minutes), (4, true), "client {c}");
        let scores: Vec<i32> = w.query::<&chimera_core::core::Score>().iter(w).map(|s| s.kills).collect();
        assert!(scores.iter().any(|s| *s >= 25), "client {c}: {scores:?}");
    }
}
