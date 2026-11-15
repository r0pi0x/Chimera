//! Halo's game types on the disc's maps (`game::rules`,
//! specs/halo_ce/multiplayer_rules.md): Hang 'Em High's starts by game type
//! and team (11.2's counts), its slayer spawns with the map's custom
//! starting equipment (4.6: a pistol and no grenades), and a slayer game
//! played to its end on `HaloSim` by real fire: three players, one shooting
//! the others with its pistol until the score to win, each respawn at a
//! slayer start, then the postgame and the results. Passes with a note
//! without an install.

use bevy::prelude::*;
use blam_formats::scenario::GameType;
use chimera_core::core::{Dead, Intent};
use chimera_sim::{mode::Match, weapon::{Inventory, Weapon}};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use halo_ce::{
    game::{
        movement,
        rules::{
            self, HaloGameType, HaloMatch, HaloPlayer,
            netgame::Netgame,
            spawning::{self, candidates, ctf_usable},
        },
        weapons::{HaloHands, PISTOL},
    },
    harness::HaloSim,
};

use super::maps::{load, mount};

/// Hang 'Em High in a simulation with `n` players and game type `gametype`
/// begun.
pub fn game_on(name: &str, gametype: &str, n: usize) -> Option<(Sim, Vec<Entity>)> {
    let mount = mount()?;
    let map = load(&mount, name, false);
    let mut sim = Sim::new(HaloSim, MapSimPlugin::new(map));
    sim.ticks(3);
    let p: Vec<Entity> = (0..n).map(|_| sim.spawn_character(Vec3::new(0.0, 50.0, 0.0), movement::ID)).collect();
    sim.ticks(1);
    rules::start_as(sim.app.world_mut(), gametype);
    sim.ticks(3);
    Some((sim, p))
}

#[test]
fn hang_em_highs_starts_by_game_type() {
    let Some((mut sim, _)) = game_on("hangemhigh", "slayer", 0) else { return };
    let n = sim.app.world().resource::<Netgame>().clone();
    assert_eq!(n.starts.len(), 67);
    let count = |g, team| candidates(Some(&n), &n.starts, g, team).len();
    assert_eq!(count(GameType::Slayer, 3), 28);
    assert_eq!(count(GameType::Race, 3), 28);
    assert_eq!(count(GameType::CaptureTheFlag, rules::RED), 19);
    assert_eq!(count(GameType::CaptureTheFlag, rules::BLUE), 20);
    assert!(n.starts.iter().filter(|s| s.game_types.eligible_for(GameType::CaptureTheFlag)).all(|s| !matches!(s.team, 0 | 1) || ctf_usable(&n, s)));
    sim.ticks(1);
}

/// `shooter` faces `target`'s centre.
fn aim(sim: &mut Sim, shooter: Entity, target: Entity) {
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    let d = sim.position(target) - eye;
    let mut i = sim.intent(shooter);
    i.yaw = (-d.x).atan2(-d.z);
    i.pitch = d.y.atan2(Vec2::new(d.x, d.z).length());
}

/// `target` put `metres` in front of `shooter`'s start (its facing).
fn face_off(sim: &mut Sim, shooter: Entity, target: Entity, metres: f32) {
    let t = *sim.app.world().get::<Transform>(shooter).unwrap();
    let yaw = sim.app.world().get::<Intent>(shooter).unwrap().yaw;
    let ahead = Quat::from_rotation_y(yaw) * Vec3::NEG_Z * metres;
    let at = Transform::from_translation(t.translation + ahead);
    chimera_sim::spawning::revive(sim.app.world_mut(), target, Some(at));
}

#[test]
fn a_slayer_game_on_hang_em_high_by_real_fire() {
    let Some((mut sim, p)) = game_on("hangemhigh", "slayer", 3) else { return };
    sim.app.world_mut().resource_mut::<HaloGameType>().0.rules.score_to_win = 3;
    let starts: Vec<Vec3> = {
        let n = sim.app.world().resource::<Netgame>().clone();
        candidates(Some(&n), &n.starts, GameType::Slayer, 3).into_iter().map(|i| n.starts[i].at).collect()
    };
    for e in &p {
        assert!(sim.app.world().get::<Dead>(*e).is_none(), "spawned");
        // gt 14 (flags 1): a pistol, no grenades.
        let inv = sim.app.world().get::<Inventory>(*e).unwrap();
        let ids: Vec<&str> = inv.weapons.iter().map(|w| sim.app.world().get::<Weapon>(*w).unwrap().id).collect();
        assert_eq!(ids, [PISTOL]);
        assert_eq!(sim.app.world().get::<HaloHands>(*e).unwrap().throw.counts, [0, 0]);
        let feet = spawning::feet(sim.app.world().get::<Transform>(*e).unwrap());
        assert!(starts.iter().any(|s| s.distance(feet) < 0.6), "at a slayer start: {feet}");
    }
    let shooter = p[0];
    let mut kills = 0;
    'game: for round in 0..3 {
        let victim = p[1 + round % 2];
        for _ in 0..400 {
            if sim.app.world().get::<Dead>(victim).is_none() && sim.app.world().get::<Dead>(shooter).is_none() {
                break;
            }
            sim.ticks(1);
        }
        face_off(&mut sim, shooter, victim, 3.0);
        for t in 0..600 {
            aim(&mut sim, shooter, victim);
            sim.intent(shooter).fire = t % 2 == 0;
            sim.ticks(1);
            if sim.app.world().get::<Dead>(victim).is_some() {
                kills += 1;
                sim.intent(shooter).fire = false;
                continue 'game;
            }
        }
        panic!("round {round}: no kill");
    }
    assert_eq!(kills, 3);
    assert_eq!(sim.app.world().get::<HaloPlayer>(shooter).unwrap().score, 3);
    sim.ticks(2);
    assert!(sim.app.world().resource::<HaloMatch>().over);
    let team = sim.app.world().get::<chimera_core::core::Team>(shooter).unwrap().0;
    assert_eq!(sim.app.world().resource::<Match>().winner, Some(team));
    sim.seconds(7.2);
    assert!(sim.app.world().resource::<HaloMatch>().results);
}

