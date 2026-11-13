//! The overlay guard (docs/DESIGN.md, "HUD layers"): the shell's overlay
//! layer (`chimera_client::client::overlay`: the minigame's panel, its
//! callout, the kill feed) is laid out the same under every composed
//! game's HUD, and never stretched with a window's aspect. For every game
//! the party registers with a HUD (`party_kit::client::CLIENTS`: a new game
//! is covered once it is in that list), its HUD is put up as in play (the
//! local player on a rung of one of its guns: its roots, its look and its
//! `GameDraws` as the app switches them) at
//! 16:9, 4:3 and 21:9, and the layer's nodes are laid out by Bevy's own
//! UI (a dummy camera, no renderer): each node's place and size, and each
//! text's font, must be one for every game at a size, and scale with the
//! window's height alone. Headless, on `kill_feed`'s every-HUD setup.

use bevy::{
    camera::{ComputedCameraValues, RenderTargetInfo, Viewport},
    prelude::*,
    ui::UiGlobalTransform,
    window::PrimaryWindow,
};
use chimera_client::client::{
    callouts::CalloutsPlugin,
    kill_feed::{FeedName, KillEntry, KillFeed},
    minigame_panel::{MinigamePanelPlugin, PanelLine},
    overlay::{OverlayLayer, OverlayRoot},
};
use chimera_core::{core::DamageKind, gameplay::Gameplay};
use chimera_sim::minigame::{
    hud::{Callout, HudLine, MinigameHud, ShownCallout},
    ladder::{self, Ladder},
};
use chimera_world::harness::Sim;

/// The sizes the layer is laid out at: 16:9, 4:3, 21:9 (two of them as
/// tall as each other).
const SIZES: [(u32, u32); 3] = [(1920, 1080), (1280, 960), (2560, 1080)];

/// What the layout of one of the layer's nodes is: its depth-first
/// index under the root, its centre and size (px), its text's size and
/// face, and the text drawn in it and under it.
#[derive(Clone, Debug, PartialEq)]
struct Laid {
    index: usize,
    centre: Vec2,
    size: Vec2,
    font: Option<f32>,
    face: Option<String>,
    text: String,
}

impl Laid {
    /// Laid out alike: the same place and size; a node whose words differ
    /// with the HUD's gun (the panel's rung line) the same font, height,
    /// top edge and centre (the panel and callout lines are centred).
    fn alike(&self, o: &Laid) -> bool {
        let near = |a: f32, b: f32| (a - b).abs() < 0.5;
        let common = self.font == o.font
            && self.face == o.face
            && near(self.size.y, o.size.y)
            && near(self.centre.y, o.centre.y)
            // Centred text of another width rounds to the half pixel.
            && (self.centre.x - o.centre.x).abs() <= 1.0;
        common && (self.text != o.text || near(self.size.x, o.size.x))
    }
}

/// The text of `e` and every node under it.
fn text_under(w: &World, e: Entity) -> String {
    let mut out = w.get::<Text>(e).map(|t| t.0.clone()).unwrap_or_default();
    if let Some(c) = w.get::<Children>(e) {
        for c in c.iter() {
            out += &text_under(w, c);
        }
    }
    out
}

/// The party's every-HUD setup with Bevy's UI layout, the shell's overlay
/// pieces, and a primary window with a camera on it.
fn laid_out() -> (Sim, Entity) {
    let (mut sim, me, _killer) = crate::kill_feed::every_hud_sim_and(true, |app| {
        app.init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy::picking::hover::HoverMap>()
            .init_resource::<bevy::picking::pointer::PointerMap>()
            .init_resource::<bevy::picking::events::PointerState>()
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::picking::backend::PointerHits>()
            .init_asset::<Font>();
        app.add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin));
        app.add_plugins((MinigamePanelPlugin, CalloutsPlugin));
    });
    let w = sim.app.world_mut();
    let window = w.query_filtered::<Entity, With<Window>>().iter(w).next().expect("the window");
    w.entity_mut(window).insert(PrimaryWindow);
    w.spawn((Camera2d, Camera::default()));
    (sim, me)
}

/// Window and camera at `size`.
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

/// A callout showing over the minigame's panel (its title and status
/// lines the gun game's), and one line in the kill feed.
fn fill(sim: &mut Sim) {
    let w = sim.app.world_mut();
    let now = w.resource::<Time>().elapsed_secs_f64();
    let mut hud = w.resource::<MinigameHud>().clone();
    hud.callout = Some(ShownCallout {
        callout: Callout::new(2, "last_rung", vec![HudLine::new("Last level!"), HudLine::new("Old Glory Knife: a kill wins")]),
        since: now,
        id: 7,
    });
    w.insert_resource(hud);
    let at = w.resource::<Time>().elapsed_secs();
    let mut feed = w.resource_mut::<KillFeed>();
    if feed.entries.is_empty() {
        feed.entries.push(KillEntry {
            id: 1_000,
            killer: Some(FeedName { entity: Entity::PLACEHOLDER, name: "AI_Alpha_9".into(), local: false, team: Some(1) }),
            victim: FeedName { entity: Entity::PLACEHOLDER, name: "Player".into(), local: true, team: Some(2) },
            weapon: None,
            kind: DamageKind::Bullet,
            headshot: false,
            suicide: false,
            team_kill: false,
            at,
        });
    } else {
        for e in &mut feed.entries {
            e.at = at;
        }
    }
}

