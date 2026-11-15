//! Halo's game types (`game::rules`, specs/halo_ce/multiplayer_rules.md)
//! without the disc: the test cases of the spec as numbers (the spawn
//! choice on Blood Gulch's starts 36 to 38, the respawn timelines, the
//! stagger, slayer's scoring, speed rule, multi-kills and sprees, the
//! starting equipment's grenades and the weapon sets, the game type's
//! choice and cleaning), then a slayer game on `HaloSim` and the greybox:
//! players join and spawn, a kill scores and its victim respawns 90 ticks
//! and a stagger later, a suicide costs a point and 300 ticks, the score to
//! win ends the game into the 7 s postgame and the results, and a team
//! game's betrayal; and over the network, the match's view and the
//! scores on two clients.

use std::time::Duration;

use bevy::prelude::*;
use blam_formats::game_type::{GameFlags, STOCK_GAME_TYPES, WeaponSet};
use chimera_core::core::{Dead, Died, Score, Team};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
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
            items::{self, starting_grenades, substitute},
            scoring::{self, Line, announcement, death_bonus, kill_penalty, multi_kill, respawn_timer},
            spawning::{Other, choose, rate, stagger_allows},
            variant,
            view::HaloMatchView,
        },
        vitality::ids,
        weapons::{ASSAULT_RIFLE, PISTOL, PLASMA_PISTOL, ROCKET_LAUNCHER, SNIPER_RIFLE, pickups::Item},
    },
    harness::HaloSim,
};

fn stock(name: &str) -> blam_formats::game_type::Rules {
    STOCK_GAME_TYPES.iter().find(|s| s.name == name).unwrap().rules
}

fn d(a: [f32; 3], b: [f32; 3]) -> f32 {
    Vec3::from(a).distance(Vec3::from(b))
}

const S36: [f32; 3] = [42.91, -67.76, 0.52];
const S37: [f32; 3] = [29.27, -81.39, 0.17];
const S38: [f32; 3] = [41.10, -90.28, 0.18];
const B: [f32; 3] = [40.0, -70.0, 0.3];
const C: [f32; 3] = [30.0, -80.0, 0.2];
const F: [f32; 3] = [42.0, -68.5, 0.3];

fn others(s: [f32; 3], friend: bool) -> Vec<Other> {
    let mut o = vec![
        Other { d: d(s, B), enemy: true, teammate: false },
        Other { d: d(s, C), enemy: true, teammate: false },
    ];
    if friend {
        o.push(Other { d: d(s, F), enemy: false, teammate: true });
    }
    o
}

#[test]
fn the_spawn_choice_of_the_spec() {
    // Free for all: the distance ratings.
    let r: Vec<f32> = [S36, S37, S38].iter().map(|s| rate(&others(*s, false), false, &[])).collect();
    assert!((r[0] - 0.55962).abs() < 1e-4, "{r:?}");
    assert_eq!(r[1], 0.0);
    assert_eq!(r[2], 1.0);
    assert_eq!(choose(&r, &[0.81, 0.90, 0.16]), Some(0), "start 36");
    assert_eq!(choose(&r, &[0.16, 0.90, 0.81]), Some(2), "start 38");
    // Team slayer with the teammate F: the friends bonus on start 36.
    let t: Vec<f32> = [S36, S37, S38].iter().map(|s| rate(&others(*s, true), true, &[])).collect();
    assert!((t[0] - 2.19923).abs() < 1e-3, "{t:?}");
    assert_eq!(t[1], 0.0);
    assert_eq!(t[2], 1.0);
    assert_eq!(choose(&t, &[0.64, 0.25, 0.04]), Some(0));
    assert_eq!(choose(&t, &[0.04, 0.25, 0.81]), Some(2));
    // No start above 0: no spawn.
    assert_eq!(choose(&[0.0, 0.0], &[0.5, 0.5]), None);
}

#[test]
fn respawn_timelines() {
    let snipers = stock("Snipers");
    let mut p = 0;
    let waits: Vec<i32> = (0..8).map(|_| respawn_timer(&snipers, &mut p, None, true)).collect();
    assert_eq!(waits, [90, 150, 300, 450, 600, 750, 750, 750]);
    // After death 4 the player kills an enemy: P 600 becomes 450.
    let mut p = 0;
    for _ in 0..4 {
        respawn_timer(&snipers, &mut p, None, true);
    }
    let mut mine = p;
    let mut victim = 0;
    respawn_timer(&snipers, &mut victim, Some(&mut mine), true);
    assert_eq!(respawn_timer(&snipers, &mut mine, None, true), 450);
    // A suicide at death 3 (P = 300).
    let mut p = 300;
    assert_eq!(respawn_timer(&snipers, &mut p, None, false), 600);
    // Team slayer, a betrayal with no enemy damage: 300 + 300.
    assert_eq!(respawn_timer(&stock("Team Slayer"), &mut 0, None, false), 600);
}

