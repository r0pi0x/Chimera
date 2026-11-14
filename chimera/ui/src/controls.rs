//! The kit's form controls (docs/plans/active/ugg-ui.md, M7; the "UGG
//! Components & Views" page's component table, its sizes in cq and its
//! tokens): what menus are built from.
//!
//! - `button` (`Kind`: primary white with a slanted right edge, secondary
//!   outlined, ghost, danger with the threat mark), an optional key cap;
//!   `tabs` (the settings' sub-tabs: the selected one white with a
//!   slanted underline); `row` (a label and a value, `min_h` 2.9 cq, a
//!   hairline under it) and `section` (a small header with the accent
//!   mark); `toggle` (a slab sliding on a slanted track, white when on);
//!   `segmented`; `slider` (track, fill, the default's mark, a slanted
//!   thumb, the value); `stepper` (`-`, the value, `+`); `dropdown` (a
//!   field and, open, its list); `key_field` (key caps, "press a key"
//!   while listening); `kv` (key/value lines); `modal` (a plate with a
//!   top bar, white or threat).
//! - **States** (`State`), one vocabulary everywhere: hover is the
//!   right-edge 12° tick plus a faint tint (`Hover`, shown live by
//!   `show_hover` from the pointer, no rebuild); focus is the same tick in
//!   accent and the accent bar at the left; selected is a white fill; a
//!   toggle that is on is white (cobalt stays for focus and changed rows:
//!   a changed row's accent square); threat red only as a mark (an error's
//!   underline, the danger button's mark), never a fill.
//! - **Wipes** (`Wipe`): a moving cover or bar on the 12° slant, clipped
//!   along it by its `Timeline` (`Prop::ClipLeft`/`ClipRight`), the
//!   menus' moves inside a screen (`wipe_cover`, `Wipe::bar`); the logo
//!   pass stays for screen changes.
//!
//! Builders spawn under a parent (`ChildOf`) and return the entity a
//! screen gives its action (with `clickable`), sized from `UiUnits` at
//! the time: screens rebuild when the units change, as the lobby does.
//! Text goes upper case (the UGG faces draw capitals).

use bevy::{
    prelude::*,
    ui::{DeprecatedInteraction as Interaction, FocusPolicy, RelativeCursorPosition},
};

use crate::{
    fonts::{self, KitFonts, Role},
    icons::IconImages,
    materials::SlantMaterial,
    motion::{Prop, Timeline, Track, UiClock},
    style::{UiSlant, UiStyle, UiText},
    theme::{Theme, Tone},
    units::UiUnits,
};

/// What the builders share: the theme, the faces, the units.
#[derive(Clone, Copy)]
pub struct Ui<'a> {
    pub theme: &'a Theme,
    pub kit: Option<&'a KitFonts>,
    pub units: UiUnits,
}

/// A control's state (module doc).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    /// The keyboard's focus.
    pub focus: bool,
    /// Changed and not applied (a row's accent square).
    pub changed: bool,
    /// Wrong (threat underline or bar, as a mark).
    pub error: bool,
    pub disabled: bool,
}

impl State {
    pub const NONE: State = State { focus: false, changed: false, error: false, disabled: false };

    pub fn focused(focus: bool) -> State {
        State { focus, ..State::NONE }
    }
}

/// A button's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// White fill, slanted right edge: the safe or main choice.
    Primary,
    /// Ink with an outline.
    Secondary,
    /// Text only.
    Ghost,
    /// Ink with a threat outline and the warning mark: it ends something.
    Danger,
}

/// A hoverable node's tint: its background `base`, `over` while hovered
/// (and pressed).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hover {
    pub base: Color,
    pub over: Color,
}

/// The right-edge tick: shown while its parent is hovered, or always
/// (`always`: the focus tick, in accent).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    pub always: bool,
}

/// What makes a node clickable: an `Interaction` that blocks the pointer.
pub fn clickable() -> (Interaction, FocusPolicy) {
    (Interaction::default(), FocusPolicy::Block)
}

/// Height of the default button, cq.
pub const BUTTON_H: f32 = 2.4;
/// A row's least height, cq.
pub const ROW_H: f32 = 2.9;

impl<'a> Ui<'a> {
    pub fn new(theme: &'a Theme, kit: Option<&'a KitFonts>, units: UiUnits) -> Self {
        Ui { theme, kit, units }
    }

    pub fn color(&self, t: Tone, a: f32) -> Color {
        self.theme.tone_a(t, a)
    }

    /// A styled box under `parent`.
    pub fn boxed(&self, c: &mut Commands, parent: Entity, s: UiStyle) -> Entity {
        c.spawn((s, Node::default(), ChildOf(parent))).id()
    }

    /// A text in a role at `size` cq (upper case), on one line.
    pub fn text(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        self.text_a(c, parent, role, size, tone, 1.0, text)
    }

