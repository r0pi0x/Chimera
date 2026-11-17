//! The form controls (chimera_ui::controls): what each builds, its states,
//! hover from the pointer, the wipe.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_ui::{
    motion::{Motion, Pace},
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
    let kit = app.world().get_resource::<chimera_ui::KitFonts>().cloned();
    let ui = Ui::new(&theme, kit.as_ref(), units);
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

/// The texts under `e` run together (a field's characters, one node
/// each).
fn joined(app: &mut App, e: Entity) -> String {
    texts_under(app, e).concat()
}

fn descendants_with_entity<C: Component>(app: &mut App, e: Entity) -> Vec<Entity> {
    let w = app.world_mut();
    let mut out = Vec::new();
    let mut stack = vec![e];
    while let Some(n) = stack.pop() {
        if w.get::<C>(n).is_some() {
            out.push(n);
        }
        if let Some(ch) = w.get::<Children>(n) {
            stack.extend(ch.iter());
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

/// A primary button is the hero: the cobalt (`hero`) slab on the slant,
/// white text, its key cap; no tick, focused or hovered (the kit's design
/// rules: a button's hover is its own lift, its focus a bar); hover lifts
/// the slab to `acc`.
#[test]
fn buttons_carry_their_kind_and_focus() {
    let mut app = app();
    let b = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Primary, "Apply", Some("Ctrl S"), 2.0, State::focused(true)));
    assert_eq!(texts_under(&mut app, b), ["APPLY", "CTRL S"]);
    let slants = descendants_with::<UiSlant>(&mut app, b);
    assert!(slants.iter().any(|s| s.fill == Tone::Hero && s.edges == (false, true)), "the hero's slab leans at the right");
    let ticks = descendants_with::<Tick>(&mut app, b);
    assert!(ticks.is_empty(), "no tick on a button");
    let texts = descendants_with::<UiText>(&mut app, b);
    assert!(texts.iter().all(|t| t.tone == Tone::OnHero), "white on the hero");
    assert!(texts.iter().any(|t| t.role == Role::DisplayRegular), "button labels are words: Display");
    assert!(texts.iter().any(|t| t.role == Role::Mono), "key caps stay Mono");
    assert_eq!(app.world().resource::<Theme>().tone(Tone::Hero).to_srgba().to_hex(), "#2D6BEB");
    *app.world_mut().get_mut::<Interaction>(b).unwrap() = Interaction::Hovered;
    app.update();
    assert!(descendants_with::<UiSlant>(&mut app, b).iter().any(|s| s.fill == Tone::Acc), "hover lifts the slab");
    // The big hero (Start, Connect, a dialog's) is bold and bigger.
    let big = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Primary, "Start", Some("Enter"), controls::HERO_H, State::NONE));
    let t = descendants_with::<UiText>(&mut app, big);
    assert!(t.iter().any(|t| t.role == Role::DisplayBold && t.size > 0.85), "{t:?}");
    // A disabled hero stays a slab.
    let off = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Primary, "Apply", None, 2.0, State { disabled: true, ..State::NONE }));
    assert!(descendants_with::<UiSlant>(&mut app, off).iter().any(|s| s.fill == Tone::Fg && s.alpha < 0.1));
    // Danger: the threat mark, never a threat fill.
    let d = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Danger, "End the game", None, 2.4, State::NONE));
    let slants = descendants_with::<UiSlant>(&mut app, d);
    assert!(slants.iter().any(|s| s.fill == Tone::Threat), "the threat mark");
    let bg = app.world().get::<BackgroundColor>(d).unwrap().0;
    assert_ne!(bg, app.world().resource::<Theme>().tone(Tone::Threat));
}

