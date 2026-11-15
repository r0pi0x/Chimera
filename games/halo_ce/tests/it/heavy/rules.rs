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