/// The netgame data of `name`, and a game of `gametype` on it with `n`
/// players.
fn flags_of(sim: &Sim, kind: blam_formats::scenario::NetgameFlagType) -> Vec<halo_ce::game::rules::netgame::Flag> {
    sim.app.world().resource::<Netgame>().flags_of(kind).cloned().collect()
}

fn put(sim: &mut Sim, e: Entity, feet: Vec3) {
    let t = Transform::from_translation(feet + Vec3::Y * spawning::centre_above_feet());
    chimera_sim::spawning::revive(sim.app.world_mut(), e, Some(t));
    sim.ticks(1);
}

#[test]
fn capture_the_flag_on_blood_gulch_and_hang_em_high() {
    use blam_formats::scenario::NetgameFlagType;
    use halo_ce::game::{maps::space, rules::{ctf::HaloFlag, netgame::WU}};
    for name in ["bloodgulch", "hangemhigh"] {
        let Some((mut sim, p)) = game_on(name, "ctf", 2) else { return };
        let n = sim.app.world().resource::<Netgame>().clone();
        if name == "bloodgulch" {
            assert_eq!(candidates(Some(&n), &n.starts, GameType::CaptureTheFlag, rules::RED).len(), 17);
            assert_eq!(candidates(Some(&n), &n.starts, GameType::CaptureTheFlag, rules::BLUE).len(), 21);
        }
        assert_eq!(flags_of(&sim, NetgameFlagType::CaptureTheFlag).len(), 2, "{name}");
        let w = sim.app.world_mut();
        let flags: Vec<HaloFlag> = w.query::<&HaloFlag>().iter(w).cloned().collect();
        assert_eq!(flags.len(), 2, "{name}");
        let home = |team| flags.iter().find(|f| f.team == team).unwrap().home;
        // Each flag at its team's capture the flag spot of the scenario.
        assert_eq!(home(rules::RED), n.flag_spot(0).unwrap().at);
        assert_eq!(home(rules::BLUE), n.flag_spot(1).unwrap().at);
        if name == "bloodgulch" {
            let spec = space::point([95.69, -159.45, -0.10]);
            assert!(home(rules::RED).distance(spec) < 0.05, "red spot {} against the spec's {spec}", home(rules::RED));
        }
        // Each player at a start of its team; red takes blue's flag home.
        let team = |sim: &Sim, e| sim.app.world().get::<chimera_core::core::Team>(e).unwrap().0;
        let red = *p.iter().find(|e| team(&sim, **e) == rules::RED).unwrap();
        put(&mut sim, red, home(rules::BLUE));
        sim.ticks(1);
        let w = sim.app.world_mut();
        assert!(w.query::<&HaloFlag>().iter(w).any(|f| f.carrier == Some(red)), "{name}: red carries");
        put(&mut sim, red, home(rules::RED) + Vec3::X * 0.6 * WU);
        sim.ticks(1);
        assert_eq!(sim.app.world().resource::<Match>().score, [1, 0], "{name}");
    }
}

#[test]
fn king_and_oddball_on_hang_em_high() {
    use blam_formats::scenario::NetgameFlagType;
    use halo_ce::game::rules::{king::{KingState, numbers}, oddball::HaloBall};
    let Some((mut sim, p)) = game_on("hangemhigh", "king", 1) else { return };
    let hills = flags_of(&sim, NetgameFlagType::Hill);
    assert_eq!(hills.len(), 74);
    assert_eq!(numbers(sim.app.world().resource::<Netgame>()).len(), 14);
    let state = sim.app.world().resource::<KingState>().clone();
    assert_eq!(state.number, 0);
    // The player at the middle of hill 0's flags is on the hill and scores.
    let zero: Vec<Vec3> = hills.iter().filter(|f| f.team == 0).map(|f| f.at).collect();
    let mid = zero.iter().copied().sum::<Vec3>() / zero.len() as f32;
    let low = zero.iter().map(|p| p.y).fold(f32::MAX, f32::min);
    put(&mut sim, p[0], Vec3::new(mid.x, low, mid.z));
    let before = sim.app.world().get::<HaloPlayer>(p[0]).unwrap().score;
    sim.ticks(30);
    let gained = sim.app.world().get::<HaloPlayer>(p[0]).unwrap().score - before;
    assert!(gained >= 29, "{gained} ticks on the hill of {} at {}", zero.len(), sim.position(p[0]));
    // Oddball: 5 ball spots, ball 0 at 450 ticks.
    let Some((mut sim, _)) = game_on("hangemhigh", "oddball", 2) else { return };
    assert_eq!(flags_of(&sim, NetgameFlagType::OddballBallSpawn).len(), 5);
    sim.seconds(15.2);
    let w = sim.app.world_mut();
    let balls: Vec<HaloBall> = w.query::<&HaloBall>().iter(w).cloned().collect();
    assert_eq!(balls.len(), 1);
    assert_eq!(Some(balls[0].home), sim.app.world().resource::<Netgame>().flags_of(NetgameFlagType::OddballBallSpawn).find(|f| f.team == 0).map(|f| f.at));
}
