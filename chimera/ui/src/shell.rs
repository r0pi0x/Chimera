//! The shell: what every out-of-match screen shares (the kit's design
//! rules, sections 7, 10, 11, 12 and 16; docs/plans/active/ugg-ui.md
//! "Signal revision 8"). A screen supplies only its contents.
//!
//! - **Zones** (`ShellLayout`, `[layout.shell]`): a 100 x 56.25 cq frame,
//!   2.6 cq margins; the bar band from 1.8 cq, 4.6 tall, a hairline rule
//!   at 7.2; the content window from 8.4 cq down to 4.6 cq from the
//!   bottom (rows from 1.8 cq in, the side panel 31 cq wide, its action
//!   bar 4.6 tall along the foot); the key hints' baseline 1.8 cq from
//!   the bottom (`keys`).
//! - **The bar** (`bar`): at the left the lockup on the root (`Left::
//!   Lockup`), elsewhere the stacked breadcrumb (`Left::Crumb`): "‹ MAIN
//!   MENU" (Mono .62, `fg_2`, its Esc cap and the right mouse button
//!   beside it) above the screen's title (Display Bold 2 cq). The
//!   parent's line is the back target (`ShellBack`): hover lifts it to
//!   `fg` with a .1 cq underline, a click goes back as Esc does. At the
//!   right one rack of square tiles (T1): "PARTY n/8", the party's
//!   avatars, each with its edge strip (`avatar`), the "+" invite slot;
//!   then the system tiles Friends, Settings, Report a bug (F10)
//!   (`ShellTile`). The order never changes; the tile whose overlay is
//!   open is white (`Bar::open`).
//! - **Back** (`back_keys`): a right click is Back on a shell screen
//!   (`ShellScreen` up, the pointer free), as is a click on the
//!   breadcrumb: both press Esc for one frame (a `KeyboardInput` pressed,
//!   then released), so each screen's own Esc does what Back does there.
//!   Not while a text field has the keys or a bind listens (`BackGuard`,
//!   held by the screens each frame), never while the left button is held,
//!   never twice for one press. With a modal layer up (`modal`) the
//!   right click is the top modal's Back (Esc, whatever the screen
//!   behind) unless it is `Blocking`; the screen gets none.
//! - **Unbuilt features** (`Disabled`): drawn `fg_4` with a `fg_5`
//!   outline, no hover lift, no sound, skipped by the keys; their tooltip
//!   still names them, "Coming soon" under it.
//! - **Tooltips** (`Tip`, `show_tips`): after 350 ms of hover, an `ink_3`
//!   plate with an outline under the node, right-aligned: the name, a
//!   note in `fg_3`, the key's cap.
//! - **The ground** (`menu_ground`): the main menu's ambient (the kit's
//!   `AmbientMaterial::MENU`) under a menu screen (Settings, Account, the
//!   first run), so every screen off the match stands on one ground.
//! - **No entrance slide**: a screen appears in place, whole; its parts
//!   don't slide or rise in (the user's review of the first run,
//!   2026-11-21). An intro and a control's own small motions (a toggle's
//!   slab, a hover, a tab's wipe) stay.
//!
//! - **The party's avatars** (the user's report, 2026-11-22: no account
//!   card in the corner): yours is the Profile tile (`ShellTile::Profile`):
//!   a click opens the Account page (`party_kit::account_page`), it lights
//!   white on hover. A hover on any member's avatar, yours too, shows a
//!   small preview card after the tooltip delay (`Tip::card`: the avatar,
//!   the display name, `@username`, "You" and the role, the ping when
//!   known); it never takes the pointer.
//!
//! Known shortcuts: the "+" opens the party panel only with a hub account
//! (`Party::invite`; parties: docs/plans/active/parties.md), Friends is
//! disabled; a hub party's others have no ping yet.

use bevy::{
    input::{ButtonState, keyboard::{Key, KeyboardInput}},
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, RelativeCursorPosition},
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use crate::{
    IconImages, Role, Theme, Tone, UiClock, UiStyle, UiUnits,
    avatar::{self, AvatarMarks, EdgeState, Face},
    controls::{Hover, Tick, Ui, pressable},
};

