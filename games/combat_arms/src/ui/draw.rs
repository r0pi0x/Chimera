//! Drawing Combat Arms' laid-out UI (`layout::Draw`: quads and texts in
//! screen px) with the shell's UI nodes, for the HUD (`ui::hud`) and the
//! in-game menu (`ui::menu`).
//!
//! - Texts: each line at its pixel position in its font's face at the
//!   scaled height (`layout::text::Scaled`), the effect's copies (outline
//!   or shadow) under it in the effect's colour. The faces are the
//!   install's font files (`data/ui/fonts.toml`'s `file`, read from
//!   FONTS.rez at startup, `CaFaces`) when there is an install, else the
//!   shell's stand-ins (`UiFonts`: Tahoma or its fallback).
//! - Quads: textures by name are the game's art, from the install's UI
//!   archives (`CaArt`, made from `ui::install`'s decoded textures: the
//!   texel rect of the quad's UVs, nine-slice quads as nine parts); without
//!   it `StandIns` paints each named texture as a flat colour of the
//!   texture's size.
//!
//! Known shortcuts: glyphs are stretched by the average width's ratio
//! (T2's ⌊a_f x_r⌋ over the face's width at the scaled height), not
//! rebuilt at the forced width, and a face's glyph widths differ from the monospaced metrics the
//! layout measures with, so each line is aligned inside its measured box
//! as its text is (left, centred, right); nine-slice and UVs don't show
//! on flat stand-ins; an image wider than its texture is stretched, not
//! repeated (W1).

use std::collections::HashMap;

use bevy::{prelude::*, text::FontSmoothing};

use super::{
    install::texture_key,
    layout::{Draw, FontList, Quad, Rgba, TextDraw, Textures},
};
use crate::client::{canvas::HAlign, fonts::UiFonts};

/// The install's font faces by role (`bold`, `light`, `number`): their
/// handles and line height over the em.
#[derive(Resource, Default)]
pub struct CaFaces(pub HashMap<String, (Handle<Font>, f32)>);

/// Stand-in textures: a size and a flat colour per name.
#[derive(Resource, Clone, Debug, Default)]
pub struct StandIns(pub HashMap<String, (IVec2, Rgba)>);

impl Textures for StandIns {
    fn size(&self, name: &str) -> Option<IVec2> {
        self.0.get(&texture_key(name)).map(|(s, _)| *s)
    }
}

impl StandIns {
    pub fn colour(&self, name: &str) -> Rgba {
        self.0.get(&texture_key(name)).map_or([255; 4], |(_, c)| *c)
    }
}

/// The install's art as images, by `install::texture_key` (none
/// without the install).
#[derive(Resource, Clone, Debug, Default)]
pub struct CaArt(pub HashMap<String, (Handle<Image>, IVec2)>);

impl CaArt {
    pub fn get(&self, name: &str) -> Option<&(Handle<Image>, IVec2)> {
        self.0.get(&texture_key(name))
    }
}

/// A decoded texture as an image (sRGB RGBA).
pub fn rgba_image(a: &super::install::Art) -> Image {
    use bevy::{
        asset::RenderAssetUsages,
        render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    };
    Image::new(
        Extent3d {
            width: a.size.x as u32,
            height: a.size.y as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        a.rgba.clone(),
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    )
}

/// Make the install's art images (`CaUi::install`), once.
pub fn load_art(ui: Res<super::CaUi>, images: Option<ResMut<Assets<Image>>>, mut commands: Commands) {
    let mut art = CaArt::default();
    if let (Some(inst), Some(mut images)) = (&ui.install, images) {
        for (k, a) in &inst.art {
            art.0.insert(k.clone(), (images.add(rgba_image(a)), a.size));
        }
    }
    commands.insert_resource(art);
}

/// Load the install's faces named in the font list (none without an
/// install, or headless).
pub fn load_faces(mut commands: Commands, fonts: Option<ResMut<Assets<Font>>>) {
    let mut faces = CaFaces::default();
    if let Some(mut assets) = fonts
        && let Some(mount) = crate::game::INFO.mount()
    {
        for face in &FontList::ours().faces {
            let Some(file) = &face.file else { continue };
            match mount.read(file) {
                Ok(bytes) => {
                    let line = crate::client::fonts::line_per_em(&bytes).unwrap_or(1.2);
                    let handle = assets.add(Font::from_bytes(bytes));
                    faces.0.insert(face.role.clone(), (handle, line));
                }
                Err(e) => warn!("combat_arms ui: font {file}: {e}; drawing with a stand-in"),
            }
        }
        info!("combat_arms ui: {} font faces from the install", faces.0.len());
    }
    commands.insert_resource(faces);
}

fn colour(c: Rgba) -> Color {
    Color::srgba_u8(c[0], c[1], c[2], c[3])
}

fn multiply(a: Rgba, b: Rgba) -> Rgba {
    let m = |x: u8, y: u8| ((x as u32 * y as u32) / 255) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), m(a[3], b[3])]
}

/// The text font for a face and a cell height in px.
pub fn text_font(face: &str, height: i32, fonts: Option<&UiFonts>, faces: Option<&CaFaces>) -> TextFont {
    let h = height.max(1) as f32;
    if let Some((handle, line)) = faces.and_then(|f| f.0.get(face)) {
        return TextFont {
            font: handle.clone().into(),
            font_size: FontSize::Px((h / line).max(1.0)),
            font_smoothing: FontSmoothing::AntiAliased,
            ..default()
        };
    }
    let bold = face != "light";
    fonts.map_or_else(
        || TextFont {
            font_size: FontSize::Px(h * 0.85),
            ..default()
        },
        |f| f.line("Tahoma", bold, h, true),
    )
}

