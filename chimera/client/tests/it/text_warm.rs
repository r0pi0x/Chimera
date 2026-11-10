//! HUD texts' glyph atlases made ahead (`client::text_warm`; backlog
//! "Nothing blocks the main thread", docs/performance.md): with the warm-up
//! done (behind the loading screen, or a slice a frame in play), the first
//! kill in the feed, the first look at the scoreboard and the minigame panel
//! with its lines make no new glyph atlas (Bevy's `FontAtlasSet`, counted:
//! not time); without it each makes its own. The same for a game's icon
//! glyphs in the feed (`WeaponIcons::register_glyph_fonts`, its headshot
//! mark) and a game's menu layout opening (`vgui::warm_fonts`). On Bevy's own text pipeline
//! and layout headless (a dummy camera), in Bevy's default face (no system
//! faces headless).

use bevy::{prelude::*, text::FontAtlasSet};
use chimera_client::client::{
    console::HeldActions,
    kill_feed::{KillFeedPlugin, KillFeedStatePlugin, SharedKillFeed},
    minigame_panel::MinigamePanelPlugin,
    minigame_scoreboard::{GunNames, MinigameScoreboardPlugin},
    text_warm::{self, TextWarm, TextWarmAppExt, TextWarmPlugin},
    vgui::{self, Painter},
    weapon_icons::{GlyphFont, WeaponIcon, WeaponIcons},
};
use chimera_core::{
    core::{Damage, DamageKind, Died, Hitgroup, Score},
    metrics::ThreadCounters,
};
use chimera_sim::{
    SimBasePlugins,
    minigame::{
        hud::MinigameHud,
        ladder::{Ladder, Rung},
        runtime::InMinigame,
        wins::MatchesWon,
    },
};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

/// The fonts' and sizes' atlases, and the atlases in all.
fn atlases(sim: &Sim) -> (usize, usize) {
    let set = sim.app.world().resource::<FontAtlasSet>();
    (set.values().filter(|a| !a.is_empty()).count(), set.values().map(Vec::len).sum())
}

/// The kit's icon font's em (the feed's glyphs: another size than the names').
const ICON_EM: f32 = 16.0;

/// The shell's feed, board and panel on a laid-out UI, with the warm-up's
/// systems or without; the kit's guns have glyph icons and a headshot mark
/// (in the default face: no icon font headless).
fn hud_sim(warm: bool) -> Sim {
    hud_sim_with(warm, |_| {})
}

fn hud_sim_with(warm: bool, extra: impl Fn(&mut App) + Send + Sync + 'static) -> Sim {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
    let mut sim = Sim::with(SimBasePlugins, move |app| {
        app.add_plugins((GreyboxMapPlugin, MinigameScoreboardPlugin, KillFeedStatePlugin, KillFeedPlugin));
        app.add_plugins(MinigamePanelPlugin("kit"));
        if warm {
            app.add_plugins(TextWarmPlugin);
        }
        app.init_resource::<Assets<Image>>()
            .init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
            .init_resource::<ButtonInput<MouseButton>>()
            .init_resource::<Touches>()
            .init_resource::<bevy::picking::hover::HoverMap>()
            .init_resource::<bevy::picking::pointer::PointerMap>()
            .init_resource::<bevy::picking::events::PointerState>()
            .add_message::<bevy::picking::pointer::PointerInput>()
            .add_message::<bevy::picking::backend::PointerHits>()
            .init_asset::<Font>()
            .init_resource::<HeldActions>()
            .insert_resource(SharedKillFeed);
        app.add_plugins((bevy::text::TextPlugin, bevy::ui::UiPlugin));
        let mut icons = WeaponIcons::default();
        let font = Handle::<Font>::default();
        let rifle = WeaponIcon::Glyph { font: font.clone(), ch: 'R', em: ICON_EM };
        icons.register("kit", move |id| (id == "kit:rifle").then(|| rifle.clone()));
        icons.register_mark("kit", chimera_client::client::kill_feed::HEADSHOT, WeaponIcon::Glyph { font: font.clone(), ch: 'H', em: ICON_EM });
        icons.register_glyph_fonts("kit", vec![GlyphFont { font: font.clone(), em: ICON_EM, chars: "R".into() }]);
        app.insert_resource(icons);
        extra(app);
    });
    sim.app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues {
                target_info: Some(RenderTargetInfo { physical_size: UVec2::new(1280, 720), scale_factor: 1.0 }),
                ..default()
            },
            viewport: Some(Viewport { physical_size: UVec2::new(1280, 720), ..default() }),
            ..default()
        },
    ));
    sim.app.insert_resource(Ladder { rungs: vec!["kit:pistol", "kit:rifle"], ..default() });
    sim.app.world_mut().insert_resource(GunNames([("kit:rifle".to_string(), "Kit Rifle".to_string())].into()));
    sim.app.world_mut().insert_resource(MinigameHud { title: "Kit".into(), ..default() });
    for (name, rung) in [("Alpha", 0), ("Bravo", 1)] {
        sim.app.world_mut().spawn((InMinigame, Name::new(name), Rung(rung), Score::default(), MatchesWon(0)));
    }
    sim.ticks(3);
    sim
}

