//! The form controls (chimera_ui::controls): what each builds, its states,
//! hover from the pointer, the wipe.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_ui::{
    ChimeraUiPlugin, KitFonts, Role, Theme, Tone, UiClock, UiSlant, UiText, UiUnits,
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

/// Text on white is ink at full strength and one pixel heavier (the
/// `InkWeight` shadow of its own colour): the selected segment, a toggle's
/// ON, the primary button's label and its key cap. Light text gets none,
/// and loses it when its tone turns light.
#[test]
fn text_on_white_is_full_strength_ink() {
    use chimera_ui::style::InkWeight;
    let mut app = app();
    let theme = app.world().resource::<Theme>().clone();
    let ink = theme.tone(Tone::Ink);
    let b = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Primary, "Start", Some("Enter"), 2.0, State::NONE));
    let seg = build(&mut app, |ui, c, root| ui.segmented(c, root, &["Windowed", "Full screen"], Some(0), State::NONE));
    let tog = build(&mut app, |ui, c, root| ui.toggle(c, root, true, State::NONE));
    let colours = |app: &mut App, e: Entity| -> Vec<(String, Color, Option<TextShadow>)> {
        let mut out = Vec::new();
        let mut stack = vec![e];
        let w = app.world();
        while let Some(n) = stack.pop() {
            if let (Some(t), Some(c)) = (w.get::<Text>(n), w.get::<TextColor>(n)) {
                out.push((t.0.clone(), c.0, w.get::<TextShadow>(n).copied()));
            }
            if let Some(ch) = w.get::<Children>(n) {
                stack.extend(ch.iter());
            }
        }
        out
    };
    let mut on_white = colours(&mut app, b);
    on_white.extend(colours(&mut app, seg[0]));
    on_white.extend(colours(&mut app, tog).into_iter().filter(|(t, ..)| t == "ON"));
    let mut names: Vec<&str> = on_white.iter().map(|(t, ..)| t.as_str()).collect();
    names.sort();
    assert_eq!(names, ["ENTER", "ON", "START", "WINDOWED"]);
    for (t, c, shadow) in &on_white {
        assert_eq!(*c, ink, "{t}: ink at full strength");
        let s = shadow.unwrap_or_else(|| panic!("{t}: the ink weight"));
        assert_eq!(s.color, ink);
        assert!(s.offset.x > 0.0 && s.offset.y == 0.0, "{t}: one pixel to the right");
    }
    // The cap's outline is full strength too.
    let cap_outline = descendants_with::<chimera_ui::UiStyle>(&mut app, b).into_iter().find_map(|s| s.outline);
    assert_eq!(cap_outline, Some((Tone::Ink, 1.0)));
    // Light text: no weight; a tone turning light drops it.
    for (t, _, shadow) in colours(&mut app, seg[1]) {
        assert!(shadow.is_none(), "{t}: light text keeps its own weight");
    }
    let e = app.world_mut().spawn((Text::new("X"), UiText::new(chimera_ui::Role::Mono, 0.6, Tone::Ink))).id();
    app.update();
    assert!(app.world().get::<InkWeight>(e).is_some());
    app.world_mut().get_mut::<UiText>(e).unwrap().tone = Tone::Fg;
    app.update();
    assert!(app.world().get::<InkWeight>(e).is_none() && app.world().get::<TextShadow>(e).is_none());
}

