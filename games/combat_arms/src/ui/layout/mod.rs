//! Combat Arms' layouts (specs/combat_arms/ui_system.md L, S, T4, W):
//! trees of widgets on Chimera's reference canvas
//! (`crate::client::canvas`), read from `CBALAYOUT` files
//! (`xml`), placed and drawn by the game's rules:
//!
//! - L: a file hangs under an invisible 1024 x 1024 root; a child's
//!   position is relative to its parent; attributes by widget type
//!   (`Kind`), values by `values` (`.` separates numbers).
//! - S: a tree's screen scale (HUD panels: x_r, y_r; menus and popups:
//!   1); images and windows as float quads (`Quad`) covering whole pixels
//!   by the half-pixel rule, buttons with floored sizes, texts at their
//!   truncated reference position; hit rects in screen px (`hit_rect`;
//!   texts' unscaled, a quirk).
//! - T4: a text aligned in its parent's box, its own position ignored on
//!   an aligned axis; wrapped lines aligned one by one.
//! - W: images' UVs (the top-left w x h texels), nine-slice borders,
//!   buttons' state textures (A normal, B hover, C pressed, D disabled,
//!   missing ones falling back to A; no A: 0 x 0), list rows, gauges.
//!
//! `Layout::draw` gives what a frame draws (`Draw`) in screen px, which
//! `ui::draw` turns into UI nodes and the tests check against the spec's
//! rects. Textures are by name through `Textures` (their sizes; the
//! game's art is in keyed archives, so `ui::draw` paints stand-ins).

pub mod text;
pub mod values;
pub mod xml;

use bevy::math::{IVec2, Vec2};

use crate::client::canvas::{self, HAlign, Node, Scale, Tree, VAlign, WidgetId};
pub use text::{Effect, Font, FontList, Measured, Scaled, Strings};
pub use values::Rgba;

/// The reference canvas, ref px.
pub const REFERENCE: Vec2 = Vec2::new(1024.0, 768.0);
/// The invisible root every file hangs from (1024 tall too: a quirk).
pub const ROOT_SIZE: IVec2 = IVec2::new(1024, 1024);
/// The placeholder texture that means "no background" on a window.
pub const BLANK: &str = "blank";

/// Whether `texture` is the placeholder blank (ours, `blank`, or the
/// game's `UI_SCR_Common\blank.dtx`): a window without a background.
pub fn is_blank(texture: &str) -> bool {
    let t = texture.replace('\\', "/").to_lowercase();
    let file = t.rsplit('/').next().unwrap_or("");
    file == BLANK || file == "blank.dtx"
}

/// The interface sounds layouts name (I3), by their values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Sound {
    #[default]
    None,
    NavigationChange,
    NavigationSelect,
    ItemSelect,
    ShopItemEquip,
    ShopItemUnequip,
    SilencerEquip,
    SilencerUnequip,
    ScopeEquip,
    ScopeUnequip,
    MagazineEquip,
    MagazineUnequip,
    Change,
    Select,
    Page,
}

pub const SOUNDS: &[(&str, Sound)] = &[
    ("IS_NONE", Sound::None),
    ("IS_NAVIGATION_CHANGE", Sound::NavigationChange),
    ("IS_NAVIGATION_SELECT", Sound::NavigationSelect),
    ("IS_ITEM_SELECT", Sound::ItemSelect),
    ("IS_SHOPITEM_EQUIP", Sound::ShopItemEquip),
    ("IS_SHOPITEM_UNEQUIP", Sound::ShopItemUnequip),
    ("IS_SILENCER_EQUIP", Sound::SilencerEquip),
    ("IS_SILENCER_UNEQUIP", Sound::SilencerUnequip),
    ("IS_SCOPE_EQUIP", Sound::ScopeEquip),
    ("IS_SCOPE_UNEQUIP", Sound::ScopeUnequip),
    ("IS_MAGAZIN_EQUIP", Sound::MagazineEquip),
    ("IS_MAGAZIN_UNEQUIP", Sound::MagazineUnequip),
    ("IS_CHANGE", Sound::Change),
    ("IS_SELECT", Sound::Select),
    ("IS_PAGE", Sound::Page),
];

