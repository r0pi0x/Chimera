//! Sprites (`.spr`, specs/combat_arms/models_ltb.md "Sprite skins"): five
//! little-endian u32 (frame count n, 1..4096; frames per second r, > 0;
//! colour-keyed flag; translucent flag; colour key), then n frames, each
//! a u16 length and that many bytes of texture path (no terminator).
//! Nothing follows the frames; anything else is refused.

/// A sprite file's header and frame list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sprite {
    /// Frames per second (> 0).
    pub rate: u32,
    pub color_keyed: bool,
    pub translucent: bool,
    pub color_key: u32,
    /// Each frame's texture path, as stored.
    pub frames: Vec<String>,
}

/// Most frames a sprite has.
pub const MAX_FRAMES: u32 = 4096;

impl Sprite {
    pub fn parse(bytes: &[u8]) -> Result<Sprite, String> {
        let u32_at = |at: usize| -> Result<u32, String> {
            Ok(u32::from_le_bytes(bytes.get(at..at + 4).ok_or("sprite cut short")?.try_into().unwrap()))
        };
        let n = u32_at(0)?;
        let rate = u32_at(4)?;
        if !(1..=MAX_FRAMES).contains(&n) {
            return Err(format!("sprite frame count {n}"));
        }
        if rate == 0 {
            return Err("sprite frame rate 0".into());
        }
        let mut at = 20;
        let mut frames = Vec::with_capacity(n as usize);
        for _ in 0..n {
            let len = u16::from_le_bytes(bytes.get(at..at + 2).ok_or("sprite cut short")?.try_into().unwrap()) as usize;
            let name = bytes.get(at + 2..at + 2 + len).ok_or("sprite cut short")?;
            frames.push(String::from_utf8_lossy(name).into_owned());
            at += 2 + len;
        }
        if at != bytes.len() {
            return Err(format!("{} bytes after the sprite's frames", bytes.len() - at));
        }
        Ok(Sprite {
            rate,
            color_keyed: u32_at(8)? != 0,
            translucent: u32_at(12)? != 0,
            color_key: u32_at(16)?,
            frames,
        })
    }

    /// How long each frame shows as a model skin: floor(1000 / r) ms.
    pub fn frame_ms(&self) -> u32 {
        1000 / self.rate.max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(n: u32, rate: u32, frames: &[&str]) -> Vec<u8> {
        let mut b = Vec::new();
        for v in [n, rate, 0, 1, 0x00FF00FF] {
            b.extend(v.to_le_bytes());
        }
        for f in frames {
            b.extend((f.len() as u16).to_le_bytes());
            b.extend(f.as_bytes());
        }
        b
    }

    #[test]
    fn reads_the_header_and_frames() {
        let names = ["A-1.DTX", "A-2.DTX", "A-3.DTX", "A-4.DTX", "A-5.DTX"];
        let s = Sprite::parse(&file(5, 10, &names)).unwrap();
        assert_eq!(s.frames, names);
        assert_eq!((s.rate, s.color_keyed, s.translucent, s.color_key), (10, false, true, 0x00FF00FF));
        // models_ltb.md R5: 10 per second is 100 ms a frame; 3: 333.
        assert_eq!(s.frame_ms(), 100);
        assert_eq!(Sprite::parse(&file(1, 3, &["x"])).unwrap().frame_ms(), 333);
    }

    #[test]
    fn refuses_bad_files() {
        assert!(Sprite::parse(&file(0, 10, &[])).is_err());
        assert!(Sprite::parse(&file(1, 0, &["x"])).is_err());
        assert!(Sprite::parse(&file(2, 10, &["x"])).is_err());
        let mut extra = file(1, 10, &["x"]);
        extra.push(0);
        assert!(Sprite::parse(&extra).is_err());
        assert!(Sprite::parse(&[1, 0]).is_err());
    }
}
