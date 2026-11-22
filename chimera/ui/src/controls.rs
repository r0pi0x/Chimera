//! The kit's form controls (docs/plans/active/ugg-ui.md, M7; the "UGG
//! Components & Views" page's component table, its sizes in cq and its
//! tokens): what menus are built from.
//!
//! - `button` (`Kind`: primary the hero, a `hero` cobalt slab with a
//!   slanted right edge and white text, `HERO_H` the big one with a bold
//!   label; secondary outlined, ghost, danger with the threat mark), labels
//!   in Display, an optional key cap (`kbd`: the lockup, equal padding,
//!   .82 of the label, full strength); `tabs` (sub-tabs: grey names, the
//!   selected one white, a hovered one white too (`HoverText`), no strip
//!   under any (the user's review, 2026-11-21); `tabs_ex` with the Q and E
//!   caps at the strip's ends, `tabs_badged` with a badge after a name); `progress` (the progress component, variant B: a sheared bar,
//!   hollow and filling while running, filled when ready, striped while
//!   connecting, a dim outline pending, the word and the share inside);
//!   `status_block` (a slanted light and STATUS on a hairline, its words
//!   the tooltip); `dialog_field` (a field that opens a dialog: the
//!   list-with-chevron icon); `row` (a label and a value, `min_h` 2.9 cq, a
//!   hairline under it) and `section` (a small header with the accent
//!   mark); `toggle_sized` (a slab on a slanted track, white when on, each
//!   label centred on its slab's place; `form::big_toggle` the one screens
//!   use; it slides the slab over,
//!   `ToggleSlide`, `Pace::Normal`, a snap at motion scale 0; focus a
//!   strip clipped to the track's bottom edge);
//!   `segmented_sized` (`form::choice` for a setting); `slider` (track, fill, the default's mark, a slanted
//!   thumb, the value); `stepper` (`-`, the value, `+` in one slanted border, the value box
//!   sized from its `StepperRange`, a key at its end dimmed); their `_ex`
//!   forms make the value box clickable and, while `Typing`, typed into
//!   in place (a number, checked by `typed_number`: any finite number
//!   inside the setting's hard limits); `dropdown` (a field with the
//!   upright `chevron` icon, flipped up while open, and, open, its list);
//!   `key_field` (key caps; listening, the capture chip: an accent tint,
//!   a blinking light, "Press a key", Esc's cap: "cancel"); `text_field`
//!   (typed text or stars, a placeholder, a thin white caret just after
//!   the text while it has the keys, the selection on an accent tint; its keys, copy, cut
//!   and paste are the screen's: `chimera_client::client::widgets::
//!   text_key`, `clipboard`); `search_field` (the same field with the
//!   search glass in front and a clear key once something is typed: every
//!   search box); `text_area` (several lines, wrapping, the same
//!   per-character text, overlay caret and selection); `kv` (key/value lines); `modal` (a plate
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
//!   rebuild; a screen drawn again keeps it: `carry_hover` starts a node
//!   spawned where a hovered one went as hovered, its tint and hero lift
//!   too, so a rebuild under a resting pointer draws no different
//!   frame), the element's own fill lifting: no tick anywhere (the
//!   user's call); a row hovered or focused lifts and shows the accent
//!   bar at the left, its control no focus of its own (one marker a row,
//!   2026-11-21); a control alone shows focus as its own accent detail; selected is a white fill;
//!   a toggle that is on is white (cobalt stays for focus and changed
//!   rows: a changed row's accent square); threat red only as a mark (an
//!   error's underline, the danger button's mark), never a fill.
//! - **Wipes** (`Wipe`): a moving cover or bar on the 12° slant, clipped
//!   along it by its `Timeline` (`Prop::ClipLeft`/`ClipRight`), the
//!   menus' moves inside a screen (`wipe_cover`, `Wipe::bar`); the logo
//!   pass stays for screen changes.
//!
//! - **Pressable** (`KitControl`): every node a builder makes pressable
//!   carries it; a screen never makes a node pressable itself (docs/
//!   DESIGN.md, "UI: always the kit"). `item` is the pressable surface a
//!   screen lays out itself (a list entry, a rail item, a card, a chip),
//!   `button_icon` a button with a kit icon, `press_tint` the state for a
//!   node another system spawned, `catch_pointer` a panel that only
//!   blocks the pointer. Screens use `form`'s `big_toggle` and `choice`
//!   (the small toggle and the natural-width segmented builders are gone,
//!   2026-11-21).
//!
//! Builders spawn under a parent (`ChildOf`) and return the entity a
//! screen gives its action, sized from `UiUnits` at
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
    motion::{Motion, Pace, Prop, Timeline, Track, UiClock},
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

/// The hero's slab (`Ui::button`): its fill lifts and its edge leans out
/// on hover, eased over `Pace::Fast` (`animate_heroes`). `lift` is how far
/// along (0 rest, 1 hovered).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct HeroSlab {
    pub lift: f32,
}

/// The accent layer over a hero's slab; its alpha is the lift.
#[derive(Component, Clone, Copy, Debug)]
pub struct HeroGlow;

/// What the progress component shows (`Ui::progress`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Progress {
    /// Not started (or not ready): a dim outline.
    Pending,
    /// No share known yet: stripes through the outline.
    Connecting,
    /// Under way, 0-1: hollow, filling from the left.
    Running(f32),
    /// Done: filled.
    Ready,
}

/// A progress bar (its state, for tests).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ProgressBar(pub Progress);

/// A progress bar's fill (its width the share done).
#[derive(Component, Clone, Copy, Debug)]
pub struct ProgressFill;

/// A status block's light (`Ui::status_block`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusLight {
    Off,
    Busy,
    Ok,
    Failed,
}

/// A status block (its light, for tests).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusBlock(pub StatusLight);

/// A segmented group's slanted track (`Ui::segmented`).
#[derive(Component, Clone, Copy, Debug)]
pub struct SegmentedTrack;

/// A segmented group's pick: the white slanted slab. `key` identifies the
/// picker by its words (`slide_picks` slides a new pick over from where
/// the same picker's last one stood).
#[derive(Component, Clone, Copy, Debug)]
pub struct SegmentedPick {
    pub track: Entity,
    pub key: u64,
}

/// A tab strip's Q or E cap (`Ui::tabs_ex`).
#[derive(Component, Clone, Copy, Debug)]
pub struct TabKey;

/// A hoverable node's tint: its background `base`, `over` while hovered
/// (and pressed).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hover {
    pub base: Color,
    pub over: Color,
}

/// A hover mark shown while its parent is hovered, or always (`always`):
/// a row's accent bar, a segmented option's tint.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Tick {
    pub always: bool,
}

/// A text whose tone follows its parent's hover (`show_hover`): the
/// tabs' and the nav's grey that goes white (the user's review,
/// 2026-11-21: no underline, the text lights).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct HoverText {
    pub base: Tone,
    pub over: Tone,
}

/// The mark of a control the kit built (`pressable`): every node a screen
/// can press carries it, so its hover, focus and selected states are the
/// kit's. `checks/tests/it/ui_kit.rs` keeps screens from making nodes
/// pressable by hand, and party_kit's `kit_conformance` test stages every
/// screen and finds any `Interaction` without it (docs/DESIGN.md, "UI:
/// always the kit").
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KitControl;

/// A setting row (`Ui::setting_row`; `kit_conformance`: a form control in
/// a row sits in a setting row's control column).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct SettingRowMark;

/// A row of the kit's (`Ui::row`, `Ui::setting_row`).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct KitRow;

/// What makes a kit node pressable: an `Interaction` that blocks the
/// pointer and `KitControl`. Kit-internal: a screen presses only what a
/// builder returns (`button`, `item`, `setting_row`, ...).
pub(crate) fn pressable() -> (Interaction, FocusPolicy, KitControl) {
    (Interaction::default(), FocusPolicy::Block, KitControl)
}

/// A panel's or a scrim's hold on the pointer (no control: it only keeps
/// clicks and hover from what is under it).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct PointerCatch;

/// What a panel or a scrim takes to catch the pointer (`PointerCatch`):
/// an `Interaction` that blocks it. Not a control: nothing to press.
pub fn catch_pointer() -> (Interaction, FocusPolicy, PointerCatch) {
    (Interaction::default(), FocusPolicy::Block, PointerCatch)
}

/// A pressable node whose hover and selected states a screen's own system
/// draws (the main menu's highlight slab that follows the keys and the
/// pointer, the match end's standings rows that preview the hovered
/// player): `own_states`. Each use is on `checks/tests/it/ui_kit.rs`'s
/// allowlist with its reason; `kit_conformance` accepts it.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct OwnStates;

/// What a node with its own drawn states takes (`OwnStates`): an
/// `Interaction` that blocks the pointer.
pub fn own_states() -> (Interaction, FocusPolicy, OwnStates) {
    (Interaction::default(), FocusPolicy::Block, OwnStates)
}

/// The kit's pressable state for a node another system spawned (a chat
/// line's name, made pressable while the history is open): no fill, the
/// kit's hover tint (`Fg` .06), `KitControl`. `Ui::item` for a node the
/// screen builds.
pub fn press_tint(theme: &Theme) -> impl Bundle + use<> {
    let over = theme.tone_a(Tone::Fg, 0.06);
    (BackgroundColor(Color::NONE), Hover { base: Color::NONE, over }, pressable())
}

/// An `Interaction` that blocks the pointer, without the kit's states:
/// for other looks only (the VGUI/GameUI widgets, in-world and HUD nodes
/// on `checks/tests/it/ui_kit.rs`'s allowlist). A screen of the kit's
/// look uses a builder (`Ui::item` for a pressable surface).
pub fn clickable() -> (Interaction, FocusPolicy) {
    (Interaction::default(), FocusPolicy::Block)
}