impl Sound {
    /// Its value in the game's tables.
    pub fn value(self) -> u8 {
        match self {
            Sound::None => 0,
            Sound::NavigationChange => 1,
            Sound::NavigationSelect => 2,
            Sound::ItemSelect => 3,
            Sound::ShopItemEquip => 4,
            Sound::ShopItemUnequip => 5,
            Sound::SilencerEquip => 6,
            Sound::SilencerUnequip => 7,
            Sound::ScopeEquip => 8,
            Sound::ScopeUnequip => 9,
            Sound::MagazineEquip => 10,
            Sound::MagazineUnequip => 11,
            Sound::Change => 19,
            Sound::Select => 20,
            Sound::Page => 21,
        }
    }
}

pub const H_ALIGNS: &[(&str, HAlign)] = &[
    ("DEFAULT", HAlign::Default),
    ("LEFT", HAlign::Left),
    ("RIGHT", HAlign::Right),
    ("HCENTER", HAlign::Centre),
];

pub const V_ALIGNS: &[(&str, VAlign)] = &[
    ("DEFAULT", VAlign::Default),
    ("TOP", VAlign::Top),
    ("BOTTOM", VAlign::Bottom),
    ("VCENTER", VAlign::Centre),
];

/// An image's texturing (Image, SimpleStretchImage, a window's
/// background).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageLook {
    pub texture: String,
    /// EnableAlpha's AlphaValue.
    pub alpha: Option<u8>,
    /// EnableColor's TextureColor (multiplies the texture).
    pub colour: Option<Rgba>,
    /// CustomUV when all four are 0 or more.
    pub uv: Option<[f32; 4]>,
    /// SimpleStretchValue (u, v, uw, vh): the nine-slice split.
    pub stretch: Option<[f32; 4]>,
}

/// A text's settings (Text, ButtonTypeText).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextLook {
    pub font: String,
    /// A string id or literal text (looked up either way).
    pub element: String,
    pub h: HAlign,
    pub v: VAlign,
    pub effect: Effect,
    /// Normalcolor with TextAlphaColor.
    pub colour: Rgba,
    pub stroke: Rgba,
    pub shadow: Rgba,
    /// Ref px; 0: no wrapping.
    pub wrap: i32,
    /// ButtonTypeText's Overcolor, Clickcolor, Disablecolor (opaque).
    pub states: Option<[Rgba; 3]>,
    /// Set by code, replacing the element's string.
    pub set: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ButtonLook {
    /// ButtonTexture: the state textures' prefix.
    pub prefix: String,
    pub over: Sound,
    pub click: Sound,
    pub stretch: Option<[f32; 4]>,
    /// CheckButton: Some(true) for CHECK, Some(false) for RADIO.
    pub check: Option<bool>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct GaugeLook {
    pub frame: String,
    pub bar: String,
    pub bar_at: IVec2,
    pub max_range: i32,
    /// The fill, percent.
    pub fill: f64,
    pub bar_height: i32,
    /// The frame's and the bar's nine-slice values
    /// (`GaugeBackTextureSimpleStretchValue`,
    /// `GaugeBarTextureSimpleStretchValue`; W2, W5).
    pub frame_stretch: Option<[f32; 4]>,
    pub bar_stretch: Option<[f32; 4]>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ListLook {
    pub gap: i32,
    pub blank: i32,
    pub back: String,
    pub auto_size: bool,
}

/// A widget's type and its own attributes (L4).
#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Root,
    Window(ImageLook),
    Image(ImageLook),
    Button(ButtonLook),
    Text(TextLook),
    Gauge(GaugeLook),
    List(ListLook),
    Column(ListLook),
    /// The types whose behaviour isn't built yet (scroll bars, combo
    /// boxes, sliders, edit boxes): placed and hit, not drawn.
    Other(String),
}

/// A widget: its kind and the attributes every widget has (L3), with its
/// state at run time.
#[derive(Clone, Debug, PartialEq)]
pub struct Widget {
    pub kind: Kind,
    pub cmd: i32,
    pub texture_alpha_size: IVec2,
    pub scroller: Option<String>,
    /// Run time: enabled (a disabled widget is hit but doesn't accept).
    pub enabled: bool,
    pub hover: bool,
    pub held: bool,
    /// Code pinned the pressed look.
    pub selected: bool,
}

impl Widget {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            cmd: 0,
            texture_alpha_size: IVec2::ZERO,
            scroller: None,
            enabled: true,
            hover: false,
            held: false,
            selected: false,
        }
    }

    pub fn text(&self) -> Option<&TextLook> {
        match &self.kind {
            Kind::Text(t) => Some(t),
            _ => None,
        }
    }
}

