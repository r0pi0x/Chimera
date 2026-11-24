//! A long session in one process (backlog "frame rate decays over a
//! session"; docs/performance.md, "A session's frame rate"): match after
//! match, each with a new ladder from every game's guns, bots walking its
//! rungs and dying. Bevy never frees an archetype or a table, and every
//! query that matches characters steps over each table it matched, on
//! every run: what a session's characters visit must stay a small set.

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, Health, Hitgroup, SpawnPoint},
    metrics::census::Census,
};
use chimera_sim::minigame::{self, ladder::{Ladder, set_rung}};
use chimera_world::harness::{Sim, Work};
use ultimate_gun_game::{MINIGAME, RUNGS, generate};

use crate::parts::ugg;

/// The games the session's ladders draw from, the same on every machine:
/// without them named, a game counted as installed or not by what this
/// machine's installs and asset store hold, and so did the counts.
const PINNED: &[&str] = &["cs_source", "hl2dm", "combat_arms"];

/// Every game the app composes (`party_roster::GAME_PARTS`).
fn every_game() -> Vec<&'static str> {
    party_roster::GAME_PARTS.iter().map(|p| p.game).collect()
}

/// The automatic gun game with eight bots, on room to spawn, its ladders
/// drawn from `games` alone (as this machine's mount report:
/// `chimera_net::minigame_net::installed_by_all`).
fn session(games: &[&str]) -> (Sim, Vec<Entity>) {
    let mut sim = ugg();
    sim.app.world_mut().insert_resource(chimera_net::net::manifest::LocalMounts(chimera_net::net::manifest::MountReport {
        games: games
            .iter()
            .map(|g| chimera_net::net::manifest::GameMount {
                game: g.to_string(),
                installed: true,
                ..Default::default()
            })
            .collect(),
    }));
    // From the start, not only for the measured matches: the first
    // match's tasks (bodies, held models, the map's kit) landing on the
    // tick the machine's load allows made the session's later counts
    // differ from run to run under load.
    sim.repeatable();
    for x in [-8.0, 8.0] {
        for z in [-25.0, -15.0, -5.0, 5.0] {
            sim.app.world_mut().spawn((SpawnPoint::default(), Transform::from_translation(Vec3::new(x, 1.0, z))));
        }
    }
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 8;
        s.quota_set = true;
    }
    new_match(&mut sim, 1);
    sim.seconds(3.0);
    let bots = bots_of(&mut sim);
    assert_eq!(bots.len(), 8);
    (sim, bots)
}

fn bots_of(sim: &mut Sim) -> Vec<Entity> {
    let w = sim.app.world_mut();
    w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect()
}

fn new_match(sim: &mut Sim, seed: u64) {
    let w = sim.app.world_mut();
    generate(w, seed, RUNGS).expect("a ladder");
    chimera_core::gameplay::new_game(w);
    minigame::start(w, MINIGAME).expect("the match starts");
}