/// The shell's numbers (`[layout.shell]`, cq; module doc).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellLayout {
    pub margin: f32,
    pub bar_y: f32,
    pub bar_h: f32,
    pub rule_y: f32,
    pub win_top: f32,
    pub win_bottom: f32,
    pub keys_y: f32,
    pub side_w: f32,
    /// Rows' inset from the window's left, and their end from its right.
    pub rows_in: f32,
    pub rows_right: f32,
    pub row_h: f32,
    /// Value controls' width (sliders, fields, dropdowns).
    pub value_w: f32,
    /// The action bar's height.
    pub act_h: f32,
    /// A tile's side.
    pub tile: f32,
}

impl ShellLayout {
    pub fn of(t: &Theme) -> Self {
        let l = |k: &str, or: f32| t.layout_opt("shell", k).unwrap_or(or);
        ShellLayout {
            margin: l("margin", 2.6),
            bar_y: l("bar_y", 1.8),
            bar_h: l("bar_h", 4.6),
            rule_y: l("rule_y", 7.2),
            win_top: l("win_top", 8.4),
            win_bottom: l("win_bottom", 4.6),
            keys_y: l("keys_y", 1.8),
            side_w: l("side_w", 31.0),
            rows_in: l("rows_in", 1.8),
            rows_right: l("rows_right", 35.0),
            row_h: l("row_h", 2.8),
            value_w: l("value_w", 14.0),
            act_h: l("act_h", 4.6),
            tile: l("tile", 3.0),
        }
    }
}

/// A screen built on the shell (its root): right-click Back works while
/// one is up.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ShellScreen;

/// The breadcrumb's parent line: a click goes back (module doc).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ShellBack;

/// The underline the breadcrumb shows while hovered.
#[derive(Component, Clone, Copy, Debug)]
pub struct CrumbLine;

/// A tile of the bar's rack (module doc), in its fixed order.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShellTile {
    /// Your own avatar (Profile, later).
    Profile,
    /// The party's "+" (invite).
    Invite,
    Friends,
    Settings,
    Report,
}

/// An unbuilt feature's control (module doc): seen, never acted on.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Disabled;

/// "Coming soon": an unbuilt feature's tooltip note.
pub const COMING_SOON: &str = "Coming soon";

/// A node's tooltip (module doc).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Tip {
    pub name: String,
    pub note: Option<String>,
    pub key: Option<String>,
    /// A party member's preview card instead of the plain plate (module doc).
    pub card: Option<Box<Member>>,
}

impl Tip {
    pub fn new(name: &str) -> Self {
        Tip { name: name.to_string(), note: None, key: None, card: None }
    }

    /// The member's preview card (avatar, names, role, ping).
    pub fn card(mut self, m: &Member) -> Self {
        self.card = Some(Box::new(m.clone()));
        self
    }

    pub fn note(mut self, n: &str) -> Self {
        self.note = Some(n.to_string());
        self
    }

    pub fn key(mut self, k: &str) -> Self {
        self.key = Some(k.to_string());
        self
    }
}

/// A shown tooltip (the plate), and the node it belongs to.
#[derive(Component, Clone, Copy, Debug)]
pub struct TipPlate(pub Entity);

/// One party member in the bar.
#[derive(Clone, Debug, PartialEq)]
pub struct Member {
    pub name: String,
    pub face: Face,
    pub marks: AvatarMarks,
    pub state: EdgeState,
    pub lead: bool,
    /// The display name, when it differs from `name` (the username).
    pub display: Option<String>,
    /// The role tag on the preview card ("Host", "Party leader"), when any.
    pub role: Option<String>,
    /// The round trip, ms, when known.
    pub ping: Option<u16>,
}

/// The party the bar shows (you first), and how big it may grow; an app
/// keeps it current (`chimera_client`'s profile), the screens draw it.
/// Written only when it differs, so screens may rebuild on its change.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct Party {
    pub members: Vec<Member>,
    pub max: u32,
    /// The "+" is live (parties available: a hub account); else disabled.
    pub invite: bool,
}

impl Default for Party {
    fn default() -> Self {
        Party {
            members: vec![Member {
                name: "Player".into(),
                face: Face::Look(avatar::AvatarLook::default()),
                marks: AvatarMarks { you: true, ..default() },
                state: EdgeState::Idle,
                lead: true,
                display: None,
                role: None,
                ping: None,
            }],
            max: 8,
            invite: false,
        }
    }
}

/// The bar's left.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Left<'a> {
    /// The root: the lockup (the screen draws its own, animated, at the
    /// bar's place) and nothing here.
    Lockup,
    /// The breadcrumb: the parent's name above the title.
    Crumb { parent: &'a str, title: &'a str },
}