#[test]
fn the_stagger() {
    // Slot 5, the timer at 0 on tick 1000: the next allowed tick is 1029.
    let first = (1000..1100).find(|t| stagger_allows(*t, 5)).unwrap();
    assert_eq!(first, 1029);
    assert!(stagger_allows(3, 9) && !stagger_allows(4, 9));
}

#[test]
fn slayer_speed_multi_kills_and_sprees() {
    assert!((kill_penalty(1.0) - 0.98).abs() < 1e-6);
    assert!((death_bonus(1.0) - 1.1).abs() < 1e-6);
    assert!((kill_penalty(1.1) - 1.0).abs() < 1e-6);
    assert!((death_bonus(0.98) - 1.08).abs() < 1e-6);
    assert!((death_bonus(0.9) - 1.0).abs() < 1e-6);
    assert!((death_bonus(1.45) - 1.5).abs() < 1e-6);
    let mut m = 1.0;
    for _ in 0..10 {
        m = kill_penalty(m);
    }
    assert!((m - 0.9).abs() < 1e-6);
    // Kills at 1000, 1100, 1250, 1300.
    let mut last = None;
    let mut c = 0;
    let mut counts = Vec::new();
    for t in [1000, 1100, 1250, 1300] {
        c = multi_kill(last, c, t);
        last = Some(t);
        counts.push(c);
    }
    assert_eq!(counts, [1, 2, 1, 2]);
    assert_eq!(announcement(2, 2), Some(Line::DoubleKill));
    assert_eq!(announcement(1, 5), Some(Line::KillingSpree));
    assert_eq!(announcement(1, 10), Some(Line::RunningRiot));
    assert_eq!(announcement(1, 15), Some(Line::RunningRiot));
    assert_eq!(announcement(2, 5), Some(Line::DoubleKill), "the multi-kill first");
    assert_eq!(announcement(1, 1), None);
    assert_eq!(announcement(5, 1), Some(Line::Killtacular));
}

#[test]
fn starting_grenades_and_weapon_sets() {
    let slayer = stock("Slayer");
    let pro = stock("Slayer Pro");
    // Slayer on Blood Gulch: the entry's flags 1, no grenades.
    assert_eq!(starting_grenades(&slayer, 4, Some(1)), [0, 0]);
    // Slayer Pro: 4 frag, 2 with 6 players, 1 with 12.
    assert_eq!(starting_grenades(&pro, 4, None), [4, 0]);
    assert_eq!(starting_grenades(&pro, 6, None), [2, 0]);
    assert_eq!(starting_grenades(&pro, 12, None), [1, 0]);
    // CTF on Hang 'Em High: flags 2, 4 plasma.
    assert_eq!(starting_grenades(&stock("CTF"), 4, Some(2)), [0, 4]);
    // Rockets: one rocket launcher (the pistol's duplicate dropped).
    let rockets = stock("Rockets");
    assert_eq!(substitute(rockets.weapon_set, ASSAULT_RIFLE), ROCKET_LAUNCHER);
    assert_eq!(substitute(rockets.weapon_set, PISTOL), ROCKET_LAUNCHER);
    // Snipers: the rifle a sniper rifle, the pistol stays.
    assert_eq!(substitute(WeaponSet::Sniper, ASSAULT_RIFLE), SNIPER_RIFLE);
    assert_eq!(substitute(WeaponSet::Sniper, PISTOL), PISTOL);
    assert_eq!(substitute(WeaponSet::Normal, halo_ce::game::weapons::FLAMETHROWER), ROCKET_LAUNCHER);
    assert_eq!(substitute(WeaponSet::Pistols, ASSAULT_RIFLE), PISTOL);
    assert_eq!(substitute(WeaponSet::Pistols, PISTOL), PLASMA_PISTOL);
    // Items: plasma set grenades, the 5-player thinning, powerups.
    let mut plasma = slayer;
    plasma.weapon_set = WeaponSet::PlasmaWeapons;
    assert_eq!(items::substitute_item(&plasma, 2, Item::Grenade(0), || 0.9), Some(Item::Grenade(1)));
    assert_eq!(items::substitute_item(&slayer, 6, Item::Grenade(0), || 0.6), None);
    assert_eq!(items::substitute_item(&slayer, 6, Item::Grenade(0), || 0.5), Some(Item::Grenade(0)));
    assert_eq!(items::substitute_item(&slayer, 10, Item::Grenade(0), || 0.31), None);
    let mut no_shields = slayer;
    no_shields.flags = GameFlags(slayer.flags.bits() | 8);
    assert_eq!(items::substitute_item(&no_shields, 2, Item::Powerup(halo_ce::game::weapons::pickups::Powerup::Overshield), || 0.0), None);
}

