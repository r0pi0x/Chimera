//! The match end's blocks never cover each other (plan ugg-ui, M4's
//! layout pass): the footer's Leave and "Next match" countdown, and the
//! chat's fixed box (its right column, `[layout.matchend]`), laid out by
//! Bevy's own UI with the Ultimate Gun Game's theme and faces at 1280x720,
//! 1920x1080 and an ultrawide 3440x1440, with no chat lines, a few, and a
//! full chat (two dozen long lines, the box open). The chat box is the
//! same size in each (it never grows over anything), its newest line sits
//! inside it, and it clears the standings and the footer.

use bevy::{
    camera::{ComputedCameraValues, RenderTargetInfo, Viewport},
    prelude::*,
    ui::UiGlobalTransform,
    window::PrimaryWindow,
};
use chimera_client::client::{
    match_end::MatchEndPlugin,
    match_end_signal::{ClimbChart, LeaveButton, StandingsRow, VoteCard, VoteCountdownColumn},
    signal_chat::{ChatDockRoot, FeedLineId, SignalChatPlugin},
    ui_scene::UiScenePlugin,
};
use chimera_core::console::Console;
use chimera_sim::SimBasePlugins;
use chimera_ui::{ChimeraUiPlugin, KitFonts, Theme, UiClock};
use chimera_world::harness::Sim;
use ultimate_gun_game::signal;

const SIZES: [(u32, u32); 3] = [(1280, 720), (1920, 1080), (3440, 1440)];

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
        app.insert_resource(Theme::from_toml(signal::THEME).unwrap());
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

fn one<C: Component>(sim: &mut Sim) -> Rect {
    let w = sim.app.world_mut();
    let e = w.query_filtered::<Entity, With<C>>().single(w).unwrap_or_else(|_| panic!("one {}", std::any::type_name::<C>()));
    rect(w, e)
}

#[test]
fn countdown_leave_and_chat_never_overlap() {
    let mut sim = sim();
    for size in SIZES {
        resize(&mut sim, size);
        let window = Rect::new(0.0, 0.0, size.0 as f32, size.1 as f32);
        let mut boxes: Vec<Rect> = Vec::new();
        for chat in ["", "chat", "busy"] {
            con(&mut sim, &format!("ui_scene matchend {chat}"));
            settle(&mut sim);
            let at = format!("{size:?} with chat {chat:?}");
            let countdown = one::<VoteCountdownColumn>(&mut sim);
            let leave = one::<LeaveButton>(&mut sim);
            let graph = one::<ClimbChart>(&mut sim);
            assert!(inside(countdown, window) && inside(leave, window) && inside(graph, window), "{at}: off the window");
            assert!(!overlap(countdown, leave), "{at}: the countdown {countdown:?} covers Leave {leave:?}");
            // Leave bottom left, the countdown bottom right.
            assert!(leave.center().x < countdown.center().x, "{at}: Leave is left of the countdown");
            let w = sim.app.world_mut();
            let cards: Vec<Rect> = w.query_filtered::<Entity, With<VoteCard>>().iter(w).collect::<Vec<_>>().into_iter().map(|e| rect(w, e)).collect();
            for c in &cards {
                assert!(!overlap(*c, countdown) && !overlap(*c, leave), "{at}: a map card {c:?} covers the footer");
            }
            let rows: Vec<Rect> = w.query_filtered::<Entity, With<StandingsRow>>().iter(w).collect::<Vec<_>>().into_iter().map(|e| rect(w, e)).collect();
            assert_eq!(rows.len(), 8);
            let last_row = rows.iter().map(|r| r.max.y).fold(0.0, f32::max);
            assert!(last_row <= leave.min.y.min(countdown.min.y) + 1.0, "{at}: the standings reach the footer");
            let dock = {
                let w = sim.app.world_mut();
                w.query_filtered::<Entity, With<ChatDockRoot>>().iter(w).next().map(|e| rect(w, e))
            };
            let dock = dock.unwrap_or_else(|| panic!("{at}: no chat box"));
            boxes.push(dock);
            assert!(inside(dock, window), "{at}: the chat box {dock:?} is off the window");
            for (what, r) in [("the countdown", countdown), ("Leave", leave), ("the climb", graph)] {
                assert!(!overlap(dock, r), "{at}: the chat box {dock:?} covers {what} {r:?}");
            }
            for r in &cards {
                assert!(!overlap(dock, *r), "{at}: the chat box covers a map card");
            }
            let w = sim.app.world_mut();
            for (i, r) in rows.iter().enumerate() {
                assert!(!overlap(dock, *r), "{at}: the chat box covers standings row {i}: {r:?}");
            }
            // The newest line is inside the box; no line sticks out below it.
            let lines: Vec<(u64, Rect)> = w.query::<(Entity, &FeedLineId)>().iter(w).map(|(e, l)| (l.id, rect(w, e))).collect();
            if chat.is_empty() {
                assert!(lines.is_empty(), "{at}: lines without chat");
                continue;
            }
            let (_, newest) = lines.iter().max_by_key(|(id, _)| *id).expect("lines");
            assert!(inside(*newest, dock), "{at}: the newest line {newest:?} left the box {dock:?}");
            assert!(lines.iter().all(|(_, r)| r.max.y <= dock.max.y + 1.0), "{at}: a line below the box");
            // Whole messages only: every shown line is inside the box.
            let w = sim.app.world_mut();
            let shown: Vec<Rect> = w.query::<(Entity, &FeedLineId, &Visibility)>().iter(w).filter(|(.., v)| **v != Visibility::Hidden).map(|(e, ..)| rect(w, e)).collect();
            assert!(!shown.is_empty() && shown.iter().all(|r| inside(*r, dock)), "{at}: a shown line is cut by the box: {shown:?} in {dock:?}");
        }
        // The box is one size, empty or full.
        assert_eq!(boxes.len(), 3);
        for b in &boxes {
            let (a, b) = (boxes[0].size(), b.size());
            assert!((a - b).abs().max_element() < 1.0, "{size:?}: the chat box grew: {a:?} -> {b:?}");
        }
        con(&mut sim, "ui_scene off");
        settle(&mut sim);
    }
}
