//! A 20-character wide name (the server's cap: twenty W's, the widest
//! letter) in every Signal widget that shows a name, laid out by Bevy's
//! own UI with the Ultimate Gun Game's theme and faces at 1280x720 and
//! 1920x1080 (headless, a dummy camera): the race chip (chasing and
//! leading), the Tab board, the kill feed, the match end (winner,
//! standings, the climb's head, the vote cards' maps) and the pre-match
//! lobby's rows. Each fitted text (`chimera_ui::Ellipsize`) is drawn
//! whole inside its own box and inside every clipping plate above it and
//! the window, and ends in `…` (`...` in the UGG faces, which lack the
//! glyph) where the whole name doesn't fit. The fit's measure
//! (`KitFonts::width`) is near what Bevy draws.

use bevy::{
    camera::{ComputedCameraValues, RenderTargetInfo, Viewport},
    prelude::*,
    text::TextLayoutInfo,
    ui::UiGlobalTransform,
    window::PrimaryWindow,
};
use chimera_client::client::{
    kill_feed::KillFeedPlugin,
    match_end::MatchEndPlugin,
    minigame_scoreboard::MinigameScoreboardPlugin,
    prematch_lobby::PreMatchLobbyPlugin,
    ui_scene::UiScenePlugin,
};
use chimera_core::{console::Console, core::NetRole};
use chimera_net::{
    net::NetSettings,
    prematch::{LobbyRow, NetLobby, status},
};
use chimera_sim::SimBasePlugins;
use chimera_ui::{ChimeraUiPlugin, Ellipsize, KitFonts, Theme, UiClock};
use chimera_world::harness::Sim;
use ultimate_gun_game::signal;

const W20: &str = "WWWWWWWWWWWWWWWWWWWW";
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
        app.add_plugins((ChimeraUiPlugin, UiScenePlugin, MinigameScoreboardPlugin, KillFeedPlugin, MatchEndPlugin, PreMatchLobbyPlugin));
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
        cam.computed = ComputedCameraValues {
            target_info: Some(RenderTargetInfo { physical_size: UVec2::new(x, y), scale_factor: 1.0 }),
            ..default()
        };
        cam.viewport = Some(Viewport { physical_size: UVec2::new(x, y), ..default() });
    }
}

fn con(sim: &mut Sim, line: &str) {
    sim.app.world_mut().resource_mut::<Console>().submit(line);
}

/// Frames for the UI to settle (the fit reads the last layout).
fn settle(sim: &mut Sim) {
    for _ in 0..12 {
        sim.app.world_mut().resource_mut::<UiClock>().step(0.5);
        sim.app.update();
    }
}

/// A node's rect in physical px.
fn rect(w: &World, e: Entity) -> Option<Rect> {
    let (t, n) = (w.get::<UiGlobalTransform>(e)?, w.get::<ComputedNode>(e)?);
    Some(Rect::from_center_size(t.translation, n.size()))
}

fn hidden(w: &World, e: Entity) -> bool {
    let mut up = Some(e);
    while let Some(a) = up {
        if w.get::<Visibility>(a) == Some(&Visibility::Hidden) {
            return true;
        }
        up = w.get::<ChildOf>(a).map(|c| c.parent());
    }
    false
}

/// Every shown fitted text: its whole text, what it shows, and where it
/// spills (empty: nowhere).
fn check(sim: &mut Sim, what: &str, size: (u32, u32)) -> Vec<(String, String)> {
    let w = sim.app.world_mut();
    let fitted: Vec<(Entity, Ellipsize, String)> =
        w.query::<(Entity, &Ellipsize, &Text)>().iter(w).map(|(e, f, t)| (e, f.clone(), t.0.clone())).collect();
    let mut out = Vec::new();
    for (e, fit, shown) in fitted {
        if hidden(w, e) {
            continue;
        }
        let Some(r) = rect(w, e) else { continue };
        let drawn = w.get::<TextLayoutInfo>(e).map_or(0.0, |l| l.size.x);
        let at = format!("{what} at {size:?}: {:?} as {shown:?}", fit.full);
        // Drawn whole in its own box.
        assert!(drawn <= r.width() + 1.0, "{at}: the text is {drawn} px in a {} px box", r.width());
        // Inside every clipping plate above it, and the window.
        let mut up = w.get::<ChildOf>(e).map(|c| c.parent());
        while let Some(a) = up {
            if w.get::<Node>(a).is_some_and(|n| n.overflow.x != OverflowAxis::Visible)
                && let Some(p) = rect(w, a)
            {
                assert!(
                    r.min.x >= p.min.x - 1.0 && r.max.x <= p.max.x + 1.0,
                    "{at}: spills out of plate {a} ({:?} in {:?})",
                    (r.min.x, r.max.x),
                    (p.min.x, p.max.x)
                );
            }
            up = w.get::<ChildOf>(a).map(|c| c.parent());
        }
        assert!(r.min.x >= -1.0 && r.max.x <= size.0 as f32 + 1.0, "{at}: off the window");
        if shown != fit.full {
            assert!(shown.ends_with('\u{2026}') || shown.ends_with("..."), "{at}: cut without an ellipsis");
        }
        out.push((fit.full, shown));
    }
    out
}

fn cut(texts: &[(String, String)], full: &str) -> usize {
    texts.iter().filter(|(f, s)| f == full && s != f).count()
}