/// What the bar shows.
#[derive(Clone, Debug)]
pub struct Bar<'a> {
    pub left: Left<'a>,
    /// You first, then by join order.
    pub party: &'a [Member],
    pub party_max: u32,
    /// The "+" is live (`Party::invite`).
    pub invite: bool,
    /// The tile whose overlay is open (white).
    pub open: Option<ShellTile>,
}

/// What `bar` spawned.
#[derive(Clone, Debug, Default)]
pub struct BarParts {
    pub bar: Option<Entity>,
    pub crumb: Option<Entity>,
    pub title: Option<Entity>,
    pub tiles: Vec<(ShellTile, Entity)>,
}

impl BarParts {
    pub fn tile(&self, t: ShellTile) -> Option<Entity> {
        self.tiles.iter().find(|(k, _)| *k == t).map(|(_, e)| *e)
    }
}

/// Spawns the bar and its rule under `frame` (a 100 x 56.25 cq frame;
/// module doc). The tiles carry `ShellTile` (with `clickable`, or
/// `Disabled`); the screen acts on them.
pub fn bar(ui: &Ui, c: &mut Commands, frame: Entity, images: &mut Assets<Image>, icons: &mut IconImages, b: &Bar) -> BarParts {
    let l = ShellLayout::of(ui.theme);
    let mut parts = BarParts::default();
    let band = ui.boxed(
        c,
        frame,
        UiStyle { h: Some(l.bar_h), justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() }.abs([Some(l.margin), Some(l.bar_y), Some(l.margin), None]),
    );
    c.entity(band).insert(Name::new("shell bar"));
    parts.bar = Some(band);
    // The left.
    let left = ui.boxed(c, band, UiStyle { gap: Some(0.35), align: Some(AlignItems::FlexStart), ..UiStyle::column() });
    if let Left::Crumb { parent, title } = b.left {
        let up = c
            .spawn((
                UiStyle { gap: Some(0.4), pad: Some([0.0, 0.0, 0.12, 0.0].into()), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(Color::NONE),
                ShellBack,
                pressable(),
                Name::new("shell crumb"),
                ChildOf(left),
            ))
            .id();
        ui.icon(c, up, images, icons, "back", 0.62, Tone::Fg2);
        ui.text(c, up, Role::Mono, 0.62, Tone::Fg2, parent);
        c.spawn((
            CrumbLine,
            Node { position_type: PositionType::Absolute, left: ui.units.val(0.8), right: Val::ZERO, bottom: Val::ZERO, height: ui.units.val(0.1), ..default() },
            BackgroundColor(ui.color(Tone::Fg, 1.0)),
            Visibility::Hidden,
            Pickable::IGNORE,
            ChildOf(up),
        ));
        let hints = ui.boxed(c, up, UiStyle { gap: Some(0.3), margin: Some([0.0, 0.0, 0.0, 0.6].into()), ..UiStyle::row() });
        let cap = ui.boxed(c, hints, UiStyle { pad: Some([0.15, 0.3, 0.15, 0.3].into()), ..UiStyle::row() }.outline(Tone::Fg4, 1.0));
        ui.text(c, cap, Role::Mono, 0.56, Tone::Fg3, "Esc");
        ui.icon(c, hints, images, icons, "mouse_right", 0.75, Tone::Fg3);
        parts.crumb = Some(up);
        parts.title = Some(ui.text(c, left, Role::DisplayBold, 2.0, Tone::Fg, title));
    }
    // The right: the party's rack, then the system tiles' rack.
    let right = ui.boxed(c, band, UiStyle { gap: Some(0.85), ..UiStyle::row() });
    let label = ui.boxed(c, right, UiStyle { gap: Some(0.25), align: Some(AlignItems::FlexEnd), ..UiStyle::column() });
    ui.text(c, label, Role::Mono, 0.5, Tone::Fg3, "Party");
    ui.text(c, label, Role::Mono, 0.5, Tone::Fg2, &format!("{}/{}", b.party.len().max(1), b.party_max.max(1)));
    let rack = |c: &mut Commands, parent: Entity| {
        c.spawn((
            UiStyle { h: Some(l.tile), ..UiStyle::row() }.bg_a(Tone::Ink, 0.75).outline(Tone::Fg4, 1.0),
            Node::default(),
            ChildOf(parent),
        ))
        .id()
    };
    let party = rack(c, right);
    let gap2 = (2.0 / ui.units.scale_factor).max(ui.units.hairline());
    for (i, m) in b.party.iter().enumerate() {
        let cell = c
            .spawn((
                Node { height: ui.units.val(l.tile), column_gap: Val::Px(gap2), flex_direction: FlexDirection::Row, ..default() },
                Tip::new(&m.name).card(m),
                pressable(),
                ChildOf(party),
            ))
            .id();
        if i == 0 {
            c.entity(cell).insert(ShellTile::Profile);
            parts.tiles.push((ShellTile::Profile, cell));
        }
        avatar::spawn(ui, c, cell, images, icons, m.face, m.marks, l.tile);
        avatar::spawn_strip(ui, c, cell, images, icons, m.state, m.lead && b.party.len() > 1, l.tile);
        if i == 0 {
            // Yours opens the Profile: it lights on hover (a white frame
            // and a veil over the avatar), as the other tiles do.
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::ZERO,
                    right: Val::ZERO,
                    top: Val::ZERO,
                    bottom: Val::ZERO,
                    border: UiRect::all(Val::Px(ui.units.hairline().max(1.0) * 1.5)),
                    ..default()
                },
                BackgroundColor(ui.color(Tone::Fg, 0.14)),
                BorderColor::all(ui.color(Tone::Fg, 1.0)),
                Tick { always: false },
                Visibility::Hidden,
                Pickable::IGNORE,
                ChildOf(cell),
            ));
        }
    }
    // The invite slot: the party panel (provisional), disabled without a
    // hub account.
    let tip = if b.invite { Tip::new("Party").note("Make a party, invite with its code, or join one") } else { Tip::new("Party").note("Sign in to a hub first") };
    let plus = tile(ui, c, party, images, icons, "plus", 0.34, ShellTile::Invite, tip, !b.invite, b.open == Some(ShellTile::Invite), i32::from(!b.party.is_empty()));
    parts.tiles.push((ShellTile::Invite, plus));
    let sys = rack(c, right);
    for (k, key, tip, dis) in [
        (ShellTile::Friends, "friends", Tip::new("Friends").note(COMING_SOON), true),
        (ShellTile::Settings, "gear", Tip::new("Settings"), false),
        (ShellTile::Report, "bug", Tip::new("Report a bug").key("F10"), false),
    ] {
        let e = tile(ui, c, sys, images, icons, key, 0.44, k, tip, dis, b.open == Some(k), i32::from(k != ShellTile::Friends));
        parts.tiles.push((k, e));
    }
    // The rule.
    let rule = ui.boxed(c, frame, UiStyle::row().abs([Some(l.margin), Some(l.rule_y), Some(l.margin), None]));
    c.spawn((Node { width: Val::Percent(100.0), height: Val::Px(ui.units.hairline()), ..default() }, BackgroundColor(ui.color(Tone::Fg5, 1.0)), ChildOf(rule)));
    parts
}

