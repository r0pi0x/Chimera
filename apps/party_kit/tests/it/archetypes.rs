//! What a long session leaves in the world (backlog 2026-10-28, "Frame
//! rate drops after a few map changes"; docs/performance.md, "Map changes
//! that slowed the tick"). Bevy never frees an archetype or a table, and
//! the party's characters carry ~130 components of which dozens come and
//! go (spawn protection, a sleeping body, the body model a map change
//! takes off and puts back, the movement and view parts of the gun's
//! game), so every map change and every death visits new component sets:
//! a session's world reached 3000 archetypes and 1700 tables, most of
//! them characters' and empty. Bots play the gun game here while their
//! component sets are run through 2047 new combinations (test markers put
//! on and taken off):
//!
//! - as sparse-set markers (new archetypes, no new tables), the tick must
//!   cost about the same: 2.5x before the per-tick exclusive code kept its
//!   queries (`chimera_core::query_cache`), each `world.query` walking
//!   every archetype a character's components are in;
//! - as table markers (a table each too), more: every query that matches
//!   characters still steps over their empty tables, Bevy's own cost
//!   (docs/tech-debt.md).
//!
//! Each case plays the same match twice from the same start: once with
//! one marker's archetype, once with all 2047 (`inflate`). The markers
//! move the same bot out of its archetype and back at the same steps in
//! both runs, so the bots keep their places in their archetypes and
//! tables and play the same (the state hashes must match), and the
//! measured ticks are the same ticks: the difference is the archetypes'
//! cost alone, the same on every run. Comparing before and after in one
//! run, as this test first did, compared two moments of the match, and
//! the bots' thinking alone swung by hundreds of thousands of
//! instructions whenever something shifted the entities' ids (a new
//! resource is an entity).
//!
//! Two checks on the tick's systems' instructions (`Sim::work`):
//!
//! - no single system grows by more than `PER_ARCHETYPE` instructions a
//!   tick (over the whole window) for each archetype added: a `world.query` per tick over the
//!   characters' components costs ~380 (planted in Combat Arms'
//!   `supplies`, 2026-11-12: +0.77M a tick at 2045 more archetypes), while
//!   stepping over empty archetypes in a system's queries costs up to ~75
//!   (`Movement step`), Bevy's own;
//! - the whole tick, over the ticks that make no new archetype
//!   (`instructions_per_tick`: a new one costs a step over the others,
//!   once), against `SPARSE_LIMIT` and `TABLE_LIMIT`: the sum of many
//!   systems' smaller steps (1.32x and 2.31x measured, 2026-11-12; the
//!   table case 2.57x on main by the evening of 2026-11-13 and 2.60x
//!   with the spawn spot fixes, whose greybox spawns moved: the base run
//!   1.7% cheaper, the inflated one no dearer, so the limit 2.6 became
//!   2.7; the per-system limit is the one a query made each tick fails).

use bevy::prelude::*;
use chimera_core::metrics::{census::Census, work::WorkCounts};
use chimera_world::harness::Sim;

use crate::gun_game::party;

/// A table marker.
#[derive(Component)]
struct M<const N: usize>;

/// A sparse-set marker.
#[derive(Component)]
#[component(storage = "SparseSet")]
struct S<const N: usize>;

macro_rules! markers {
    ($name:ident, $unname:ident, $t:ident) => {
        fn $name(w: &mut World, e: Entity, bit: usize) {
            let mut e = w.entity_mut(e);
            match bit {
                0 => e.insert($t::<0>),
                1 => e.insert($t::<1>),
                2 => e.insert($t::<2>),
                3 => e.insert($t::<3>),
                4 => e.insert($t::<4>),
                5 => e.insert($t::<5>),
                6 => e.insert($t::<6>),
                7 => e.insert($t::<7>),
                8 => e.insert($t::<8>),
                9 => e.insert($t::<9>),
                _ => e.insert($t::<10>),
            };
        }
        fn $unname(w: &mut World, e: Entity) {
            w.entity_mut(e).remove::<(
                $t<0>,
                $t<1>,
                $t<2>,
                $t<3>,
                $t<4>,
                $t<5>,
                $t<6>,
                $t<7>,
                $t<8>,
                $t<9>,
                $t<10>,
            )>();
        }
    };
}
markers!(mark_table, unmark_table, M);
markers!(mark_sparse, unmark_sparse, S);