#[test]
fn the_game_type_is_chosen_by_name() {
    assert_eq!(variant::by_name("slayer").unwrap().name, "Slayer");
    assert_eq!(variant::by_name("Team_Slayer").unwrap().name, "Team Slayer");
    assert_eq!(variant::by_name("ctf").unwrap().name, "CTF");
    assert_eq!(variant::by_name("iron ctf").unwrap().name, "Iron CTF");
    assert_eq!(variant::by_name("slayer_pro").unwrap().rules.score_to_win, 25);
    assert!(variant::by_name("nonsense").is_err());
    let mut r = stock("Slayer");
    r.health = 9.0;
    r.respawn_time = -5;
    let c = variant::clean(r);
    assert_eq!((c.health, c.respawn_time), (4.0, 0));
    assert_eq!(variant::mode_key(&stock("Team Slayer")), "team_slayer");
    assert_eq!(variant::winning_score(&stock("King")), 3600);
}

// ---------------------------------------------------------------------------
// A game on the greybox.

/// A slayer game (`gametype`) on the greybox with `n` players, begun.
fn game(gametype: &str, n: usize) -> (Sim, Vec<Entity>) {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    let players: Vec<Entity> = (0..n).map(|i| sim.spawn_character(greybox::SPAWNS[i % 2], movement::ID)).collect();
    sim.ticks(2);
    rules::start_as(sim.app.world_mut(), gametype);
    sim.ticks(2);
    (sim, players)
}

/// `killer` kills `victim` through the damage seam (a pistol round
/// larger than any shield and body).
fn kill(sim: &mut Sim, killer: Option<Entity>, victim: Entity) {
    let at = sim.position(victim);
    sim.app.world_mut().write_message(HaloHit::direct(victim, ids::PISTOL_BULLET, 10_000.0, 1.0, None, at, Vec3::NEG_Z, killer));
    sim.ticks(1);
}

fn player(sim: &Sim, e: Entity) -> HaloPlayer {
    sim.app.world().get::<HaloPlayer>(e).unwrap().clone()
}

fn dead(sim: &Sim, e: Entity) -> bool {
    sim.app.world().get::<Dead>(e).is_some()
}

#[test]
fn a_slayer_game_scores_kills_and_respawns_the_dead() {
    let (mut sim, p) = game("slayer", 2);
    let (a, b) = (p[0], p[1]);
    assert!(!dead(&sim, a) && !dead(&sim, b), "both spawned");
    assert_ne!(sim.app.world().get::<Team>(a).unwrap().0, sim.app.world().get::<Team>(b).unwrap().0, "free for all");
    // Spawned with the assault rifle in hand and the pistol (generic or no
    // entry on the greybox), 4 frag grenades.
    let inv = sim.app.world().get::<chimera_sim::weapon::Inventory>(a).unwrap();
    assert_eq!(inv.weapons.len(), 2);
    kill(&mut sim, Some(a), b);
    assert!(dead(&sim, b));
    assert_eq!(player(&sim, a).score, 1);
    assert_eq!(sim.app.world().get::<Score>(a).unwrap().kills, 1);
    assert_eq!(sim.app.world().get::<Score>(b).unwrap().deaths, 1);
    let speed = |sim: &Sim, e| sim.app.world().get::<movement::SpeedMultiplier>(e).unwrap().0;
    assert!((speed(&sim, a) - 0.98).abs() < 1e-5 && (speed(&sim, b) - 1.1).abs() < 1e-5);
    // The wait: 90 ticks, then the stagger (slot 1 modulo 32).
    let t0 = sim.app.world().resource::<HaloMatch>().time;
    let mut back = None;
    for _ in 0..200 {
        sim.ticks(1);
        if !dead(&sim, b) {
            back = Some(sim.app.world().resource::<HaloMatch>().time);
            break;
        }
    }
    let back = back.expect("respawned");
    let slot = player(&sim, b).slot as i64;
    assert!(back - t0 >= 90 && back - t0 < 90 + 33, "{t0} -> {back}");
    assert_eq!((back - 1).rem_euclid(32), slot, "on its stagger tick");
    // A suicide: -1, 300 ticks.
    kill(&mut sim, Some(a), a);
    assert_eq!(player(&sim, a).score, 0);
    assert!(player(&sim, a).respawn >= 299);
}