/// Height of the default button, cq.
pub const BUTTON_H: f32 = 2.4;
/// Height of the big hero (Start, Connect, Done; a dialog's), cq.
pub const HERO_H: f32 = 2.8;
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

/// A toggle's words (off, on): the default, for a setting that switches a
/// feature.
pub const TOGGLE_ON_OFF: (&str, &str) = ("Off", "On");
/// A toggle that answers a question ("Include a screenshot", "Ask ...").
pub const TOGGLE_YES_NO: (&str, &str) = ("No", "Yes");

/// The words for a toggle labelled `label`: Yes/No for a question-style
/// label (it starts with Include, Ask, Allow or Send, or ends in `?`),
/// else On/Off.
pub fn toggle_words_for(label: &str) -> (&'static str, &'static str) {
    let l = label.trim();
    let first = l.split_whitespace().next().unwrap_or("");
    if l.ends_with('?') || ["include", "ask", "allow", "send"].iter().any(|w| first.eq_ignore_ascii_case(w)) {
        TOGGLE_YES_NO
    } else {
        TOGGLE_ON_OFF
    }
}

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
        // The lockup (design rules 6): the cap's text .82 of the label's,
        // padded equally above and below so its box centres on the label's
        // capitals, a 1 px outline, full strength.
        let e = c
            .spawn((
                UiStyle { pad: Some([0.2, 0.3, 0.2, 0.3].into()), shrink: Some(0.0), ..UiStyle::row() }.outline(tone, 1.0),
                Node::default(),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        self.text_a(c, e, Role::Mono, size * 0.82, tone, 1.0, key);
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
    pub fn lockup(&self, c: &mut Commands, parent: Entity, _images: &mut Assets<Image>, _icons: &mut IconImages, lk: f32) -> Entity {
        self.lockup_drawn(c, parent, lk)
    }

    /// `lockup` with its mark drawn by `draw_kit_icons` (no image assets
    /// at hand: the first run's bar). The same lockup as the main menu's.
    pub fn lockup_drawn(&self, c: &mut Commands, parent: Entity, lk: f32) -> Entity {
        self.lockup_parts(c, parent, lk).root
    }

    /// `lockup_drawn` with its parts named, for a screen that animates
    /// them (the first run's intro): the same nodes, nothing else.
    pub fn lockup_parts(&self, c: &mut Commands, parent: Entity, lk: f32) -> LockupParts {
        let root = self.boxed(c, parent, UiStyle { gap: Some(0.55 * lk), shrink: Some(0.0), ..UiStyle::row() });
        let mark = self.kit_icon(c, root, "mark", 2.05 * lk, MARK_ASPECT, Tone::Fg);
        let words = self.boxed(c, root, UiStyle { gap: Some(0.3 * lk), ..UiStyle::column() });
        let ult = self.boxed(c, words, UiStyle::row().justify(JustifyContent::SpaceBetween));
        let ultimate = "ULTIMATE".chars().map(|ch| self.text(c, ult, Role::DisplayRegular, 0.62 * lk, Tone::Fg2, &ch.to_string())).collect();
        let wordmark = self.text(c, words, Role::Wordmark, lk, Tone::Fg, "Gun game");
        LockupParts { root, mark, ultimate, wordmark }
    }

    // -----------------------------------------------------------------
    // Buttons and tabs.

    /// A button, `h` cq tall (`BUTTON_H`; small 1.9; `HERO_H` the big
    /// hero: Start, Connect, a dialog's own), its label (Display Regular
    /// caps; the big hero Display Bold .9 cq) and an optional key cap (the
    /// lockup: `kbd`, .5 cq after the label). Returns the button (give it
    /// its action).
    ///
    /// The primary is the hero: a `hero` slab with its right edge on the
    /// slant and white text (`on_hero`); hover lifts it to `acc` and leans
    /// its edge out .4 cq (`HeroSlab`); disabled it stays a slab at `fg`
    /// .07 with `fg_3` text.
    #[allow(clippy::too_many_arguments)]
    pub fn button(&self, c: &mut Commands, parent: Entity, kind: Kind, label: &str, key: Option<&str>, h: f32, st: State) -> Entity {
        self.button_icon(c, parent, kind, None, label, key, h, st)
    }

    /// `button` with one of the kit's icons (`kit_icon`: icons.toml's key)
    /// before its label, in the label's tone (Leave's arrow).
    #[allow(clippy::too_many_arguments)]
    pub fn button_icon(&self, c: &mut Commands, parent: Entity, kind: Kind, icon: Option<&'static str>, label: &str, key: Option<&str>, h: f32, st: State) -> Entity {
        let big = kind == Kind::Primary && h >= HERO_H - 0.01;
        let size = if big { 0.9 } else if h < 2.2 { 0.58 } else if h > 3.0 { 0.8 } else { 0.66 };
        let lean = h * self.theme.tan();
        let right = match kind {
            Kind::Primary => 1.5 + lean,
            Kind::Ghost => 0.9,
            _ => 1.2,
        };
        let left = match kind {
            Kind::Ghost => 0.6,
            Kind::Primary => 1.4,
            _ => 1.05,
        };
        let (base, over, text_tone) = match kind {
            Kind::Primary if st.disabled => (Color::NONE, Color::NONE, Tone::Fg3),
            _ if st.disabled => (self.color(Tone::Fg, 0.05), self.color(Tone::Fg, 0.05), Tone::Fg3),
            Kind::Primary => (Color::NONE, Color::NONE, Tone::OnHero),
            Kind::Secondary | Kind::Danger => (self.color(Tone::Ink, 0.85), self.color(Tone::Fg, 0.06), Tone::Fg),
            Kind::Ghost => (Color::NONE, self.color(Tone::Fg, 0.06), Tone::Fg2),
        };
        let mut s = UiStyle { h: Some(h), pad: Some([0.0, right, 0.0, left].into()), gap: Some(0.5), shrink: Some(0.0), ..UiStyle::row() };
        s = match kind {
            Kind::Primary => s,
            _ if st.disabled => s.outline(Tone::Fg5, 1.0),
            Kind::Secondary => s.outline(Tone::Fg3, 1.0),
            Kind::Danger => s.outline(Tone::Threat, 0.55),
            _ => s,
        };
        let b = c.spawn((s, Node::default(), BackgroundColor(base), ChildOf(parent))).id();
        if !st.disabled {
            c.entity(b).insert((Hover { base, over }, pressable()));
        } else {
            c.entity(b).insert(pressable());
        }
        if kind == Kind::Primary {
            // The slab behind, its right edge on the slant.
            let (fill, a) = if st.disabled { (Tone::Fg, 0.07) } else { (Tone::Hero, 1.0) };
            let slab = c
                .spawn((
                    HeroSlab::default(),
                    Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() },
                    UiSlant::new(fill, a).edges(false, true),
                    Pickable::IGNORE,
                    ChildOf(b),
                ))
                .id();
            if !st.disabled {
                // The hover's accent, faded in over the cobalt (`animate_heroes`).
                c.spawn((
                    HeroGlow,
                    Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() },
                    UiSlant::new(Tone::Acc, 0.0).edges(false, true),
                    Pickable::IGNORE,
                    ChildOf(slab),
                ));
            }
        }
        if kind == Kind::Danger {
            self.mark(c, b, Tone::Threat, 0.5, 0.7);
        }
        let role = if big { Role::DisplayBold } else { Role::DisplayRegular };
        if let Some(icon) = icon {
            self.kit_icon(c, b, icon, size * 1.15, 1.0, text_tone);
        }
        self.text(c, b, role, size, text_tone, label);
        if let Some(k) = key {
            let cap_tone = match kind {
                _ if st.disabled => Tone::Fg4,
                Kind::Primary => Tone::OnHero,
                _ => Tone::Fg2,
            };
            self.kbd(c, b, k, size, cap_tone);
        }
        if st.focus && !st.disabled {
            // On the hero's own cobalt the bar is white.
            if kind == Kind::Primary {
                let e = self.focus_bar(c, b, 0.24);
                c.entity(e).insert(BackgroundColor(self.color(Tone::Fg, 1.0)));
            } else {
                self.focus_bar(c, b, 0.24);
            }
        }
        if st.error {
            self.underline(c, b, Tone::Threat, 0.18);
        }
        b
    }

    /// Sub-tabs (the settings' pages): Display .78 cq, the selected one
    /// white, others `fg_3` (white while hovered), no underline; a
    /// hairline under all. Returns each tab.
    pub fn tabs(&self, c: &mut Commands, parent: Entity, names: &[&str], selected: usize, focus: bool) -> Vec<Entity> {
        self.tabs_ex(c, parent, names, selected, focus, None)
    }

    /// `tabs` with the keys that step them as caps at the strip's ends (Q
    /// before the first, E after the last; design rules 11.5); the
    /// hairline runs on to the parent's edge.
    pub fn tabs_ex(&self, c: &mut Commands, parent: Entity, names: &[&str], selected: usize, focus: bool, keys: Option<(&str, &str)>) -> Vec<Entity> {
        self.tabs_badged(c, parent, names, &[], selected, focus, keys)
    }

    /// `tabs_ex` with a badge after a tab's name (`badges[i]`, None or
    /// missing: none; `badge`: a page's changes). The selected tab's
    /// name is white, the others `fg_3`, white while hovered
    /// (`HoverText`); no strip under any (the user's review, 2026-11-21).
    #[allow(clippy::too_many_arguments)]
    pub fn tabs_badged(&self, c: &mut Commands, parent: Entity, names: &[&str], badges: &[Option<String>], selected: usize, _focus: bool, keys: Option<(&str, &str)>) -> Vec<Entity> {
        let bar = c
            .spawn((UiStyle { gap: Some(1.6), align: Some(AlignItems::FlexEnd), ..UiStyle::row() }, Node::default(), ChildOf(parent)))
            .id();
        self.hairline(c, bar, Tone::Fg4, 1.0);
        let cap = |c: &mut Commands, k: &str| {
            let w = self.boxed(c, bar, UiStyle { pad: Some([0.0, 0.0, 0.75, 0.0].into()), ..UiStyle::row() });
            let e = self.kbd(c, w, k, 0.74, Tone::Fg2);
            c.entity(e).insert(TabKey);
        };
        if let Some((q, _)) = keys {
            cap(c, q);
        }
        let tabs: Vec<Entity> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                let sel = i == selected;
                let tone = if sel { Tone::Fg } else { Tone::Fg3 };
                let t = c
                    .spawn((
                        UiStyle { gap: Some(0.5), pad: Some([0.0, 0.0, 0.7, 0.0].into()), align: Some(AlignItems::Center), ..UiStyle::row() },
                        Node::default(),
                        BackgroundColor(Color::NONE),
                        Hover { base: Color::NONE, over: Color::NONE },
                        pressable(),
                        ChildOf(bar),
                    ))
                    .id();
                let tx = self.text(c, t, Role::DisplayRegular, 0.86, tone, n);
                if !sel {
                    c.entity(tx).insert(HoverText { base: Tone::Fg3, over: Tone::Fg });
                }
                if let Some(Some(b)) = badges.get(i) {
                    self.badge(c, t, b, crate::form::BadgeKind::Hero);
                }
                t
            })
            .collect();
        if let Some((_, e)) = keys {
            cap(c, e);
        }
        tabs
    }

    /// A pressable surface laid out as `style` says (a list entry, a rail
    /// item, a card, a chip, a fold's head): the kit's states on it. Hover
    /// lifts its fill (`Fg` .06 over none, a faint `Fg` fill by .06, any
    /// other fill to `Fg` .1); `selected` is the white fill (give its text
    /// `Tone::Ink`); `st.focus` the row tint and the accent bar;
    /// `st.disabled` takes no hover. Returns it (give it its action).
    pub fn item(&self, c: &mut Commands, parent: Entity, style: UiStyle, st: State, selected: bool) -> Entity {
        let bg = style.bg;
        let e = c.spawn((UiStyle { bg: None, ..style }, Node::default(), self.item_bundle(bg, st, selected), ChildOf(parent))).id();
        if st.focus && !selected {
            self.focus_bar(c, e, 0.24);
        }
        e
    }

    /// What `item` puts on its node, for a screen that spawns its nodes
    /// in a `with_children` closure (the match end's): its fill (`bg`, a
    /// tone and alpha), the kit's hover and `KitControl`.
    pub fn item_bundle(&self, bg: Option<(Tone, f32)>, st: State, selected: bool) -> impl Bundle + use<> {
        let base = if selected {
            self.color(Tone::Fg, 1.0)
        } else if st.focus {
            self.color(Tone::Fg, 0.06)
        } else {
            bg.map_or(Color::NONE, |(t, a)| self.color(t, a))
        };
        let over = match (selected || st.disabled, st.focus, bg) {
            (true, ..) => base,
            (_, true, _) => self.color(Tone::Fg, 0.09),
            (_, _, None) => self.color(Tone::Fg, 0.06),
            (_, _, Some((Tone::Fg, a))) if a < 0.5 => self.color(Tone::Fg, a + 0.06),
            _ => self.color(Tone::Fg, 0.1),
        };
        (BackgroundColor(base), Hover { base, over }, pressable())
    }

    // -----------------------------------------------------------------
    // Rows.

    /// A settings row: the label (Display .8 cq) with an optional small
    /// note, the value at the right. Hover: its fill lifts (no tick);
    /// focus: tint .06 and the accent bar; changed: the accent square before the label; error: the
    /// threat bar at the left. Returns (row, value box).
    pub fn row(&self, c: &mut Commands, parent: Entity, label: &str, small: Option<&str>, min_h: f32, st: State) -> (Entity, Entity) {
        let (row, _, vl) = self.row_parts(c, parent, label, small, min_h, st);
        (row, vl)
    }

    /// `row`, also returning its label box: (row, label box, value box).
    pub fn row_parts(&self, c: &mut Commands, parent: Entity, label: &str, small: Option<&str>, min_h: f32, st: State) -> (Entity, Entity, Entity) {
        // One marker a row (the user's review, 2026-11-21): hovered or
        // focused, the row alone lifts and shows the accent bar; the
        // control in it draws no focus line of its own.
        let base = if st.focus { self.color(Tone::Fg, 0.06) } else { Color::NONE };
        let over = if st.focus { base } else { self.color(Tone::Fg, 0.05) };
        let row = c
            .spawn((
                UiStyle { pad: Some([0.0, 1.3, 0.0, 1.1].into()), gap: Some(1.2), shrink: Some(0.0), ..UiStyle::row() }
                    .justify(JustifyContent::SpaceBetween),
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                pressable(),
                KitRow,
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
        } else if !st.disabled {
            let bar = self.focus_bar(c, row, 0.24);
            c.entity(bar).insert((Tick { always: false }, Visibility::Hidden));
        }
        (row, lb, vl)
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

    /// A toggle: a slab sliding on a slanted track, OFF/ON (or its
    /// words) each centred on its half (the slab's place); on, the slab
    /// is white at the right; `slide` slides it over from the other side
    /// (`ToggleSlide`: flipped at `since`, `TOGGLE_SLIDE_SECS`, eased out;
    /// none under reduced motion). It is `w` x `h` cq: the slab and its
    /// insets scale with the height, each half as wide as the track
    /// allows. Screens use `form`'s `big_toggle` (the user's review,
    /// 2026-11-21: the small toggle is gone).
    #[allow(clippy::too_many_arguments)]
    pub fn toggle_sized(&self, c: &mut Commands, parent: Entity, on: bool, st: State, slide: Option<ToggleSlide>, words: (&str, &str), w: f32, h: f32) -> Entity {
        let k = h / TOGGLE_H;
        let (off_left, top) = (TOGGLE_OFF_LEFT * k, 0.2 * k);
        let slab_w = w / 2.0 - off_left - 0.05 * k;
        let on_left = w - off_left - slab_w;
        let word = 0.5 * k.min(1.3);
        let t = c
            .spawn((
                UiStyle { w: Some(w), h: Some(h), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(Color::NONE),
                Hover { base: Color::NONE, over: Color::NONE },
                pressable(),
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
        let left = if on { on_left } else { off_left };
        let s = self.slant_at(c, t, slab, [Some(left), Some(top), None, None], slab_w, h - 2.0 * top);
        c.entity(s).insert((ToggleSlab { track: t }, UiTransform::default()));
        if let Some(sl) = slide.filter(|sl| !sl.motion.snaps()) {
            let from = if on { off_left - on_left } else { on_left - off_left };
            let tl = Timeline::new([Track::new(Prop::X, from, 0.0).dur(sl.motion.secs(Pace::Normal)).ease(Pace::Normal.ease())]).started(sl.since);
            if !tl.done(sl.now) {
                let x = tl.sample(Prop::X, tl.local(sl.now)).unwrap_or(0.0);
                c.entity(s).insert((tl, UiTransform { translation: Val2::px(self.units.cq(x), 0.0), ..default() }));
            }
        }
        // Each label centred on its half, where the slab stands for it.
        for (l, x, lit) in [(words.0, off_left, !on), (words.1, on_left, on)] {
            let half = self.boxed(
                c,
                t,
                UiStyle { abs: Some([Some(x), Some(0.0), None, Some(0.0)]), w: Some(slab_w), justify: Some(JustifyContent::Center), ..UiStyle::row() },
            );
            c.entity(half).insert((ToggleLabel, Pickable::IGNORE));
            // On: ink on the white slab; off: white on the dim one; the
            // side the slab isn't on, faint.
            let tone = match (lit, on) {
                (true, true) => Tone::Ink,
                (true, false) => Tone::Fg,
                _ => Tone::Fg3,
            };
            self.text(c, half, Role::Mono, word, tone, l);
        }
        if (st.focus || st.error) && !st.disabled {
            // Focus (error: threat) as a strip along the track's bottom
            // edge, its ends on the track's slant (Rule 2).
            let tone = if st.error { Tone::Threat } else { Tone::Acc };
            let sh = 0.2;
            let tan = self.theme.tan();
            let strip = self.slant_at(c, t, UiSlant::new(tone, 1.0), [Some(0.0), None, None, Some(0.0)], w - h * tan + sh * tan, sh);
            c.entity(strip).insert(ToggleFocus);
        }
        t
    }

    /// A segmented choice: the toggle's look (a slanted track with a
    /// `fg_4` outline, the options inset in it), each option Mono; the
    /// selected one a white slanted pick (ink text); focus underlines it
    /// in accent, clipped to the pick's slant. A changed selection slides
    /// the pick over from the old one (`slide_picks`). The options at
    /// `off` are greyed out (no hover, not pressable). `size` = (w, h) cq:
    /// the options share the width equally (`form`'s `choice`, a filter
    /// bar of a set width); None: as wide as their words. Returns the
    /// options.
    #[allow(clippy::too_many_arguments)]
    pub fn segmented_sized(&self, c: &mut Commands, parent: Entity, options: &[&str], selected: Option<usize>, off: &[usize], st: State, size: Option<(f32, f32)>) -> Vec<Entity> {
        // One slanted track (fill .05, a `fg_4` outline), slanted `fg_5`
        // dividers, the pick a white slanted slab; the words stay upright
        // (Display for words, Mono for numbers). Hover and pick are
        // parallelograms on the dividers' slant, not rectangles.
        let h = size.map_or(TOGGLE_H, |s| s.1) - 0.4;
        let lean = h * self.theme.tan();
        let u = self.units;
        let key = {
            use std::hash::{Hash, Hasher};
            let mut hs = std::collections::hash_map::DefaultHasher::new();
            options.hash(&mut hs);
            hs.finish()
        };
        let g = c
            .spawn((UiStyle { w: size.map(|s| s.0), shrink: Some(0.0), pad: Some([0.2, 0.25, 0.2, 0.25].into()), ..UiStyle::row() }, Node::default(), SegmentedTrack, ChildOf(parent)))
            .id();
        let full = || Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() };
        c.spawn((full(), UiSlant::new(Tone::Fg, 0.05), Pickable::IGNORE, ChildOf(g)));
        c.spawn((full(), UiSlant::new(Tone::Fg4, 1.0).outline(u.phys(u.hairline())), Pickable::IGNORE, ChildOf(g)));
        let n = options.len();
        // Option i's parallelogram: on the dividers' slant at both sides
        // (a box reaching half a lean past the rectangle at an inner edge).
        let shape = |i: usize| Node {
            position_type: PositionType::Absolute,
            left: if i == 0 { Val::ZERO } else { u.val(-lean / 2.0) },
            right: if i + 1 == n { Val::ZERO } else { u.val(-lean / 2.0) },
            top: Val::ZERO,
            bottom: Val::ZERO,
            ..default()
        };
        options
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let sel = selected == Some(i);
                let greyed = off.contains(&i) && !sel;
                let (lp, rp) = (0.75 + if i == 0 { lean * 0.6 } else { lean * 0.5 }, 0.75 + if i + 1 == n { lean * 0.6 } else { lean * 0.5 });
                let b = c
                    .spawn((
                        UiStyle {
                            h: Some(h),
                            pad: Some(if size.is_some() { [0.0, 0.3, 0.0, 0.3] } else { [0.0, rp, 0.0, lp] }.into()),
                            w: size.map(|s| (s.0 - 0.5) / n as f32),
                            justify: Some(JustifyContent::Center),
                            ..UiStyle::row()
                        },
                        Node::default(),
                        ChildOf(g),
                    ))
                    .id();
                if !greyed {
                    c.entity(b).insert(pressable());
                }
                if sel {
                    c.spawn((
                        shape(i),
                        UiSlant::new(Tone::Fg, 1.0),
                        SegmentedPick { track: g, key },
                        UiTransform::default(),
                        Pickable::IGNORE,
                        ChildOf(b),
                    ))
                    .with_children(|k| {
                        if st.focus {
                            // The focus strip on the pick's foot: its box
                            // starts at the pick's bottom-left corner and
                            // ends at the bottom-right one, on the same
                            // slant, so it stays inside the pick.
                            let sh = 0.2;
                            let lean_s = sh * self.theme.tan();
                            k.spawn((
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::ZERO,
                                    right: u.val(lean - lean_s),
                                    bottom: Val::ZERO,
                                    height: u.val(sh),
                                    ..default()
                                },
                                UiSlant::new(Tone::Acc, 1.0),
                                Pickable::IGNORE,
                            ));
                        }
                    });
                } else {
                    if !greyed {
                        // The hover: the option's own parallelogram, shown
                        // while it is hovered (`show_hover`).
                        c.spawn((shape(i), UiSlant::new(Tone::Fg, 0.06), Tick { always: false }, Visibility::Hidden, Pickable::IGNORE, ChildOf(b)));
                    }
                    if i > 0 && selected != Some(i - 1) {
                        // A slanted divider at its left edge.
                        c.spawn((
                            Node { position_type: PositionType::Absolute, left: u.val(-lean / 2.0), top: Val::ZERO, bottom: Val::ZERO, width: Val::Px(u.cq(lean) + u.hairline()), ..default() },
                            UiSlant::new(Tone::Fg4, 1.0),
                            Pickable::IGNORE,
                            ChildOf(b),
                        ));
                    }
                }
                let (role, size) = crate::form::seg_face(o);
                self.text(c, b, role, size, if sel { Tone::Ink } else if greyed { Tone::Fg4 } else { Tone::Fg2 }, o);
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
                pressable(),
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
                pressable(),
                crate::text_menu::TextField {
                    masked: false,
                    editing: typing.is_some(),
                    selected: typing.is_some_and(|t| t.caret != t.anchor),
                    empty: typing.map_or(value.is_empty(), |t| t.text.is_empty()),
                },
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
    pub(crate) fn edit_chars(&self, c: &mut Commands, parent: Entity, text: &str, face: Face, caret: Option<(usize, usize)>) -> Vec<Entity> {
        let n = text.chars().count();
        let caret = caret.map(|(a, b)| (a.min(n), b.min(n)));
        self.edit_run(c, parent, text, face, caret, 0, false)
    }

    /// One line of an edit text: `text` starts at char `base` of the
    /// whole (so `caret`'s indices are the whole's); `wrap` lets its
    /// characters break onto more lines inside `parent` (a text area's).
    #[allow(clippy::too_many_arguments)]
    fn edit_run(&self, c: &mut Commands, parent: Entity, text: &str, face: Face, caret: Option<(usize, usize)>, base: usize, wrap: bool) -> Vec<Entity> {
        let line = self.units.text_px(face.size) / self.units.scale_factor;
        let wrap_node = if wrap {
            Node {
                flex_shrink: 1.0,
                flex_grow: 1.0,
                min_height: Val::Px(line),
                align_items: AlignItems::Center,
                flex_wrap: FlexWrap::Wrap,
                ..default()
            }
        } else {
            Node { flex_shrink: 0.0, align_items: AlignItems::Center, ..default() }
        };
        let wrap = c.spawn((wrap_node, Pickable::IGNORE, ChildOf(parent))).id();
        let n = text.chars().count();
        let (lo, hi) = caret.map_or((0, 0), |(a, b)| (a.min(b), a.max(b)));
        let mut out = Vec::with_capacity(n);
        for (i, ch) in text.chars().enumerate() {
            let sel = base + i >= lo && base + i < hi;
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
        if let Some((at, _)) = caret.filter(|(at, _)| (base..=base + n).contains(at)) {
            let at = at - base;
            let (holder, left) = match out.get(at) {
                Some(cell) => (*cell, Val::ZERO),
                None => (out.last().copied().unwrap_or(wrap), if n == 0 { Val::ZERO } else { percent(100.0) }),
            };
            // Thin (1-2 px): one physical pixel, two on a window over 1440
            // lines; just after the character before it (at the boundary's
            // left), in the text's white.
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
                BackgroundColor(self.color(Tone::Fg, 1.0)),
                TextCaret,
                ZIndex(1),
                Pickable::IGNORE,
                ChildOf(holder),
            ));
        }
        out
    }

    /// A stepper: one compact control, a single slanted border around
    /// `-`, the value and `+` (keys 1.5 x 1.7 cq, no gaps), the value
    /// box as wide as `range` asks (`StepperRange::width`). Returns
    /// (minus, plus).
    pub fn stepper(&self, c: &mut Commands, parent: Entity, value: &str, range: StepperRange, st: State) -> (Entity, Entity) {
        let p = self.stepper_ex(c, parent, value, range, st, None);
        (p.minus, p.plus)
    }

    /// `stepper` with its value box clickable (a number typed in) and,
    /// while `typing`, a text field in its place. Parts, in order: minus,
    /// value, plus, the border (an overlay that takes no room). A key at
    /// its end of the range (`StepperRange::reached`) or a disabled
    /// stepper is dimmed (still a node, so a click lands where it did).
    pub fn stepper_ex(&self, c: &mut Commands, parent: Entity, value: &str, range: StepperRange, st: State, typing: Option<Typing>) -> StepperParts {
        self.stepper_at(c, parent, value, range, st, typing, None)
    }

    /// `stepper` filling a column `w` cq wide (a settings row's control
    /// column, `form`): the keys keep their size, the value box takes the
    /// rest (never less than `range` asks).
    pub fn stepper_filling(&self, c: &mut Commands, parent: Entity, value: &str, range: StepperRange, st: State, w: f32) -> (Entity, Entity) {
        let p = self.stepper_at(c, parent, value, range, st, None, Some(w));
        (p.minus, p.plus)
    }

    #[allow(clippy::too_many_arguments)]
    fn stepper_at(&self, c: &mut Commands, parent: Entity, value: &str, range: StepperRange, st: State, typing: Option<Typing>, fill: Option<f32>) -> StepperParts {
        let g = c.spawn((UiStyle { w: fill, shrink: Some(0.0), ..UiStyle::row() }, Node { align_self: AlignSelf::FlexStart, ..default() }, ChildOf(parent))).id();
        let key = |c: &mut Commands, label: &str, dim: bool, edges: (bool, bool)| {
            let b = c
                .spawn((
                    UiStyle { w: Some(STEPPER_KEY_W), h: Some(STEPPER_H), justify: Some(JustifyContent::Center), ..UiStyle::row() },
                    Node::default(),
                    BackgroundColor(Color::NONE),
                    pressable(),
                    ChildOf(g),
                ))
                .id();
            // The hover tint follows the border's slant at its own end.
            if !dim {
                c.spawn((
                    Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, ..default() },
                    UiSlant::new(Tone::Fg, 0.09).edges(edges.0, edges.1),
                    Tick { always: false },
                    Visibility::Hidden,
                    Pickable::IGNORE,
                    ChildOf(b),
                ));
            }
            self.text(c, b, Role::Mono, 0.8, if dim { Tone::Fg4 } else { Tone::Fg }, label);
            b
        };
        let off = st.disabled;
        let minus = key(c, "-", off || range.at_min, (true, false));
        // As tall as the keys, so the focus underline sits under the
        // value with a gap, as the slider's and the fields' do.
        let w = fill.map_or(0.0, |f| f - 2.0 * STEPPER_KEY_W).max(range.width(value));
        let value = self.value_box(c, g, value, w, st, typing, ValueLook::STEPPER);
        let plus = key(c, "+", off || range.at_max, (false, true));
        // The one border: the whole control's, leaning with the kit's slant.
        let tone = if off { Tone::Fg5 } else if st.focus { Tone::Fg2 } else { Tone::Fg3 };
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, right: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, ..default() },
            UiSlant::new(tone, 1.0).outline(self.units.phys(self.units.hairline())),
            StepperBorder,
            Pickable::IGNORE,
            ChildOf(g),
        ));
        if off {
            c.entity(value).insert(Pickable::IGNORE);
        }
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
                pressable(),
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
                        pressable(),
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
    /// listening, the capture chip (an accent tint and outline, a blinking
    /// light, "Press a key", Esc's cap: "cancel"). Returns the field.
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
        let f = c.spawn((s, Node::default(), BackgroundColor(base), Hover { base, over }, pressable(), ChildOf(parent))).id();
        if listening {
            // The capture chip (the user's review, 2026-11-21): a
            // blinking light, "Press a key", Esc's cap at the end.
            let light = self.mark(c, f, Tone::AccText, 0.5, 0.5);
            c.entity(light).insert(crate::form::Blink);
            self.text(c, f, Role::Mono, 0.58, Tone::AccText, "Press a key");
            let end = self.boxed(c, f, UiStyle { gap: Some(0.4), grow: Some(1.0), justify: Some(JustifyContent::FlexEnd), ..UiStyle::row() });
            self.kbd(c, end, "Esc", 0.5, Tone::Fg3);
            self.text(c, end, Role::Mono, 0.5, Tone::Fg3, "cancel");
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
    /// accent. Error: the threat underline. Disabled: dimmed and not
    /// clickable. Hover is its own tint (no tick). Returns the field (give
    /// it its focusing action; `TextHits` places a click).
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
        self.field(c, parent, value, placeholder, caret, masked, w, st, false, (1.9, TYPED)).field
    }

    /// `text_field` at a screen's hero size: `h` cq tall, the typed text
    /// `text` cq (`[layout.field]`'s `hero_h` and `hero_text`: a first-run
    /// screen's one field; the same look, caret and selection).
    #[allow(clippy::too_many_arguments)]
    pub fn text_field_hero(&self, c: &mut Commands, parent: Entity, value: &str, placeholder: &str, caret: Option<TextCursor>, w: f32, st: State) -> Entity {
        let size = (self.theme.layout_opt("field", "hero_h").unwrap_or(3.2), self.theme.layout_opt("field", "hero_text").unwrap_or(0.9));
        self.field(c, parent, value, placeholder, caret, false, w, st, false, size).field
    }

    /// A search field: `text_field` with the search glass before the text
    /// and, once something is typed, a clear key (`SearchParts::clear`,
    /// give it its action) at the right. Every search box is this one:
    /// the same look, the same caret and selection, nothing moving as
    /// the text grows.
    #[allow(clippy::too_many_arguments)]
    pub fn search_field(
        &self,
        c: &mut Commands,
        parent: Entity,
        value: &str,
        placeholder: &str,
        caret: Option<TextCursor>,
        w: f32,
        st: State,
    ) -> SearchParts {
        self.field(c, parent, value, placeholder, caret, false, w, st, true, (1.9, TYPED))
    }

    #[allow(clippy::too_many_arguments)]
    fn field(
        &self,
        c: &mut Commands,
        parent: Entity,
        value: &str,
        placeholder: &str,
        caret: Option<TextCursor>,
        masked: bool,
        w: f32,
        st: State,
        search: bool,
        (h, typed): (f32, f32),
    ) -> SearchParts {
        let typing = caret.is_some() && !st.disabled;
        let (base, over) = if st.disabled {
            (self.color(Tone::Fg, 0.03), self.color(Tone::Fg, 0.03))
        } else if typing {
            (self.color(Tone::Fg, 0.08), self.color(Tone::Fg, 0.08))
        } else {
            (self.color(Tone::Fg, 0.04), self.color(Tone::Fg, 0.07))
        };
        let f = c
            .spawn((
                UiStyle { min_w: Some(w), h: Some(h), pad: Some([0.0, 0.8].into()), shrink: Some(0.0), gap: search.then_some(0.6), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                RelativeCursorPosition::default(),
                ChildOf(parent),
            ))
            .id();
        if !st.disabled {
            c.entity(f).insert((pressable(), crate::text_menu::TextField::of(value, caret.filter(|_| typing), masked)));
        }
        if search {
            let px = self.units.phys(self.units.cq(SEARCH_H)).round().max(1.0);
            c.spawn((
                ImageNode { color: self.theme.tone(if value.is_empty() || st.disabled { Tone::Fg3 } else { Tone::Fg }), ..default() },
                Node {
                    width: Val::Px(px / self.units.scale_factor),
                    height: Val::Px(px / self.units.scale_factor),
                    flex_shrink: 0.0,
                    ..default()
                },
                KitIcon { key: "search", px: px as u32 },
                Pickable::IGNORE,
                ChildOf(f),
            ));
        }
        let shown: String = if masked { "*".repeat(value.chars().count()) } else { value.to_string() };
        // Typed text in the sentence face (`TYPED`), its own case; stars
        // in Mono.
        let (role, size) = if masked { (Role::Mono, 0.66) } else { (Role::Text, typed) };
        let tone = if st.disabled { Tone::Fg3 } else { Tone::Fg };
        if value.is_empty() && !typing {
            self.text(c, f, Role::Mono, 0.62 * typed / TYPED, Tone::Fg3, placeholder);
            c.entity(f).insert(EditText { chars: Vec::new() });
        } else {
            let face = Face { role, size, tone, plain: !masked };
            let k = caret.filter(|_| typing).map(|k| (char_of(value, k.at), char_of(value, k.anchor)));
            let chars = self.edit_chars(c, f, &shown, face, k);
            c.entity(f).insert(EditText { chars });
        }
        let mut clear = None;
        if search && !value.is_empty() && !st.disabled {
            self.boxed(c, f, UiStyle { grow: Some(1.0), ..UiStyle::row() });
            let k = c
                .spawn((
                    UiStyle { w: Some(1.4), h: Some(1.4), justify: Some(JustifyContent::Center), shrink: Some(0.0), ..UiStyle::row() },
                    Node::default(),
                    BackgroundColor(Color::NONE),
                    Hover { base: Color::NONE, over: self.color(Tone::Fg, 0.1) },
                    pressable(),
                    SearchClear,
                    ChildOf(f),
                ))
                .id();
            self.text(c, k, Role::Mono, 0.6, Tone::Fg3, "x");
            clear = Some(k);
        }
        if st.error {
            self.underline(c, f, Tone::Threat, 0.2);
        } else if (typing || st.focus) && !st.disabled {
            self.underline(c, f, Tone::Acc, 0.2);
        } else {
            self.hairline(c, f, Tone::Fg3, 1.0);
        }
        SearchParts { field: f, clear }
    }

    /// A text area (`w` cq wide, `h` tall; `value` may hold newlines): the
    /// field's text one node a character, the lines wrapping inside the
    /// box, `placeholder` shown while it is empty, the caret an overlay
    /// (`caret`: byte offsets, as `text_field`'s; Some while it has the
    /// keys) and the selection a tint, so typing never moves what is
    /// already there. Error: the threat underline. A click's place is not
    /// found (no `EditText`: the caret goes where the screen puts it).
    #[allow(clippy::too_many_arguments)]
    pub fn text_area(
        &self,
        c: &mut Commands,
        parent: Entity,
        value: &str,
        placeholder: &str,
        caret: Option<TextCursor>,
        w: f32,
        h: f32,
        st: State,
    ) -> Entity {
        let typing = caret.is_some() && !st.disabled;
        let base = self.color(Tone::Fg, if typing { 0.08 } else { 0.05 });
        let area = c
            .spawn((
                UiStyle { w: Some(w), h: Some(h), pad: Some([0.7, 0.9].into()), ..UiStyle::column() }.align(AlignItems::Stretch),
                Node { overflow: Overflow::clip(), ..default() },
                BackgroundColor(base),
                RelativeCursorPosition::default(),
                crate::text_menu::TextField::of(value, caret.filter(|_| typing), false),
                ChildOf(parent),
            ))
            .id();
        let face = Face { role: Role::Text, size: TYPED, tone: if st.disabled { Tone::Fg3 } else { Tone::Fg }, plain: true };
        let k = caret.filter(|_| typing).map(|k| (char_of(value, k.at), char_of(value, k.anchor)));
        if value.is_empty() {
            let line = self.boxed(c, area, UiStyle::row());
            self.edit_run(c, line, "", face, k, 0, false);
            self.text_plain(c, line, Role::Text, TYPED, Tone::Fg3, placeholder);
        } else {
            let mut base = 0;
            for l in value.split('\n') {
                let line = self.boxed(c, area, UiStyle::row().align(AlignItems::FlexStart));
                self.edit_run(c, line, l, face, k, base, true);
                base += l.chars().count() + 1;
            }
        }
        if st.error {
            self.underline(c, area, Tone::Threat, 0.2);
        } else if typing || st.focus {
            self.underline(c, area, Tone::Acc, 0.2);
        } else {
            self.hairline(c, area, Tone::Fg3, 1.0);
        }
        area
    }

    /// The progress component (design rules 11.6, variant B): a sheared
    /// bar `w` x `h` cq with its words (`label`, Display caps) and the
    /// percentage (Mono) inside, centred, `size` cq. In fg only:
    /// running a hollow `fg_2` outline filling from the left in `fg_2`
    /// (the words in ink over the fill); ready a solid `fg` slab with ink
    /// words, no percentage; pending a dim `fg_4` outline with `fg_3`
    /// words; connecting (no percentage known) the hollow outline with
    /// 12° stripes through it. `inside: false` leaves the words out (the
    /// loading screen's big bar has them above).
    #[allow(clippy::too_many_arguments)]
    pub fn progress(&self, c: &mut Commands, parent: Entity, p: Progress, label: &str, w: f32, h: f32, size: f32, inside: bool) -> Entity {
        let u = self.units;
        let outline_px = (1.5 * u.phys(u.height) / 1080.0).round().max(1.0);
        let bar = c
            .spawn((
                UiStyle { w: Some(w), h: Some(h), shrink: Some(0.0), justify: Some(JustifyContent::Center), ..UiStyle::row() },
                Node::default(),
                ProgressBar(p),
                ChildOf(parent),
            ))
            .id();
        let full = || Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, bottom: Val::ZERO, right: Val::ZERO, ..default() };
        let (text_tone, pct) = match p {
            Progress::Ready => {
                c.spawn((full(), UiSlant::new(Tone::Fg, 1.0), Pickable::IGNORE, ChildOf(bar)));
                (Tone::Ink, None)
            }
            Progress::Pending => {
                c.spawn((full(), UiSlant::new(Tone::Fg4, 1.0).outline(outline_px), Pickable::IGNORE, ChildOf(bar)));
                (Tone::Fg3, None)
            }
            Progress::Connecting => {
                // Stripes on the slant, every .8 cq, then the outline. Each
                // is cut to the track's own edges: its box clamped to the
                // bar's, which keeps its leaning sides on the track's
                // (a stripe past an end is just narrower).
                let lean = h * self.theme.tan();
                let n = (w / 0.8).ceil() as i32 + 2;
                let clip = c
                    .spawn((Node { overflow: Overflow::clip(), ..full() }, Pickable::IGNORE, ChildOf(bar)))
                    .id();
                for k in -1..n {
                    let (a, b) = ((k as f32 * 0.8).max(0.0), (k as f32 * 0.8 + 0.35 + lean).min(w));
                    if b - a <= lean {
                        continue;
                    }
                    c.spawn((
                        Node { position_type: PositionType::Absolute, left: u.val(a), top: Val::ZERO, bottom: Val::ZERO, width: u.val(b - a), ..default() },
                        UiSlant::new(Tone::Fg, 0.14),
                        Pickable::IGNORE,
                        ChildOf(clip),
                    ));
                }
                c.spawn((full(), UiSlant::new(Tone::Fg3, 1.0).outline(outline_px), Pickable::IGNORE, ChildOf(bar)));
                (Tone::Fg, None)
            }
            Progress::Running(f) => {
                let f = f.clamp(0.0, 1.0);
                c.spawn((full(), UiSlant::new(Tone::Fg2, 1.0).outline(outline_px), Pickable::IGNORE, ChildOf(bar)));
                if f > 0.0 {
                    // The share done: the track's own shape, its right end
                    // cut along the slant, so it stays inside the track.
                    c.spawn((full(), UiSlant::new(Tone::Fg2, 1.0).clipped(0.0, 1.0 - f), Pickable::IGNORE, ChildOf(bar)));
                }
                (Tone::Fg, Some(f))
            }
        };
        let words = match (p, pct) {
            (Progress::Running(_), Some(f)) if label.is_empty() => format!("{:.0}%", f * 100.0),
            (Progress::Running(_), Some(f)) => format!("{label} {:.0}%", f * 100.0),
            _ => label.to_string(),
        };
        let write = |c: &mut Commands, holder: Entity, tone: Tone| {
            if inside && !words.is_empty() {
                let t = self.boxed(c, holder, UiStyle { w: Some(w), h: Some(h), shrink: Some(0.0), justify: Some(JustifyContent::Center), ..UiStyle::row() });
                self.text(c, t, Role::DisplayRegular, size, tone, &words);
            }
        };
        write(c, bar, text_tone);
        if let Some(f) = pct {
            // The words again in ink over the slab, clipped to the share done.
            let fill = c
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: Val::ZERO,
                        top: Val::ZERO,
                        bottom: Val::ZERO,
                        width: Val::Percent(f * 100.0),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    ProgressFill,
                    Pickable::IGNORE,
                    ChildOf(bar),
                ))
                .id();
            let inner = c
                .spawn((Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, width: u.val(w), height: u.val(h), ..default() }, ChildOf(fill)))
                .id();
            write(c, inner, Tone::Ink);
        }
        bar
    }

    /// The status block (design rules 16): a small slanted light (white
    /// when it worked, threat when it failed, an outline otherwise) and
    /// STATUS beside it on a hairline underline; its words are the
    /// hover's tooltip (`shell::Tip`).
    pub fn status_block(&self, c: &mut Commands, parent: Entity, light: StatusLight) -> Entity {
        let b = c
            .spawn((
                UiStyle { gap: Some(0.45), pad: Some([0.25, 0.4, 0.25, 0.4].into()), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                pressable(),
                StatusBlock(light),
                ChildOf(parent),
            ))
            .id();
        self.hairline(c, b, Tone::Fg4, 1.0);
        let lean = 0.55 * self.theme.tan();
        let mut l = c.spawn((
            UiStyle { w: Some(0.9 + lean), h: Some(0.55), shrink: Some(0.0), ..UiStyle::row() },
            Node::default(),
            Pickable::IGNORE,
            ChildOf(b),
        ));
        match light {
            StatusLight::Ok => l.insert(UiSlant::new(Tone::Fg, 1.0)),
            StatusLight::Failed => l.insert(UiSlant::new(Tone::Threat, 1.0)),
            StatusLight::Off | StatusLight::Busy => l.insert(UiSlant::new(Tone::Fg3, 1.0).outline(self.units.phys(self.units.hairline()) * 1.5)),
        };
        self.text(c, b, Role::Mono, 0.5, Tone::Fg2, "Status");
        b
    }

    /// A field that opens a dialog (Host's Maps): the value at the left in
    /// the text face, the list-with-chevron icon at the right (design rules
    /// 16: not a dropdown's chevron, not a link's arrow). Clickable;
    /// returns the field.
    #[allow(clippy::too_many_arguments)]
    pub fn dialog_field(&self, c: &mut Commands, parent: Entity, images: &mut Assets<Image>, icons: &mut IconImages, value: &str, w: f32, st: State) -> Entity {
        let base = self.color(Tone::Fg, 0.04);
        let over = self.color(Tone::Fg, 0.07);
        let f = c
            .spawn((
                UiStyle { w: Some(w), h: Some(1.9), pad: Some([0.0, 0.8, 0.0, 0.8].into()), justify: Some(JustifyContent::SpaceBetween), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                pressable(),
                ChildOf(parent),
            ))
            .id();
        self.hairline(c, f, Tone::Fg3, 1.0);
        self.sentence(c, f, TYPED, Tone::Fg, value);
        self.icon(c, f, images, icons, "listchev", 0.85, Tone::Fg2);
        if st.focus {
            self.underline(c, f, Tone::Acc, 0.18);
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

    /// `kv` with a status light before each value (`Some(tone)`: the
    /// light lit in it, `None`: unlit, the value `fg_3`): a check list's
    /// rows (reaching a hub: the address, the certificate, the code).
    pub fn kv_lit(&self, c: &mut Commands, parent: Entity, rows: &[(&str, String, Option<Tone>)], kw: f32) -> Entity {
        let list = self.boxed(c, parent, UiStyle::column());
        for (k, v, light) in rows {
            let line = self.boxed(c, list, UiStyle { gap: Some(1.0), pad: Some([0.6, 0.0].into()), align: Some(AlignItems::Center), ..UiStyle::row() });
            self.hairline(c, line, Tone::Fg, 0.07);
            let kc = self.boxed(c, line, UiStyle { w: Some(kw), shrink: Some(0.0), ..UiStyle::row() });
            self.text(c, kc, Role::Mono, 0.6, Tone::Fg3, k);
            self.mark(c, line, light.unwrap_or(Tone::Fg4), 0.6, 0.6);
            self.text_plain(c, line, Role::Mono, 0.6, if light.is_some() { Tone::Fg } else { Tone::Fg3 }, v);
        }
        list
    }

    /// The blueprint plate's corner brackets on `plate` (absolute, over
    /// its edges): an L at the top left and one at the bottom right,
    /// `len` cq long, 2 px, in `tone` (`acc_text`: a pane that explains).
    pub fn corners(&self, c: &mut Commands, plate: Entity, tone: Tone, len: f32) {
        let px = 2.0 / self.units.scale_factor;
        let t = self.units.val(len);
        let bar = |c: &mut Commands, node: Node| {
            c.spawn((node, BackgroundColor(self.color(tone, 0.8)), Pickable::IGNORE, ChildOf(plate)));
        };
        let (m, z) = (Val::Px(-1.0), Val::Px(px));
        bar(c, Node { position_type: PositionType::Absolute, left: m, top: m, width: t, height: z, ..default() });
        bar(c, Node { position_type: PositionType::Absolute, left: m, top: m, width: z, height: t, ..default() });
        bar(c, Node { position_type: PositionType::Absolute, right: m, bottom: m, width: t, height: z, ..default() });
        bar(c, Node { position_type: PositionType::Absolute, right: m, bottom: m, width: z, height: t, ..default() });
    }

    /// A check box on the slant (`h` cq): an outline in `fg_2`, filled
    /// white with an ink tick when `on`; the label after it (Display
    /// Bold). Returns the row (give it its action).
    pub fn check(&self, c: &mut Commands, parent: Entity, on: bool, label: &str, h: f32) -> Entity {
        let row = self.boxed(c, parent, UiStyle { gap: Some(0.7 * h), align: Some(AlignItems::Center), pad: Some([0.3, 0.6, 0.3, 0.0].into()), ..UiStyle::row() });
        c.entity(row).insert(pressable());
        let bx = self.boxed(c, row, UiStyle { w: Some(h), h: Some(h), align: Some(AlignItems::Center), justify: Some(JustifyContent::Center), shrink: Some(0.0), ..UiStyle::row() });
        let slant = if on { UiSlant::new(Tone::Fg, 1.0) } else { UiSlant::new(Tone::Fg2, 1.0).outline(1.0) };
        c.spawn((
            Node { position_type: PositionType::Absolute, left: Val::ZERO, top: Val::ZERO, right: Val::ZERO, bottom: Val::ZERO, ..default() },
            slant,
            Pickable::IGNORE,
            ChildOf(bx),
        ));
        if on {
            self.kit_icon(c, bx, "tick", 0.7 * h, 1.0, Tone::Ink);
        }
        self.text(c, row, Role::DisplayBold, 0.67 * h, Tone::Fg, label);
        row
    }

    /// A modal's plate `w` cq wide (`Ink2`, an outline, a .22 cq top bar:
    /// white for a question, threat for one that ends something), centred
    /// in `parent`, which becomes the modal layer (`modal::Modal`: the UI
    /// behind it takes no input). Returns the plate.
    pub fn modal(&self, c: &mut Commands, parent: Entity, w: f32, danger: bool) -> Entity {
        self.modal_with(c, parent, w, danger, false)
    }

    /// `modal` that right click does not close (`modal::Blocking`: a
    /// required decision, a first-run step, an unsaved-changes guard).
    pub fn modal_blocking(&self, c: &mut Commands, parent: Entity, w: f32, danger: bool) -> Entity {
        self.modal_with(c, parent, w, danger, true)
    }

    fn modal_with(&self, c: &mut Commands, parent: Entity, w: f32, danger: bool, blocking: bool) -> Entity {
        c.entity(parent).insert(crate::modal::Modal);
        if blocking {
            c.entity(parent).insert(crate::modal::Blocking);
        }
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
/// A search field's glass, cq tall.
pub const SEARCH_H: f32 = 0.8;
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

/// The `mark` icon's width over its height (icons.toml: 1248 x 896).
/// The lockup's nodes (`Ui::lockup_parts`).
#[derive(Clone, Debug)]
pub struct LockupParts {
    pub root: Entity,
    /// The mark (three helmeted profiles).
    pub mark: Entity,
    /// ULTIMATE, one node a letter (spread over the wordmark's width).
    pub ultimate: Vec<Entity>,
    /// GUN GAME in the wordmark role.
    pub wordmark: Entity,
}

pub const MARK_ASPECT: f32 = 1248.0 / 896.0;

/// A toggle's size, cq.
pub const TOGGLE_W: f32 = 4.4;
pub const TOGGLE_H: f32 = 1.5;
/// Its slab's width and its two places (left edge, cq).
pub const TOGGLE_SLAB_W: f32 = 1.9;
pub const TOGGLE_OFF_LEFT: f32 = 0.25;
pub const TOGGLE_ON_LEFT: f32 = 2.25;

/// A toggle flipped at `since` (UI clock), drawn at `now`: its slab
/// slides over (`Ui::toggle_sized`) over `Pace::Normal` of `motion`; scale 0:
/// it jumps.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ToggleSlide {
    pub since: f64,
    pub now: f64,
    pub motion: Motion,
}

/// A toggle's focus strip (`Ui::toggle`).
#[derive(Component, Clone, Copy, Debug)]
pub struct ToggleFocus;

/// A toggle's slab (the moving part).
#[derive(Component, Clone, Copy, Debug)]
pub struct ToggleSlab {
    pub track: Entity,
}

/// A toggle's label box (OFF, then ON), each its half's width.
#[derive(Component, Clone, Copy, Debug)]
pub struct ToggleLabel;

/// How `Ui::edit_chars` draws a text.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Face {
    pub(crate) role: Role,
    pub(crate) size: f32,
    pub(crate) tone: Tone,
    /// A person's own text, in its own case (`style::KeepCase`).
    pub(crate) plain: bool,
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
    const STEPPER: ValueLook = ValueLook { h: STEPPER_H, pad: 0.25, size: 0.7, justify: JustifyContent::Center, rest_line: false };
}

/// What `Ui::slider_ex` built: the track (the drag's) and the value box.
#[derive(Clone, Copy, Debug)]
pub struct SliderParts {
    pub track: Entity,
    pub value: Entity,
}

/// What `Ui::search_field` built: the field and, once something is typed,
/// its clear key.
#[derive(Clone, Copy, Debug)]
pub struct SearchParts {
    pub field: Entity,
    pub clear: Option<Entity>,
}

/// A search field's clear key (`Ui::search_field`).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SearchClear;

/// A stepper's key width and height, cq (the control's height too).
pub const STEPPER_KEY_W: f32 = 1.5;
pub const STEPPER_H: f32 = 1.7;

/// The one border around a stepper's parts (`Ui::stepper_ex`).
#[derive(Component, Clone, Copy, Debug)]
pub struct StepperBorder;

/// What a stepper knows of its setting, to size its value box and dim a
/// key at an end (builder-style: `StepperRange::new().range(1.0, 65535.0)
/// .step(1.0).unit("s").reached(at_min, at_max)`). With nothing set the
/// box fits the value it shows.
#[derive(Clone, Copy, Debug, Default)]
pub struct StepperRange<'a> {
    pub min: Option<f32>,
    pub max: Option<f32>,
    pub step: Option<f32>,
    /// Shown after the number (a unit, a percent sign).
    pub unit: &'a str,
    /// The number is shown times this (a percentage: 100); 0: as is.
    pub scale: f32,
    /// A longest word the value may show (a stepper through labels).
    pub longest: &'a str,
    pub at_min: bool,
    pub at_max: bool,
}

impl<'a> StepperRange<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn range(mut self, min: f32, max: f32) -> Self {
        self.min = Some(min);
        self.max = Some(max);
        self
    }

    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn unit(mut self, unit: &'a str) -> Self {
        self.unit = unit;
        self
    }

    pub fn scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }

    pub fn longest(mut self, word: &'a str) -> Self {
        self.longest = word;
        self
    }

    /// Which ends the value has reached (their key dims).
    pub fn reached(mut self, at_min: bool, at_max: bool) -> Self {
        self.at_min = at_min;
        self.at_max = at_max;
        self
    }

    /// `v` as the box shows it: scaled, as many decimals as the step
    /// has (at most two), then the unit.
    fn text_of(&self, v: f32) -> String {
        let k = if self.scale > 0.0 { self.scale } else { 1.0 };
        let d = self.step.map_or(0, |s| (0..=2).find(|d| ((s * k * 10f32.powi(*d)).round() - s * k * 10f32.powi(*d)).abs() < 1e-3).unwrap_or(2) as usize);
        format!("{:.d$}{}", v * k, self.unit)
    }

    /// The value box's width, cq: room for the longest of the min, the
    /// max (each with the step's decimals and the unit), `longest` and
    /// `value` itself, one Mono character `STEPPER_CHAR_W` cq at the
    /// stepper's text size, a little padding; at least two characters.
    pub fn width(&self, value: &str) -> f32 {
        let mut n = value.chars().count().max(self.longest.chars().count()).max(2);
        for v in [self.min, self.max].into_iter().flatten() {
            n = n.max(self.text_of(v).chars().count());
        }
        n as f32 * STEPPER_CHAR_W + 0.7
    }
}

