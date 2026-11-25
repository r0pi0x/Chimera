//! Sprites, SPR version 2 (specs/goldsrc/spr.md): `Sprite::parse` reads
//! the 40-byte header, the palette and every frame (single frames and,
//! though no installed sprite has one, frame groups); `Sprite::rgba`
//! converts a frame by the texture format. `parse_sprite_list` reads the
//! HUD's rectangle lists (`sprites/hud.txt`, `weapon_*.txt`).

use chimera_vfs::bytes::Le;

use crate::{Result, err};

pub const MAGIC: &[u8; 4] = b"IDSP";
pub const VERSION: i32 = 2;

/// How the sprite blends (the header's texture format).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextureFormat {
    Normal,
    Additive,
    IndexAlpha,
    AlphaTest,
}

/// One picture: origin (top-left relative to the centre, y up), size and
/// palette indices row by row from the top.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Frame {
    pub origin: [i32; 2],
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// A frame entry: one frame, or a group with cumulative end times.
#[derive(Clone, Debug, PartialEq)]
pub enum FrameEntry {
    Single(Frame),
    Group { intervals: Vec<f32>, frames: Vec<Frame> },
}

/// A sprite file.
#[derive(Clone, Debug, PartialEq)]
pub struct Sprite {
    /// 0 upright facing, 1 facing, 2 parallel, 3 oriented, 4 parallel with
    /// the entity's roll.
    pub orientation: i32,
    pub format: TextureFormat,
    pub radius: f32,
    pub max_width: i32,
    pub max_height: i32,
    pub beam_length: f32,
    pub sync: i32,
    pub palette: Vec<[u8; 3]>,
    pub frames: Vec<FrameEntry>,
    /// Bytes after the last frame (0 in every installed file).
    pub trailing: usize,
}

fn frame(r: &mut Le) -> Result<Frame> {
    let origin = [r.i32()?, r.i32()?];
    let (w, h) = (r.i32()?, r.i32()?);
    if !(0..=4096).contains(&w) || !(0..=4096).contains(&h) {
        return err(format!("sprite frame size {w} x {h}"));
    }
    Ok(Frame {
        origin,
        width: w as u32,
        height: h as u32,
        pixels: r.take((w * h) as usize)?.to_vec(),
    })
}

impl Sprite {
    pub fn parse(bytes: &[u8]) -> Result<Sprite> {
        let mut r = Le::new(bytes);
        if r.take(4)? != MAGIC {
            return err("not a sprite (IDSP)");
        }
        let version = r.i32()?;
        if version != VERSION {
            return err(format!("sprite version {version}, not {VERSION}"));
        }
        let orientation = r.i32()?;
        let format = match r.i32()? {
            0 => TextureFormat::Normal,
            1 => TextureFormat::Additive,
            2 => TextureFormat::IndexAlpha,
            3 => TextureFormat::AlphaTest,
            f => return err(format!("sprite texture format {f}")),
        };
        let radius = r.f32()?;
        let (max_width, max_height, count) = (r.i32()?, r.i32()?, r.i32()?);
        let beam_length = r.f32()?;
        let sync = r.i32()?;
        let colours = r.u16()? as usize;
        let palette: Vec<[u8; 3]> = r
            .take(colours * 3)?
            .chunks_exact(3)
            .map(|c| [c[0], c[1], c[2]])
            .collect();
        let mut frames = Vec::with_capacity(count.max(0) as usize);
        for _ in 0..count {
            if r.i32()? == 0 {
                frames.push(FrameEntry::Single(frame(&mut r)?));
            } else {
                let n = r.i32()?;
                let intervals = (0..n).map(|_| r.f32()).collect::<std::result::Result<Vec<_>, _>>()?;
                let frames_ = (0..n).map(|_| frame(&mut r)).collect::<Result<Vec<_>>>()?;
                frames.push(FrameEntry::Group {
                    intervals,
                    frames: frames_,
                });
            }
        }
        Ok(Sprite {
            orientation,
            format,
            radius,
            max_width,
            max_height,
            beam_length,
            sync,
            palette,
            frames,
            trailing: r.left(),
        })
    }

    /// Every frame in order, groups flattened.
    pub fn all_frames(&self) -> Vec<&Frame> {
        self.frames
            .iter()
            .flat_map(|f| match f {
                FrameEntry::Single(f) => vec![f],
                FrameEntry::Group { frames, .. } => frames.iter().collect(),
            })
            .collect()
    }

    /// A frame as RGBA8 by the texture format (additive frames are opaque
    /// here: the blending is the drawer's).
    pub fn rgba(&self, frame: &Frame) -> Vec<u8> {
        let last = self.palette.last().copied().unwrap_or([255; 3]);
        let col = |i: u8| self.palette.get(i as usize).copied().unwrap_or([0; 3]);
        frame
            .pixels
            .iter()
            .flat_map(|&i| match self.format {
                TextureFormat::Normal | TextureFormat::Additive => {
                    let c = col(i);
                    [c[0], c[1], c[2], 255]
                }
                TextureFormat::IndexAlpha => [last[0], last[1], last[2], i],
                TextureFormat::AlphaTest => {
                    let c = col(i);
                    [c[0], c[1], c[2], if i == 255 { 0 } else { 255 }]
                }
            })
            .collect()
    }
}

/// One rectangle of a HUD sprite list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpriteListEntry {
    pub name: String,
    /// 320 or 640.
    pub resolution: u32,
    /// The sprite's file name without folder or extension (`640hud7`).
    pub sprite: String,
    pub rect: [i32; 4],
}

/// `sprites/hud.txt` or `weapon_*.txt`: a count, then entries of name,
/// resolution, sprite, x, y, width, height (whitespace-separated); the
/// entries there when the text ends before the count's.
pub fn parse_sprite_list(text: &str) -> Result<Vec<SpriteListEntry>> {
    let text: String = text
        .lines()
        .map(|l| l.split("//").next().unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n");
    let mut words = text.split_whitespace();
    let count: usize = words
        .next()
        .and_then(|w| w.parse().ok())
        .ok_or_else(|| crate::Error("sprite list: no count".into()))?;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        // A list may hold fewer entries than its count says (the
        // install's `hud.txt`: 215 counted, 193 there): it ends there.
        let Some(name) = words.next() else { break };
        let mut next = || {
            words
                .next()
                .ok_or_else(|| crate::Error("sprite list: entry cut short".into()))
        };
        let name = name.to_string();
        let resolution = next()?
            .parse()
            .map_err(|_| crate::Error(format!("sprite list {name}: resolution")))?;
        let sprite = next()?.to_string();
        let mut rect = [0; 4];
        for v in &mut rect {
            *v = next()?
                .parse()
                .map_err(|_| crate::Error(format!("sprite list {name}: rectangle")))?;
        }
        out.push(SpriteListEntry {
            name,
            resolution,
            sprite,
            rect,
        });
    }
    Ok(out)
}