    /// `text` with the tone at `alpha`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_a(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, alpha: f32, text: &str) -> Entity {
        c.spawn((
            Text::new(text.to_uppercase()),
            UiText { role, size, tone, alpha },
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ChildOf(parent),
        ))
        .id()
    }

    /// Text that wraps within its parent's width.
    pub fn paragraph(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        c.spawn((
            Text::new(text.to_uppercase()),
            UiText::new(role, size, tone),
            // Wrapped lines need room: 1.5 lines.
            TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            bevy::text::LineHeight::RelativeToFont(1.55),
            ChildOf(parent),
        ))
        .id()
    }

    /// A slanted fill (`UiSlant`) as an absolutely placed box: `left`,
    /// `top` in cq, `w` x `h` cq.
    pub fn slant_at(&self, c: &mut Commands, parent: Entity, slant: UiSlant, ltrb: [Option<f32>; 4], w: f32, h: f32) -> Entity {
        c.spawn((style_abs(ltrb, w, h), Node::default(), slant, Pickable::IGNORE, ChildOf(parent))).id()
    }

    /// The right-edge tick `right` cq in from the edge of a box `h` cq
    /// tall: white while hovered, or (focus) always and in accent.
    pub fn tick(&self, c: &mut Commands, parent: Entity, right: f32, focus: bool) -> Entity {
        let u = &self.units;
        let lean = 0.52 * u.cq(1.0) * self.theme.tan();
        let tone = if focus { Tone::Acc } else { Tone::Fg };
        c.spawn((
            Tick { always: focus },
            Node {
                position_type: PositionType::Absolute,
                right: u.val(right),
                top: percent(24.0),
                bottom: percent(24.0),
                width: Val::Px(u.snap(u.cq(0.22) + lean)),
                ..default()
            },
            UiSlant::new(tone, 1.0),
            if focus { Visibility::Inherited } else { Visibility::Hidden },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// The focus bar: accent, `w` cq wide down the left edge.
    pub fn focus_bar(&self, c: &mut Commands, parent: Entity, w: f32) -> Entity {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                top: Val::ZERO,
                bottom: Val::ZERO,
                width: self.units.val(w),
                ..default()
            },
            BackgroundColor(self.color(Tone::Acc, 1.0)),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// An underline `h` cq tall along the bottom (focus: accent; error:
    /// threat), slanted.
    pub fn underline(&self, c: &mut Commands, parent: Entity, tone: Tone, h: f32) -> Entity {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                right: Val::ZERO,
                bottom: Val::ZERO,
                height: self.units.val(h),
                ..default()
            },
            BackgroundColor(self.color(tone, 1.0)),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// A hairline along the bottom in `tone` at `alpha`.
    pub fn hairline(&self, c: &mut Commands, parent: Entity, tone: Tone, alpha: f32) -> Entity {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                right: Val::ZERO,
                bottom: Val::ZERO,
                height: Val::Px(self.units.hairline()),
                ..default()
            },
            BackgroundColor(self.color(tone, alpha)),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// A small slanted square in `tone`, `s` cq (the changed mark, a
    /// section's mark, a status light).
    pub fn mark(&self, c: &mut Commands, parent: Entity, tone: Tone, w: f32, h: f32) -> Entity {
        let lean = h * self.theme.tan();
        c.spawn((
            UiStyle { w: Some(w + lean), h: Some(h), shrink: Some(0.0), ..UiStyle::row() },
            Node::default(),
            UiSlant::new(tone, 1.0),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// A key cap (outlined, Mono), `size` cq text.
    pub fn kbd(&self, c: &mut Commands, parent: Entity, key: &str, size: f32, tone: Tone) -> Entity {
        let e = c
            .spawn((
                UiStyle { pad: Some([0.24, 0.36, 0.18, 0.36].into()), ..UiStyle::row() }.outline(tone, 0.75),
                Node::default(),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        self.text_a(c, e, Role::Mono, size * 0.88, tone, 0.75, key);
        e
    }

    /// The kit's icon `key` (`IconImages`), `h` cq tall, in `tone`.
    #[allow(clippy::too_many_arguments)]
    pub fn icon(&self, c: &mut Commands, parent: Entity, images: &mut Assets<Image>, icons: &mut IconImages, key: &str, h: f32, tone: Tone) -> Entity {
        let u = self.units;
        let px = u.phys(u.cq(h)).round().max(1.0) as u32;
        let aspect = icons.icons().get(key).map_or(1.0, |i| i.aspect());
        let image = icons.get(images, key, px).unwrap_or_default();
        c.spawn((
            ImageNode { image, color: self.theme.tone(tone), ..default() },
            Node {
                width: Val::Px((aspect * px as f32).ceil() / u.scale_factor),
                height: Val::Px(px as f32 / u.scale_factor),
                flex_shrink: 0.0,
                ..default()
            },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// The lockup at `lk` cq (the wordmark's size): the mark, ULTIMATE
    /// spread over the wordmark's width, GUN GAME in the wordmark role.
    pub fn lockup(&self, c: &mut Commands, parent: Entity, images: &mut Assets<Image>, icons: &mut IconImages, lk: f32) -> Entity {
        let l = self.boxed(c, parent, UiStyle { gap: Some(0.55 * lk), shrink: Some(0.0), ..UiStyle::row() });
        self.icon(c, l, images, icons, "mark", 2.05 * lk, Tone::Fg);
        let words = self.boxed(c, l, UiStyle { gap: Some(0.3 * lk), ..UiStyle::column() });
        let ult = self.boxed(c, words, UiStyle::row().justify(JustifyContent::SpaceBetween));
        for ch in "ULTIMATE".chars() {
            self.text(c, ult, Role::DisplayRegular, 0.62 * lk, Tone::Fg2, &ch.to_string());
        }
        self.text(c, words, Role::Wordmark, lk, Tone::Fg, "Gun game");
        l
    }

    // -----------------------------------------------------------------
    // Buttons and tabs.

    /// A button, `h` cq tall (`BUTTON_H`; small 1.9, large 3.2), its label
    /// and an optional key cap. Returns the button (give it its action).
    #[allow(clippy::too_many_arguments)]
    pub fn button(&self, c: &mut Commands, parent: Entity, kind: Kind, label: &str, key: Option<&str>, h: f32, st: State) -> Entity {
        let size = if h < 2.2 { 0.58 } else if h > 3.0 { 0.8 } else { 0.66 };
        let lean = h * self.theme.tan();
        let right = match kind {
            Kind::Primary => 1.5 + lean,
            Kind::Ghost => 0.9,
            _ => 1.5,
        };
        let left = if kind == Kind::Ghost { 0.6 } else { 1.05 };
        let (base, over, text_tone) = match kind {
            _ if st.disabled => (self.color(Tone::Fg, 0.05), self.color(Tone::Fg, 0.05), Tone::Fg3),
            Kind::Primary => (Color::NONE, Color::NONE, Tone::Ink),
            Kind::Secondary | Kind::Danger => (self.color(Tone::Ink, 0.85), self.color(Tone::Fg, 0.06), Tone::Fg),
            Kind::Ghost => (Color::NONE, self.color(Tone::Fg, 0.06), Tone::Fg2),
        };
        let mut s = UiStyle { h: Some(h), pad: Some([0.0, right, 0.0, left].into()), gap: Some(0.8), shrink: Some(0.0), ..UiStyle::row() };
        s = match kind {
            _ if st.disabled => s.outline(Tone::Fg5, 1.0),
            Kind::Secondary => s.outline(Tone::Fg3, 1.0),
            Kind::Danger => s.outline(Tone::Threat, 0.55),
            _ => s,
        };
        let b = c
            .spawn((s, Node::default(), BackgroundColor(base), Hover { base, over }, clickable(), ChildOf(parent)))
            .id();
        if kind == Kind::Primary && !st.disabled {
            // The white slab behind, its right edge on the slant.
            c.spawn((
                Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() },
                UiSlant::new(Tone::Fg, 1.0).edges(false, true),
                Pickable::IGNORE,
                ChildOf(b),
            ));
        }
        if kind == Kind::Danger {
            self.mark(c, b, Tone::Threat, 0.5, 0.7);
        }
        self.text(c, b, Role::Mono, size, text_tone, label);
        if let Some(k) = key {
            self.kbd(c, b, k, size, text_tone);
        }
        if st.focus && !st.disabled {
            self.focus_bar(c, b, 0.24);
        }
        if !st.disabled {
            self.tick(c, b, if kind == Kind::Primary { 0.45 + lean } else { 0.45 }, st.focus);
        }
        if st.error {
            self.underline(c, b, Tone::Threat, 0.18);
        }
        b
    }

    /// Sub-tabs (the settings' pages): Display .78 cq, the selected one
    /// white with a slanted white underline, a focused one's underline in
    /// accent, others `fg_3`; a hairline under all. Returns each tab.
    pub fn tabs(&self, c: &mut Commands, parent: Entity, names: &[&str], selected: usize, focus: bool) -> Vec<Entity> {
        let bar = c
            .spawn((UiStyle { gap: Some(1.6), align: Some(AlignItems::FlexEnd), ..UiStyle::row() }, Node::default(), ChildOf(parent)))
            .id();
        self.hairline(c, bar, Tone::Fg4, 1.0);
        names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let sel = i == selected;
                let tone = if sel { Tone::Fg } else { Tone::Fg3 };
                let t = c
                    .spawn((
                        UiStyle { pad: Some([0.0, 0.0, 0.7, 0.0].into()), ..UiStyle::column() },
                        Node::default(),
                        BackgroundColor(Color::NONE),
                        Hover { base: Color::NONE, over: Color::NONE },
                        clickable(),
                        ChildOf(bar),
                    ))
                    .id();
                self.text(c, t, Role::DisplayRegular, 0.78, tone, n);
                if sel {
                    c.spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::ZERO,
                            right: Val::ZERO,
                            bottom: Val::ZERO,
                            height: self.units.val(0.24),
                            ..default()
                        },
                        UiSlant::new(if focus { Tone::Acc } else { Tone::Fg }, 1.0),
                        Pickable::IGNORE,
                        ChildOf(t),
                    ));
                } else {
                    // Hover: a short underline (the tick's place in a tab).
                    c.spawn((
                        Tick { always: false },
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::ZERO,
                            bottom: Val::ZERO,
                            width: percent(18.0),
                            height: self.units.val(0.24),
                            ..default()
                        },
                        UiSlant::new(Tone::Fg3, 1.0),
                        Visibility::Hidden,
                        Pickable::IGNORE,
                        ChildOf(t),
                    ));
                }
                t
            })
            .collect()
    }

    // -----------------------------------------------------------------
    // Rows.

    /// A settings row: the label (Display .8 cq) with an optional small
    /// note, the value at the right. Focus: tint .06, the accent bar and
    /// tick; changed: the accent square before the label; error: the
    /// threat bar at the left. Returns (row, value box).
    pub fn row(&self, c: &mut Commands, parent: Entity, label: &str, small: Option<&str>, min_h: f32, st: State) -> (Entity, Entity) {
        let base = if st.focus { self.color(Tone::Fg, 0.06) } else { Color::NONE };
        let over = if st.focus { base } else { self.color(Tone::Fg, 0.035) };
        let row = c
            .spawn((
                UiStyle { pad: Some([0.0, 1.3, 0.0, 1.1].into()), gap: Some(1.2), shrink: Some(0.0), ..UiStyle::row() }
                    .justify(JustifyContent::SpaceBetween),
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                clickable(),
                ChildOf(parent),
            ))
            .id();
        // `min_h` (UiStyle has no min height).
        c.entity(row).insert(MinHeight(min_h));
        self.hairline(c, row, Tone::Fg, 0.07);
        let lb = self.boxed(c, row, UiStyle { gap: Some(0.6), shrink: Some(1.0), ..UiStyle::row() });
        if st.changed {
            self.mark(c, lb, Tone::Acc, 0.42, 0.42);
        }
        self.text(c, lb, Role::DisplayRegular, 0.8, if st.disabled { Tone::Fg3 } else { Tone::Fg }, label);
        if let Some(sm) = small {
            self.text(c, lb, Role::Mono, 0.52, Tone::Fg3, sm);
        }
        let vl = self.boxed(c, row, UiStyle { gap: Some(0.6), shrink: Some(0.0), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
        if st.error {
            // Threat as a mark: the bar at the left.
            let bar = self.focus_bar(c, row, 0.24);
            c.entity(bar).insert(BackgroundColor(self.color(Tone::Threat, 1.0)));
        } else if st.focus {
            self.focus_bar(c, row, 0.24);
        }
        self.tick(c, row, 0.35, st.focus);
        (row, vl)
    }

    /// A section header: the accent mark, the title in Mono .56 `fg_3`, a
    /// hairline to the right.
    pub fn section(&self, c: &mut Commands, parent: Entity, title: &str, note: Option<&str>) -> Entity {
        let h = self.boxed(c, parent, UiStyle { gap: Some(0.7), margin: Some([1.2, 0.0, 0.5, 0.0].into()), pad: Some([0.0, 0.0, 0.0, 1.1].into()), shrink: Some(0.0), ..UiStyle::row() });
        self.mark(c, h, Tone::Acc, 0.42, 0.7);
        self.text(c, h, Role::Mono, 0.56, Tone::Fg3, title);
        if let Some(n) = note {
            self.text(c, h, Role::Mono, 0.56, Tone::Fg2, n);
        }
        let line = self.boxed(c, h, UiStyle { h: None, grow: Some(1.0), ..UiStyle::row() });
        c.entity(line).insert((MinHeight(0.0),));
        c.spawn((Node { width: percent(100.0), height: Val::Px(self.units.hairline()), ..default() }, BackgroundColor(self.color(Tone::Fg, 0.07)), ChildOf(line)));
        h
    }

    // -----------------------------------------------------------------
    // Values.

    /// A toggle: a slab sliding on a slanted track 4.4 x 1.5 cq, OFF/ON
    /// under it; on, the slab is white at the right. Returns the toggle.
    pub fn toggle(&self, c: &mut Commands, parent: Entity, on: bool, st: State) -> Entity {
        let t = c
            .spawn((
                UiStyle { w: Some(4.4), h: Some(1.5), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(Color::NONE),
                Hover { base: Color::NONE, over: Color::NONE },
                clickable(),
                ChildOf(parent),
            ))
            .id();
        let full = |c: &mut Commands, s: UiSlant| {
            c.spawn((
                Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, ..default() },
                s,
                Pickable::IGNORE,
                ChildOf(t),
            ));
        };
        full(c, UiSlant::new(Tone::Fg, 0.05));
        full(c, UiSlant::new(if st.focus { Tone::Fg2 } else { Tone::Fg4 }, 1.0).outline(self.units.phys(self.units.hairline())));
        // The slab: half the track less its insets.
        let slab = UiSlant::new(if on { Tone::Fg } else { Tone::Fg3 }, 1.0);
        let left = if on { 2.25 } else { 0.25 };
        self.slant_at(c, t, slab, [Some(left), Some(0.2), None, None], 1.9, 1.1);
        let labels = self.boxed(
            c,
            t,
            UiStyle { abs: Some([Some(0.0), Some(0.0), Some(0.0), Some(0.0)]), justify: Some(JustifyContent::SpaceAround), ..UiStyle::row() },
        );
        c.entity(labels).insert(Pickable::IGNORE);
        self.text(c, labels, Role::Mono, 0.5, if on { Tone::Fg3 } else { Tone::Ink }, "Off");
        self.text(c, labels, Role::Mono, 0.5, if on { Tone::Ink } else { Tone::Fg3 }, "On");
        if st.focus {
            self.underline(c, t, Tone::Acc, 0.2);
        }
        t
    }

    /// A segmented choice: outlined, each option Mono .58 cq; the selected
    /// one white (ink text); focus underlines it in accent. Returns the
    /// options.
    pub fn segmented(&self, c: &mut Commands, parent: Entity, options: &[&str], selected: Option<usize>, st: State) -> Vec<Entity> {
        let g = c
            .spawn((UiStyle { shrink: Some(0.0), ..UiStyle::row() }.bg_a(Tone::Ink, 0.5).outline(Tone::Fg4, 1.0), Node::default(), ChildOf(parent)))
            .id();
        options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let sel = selected == Some(i);
                let base = if sel { self.color(Tone::Fg, 1.0) } else { Color::NONE };
                let over = if sel { base } else { self.color(Tone::Fg, 0.06) };
                let b = c
                    .spawn((
                        UiStyle { h: Some(1.7), pad: Some([0.0, 0.75].into()), ..UiStyle::row() },
                        Node::default(),
                        BackgroundColor(base),
                        Hover { base, over },
                        clickable(),
                        ChildOf(g),
                    ))
                    .id();
                self.text(c, b, Role::Mono, 0.58, if sel { Tone::Ink } else { Tone::Fg2 }, o);
                if sel && st.focus {
                    self.underline(c, b, Tone::Acc, 0.2);
                }
                b
            })
            .collect()
    }

    /// A slider `w` cq wide: the track (1.6 cq tall: a .26 cq line in
    /// `fg_4`, the fill in white to `frac`, the default's 1 px mark), a
    /// slanted thumb (accent when focused), the value (Mono .66, its
    /// underline accent when focused). Returns the track (give it the
    /// action; `RelativeCursorPosition` says where it was pressed).
    #[allow(clippy::too_many_arguments)]
    pub fn slider(&self, c: &mut Commands, parent: Entity, frac: f32, mark: Option<f32>, value: &str, w: f32, st: State) -> Entity {
        let frac = frac.clamp(0.0, 1.0);
        let s = self.boxed(c, parent, UiStyle { w: Some(w), gap: Some(0.9), shrink: Some(0.0), ..UiStyle::row() });
        let tr = c
            .spawn((
                UiStyle { h: Some(1.6), grow: Some(1.0), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(Color::NONE),
                Hover { base: Color::NONE, over: Color::NONE },
                RelativeCursorPosition::default(),
                clickable(),
                ChildOf(s),
            ))
            .id();
        let line = |c: &mut Commands, w: Val, tone: Tone| {
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::ZERO,
                    width: w,
                    top: Val::Px(self.units.snap(self.units.cq(0.8 - 0.13))),
                    height: self.units.val(0.26),
                    ..default()
                },
                BackgroundColor(self.color(tone, 1.0)),
                Pickable::IGNORE,
                ChildOf(tr),
            ));
        };
        line(c, percent(100.0), Tone::Fg4);
        line(c, percent(frac * 100.0), Tone::Fg);
        if let Some(d) = mark {
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: percent(d.clamp(0.0, 1.0) * 100.0),
                    top: percent(18.0),
                    bottom: percent(18.0),
                    width: Val::Px(self.units.hairline()),
                    ..default()
                },
                BackgroundColor(self.color(Tone::Fg3, 1.0)),
                Pickable::IGNORE,
                ChildOf(tr),
            ));
        }
        let lean = 1.6 * self.theme.tan();
        let thumb_w = 0.62 + lean;
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: percent(frac * 100.0),
                margin: UiRect::left(self.units.val(-thumb_w / 2.0)),
                top: Val::ZERO,
                bottom: Val::ZERO,
                width: self.units.val(thumb_w),
                ..default()
            },
            UiSlant::new(if st.focus { Tone::Acc } else { Tone::Fg }, 1.0),
            Pickable::IGNORE,
            ChildOf(tr),
        ));
        let nv = self.boxed(c, s, UiStyle { min_w: Some(3.4), h: Some(1.6), pad: Some([0.0, 0.4].into()), justify: Some(JustifyContent::FlexEnd), ..UiStyle::row() });
        self.text(c, nv, Role::Mono, 0.66, Tone::Fg, value);
        if st.focus {
            self.underline(c, nv, Tone::Acc, 0.18);
        } else if st.error {
            self.underline(c, nv, Tone::Threat, 0.18);
        } else {
            self.hairline(c, nv, Tone::Fg4, 1.0);
        }
        tr
    }

    /// A stepper: `-`, the value (`w` cq at least), `+`, on slanted
    /// outlined keys 1.9 x 1.7 cq. Returns (minus, plus).
    pub fn stepper(&self, c: &mut Commands, parent: Entity, value: &str, w: f32, st: State) -> (Entity, Entity) {
        let g = self.boxed(c, parent, UiStyle { gap: Some(0.25), shrink: Some(0.0), ..UiStyle::row() });
        let key = |c: &mut Commands, label: &str| {
            let b = c
                .spawn((
                    UiStyle { w: Some(1.9), h: Some(1.7), justify: Some(JustifyContent::Center), ..UiStyle::row() },
                    Node::default(),
                    BackgroundColor(Color::NONE),
                    Hover { base: Color::NONE, over: Color::NONE },
                    clickable(),
                    ChildOf(g),
                ))
                .id();
            c.spawn((
                Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, ..default() },
                UiSlant::new(Tone::Fg4, 1.0).outline(self.units.phys(self.units.hairline())),
                Pickable::IGNORE,
                ChildOf(b),
            ));
            self.text(c, b, Role::Mono, 0.8, Tone::Fg, label);
            b
        };
        let minus = key(c, "-");
        let out = self.boxed(c, g, UiStyle { min_w: Some(w), pad: Some([0.0, 0.3].into()), justify: Some(JustifyContent::Center), ..UiStyle::row() });
        self.text(c, out, Role::Mono, 0.7, Tone::Fg, value);
        if st.focus {
            self.underline(c, out, Tone::Acc, 0.18);
        }
        let plus = key(c, "+");
        (minus, plus)
    }

    /// A dropdown `w` cq wide: the field (1.9 cq, a faint fill, a hairline
    /// under it, accent when focused; white when open) and, when `open`,
    /// its list below (the chosen option's accent cut, the active one's
    /// accent tick). Returns (field, options).
    #[allow(clippy::too_many_arguments)]
    pub fn dropdown(
        &self,
        c: &mut Commands,
        parent: Entity,
        options: &[&str],
        selected: usize,
        open: Option<usize>,
        w: f32,
        st: State,
    ) -> (Entity, Vec<Entity>) {
        let holder = self.boxed(c, parent, UiStyle { w: Some(w), shrink: Some(0.0), ..UiStyle::column() });
        let (base, over) = match open {
            Some(_) => (self.color(Tone::Fg, 1.0), self.color(Tone::Fg, 1.0)),
            None => (self.color(Tone::Fg, 0.04), self.color(Tone::Fg, 0.07)),
        };
        let f = c
            .spawn((
                UiStyle { h: Some(1.9), gap: Some(0.6), pad: Some([0.0, 0.8].into()), justify: Some(JustifyContent::SpaceBetween), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                clickable(),
                ChildOf(holder),
            ))
            .id();
        let ink = open.is_some();
        self.text(c, f, Role::Mono, 0.66, if ink { Tone::Ink } else { Tone::Fg }, options.get(selected).copied().unwrap_or("-"));
        // The arrow: a small slanted bar standing in for the triangle.
        self.mark(c, f, if ink { Tone::Ink } else { Tone::Fg2 }, 0.5, 0.5);
        if open.is_none() {
            if st.focus {
                self.underline(c, f, Tone::Acc, 0.2);
            } else {
                self.hairline(c, f, Tone::Fg3, 1.0);
            }
        }
        let mut out = Vec::new();
        if let Some(active) = open {
            let list = c
                .spawn((
                    UiStyle { abs: Some([Some(0.0), Some(1.9), Some(0.0), None]), pad: Some([0.3, 0.0].into()), ..UiStyle::column() }
                        .bg(Tone::Ink2)
                        .outline(Tone::Fg4, 1.0),
                    Node::default(),
                    GlobalZIndex(950),
                    ChildOf(holder),
                ))
                .id();
            for (i, o) in options.iter().enumerate() {
                let act = i == active;
                let base = if act { self.color(Tone::Fg, 0.07) } else { Color::NONE };
                let e = c
                    .spawn((
                        UiStyle { h: Some(1.8), gap: Some(0.6), pad: Some([0.0, 1.4, 0.0, 0.8].into()), ..UiStyle::row() },
                        Node::default(),
                        BackgroundColor(base),
                        Hover { base, over: self.color(Tone::Fg, 0.07) },
                        clickable(),
                        ChildOf(list),
                    ))
                    .id();
                let cut = if i == selected { Tone::Acc } else { Tone::Fg5 };
                self.mark(c, e, cut, 0.36, 0.36);
                self.text(c, e, Role::Mono, 0.62, if i == selected || act { Tone::Fg } else { Tone::Fg2 }, o);
                self.tick(c, e, 0.45, act);
                out.push(e);
            }
        }
        (f, out)
    }

    /// A key-binding field (min `w` cq, 1.8 cq tall): the keys as caps
    /// (the first a white slanted slab, others outlined), a dash for none;
    /// listening, an accent tint and outline and "Press a key, button or
    /// wheel • Esc: cancel". Returns the field.
    pub fn key_field(&self, c: &mut Commands, parent: Entity, keys: &[String], listening: bool, w: f32, st: State) -> Entity {
        let (base, over) = if listening {
            (self.color(Tone::Acc, 0.16), self.color(Tone::Acc, 0.16))
        } else {
            (self.color(Tone::Fg, 0.04), self.color(Tone::Fg, 0.08))
        };
        let mut s = UiStyle { min_w: Some(w), h: Some(1.8), gap: Some(0.35), pad: Some([0.0, 1.2, 0.0, 0.55].into()), shrink: Some(0.0), ..UiStyle::row() };
        if listening {
            s = s.outline(Tone::Acc, 1.0);
        }
        let f = c.spawn((s, Node::default(), BackgroundColor(base), Hover { base, over }, clickable(), ChildOf(parent))).id();
        if listening {
            self.mark(c, f, Tone::Acc, 0.36, 0.8);
            self.text(c, f, Role::Mono, 0.58, Tone::Fg, "Press a key, button or wheel • Esc: cancel");
        } else if keys.is_empty() {
            self.text(c, f, Role::Mono, 0.62, Tone::Fg3, "-");
        } else {
            for (i, k) in keys.iter().enumerate() {
                let cap = c
                    .spawn((
                        UiStyle {
                            h: Some(1.2),
                            min_w: Some(1.25),
                            pad: Some([0.0, if i == 0 { 0.5 } else { 0.35 }, 0.0, 0.35].into()),
                            justify: Some(JustifyContent::Center),
                            ..UiStyle::row()
                        },
                        Node::default(),
                        Pickable::IGNORE,
                        ChildOf(f),
                    ))
                    .id();
                if i == 0 {
                    c.spawn((
                        Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, ..default() },
                        UiSlant::new(Tone::Fg, 1.0).edges(false, true),
                        Pickable::IGNORE,
                        ChildOf(cap),
                    ));
                } else {
                    c.entity(cap).insert(UiStyle {
                        h: Some(1.2),
                        min_w: Some(1.25),
                        pad: Some([0.0, 0.35].into()),
                        justify: Some(JustifyContent::Center),
                        ..UiStyle::row()
                    }
                    .outline(Tone::Fg3, 1.0));
                }
                self.text(c, cap, Role::Mono, 0.58, if i == 0 { Tone::Ink } else { Tone::Fg2 }, k);
            }
        }
        if st.error {
            self.underline(c, f, Tone::Threat, 0.2);
        } else if st.focus && !listening {
            self.underline(c, f, Tone::Acc, 0.2);
        }
        if !listening {
            self.tick(c, f, 0.35, false);
        }
        f
    }

    /// Key/value lines (the help pane's console name, default, range):
    /// keys Mono .6 `fg_3` in a `kw` cq column, values `fg`, a hairline
    /// under each.
    pub fn kv(&self, c: &mut Commands, parent: Entity, pairs: &[(&str, String)], kw: f32) -> Entity {
        let list = self.boxed(c, parent, UiStyle::column());
        for (k, v) in pairs {
            let line = self.boxed(c, list, UiStyle { gap: Some(1.0), pad: Some([0.6, 0.0].into()), ..UiStyle::row() });
            self.hairline(c, line, Tone::Fg, 0.07);
            let kc = self.boxed(c, line, UiStyle { w: Some(kw), shrink: Some(0.0), ..UiStyle::row() });
            self.text(c, kc, Role::Mono, 0.6, Tone::Fg3, k);
            self.text(c, line, Role::Mono, 0.6, Tone::Fg, v);
        }
        list
    }

    /// A modal's plate `w` cq wide (`Ink2`, an outline, a .22 cq top bar:
    /// white for a question, threat for one that ends something), centred
    /// in `parent`. Returns the plate.
    pub fn modal(&self, c: &mut Commands, parent: Entity, w: f32, danger: bool) -> Entity {
        let m = c
            .spawn((
                UiStyle { w: Some(w), pad: Some([1.8, 2.0, 1.7, 2.0].into()), ..UiStyle::column() }.bg(Tone::Ink2).outline(Tone::Fg4, 1.0),
                Node::default(),
                FocusPolicy::Block,
                ChildOf(parent),
            ))
            .id();
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, height: self.units.val(0.22), ..default() },
            BackgroundColor(self.color(if danger { Tone::Threat } else { Tone::Fg }, 1.0)),
            ChildOf(m),
        ));
        m
    }
}

