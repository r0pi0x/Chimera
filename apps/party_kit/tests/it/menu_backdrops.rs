//! The Signal menu's backdrops (`party_kit::menu_backdrops`, M5 of
//! docs/plans/active/ugg-ui.md): each paints through its whole loop at
//! 720 and 1080 lines (finite, inside its frame's reach, never empty
//! once on); the Ladder's beat follows the look book's keyframes (a 4 %
//! hoist at .55 s, the landing at .95 s, the knife's 5 %) and lights the
//! meter on landing; the game's own kill icon replaces the class icon
//! when at hand; the menu's frame stays in its budget (instructions a
//! frame, the backdrops' painting counted headless).

use bevy::prelude::*;
use chimera_ui::{Icons, UiClock};
use party_kit::menu_backdrops::{
    self, LADDER_PERIOD,
    paint::Paint,
    scenes::{self, Pal, Shapes},
};

use super::signal_menu::{MENU_THEME, sim_with, step};

fn pal() -> Pal {
    Pal {
        fg: LinearRgba::WHITE,
        acc: LinearRgba::BLUE,
    }
}

#[test]
fn each_backdrop_paints_through_its_loop() {
    let shapes = Shapes::new(&Icons::shipped());
    assert_eq!(shapes.facets.len(), 6, "the mark's six facets");
    let rungs = menu_backdrops::rungs(None, true);
    assert!(rungs.iter().all(|r| r.game_icon.is_none()));
    let mut p = Paint::new();
    for (w, h) in [(1280.0, 720.0), (1920.0, 1080.0)] {
        for (which, period) in [(0, LADDER_PERIOD), (1, 8.0 * 3.4), (2, 16.0)] {
            let mut t = 0.0;
            while t < period {
                p.clear();
                menu_backdrops::paint(&mut p, &shapes, pal(), &rungs, which, w, h, t, false);
                assert!(p.pos.iter().flatten().all(|v| v.is_finite()), "backdrop {which} at {t}");
                assert_eq!(p.pos.len(), p.col.len());
                assert!(p.idx.iter().all(|&i| (i as usize) < p.pos.len()));
                // On screen (a little past for strokes) from the moment
                // it is on.
                assert!(
                    p.pos
                        .iter()
                        .all(|q| q[0] > -w && q[0] < 2.0 * w && q[1] > -h && q[1] < 2.0 * h)
                );
                if t > 0.5 && t < period - 0.6 {
                    assert!(p.triangles() > 0, "backdrop {which} empty at {t}");
                }
                t += 0.25;
            }
        }
    }
}

#[test]
fn the_ladder_beat_follows_the_keyframes() {
    use scenes::{BEAT, LAND, ladder_state};
    // Beat 1 carries plate 0 to plate 1.
    let at = |tau: f32| ladder_state(BEAT + tau);
    assert!((at(0.55).b - 1.04).abs() < 1e-3, "the hoist, 4 % past: {}", at(0.55).b);
    assert!((at(LAND).b - 1.0).abs() < 1e-3, "the landing");
    assert!((at(0.65).c - 1.02).abs() < 1e-3, "the camera behind and lower");
    assert!((at(1.5).b - 1.0).abs() < 1e-3, "the rest");
    // The knife's beat: 5 %, then it arrives.
    let k = ladder_state(13.0 * BEAT + 0.58);
    assert!(k.fin && (k.b - 13.05).abs() < 1e-3, "{k:?}");
    assert!(ladder_state(13.0 * BEAT + 1.2).arr > 0.0);
    // The meter's tick lights on the landing: before it the last rung.
    assert!(at(0.5).land < 0.0 && at(1.0).land >= 0.0);
}

