//! The form controls (chimera_ui::controls): what each builds, its states,
//! hover from the pointer, the wipe.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_ui::{
    ChimeraUiPlugin, Theme, Tone, UiClock, UiSlant, UiText, UiUnits,
    controls::{self, Hover, Kind, State, Tick, Ui, Wipe},
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin)).add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app
}

/// Builds with the app's theme and units under a fresh root.
fn build<R>(app: &mut App, f: impl FnOnce(&Ui, &mut Commands, Entity) -> R) -> R {
    let theme = app.world().resource::<Theme>().clone();
    let units = *app.world().resource::<UiUnits>();
    let ui = Ui::new(&theme, None, units);
    let world = app.world_mut();
    let root = world.spawn(Node::default()).id();
    let mut queue = bevy::ecs::world::CommandQueue::default();
    let mut c = Commands::new(&mut queue, world);
    let r = f(&ui, &mut c, root);
    queue.apply(world);
    app.update();
    r
}

fn texts_under(app: &mut App, e: Entity) -> Vec<String> {
    let w = app.world_mut();
    let mut out = Vec::new();
    let mut stack = vec![e];
    while let Some(n) = stack.pop() {
        if let Some(t) = w.get::<Text>(n) {
            out.push(t.0.clone());
        }
        if let Some(ch) = w.get::<Children>(n) {
            stack.extend(ch.iter().rev());
        }
    }
    out
}

fn descendants_with<C: Component + Clone>(app: &mut App, e: Entity) -> Vec<C> {
    let w = app.world_mut();
    let mut out = Vec::new();
    let mut stack = vec![e];
    while let Some(n) = stack.pop() {
        if let Some(c) = w.get::<C>(n) {
            out.push(c.clone());
        }
        if let Some(ch) = w.get::<Children>(n) {
            stack.extend(ch.iter());
        }
    }
    out
}

/// A primary button: the white slab on the slant, ink text, its key cap;
/// focus adds the accent tick (always shown).
#[test]
fn buttons_carry_their_kind_and_focus() {
    let mut app = app();
    let b = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Primary, "Apply", Some("Ctrl S"), 2.0, State::focused(true)));
    assert_eq!(texts_under(&mut app, b), ["APPLY", "CTRL S"]);
    let slants = descendants_with::<UiSlant>(&mut app, b);
    assert!(slants.iter().any(|s| s.fill == Tone::Fg && s.edges == (false, true)), "the white slab leans at the right");
    let ticks = descendants_with::<Tick>(&mut app, b);
    assert_eq!(ticks, [Tick { always: true }], "focus: the tick shows always");
    let texts = descendants_with::<UiText>(&mut app, b);
    assert_eq!(texts[0].tone, Tone::Ink);
    // Danger: the threat mark, never a threat fill.
    let d = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Danger, "End the game", None, 2.4, State::NONE));
    let slants = descendants_with::<UiSlant>(&mut app, d);
    assert!(slants.iter().any(|s| s.fill == Tone::Threat), "the threat mark");
    let bg = app.world().get::<BackgroundColor>(d).unwrap().0;
    assert_ne!(bg, app.world().resource::<Theme>().tone(Tone::Threat));
}

/// Hover is the tick and a faint tint, live from the pointer, back off
/// when it leaves.
#[test]
fn hover_shows_the_tick_and_tint() {
    let mut app = app();
    let (row, _) = build(&mut app, |ui, c, root| ui.row(c, root, "Field of view", None, 2.9, State::NONE));
    let tick = app.world().get::<Children>(row).unwrap().iter().find(|e| app.world().get::<Tick>(*e).is_some()).unwrap();
    assert_eq!(app.world().get::<Visibility>(tick), Some(&Visibility::Hidden));
    let hover = *app.world().get::<Hover>(row).unwrap();
    *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Hovered;
    app.update();
    assert_eq!(app.world().get::<Visibility>(tick), Some(&Visibility::Inherited));
    assert_eq!(app.world().get::<BackgroundColor>(row).unwrap().0, hover.over);
    *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::None;
    app.update();
    assert_eq!(app.world().get::<Visibility>(tick), Some(&Visibility::Hidden));
    assert_eq!(app.world().get::<BackgroundColor>(row).unwrap().0, hover.base);
}

