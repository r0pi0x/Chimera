//! The pixel decoders of a bitmap image (specs/halo_ce/bitmaps.md, "Pixel
//! formats", "Block compression", "The bump palette" and "The swizzle
//! rule"): one stored level's bytes in, RGBA8 out (top row first, slices
//! of a volume one after the other), for every format the disc uses.
//!
//! Plain functions of their arguments: no state, so any number of threads
//! may decode at once. The bump palette is generated on each call that
//! needs it (249 entries, microseconds) instead of cached.
//!
//! Known shortcuts:
//! - The swizzled decoder looks every texel up through three small
//!   per-axis offset tables and converts it through a function pointer;
//!   straightforward, not vectorised.
//! - The a8 format decodes to (0, 0, 0, A), the spec's default for its
//!   open question.

use std::io;

use super::{PixelFormat, layout::Storage};

fn err(msg: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.into())
}

/// A 5-bit value to 8 bits.
fn e5(v: u32) -> u8 {
    ((v << 3) | (v >> 2)) as u8
}

/// A 6-bit value to 8 bits.
fn e6(v: u32) -> u8 {
    ((v << 2) | (v >> 4)) as u8
}

/// A 4-bit value to 8 bits.
fn e4(v: u32) -> u8 {
    (v * 17) as u8
}

/// An r5g6b5 value to opaque RGBA8.
fn rgb565(v: u16) -> [u8; 4] {
    let v = u32::from(v);
    [e5(v >> 11), e6((v >> 5) & 0x3F), e5(v & 0x1F), 255]
}

fn texel_a8(b: &[u8]) -> [u8; 4] {
    [0, 0, 0, b[0]]
}
fn texel_y8(b: &[u8]) -> [u8; 4] {
    [b[0], b[0], b[0], 255]
}
fn texel_ay8(b: &[u8]) -> [u8; 4] {
    [b[0], b[0], b[0], b[0]]
}
fn texel_a8y8(b: &[u8]) -> [u8; 4] {
    [b[0], b[0], b[0], b[1]]
}
fn texel_r5g6b5(b: &[u8]) -> [u8; 4] {
    rgb565(u16::from_le_bytes([b[0], b[1]]))
}
fn texel_a1r5g5b5(b: &[u8]) -> [u8; 4] {
    let v = u32::from(u16::from_le_bytes([b[0], b[1]]));
    [
        e5((v >> 10) & 0x1F),
        e5((v >> 5) & 0x1F),
        e5(v & 0x1F),
        if v & 0x8000 != 0 { 255 } else { 0 },
    ]
}
fn texel_a4r4g4b4(b: &[u8]) -> [u8; 4] {
    let v = u32::from(u16::from_le_bytes([b[0], b[1]]));
    [e4((v >> 8) & 0xF), e4((v >> 4) & 0xF), e4(v & 0xF), e4((v >> 12) & 0xF)]
}
fn texel_x8r8g8b8(b: &[u8]) -> [u8; 4] {
    [b[2], b[1], b[0], 255]
}
fn texel_a8r8g8b8(b: &[u8]) -> [u8; 4] {
    [b[2], b[1], b[0], b[3]]
}

/// The built-in 256-entry palette of the bump format (code 17), generated
/// by the spec's rule: normal vectors on a grid of -9..=9 on two axes.
pub fn bump_palette() -> [[u8; 4]; 256] {
    // The coordinate byte of grid index -9..=9.
    const COORD: [u8; 19] = [
        25, 47, 66, 82, 96, 107, 116, 122, 126, 128, 129, 133, 139, 148, 159, 173, 189, 208, 229,
    ];
    let mut out = [[0u8; 4]; 256];
    let mut n = 0;
    for j in -9i32..=9 {
        for i in -9i32..=9 {
            if i.abs() + j.abs() > 11 {
                continue;
            }
            let comp = |k: i32| 0.8 * f64::from(k.signum()) * (f64::from(k) / 9.0).powi(2);
            let (x, y) = (comp(i), comp(j));
            let z = (1.0 - x * x - y * y).max(0.0).sqrt();
            let b = (127.5 + 127.5 * z + 0.5).floor() as u8;
            out[n] = [COORD[(i + 9) as usize], COORD[(j + 9) as usize], b, 255];
            n += 1;
        }
    }
    debug_assert_eq!(n, 249);
    // 249..=254 stay (0, 0, 0, 0).
    out[255] = [128, 128, 255, 0];
    out
}

