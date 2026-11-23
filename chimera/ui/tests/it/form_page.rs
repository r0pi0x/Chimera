//! A form page's components (`chimera_ui::form_page`): masks that reveal
//! nothing, busy as disabled, the footer's summary at any size, the
//! avatar pick's outline outside its tile, the read-only rows' icon
//! column, clickable icons apart from decorative ones, badges as data and
//! the Copy key.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_ui::{
    ChimeraUiPlugin, Theme, Tone, UiClock, UiStyle, UiUnits,
    controls::{BusyLight, Hover, Kind, KitControl, Size, State, Ui},
    form_page::{
        self, AVATAR_GAP, AVATAR_RING, AvatarTile, BadgeLook, BadgeShape, BadgeSpec, ChangesList, Copies, CopyKey, FieldSpec, FieldStatus,
        FooterAction, FooterSpec, IconKey, InertIcon, RowTools, RowValue, SecretSpec, TOOLS_W,
    },
    secret::{self, DOT},
};

const SIGNAL: &str = include_str!("../../../../apps/ultimate_gun_game/data/theme.toml");

fn app() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default(), bevy::text::TextPlugin)).add_plugins(ChimeraUiPlugin);
    app.insert_resource(UiUnits::new(1080.0, 1.0));
    app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    app.insert_resource(UiClock::manual());
    app.insert_resource(chimera_ui::clipboard::Clipboard::memory(None));
    app
}

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

fn under(app: &mut App, e: Entity) -> Vec<Entity> {
    let w = app.world();
    let mut out = Vec::new();
    let mut stack = vec![e];
    while let Some(n) = stack.pop() {
        out.push(n);
        if let Some(ch) = w.get::<Children>(n) {
            stack.extend(ch.iter());
        }
    }
    out
}

fn text(app: &mut App, e: Entity) -> String {
    under(app, e).into_iter().filter_map(|n| app.world().get::<Text>(n).map(|t| t.0.clone())).collect::<Vec<_>>().join("|")
}

fn with<C: Component>(app: &mut App, e: Entity) -> Vec<Entity> {
    under(app, e).into_iter().filter(|n| app.world().get::<C>(*n).is_some()).collect()
}

#[test]
fn masks_reveal_nothing_but_a_fixed_formats_dashes() {
    let run: String = std::iter::repeat_n(DOT, secret::MASK_LEN).collect();
    // An address: the same run whatever its octets; a port a fixed 5 dots.
    assert_eq!(secret::mask_address("203.0.113.24:27015"), format!("{run}:•••••"));
    assert_eq!(secret::mask_address("1.2.3.4:1"), secret::mask_address("203.0.113.24:27015"), "nothing of the digits");
    assert_eq!(secret::mask_address("10.0.0.1"), run, "no port: the run alone");
    assert_eq!(secret::mask_address("hub.example.net:27080"), format!("{run}:•••••"));
    assert_eq!(secret::masked("203.0.113.24", false), run, "never a dot an octet's digit");
    assert!(!secret::masked("203.0.113.24:27015", false).contains('.'));
    // Only a fixed-format secret keeps its separators.
    assert_eq!(secret::masked("AB12-CD34-EF56", true), "••••-••••-••••");
    assert_eq!(secret::masked("an invite code", false), run);
    let mut app = app();
    let (field, row) = build(&mut app, |ui, c, root| {
        let f = ui.secret_field(c, root, &SecretSpec { label: "Hub address", key: "k", value: "203.0.113.24:27015", w: 30.0, ..default() });
        let (r, keys) = ui.value_row(c, root, "Address", RowValue::Secret { key: "a", value: "203.0.113.24:27015", revealed: false, copied: false }, 7.0, true);
        assert_eq!(keys.len(), 2, "the eye and Copy");
        (f.root, r)
    });
    for e in [field, row] {
        let t = text(&mut app, e);
        assert!(!t.chars().any(|ch| ch.is_ascii_digit()), "no digit shows: {t}");
        assert!(t.contains(&format!("{run}:•••••")), "{t}");
    }
}

