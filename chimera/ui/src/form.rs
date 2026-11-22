//! A settings form's pieces (docs/plans/active/settings-games.md, step 1;
//! the user's review of the "Options with Games" mockup, 2026-11-21), on
//! top of `controls`, sized by the theme's `[layout.form]`:
//!
//! - `badge`: a count chip (a page's changes on a tab, a game's on the
//!   game rail; anywhere a number rides on a label), slanted, in three
//!   looks (`BadgeKind`).
//! - `setting_row`: `controls::row` with the Chimera mark after the label
//!   when the setting is ours, not the original game's (`origin_mark`),
//!   one control column `control_w` wide for every control, and the
//!   reset key (`reset_key`) after it when the value was changed. One
//!   marker a row: the row lifts and shows the accent bar (hovered or
//!   focused); the control in it is given no focus of its own.
//! - `big_toggle`: the toggle as wide as the column and `control_h`
//!   tall, its slab's slide kept (`controls::ToggleSlide`).
//! - `choice`: a choice set that fits the column shows segmented
//!   (`fits_segmented`: each option's words in an equal share), else a
//!   dropdown.
//! - The key-capture chip is `controls::key_field` listening: its light
//!   blinks (`Blink`, `blink`: shown `layout.form.blink` s of each two,
//!   steady under reduced motion), "Press a key", Esc's cap: "cancel".
//!   Nothing here holds input back (no blocking animation).
//!
//! - `form_field`: a form's field (a sign-up's username, a rename): its
//!   label as a title, a one-line help under it, the input (the kit's
//!   `text_field`, its text smaller than the label). Kept minimal and in
//!   this one place for the forms design pass to restyle.
//!
//! `ui_gallery form` (`chimera_client::client::ui_scene`) shows each in
//! its states.

use bevy::prelude::*;

use crate::{
    controls::{self, Hover, KitIcon, State, TextCursor, ToggleSlide, Ui, pressable},
    fonts::Role,
    motion::UiClock,
    style::{UiSlant, UiStyle},
    theme::{Theme, Tone},
};

/// A form field's nodes (`Ui::form_field`).
#[derive(Clone, Copy, Debug)]
pub struct FormField {
    pub root: Entity,
    /// The label's line (an eye key or a count may go after the label).
    pub head: Entity,
    /// The text field (give it its focusing action).
    pub input: Entity,
}

/// The theme's layout table.
const L: &str = "form";

/// A form's sizes (cq), from `[layout.form]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormSizes {
    /// Every control's width in a settings row.
    pub control_w: f32,
    /// A toggle's and a segmented choice's height.
    pub control_h: f32,
    /// The reset key's column.
    pub state_w: f32,
    pub badge_h: f32,
    pub seg_char: f32,
    pub seg_pad: f32,
    pub blink: f32,
}

impl FormSizes {
    pub fn of(t: &Theme) -> FormSizes {
        let g = |k: &str, d: f32| t.layout_opt(L, k).unwrap_or(d);
        FormSizes {
            control_w: g("control_w", 14.0),
            control_h: g("control_h", 2.2),
            state_w: g("state_w", 2.6),
            badge_h: g("badge_h", 1.15),
            seg_char: g("seg_char", 0.36),
            seg_pad: g("seg_pad", 0.8),
            blink: g("blink", 1.0),
        }
    }
}

/// A badge's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeKind {
    /// Cobalt with white: changes, news.
    Hero,
    /// A faint fill with `fg_2`: a count that asks nothing.
    Quiet,
    /// Ink with white: on a white (selected) item.
    Ink,
}

/// A badge (`Ui::badge`).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Badge(pub String);

/// The Chimera mark on a setting that is ours (`Ui::origin_mark`).
#[derive(Component, Clone, Copy, Debug)]
pub struct OriginMark;

/// A row's reset key (`Ui::reset_key`).
#[derive(Component, Clone, Copy, Debug)]
pub struct ResetKey;

/// A node that blinks (the capture chip's light): shown for half of each
/// `layout.form.blink` s, always under reduced motion.
#[derive(Component, Clone, Copy, Debug)]
pub struct Blink;

/// `Blink`'s visibility from the UI clock (no redraw).
pub fn blink(
    clock: Option<Res<UiClock>>,
    theme: Option<Res<Theme>>,
    reduced: Option<Res<crate::ReducedMotion>>,
    mut nodes: Query<&mut Visibility, With<Blink>>,
) {
    if nodes.is_empty() {
        return;
    }
    let now = clock.map_or(0.0, |c| c.now);
    let period = theme.map_or(1.0, |t| FormSizes::of(&t).blink as f64).max(0.05);
    let on = reduced.is_some_and(|r| r.0) || (now / period).fract() < 0.5;
    for mut v in &mut nodes {
        v.set_if_neq(if on { Visibility::Inherited } else { Visibility::Hidden });
    }
}

