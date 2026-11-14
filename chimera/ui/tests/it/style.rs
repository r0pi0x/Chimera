//! The style layer (chimera_ui::style) and BSN scenes built on it.

use bevy::prelude::*;
use chimera_ui::{ChimeraUiPlugin, Role, Theme, Tone, UiText, UiUnits, style};

/// A styled node's px are the look book's CSS (`v cqw`, a 16:9 frame's
/// width / 100) rounded to whole physical pixels, at 1080p and 1440p.
#[test]
fn styled_px_follow_the_css_formula() {
    let s = style!(row, gap: 0.6, pad: [0.7, 0.8], h: 2.15, w: 21.0);
    for (w, h) in [(1920.0f32, 1080.0f32), (2560.0, 1440.0)] {
        let u = UiUnits::new(h, 1.0);
        let cqw = w / 100.0;
        let css = |v: f32| Val::Px((v * cqw).round());
        let n = s.node(&u);
        assert_eq!(n.flex_direction, FlexDirection::Row);
        assert_eq!(n.column_gap, css(0.6), "gap at {h}");
        assert_eq!(n.height, css(2.15), "height at {h}");
        assert_eq!(n.width, css(21.0), "width at {h}");
        assert_eq!((n.padding.top, n.padding.bottom), (css(0.7), css(0.7)));
        assert_eq!((n.padding.left, n.padding.right), (css(0.8), css(0.8)));
    }
    // At 150 % scale the same physical pixels (logical px times 1.5 whole).
    let u = UiUnits::new(720.0, 1.5);
    let Val::Px(g) = s.node(&u).column_gap else { panic!() };
    assert_eq!((g * 1.5).fract(), 0.0);
    // CSS shorthands: four values in CSS order.
    let n = style!(column, pad: [1.0, 2.0, 3.0, 4.0]).node(&UiUnits::new(720.0, 1.0));
    assert_eq!(n.padding, UiRect::new(Val::Px(51.0), Val::Px(26.0), Val::Px(13.0), Val::Px(38.0)));
}

fn app(h: f32) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin, bevy::scene::ScenePlugin))
        .add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(h, 1.0));
    app.insert_resource(Theme::from_toml(include_str!("../../../../apps/ultimate_gun_game/data/theme.toml")).unwrap());
    app
}

/// Styled nodes follow the window: laid out again when `UiUnits` change.
#[test]
fn styles_follow_the_units() {
    let mut app = app(1080.0);
    let e = app.world_mut().spawn((style!(row, h: 2.15).bg(Tone::Plate), Text::new("x"), UiText::new(Role::Mono, 0.62, Tone::Fg2))).id();
    app.update();
    assert_eq!(app.world().get::<Node>(e).unwrap().height, Val::Px((2.15f32 * 19.2).round()));
    assert_eq!(app.world().get::<TextFont>(e).unwrap().font_size, bevy::text::FontSize::Px((0.62f32 * 19.2).round()));
    app.world_mut().insert_resource(UiUnits::new(1440.0, 1.0));
    app.update();
    assert_eq!(app.world().get::<Node>(e).unwrap().height, Val::Px((2.15f32 * 25.6).round()));
}

/// A board-like tree in BSN: styled rows from data (a dynamic list), text
/// in the kit's roles, a slanted material node, any marker beside them
/// (a parity id).
#[test]
fn bsn_builds_styled_ui_trees() {
    use bevy::scene::{CommandsSceneExt, Scene, bsn};
    #[derive(Component, Default, Clone, Debug)]
    struct ParityId(String);

    fn row(i: usize, name: &str) -> impl Scene {
        let name = name.to_string();
        let row_style = style!(row, gap: 0.6, pad: [0.0, 0.8], h: 2.15);
        let bar_style = style!(row, w: 0.32, h: 2.15);
        let name_text = UiText::new(Role::DisplayBold, 0.78, Tone::Fg);
        bsn! {
            row_style
            ParityId({format!("board.row.{i}")})
            Children [
                chimera_ui::UiSlant { fill: Tone::Acc }
                bar_style
                --
                Text({name})
                name_text
                ParityId({format!("board.row.{i}.name")})
            ]
        }
    }

    let mut app = app(1080.0);
    let names = ["nadia.exe", "kestrel", "vex"];
    let rows: Vec<_> = names.iter().enumerate().map(|(i, n)| row(i, n)).collect();
    let board = style!(column, gap: 0.2);
    app.world_mut().commands().spawn_scene(bsn! {
        board
        Children [ {rows} ]
    });
    app.update();
    app.update();
    let w = app.world_mut();
    let mut q = w.query::<(&ParityId, &Node)>();
    let mut ids: Vec<String> = q.iter(w).map(|(p, _)| p.0.clone()).collect();
    ids.sort();
    assert_eq!(ids, ["board.row.0", "board.row.0.name", "board.row.1", "board.row.1.name", "board.row.2", "board.row.2.name"]);
    let mut texts = w.query::<(&Text, &TextFont, &ParityId)>();
    for (t, f, p) in texts.iter(w) {
        assert!(names.contains(&t.0.as_str()), "{p:?}");
        assert_eq!(f.weight.0, 700);
    }
    let mut slants = w.query::<&MaterialNode<chimera_ui::SlantMaterial>>();
    assert_eq!(slants.iter(w).count(), 3, "each row's slanted bar has its material");
}