/// Hover is the row's own fill lifting, live from the pointer, back off
/// when it leaves; no tick on a row, focused or not (focus: the bar).
#[test]
fn hover_lifts_the_fill_and_shows_no_tick() {
    let mut app = app();
    let (row, _) = build(&mut app, |ui, c, root| ui.row(c, root, "Field of view", None, 2.9, State::NONE));
    assert!(descendants_with::<Tick>(&mut app, row).is_empty(), "no tick");
    let hover = *app.world().get::<Hover>(row).unwrap();
    *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::Hovered;
    app.update();
    assert_eq!(app.world().get::<BackgroundColor>(row).unwrap().0, hover.over);
    *app.world_mut().get_mut::<Interaction>(row).unwrap() = Interaction::None;
    app.update();
    assert_eq!(app.world().get::<BackgroundColor>(row).unwrap().0, hover.base);
    let (focused, _) = build(&mut app, |ui, c, root| ui.row(c, root, "Field of view", None, 2.9, State::focused(true)));
    assert!(descendants_with::<Tick>(&mut app, focused).is_empty(), "focused: the bar, no tick");
    let k = build(&mut app, |ui, c, root| ui.key_field(c, root, &["SPACE".into()], false, 8.5, State::NONE));
    assert!(descendants_with::<Tick>(&mut app, k).is_empty(), "no tick on a key field");
    let (_, opts) = build(&mut app, |ui, c, root| ui.dropdown(c, root, &["A", "B"], 0, Some(1), 12.0, State::NONE));
    for o in opts {
        assert!(descendants_with::<Tick>(&mut app, o).is_empty(), "no tick on a list line");
    }
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
    // Segmented (slanted): the pick a white slanted slab, the others none.
    assert_eq!(descendants_with_entity::<controls::SegmentedPick>(&mut app, opts[1]).len(), 1);
    assert!(descendants_with_entity::<controls::SegmentedPick>(&mut app, opts[0]).is_empty());
    let pick = descendants_with::<UiSlant>(&mut app, opts[1]);
    assert!(pick.iter().any(|s| s.fill == Tone::Fg && s.outline == 0.0), "the pick is a UiSlant");
    // Words in Display, numbers in Mono.
    let roles: Vec<Role> = descendants_with::<UiText>(&mut app, opts[0]).iter().map(|t| t.role).collect();
    assert_eq!(roles, [Role::DisplayRegular], "NONE is a word");
    let roles: Vec<Role> = descendants_with::<UiText>(&mut app, opts[1]).iter().map(|t| t.role).collect();
    assert_eq!(roles, [Role::Mono], "60 is a number");
}

/// Text on white is ink at full strength and one pixel heavier (the
/// `InkWeight` shadow of its own colour): the selected segment, a toggle's
/// ON (the hero's label is white on cobalt). Light text gets none,
/// and loses it when its tone turns light.
#[test]
fn text_on_white_is_full_strength_ink() {
    use chimera_ui::style::InkWeight;
    let mut app = app();
    let theme = app.world().resource::<Theme>().clone();
    let ink = theme.tone(Tone::Ink);
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
    let mut on_white = colours(&mut app, seg[0]);
    on_white.extend(colours(&mut app, tog).into_iter().filter(|(t, ..)| t == "ON"));
    let mut names: Vec<&str> = on_white.iter().map(|(t, ..)| t.as_str()).collect();
    names.sort();
    assert_eq!(names, ["ON", "WINDOWED"]);
    for (t, c, shadow) in &on_white {
        assert_eq!(*c, ink, "{t}: ink at full strength");
        let s = shadow.unwrap_or_else(|| panic!("{t}: the ink weight"));
        assert_eq!(s.color, ink);
        assert!(s.offset.x > 0.0 && s.offset.y == 0.0, "{t}: one pixel to the right");
    }
    // A key cap is full strength (the lockup: no .75 alpha).
    let b = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Secondary, "Revert", Some("Ctrl Z"), 2.0, State::NONE));
    let cap_outline = descendants_with::<chimera_ui::UiStyle>(&mut app, b).into_iter().filter_map(|s| s.outline).last();
    assert!(cap_outline.is_some_and(|(_, a)| a == 1.0), "{cap_outline:?}");
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
    assert_eq!(joined(&mut app, f), "Lan:27016");
    assert_eq!(caret_count(&mut app, f), 1);
    let f = build(&mut app, |ui, c, root| ui.text_field(c, root, "pässwd", "", None, true, 15.0, State::NONE));
    assert_eq!(joined(&mut app, f), "******");
    // No hover tick on a text field (the kit's design rules): its own
    // tint shows hover.
    assert!(descendants_with::<Tick>(&mut app, f).is_empty());
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
    assert_eq!(joined(&mut app, f).replace('\u{a0}', " "), "Zoë's Server");
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
    assert_eq!(joined(&mut app, f), "Lan:27016");
    let sel = selections(&mut app, f);
    assert_eq!(sel.len(), 3, "a tint behind each selected character");
    let on: String = sel.iter().map(|e| joined(&mut app, *e)).collect::<Vec<_>>().concat();
    assert_eq!(on.chars().count(), 3);
    assert_eq!(descendants_with::<controls::TextCaret>(&mut app, f).len(), 1);
    // "ä" is two bytes: the selection's ends are byte offsets.
    let f = build(&mut app, |ui, c, root| {
        ui.text_field(c, root, "pässwd", "", Some(controls::TextCursor { at: 1, anchor: 4 }), true, 15.0, State::focused(true))
    });
    let sel = selections(&mut app, f);
    assert_eq!(sel.len(), 2, "two chars selected, as stars");
    assert_eq!(joined(&mut app, f), "******");
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
    assert_eq!(joined(&mut app, p.value), "0.5");
    let typing = controls::Typing { text: "1.6", caret: 3, anchor: 3, error: true, suffix: None };
    let p = build(&mut app, |ui, c, root| ui.slider_ex(c, root, 1.0, None, "1.6", 15.0, State::NONE, Some(typing)));
    assert_eq!(joined(&mut app, p.value), "1.6", "as typed");
    assert_eq!(descendants_with::<controls::TextCaret>(&mut app, p.value).len(), 1, "the caret");
    let s = build(&mut app, |ui, c, root| ui.stepper_ex(c, root, "8", 4.0, State::NONE, None));
    assert!(app.world().get::<Interaction>(s.value).is_some(), "a stepper's value too");
}