/// Texture sizes by name, texels (None: missing).
pub trait Textures {
    fn size(&self, name: &str) -> Option<IVec2>;
}

impl Textures for std::collections::HashMap<String, IVec2> {
    fn size(&self, name: &str) -> Option<IVec2> {
        self.get(name).copied()
    }
}

/// A button's state texture suffix (W3): D disabled, C pressed (hovered
/// and held, or pinned), B hover, else A.
pub fn button_state(w: &Widget) -> char {
    if !w.enabled {
        'D'
    } else if w.selected || (w.hover && w.held) {
        'C'
    } else if w.hover {
        'B'
    } else {
        'A'
    }
}

/// The state texture to draw (`prefix` + state, falling back to A), None
/// when A is missing.
pub fn button_texture(prefix: &str, state: char, textures: &impl Textures) -> Option<String> {
    let a = format!("{prefix}A");
    textures.size(&a)?;
    let s = format!("{prefix}{state}");
    Some(if textures.size(&s).is_some() { s } else { a })
}

/// Pixel coverage of a span drawn half a pixel up-left: [⌈X − ½⌉, ⌈X + W
/// − ½⌉).
pub fn cover(x: f64, w: f64) -> (i32, i32) {
    ((x - 0.5).ceil() as i32, (x + w - 0.5).ceil() as i32)
}

/// A rect [x0, x1) x [y0, y1) in screen px.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PxRect {
    pub x0: i32,
    pub x1: i32,
    pub y0: i32,
    pub y1: i32,
}

impl PxRect {
    pub fn new(x0: i32, x1: i32, y0: i32, y1: i32) -> Self {
        Self { x0, x1, y0, y1 }
    }
    pub fn contains(self, p: IVec2) -> bool {
        p.x >= self.x0 && p.x < self.x1 && p.y >= self.y0 && p.y < self.y1
    }
    pub fn size(self) -> IVec2 {
        IVec2::new(self.x1 - self.x0, self.y1 - self.y0)
    }
}

/// A textured quad: nominal (float, screen px) and the pixels it covers.
#[derive(Clone, Debug, PartialEq)]
pub struct Quad {
    pub at: [f64; 2],
    pub size: [f64; 2],
    pub cover: PxRect,
    pub texture: String,
    /// u0, v0, u1, v1.
    pub uv: [f32; 4],
    pub colour: Rgba,
    /// Nine-slice: the column and row edges, px.
    pub slices: Option<([f64; 4], [f64; 4])>,
}

impl Quad {
    fn new(at: [f64; 2], size: [f64; 2], texture: String, uv: [f32; 4], colour: Rgba) -> Self {
        let (x0, x1) = cover(at[0], size[0]);
        let (y0, y1) = cover(at[1], size[1]);
        Self {
            at,
            size,
            cover: PxRect::new(x0, x1, y0, y1),
            texture,
            uv,
            colour,
            slices: None,
        }
    }
}

