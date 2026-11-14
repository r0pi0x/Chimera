//! A small style layer: the CSS subset the look book uses, declared in
//! cq and theme tokens and resolved to Bevy UI through `UiUnits`, so a
//! screen is written once, declaratively, and follows the window's size.
//!
//! - `UiStyle` (component): flex row/column, gap, padding and margin
//!   (CSS shorthand: one, two or four values), width/height (and min/max),
//!   absolute offsets, alignment, grow/shrink, all in cq; a background
//!   tone. Built with `style!` (`style!(row, gap: 0.6, pad: [0.7, 0.8],
//!   h: 2.15)`) or the builder (`Style::row().gap(0.6)`).
//! - `UiText` (component): a text role at a size in cq, a tone and its
//!   alpha; resolves the `TextFont`, `LetterSpacing`, `FontHinting` and
//!   `TextColor` from the theme and the kit's faces (`fonts::text`).
//! - `UiSlant` (component): a 12° slanted fill (`SlantMaterial`) in a tone,
//!   which edges lean, an accent bar.
//!
//! `resolve_styles` (in `UiKitSystems`) writes the Bevy components when a
//! style is added or changed and again for every styled node when
//! `UiUnits`, the theme or the faces change. Sizes snap to whole physical
//! pixels: at 16:9 a value is `round(v * width / 100)` physical pixels,
//! the look book's `v cqw` rounded (`styled_px_follow_the_css_formula`).
//!
//! All three are `Default + Clone`, so they work in `bsn!` scenes as
//! template values (`{style!(row, gap: 0.6)}`), beside any other
//! component a screen adds (a parity id, a marker).

use bevy::prelude::*;

use crate::{
    fonts::{self, KitFonts, Role},
    materials::SlantMaterial,
    theme::{Theme, Tone},
    units::UiUnits,
};

/// A box's four sides in cq (CSS order: top, right, bottom, left).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Sides(pub [f32; 4]);

impl From<f32> for Sides {
    fn from(v: f32) -> Self {
        Sides([v; 4])
    }
}

impl From<[f32; 1]> for Sides {
    fn from(v: [f32; 1]) -> Self {
        Sides([v[0]; 4])
    }
}

/// `[vertical, horizontal]`, as CSS's two values.
impl From<[f32; 2]> for Sides {
    fn from([v, h]: [f32; 2]) -> Self {
        Sides([v, h, v, h])
    }
}

/// `[top, horizontal, bottom]`, as CSS's three values.
impl From<[f32; 3]> for Sides {
    fn from([t, h, b]: [f32; 3]) -> Self {
        Sides([t, h, b, h])
    }
}

impl From<[f32; 4]> for Sides {
    fn from(v: [f32; 4]) -> Self {
        Sides(v)
    }
}

// Float literals in `style!(.., pad: [0.7, 0.8])` are f64 until a
// single impl pins them: the same shapes in f64.
impl From<f64> for Sides {
    fn from(v: f64) -> Self {
        Sides::from(v as f32)
    }
}

impl From<[f64; 2]> for Sides {
    fn from([v, h]: [f64; 2]) -> Self {
        Sides::from([v as f32, h as f32])
    }
}

impl From<[f64; 3]> for Sides {
    fn from([t, h, b]: [f64; 3]) -> Self {
        Sides::from([t as f32, h as f32, b as f32])
    }
}

impl From<[f64; 4]> for Sides {
    fn from(v: [f64; 4]) -> Self {
        Sides(v.map(|x| x as f32))
    }
}

impl Sides {
    fn rect(self, u: &UiUnits) -> UiRect {
        let [t, r, b, l] = self.0;
        UiRect::new(u.val(l), u.val(r), u.val(t), u.val(b))
    }
}

/// Flex direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Flow {
    #[default]
    Row,
    Column,
}

/// A node's style in cq (module doc). Unset fields leave Bevy's defaults.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct UiStyle {
    pub flow: Flow,
    pub gap: Option<f32>,
    pub pad: Option<Sides>,
    pub margin: Option<Sides>,
    pub w: Option<f32>,
    pub h: Option<f32>,
    /// Width in percent of the parent.
    pub w_pct: Option<f32>,
    pub h_pct: Option<f32>,
    pub min_w: Option<f32>,
    pub max_w: Option<f32>,
    /// Absolute position: left, top, right, bottom in cq (any of them).
    pub abs: Option<[Option<f32>; 4]>,
    pub align: Option<AlignItems>,
    pub justify: Option<JustifyContent>,
    pub grow: Option<f32>,
    pub shrink: Option<f32>,
    /// A background tone and its alpha (display space).
    pub bg: Option<(Tone, f32)>,
    /// A 1-physical-pixel inset outline in a tone and alpha.
    pub outline: Option<(Tone, f32)>,
}

