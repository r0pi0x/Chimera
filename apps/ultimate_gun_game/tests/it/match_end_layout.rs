//! The match end's blocks (plan ugg-ui, M4's layout pass), laid out by
//! Bevy's own UI with the Ultimate Gun Game's theme and faces at 1280x720
//! and 1920x1080, with 2 and 16 players (and no entry for you), and no
//! chat, a few lines, and a busy chat:
//!
//! - Leave top left; the chat down the left column from under it to the
//!   bottom; the standings down the right (top-aligned, the winner their
//!   first row and a taller one); the details line top centre; the climb
//!   with the stat tiles directly under it; the vote cards four to a row under
//!   the tiles in the centre and the countdown bottom right; the climb small and centred;
//! - none of them overlaps another, all inside the window, every
//!   standings row inside its column, the chat box one size whatever it
//!   holds and its newest line inside it;
//! - the tiles show your stats, or the winner's when you have no entry.

use bevy::{
    camera::{ComputedCameraValues, RenderTargetInfo, Viewport},
    prelude::*,
    ui::UiGlobalTransform,
    window::PrimaryWindow,
};
use chimera_client::client::{
    match_end::MatchEndPlugin,
    match_end_signal::{ClimbChart, DetailsLine, LeaveButton, StandingsPanel, StandingsRow, StatTile, StatsRow, VoteCard, VoteCountdownColumn, WinnerRow},
    signal_chat::{ChatDockRoot, FeedLineId, SignalChatPlugin},
    ui_scene::UiScenePlugin,
};
use chimera_client::client::{match_end::MatchEndStaged, match_end_signal};
use chimera_core::console::Console;
use chimera_sim::SimBasePlugins;
use chimera_ui::{ChimeraUiPlugin, KitFonts, Theme, UiClock};
use chimera_world::harness::Sim;
use ultimate_gun_game::signal;

const SIZES: [(u32, u32); 2] = [(1280, 720), (1920, 1080)];

fn sim() -> Sim {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy::picking::hover::HoverMap>()
            .init_resource::<bevy::picking::pointer::PointerMap>()
            .init_resource::<bevy::picking::events::PointerState>()
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::picking::backend::PointerHits>()
            .init_asset::<Font>();
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        app.add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin));
        app.insert_resource(Theme::from_toml(&signal::THEME).unwrap());
        let kit = KitFonts::load(&mut app.world_mut().resource_mut::<Assets<Font>>(), signal::FONTS);
        app.insert_resource(kit);
        app.add_plugins((ChimeraUiPlugin, UiScenePlugin, MatchEndPlugin, SignalChatPlugin));
        app.insert_resource(UiClock::manual());
    });
    let w = sim.app.world_mut();
    w.spawn((Window::default(), PrimaryWindow));
    w.spawn((Camera2d, Camera::default()));
    sim
}

fn resize(sim: &mut Sim, (x, y): (u32, u32)) {
    let w = sim.app.world_mut();
    for mut win in w.query::<&mut Window>().iter_mut(w) {
        win.resolution.set(x as f32, y as f32);
    }
    for mut cam in w.query::<&mut Camera>().iter_mut(w) {
        cam.computed = ComputedCameraValues { target_info: Some(RenderTargetInfo { physical_size: UVec2::new(x, y), scale_factor: 1.0 }), ..default() };
        cam.viewport = Some(Viewport { physical_size: UVec2::new(x, y), ..default() });
    }
}

fn con(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
}

fn settle(sim: &mut Sim) {
    for _ in 0..12 {
        sim.app.world_mut().resource_mut::<UiClock>().step(0.5);
        sim.app.update();
    }
}

fn rect(w: &World, e: Entity) -> Rect {
    let (t, n) = (w.get::<UiGlobalTransform>(e).expect("laid out"), w.get::<ComputedNode>(e).expect("laid out"));
    Rect::from_center_size(t.translation, n.size())
}

fn overlap(a: Rect, b: Rect) -> bool {
    a.min.x < b.max.x - 0.5 && b.min.x < a.max.x - 0.5 && a.min.y < b.max.y - 0.5 && b.min.y < a.max.y - 0.5
}

