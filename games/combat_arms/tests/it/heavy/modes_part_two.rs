//! The modes of specs/combat_arms/game_modes_2.md and the bots of
//! bots_objectives.md on real maps from the user's install:
//!
//! - GRAYHAMMER in Search & Destroy and in Capture the Flag with four bots
//!   a team: they find and fight each other on its `Doobic` and
//!   `Capturetheflag` points, and (bots_objectives.md: no AI plays an
//!   objective) never get, plant or carry a bomb or take a flag; rounds
//!   end by elimination.
//! - BLOODMONEY in Hired Guns: its 84 `SquadMatch` drop points (80 bars a
//!   round), its two banks and two unit spawns; a hired unit comes at
//!   `UNITSPAWN_ALPHA`, follows its hirer over the map, and a pocket is
//!   banked in `SaveZone_A`.
//! - CITYOFWINGS in VIP Escort: the VIP at `VipTeleportPoint`, the exit
//!   (`EscapeZoneTrigger`) shut, then open; the VIP's escape wins the
//!   round.
//!
//! Pass with a note without `[games.combat_arms] path` in
//! chimera.local.toml.

use bevy::prelude::*;
use chimera_core::{
    core::{Dead, Team},
    mount::config::LocalConfig,
};
use chimera_world::{harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::{
        GAME, INFO, TICK_INTERVAL,
        bots::{self, CaBot},
        inventory::UNIT,
        movement,
        rules::{
            CaMatch, Phase,
            escort::{self},
            hired_guns::CaBank,
            mercs::{self, MercOrder},
            objectives::{BOMB, CaCarry, CaObjectives, FlagAt},
            objects::CaMapObjects,
            spawns::{self, CaProtected},
        },
        world,
    },
    harness::{CaSim, with_mode},
};

fn mount() -> Option<chimera_core::mount::Mount> {
    let Some(install) = LocalConfig::load().ok().and_then(|c| c.game_path(GAME)) else {
        eprintln!("skipping: no Combat Arms install configured");
        return None;
    };
    if !chimera_core::mount::served::usable("combat_arms", &install, "Game") {
        eprintln!("skipping: no Combat Arms install at {}", install.display());
        return None;
    }
    Some((INFO.open)(&install).unwrap())
}

fn sim_on(mount: &chimera_core::mount::Mount, map: &str, mode: &str) -> Sim {
    let data = world::load_from(mount, map).unwrap();
    let mut sim = Sim::new(CaSim, MapSimPlugin::new(data));
    sim.ticks(2);
    with_mode(sim.app.world_mut(), mode);
    sim
}

fn m(sim: &Sim) -> CaMatch {
    sim.app.world().resource::<CaMatch>().clone()
}

fn human(sim: &mut Sim, team: u8) -> Entity {
    let p = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), movement::ID);
    sim.app.world_mut().entity_mut(p).insert(Team(team));
    p
}

fn unprotect(sim: &mut Sim) {
    let w = sim.app.world_mut();
    let all: Vec<Entity> = w.query_filtered::<Entity, With<CaProtected>>().iter(w).collect();
    for e in all {
        spawns::unprotect(w, e);
    }
}

fn put(sim: &mut Sim, p: Entity, to: Vec3) {
    let w = sim.app.world_mut();
    w.get_mut::<Transform>(p).unwrap().translation = to;
    if let Some(mut pos) = w.get_mut::<avian3d::prelude::Position>(p) {
        pos.0 = to;
    }
}

fn until(sim: &mut Sim, secs: f64, test: impl Fn(&Sim) -> bool) -> bool {
    for _ in 0..(secs / TICK_INTERVAL) as usize {
        if test(sim) {
            return true;
        }
        sim.ticks(1);
    }
    test(sim)
}

