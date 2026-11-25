//! GoldSrc's sprites (specs/goldsrc/spr.md) in Chimera's types, for any
//! GoldSrc game:
//!
//! - **Map sprites** (`add_map`): each `env_sprite` and `env_glow` whose
//!   `model` is a `.spr` (entities.md, "env_sprite, env_glow") as a
//!   `MapSprite` at its origin: its first frame, the frame's size ×
//!   `scale` (0: 1), `rendercolor` (all 0: white) and `renderamt` / 255
//!   as its colour (no `renderamt` with mode 0: opaque), added in the
//!   additive and glow modes (5, 3; 3 drawn over everything, faded where
//!   hidden) or for an additive sprite file, else blended by its alpha;
//!   the entity's on/off (an unnamed sprite or `Start on`) is its
//!   `start_on`, which the logic switches (`logic::bridge`, `EntityPart`).
//! - **Sheets** (`particle_material`): a sprite's frames stacked in one
//!   texture as a particle material's sprite sheet (sequence 0, the frames
//!   in order), for effects a game draws as particles (CS 1.6's HE
//!   explosion and smoke puffs); additive by its texture format, else
//!   blended by its alpha.
//!
//! Known shortcuts:
//! - A map sprite shows its first frame only (no `framerate`
//!   animation) and always faces the view (the upright, oriented and
//!   rolled types, spr.md "Orientation types", are drawn parallel to it).
//! - `cycler_sprite` (studio models in the stock maps) isn't drawn.

use bevy::math::{Vec2, Vec4};
use goldsrc_formats::spr::{Sprite, TextureFormat};

use super::{METERS_PER_UNIT, to_engine};
use crate::{
    map::{
        MapData, MapSprite, MapTexture, SpriteBlend,
        particles::{ParticleBlend, ParticleMaterial},
    },
    mount::Mount,
};

/// The sprite file at `path`.
pub fn read(mount: &Mount, path: &str) -> Result<Sprite, String> {
    let bytes = mount.read(path).map_err(|e| format!("{path}: {e}"))?;
    Sprite::parse(&bytes).map_err(|e| format!("{path}: {e}"))
}

/// Frame `i` of `spr` as a texture named `name` (pixels only when
/// `decode`).
pub fn frame_texture(spr: &Sprite, i: usize, name: &str, decode: bool) -> Option<MapTexture> {
    let frames = spr.all_frames();
    let frame = *frames.get(i)?;
    Some(MapTexture {
        name: name.to_string(),
        srgb: true,
        mips: Vec::new(),
        width: frame.width,
        height: frame.height,
        rgba8: if decode { spr.rgba(frame) } else { Vec::new() },
        bc: None,
    })
}

/// Every frame of `spr` stacked top to bottom in one texture of the
/// widest frame's width (each frame at the left of its row), and each
/// frame's rectangle there (min u, min v, max u, max v).
pub fn sheet(spr: &Sprite, name: &str, decode: bool) -> Option<(MapTexture, Vec<[f32; 4]>)> {
    let frames = spr.all_frames();
    let width = frames.iter().map(|f| f.width).max()?;
    let height: u32 = frames.iter().map(|f| f.height).sum();
    if width == 0 || height == 0 {
        return None;
    }
    let mut rgba = if decode { vec![0u8; (width * height * 4) as usize] } else { Vec::new() };
    let mut rects = Vec::with_capacity(frames.len());
    let mut y = 0u32;
    for f in &frames {
        if decode {
            let px = spr.rgba(f);
            for row in 0..f.height {
                let src = &px[(row * f.width * 4) as usize..((row + 1) * f.width * 4) as usize];
                let at = (((y + row) * width) * 4) as usize;
                rgba[at..at + src.len()].copy_from_slice(src);
            }
        }
        rects.push([
            0.0,
            y as f32 / height as f32,
            f.width as f32 / width as f32,
            (y + f.height) as f32 / height as f32,
        ]);
        y += f.height;
    }
    let texture = MapTexture {
        name: name.to_string(),
        srgb: true,
        mips: Vec::new(),
        width,
        height,
        rgba8: rgba,
        bc: None,
    };
    Some((texture, rects))
}