/// The first kill in the feed (a headshot: the gun's glyph and the mark) and
/// the scoreboard's first showing, in one
/// frame: the atlases made, and the main thread's instructions (0 without
/// counters).
fn first_show(sim: &mut Sim) -> ((usize, usize), u64) {
    let a = sim.spawn_character(Vec3::new(0.0, 1.0, 0.0), chimera_sim::movement::placeholder::ID);
    let b = sim.spawn_character(Vec3::new(5.0, 1.0, 0.0), chimera_sim::movement::placeholder::ID);
    let before = atlases(sim);
    let mut counters = ThreadCounters::current();
    let start = counters.read();
    sim.app.world_mut().write_message(Died {
        entity: b,
        attacker: Some(a),
        damage: Damage {
            target: b,
            attacker: Some(a),
            amount: 1.0,
            point: Vec3::ZERO,
            dir: Vec3::NEG_Z,
            hitgroup: Hitgroup::Head,
            kind: DamageKind::Bullet,
            weapon: Some("kit:rifle"),
            force: Vec3::ZERO,
        },
    });
    sim.app.world_mut().resource_mut::<HeldActions>().showscores = true;
    sim.app.update();
    sim.app.update();
    let spent = (counters.read() - start).hw.map_or(0, |h| h.instructions);
    let after = atlases(sim);
    assert!(
        sim.app.world_mut().query::<&chimera_client::client::kill_feed::KillFeedRow>().iter(sim.app.world()).count() > 0,
        "the feed shows the kill"
    );
    ((after.0 - before.0, after.1 - before.1), spent)
}

#[test]
fn a_huds_first_show_makes_no_atlas_after_the_warm_up() {
    // Without the warm-up: the feed's font makes its atlas on the frame it
    // first shows (the board's size is the panel's title's, up already).
    let mut cold = hud_sim(false);
    let (made, cold_cost) = first_show(&mut cold);
    assert!(made.0 >= 1, "the feed's font makes an atlas: {made:?}");

    // Behind the loading screen (`settle` a frame, as `client::console`'s
    // settling calls it), then the same show.
    let mut sim = hud_sim(true);
    let mut frames = 0;
    while !text_warm::settle(sim.app.world_mut()) {
        sim.app.update();
        frames += 1;
        assert!(frames < 30, "the warm-up ends");
    }
    let warm = sim.app.world().resource::<TextWarm>();
    assert!(warm.warmed >= 2);
    // Each atlas came made from the pool, under the key Bevy looked up.
    assert!(warm.seeded >= 2 && warm.missed == 0, "{} seeded, {} missed", warm.seeded, warm.missed);
    let (made, warm_cost) = first_show(&mut sim);
    assert_eq!(made, (0, 0), "a font and size the HUD declared made an atlas on its first show");
    sim.app.update();
    assert_eq!(sim.app.world().resource::<TextWarm>().unwarmed, 0);
    // A text at a size nobody declared makes its atlas as it shows, and the
    // warm-up says so (`TextWarm::unwarmed`, and a log line).
    sim.app.world_mut().spawn((Text::new("undeclared"), TextFont { font_size: FontSize::Px(41.0), ..default() }));
    sim.app.update();
    sim.app.update();
    assert_eq!(sim.app.world().resource::<TextWarm>().unwarmed, 1);
    eprintln!(
        "first show: {:.1} M instructions cold, {:.1} M warmed ({frames} frames of warm-up)",
        cold_cost as f64 / 1e6,
        warm_cost as f64 / 1e6
    );
}

