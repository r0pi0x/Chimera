//! The match end in the Signal look (`client::match_end_signal`), staged
//! with its made-up players (`stage`): the standings with their marks
//! (your row, the selected winner's pointer, session wins), the winner's
//! slab sweeping on the kit's clock, and the climb chart following the
//! hovered row (drawn along its length), back to the selected one; the
//! shell's theme keeps the old screen. The clock is stepped by hand
//! (`UiClock::manual`).

use bevy::prelude::*;
use chimera_client::client::{
    match_end::{MatchEndPlugin, MatchEndRoot, MatchEndStaged, MatchEndView},
    match_end_signal::{self, ClimbChart, ClimbSeg, EndSelection, EndState, SignalEndRoot, StandingsRow, VoteCard, VoteCountdown, VoteTicks, WinnerSlab},
};
use chimera_sim::SimBasePlugins;
use chimera_ui::{Theme, UiClock};
use chimera_world::harness::Sim;

const SIGNAL: &str = "name = \"test\"\n[flags.hud]\nsignal = true\n";

fn sim(signal: bool) -> Sim {
    let mut sim = Sim::with(SimBasePlugins, move |app| {
        if signal {
            app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
        }
        app.insert_resource(UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        app.add_plugins(MatchEndPlugin);
    });
    match_end_signal::stage(sim.app.world_mut(), None).unwrap();
    sim.ticks(3);
    sim
}

fn step(sim: &mut Sim, dt: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(dt);
    sim.ticks(1);
}

fn visible(sim: &mut Sim, root: bool) -> bool {
    let w = sim.app.world_mut();
    let v = if root {
        *w.query_filtered::<&Visibility, With<SignalEndRoot>>().single(w).unwrap()
    } else {
        *w.query_filtered::<&Visibility, With<MatchEndRoot>>().single(w).unwrap()
    };
    v != Visibility::Hidden
}

fn chart(sim: &mut Sim) -> usize {
    let w = sim.app.world_mut();
    w.query::<&ClimbChart>().single(w).unwrap().index
}

/// The chosen climb's line: how much of it shows (px of its segments).
fn drawn(sim: &mut Sim) -> f32 {
    let w = sim.app.world_mut();
    w.query_filtered::<&Node, With<ClimbSeg>>()
        .iter(w)
        .map(|n| match (n.width, n.height) {
            (Val::Px(a), Val::Px(b)) => a.max(b),
            _ => 0.0,
        })
        .sum()
}

#[test]
fn standings_carry_the_marks_and_the_winner_sweeps_in() {
    let mut sim = sim(true);
    assert!(visible(&mut sim, true) && !visible(&mut sim, false), "the Signal screen, not the shell's");
    let w = sim.app.world_mut();
    let rows: Vec<StandingsRow> = w.query::<&StandingsRow>().iter(w).copied().collect();
    assert_eq!(rows.len(), 8);
    assert_eq!(rows.iter().filter(|r| r.you).map(|r| r.index).collect::<Vec<_>>(), [2], "your row");
    assert_eq!(sim.app.world().resource::<EndSelection>().pinned, 0, "the winner's row selected");
    let wins: Vec<u32> = {
        let w = sim.app.world_mut();
        w.query::<&chimera_client::client::race_chip::SessionWins>().iter(w).map(|s| s.0).collect()
    };
    assert_eq!(wins.len(), 4, "only players with wins show the trophy: {wins:?}");
    // The slab: hidden before .55 s, across the name mid-way, gone after.
    let slab = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let (n, v) = w.query_filtered::<(&Node, &Visibility), With<WinnerSlab>>().single(w).unwrap();
        (*v != Visibility::Hidden, n.width)
    };
    assert!(!slab(&mut sim).0, "not yet");
    step(&mut sim, 0.85);
    let (on, width) = slab(&mut sim);
    assert!(on && matches!(width, Val::Px(w) if w > 0.0), "sweeping: {width:?}");
    step(&mut sim, 1.0);
    assert!(!slab(&mut sim).0, "gone after the sweep");
}

#[test]
fn climb_chart_follows_hover() {
    let mut sim = sim(true);
    assert_eq!(chart(&mut sim), 0, "the winner's climb first");
    let full = drawn(&mut sim);
    assert!(full > 0.0, "drawn at once the first time");
    // Hover the third row: its climb, drawn from nothing along the line.
    sim.app.world_mut().resource_mut::<EndSelection>().hovered = Some(3);
    sim.ticks(2);
    assert_eq!(chart(&mut sim), 3);
    assert!(sim.app.world().resource::<EndState>().drawing.is_some());
    let start = drawn(&mut sim);
    step(&mut sim, 0.15);
    let mid = drawn(&mut sim);
    step(&mut sim, 1.2);
    let end = drawn(&mut sim);
    assert!(start < mid && mid < end, "drawn along its length: {start} {mid} {end}");
    // Off the row: the selected one again; a click pins another.
    sim.app.world_mut().resource_mut::<EndSelection>().hovered = None;
    sim.ticks(2);
    assert_eq!(chart(&mut sim), 0);
    sim.app.world_mut().resource_mut::<EndSelection>().pinned = 5;
    sim.ticks(2);
    assert_eq!(chart(&mut sim), 5);
}

#[test]
fn the_shells_theme_keeps_the_old_screen() {
    let mut sim = sim(false);
    assert!(sim.app.world().resource::<MatchEndView>().shown);
    assert!(!visible(&mut sim, true), "no Signal screen without the theme");
}

/// One countdown (the vote's close and the next match's start) with its
/// tick bar lit as it runs; four cards, yours marked; at the lock the
/// others dim, the winner is named under the countdown, which reads 0.
#[test]
fn match_end_has_one_countdown() {
    let mut sim = sim(true);
    let count = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query_filtered::<&Text, With<VoteCountdown>>().iter(w).map(|t| t.0.clone()).collect::<Vec<_>>()
    };
    assert_eq!(count(&mut sim), ["00:13"], "one countdown, 13 s left (staged)");
    let lit = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let bar = w.query::<&VoteTicks>().single(w).unwrap().clone();
        let m = w.resource::<Assets<chimera_ui::TickBarMaterial>>().get(&bar.material).unwrap();
        (m.counts.y as u32, m.counts.z as u32)
    };
    assert_eq!(lit(&mut sim), (14, 34), "7 of 20 s gone of 40 ticks; the last 3 s hot");
    let cards = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let mut c: Vec<VoteCard> = w.query::<&VoteCard>().iter(w).copied().collect();
        c.sort_by_key(|c| c.index);
        c
    };
    let c = cards(&mut sim);
    assert_eq!(c.len(), 4);
    assert_eq!(c.iter().filter(|c| c.mine).map(|c| c.index).collect::<Vec<_>>(), [2], "your vote");
    assert!(c.iter().all(|c| !c.locked));
    // The close: the winner (most votes), locked, named, 0 left.
    sim.app.world_mut().resource_mut::<MatchEndStaged>().view.vote.as_mut().unwrap().winner = Some(0);
    sim.ticks(3);
    let c = cards(&mut sim);
    assert!(c.iter().all(|c| c.locked) && c[0].won && !c[1].won, "{c:?}");
    assert_eq!(count(&mut sim), ["00:00"]);
    assert_eq!(lit(&mut sim).0, 40);
    let hint = {
        let w = sim.app.world_mut();
        let view = w.resource::<MatchEndView>().clone();
        match_end_signal::vote_hint(&view)
    };
    assert_eq!(hint, "Next: de_dust2 \u{2022} starting");
}