/// A text as drawn: each line's top-left in screen px, its font and
/// colours, the effect's copies (offsets) under it.
#[derive(Clone, Debug, PartialEq)]
pub struct TextDraw {
    pub widget: WidgetId,
    pub lines: Vec<(IVec2, String)>,
    pub font: Scaled,
    pub face: String,
    pub colour: Rgba,
    pub effect: Effect,
    pub effect_colour: Rgba,
    pub copies: Vec<IVec2>,
    /// Its horizontal alignment (a stand-in face whose glyphs differ from
    /// the metrics is drawn aligned the same way inside the measured box).
    pub h: HAlign,
    /// Where the text's reference box went (aligned) and its size.
    pub reference_at: IVec2,
    pub reference_size: IVec2,
    /// The string image's rect in screen px.
    pub rect: PxRect,
    /// The glyphs' horizontal stretch over the face's own proportions at
    /// the scaled height: ⌊a_f x_r⌋ over a_f y_r (T2), 1 at (1, 1).
    pub stretch: f32,
}

/// The horizontal stretch of a font's glyphs drawn at `scaled` (T2): the
/// forced average width over the width the face has at the scaled height.
pub fn glyph_stretch(base: &text::Font, scaled: Scaled) -> f32 {
    let natural = base.average as f64 * scaled.height as f64 / base.height.max(1) as f64;
    if natural <= 0.0 || scaled.advance <= 0 {
        return 1.0;
    }
    (scaled.advance as f64 / natural) as f32
}

/// What a tree draws, in draw order.
#[derive(Clone, Debug, PartialEq)]
pub enum Draw {
    Quad(WidgetId, Quad),
    Text(TextDraw),
}

/// Something a layout needs that it doesn't have.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutError(pub String);

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A layout: widgets under the file's invisible root, with the tree's
/// screen scale and an alpha (fades) over all of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub tree: Tree<Widget>,
    pub scale: Scale,
    /// A fade's alpha over the whole tree.
    pub alpha: u8,
    /// A code-set pixel offset of the hit rects (normally 0).
    pub hit_offset: IVec2,
}

impl Default for Layout {
    fn default() -> Self {
        Self::new()
    }
}

/// The context a layout is placed and drawn in.
pub struct Ctx<'a, T: Textures> {
    pub fonts: &'a FontList,
    pub strings: &'a Strings,
    pub textures: &'a T,
}

impl Layout {
    /// An empty layout: the invisible 1024 x 1024 root at (0, 0), taking
    /// no clicks itself.
    pub fn new() -> Self {
        let mut root = Node::new("", IVec2::ZERO, ROOT_SIZE, Widget::new(Kind::Root));
        root.mouse = false;
        Self {
            tree: Tree::new(root),
            scale: Scale::ONE,
            alpha: 255,
            hit_offset: IVec2::ZERO,
        }
    }

    /// Add a widget under `parent`.
    pub fn add(&mut self, parent: WidgetId, name: &str, pos: IVec2, size: IVec2, kind: Kind) -> WidgetId {
        self.tree.add(parent, Node::new(name, pos, size, Widget::new(kind)))
    }

    pub fn find(&self, name: &str) -> Option<WidgetId> {
        self.tree.find(name)
    }

    pub fn widget(&self, id: WidgetId) -> &Widget {
        &self.tree.get(id).data
    }

    pub fn widget_mut(&mut self, id: WidgetId) -> &mut Widget {
        &mut self.tree.get_mut(id).data
    }

    /// Check every text's font is in the list: a missing one fails the
    /// load naming the control (T1).
    pub fn check_fonts(&self, fonts: &FontList) -> Result<(), LayoutError> {
        for id in self.tree.order() {
            let n = self.tree.get(id);
            if let Kind::Text(t) = &n.data.kind
                && fonts.get(&t.font).is_none()
            {
                return Err(LayoutError(format!(
                    "control {:?}: font {:?} is not in the font list",
                    n.name, t.font
                )));
            }
        }
        Ok(())
    }