fn inside(a: Rect, b: Rect) -> bool {
    a.min.x >= b.min.x - 1.0 && a.max.x <= b.max.x + 1.0 && a.min.y >= b.min.y - 1.0 && a.max.y <= b.max.y + 1.0
}

fn all<C: Component>(sim: &mut Sim) -> Vec<Rect> {
    let w = sim.app.world_mut();
    let es: Vec<Entity> = w.query_filtered::<Entity, With<C>>().iter(w).collect();
    es.into_iter().map(|e| rect(w, e)).collect()
}

fn one<C: Component>(sim: &mut Sim) -> Rect {
    let v = all::<C>(sim);
    assert_eq!(v.len(), 1, "one {}", std::any::type_name::<C>());
    v[0]
}

/// The staged match with `n` players (the winner first; you third, or
/// nobody of you with `spectate`).
fn players(sim: &mut Sim, n: usize, spectate: bool) {
    let mut f = match_end_signal::staged_fixture();
    let base = f.rows.clone();
    let mut rows: Vec<_> = (0..n).map(|i| base[i % base.len()].clone()).collect();
    for (i, r) in rows.iter_mut().enumerate() {
        if i >= base.len() {
            r.name = format!("{}_{i}", r.name);
            r.winner = false;
            r.you = false;
            r.host = false;
        }
    }
    if n == 2 {
        rows[1] = base[2].clone();
    }
    if spectate {
        rows.iter_mut().for_each(|r| {
            r.you = false;
            r.host = false;
        });
    }
    f.rows = rows;
    sim.app.world_mut().insert_resource(MatchEndStaged { view: f.view(), left: f.left });
}

fn texts_under(sim: &mut Sim, root: Entity) -> Vec<String> {
    let w = sim.app.world_mut();
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Some(t) = w.get::<Text>(e) {
            out.push(t.0.clone());
        }
        if let Some(c) = w.get::<Children>(e) {
            stack.extend(c.iter());
        }
    }
    out
}

