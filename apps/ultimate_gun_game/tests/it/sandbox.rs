//! The sandbox (`sandbox.rs`, `rung_command.rs`): a match alone with every
//! game's guns where nothing advances or ends, and the `rung` command that
//! puts a player on a rung by number, gun id or a unique part of it. On the
//! party's simulation, headless; the player is the local one, as a host's.

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{Damage, DamageKind, Hitgroup, LocalPlayer},
    slots::GameIdentity,
};
use chimera_sim::{
    minigame::{self, ladder::{Ladder, Rung}},
    mode::state::{Match, Phase},
    testkit,
};
use chimera_world::harness::Sim;
use combat_arms::game::movement;
use ultimate_gun_game::sandbox::SANDBOX;

use crate::parts::ugg;

const FRAG: &str = "halo_ce:frag_grenade";
const SNIPER: &str = "halo_ce:sniper_rifle";

/// The sandbox playing with the local player `me` and a bot-like `other`.
fn play() -> (Sim, Entity, Entity) {
    let mut sim = ugg();
    if let Err(e) = ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms") {
        panic!("the definition: {e}");
    }
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    let other = sim.spawn_character(Vec3::new(30.0, 1.0, 0.0), movement::ID);
    sim.seconds(1.5);
    (sim, me, other)
}

fn run(sim: &mut Sim, line: &str) -> Vec<String> {
    let before = sim.app.world().resource::<Console>().output.len();
    sim.app.world_mut().resource_mut::<Console>().submit(line.to_string());
    sim.ticks(3);
    sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect()
}

fn rung(sim: &Sim, e: Entity) -> usize {
    sim.app.world().get::<Rung>(e).map_or(usize::MAX, |r| r.0)
}

fn kill(sim: &mut Sim, by: Entity, victim: Entity) {
    let at = sim.position(victim);
    sim.app.world_mut().write_message(Damage {
        target: victim,
        attacker: Some(by),
        amount: 5000.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.seconds(1.0);
}

#[test]
fn the_sandbox_runs_with_no_end_and_lists_halos_guns_and_grenade() {
    let (mut sim, me, _) = play();
    let m = sim.app.world().resource::<Match>();
    assert_eq!(m.phase, Phase::Live);
    assert!(m.winner.is_none() && m.round_ends.is_some_and(|e| e > 1.0e6), "no end in reach: {m:?}");
    assert!(sim.app.world().get::<chimera_core::core::Dead>(me).is_none(), "the player lives");
    let list = run(&mut sim, "rung list").join("\n");
    for id in [SNIPER, "halo_ce:assault_rifle", FRAG, "cs_source:weapon_ak47", "combat_arms:m4a1"] {
        assert!(list.contains(id), "{id} in the list:\n{list}");
    }
    assert!(list.contains("  1  "), "numbered from 1: {list}");
    // Every rung of the ladder but the sentinel, with its game and band.
    let ladder = sim.app.world().resource::<Ladder>().clone();
    assert_eq!(list.lines().count(), ladder.rungs.len(), "one line a rung and the heading, less the sentinel");
    assert!(list.contains(" halo_ce  gun"), "{list}");
}

#[test]
fn rung_puts_the_player_on_a_rung_by_number_id_or_fragment() {
    let (mut sim, me, _) = play();
    let ladder = sim.app.world().resource::<Ladder>().clone();
    let at = |id: &str| ladder.rungs.iter().position(|r| *r == id).unwrap();
    run(&mut sim, "rung 5");
    sim.seconds(1.0);
    assert_eq!(rung(&sim, me), 4, "rung 5 is index 4 (numbered from 1)");
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(ladder.rungs[4]));
    run(&mut sim, &format!("rung {SNIPER}"));
    sim.seconds(1.0);
    assert_eq!(rung(&sim, me), at(SNIPER));
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(SNIPER), "its gun in hand");
    assert_eq!(sim.app.world().get::<GameIdentity>(me).map(|g| g.0.as_str()), Some("halo_ce"), "its game's identity");
    run(&mut sim, "rung frag_grenade");
    sim.seconds(1.0);
    assert_eq!(rung(&sim, me), at(FRAG));
    assert_eq!(testkit::active_id(sim.app.world(), me), Some(FRAG));
    run(&mut sim, "rung prev");
    assert_eq!(rung(&sim, me), at(FRAG) - 1);
    run(&mut sim, "rung next");
    assert_eq!(rung(&sim, me), at(FRAG));
}

#[test]
fn an_ambiguous_or_unknown_name_prints_an_error_and_changes_nothing() {
    let (mut sim, me, _) = play();
    run(&mut sim, "rung 3");
    sim.seconds(0.5);
    for bad in ["rung halo_ce", "rung no_such_gun_anywhere", "rung 0"] {
        let said = run(&mut sim, bad).join("\n");
        assert!(!said.is_empty(), "{bad} says why");
        assert_eq!(rung(&sim, me), 2, "{bad} changed nothing");
    }
}

/// A launch's `+rung` naming several guns (as `+rung minigun` did: Combat
/// Arms' and TF2's) waits for the match, then says why it did nothing,
/// naming the candidates, on the console (and in the log).
#[test]
fn a_deferred_ambiguous_rung_says_why_when_applied() {
    let mut sim = ugg();
    ultimate_gun_game::sandbox::generate(sim.app.world_mut(), "combat_arms").expect("the definition");
    let said = run(&mut sim, "rung rifle").join("\n");
    assert!(said.contains("waiting"), "kept for the match: {said}");
    let before = sim.app.world().resource::<Console>().output.len();
    minigame::start(sim.app.world_mut(), SANDBOX).expect("the sandbox starts");
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(me).insert(LocalPlayer);
    sim.seconds(1.5);
    let out: Vec<String> = sim.app.world().resource::<Console>().output[before..].iter().map(|l| l.text.clone()).collect();
    let line = out.iter().find(|l| l.starts_with("rung rifle:")).unwrap_or_else(|| panic!("the refusal is said: {out:?}"));
    assert!(line.contains("several") && line.contains(SNIPER), "the candidates named: {line}");
    assert!(sim.app.world().get_resource::<minigame::rung_command::PendingRung>().is_none(), "applied once");
    assert_eq!(rung(&sim, me), 0, "nothing changed");
}

#[test]
fn a_kill_never_moves_the_player_and_the_last_rung_does_not_win() {
    let (mut sim, me, other) = play();
    run(&mut sim, "rung 4");
    sim.seconds(1.0);
    kill(&mut sim, me, other);
    sim.seconds(2.0);
    assert_eq!(rung(&sim, me), 3, "a kill left the rung where it was put");
    // On the last rung a kill would win a plain ladder.
    let n = sim.app.world().resource::<Ladder>().rungs.len() - 1;
    run(&mut sim, &format!("rung {n}"));
    sim.seconds(1.0);
    assert_eq!(rung(&sim, me), n - 1);
    kill(&mut sim, me, other);
    sim.seconds(3.0);
    assert_eq!(rung(&sim, me), n - 1, "held on the last rung");
    let m = sim.app.world().resource::<Match>();
    assert!(m.winner.is_none() && m.phase == Phase::Live, "the match goes on: {m:?}");
}