    /// A button's size (W3): its Size, or its A texture's when 0; 0 x 0
    /// when its A texture is missing.
    pub fn size_of(&self, id: WidgetId, textures: &impl Textures) -> IVec2 {
        let n = self.tree.get(id);
        if let Kind::Button(b) = &n.data.kind {
            let Some(a) = textures.size(&format!("{}A", b.prefix)) else {
                return IVec2::ZERO;
            };
            if n.size == IVec2::ZERO {
                return a;
            }
        }
        n.size
    }

    /// A text's string: set by code, else its element looked up.
    pub fn string_of(&self, id: WidgetId, strings: &Strings) -> String {
        match self.widget(id).text() {
            Some(t) => t.set.clone().unwrap_or_else(|| strings.resolve(&t.element)),
            None => String::new(),
        }
    }

    /// A text measured with its font at the tree's scale.
    pub fn measure(&self, id: WidgetId, ctx: &Ctx<impl Textures>) -> Option<Measured> {
        let t = self.widget(id).text()?;
        let font = ctx.fonts.get(&t.font)?;
        Some(text::measure(&self.string_of(id, ctx.strings), font, t.wrap, self.scale))
    }

    /// A widget's reference box as a parent of aligned texts (T4): its
    /// position and size, a text's from its measure (the wrap width when
    /// it wraps to two lines or more; n·h_f + (n − 1)·⌊h_f/6⌋ tall, raw).
    fn parent_box(&self, id: WidgetId, ctx: &Ctx<impl Textures>) -> (IVec2, IVec2) {
        if let Some(t) = self.widget(id).text()
            && let Some(m) = self.measure(id, ctx)
        {
            let at = self.text_at(id, ctx).map_or(self.tree.abs_pos(id), |p| p.0);
            let n = m.lines.len() as i32;
            let w = if m.wrapped && n >= 2 { t.wrap } else { m.reference.x };
            let h = n * m.font.height + (n - 1) * m.font.line_gap();
            return (at, IVec2::new(w, h));
        }
        (self.tree.abs_pos(id), self.size_of(id, ctx.textures))
    }

    /// A text's aligned reference position and its measure (T4).
    pub fn text_at(&self, id: WidgetId, ctx: &Ctx<impl Textures>) -> Option<(IVec2, Measured)> {
        let t = self.widget(id).text()?;
        let m = self.measure(id, ctx)?;
        let parent = self.tree.get(id).parent.unwrap_or(Tree::<Widget>::ROOT);
        let (pa, ps) = self.parent_box(parent, ctx);
        let own = self.tree.abs_pos(id);
        Some((canvas::align(own, m.reference, pa, ps, t.h, t.v), m))
    }

    /// Reference to screen for a text: truncated.
    fn text_px(&self, r: IVec2) -> IVec2 {
        IVec2::new(
            (r.x as f64 * self.scale.x).floor() as i32,
            (r.y as f64 * self.scale.y).floor() as i32,
        )
    }