// ---------------------------------------------------------------------
// Laid out by Bevy's own UI (a window and a camera, no renderer): the
// typed text's places, the caret, a click, the toggle.

/// `app()` with Bevy's UI layout, a 1920 x 1080 window (scale 1) and a
/// camera on it.
const KIT_FILES: chimera_ui::FontFiles<'static> = chimera_ui::fonts::OWN;

fn laid_out() -> App {
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo, Viewport};
    let mut app = app();
    app.init_resource::<Assets<bevy::image::TextureAtlasLayout>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .init_resource::<Touches>()
        .init_resource::<bevy::picking::hover::HoverMap>()
        .init_resource::<bevy::picking::pointer::PointerMap>()
        .init_resource::<bevy::picking::events::PointerState>()
        .add_message::<bevy::picking::pointer::PointerInput>()
        .add_message::<bevy::picking::backend::PointerHits>()
        .init_asset::<Image>()
        .init_asset::<Mesh>()
        .init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.add_plugins((bevy::ui::UiPlugin, bevy::camera::visibility::VisibilityPlugin));
    let mut window = Window::default();
    window.resolution.set(1920.0, 1080.0);
    window.resolution.set_scale_factor_override(Some(1.0));
    app.world_mut().spawn((window, bevy::window::PrimaryWindow));
    let size = UVec2::new(1920, 1080);
    app.world_mut().spawn((
        Camera2d,
        Camera {
            computed: ComputedCameraValues { target_info: Some(RenderTargetInfo { physical_size: size, scale_factor: 1.0 }), ..default() },
            viewport: Some(Viewport { physical_size: size, ..default() }),
            ..default()
        },
    ));
    // The Ultimate Gun Game's faces (the value boxes' Mono), in before the
    // first build (a text laid out).
    let kit = chimera_ui::KitFonts::load(&mut app.world_mut().resource_mut::<Assets<Font>>(), KIT_FILES);
    app.insert_resource(kit);
    app.world_mut().spawn((Text::new("0"), Node { position_type: PositionType::Absolute, ..default() }));
    for _ in 0..5 {
        app.update();
    }
    app
}

/// `build` and two more frames (layout, then the transforms read).
fn build_laid<R>(app: &mut App, f: impl FnOnce(&Ui, &mut Commands, Entity) -> R) -> R {
    let r = build(app, f);
    // A new text's font list resolves over a few frames.
    for _ in 0..5 {
        app.update();
    }
    r
}

