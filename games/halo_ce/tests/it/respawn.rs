//! Halo's respawn rules (`game::respawn`; specs/halo_ce/multiplayer_rules.md
//! 4.1 to 4.6, "Respawn timelines"): the timer of a death (Slayer's:
//! 90 ticks for a kill, 300 for a suicide; growth; the penalty a kill
//! pays back), the stagger's example, a start's rating and the draw's
//! choice, and in the game on the greybox a new player's loadout and a
//! dead player's return to a start after its wait with the loadout again.

use bevy::prelude::*;
use chimera_core::core::{Damage, DamageKind, Dead, Died, Health, Hitgroup};
use chimera_sim::weapon::{Inventory, Weapon};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
};
use halo_ce::{
    game::{
        movement,
        respawn::{HaloRules, Respawns, choose, rate, staggered, timer},
        weapons::{ASSAULT_RIFLE, HaloHands, PISTOL},
    },
    harness::HaloSim,
};

#[test]
fn the_respawn_timelines_of_the_spec() {
    let slayer = HaloRules::default();
    // Slayer, killed by an enemy, first death: 0 + 0, at least 90.
    assert_eq!(timer(&slayer, 0, true).0, 90);
    // Slayer, suicide: 0 + 0 + 300.
    assert_eq!(timer(&slayer, 0, false).0, 300);
    // Team Slayer (respawn 10 s), killed by an enemy: 300; by a teammate: 600.
    let team = HaloRules { respawn_time: 300, growth: 0, suicide: 300 };
    assert_eq!((timer(&team, 0, true).0, timer(&team, 0, false).0), (300, 600));
    // Snipers (respawn 0, growth 150, suicide 300), enemy kills, no kills of
    // its own: deaths 1 to 8 wait 90, 150, 300, 450, 600, 750, 750, 750.
    let snipers = HaloRules { respawn_time: 0, growth: 150, suicide: 300 };
    let mut p = 0;
    let waits: Vec<i32> = (0..8)
        .map(|_| {
            let (w, next) = timer(&snipers, p, true);
            p = next;
            w
        })
        .collect();
    assert_eq!(waits, [90, 150, 300, 450, 600, 750, 750, 750]);
    // Suicide at death 3 (P = 300): 300 + 300.
    assert_eq!(timer(&snipers, 300, false).0, 600);
}

#[test]
fn the_stagger_and_a_starts_rating() {
    // Slot 5, the timer at 0 on tick 1000 (1000 mod 32 = 8): it spawns on
    // 1029; at game time 3 or less at once.
    let first = (1000..1100).find(|t| staggered(*t, 5)).unwrap();
    assert_eq!(first, 1029);
    assert!(staggered(2, 5) && staggered(3, 17));
    // Alone: 1. A living player within 0.25 WU, or an enemy within 2: 0;
    // an enemy at 3.5 WU: x 0.5; beyond 5: no change.
    assert_eq!(rate(&[]), 1.0);
    assert_eq!((rate(&[0.1]), rate(&[1.9])), (0.0, 0.0));
    assert!((rate(&[3.5]) - 0.5).abs() < 1e-6);
    assert!((rate(&[3.5, 8.0]) - 0.5).abs() < 1e-6);
    // The choice: rating times the square root of a draw, the largest
    // above 0, the earlier of equals; none when every rating is 0.
    let mut draws = [0.25f32, 0.64, 0.5].into_iter();
    assert_eq!(choose(&[1.0, 1.0, 0.0], &mut || draws.next().unwrap()), Some(1));
    assert_eq!(choose(&[0.0, 0.0], &mut || 0.9), None);
    assert_eq!(choose(&[1.0, 1.0], &mut || 0.5), Some(0));
}

fn game() -> (Sim, Entity) {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.app.insert_resource(HaloRules::default());
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(3);
    (sim, p)
}

fn held(sim: &Sim, p: Entity) -> Vec<&'static str> {
    let inv = sim.app.world().get::<Inventory>(p).unwrap();
    inv.weapons.iter().filter_map(|w| sim.app.world().get::<Weapon>(*w)).map(|w| w.id).collect()
}

#[test]
fn a_new_player_holds_the_rifle_the_pistol_and_four_frags() {
    let (mut sim, p) = game();
    sim.ticks(5);
    assert_eq!(held(&sim, p), [ASSAULT_RIFLE, PISTOL]);
    let inv = sim.app.world().get::<Inventory>(p).unwrap();
    assert_eq!(inv.active, inv.weapons.first().copied(), "the rifle in hand");
    assert_eq!(sim.app.world().get::<HaloHands>(p).unwrap().throw.counts, [4, 0]);
}

fn kill(sim: &mut Sim, p: Entity, attacker: Option<Entity>) {
    sim.app.world_mut().get_mut::<Health>(p).unwrap().current = 0.0;
    let damage = Damage {
        target: p,
        attacker,
        amount: 0.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    };
    sim.app.world_mut().write_message(Died { entity: p, attacker, damage });
}

#[test]
fn a_dead_player_returns_after_its_wait_at_a_start_with_the_loadout() {
    let (mut sim, p) = game();
    sim.ticks(5);
    // A suicide: 300 ticks, then the stagger (slot 0: a tick that is 0 mod
    // 32), at a start of the map.
    kill(&mut sim, p, None);
    sim.ticks(2);
    assert!(sim.app.world().get::<Dead>(p).is_some(), "held dead");
    sim.ticks(290);
    assert!(sim.app.world().get::<Dead>(p).is_some(), "still waiting at 292 ticks");
    sim.ticks(80);
    assert!(sim.app.world().get::<Dead>(p).is_none(), "back within the wait and the stagger");
    assert!(sim.app.world().get::<Health>(p).unwrap().current > 0.0);
    let at = sim.position(p);
    assert!(greybox::SPAWNS.iter().any(|s| s.distance(at) < 1.5), "at a start: {at}");
    sim.ticks(5);
    assert_eq!(held(&sim, p), [ASSAULT_RIFLE, PISTOL]);
    assert_eq!(sim.app.world().get::<HaloHands>(p).unwrap().throw.counts, [4, 0], "the grenades again");
    let wait = sim.app.world().resource::<Respawns>().waiting.len();
    assert_eq!(wait, 0);
}

#[test]
fn a_kill_by_an_enemy_waits_90_ticks() {
    let (mut sim, p) = game();
    let enemy = sim.spawn_character(greybox::SPAWNS[1], movement::ID);
    sim.ticks(5);
    kill(&mut sim, p, Some(enemy));
    sim.ticks(80);
    assert!(sim.app.world().get::<Dead>(p).is_some(), "not before 90 ticks");
    sim.ticks(60);
    assert!(sim.app.world().get::<Dead>(p).is_none(), "90 ticks and the stagger (at most 31 more)");
}