/// A focused stepper's underline sits under its value with a gap, as the
/// slider's does: its value box is as tall as the keys (1.7 cq), not the
/// text's line.
#[test]
fn stepper_underline_clears_the_value() {
    let mut app = app();
    let (minus, _) = build(&mut app, |ui, c, root| ui.stepper(c, root, "Rotation · 138 maps", 13.0, State::focused(true)));
    let group = app.world().get::<ChildOf>(minus).unwrap().parent();
    let out = app.world().get::<Children>(group).unwrap()[1];
    let px = |v: f32| (v * 19.2).round();
    assert_eq!(app.world().get::<Node>(out).unwrap().height, Val::Px(px(1.7)));
    // The value's text is smaller than its box: room above the underline.
    let text = descendants_with::<UiText>(&mut app, out);
    assert!(text[0].size < 1.7 - 2.0 * 0.18, "the text clears the .18 cq underline");
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

/// A text field shows its text (stars when masked) split at the caret,
/// the placeholder when empty and not typed into.
#[test]
fn text_field_shows_text_caret_and_placeholder() {
    let mut app = app();
    let caret_count = |app: &mut App, e: Entity| descendants_with::<controls::TextCaret>(app, e).len();
    let f = build(&mut app, |ui, c, root| ui.text_field(c, root, "", "host or host:port", None, false, 15.0, State::NONE));
    assert_eq!(texts_under(&mut app, f), ["HOST OR HOST:PORT"]);
    assert_eq!(caret_count(&mut app, f), 0);
    // What is typed keeps its case (the placeholder is a label).
    let f = build(&mut app, |ui, c, root| ui.text_field(c, root, "Lan:27016", "", Some(3.into()), false, 15.0, State::focused(true)));
    assert_eq!(texts_under(&mut app, f), ["Lan", ":27016"]);
    assert_eq!(caret_count(&mut app, f), 1);
    let f = build(&mut app, |ui, c, root| ui.text_field(c, root, "pässwd", "", None, true, 15.0, State::NONE));
    assert_eq!(texts_under(&mut app, f), ["******"]);
}

/// Typed text keeps its case in our sentence face (`Role::Text`, Display
/// Regular): no system face, no fallback list; a sentence and a cvar name
/// in Mono keep their case too, a label does not.
#[test]
fn typed_text_keeps_its_case_in_our_face() {
    use bevy::text::FontSource;
    let mut app = app();
    let f = build(&mut app, |ui, c, root| ui.text_field(c, root, "Zoë's Server", "", None, false, 15.0, State::NONE));
    app.update();
    assert_eq!(texts_under(&mut app, f), ["Zoë's Server"]);
    let kit = app.world().resource::<KitFonts>().clone();
    let styled = descendants_with::<UiText>(&mut app, f);
    assert!(styled.iter().all(|t| t.role == Role::Text), "{styled:?}");
    let fonts: Vec<TextFont> = descendants_with::<TextFont>(&mut app, f);
    assert!(!fonts.is_empty());
    for font in fonts {
        assert_eq!(font.font, FontSource::Handle(kit.display_regular.clone().unwrap()), "typed text in UGG Display Regular");
    }
    let s = build(&mut app, |ui, c, root| ui.sentence(c, root, controls::SENTENCE, Tone::Fg2, "Paused for you only"));
    let v = build(&mut app, |ui, c, root| ui.text_plain(c, root, Role::Mono, 0.6, Tone::Fg, "fov_desired"));
    let l = build(&mut app, |ui, c, root| ui.text(c, root, Role::Mono, 0.6, Tone::Fg, "Console"));
    app.update();
    assert_eq!(texts_under(&mut app, s), ["Paused for you only"]);
    assert_eq!(texts_under(&mut app, v), ["fov_desired"]);
    assert_eq!(texts_under(&mut app, l), ["CONSOLE"]);
}

/// A text in an upper-casing role shows in capitals however a screen sets
/// it later (`style::case_texts`), unless it keeps its case (`KeepCase`).
#[test]
fn upper_roles_stay_in_capitals_when_set_later() {
    let mut app = app();
    let label = app.world_mut().spawn((Text::new("x"), UiText::new(Role::DisplayBold, 1.0, Tone::Fg))).id();
    let kept = app.world_mut().spawn((Text::new("x"), UiText::new(Role::Mono, 0.6, Tone::Fg), chimera_ui::KeepCase)).id();
    let sentence = app.world_mut().spawn((Text::new("x"), UiText::new(Role::Text, 0.86, Tone::Fg))).id();
    app.update();
    for e in [label, kept, sentence] {
        app.world_mut().get_mut::<Text>(e).unwrap().0 = "Waiting for 2 players".into();
    }
    app.update();
    let text = |app: &App, e| app.world().get::<Text>(e).unwrap().0.clone();
    assert_eq!(text(&app, label), "WAITING FOR 2 PLAYERS");
    assert_eq!(text(&app, kept), "Waiting for 2 players");
    assert_eq!(text(&app, sentence), "Waiting for 2 players");
}

/// The `TextSelection` nodes under `e`.
fn selections(app: &mut App, e: Entity) -> Vec<Entity> {
    let w = app.world_mut();
    let mut out = Vec::new();
    let mut stack = vec![e];
    while let Some(n) = stack.pop() {
        if w.get::<controls::TextSelection>(n).is_some() {
            out.push(n);
        }
        if let Some(ch) = w.get::<Children>(n) {
            stack.extend(ch.iter());
        }
    }
    out
}

/// A selection shows: its text on an accent tint (`TextSelection`), the
/// caret at its end; a masked field's selection covers its stars.
#[test]
fn text_field_shows_its_selection() {
    let mut app = app();
    let f = build(&mut app, |ui, c, root| {
        ui.text_field(c, root, "Lan:27016", "", Some(controls::TextCursor { at: 3, anchor: 0 }), false, 15.0, State::focused(true))
    });
    assert_eq!(texts_under(&mut app, f), ["Lan", ":27016"]);
    let sel = selections(&mut app, f);
    assert_eq!(sel.len(), 1);
    assert_eq!(texts_under(&mut app, sel[0]), ["Lan"], "the selected text is on the tint");
    assert_eq!(descendants_with::<controls::TextCaret>(&mut app, f).len(), 1);
    // "ä" is two bytes: the selection's ends are byte offsets.
    let f = build(&mut app, |ui, c, root| {
        ui.text_field(c, root, "pässwd", "", Some(controls::TextCursor { at: 1, anchor: 4 }), true, 15.0, State::focused(true))
    });
    let sel = selections(&mut app, f);
    assert_eq!(texts_under(&mut app, sel[0]), ["**"], "two chars selected, as stars");
    assert_eq!(texts_under(&mut app, f), ["*", "**", "***"]);
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

/// A typed number: any finite one, inside the hard limits only.
#[test]
fn a_typed_number_is_checked_only_against_hard_limits() {
    let hard = (Some(0.0), None);
    assert_eq!(controls::typed_number(" 2.5 ", hard), Ok(2.5), "past a slider's range is fine");
    assert_eq!(controls::typed_number("-1", hard), Err("At least 0".into()));
    assert_eq!(controls::typed_number("abc", hard), Err("Not a number".into()));
    assert_eq!(controls::typed_number("inf", (None, None)), Err("Not a number".into()));
    assert_eq!(controls::typed_number("NaN", (None, None)), Err("Not a number".into()));
    assert_eq!(controls::typed_number("200", (None, Some(179.0))), Err("At most 179".into()));
}

/// The slider's value box is clickable; typing, it is a text field with
/// the caret, refused text underlined in threat.
#[test]
fn a_sliders_value_box_types() {
    let mut app = app();
    let p = build(&mut app, |ui, c, root| ui.slider_ex(c, root, 0.5, None, "0.5", 15.0, State::NONE, None));
    assert!(app.world().get::<Interaction>(p.value).is_some(), "clickable");
    assert_eq!(texts_under(&mut app, p.value), ["0.5"]);
    let typing = controls::Typing { text: "1.6", caret: 3, anchor: 3, error: true };
    let p = build(&mut app, |ui, c, root| ui.slider_ex(c, root, 1.0, None, "1.6", 15.0, State::NONE, Some(typing)));
    assert_eq!(texts_under(&mut app, p.value), ["1.6"], "as typed");
    assert_eq!(descendants_with::<controls::TextCaret>(&mut app, p.value).len(), 1, "the caret");
    let s = build(&mut app, |ui, c, root| ui.stepper_ex(c, root, "8", 4.0, State::NONE, None));
    assert!(app.world().get::<Interaction>(s.value).is_some(), "a stepper's value too");
}
