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
//!   Text in a dark tone (`Tone::is_dark`: ink on a white or accent fill)
//!   is drawn one physical pixel wider (`InkWeight`: a shadow of its own
//!   colour one pixel to the right). Bevy blends a glyph's edges in linear
//!   light, which thickens light text on dark and thins dark text on
//!   light: small Mono in ink on white showed as mid grey (its darkest
//!   pixel 60 % of the way from white to ink), so selected segments and
//!   the primary buttons read faint beside the light labels.
//! - `UiSlant` (component): a 12° slanted fill (`SlantMaterial`) in a tone,
//!   which edges lean, an accent bar.
//!
//! `case_texts` upper-cases a `UiText` whose role does (`TypeSpec::upper`)
//! whenever its text changes, unless it is marked `KeepCase`.
//!
//! `resolve_styles` (in `UiKitSystems`) writes the Bevy components when a
//! style is added or changed and again for every styled node when
//! `UiUnits`, the theme or the faces change. A style merges into the
//! node's `Node`: it writes only the fields it names, so layout set in
//! code beside it (`overflow: clip`, `display`) stays; when a style
//! changes, fields only its old value named go back to Bevy's defaults
//! (`AppliedStyle` keeps the value last written). Sizes snap to whole physical
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

    /// The Bevy `Node` at these units (the style's fields over Bevy's
    /// defaults).
    pub fn node(&self, u: &UiUnits) -> Node {
        let mut n = Node::default();
        self.apply(&mut n, u);
        n
    }

    /// Writes the fields this style names into `n`, leaving every other
    /// field as it is (an `overflow`, a `display` set in code).
    pub fn apply(&self, n: &mut Node, u: &UiUnits) {
        n.flex_direction = match self.flow {
            Flow::Row => FlexDirection::Row,
            Flow::Column => FlexDirection::Column,
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
    }

    /// Returns the fields this style names to Bevy's defaults (a changed
    /// style forgets what its previous value set; `resolve_styles`).
    fn clear(&self, n: &mut Node) {
        let d = Node::default();
        n.flex_direction = d.flex_direction;
        if self.gap.is_some() {
            (n.column_gap, n.row_gap) = (d.column_gap, d.row_gap);
        }
        if self.pad.is_some() {
            n.padding = d.padding;
        }
        if self.margin.is_some() {
            n.margin = d.margin;
        }
        if self.w.is_some() || self.w_pct.is_some() {
            n.width = d.width;
        }
        if self.h.is_some() || self.h_pct.is_some() {
            n.height = d.height;
        }
        if self.min_w.is_some() {
            n.min_width = d.min_width;
        }
        if self.max_w.is_some() {
            n.max_width = d.max_width;
        }
        if self.abs.is_some() {
            n.position_type = d.position_type;
            (n.left, n.top, n.right, n.bottom) = (d.left, d.top, d.right, d.bottom);
        }
        if self.align.is_some() {
            n.align_items = d.align_items;
        }
        if self.justify.is_some() {
            n.justify_content = d.justify_content;
        }
        if self.grow.is_some() {
            n.flex_grow = d.flex_grow;
        }
        if self.shrink.is_some() {
            n.flex_shrink = d.flex_shrink;
        }
    }
}

/// The style `resolve_styles` last wrote into a node, so a changed style
/// returns the fields only its old value named to their defaults.
#[derive(Component, Clone, Debug, Default)]
pub struct AppliedStyle(UiStyle);

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

/// Marks the shadow `resolve_styles` gives dark text (module doc), so it
/// goes again when the text's tone turns light.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct InkWeight;

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
    /// Clipped this share (0-1 of the row) from the left and from the
    /// right, the cuts leaning with the slant (a progress bar's fill).
    pub clip: (f32, f32),
    /// Only slanted stripes of the fill: (period in cq, lit share 0-1,
    /// offset along as a share of the period); `UiSlant::hatched`.
    pub hatch: Option<(f32, f32, f32)>,
}

