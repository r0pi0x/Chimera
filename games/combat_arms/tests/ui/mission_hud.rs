//! The mission display (ours, not from the original: world_objects.md Open
//! question 15; `ui::hud::mission`), headless: the boxes at two
//! resolutions scale like the rest of the HUD (and the timer's text
//! stretches with the horizontal scale), the timer's text over time, a
//! caption and message expiring, the objective list's contents by team,
//! and the whole path from the notices to `CaMissionHud` in a running sim.

use bevy::prelude::*;
use chimera_core::core::{LocalPlayer, Team};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::objects::{
        events::{Notice, Objective, ObjectiveState},
        mission::CaMission,
    },
    harness::CaSim,
    ui::{
        hud::{
            CaHudStatePlugin,
            mission::{CaMissionHud, MissionHudData, Piece, boxes, clock_text, view, wrap},
            model::Screen,
            score::places,
        },
        layout::{self, Effect, FontList, TextLook, text::EFFECTS},
    },
};

fn id(s: &str) -> String {
    s.to_string()
}

fn objective(name: &str, team: u8, state: ObjectiveState, count: Option<(i32, i32)>) -> Objective {
    Objective {
        id: id(name),
        team,
        state,
        count,
    }
}

/// A mission with a timer (60 s from 100), a caption (4 s from 100), a
/// message (6 s from 100) and three objectives.
fn mission() -> CaMission {
    let mut m = CaMission::default();
    m.apply(1, "VipExitTimer", &Notice::Timer { on: true, seconds: 60.0 }, 100.0);
    m.apply(2, "Cap", &Notice::Caption { id: id("Cap404"), showtime: 4.0, team: 0 }, 100.0);
    m.apply(3, "Msg", &Notice::Message { id: id("Msg1"), showtime: 6.0, team: 1 }, 100.0);
    m.apply(4, "Mis", &Notice::Objective(objective("Reach the exit", 0, ObjectiveState::InProgress, None)), 100.0);
    m.apply(4, "Mis", &Notice::Objective(objective("Kills", 0, ObjectiveState::Normal, Some((2, 10)))), 100.0);
    m.apply(4, "Mis", &Notice::Objective(objective("Defend", 2, ObjectiveState::Success, None)), 100.0);
    m
}

fn shown(m: &CaMission, now: f64, team: Option<u8>) -> CaMissionHud {
    view(m, &MissionHudData::ours(), now, team, &|s| s.to_string())
}

#[test]
fn the_data_parses_and_names_known_effects() {
    let d = MissionHudData::ours();
    for e in [&d.timer.effect, &d.caption.effect, &d.message.effect, &d.objectives.effect] {
        assert!(EFFECTS.iter().any(|(n, _)| n.eq_ignore_ascii_case(e)), "unknown effect {e}");
    }
    let fonts = FontList::ours();
    for f in [&d.timer.font, &d.caption.font, &d.message.font, &d.objectives.font] {
        assert!(fonts.get(f).is_some(), "the font list has no {f}");
    }
}

#[test]
fn the_timer_counts_down_in_minutes_and_seconds() {
    let m = mission();
    let t = |now: f64| shown(&m, now, None).timers;
    assert_eq!(clock_text(60.0), "01:00");
    assert_eq!(clock_text(0.2), "00:01", "whole seconds, rounded up");
    assert_eq!(clock_text(-3.0), "00:00");
    assert_eq!(t(100.0)[0].text, "01:00");
    assert_eq!(t(130.5)[0].text, "00:30");
    assert!(!t(130.0)[0].warn);
    assert_eq!(t(151.0)[0].text, "00:09");
    assert!(t(151.0)[0].warn, "the last ten seconds in the warning colour");
    assert_eq!(t(160.0)[0].text, "00:00");
    // Two timers: named, soonest first.
    let mut two = m.clone();
    two.apply(9, "Bomb", &Notice::Timer { on: true, seconds: 20.0 }, 100.0);
    let lines: Vec<String> = shown(&two, 100.0, None).timers.into_iter().map(|l| l.text).collect();
    assert_eq!(lines, ["Bomb 00:20", "VipExitTimer 01:00"]);
}