/// Each character's centre x (physical px) of a field or value box.
fn char_xs(app: &App, field: Entity) -> Vec<f32> {
    let chars = &app.world().get::<controls::EditText>(field).expect("an EditText").chars;
    chars.iter().map(|e| app.world().get::<bevy::ui::UiGlobalTransform>(*e).unwrap().affine().translation.x).collect()
}

fn size_of(app: &App, e: Entity) -> Vec2 {
    app.world().get::<bevy::ui::ComputedNode>(e).unwrap().size
}

fn centre_of(app: &App, e: Entity) -> Vec2 {
    app.world().get::<bevy::ui::UiGlobalTransform>(e).unwrap().affine().translation
}

/// The UiText of every text under `e`.
fn faces(app: &mut App, e: Entity) -> Vec<(chimera_ui::Role, u32)> {
    descendants_with::<UiText>(app, e).iter().map(|t| (t.role, (t.size * 1000.0).round() as u32)).collect()
}

/// A value box typed into keeps its box (the same size and place), its
/// face and size, and its characters where they were at rest: clicking it
/// moves nothing. The caret takes no room: its place never moves a
/// character.
#[test]
fn a_value_box_typed_into_keeps_its_box_face_and_places() {
    let mut app = laid_out();
    let rest = build_laid(&mut app, |ui, c, root| ui.slider_ex(c, root, 0.8, None, "160", 15.0, State::focused(true), None)).value;
    let typing = |caret: usize, anchor: usize| controls::Typing { text: "160", caret, anchor, error: false, suffix: None };
    let all = build_laid(&mut app, |ui, c, root| ui.slider_ex(c, root, 0.8, None, "160", 15.0, State::focused(true), Some(typing(3, 0)))).value;
    assert_eq!(size_of(&app, rest), size_of(&app, all), "the same box");
    assert_eq!(centre_of(&app, rest), centre_of(&app, all), "in the same place");
    let face_rest = faces(&mut app, rest);
    assert!(!face_rest.is_empty());
    assert_eq!(face_rest, faces(&mut app, all), "the same role and size");
    assert_eq!(char_xs(&app, rest), char_xs(&app, all), "each character where it was");
    // The caret at each place: the characters stay.
    let at0 = build_laid(&mut app, |ui, c, root| ui.slider_ex(c, root, 0.8, None, "160", 15.0, State::focused(true), Some(typing(0, 0)))).value;
    let at2 = build_laid(&mut app, |ui, c, root| ui.slider_ex(c, root, 0.8, None, "160", 15.0, State::focused(true), Some(typing(2, 2)))).value;
    assert_eq!(char_xs(&app, at0), char_xs(&app, rest));
    assert_eq!(char_xs(&app, at2), char_xs(&app, rest));
    assert_eq!(size_of(&app, at2), size_of(&app, rest));
    // The caret sits on the boundary between the second and third.
    let caret = {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, With<controls::TextCaret>>();
        q.iter(w).filter(|e| is_under(w, *e, at2)).collect::<Vec<_>>()
    };
    assert_eq!(caret.len(), 1);
    assert_eq!(app.world().get::<Node>(caret[0]).unwrap().position_type, PositionType::Absolute, "it takes no room");
    let xs = char_xs(&app, rest);
    let cx = centre_of(&app, caret[0]).x;
    assert!(cx > xs[1] && cx < xs[2], "between the 2nd and 3rd: {cx} in {xs:?}");
    // A percentage's suffix stays after the typed number.
    let pct = build_laid(&mut app, |ui, c, root| {
        ui.slider_ex(c, root, 0.8, None, "160%", 15.0, State::NONE, Some(controls::Typing { suffix: Some("%"), ..typing(3, 3) }))
    })
    .value;
    assert_eq!(joined(&mut app, pct), "160%");
    let pct_rest = build_laid(&mut app, |ui, c, root| ui.slider_ex(c, root, 0.8, None, "160%", 15.0, State::NONE, None)).value;
    assert_eq!(size_of(&app, pct), size_of(&app, pct_rest), "a percentage's box keeps its size too");
}

fn is_under(w: &World, mut e: Entity, root: Entity) -> bool {
    while let Some(p) = w.get::<ChildOf>(e) {
        if p.parent() == root {
            return true;
        }
        e = p.parent();
    }
    false
}