/// Whether `options` fit `w` cq as a segmented choice: each in an equal
/// share, its words (`seg_char` a character) and its padding.
pub fn fits_segmented(sizes: &FormSizes, options: &[&str], w: f32) -> bool {
    if options.is_empty() || options.len() > 5 {
        return false;
    }
    let share = (w - 0.5) / options.len() as f32;
    options.iter().all(|o| o.chars().count() as f32 * sizes.seg_char + sizes.seg_pad <= share)
}

/// What `Ui::choice` drew.
#[derive(Clone, Debug, PartialEq)]
pub enum Choice {
    /// Each option (give each its pick).
    Segmented(Vec<Entity>),
    /// The field (give it its opening) and, open, the list's options.
    Dropdown { field: Entity, options: Vec<Entity> },
}

/// What `Ui::setting_row` made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SettingRow {
    pub row: Entity,
    /// The label's box (a status after the label).
    pub label: Entity,
    /// The control column (`control_w` wide).
    pub control: Entity,
    /// The reset key, when the value was changed (give it its reset).
    pub reset: Option<Entity>,
}

/// How a setting row reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RowLook<'a> {
    /// A small note after the label (a cvar, the guns it holds for).
    pub note: Option<&'a str>,
    /// The setting is ours, not the original game's: the Chimera mark.
    pub ours: bool,
    /// Changed from where it comes from: the reset key.
    pub changed: bool,
    pub min_h: f32,
    /// The control column's width (cq); 0: `[layout.form] control_w`. A
    /// narrower column only where the form's doesn't fit (the Guns
    /// dialog's rules column); every row of one list the same.
    pub control_w: f32,
}

