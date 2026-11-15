//! Combat Arms' bots on each of Halo's eight rung guns (the assault
//! rifle, pistol, plasma rifle, plasma pistol, needler, shotgun, sniper
//! rifle and rocket launcher) against the same bots on a CS:S AK-47: four
//! bots on a ladder of the one gun on the greybox (no install needed for
//! Halo's guns: their data is the crate's), `MINUTES` simulated minute
//! each (`measure` takes a map, for a Halo map's run by hand).
//! Counted: rounds fired, hits (each `Damage` one bot dealt another: a
//! shotgun's pellets each count), kills, suicides (a death by one's own
//! hand, a rocket's), reloads, overheats, dry pulls, and each bot's
//! longest time engaged (attacking a target it sees) without a round; a
//! table with `--nocapture`. Run alone:
//! `cargo test -p party_kit --features dev --test it heavy::halo_gun_bots -- --nocapture --test-threads 1`.
//!
//! What it asserts: every gun's bots fire. Measured 2026-11-15 over 3 minutes
//! (greybox; Hang 'Em High in brackets), kills a minute: assault rifle 4.7 (1.7),
//! pistol 11.7 (3.0), plasma rifle 5.3 (1.0), plasma pistol 0 (0.3),
//! needler 8.7 (4.3), shotgun 10.7 (2.3), sniper rifle 5.7 (2.0), rocket
//! launcher 12.7 (4.7, 4 of them suicides), the AK-47 11.3 (3.7), the M4A1
//! 11.3 (3.3). Found: Halo's battery guns carry no magazine of rounds, so
//! the bots, reading a gun's rounds off its `Magazine`, took the plasma
//! rifle and pistol for empty and never fired them (0 rounds in 3 min;
//! `halo_ce::game::weapons::systems::battery_rounds` since). Left to the
//! bots' brain (Combat Arms' `bots::brain::post_stimuli`): it lets go of
//! the trigger, resets its aim and paces the next round on the engine's
//! `WeaponEventKind::Fired`, which Halo's guns don't send (theirs are
//! `Action("fire")`), so a bot holds a semi-automatic Halo gun's trigger
//! down through a burst of two: a sniper rifle bot stood 34 s engaged
//! without a round (149 bot-seconds in spells of 5 s or more in 3 min);
//! the plasma pistol, held, charges and overheats on its charged bolt (36
//! overheats, no kill); and no bot hears a Halo gun's shots.

use std::collections::HashMap;

use bevy::prelude::*;
use chimera_core::core::{Damage, Died};
use chimera_sim::{
    minigame::{self, MinigameAppExt},
    weapon::{WeaponEvent, WeaponEventKind},
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim, map::MapSimPlugin};
use combat_arms::{
    game::bots::{BotState, CaBot},
    harness::CaSim,
};
use halo_ce::game::weapons;
use party_kit::PartyPartsPlugin;

/// Simulated minutes per gun.
const MINUTES: f64 = 1.0;

/// Halo's eight rung guns, then the baseline.
const GUNS: [&str; 9] = [
    weapons::ASSAULT_RIFLE,
    weapons::PISTOL,
    weapons::PLASMA_RIFLE,
    weapons::PLASMA_PISTOL,
    weapons::NEEDLER,
    weapons::SHOTGUN,
    weapons::SNIPER_RIFLE,
    weapons::ROCKET_LAUNCHER,
    "cs_source:weapon_ak47",
];

/// A ladder of `gun` alone, long enough that no bot wins.
fn ladder(gun: &str) -> String {
    let rungs = vec![gun; 40];
    crate::hl2dm_parts::ladder_text(&rungs, false)
        .replace(
            "weapons = [\"cs_source\", \"hl2dm\", \"combat_arms\"]",
            "weapons = [\"cs_source\", \"hl2dm\", \"halo_ce\", \"combat_arms\"]",
        )
        .replace("test:gun_game_hl2dm", "test:halo_gun_bots")
}

/// Four bots on `gun` on `map` (None: the greybox).
fn party(map: Option<chimera_world::map::MapData>, gun: &str, bots: u32) -> Sim {
    let mut sim = match map {
        Some(map) => Sim::new(CaSim, (MapSimPlugin::new(map), PartyPartsPlugin)),
        None => Sim::new(CaSim, (GreyboxMapPlugin, PartyPartsPlugin)),
    };
    crate::gun_game::no_finale(sim.app.world_mut());
    crate::gun_game::plain_rungs(sim.app.world_mut());
    sim.app.add_minigames(&[("halo_gun_bots.toml", &ladder(gun))]).unwrap();
    minigame::start(sim.app.world_mut(), "test:halo_gun_bots").expect("the gun game starts");
    let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
    s.quota = bots;
    s.quota_set = true;
    sim
}