/// A square tile of a rack: an icon `icon_h` of the side, its tooltip;
/// disabled (`Disabled`, `fg_4`), open (white, the icon ink) or live.
/// `divider`: a `fg_5` line at its left (not the rack's first).
#[allow(clippy::too_many_arguments)]
fn tile(ui: &Ui, c: &mut Commands, parent: Entity, images: &mut Assets<Image>, icons: &mut IconImages, icon: &str, icon_h: f32, k: ShellTile, tip: Tip, disabled: bool, open: bool, divider: i32) -> Entity {
    let l = ShellLayout::of(ui.theme);
    let (base, over, tone) = if disabled {
        (Color::NONE, Color::NONE, Tone::Fg4)
    } else if open {
        (ui.color(Tone::Fg, 1.0), ui.color(Tone::Fg, 1.0), Tone::Ink)
    } else {
        (Color::NONE, ui.color(Tone::Fg, 0.08), Tone::Fg)
    };
    let e = c
        .spawn((
            UiStyle { w: Some(l.tile), h: Some(l.tile), justify: Some(JustifyContent::Center), ..UiStyle::row() },
            Node::default(),
            BackgroundColor(base),
            k,
            tip,
            Name::new(format!("shell tile {k:?}")),
            ChildOf(parent),
        ))
        .id();
    if disabled {
        c.entity(e).insert((Disabled, pressable()));
    } else {
        c.entity(e).insert((Hover { base, over }, pressable()));
    }
    if divider > 0 {
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, width: Val::Px(ui.units.hairline()), ..default() },
            BackgroundColor(ui.color(Tone::Fg5, 1.0)),
            Pickable::IGNORE,
            ChildOf(e),
        ));
    }
    ui.icon(c, e, images, icons, icon, l.tile * icon_h, tone);
    e
}