/// The offset tables of the swizzle rule for a level: `n = xs[x] | ys[y] |
/// zs[z]` is the texel number of (x, y, z). Dimensions must be powers of
/// two (checked by the layout).
fn swizzle_tables(w: u32, h: u32, d: u32) -> [Vec<u32>; 3] {
    let dims = [w, h, d];
    let mut masks = [0u32; 3];
    let mut bit = 0;
    let mut k = 0;
    loop {
        let mut any = false;
        for (axis, &dim) in dims.iter().enumerate() {
            if (1u64 << k) < u64::from(dim) {
                masks[axis] |= 1 << bit;
                bit += 1;
                any = true;
            }
        }
        if !any {
            break;
        }
        k += 1;
    }
    let table = |len: u32, mask: u32| -> Vec<u32> {
        (0..len)
            .map(|v| {
                let (mut out, mut m, mut v) = (0u32, mask, v);
                while m != 0 {
                    let low = m & m.wrapping_neg();
                    if v & 1 != 0 {
                        out |= low;
                    }
                    v >>= 1;
                    m &= m - 1;
                }
                out
            })
            .collect()
    };
    [table(w, masks[0]), table(h, masks[1]), table(d, masks[2])]
}

/// The texel number of (x, y, z) in a swizzled level of `w` x `h` x `d`
/// (the spec's rule; the decoder uses the tables, this is for tests and
/// callers that want a single texel).
pub fn swizzled_index(w: u32, h: u32, d: u32, x: u32, y: u32, z: u32) -> u32 {
    let [xs, ys, zs] = swizzle_tables(w, h, d);
    xs[x as usize] | ys[y as usize] | zs[z as usize]
}

/// Decode one stored level of an uncompressed format to RGBA8. `src` is the
/// level's bytes (at least `w * h * d` texels); `storage` is `Swizzled` or
/// `Linear`.
pub fn decode_uncompressed(
    format: PixelFormat,
    storage: Storage,
    w: u32,
    h: u32,
    d: u32,
    src: &[u8],
) -> io::Result<Vec<u8>> {
    let bpp = format.texel_bytes();
    if bpp == 0 || storage == Storage::Blocks {
        return Err(err(format!("{format:?} is not an uncompressed {storage:?} format")));
    }
    let (wu, hu, du) = (w as usize, h as usize, d as usize);
    let texels = wu
        .checked_mul(hu)
        .and_then(|t| t.checked_mul(du))
        .ok_or_else(|| err("level too large"))?;
    if src.len() < texels * bpp {
        return Err(err(format!(
            "{format:?} level {w}x{h}x{d} needs {} bytes, {} given",
            texels * bpp,
            src.len()
        )));
    }
    if storage == Storage::Swizzled && !(w.is_power_of_two() && h.is_power_of_two() && d.is_power_of_two()) {
        return Err(err(format!("swizzled level {w}x{h}x{d}: not powers of two")));
    }
    let palette = (format == PixelFormat::P8Bump).then(bump_palette);
    let conv: fn(&[u8]) -> [u8; 4] = match format {
        PixelFormat::A8 => texel_a8,
        PixelFormat::Y8 => texel_y8,
        PixelFormat::Ay8 => texel_ay8,
        PixelFormat::A8y8 => texel_a8y8,
        PixelFormat::R5g6b5 => texel_r5g6b5,
        PixelFormat::A1r5g5b5 => texel_a1r5g5b5,
        PixelFormat::A4r4g4b4 => texel_a4r4g4b4,
        PixelFormat::X8r8g8b8 => texel_x8r8g8b8,
        PixelFormat::A8r8g8b8 => texel_a8r8g8b8,
        _ => texel_y8, // P8Bump (replaced by the palette below)
    };
    let mut out = vec![0u8; texels * 4];
    let tables = (storage == Storage::Swizzled).then(|| swizzle_tables(w, h, d));
    let mut o = 0;
    for z in 0..du {
        for y in 0..hu {
            for x in 0..wu {
                let n = match &tables {
                    Some([xs, ys, zs]) => (xs[x] | ys[y] | zs[z]) as usize,
                    None => (z * hu + y) * wu + x,
                };
                let t = &src[n * bpp..n * bpp + bpp];
                let px = match &palette {
                    Some(p) => p[t[0] as usize],
                    None => conv(t),
                };
                out[o..o + 4].copy_from_slice(&px);
                o += 4;
            }
        }
    }
    Ok(out)
}