#[test]
fn the_blocks_sit_where_the_layout_puts_them_and_never_overlap() {
    let mut sim = sim();
    for size in SIZES {
        resize(&mut sim, size);
        let window = Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32);
        let (w, h) = (size.0 as f32, size.1 as f32);
        for (n, spectate) in [(2usize, false), (16, false), (8, true)] {
            let mut boxes: Vec<Rect> = Vec::new();
            for chat in ["", "chat", "busy"] {
                con(&mut sim, &format!("ui_scene matchend {chat}"));
                settle(&mut sim);
                players(&mut sim, n, spectate);
                settle(&mut sim);
                let at = format!("{size:?}, {n} players{}, chat {chat:?}", if spectate { " (spectating)" } else { "" });
                let leave = one::<LeaveButton>(&mut sim);
                let details = one::<DetailsLine>(&mut sim);
                let graph = one::<ClimbChart>(&mut sim);
                let stats = one::<StatsRow>(&mut sim);
                let panel = one::<StandingsPanel>(&mut sim);
                let winner = one::<WinnerRow>(&mut sim);
                let countdown = one::<VoteCountdownColumn>(&mut sim);
                let cards = all::<VoteCard>(&mut sim);
                let rows = all::<StandingsRow>(&mut sim);
                let tiles = all::<StatTile>(&mut sim);
                let dock = {
                    let w = sim.app.world_mut();
                    w.query_filtered::<Entity, With<ChatDockRoot>>().iter(w).next().map(|e| rect(w, e))
                }
                .unwrap_or_else(|| panic!("{at}: no chat box"));
                assert_eq!(cards.len(), 4, "{at}");
                assert_eq!(rows.len(), n, "{at}");
                assert_eq!(tiles.len(), 6, "{at}");

                // Where each sits.
                assert!(leave.min.x < w * 0.1 && leave.min.y < h * 0.1, "{at}: Leave is top left: {leave:?}");
                assert!(leave.width() < w * 0.12 && leave.height() < h * 0.06, "{at}: Leave is small: {leave:?}");
                assert!(dock.min.x < w * 0.1 && dock.min.y >= leave.max.y, "{at}: the chat is the left column under Leave: {dock:?}");
                assert!(dock.max.y > h * 0.9, "{at}: the chat runs to the bottom: {dock:?}");
                assert!(panel.min.y < h * 0.07 && panel.min.x > w * 0.6 && (panel.width() / w - 0.3).abs() < 0.03, "{at}: standings top right, 30 %: {panel:?}");
                assert!(details.center().y < h * 0.1 && details.max.y <= graph.min.y, "{at}: the details line is above the climb: {details:?}");
                assert!((details.center().x - graph.center().x).abs() < w * 0.02, "{at}: the details line is centred over the climb");
                assert!(graph.width() > w * 0.25 && graph.width() < w * 0.3 && graph.height() > h * 0.15 && graph.height() < h * 0.4, "{at}: a small, readable climb: {graph:?}");
                assert!(stats.min.y >= graph.max.y - 1.0 && stats.min.y < graph.max.y + h * 0.08, "{at}: the tiles directly under the climb: {stats:?} {graph:?}");
                assert!(stats.min.x <= graph.min.x + 1.0 && stats.max.x >= graph.max.x - 1.0 && (stats.center().x - graph.center().x).abs() < 2.0, "{at}: the climb is centred over the tiles");
                assert!(cards.iter().all(|c| c.min.y >= stats.max.y && c.center().x > dock.max.x && c.center().x < panel.min.x), "{at}: cards in the centre under the tiles: {cards:?}");
                let row_y = cards[0].min.y;
                assert!(cards.iter().all(|c| (c.min.y - row_y).abs() < 1.0), "{at}: four cards in one row: {cards:?}");
                let (lo, hi) = cards.iter().fold((f32::MAX, f32::MIN), |(a, b), c| (a.min(c.min.x), b.max(c.max.x)));
                assert!(lo < stats.min.x + 2.0 && hi > stats.max.x - w * 0.02, "{at}: the cards span the middle column");
                assert!(countdown.center().x > w * 0.7 && countdown.max.y > h * 0.9, "{at}: the countdown bottom right: {countdown:?}");

                // The winner is the first row, taller than the rest, in the accent frame's place.
                let first = rows.iter().map(|r| r.min.y).fold(f32::MAX, f32::min);
                assert!((winner.min.y - first).abs() < 1.0, "{at}: the winner is the first row");
                for r in rows.iter().filter(|r| (r.min.y - winner.min.y).abs() > 1.0) {
                    assert!(winner.height() > r.height() * 1.5, "{at}: the winner's row {winner:?} is taller than {r:?}");
                    assert!(r.min.y >= winner.max.y - 1.0, "{at}: a row above the winner");
                }
                for (i, r) in rows.iter().enumerate() {
                    assert!(inside(*r, panel), "{at}: standings row {i} {r:?} is outside its column {panel:?}");
                }
                                assert!(panel.max.y <= countdown.min.y + 1.0, "{at}: the standings reach the countdown");

                // Nothing covers anything, nothing leaves the window.
                let mut blocks = vec![("Leave", leave), ("the chat", dock), ("the details", details), ("the climb", graph), ("the tiles", stats), ("the standings", panel), ("the countdown", countdown)];
                blocks.extend(cards.iter().map(|c| ("a card", *c)));
                for (i, (an, a)) in blocks.iter().enumerate() {
                    assert!(inside(*a, window), "{at}: {an} {a:?} is off the window");
                    for (bn, b) in &blocks[i + 1..] {
                        assert!(!overlap(*a, *b), "{at}: {an} {a:?} covers {bn} {b:?}");
                    }
                }
                for t in &tiles {
                    assert!(inside(*t, stats), "{at}: a tile {t:?} leaves its row {stats:?}");
                }

                // The tiles: yours, else the winner's.
                let stats_e = {
                    let w = sim.app.world_mut();
                    w.query_filtered::<Entity, With<StatsRow>>().single(w).unwrap()
                };
                let texts = texts_under(&mut sim, stats_e);
                if spectate {
                    assert!(texts.iter().any(|t| t == "NADIA.EXE'S STATS"), "{at}: the winner's stats: {texts:?}");
                } else {
                    assert!(texts.iter().any(|t| t == "YOUR STATS"), "{at}: your stats: {texts:?}");
                }

                // The chat box: one size, the newest line inside it, whole lines only.
                boxes.push(dock);
                let w2 = sim.app.world_mut();
                let lines: Vec<(u64, Rect)> = w2.query::<(Entity, &FeedLineId)>().iter(w2).map(|(e, l)| (l.id, rect(w2, e))).collect();
                if chat.is_empty() {
                    assert!(lines.is_empty(), "{at}: lines without chat");
                    continue;
                }
                let (_, newest) = lines.iter().max_by_key(|(id, _)| *id).expect("lines");
                assert!(inside(*newest, dock), "{at}: the newest line {newest:?} left the box {dock:?}");
                assert!(lines.iter().all(|(_, r)| r.max.y <= dock.max.y + 1.0), "{at}: a line below the box");
                let shown: Vec<Rect> = w2.query::<(Entity, &FeedLineId, &Visibility)>().iter(w2).filter(|(.., v)| **v != Visibility::Hidden).map(|(e, ..)| rect(w2, e)).collect();
                assert!(!shown.is_empty() && shown.iter().all(|r| inside(*r, dock)), "{at}: a shown line is cut by the box: {shown:?} in {dock:?}");
            }
            assert_eq!(boxes.len(), 3);
            for b in &boxes {
                let (a, b) = (boxes[0].size(), b.size());
                assert!((a - b).abs().max_element() < 1.0, "{size:?}, {n} players: the chat box grew: {a:?} -> {b:?}");
            }
            con(&mut sim, "ui_scene off");
            settle(&mut sim);
        }
        con(&mut sim, "ui_scene off");
        settle(&mut sim);
    }
}