/// A row: the label, the changed mark in accent, its least height; a
/// toggle on is white with ON in ink; segmented's selected is white.
#[test]
fn rows_toggles_and_segments_show_their_state() {
    let mut app = app();
    let (row, vl, tog) = build(&mut app, |ui, c, root| {
        let st = State { changed: true, ..State::NONE };
        let (row, vl) = ui.row(c, root, "Vertical sync", None, 2.9, st);
        let t = ui.toggle(c, vl, true, State::NONE);
        (row, vl, t)
    });
    assert_eq!(texts_under(&mut app, row)[0], "VERTICAL SYNC");
    let slants = descendants_with::<UiSlant>(&mut app, row);
    assert!(slants.iter().any(|s| s.fill == Tone::Acc), "changed: the accent square");
    let px = (2.9f32 * 19.2).round();
    assert_eq!(app.world().get::<Node>(row).unwrap().min_height, Val::Px(px));
    assert!(app.world().get::<ChildOf>(tog).is_some_and(|p| p.parent() == vl));
    let tog_slants = descendants_with::<UiSlant>(&mut app, tog);
    assert!(tog_slants.iter().any(|s| s.fill == Tone::Fg && s.alpha == 1.0 && s.outline == 0.0), "on: the white slab");
    let on_tone = descendants_with::<UiText>(&mut app, tog).into_iter().find(|t| t.tone == Tone::Ink);
    assert!(on_tone.is_some());
    assert_eq!(texts_under(&mut app, tog), ["OFF", "ON"]);
    let opts = build(&mut app, |ui, c, root| ui.segmented(c, root, &["None", "60", "144"], Some(1), State::NONE));
    assert_eq!(opts.len(), 3);
    let theme = app.world().resource::<Theme>().clone();
    assert_eq!(app.world().get::<BackgroundColor>(opts[1]).unwrap().0, theme.tone_a(Tone::Fg, 1.0));
    assert_eq!(app.world().get::<BackgroundColor>(opts[0]).unwrap().0, Color::NONE);
}

/// Slider values: a press's place on the track to a value on its step,
/// and back.
#[test]
fn slider_maps_presses_to_steps() {
    assert_eq!(controls::value_at(0.5, 75.0, 90.0, 1.0), 83.0);
    assert_eq!(controls::value_at(0.0, 0.1, 20.0, 0.1), 0.1);
    assert_eq!(controls::value_at(1.2, 0.0, 1.0, 0.05), 1.0);
    assert!((controls::frac_of(2.2, 1.6, 2.6) - 0.6).abs() < 1e-5);
    let pos = bevy::ui::RelativeCursorPosition { cursor_over: true, normalized: Some(Vec2::new(0.25, 0.0)) };
    assert_eq!(controls::slider_frac(&pos), Some(0.75));
    let mut app = app();
    let tr = build(&mut app, |ui, c, root| ui.slider(c, root, 0.4, Some(1.0), "83", 15.0, State::focused(true)));
    assert!(app.world().get::<bevy::ui::RelativeCursorPosition>(tr).is_some());
    let slants = descendants_with::<UiSlant>(&mut app, tr);
    assert_eq!(slants.iter().filter(|s| s.fill == Tone::Acc).count(), 1, "focus: the accent thumb");
}

/// The dropdown lists its options only open; the key field shows caps,
/// or the listening line.
#[test]
fn dropdown_and_key_field() {
    let mut app = app();
    let (_, shut) = build(&mut app, |ui, c, root| ui.dropdown(c, root, &["Party playlist", "None"], 0, None, 12.0, State::NONE));
    assert!(shut.is_empty());
    let (f, open) = build(&mut app, |ui, c, root| ui.dropdown(c, root, &["Party playlist", "None"], 0, Some(1), 12.0, State::NONE));
    assert_eq!(open.len(), 2);
    assert_eq!(texts_under(&mut app, f), ["PARTY PLAYLIST"]);
    let k = build(&mut app, |ui, c, root| ui.key_field(c, root, &["SPACE".into(), "MWHEELDOWN".into()], false, 8.5, State::NONE));
    assert_eq!(texts_under(&mut app, k), ["SPACE", "MWHEELDOWN"]);
    let l = build(&mut app, |ui, c, root| ui.key_field(c, root, &[], true, 8.5, State::NONE));
    assert!(texts_under(&mut app, l)[0].starts_with("PRESS A KEY"));
}

/// A wipe cover clips away along the slant over its time and is gone at
/// the end; one already over isn't built.
#[test]
fn wipe_cover_clears_and_goes() {
    let mut app = app();
    let now = app.world().resource::<UiClock>().now;
    let cover = build(&mut app, |ui, c, root| ui.wipe_cover(c, root, Tone::Ink, now, now, 0.3)).unwrap();
    assert!(app.world().get::<Wipe>(cover).is_some());
    app.world_mut().resource_mut::<UiClock>().step(0.15);
    app.update();
    let tl = app.world().get::<chimera_ui::Timeline>(cover).unwrap();
    let mid = tl.value_or(chimera_ui::Prop::ClipLeft, 0.0);
    assert!(mid > 0.3 && mid < 1.0, "half-way clipped: {mid}");
    app.world_mut().resource_mut::<UiClock>().step(0.3);
    app.update();
    app.update();
    assert!(app.world().get_entity(cover).is_err(), "gone when done");
    let none = build(&mut app, |ui, c, root| ui.wipe_cover(c, root, Tone::Ink, now, now + 1.0, 0.3));
    assert!(none.is_none());
}