/// The pointer at physical x `x` (y in the middle of `e`).
fn point(app: &mut App, x: f32, e: Entity) {
    let y = centre_of(app, e).y;
    let w = app.world_mut();
    let mut q = w.query::<&mut Window>();
    q.single_mut(w).unwrap().set_physical_cursor_position(Some(bevy::math::DVec2::new(x as f64, y as f64)));
    app.update();
}

fn char_at(app: &mut App, field: Entity) -> Option<usize> {
    use bevy::ecs::system::RunSystemOnce;
    app.world_mut().run_system_once(move |h: controls::TextHits| h.char_at(field)).unwrap()
}

/// A click places the caret at the boundary nearest the pointer (from
/// the layout): before the first character, between any two, after the
/// last; a text field's text alike.
#[test]
fn a_click_places_the_caret_between_characters() {
    let mut app = laid_out();
    let f = build_laid(&mut app, |ui, c, root| ui.text_field(c, root, "Lan:27016", "", Some(9.into()), false, 15.0, State::focused(true)));
    let xs = char_xs(&app, f);
    assert_eq!(xs.len(), 9);
    point(&mut app, xs[0] - 6.0, f);
    assert_eq!(char_at(&mut app, f), Some(0), "left of the text");
    point(&mut app, (xs[3] + xs[4]) / 2.0, f);
    assert_eq!(char_at(&mut app, f), Some(4), "between the 4th and 5th");
    point(&mut app, xs[3] - 1.0, f);
    assert_eq!(char_at(&mut app, f), Some(3), "just left of the 4th's middle");
    point(&mut app, xs[8] + 200.0, f);
    assert_eq!(char_at(&mut app, f), Some(9), "past the end");
    // A value box too.
    let v = build_laid(&mut app, |ui, c, root| ui.stepper_ex(c, root, "8.5", 4.0, State::NONE, None)).value;
    let xs = char_xs(&app, v);
    point(&mut app, (xs[0] + xs[1]) / 2.0, v);
    assert_eq!(char_at(&mut app, v), Some(1));
}

/// The caret is thin (one physical pixel at 1080 lines) and sits just
/// after the character before it, not over it.
#[test]
fn the_caret_is_thin_and_just_after_the_text() {
    let mut app = laid_out();
    let f = build_laid(&mut app, |ui, c, root| ui.text_field(c, root, "Lan", "", Some(3.into()), false, 15.0, State::focused(true)));
    let caret = descendants_with_entity::<controls::TextCaret>(&mut app, f);
    assert_eq!(caret.len(), 1);
    let cw = size_of(&app, caret[0]).x;
    assert!(cw <= 1.01, "one physical px wide, not {cw}");
    let last = *app.world().get::<controls::EditText>(f).unwrap().chars.last().unwrap();
    let right = centre_of(&app, last).x + size_of(&app, last).x / 2.0;
    let left = centre_of(&app, caret[0]).x - cw / 2.0;
    assert!((left - right).abs() <= 0.51, "the caret's left {left} at the text's end {right}");
}

/// The pointer's gestures in a text: a press places, a drag selects from
/// it, a second press soon after near the first selects all.
#[test]
fn a_press_places_a_drag_selects_a_double_press_selects_all() {
    let mut p = controls::TextPress::default();
    assert_eq!(p.press(10.0, 2, 5), (2, 2));
    assert_eq!(p.drag(4), Some((4, 2)), "dragged right from 2");
    p.release();
    assert_eq!(p.drag(1), None, "let go");
    assert_eq!(p.press(10.2, 2, 5), (5, 0), "a double press: all of it");
    assert_eq!(p.press(11.0, 3, 5), (3, 3));
    assert_eq!(p.press(11.0 + controls::DOUBLE_CLICK_SECS + 0.1, 3, 5), (3, 3), "too slow for a double");
    assert_eq!(p.press(20.0, 0, 5), (0, 0));
    assert_eq!(p.press(20.1, 4, 5), (4, 4), "too far for a double");
}