/// In play (the command line's `--map`: no loading screen), what the
/// sources name warms a slice a frame (`GLYPHS_PER_FRAME` glyphs at most,
/// the atlases from the pool made on a task), before the HUD shows it.
#[test]
fn in_play_the_warm_up_goes_a_slice_a_frame() {
    let mut sim = hud_sim(true);
    assert!(sim.app.world().resource::<TextWarm>().idle(), "the HUD's fonts warmed as the app starts");
    // Four sizes a hidden label will show in: found the next time the
    // sources are asked.
    for size in [30.0, 31.0, 32.0, 33.0] {
        sim.app.world_mut().spawn((Text::default(), TextFont { font_size: FontSize::Px(size), ..default() }, Visibility::Hidden));
    }
    let (mut frames, mut busy) = (0, 0);
    let mut counters = ThreadCounters::current();
    let (mut idle_cost, mut busy_cost) = (Vec::new(), Vec::new());
    loop {
        let start = counters.read();
        sim.app.update();
        let spent = (counters.read() - start).hw.map_or(0, |h| h.instructions);
        frames += 1;
        assert!(frames < text_warm::RECOLLECT_FRAMES as usize + 100, "the warm-up ends");
        let w = sim.app.world_mut();
        let out: usize = w.query_filtered::<&Text, With<text_warm::WarmNode>>().iter(w).map(|t| t.0.chars().count()).sum();
        assert!(out <= text_warm::GLYPHS_PER_FRAME, "{out} glyphs laid out in one frame");
        if out > 0 {
            busy += 1;
            busy_cost.push(spent);
        } else if busy == 0 {
            idle_cost.push(spent);
        } else if sim.app.world().resource::<TextWarm>().idle() {
            break;
        }
    }
    // 4 x 95 glyphs, 16 a frame.
    assert!(busy >= 24, "spread over {busy} frames");
    idle_cost.sort_unstable();
    eprintln!(
        "in play: frames {:.2} M idle (median), {:.2} M at worst warming {} glyphs a frame",
        idle_cost.get(idle_cost.len() / 2).copied().unwrap_or(0) as f64 / 1e6,
        busy_cost.iter().max().copied().unwrap_or(0) as f64 / 1e6,
        text_warm::GLYPHS_PER_FRAME
    );
    let before = atlases(&sim);
    let w = sim.app.world_mut();
    let mut q = w.query_filtered::<&mut Text, Without<text_warm::WarmNode>>();
    for mut t in q.iter_mut(w) {
        if t.0.is_empty() {
            t.0 = "Level 7: Kit Rifle".into();
        }
    }
    sim.app.update();
    assert_eq!(atlases(&sim), before, "the labels' first show made no atlas");
    assert_eq!(sim.app.world().resource::<TextWarm>().missed, 0, "every atlas put in ahead was Bevy's key");
}


/// A game's menu (`GameMenus`: a buy or team menu, a message of the day)
/// opening for the first time: its labels' and buttons' fonts were declared
/// (`vgui::warm_fonts`), so it makes no atlas after the warm-up; without it,
/// one.
#[test]
fn a_menus_first_opening_makes_no_atlas_after_the_warm_up() {
    use chimera_client::client::fonts::UiFonts;
    use chimera_world::map::hud::{ActiveHud, GameHud, GameMenus, UiControl, UiFontSize, UiKind, UiLayout};
    let layout = UiLayout {
        controls: vec![
            UiControl {
                text: "Pick a side".into(),
                font: Some("KitMenuTitle".into()),
                ..UiControl::new("Title", UiKind::Label, 20.0, 20.0, 200.0, 24.0)
            },
            UiControl { text: "Join".into(), ..UiControl::new("Join", UiKind::Button, 20.0, 60.0, 120.0, 24.0) },
        ],
        ..default()
    };
    let hud = GameHud {
        menus: Some(GameMenus { layouts: [("kit_menu".to_string(), layout.clone())].into(), ..default() }),
        ..default()
    };
    let open = |warm: bool| {
        let hud = hud.clone();
        let mut sim = hud_sim_with(warm, move |app| {
            // The title in a scheme font of its own size (the buttons'
            // `Default` is the panel's, up already).
            let mut fonts = UiFonts::default();
            let title = UiFontSize { family: "Kit Sans".into(), tall: 22.0, weight: 700, yres: None, antialias: true, additive: false };
            fonts.set_client([("KitMenuTitle".to_string(), vec![title])].into());
            app.insert_resource(fonts).insert_resource(ActiveHud(std::sync::Arc::new(hud.clone()), default()));
            if warm {
                app.warm_texts(vgui::warm_fonts);
            }
        });
        let mut frames = 0;
        while warm && !text_warm::settle(sim.app.world_mut()) {
            sim.app.update();
            frames += 1;
            assert!(frames < 30, "the warm-up ends");
        }
        let before = atlases(&sim);
        let mut counters = ThreadCounters::current();
        let start = counters.read();
        let w = sim.app.world_mut();
        let root = w.spawn(Node { width: percent(100.0), height: percent(100.0), ..default() }).id();
        let hud = w.resource::<ActiveHud>().clone();
        let fonts = w.remove_resource::<UiFonts>().unwrap();
        {
            let painter = Painter::new(&hud, &fonts, 720.0);
            let mut commands = w.commands();
            let drawn = painter.spawn(&mut commands, root, Vec2::new(1280.0, 720.0), &layout, vgui::VguiMenu::Team, &mut vgui::Shown::of);
            assert_eq!(drawn.len(), 2);
        }
        w.flush();
        w.insert_resource(fonts);
        sim.app.update();
        sim.app.update();
        let spent = (counters.read() - start).hw.map_or(0, |h| h.instructions);
        let after = atlases(&sim);
        ((after.0 - before.0, after.1 - before.1), spent)
    };
    let (made, cold) = open(false);
    assert!(made.0 >= 1, "the menu's font makes an atlas as it opens");
    let (made, warm) = open(true);
    assert_eq!(made, (0, 0), "a menu font declared ahead made an atlas on the menu's opening");
    eprintln!("menu's first opening: {:.1} M instructions cold, {:.1} M warmed", cold as f64 / 1e6, warm as f64 / 1e6);
}
