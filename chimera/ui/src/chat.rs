//! The chat's look (docs/plans/active/ugg-ui.md, M10 "Chat"): what a
//! line of the one chat feed is (`FeedLine`, its `LineKind`), how long it
//! stays (`line_alpha`: held `HOLD` s, then faded over `FADE` s), its
//! time stamp in the history (`stamp`), a player's name colour
//! (`name_color`) and the builders the shell's chat draws with
//! (`Ui::chat_line`, `Ui::chat_plate`, `Ui::chat_field`). No game, no
//! network: `chimera_client::client::signal_chat` feeds and places it.
//!
//! - **Lines**: a player's line is the name (in the player's colour, the
//!   BOT mark or the HOST tag beside it, TEAM or RADIO before it when it
//!   is one), then the message in the sentence face
//!   (`SENTENCE`, the people's own case kept). A notice (a join, a
//!   leave, a rename, a vote, "X is on the last level!") is no player's
//!   line: a slanted accent mark and the sentence in `fg_2`. A system
//!   line (the server's own word, a refusal) has a SERVER tag in Mono and
//!   the sentence in `fg_3`.
//! - **Closed**, each line sits on its own straight ink backing; **open**
//!   (the history), the lines and the field sit on one plate whose right
//!   edge leans 12° (`chat_plate`: the user's "slanted Signal plate"; its
//!   left edge stays straight so text keeps an even margin), each line
//!   with its stamp in Mono.
//! - **The field**: the text field's look (fill .08, the accent underline,
//!   the caret in accent, the selection on an accent tint), led by the
//!   channel's key cap (ALL or TEAM), in the sentence face with the case
//!   kept; the count of characters left in Mono once under 20.
//!
//! Everything a person wrote, names included, keeps its own case in the
//! text face (`SENTENCE`, `Role::Text`); tags and stamps are Mono.

use bevy::prelude::*;

use crate::{
    controls::{Face, TextCursor, Ui, char_of},
    fonts::{self, Role},
    style::UiSlant,
    theme::Tone,
};

/// The face sentences and typed text are set in: the text face, with its
/// lower case (`Role::Text`), at `controls::TYPED` in the feed.
pub const SENTENCE: Role = Role::Text;

/// Seconds a line stays fully shown, then fades over `FADE`.
pub const HOLD: f32 = 8.0;
pub const FADE: f32 = 1.0;
/// Most characters (bytes) a line takes: Source's say limit, as the
/// server's (`chimera_net::net::chat::MAX_SAY`).
pub const MAX_CHARS: usize = 127;
/// The field shows how many are left from this many on.
pub const COUNT_FROM: usize = 20;

/// What a line is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LineKind {
    /// A player's line to everyone.
    All,
    /// A player's line to the team.
    Team,
    /// A radio call (a game's radio menus).
    Radio,
    /// Something that happened: a join, a leave, a rename, a vote, a
    /// player on the last level.
    Notice,
    /// The server's own word (a dedicated server's console, a refusal).
    System,
}

impl LineKind {
    /// A player's line (a name leads it).
    pub fn player(self) -> bool {
        matches!(self, LineKind::All | LineKind::Team | LineKind::Radio)
    }
}

/// A sender's role tag beside the name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NameTag {
    Bot,
    Host,
}

/// A line as drawn.
#[derive(Clone, Debug, PartialEq)]
pub struct FeedLine<'a> {
    pub kind: LineKind,
    pub name: Option<&'a str>,
    pub name_color: Color,
    pub tag: Option<NameTag>,
    pub text: &'a str,
    /// The history's time stamp (`stamp`); None closed.
    pub stamp: Option<String>,
    /// The sender is muted here (the history shows the name struck dim).
    pub muted: bool,
}

/// A line's opacity `age` seconds after it came (1 held for `hold`, then
/// linearly to 0 over `fade`).
pub fn line_alpha(age: f32, hold: f32, fade: f32) -> f32 {
    if age <= hold {
        1.0
    } else if fade <= 0.0 {
        0.0
    } else {
        (1.0 - (age - hold) / fade).clamp(0.0, 1.0)
    }
}

