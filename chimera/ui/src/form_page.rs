//! A form page's components (docs/plans/active/onboarding.md, "Form
//! components"; the Form Kit mockup the user approved with the review of
//! 2026-11-22): the account and onboarding pages are forms, where the
//! player types, the hub answers and something comes out. Beside
//! `form`'s settings rows, on top of `controls`.
//!
//! - `form_field` (`FieldSpec`): a title (Display Bold, upper case,
//!   larger than the typed value), a Required/Optional tag, one help line
//!   always shown, the input, and a status slot under it whose height is
//!   kept so nothing moves (`FieldStatus`: a hint, checking with the busy
//!   light, success with a tick in the box, an error that offers its fixes
//!   as clicks, locked with a lock and why).
//! - `secret_field`: a masked value (`secret::masked`: an address reveals
//!   nothing of its shape, the port a fixed 5 dots; only a fixed-format
//!   key keeps its dashes) with the eye key and a Copy key; Copy works
//!   hidden and turns into a tick with "Copied" for `COPIED_SECS`
//!   (`Copies`, `CopyKey`: the kit copies, a screen adds nothing).
//! - `icon_key`: a clickable icon (eye, copy, a list's chevron) on its own
//!   plate with a hairline frame, so it reads as a key without hovering;
//!   a decorative icon (`form_page`'s lock) has none and is dimmer.
//! - Buttons are the kit's one set (`Ui::button`, one height
//!   `controls::Size`: Small, Regular, Large; a row shares one): primary (the cobalt slab; hover lifts it and
//!   leans its edge out), secondary (outline), tertiary (`Kind::Tertiary`:
//!   grey to white, no underline), destructive (`Kind::Destructive`, filled
//!   `Kind::DestructiveConfirm` only in its confirm step); busy
//!   (`State::busy`: "Saving…") is disabled: not pressable, no hover,
//!   dimmed, its light pulsing.
//! - `result_card`: what the hub hands out once (an invite link): accent
//!   bar and tint, "Shown once", the whole value wrapped in Mono, a
//!   primary Copy (it reads "Copied" on white after) and a tertiary "Make
//!   another", the facts under it.
//! - `avatar_grid`: all 32 looks, 8 x 4; the pick framed by an outline
//!   outside the tile (`AVATAR_RING`, `AVATAR_GAP`: it never covers a
//!   tile's edge, its own or a neighbour's), no bar under it.
//! - `form_section`: the slanted accent block, the title, a count, a
//!   hairline; one sentence under it.
//! - `value_row`: a read-only row (mono key, value, and its icons in one
//!   right-hand column `TOOLS_W` wide, right-aligned, so a lock and a
//!   secret's eye and Copy line up across rows).
//! - `path_row`: a saved file's whole path on a line of its own (wrapped,
//!   never cut), its status and "Show in folder" on the line above.
//! - `form_footer`: the form's last line: what the last call did or the
//!   pending changes ("1 change · avatar"; from two on "N changes" with a
//!   key that unfolds the list above the bar, so a big form's summary
//!   stays one line), the actions at the right, primary last; while the
//!   form saves every action (Discard too) is busy.
//! - `badge_look` (`BadgeLook`): the kit's badge in a look given as data
//!   (`BadgeSpec`: filled or outline, a tone); its slant sits inside its
//!   own margins, so a clipping parent never cuts its corners.
//!
//! `ui_gallery formkit` (`chimera_client::client::ui_scene`) shows each
//! in its states.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use serde::Deserialize;

use crate::{
    avatar::{self, AvatarLook, AvatarMarks, Face},
    controls::{self, Hover, Kind, Size, State, TextCursor, Ui, pressable},
    fonts::Role,
    form::Blink,
    icons::IconImages,
    motion::UiClock,
    secret::{self, Secret},
    style::{UiSlant, UiStyle},
    theme::Tone,
};

/// How long a Copy key shows its tick and "Copied", seconds.
pub const COPIED_SECS: f64 = 2.0;
/// A field's input height and its typed text's size, cq.
pub const FIELD_H: f32 = 2.5;
pub const FIELD_TEXT: f32 = 0.8;
/// A field's title's size, cq (larger than the typed text).
pub const TITLE: f32 = 0.88;
/// The status slot's height, cq (kept when empty).
pub const STATUS_H: f32 = 1.2;
/// An icon key's side inside a field or a row, cq.
pub const KEY_H: f32 = 1.8;
/// A read-only row's icon column: two keys and their gap, cq.
pub const TOOLS_W: f32 = 2.0 * KEY_H + 0.3;
/// The avatar grid's gap and its pick's outline (width, offset), cq: the
/// outline lies outside the tile and inside the gap.
pub const AVATAR_GAP: f32 = 0.5;
pub const AVATAR_RING: (f32, f32) = (0.18, 0.1);

// ---------------------------------------------------------------------------
// Copy keys.

/// A Copy key: a click puts `value` on the clipboard and marks `key`
/// copied (`Copies`).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct CopyKey {
    pub key: String,
    pub value: String,
}

/// The last Copy and when (`COPIED_SECS` later it is forgotten). A screen
/// puts it in what it compares to redraw.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Copies {
    pub last: Option<(String, f64)>,
}