/// The key hints along the foot (cap, then what it does), at the left.
pub fn keys(ui: &Ui, c: &mut Commands, frame: Entity, keys: &[(&str, &str)]) -> Entity {
    let l = ShellLayout::of(ui.theme);
    let foot = ui.boxed(c, frame, UiStyle { gap: Some(1.6), ..UiStyle::row() }.abs([Some(l.margin), None, None, Some(l.keys_y)]));
    for (k, what) in keys {
        let g = ui.boxed(c, foot, UiStyle { gap: Some(0.6), ..UiStyle::row() });
        let cap = ui.boxed(c, g, UiStyle { pad: Some([0.2, 0.34, 0.2, 0.34].into()), ..UiStyle::row() }.outline(Tone::Fg3, 1.0));
        ui.text(c, cap, Role::Mono, 0.6, Tone::Fg, k);
        ui.text(c, g, Role::Mono, 0.6, Tone::Fg2, what);
    }
    foot
}

/// The content window (straight; `ink` .91, a `fg_5` outline) between the
/// rule and the key hints.
pub fn window(ui: &Ui, c: &mut Commands, frame: Entity) -> Entity {
    let l = ShellLayout::of(ui.theme);
    c.spawn((
        UiStyle::column().abs([Some(l.margin), Some(l.win_top), Some(l.margin), Some(l.win_bottom)]).bg_a(Tone::Ink, 0.91).outline(Tone::Fg5, 1.0),
        Node::default(),
        Name::new("shell window"),
        ChildOf(frame),
    ))
    .id()
}

/// The window's action bar along its foot: a hairline above; returns the
/// bar (status at its left, the hero last).
pub fn action_bar(ui: &Ui, c: &mut Commands, win: Entity) -> Entity {
    let l = ShellLayout::of(ui.theme);
    let bar = c
        .spawn((
            UiStyle { h: Some(l.act_h), gap: Some(0.8), pad: Some([0.0, l.rows_in, 0.0, l.rows_in].into()), ..UiStyle::row() }.abs([Some(0.0), None, Some(0.0), Some(0.0)]),
            Node::default(),
            Name::new("shell actions"),
            ChildOf(win),
        ))
        .id();
    c.spawn((
        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: Val::Px(ui.units.hairline()), ..default() },
        BackgroundColor(ui.color(Tone::Fg5, 1.0)),
        Pickable::IGNORE,
        ChildOf(bar),
    ));
    bar
}

// ---------------------------------------------------------------------------
// Back, tooltips, the crumb's hover.

/// Holds right-click Back off for a frame (module doc): a screen whose
/// text field has the keys, or whose bind listens, calls `hold` each
/// frame (in `Update`); `back_keys` reads it the next `PreUpdate`.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct BackGuard {
    held: bool,
    last: bool,
}

impl BackGuard {
    pub fn hold(&mut self) {
        self.held = true;
    }

    /// Held last frame.
    pub fn held(&self) -> bool {
        self.last
    }
}

/// A Back the shell pressed (Esc for one frame), so tests and cues can see
/// it.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShellBackPressed {
    pub right_click: bool,
}

/// Whether the pointer is free (a menu's): no window, or one whose cursor
/// is visible and not grabbed.
fn pointer_free(cursor: &Query<&CursorOptions, With<PrimaryWindow>>) -> bool {
    cursor.single().map_or(true, |o| o.visible && o.grab_mode == CursorGrabMode::None)
}