fn style_abs(ltrb: [Option<f32>; 4], w: f32, h: f32) -> UiStyle {
    UiStyle { w: Some(w), h: Some(h), ..UiStyle::row() }.abs(ltrb)
}

/// A node's least height in cq (rows): `resolve_min_heights` writes it
/// after the style.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct MinHeight(pub f32);

/// A slider's value at a press on its track: 0-1 from the cursor
/// (`RelativeCursorPosition`'s -0.5..0.5).
pub fn slider_frac(pos: &RelativeCursorPosition) -> Option<f32> {
    pos.normalized.map(|n| (n.x + 0.5).clamp(0.0, 1.0))
}

/// A value's place in `min..=max` (0-1).
pub fn frac_of(value: f32, min: f32, max: f32) -> f32 {
    if max > min { ((value - min) / (max - min)).clamp(0.0, 1.0) } else { 0.0 }
}

/// The value at `frac` of `min..=max`, on `step`.
pub fn value_at(frac: f32, min: f32, max: f32, step: f32) -> f32 {
    let v = min + frac.clamp(0.0, 1.0) * (max - min);
    let v = if step > 0.0 { min + ((v - min) / step).round() * step } else { v };
    ((v.clamp(min, max)) * 100.0).round() / 100.0
}

// ---------------------------------------------------------------------
// Wipes.

