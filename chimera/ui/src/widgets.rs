//! Small builders on Bevy UI from the theme (docs/plans/active/ugg-ui.md,
//! 2.1): `tag` (YOU, HOST, BOT, RUNG: the look book's tag styles; round V
//! may restyle them) and `kbd` (a key cap). Each returns the bundle of a
//! node and its text child's bundle; the caller spawns them
//! (`spawn_pair`).
//!
//! Known shortcuts: HOST's inset bar is a left border, not an inset
//! shadow.

use bevy::prelude::*;

use crate::{
    fonts::{self, KitFonts, Role},
    theme::{Theme, Tone},
    units::UiUnits,
};

/// A tag's look.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TagKind {
    /// You: light fill, ink text.
    You,
    /// The host: a faint fill with a bar at the left.
    Host,
    /// A bot: outlined.
    Bot,
    /// Accent fill (a rung, the leader).
    Accent,
}

/// Text size of tags, cq (the board's; the lobby's are .62).
pub const TAG_CQ: f32 = 0.5;

/// A tag's box and its text (module doc).
pub fn tag(
    theme: &Theme,
    kit: Option<&KitFonts>,
    units: &UiUnits,
    kind: TagKind,
    text: &str,
    size_cq: f32,
) -> (impl Bundle + use<>, impl Bundle + use<>) {
    let s = fonts::text(theme, kit, Role::Mono, units.text_px(size_cq));
    let k = size_cq / 0.62;
    let (bg, fg, border, border_color) = match kind {
        TagKind::You => (theme.tone(Tone::Fg), theme.tone(Tone::Ink), UiRect::ZERO, Color::NONE),
        TagKind::Host => (
            theme.tone(Tone::Fg4),
            theme.tone(Tone::Fg),
            UiRect::left(Val::Px(units.snap(units.cq(0.2 * k)).max(units.hairline()))),
            theme.tone(Tone::Fg2),
        ),
        TagKind::Bot => (Color::NONE, theme.tone(Tone::Fg2), UiRect::all(Val::Px(units.hairline())), theme.tone(Tone::Fg3)),
        TagKind::Accent => (theme.tone(Tone::Acc), theme.tone(Tone::OnAcc), UiRect::ZERO, Color::NONE),
    };
    let node = (
        Node {
            padding: UiRect::new(units.val(0.4 * k), units.val(0.4 * k), units.val(0.26 * k), units.val(0.2 * k)),
            border,
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(border_color),
    );
    // Tags track wider than Mono's text (.1 em).
    let spacing = bevy::text::LetterSpacing::Px(0.1 * units.text_px(size_cq));
    let label = (Text::new(s.case(text).to_uppercase()), s.font.clone(), spacing, s.hinting, TextColor(fg));
    (node, label)
}

/// A key cap: outlined, the key in Mono.
pub fn kbd(
    theme: &Theme,
    kit: Option<&KitFonts>,
    units: &UiUnits,
    key: &str,
    size_cq: f32,
) -> (impl Bundle + use<>, impl Bundle + use<>) {
    let s = fonts::text(theme, kit, Role::Mono, units.text_px(size_cq));
    let node = (
        Node {
            padding: UiRect::new(units.val(0.42), units.val(0.42), units.val(0.28), units.val(0.22)),
            border: UiRect::all(Val::Px(units.hairline())),
            ..default()
        },
        BorderColor::all(theme.tone(Tone::Fg3)),
    );
    let label = (Text::new(s.case(key).to_uppercase()), s.bundle(), TextColor(theme.tone(Tone::Fg)));
    (node, label)
}

/// Spawns a (box, text) pair as a parent and child; returns the box.
pub fn spawn_pair(commands: &mut Commands, (node, label): (impl Bundle, impl Bundle)) -> Entity {
    let e = commands.spawn(node).id();
    let t = commands.spawn(label).id();
    commands.entity(e).add_child(t);
    e
}
