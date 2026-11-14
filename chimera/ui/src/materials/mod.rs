//! The kit's UI materials (docs/plans/active/ugg-ui.md, 2.5): WGSL beside
//! the Rust, embedded (`embedded://chimera_ui/materials/<name>.wesl`).
//! Bevy's UI has no skew, so the slant is drawn here.
//!
//! - `SlantMaterial` (`slant.wesl`): a parallelogram fill at the theme's
//!   slant, edges anti-aliased over one physical pixel, an optional
//!   accent bar at the left, a wipe clipping from the left and the right
//!   along the slant, an alpha ramp. The menu's highlight bar, the
//!   toast's wipe, the chip's meter ticks, health segments, vote pips,
//!   the winner slab, the backdrop swap's edge; or only its outline
//!   (`outline`: the lobby's unready block). The node's box holds the
//!   whole parallelogram (unlike CSS's skew, which leans out of its box):
//!   make it the fill's width plus its height times the slant's tan.
//! - `TickBarMaterial` (`tick_bar.wesl`): `n` ticks, `lit`, `hot`, by
//!   whole physical pixel column (`units::TickLayout`).
//! - `PassMaterial` (`pass.wesl`): the logo pass, four slanted bands and
//!   the mark on one full-screen node, its geometry from the theme
//!   (`layout.pass`).
//! - `LinesMaterial` (`lines.wesl`): the match end's broad slanted
//!   planes (look book V's rule): four layers of planes 16, 32, 64 and
//!   128 units wide (1/192 of the width), each repeating every three
//!   widths, each moving one spacing per `lines_loop` s (the widest
//!   fastest) on the renderer's clock (no CPU work a frame) or frozen
//!   (`freeze`); each adds `PLANES_ALPHA` of `fg` over the ink in display
//!   space, written as the linear-light alpha that lands there.
//! - `AmbientMaterial` (`ambient.wesl`): the ambient background every
//!   between-match screen shares (look book V: slanted light bands, a
//!   faceted field re-levelling every 24 s, one soft cobalt bloom, a cool
//!   lift at the bottom, a vignette, a dither), our port of the look
//!   book's fragment shader; per screen a calm column (the menu's) and
//!   the bloom's place (`AmbientMaterial::MENU`, `MATCH_END`). Composed in
//!   display space as the look book's, written as linear light; opaque.
//! - `EdgeMaterial` (`edge.wesl`): a glow along the screen's edges (an
//!   inset shadow and a hairline border), its opacity a parameter: the air
//!   strike's pulse. Its middle is discarded: never a full-screen fill.
//! - `RingMaterial` (`ring.wesl`): a countdown ring, the arc from the top
//!   clockwise over a track (the effects chips').
//!
//! Colours go in as linear RGBA (`LinearRgba::from(color)`); builders
//! take Bevy `Color`s. `KitMaterialsPlugin` registers them (a
//! `UiMaterialPlugin` each where a renderer runs; the assets alone
//! headless, so screens' systems run in tests).

use bevy::{asset::embedded_asset, prelude::*, reflect::TypePath, render::render_resource::AsBindGroup, shader::ShaderRef};

use crate::{
    theme::{Theme, Tone},
    units::TickLayout,
};

/// Registers the kit's materials (module doc).
pub struct KitMaterialsPlugin;

impl Plugin for KitMaterialsPlugin {
    fn build(&self, app: &mut App) {
        if app.get_sub_app(bevy::render::RenderApp).is_some() {
            embedded_asset!(app, "slant.wesl");
            embedded_asset!(app, "tick_bar.wesl");
            embedded_asset!(app, "pass.wesl");
            embedded_asset!(app, "lines.wesl");
            embedded_asset!(app, "ambient.wesl");
            embedded_asset!(app, "edge.wesl");
            embedded_asset!(app, "ring.wesl");
            app.add_plugins((
                UiMaterialPlugin::<SlantMaterial>::default(),
                UiMaterialPlugin::<TickBarMaterial>::default(),
                UiMaterialPlugin::<PassMaterial>::default(),
                UiMaterialPlugin::<LinesMaterial>::default(),
                UiMaterialPlugin::<AmbientMaterial>::default(),
                UiMaterialPlugin::<EdgeMaterial>::default(),
                UiMaterialPlugin::<RingMaterial>::default(),
            ));
        } else if app.is_plugin_added::<AssetPlugin>() {
            app.init_asset::<SlantMaterial>()
                .init_asset::<TickBarMaterial>()
                .init_asset::<PassMaterial>()
                .init_asset::<LinesMaterial>()
                .init_asset::<AmbientMaterial>()
                .init_asset::<EdgeMaterial>()
                .init_asset::<RingMaterial>();
        }
    }
}

