//! Options > Multiplayer > Import Spray (specs/hl2dm/game_ui.md 2.3): a
//! Targa (24-bit RGB or 32-bit RGBA) or JPEG picture whose width and
//! height are each 256, 128, 64, 32, 16, 8, 4, 2 or 1 becomes the VTF
//! `<stem>.vtf` in a folder of Chimera's own user data (never the game's
//! install); a VTF is taken as it is. The VTF is written as version 7.2,
//! one uncompressed RGBA8888 level, clamped, without mip maps.
//!
//! Known shortcuts (docs/tech-debt.md): a spray of a name that exists is
//! overwritten (the real game's behaviour is not known: the spec's open
//! question 7); the file name's characters other than letters, digits,
//! `_` and `-` become `_` so that `cl_logofile`'s unquoted path holds
//! together; the error words are ours (the spec names the title and the
//! wrong-size sentence only). Why it's OK for now: the picker lists what
//! was written and the stock sprays are unaffected.

use std::path::Path;

/// The error box's title.
pub const ERROR_TITLE: &str = "Spray Import Error";

/// Why a picture was not imported (each with the box's words, `text`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportError {
    /// The file can't be read.
    CannotRead,
    /// The picture isn't one of the formats, or is damaged.
    Corrupt,
    /// A TGA that isn't 24-bit RGB or 32-bit RGBA.
    WrongTgaFormat,
    /// A side that isn't a power of two up to 256.
    WrongSize,
    /// The VTF can't be written.
    CannotWrite,
}

impl ImportError {
    pub fn text(self) -> &'static str {
        match self {
            ImportError::CannotRead => "The image file could not be read.",
            ImportError::Corrupt => "The image file is corrupt or not a TGA, JPG or VTF picture.",
            ImportError::WrongTgaFormat => "The TGA file must be 24-bit RGB or 32-bit RGBA.",
            ImportError::WrongSize => {
                "Source image is not the correct size. Width and height must each be 256, 128, 64, 32, 16, 8, 4, 2 or 1."
            }
            ImportError::CannotWrite => "The spray could not be written.",
        }
    }
}

/// Whether a side is one the import accepts.
pub fn side_ok(n: u32) -> bool {
    n != 0 && n <= 256 && n.is_power_of_two()
}

/// `name` as a spray's file stem: letters, digits, `_` and `-` stay, the
/// rest becomes `_`.
pub fn stem_of(name: &str) -> String {
    let stem: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == '-' { c } else { '_' })
        .collect();
    if stem.is_empty() { "spray".into() } else { stem }
}