impl Default for UiSlant {
    fn default() -> Self {
        UiSlant { fill: Tone::Fg, alpha: 1.0, edges: (true, true), accent: None, outline: 0.0, clip: (0.0, 0.0), hatch: None }
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

    /// Only slanted stripes `period` cq apart, `lit` of each drawn, the
    /// pattern `phase` (a share of the period) along to the right.
    pub fn hatched(mut self, period: f32, lit: f32, phase: f32) -> Self {
        self.hatch = Some((period, lit, phase));
        self
    }

    /// Cut `left` and `right` (0-1 of the row) along the slant.
    pub fn clipped(mut self, left: f32, right: f32) -> Self {
        self.clip = (left, right);
        self
    }
}

/// A styled text that keeps its case although its role upper-cases
/// (`Ui::text_plain`: a cvar name in Mono, a server's name).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct KeepCase;

/// A styled text whose role upper-cases (`TypeSpec::upper`) shows in
/// capitals however its text was set (a screen that writes its `Text`
/// later need not upper-case it): the faces have lower case since v0.5,
/// and titles, labels and buttons stay as they were. `KeepCase` opts out.
#[allow(clippy::type_complexity)]
pub fn case_texts(
    theme: Res<Theme>,
    mut texts: Query<(&UiText, &mut Text), (Without<KeepCase>, Or<(Changed<Text>, Changed<UiText>)>)>,
) {
    for (t, mut text) in &mut texts {
        if theme.type_spec(t.role.key()).is_some_and(|s| s.upper) && text.0.chars().any(char::is_lowercase) {
            text.0 = text.0.to_uppercase();
        }
    }
}

/// Writes styled nodes' Bevy components (module doc).
#[allow(clippy::type_complexity)]
pub fn resolve_styles(
    units: Res<UiUnits>,
    theme: Res<Theme>,
    kit: Option<Res<KitFonts>>,
    mut slants: Option<ResMut<Assets<SlantMaterial>>>,
    mut styles: Query<(Entity, Ref<UiStyle>, Option<&mut Node>, Option<&mut AppliedStyle>)>,
    texts: Query<(Entity, Ref<UiText>, Has<InkWeight>)>,
    slanted: Query<(Entity, Ref<UiSlant>, Option<&MaterialNode<SlantMaterial>>)>,
    mut commands: Commands,
) {
    let all = units.is_changed() || theme.is_changed() || kit.as_ref().is_some_and(|k| k.is_changed());
    for (e, s, node, applied) in &mut styles {
        if !(all || s.is_changed()) {
            continue;
        }
        let mut c = commands.entity(e);
        // Merge into the node's own fields: a style sets only what it names
        // (an `overflow: clip` set in code stays).
        match node {
            Some(mut n) => {
                if let Some(old) = applied.as_deref().filter(|a| a.0 != *s) {
                    old.0.clear(&mut n);
                }
                s.apply(&mut n, &units);
            }
            None => {
                c.insert(s.node(&units));
            }
        }
        match applied {
            Some(mut a) if a.0 != *s => a.0 = UiStyle::clone(&s),
            Some(_) => {}
            None => {
                c.insert(AppliedStyle(UiStyle::clone(&s)));
            }
        }
        if let Some((t, a)) = s.bg {
            c.insert(BackgroundColor(theme.tone_a(t, a)));
        }
        if let Some((t, a)) = s.outline {
            c.insert(Outline::new(Val::Px(units.hairline()), Val::Px(-units.hairline()), theme.tone_a(t, a)));
        }
    }
    for (e, t, weighted) in &texts {
        if !(all || t.is_changed()) {
            continue;
        }
        let s = fonts::text(&theme, kit.as_deref(), t.role, units.text_px(t.size));
        let bundle = s.bundle();
        let color = theme.tone_a(t.tone, t.alpha);
        let mut c = commands.entity(e);
        c.insert((bundle, TextColor(color)));
        if t.tone.is_dark() {
            c.insert((InkWeight, TextShadow { offset: Vec2::new(units.hairline(), 0.0), color }));
        } else if weighted {
            c.remove::<(InkWeight, TextShadow)>();
        }
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
            if let Some((period, lit, phase)) = s.hatch {
                let px = units.phys(units.cq(period)).max(1.0);
                m = m.hatch(px, lit, phase.rem_euclid(1.0) * px);
            }
            if s.clip != (0.0, 0.0) {
                m = m.clip(s.clip.0, s.clip.1).slanted_cut();
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