/// The builder's name for `UiStyle`.
pub type Style = UiStyle;

impl UiStyle {
    pub fn row() -> Self {
        UiStyle { flow: Flow::Row, align: Some(AlignItems::Center), ..default() }
    }

    pub fn column() -> Self {
        UiStyle { flow: Flow::Column, ..default() }
    }

    pub fn gap(mut self, cq: f32) -> Self {
        self.gap = Some(cq);
        self
    }

    pub fn pad(mut self, s: impl Into<Sides>) -> Self {
        self.pad = Some(s.into());
        self
    }

    pub fn margin(mut self, s: impl Into<Sides>) -> Self {
        self.margin = Some(s.into());
        self
    }

    pub fn w(mut self, cq: f32) -> Self {
        self.w = Some(cq);
        self
    }

    pub fn h(mut self, cq: f32) -> Self {
        self.h = Some(cq);
        self
    }

    pub fn w_pct(mut self, p: f32) -> Self {
        self.w_pct = Some(p);
        self
    }

    pub fn h_pct(mut self, p: f32) -> Self {
        self.h_pct = Some(p);
        self
    }

    pub fn min_w(mut self, cq: f32) -> Self {
        self.min_w = Some(cq);
        self
    }

    pub fn max_w(mut self, cq: f32) -> Self {
        self.max_w = Some(cq);
        self
    }

    /// Absolutely placed: left, top, right, bottom (cq; `None` unset).
    pub fn abs(mut self, ltrb: [Option<f32>; 4]) -> Self {
        self.abs = Some(ltrb);
        self
    }

    pub fn align(mut self, a: AlignItems) -> Self {
        self.align = Some(a);
        self
    }

    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify = Some(j);
        self
    }

    pub fn grow(mut self, g: f32) -> Self {
        self.grow = Some(g);
        self
    }

    pub fn shrink(mut self, s: f32) -> Self {
        self.shrink = Some(s);
        self
    }

    pub fn bg(mut self, t: Tone) -> Self {
        self.bg = Some((t, 1.0));
        self
    }

    pub fn bg_a(mut self, t: Tone, a: f32) -> Self {
        self.bg = Some((t, a));
        self
    }

    pub fn outline(mut self, t: Tone, a: f32) -> Self {
        self.outline = Some((t, a));
        self
    }

    /// The Bevy `Node` at these units.
    pub fn node(&self, u: &UiUnits) -> Node {
        let mut n = Node {
            flex_direction: match self.flow {
                Flow::Row => FlexDirection::Row,
                Flow::Column => FlexDirection::Column,
            },
            ..default()
        };
        if let Some(g) = self.gap {
            n.column_gap = u.val(g);
            n.row_gap = u.val(g);
        }
        if let Some(p) = self.pad {
            n.padding = p.rect(u);
        }
        if let Some(m) = self.margin {
            n.margin = m.rect(u);
        }
        if let Some(w) = self.w {
            n.width = u.val(w);
        }
        if let Some(h) = self.h {
            n.height = u.val(h);
        }
        if let Some(p) = self.w_pct {
            n.width = percent(p);
        }
        if let Some(p) = self.h_pct {
            n.height = percent(p);
        }
        if let Some(w) = self.min_w {
            n.min_width = u.val(w);
        }
        if let Some(w) = self.max_w {
            n.max_width = u.val(w);
        }
        if let Some([l, t, r, b]) = self.abs {
            n.position_type = PositionType::Absolute;
            let v = |x: Option<f32>| x.map_or(Val::Auto, |x| u.val(x));
            (n.left, n.top, n.right, n.bottom) = (v(l), v(t), v(r), v(b));
        }
        if let Some(a) = self.align {
            n.align_items = a;
        }
        if let Some(j) = self.justify {
            n.justify_content = j;
        }
        if let Some(g) = self.grow {
            n.flex_grow = g;
        }
        if let Some(s) = self.shrink {
            n.flex_shrink = s;
        }
        n
    }
}

