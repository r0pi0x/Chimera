//! A long session in one process (backlog "frame rate decays over a
//! session"; docs/performance.md, "A session's frame rate"): match after
//! match, each with a new ladder from every game's guns, bots walking its
//! rungs and dying. Bevy never frees an archetype or a table, and every
//! query that matches characters steps over each table it matched, on
//! every run: what a session's characters visit must stay a small set.

use bevy::prelude::*;
use chimera_core::{
    core::{Damage, DamageKind, Health, Hitgroup, SpawnPoint},
    metrics::{ThreadCounters, census::Census},
};
use chimera_sim::minigame::{self, ladder::{Ladder, set_rung}};
use chimera_world::harness::Sim;
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

/// Instructions per tick on this thread, over `n` ticks without a new
/// archetype; None without hardware counters.
fn cost(sim: &mut Sim, n: usize) -> Option<f64> {
    let mut c = ThreadCounters::current();
    let (mut total, mut steady) = (0u64, 0u64);
    let mut last = sim.app.world().archetypes().len();
    for _ in 0..n {
        let a = c.read().hw?.instructions;
        sim.ticks(1);
        let used = c.read().hw?.instructions.saturating_sub(a);
        let now = sim.app.world().archetypes().len();
        if now == last {
            total += used;
            steady += 1;
        }
        last = now;
    }
    (steady > 0).then(|| total as f64 / steady as f64)
}

/// Match after match the characters' tables and archetypes stop growing
/// and a tick costs the same. Before their components that come and go were sparse sets
/// (8 matches): 1112 -> 2272 tables, 7.5M -> 9.1M instructions a tick
/// (20,000 tables in the real app: 8.3M -> 39.8M, and 60 -> 30 fps).
#[test]
fn a_session_of_matches_stops_making_tables() {
    let (mut sim, _) = session();
    let mut rows = Vec::new();
    for m in 0..8u64 {
        play_match(&mut sim, 100 + m);
        let c = Census::take(sim.app.world_mut());
        let tick = cost(&mut sim, 60);
        eprintln!("match {m}: {} archetypes, {} tables, {:?} instructions a tick", c.archetypes_made, c.tables, tick);
        rows.push((c.archetypes_made, c.tables, tick));
    }
    let (first, last) = (rows[1], rows[7]);
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
    // after four matches and rising; now about 1,200, none new from the
    // sixth).
    let made = chimera_core::metrics::census::made_by(sim.app.world(), "Health", 20);
    assert!(
        last.0 < 1600 && last.0 - rows[3].0 < 200,
        "{} archetypes after four matches, {} after eight: a part that comes and goes on characters, one at a time, makes \
         one for every combination of the others. Keep it on them from the spawn \
         (`chimera_core::character::CharacterPartsExt::on_every_character`) or put a game's parts on and off in one move \
         (docs/DESIGN.md, \"ECS style\"). {made}",
        rows[3].0,
        last.0
    );
    if let (Some(a), Some(b)) = (first.2, last.2) {
        assert!(b < a * 1.25, "a tick cost {a:.0} instructions after a match, {b:.0} after seven more");
    }
}