/// Bots fight in S&D and CTF on GRAYHAMMER but take no objective (the
/// rules fire each mode's start trigger from the mission data, S&D's
/// `Level02StartTrigger`, CTF's `Level05StartTrigger`: it shows the map's
/// gate, `Gate_Nml`, among its mode's objects).
#[test]
fn bots_play_no_objective_on_grayhammer() {
    let Some(mount) = mount() else { return };
    // Search & Destroy.
    let mut sim = sim_on(&mount, "grayhammer", "TMM");
    {
        let o = sim.app.world().resource::<CaMapObjects>();
        assert!(o.bomb_zones.iter().filter(|z| z.family == "Doobic").count() >= 2, "the map's sites");
    }
    for team in [1, 2, 1, 2, 1, 2, 1, 2] {
        bots::add_bot(sim.app.world_mut(), Some(team)).unwrap();
    }
    // The warm-up and a 180 s round at most (the time-out ends it).
    let mut deaths = 0usize;
    let mut was_dead = 0usize;
    let mut planted = false;
    let mut bombs = false;
    for _ in 0..(215.0 / TICK_INTERVAL / 10.0) as usize {
        sim.ticks(10);
        let w = sim.app.world_mut();
        let dead = w.query_filtered::<(), (With<CaBot>, With<Dead>)>().iter(w).count();
        deaths += dead.saturating_sub(was_dead);
        was_dead = dead;
        planted |= w.resource::<CaObjectives>().planted.is_some();
        let bots: Vec<Entity> = w.query_filtered::<Entity, With<CaBot>>().iter(w).collect();
        bombs |= bots
            .iter()
            .any(|b| combat_arms::game::rules::objectives::has_weapon(w, *b, BOMB).is_some());
    }
    let mm = m(&sim);
    eprintln!("grayhammer S&D: {deaths} deaths, round wins {:?}, rounds {}", mm.round_wins, mm.round);
    assert!(deaths >= 2, "bots fought: {deaths} deaths");
    assert!(mm.round_wins.iter().sum::<u32>() >= 1, "a round ended");
    assert!(!planted, "no bot planted");
    assert!(!bombs, "no bot was given a bomb");

    // Capture the flag.
    let mut sim = sim_on(&mount, "grayhammer", "CTF");
    for team in [1, 2, 1, 2, 1, 2, 1, 2] {
        bots::add_bot(sim.app.world_mut(), Some(team)).unwrap();
    }
    let mut taken = false;
    // 180 s: the teams patrol their own halves first; when they meet
    // depends on their paths (about 90-100 s).
    for _ in 0..(180.0 / TICK_INTERVAL / 10.0) as usize {
        sim.ticks(10);
        let o = sim.app.world().resource::<CaObjectives>();
        taken |= o.flags.iter().any(|f| f.at != FlagAt::Home);
    }
    let mm = m(&sim);
    eprintln!("grayhammer CTF: kills {:?}, deaths {:?}, flags {:?}", mm.kills, mm.deaths, sim.app.world().resource::<CaObjectives>().flags);
    assert_eq!(sim.app.world().resource::<CaObjectives>().flags.len(), 2, "the map's two flags");
    assert!(mm.kills.iter().sum::<u32>() >= 2, "bots fought: {:?}", mm.kills);
    assert!(!taken, "no bot took a flag");
}

