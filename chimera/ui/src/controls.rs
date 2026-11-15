//! The kit's form controls (docs/plans/active/ugg-ui.md, M7; the "UGG
//! Components & Views" page's component table, its sizes in cq and its
//! tokens): what menus are built from.
//!
//! - `button` (`Kind`: primary white with a slanted right edge, secondary
//!   outlined, ghost, danger with the threat mark), an optional key cap;
//!   `tabs` (the settings' sub-tabs: the selected one white with a
//!   slanted underline); `row` (a label and a value, `min_h` 2.9 cq, a
//!   hairline under it) and `section` (a small header with the accent
//!   mark); `toggle` (a slab on a slanted track, white when on, each
//!   label centred on its slab's place; `toggle_ex` slides the slab over,
//!   `ToggleSlide`, 140 ms eased, none under reduced motion; focus a
//!   strip clipped to the track's bottom edge);
//!   `segmented`; `slider` (track, fill, the default's mark, a slanted
//!   thumb, the value); `stepper` (`-`, the value, `+`); their `_ex`
//!   forms make the value box clickable and, while `Typing`, typed into
//!   in place (a number, checked by `typed_number`: any finite number
//!   inside the setting's hard limits); `dropdown` (a field with the
//!   upright `chevron` icon, flipped up while open, and, open, its list);
//!   `key_field` (key caps, "press a key" while listening); `text_field`
//!   (typed text or stars, a placeholder, the caret in accent while it
//!   has the keys, the selection on an accent tint; its keys, copy, cut
//!   and paste are the screen's: `chimera_client::client::widgets::
//!   text_key`, `clipboard`); `kv` (key/value lines); `modal` (a plate
//!   with a top bar, white or threat).
//! - **Text** in a value box or a text field (`edit_chars`): one node a
//!   character, at rest and typing alike, so opening one moves nothing
//!   (the same box, face and size; typed text in `Role::Text` at
//!   `TYPED`, stars and numbers in Mono); the caret a thin overlay (one
//!   physical pixel, two over 1440 lines) just after the character
//!   before it, taking no room; the selection a tint behind its
//!   characters. `TextHits` finds the boundary under the pointer from the
//!   layout; `TextPress` makes a press place the caret, a drag select and
//!   a double press select all (the screens keep one).
//! - **States** (`State`; the kit's design rules, 2026-11-14): hover is a
//!   faint tint (`Hover`, shown live by `show_hover` from the pointer, no
//!   rebuild), the element's own fill lifting: no tick anywhere (the
//!   user's call; a sub-tab's short underline stays); focus is the accent bar at the left (a
//!   control's own accent detail on its shape); selected is a white fill;
//!   a toggle that is on is white (cobalt stays for focus and changed
//!   rows: a changed row's accent square); threat red only as a mark (an
//!   error's underline, the danger button's mark), never a fill.
//! - **Wipes** (`Wipe`): a moving cover or bar on the 12° slant, clipped
//!   along it by its `Timeline` (`Prop::ClipLeft`/`ClipRight`), the
//!   menus' moves inside a screen (`wipe_cover`, `Wipe::bar`); the logo
//!   pass stays for screen changes.
//!
//! Builders spawn under a parent (`ChildOf`) and return the entity a
//! screen gives its action (with `clickable`), sized from `UiUnits` at
//! the time: screens rebuild when the units change, as the lobby does.
//! Labels go upper case (`text`); sentences (`paragraph`) and what a
//! person types or is called (`text_field`, `text_plain`) keep their case,
//! in the sentence face (`Role::Text`), ours like every other text.

use bevy::{
    prelude::*,
    ui::{ComputedNode, DeprecatedInteraction as Interaction, FocusPolicy, RelativeCursorPosition, UiGlobalTransform, Val2},
};