/// Voting, a lead, the lock and a winner change only colours, the outline
/// and the badge: the cards keep their size and place, and stay clear of
/// the standings and the countdown.
#[test]
fn voting_never_moves_or_resizes_the_map_cards() {
    let mut sim = sim();
    for size in SIZES {
        resize(&mut sim, size);
        con(&mut sim, "ui_scene matchend");
        settle(&mut sim);
        let f = match_end_signal::staged_fixture();
        let base = all::<VoteCard>(&mut sim);
        assert_eq!(base.len(), 4, "{size:?}");
        let states: [(Option<usize>, Vec<u32>, Option<usize>); 4] =
            [(Some(1), vec![0, 1, 0, 0], None), (Some(3), vec![2, 1, 0, 5], None), (Some(0), vec![8, 8, 8, 8], None), (Some(2), vec![1, 0, 4, 2], Some(2))];
        for (mine, counts, winner) in states {
            let mut view = f.view();
            let v = view.vote.as_mut().expect("a vote");
            v.mine = mine;
            v.counts = counts.clone();
            v.winner = winner;
            sim.app.world_mut().insert_resource(MatchEndStaged { view, left: f.left });
            settle(&mut sim);
            let now = all::<VoteCard>(&mut sim);
            assert_eq!(now.len(), 4, "{size:?} {counts:?}");
            let (panel, countdown) = (one::<StandingsPanel>(&mut sim), one::<VoteCountdownColumn>(&mut sim));
            for (a, b) in base.iter().zip(&now) {
                assert!((a.min - b.min).abs().max_element() < 0.5 && (a.size() - b.size()).abs().max_element() < 0.5, "{size:?}: a card moved or resized: {a:?} -> {b:?} ({mine:?}, {counts:?}, {winner:?})");
                assert!(!overlap(*b, panel) && !overlap(*b, countdown), "{size:?}: a card covers the standings or countdown: {b:?}");
            }
        }
        con(&mut sim, "ui_scene off");
        settle(&mut sim);
    }
}