/// A style declared as CSS-like keys (module doc): `style!(row, gap: 0.6,
/// pad: [0.7, 0.8], h: 2.15, bg: Tone::Plate)`. The first word is `row`
/// or `column`; each `key: value` calls the builder method of that name.
#[macro_export]
macro_rules! style {
    (row $(, $k:ident : $v:expr)* $(,)?) => {
        $crate::style::UiStyle::row()$(.$k($v))*
    };
    (column $(, $k:ident : $v:expr)* $(,)?) => {
        $crate::style::UiStyle::column()$(.$k($v))*
    };
}

/// A text role at a size (module doc).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct UiText {
    pub role: Role,
    /// Size in cq (floored by `UiUnits::text_px`).
    pub size: f32,
    pub tone: Tone,
    /// Alpha (display space).
    pub alpha: f32,
}

impl Default for UiText {
    fn default() -> Self {
        UiText { role: Role::Mono, size: 0.62, tone: Tone::Fg, alpha: 1.0 }
    }
}

impl UiText {
    pub fn new(role: Role, size: f32, tone: Tone) -> Self {
        UiText { role, size, tone, alpha: 1.0 }
    }
}

/// A slanted fill (module doc).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct UiSlant {
    pub fill: Tone,
    pub alpha: f32,
    /// Left and right edges lean.
    pub edges: (bool, bool),
    /// An accent bar at the left in this tone, cq wide.
    pub accent: Option<(Tone, f32)>,
    /// Drawn as an outline this many physical px wide (0: filled).
    pub outline: f32,
}

impl Default for UiSlant {
    fn default() -> Self {
        UiSlant { fill: Tone::Fg, alpha: 1.0, edges: (true, true), accent: None, outline: 0.0 }
    }
}

impl UiSlant {
    /// `fill` at `alpha`, both edges leaning.
    pub fn new(fill: Tone, alpha: f32) -> Self {
        UiSlant { fill, alpha, ..default() }
    }

    /// Which edges lean.
    pub fn edges(mut self, left: bool, right: bool) -> Self {
        self.edges = (left, right);
        self
    }

    /// Only its outline, `px` physical pixels wide.
    pub fn outline(mut self, px: f32) -> Self {
        self.outline = px;
        self
    }
}

/// Writes styled nodes' Bevy components (module doc).
#[allow(clippy::type_complexity)]
pub fn resolve_styles(
    units: Res<UiUnits>,
    theme: Res<Theme>,
    kit: Option<Res<KitFonts>>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    styles: Query<(Entity, Ref<UiStyle>)>,
    texts: Query<(Entity, Ref<UiText>)>,
    slanted: Query<(Entity, Ref<UiSlant>, Option<&MaterialNode<SlantMaterial>>)>,
    mut commands: Commands,
) {
    let all = units.is_changed() || theme.is_changed() || kit.as_ref().is_some_and(|k| k.is_changed());
    for (e, s) in &styles {
        if !(all || s.is_changed()) {
            continue;
        }
        let mut c = commands.entity(e);
        c.insert(s.node(&units));
        if let Some((t, a)) = s.bg {
            c.insert(BackgroundColor(theme.tone_a(t, a)));
        }
        if let Some((t, a)) = s.outline {
            c.insert(Outline::new(Val::Px(units.hairline()), Val::Px(-units.hairline()), theme.tone_a(t, a)));
        }
    }
    for (e, t) in &texts {
        if !(all || t.is_changed()) {
            continue;
        }
        let s = fonts::text(&theme, kit.as_deref(), t.role, units.text_px(t.size));
        commands.entity(e).insert((s.bundle(), TextColor(theme.tone_a(t.tone, t.alpha))));
    }
    if let Some(slants) = slants.as_mut() {
        for (e, s, mat) in &slanted {
            if !(all || s.is_changed()) {
                continue;
            }
            let mut m = SlantMaterial::new(&theme, theme.tone_a(s.fill, s.alpha)).edges(s.edges.0, s.edges.1);
            if let Some((t, w)) = s.accent {
                m = m.accent(theme.tone(t), units.phys(units.cq(w)));
            }
            if s.outline > 0.0 {
                m = m.outline(s.outline);
            }
            if let Some(mut existing) = mat.and_then(|h| slants.get_mut(&h.0)) {
                *existing = m;
                continue;
            }
            let handle = slants.add(m);
            commands.entity(e).insert(MaterialNode(handle));
        }
    }
}
