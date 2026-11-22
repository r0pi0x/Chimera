//! Values hidden by default (the user's review of the first run,
//! 2026-11-21: streamers): addresses (a hub's, a server's), the recovery
//! key. Shown masked, with an eye key after them that reveals the value
//! (like a password field) and hides it again. Masked again each time a
//! screen opens: a reveal is kept by key in `Reveals` only while a node
//! showing that key is drawn (`forget_reveals`), so a screen drawn anew
//! starts hidden. Copying never needs the reveal (callers copy the real
//! value).
//!
//! - `Ui::secret`: the value (masked or not) and its eye key, in a row.
//! - `Ui::reveal_key`: the eye key alone (beside a text field the caller
//!   draws masked: `text_field`'s `masked`).
//! - `mask`: a fixed run of dots (an address: nothing of its shape shown);
//!   `mask_chars`: one dot a letter or digit, its separators kept (a key's
//!   groups).
//! - `looks_like_address`: an IP address, with or without a port, or a
//!   `host:port` (for names that fall back to an address);
//!   `name_or_masked`, `mask_addresses` (a sentence's addresses).
//!
//! A screen puts `Reveals` in what it compares to redraw, so a click on an
//! eye redraws it.

use std::collections::BTreeSet;

use bevy::{
    prelude::*,
    ui::DeprecatedInteraction as Interaction,
};

use crate::{
    controls::{Hover, Ui},
    fonts::Role,
    style::UiStyle,
    theme::Tone,
};

/// The mask's dot.
pub const DOT: char = '•';
/// How many dots `mask` shows.
pub const MASK_LEN: usize = 10;

/// The keys revealed now (module doc).
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct Reveals(pub BTreeSet<String>);

impl Reveals {
    pub fn shown(&self, key: &str) -> bool {
        self.0.contains(key)
    }

    pub fn toggle(&mut self, key: &str) {
        if !self.0.remove(key) {
            self.0.insert(key.to_string());
        }
    }
}

/// An eye key: a click reveals or hides `0`.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct RevealKey(pub String);

/// A node showing the secret `0` (masked or not).
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct Secret(pub String);

/// A fixed run of dots: nothing of the value's shape.
pub fn mask(_value: &str) -> String {
    std::iter::repeat_n(DOT, MASK_LEN).collect()
}

/// One dot a letter or digit, the separators kept (a key's groups).
pub fn mask_chars(value: &str) -> String {
    value.chars().map(|c| if c.is_alphanumeric() { DOT } else { c }).collect()
}

/// `value` as shown: itself when revealed, else `mask`.
pub fn shown(value: &str, revealed: bool) -> String {
    if revealed { value.to_string() } else { mask(value) }
}

/// An IP address (v4 or v6, a port or not), or a `host:port`.
pub fn looks_like_address(s: &str) -> bool {
    let s = s.trim();
    if s.parse::<std::net::IpAddr>().is_ok() || s.parse::<std::net::SocketAddr>().is_ok() {
        return true;
    }
    match s.rsplit_once(':') {
        Some((host, port)) => (host.contains('.') || host == "localhost") && !host.contains(' ') && port.parse::<u16>().is_ok(),
        None => false,
    }
}

/// `text` with an address masked when it is one (a name falling back to
/// an address), else as it is.
pub fn name_or_masked(text: &str, revealed: bool) -> String {
    if !revealed && looks_like_address(text) { mask(text) } else { text.to_string() }
}

/// `text` with each word that is an address (`looks_like_address`, its
/// trailing punctuation aside) masked: a status line naming addresses.
pub fn mask_addresses(text: &str) -> String {
    text.split(' ')
        .map(|w| {
            let core = w.trim_end_matches(|c: char| matches!(c, ',' | ';' | '.' | ')' | ':'));
            let core = core.trim_start_matches('(');
            if !core.is_empty() && looks_like_address(core) { w.replacen(core, &mask(core), 1) } else { w.to_string() }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

impl Ui<'_> {
    /// The eye key for `key`, `h` cq square: the eye while hidden, the
    /// struck eye while shown. Clickable (`reveal_clicks`).
    pub fn reveal_key(&self, c: &mut Commands, parent: Entity, key: &str, revealed: bool, h: f32) -> Entity {
        let (base, over) = (Color::NONE, self.color(Tone::Fg, 0.08));
        let k = c
            .spawn((
                UiStyle { w: Some(h), h: Some(h), justify: Some(JustifyContent::Center), align: Some(AlignItems::Center), shrink: Some(0.0), ..UiStyle::row() },
                Node::default(),
                BackgroundColor(base),
                Hover { base, over },
                crate::controls::pressable(),
                RevealKey(key.to_string()),
                Secret(key.to_string()),
                ChildOf(parent),
            ))
            .id();
        self.kit_icon(c, k, if revealed { "eye_off" } else { "eye" }, h * 0.6, 1.0, if revealed { Tone::AccText } else { Tone::Fg2 });
        k
    }

    /// `value` in `role` at `size`, masked unless `revealed` (`keep`: one
    /// dot a character, separators kept; else a fixed run), then its eye
    /// key. Returns the row; the text is its first child.
    #[allow(clippy::too_many_arguments)]
    pub fn secret(&self, c: &mut Commands, parent: Entity, key: &str, value: &str, revealed: bool, keep: bool, role: Role, size: f32, tone: Tone) -> Entity {
        let row = self.boxed(c, parent, UiStyle { gap: Some(0.5), align: Some(AlignItems::Center), ..UiStyle::row() });
        c.entity(row).insert(Secret(key.to_string()));
        let text = if revealed {
            value.to_string()
        } else if keep {
            mask_chars(value)
        } else {
            mask(value)
        };
        self.text_plain(c, row, role, size, tone, &text);
        self.reveal_key(c, row, key, revealed, (size * 1.6).max(1.3));
        row
    }
}

/// A click on an eye key reveals or hides its value.
pub fn reveal_clicks(keys: Query<(&Interaction, &RevealKey), Changed<Interaction>>, mut reveals: ResMut<Reveals>) {
    for (i, k) in &keys {
        if *i == Interaction::Pressed {
            reveals.toggle(&k.0);
        }
    }
}

/// A reveal no drawn node shows is forgotten: the next screen showing it
/// starts hidden (module doc).
pub fn forget_reveals(nodes: Query<&Secret>, mut reveals: ResMut<Reveals>) {
    if reveals.0.is_empty() {
        return;
    }
    let drawn: BTreeSet<&str> = nodes.iter().map(|s| s.0.as_str()).collect();
    if reveals.0.iter().any(|k| !drawn.contains(k.as_str())) {
        reveals.0.retain(|k| drawn.contains(k.as_str()));
    }
}

pub(crate) fn add(app: &mut App) {
    app.init_resource::<Reveals>().add_systems(Update, reveal_clicks).add_systems(PostUpdate, forget_reveals);
}