/// What the bots did with the gun.
#[derive(Resource, Default, Debug, Clone)]
pub(crate) struct Tally {
    pub rounds: u32,
    pub hits: u32,
    pub kills: u32,
    pub suicides: u32,
    pub reloads: u32,
    pub overheats: u32,
    pub dry: u32,
    /// Per bot: the last round's time (s).
    last: HashMap<Entity, f64>,
    /// Per bot: since when it has been engaged without a round (s).
    engaged: HashMap<Entity, f64>,
    /// The longest time any bot stood engaged without a round (s).
    pub longest_idle: f64,
    /// Engaged time in all (bot-seconds), and of it what passed in
    /// spells of 5 s or more without a round.
    pub engaged_s: f64,
    pub idle_s: f64,
}

fn count(mut events: MessageReader<WeaponEvent>, mut damage: MessageReader<Damage>, mut died: MessageReader<Died>, mut t: ResMut<Tally>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    for e in events.read() {
        if e.replay {
            continue;
        }
        match &e.kind {
            WeaponEventKind::Fired { .. } | WeaponEventKind::Action("fire" | "fire_last" | "fire_charged") => {
                t.rounds += 1;
                t.last.insert(e.owner, now);
            }
            WeaponEventKind::ReloadStarted { .. } => t.reloads += 1,
            WeaponEventKind::Action("overheated") => t.overheats += 1,
            WeaponEventKind::DryFire => t.dry += 1,
            _ => {}
        }
    }
    for d in damage.read() {
        if d.attacker.is_some_and(|a| a != d.target) {
            t.hits += 1;
        }
    }
    for d in died.read() {
        if d.attacker.is_none_or(|a| a == d.entity) {
            t.suicides += 1;
        } else {
            t.kills += 1;
        }
    }
}

/// Each bot engaged (attacking a target it sees) without a round: how
/// long.
fn watch(bots: Query<(Entity, &CaBot)>, mut t: ResMut<Tally>, time: Res<Time>) {
    let now = time.elapsed_secs_f64();
    let dt = time.delta_secs_f64();
    for (e, bot) in &bots {
        let engaged = matches!(bot.state, BotState::Attack { .. }) && bot.visible && bot.target.is_some();
        if !engaged {
            t.engaged.remove(&e);
            continue;
        }
        t.engaged_s += dt;
        let since = *t.engaged.entry(e).or_insert(now);
        let from = t.last.get(&e).copied().unwrap_or(since).max(since);
        let idle = now - from;
        if idle >= 5.0 {
            t.idle_s += dt;
        }
        t.longest_idle = t.longest_idle.max(idle);
    }
}

/// `gun`'s bots over `MINUTES` (after the bots are in and the mesh made).
pub(crate) fn measure(map: Option<chimera_world::map::MapData>, gun: &str, bots: u32, minutes: f64) -> Tally {
    let mut sim = party(map, gun, bots);
    sim.app.init_resource::<Tally>().add_systems(FixedPostUpdate, (count, watch));
    sim.seconds(6.0);
    *sim.app.world_mut().resource_mut::<Tally>() = Tally::default();
    sim.seconds(minutes * 60.0);
    sim.app.world().resource::<Tally>().clone()
}

fn table(map: &str, map_data: impl Fn() -> Option<chimera_world::map::MapData>) -> Vec<(&'static str, Tally)> {
    println!("{map}: 4 bots, {MINUTES} min each");
    println!("{:32} {:>7} {:>6} {:>6} {:>7} {:>5} {:>8} {:>8} {:>5} {:>9} {:>8}", "gun", "rounds", "hits", "kills", "kills/m", "self", "reloads", "overheat", "dry", "idle max", "idle 5s+");
    let mut rows = Vec::new();
    for gun in GUNS {
        let t = measure(map_data(), gun, 4, MINUTES);
        println!(
            "{gun:32} {:>7} {:>6} {:>6} {:>7.1} {:>5} {:>8} {:>8} {:>5} {:>8.1}s {:>7.0}s",
            t.rounds,
            t.hits,
            t.kills,
            t.kills as f64 / MINUTES,
            t.suicides,
            t.reloads,
            t.overheats,
            t.dry,
            t.longest_idle,
            t.idle_s
        );
        rows.push((gun, t));
    }
    rows
}

fn check(rows: &[(&str, Tally)]) {
    for (gun, t) in rows {
        assert!(t.rounds > 0, "{gun}: the bots never fired");
    }
}

#[test]
fn bots_fire_and_kill_with_each_of_halos_guns_on_the_greybox() {
    let rows = table("greybox", || None);
    check(&rows);
}