#[test]
fn busy_is_disabled_and_a_saving_form_disables_every_action() {
    let mut app = app();
    let (busy, off, on) = build(&mut app, |ui, c, root| {
        (
            ui.button(c, root, Kind::Primary, "Saving...", None, Size::Regular, State::busy(true)),
            ui.button(c, root, Kind::Secondary, "Discard", None, Size::Regular, State { disabled: true, ..State::NONE }),
            ui.button(c, root, Kind::Primary, "Save changes", None, Size::Regular, State::NONE),
        )
    });
    for b in [busy, off] {
        let w = app.world();
        assert!(w.get::<Interaction>(b).is_none() && w.get::<KitControl>(b).is_none(), "not pressable");
        assert!(w.get::<Hover>(b).is_none(), "no hover");
    }
    assert!(app.world().get::<Interaction>(on).is_some());
    assert_eq!(with::<BusyLight>(&mut app, busy).len(), 1, "busy pulses its light");
    // A footer saving: Discard and Save both wait.
    let changes = vec!["username".to_string(), "avatar".into()];
    let actions = [
        FooterAction { kind: Kind::Tertiary, label: "Discard", busy_label: None, key: None, disabled: false },
        FooterAction { kind: Kind::Primary, label: "Save changes", busy_label: Some("Saving..."), key: Some("Ctrl+S"), disabled: false },
    ];
    let parts = build(&mut app, |ui, c, root| ui.form_footer(c, root, &FooterSpec { changes: &changes, busy: Some("Saving..."), actions: &actions, ..default() }));
    for a in &parts.actions {
        assert!(app.world().get::<Interaction>(*a).is_none(), "every action waits while it saves");
    }
    assert!(text(&mut app, parts.actions[1]).contains("SAVING"), "the primary says it");
}

#[test]
fn the_footers_summary_stays_one_line_for_a_big_form() {
    assert_eq!(form_page::changes_summary(&[]), None);
    assert_eq!(form_page::changes_summary(&["avatar".into()]).as_deref(), Some("1 change · avatar"));
    let many: Vec<String> = (0..30).map(|i| format!("field_{i}")).collect();
    assert_eq!(form_page::changes_summary(&many).as_deref(), Some("30 changes"));
    let actions = [FooterAction { kind: Kind::Primary, label: "Save changes", busy_label: Some("Saving..."), key: None, disabled: false }];
    let mut app = app();
    let (shut, bar) = build(&mut app, |ui, c, root| (ui.form_footer(c, root, &FooterSpec { changes: &many, actions: &actions, ..default() }), root));
    let t = text(&mut app, bar);
    assert!(t.contains("30 CHANGES") && !t.contains("FIELD_7"), "folded: one line ({t})");
    assert!(shut.toggle.is_some() && shut.list.is_none());
    let open = build(&mut app, |ui, c, root| ui.form_footer(c, root, &FooterSpec { changes: &many, open: true, actions: &actions, ..default() }));
    let list = open.list.expect("unfolded");
    assert!(app.world().get::<ChangesList>(list).is_some());
    assert!(text(&mut app, list).contains("FIELD_29"), "every change in the list");
}

#[test]
fn the_avatar_picks_outline_is_outside_its_tile() {
    let mut app = app();
    let tiles = build(&mut app, |ui, c, root| ui.avatar_grid(c, root, None, None, 9, false, 40.0));
    assert_eq!(tiles.len(), 32);
    let units = *app.world().resource::<UiUnits>();
    let (w, off) = AVATAR_RING;
    for (i, t) in tiles.iter().enumerate() {
        assert_eq!(app.world().get::<AvatarTile>(*t).map(|a| a.0), Some(i as u8));
        let outline = app.world().get::<Outline>(*t);
        if i == 9 {
            let o = outline.expect("the pick is outlined");
            assert_eq!((o.width, o.offset), (units.val(w), units.val(off)), "outside the tile (a positive offset)");
            assert!(off > 0.0 && w + off < AVATAR_GAP, "the outline stays in the gap: no tile's edge covered");
            assert_eq!(o.color, app.world().resource::<Theme>().tone(Tone::Fg));
        } else {
            assert!(outline.is_none(), "tile {i}: no frame");
        }
        // The look fills the tile; nothing drawn over it (no inset frame,
        // no bar under the pick).
        assert_eq!(app.world().get::<Children>(*t).map_or(0, |c| c.len()), 1, "tile {i}: only its look");
        assert!(app.world().get::<UiStyle>(*t).unwrap().pad.is_none());
    }
}