/// The four colours of a block's colour part (8 bytes) and whether index 3
/// is transparent (DXT1's three-colour mode).
fn block_colors(b: &[u8], dxt1: bool) -> [[u8; 4]; 4] {
    let c0v = u16::from_le_bytes([b[0], b[1]]);
    let c1v = u16::from_le_bytes([b[2], b[3]]);
    let (c0, c1) = (rgb565(c0v), rgb565(c1v));
    let mix = |wa: u32, wb: u32, div: u32| -> [u8; 4] {
        let mut c = [0u8, 0, 0, 255];
        for k in 0..3 {
            c[k] = ((wa * u32::from(c0[k]) + wb * u32::from(c1[k])) / div) as u8;
        }
        c
    };
    if c0v > c1v || !dxt1 {
        [c0, c1, mix(2, 1, 3), mix(1, 2, 3)]
    } else {
        [c0, c1, mix(1, 1, 2), [0, 0, 0, 0]]
    }
}

/// The eight alpha values of a DXT5 block's endpoints.
fn dxt5_alphas(a0: u8, a1: u8) -> [u8; 8] {
    let (p, q) = (u32::from(a0), u32::from(a1));
    let mut a = [a0, a1, 0, 0, 0, 0, 0, 0];
    if a0 > a1 {
        for k in 2..8u32 {
            a[k as usize] = (((8 - k) * p + (k - 1) * q) / 7) as u8;
        }
    } else {
        for k in 2..6u32 {
            a[k as usize] = (((6 - k) * p + (k - 1) * q) / 5) as u8;
        }
        a[6] = 0;
        a[7] = 255;
    }
    a
}

/// Decode one stored level of a block compressed format (DXT1, DXT3, DXT5)
/// to RGBA8: `ceil(w/4) x ceil(h/4)` blocks in `src`, edge blocks clipped.
pub fn decode_blocks(format: PixelFormat, w: u32, h: u32, src: &[u8]) -> io::Result<Vec<u8>> {
    let bs = format.block_bytes();
    if bs == 0 {
        return Err(err(format!("{format:?} is not block compressed")));
    }
    let (wu, hu) = (w as usize, h as usize);
    let (bx, by) = (wu.div_ceil(4), hu.div_ceil(4));
    let need = bx * by * bs;
    if src.len() < need {
        return Err(err(format!(
            "{format:?} level {w}x{h} needs {need} bytes, {} given",
            src.len()
        )));
    }
    let mut out = vec![0u8; wu * hu * 4];
    for brow in 0..by {
        for bcol in 0..bx {
            let b = &src[(brow * bx + bcol) * bs..][..bs];
            let (alpha, color) = match format {
                PixelFormat::Dxt1 => (None, &b[0..8]),
                _ => (Some(&b[0..8]), &b[8..16]),
            };
            let colors = block_colors(color, format == PixelFormat::Dxt1);
            let idx = u32::from_le_bytes([color[4], color[5], color[6], color[7]]);
            let alphas = match (format, alpha) {
                (PixelFormat::Dxt5, Some(a)) => {
                    let table = dxt5_alphas(a[0], a[1]);
                    let bits = u64::from_le_bytes([a[2], a[3], a[4], a[5], a[6], a[7], 0, 0]);
                    Some(std::array::from_fn::<u8, 16, _>(|i| {
                        table[((bits >> (3 * i)) & 7) as usize]
                    }))
                }
                (PixelFormat::Dxt3, Some(a)) => {
                    let bits = u64::from_le_bytes(a.try_into().expect("8 bytes"));
                    Some(std::array::from_fn::<u8, 16, _>(|i| {
                        e4(((bits >> (4 * i)) & 0xF) as u32)
                    }))
                }
                _ => None,
            };
            for i in 0..16usize {
                let (x, y) = (bcol * 4 + i % 4, brow * 4 + i / 4);
                if x >= wu || y >= hu {
                    continue;
                }
                let mut px = colors[((idx >> (2 * i)) & 3) as usize];
                if let Some(a) = &alphas {
                    px[3] = a[i];
                }
                out[(y * wu + x) * 4..][..4].copy_from_slice(&px);
            }
        }
    }
    Ok(out)
}
