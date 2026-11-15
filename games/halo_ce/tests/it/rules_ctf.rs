//! Capture the flag (`game::rules::ctf`, specs/halo_ce/multiplayer_rules.md
//! 6.4) on `HaloSim` and the greybox, with two flag spots and two starts
//! where two players stand (a pinned `Netgame`): the spec's test cases, a
//! capture 0.6 WU from the home spot scoring and 1.0 WU not, "flag at home
//! to score" refusing with the own flag taken, a dropped flag returning
//! 511 ticks after it was last held (also with "flag must reset", where the
//! owner's touch does nothing), the third capture ending the game; and the
//! carrier's refused trigger.

use bevy::prelude::*;
use blam_formats::{
    game_type::GameTypeOptions,
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
            self, BLUE, HaloGameType, HaloMatch, RED,
            ctf::HaloFlag,
            netgame::{Flag, Netgame, Start, WU},
            spawning::{centre_above_feet, feet},
        },
        vitality::ids,
    },
    harness::HaloSim,
};

struct Ctf {
    sim: Sim,
    red: Entity,
    blue: Entity,
    spots: [Vec3; 2],
}

/// A capture the flag game on the greybox: red at the first spawn, blue
/// at the second, each team's flag where its player stood.
fn ctf(options: impl FnOnce(&mut GameTypeOptions)) -> Ctf {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    let a = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    let b = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.ticks(30);
    let spots = [a, b].map(|e| feet(sim.app.world().get::<Transform>(e).unwrap()));
    let all = GameTypes([SLOT_ALL, 0, 0, 0]);
    let n = Netgame {
        starts: (0..2).map(|i| Start { at: spots[i], yaw: 0.0, team: i as i16, game_types: all }).collect(),
        flags: (0..2).map(|i| Flag { at: spots[i], yaw: 0.0, kind: NetgameFlagType::CaptureTheFlag, team: i as i16 }).collect(),
        ..default()
    }
    .pinned();
    sim.app.world_mut().insert_resource(n);
    rules::start_as(sim.app.world_mut(), "ctf");
    options(&mut sim.app.world_mut().resource_mut::<HaloGameType>().0.rules.options);
    sim.ticks(3);
    let team = |sim: &Sim, e| sim.app.world().get::<Team>(e).unwrap().0;
    let (red, blue) = if team(&sim, a) == RED { (a, b) } else { (b, a) };
    assert_eq!(team(&sim, blue), BLUE);
    Ctf { sim, red, blue, spots }
}

impl Ctf {
    fn flag(&mut self, team: u8) -> (Entity, HaloFlag) {
        let w = self.sim.app.world_mut();
        w.query::<(Entity, &HaloFlag)>().iter(w).find(|(_, f)| f.team == team).map(|(e, f)| (e, f.clone())).unwrap()
    }

    /// `e`'s feet put at `at`.
    fn put(&mut self, e: Entity, at: Vec3) {
        let t = Transform::from_translation(at + Vec3::Y * centre_above_feet());
        chimera_sim::spawning::revive(self.sim.app.world_mut(), e, Some(t));
        self.sim.ticks(1);
    }

    fn score(&self) -> [i32; 2] {
        self.sim.app.world().resource::<Match>().score
    }

    fn time(&self) -> i64 {
        self.sim.app.world().resource::<HaloMatch>().time
    }
}

#[test]
fn the_flags_start_at_their_spots() {
    let mut g = ctf(|_| {});
    assert_eq!(g.flag(RED).1.home, g.spots[0]);
    assert_eq!(g.flag(BLUE).1.home, g.spots[1]);
    assert!(g.flag(RED).1.carrier.is_none());
    assert!(g.sim.app.world().get::<Dead>(g.red).is_none() && g.sim.app.world().get::<Dead>(g.blue).is_none());
}