/// Right-click and the breadcrumb press Esc (module doc). In `PreUpdate`,
/// before Bevy's input systems read the keyboard.
#[allow(clippy::too_many_arguments)]
pub fn back_keys(
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut guard: ResMut<BackGuard>,
    screens: Query<(), With<ShellScreen>>,
    modals: Option<Res<crate::modal::Modals>>,
    blocking: Query<(), With<crate::modal::Blocking>>,
    crumbs: Query<&Interaction, (With<ShellBack>, Changed<Interaction>)>,
    cursor: Query<&CursorOptions, With<PrimaryWindow>>,
    window: Query<Entity, With<PrimaryWindow>>,
    fields: Query<&RelativeCursorPosition, With<crate::text_menu::TextField>>,
    mut release: Local<bool>,
    mut keys: MessageWriter<KeyboardInput>,
    mut pressed: MessageWriter<ShellBackPressed>,
) {
    guard.last = guard.held;
    guard.held = false;
    let win = window.single().unwrap_or(Entity::PLACEHOLDER);
    let esc = |state| KeyboardInput { key_code: KeyCode::Escape, logical_key: Key::Escape, state, text: None, repeat: false, window: win };
    if std::mem::take(&mut *release) {
        keys.write(esc(ButtonState::Released));
        return;
    }
    // A right click on a text field is the field's menu (`text_menu`).
    if crate::text_menu::over_field(&fields) {
        return;
    }
    // A modal layer up (`modal`): right click is its Back, Esc as ever,
    // unless it is `Blocking`; the screen behind gets no Back.
    let top = modals.as_ref().and_then(|m| m.top());
    if top.is_some_and(|t| blocking.contains(t)) {
        return;
    }
    if (screens.is_empty() && top.is_none()) || guard.last || !pointer_free(&cursor) {
        return;
    }
    if top.is_some() {
        if mouse.is_some_and(|m| m.just_pressed(MouseButton::Right) && !m.pressed(MouseButton::Left)) {
            keys.write(esc(ButtonState::Pressed));
            *release = true;
        }
        return;
    }
    // The crumb's Interaction is last frame's (Bevy's focus runs later in
    // PreUpdate): pressed then means a click on it.
    let crumb = crumbs.iter().any(|i| *i == Interaction::Pressed);
    let right = mouse.is_some_and(|m| m.just_pressed(MouseButton::Right) && !m.pressed(MouseButton::Left));
    if crumb || right {
        keys.write(esc(ButtonState::Pressed));
        pressed.write(ShellBackPressed { right_click: right });
        *release = true;
    }
}

/// The breadcrumb's hover: its text to `fg` and the underline.
#[allow(clippy::type_complexity)]
pub fn crumb_hover(
    theme: Res<Theme>,
    crumbs: Query<(&Interaction, &Children), (With<ShellBack>, Changed<Interaction>)>,
    mut texts: Query<&mut crate::UiText>,
    mut lines: Query<&mut Visibility, With<CrumbLine>>,
) {
    let _ = theme;
    for (i, kids) in &crumbs {
        let on = *i != Interaction::None;
        for k in kids.iter() {
            if let Ok(mut t) = texts.get_mut(k) {
                let tone = if on { Tone::Fg } else { Tone::Fg2 };
                if t.tone != tone {
                    t.tone = tone;
                }
            }
            if let Ok(mut v) = lines.get_mut(k) {
                *v = if on { Visibility::Inherited } else { Visibility::Hidden };
            }
        }
    }
}

/// Seconds of hover before a tooltip shows.
pub const TIP_DELAY: f64 = 0.35;