/// The party's gun game on the greybox with eight Combat Arms bots,
/// settled.
fn bots_playing() -> (Sim, Vec<Entity>) {
    let mut sim = party();
    // The same play on every run, so the counts compare.
    sim.repeatable();
    for x in [-8.0, 8.0] {
        for z in [-25.0, -15.0, -5.0, 5.0] {
            sim.app.world_mut().spawn((
                chimera_core::core::SpawnPoint::default(),
                Transform::from_translation(Vec3::new(x, 1.0, z)),
            ));
        }
    }
    {
        let mut s = sim.app.world_mut().resource_mut::<combat_arms::game::bots::BotSettings>();
        s.quota = 8;
        s.quota_set = true;
    }
    sim.seconds(5.0);
    let bots: Vec<Entity> = {
        let w = sim.app.world_mut();
        w.query_filtered::<Entity, With<combat_arms::game::bots::CaBot>>().iter(w).collect()
    };
    assert_eq!(bots.len(), 8);
    (sim, bots)
}

/// The instructions per tick of `n` ticks (`Sim::work`: the tick's
/// systems, the same on every run), counting only the ticks that met no
/// new archetype (`Work::steady_ticks`), and the systems' counts per
/// tick; None without hardware counters. A new archetype has every query
/// that its components could match step once over all the archetypes
/// holding the query's rarest component (Bevy's
/// `QueryState::update_archetypes`), here the bots' thousands: Bevy's
/// cost, paid per archetype made, not per tick, and the bots make some
/// while they play (a gun of another game, a death, a sleeping body).
fn instructions_per_tick(sim: &mut Sim, n: u64) -> Option<(f64, WorkCounts)> {
    let w = sim.work(n)?;
    let steady = w.steady_ticks();
    assert!(steady.len() as u64 >= n / 4, "only {} of {n} ticks made no archetype", steady.len());
    Some((steady.iter().sum::<u64>() as f64 / steady.len() as f64, w.systems.per(n)))
}

/// One phase of a run: the archetypes made and tables after the
/// markers, the instructions per tick after, and the state's hash at the
/// end of the window.
struct Phase {
    now: (usize, usize),
    work: Option<(f64, WorkCounts)>,
    hash: u64,
}

/// Runs the bots' component sets through combinations of 11 markers
/// (`mark`, `unmark`): every one of the 2047 (`all`), or only the first
/// marker each time, which makes one archetype, then measures. Either way
/// each step moves the same bot out of its archetype and back, in the same
/// order, so the bots end in the same places in their archetypes and
/// tables, and play on alike: the runs differ only in the archetypes made.
fn inflate(
    sim: &mut Sim,
    bots: &[Entity],
    all: bool,
    mark: fn(&mut World, Entity, usize),
    unmark: fn(&mut World, Entity),
) -> Phase {
    {
        let w = sim.app.world_mut();
        for combo in 1..2048usize {
            let e = bots[combo % bots.len()];
            let bits = if all { combo } else { 1 };
            for bit in (0..11).filter(|b| bits & (1 << b) != 0) {
                mark(w, e, bit);
            }
            unmark(w, e);
        }
    }
    let c = Census::take(sim.app.world_mut());
    let now = (c.archetypes_made, c.tables);
    // Each kept query meets the new archetypes once (Bevy's one-off cost).
    sim.seconds(1.0);
    let work = instructions_per_tick(sim, 400);
    Phase { now, work, hash: sim.state_hash() }
}