/// Spawn one text (its effect's copies first) under `p`.
pub fn spawn_text(p: &mut ChildSpawnerCommands, t: &TextDraw, fonts: Option<&UiFonts>, faces: Option<&CaFaces>) {
    let font = text_font(&t.face, t.font.height, fonts, faces);
    let justify = match t.h {
        HAlign::Centre => JustifyContent::Center,
        HAlign::Right => JustifyContent::End,
        _ => JustifyContent::Start,
    };
    for (at, line) in &t.lines {
        let width = line.chars().count() as i32 * t.font.advance;
        // The glyphs stretch about the box's centre (T2): the node is the
        // box over the stretch, so scaled it fills the box.
        let stretch = if t.stretch > 0.0 { t.stretch } else { 1.0 };
        let inner = width as f32 / stretch;
        let shift = (width as f32 - inner) / 2.0;
        let mut one = |off: IVec2, c: Rgba| {
            p.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px((at.x + off.x) as f32 + shift),
                    top: px((at.y + off.y) as f32),
                    width: px(inner),
                    height: px(t.font.height as f32),
                    justify_content: justify,
                    align_items: AlignItems::Center,
                    overflow: Overflow::visible(),
                    ..default()
                },
                UiTransform::from_scale(Vec2::new(stretch, 1.0)),
                children![(Text::new(line.clone()), font.clone(), TextColor(colour(c)), TextLayout::no_wrap())],
            ));
        };
        for off in &t.copies {
            one(*off, t.effect_colour);
        }
        one(IVec2::ZERO, t.colour);
    }
}

/// An image node over `at` (x0, y0, x1, y1 px) showing texels `rect`.
pub fn image_part(p: &mut ChildSpawnerCommands, image: &Handle<Image>, at: [f32; 4], rect: Rect, c: Rgba) {
    let (w, h) = (at[2] - at[0], at[3] - at[1]);
    if w <= 0.0 || h <= 0.0 || rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    p.spawn((
        Node {
            position_type: PositionType::Absolute,
            left: px(at[0]),
            top: px(at[1]),
            width: px(w),
            height: px(h),
            ..default()
        },
        ImageNode {
            image: image.clone(),
            color: colour(c),
            rect: Some(rect),
            image_mode: NodeImageMode::Stretch,
            ..default()
        },
    ));
}

/// A quad drawn with the install's image: its UVs as a texel rect
/// (clamped to the texture: no repeat), nine-slice as nine parts, each
/// border's texels at their drawn size.
fn spawn_image(p: &mut ChildSpawnerCommands, q: &Quad, image: &Handle<Image>, size: IVec2) {
    let (tw, th) = (size.x as f32, size.y as f32);
    match q.slices {
        Some((cols, rows)) => {
            let (bl, br) = ((cols[1] - cols[0]) as f32, (cols[3] - cols[2]) as f32);
            let (bt, bb) = ((rows[1] - rows[0]) as f32, (rows[3] - rows[2]) as f32);
            let tx = [0.0, bl.min(tw), (tw - br).max(0.0), tw];
            let ty = [0.0, bt.min(th), (th - bb).max(0.0), th];
            for r in 0..3 {
                for c in 0..3 {
                    let at = [cols[c] as f32, rows[r] as f32, cols[c + 1] as f32, rows[r + 1] as f32];
                    let rect = Rect::new(tx[c], ty[r], tx[c + 1].max(tx[c]), ty[r + 1].max(ty[r]));
                    image_part(p, image, at, rect, q.colour);
                }
            }
        }
        None => {
            let r = q.cover;
            let [u0, v0, u1, v1] = q.uv;
            let rect = Rect::new(
                (u0 * tw).clamp(0.0, tw),
                (v0 * th).clamp(0.0, th),
                (u1 * tw).clamp(0.0, tw),
                (v1 * th).clamp(0.0, th),
            );
            image_part(p, image, [r.x0 as f32, r.y0 as f32, r.x1 as f32, r.y1 as f32], rect, q.colour);
        }
    }
}

/// Spawn a draw list under `p`, in order: quads with the install's image
/// when `art` has it, else the stand-in's flat colour.
pub fn spawn_draws(
    p: &mut ChildSpawnerCommands,
    draws: &[Draw],
    stand_ins: &StandIns,
    art: Option<&CaArt>,
    fonts: Option<&UiFonts>,
    faces: Option<&CaFaces>,
) {
    for d in draws {
        match d {
            Draw::Quad(_, q) => {
                if let Some((image, size)) = art.and_then(|a| a.get(&q.texture)) {
                    spawn_image(p, q, image, *size);
                    continue;
                }
                let r = q.cover;
                let s = r.size();
                p.spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(r.x0 as f32),
                        top: px(r.y0 as f32),
                        width: px(s.x as f32),
                        height: px(s.y as f32),
                        ..default()
                    },
                    BackgroundColor(colour(multiply(stand_ins.colour(&q.texture), q.colour))),
                ));
            }
            Draw::Text(t) => spawn_text(p, t, fonts, faces),
        }
    }
}