/// A slanted fill (module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct SlantMaterial {
    #[uniform(0)]
    pub fill: LinearRgba,
    #[uniform(1)]
    pub accent: LinearRgba,
    /// x: tan of the slant, y: left edge slanted (0/1), z: right edge
    /// slanted (0/1), w: the accent bar's width in physical px.
    #[uniform(2)]
    pub shape: Vec4,
    /// x: clip from the left (0-1), y: clip from the right, z, w: alpha
    /// at the left and the right.
    #[uniform(3)]
    pub wipe: Vec4,
    /// x: drawn as an outline this many physical px wide (0: filled);
    /// y: the wipe's cuts lean with the slant (0/1) even where the fill's
    /// own edges are upright (the toast's wipe).
    #[uniform(4)]
    pub line: Vec4,
}

impl SlantMaterial {
    /// `fill` slanted on both edges at the theme's slant, no accent, no
    /// clip, even alpha.
    pub fn new(theme: &Theme, fill: Color) -> SlantMaterial {
        SlantMaterial {
            fill: fill.into(),
            accent: LinearRgba::NONE,
            shape: Vec4::new(theme.tan(), 1.0, 1.0, 0.0),
            wipe: Vec4::new(0.0, 0.0, 1.0, 1.0),
            line: Vec4::ZERO,
        }
    }

    /// Only its outline, `px` physical pixels wide (an unlit block).
    pub fn outline(mut self, px: f32) -> Self {
        self.line.x = px;
        self
    }

    /// The wipe's cuts lean with the slant (an upright box wiped in
    /// along the slant, as the toast is).
    pub fn slanted_cut(mut self) -> Self {
        self.line.y = 1.0;
        self
    }

    /// Which edges slant (a bar flush at the left: `edges(false, true)`).
    pub fn edges(mut self, left: bool, right: bool) -> Self {
        self.shape.y = left as u8 as f32;
        self.shape.z = right as u8 as f32;
        self
    }

    /// An accent bar `px` physical pixels wide along the left edge.
    pub fn accent(mut self, color: Color, px: f32) -> Self {
        self.accent = color.into();
        self.shape.w = px;
        self
    }

    /// Clipped from the left and the right (0-1 of the row, along the
    /// slant): a wipe.
    pub fn clip(mut self, left: f32, right: f32) -> Self {
        self.set_clip(left, right);
        self
    }

    /// Alpha from `left` at the left edge to `right` at the right.
    pub fn ramp(mut self, left: f32, right: f32) -> Self {
        self.wipe.z = left;
        self.wipe.w = right;
        self
    }

    pub fn set_clip(&mut self, left: f32, right: f32) {
        self.wipe.x = left;
        self.wipe.y = right;
    }

    pub fn set_fill(&mut self, fill: Color) {
        self.fill = fill.into();
    }
}

impl UiMaterial for SlantMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/slant.wesl".into()
    }
}

/// A tick bar (module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct TickBarMaterial {
    #[uniform(0)]
    pub on: LinearRgba,
    #[uniform(1)]
    pub off: LinearRgba,
    #[uniform(2)]
    pub hot: LinearRgba,
    /// x: n, y: lit, z: hot from, w: pitch (physical px).
    #[uniform(3)]
    pub counts: Vec4,
    /// x: tick width, y: tick height (physical px), z: tan of the skew.
    #[uniform(4)]
    pub dims: Vec4,
}

impl TickBarMaterial {
    /// `n` ticks laid out as `layout`, none lit, in the theme's colours
    /// (lit `fg`, unlit `fg` at .16, hot `acc`).
    pub fn new(theme: &Theme, n: u32, layout: TickLayout) -> TickBarMaterial {
        TickBarMaterial {
            on: theme.tone(Tone::Fg).into(),
            off: theme.tone(Tone::Fg).with_alpha(crate::theme::light_alpha(0.16)).into(),
            hot: theme.tone(Tone::Acc).into(),
            counts: Vec4::new(n as f32, 0.0, n as f32, layout.pitch as f32),
            dims: Vec4::new(layout.tick as f32, layout.h as f32, 0.0, 0.0),
        }
    }

    /// The unlit colour.
    pub fn unlit(mut self, c: Color) -> Self {
        self.off = c.into();
        self
    }

    /// Ticks skewed at the theme's slant (the chip's meter).
    pub fn skewed(mut self, theme: &Theme) -> Self {
        self.dims.z = theme.tan();
        self
    }

    /// `lit` ticks lit, the lit ones from `hot` on in the hot colour.
    pub fn set(&mut self, lit: u32, hot: u32) {
        self.counts.y = lit as f32;
        self.counts.z = hot as f32;
    }

    pub fn set_layout(&mut self, layout: TickLayout) {
        self.counts.w = layout.pitch as f32;
        self.dims.x = layout.tick as f32;
        self.dims.y = layout.h as f32;
    }
}

