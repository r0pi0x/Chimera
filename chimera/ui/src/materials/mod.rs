//! The kit's UI materials (docs/plans/active/ugg-ui.md, 2.5): WGSL beside
//! the Rust, embedded (`embedded://chimera_ui/materials/<name>.wesl`).
//! Bevy's UI has no skew, so the slant is drawn here.
//!
//! - `SlantMaterial` (`slant.wesl`): a parallelogram fill at the theme's
//!   slant, edges anti-aliased over one physical pixel, an optional
//!   accent bar at the left, a wipe clipping from the left and the right
//!   along the slant, an alpha ramp. The menu's highlight bar, the
//!   toast's wipe, the chip's meter ticks, health segments, vote pips,
//!   the winner slab, the backdrop swap's edge.
//! - `TickBarMaterial` (`tick_bar.wesl`): `n` ticks, `lit`, `hot`, by
//!   whole physical pixel column (`units::TickLayout`).
//! - `PassMaterial` (`pass.wesl`): the logo pass, four slanted bands and
//!   the mark on one full-screen node, its geometry from the theme
//!   (`layout.pass`).
//! - `LinesMaterial` (`lines.wesl`): the match end's background, slanted
//!   hairlines and slabs on a 24-column grid moving on the renderer's
//!   clock (`globals.time`: no CPU work a frame), or frozen at a time
//!   (`freeze`, screenshots).
//! - `AmbientMaterial` (`ambient.wesl`): the main menu's ambient
//!   background, a stub (a slow accent glow on ink) until Look Book V.
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
            app.add_plugins((
                UiMaterialPlugin::<SlantMaterial>::default(),
                UiMaterialPlugin::<TickBarMaterial>::default(),
                UiMaterialPlugin::<PassMaterial>::default(),
                UiMaterialPlugin::<LinesMaterial>::default(),
                UiMaterialPlugin::<AmbientMaterial>::default(),
            ));
        } else if app.is_plugin_added::<AssetPlugin>() {
            app.init_asset::<SlantMaterial>()
                .init_asset::<TickBarMaterial>()
                .init_asset::<PassMaterial>()
                .init_asset::<LinesMaterial>()
                .init_asset::<AmbientMaterial>();
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
        }
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
    /// x: from, y: travel, z: mark x in the ink band, w: ink band width.
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

/// The match end's moving lines (module doc).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct LinesMaterial {
    #[uniform(0)]
    pub color: LinearRgba,
    /// x: loop in seconds, y: far alpha, z: near alpha, w: tan.
    #[uniform(1)]
    pub params: Vec4,
    /// x: opacity, y: columns, z: time offset, w: frozen time (< 0: live).
    #[uniform(2)]
    pub extra: Vec4,
}

impl LinesMaterial {
    /// The look book's: `fg` lines, far at 4 %, near at 2 % (display
    /// space: `light_alpha`), a 24 s loop
    /// (`motion.dur.matchend.lines_loop`), 24 columns.
    pub fn new(theme: &Theme) -> LinesMaterial {
        LinesMaterial {
            color: theme.tone(Tone::Fg).into(),
            params: Vec4::new(
                theme.dur("matchend", "lines_loop").max(1.0),
                crate::theme::light_alpha(0.04),
                crate::theme::light_alpha(0.02),
                theme.tan(),
            ),
            extra: Vec4::new(1.0, 24.0, 0.0, -1.0),
        }
    }

    /// Stopped at `t` seconds into the loop (screenshots, tests); `None`
    /// to move again.
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

/// The main menu's ambient background (module doc; a stub).
#[derive(Asset, TypePath, AsBindGroup, Debug, Clone, PartialEq)]
pub struct AmbientMaterial {
    #[uniform(0)]
    pub ink: LinearRgba,
    #[uniform(1)]
    pub acc: LinearRgba,
    /// x: glow strength, y: loop in seconds, z: tan, w: opacity.
    #[uniform(2)]
    pub params: Vec4,
}

impl AmbientMaterial {
    pub fn new(theme: &Theme) -> AmbientMaterial {
        AmbientMaterial {
            ink: theme.tone(Tone::Ink).into(),
            acc: theme.tone(Tone::Acc).into(),
            params: Vec4::new(0.10, 40.0, theme.tan(), 1.0),
        }
    }
}

impl UiMaterial for AmbientMaterial {
    fn fragment_shader() -> ShaderRef {
        "embedded://chimera_ui/materials/ambient.wesl".into()
    }
}