#[test]
fn a_twenty_w_name_fits_every_signal_widget() {
    let mut sim = sim();
    con(&mut sim, &format!("ui_scene_name {W20}"));
    // The kit's faces draw the cut's mark: `…`, else three dots.
    let kit = sim.app.world().resource::<KitFonts>().clone();
    let theme = sim.app.world().resource::<Theme>().clone();
    for role in [chimera_ui::Role::DisplayBold, chimera_ui::Role::Mono] {
        let f = chimera_ui::fonts::text(&theme, Some(&kit), role, 20.0).font;
        assert!(kit.has_glyph(&f, '\u{2026}') || kit.has_glyph(&f, '.'), "{role:?} draws no ellipsis or dot");
    }
    for size in SIZES {
        resize(&mut sim, size);
        for scene in ["chip", "lead", "board", "feed", "matchend"] {
            con(&mut sim, &format!("ui_scene {scene}"));
            settle(&mut sim);
            let texts = check(&mut sim, scene, size);
            // The chip's plate, measured to fit its line (`ChipFit`), stays
            // within the leading plate's 23 cq with the bigger gun icons
            // (the names cut for them) at 1080 lines; the feed's rows
            // within its 26 cq.
            {
                let w = sim.app.world_mut();
                let cq = chimera_ui::units::cq(1.0, size.1 as f32);
                let fit = w.get_resource::<chimera_client::client::race_chip::ChipFit>().map_or(0.0, |f| f.0);
                if matches!(scene, "chip" | "lead") && size.1 >= 1080 {
                    assert!(fit <= 23.5, "{scene} at {size:?}: the plate grew to {fit} cq");
                }
                let rows: Vec<f32> = w
                    .query_filtered::<&ComputedNode, With<chimera_client::client::kill_feed::KillFeedRow>>()
                    .iter(w)
                    .map(|n| n.size.x * n.inverse_scale_factor / cq)
                    .collect();
                assert!(rows.iter().all(|r| *r <= 26.2), "{scene} at {size:?}: feed rows {rows:?} cq");
                if scene == "feed" {
                    assert!(!rows.is_empty());
                }
            }
            let names: Vec<&(String, String)> = texts.iter().filter(|(f, _)| f == W20).collect();
            assert!(!names.is_empty(), "{scene} at {size:?}: no fitted name shown: {texts:?}");
            match scene {
                // The board's name column, the winner, the standings, the
                // chip's names: narrower than twenty W's.
                "board" | "matchend" | "chip" | "lead" => {
                    assert!(cut(&texts, W20) > 0, "{scene} at {size:?}: {texts:?}");
                }
                _ => {}
            }
            con(&mut sim, "ui_scene off");
            settle(&mut sim);
        }
        // The pre-match lobby, its rows from the lobby as a host sees it.
        let w = sim.app.world_mut();
        w.insert_resource(NetRole::Server);
        w.insert_resource(NetSettings { maxplayers: 6, ..default() });
        let row = |id: Option<u64>, st: u8| LobbyRow { id, name: W20.into(), status: st, done: 0, total: 0, step: chimera_net::prematch::NO_STEP, permille: 0, ping_ms: Some(40), wins: 3, avatar: 0, party: 0, spectator: false };
        let lobby = w
            .spawn(NetLobby {
                mode: "Ultimate Gun Game".into(),
                map: "cs_source:de_dust2".into(),
                rows: vec![row(Some(5), status::READY), row(Some(6), status::NOT_READY), row(None, status::READY)],
                ..default()
            })
            .id();
        settle(&mut sim);
        let texts = check(&mut sim, "lobby", size);
        assert!(texts.iter().filter(|(f, _)| f == W20).count() >= 3, "lobby at {size:?}: {texts:?}");
        let w = sim.app.world_mut();
        w.despawn(lobby);
        w.insert_resource(NetRole::Standalone);
        settle(&mut sim);
    }
}

#[test]
fn the_kits_measure_is_near_what_bevy_draws() {
    // Texts laid out by Bevy against the kit's measure, in each role and
    // at the sizes the screens use; within 6 % (Parley's kerning: the
    // UGG faces' W W pair draws wider, which `fit_texts` corrects from
    // the layout).
    let mut sim = sim();
    resize(&mut sim, (1920, 1080));
    settle(&mut sim);
    let kit = sim.app.world().resource::<KitFonts>().clone();
    let theme = sim.app.world().resource::<Theme>().clone();
    let mut probes = Vec::new();
    for role in [chimera_ui::Role::DisplayBold, chimera_ui::Role::DisplayBlack, chimera_ui::Role::Mono] {
        for text in [W20, "LOWGRAV_LARRY", "SGT. BISCUIT", "WWWWWWWWWWWWW..."] {
            for px in [9.0, 13.0, 20.0, 40.0] {
                let s = chimera_ui::fonts::text(&theme, Some(&kit), role, px);
                let e = sim
                    .app
                    .world_mut()
                    .spawn((Text::new(text), s.bundle(), TextLayout::new(Justify::Left, LineBreak::NoWrap), Node { position_type: PositionType::Absolute, ..default() }))
                    .id();
                probes.push((e, role, text, s, px));
            }
        }
    }
    settle(&mut sim);
    for (e, role, text, s, px) in probes {
        let drawn = sim.app.world().get::<TextLayoutInfo>(e).unwrap().size.x;
        let measured = kit.width(&s.font, Some(&s.spacing), text);
        assert!((measured - drawn).abs() <= drawn * 0.06 + 1.0, "{role:?} {px} {text}: measured {measured}, drawn {drawn}");
    }
}