impl Copies {
    pub fn copied(&self, key: &str) -> bool {
        self.last.as_ref().is_some_and(|(k, _)| k == key)
    }

    /// Marks `key` copied now (a screen's own Ctrl+C on a value).
    pub fn mark(&mut self, key: &str, now: f64) {
        self.last = Some((key.to_string(), now));
    }
}

/// A click on a Copy key copies its value.
pub fn copy_clicks(
    keys: Query<(&Interaction, &CopyKey), Changed<Interaction>>,
    clock: Option<Res<UiClock>>,
    mut copies: ResMut<Copies>,
    clipboard: Option<ResMut<crate::clipboard::Clipboard>>,
) {
    let mut clipboard = clipboard;
    for (i, k) in &keys {
        if *i == Interaction::Pressed {
            if let Some(cb) = clipboard.as_deref_mut() {
                cb.set(&k.value);
            }
            copies.mark(&k.key, clock.as_deref().map_or(0.0, |c| c.now));
        }
    }
}

/// Forgets a Copy `COPIED_SECS` after it.
pub fn expire_copies(clock: Option<Res<UiClock>>, mut copies: ResMut<Copies>) {
    let now = clock.map_or(0.0, |c| c.now);
    if copies.last.as_ref().is_some_and(|(_, at)| now - at >= COPIED_SECS) {
        copies.last = None;
    }
}

// ---------------------------------------------------------------------------
// Specs.

/// A field's status line (module doc).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum FieldStatus<'a> {
    #[default]
    None,
    /// A quiet line (what to type, a count).
    Hint(&'a str),
    /// Asked of the hub: the busy light.
    Checking(&'a str),
    /// Good: the accent light and a tick in the box.
    Ok(&'a str),
    /// A warning (amber): something to do before it can go on.
    Warn(&'a str),
    /// Wrong: the threat line and tint; `fixes` offered as clicks.
    Error { text: &'a str, fixes: &'a [String] },
    /// Can't be changed now: dimmed, a lock, why and until when.
    Locked(&'a str),
}

/// A form field (`Ui::form_field`).
#[derive(Clone, Debug, Default)]
pub struct FieldSpec<'a> {
    pub label: &'a str,
    /// Required / Optional (or a count), at the title's right.
    pub tag: Option<&'a str>,
    pub help: &'a str,
    pub value: &'a str,
    pub placeholder: &'a str,
    /// Some while the field has the keys.
    pub caret: Option<TextCursor>,
    pub focus: bool,
    pub status: FieldStatus<'a>,
    /// A key cap after the tag ("Ctrl+V").
    pub key: Option<&'a str>,
    pub w: f32,
    /// A screen's one field (a first-run step's): `[layout.field]`'s
    /// `hero_h` tall, its text `hero_text`.
    pub hero: bool,
    /// The value is masked by the caller: its eye key in the box (the
    /// reveal's key, shown now).
    pub reveal: Option<(&'a str, bool)>,
}

/// What `form_field` made.
#[derive(Clone, Debug, PartialEq)]
pub struct FormField {
    pub root: Entity,
    /// The title's line.
    pub head: Entity,
    /// The input (give it its focusing action).
    pub input: Entity,
    /// The status slot.
    pub status: Entity,
    /// The error's fixes, one a suggestion (give each its action).
    pub fixes: Vec<Entity>,
}

/// A masked field (`Ui::secret_field`).
#[derive(Clone, Debug, Default)]
pub struct SecretSpec<'a> {
    pub label: &'a str,
    pub tag: Option<&'a str>,
    pub help: &'a str,
    /// The reveal's and the copy's key (`Reveals`, `Copies`).
    pub key: &'a str,
    pub value: &'a str,
    pub revealed: bool,
    /// A fixed-format secret (the recovery key): its dashes show.
    pub fixed: bool,
    pub copied: bool,
    pub w: f32,
}

/// What `secret_field` made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SecretParts {
    pub root: Entity,
    pub eye: Entity,
    pub copy: Entity,
}

/// A read-only row's value (`Ui::value_row`).
#[derive(Clone, Debug)]
pub enum RowValue<'a> {
    Text(&'a str),
    Mono(&'a str),
    /// A badge in a look.
    Badge(&'a str, BadgeLook),
    /// Masked with its eye and Copy keys (`key`: `Reveals`, `Copies`).
    Secret { key: &'a str, value: &'a str, revealed: bool, copied: bool },
}

/// What a result card says (`Ui::result_card`).
#[derive(Clone, Debug, Default)]
pub struct ResultSpec<'a> {
    /// Its head ("Invite made") and the tag at the right ("Shown once").
    pub head: &'a str,
    pub tag: &'a str,
    pub title: &'a str,
    /// The value, whole, wrapped in Mono.
    pub value: &'a str,
    pub copy: &'a str,
    pub copied: &'a str,
    pub is_copied: bool,
    pub another: &'a str,
    pub facts: &'a [(&'a str, String)],
}

/// What `result_card` made.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResultParts {
    pub root: Entity,
    pub copy: Entity,
    pub another: Entity,
}