impl Ui<'_> {
    pub fn form(&self) -> FormSizes {
        FormSizes::of(self.theme)
    }

    /// A badge: `text` (a count) in Mono on a slanted chip `badge_h` tall.
    pub fn badge(&self, c: &mut Commands, parent: Entity, text: &str, kind: BadgeKind) -> Entity {
        let h = self.form().badge_h;
        let (fill, a, ink) = match kind {
            BadgeKind::Hero => (Tone::Hero, 1.0, Tone::OnHero),
            BadgeKind::Quiet => (Tone::Fg, 0.1, Tone::Fg2),
            BadgeKind::Ink => (Tone::Ink, 1.0, Tone::Fg),
        };
        let lean = h * self.theme.tan();
        let b = c
            .spawn((
                UiStyle { h: Some(h), min_w: Some(h + lean), pad: Some([0.0, 0.35 + lean / 2.0].into()), justify: Some(JustifyContent::Center), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                UiSlant::new(fill, a),
                Badge(text.to_string()),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        self.text(c, b, Role::Mono, h * 0.5, ink, text);
        b
    }

    /// An icon from the kit's set, `h` cq tall in `tone`, drawn by
    /// `controls::draw_kit_icons` (no image assets needed here).
    pub fn kit_icon(&self, c: &mut Commands, parent: Entity, key: &'static str, h: f32, aspect: f32, tone: Tone) -> Entity {
        let u = self.units;
        let px = u.phys(u.cq(h)).round().max(1.0);
        c.spawn((
            ImageNode { color: self.theme.tone(tone), ..default() },
            Node { width: Val::Px((aspect * px).ceil() / u.scale_factor), height: Val::Px(px / u.scale_factor), flex_shrink: 0.0, ..default() },
            KitIcon { key, px: px as u32 },
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// The Chimera mark (a setting that is ours): the kit's `chimera`
    /// icon, `h` cq tall, in accent text.
    pub fn origin_mark(&self, c: &mut Commands, parent: Entity, h: f32) -> Entity {
        let e = self.kit_icon(c, parent, "chimera", h, 14.0 / 16.0, Tone::AccText);
        c.entity(e).insert(OriginMark);
        e
    }

    /// A reset key: the `reset` icon in a square outlined in accent,
    /// lifting while hovered. Returns it (clickable).
    pub fn reset_key(&self, c: &mut Commands, parent: Entity) -> Entity {
        let s = (self.form().state_w - 0.5).max(1.4);
        let (base, over) = (Color::NONE, self.color(Tone::Acc2, 1.0));
        let k = c
            .spawn((
                UiStyle { w: Some(s), h: Some(s), justify: Some(JustifyContent::Center), shrink: Some(0.0), ..UiStyle::row() }.outline(Tone::Acc, 0.6),
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                pressable(),
                ResetKey,
                ChildOf(parent),
            ))
            .id();
        self.kit_icon(c, k, "reset", s * 0.5, 1.0, Tone::AccText);
        k
    }

    /// A settings row (module doc): label, the Chimera mark when ours,
    /// the control column, the reset key when changed.
    pub fn setting_row(&self, c: &mut Commands, parent: Entity, label: &str, look: RowLook, st: State) -> SettingRow {
        let f = self.form();
        let min_h = if look.min_h > 0.0 { look.min_h } else { controls::ROW_H };
        let (row, lb, vl) = self.row_parts(c, parent, label, look.note, min_h, st);
        if look.ours {
            self.origin_mark(c, lb, 0.7);
        }
        c.entity(row).insert(controls::SettingRowMark);
        let w = if look.control_w > 0.0 { look.control_w } else { f.control_w };
        let control = self.boxed(c, vl, UiStyle { w: Some(w), gap: Some(0.4), shrink: Some(0.0), justify: Some(JustifyContent::FlexEnd), ..UiStyle::row() });
        let state = self.boxed(c, vl, UiStyle { w: Some(f.state_w), shrink: Some(0.0), justify: Some(JustifyContent::FlexEnd), ..UiStyle::row() });
        let reset = look.changed.then(|| self.reset_key(c, state));
        SettingRow { row, label: lb, control, reset }
    }

    /// The big toggle: `control_w` x `control_h` (module doc).
    pub fn big_toggle(&self, c: &mut Commands, parent: Entity, on: bool, st: State, slide: Option<ToggleSlide>, words: (&str, &str)) -> Entity {
        let f = self.form();
        self.toggle_sized(c, parent, on, st, slide, words, f.control_w, f.control_h)
    }

    /// A choice in the column (module doc): segmented when it fits
    /// `control_w`, else a dropdown (`open`: its list's active option).
    pub fn choice(&self, c: &mut Commands, parent: Entity, options: &[&str], selected: Option<usize>, open: Option<usize>, st: State) -> Choice {
        self.choice_with(c, parent, options, selected, &[], open, st)
    }

    /// `choice` with the options at `off` greyed out (segmented: no hover,
    /// not pressable; a choice that can't be made now).
    #[allow(clippy::too_many_arguments)]
    pub fn choice_with(&self, c: &mut Commands, parent: Entity, options: &[&str], selected: Option<usize>, off: &[usize], open: Option<usize>, st: State) -> Choice {
        let f = self.form();
        if fits_segmented(&f, options, f.control_w) {
            Choice::Segmented(self.segmented_sized(c, parent, options, selected, off, st, Some((f.control_w, f.control_h))))
        } else {
            let (field, options) = self.dropdown(c, parent, options, selected.unwrap_or(0), open, f.control_w, st);
            Choice::Dropdown { field, options }
        }
    }

    /// A form's field (module doc): `label` as a title (`aside` at the
    /// line's right: a count, "optional"), `help` in one line under it,
    /// then the input `w` cq wide. The caller adds its status line after.
    #[allow(clippy::too_many_arguments)]
    pub fn form_field(
        &self,
        c: &mut Commands,
        parent: Entity,
        label: &str,
        help: &str,
        aside: Option<&str>,
        value: &str,
        placeholder: &str,
        caret: Option<TextCursor>,
        masked: bool,
        w: f32,
        st: State,
    ) -> FormField {
        let root = self.boxed(c, parent, UiStyle { w: Some(w), gap: Some(0.35), ..UiStyle::column() });
        let head = self.boxed(c, root, UiStyle { gap: Some(0.8), align: Some(AlignItems::Baseline), ..UiStyle::row() });
        self.text(c, head, Role::DisplayBold, 1.0, if st.error { Tone::Threat } else { Tone::Fg }, label);
        if let Some(a) = aside {
            self.boxed(c, head, UiStyle::row().grow(1.0));
            self.text(c, head, Role::Mono, 0.56, Tone::Fg3, a);
        }
        if !help.is_empty() {
            let line = self.boxed(c, root, UiStyle { w: Some(w), ..UiStyle::column() });
            self.paragraph(c, line, Role::Text, 0.78, Tone::Fg3, help);
        }
        self.boxed(c, root, UiStyle { h: Some(0.25), ..UiStyle::row() });
        let input = self.text_field(c, root, value, placeholder, caret, masked, w, st);
        FormField { root, head, input }
    }
}