/// A time stamp, `secs` into the session: "m:ss", "h:mm:ss" past an hour.
pub fn stamp(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    let (h, m, s) = (s / 3600, (s / 60) % 60, s % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// The names' colours (ours, light enough on ink): a player keeps one by
/// their name.
pub const NAME_COLORS: [[u8; 3]; 8] = [
    [125, 200, 255],
    [255, 170, 120],
    [150, 230, 160],
    [240, 150, 220],
    [250, 220, 120],
    [170, 160, 255],
    [120, 225, 215],
    [255, 140, 150],
];

/// `name`'s colour (`NAME_COLORS`, by a hash of the name: the same on
/// every client).
pub fn name_color(name: &str) -> Color {
    // FNV-1a: stable across runs and machines.
    let mut h: u32 = 0x811c_9dc5;
    for b in name.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    let [r, g, b] = NAME_COLORS[(h as usize) % NAME_COLORS.len()];
    Color::srgb_u8(r, g, b)
}

/// A line's root (tests find the lines by kind).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatLineNode(pub LineKind);

/// A notice's slanted mark.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct NoticeMark;

/// A line's sender name (a click in the history mutes or unmutes them).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct ChatName(pub String);

/// The chat's text field.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChatField;

impl Ui<'_> {
    fn font(&self, role: Role, size: f32) -> (TextFont, bevy::text::LetterSpacing, bevy::text::FontHinting, bevy::text::LineHeight) {
        fonts::text(self.theme, self.kit, role, self.units.text_px(size)).bundle()
    }

    /// Text in `role` at `size` cq in `color`, its case kept, on one line.
    fn span(&self, c: &mut Commands, parent: Entity, role: Role, size: f32, color: Color, text: &str) -> Entity {
        c.spawn((
            Text::new(text),
            self.font(role, size),
            TextColor(color),
            TextLayout::new(Justify::Left, LineBreak::NoWrap),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// A small Mono tag (TEAM, RADIO, SERVER, BOT, HOST) in `tone`.
    fn chat_tag(&self, c: &mut Commands, parent: Entity, text: &str, fill: Option<Tone>, tone: Tone) -> Entity {
        let t = c
            .spawn((
                Node {
                    padding: UiRect::axes(self.units.val(0.22), self.units.val(0.06)),
                    flex_shrink: 0.0,
                    align_items: AlignItems::Center,
                    ..default()
                },
                BackgroundColor(fill.map_or(Color::NONE, |f| self.theme.tone(f))),
                Outline::new(Val::Px(if fill.is_none() { self.units.hairline() } else { 0.0 }), Val::ZERO, self.theme.tone(Tone::Fg4)),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        self.span(c, t, Role::Mono, 0.46, self.theme.tone(tone), text);
        t
    }

    /// One line of the feed (module doc) under `parent`, `size` cq text;
    /// `backed`: on its own ink backing (the closed feed). Its node.
    pub fn chat_line(&self, c: &mut Commands, parent: Entity, line: &FeedLine, size: f32, backed: bool) -> Entity {
        let row = c
            .spawn((
                ChatLineNode(line.kind),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: self.units.val(0.42),
                    row_gap: self.units.val(0.2),
                    padding: UiRect::axes(self.units.val(0.55), self.units.val(0.22)),
                    max_width: Val::Percent(100.0),
                    align_self: AlignSelf::FlexStart,
                    ..default()
                },
                BackgroundColor(if backed { self.theme.tone_a(Tone::Ink, 0.72) } else { Color::NONE }),
                Pickable::IGNORE,
                ChildOf(parent),
            ))
            .id();
        if let Some(s) = &line.stamp {
            self.span(c, row, Role::Mono, 0.5, self.theme.tone(Tone::Fg3), s);
        }
        match line.kind {
            LineKind::Notice => {
                let h = self.units.cq(size * 0.9);
                c.spawn((
                    NoticeMark,
                    Node { width: Val::Px(self.units.snap(h * 0.55)), height: Val::Px(self.units.snap(h)), flex_shrink: 0.0, ..default() },
                    UiSlant::new(Tone::Acc, 1.0),
                    Pickable::IGNORE,
                    ChildOf(row),
                ));
                self.span(c, row, SENTENCE, size, self.theme.tone(Tone::Fg2), line.text);
            }
            LineKind::System => {
                self.chat_tag(c, row, "SERVER", None, Tone::Fg2);
                self.span(c, row, SENTENCE, size, self.theme.tone(Tone::Fg2), line.text);
            }
            kind => {
                match kind {
                    LineKind::Team => {
                        self.chat_tag(c, row, "TEAM", Some(Tone::Acc), Tone::OnAcc);
                    }
                    LineKind::Radio => {
                        self.chat_tag(c, row, "RADIO", None, Tone::Fg2);
                    }
                    _ => {}
                }
                if let Some(name) = line.name {
                    let n = c
                        .spawn((
                            Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, column_gap: self.units.val(0.3), flex_shrink: 0.0, ..default() },
                            ChatName(name.to_string()),
                            ChildOf(row),
                        ))
                        .id();
                    let color = if line.muted { line.name_color.with_alpha(0.4) } else { line.name_color };
                    self.span(c, n, SENTENCE, size, color, &format!("{name}:"));
                    match line.tag {
                        Some(NameTag::Bot) => {
                            self.chat_tag(c, n, "BOT", None, Tone::Fg2);
                        }
                        Some(NameTag::Host) => {
                            self.chat_tag(c, n, "HOST", Some(Tone::Acc), Tone::OnAcc);
                        }
                        None => {}
                    }
                }
                let text = c
                    .spawn((
                        Text::new(line.text),
                        self.font(SENTENCE, size),
                        TextColor(self.theme.tone(Tone::Fg)),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                        Node { flex_shrink: 1.0, ..default() },
                        Pickable::IGNORE,
                        ChildOf(row),
                    ))
                    .id();
                let _ = text;
            }
        }
        row
    }

    /// The history's plate under `parent`: ink .82, the right edge leaning
    /// 12°, the left straight (module doc). Absolutely placed to fill its
    /// parent; its node (put it first, the lines over it).
    pub fn chat_plate(&self, c: &mut Commands, parent: Entity) -> Entity {
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                top: Val::ZERO,
                bottom: Val::ZERO,
                right: self.units.val(-1.2),
                ..default()
            },
            UiSlant::new(Tone::Ink, 0.82).edges(false, true),
            Pickable::IGNORE,
            ChildOf(parent),
        ))
        .id()
    }

    /// The chat's field under `parent` (module doc): the channel's cap
    /// (`team`), `text` with the caret where `cursor` says (None: not
    /// typing: `placeholder`), the characters left near the limit.
    #[allow(clippy::too_many_arguments)]
    pub fn chat_field(
        &self,
        c: &mut Commands,
        parent: Entity,
        team: bool,
        text: &str,
        cursor: Option<TextCursor>,
        placeholder: &str,
        size: f32,
    ) -> Entity {
        let typing = cursor.is_some();
        let f = c
            .spawn((
                ChatField,
                bevy::ui::RelativeCursorPosition::default(),
                crate::text_menu::TextField::of(text, cursor, false),
                Node {
                    flex_direction: FlexDirection::Row,
                    align_items: AlignItems::Center,
                    column_gap: self.units.val(0.5),
                    min_height: self.units.val(1.9),
                    padding: UiRect::axes(self.units.val(0.6), self.units.val(0.2)),
                    flex_shrink: 0.0,
                    ..default()
                },
                BackgroundColor(self.theme.tone_a(Tone::Fg, if typing { 0.08 } else { 0.04 })),
                ChildOf(parent),
            ))
            .id();
        // The underline: accent while typing, a hairline at rest.
        c.spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::ZERO,
                right: Val::ZERO,
                bottom: Val::ZERO,
                height: if typing { self.units.val(0.2) } else { Val::Px(self.units.hairline()) },
                ..default()
            },
            BackgroundColor(if typing { self.theme.tone(Tone::Acc) } else { self.theme.tone(Tone::Fg3) }),
            Pickable::IGNORE,
            ChildOf(f),
        ));
        self.kbd(c, f, if team { "TEAM" } else { "ALL" }, 0.54, Tone::Fg2);
        let body = c
            .spawn((Node { flex_direction: FlexDirection::Row, align_items: AlignItems::Center, flex_grow: 1.0, ..default() }, Pickable::IGNORE, ChildOf(f)))
            .id();
        match cursor {
            None => {
                self.span(c, body, SENTENCE, size, self.theme.tone(Tone::Fg3), placeholder);
            }
            Some(k) => {
                // One node a character and the caret an overlay, as the
                // kit's text field: typing moves nothing already there.
                let face = Face { role: Role::Text, size, tone: Tone::Fg, plain: true };
                self.edit_chars(c, body, text, face, Some((char_of(text, k.at), char_of(text, k.anchor))));
            }
        }
        let left = MAX_CHARS.saturating_sub(text.len());
        if typing && left < COUNT_FROM {
            self.span(c, f, Role::Mono, 0.5, self.theme.tone(if left == 0 { Tone::Threat } else { Tone::Fg3 }), &left.to_string());
        }
        f
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_hold_then_fade() {
        assert_eq!(line_alpha(0.0, HOLD, FADE), 1.0);
        assert_eq!(line_alpha(HOLD, HOLD, FADE), 1.0);
        assert!((line_alpha(HOLD + FADE / 2.0, HOLD, FADE) - 0.5).abs() < 1e-5);
        assert_eq!(line_alpha(HOLD + FADE, HOLD, FADE), 0.0);
        assert_eq!(line_alpha(100.0, HOLD, FADE), 0.0);
        assert_eq!(line_alpha(HOLD + 0.1, HOLD, 0.0), 0.0);
    }

    #[test]
    fn stamps_read_as_clock_time() {
        assert_eq!(stamp(0.0), "0:00");
        assert_eq!(stamp(65.4), "1:05");
        assert_eq!(stamp(3725.0), "1:02:05");
    }

    #[test]
    fn a_name_keeps_its_colour() {
        assert_eq!(name_color("kestrel"), name_color("kestrel"));
        let distinct: std::collections::HashSet<[u8; 4]> =
            ["a", "b", "c", "d", "e", "f", "g", "h", "i", "j"].iter().map(|n| name_color(n).to_srgba().to_u8_array()).collect();
        assert!(distinct.len() > 3, "{distinct:?}");
    }
}