/// The bots playing, then their component sets run through the sparse
/// markers, measured, then through the table markers, measured: with
/// every combination (`all`) or one marker.
fn run(all: bool) -> (Phase, Phase) {
    let (mut sim, bots) = bots_playing();
    // Warm (the first ticks make their own archetypes).
    sim.seconds(3.0);
    let sparse = inflate(&mut sim, &bots, all, mark_sparse, unmark_sparse);
    let table = inflate(&mut sim, &bots, all, mark_table, unmark_table);
    (sparse, table)
}

/// The most a single system's instructions per tick may grow for each
/// archetype added (module doc).
const PER_ARCHETYPE: f64 = 200.0;
/// The most the whole tick may grow with 2045 more sparse-set archetypes.
const SPARSE_LIMIT: f64 = 1.5;
/// The most the whole tick may grow with 4090 more archetypes and 2045
/// more tables.
const TABLE_LIMIT: f64 = 2.7;

/// Checks one case: the whole tick within `limit` times the base run's,
/// and no system grown by more than `PER_ARCHETYPE` per added archetype.
fn check(case: &str, base: &Phase, inflated: &Phase, limit: f64) {
    let (Some((b, sys_b)), Some((a, sys_a))) = (&base.work, &inflated.work) else {
        eprintln!("skipping the comparison: no hardware counters ({})", chimera_core::metrics::hw_status());
        return;
    };
    let (a, b) = (*a, *b);
    let added = inflated.now.0.saturating_sub(base.now.0).max(1);
    let table = sys_a.diff_table(sys_b, 20);
    eprintln!(
        "{case}: archetypes, tables {:?} vs {:?}: {:.3}M vs {:.3}M instructions per tick ({:.3}x, limit {limit}x)\n{table}",
        base.now,
        inflated.now,
        b / 1e6,
        a / 1e6,
        a / b
    );
    let worst = sys_a.rises(sys_b, 1);
    if let Some((label, before, after)) = worst.first() {
        let per = (*after as f64 - *before as f64) / added as f64;
        eprintln!("{case}: most grown system {label}: {per:.0} instructions a tick per added archetype (limit {PER_ARCHETYPE})");
        assert!(
            per <= PER_ARCHETYPE,
            "{case}: {label} cost {before} instructions a tick with {} archetypes, {after} with {}: {per:.0} more per \
             archetype, as a query made each tick costs (a `world.query` in per-tick exclusive code: keep it, \
             `chimera_core::query_cache`). Per tick:\n{table}",
            base.now.0,
            inflated.now.0
        );
    }
    assert!(
        a < b * limit,
        "{case}: a tick cost {:.2}M instructions with {} archetypes and {} tables, {:.2}M with {} and {}: the same \
         ticks of the same play, so the archetypes cost it; something walks every archetype each tick (a \
         `world.query` in per-tick exclusive code: keep it, `chimera_core::query_cache`). Per tick:\n{table}",
        b / 1e6,
        base.now.0,
        base.now.1,
        a / 1e6,
        inflated.now.0,
        inflated.now.1
    );
}

#[test]
fn the_tick_costs_the_same_with_many_more_character_archetypes() {
    // The compute pool is the process's: other tests' tasks would count.
    if !chimera_world::harness::own_process("archetypes::the_tick_costs_the_same_with_many_more_character_archetypes") {
        return;
    }
    let (base_sparse, base_table) = run(false);
    let (sparse, table) = run(true);
    assert!(
        sparse.now.0 >= base_sparse.now.0 + 2000,
        "the markers made archetypes: {:?} vs {:?}",
        base_sparse.now,
        sparse.now
    );
    assert!(
        table.now.1 >= base_table.now.1 + 2000,
        "the table markers made tables: {:?} vs {:?}",
        base_table.now,
        table.now
    );
    assert!(
        base_sparse.hash == sparse.hash && base_table.hash == table.hash,
        "the two runs played apart (`Sim::state_hash`), so their ticks don't compare: the markers must change \
         nothing the game sees, and the simulation must repeat (ultimate_gun_game's \
         `session::a_match_repeats_tick_for_tick`)"
    );
    check("sparse-set markers", &base_sparse, &sparse, SPARSE_LIMIT);
    check("table markers", &base_table, &table, TABLE_LIMIT);
}