/// One Mono character of a stepper's value, cq.
const STEPPER_CHAR_W: f32 = 0.46;

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
    mut texts: Query<(&HoverText, &mut UiText)>,
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
                if let Ok((h, mut text)) = texts.get_mut(ch) {
                    let want = if over { h.over } else { h.base };
                    if text.tone != want {
                        text.tone = want;
                    }
                }
            }
        }
    }
}

/// The pointer's states of the last frame (`carry_hover`): each hovered
/// or pressed node's place in its tree (`tree_place`), the node, its hero
/// slab's lift.
#[derive(Resource, Default)]
pub struct HoverCarry {
    last: Vec<(Vec<(Entity, u32)>, Entity, Option<f32>)>,
}

/// A node's place in its tree: each node from the root down to it, with
/// its index under its parent (the root's 0).
fn tree_place(e: Entity, parents: &Query<&ChildOf>, children: &Query<&Children>) -> Vec<(Entity, u32)> {
    let mut place = Vec::new();
    let mut at = e;
    loop {
        let Ok(p) = parents.get(at) else {
            place.push((at, 0));
            break;
        };
        let i = children.get(p.parent()).ok().and_then(|c| c.iter().position(|ch| ch == at)).unwrap_or(0);
        place.push((at, i as u32));
        at = p.parent();
    }
    place.reverse();
    place
}

