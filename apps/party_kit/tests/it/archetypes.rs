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
//!   cost the same: 2.5x before the per-tick exclusive code kept its
//!   queries (`chimera_core::query_cache`), each `world.query` walking
//!   every archetype a character's components are in;
//! - as table markers (a table each too), within 2.2x (8.4M instructions
//!   a tick at 4800 archetypes and 2400 tables, 24.6M before the fix):
//!   every query that matches characters still steps over their empty
//!   tables, Bevy's own cost (docs/tech-debt.md).
//!
//! Both count the tick's systems' instructions (`Sim::work`, the same on
//! every run, and the systems that grew on a failure) over the ticks that
//! make no new archetype (`instructions_per_tick`): a new one costs a
//! step over the others, once.

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

/// Runs the bots' component sets through every combination of 11
/// markers (`mark`, `unmark`), then measures again: (archetypes and tables
/// made before, after, instructions per tick before, after).
fn inflate(
    sim: &mut Sim,
    bots: &[Entity],
    mark: fn(&mut World, Entity, usize),
    unmark: fn(&mut World, Entity),
) -> ((usize, usize), (usize, usize), Option<(f64, WorkCounts)>, Option<(f64, WorkCounts)>) {
    let counts = |sim: &mut Sim| {
        let c = Census::take(sim.app.world_mut());
        (c.archetypes_made, c.tables)
    };
    let before = instructions_per_tick(sim, 400);
    let was = counts(sim);
    {
        let w = sim.app.world_mut();
        for combo in 1..2048usize {
            let e = bots[combo % bots.len()];
            for bit in (0..11).filter(|b| combo & (1 << b) != 0) {
                mark(w, e, bit);
            }
            unmark(w, e);
        }
    }
    let now = counts(sim);
    sim.seconds(1.0);
    let after = instructions_per_tick(sim, 400);
    (was, now, before, after)
}

#[test]
fn the_tick_costs_the_same_with_many_more_character_archetypes() {
    let (mut sim, bots) = bots_playing();
    // Warm (the first ticks make their own archetypes).
    sim.seconds(3.0);
    let (was, now, before, after) = inflate(&mut sim, &bots, mark_sparse, unmark_sparse);
    assert!(now.0 >= was.0 + 2000, "the markers made archetypes: {was:?} -> {now:?}");
    let (was_t, now_t, before_t, after_t) = inflate(&mut sim, &bots, mark_table, unmark_table);
    assert!(now_t.1 >= was_t.1 + 2000, "the table markers made tables: {was_t:?} -> {now_t:?}");
    let (Some((before, sys_before)), Some((after, sys_after)), Some((before_t, sys_before_t)), Some((after_t, sys_after_t))) =
        (before, after, before_t, after_t)
    else {
        eprintln!("skipping the comparison: no hardware counters ({})", chimera_core::metrics::hw_status());
        return;
    };
    eprintln!(
        "sparse markers: archetypes, tables {was:?} -> {now:?}: {:.2}M -> {:.2}M instructions per tick ({:.2}x)",
        before / 1e6,
        after / 1e6,
        after / before
    );
    eprintln!(
        "table markers: archetypes, tables {was_t:?} -> {now_t:?}: {:.2}M -> {:.2}M instructions per tick ({:.2}x)",
        before_t / 1e6,
        after_t / 1e6,
        after_t / before_t
    );
    assert!(
        after < before * 1.3,
        "a tick cost {:.2}M instructions with {} archetypes, {:.2}M with {}: something walks every archetype \
         each tick (a `world.query` in per-tick exclusive code: keep it, `chimera_core::query_cache`). Per tick:\n{}",
        before / 1e6,
        was.0,
        after / 1e6,
        now.0,
        sys_after.diff_table(&sys_before, 20)
    );
    assert!(
        after_t < before_t * 2.2,
        "a tick cost {:.2}M instructions with {} tables, {:.2}M with {}. Per tick:\n{}",
        before_t / 1e6,
        was_t.1,
        after_t / 1e6,
        now_t.1,
        sys_after_t.diff_table(&sys_before_t, 20)
    );
}