impl UiMaterial for TickBarMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/tick_bar.wesl".into()
    }
}

/// The logo pass (module doc): one full-screen node; `set_progress` each
/// frame of the pass's 1.2 s.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct PassMaterial {
    #[uniform(0)]
    pub ink: LinearRgba,
    #[uniform(1)]
    pub acc: LinearRgba,
    #[uniform(2)]
    pub fg: LinearRgba,
    /// x: progress 0-1, y: tan, z: has a mark.
    #[uniform(3)]
    pub state: Vec4,
    /// x: from, y: travel, z: the mark's centre in the ink band (at the
    /// frame's mid-height, as every offset), w: ink band width.
    #[uniform(4)]
    pub sweep: Vec4,
    /// x, y: the accent hairline's offset and width; z, w: the accent band's.
    #[uniform(5)]
    pub bands: Vec4,
    /// x, y: the light band's offset and width; z, w: the mark's size.
    #[uniform(6)]
    pub bands2: Vec4,
    /// The mark's wipe in (start, length) and out (start, length), as
    /// fractions of the pass.
    #[uniform(7)]
    pub mark_wipe: Vec4,
    /// The mark's coverage (alpha), drawn in `fg`.
    #[texture(8)]
    #[sampler(9)]
    pub mark: Option<Handle<Image>>,
}

impl PassMaterial {
    /// The pass from the theme's colours, `layout.pass` and
    /// `motion.dur.pass`, at progress 0.
    pub fn new(theme: &Theme, mark: Option<Handle<Image>>) -> PassMaterial {
        let l = |k: &str| theme.layout("pass", k);
        let total = theme.dur("pass", "total").max(1e-3);
        let d = |k: &str| theme.dur("pass", k) / total;
        PassMaterial {
            ink: theme.tone(Tone::Ink).into(),
            acc: theme.tone(Tone::Acc).into(),
            fg: theme.tone(Tone::Fg).into(),
            state: Vec4::new(0.0, theme.tan(), mark.is_some() as u8 as f32, 0.0),
            sweep: Vec4::new(l("from"), l("travel"), l("mark_x"), l("band_ink_w")),
            bands: Vec4::new(l("band_acc_x"), l("band_acc_w"), l("band_acc2_x"), l("band_acc2_w")),
            bands2: Vec4::new(l("band_fg_x"), l("band_fg_w"), l("mark_w"), l("mark_h")),
            mark_wipe: Vec4::new(d("mark_in_at"), d("mark_in"), d("mark_out_at"), d("mark_out")),
            mark,
        }
    }

    pub fn set_progress(&mut self, p: f32) {
        self.state.x = p;
    }
}

impl UiMaterial for PassMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/pass.wesl".into()
    }
}

/// The match end's broad slanted planes (module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct LinesMaterial {
    /// The planes' colour and the ground under them, display space (0-1).
    #[uniform(0)]
    pub color: Vec4,
    #[uniform(1)]
    pub ground: Vec4,
    /// x: seconds a layer takes to move one spacing, y: alpha per layer
    /// (display space), w: tan.
    #[uniform(2)]
    pub params: Vec4,
    /// x: opacity, y: units across the width, z: time offset, w: frozen
    /// time (< 0: live).
    #[uniform(3)]
    pub extra: Vec4,
}

impl LinesMaterial {
    /// Each layer's alpha over the ambient (the look book's "ambient +
    /// planes"; alone it is .02).
    pub const PLANES_ALPHA: f32 = 0.006;

    /// The look book's "ambient + planes": `fg` planes over `ink`,
    /// `PLANES_ALPHA` a layer (display space), one spacing per
    /// `motion.dur.matchend.lines_loop` s (96), 192 units across.
    pub fn new(theme: &Theme) -> LinesMaterial {
        let display = |c: Color| {
            let s = c.to_srgba();
            Vec4::new(s.red, s.green, s.blue, s.alpha)
        };
        LinesMaterial {
            color: display(theme.tone(Tone::Fg)),
            ground: display(theme.tone(Tone::Ink)),
            params: Vec4::new(
                theme.dur_opt("matchend", "lines_loop").unwrap_or(96.0).max(1.0),
                Self::PLANES_ALPHA,
                0.0,
                theme.tan(),
            ),
            extra: Vec4::new(1.0, 192.0, 0.0, -1.0),
        }
    }

    /// Each layer's alpha (display space): .006 under the ambient, .02 alone.
    pub fn alpha(mut self, a: f32) -> Self {
        self.params.y = a;
        self
    }

    /// Stopped at `t` seconds (screenshots, tests); `None` to move again.
    pub fn freeze(mut self, t: Option<f32>) -> Self {
        self.extra.w = t.unwrap_or(-1.0);
        self
    }

    pub fn set_opacity(&mut self, a: f32) {
        self.extra.x = a;
    }
}