/// Whether `new` (a node spawned this frame) stands where `old` stood:
/// under the same node that stayed (its deepest ancestor not spawned this
/// frame), at the same indices below the part drawn again (whose own
/// index there may differ: the old part went from the list), or, a whole
/// tree drawn again, at the same indices from its root.
fn same_place(new: &[(Entity, u32)], old: &[(Entity, u32)], born: &impl Fn(Entity) -> bool) -> bool {
    if new.len() != old.len() {
        return false;
    }
    let from = match new[..new.len() - 1].iter().rposition(|(e, _)| !born(*e)) {
        Some(k) if old[k].0 != new[k].0 => return false,
        Some(k) => k + 2,
        None => 1,
    };
    (from..new.len()).all(|j| new[j].1 == old[j].1)
}

/// A screen drawn again keeps the pointer's look (module doc, "States"):
/// a clickable node spawned where last frame's node, despawned since, was
/// hovered or pressed (`same_place`) starts hovered, its `Hover` tint on
/// and its hero slab as far lifted, before the styles and the layout, so
/// the redrawn frame draws as the one before (Bevy's focus would see it
/// only next frame, and the lift would ease in again: a hovered button
/// flashed on every redraw). The next frame's focus corrects it if the
/// pointer isn't on it. Then remembers this frame's.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn carry_hover(
    mut carry: ResMut<HoverCarry>,
    mut nodes: Query<(Entity, &mut Interaction, Option<&Hover>, Option<&mut BackgroundColor>, Option<&Children>)>,
    new: Query<(), Added<ComputedNode>>,
    mut slabs: Query<(&mut HeroSlab, &mut Node, Option<&Children>)>,
    mut glows: Query<&mut UiSlant, With<HeroGlow>>,
    parents: Query<&ChildOf>,
    children: Query<&Children>,
    entities: &bevy::ecs::entity::Entities,
    units: Res<UiUnits>,
) {
    if !carry.last.is_empty() {
        let born_now: Vec<Entity> = nodes.iter_mut().filter(|(_, i, ..)| i.is_added() && **i == Interaction::None).map(|(e, ..)| e).collect();
        let born = |e: Entity| new.contains(e);
        for e in born_now {
            let place = tree_place(e, &parents, &children);
            // Only in place of a node that went (a screen drawn again).
            let Some(&(_, _, lift)) =
                carry.last.iter().find(|(old_place, old, _)| *old != e && !entities.contains_spawned(*old) && same_place(&place, old_place, &born))
            else {
                continue;
            };
            let Ok((_, mut i, hover, bg, kids)) = nodes.get_mut(e) else { continue };
            *i = Interaction::Hovered;
            if let (Some(h), Some(mut bg)) = (hover, bg) {
                bg.0 = h.over;
            }
            let Some(lift) = lift else { continue };
            for ch in kids.into_iter().flat_map(|k| k.iter()) {
                let Ok((mut slab, mut node, glow)) = slabs.get_mut(ch) else { continue };
                slab.lift = lift;
                let k = Pace::Fast.ease().sample(lift);
                node.right = if k > 0.0 { units.val(-0.4 * k) } else { Val::ZERO };
                for g in glow.into_iter().flat_map(|g| g.iter()) {
                    if let Ok(mut g) = glows.get_mut(g) {
                        g.alpha = k;
                    }
                }
            }
        }
    }
    carry.last.clear();
    for (e, i, _, _, kids) in &nodes {
        if *i == Interaction::None {
            continue;
        }
        let lift = kids.into_iter().flat_map(|k| k.iter()).find_map(|ch| slabs.get(ch).ok().map(|(s, ..)| s.lift));
        carry.last.push((tree_place(e, &parents, &children), e, lift));
    }
}