    /// A text as drawn (T4, T6): each line aligned on its own when it
    /// wraps, its colour by its button's state.
    pub fn text_draw(&self, id: WidgetId, ctx: &Ctx<impl Textures>) -> Option<TextDraw> {
        let t = self.widget(id).text()?;
        let (at, m) = self.text_at(id, ctx)?;
        let font = ctx.fonts.get(&t.font)?;
        let top = self.text_px(at);
        let lines = if m.wrapped {
            let parent = self.tree.get(id).parent.unwrap_or(Tree::<Widget>::ROOT);
            let (pa, ps) = self.parent_box(parent, ctx);
            m.lines
                .iter()
                .enumerate()
                .map(|(k, l)| {
                    let w = text::to_reference(IVec2::new(canvas::measure(l, &m.font), 0), self.scale).x;
                    let x = canvas::align(self.tree.abs_pos(id), IVec2::new(w, 0), pa, ps, t.h, VAlign::Default).x;
                    let px = self.text_px(IVec2::new(x, at.y));
                    (IVec2::new(px.x, top.y + k as i32 * m.font.line_step()), l.clone())
                })
                .collect()
        } else {
            vec![(top, m.lines[0].clone())]
        };
        // A button text follows its button's state (always opaque).
        let mut colour = t.colour;
        if let (Some(states), Some(p)) = (t.states, self.tree.get(id).parent) {
            let b = self.widget(p);
            if matches!(b.kind, Kind::Button(_)) {
                colour = match button_state(b) {
                    'B' => states[0],
                    'C' => states[1],
                    'D' => states[2],
                    _ => colour,
                };
            }
        }
        let fade = |mut c: Rgba| {
            c[3] = ((c[3] as u32 * self.alpha as u32) / 255) as u8;
            c
        };
        let effect_colour = if t.effect.is_stroke() { t.stroke } else { t.shadow };
        Some(TextDraw {
            widget: id,
            lines,
            font: m.font,
            face: font.face.clone(),
            colour: fade(colour),
            effect: t.effect,
            effect_colour: fade(effect_colour),
            copies: t.effect.copies(m.wrapped),
            h: t.h,
            reference_at: at,
            reference_size: m.reference,
            rect: PxRect::new(top.x, top.x + m.px.x, top.y, top.y + m.px.y),
            stretch: glyph_stretch(font, m.font),
        })
    }

    /// An image's, window's or nine-slice image's nominal quad (S2), with
    /// σ the code's pop scale.
    pub fn image_quad(&self, id: WidgetId, sigma: Vec2) -> ([f64; 2], [f64; 2]) {
        let (p, s) = (self.tree.abs_pos(id), self.tree.get(id).size);
        let (sx, sy) = (self.scale.x, self.scale.y);
        let (gx, gy) = (sigma.x as f64, sigma.y as f64);
        (
            [
                p.x as f64 * sx + (1.0 - gx) * s.x as f64 * sx / 2.0,
                p.y as f64 * sy + (1.0 - gy) * s.y as f64 * sy / 2.0,
            ],
            [s.x as f64 * sx * gx, s.y as f64 * sy * gy],
        )
    }

    /// A button's quad (S2): σ about the screen origin, sizes floored.
    pub fn button_quad(&self, id: WidgetId, size: IVec2, sigma: Vec2) -> ([f64; 2], [f64; 2]) {
        let p = self.tree.abs_pos(id);
        let (sx, sy) = (self.scale.x * sigma.x as f64, self.scale.y * sigma.y as f64);
        (
            [p.x as f64 * sx, p.y as f64 * sy],
            [(size.x as f64 * sx).floor(), (size.y as f64 * sy).floor()],
        )
    }

    /// A widget's hit rect in screen px (S3): ⌊x s⌋ + o for ⌊w s⌋; a
    /// nine-slice image's size from its TextureAlphaSize; a text's
    /// reference rect, unscaled (quirk 5).
    pub fn hit_rect(&self, id: WidgetId, ctx: &Ctx<impl Textures>) -> PxRect {
        let n = self.tree.get(id);
        if let Kind::Text(_) = n.data.kind {
            return match self.text_at(id, ctx) {
                Some((at, m)) => PxRect::new(at.x, at.x + m.reference.x, at.y, at.y + m.reference.y),
                None => PxRect::new(0, 0, 0, 0),
            };
        }
        let p = self.tree.abs_pos(id);
        let mut size = self.size_of(id, ctx.textures);
        if let Kind::Image(look) = &n.data.kind
            && look.stretch.is_some()
            && let Some(t) = ctx.textures.size(&look.texture)
        {
            let a = n.data.texture_alpha_size;
            if a.x != 0 && t.x > 0 {
                size.x = size.x * a.x / t.x;
            }
            if a.y != 0 && t.y > 0 {
                size.y = size.y * a.y / t.y;
            }
        }
        let fx = |v: i32, s: f64| (v as f64 * s).floor() as i32;
        let x = fx(p.x, self.scale.x) + self.hit_offset.x;
        let y = fx(p.y, self.scale.y) + self.hit_offset.y;
        PxRect::new(x, x + fx(size.x, self.scale.x), y, y + fx(size.y, self.scale.y))
    }