#[test]
fn a_games_own_icon_replaces_the_class_icon() {
    use chimera_client::client::weapon_icons::{WeaponIcon, WeaponIcons};
    let mut icons = WeaponIcons::default();
    icons.register("cs_source", |id| {
        (id == "cs_source:weapon_glock").then(|| WeaponIcon::Glyph {
            font: Handle::default(),
            ch: 'c',
            em: 20.0,
        })
    });
    let rungs = menu_backdrops::rungs(Some(&icons), true);
    assert!(rungs[0].game_icon.is_some(), "the Glock's glyph");
    assert!(rungs[1].game_icon.is_none(), "no icon for the Deagle here: the class's");
    assert!(
        menu_backdrops::rungs(Some(&icons), false)
            .iter()
            .all(|r| r.game_icon.is_none()),
        "ours when not preferred"
    );
    // Painted as a glyph (a text) instead of triangles.
    let shapes = Shapes::new(&Icons::shipped());
    let mut p = Paint::new();
    menu_backdrops::paint(&mut p, &shapes, pal(), &rungs, 0, 1920.0, 1080.0, 1.0, false);
    assert!(p.texts.iter().any(|t| t.text == "c"));
}

/// The most the menu's systems may cost a frame (instructions), the
/// backdrops' painting included: the plan's 3 M (section 7).
const MENU_FRAME_LIMIT: u64 = 3_000_000;

/// The menu's systems (`main_menu`, `signal_menu`, `menu_backdrops`), as
/// the work counts name them.
const MENU_SYSTEMS: &[&str] = &[
    "look_loaded",
    "typing",
    "clicks",
    "act",
    "follow_loads",
    "follow_reconnect",
    "refresh_values",
    "ca_loading_gate",
    "fly",
    "follow_shown",
    "cameras",
    "show",
    "backdrop",
    "progress",
    "loading_under_lobby",
    "entry_hover",
    "music",
    "build",
    "select",
    "rotate",
    "paint_tabs",
    "place_highlight",
    "draw",
    "fade",
    "resolve_styles",
    "step_timelines",
    "apply_timelines",
];

#[test]
fn the_menus_frame_stays_in_budget() {
    let mut sim = sim_with(MENU_THEME);
    // Past the entrance, then each backdrop in turn (the Ladder's frames
    // are the busiest: plates, icons, chequer, glint).
    step(&mut sim, 2.0);
    let mut worst = 0u64;
    // The Ladder mid-climb (six plates, icons, brackets), the Assembly
    // hatching, the Ident under its sheen (every facet in eleven bands).
    for (which, at) in [(0, 6.6), (0, 21.2), (1, 2.0), (2, 2.4), (2, 5.4)] {
        {
            let now = sim.app.world().resource::<UiClock>().now;
            let mut s = sim.app.world_mut().resource_mut::<party_kit::signal_menu::SignalMenu>();
            s.choose(which, now);
            s.previous = None;
            s.since = now - at;
        }
        let Some(work) = sim.work(60) else {
            eprintln!(
                "skipping: no hardware counters ({})",
                chimera_core::metrics::hw_status()
            );
            return;
        };
        // The menu's own systems (the shell's and the game's aren't the
        // menu's cost).
        let total: u64 = work
            .systems
            .0
            .iter()
            .filter(|(k, _)| MENU_SYSTEMS.iter().any(|s| k.ends_with(&format!(" {s}"))))
            .map(|(_, v)| v.0)
            .sum::<u64>()
            / 60;
        let draw: u64 = work
            .systems
            .0
            .iter()
            .filter(|(k, _)| k.ends_with(" draw"))
            .map(|(_, v)| v.0)
            .sum::<u64>()
            / 60;
        let tris = sim.app.world().resource::<menu_backdrops::Canvases>().triangles;
        eprintln!("backdrop {which} at {at} s: the frame {total} instructions, painting {draw}, {tris} triangles");
        assert!(tris > 0);
        worst = worst.max(total);
    }
    assert!(
        worst < MENU_FRAME_LIMIT,
        "the menu's frame: {worst} instructions (limit {MENU_FRAME_LIMIT})"
    );
}