/// The heroes' hover: each slab's lift eases toward its button's hover
/// over `Pace::Fast` (a snap at motion scale 0); the accent fades in and
/// the right edge leans out .4 cq with it.
pub fn animate_heroes(
    clock: Res<UiClock>,
    motion: Option<Res<Motion>>,
    reduced: Option<Res<crate::ReducedMotion>>,
    units: Res<UiUnits>,
    mut last: Local<f64>,
    interactions: Query<&Interaction, With<Hover>>,
    mut slabs: Query<(&mut HeroSlab, &ChildOf, &mut Node, Option<&Children>)>,
    mut glows: Query<&mut UiSlant, With<HeroGlow>>,
) {
    let dt = (clock.now - *last).max(0.0) as f32;
    *last = clock.now;
    let motion = if reduced.is_some_and(|r| r.0) { Motion::new(0.0) } else { motion.map(|m| *m).unwrap_or_default() };
    for (mut slab, parent, mut node, children) in &mut slabs {
        // A disabled hero has no `Hover`: it never lifts.
        let over = interactions.get(parent.parent()).is_ok_and(|i| *i != Interaction::None);
        let target = if over { 1.0 } else { 0.0 };
        let eased = |t: f32| Pace::Fast.ease().sample(t);
        let want = motion.approach(slab.lift, target, dt, Pace::Fast);
        if want == slab.lift && !units.is_changed() {
            continue;
        }
        slab.lift = want;
        let k = eased(want);
        let right = if k > 0.0 { units.val(-0.4 * k) } else { Val::ZERO };
        if node.right != right {
            node.right = right;
        }
        for ch in children.into_iter().flat_map(|c| c.iter()) {
            if let Ok(mut g) = glows.get_mut(ch)
                && g.alpha != k
            {
                g.alpha = k;
            }
        }
    }
}