#[test]
fn read_only_rows_line_their_icons_up_and_keys_look_clickable() {
    let mut app = app();
    let (rows, field) = build(&mut app, |ui, c, root| {
        let a = ui.value_row(c, root, "Hub", RowValue::Text("Friday Night Hub"), 7.0, true).0;
        let b = ui.value_row(c, root, "Address", RowValue::Secret { key: "x", value: "10.0.0.1:27015", revealed: false, copied: false }, 7.0, true).0;
        let r = ui.value_row(c, root, "Role", RowValue::Badge("Tester", BadgeLook::OUTLINE), 7.0, true).0;
        let f = ui.form_field(c, root, &FieldSpec { label: "Username", value: "night_owl", status: FieldStatus::Locked("Renamed a moment ago"), w: 24.0, ..default() });
        (vec![a, b, r], f)
    });
    for r in &rows {
        let tools = with::<RowTools>(&mut app, *r);
        assert_eq!(tools.len(), 1);
        let s = app.world().get::<UiStyle>(tools[0]).unwrap();
        assert_eq!((s.w, s.justify), (Some(TOOLS_W), Some(JustifyContent::FlexEnd)), "one right-aligned column a row");
    }
    // Clickable icons have a plate and frame; a lock has neither and takes
    // no press.
    for k in with::<IconKey>(&mut app, rows[1]) {
        let w = app.world();
        assert!(w.get::<Interaction>(k).is_some() && w.get::<Outline>(k).is_some());
        assert!(w.get::<BackgroundColor>(k).unwrap().0.alpha() > 0.0, "visible at rest");
    }
    for lock in with::<InertIcon>(&mut app, rows[0]).into_iter().chain(with::<InertIcon>(&mut app, field.root)) {
        let w = app.world();
        assert!(w.get::<Interaction>(lock).is_none() && w.get::<Outline>(lock).is_none());
    }
    assert!(app.world().get::<Interaction>(field.input).is_none(), "a locked field takes no press");
    assert!(text(&mut app, field.status).contains("RENAMED A MOMENT AGO"));
}

#[test]
fn a_fields_error_offers_its_fixes_and_the_status_slot_keeps_its_place() {
    let mut app = app();
    let fixes = vec!["rook_77".to_string(), "rook_fn".into()];
    let (err, rest) = build(&mut app, |ui, c, root| {
        (
            ui.form_field(c, root, &FieldSpec { label: "Username", tag: Some("Required"), help: "3 to 16 letters", value: "rook", status: FieldStatus::Error { text: "Taken", fixes: &fixes }, w: 24.0, ..default() }),
            ui.form_field(c, root, &FieldSpec { label: "Display name", value: "", w: 24.0, ..default() }),
        )
    });
    assert_eq!(err.fixes.len(), 2);
    for f in &err.fixes {
        assert!(app.world().get::<KitControl>(*f).is_some(), "a fix is a click");
    }
    for f in [&err, &rest] {
        assert!(app.world().get::<chimera_ui::controls::MinHeight>(f.status).is_some(), "the slot is reserved");
    }
    let t = text(&mut app, err.root);
    assert!(t.contains("USERNAME") && t.contains("REQUIRED") && t.contains("TAKEN"), "{t}");
}

#[test]
fn badges_take_their_look_as_data_and_keep_their_slant_inside() {
    let spec: BadgeSpec = toml::from_str("style = \"outline\"\ntone = \"warn\"").unwrap();
    assert_eq!(BadgeLook::from_spec(&spec), BadgeLook { fill: None, border: Some(Tone::Warn), ink: Tone::Warn });
    let filled: BadgeSpec = toml::from_str("style = \"filled\"\ntone = \"hero\"").unwrap();
    assert_eq!(BadgeLook::from_spec(&filled), BadgeLook::HERO);
    assert_eq!(BadgeLook::from_spec(&BadgeSpec::default()), BadgeLook::OUTLINE);
    let mut app = app();
    let b = build(&mut app, |ui, c, root| ui.badge_look(c, root, "Tester", BadgeLook::OUTLINE));
    let shape = *app.world().get::<BadgeShape>(b).unwrap();
    let m = app.world().get::<UiStyle>(b).unwrap().margin.unwrap();
    assert!(shape.lean > 0.0 && m.0[1] >= shape.lean / 2.0 - 1e-4 && m.0[3] >= shape.lean / 2.0 - 1e-4, "its lean has room at each side");
}

#[test]
fn a_copy_key_copies_hidden_and_says_so_for_two_seconds() {
    let mut app = app();
    let parts = build(&mut app, |ui, c, root| {
        ui.secret_field(c, root, &SecretSpec { label: "Recovery key", key: "rk", value: "AB12-CD34", fixed: true, w: 30.0, ..default() })
    });
    assert_eq!(app.world().get::<CopyKey>(parts.copy).map(|k| k.value.as_str()), Some("AB12-CD34"));
    *app.world_mut().get_mut::<Interaction>(parts.copy).unwrap() = Interaction::Pressed;
    app.update();
    assert!(app.world().resource::<Copies>().copied("rk"));
    assert_eq!(app.world_mut().resource_mut::<chimera_ui::clipboard::Clipboard>().get().as_deref(), Some("AB12-CD34"), "copied while hidden");
    app.world_mut().resource_mut::<UiClock>().step(form_page::COPIED_SECS as f32 + 0.1);
    app.update();
    assert!(!app.world().resource::<Copies>().copied("rk"), "forgotten after");
}