#[test]
fn captions_and_messages_expire_and_follow_their_team() {
    let m = mission();
    let at = |now: f64, team: Option<u8>| {
        let h = shown(&m, now, team);
        (h.caption, h.message)
    };
    assert_eq!(at(100.0, Some(1)), (vec![id("Cap404")], vec![id("Msg1")]));
    assert_eq!(at(103.9, Some(1)).0, vec![id("Cap404")]);
    assert_eq!(at(104.0, Some(1)).0, Vec::<String>::new(), "the caption's 4 s are over");
    assert_eq!(at(104.0, Some(1)).1, vec![id("Msg1")], "the message has 2 s left");
    assert!(at(106.0, Some(1)).1.is_empty());
    // The message is team 1's.
    assert!(at(101.0, Some(2)).1.is_empty(), "not team 2's");
    assert!(!at(101.0, Some(2)).0.is_empty(), "the caption is everyone's");
    assert!(!at(101.0, None).1.is_empty(), "no team: shown");
    // Long texts wrap.
    assert_eq!(wrap("one two three four", 9), ["one two", "three", "four"]);
}

#[test]
fn the_objective_list_shows_marks_counts_and_the_teams_own() {
    let m = mission();
    let lines = |team| -> Vec<String> { shown(&m, 100.0, team).objectives.into_iter().map(|o| o.text).collect() };
    assert_eq!(lines(Some(1)), ["- Reach the exit", "- Kills (2/10)"]);
    assert_eq!(lines(Some(2)), ["- Reach the exit", "- Kills (2/10)", "+ Defend"]);
    // A change shows in place; a removal drops the line.
    let mut m2 = m.clone();
    m2.apply(4, "Mis", &Notice::Objective(objective("Kills", 0, ObjectiveState::Success, Some((10, 10)))), 101.0);
    m2.apply(4, "Mis", &Notice::ObjectiveRemoved { id: id("Reach the exit") }, 102.0);
    let l: Vec<String> = shown(&m2, 102.0, Some(2)).objectives.into_iter().map(|o| o.text).collect();
    assert_eq!(l, ["+ Kills (10/10)", "+ Defend"]);
    // No objectives, no list (and no title box).
    let none = shown(&CaMission::default(), 0.0, None);
    assert_eq!(none, CaMissionHud::default());
    assert!(boxes(&none, &MissionHudData::ours(), 0).is_empty());
}