fn kill(sim: &mut Sim, p: Entity) {
    let at = sim.position(p);
    sim.app.world_mut().write_message(Damage {
        target: p,
        attacker: None,
        amount: 100_000.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Chest,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
}

/// One match: every bot walks a new ladder's rungs (each its own way
/// through them), and some die on each.
fn play_match(sim: &mut Sim, seed: u64) {
    new_match(sim, seed);
    sim.seconds(0.5);
    let ladder = sim.app.world().resource::<Ladder>().clone();
    for r in 0..ladder.rungs.len() {
        let bots = bots_of(sim);
        for (i, b) in bots.iter().enumerate() {
            let rung = (r + i * 3) % ladder.rungs.len();
            if sim.app.world().get::<Health>(*b).is_some() {
                set_rung(sim.app.world_mut(), *b, rung, &ladder);
            }
            if (r + i) % 3 == 0 {
                kill(sim, *b);
            }
        }
        sim.seconds(0.4);
    }
}

/// What `n` ticks cost (`Sim::work`: instructions in the tick's systems,
/// the same on every run of the same build, idle or loaded); None without
/// hardware counters.
fn cost(sim: &mut Sim, n: u64) -> Option<Work> {
    sim.work(n)
}

/// The median of the ticks that made no new archetype
/// (`Work::steady_ticks`): a death or a ragdoll makes a tick several times
/// another, and each match's fights differ, so a mean follows the fight.
fn steady(w: &Work) -> f64 {
    let mut t = w.steady_ticks();
    t.sort_unstable();
    t.get(t.len() / 2).copied().unwrap_or(0) as f64
}

/// The archetypes after eight matches of `PINNED`'s ladders, and made
/// after the fourth (887 and 11, all the guns' and their shots': see the
/// main test's history). Every resource is an entity, an archetype of its
/// own, whichever game registers it: with TF2's Spy and batch 3b, 950
/// after eight, 11 after the fourth.
const ARCHETYPES_PINNED: usize = 970;
const LATER_PINNED: usize = 40;
/// The same with every game's ladders (943 and 34 with five games; 1026
/// and 16 with DoD:S the sixth, 2026-11-23: a game's guns and bodies
/// add their own, made in the first matches; 1100 and 13 with the rung
/// passives and utility items, 2026-11-24: their weapons' parts, none of
/// them characters', flat from the sixth match).
const ARCHETYPES_EVERY_GAME: usize = 1_150;
const LATER_EVERY_GAME: usize = 60;
/// Characters' archetypes made after the fourth match (none now): a
/// character's parts, kept on or moved together, visit a set that stops
/// growing; a new one is a part that comes and goes.
const LATER_CHARACTERS: usize = 5;

/// One match's counts: archetypes made, tables, and what 60 ticks cost.
type Row = (usize, usize, Option<Work>);

/// Eight matches of ladders from `games` (each its own seed): each one's
/// counts after it, and the session.
fn eight_matches(games: &[&str]) -> (Sim, Vec<Row>) {
    let (mut sim, _) = session(games);
    sim.repeatable();
    let mut rows = Vec::new();
    for m in 0..8u64 {
        play_match(&mut sim, 100 + m);
        let c = Census::take(sim.app.world_mut());
        let tick = cost(&mut sim, 60);
        eprintln!(
            "match {m}: {} archetypes, {} tables, {:?} instructions a tick (steady median; all ticks' median {:?})",
            c.archetypes_made,
            c.tables,
            tick.as_ref().map(steady),
            tick.as_ref().map(Work::median)
        );
        rows.push((c.archetypes_made, c.tables, tick));
    }
    let characters = chimera_core::metrics::census::command(sim.app.world_mut(), &["made".to_string()]);
    eprintln!("{characters:?}");
    (sim, rows)
}

/// The characters' archetypes (those holding `Health`) made since the
/// `from`th.
fn characters_since(world: &World, from: usize) -> usize {
    let Some(health) = world.components().component_id::<Health>() else { return 0 };
    world.archetypes().iter().filter(|a| a.id().index() >= from && a.contains(health)).count()
}

/// The bound on archetypes: under `total` after eight matches, fewer than
/// `later` made after the fourth, and of those fewer than
/// `LATER_CHARACTERS` characters'.
fn archetypes_level_off(sim: &Sim, rows: &[Row], total: usize, later: usize) {
    let since = chimera_core::metrics::census::made_since(sim.app.world(), rows[3].0, 20);
    eprintln!("{since}");
    let last = rows[7].0;
    let characters = characters_since(sim.app.world(), rows[3].0);
    assert!(
        last < total && last - rows[3].0 < later && characters < LATER_CHARACTERS,
        "{} archetypes after four matches, {} after eight (bounds {total} and {later} more): a part that comes and goes \
         on characters, one at a time, makes one for every combination of the others. Keep it on them from the spawn \
         (`chimera_core::character::CharacterPartsExt::on_every_character`), keep it on with a value meaning none, or put \
         a game's parts on and off in one move (docs/DESIGN.md, \"ECS style\"); {characters} of them characters' \
         (bound {LATER_CHARACTERS}). {since}",
        rows[3].0,
        last
    );
}

/// Match after match the characters' tables and archetypes stop growing
/// and a tick costs the same. Before their components that come and go were sparse sets
/// (8 matches): 1112 -> 2272 tables, 7.5M -> 9.1M instructions a tick (means)
/// (20,000 tables in the real app: 8.3M -> 39.8M, and 60 -> 30 fps).
#[test]
fn a_session_of_matches_stops_making_tables() {
    let (mut sim, rows) = eight_matches(PINNED);
    let (first, last) = ((rows[1].0, rows[1].1), (rows[7].0, rows[7].1));
    let what = chimera_core::metrics::census::command(sim.app.world_mut(), &["varies".to_string()]);
    // The growth bound is 80 since the equal-games draw (backlog, "Ladder
    // draw"): each ladder now deals every game's guns about equally, so
    // the first matches meet more kinds of characters; the growth still
    // levels off (+21, +13, +5, +10, +6, +6, +6 tables a match).
    assert!(
        last.1 < 500 && last.1 - first.1 < 80,
        "tables {} after a match, {} after seven more: a component that comes and goes on characters and isn't a sparse \
         set makes one for every combination, and every query matching characters steps over them all, for good. \
         Give it `#[component(storage = \"SparseSet\")]`. {what:?}",
        first.1,
        last.1
    );
    // Archetypes: each costs every tick for good, so the characters must
    // stop making them (before their parts were kept from the spawn: 2,300
    // after four matches and rising; then about 1,140, none new from the
    // sixth; with a game switch's parts kept or moved together, about 960,
    // 351 of them characters', and 40 more after the fourth match; Halo's
    // four guns as rungs, its stand-in movement and shield parts on and
    // off with them, about 1,220, a few new after the sixth; its eight,
    // more Halo rungs to arrive on, about 1,300; with TF2's guns too, 1,367
    // and 104 more after the fourth, until Halo's movement and vitality
    // went on and off in one move and its kick stayed on from the spawn:
    // 1,237, 70 more, 5 of them with Halo's parts; with TF2's classes as
    // bodies, Halo's grenade rung and the sandbox, 1,296 and 118 more, 92
    // of them characters': each rung's parts went on and off one at a
    // time, so every combination of a dozen came, step by step. Now
    // HL2:DM's suit and TF2's class layer go on and off with their
    // movement in one move (`slots::MovementImpl::leave`), and a rung's
    // body name, HL2:DM model, CS:S and TF2 class and Combat Arms' view
    // driver stay on, set to none (`BodyName::none`, `OwnViewAnim(false)`):
    // 1,002 and 31 more, 3 of them characters'. The rest are the guns
    // (and their shots) the later ladders draw for the first time: a gun's
    // build makes its own few once, bounded by the catalogue, not by the
    // matches. `made_since` names what the window made. The count still moves with
    // which ladders the eight matches draw (the range, the balance table and
    // Halo's ninth gun changed the draws), so the bound keeps some room:
    // 1,300 and 80; with the equal-games draw (backlog, "Ladder draw"),
    // which reaches more games' guns in the first matches, 1,318 after
    // eight, 40 since the fourth: 1,400 and 80. The ladders drew from the
    // games this machine's installs and asset store held, so the count
    // was the machine's: with TF2's and Halo's 1,923. Now pinned to CS:S,
    // HL2:DM and Combat Arms (`PINNED`; every game: the next test), 952,
    // 178 of them characters', 11 after the fourth; avian's mass marker
    // (`RecomputeMassProperties`, on and off at each change of a
    // character's collider) no longer comes (`core::no_collider_mass`),
    // and Combat Arms' rung look and on-demand hitboxes, Halo's body kind,
    // the damage scale, the rung classes and `InMinigame` at a match's
    // start stay on with a value meaning none: 887, 112 characters', none
    // after the fourth.)
    archetypes_level_off(&sim, &rows, ARCHETYPES_PINNED, LATER_PINNED);
    // Each match plays a ladder of its own, so their ticks differ; the
    // counts repeat from run to run (the session plays out the same:
    // `a_match_repeats_tick_for_tick`), so the bound only has to hold the
    // ladders' spread, not the machine's.
    if let (Some(a), Some(b)) = (&rows[1].2, &rows[7].2) {
        let (sa, sb) = (steady(a), steady(b));
        assert!(
            sb < sa * 1.25,
            "a tick cost {sa:.0} instructions after a match, {sb:.0} after seven more (per tick, by system):\n{}",
            b.systems.per(60).diff_table(&a.systems.per(60), 25)
        );
    }
}

/// The same with every game's ladders (all five composed games:
/// TF2's classes and Halo's bodies, movements and vitality in play too):
/// what the user's long session played (5 minutes: 4,109 archetypes, the
/// tick from 0.54 to 1.51 ms). Each game's built-in parts where its
/// install or the asset store isn't reachable, so the bound holds either
/// way. Before the fixes above: 1,337, 521 characters'; now 943, 130, none
/// after the fourth match.
#[test]
fn a_session_with_every_game_stops_making_archetypes() {
    let (sim, rows) = eight_matches(&every_game());
    archetypes_level_off(&sim, &rows, ARCHETYPES_EVERY_GAME, LATER_EVERY_GAME);
}

/// The session's matches play out the same on every run, and cost the
/// same to well under a per cent (docs/performance.md, "Work counts"):
/// sessions in one process end in the same state (`Sim::state_hash`),
/// and their ticks count the same instructions.
#[test]
fn a_match_repeats_tick_for_tick() {
    if !chimera_world::harness::own_process("session::a_match_repeats_tick_for_tick") {
        return;
    }
    let mut runs = Vec::new();
    // The first run fills the process's caches (models, data); the next
    // two must agree.
    for _ in 0..3 {
        let (mut sim, _) = session(PINNED);
        sim.repeatable();
        play_match(&mut sim, 100);
        let work = cost(&mut sim, 60);
        runs.push((sim.state_hash(), work));
    }
    assert!(
        runs[0].0 == runs[1].0 && runs[1].0 == runs[2].0,
        "three runs of the same match ended apart: something in the simulation isn't deterministic"
    );
    let (Some(a), Some(b)) = (&runs[1].1, &runs[2].1) else {
        eprintln!("skipping the counts: no hardware counters ({})", chimera_core::metrics::hw_status());
        return;
    };
    let (ta, tb) = (a.total() as f64, b.total() as f64);
    eprintln!("a match's 60 ticks: {ta:.0} and {tb:.0} instructions");
    assert!(
        (ta - tb).abs() / ta < 0.01,
        "the same ticks cost {ta:.0} and {tb:.0} instructions:\n{}",
        b.systems.diff_table(&a.systems, 20)
    );
}



