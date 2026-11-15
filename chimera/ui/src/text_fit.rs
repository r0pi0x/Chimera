//! Text cut to a width with an ellipsis (docs/plans/active/ugg-ui.md,
//! "Names"): a player's name (at most 20 characters, the server's cap)
//! or a map's in a Signal widget never runs out of its plate.
//!
//! - `ellipsize(text, max, width)`: `text` as it is when it fits in
//!   `max`, else its longest start that fits with `…` after it (trailing
//!   spaces dropped), by any width function; `ellipsize_with` another
//!   mark: a kit face without the `…` glyph (the UGG faces v0.4) ends in
//!   `...` (`DOTS`).
//! - `KitFonts::width` measures a text in a `TextFont` and its
//!   `LetterSpacing` from the kit's own faces' bytes (the glyphs'
//!   advances and the tracking; a face that isn't the kit's is guessed at
//!   0.62 em a character, wide on purpose). Logical px.
//! - `Ellipsize` on a `Text` entity: its whole text (`full`) and where it
//!   must fit: `max` cq, and/or (`in_parent`) the space its parent leaves
//!   it (the parent's content width less its other children and the gaps,
//!   from the last layout), for a cell whose width doesn't follow the text
//!   (a fixed column, the one growing column of a row). `fit_texts` (the
//!   kit's systems, before Bevy's layout) writes the fitted text when the
//!   text, the font, the units or that space change. A small margin
//!   (`MARGIN_EM` of the size, at least 1 px) covers the shaper's own
//!   rounding.
//!
//! Known shortcuts: the measure adds the glyphs' advances (skrifa), not
//! the shaper Bevy draws with (Parley, with the faces' kerning: the UGG
//! faces' W W pair is some 4 % wider); where the drawn text then spills,
//! `fit_texts` cuts again from Bevy's own layout of it a frame later
//! (`TextLayoutInfo`), so a long name may show one frame cropped.

use bevy::{
    platform::collections::{HashMap, HashSet},
    prelude::*,
    text::{FontSize, FontSource, LetterSpacing, TextLayoutInfo},
    ui::ComputedNode,
};

use skrifa::{MetadataProvider, instance::{LocationRef, Size}};

use crate::{KitFonts, UiUnits};

/// The ellipsis.
pub const ELLIPSIS: &str = "\u{2026}";

/// The margin kept free at the end, in em of the text's size.
pub const MARGIN_EM: f32 = 0.08;

/// A face that isn't the kit's: this many em a character.
pub const GUESS_EM: f32 = 0.62;

/// What ends a cut text in a face without the ellipsis (the UGG faces).
pub const DOTS: &str = "...";

/// `text` fitted to `max` by `width` (module doc).
pub fn ellipsize(text: &str, max: f32, width: impl Fn(&str) -> f32) -> String {
    ellipsize_with(text, max, width, ELLIPSIS)
}

/// `ellipsize` with `mark` for the ellipsis.
pub fn ellipsize_with(text: &str, max: f32, width: impl Fn(&str) -> f32, mark: &str) -> String {
    if width(text) <= max {
        return text.to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    let mut best = String::new();
    // The longest start that fits with the ellipsis (widths grow with
    // the characters taken: a binary search).
    let (mut lo, mut hi) = (0usize, chars.len());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let head: String = chars[..mid].iter().collect();
        let cut = format!("{}{mark}", head.trim_end());
        if width(&cut) <= max {
            lo = mid;
            best = cut;
        } else {
            hi = mid - 1;
        }
    }
    if best.is_empty() { mark.to_string() } else { best }
}

impl KitFonts {
    /// `text`'s width in `font` with `spacing` between letters, logical px
    /// (module doc).
    pub fn width(&self, font: &TextFont, spacing: Option<&LetterSpacing>, text: &str) -> f32 {
        let em = match font.font_size {
            FontSize::Px(px) => px,
            _ => 16.0,
        };
        let track = match spacing {
            Some(LetterSpacing::Px(px)) => *px,
            Some(LetterSpacing::Rem(r)) => r * 16.0,
            _ => 0.0,
        };
        let n = text.chars().count() as f32;
        let bytes = match &font.font {
            FontSource::Handle(h) => self.bytes_of(h.id()),
            _ => None,
        };
        let Some(face) = bytes.and_then(|b| skrifa::FontRef::from_index(b, 0).ok()) else {
            return n * (em * GUESS_EM + track);
        };
        let map = face.charmap();
        let metrics = face.glyph_metrics(Size::new(em), LocationRef::default());
        // Each glyph's advance to the next whole pixel: the drawn text's
        // pen moves on whole pixels (hinted faces), so a sum of exact
        // advances comes out short.
        text.chars()
            .map(|c| map.map(c).and_then(|g| metrics.advance_width(g)).unwrap_or(em * GUESS_EM) + track)
            .sum()
    }

    /// Whether the face of `font` draws `c` (the kit's faces only).
    pub fn has_glyph(&self, font: &TextFont, c: char) -> bool {
        let FontSource::Handle(h) = &font.font else { return false };
        self.bytes_of(h.id())
            .and_then(|b| skrifa::FontRef::from_index(b, 0).ok())
            .and_then(|f| f.charmap().map(c))
            .is_some_and(|g| g.to_u32() != 0)
    }
}