/// Where a segmented picker's pick last stood (`slide_picks`): per picker
/// (its words and its track's place), the pick's centre from the track's
/// centre and its width, logical px, and when it was seen.
#[derive(Resource, Default)]
pub struct PickMemory {
    seen: std::collections::BTreeMap<(u64, i32, i32), (f32, f32, f64)>,
}

/// A pick already placed (`slide_picks`).
#[derive(Component)]
pub struct PickPlaced;

/// The slide a new pick starts with, from where the picker's last pick
/// stood (`old`: centre and width) to `new`: the shift of its centre
/// (logical px) and its width's scale at the start, or none (nothing
/// before, it hasn't moved, or motion is off).
pub fn pick_slide(old: Option<(f32, f32)>, new: (f32, f32), motion: &Motion) -> Option<(f32, f32)> {
    let (oc, ow) = old?;
    let (c, w) = new;
    if motion.snaps() || w <= 0.0 || ((oc - c).abs() <= 0.5 && (ow - w).abs() <= 0.5) {
        return None;
    }
    Some((oc - c, ow / w))
}

/// A new segmented pick slides in from where the same picker's last one
/// stood (centre and width), over `Pace::Normal` and its easing; at motion
/// scale 0, or with no earlier pick seen, it is just there. Runs after
/// layout, and moves the pick's drawn place too so its first frame is
/// already at the start of the slide.
pub fn slide_picks(
    clock: Res<UiClock>,
    motion: Option<Res<Motion>>,
    reduced: Option<Res<crate::ReducedMotion>>,
    units: Res<UiUnits>,
    mut mem: ResMut<PickMemory>,
    tracks: Query<&UiGlobalTransform, (Without<SegmentedPick>, Without<ToggleSlab>)>,
    mut picks: Query<(Entity, Option<&SegmentedPick>, Option<&ToggleSlab>, &ComputedNode, &mut UiGlobalTransform, Has<PickPlaced>, Option<&Timeline>), Or<(With<SegmentedPick>, With<ToggleSlab>)>>,
    mut commands: Commands,
) {
    let motion = if reduced.is_some_and(|r| r.0) { Motion::new(0.0) } else { motion.map(|m| *m).unwrap_or_default() };
    for (e, pick, slab, node, mut gt, placed, tl) in &mut picks {
        // A segmented pick keys by its words, a toggle's slab by its place.
        let (track, pkey) = match (pick, slab) {
            (Some(p), _) => (p.track, p.key),
            (_, Some(t)) => (t.track, 1),
            _ => continue,
        };
        let Ok(tg) = tracks.get(track) else { continue };
        let (w, k) = (node.size().x * node.inverse_scale_factor, node.inverse_scale_factor);
        if w <= 0.0 {
            continue;
        }
        let key = (pkey, (tg.translation.x / 8.0).round() as i32, (tg.translation.y / 8.0).round() as i32);
        if !placed {
            commands.entity(e).insert(PickPlaced);
            if tl.is_some() {
                // Slid by its builder (`ToggleSlide`).
                continue;
            }
            let centre = (gt.translation.x - tg.translation.x) * k;
            let old = mem.seen.get(&key).filter(|(.., at)| clock.now - at < 0.5).map(|&(c, w, _)| (c, w));
            if let Some((dx_logical, sx)) = pick_slide(old, (centre, w), &motion) {
                let dur = motion.secs(Pace::Normal);
                let ease = Pace::Normal.ease();
                commands.entity(e).insert(
                    Timeline::new([
                        Track::new(Prop::X, dx_logical / units.cq(1.0).max(0.001), 0.0).dur(dur).ease(ease),
                        Track::new(Prop::ScaleX, sx, 1.0).dur(dur).ease(ease),
                    ])
                    .started(clock.now),
                );
                // This frame's drawing already at the start.
                let mut a: bevy::math::Affine2 = (*gt).into();
                a.translation.x += dx_logical / k;
                a.matrix2.x_axis.x *= sx;
                *gt = a.into();
            }
            continue;
        }
        if tl.is_some_and(|t| !t.done(clock.now)) {
            continue;
        }
        mem.seen.insert(key, ((gt.translation.x - tg.translation.x) * k, w, clock.now));
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
        crate::motion::init(app);
        app.init_resource::<PickMemory>()
            .init_resource::<HoverCarry>()
            .add_systems(Update, (show_hover, animate_heroes, crate::form::blink))
            .add_systems(PostUpdate, carry_hover.before(crate::style::resolve_styles).in_set(crate::UiKitSystems))
            .add_systems(PostUpdate, draw_kit_icons.in_set(crate::UiKitSystems))
            .add_systems(
                PostUpdate,
                (resolve_min_heights.after(crate::style::resolve_styles), apply_wipes.after(crate::motion::step_timelines))
                    .in_set(crate::UiKitSystems),
            )
            .add_systems(PostUpdate, slide_picks.after(bevy::ui::UiSystems::Layout).before(bevy::ui::UiSystems::PostLayout));
    }
}

/// The text a role takes at `size` cq (for measuring in tests).
pub fn styled(ui: &Ui, role: Role, size: f32) -> fonts::Styled {
    fonts::text(ui.theme, ui.kit, role, ui.units.text_px(size))
}