impl UiMaterial for LinesMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/lines.wesl".into()
    }
}

/// The shared ambient background (module doc): the look book's ambient
/// shader, ported. Colours go in display space (sRGB, as the look book's
/// shader takes them); the shader composes there and writes linear light.
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct AmbientMaterial {
    /// The ground, the type colour and the accent, display space (0-1).
    #[uniform(0)]
    pub ink: Vec4,
    #[uniform(1)]
    pub fg: Vec4,
    #[uniform(2)]
    pub acc: Vec4,
    /// x: the calm column's right edge (0-1 of the width; < 0: none, the
    /// whole screen live), y, z: the bloom's centre (x, and y up; 0-1),
    /// w: tan of the slant.
    #[uniform(3)]
    pub place: Vec4,
    /// x: time offset (s), y: frozen time (< 0: live), z: opacity.
    #[uniform(4)]
    pub clock: Vec4,
}

/// A colour as the shader's display-space vector.
fn display(c: Color) -> Vec4 {
    let s = c.to_srgba();
    Vec4::new(s.red, s.green, s.blue, s.alpha)
}

impl AmbientMaterial {
    /// The look book's main menu: a calm column to 30 % of the width, the
    /// bloom at (.68, .56).
    pub const MENU: (f32, Vec2) = (0.30, Vec2::new(0.68, 0.56));
    /// The match end: no column, the bloom at (.64, .68).
    pub const MATCH_END: (f32, Vec2) = (-1.0, Vec2::new(0.64, 0.68));

    /// The theme's ink, fg and accent, live, laid out as the match end
    /// (`MATCH_END`; `column` and `bloom` change it). Its clock starts 40 s
    /// in, as the look book's.
    pub fn new(theme: &Theme) -> AmbientMaterial {
        AmbientMaterial {
            ink: display(theme.tone(Tone::Ink)),
            fg: display(theme.tone(Tone::Fg)),
            acc: display(theme.tone(Tone::Acc)),
            place: Vec4::new(Self::MATCH_END.0, Self::MATCH_END.1.x, Self::MATCH_END.1.y, theme.tan()),
            clock: Vec4::new(40.0, -1.0, 1.0, 0.0),
        }
    }

    /// The calm column's right edge (0-1 of the width), or `None`.
    pub fn column(mut self, edge: Option<f32>) -> Self {
        self.place.x = edge.unwrap_or(-1.0);
        self
    }

    /// The bloom's centre: x, and y up, 0-1 of the screen.
    pub fn bloom(mut self, at: Vec2) -> Self {
        self.place.y = at.x;
        self.place.z = at.y;
        self
    }

    /// Stopped at `t` s (screenshots, tests); `None` to move again.
    pub fn freeze(mut self, t: Option<f32>) -> Self {
        self.clock.y = t.unwrap_or(-1.0);
        self
    }

    pub fn set_opacity(&mut self, a: f32) {
        self.clock.z = a;
    }
}

impl UiMaterial for AmbientMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/ambient.wesl".into()
    }
}

/// The screen's edge glowing (module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct EdgeMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x: opacity (0-1), y: the glow's blur, z: its spread, w: the
    /// border's width, in physical px.
    #[uniform(1)]
    pub params: Vec4,
    /// x: the glow's alpha, y: the border's (before the opacity).
    #[uniform(2)]
    pub alphas: Vec4,
}

impl EdgeMaterial {
    /// `color` glowing `blur` px wide from `spread` px in, a `border` px
    /// line at the edge (physical px); off until `set_opacity`.
    pub fn new(color: Color, blur: f32, spread: f32, border: f32) -> EdgeMaterial {
        EdgeMaterial {
            color: color.with_alpha(1.0).into(),
            params: Vec4::new(0.0, blur, spread, border),
            alphas: Vec4::new(0.55, 0.85, 0.0, 0.0),
        }
    }

    pub fn set_opacity(&mut self, a: f32) {
        self.params.x = a;
    }
}

impl UiMaterial for EdgeMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/edge.wesl".into()
    }
}

/// A countdown ring (module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct RingMaterial {
    #[uniform(0)]
    pub track: LinearRgba,
    #[uniform(1)]
    pub arc: LinearRgba,
    /// x: the arc's share of the ring (0-1, from the top clockwise), y:
    /// the line's width in physical px.
    #[uniform(2)]
    pub params: Vec4,
}

impl RingMaterial {
    pub fn new(track: Color, arc: Color, width: f32) -> RingMaterial {
        RingMaterial { track: track.into(), arc: arc.into(), params: Vec4::new(1.0, width, 0.0, 0.0) }
    }

    pub fn set_share(&mut self, share: f32) {
        self.params.x = share.clamp(0.0, 1.0);
    }
}

impl UiMaterial for RingMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/ring.wesl".into()
    }
}