/// A text fitted to its place (module doc).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Ellipsize {
    /// The whole text (in its case).
    pub full: String,
    /// At most this wide, cq.
    pub max: Option<f32>,
    /// At most the space its parent leaves it.
    pub in_parent: bool,
}

impl Ellipsize {
    /// At most `max` cq wide.
    pub fn max(full: impl Into<String>, max: f32) -> Self {
        Ellipsize { full: full.into(), max: Some(max), in_parent: false }
    }

    /// In the space its parent leaves it.
    pub fn in_parent(full: impl Into<String>) -> Self {
        Ellipsize { full: full.into(), max: None, in_parent: true }
    }
}

/// The width (logical px) an `Ellipsize`d text may take: its `max`, and
/// what its parent leaves it at the last layout (None before one).
fn room(
    e: Entity,
    fit: &Ellipsize,
    units: &UiUnits,
    parent: Option<&ChildOf>,
    nodes: &Query<(&ComputedNode, Option<&Node>)>,
    children: &Query<&Children>,
) -> Option<f32> {
    let mut room = fit.max.map(|m| units.cq(m));
    if fit.in_parent {
        let p = parent?.parent();
        let (pn, pnode) = nodes.get(p).ok()?;
        let k = pn.inverse_scale_factor();
        if pn.size().x <= 0.0 {
            return room;
        }
        let mut free = pn.content_box().width() * k;
        let gap = match pnode.map(|n| n.column_gap) {
            Some(Val::Px(g)) => g,
            _ => 0.0,
        };
        let row = pnode.is_none_or(|n| matches!(n.flex_direction, FlexDirection::Row | FlexDirection::RowReverse));
        if row && let Ok(kids) = children.get(p) {
            let mut shown = 0;
            for c in kids.iter() {
                let Ok((cn, cnode)) = nodes.get(c) else { continue };
                if cnode.is_some_and(|n| n.position_type == PositionType::Absolute || n.display == Display::None) {
                    continue;
                }
                shown += 1;
                if c != e {
                    free -= cn.size().x * k;
                }
            }
            free -= gap * (shown.max(1) - 1) as f32;
        }
        room = Some(room.map_or(free, |r| r.min(free)));
    }
    room
}

/// FNV-1a of `s` (the same on every run).
fn fnv(s: &str) -> u64 {
    s.bytes().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ b as u64).wrapping_mul(0x0100_0000_01b3))
}

/// What `fit_texts` last did for a text: for which text, font and room
/// (`key`), the room it took off after seeing the drawn text spill
/// (`extra`, logical px), what it showed.
pub struct Done {
    key: (u64, u32, u32),
    extra: f32,
    shown: String,
}

/// `Ellipsize`d texts fitted (module doc). The measure (glyph advances)
/// misses the shaper's kerning, which widens some pairs (the UGG faces'
/// W W): when the text Bevy laid out last frame spills its room, the fit
/// takes the overshoot off and cuts again (a frame later), until it fits.
#[allow(clippy::type_complexity)]
pub fn fit_texts(
    units: Res<UiUnits>,
    kit: Option<Res<KitFonts>>,
    mut texts: Query<(Entity, &Ellipsize, &TextFont, Option<&LetterSpacing>, &mut Text, Option<&ChildOf>, Option<&TextLayoutInfo>)>,
    nodes: Query<(&ComputedNode, Option<&Node>)>,
    children: Query<&Children>,
    mut done: Local<HashMap<Entity, Done>>,
) {
    if units.is_changed() || kit.as_ref().is_some_and(|k| k.is_changed()) {
        done.clear();
    }
    let none = KitFonts::default();
    let kit = kit.as_deref().unwrap_or(&none);
    let mut seen = HashSet::new();
    for (e, fit, font, spacing, mut text, parent, laid) in &mut texts {
        seen.insert(e);
        let Some(room) = room(e, fit, &units, parent, &nodes, &children) else {
            if text.0 != fit.full {
                text.0 = fit.full.clone();
            }
            continue;
        };
        let size = match font.font_size {
            FontSize::Px(px) => px,
            _ => 16.0,
        };
        let key = (fnv(&format!("{}\u{0}{:?}{:?}", fit.full, font.font, spacing)), room.to_bits(), size.to_bits());
        let margin = (size * MARGIN_EM).max(1.0);
        let mut extra = 0.0;
        if let Some(d) = done.get(&e)
            && d.key == key
        {
            // The text Bevy laid out last (it is the one shown): within its
            // room, nothing to do; spilling, take the overshoot off.
            let drawn = laid.filter(|_| text.0 == d.shown).map(|l| l.size.x / l.scale_factor.max(1e-3));
            match drawn {
                Some(w) if w > room - margin * 0.5 && text.0 != ELLIPSIS && text.0 != DOTS => extra = d.extra + (w - (room - margin)).max(0.5),
                _ => continue,
            }
        }
        // The kit's faces without the ellipsis glyph end in three dots.
        let mark = match &font.font {
            FontSource::Handle(h) if kit.bytes_of(h.id()).is_some() && !kit.has_glyph(font, '\u{2026}') => DOTS,
            _ => ELLIPSIS,
        };
        let fitted = ellipsize_with(&fit.full, room - margin - extra, |t| kit.width(font, spacing, t), mark);
        if text.0 != fitted {
            text.0 = fitted.clone();
        }
        done.insert(e, Done { key, extra, shown: fitted });
    }
    done.retain(|e, _| seen.contains(e));
}