/// The boxes are the data's reference boxes, scaled by (x_r, y_r) at any
/// resolution; the timer goes under the score panel's objective lines; the
/// timer's text stretches with the horizontal scale (T2).
#[test]
fn the_boxes_scale_like_the_hud_at_two_resolutions() {
    let data = MissionHudData::ours();
    let m = mission();
    let hud = shown(&m, 100.0, Some(1));
    let all = boxes(&hud, &data, 0);
    let pieces: Vec<Piece> = all.iter().map(|b| b.piece).collect();
    assert_eq!(
        pieces,
        [
            Piece::Timer { warn: false },
            Piece::Caption,
            Piece::Message,
            Piece::ObjectivesTitle,
            Piece::Objective(ObjectiveState::InProgress),
            Piece::Objective(ObjectiveState::Normal),
        ]
    );
    let timer = &all[0];
    assert_eq!((timer.at, timer.size), (Vec2::from(data.timer.at), Vec2::from(data.timer.size)));
    // Under the score panel's lines: 4 lines of 16 from 84.
    let low = boxes(&hud, &data, 4);
    assert_eq!(low[0].at.y, places::OBJECTIVES.0.y + 16.0 * 4.0 + 2.0);
    assert_eq!(low[0].at.x, timer.at.x);
    // The objective rows go down by one row each from the data's place.
    let rows: Vec<f32> = all.iter().filter(|b| matches!(b.piece, Piece::ObjectivesTitle | Piece::Objective(_))).map(|b| b.at.y).collect();
    assert_eq!(rows, [10.0, 26.0, 42.0]);

    let fonts = FontList::ours();
    for (w, h) in [(1024.0_f32, 768.0_f32), (1920.0, 1080.0)] {
        let screen = Screen::new(w, h);
        let (xr, yr) = (w / 1024.0, h / 768.0);
        for b in &all {
            let r = screen.rect(b.at, b.size);
            assert!((r.min.x - b.at.x * xr).abs() < 1e-3 && (r.min.y - b.at.y * yr).abs() < 1e-3, "{b:?} at {w}x{h}");
            assert!(((r.max.x - r.min.x) - b.size.x * xr).abs() < 1e-3 && ((r.max.y - r.min.y) - b.size.y * yr).abs() < 1e-3);
        }
        // The timer's text: stretched as the HUD's text is, inside its box.
        let look = TextLook {
            font: timer.font.clone(),
            h: timer.align,
            v: chimera_client::client::canvas::VAlign::Centre,
            effect: Effect::Shadow,
            colour: timer.colour,
            set: Some(timer.text.clone()),
            ..default()
        };
        let scale = chimera_client::client::canvas::Scale::of(Vec2::new(w, h), layout::REFERENCE);
        let draw = layout::boxed_text(timer.at.as_ivec2(), timer.size.as_ivec2(), IVec2::ZERO, look, scale, &fonts).expect("text");
        if (w / h - 4.0 / 3.0).abs() < 1e-3 {
            assert!((draw.stretch - 1.0).abs() < 1e-6, "4:3 unstretched");
        } else {
            assert!(draw.stretch > 1.0, "16:9 stretches the glyphs wider: {}", draw.stretch);
        }
        let r = screen.rect(timer.at, timer.size);
        assert!(draw.rect.x0 as f32 >= r.min.x - 1.0 && draw.rect.x1 as f32 <= r.max.x + 1.0, "inside its box at {w}x{h}");
        // Centred: the text's middle is the box's.
        let mid = (draw.rect.x0 + draw.rect.x1) as f32 / 2.0;
        assert!((mid - (r.min.x + r.max.x) / 2.0).abs() <= 2.0, "centred at {w}x{h}: {mid}");
    }
}

/// From the state to the display in a running sim, then time passing.
#[test]
fn the_display_follows_the_state_and_the_clock() {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaHudStatePlugin));
    });
    let me = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), combat_arms::game::movement::ID);
    sim.app.world_mut().entity_mut(me).insert((Team(1), LocalPlayer));
    sim.ticks(2);
    assert_eq!(*sim.app.world().resource::<CaMissionHud>(), CaMissionHud::default(), "nothing before a notice");
    let now = sim.app.world().resource::<chimera_core::core::SimClock>().now;
    {
        let mut m = sim.app.world_mut().resource_mut::<CaMission>();
        m.apply(1, "Clock", &Notice::Timer { on: true, seconds: 10.0 }, now);
        m.apply(2, "Cap", &Notice::Caption { id: id("Cap404"), showtime: 2.0, team: 0 }, now);
        m.apply(3, "Mis", &Notice::Objective(objective("Find", 0, ObjectiveState::Normal, None)), now);
    }
    sim.ticks(2);
    let hud = sim.app.world().resource::<CaMissionHud>().clone();
    assert_eq!(hud.timers.len(), 1);
    assert_eq!(hud.timers[0].text, "00:10");
    assert_eq!(hud.caption, [id("Cap404")]);
    assert_eq!(hud.objectives[0].text, "- Find");
    sim.seconds(3.0);
    let hud = sim.app.world().resource::<CaMissionHud>().clone();
    assert!(hud.caption.is_empty(), "the caption ran out");
    assert_eq!(hud.timers[0].text, "00:07");
    assert_eq!(hud.objectives.len(), 1, "the list stays");
}