/// The toggle: OFF and ON each centred on its slab's place (within a px),
/// so ON sits as OFF does; flipped, its slab slides over in about 140 ms
/// (eased; jumps under reduced motion); focus is a strip on the track's
/// bottom edge, no straight underline.
#[test]
fn the_toggles_labels_are_centred_and_its_slab_slides() {
    let mut app = laid_out();
    let t = build_laid(&mut app, |ui, c, root| ui.toggle(c, root, true, State::focused(true)));
    let units = *app.world().resource::<UiUnits>();
    let left = centre_of(&app, t).x - size_of(&app, t).x / 2.0;
    let labels: Vec<Entity> = {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, With<controls::ToggleLabel>>();
        q.iter(w).filter(|e| is_under(w, *e, t)).collect()
    };
    assert_eq!(labels.len(), 2);
    let mut offs = Vec::new();
    for l in labels {
        let text = descendants_of_text(&mut app, l);
        let tc = centre_of(&app, text).x;
        let want = left + if joined(&mut app, l) == "OFF" {
            units.cq(controls::TOGGLE_OFF_LEFT + controls::TOGGLE_SLAB_W / 2.0)
        } else {
            units.cq(controls::TOGGLE_ON_LEFT + controls::TOGGLE_SLAB_W / 2.0)
        };
        offs.push(tc - want);
        assert!((tc - want).abs() <= 1.0, "{} centred on its slab: {tc} vs {want}", joined(&mut app, l));
    }
    assert!((offs[0] - offs[1]).abs() <= 1.0, "ON sits as OFF does: {offs:?}");
    assert_eq!(descendants_with::<controls::ToggleFocus>(&mut app, t).len(), 1, "focus: the strip");
    assert!(descendants_with::<UiSlant>(&mut app, t).iter().any(|s| s.fill == Tone::Acc), "the strip is slanted, accent");

    // The slide.
    let now = app.world().resource::<UiClock>().now;
    let slide = controls::ToggleSlide { since: now, now, motion: Motion::default() };
    let t = build(&mut app, |ui, c, root| ui.toggle_ex(c, root, true, State::NONE, Some(slide)));
    let slab = descendants_entities::<controls::ToggleSlab>(&mut app, t)[0];
    let x = |app: &App| match app.world().get::<UiTransform>(slab).unwrap().translation.x {
        Val::Px(v) => v,
        _ => 0.0,
    };
    let start = x(&app);
    let off = units.cq(controls::TOGGLE_OFF_LEFT - controls::TOGGLE_ON_LEFT);
    assert!((start - off).abs() < 1.0, "starts at the off place: {start} vs {off}");
    app.world_mut().resource_mut::<UiClock>().step(0.07);
    app.update();
    let mid = x(&app);
    assert!(mid > off && mid < 0.0, "half-way it is between: {mid}");
    app.world_mut().resource_mut::<UiClock>().step(0.2);
    app.update();
    assert_eq!(x(&app), 0.0, "at rest on the on side after the normal pace");
    let reduced = controls::ToggleSlide { motion: Motion::new(0.0), ..slide };
    let r = build(&mut app, |ui, c, root| ui.toggle_ex(c, root, true, State::NONE, Some(reduced)));
    let rs = descendants_entities::<controls::ToggleSlab>(&mut app, r)[0];
    assert!(app.world().get::<chimera_ui::Timeline>(rs).is_none(), "reduced motion: it jumps");
}

fn descendants_entities<C: Component>(app: &mut App, e: Entity) -> Vec<Entity> {
    let w = app.world_mut();
    let mut out = Vec::new();
    let mut stack = vec![e];
    while let Some(n) = stack.pop() {
        if w.get::<C>(n).is_some() {
            out.push(n);
        }
        if let Some(ch) = w.get::<Children>(n) {
            stack.extend(ch.iter());
        }
    }
    out
}

fn descendants_of_text(app: &mut App, e: Entity) -> Entity {
    descendants_entities::<Text>(app, e)[0]
}