/// One action of a form's footer.
#[derive(Clone, Copy, Debug)]
pub struct FooterAction<'a> {
    pub kind: Kind,
    pub label: &'a str,
    /// The label while the form saves ("Saving…"), the primary's.
    pub busy_label: Option<&'a str>,
    pub key: Option<&'a str>,
    pub disabled: bool,
}

/// A form's footer (`Ui::form_footer`).
#[derive(Clone, Debug, Default)]
pub struct FooterSpec<'a> {
    /// The pending changes, one short name each.
    pub changes: &'a [String],
    /// The changes' list unfolded.
    pub open: bool,
    /// What the last call did (its light's tone, its line); shown when no
    /// change is pending.
    pub said: Option<(Tone, &'a str)>,
    /// A save under way: its line ("Saving… the hub is renaming you").
    pub busy: Option<&'a str>,
    pub actions: &'a [FooterAction<'a>],
}

/// What `form_footer` made.
#[derive(Clone, Debug, PartialEq)]
pub struct FooterParts {
    /// The key unfolding the changes (two or more).
    pub toggle: Option<Entity>,
    /// The unfolded list, when open.
    pub list: Option<Entity>,
    /// Each action, in `FooterSpec::actions`' order.
    pub actions: Vec<Entity>,
}

/// The summary a footer shows for `changes` (module doc): none, "1 change
/// · avatar", or "N changes".
pub fn changes_summary(changes: &[String]) -> Option<String> {
    match changes {
        [] => None,
        [one] => Some(format!("1 change · {one}")),
        many => Some(format!("{} changes", many.len())),
    }
}

/// A badge's look, as data: a fill (or none: an outline), its border and
/// its text's tone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BadgeLook {
    pub fill: Option<(Tone, f32)>,
    pub border: Option<Tone>,
    pub ink: Tone,
}

/// A badge's look in a data file: `style` "filled" or "outline", `tone` a
/// theme tone's name (`Tone::from_name`: "hero", "warn", "threat", "fg_3").
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct BadgeSpec {
    #[serde(default)]
    pub style: String,
    #[serde(default)]
    pub tone: Option<String>,
}

impl BadgeLook {
    /// The quiet outline (a role, a count that asks nothing).
    pub const OUTLINE: BadgeLook = BadgeLook { fill: None, border: Some(Tone::Fg3), ink: Tone::Fg2 };
    /// Filled cobalt (news, the accent).
    pub const HERO: BadgeLook = BadgeLook { fill: Some((Tone::Hero, 1.0)), border: None, ink: Tone::OnHero };
    /// A warning's outline ("Expires today").
    pub const WARN: BadgeLook = BadgeLook { fill: None, border: Some(Tone::Warn), ink: Tone::Warn };

    pub fn of(kind: crate::form::BadgeKind) -> BadgeLook {
        use crate::form::BadgeKind;
        match kind {
            BadgeKind::Hero => BadgeLook::HERO,
            BadgeKind::Quiet => BadgeLook { fill: Some((Tone::Fg, 0.1)), border: None, ink: Tone::Fg2 },
            BadgeKind::Ink => BadgeLook { fill: Some((Tone::Ink, 1.0)), border: None, ink: Tone::Fg },
            BadgeKind::Outline => BadgeLook::OUTLINE,
            BadgeKind::Warn => BadgeLook::WARN,
        }
    }

    /// The look a data file names (`BadgeSpec`); an unknown tone: the
    /// outline's.
    pub fn from_spec(spec: &BadgeSpec) -> BadgeLook {
        let tone = spec.tone.as_deref().and_then(Tone::from_name);
        match spec.style.as_str() {
            "filled" | "fill" => {
                let t = tone.unwrap_or(Tone::Hero);
                let ink = if matches!(t, Tone::Hero | Tone::Acc) { Tone::OnHero } else { Tone::Ink };
                BadgeLook { fill: Some((t, 1.0)), border: None, ink }
            }
            _ => match tone {
                Some(t) => BadgeLook { fill: None, border: Some(t), ink: t },
                None => BadgeLook::OUTLINE,
            },
        }
    }
}

/// A badge's node (`Ui::badge_look`) and its look.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct BadgeShape {
    pub look: BadgeLook,
    /// Its slant's lean, cq: the margin kept at each side.
    pub lean: f32,
}

/// An icon key (`Ui::icon_key`): clickable, on its plate.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct IconKey;

/// A decorative icon in a row or a field (a lock): no plate, dimmer.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct InertIcon;

/// An avatar grid's tile (`Ui::avatar_grid`): its look's byte.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AvatarTile(pub u8);

/// A read-only row's icon column (`Ui::value_row`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct RowTools;

/// A form footer's summary ("2 changes") and its unfolded list.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ChangesSummary;
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct ChangesList;

