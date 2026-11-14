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
    match_end_signal::{self, EndGround, LeaveButton, ClimbChart, ClimbSeg, EndSelection, EndState, SignalEndRoot, StandingsRow, VoteCard, VoteCountdown, VoteTicks, WinnerSlab},
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

/// The ground (the ambient and the planes over it) under every part, and
/// the way out.
#[test]
fn the_ground_and_leave() {
    let mut sim = sim(true);
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&EndGround>().iter(w).count(), 2, "the ambient and the planes");
    assert_eq!(w.query::<&LeaveButton>().iter(w).count(), 1);
}

/// Off the stage: the standings read the session wins from the results
/// the server sends (`MatchResults`' `ResultRow::wins`, this match
/// counted), "+1" on the winner's row only, no trophy without wins.
#[test]
fn the_standings_read_the_servers_session_wins() {
    use chimera_client::client::race_chip::SessionWins;
    use chimera_sim::minigame::{NextMatch, hud::MinigameHud, results::{MatchResults, ResultRow}};
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.insert_resource(Theme::from_toml(SIGNAL).expect("theme"));
        app.insert_resource(UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        app.add_plugins(MatchEndPlugin);
    });
    let row = |name: &str, wins, winner| ResultRow { name: name.into(), kills: 3, deaths: 1, rung: Some(2), winner, wins, ..Default::default() };
    let results = MatchResults {
        title: "Kit gun game".into(),
        rungs: 3,
        rows: vec![row("Ann", 2, true), row("Bot Cy", 3, false), row("Bob", 0, false)],
        ..default()
    };
    sim.app.insert_resource(MinigameHud { title: "Kit gun game".into(), results: Some(results), ..default() });
    let at = sim.app.world().resource::<chimera_core::core::SimClock>().now + 20.0;
    sim.app.insert_resource(NextMatch { map: None, at });
    sim.ticks(3);
    assert!(visible(&mut sim, true), "the Signal screen");
    let w = sim.app.world_mut();
    let mut wins: Vec<u32> = w.query::<&SessionWins>().iter(w).map(|s| s.0).collect();
    wins.sort();
    assert_eq!(wins, [2, 3], "Ann's (this match counted) and the bot's; none for Bob");
    // "+1" once, under the winner's row (the first).
    let plus: Vec<Entity> = w.query::<(Entity, &Text)>().iter(w).filter(|(_, t)| t.0 == "+1").map(|(e, _)| e).collect();
    assert_eq!(plus.len(), 1, "one +1");
    let mut at = plus[0];
    let row = loop {
        let parent = w.get::<ChildOf>(at).expect("under a row").parent();
        if let Some(r) = w.get::<StandingsRow>(parent) {
            break *r;
        }
        at = parent;
    };
    assert_eq!(row.index, 0, "on the winner's row");
}

/// The games' HUDs under the screen (`hud::HudCover`): their roots and
/// their parts outside a root hidden while it is up, a part its game shows
/// meanwhile kept hidden; after, each set back (to what its game set
/// last), a part its game had hidden left hidden.
#[test]
fn the_games_huds_are_covered_while_it_is_up() {
    use chimera_client::client::hud::{HudCover, HudPart, HudRoot};
    let mut sim = sim(true);
    let hud = sim.app.world().get_resource::<chimera_core::gameplay::Gameplay>().map_or("test", |g| g.hud);
    let w = sim.app.world_mut();
    let root = w.spawn((Node::default(), HudRoot(hud))).id();
    let part = w.spawn((Node::default(), HudPart, Visibility::Hidden)).id();
    let off = w.spawn((Node::default(), HudPart, Visibility::Hidden)).id();
    sim.ticks(1);
    let vis = |sim: &Sim, e: Entity| *sim.app.world().get::<Visibility>(e).unwrap();
    assert!(sim.app.world().resource::<HudCover>().covered());
    assert_eq!(vis(&sim, root), Visibility::Hidden);
    // Its game shows the part under the screen: hidden still.
    *sim.app.world_mut().get_mut::<Visibility>(part).unwrap() = Visibility::Visible;
    sim.ticks(1);
    assert_eq!(vis(&sim, part), Visibility::Hidden);
    match_end_signal::unstage(sim.app.world_mut());
    sim.ticks(2);
    assert!(!sim.app.world().resource::<HudCover>().covered());
    assert_eq!(vis(&sim, root), Visibility::Inherited, "the root back");
    assert_eq!(vis(&sim, part), Visibility::Visible, "the part as its game set it");
    assert_eq!(vis(&sim, off), Visibility::Hidden, "a part its game hid stays hidden");
}