/// The dropdown's mark is the kit's chevron icon, pointing down, up while
/// the list is open.
#[test]
fn the_dropdown_shows_a_chevron_that_flips_open() {
    let mut app = app();
    let (f, _) = build(&mut app, |ui, c, root| ui.dropdown(c, root, &["A", "B"], 0, None, 12.0, State::NONE));
    let shut = descendants_with::<controls::Chevron>(&mut app, f);
    assert_eq!(shut, [controls::Chevron { up: false }]);
    let img = descendants_entities::<controls::Chevron>(&mut app, f)[0];
    assert!(!app.world().get::<ImageNode>(img).unwrap().flip_y);
    assert_eq!(app.world().get::<controls::KitIcon>(img).unwrap().key, "chevron");
    let (f, _) = build(&mut app, |ui, c, root| ui.dropdown(c, root, &["A", "B"], 0, Some(0), 12.0, State::NONE));
    let img = descendants_entities::<controls::Chevron>(&mut app, f)[0];
    assert!(app.world().get::<ImageNode>(img).unwrap().flip_y, "open: up");
    assert!(chimera_ui::Icons::shipped().get("chevron").is_some(), "the icon ships");
}



/// The progress component (design rules 11.6, variant B): its fill is the
/// share done (0, 42 and 100 %), its words and percentage inside, ready a
/// solid slab without a percentage, connecting striped.
#[test]
fn progress_fills_by_its_share() {
    use chimera_ui::controls::{Progress, ProgressBar, ProgressFill};
    let mut app = app();
    for (f, pct) in [(0.0, "0%"), (0.42, "42%"), (1.0, "100%")] {
        let b = build(&mut app, |ui, c, root| ui.progress(c, root, Progress::Running(f), "Loading", 10.5, 1.15, 0.56, true));
        assert_eq!(app.world().get::<ProgressBar>(b), Some(&ProgressBar(Progress::Running(f))));
        let fill = descendants_with_entity::<ProgressFill>(&mut app, b);
        assert_eq!(fill.len(), 1);
        assert_eq!(app.world().get::<Node>(fill[0]).unwrap().width, Val::Percent(f * 100.0));
        let t = texts_under(&mut app, b);
        assert!(t.iter().all(|t| t == &format!("LOADING {pct}")), "{t:?}");
    }
    let r = build(&mut app, |ui, c, root| ui.progress(c, root, Progress::Ready, "Loaded", 10.5, 1.15, 0.56, true));
    assert!(descendants_with_entity::<ProgressFill>(&mut app, r).is_empty());
    assert!(descendants_with::<UiSlant>(&mut app, r).iter().any(|s| s.fill == Tone::Fg && s.outline == 0.0), "ready: filled");
    assert_eq!(texts_under(&mut app, r), ["LOADED"]);
    let w = build(&mut app, |ui, c, root| ui.progress(c, root, Progress::Connecting, "Connecting", 10.5, 1.15, 0.56, true));
    assert!(descendants_with::<UiSlant>(&mut app, w).len() > 5, "stripes");
}

/// Tabs carry their Q and E caps at the strip's ends; the active one's
/// strip is cobalt.
#[test]
fn tabs_have_their_keys_at_the_ends() {
    let mut app = app();
    let parent = build(&mut app, |ui, c, root| {
        ui.tabs_ex(c, root, &["Server", "Game"], 0, false, Some(("Q", "E")));
        root
    });
    let all = texts_under(&mut app, parent);
    assert_eq!(all, ["Q", "SERVER", "GAME", "E"]);
    assert_eq!(descendants_with_entity::<controls::TabKey>(&mut app, parent).len(), 2);
    assert!(descendants_with::<UiSlant>(&mut app, parent).iter().any(|s| s.fill == Tone::Acc), "the cobalt strip");
}

/// Motion tokens: the scale multiplies every pace; 0 snaps (approach lands
/// at once, no slide starts), more is slower.
#[test]
fn motion_scale_zero_snaps_and_more_slows() {
    let normal = Motion::default();
    let slow = Motion::new(2.0);
    let off = Motion::new(0.0);
    assert!(Pace::Fast.base() < Pace::Normal.base() && Pace::Normal.base() < Pace::Slow.base());
    assert_eq!(slow.secs(Pace::Normal), 2.0 * normal.secs(Pace::Normal));
    assert_eq!(off.secs(Pace::Slow), 0.0);
    assert!(off.snaps() && !normal.snaps());
    assert_eq!(off.approach(0.0, 1.0, 0.0, Pace::Fast), 1.0, "scale 0: there at once");
    let part = normal.approach(0.0, 1.0, 0.03, Pace::Fast);
    assert!(part > 0.0 && part < 1.0, "scale 1: on its way: {part}");
    assert!(slow.approach(0.0, 1.0, 0.03, Pace::Fast) < part, "scale 2: slower");
    // A pick slides from where it stood; not at scale 0, not when it hasn't moved.
    assert_eq!(controls::pick_slide(Some((40.0, 50.0)), (100.0, 60.0), &normal), Some((-60.0, 50.0 / 60.0)));
    assert_eq!(controls::pick_slide(Some((40.0, 50.0)), (100.0, 60.0), &off), None);
    assert_eq!(controls::pick_slide(Some((100.0, 60.0)), (100.0, 60.0), &normal), None);
    assert_eq!(controls::pick_slide(None, (100.0, 60.0), &normal), None);
}

