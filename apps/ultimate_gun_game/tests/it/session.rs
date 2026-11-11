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

/// The automatic gun game with eight bots, on room to spawn.
fn session() -> (Sim, Vec<Entity>) {
    let mut sim = ugg();
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

/// Match after match the characters' tables and archetypes stop growing
/// and a tick costs the same. Before their components that come and go were sparse sets
/// (8 matches): 1112 -> 2272 tables, 7.5M -> 9.1M instructions a tick (means)
/// (20,000 tables in the real app: 8.3M -> 39.8M, and 60 -> 30 fps).
#[test]
fn a_session_of_matches_stops_making_tables() {
    let (mut sim, _) = session();
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
    let (first, last) = ((rows[1].0, rows[1].1), (rows[7].0, rows[7].1));
    let what = chimera_core::metrics::census::command(sim.app.world_mut(), &["varies".to_string()]);
    assert!(
        last.1 < 500 && last.1 - first.1 < 60,
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
    // 351 of them characters', and 40 more after the fourth match).
    let made = chimera_core::metrics::census::made_by(sim.app.world(), "Health", 20);
    eprintln!("{made}");
    assert!(
        last.0 < 1150 && last.0 - rows[3].0 < 100,
        "{} archetypes after four matches, {} after eight: a part that comes and goes on characters, one at a time, makes \
         one for every combination of the others. Keep it on them from the spawn \
         (`chimera_core::character::CharacterPartsExt::on_every_character`) or put a game's parts on and off in one move \
         (docs/DESIGN.md, \"ECS style\"). {made}",
        rows[3].0,
        last.0
    );
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
        let (mut sim, _) = session();
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



