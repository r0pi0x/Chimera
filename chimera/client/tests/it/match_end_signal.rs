//! The match end in the Signal look (`client::match_end_signal`), staged
//! with its made-up players (`stage`): the standings with their marks
//! (your row selected, session wins), the winner band's slab sweeping on
//! the kit's clock, the climb's bars and tiles following the selected row;
//! the winner moment's timeline (`winner_moment`); the shell's theme keeps
//! the old screen. The clock is stepped by hand (`UiClock::manual`).

use bevy::prelude::*;
use chimera_client::client::{
    match_end::{MatchEndPlugin, MatchEndRoot, MatchEndStaged, MatchEndView},
    match_end_signal::{self, BarKind, EndGround, EndSelection, LeaveButton, RungBar, RungBars, SignalEndRoot, StandingsRow, StatTile, VoteCard, VoteCountdown, VoteTicks},
    winner_moment::{MomentTimes, WinnerBand, WinnerMoment, WinnerSlab},
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

fn bars(sim: &mut Sim) -> (usize, Vec<RungBar>) {
    let w = sim.app.world_mut();
    let i = w.query::<&RungBars>().single(w).unwrap().index;
    let mut b: Vec<RungBar> = w.query::<&RungBar>().iter(w).copied().collect();
    b.sort_by_key(|b| b.rung);
    (i, b)
}

#[test]
fn standings_carry_the_marks_and_the_band_sweeps_in() {
    let mut sim = sim(true);
    assert!(visible(&mut sim, true) && !visible(&mut sim, false), "the Signal screen, not the shell's");
    let w = sim.app.world_mut();
    let rows: Vec<StandingsRow> = w.query::<&StandingsRow>().iter(w).copied().collect();
    assert_eq!(rows.len(), 8);
    assert_eq!(rows.iter().filter(|r| r.you).map(|r| r.index).collect::<Vec<_>>(), [2], "your row");
    assert_eq!(sim.app.world().resource::<EndSelection>().pinned, 2, "your row selected");
    let wins: Vec<u32> = {
        let w = sim.app.world_mut();
        w.query::<&chimera_client::client::race_chip::SessionWins>().iter(w).map(|s| s.0).collect()
    };
    assert_eq!(wins.len(), 5, "players with wins show the trophy, and the band its winner's: {wins:?}");
    let w = sim.app.world_mut();
    assert_eq!(w.query::<&WinnerBand>().iter(w).map(|b| b.big).collect::<Vec<_>>(), [false], "one band, in place (no moment)");
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

/// The bars and tiles are the selected row's: yours first, another's on a
/// click; a bar a rung, the fastest and the longest stall marked, the rung
/// reached but not finished outlined, rungs never reached empty.
#[test]
fn the_climb_follows_the_selected_row() {
    let mut sim = sim(true);
    let (i, b) = bars(&mut sim);
    assert_eq!(i, 2, "your climb first");
    assert_eq!(b.len(), 14, "a bar a rung");
    let kind = |k: BarKind| b.iter().filter(|x| x.kind == k).map(|x| x.rung).collect::<Vec<_>>();
    assert_eq!(kind(BarKind::Open), [11], "you reached 11: open");
    assert_eq!(kind(BarKind::Unreached), [12, 13, 14]);
    assert_eq!((kind(BarKind::Fastest).len(), kind(BarKind::Stall).len()), (1, 1));
    sim.app.world_mut().resource_mut::<EndSelection>().pinned = 0;
    sim.ticks(2);
    let (i, b) = bars(&mut sim);
    assert_eq!(i, 0);
    assert!(b.iter().all(|x| !matches!(x.kind, BarKind::Open | BarKind::Unreached)), "the winner finished every rung: {b:?}");
    let w = sim.app.world_mut();
    let labels: Vec<&str> = w.query::<&StatTile>().iter(w).map(|t| t.label).collect();
    assert_eq!(labels.len(), 7, "{labels:?}");
    for want in ["Place", "Rung", "Kills", "K/D", "Streak", "Fastest", "Stall"] {
        assert!(labels.contains(&want), "{want} in {labels:?}");
    }
}

#[test]
fn rung_times_sum_up_each_rungs_stays() {
    use chimera_client::client::{match_end::MatchEndRow, match_end_signal::rung_times};
    use chimera_sim::minigame::climb::Climb;
    // Up at 10 and 30, down to 2 at 40 (a knife death), up at 70; at 3 when it ends at 100.
    let row = MatchEndRow { rung: Some(3), climb: Climb { steps: vec![(10.0, 2), (30.0, 3), (40.0, 2), (70.0, 3)], ..default() }, ..default() };
    let t = rung_times(&row, 100.0, 5);
    assert_eq!(t.secs, vec![Some(10.0), Some(50.0), Some(40.0), None, None]);
    assert_eq!(t.open, Some(2), "rung 3 not finished");
    assert_eq!((t.fastest, t.stall), (Some(0), Some(1)));
    // The winner's last rung ends at the winning kill and counts.
    let win = MatchEndRow { winner: true, rung: Some(2), climb: Climb { steps: vec![(10.0, 2)], kills: vec![10.0, 25.0], ..default() }, ..default() };
    let t = rung_times(&win, 30.0, 2);
    assert_eq!(t.secs, vec![Some(10.0), Some(15.0)]);
    assert_eq!((t.open, t.fastest, t.stall), (None, Some(0), Some(1)));
}

#[test]
fn the_shells_theme_keeps_the_old_screen() {
    let mut sim = sim(false);
    assert!(sim.app.world().resource::<MatchEndView>().shown);
    assert!(!visible(&mut sim, true), "no Signal screen without the theme");
}

/// One countdown (the vote's close and the next match's start) with its
/// tick bar lit as it runs; four cards, yours marked; at the lock the
/// others dim and the countdown reads 0.
#[test]
fn match_end_has_one_countdown() {
    let mut sim = sim(true);
    let count = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        w.query_filtered::<&Text, With<VoteCountdown>>().iter(w).map(|t| t.0.clone()).collect::<Vec<_>>()
    };
    assert_eq!(count(&mut sim), ["0:13"], "one countdown, 13 s left (staged)");
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
    sim.app.world_mut().resource_mut::<MatchEndStaged>().view.vote.as_mut().unwrap().winner = Some(0);
    sim.ticks(3);
    let c = cards(&mut sim);
    assert!(c.iter().all(|c| c.locked) && c[0].won && !c[1].won, "{c:?}");
    assert_eq!(count(&mut sim), ["0:00"]);
    assert_eq!(lit(&mut sim).0, 40);
}

/// The ground (the ambient and the planes over it) under every part, and
/// the way out.
#[test]
fn the_ground_and_leave() {
    let mut sim = sim(true);
    let w = sim.app.world_mut();
    assert_eq!(w.query::<(&EndGround, &Visibility)>().iter(w).filter(|(_, v)| **v != Visibility::Hidden).count(), 2, "the ambient and the planes");
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
    let mut wins: Vec<u32> = w.query_filtered::<&SessionWins, Without<Text>>().iter(w).map(|s| s.0).collect();
    wins.sort();
    assert_eq!(wins, [2, 3], "Ann's (this match counted) and the bot's in the standings; none for Bob");
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

/// The moment's clock: the card holds `moment_in + moment_hold`, waits
/// for the results if they come later, moves over `moment_move`; the
/// scrim fades to opaque from the move; a skip puts everything in place.
#[test]
fn the_winner_moments_timeline() {
    let t = MomentTimes { intro: 0.5, hold: 2.0, moves: 0.8, fade: 0.6, scrim: 0.6, slab_loop: 1.6 };
    let mut m = WinnerMoment { started: Some(10.0), ..default() };
    assert_eq!(m.move_at(&t), None, "no move before the results");
    assert_eq!(m.progress(12.0, &t), 0.0);
    assert!((m.scrim(10.25, &t) - 0.3).abs() < 1e-4, "the scrim comes in with the card");
    m.shown_at = Some(11.0);
    assert_eq!(m.move_at(&t), Some(12.5), "results early: the hold stands");
    assert!((m.progress(12.9, &t) - 0.5).abs() < 1e-4);
    assert!((m.scrim(12.8, &t) - 0.8).abs() < 1e-4);
    assert_eq!(m.scrim(13.2, &t), 1.0);
    assert!(m.settled(13.3, &t));
    m.shown_at = Some(14.0);
    assert_eq!(m.move_at(&t), Some(14.0), "results late: the card waits for them");
    m.skipped = true;
    assert_eq!((m.progress(14.0, &t), m.scrim(14.0, &t)), (1.0, 1.0), "skipped: in place at once");
}

/// Nothing blocks input during the winner moment: Leave, pressed while the
/// card is still moving, leaves.
#[test]
fn leave_works_during_the_winner_moment() {
    #[derive(Resource)]
    struct Left;
    let mut sim = sim(true);
    match_end_signal::unstage(sim.app.world_mut());
    sim.ticks(2);
    let w = sim.app.world_mut();
    w.resource_mut::<chimera_core::console::Console>().add_command(chimera_core::console::Command {
        name: "disconnect".into(),
        help: String::new(),
        run: std::sync::Arc::new(|w: &mut World, _: &[String]| {
            w.insert_resource(Left);
            Ok(None)
        }),
        complete: None,
    });
    match_end_signal::stage_with(w, None, true).unwrap();
    sim.ticks(2);
    step(&mut sim, 2.85);
    let w = sim.app.world_mut();
    assert!(w.query::<&WinnerBand>().single(w).unwrap().big, "still the card, moving");
    let leave = w.query_filtered::<Entity, With<LeaveButton>>().single(w).expect("Leave is there during the move");
    w.entity_mut(leave).insert(chimera_client::client::widgets::Interaction::Pressed);
    sim.ticks(2);
    assert!(sim.app.world().contains_resource::<Left>(), "Leave during the moment");
}