/// The longest stall's label clears the chosen climb (above the box,
/// ending at its right end, where the line climbs on; under it for a
/// stall to the match's end) and the box; its outline is dashed.
#[test]
fn the_stall_label_clears_the_line() {
    use chimera_client::client::match_end_signal::{ChartAxes, StallLabel, stall_dashes};
    let sim = sim(true);
    let view = sim.app.world().resource::<MatchEndView>().clone();
    let end = view.length;
    let mut checked = 0;
    for row in &view.rows {
        let axes = ChartAxes::new(view.length, view.rungs.max(row.rung.unwrap_or(1)));
        let Some((a, b, r)) = row.climb.longest_stall(end) else { continue };
        let at = StallLabel::place(&axes, a, b, r as u32, end);
        // "STALL 1:08 • R08": 16 characters of the 6.4-unit mono type,
        // tracked, with the plate's padding (generous).
        let plate = at.plate(16.0 * 6.4 * 0.72 + 2.0 * StallLabel::PAD_X);
        let pts = axes.points(&row.climb, end);
        for s in pts.windows(2) {
            let seg = Rect::from_corners(s[0], s[1]).inflate(1.6 / 2.0);
            assert!(plate.intersect(seg).is_empty(), "{}: the label {plate:?} crosses the line at {seg:?}", row.name);
        }
        let (x0, x1, y) = (axes.x(a), axes.x(b), axes.y(r as f32));
        let stall = Rect::new(x0, y - 5.0, x1, y + 5.0);
        assert!(plate.intersect(stall).is_empty(), "{}: the label over its box", row.name);
        let dashes = stall_dashes(x0, x1, y - 5.0, y + 5.0, 0.5);
        assert!(dashes.len() > 8 && dashes.iter().all(|d| stall.contains(d.min) && stall.contains(d.max)), "dashes along the box");
        checked += 1;
    }
    assert!(checked >= 4, "the staged rows have stalls");
    // A stall to the end: the label under the box.
    let axes = ChartAxes::new(600.0, 14);
    let at = StallLabel::place(&axes, 500.0, 600.0, 14, 600.0);
    assert!(!at.anchor_end && at.top > axes.y(14.0) + 5.0);
}

/// The climb's time axis ends at the match's length rounded up to a time
/// mark (6:03 to 7:00 by minutes), not at a fixed ten minutes.
#[test]
fn the_climb_axis_ends_at_the_match_length_rounded_up() {
    use chimera_client::client::match_end_signal::{ChartAxes, mmss};
    let check = |len: f32, span: f32, step: f32| {
        let a = ChartAxes::new(len, 14);
        assert_eq!((a.span, a.step), (span, step), "{len} s");
        assert!(a.span / a.step <= ChartAxes::MAX_MARKS);
    };
    check(363.0, 420.0, 60.0);
    check(70.0, 90.0, 30.0);
    check(240.0, 240.0, 30.0);
    check(480.0, 480.0, 60.0);
    check(481.0, 600.0, 120.0);
    check(1501.0, 1800.0, 300.0);
    check(0.0, 30.0, 30.0);

    // The staged match, ended at 6:03.
    let mut sim = sim(true);
    sim.app.world_mut().resource_mut::<MatchEndStaged>().view.length = 363.0;
    step(&mut sim, 0.1);
    step(&mut sim, 0.1);
    let axes = ChartAxes::new(363.0, 14);
    let w = sim.app.world_mut();
    let root = w.query_filtered::<Entity, With<ClimbChart>>().single(w).unwrap();
    let mut texts = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Some(t) = w.get::<Text>(e) {
            texts.push(t.0.clone());
        }
        if let Some(c) = w.get::<Children>(e) {
            stack.extend(c.iter());
        }
    }
    let last = mmss(axes.span);
    assert!(texts.contains(&last), "the axis ends at {last}: {texts:?}");
    assert!(!texts.iter().any(|t| t == "10:00"), "no fixed ten minutes: {texts:?}");
    let marks = texts.iter().filter(|t| t.contains(':') && t.len() <= 5).count();
    assert_eq!(marks as f32, axes.span / axes.step + 1.0, "{texts:?}");
}