/// Shows a hovered `Tip`'s plate after `TIP_DELAY` and takes it down when
/// the hover ends.
#[allow(clippy::too_many_arguments)]
pub fn show_tips(
    theme: Res<Theme>,
    kit: Option<Res<crate::KitFonts>>,
    units: Res<UiUnits>,
    clock: Res<UiClock>,
    tips: Query<(Entity, &Tip, &Interaction)>,
    plates: Query<(Entity, &TipPlate)>,
    (mut images, mut icons): (Option<ResMut<Assets<Image>>>, Option<ResMut<IconImages>>),
    mut since: Local<Option<(Entity, f64)>>,
    mut c: Commands,
) {
    let now = clock.now;
    let hovered = tips.iter().find(|(_, _, i)| **i != Interaction::None).map(|(e, ..)| e);
    match (hovered, *since) {
        (Some(e), Some((s, _))) if e == s => {}
        (Some(e), _) => *since = Some((e, now)),
        (None, _) => *since = None,
    }
    for (p, plate) in &plates {
        if Some(plate.0) != hovered {
            c.entity(p).despawn();
        }
    }
    let Some((e, t0)) = *since else { return };
    if now - t0 < TIP_DELAY || plates.iter().any(|(_, p)| p.0 == e) {
        return;
    }
    let Ok((_, tip, _)) = tips.get(e) else { return };
    let ui = Ui::new(&theme, kit.as_deref(), *units);
    // Under the node, right-aligned: a plain holder placed in percent, the
    // styled plate in it.
    let holder = c
        .spawn((
            TipPlate(e),
            Node {
                position_type: PositionType::Absolute,
                right: Val::ZERO,
                top: Val::Percent(100.0),
                padding: UiRect::top(units.val(0.5)),
                ..default()
            },
            GlobalZIndex(i32::MAX - 8),
            Pickable::IGNORE,
            ChildOf(e),
        ))
        .id();
    let plate = c
        .spawn((
            UiStyle { gap: Some(0.5), pad: Some([0.4, 0.6, 0.4, 0.6].into()), ..UiStyle::row() }.bg(Tone::Ink3).outline(Tone::Fg4, 1.0),
            Node::default(),
            BoxShadow::new(Color::srgba(0.0, 0.0, 0.0, 0.5), Val::ZERO, units.val(0.3), Val::ZERO, units.val(0.8)),
            Pickable::IGNORE,
            ChildOf(holder),
        ))
        .id();
    if let (Some(m), Some(images), Some(icons)) = (tip.card.as_deref(), images.as_deref_mut(), icons.as_deref_mut()) {
        // A party member's preview card: the avatar, the names, the tags.
        avatar::spawn(&ui, &mut c, plate, images, icons, m.face, m.marks, 3.4);
        let col = ui.boxed(&mut c, plate, UiStyle { gap: Some(0.3), justify: Some(JustifyContent::Center), ..UiStyle::column() });
        ui.text(&mut c, col, Role::DisplayBold, 1.0, Tone::Fg, m.display.as_deref().unwrap_or(&m.name));
        if m.display.as_deref().is_some_and(|d| d != m.name) {
            ui.text(&mut c, col, Role::Mono, 0.56, Tone::Fg3, &format!("@{}", m.name));
        }
        let tags = ui.boxed(&mut c, col, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), ..UiStyle::row() });
        for tag in [m.marks.you.then_some("You"), m.role.as_deref()].into_iter().flatten() {
            let t = ui.boxed(&mut c, tags, UiStyle { pad: Some([0.1, 0.3, 0.1, 0.3].into()), ..UiStyle::row() }.outline(Tone::Fg3, 1.0));
            ui.text(&mut c, t, Role::Mono, 0.5, Tone::Fg2, &tag.to_uppercase());
        }
        if let Some(ms) = m.ping {
            ui.text(&mut c, tags, Role::Mono, 0.54, Tone::Fg3, &format!("{ms} ms"));
        }
        return;
    }
    ui.text(&mut c, plate, Role::Mono, 0.6, Tone::Fg, &tip.name);
    if let Some(n) = &tip.note {
        ui.text(&mut c, plate, Role::Mono, 0.6, Tone::Fg3, n);
    }
    if let Some(k) = &tip.key {
        let cap = ui.boxed(&mut c, plate, UiStyle { pad: Some([0.12, 0.26, 0.12, 0.26].into()), ..UiStyle::row() }.outline(Tone::Fg3, 1.0));
        ui.text(&mut c, cap, Role::Mono, 0.5, Tone::Fg2, k);
    }
}

/// The shell's systems.
pub struct ShellPlugin;

impl Plugin for ShellPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BackGuard>()
            .init_resource::<Party>()
            .add_message::<KeyboardInput>()
            .add_message::<ShellBackPressed>()
            .add_systems(PreUpdate, back_keys.before(bevy::input::InputSystems))
            .add_systems(Update, (crumb_hover, show_tips));
    }
}

/// The main menu's ground under a menu screen (module doc, "The ground"):
/// the ambient across `parent`, taking no pointer. None without the
/// materials (headless): the screen's own colour shows.
pub fn menu_ground(c: &mut Commands, parent: Entity, t: &Theme, ambients: Option<&mut Assets<crate::AmbientMaterial>>) -> Option<Entity> {
    let amb = ambients?;
    let (col, bloom) = crate::AmbientMaterial::MENU;
    Some(
        c.spawn((
            MenuGround,
            MaterialNode(amb.add(crate::AmbientMaterial::new(t).column(Some(col)).bloom(bloom))),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id(),
    )
}

/// The ground's node (`menu_ground`).
#[derive(Component, Clone, Copy, Debug)]
pub struct MenuGround;