/// `game`'s HUD up at `size`, the layer laid out: its nodes, depth first.
fn layout(sim: &mut Sim, me: Entity, game: &'static str, size: (u32, u32)) -> Vec<Laid> {
    let l = sim.app.world().resource::<Ladder>().clone();
    let rung = l
        .rungs
        .iter()
        .position(|g| g.starts_with(&format!("{game}:")))
        .unwrap_or_else(|| panic!("no rung of {game}'s guns in `every_hud_sim`'s ladder: add one (kill_feed.rs)"));
    ladder::set_rung(sim.app.world_mut(), me, rung, &l);
    sim.seconds(0.5);
    resize(sim, size);
    fill(sim);
    for _ in 0..3 {
        sim.ticks(1);
        sim.app.update();
    }
    assert_eq!(sim.app.world().resource::<Gameplay>().hud, game, "{game}'s HUD up");
    let w = sim.app.world_mut();
    let root = w.resource::<OverlayLayer>().0;
    assert!(w.get::<OverlayRoot>(root).is_some(), "the layer's root");
    assert!(w.get::<ChildOf>(root).is_none(), "the layer's root is a root");
    // No game's scale reaches the layer: Bevy's global UI scale stays 1.
    assert_eq!(w.get_resource::<UiScale>().map_or(1.0, |s| s.0), 1.0, "UiScale under {game}'s HUD");
    let mut nodes = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if e != root {
            nodes.push(e);
        }
        if let Some(c) = w.get::<Children>(e) {
            stack.extend(c.iter().rev());
        }
    }
    let mut out = Vec::new();
    for (index, e) in nodes.into_iter().enumerate() {
        // Shown: nothing on the way up hidden (no renderer here to
        // propagate `InheritedVisibility`).
        let mut up = Some(e);
        let mut hidden = false;
        while let Some(a) = up {
            hidden |= w.get::<Visibility>(a) == Some(&Visibility::Hidden);
            up = w.get::<ChildOf>(a).map(|c| c.parent());
        }
        if hidden {
            continue;
        }
        let (Some(t), Some(n)) = (w.get::<UiGlobalTransform>(e), w.get::<ComputedNode>(e)) else { continue };
        let font = w.get::<TextFont>(e).map(|f| match f.font_size {
            bevy::text::FontSize::Px(p) => p,
            _ => f32::NAN,
        });
        let face = w.get::<TextFont>(e).map(|f| format!("{:?}", f.font));
        out.push(Laid { index, centre: t.translation, size: n.size, font, face, text: text_under(w, e) });
    }
    out
}

/// What `layout` drew of the panel and the callout (their texts).
fn texts(sim: &mut Sim) -> Vec<String> {
    let w = sim.app.world_mut();
    let mut out: Vec<String> = w.query::<(&PanelLine, &Text)>().iter(w).map(|(_, t)| t.0.clone()).collect();
    out.extend(w.resource::<chimera_client::client::callouts::CalloutView>().lines.iter().cloned());
    out
}

#[test]
fn the_overlay_is_laid_out_alike_under_every_games_hud() {
    let (mut sim, me) = laid_out();
    let games: Vec<&'static str> =
        party_kit::client::CLIENTS.iter().filter(|c| c.hud.is_some()).map(|c| c.game).collect();
    assert!(games.len() >= 3, "every composed game with a HUD: {games:?}");
    let mut by_size: Vec<Vec<Laid>> = Vec::new();
    for size in SIZES {
        let mut first: Option<(&str, Vec<Laid>, Vec<String>)> = None;
        for &game in &games {
            let laid = layout(&mut sim, me, game, size);
            let shown = texts(&mut sim);
            assert!(shown.iter().any(|t| t.starts_with("Gun Game")), "the panel under {game}'s HUD: {shown:?}");
            assert!(shown.iter().any(|t| t == "Last level!"), "the callout under {game}'s HUD: {shown:?}");
            assert!(laid.len() >= 6, "the layer's nodes under {game}'s HUD at {size:?}: {laid:?}");
            match &first {
                None => first = Some((game, laid, shown)),
                Some((g0, l0, s0)) => {
                    assert_eq!(shown.len(), s0.len(), "as many lines under {game}'s HUD ({shown:?}) as under {g0}'s ({s0:?})");
                    assert_eq!(laid.len(), l0.len(), "as many nodes under {game}'s HUD as under {g0}'s at {size:?}");
                    for (a, b) in laid.iter().zip(l0) {
                        assert_eq!(a.index, b.index);
                        assert!(a.alike(b), "node {} under {game}'s HUD at {size:?}: {a:?}; under {g0}'s: {b:?}", a.index);
                    }
                }
            }
        }
        by_size.push(first.expect("a game").1);
    }
    // One rule for every aspect: sizes and fonts follow the height alone
    // (1920x1080 and 2560x1080 alike; 1280x960 by 960/1080), never the
    // width (a 4:3 canvas stretched).
    let (wide, square, ultra) = (&by_size[0], &by_size[1], &by_size[2]);
    assert_eq!(wide.len(), ultra.len());
    // (Texts: the boxes holding them span the screen.)
    for (a, b) in wide.iter().zip(ultra).filter(|(a, _)| a.font.is_some()) {
        assert!((a.size.y - b.size.y).abs() < 0.5, "node {} at 16:9 {a:?} and 21:9 {b:?}", a.index);
        assert!(a.text != b.text || (a.size.x - b.size.x).abs() < 0.5, "node {} at 16:9 {a:?} and 21:9 {b:?}", a.index);
        assert_eq!(a.font, b.font);
    }
    let k = 960.0 / 1080.0;
    for (a, b) in wide.iter().zip(square) {
        if let (Some(fa), Some(fb)) = (a.font, b.font) {
            assert!((fa * k - fb).abs() < 0.05, "node {}'s font at 16:9 {fa} and 4:3 {fb}", a.index);
        }
    }
}
