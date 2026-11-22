//! Combat Arms' text (specs/combat_arms/ui_system.md T and R): the font
//! list and its scaling, measuring and wrapping with a font, line
//! spacing, the outline and shadow copies, and the string tables.
//!
//! Fonts are monospaced here: every glyph advances the font's average
//! width, as the spec's test fonts (the real faces' advances are its Open
//! question 3). Our font list (`data/ui/fonts.toml`) names each face's
//! heights and its average width as a fraction of the height.

use std::collections::HashMap;

use bevy::math::IVec2;
use serde::Deserialize;

use crate::client::canvas::{self, Metrics, Scale};

// ---------------------------------------------------------------------------
// Fonts (T1, T2)

/// A font of the list: its name in layouts, size at 1024 x 768 and face.
#[derive(Clone, Debug, PartialEq)]
pub struct Font {
    pub name: String,
    /// Cell height, px at 1024 x 768.
    pub height: i32,
    /// Average glyph width, px at that height.
    pub average: i32,
    /// The face (`FontFace::role`): drawn with its file from the install
    /// or a stand-in.
    pub face: String,
}

/// A font as drawn at a scale: ⌊H_f s_y⌋ tall and ⌊a_f s_x⌋ wide a
/// glyph, or the font itself at (1, 1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Scaled {
    pub height: i32,
    pub advance: i32,
}

impl Font {
    pub fn scaled(&self, s: Scale) -> Scaled {
        if s.is_one() {
            return Scaled {
                height: self.height,
                advance: self.average,
            };
        }
        Scaled {
            height: (self.height as f64 * s.y).floor() as i32,
            advance: (self.average as f64 * s.x).floor() as i32,
        }
    }
}

impl Metrics for Scaled {
    fn advance(&self, _: char) -> i32 {
        self.advance
    }
    fn height(&self) -> i32 {
        self.height
    }
}

impl Scaled {
    /// The gap between wrapped lines: a sixth of the height, floored.
    pub fn line_gap(self) -> i32 {
        self.height.div_euclid(6)
    }

    /// From one wrapped line's top to the next's.
    pub fn line_step(self) -> i32 {
        self.height + self.line_gap()
    }
}

/// A face of the font list (`data/ui/fonts.toml`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct FontFace {
    /// What the face is for (`bold`, `light`, `number`).
    pub role: String,
    /// The names' prefix: "bold" gives "bold 11" ... (the number is the
    /// height).
    pub prefix: String,
    /// The heights listed, px at 1024 x 768 (inclusive range).
    pub heights: [i32; 2],
    /// The average glyph width over the height.
    pub average: f32,
    /// The face's file in the install (`fonts/...`), when it is read
    /// from there.
    #[serde(default)]
    pub file: Option<String>,
    /// Bold weight for the stand-in face.
    #[serde(default)]
    pub bold: bool,
}

#[derive(Debug, Deserialize)]
struct FontsFile {
    face: Vec<FontFace>,
}

/// The font list: fonts by exact name (case-sensitive).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FontList {
    pub faces: Vec<FontFace>,
    pub fonts: HashMap<String, Font>,
}

impl FontList {
    /// Our font list (`data/ui/fonts.toml`).
    pub fn ours() -> Self {
        Self::parse(include_str!("../../../data/ui/fonts.toml")).expect("data/ui/fonts.toml")
    }

    pub fn parse(toml_text: &str) -> Result<Self, String> {
        let file: FontsFile = toml::from_str(toml_text).map_err(|e| e.to_string())?;
        let mut list = Self::default();
        for f in &file.face {
            for h in f.heights[0]..=f.heights[1] {
                list.add(Font {
                    name: format!("{} {h}", f.prefix),
                    height: h,
                    average: (h as f32 * f.average).round().max(1.0) as i32,
                    face: f.role.clone(),
                });
            }
        }
        list.faces = file.face;
        Ok(list)
    }

    pub fn add(&mut self, f: Font) {
        self.fonts.insert(f.name.clone(), f);
    }

    /// A font by its exact name.
    pub fn get(&self, name: &str) -> Option<&Font> {
        self.fonts.get(name)
    }

    pub fn face(&self, role: &str) -> Option<&FontFace> {
        self.faces.iter().find(|f| f.role == role)
    }
}

// ---------------------------------------------------------------------------
// Measuring and wrapping (T3, T5)

/// A text measured with its scaled font: its lines, its size in screen px
/// and in reference px.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    pub font: Scaled,
    pub lines: Vec<String>,
    pub wrapped: bool,
    pub px: IVec2,
    pub reference: IVec2,
}

/// The wrap width in px: ⌊WrapWidth s_x⌋, raised to the scaled font's
/// average width (its height if that is below 1).
pub fn wrap_px(wrap: i32, font: Scaled, s: Scale) -> i32 {
    let w = (wrap as f64 * s.x).floor() as i32;
    let least = if font.advance < 1 { font.height } else { font.advance };
    w.max(least)
}

