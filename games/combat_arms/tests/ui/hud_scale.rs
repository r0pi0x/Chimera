//! The HUD's elements keep the spec's proportions at every resolution
//! (ui_system.md S2, W2, W5, T2): rectangles computed headlessly from a
//! layout written in the game's format, at 1024 x 768 and two wider
//! screens. Written for the 2026-10-31 report that the score bar's
//! text and the stamina gauge's background looked wrong off 4:3.

use std::collections::HashMap;

use bevy::{math::IVec2, prelude::*};
use chimera_client::client::canvas::Scale;
use combat_arms::ui::layout::{self, Ctx, Draw, Font, FontList, Strings, xml};

/// The stamina block's gauge and a score-bar-like text, as the install's
/// HUD layout has them (sizes and stretch values from its data).
const HUD: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<CBALAYOUT>
  <Control Type="window">
    <Property Name="character" ParentName="none" Size="238, 49" Position="-7, 717" Texture="blank"/>
    <Control Type="Gauge">
      <Property Name="char_sp_gauge" Size="124, 16" Position="104, 34" GaugeBackFrameTexture="gauge_bg" GaugeBackTextureSimpleStretchValue="0.4, 0, 0.2, 1" GaugeBarPosition="2.2" GaugeBarTexture="gauge_bar" GaugeBarTextureSimpleStretchValue="0.25, 0, 0.2, 1" GaugeBarMaxRange="120" GaugeBarHeight="16" GaugeBarWidth="100"/>
    </Control>
    <Control Type="Text">
      <Property Name="Mode" Position="100, 10" Font="bold 20" HorizontalAlign="DEFAULT" Element="Elimination" Normalcolor="255,255,255"/>
    </Control>
  </Control>
</CBALAYOUT>
"#;

const SCREENS: [(f32, f32); 3] = [(1024.0, 768.0), (1920.0, 1080.0), (2560.0, 1080.0)];

struct World {
    fonts: FontList,
    strings: Strings,
    textures: HashMap<String, IVec2>,
}

impl World {
    fn new() -> Self {
        let mut fonts = FontList::default();
        fonts.add(Font {
            name: "bold 20".into(),
            height: 20,
            average: 12,
            face: "bold".into(),
        });
        let mut textures = HashMap::new();
        textures.insert("gauge_bg".to_string(), IVec2::new(16, 8));
        textures.insert("gauge_bar".to_string(), IVec2::new(16, 6));
        Self {
            fonts,
            strings: Strings::parse(""),
            textures,
        }
    }
    fn ctx(&self) -> Ctx<'_, HashMap<String, IVec2>> {
        Ctx {
            fonts: &self.fonts,
            strings: &self.strings,
            textures: &self.textures,
        }
    }
    fn layout(&self, screen: (f32, f32)) -> layout::Layout {
        let mut l = xml::read(HUD).unwrap();
        l.scale = Scale::of(Vec2::new(screen.0, screen.1), layout::REFERENCE);
        l
    }
}

/// W5 and W2: the gauge's frame and bar are nine-slices, so each end
/// keeps its texel width on screen at any resolution, and the quads'
/// rectangles are the reference rectangles scaled by (x_r, y_r).
#[test]
fn gauge_frame_and_bar_are_nine_slices_at_every_resolution() {
    let w = World::new();
    for screen in SCREENS {
        let l = w.layout(screen);
        let (xr, yr) = (screen.0 as f64 / 1024.0, screen.1 as f64 / 768.0);
        let id = l.find("char_sp_gauge").unwrap();
        let quads: Vec<_> = l
            .draw(&w.ctx())
            .into_iter()
            .filter_map(|d| match d {
                Draw::Quad(i, q) if i == id => Some(q),
                _ => None,
            })
            .collect();
        assert_eq!(quads.len(), 2, "{screen:?}: frame and bar");
        let (frame, bar) = (&quads[0], &quads[1]);
        // Absolute reference position: the window's (-7, 717) plus (104, 34).
        let (fx, fy) = (97.0 * xr, 751.0 * yr);
        assert!((frame.at[0] - fx).abs() < 1e-6 && (frame.at[1] - fy).abs() < 1e-6, "{screen:?}: frame at {:?}", frame.at);
        assert!((frame.size[0] - 124.0 * xr).abs() < 1e-6 && (frame.size[1] - 16.0 * yr).abs() < 1e-6, "{screen:?}: frame size");
        // The frame's ends are 0.4 and 0.4 of a 16 texel texture: 6.4 px,
        // unscaled; the middle takes the rest.
        let (cols, _) = frame.slices.expect("the frame is a nine-slice");
        assert!((cols[1] - cols[0] - 6.4).abs() < 1e-6, "{screen:?}: left end {}", cols[1] - cols[0]);
        assert!((cols[3] - cols[2] - 6.4).abs() < 1e-6, "{screen:?}: right end {}", cols[3] - cols[2]);
        // The bar: at frame + (2, 2) ref, its full width (100 %) 120 ref.
        assert!((bar.at[0] - 99.0 * xr).abs() < 1e-6 && (bar.at[1] - 753.0 * yr).abs() < 1e-6, "{screen:?}: bar at {:?}", bar.at);
        assert!((bar.size[0] - 120.0 * xr).abs() < 1e-6 && (bar.size[1] - 16.0 * yr).abs() < 1e-6, "{screen:?}: bar size");
        let (bcols, _) = bar.slices.expect("the bar is a nine-slice");
        assert!((bcols[1] - bcols[0] - 4.0).abs() < 1e-6, "{screen:?}: bar's left end");
        // The bar lies inside the frame's width at every resolution.
        assert!(bar.at[0] >= frame.at[0] && bar.at[0] + bar.size[0] <= frame.at[0] + frame.size[0], "{screen:?}: bar inside the frame");
    }
}

/// T2: text scales by x_r across and y_r down, so its glyphs stretch
/// horizontally by ⌊a_f x_r⌋ over the face's width at the scaled height
/// (1 on a 4:3 screen), and its box is the reference box scaled.
#[test]
fn text_glyphs_stretch_with_the_horizontal_scale() {
    let w = World::new();
    for screen in SCREENS {
        let l = w.layout(screen);
        let (xr, yr) = (screen.0 as f64 / 1024.0, screen.1 as f64 / 768.0);
        let t = l.text_draw(l.find("Mode").unwrap(), &w.ctx()).unwrap();
        assert_eq!(t.font.height, (20.0 * yr).floor() as i32, "{screen:?}: height");
        assert_eq!(t.font.advance, (12.0 * xr).floor() as i32, "{screen:?}: advance");
        let want = t.font.advance as f64 / (12.0 * t.font.height as f64 / 20.0);
        assert!((t.stretch as f64 - want).abs() < 1e-4, "{screen:?}: stretch {} want {want}", t.stretch);
        // 4:3 is unstretched; a wider screen stretches the glyphs wider.
        if screen.0 * 3.0 == screen.1 * 4.0 {
            assert!((t.stretch - 1.0).abs() < 1e-6);
        } else {
            assert!(t.stretch > 1.2, "{screen:?}: {}", t.stretch);
        }
        // Eleven glyphs: the box is the scaled advance times the count,
        // from the truncated scaled top-left (-7 + 100, 717 + 10).
        let top = t.lines[0].0;
        assert_eq!(top, IVec2::new((93.0 * xr).floor() as i32, (727.0 * yr).floor() as i32), "{screen:?}: top-left");
        assert_eq!(t.rect.x1 - t.rect.x0, 11 * t.font.advance, "{screen:?}: width");
    }
}
