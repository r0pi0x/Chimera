//! UI parity ids (docs/OBSERVABILITY.md, "UI parity against the look
//! book"): `UiParityId` names a node the way the look book's mockup names
//! its element (`data-ui="board.row.1.name"`), so a tool can measure both
//! and compare them (`tools/ui-parity/`).
//!
//! `ParityRect::of` reads a laid-out node: its box in physical pixels
//! (`ComputedNode::size` around `UiGlobalTransform`'s translation, the
//! node's centre) and, for a text, its font size, letter spacing and
//! colour. The shell's `ui_dump` writes them as JSON.
//!
//! Ids are plain strings, dot-separated, rows counted from 1; a screen's
//! ids start with its name (`board.`, `chip.`, `feed.`). Only the screens
//! the parity check covers carry them.

use bevy::{
    prelude::*,
    text::{FontSize, LetterSpacing, TextColor, TextFont},
    ui::{ComputedNode, UiGlobalTransform},
};

/// A node's name in the look book (module doc).
#[derive(Component, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UiParityId(pub String);

impl UiParityId {
    pub fn new(id: impl Into<String>) -> Self {
        UiParityId(id.into())
    }
}

/// Names `e` (a node just spawned under `p`) for the parity check; nothing
/// when `e` is `None`.
pub fn name(p: &mut ChildSpawnerCommands, e: impl Into<Option<Entity>>, id: impl Into<String>) {
    if let Some(e) = e.into() {
        p.commands().entity(e).insert(UiParityId::new(id));
    }
}

/// A text's look: size and spacing in physical px, colour as sRGB 0-255
/// and alpha 0-1.
#[derive(Clone, Debug, PartialEq)]
pub struct ParityText {
    pub text: String,
    pub font_size: f32,
    pub letter_spacing: f32,
    pub weight: u16,
    pub color: [f32; 4],
}

/// A named node as laid out (module doc), in physical px from the
/// window's top left.
#[derive(Clone, Debug, PartialEq)]
pub struct ParityRect {
    pub id: String,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub visible: bool,
    pub text: Option<ParityText>,
}

impl ParityRect {
    /// `node` laid out at `at`, with its text when it has one;
    /// `scale_factor` turns logical text sizes into physical.
    pub fn of(
        id: &UiParityId,
        node: &ComputedNode,
        at: &UiGlobalTransform,
        visible: bool,
        text: Option<(&str, &TextFont, Option<&LetterSpacing>, Option<&TextColor>)>,
        scale_factor: f32,
    ) -> ParityRect {
        let (_, _, centre) = at.to_scale_angle_translation();
        let size = node.size;
        let text = text.map(|(s, font, spacing, color)| {
            let px = match font.font_size {
                FontSize::Px(v) => v,
                other => other.eval(Vec2::ZERO, bevy::text::RemSize(16.0)),
            };
            let ls = match spacing {
                Some(LetterSpacing::Px(v)) => *v,
                _ => 0.0,
            };
            let c = color.map_or(Color::WHITE, |c| c.0).to_srgba();
            ParityText {
                text: s.to_string(),
                font_size: px * scale_factor,
                letter_spacing: ls * scale_factor,
                weight: font.weight.0,
                color: [c.red * 255.0, c.green * 255.0, c.blue * 255.0, c.alpha],
            }
        });
        ParityRect {
            id: id.0.clone(),
            x: centre.x - size.x / 2.0,
            y: centre.y - size.y / 2.0,
            w: size.x,
            h: size.y,
            visible,
            text,
        }
    }
}

/// What a named node's query reads (`collect`).
pub type ParityQueryData = (
    &'static UiParityId,
    &'static ComputedNode,
    &'static UiGlobalTransform,
    Option<&'static InheritedVisibility>,
    Option<(&'static Text, &'static TextFont, Option<&'static LetterSpacing>, Option<&'static TextColor>)>,
);

/// Every named node in `nodes`, laid out (after `PostUpdate`'s UI
/// layout), sorted by id; `scale_factor` the window's.
pub fn collect(nodes: &Query<ParityQueryData>, scale_factor: f32) -> Vec<ParityRect> {
    let mut out: Vec<ParityRect> = nodes
        .iter()
        .map(|(id, node, at, vis, text)| {
            let text = text.map(|(t, f, l, c)| (t.0.as_str(), f, l, c));
            ParityRect::of(id, node, at, vis.is_none_or(|v| v.get()), text, scale_factor)
        })
        .collect();
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}