/// Measure `text` (wrapped when `wrap` > 0, where a two-character `\n`
/// becomes a newline) with `font` at `s`.
pub fn measure(text: &str, font: &Font, wrap: i32, s: Scale) -> Measured {
    let f = font.scaled(s);
    let (lines, px) = if wrap > 0 {
        let text = text.replace("\\n", "\n");
        let lines = canvas::wrap(&text, &f, wrap_px(wrap, f, s));
        let w = lines.iter().map(|l| canvas::measure(l, &f)).max().unwrap_or(0);
        let h = lines.len() as i32 * f.line_step();
        (lines, IVec2::new(w, h))
    } else {
        (vec![text.to_string()], IVec2::new(canvas::measure(text, &f), f.height))
    };
    Measured {
        font: f,
        lines,
        wrapped: wrap > 0,
        px,
        reference: to_reference(px, s),
    }
}

/// Screen px to reference px, truncated.
pub fn to_reference(px: IVec2, s: Scale) -> IVec2 {
    IVec2::new((px.x as f64 / s.x).floor() as i32, (px.y as f64 / s.y).floor() as i32)
}

// ---------------------------------------------------------------------------
// Effects (T6)

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Effect {
    #[default]
    None,
    Shadow,
    Shadow2x,
    Stroke,
    Stroke2x,
}

pub const EFFECTS: &[(&str, Effect)] = &[
    ("DEFAULT", Effect::None),
    ("SHADOW", Effect::Shadow),
    ("SHADOW2X", Effect::Shadow2x),
    ("STROKE", Effect::Stroke),
    ("STROKE2X", Effect::Stroke2x),
];

impl Effect {
    /// The copies drawn under the text in the effect's colour, px offsets
    /// in drawing order: a shadow's one, or a stroke's eight (unwrapped:
    /// one under the text and none up-left).
    pub fn copies(self, wrapped: bool) -> Vec<IVec2> {
        let stroke = |e: i32| {
            let first = if wrapped { IVec2::new(-e, -e) } else { IVec2::ZERO };
            vec![
                first,
                IVec2::new(-e, 0),
                IVec2::new(e, 0),
                IVec2::new(0, -e),
                IVec2::new(0, e),
                IVec2::new(e, -e),
                IVec2::new(-e, e),
                IVec2::new(e, e),
            ]
        };
        match self {
            Effect::None => Vec::new(),
            Effect::Shadow => vec![IVec2::ONE],
            Effect::Shadow2x => vec![IVec2::splat(2)],
            Effect::Stroke => stroke(1),
            Effect::Stroke2x => stroke(2),
        }
    }

    pub fn is_stroke(self) -> bool {
        matches!(self, Effect::Stroke | Effect::Stroke2x)
    }
}

// ---------------------------------------------------------------------------
// String tables (R1, R2)

/// A string table: values by lowercased key, and a key suffix code may
/// set (the shop's rental view).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Strings {
    pub values: HashMap<String, String>,
    pub suffix: Option<String>,
    /// Keys given twice (the first kept).
    pub duplicates: Vec<String>,
}

impl Strings {
    /// Our table (`data/ui/strings.txt`).
    pub fn ours() -> Self {
        Self::parse(include_str!("../../../data/ui/strings.txt"))
    }

    /// A table: lines split on CR and LF; a line without `=` ignored; the
    /// key before `=`, trimmed and lowercased; the value between the first
    /// `"` after `=` and the next one not doubled (`""` is one quote).
    pub fn parse(text: &str) -> Self {
        let mut t = Self::default();
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        for line in text.split(['\r', '\n']) {
            let Some((key, rest)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_lowercase();
            let value = quoted(rest);
            if t.values.contains_key(&key) {
                t.duplicates.push(key);
            } else {
                t.values.insert(key, value);
            }
        }
        t
    }

    /// What `id` shows: empty for an empty id; else its leading `$`s
    /// stripped and lowercased, looked up (with the suffix first when one
    /// is set); not found, the id unchanged.
    pub fn resolve(&self, id: &str) -> String {
        if id.is_empty() {
            return String::new();
        }
        let key = id.trim_start_matches('$').to_lowercase();
        if let Some(sfx) = &self.suffix
            && let Some(v) = self.values.get(&format!("{key}{}", sfx.to_lowercase()))
        {
            return v.clone();
        }
        self.values.get(&key).cloned().unwrap_or_else(|| id.to_string())
    }
}

/// The text between the first `"` and the next one not doubled.
fn quoted(s: &str) -> String {
    let Some(start) = s.find('"') else {
        return String::new();
    };
    let mut out = String::new();
    let mut chars = s[start + 1..].chars().peekable();
    while let Some(c) = chars.next() {
        if c == '"' {
            if chars.peek() == Some(&'"') {
                chars.next();
                out.push('"');
            } else {
                break;
            }
        } else {
            out.push(c);
        }
    }
    out
}