/// An uncompressed RGBA8888 VTF (version 7.2, one level).
pub fn encode_vtf(width: u32, height: u32, rgba: &[u8]) -> Vec<u8> {
    const HEADER: usize = 80;
    // Clamp S and T, no mip maps, no level of detail, 8-bit alpha.
    const FLAGS: u32 = 0x4 | 0x8 | 0x100 | 0x200 | 0x2000;
    let mut v = Vec::with_capacity(HEADER + rgba.len());
    v.extend_from_slice(b"VTF\0");
    v.extend_from_slice(&7u32.to_le_bytes());
    v.extend_from_slice(&2u32.to_le_bytes());
    v.extend_from_slice(&(HEADER as u32).to_le_bytes());
    v.extend_from_slice(&(width as u16).to_le_bytes());
    v.extend_from_slice(&(height as u16).to_le_bytes());
    v.extend_from_slice(&FLAGS.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // frames
    v.extend_from_slice(&0u16.to_le_bytes()); // first frame
    v.extend_from_slice(&[0; 4]);
    for r in [0.5f32, 0.5, 0.5] {
        v.extend_from_slice(&r.to_le_bytes());
    }
    v.extend_from_slice(&[0; 4]);
    v.extend_from_slice(&1.0f32.to_le_bytes()); // bump map scale
    v.extend_from_slice(&0u32.to_le_bytes()); // RGBA8888
    v.push(1); // mip count
    v.extend_from_slice(&u32::MAX.to_le_bytes()); // no low resolution image
    v.extend_from_slice(&[0, 0]);
    v.extend_from_slice(&1u16.to_le_bytes()); // depth
    v.resize(HEADER, 0);
    v.extend_from_slice(rgba);
    v
}

/// Import the picture at `source` into `logos_dir` as `<stem>.vtf`; the
/// stem.
pub fn import(source: &Path, logos_dir: &Path) -> Result<String, ImportError> {
    let ext = source.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let stem = stem_of(&source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default());
    let bytes = std::fs::read(source).map_err(|_| ImportError::CannotRead)?;
    let vtf = match ext.as_str() {
        "vtf" => {
            vtf::from_bytes(&bytes).map_err(|_| ImportError::Corrupt)?;
            bytes
        }
        "tga" | "jpg" | "jpeg" => {
            let format = if ext == "tga" { image::ImageFormat::Tga } else { image::ImageFormat::Jpeg };
            let picture = image::load_from_memory_with_format(&bytes, format).map_err(|_| ImportError::Corrupt)?;
            if format == image::ImageFormat::Tga && !matches!(picture.color(), image::ColorType::Rgb8 | image::ColorType::Rgba8) {
                return Err(ImportError::WrongTgaFormat);
            }
            let rgba = picture.to_rgba8();
            if !side_ok(rgba.width()) || !side_ok(rgba.height()) {
                return Err(ImportError::WrongSize);
            }
            encode_vtf(rgba.width(), rgba.height(), rgba.as_raw())
        }
        _ => return Err(ImportError::Corrupt),
    };
    std::fs::create_dir_all(logos_dir).map_err(|_| ImportError::CannotWrite)?;
    std::fs::write(logos_dir.join(format!("{stem}.vtf")), vtf).map_err(|_| ImportError::CannotWrite)?;
    Ok(stem)
}

/// The stems of the sprays in `logos_dir` (`.vtf` files), sorted.
pub fn imported(logos_dir: &Path) -> Vec<String> {
    let mut stems: Vec<String> = std::fs::read_dir(logos_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix(".vtf").or_else(|| name.strip_suffix(".VTF"))?;
            Some(stem.to_string())
        })
        .collect();
    stems.sort();
    stems
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("chimera_spray_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn tga(path: &Path, w: u32, h: u32, alpha: bool) {
        let mut img = image::RgbaImage::new(w, h);
        for p in img.pixels_mut() {
            *p = image::Rgba([200, 100, 50, 128]);
        }
        if alpha {
            img.save_with_format(path, image::ImageFormat::Tga).unwrap();
        } else {
            image::DynamicImage::ImageRgba8(img).to_rgb8().save_with_format(path, image::ImageFormat::Tga).unwrap();
        }
    }

    #[test]
    fn the_written_vtf_reads_back() {
        let rgba: Vec<u8> = (0..4 * 4 * 4).map(|i| i as u8).collect();
        let bytes = encode_vtf(4, 4, &rgba);
        let v = vtf::from_bytes(&bytes).expect("a VTF");
        let back = v.highres_image.decode(0).expect("level 0").to_rgba8();
        assert_eq!((back.width(), back.height()), (4, 4));
        assert_eq!(back.into_raw(), rgba);
    }

    #[test]
    fn a_128_square_tga_becomes_a_vtf_named_after_the_file() {
        let dir = temp("ok");
        let out = dir.join("logos");
        let src = dir.join("My Spray.tga");
        tga(&src, 128, 128, true);
        assert_eq!(import(&src, &out), Ok("My_Spray".to_string()));
        let file = std::fs::read(out.join("My_Spray.vtf")).unwrap();
        let back = vtf::from_bytes(&file).unwrap();
        let img = back.highres_image.decode(0).unwrap().to_rgba8();
        assert_eq!((img.width(), img.height()), (128, 128));
        assert_eq!(img.get_pixel(3, 3).0, [200, 100, 50, 128]);
        assert_eq!(imported(&out), ["My_Spray"]);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_24_bit_tga_is_taken_and_a_wrong_size_is_refused() {
        let dir = temp("size");
        let out = dir.join("logos");
        let ok = dir.join("rgb.tga");
        tga(&ok, 64, 32, false);
        assert_eq!(import(&ok, &out), Ok("rgb".to_string()));
        let bad = dir.join("bad.tga");
        tga(&bad, 100, 100, true);
        assert_eq!(import(&bad, &out), Err(ImportError::WrongSize));
        assert!(ImportError::WrongSize.text().starts_with("Source image is not the correct size."));
        let big = dir.join("big.tga");
        tga(&big, 512, 512, true);
        assert_eq!(import(&big, &out), Err(ImportError::WrongSize));
        assert_eq!(imported(&out), ["rgb"], "nothing written for the refused ones");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn jpeg_and_unreadable_files() {
        let dir = temp("jpg");
        let out = dir.join("logos");
        let src = dir.join("photo.jpg");
        image::DynamicImage::ImageRgb8(image::RgbImage::new(16, 16))
            .save_with_format(&src, image::ImageFormat::Jpeg)
            .unwrap();
        assert_eq!(import(&src, &out), Ok("photo".to_string()));
        assert_eq!(import(&dir.join("none.tga"), &out), Err(ImportError::CannotRead));
        let junk = dir.join("junk.tga");
        std::fs::write(&junk, b"not a picture").unwrap();
        assert_eq!(import(&junk, &out), Err(ImportError::Corrupt));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_grey_tga_is_the_wrong_format() {
        let dir = temp("grey");
        let src = dir.join("grey.tga");
        image::DynamicImage::ImageLuma8(image::GrayImage::new(16, 16))
            .save_with_format(&src, image::ImageFormat::Tga)
            .unwrap();
        assert_eq!(import(&src, &dir.join("logos")), Err(ImportError::WrongTgaFormat));
        std::fs::remove_dir_all(&dir).ok();
    }
}