#[test]
fn a_capture_scores_within_one_world_unit() {
    let mut g = ctf(|_| {});
    let (red, spots) = (g.red, g.spots);
    // Red takes blue's flag.
    g.put(red, spots[1]);
    assert_eq!(g.flag(BLUE).1.carrier, Some(red));
    // 1.0 WU from its own spot: not less, no capture.
    g.put(red, spots[0] + Vec3::X * WU * 1.0);
    g.sim.ticks(1);
    assert_eq!(g.score(), [0, 0]);
    // 0.6 WU: red +1, the flag back at blue's spot.
    g.put(red, spots[0] + Vec3::X * WU * 0.6);
    assert_eq!(g.score(), [1, 0]);
    let (_, f) = g.flag(BLUE);
    assert!(f.carrier.is_none() && !f.away);
}

#[test]
fn flag_at_home_to_score_refuses_with_the_own_flag_taken() {
    let mut g = ctf(|o| {
        if let GameTypeOptions::CaptureTheFlag { flag_at_home_to_score, .. } = o {
            *flag_at_home_to_score = true;
        }
    });
    let (red, blue, spots) = (g.red, g.blue, g.spots);
    g.put(blue, spots[0] + Vec3::Z * 0.1);
    assert_eq!(g.flag(RED).1.carrier, Some(blue));
    g.put(blue, spots[1] + Vec3::Z * 30.0);
    g.put(red, spots[1]);
    g.put(red, spots[0] + Vec3::X * WU * 0.6);
    g.sim.ticks(5);
    assert_eq!(g.score(), [0, 0], "own flag away");
}

#[test]
fn a_dropped_flag_returns_after_510_ticks() {
    for must_reset in [false, true] {
        let mut g = ctf(|o| {
            if let GameTypeOptions::CaptureTheFlag { flag_must_reset, .. } = o {
                *flag_must_reset = must_reset;
            }
        });
        let (red, blue, spots) = (g.red, g.blue, g.spots);
        g.put(blue, spots[0]);
        assert_eq!(g.flag(RED).1.carrier, Some(blue));
        // Blue walks off with it and dies.
        g.put(blue, spots[0] + Vec3::X * 8.0);
        let at = g.sim.position(blue);
        g.sim.app.world_mut().write_message(HaloHit::direct(blue, ids::PISTOL_BULLET, 10_000.0, 1.0, None, at, Vec3::NEG_Z, Some(red)));
        g.sim.ticks(2);
        let (_, f) = g.flag(RED);
        assert!(f.carrier.is_none() && f.away);
        let last = f.last_held;
        // Red touches its dropped flag: with must reset nothing happens.
        g.put(red, f.home + Vec3::X * 8.0);
        let (_, f) = g.flag(RED);
        assert_eq!(f.away, must_reset, "must reset {must_reset}");
        if must_reset {
            g.put(red, spots[0] + Vec3::Z * 20.0);
            while g.time() <= last + 510 {
                assert!(g.flag(RED).1.away, "still away at {}", g.time());
                g.sim.ticks(1);
            }
            g.sim.ticks(1);
            assert!(!g.flag(RED).1.away, "returned at L + 511");
        }
    }
}

#[test]
fn the_third_capture_ends_the_game_and_the_carrier_cannot_fire() {
    let mut g = ctf(|_| {});
    let (red, spots) = (g.red, g.spots);
    for n in 1..=3 {
        g.put(red, spots[1]);
        g.sim.app.world_mut().get_mut::<Intent>(red).unwrap().fire = true;
        g.sim.ticks(1);
        assert!(!g.sim.app.world().get::<Intent>(red).unwrap().fire, "the carrier's trigger refused");
        g.put(red, spots[0]);
        assert_eq!(g.score()[0], n);
    }
    g.sim.ticks(1);
    assert!(g.sim.app.world().resource::<HaloMatch>().over);
    assert_eq!(g.sim.app.world().resource::<Match>().winner, Some(RED));
}