/// Hired Guns on BLOODMONEY: its objects, the bars, a unit following its
/// hirer over the map, a pocket banked.
#[test]
fn hired_guns_on_bloodmoney() {
    let Some(mount) = mount() else { return };
    let mut sim = sim_on(&mount, "bloodmoney", "SQM");
    {
        let o = sim.app.world().resource::<CaMapObjects>().clone();
        assert_eq!(o.drops.iter().filter(|d| d.family == "SquadMatch").count(), 84);
        assert_eq!(o.banks.len(), 2);
        assert!(o.banks.iter().any(|b| b.team == Some(1)) && o.banks.iter().any(|b| b.team == Some(2)));
        assert_eq!(o.named("UNITSPAWN_ALPHA").count(), 1);
        assert_eq!(o.named("UNITSPAWN_BRAVO").count(), 1);
    }
    let a = human(&mut sim, 1);
    let _b = human(&mut sim, 2);
    // Past the warm-up to the first real round.
    assert!(until(&mut sim, 30.0, |s| m(s).phase == Phase::Live && !m(s).warmup), "round 1");
    unprotect(&mut sim);
    let o = sim.app.world().resource::<CaObjectives>().clone();
    assert_eq!((o.placed, o.bullion.len()), (80, 80));
    assert_eq!(sim.app.world().get::<CaBank>(a).map(|b| b.0), Some(1000));
    // A unit at UNITSPAWN_ALPHA, following its hirer.
    let spawn = sim.app.world().resource::<CaMapObjects>().named("UNITSPAWN_ALPHA").next().unwrap().at.translation;
    let merc = mercs::hire(sim.app.world_mut(), a, 0).unwrap();
    assert!((sim.position(merc) - spawn).length() < 0.05);
    mercs::set_order(sim.app.world_mut(), a, MercOrder::FollowMe);
    // The unit and its hirer go to two corners of the map's largest flat
    // navigation area (the unit walking out through the truck's doors:
    // `heavy::world_objects`).
    sim.ticks(2);
    let nav = sim.app.world().resource::<bots::brain::BotNav>().clone();
    let area = nav
        .mesh
        .areas
        .iter()
        .filter(|a| a.heights.iter().all(|h| (h - a.heights[0]).abs() < 0.05))
        .max_by(|a, b| (a.max - a.min).length().total_cmp(&(b.max - b.min).length()))
        .unwrap();
    let inset = (area.max - area.min).signum() * 0.6;
    let (lo, hi) = (area.min + inset, area.max - inset);
    let p = Vec3::new(lo.x, area.heights[0], lo.y);
    let q = Vec3::new(hi.x, area.heights[0], hi.y);
    put(&mut sim, merc, p + Vec3::Y);
    let start = sim.position(merc).distance(q + Vec3::Y) / UNIT;
    let mut close = false;
    for _ in 0..(20.0 / TICK_INTERVAL) as usize {
        put(&mut sim, a, q + Vec3::Y);
        sim.ticks(1);
        if sim.position(merc).distance(q + Vec3::Y) / UNIT < 260.0 {
            close = true;
            break;
        }
    }
    let end = sim.position(merc).distance(q + Vec3::Y) / UNIT;
    eprintln!("bloodmoney: the unit from {start:.0} u to {end:.0} u of its hirer");
    assert!(close, "it followed its hirer: {start:.0} → {end:.0} u");
    // A pocket into Alpha's bank.
    let bank = sim.app.world().resource::<CaMapObjects>().banks.iter().find(|b| b.team == Some(1)).unwrap().area;
    sim.app.world_mut().entity_mut(a).insert(CaCarry {
        bars: 4,
        money: 400,
        ..default()
    });
    for _ in 0..3 {
        put(&mut sim, a, bank.centre);
        sim.ticks(1);
    }
    assert_eq!(sim.app.world().resource::<CaObjectives>().banked, [4, 0]);
    assert_eq!(sim.app.world().get::<CaBank>(a).map(|b| b.0), Some(800 + 400));
}

/// VIP Escort on CITYOFWINGS: the VIP at the map's teleport point, the
/// exit shut then open, the escape's round.
#[test]
fn vip_escort_on_cityofwings() {
    let Some(mount) = mount() else { return };
    let mut sim = sim_on(&mount, "cityofwings", "ESC");
    let point = sim.app.world().resource::<CaMapObjects>().named("VipTeleportPoint").next().unwrap().at;
    let exit = sim.app.world().resource::<CaMapObjects>().escape_zones[0].area;
    let alpha = [human(&mut sim, 1), human(&mut sim, 1)];
    let _bravo = [human(&mut sim, 2), human(&mut sim, 2)];
    assert!(until(&mut sim, 30.0, |s| m(s).phase == Phase::Live && !m(s).warmup), "round 1");
    let vip = escort::vip(sim.app.world_mut()).expect("a VIP");
    assert!(alpha.contains(&vip));
    let at = sim.position(vip);
    eprintln!("cityofwings: VIP at {at}, its point {}, the exit {:?}", point.translation, exit.centre);
    assert!((at.xz() - point.translation.xz()).length() < 0.1, "at its teleport point");
    // The exit shut: nothing.
    put(&mut sim, vip, exit.centre);
    sim.ticks(3);
    assert!(m(&sim).winner.is_none(), "the exit is shut");
    let ends = m(&sim).round_ends.unwrap();
    assert!(until(&mut sim, 66.0, |s| s.app.world().resource::<CaObjectives>().exit.is_some_and(|x| x.open)));
    let left = ends - sim.app.world().resource::<chimera_core::core::SimClock>().now;
    // The exit timer's 60 s + its 1 s run-out (game_modes_2.md Follow-up
    // (2026-10-31b)): 119 s left.
    assert!((left - 119.0).abs() < 0.2, "open with 119 s left: {left:.2}");
    for _ in 0..3 {
        put(&mut sim, vip, exit.centre);
        sim.ticks(1);
    }
    assert_eq!(m(&sim).winner, Some(1), "the escape");
    assert!(until(&mut sim, 5.0, |s| m(s).round_wins == [1, 0]));
}