/// A slanted node wiped by its `Timeline` (`Prop::ClipLeft`,
/// `ClipRight`): its `SlantMaterial`'s clip follows; `despawn_done`: gone
/// when the timeline ends (a cover wiped away).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Wipe {
    pub despawn_done: bool,
}

impl<'a> Ui<'a> {
    /// A cover in `tone` over `parent` (absolute, full size) that wipes
    /// away to the right along the slant over `dur` s from `since` on the
    /// UI clock: the move inside a screen (a page change). Nothing when
    /// it is already over at `now`.
    pub fn wipe_cover(&self, c: &mut Commands, parent: Entity, tone: Tone, since: f64, now: f64, dur: f32) -> Option<Entity> {
        if (now - since) as f32 >= dur {
            return None;
        }
        let e = c
            .spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::ZERO,
                    right: Val::ZERO,
                    top: Val::ZERO,
                    bottom: Val::ZERO,
                    ..default()
                },
                UiSlant::new(tone, 1.0).edges(false, false),
                Timeline::new([Track::new(Prop::ClipLeft, 0.0, 1.0).dur(dur).ease(crate::motion::Ease::OUT)]).started(since),
                Wipe { despawn_done: true },
                ZIndex(20),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        Some(e)
    }
}

/// The wipes' clip from their timelines (module doc).
pub fn apply_wipes(
    clock: Res<UiClock>,
    wipes: Query<(Entity, &Wipe, &Timeline, Option<&MaterialNode<SlantMaterial>>)>,
    mut mats: Option<ResMut<Assets<SlantMaterial>>>,
    mut commands: Commands,
) {
    for (e, w, tl, mat) in &wipes {
        if w.despawn_done && tl.done(clock.now) {
            commands.entity(e).despawn();
            continue;
        }
        let (Some(mat), Some(mats)) = (mat, mats.as_mut()) else { continue };
        let left = tl.value_or(Prop::ClipLeft, 0.0);
        let right = tl.value_or(Prop::ClipRight, 0.0);
        if let Some(m) = mats.get(&mat.0)
            && (m.wipe.x != left || m.wipe.y != right)
            && let Some(mut m) = mats.get_mut(&mat.0)
        {
            m.set_clip(left, right);
            m.line.y = 1.0;
        }
    }
}