impl Ui<'_> {
    // -----------------------------------------------------------------
    // Pieces.

    /// A clickable icon `h` cq square (module doc): its plate (`Fg` .06)
    /// and a hairline frame at rest, lifting on hover; `on` (pressed, a
    /// tick shown) the plate brighter and the icon white. Returns it (give
    /// it its action).
    pub fn icon_key(&self, c: &mut Commands, parent: Entity, icon: &'static str, h: f32, on: bool) -> Entity {
        let (base, over) = if on { (self.color(Tone::Fg, 0.16), self.color(Tone::Fg, 0.2)) } else { (self.color(Tone::Fg, 0.06), self.color(Tone::Fg, 0.14)) };
        let k = c
            .spawn((
                UiStyle { w: Some(h), h: Some(h), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() }.outline(Tone::Fg4, 1.0),
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                pressable(),
                IconKey,
                ChildOf(parent),
            ))
            .id();
        let tone = if on && icon == "tick" { Tone::AccText } else if on { Tone::Fg } else { Tone::Fg2 };
        self.kit_icon(c, k, icon, h * 0.55, 1.0, tone);
        k
    }

    /// A decorative icon in an icon key's cell (`h` cq, centred, no
    /// plate, `Fg3`): it lines up with keys in the same column.
    pub fn inert_icon(&self, c: &mut Commands, parent: Entity, icon: &'static str, h: f32) -> Entity {
        let cell = c
            .spawn((
                UiStyle { w: Some(h), h: Some(h), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                InertIcon,
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        self.kit_icon(c, cell, icon, h * 0.5, 14.0 / 16.0, Tone::Fg3);
        cell
    }

    /// The badge in `look` (module doc): Mono text on a slanted chip
    /// `badge_h` tall, a margin of its lean at each side so the slant is
    /// never cut.
    pub fn badge_look(&self, c: &mut Commands, parent: Entity, text: &str, look: BadgeLook) -> Entity {
        let h = self.form().badge_h;
        let lean = h * self.theme.tan();
        let slant = match (look.fill, look.border) {
            (Some((t, a)), _) => UiSlant::new(t, a),
            (None, Some(t)) => UiSlant::new(t, 1.0).outline(1.0),
            (None, None) => UiSlant::new(Tone::Fg3, 1.0).outline(1.0),
        };
        let b = c
            .spawn((
                UiStyle {
                    h: Some(h),
                    min_w: Some(h + lean),
                    pad: Some([0.0, 0.35 + lean / 2.0].into()),
                    margin: Some([0.0, lean / 2.0].into()),
                    justify: Some(JustifyContent::Center),
                    shrink: Some(0.0),
                    ..UiStyle::row()
                },
                Node::default(),
                slant,
                crate::form::Badge(text.to_string()),
                BadgeShape { look, lean },
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        self.text(c, b, Role::Mono, h * 0.5, look.ink, text);
        b
    }

    /// A light (the slanted square of a status line): `busy` pulses.
    fn light(&self, c: &mut Commands, parent: Entity, tone: Tone, busy: bool) -> Entity {
        let l = self.mark(c, parent, tone, 0.45, 0.45);
        if busy {
            c.entity(l).insert(Blink);
        }
        l
    }

    /// A field's title line: the label, the tag and a key cap at the right.
    fn title_line(&self, c: &mut Commands, parent: Entity, label: &str, tag: Option<&str>, key: Option<&str>, tone: Tone) -> Entity {
        let head = self.boxed(c, parent, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
        self.text(c, head, Role::DisplayBold, TITLE, tone, label);
        self.boxed(c, head, UiStyle::row().grow(1.0));
        if let Some(k) = key {
            self.kbd(c, head, k, 0.5, Tone::Fg3);
        }
        if let Some(t) = tag {
            self.text(c, head, Role::Mono, 0.5, Tone::Fg3, t);
        }
        head
    }

    fn help_line(&self, c: &mut Commands, parent: Entity, help: &str, w: f32) {
        if help.is_empty() {
            return;
        }
        let line = self.boxed(c, parent, UiStyle { w: Some(w), margin: Some([0.15, 0.0, 0.35, 0.0].into()), ..UiStyle::column() });
        self.paragraph(c, line, Role::Text, 0.72, Tone::Fg2, help);
    }

    // -----------------------------------------------------------------
    // Fields.

    /// A form field (module doc). The status slot keeps its height when
    /// empty.
    pub fn form_field(&self, c: &mut Commands, parent: Entity, f: &FieldSpec) -> FormField {
        let locked = matches!(f.status, FieldStatus::Locked(_));
        let error = matches!(f.status, FieldStatus::Error { .. });
        let root = self.boxed(c, parent, UiStyle { w: Some(f.w), ..UiStyle::column() });
        let head = self.title_line(c, root, f.label, f.tag, f.key, if locked { Tone::Fg3 } else { Tone::Fg });
        self.help_line(c, root, f.help, f.w);
        let st = State { focus: f.focus, error, disabled: locked, ..State::NONE };
        let size = if f.hero {
            (self.theme.layout_opt("field", "hero_h").unwrap_or(3.2), self.theme.layout_opt("field", "hero_text").unwrap_or(0.9))
        } else {
            (FIELD_H, FIELD_TEXT)
        };
        let input = self.field(c, root, f.value, f.placeholder, f.caret, false, f.w, st, false, size).field;
        if let Some((key, shown)) = f.reveal {
            self.boxed(c, input, UiStyle::row().grow(1.0));
            self.reveal_key(c, input, key, shown, KEY_H);
        }
        if error {
            c.entity(input).insert(BackgroundColor(self.color(Tone::Threat, 0.06)));
        }
        match f.status {
            FieldStatus::Ok(_) => {
                self.boxed(c, input, UiStyle::row().grow(1.0));
                self.kit_icon(c, input, "tick", 0.9, 1.0, Tone::AccText);
            }
            FieldStatus::Locked(_) => {
                self.boxed(c, input, UiStyle::row().grow(1.0));
                self.inert_icon(c, input, "lock", KEY_H);
            }
            _ => {}
        }
        let (status, fixes) = self.status_slot(c, root, &f.status);
        FormField { root, head, input, status, fixes }
    }

    /// A field's status slot (`FieldStatus`), `STATUS_H` tall at least.
    pub fn status_slot(&self, c: &mut Commands, parent: Entity, status: &FieldStatus) -> (Entity, Vec<Entity>) {
        let slot = self.boxed(c, parent, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), margin: Some([0.45, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
        c.entity(slot).insert(controls::MinHeight(STATUS_H));
        let mut fixes = Vec::new();
        let (tone, text, busy) = match status {
            FieldStatus::None => return (slot, fixes),
            FieldStatus::Hint(t) => (Tone::Fg3, *t, false),
            FieldStatus::Warn(t) => (Tone::Warn, *t, false),
            FieldStatus::Checking(t) => (Tone::Acc, *t, true),
            FieldStatus::Ok(t) => (Tone::AccText, *t, false),
            FieldStatus::Error { text, .. } => (Tone::Threat, *text, false),
            FieldStatus::Locked(t) => (Tone::Fg3, *t, false),
        };
        let light = match status {
            FieldStatus::Hint(_) | FieldStatus::Locked(_) => Tone::Fg4,
            FieldStatus::Ok(_) => Tone::Acc,
            _ => tone,
        };
        self.light(c, slot, light, busy);
        self.text_plain(c, slot, Role::Mono, 0.54, if busy { Tone::Fg2 } else { tone }, &text.to_uppercase());
        if let FieldStatus::Error { fixes: offered, .. } = status
            && !offered.is_empty()
        {
            // The fixes on their own line under the status (a long status
            // never pushes them out of the field's width).
            let line = self.boxed(c, parent, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), margin: Some([0.2, 0.0, 0.0, 0.9].into()), ..UiStyle::row() });
            for s in offered.iter() {
                // A fix: a tertiary key in the line, grey to white.
                fixes.push(self.button(c, line, Kind::Secondary, s, None, Size::Small, State::NONE));
            }
        }
        (slot, fixes)
    }

    /// A choice as a form's field (the kit's rule: segmented when every
    /// option fits an equal share of `w`, else a dropdown), with its title
    /// and help like `form_field`. Returns the root and what `choice` drew.
    #[allow(clippy::too_many_arguments)]
    pub fn choice_field(&self, c: &mut Commands, parent: Entity, label: &str, help: &str, options: &[&str], selected: usize, open: Option<usize>, st: State, w: f32) -> (Entity, crate::form::Choice) {
        let root = self.boxed(c, parent, UiStyle { w: Some(w), ..UiStyle::column() });
        self.title_line(c, root, label, None, None, Tone::Fg);
        self.help_line(c, root, help, w);
        let f = self.form();
        let ch = if crate::form::fits_segmented(self.theme, &f, options, w) {
            crate::form::Choice::Segmented(self.segmented_sized(c, root, options, Some(selected), &[], st, Some(w), Size::Regular))
        } else {
            let (field, options) = self.dropdown(c, root, options, selected, open, w, st);
            crate::form::Choice::Dropdown { field, options }
        };
        self.status_slot(c, root, &FieldStatus::None);
        (root, ch)
    }

    /// A masked field (module doc): the value masked unless revealed, the
    /// eye key and the Copy key (a tick while copied) in the box; the
    /// status "Copied" while it is.
    pub fn secret_field(&self, c: &mut Commands, parent: Entity, f: &SecretSpec) -> SecretParts {
        let root = self.boxed(c, parent, UiStyle { w: Some(f.w), ..UiStyle::column() });
        self.title_line(c, root, f.label, f.tag, None, Tone::Fg);
        self.help_line(c, root, f.help, f.w);
        let bx = self.boxed(c, root, UiStyle { w: Some(f.w), h: Some(FIELD_H), gap: Some(0.35), pad: Some([0.0, 0.35, 0.0, 0.8].into()), ..UiStyle::row() }.bg_a(Tone::Fg, 0.045));
        c.entity(bx).insert(Secret(f.key.to_string()));
        let shown = if f.revealed { f.value.to_string() } else { secret::masked(f.value, f.fixed) };
        let t = self.text_plain(c, bx, Role::Mono, FIELD_TEXT, if f.revealed { Tone::Fg } else { Tone::Fg2 }, &shown);
        c.entity(t).insert(SecretText(f.key.to_string()));
        self.boxed(c, bx, UiStyle::row().grow(1.0));
        let eye = self.reveal_key(c, bx, f.key, f.revealed, KEY_H);
        let copy = self.copy_key(c, bx, f.key, f.value, f.copied);
        self.hairline(c, bx, Tone::Fg3, 1.0);
        let status = if f.copied { FieldStatus::Ok("Copied") } else { FieldStatus::None };
        self.status_slot(c, root, &status);
        SecretParts { root, eye, copy }
    }

    /// A Copy key for `value` (a tick while `copied`).
    pub fn copy_key(&self, c: &mut Commands, parent: Entity, key: &str, value: &str, copied: bool) -> Entity {
        let k = self.icon_key(c, parent, if copied { "tick" } else { "copy" }, KEY_H, copied);
        c.entity(k).insert(CopyKey { key: key.to_string(), value: value.to_string() });
        k
    }

    // -----------------------------------------------------------------
    // Blocks.

    /// A section's head (module doc): `count` after the title, `sentence`
    /// under it. Returns the head's row.
    pub fn form_section(&self, c: &mut Commands, parent: Entity, title: &str, count: Option<&str>, sentence: Option<&str>) -> Entity {
        let wrap = self.boxed(c, parent, UiStyle { gap: Some(0.3), shrink: Some(0.0), ..UiStyle::column() });
        let head = self.boxed(c, wrap, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
        self.mark(c, head, Tone::Acc, 0.5, 1.1);
        self.text(c, head, Role::DisplayBold, 0.86, Tone::Fg, title);
        if let Some(n) = count {
            self.text(c, head, Role::Mono, 0.52, Tone::Fg3, n);
        }
        let rule = self.boxed(c, head, UiStyle { h: Some(0.06), grow: Some(1.0), margin: Some([0.0, 0.0, 0.0, 0.5].into()), ..UiStyle::row() });
        c.entity(rule).insert(BackgroundColor(self.color(Tone::Fg5, 1.0)));
        if let Some(s) = sentence {
            let line = self.boxed(c, wrap, UiStyle { margin: Some([0.0, 0.0, 0.0, 1.4].into()), ..UiStyle::column() });
            self.paragraph(c, line, Role::Text, 0.72, Tone::Fg2, s);
        }
        head
    }

    /// A read-only row (module doc): `key` in Mono `kw` cq wide, the
    /// value, then the icon column (`TOOLS_W`, right-aligned: a lock, or a
    /// secret's eye and Copy). Returns (row, the tools' keys).
    pub fn value_row(&self, c: &mut Commands, parent: Entity, key: &str, value: RowValue, kw: f32, locked: bool) -> (Entity, Vec<Entity>) {
        let row = self.boxed(c, parent, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), shrink: Some(0.0), w_pct: Some(100.0), ..UiStyle::row() });
        c.entity(row).insert(controls::MinHeight(2.6));
        let rule = self.boxed(c, row, UiStyle { h: Some(0.06), ..UiStyle::row() }.abs([Some(0.0), None, Some(0.0), Some(0.0)]));
        c.entity(rule).insert(BackgroundColor(self.color(Tone::Fg5, 1.0)));
        let k = self.boxed(c, row, UiStyle { w: Some(kw), shrink: Some(0.0), ..UiStyle::row() });
        self.text(c, k, Role::Mono, 0.52, Tone::Fg3, key);
        let v = self.boxed(c, row, UiStyle { grow: Some(1.0), shrink: Some(1.0), gap: Some(0.6), align: Some(AlignItems::Center), ..UiStyle::row() });
        let tools = self.boxed(c, row, UiStyle { w: Some(TOOLS_W), gap: Some(0.3), shrink: Some(0.0), justify: Some(JustifyContent::FlexEnd), ..UiStyle::row() });
        c.entity(tools).insert(RowTools);
        let mut keys = Vec::new();
        match value {
            RowValue::Text(t) => {
                self.text_plain(c, v, Role::Text, 0.8, Tone::Fg, t);
            }
            RowValue::Mono(t) => {
                self.text_plain(c, v, Role::Mono, 0.66, Tone::Fg, t);
            }
            RowValue::Badge(t, look) => {
                self.badge_look(c, v, t, look);
            }
            RowValue::Secret { key: sk, value: sv, revealed, copied } => {
                c.entity(v).insert(Secret(sk.to_string()));
                let shown = if revealed { sv.to_string() } else { secret::masked(sv, false) };
                let t = self.text_plain(c, v, Role::Mono, 0.66, if revealed { Tone::Fg } else { Tone::Fg2 }, &shown);
                c.entity(t).insert(SecretText(sk.to_string()));
                keys.push(self.reveal_key(c, tools, sk, revealed, KEY_H));
                keys.push(self.copy_key(c, tools, sk, sv, copied));
            }
        }
        if locked && keys.is_empty() {
            self.inert_icon(c, tools, "lock", KEY_H);
        }
        (row, keys)
    }

    /// A saved file's row (module doc): the light, `status` and the "Show
    /// in folder" key (`show`) on the first line, the whole path under it
    /// across the row's width `w`. Returns (row, the Show key).
    pub fn path_row(&self, c: &mut Commands, parent: Entity, status: &str, path: &str, show: &str, w: f32) -> (Entity, Entity) {
        let row = self.boxed(c, parent, UiStyle { w: Some(w), gap: Some(0.5), pad: Some([0.8, 1.0].into()), shrink: Some(0.0), ..UiStyle::column() }.bg_a(Tone::Fg, 0.035).outline(Tone::Fg5, 1.0));
        let top = self.boxed(c, row, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() });
        self.light(c, top, Tone::Acc, false);
        self.text(c, top, Role::Mono, 0.52, Tone::AccText, status);
        self.boxed(c, top, UiStyle::row().grow(1.0));
        let b = self.button(c, top, Kind::Secondary, show, None, Size::Small, State::NONE);
        let p = self.boxed(c, row, UiStyle { w: Some(w - 2.0), ..UiStyle::column() });
        let t = self.paragraph(c, p, Role::Mono, 0.62, Tone::Fg, path);
        c.entity(t).insert((PathText, crate::style::KeepCase, TextLayout::new(Justify::Left, LineBreak::AnyCharacter)));
        (row, b)
    }

    /// A result card (module doc), `w` cq wide.
    pub fn result_card(&self, c: &mut Commands, parent: Entity, r: &ResultSpec, w: f32) -> ResultParts {
        let card = self.boxed(c, parent, UiStyle { w: Some(w), gap: Some(0.8), pad: Some([1.2, 1.4, 1.3, 1.8].into()), shrink: Some(0.0), ..UiStyle::column() }.bg_a(Tone::Acc, 0.08).outline(Tone::Acc, 0.32));
        let bar = self.boxed(c, card, UiStyle { w: Some(0.3), ..UiStyle::column() }.abs([Some(0.0), Some(0.0), None, Some(0.0)]));
        c.entity(bar).insert(BackgroundColor(self.color(Tone::Acc, 1.0)));
        let head = self.boxed(c, card, UiStyle { gap: Some(0.8), align: Some(AlignItems::Center), ..UiStyle::row() });
        self.light(c, head, Tone::Acc, false);
        self.text(c, head, Role::Mono, 0.54, Tone::AccText, r.head);
        self.boxed(c, head, UiStyle::row().grow(1.0));
        self.text(c, head, Role::Mono, 0.54, Tone::Fg3, r.tag);
        self.text(c, card, Role::DisplayBold, 1.1, Tone::Fg, r.title);
        let v = self.boxed(c, card, UiStyle { pad: Some([0.7, 0.9].into()), ..UiStyle::column() }.bg_a(Tone::Ink, 0.55).outline(Tone::Fg4, 1.0));
        let inner = self.boxed(c, v, UiStyle { w: Some(w - 5.2), ..UiStyle::column() });
        let v = self.paragraph(c, inner, Role::Mono, 0.66, Tone::Fg, r.value);
        c.entity(v).insert((crate::style::KeepCase, TextLayout::new(Justify::Left, LineBreak::AnyCharacter)));
        let row = self.boxed(c, card, UiStyle { gap: Some(0.9), align: Some(AlignItems::Center), ..UiStyle::row() });
        let copy = if r.is_copied {
            // "Copied" on white: the copy done (still pressable: again).
            let b = self.item(c, row, UiStyle { h: Some(controls::BUTTON_H), pad: Some([0.0, 1.4].into()), gap: Some(0.5), ..UiStyle::row() }, State::NONE, true);
            self.kit_icon(c, b, "tick", 0.8, 1.0, Tone::Ink);
            self.text(c, b, Role::DisplayRegular, 0.66, Tone::Ink, r.copied);
            b
        } else {
            self.button(c, row, Kind::Primary, r.copy, Some("Ctrl+C"), Size::Regular, State::NONE)
        };
        let another = self.button(c, row, Kind::Tertiary, r.another, None, Size::Regular, State::NONE);
        if !r.facts.is_empty() {
            let facts = self.boxed(c, card, UiStyle { gap: Some(1.6), align: Some(AlignItems::Center), ..UiStyle::row() });
            for (k, v) in r.facts {
                let f = self.boxed(c, facts, UiStyle { gap: Some(0.5), ..UiStyle::row() });
                self.text(c, f, Role::Mono, 0.5, Tone::Fg3, k);
                self.text_plain(c, f, Role::Mono, 0.5, Tone::Fg2, &v.to_uppercase());
            }
        }
        ResultParts { root: card, copy, another }
    }

    /// The avatar grid (module doc): 32 looks, 8 a row, `w` cq wide; the
    /// pick (`chosen`) framed by an outline outside its tile, white (the
    /// accent while the grid has the keys: `focus`). Returns the tiles in
    /// order (give each its pick: `AvatarTile` holds its byte).
    #[allow(clippy::too_many_arguments)]
    pub fn avatar_grid(
        &self,
        c: &mut Commands,
        parent: Entity,
        mut images: Option<&mut Assets<Image>>,
        mut icons: Option<&mut IconImages>,
        chosen: u8,
        focus: bool,
        w: f32,
    ) -> Vec<Entity> {
        let (ring_w, ring_off) = AVATAR_RING;
        let room = ring_w + ring_off;
        let tile = ((w - 2.0 * room - 7.0 * AVATAR_GAP) / 8.0).max(1.2);
        // The pad keeps the pick's outline inside the grid (a scroll view
        // clips).
        let grid = self.boxed(c, parent, UiStyle { gap: Some(AVATAR_GAP), pad: Some([room + 0.05, room + 0.05].into()), shrink: Some(0.0), ..UiStyle::column() });
        let mut out = Vec::with_capacity(32);
        for row in 0..4u8 {
            let line = self.boxed(c, grid, UiStyle { gap: Some(AVATAR_GAP), ..UiStyle::row() });
            for col in 0..8u8 {
                let b = row * 8 + col;
                let sel = b == chosen;
                let (base, over) = (Color::NONE, self.color(Tone::Fg, 0.1));
                let cell = c
                    .spawn((
                        UiStyle { w: Some(tile), h: Some(tile), shrink: Some(0.0), justify: Some(JustifyContent::Center), ..UiStyle::row() },
                        Node::default(),
                        BackgroundColor(base),
                        Hover { base, over },
                        pressable(),
                        AvatarTile(b),
                        ChildOf(line),
                    ))
                    .id();
                if sel {
                    let tone = if focus { Tone::Acc } else { Tone::Fg };
                    c.entity(cell).insert(Outline::new(self.units.val(ring_w), self.units.val(ring_off), self.color(tone, 1.0)));
                }
                let look = AvatarLook::from_byte(b).unwrap_or(AvatarLook::new(0, 0));
                let a = match (images.as_deref_mut(), icons.as_deref_mut()) {
                    (Some(images), Some(icons)) => avatar::spawn(self, c, cell, images, icons, Face::Look(look), AvatarMarks::default(), tile),
                    _ => {
                        let e = self.boxed(c, cell, UiStyle { w: Some(tile), h: Some(tile), ..UiStyle::row() });
                        c.entity(e).insert(BackgroundColor(look.ground()));
                        e
                    }
                };
                c.entity(a).insert(Pickable::IGNORE);
                out.push(cell);
            }
        }
        out
    }

    /// A form's footer in `bar` (module doc; the shell's action bar).
    pub fn form_footer(&self, c: &mut Commands, bar: Entity, f: &FooterSpec) -> FooterParts {
        let st = self.boxed(c, bar, UiStyle { gap: Some(0.7), grow: Some(1.0), shrink: Some(1.0), align: Some(AlignItems::Center), ..UiStyle::row() });
        let mut toggle = None;
        let mut list = None;
        if let Some(busy) = f.busy {
            self.light(c, st, Tone::Acc, true);
            self.text_plain(c, st, Role::Mono, 0.54, Tone::Fg2, &busy.to_uppercase());
        } else if let Some(summary) = changes_summary(f.changes) {
            self.light(c, st, Tone::Acc, false);
            let s = self.text_plain(c, st, Role::Mono, 0.54, Tone::Fg, &summary.to_uppercase());
            c.entity(s).insert(ChangesSummary);
            if f.changes.len() > 1 {
                let k = self.icon_key(c, st, "chevron", 1.5, f.open);
                toggle = Some(k);
                if f.open {
                    // The list above the bar: one line a change.
                    let l = self.boxed(c, bar, UiStyle { gap: Some(0.35), pad: Some([0.8, 1.2].into()), min_w: Some(18.0), ..UiStyle::column() }.bg_a(Tone::Ink3, 0.97).outline(Tone::Fg4, 1.0));
                    c.entity(l).insert((ChangesList, Node { position_type: PositionType::Absolute, bottom: Val::Percent(100.0), left: self.units.val(1.0), ..default() }, GlobalZIndex(5)));
                    for ch in f.changes {
                        let line = self.boxed(c, l, UiStyle { gap: Some(0.6), align: Some(AlignItems::Center), ..UiStyle::row() });
                        self.mark(c, line, Tone::Acc, 0.3, 0.3);
                        self.text_plain(c, line, Role::Mono, 0.54, Tone::Fg2, &ch.to_uppercase());
                    }
                    list = Some(l);
                }
            }
        } else if let Some((tone, text)) = f.said {
            self.light(c, st, tone, false);
            self.paragraph(c, st, Role::Mono, 0.54, if tone == Tone::Threat { Tone::Threat } else { Tone::Fg2 }, &text.to_uppercase());
        }
        let busy = f.busy.is_some();
        let actions = f
            .actions
            .iter()
            .map(|a| {
                let label = if busy { a.busy_label.unwrap_or(a.label) } else { a.label };
                let st = State { disabled: a.disabled && !busy, busy: busy && a.busy_label.is_some(), ..State::NONE };
                // Every action waits while the form saves (Discard too).
                let st = if busy && !st.busy { State { disabled: true, ..State::NONE } } else { st };
                self.button(c, bar, a.kind, label, if busy { None } else { a.key }, Size::Regular, st)
            })
            .collect();
        FooterParts { toggle, list, actions }
    }
}

/// A secret's shown text (masked or not), by its key.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct SecretText(pub String);

/// A path row's path text.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PathText;


pub(crate) fn add(app: &mut App) {
    app.init_resource::<Copies>().add_systems(Update, (copy_clicks, expire_copies).chain());
}
