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
//! `halo_ce::game::weapons::systems::battery_rounds` since). And the
//! bots let go of the trigger, re-aimed, paced the next round and heard
//! gunfire only on the engine's `WeaponEventKind::Fired`, the engine
//! frame's hitscan command, which an own-frame weapon such as Halo's
//! doesn't send, so a bot held a semi-automatic Halo gun's trigger down:
//! in a minute the sniper rifle's bots fired 29 rounds for 3 kills (one
//! stood 29 s engaged without a round) and no bot heard a Halo shot. Since
//! they read the shot fact (`WeaponEventKind::ShotMade`, which Halo's
//! frame sends too): 59 rounds, 15 kills, its magazines and reloads
//! (asserted), and a Halo shot behind a bot is heard (asserted). The
//! plasma pistol's press only begins a charge (no shot to let go on), so
//! its bots held each charge to the charged bolt and overheated (53 bolts,
//! 35 overheats, no kill a minute); since its trigger states that it fires
//! when released (`weapon::FiresOnRelease`) the bots tap it (asserted: no
//! charged bolt, at most 3 overheats, kills).

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
    /// The plasma pistol's charged bolts.
    pub charged: u32,
    /// Halo's grenade rung's throws (its shot facts).
    pub throws: u32,
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
        if matches!(e.kind, WeaponEventKind::ShotMade { id: weapons::rung::FRAG_RUNG, .. }) {
            t.throws += 1;
        }
        if matches!(e.kind, WeaponEventKind::Action("fire_charged")) {
            t.charged += 1;
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
        // Every gun's bots let go of the trigger after a round (Halo's by
        // the shot fact, `WeaponEventKind::ShotMade`): none stands engaged
        // without a round for 10 s (the sniper rifle's did for 29 s).
        assert!(t.longest_idle < 10.0, "{gun}: a bot engaged {:.1} s without a round", t.longest_idle);
    }
    let row = |id| &rows.iter().find(|(g, _)| *g == id).unwrap().1;
    let sniper = row(weapons::SNIPER_RIFLE);
    assert!(sniper.reloads > 0 && sniper.rounds >= 4 * sniper.reloads, "the sniper rifle's magazines: {sniper:?}");
    // The plasma pistol fires when its trigger is released
    // (`weapon::FiresOnRelease`): tapped, never held to the charged bolt.
    let pistol = row(weapons::PLASMA_PISTOL);
    assert_eq!(pistol.charged, 0, "the plasma pistol tapped, never charged: {pistol:?}");
    assert!(pistol.overheats <= 3, "the plasma pistol tapped, not overheated: {pistol:?}");
    assert!(pistol.kills > 0, "the plasma pistol kills: {pistol:?}");
}

/// A Halo player's shots behind a bot are gunfire it hears (as another
/// game's `Fired` rounds are): the stimulus from the shooter, and the bot
/// leaves its patrol.
#[test]
fn a_bot_hears_a_halo_shot_behind_it() {
    use combat_arms::game::bots::brain::{Kind, Stimuli};
    let mut sim = party(None, weapons::ASSAULT_RIFLE, 1);
    sim.seconds(6.0);
    let shooter = crate::gun_game::soldier(&mut sim);
    sim.ticks(2);
    {
        let w = sim.app.world_mut();
        let l = w.resource::<chimera_sim::minigame::ladder::Ladder>().clone();
        chimera_sim::minigame::ladder::set_rung(w, shooter, 0, &l);
    }
    sim.seconds(2.0);
    let w = sim.app.world_mut();
    let (bot, at) = w.query::<(Entity, &Transform, &CaBot)>().iter(w).map(|(e, t, _)| (e, t.translation)).next().expect("a bot");
    assert_eq!(w.get::<CaBot>(bot).unwrap().state, BotState::Patrol, "patrolling first");
    let mut heard = false;
    // A short burst (a shot is reacted to 0.5-1.5 s later; another shot
    // sets that back), then 3 s to react.
    for _ in 0..20 {
        // 8 m behind the bot (it looks down its yaw), aiming away.
        let yaw = sim.intent(bot).yaw;
        let behind = at + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 8.0;
        sim.app.world_mut().get_mut::<Transform>(shooter).unwrap().translation = behind;
        sim.intent(shooter).yaw = yaw + std::f32::consts::PI;
        sim.intent(shooter).fire = true;
        sim.ticks(1);
        heard |= sim.app.world().resource::<Stimuli>().list.iter().any(|s| s.kind == Kind::Gunfire && s.source == shooter);
    }
    assert!(heard, "no gunfire stimulus from the Halo player");
    sim.intent(shooter).fire = false;
    let mut left = false;
    for _ in 0..200 {
        sim.ticks(1);
        left |= sim.app.world().get::<CaBot>(bot).unwrap().state != BotState::Patrol;
    }
    assert!(left, "the bot never left its patrol");
}

#[test]
fn bots_fire_and_kill_with_each_of_halos_guns_on_the_greybox() {
    let rows = table("greybox", || None);
    check(&rows);
}

/// Halo's grenade rung (`weapons::rung`): four bots on it on the greybox
/// for a minute press its grenade action (`weapon::AttackAction`), throw
/// and kill, and fewer of their deaths are their own than their kills.
#[test]
fn bots_clear_halos_grenade_rung_on_the_greybox() {
    let t = measure(None, weapons::rung::FRAG_RUNG, 4, MINUTES);
    println!("frag rung, greybox, 4 bots, {MINUTES} min: {} throws, {} kills, {} self-kills, {} dry pulls", t.throws, t.kills, t.suicides, t.dry);
    assert!(t.throws > 0, "the bots threw: {t:?}");
    assert!(t.kills >= 1, "a kill: {t:?}");
    assert!(t.suicides < t.kills, "fewer self-kills than kills: {t:?}");
}