/// A hero button eases to its hover over the fast pace, snaps at scale 0,
/// and eases back.
#[test]
fn hero_hover_eases_through_the_motion_tokens() {
    for (scale, snaps) in [(1.0, false), (0.0, true)] {
        let mut app = app();
        app.insert_resource(Motion::new(scale));
        let b = build(&mut app, |ui, c, root| ui.button(c, root, Kind::Primary, "Start", None, controls::HERO_H, State::NONE));
        let slab = descendants_entities::<controls::HeroSlab>(&mut app, b)[0];
        let glow = descendants_entities::<controls::HeroGlow>(&mut app, b)[0];
        let lift = |app: &App| app.world().get::<controls::HeroSlab>(slab).unwrap().lift;
        let alpha = |app: &App| app.world().get::<UiSlant>(glow).unwrap().alpha;
        assert_eq!(alpha(&app), 0.0);
        *app.world_mut().get_mut::<Interaction>(b).unwrap() = Interaction::Hovered;
        app.world_mut().resource_mut::<UiClock>().step(0.03);
        app.update();
        if snaps {
            assert_eq!(lift(&app), 1.0, "scale 0: hovered in one frame");
            assert_eq!(alpha(&app), 1.0);
        } else {
            let l = lift(&app);
            assert!(l > 0.0 && l < 1.0, "on its way after 30 ms: {l}");
            assert!(alpha(&app) > 0.0 && alpha(&app) < 1.0);
            app.world_mut().resource_mut::<UiClock>().step(0.2);
            app.update();
            assert_eq!(lift(&app), 1.0);
            assert_eq!(alpha(&app), 1.0);
        }
        *app.world_mut().get_mut::<Interaction>(b).unwrap() = Interaction::None;
        app.world_mut().resource_mut::<UiClock>().step(0.3);
        app.update();
        assert_eq!(lift(&app), 0.0, "back at rest");
        assert_eq!(alpha(&app), 0.0);
    }
}

/// The segmented picker is the toggle's look (inset in its track, its
/// height), hovers as a parallelogram, and its focus strip is a child of
/// the pick (inside it).
#[test]
fn segmented_is_inset_like_the_toggle_with_slanted_hover() {
    let mut app = app();
    let opts = build(&mut app, |ui, c, root| ui.segmented(c, root, &["A", "B", "C"], Some(1), State::focused(true)));
    let track = app.world().get::<ChildOf>(opts[0]).unwrap().parent();
    let pad = app.world().get::<chimera_ui::UiStyle>(track).unwrap().pad.unwrap();
    assert!(pad.0.iter().all(|p| *p > 0.0), "inset: {pad:?}");
    let h = app.world().get::<chimera_ui::UiStyle>(opts[0]).unwrap().h.unwrap();
    assert!((h + pad.0[0] + pad.0[2] - controls::TOGGLE_H).abs() < 0.001, "as tall as the toggle");
    // Hover: a slanted tick on the others, none on the pick.
    assert_eq!(descendants_with_entity::<Tick>(&mut app, opts[0]).len(), 1);
    assert!(descendants_with_entity::<Tick>(&mut app, opts[1]).is_empty());
    let pick = descendants_with_entity::<controls::SegmentedPick>(&mut app, opts[1])[0];
    let strip = app.world().get::<Children>(pick).expect("the focus strip is the pick's child");
    assert_eq!(strip.len(), 1);
}