    /// The widget under `pointer` (screen px), by the hit rects (I1).
    pub fn hit(&self, pointer: IVec2, ctx: &Ctx<impl Textures>) -> Option<WidgetId> {
        self.tree.hit(|id| self.hit_rect(id, ctx).contains(pointer))
    }

    /// The widget under `pointer` that accepts a click: the deepest, then
    /// its ancestors in turn, not the root; buttons accept while enabled.
    pub fn click_target(&self, pointer: IVec2, ctx: &Ctx<impl Textures>) -> Option<WidgetId> {
        let deepest = self.hit(pointer, ctx)?;
        std::iter::once(deepest)
            .chain(self.tree.ancestors(deepest))
            .take_while(|&i| i != Tree::<Widget>::ROOT)
            .find(|&i| {
                let w = self.widget(i);
                matches!(w.kind, Kind::Button(_)) && w.enabled
            })
    }

    /// An image quad's UVs (W1): CustomUV when set, else the top-left w x
    /// h texels.
    fn uv(&self, id: WidgetId, look: &ImageLook, textures: &impl Textures) -> [f32; 4] {
        if let Some(uv) = look.uv
            && uv.iter().all(|v| *v >= 0.0)
        {
            return uv;
        }
        let s = self.tree.get(id).size;
        match textures.size(&look.texture) {
            Some(t) if t.x > 0 && t.y > 0 => [0.0, 0.0, s.x as f32 / t.x as f32, s.y as f32 / t.y as f32],
            _ => [0.0, 0.0, 1.0, 1.0],
        }
    }

    /// Nine-slice edges (W2): borders in texels, unscaled; columns clamped
    /// into the quad, rows not.
    fn slices(at: [f64; 2], size: [f64; 2], t: IVec2, s: [f32; 4]) -> ([f64; 4], [f64; 4]) {
        let (tw, th) = (t.x as f64, t.y as f64);
        let [u, v, uw, vh] = s.map(|x| x as f64);
        let (bl, br) = (tw * u, tw * (1.0 - u - uw));
        let (bt, bb) = (th * v, th * (1.0 - v - vh));
        let (x, w) = (at[0], size[0]);
        let clamp = |c: f64| c.clamp(x, x + w);
        let (y, h) = (at[1], size[1]);
        (
            [x, clamp(x + bl), clamp(x + w - br), x + w],
            [y, y + bt, y + h - bb, y + h],
        )
    }