use crate::{
    fonts::{self, KitFonts, Role},
    icons::IconImages,
    materials::SlantMaterial,
    motion::{Prop, Timeline, Track, UiClock},
    style::{self, UiSlant, UiStyle, UiText},
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

/// A hover mark shown while its parent is hovered, or always (`always`):
/// the sub-tabs' short underline (rows, lines, buttons and fields have no
/// tick: their own fill lifts).
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
/// Typed text's size in a field, cq (`Role::Text`).
pub const TYPED: f32 = 0.74;
/// A sentence's size, cq (`Role::Text`: descriptions, help, dialog
/// bodies, notices).
pub const SENTENCE: f32 = 0.86;
/// A small sentence's size, cq (`Role::Text`: a dialog's footnote, a
/// field's hint, a status line under a list).
pub const NOTE: f32 = 0.72;

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
        self.text_with_case(c, parent, role, size, tone, alpha, &text.to_uppercase())
    }

    /// `text` as it is (a sentence, a name, what a person types in a
    /// field, a cvar name in Mono), in a role at `size` cq, on one line.
    /// Labels use `text` (capitals).
    pub fn text_plain(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        self.text_plain_a(c, parent, role, size, tone, 1.0, text)
    }

    /// A sentence on one line in its own case (`Role::Text` at `size` cq:
    /// `SENTENCE`, `NOTE`).
    pub fn sentence(&self, c: &mut Commands, parent: Entity, size: f32, tone: Tone, text: &str) -> Entity {
        self.text_with_case(c, parent, Role::Text, size, tone, 1.0, text)
    }

    /// `text_plain` with the tone at `alpha`.
    #[allow(clippy::too_many_arguments)]
    pub fn text_plain_a(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, alpha: f32, text: &str) -> Entity {
        let e = self.text_with_case(c, parent, role, size, tone, alpha, text);
        c.entity(e).insert(style::KeepCase);
        e
    }

    #[allow(clippy::too_many_arguments)]
    pub fn text_with_case(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, alpha: f32, text: &str) -> Entity {
        c.spawn((
            Text::new(text),
            UiText { role, size, tone, alpha },
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            ChildOf(parent),
        ))
        .id()
    }

    /// Text that wraps within its parent's width, in its own case: a
    /// sentence (descriptions, help, dialog bodies) in `Role::Text`, whose
    /// lines are 1.3 em (`TypeSpec::line`).
    pub fn paragraph(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, tone: Tone, text: &str) -> Entity {
        c.spawn((
            Text::new(text),
            UiText::new(role, size, tone),
            TextLayout::new(Justify::Left, LineBreak::WordBoundary),
            ChildOf(parent),
        ))
        .id()
    }

    /// A slanted fill (`UiSlant`) as an absolutely placed box: `left`,
    /// `top` in cq, `w` x `h` cq.
    pub fn slant_at(&self, c: &mut Commands, parent: Entity, slant: UiSlant, ltrb: [Option<f32>; 4], w: f32, h: f32) -> Entity {
        c.spawn((style_abs(ltrb, w, h), Node::default(), slant, Pickable::IGNORE, ChildOf(parent))).id()
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

    /// A key cap (outlined, Mono), `size` cq text: muted at .75, but a
    /// dark one (on a primary button's white) at full strength.
    pub fn kbd(&self, c: &mut Commands, parent: Entity, key: &str, size: f32, tone: Tone) -> Entity {
        let a = if tone.is_dark() { 1.0 } else { 0.75 };
        let e = c
            .spawn((
                UiStyle { pad: Some([0.24, 0.36, 0.18, 0.36].into()), ..UiStyle::row() }.outline(tone, a),
                Node::default(),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        self.text_a(c, e, Role::Mono, size * 0.88, tone, a, key);
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
    /// note, the value at the right. Hover: its fill lifts (no tick);
    /// focus: tint .06 and the accent bar; changed: the accent square before the label; error: the
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
    /// each centred on its half (the slab's place); on, the slab is white
    /// at the right. Focus: the track's own slanted outline in accent (no
    /// underline). Returns the toggle.
    pub fn toggle(&self, c: &mut Commands, parent: Entity, on: bool, st: State) -> Entity {
        self.toggle_ex(c, parent, on, st, None)
    }

    /// `toggle` with the slab sliding over from the other side
    /// (`ToggleSlide`: flipped at `since`, `TOGGLE_SLIDE_SECS`, eased out;
    /// none under reduced motion).
    pub fn toggle_ex(&self, c: &mut Commands, parent: Entity, on: bool, st: State, slide: Option<ToggleSlide>) -> Entity {
        let t = c
            .spawn((
                UiStyle { w: Some(TOGGLE_W), h: Some(TOGGLE_H), shrink: Some(0.0), ..UiStyle::row() },
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
        full(c, UiSlant::new(Tone::Fg4, 1.0).outline(self.units.phys(self.units.hairline())));
        // The slab: half the track less its insets.
        let slab = UiSlant::new(if on { Tone::Fg } else { Tone::Fg4 }, 1.0);
        let left = if on { TOGGLE_ON_LEFT } else { TOGGLE_OFF_LEFT };
        let s = self.slant_at(c, t, slab, [Some(left), Some(0.2), None, None], TOGGLE_SLAB_W, 1.1);
        c.entity(s).insert((ToggleSlab, UiTransform::default()));
        if let Some(sl) = slide.filter(|sl| !sl.reduced) {
            let from = if on { TOGGLE_OFF_LEFT - TOGGLE_ON_LEFT } else { TOGGLE_ON_LEFT - TOGGLE_OFF_LEFT };
            let tl = Timeline::new([Track::new(Prop::X, from, 0.0).dur(TOGGLE_SLIDE_SECS).ease(crate::motion::Ease::OUT)]).started(sl.since);
            if !tl.done(sl.now) {
                let x = tl.sample(Prop::X, tl.local(sl.now)).unwrap_or(0.0);
                c.entity(s).insert((tl, UiTransform { translation: Val2::px(self.units.cq(x), 0.0), ..default() }));
            }
        }
        // Each label centred on its half, where the slab stands for it.
        for (l, x, lit) in [("Off", TOGGLE_OFF_LEFT, !on), ("On", TOGGLE_ON_LEFT, on)] {
            let half = self.boxed(
                c,
                t,
                UiStyle { abs: Some([Some(x), Some(0.0), None, Some(0.0)]), w: Some(TOGGLE_SLAB_W), justify: Some(JustifyContent::Center), ..UiStyle::row() },
            );
            c.entity(half).insert((ToggleLabel, Pickable::IGNORE));
            // On: ink on the white slab; off: white on the dim one; the
            // side the slab isn't on, faint.
            let tone = match (lit, on) {
                (true, true) => Tone::Ink,
                (true, false) => Tone::Fg,
                _ => Tone::Fg3,
            };
            self.text(c, half, Role::Mono, 0.5, tone, l);
        }
        if (st.focus || st.error) && !st.disabled {
            // Focus (error: threat) as a strip along the track's bottom
            // edge, its ends on the track's slant (Rule 2).
            let tone = if st.error { Tone::Threat } else { Tone::Acc };
            let h = 0.2;
            let tan = self.theme.tan();
            let strip = self.slant_at(c, t, UiSlant::new(tone, 1.0), [Some(0.0), None, None, Some(0.0)], TOGGLE_W - TOGGLE_H * tan + h * tan, h);
            c.entity(strip).insert(ToggleFocus);
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
        self.slider_ex(c, parent, frac, mark, value, w, st, None).track
    }

    /// `slider` with its value box clickable (give it the action that
    /// starts typing) and, while `typing`, a text field in its place.
    #[allow(clippy::too_many_arguments)]
    pub fn slider_ex(
        &self,
        c: &mut Commands,
        parent: Entity,
        frac: f32,
        mark: Option<f32>,
        value: &str,
        w: f32,
        st: State,
        typing: Option<Typing>,
    ) -> SliderParts {
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
        let value = self.value_box(c, s, value, 3.4, st, typing, ValueLook::SLIDER);
        SliderParts { track: tr, value }
    }

    /// A number's box (a slider's, a stepper's): clickable, a faint tint
    /// on hover. Typing (`Typing`) keeps its geometry and its face: the
    /// same box, the typed text in the same role and size one character a
    /// node (`edit_chars`, as at rest, so nothing moves when it opens),
    /// the caret an overlay, the selection a tint behind its characters,
    /// the underline accent (threat when the text was refused). A
    /// `suffix` (a percentage's `%`) stays after the typed text.
    #[allow(clippy::too_many_arguments)]
    fn value_box(&self, c: &mut Commands, parent: Entity, value: &str, w: f32, st: State, typing: Option<Typing>, look: ValueLook) -> Entity {
        let over = self.color(Tone::Fg, 0.06);
        let base = if typing.is_some() { over } else { Color::NONE };
        let nv = c
            .spawn((
                UiStyle { min_w: Some(w), h: Some(look.h), pad: Some([0.0, look.pad].into()), justify: Some(look.justify), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                RelativeCursorPosition::default(),
                clickable(),
                ChildOf(parent),
            ))
            .id();
        let face = Face { role: Role::Mono, size: look.size, tone: Tone::Fg, plain: false };
        match typing {
            Some(t) => {
                let (at, anchor) = (char_of(t.text, t.caret), char_of(t.text, t.anchor));
                let chars = self.edit_chars(c, nv, t.text, face, Some((at, anchor)));
                if let Some(sfx) = t.suffix {
                    self.text_with_case(c, nv, face.role, face.size, face.tone, 1.0, sfx);
                }
                c.entity(nv).insert(EditText { chars });
            }
            // At rest in capitals, as labels are (the UGG faces draw
            // capitals only); a value with a space as one text (a lone
            // space lays out as nothing), never typed into.
            None if value.contains(' ') => {
                self.text(c, nv, face.role, face.size, face.tone, value);
            }
            None => {
                let chars = self.edit_chars(c, nv, &value.to_uppercase(), face, None);
                c.entity(nv).insert(EditText { chars });
            }
        }
        let error = st.error || typing.is_some_and(|t| t.error);
        if error {
            self.underline(c, nv, Tone::Threat, 0.18);
        } else if st.focus || typing.is_some() {
            self.underline(c, nv, Tone::Acc, 0.18);
        } else if look.rest_line {
            self.hairline(c, nv, Tone::Fg4, 1.0);
        }
        nv
    }

    /// `text` under `parent` one character a node (each its own `Text` in
    /// `face`, in a row that takes no more room than the text), so a caret
    /// and a selection are placed by the layout itself: the caret
    /// (`caret`: (at, anchor), char indices) an accent bar over the
    /// boundary that takes no room (absolute, centred on it), the
    /// selection an accent tint behind its characters. Returns the
    /// characters' nodes (`EditText`, `TextHits` hit-test them).
    fn edit_chars(&self, c: &mut Commands, parent: Entity, text: &str, face: Face, caret: Option<(usize, usize)>) -> Vec<Entity> {
        let wrap = c
            .spawn((Node { flex_shrink: 0.0, align_items: AlignItems::Center, ..default() }, Pickable::IGNORE, ChildOf(parent)))
            .id();
        let n = text.chars().count();
        let (lo, hi) = caret.map_or((0, 0), |(a, b)| (a.min(b).min(n), a.max(b).min(n)));
        let mut out = Vec::with_capacity(n);
        for (i, ch) in text.chars().enumerate() {
            let sel = i >= lo && i < hi;
            let cell = c
                .spawn((
                    Node { flex_shrink: 0.0, align_items: AlignItems::Center, ..default() },
                    BackgroundColor(if sel { self.color(Tone::Acc, 0.45) } else { Color::NONE }),
                    Pickable::IGNORE,
                    ChildOf(wrap),
                ))
                .id();
            if sel {
                c.entity(cell).insert(TextSelection);
            }
            // A space would be laid out as nothing on its own.
            let shown = if ch == ' ' && face.plain { '\u{a0}'.to_string() } else { ch.to_string() };
            let t = self.text_with_case(c, cell, face.role, face.size, face.tone, 1.0, &shown);
            if face.plain {
                c.entity(t).insert(style::KeepCase);
            }
            out.push(cell);
        }
        if let Some((at, _)) = caret {
            let at = at.min(n);
            let (holder, left) = match out.get(at) {
                Some(cell) => (*cell, Val::ZERO),
                None => (out.last().copied().unwrap_or(wrap), if n == 0 { Val::ZERO } else { percent(100.0) }),
            };
            // Thin: one physical pixel, two on a window over 1440 lines;
            // just after the character before it (at the boundary's left).
            let w = self.units.hairline() * if self.units.phys(self.units.height) > 1440.0 { 2.0 } else { 1.0 };
            c.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left,
                    top: percent(50.0),
                    margin: UiRect { top: self.units.val(-0.5), ..default() },
                    width: Val::Px(w),
                    height: self.units.val(1.0),
                    ..default()
                },
                BackgroundColor(self.color(Tone::Acc, 1.0)),
                TextCaret,
                ZIndex(1),
                Pickable::IGNORE,
                ChildOf(holder),
            ));
        }
        out
    }

    /// A stepper: `-`, the value (`w` cq at least), `+`, on slanted
    /// outlined keys 1.9 x 1.7 cq. Returns (minus, plus).
    pub fn stepper(&self, c: &mut Commands, parent: Entity, value: &str, w: f32, st: State) -> (Entity, Entity) {
        let p = self.stepper_ex(c, parent, value, w, st, None);
        (p.minus, p.plus)
    }

    /// `stepper` with its value box clickable (a number typed in) and,
    /// while `typing`, a text field in its place.
    pub fn stepper_ex(&self, c: &mut Commands, parent: Entity, value: &str, w: f32, st: State, typing: Option<Typing>) -> StepperParts {
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
        // As tall as the keys, so the focus underline sits under the
        // value with a gap, as the slider's and the fields' do.
        let value = self.value_box(c, g, value, w, st, typing, ValueLook::STEPPER);
        let plus = key(c, "+");
        StepperParts { minus, value, plus }
    }

    /// A dropdown `w` cq wide: the field (1.9 cq, a faint fill, a hairline
    /// under it, accent when focused; white when open) and, when `open`,
    /// its list below (the chosen option's accent cut, the keys' line the
    /// accent bar). Returns (field, options).
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
        self.chevron(c, f, if ink { Tone::Ink } else { Tone::Fg2 }, open.is_some());
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
                if act {
                    // The keys' line: the accent bar (no tick).
                    self.focus_bar(c, e, 0.2);
                }
                out.push(e);
            }
        }
        (f, out)
    }

    /// The dropdown's mark: the kit's `chevron` icon (an upright down V:
    /// the dropdown is a straight box), `CHEVRON_H` cq tall in `tone`, flipped up while
    /// `up` (the list open). Its image is drawn by `draw_kit_icons`.
    /// Returns the node.
    pub fn chevron(&self, c: &mut Commands, parent: Entity, tone: Tone, up: bool) -> Entity {
        let u = self.units;
        let px = u.phys(u.cq(CHEVRON_H)).round().max(1.0);
        c.spawn((
            ImageNode { color: self.theme.tone(tone), flip_y: up, ..default() },
            Node {
                width: Val::Px((CHEVRON_ASPECT * px).ceil() / u.scale_factor),
                height: Val::Px(px / u.scale_factor),
                flex_shrink: 0.0,
                ..default()
            },
            KitIcon { key: "chevron", px: px as u32 },
            Chevron { up },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
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
        f
    }

    /// A text field (min `w` cq, 1.9 cq tall, the dropdown's field): the
    /// value in Mono as typed (its case kept; stars when `masked`) one
    /// character a node (`edit_chars`: the same at rest and typing),
    /// `placeholder` in `fg_3` when empty; `caret` (Some only while it has
    /// the keys; byte offsets into `value`) an accent bar over its place
    /// that takes no room, the selection between it and its anchor on an
    /// accent tint (`TextSelection`), the field tinted and underlined in
    /// accent. Error: the threat underline. Hover is its own tint (no
    /// tick). Returns the field (give it its focusing action; `TextHits`
    /// places a click).
    #[allow(clippy::too_many_arguments)]
    pub fn text_field(
        &self,
        c: &mut Commands,
        parent: Entity,
        value: &str,
        placeholder: &str,
        caret: Option<TextCursor>,
        masked: bool,
        w: f32,
        st: State,
    ) -> Entity {
        let typing = caret.is_some();
        let (base, over) = if typing {
            (self.color(Tone::Fg, 0.08), self.color(Tone::Fg, 0.08))
        } else {
            (self.color(Tone::Fg, 0.04), self.color(Tone::Fg, 0.07))
        };
        let f = c
            .spawn((
                UiStyle { min_w: Some(w), h: Some(1.9), pad: Some([0.0, 0.8].into()), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                RelativeCursorPosition::default(),
                clickable(),
                ChildOf(parent),
            ))
            .id();
        let shown: String = if masked { "*".repeat(value.chars().count()) } else { value.to_string() };
        // Typed text in the sentence face, its own case; stars in Mono.
        let (role, size) = if masked { (Role::Mono, 0.66) } else { (Role::Text, TYPED) };
        if value.is_empty() && !typing {
            self.text(c, f, Role::Mono, 0.62, Tone::Fg3, placeholder);
            c.entity(f).insert(EditText { chars: Vec::new() });
        } else {
            // Typed text in the sentence face (`TYPED`), its own case;
            // stars in Mono.
            let face = Face { role, size, tone: Tone::Fg, plain: !masked };
            let k = caret.map(|k| (char_of(value, k.at), char_of(value, k.anchor)));
            let chars = self.edit_chars(c, f, &shown, face, k);
            c.entity(f).insert(EditText { chars });
        }
        if st.error {
            self.underline(c, f, Tone::Threat, 0.2);
        } else if typing || st.focus {
            self.underline(c, f, Tone::Acc, 0.2);
        } else {
            self.hairline(c, f, Tone::Fg3, 1.0);
        }
        f
    }

    /// Key/value lines (the help pane's console name, default, range):
    /// keys Mono .6 `fg_3` in a `kw` cq column (capitals), values `fg` in
    /// Mono as given (a cvar name in its lower case; callers upper-case
    /// word values), a hairline under each.
    pub fn kv(&self, c: &mut Commands, parent: Entity, pairs: &[(&str, String)], kw: f32) -> Entity {
        let list = self.boxed(c, parent, UiStyle::column());
        for (k, v) in pairs {
            let line = self.boxed(c, list, UiStyle { gap: Some(1.0), pad: Some([0.6, 0.0].into()), ..UiStyle::row() });
            self.hairline(c, line, Tone::Fg, 0.07);
            let kc = self.boxed(c, line, UiStyle { w: Some(kw), shrink: Some(0.0), ..UiStyle::row() });
            self.text(c, kc, Role::Mono, 0.6, Tone::Fg3, k);
            self.text_plain(c, line, Role::Mono, 0.6, Tone::Fg, v);
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

/// The dropdown chevron's height, cq, and its icon's aspect (18 x 12
/// units, data/icons.toml).
pub const CHEVRON_H: f32 = 0.58;
pub const CHEVRON_ASPECT: f32 = 1.5;

/// The dropdown's chevron (`Ui::chevron`): pointing up (the list open).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Chevron {
    pub up: bool,
}

/// An `ImageNode` that shows the kit's icon `key` at `px` physical px
/// tall, its image filled in by `draw_kit_icons` (builders without the
/// image assets: `Ui::chevron`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct KitIcon {
    pub key: &'static str,
    pub px: u32,
}

/// Fills in `KitIcon`s' images (module doc).
pub fn draw_kit_icons(
    mut nodes: Query<(&KitIcon, &mut ImageNode), Changed<KitIcon>>,
    images: Option<ResMut<Assets<Image>>>,
    icons: Option<ResMut<IconImages>>,
) {
    let (Some(mut images), Some(mut icons)) = (images, icons) else { return };
    for (k, mut img) in &mut nodes {
        if let Some(h) = icons.get(&mut images, k.key, k.px) {
            img.image = h;
        }
    }
}

/// A toggle's size, cq.
pub const TOGGLE_W: f32 = 4.4;
pub const TOGGLE_H: f32 = 1.5;
/// Its slab's width and its two places (left edge, cq).
pub const TOGGLE_SLAB_W: f32 = 1.9;
pub const TOGGLE_OFF_LEFT: f32 = 0.25;
pub const TOGGLE_ON_LEFT: f32 = 2.25;
/// The slab's slide from one side to the other, seconds.
pub const TOGGLE_SLIDE_SECS: f32 = 0.14;

/// A toggle flipped at `since` (UI clock), drawn at `now`: its slab
/// slides over (`Ui::toggle_ex`); `reduced`: it jumps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToggleSlide {
    pub since: f64,
    pub now: f64,
    pub reduced: bool,
}

/// A toggle's focus strip (`Ui::toggle`).
#[derive(Component, Clone, Copy, Debug)]
pub struct ToggleFocus;

/// A toggle's slab (the moving part).
#[derive(Component, Clone, Copy, Debug)]
pub struct ToggleSlab;

/// A toggle's label box (OFF, then ON), each its half's width.
#[derive(Component, Clone, Copy, Debug)]
pub struct ToggleLabel;

/// How `Ui::edit_chars` draws a text.
#[derive(Clone, Copy, Debug)]
struct Face {
    role: Role,
    size: f32,
    tone: Tone,
    /// A person's own text, in its own case (`style::KeepCase`).
    plain: bool,
}

/// A text field's or a value box's characters, one node each, in order
/// (`Ui::edit_chars`): on the field, for `TextHits`.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct EditText {
    pub chars: Vec<Entity>,
}

/// The char index of byte offset `b` of `text` (rounded down to a char).
pub fn char_of(text: &str, b: usize) -> usize {
    let b = (0..=b.min(text.len())).rev().find(|i| text.is_char_boundary(*i)).unwrap_or(0);
    text[..b].chars().count()
}

/// The boundary a pointer at physical x `px` is nearest, of characters
/// whose centres are at `centres` (physical x, in order): how many of
/// them lie left of it.
pub fn boundary_at(px: f32, centres: &[f32]) -> usize {
    centres.iter().filter(|c| **c < px).count()
}

/// Where the pointer is in a text field or value box (`EditText`): the
/// char boundary it is nearest, from the layout (the field's
/// `RelativeCursorPosition`, each character's place). None: no pointer,
/// or not laid out.
#[derive(bevy::ecs::system::SystemParam)]
pub struct TextHits<'w, 's> {
    fields: Query<'w, 's, (&'static EditText, &'static RelativeCursorPosition, &'static ComputedNode, &'static UiGlobalTransform)>,
    nodes: Query<'w, 's, &'static UiGlobalTransform>,
}

impl TextHits<'_, '_> {
    pub fn char_at(&self, field: Entity) -> Option<usize> {
        let (text, rel, node, at) = self.fields.get(field).ok()?;
        let n = rel.normalized?;
        let px = at.affine().translation.x + n.x * node.size.x;
        let centres: Vec<f32> = text.chars.iter().filter_map(|e| self.nodes.get(*e).ok()).map(|t| t.affine().translation.x).collect();
        (centres.len() == text.chars.len()).then(|| boundary_at(px, &centres))
    }

    /// The field's length in characters.
    pub fn len(&self, field: Entity) -> Option<usize> {
        self.fields.get(field).ok().map(|f| f.0.chars.len())
    }
}

/// Seconds within which a second press near the first selects all.
pub const DOUBLE_CLICK_SECS: f64 = 0.4;

/// The pointer in a text (char indices; a screen keeps one): a press
/// places the caret, a drag from it selects, a double press selects all.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextPress {
    last: Option<(f64, usize)>,
    held: Option<usize>,
}

impl TextPress {
    /// A press at `at` of a text `len` chars long at `now`: (caret,
    /// anchor), all of it on a double press.
    pub fn press(&mut self, now: f64, at: usize, len: usize) -> (usize, usize) {
        if let Some((t, a)) = self.last
            && now - t <= DOUBLE_CLICK_SECS
            && a.abs_diff(at) <= 1
        {
            self.last = None;
            self.held = None;
            return (len, 0);
        }
        self.last = Some((now, at));
        self.held = Some(at);
        (at, at)
    }

    /// The pointer held and at `at`: (caret, anchor) from the press.
    pub fn drag(&self, at: usize) -> Option<(usize, usize)> {
        self.held.map(|a| (at, a))
    }

    /// Held since a press.
    pub fn held(&self) -> bool {
        self.held.is_some()
    }

    /// The button let go.
    pub fn release(&mut self) {
        self.held = None;
    }
}

/// A value box being typed into (`Ui::slider_ex`, `Ui::stepper_ex`):
/// the text, the caret and its selection's other end (byte indices),
/// the text refused.
#[derive(Clone, Copy, Debug)]
pub struct Typing<'a> {
    pub text: &'a str,
    pub caret: usize,
    pub anchor: usize,
    pub error: bool,
    /// Shown after the typed text, not typed (a percentage's `%`).
    pub suffix: Option<&'a str>,
}

/// A value box's look: a slider's (right-aligned, a hairline at rest)
/// or a stepper's (centred, as tall as its keys, nothing at rest).
#[derive(Clone, Copy, Debug)]
struct ValueLook {
    h: f32,
    pad: f32,
    size: f32,
    justify: JustifyContent,
    rest_line: bool,
}

impl ValueLook {
    const SLIDER: ValueLook = ValueLook { h: 1.6, pad: 0.4, size: 0.66, justify: JustifyContent::FlexEnd, rest_line: true };
    const STEPPER: ValueLook = ValueLook { h: 1.7, pad: 0.3, size: 0.7, justify: JustifyContent::Center, rest_line: false };
}

/// What `Ui::slider_ex` built: the track (the drag's) and the value box.
#[derive(Clone, Copy, Debug)]
pub struct SliderParts {
    pub track: Entity,
    pub value: Entity,
}

/// What `Ui::stepper_ex` built.
#[derive(Clone, Copy, Debug)]
pub struct StepperParts {
    pub minus: Entity,
    pub value: Entity,
    pub plus: Entity,
}

/// A number typed into a value box, checked (module doc): any finite
/// number, within `hard` (inclusive) where given; else why not.
pub fn typed_number(text: &str, hard: (Option<f32>, Option<f32>)) -> Result<f32, String> {
    let v: f32 = text.trim().parse().map_err(|_| "Not a number".to_string())?;
    if !v.is_finite() {
        return Err("Not a number".into());
    }
    if let Some(lo) = hard.0
        && v < lo
    {
        return Err(format!("At least {lo}"));
    }
    if let Some(hi) = hard.1
        && v > hi
    {
        return Err(format!("At most {hi}"));
    }
    Ok(v)
}

/// A text field's caret (`Ui::text_field`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextCaret;

/// A text field's selected text, on its accent tint (`Ui::text_field`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextSelection;

/// Where a text field's caret is and the other end of its selection
/// (byte offsets into the value; equal: nothing selected).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TextCursor {
    pub at: usize,
    pub anchor: usize,
}

impl From<usize> for TextCursor {
    /// A caret with nothing selected.
    fn from(at: usize) -> Self {
        Self { at, anchor: at }
    }
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
        app.add_systems(Update, show_hover).add_systems(PostUpdate, draw_kit_icons.in_set(crate::UiKitSystems)).add_systems(
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