/// Hover tints and ticks from the pointer (module doc).
pub fn show_hover(
    mut nodes: Query<(&Interaction, Option<&Hover>, Option<&Children>, Option<&mut BackgroundColor>), Changed<Interaction>>,
    mut ticks: Query<(&Tick, &mut Visibility)>,
) {
    for (i, hover, children, bg) in &mut nodes {
        let over = *i != Interaction::None;
        if let (Some(h), Some(mut bg)) = (hover, bg) {
            let want = if over { h.over } else { h.base };
            if bg.0 != want {
                bg.0 = want;
            }
        }
        if let Some(children) = children {
            for ch in children.iter() {
                if let Ok((t, mut v)) = ticks.get_mut(ch) {
                    v.set_if_neq(if over || t.always { Visibility::Inherited } else { Visibility::Hidden });
                }
            }
        }
    }
}

/// `MinHeight`'s least height after the style's `Node`.
pub fn resolve_min_heights(units: Res<UiUnits>, mut nodes: Query<(Ref<MinHeight>, Ref<UiStyle>, &mut Node)>) {
    for (m, s, mut n) in &mut nodes {
        if units.is_changed() || m.is_changed() || s.is_changed() || n.is_changed() {
            let v = units.val(m.0);
            if n.min_height != v {
                n.min_height = v;
            }
        }
    }
}

/// The controls' systems (in the kit's plugin).
pub struct ControlsPlugin;

impl Plugin for ControlsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_hover).add_systems(
            PostUpdate,
            (resolve_min_heights.after(crate::style::resolve_styles), apply_wipes.after(crate::motion::step_timelines))
                .in_set(crate::UiKitSystems),
        );
    }
}

/// The text a role takes at `size` cq (for measuring in tests).
pub fn styled(ui: &Ui, role: Role, size: f32) -> fonts::Styled {
    fonts::text(ui.theme, ui.kit, role, ui.units.text_px(size))
}