    /// What the tree draws this frame, in draw order (hidden subtrees
    /// left out).
    pub fn draw(&self, ctx: &Ctx<impl Textures>) -> Vec<Draw> {
        let mut out = Vec::new();
        let fade = |mut c: Rgba| {
            c[3] = ((c[3] as u32 * self.alpha as u32) / 255) as u8;
            c
        };
        for id in self.tree.order() {
            if !self.tree.visible(id) {
                continue;
            }
            let w = self.widget(id);
            match &w.kind {
                Kind::Window(look) | Kind::Image(look) => {
                    let window = matches!(w.kind, Kind::Window(_));
                    if look.texture.is_empty() || (window && is_blank(&look.texture)) {
                        continue;
                    }
                    // A missing texture draws nothing (still hit).
                    let Some(t) = ctx.textures.size(&look.texture) else {
                        continue;
                    };
                    let (at, size) = self.image_quad(id, Vec2::ONE);
                    let mut colour = look.colour.unwrap_or([255; 4]);
                    colour[3] = look.alpha.unwrap_or(255);
                    let mut q = Quad::new(at, size, look.texture.clone(), self.uv(id, look, ctx.textures), fade(colour));
                    q.slices = look.stretch.map(|s| Self::slices(at, size, t, s));
                    out.push(Draw::Quad(id, q));
                }
                Kind::Button(b) => {
                    let size = self.size_of(id, ctx.textures);
                    let Some(tex) = button_texture(&b.prefix, button_state(w), ctx.textures) else {
                        continue;
                    };
                    let (at, size) = self.button_quad(id, size, Vec2::ONE);
                    let mut q = Quad::new(at, size, tex.clone(), [0.0, 0.0, 1.0, 1.0], fade([255; 4]));
                    if let (Some(s), Some(t)) = (b.stretch, ctx.textures.size(&tex)) {
                        q.slices = Some(Self::slices(at, size, t, s));
                    }
                    out.push(Draw::Quad(id, q));
                }
                Kind::Text(_) => {
                    if let Some(t) = self.text_draw(id, ctx) {
                        out.push(Draw::Text(t));
                    }
                }
                Kind::Gauge(g) => {
                    let (at, size) = self.image_quad(id, Vec2::ONE);
                    if let Some(t) = ctx.textures.size(&g.frame) {
                        let mut q = Quad::new(at, size, g.frame.clone(), [0.0, 0.0, 1.0, 1.0], fade([255; 4]));
                        q.slices = g.frame_stretch.map(|s| Self::slices(at, size, t, s));
                        out.push(Draw::Quad(id, q));
                    }
                    if let Some(bw) = canvas::gauge_width(g.fill, g.max_range) {
                        let p = self.tree.abs_pos(id) + g.bar_at;
                        let (sx, sy) = (self.scale.x, self.scale.y);
                        let (at, size) = ([p.x as f64 * sx, p.y as f64 * sy], [bw as f64 * sx, g.bar_height as f64 * sy]);
                        let mut q = Quad::new(at, size, g.bar.clone(), [0.0, 0.0, 1.0, 1.0], fade([255; 4]));
                        if let (Some(s), Some(t)) = (g.bar_stretch, ctx.textures.size(&g.bar)) {
                            q.slices = Some(Self::slices(at, size, t, s));
                        }
                        out.push(Draw::Quad(id, q));
                    }
                }
                Kind::List(l) | Kind::Column(l) if !l.back.is_empty() => {
                    if ctx.textures.size(&l.back).is_some() {
                        let (at, size) = self.image_quad(id, Vec2::ONE);
                        out.push(Draw::Quad(id, Quad::new(at, size, l.back.clone(), [0.0, 0.0, 1.0, 1.0], fade([255; 4]))));
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// Centre `id` (a popup's first top-level control) on a `screen` of
    /// px (S1): its position in screen px.
    pub fn centre(&mut self, id: WidgetId, screen: IVec2) {
        let size = self.tree.get(id).size;
        let parent = self.tree.get(id).parent.map_or(IVec2::ZERO, |p| self.tree.abs_pos(p));
        self.tree.get_mut(id).pos = canvas::centre(screen, size) - parent;
    }
}

/// A text placed in a box (`at`, `size`, ref px) at `own` inside it, on
/// a tree at `scale`, as `Layout::text_draw` places it (the HUD's labels
/// and values on their panels).
pub fn boxed_text(at: IVec2, size: IVec2, own: IVec2, look: TextLook, scale: Scale, fonts: &FontList) -> Option<TextDraw> {
    let mut l = Layout::new();
    l.scale = scale;
    let parent = l.add(0, "box", at, size, Kind::Window(ImageLook::default()));
    let id = l.add(parent, "text", own, IVec2::ZERO, Kind::Text(look));
    let none = std::collections::HashMap::<String, IVec2>::new();
    let strings = Strings::default();
    l.text_draw(
        id,
        &Ctx {
            fonts,
            strings: &strings,
            textures: &none,
        },
    )
}

/// List rows (W4): row 0's top at the list's y + Blank, each next at the
/// previous top + its height + ItemGap (no clipping without a scroller).
pub fn list_rows(list_y: i32, blank: i32, gap: i32, heights: &[i32]) -> Vec<i32> {
    let mut y = list_y + blank;
    heights
        .iter()
        .map(|h| {
            let top = y;
            y += h + gap;
            top
        })
        .collect()
}