#[test]
fn the_score_to_win_ends_the_game() {
    let (mut sim, p) = game("slayer", 2);
    sim.app.world_mut().resource_mut::<HaloGameType>().0.rules.score_to_win = 2;
    let (a, b) = (p[0], p[1]);
    for _ in 0..2 {
        while dead(&sim, b) {
            sim.ticks(1);
        }
        kill(&mut sim, Some(a), b);
    }
    sim.ticks(1);
    assert!(sim.app.world().resource::<HaloMatch>().over, "the postgame");
    let team_a = sim.app.world().get::<Team>(a).unwrap().0;
    assert_eq!(sim.app.world().resource::<Match>().winner, Some(team_a));
    // Nobody spawns in the postgame; 7 s later everyone is dead and the
    // results are up.
    sim.seconds(6.5);
    assert!(dead(&sim, b) && !sim.app.world().resource::<HaloMatch>().results);
    sim.seconds(1.0);
    assert!(sim.app.world().resource::<HaloMatch>().results);
    assert!(dead(&sim, a) && dead(&sim, b));
    assert_eq!(player(&sim, a).score, 2, "the end's deaths score nothing");
}

#[test]
fn a_team_slayer_betrayal_costs_the_team() {
    let (mut sim, p) = game("team_slayer", 4);
    let teams: Vec<u8> = p.iter().map(|e| sim.app.world().get::<Team>(*e).unwrap().0).collect();
    assert_eq!(teams.iter().filter(|t| **t == rules::RED).count(), 2, "{teams:?}");
    assert_eq!(teams.iter().filter(|t| **t == rules::BLUE).count(), 2, "{teams:?}");
    let a = p[0];
    let mate = *p.iter().skip(1).find(|e| sim.app.world().get::<Team>(**e).unwrap().0 == teams[0]).unwrap();
    let enemy = *p.iter().find(|e| sim.app.world().get::<Team>(**e).unwrap().0 != teams[0]).unwrap();
    kill(&mut sim, Some(a), enemy);
    kill(&mut sim, Some(a), mate);
    let side = (teams[0] - 1) as usize;
    assert_eq!(sim.app.world().resource::<Match>().score[side], 0, "+1 then -1");
    assert_eq!(player(&sim, a).betrayals, 1);
    assert_eq!(player(&sim, mate).respawn, 600, "300 + the suicide penalty 300");
    assert_eq!(player(&sim, enemy).respawn, 299, "300, a tick of it counted");
}

#[test]
fn a_death_without_a_killer_scores_nothing() {
    let (mut sim, p) = game("slayer", 2);
    kill(&mut sim, None, p[1]);
    assert_eq!(player(&sim, p[1]).score, 0);
    assert_eq!(player(&sim, p[1]).respawn, 300, "0 + the suicide penalty");
    let _ = scoring::MIN_RESPAWN_TICKS;
    let _ = Died::clone;
}

/// Over the network: the match's view and each player's score on both
/// clients after a kill.
#[test]
fn the_match_is_seen_by_every_client() {
    let link = LinkConditions { latency: Duration::from_millis(20), jitter: Duration::ZERO, loss: 0.0 };
    let mut sim = NetSim::new(HaloSim, link, 9, 2, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(halo_ce::game::TICK_INTERVAL);
    sim.until_joined(600);
    rules::start(sim.server.app.world_mut());
    sim.ticks(30);
    let a = sim.character_of(0).unwrap();
    let b = sim.character_of(1).unwrap();
    let at = sim.server.app.world().get::<Transform>(b).unwrap().translation;
    sim.server.app.world_mut().write_message(HaloHit::direct(b, ids::PISTOL_BULLET, 10_000.0, 1.0, None, at, Vec3::NEG_Z, Some(a)));
    sim.ticks(30);
    for c in 0..2 {
        let w = sim.clients[c].app.world_mut();
        let views: Vec<HaloMatchView> = w.query::<&HaloMatchView>().iter(w).cloned().collect();
        assert_eq!(views.len(), 1, "client {c}");
        assert_eq!(views[0].name, "Slayer");
        assert!(views[0].time > 30);
        let scores: Vec<(i32, u32)> = w.query::<&Score>().iter(w).map(|s| (s.kills, s.deaths)).collect();
        assert!(scores.contains(&(1, 0)) && scores.contains(&(0, 1)), "client {c}: {scores:?}");
    }
}