/// The sprite at `path` as a particle material named by its path, with
/// its sheet appended to `data`'s textures (module doc); its index in
/// `data.particles`, the one there already when added before; None (a
/// warning) when it doesn't read.
pub fn particle_material(mount: &Mount, path: &str, decode: bool, data: &mut MapData) -> Option<usize> {
    if let Some(i) = data.particles.find(path) {
        return Some(i);
    }
    let spr = match read(mount, path) {
        Ok(s) => s,
        Err(e) => {
            data.warnings.push(e);
            return None;
        }
    };
    let (texture, rects) = sheet(&spr, path, decode)?;
    data.textures.push(texture);
    data.particles.materials.push(ParticleMaterial {
        name: path.to_string(),
        texture: Some(data.textures.len() - 1),
        blend: if spr.format == TextureFormat::Additive { ParticleBlend::Additive } else { ParticleBlend::Alpha },
        vertex_color: true,
        vertex_alpha: true,
        sequences: vec![rects],
        ..Default::default()
    });
    Some(data.particles.materials.len() - 1)
}

/// The size of `spr`'s first frame, units (width, height).
pub fn frame_size(spr: &Sprite) -> Vec2 {
    spr.all_frames().first().map_or(Vec2::ZERO, |f| Vec2::new(f.width as f32, f.height as f32))
}

fn floats<const N: usize>(text: Option<&str>) -> Option<[f32; N]> {
    let v: Vec<f32> = text?.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    v.try_into().ok()
}

/// The map's `env_sprite`s and `env_glow`s as sprites (module doc); a
/// sprite file that doesn't read is a warning.
pub fn add_map(mount: &Mount, decode: bool, data: &mut MapData) {
    let mut loaded: Vec<(String, Option<(usize, Vec2, TextureFormat)>)> = Vec::new();
    for i in 0..data.entities.len() {
        let e = &data.entities[i];
        let class = e.classname().to_ascii_lowercase();
        if class != "env_sprite" && class != "env_glow" {
            continue;
        }
        let Some(model) = e.get("model").map(|m| m.trim().to_ascii_lowercase()).filter(|m| m.ends_with(".spr")) else {
            continue;
        };
        let Some(origin) = floats::<3>(e.get("origin")) else { continue };
        let num = |k: &str| e.get(k).and_then(|v| v.trim().parse::<f32>().ok()).unwrap_or(0.0);
        let mode = num("rendermode") as i32;
        let amount = num("renderamt");
        let color = floats::<3>(e.get("rendercolor")).filter(|c| c.iter().any(|v| *v > 0.0)).unwrap_or([255.0; 3]);
        let scale = if num("scale") > 0.0 { num("scale") } else { 1.0 };
        let named = e.get("targetname").is_some_and(|n| !n.trim().is_empty());
        let flags = num("spawnflags") as i32;
        let start_on = class == "env_glow" || !named || flags & 1 != 0;
        let found = match loaded.iter().find(|(p, _)| *p == model) {
            Some((_, f)) => *f,
            None => {
                let f = match read(mount, &model) {
                    Ok(spr) => frame_texture(&spr, 0, &model, decode).map(|t| {
                        data.textures.push(t);
                        (data.textures.len() - 1, frame_size(&spr), spr.format)
                    }),
                    Err(e) => {
                        data.warnings.push(e);
                        None
                    }
                };
                loaded.push((model.clone(), f));
                f
            }
        };
        let Some((texture, size, format)) = found else { continue };
        let additive = matches!(mode, 3 | 5) || (mode == 0 && format == TextureFormat::Additive);
        let alpha = if mode == 0 { 1.0 } else { (amount / 255.0).clamp(0.0, 1.0) };
        let size = size * scale * METERS_PER_UNIT;
        data.sprites.push(MapSprite {
            position: to_engine(origin),
            texture,
            size,
            color: Vec4::new(color[0] / 255.0, color[1] / 255.0, color[2] / 255.0, alpha),
            glow: mode == 3,
            blend: if additive { SpriteBlend::Add } else { SpriteBlend::Blend },
            proxy: size.x.min(size.y) * 0.25,
            entity: Some(i),
            start_on,
        });
    }
}
