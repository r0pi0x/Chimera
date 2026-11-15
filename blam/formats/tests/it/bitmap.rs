//! The bitmap reader and decoders on synthetic data: every case of
//! specs/halo_ce/bitmaps.md's test section (a), byte for byte, on the
//! decoder functions directly and, where a whole tag matters (levels,
//! cube faces, the stored-level rule, pixel fetch at an absolute offset),
//! through a bitmap tag built with `map_fixture`.

use blam_formats::bitmap::{
    Bitmap, BitmapType, ImageType, Layout, PixelFormat, Storage, Usage, bump_palette, decode_blocks, decode_image,
    decode_uncompressed, read_image, swizzled_index,
};
use blam_formats::cache::MapFile;

use crate::map_fixture::{CacheFile, HEADER, Struct, TagData, c};

const SWZ: Storage = Storage::Swizzled;

fn px(format: PixelFormat, w: u32, h: u32, d: u32, bytes: &[u8]) -> Vec<[u8; 4]> {
    let out = decode_uncompressed(format, SWZ, w, h, d, bytes).unwrap();
    out.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect()
}

#[test]
fn small_formats_two_by_one() {
    use PixelFormat::*;
    #[rustfmt::skip]
    type Case<'a> = (PixelFormat, &'a [u8], [[u8; 4]; 2]);
    let cases: &[Case] = &[
        (A8, &[0x00, 0xFF], [[0, 0, 0, 0], [0, 0, 0, 255]]),
        (Y8, &[0x00, 0x80], [[0, 0, 0, 255], [128, 128, 128, 255]]),
        (Ay8, &[0x40, 0xFF], [[64, 64, 64, 64], [255, 255, 255, 255]]),
        (A8y8, &[0x10, 0x80, 0xFF, 0x00], [[16, 16, 16, 128], [255, 255, 255, 0]]),
        (R5g6b5, &[0x00, 0xF8, 0xE0, 0x07], [[255, 0, 0, 255], [0, 255, 0, 255]]),
        (
            R5g6b5,
            &[0x10, 0x84, 0x1F, 0x00],
            [[132, 130, 132, 255], [0, 0, 255, 255]],
        ),
        (A1r5g5b5, &[0x00, 0xFC, 0xE0, 0x03], [[255, 0, 0, 255], [0, 255, 0, 0]]),
        (
            A1r5g5b5,
            &[0x10, 0x00, 0xFF, 0xFF],
            [[0, 0, 132, 0], [255, 255, 255, 255]],
        ),
        (
            A4r4g4b4,
            &[0x21, 0x84, 0x0F, 0xF0],
            [[68, 34, 17, 136], [0, 0, 255, 255]],
        ),
        (
            X8r8g8b8,
            &[0x10, 0x20, 0x30, 0xFF, 0x10, 0x20, 0x30, 0x00],
            [[48, 32, 16, 255], [48, 32, 16, 255]],
        ),
        (
            A8r8g8b8,
            &[0x10, 0x20, 0x30, 0x40, 0, 0, 0, 0],
            [[48, 32, 16, 64], [0, 0, 0, 0]],
        ),
    ];
    for (format, bytes, want) in cases {
        assert_eq!(px(*format, 2, 1, 1, bytes), want, "{format:?} {bytes:02X?}");
    }
}

/// 16 a8r8g8b8 texels, texel k = bytes k, 0, 0, FF.
fn sixteen() -> Vec<u8> {
    (0..16u8).flat_map(|k| [k, 0, 0, 0xFF]).collect()
}

/// The blue channel of each decoded texel, as rows.
fn blue_rows(out: &[[u8; 4]], w: usize) -> Vec<Vec<u8>> {
    out.chunks(w).map(|r| r.iter().map(|p| p[2]).collect()).collect()
}

#[test]
fn swizzled_four_by_four() {
    let out = px(PixelFormat::A8r8g8b8, 4, 4, 1, &sixteen());
    // The file stores blue in byte 0, which decodes to the B channel (index 2).
    assert_eq!(
        blue_rows(&out, 4),
        vec![
            vec![0, 1, 4, 5],
            vec![2, 3, 6, 7],
            vec![8, 9, 12, 13],
            vec![10, 11, 14, 15]
        ]
    );
    assert!(out.iter().all(|p| p[3] == 255 && p[0] == 0 && p[1] == 0));
}

#[test]
fn swizzled_eight_by_two_and_two_by_eight() {
    let out = px(PixelFormat::A8r8g8b8, 8, 2, 1, &sixteen());
    assert_eq!(
        blue_rows(&out, 8),
        vec![vec![0, 1, 4, 5, 8, 9, 12, 13], vec![2, 3, 6, 7, 10, 11, 14, 15]]
    );
    let out = px(PixelFormat::A8r8g8b8, 2, 8, 1, &sixteen());
    let want: Vec<Vec<u8>> = (0..8u8).map(|y| vec![2 * y, 2 * y + 1]).collect();
    assert_eq!(blue_rows(&out, 2), want);
}

#[test]
fn swizzled_volume_and_degenerate_levels() {
    // 2 x 2 x 2: slice z, row y, column x holds blue x + 2y + 4z.
    let out = px(PixelFormat::A8r8g8b8, 2, 2, 2, &sixteen()[..32]);
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                assert_eq!(out[z * 4 + y * 2 + x][2] as usize, x + 2 * y + 4 * z);
            }
        }
    }
    // 1 x N and N x 1 are plain order.
    for (w, h) in [(1, 16), (16, 1)] {
        let out = px(PixelFormat::A8r8g8b8, w, h, 1, &sixteen());
        let blues: Vec<u8> = out.iter().map(|p| p[2]).collect();
        assert_eq!(blues, (0..16).collect::<Vec<u8>>());
    }
    // The rule's masks: 4x4 x = 0101, y = 1010; 8x2 x = 1101, y = 0010.
    assert_eq!(swizzled_index(4, 4, 1, 3, 3, 0), 15);
    assert_eq!(swizzled_index(8, 2, 1, 7, 1, 0), 15);
    assert_eq!(swizzled_index(8, 2, 1, 0, 1, 0), 2);
    assert_eq!(swizzled_index(2, 8, 1, 1, 7, 0), 15);
    assert_eq!(swizzled_index(2, 2, 2, 1, 1, 1), 7);
}

fn blocks(format: PixelFormat, bytes: &[u8]) -> Vec<Vec<[u8; 4]>> {
    let out = decode_blocks(format, 4, 4, bytes).unwrap();
    out.chunks(16)
        .map(|r| r.chunks(4).map(|p| [p[0], p[1], p[2], p[3]]).collect())
        .collect()
}

const COLOR_PART: [u8; 8] = [0x1F, 0x00, 0x00, 0xF8, 0xE4, 0x1B, 0x50, 0xFA];

#[test]
fn dxt1_four_colour_block() {
    let got = blocks(PixelFormat::Dxt1, &[0x00, 0xF8, 0x1F, 0x00, 0xE4, 0x1B, 0x50, 0xFA]);
    let want: [[[u8; 4]; 4]; 4] = [
        [[255, 0, 0, 255], [0, 0, 255, 255], [170, 0, 85, 255], [85, 0, 170, 255]],
        [[85, 0, 170, 255], [170, 0, 85, 255], [0, 0, 255, 255], [255, 0, 0, 255]],
        [[255, 0, 0, 255], [255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255]],
        [
            [170, 0, 85, 255],
            [170, 0, 85, 255],
            [85, 0, 170, 255],
            [85, 0, 170, 255],
        ],
    ];
    assert_eq!(got, want.map(|r| r.to_vec()).to_vec());
}

#[test]
fn dxt1_three_colour_block_with_key() {
    let got = blocks(PixelFormat::Dxt1, &[0x1F, 0x00, 0x00, 0xF8, 0xE4, 0xFF, 0x00, 0x55]);
    let (b, r, m, t) = ([0, 0, 255, 255], [255, 0, 0, 255], [127, 0, 127, 255], [0, 0, 0, 0]);
    let want = vec![vec![b, r, m, t], vec![t; 4], vec![b; 4], vec![r; 4]];
    assert_eq!(got, want);
}

#[test]
fn dxt3_block() {
    let mut bytes = vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xBA, 0xDC, 0xFE];
    bytes.extend_from_slice(&COLOR_PART);
    let got = blocks(PixelFormat::Dxt3, &bytes);
    let want: [[[u8; 4]; 4]; 4] = [
        [[0, 0, 255, 0], [255, 0, 0, 17], [85, 0, 170, 34], [170, 0, 85, 51]],
        [[170, 0, 85, 68], [85, 0, 170, 85], [255, 0, 0, 102], [0, 0, 255, 119]],
        [[0, 0, 255, 136], [0, 0, 255, 153], [255, 0, 0, 170], [255, 0, 0, 187]],
        [
            [85, 0, 170, 204],
            [85, 0, 170, 221],
            [170, 0, 85, 238],
            [170, 0, 85, 255],
        ],
    ];
    assert_eq!(got, want.map(|r| r.to_vec()).to_vec());
}

#[test]
fn dxt5_blocks_eight_and_six_value_alpha() {
    let mut eight = vec![0xFF, 0x00, 0x88, 0xC6, 0xFA, 0x88, 0xC6, 0xFA];
    eight.extend_from_slice(&COLOR_PART);
    let want: [[[u8; 4]; 4]; 4] = [
        [[0, 0, 255, 255], [255, 0, 0, 0], [85, 0, 170, 218], [170, 0, 85, 182]],
        [[170, 0, 85, 145], [85, 0, 170, 109], [255, 0, 0, 72], [0, 0, 255, 36]],
        [[0, 0, 255, 255], [0, 0, 255, 0], [255, 0, 0, 218], [255, 0, 0, 182]],
        [[85, 0, 170, 145], [85, 0, 170, 109], [170, 0, 85, 72], [170, 0, 85, 36]],
    ];
    assert_eq!(blocks(PixelFormat::Dxt5, &eight), want.map(|r| r.to_vec()).to_vec());

    let mut six = vec![0x10, 0xF0, 0x88, 0xC6, 0xFA, 0x88, 0xC6, 0xFA];
    six.extend_from_slice(&COLOR_PART);
    let want: [[[u8; 4]; 4]; 4] = [
        [[0, 0, 255, 16], [255, 0, 0, 240], [85, 0, 170, 60], [170, 0, 85, 105]],
        [[170, 0, 85, 150], [85, 0, 170, 195], [255, 0, 0, 0], [0, 0, 255, 255]],
        [[0, 0, 255, 16], [0, 0, 255, 240], [255, 0, 0, 60], [255, 0, 0, 105]],
        [[85, 0, 170, 150], [85, 0, 170, 195], [170, 0, 85, 0], [170, 0, 85, 255]],
    ];
    assert_eq!(blocks(PixelFormat::Dxt5, &six), want.map(|r| r.to_vec()).to_vec());
}

#[test]
fn partial_blocks_are_clipped() {
    // A 2 x 3 level is one block; only the 6 texels inside come out.
    let out = decode_blocks(
        PixelFormat::Dxt1,
        2,
        3,
        &[0x00, 0xF8, 0x1F, 0x00, 0xE4, 0x1B, 0x50, 0xFA],
    )
    .unwrap();
    assert_eq!(out.len(), 2 * 3 * 4);
    assert_eq!(&out[0..8], [255, 0, 0, 255, 0, 0, 255, 255]);
    assert!(decode_blocks(PixelFormat::Dxt1, 4, 4, &[0; 7]).is_err());
}

#[test]
fn bump_palette_anchors_and_lookups() {
    let p = bump_palette();
    let anchors: [(usize, [u8; 4]); 10] = [
        (0, [122, 25, 204, 255]),
        (4, [133, 25, 204, 255]),
        (5, [116, 47, 226, 255]),
        (103, [122, 126, 255, 255]),
        (115, [25, 128, 204, 255]),
        (124, [128, 128, 255, 255]),
        (133, [229, 128, 204, 255]),
        (248, [133, 229, 204, 255]),
        (249, [0, 0, 0, 0]),
        (255, [128, 128, 255, 0]),
    ];
    for (i, v) in anchors {
        assert_eq!(p[i], v, "entry {i}");
    }
    assert!(p[249..255].iter().all(|e| *e == [0; 4]));
    // The blue table by (|j|, |i|) from the spec, 0 where the pair is not kept.
    #[rustfmt::skip]
    let blue: [[u8; 10]; 10] = [
        [0xFF, 0xFF, 0xFF, 0xFE, 0xFD, 0xFB, 0xF7, 0xEF, 0xE2, 0xCC],
        [0xFF, 0xFF, 0xFF, 0xFE, 0xFD, 0xFB, 0xF7, 0xEF, 0xE2, 0xCC],
        [0xFF, 0xFF, 0xFF, 0xFE, 0xFD, 0xFB, 0xF7, 0xEF, 0xE2, 0xCC],
        [0xFE, 0xFE, 0xFE, 0xFE, 0xFD, 0xFB, 0xF6, 0xEE, 0xE2, 0],
        [0xFD, 0xFD, 0xFD, 0xFD, 0xFC, 0xF9, 0xF5, 0xED, 0, 0],
        [0xFB, 0xFB, 0xFB, 0xFB, 0xF9, 0xF7, 0xF2, 0, 0, 0],
        [0xF7, 0xF7, 0xF7, 0xF6, 0xF5, 0xF2, 0, 0, 0, 0],
        [0xEF, 0xEF, 0xEF, 0xEE, 0xED, 0, 0, 0, 0, 0],
        [0xE2, 0xE2, 0xE2, 0xE2, 0, 0, 0, 0, 0, 0],
        [0xCC, 0xCC, 0xCC, 0, 0, 0, 0, 0, 0, 0],
    ];
    let mut n = 0;
    for j in -9i32..=9 {
        for i in -9i32..=9 {
            if i.abs() + j.abs() > 11 {
                continue;
            }
            assert_eq!(
                p[n][2],
                blue[j.unsigned_abs() as usize][i.unsigned_abs() as usize],
                "entry {n}"
            );
            assert_eq!(p[n][3], 255);
            n += 1;
        }
    }
    assert_eq!(n, 249);
    // Texel lookups, 2 x 1 swizzled.
    let got = px(PixelFormat::P8Bump, 2, 1, 1, &[0x00, 0x7C]);
    assert_eq!(got, [[122, 25, 204, 255], [128, 128, 255, 255]]);
    let got = px(PixelFormat::P8Bump, 2, 1, 1, &[0xF8, 0xFF]);
    assert_eq!(got, [[133, 229, 204, 255], [128, 128, 255, 0]]);
}

#[test]
fn size_model() {
    use ImageType::*;
    use PixelFormat::*;
    // (type, format, storage, w, h, mip count, pixels size)
    let cases = [
        (Texture2d, Dxt1, Storage::Blocks, 256, 256, 8, 43_776),
        (Texture2d, Dxt3, Storage::Blocks, 512, 512, 9, 349_568),
        (Texture2d, Dxt1, Storage::Blocks, 256, 512, 9, 87_424),
        (CubeMap, Dxt1, Storage::Blocks, 64, 64, 6, 16_896),
        (Texture2d, A8r8g8b8, SWZ, 32, 32, 5, 5_504),
    ];
    for (t, f, s, w, h, m, size) in cases {
        let l = Layout::new(t, f, s, w, h, 1, m).unwrap();
        assert_eq!(l.size, size, "{f:?} {w}x{h}");
    }
    let l = Layout::new(Texture2d, Dxt1, Storage::Blocks, 256, 256, 1, 8).unwrap();
    assert_eq!(l.stored_levels(), 7);
    let l = Layout::new(Texture2d, Dxt1, Storage::Blocks, 256, 512, 1, 9).unwrap();
    assert_eq!(l.stored_levels(), 8);
    assert_eq!((l.levels[7].width, l.levels[7].height), (2, 4));
    let l = Layout::new(CubeMap, Dxt1, Storage::Blocks, 64, 64, 1, 6).unwrap();
    assert_eq!((l.face_stride, l.faces), (2_816, 6));
    // A 3D 32 x 32 x 32 volume with every level stored: 6 levels.
    let l = Layout::new(Texture3d, Ay8, SWZ, 32, 32, 32, 5).unwrap();
    assert_eq!(l.stored_levels(), 6);
}

#[test]
fn invalid_layouts_are_refused() {
    use ImageType::*;
    use PixelFormat::*;
    let bad = [
        Layout::new(Texture2d, A8r8g8b8, SWZ, 4, 4, 2, 0),    // depth on 2D
        Layout::new(CubeMap, A8r8g8b8, SWZ, 4, 8, 1, 0),      // not square
        Layout::new(Texture2d, A8r8g8b8, SWZ, 4, 4, 1, 3),    // too many mips
        Layout::new(Texture2d, A8r8g8b8, SWZ, 0, 4, 1, 0),    // zero side
        Layout::new(Texture2d, A8r8g8b8, SWZ, 5000, 4, 1, 0), // too large
        Layout::new(Texture2d, A8r8g8b8, SWZ, 6, 4, 1, 0),    // swizzled, not pow2
        Layout::new(Texture3d, Dxt1, Storage::Blocks, 4, 4, 4, 0), // compressed volume
        Layout::new(Texture2d, Dxt1, Storage::Linear, 4, 4, 1, 0), // wrong storage
        Layout::new(Texture2d, A8r8g8b8, Storage::Blocks, 4, 4, 1, 0), // wrong storage
    ];
    for (i, r) in bad.iter().enumerate() {
        assert!(r.is_err(), "case {i}");
    }
    // A non-power-of-two linear image is fine (interface bitmaps).
    assert!(Layout::new(Texture2d, A4r4g4b4, Storage::Linear, 6, 5, 1, 0).is_ok());
}

// Whole tags.

/// One bitmaps-block element.
#[allow(clippy::too_many_arguments)]
fn element(w: i16, h: i16, d: i16, kind: i16, format: i16, flags: u16, mips: i16, offset: u32, size: u32) -> Vec<u8> {
    Struct::new(0x30)
        .u32(0x00, c("bitm"))
        .i16(0x04, w)
        .i16(0x06, h)
        .i16(0x08, d)
        .i16(0x0A, kind)
        .i16(0x0C, format)
        .u16(0x0E, flags)
        .i16(0x10, 3)
        .i16(0x12, -4)
        .i16(0x14, mips)
        .u16(0x16, 0x77EA)
        .u32(0x18, offset)
        .u32(0x1C, size)
        .u32(0x20, 0xE175_0001)
        .i32(0x24, -1)
        .into()
}

/// A map with the scenario and one bitmap tag holding `images` (and
/// `sequences`), the pixel bytes right after the header (file offset
/// 0x800).
fn bitmap_map(images: Vec<Vec<u8>>, sequences: Vec<Vec<u8>>, pixels: &[u8]) -> MapFile {
    bitmap_map_with(images, sequences, pixels, |s| s)
}

fn bitmap_map_with(
    images: Vec<Vec<u8>>,
    sequences: Vec<Vec<u8>>,
    pixels: &[u8],
    fixed: impl FnOnce(Struct) -> Struct,
) -> MapFile {
    let mut td = TagData::new(&[("scnr", "levels\\test\\tiny"), ("bitm", "bitmaps\\test")]);
    td.set_struct(0, &[0; 16]);
    let ib = td.block(&images);
    let sb = td.block(&sequences);
    let s = fixed(Struct::new(0x6C).put(0x54, &sb).put(0x60, &ib));
    td.set_struct(1, &s.0);
    let f = CacheFile::with_bsps("tiny", 1, pixels.to_vec(), td.finish());
    MapFile::from_image(f.image()).unwrap()
}

const OFF: u32 = HEADER as u32;

fn bitm(map: &MapFile) -> &blam_formats::cache::TagEntry {
    map.find(blam_formats::cache::class::BITMAP, "bitmaps\\test").unwrap()
}

#[test]
fn mip_chain_two_levels() {
    #[rustfmt::skip]
    let data: Vec<u8> = vec![
        0x00, 0x00, 0xFF, 0xFF, 0x00, 0xFF, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
        0x10, 0x20, 0x30, 0xFF,
    ];
    let mut pixels = data.clone();
    pixels.resize(128, 0);
    let map = bitmap_map(vec![element(2, 2, 1, 0, 11, 0x89, 1, OFF, 128)], vec![], &pixels);
    let img = read_image(&map, bitm(&map), 0).unwrap();
    let layout = img.layout().unwrap();
    assert_eq!(layout.size, 128);
    assert_eq!(layout.levels[1].offset, 16);
    let d = decode_image(&map, bitm(&map), 0).unwrap();
    assert_eq!(d.stored_levels(), 2);
    let l0 = &d.faces[0][0];
    assert_eq!((l0.width, l0.height), (2, 2));
    assert_eq!(
        l0.rgba,
        [255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255]
    );
    assert_eq!(d.faces[0][1].rgba, [48, 32, 16, 255]);
    assert_eq!(img.decode_level(&map, 0, 1).unwrap().rgba, [48, 32, 16, 255]);
    assert!(img.decode_level(&map, 0, 2).is_err());
    assert!(img.decode_level(&map, 1, 0).is_err());
    // The element's other fields.
    assert_eq!((img.registration, img.owner.0), ([3, -4], 0xE175_0001));
}

#[test]
fn compressed_chain_stores_only_whole_blocks() {
    let mut pixels = Vec::new();
    for b in [
        [0x00, 0xF8, 0x00, 0xF8],
        [0xE0, 0x07, 0xE0, 0x07],
        [0x1F, 0x00, 0x1F, 0x00],
        [0xFF, 0xFF, 0xFF, 0xFF],
        [0x10, 0x84, 0x10, 0x84],
    ] {
        pixels.extend_from_slice(&b);
        pixels.extend_from_slice(&[0; 4]);
    }
    assert_eq!(pixels.len(), 40);
    // Poison after the stored levels: it must never be decoded.
    pixels.resize(128, 0xEE);
    let map = bitmap_map(vec![element(8, 8, 1, 0, 14, 0x83, 3, OFF, 128)], vec![], &pixels);
    let img = read_image(&map, bitm(&map), 0).unwrap();
    assert_eq!(img.mipmap_count, 3);
    let d = img.decode(&map).unwrap();
    assert_eq!(d.stored_levels(), 2);
    let at = |x: usize, y: usize| {
        let i = (y * 8 + x) * 4;
        <[u8; 4]>::try_from(&d.faces[0][0].rgba[i..i + 4]).unwrap()
    };
    assert_eq!(at(0, 0), [255, 0, 0, 255]);
    assert_eq!(at(4, 0), [0, 255, 0, 255]);
    assert_eq!(at(0, 4), [0, 0, 255, 255]);
    assert_eq!(at(4, 4), [255, 255, 255, 255]);
    assert!(d.faces[0][1].rgba.chunks(4).all(|p| p == [132, 130, 132, 255]));
    // The blocks as stored, untouched.
    let b = img.blocks(&map).unwrap().unwrap();
    assert_eq!(b.format, PixelFormat::Dxt1);
    assert_eq!(b.stored_levels(), 2);
    assert_eq!((b.faces[0][0].blocks_x, b.faces[0][0].blocks_y), (2, 2));
    assert_eq!(b.faces[0][0].data, &pixels[..32]);
    assert_eq!(b.faces[0][1].data, &pixels[32..40]);
    // No blocks for an uncompressed image.
    let map = bitmap_map(vec![element(1, 1, 1, 0, 1, 0x89, 0, OFF, 128)], vec![], &[7; 128]);
    assert!(read_image(&map, bitm(&map), 0).unwrap().blocks(&map).unwrap().is_none());
}

#[test]
fn cube_map_faces_start_at_the_stride() {
    // 2 x 2 a8r8g8b8 cube, stride 128, size 768; face f is (f * 40, 0, 0, 255).
    let mut pixels = vec![0u8; 768];
    for f in 0..6 {
        for t in 0..4 {
            pixels[f * 128 + t * 4..][..4].copy_from_slice(&[0, 0, f as u8 * 40, 0xFF]);
        }
    }
    assert_eq!(&pixels[640..644], [0, 0, 200, 0xFF]);
    let map = bitmap_map(vec![element(2, 2, 1, 2, 11, 0x89, 0, OFF, 768)], vec![], &pixels);
    let img = read_image(&map, bitm(&map), 0).unwrap();
    assert_eq!((img.layout().unwrap().face_stride, img.face_count()), (128, 6));
    let d = img.decode(&map).unwrap();
    assert_eq!(d.faces.len(), 6);
    assert_eq!(&d.faces[3][0].rgba[0..4], [120, 0, 0, 255]);
    assert_eq!(&d.faces[5][0].rgba[0..4], [200, 0, 0, 255]);
    // A pixels size short of the six faces is refused.
    let map = bitmap_map(vec![element(2, 2, 1, 2, 11, 0x89, 0, OFF, 640)], vec![], &pixels);
    assert!(read_image(&map, bitm(&map), 0).unwrap().decode(&map).is_err());
}

#[test]
fn linear_interface_image() {
    // a4r4g4b4 4 x 2, linear flag: rows of 8 bytes in plain order.
    let row = [0x21, 0x84, 0x0F, 0xF0, 0x21, 0x84, 0x0F, 0xF0];
    let mut pixels = row.to_vec();
    pixels.extend_from_slice(&[0x00, 0xF0, 0x00, 0xF0, 0x00, 0xF0, 0x00, 0xF0]);
    pixels.resize(128, 0);
    let map = bitmap_map(vec![element(4, 2, 1, 0, 9, 0x91, 0, OFF, 128)], vec![], &pixels);
    let d = decode_image(&map, bitm(&map), 0).unwrap();
    let l = &d.faces[0][0];
    assert_eq!(&l.rgba[0..8], [68, 34, 17, 136, 0, 0, 255, 255]);
    assert_eq!(&l.rgba[16..20], [0, 0, 0, 255]);
}

#[test]
fn volume_slices_decode() {
    // 2 x 2 x 2 y8 volume, swizzled: texel k = value k; one level of 8 bytes.
    let mut pixels: Vec<u8> = (0..9).collect();
    pixels.resize(128, 0);
    let map = bitmap_map(vec![element(2, 2, 2, 1, 1, 0x89, 1, OFF, 128)], vec![], &pixels);
    let d = decode_image(&map, bitm(&map), 0).unwrap();
    assert_eq!(d.stored_levels(), 2);
    let l = &d.faces[0][0];
    assert_eq!((l.width, l.height, l.depth, l.rgba.len()), (2, 2, 2, 32));
    let reds: Vec<u8> = l.rgba.chunks(4).map(|p| p[0]).collect();
    assert_eq!(reds, [0, 1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(d.faces[0][1].rgba, [8, 8, 8, 255]);
}

#[test]
fn tag_fields_sequences_and_sprites() {
    let sprite = Struct::new(0x20)
        .i16(0, 1)
        .f32(0x08, 0.25)
        .f32(0x0C, 0.5)
        .f32(0x10, 0.125)
        .f32(0x14, 0.75)
        .f32(0x18, 0.1)
        .f32(0x1C, 0.2);
    // Block pointers need the heap, so build the sequences inside the map.
    let mut td = TagData::new(&[("scnr", "levels\\test\\tiny"), ("bitm", "bitmaps\\test")]);
    td.set_struct(0, &[0; 16]);
    let images = td.block(&[
        element(4, 4, 1, 0, 2, 0x89, 0, OFF, 128),
        element(4, 4, 1, 0, 2, 0x89, 0, OFF, 128),
        element(4, 4, 1, 0, 2, 0x89, 0, OFF, 128),
    ]);
    let sprites = td.block(&[sprite.0.clone(), sprite.0.clone()]);
    let none = [0u8; 12];
    let seq = |name: &str, first: i16, count: i16, sp: &[u8]| -> Vec<u8> {
        Struct::new(0x40)
            .str(0, name)
            .i16(0x20, first)
            .i16(0x22, count)
            .put(0x34, sp)
            .0
    };
    let seqs = td.block(&[
        seq("import #1", 0, 2, &none),
        seq("", 2, 1, &none),
        seq("sprites", 0, 0, &sprites),
    ]);
    let fixed = Struct::new(0x6C)
        .i16(0x00, 3)
        .i16(0x02, 1)
        .i16(0x04, 4)
        .u16(0x06, 8)
        .f32(0x08, 0.5)
        .f32(0x10, 0.004)
        .i16(0x18, 640)
        .i16(0x1A, 480)
        .f32(0x44, 2.0)
        .i16(0x4C, 3)
        .i16(0x50, 4)
        .put(0x54, &seqs)
        .put(0x60, &images);
    td.set_struct(1, &fixed.0);
    let f = CacheFile::with_bsps("tiny", 1, vec![0; 128], td.finish());
    let map = MapFile::from_image(f.image()).unwrap();
    let bm = Bitmap::read(&map, bitm(&map)).unwrap();
    assert_eq!(bm.path, "bitmaps\\test");
    assert_eq!(bm.bitmap_type, BitmapType::Sprites);
    assert_eq!(bm.usage, Usage::LightMap);
    assert_eq!((bm.flags, bm.detail_fade_factor, bm.bump_height), (8, 0.5, 0.004));
    assert_eq!((bm.color_plate_size, bm.blur_filter_size), ([640, 480], 2.0));
    assert_eq!((bm.author_mipmap_count, bm.sprite_spacing), (3, 4));
    assert_eq!(bm.images.len(), 3);
    assert_eq!(bm.sequences.len(), 3);
    assert_eq!(bm.sequences[0].name, "import #1");
    assert_eq!((bm.sequences[1].first_bitmap, bm.sequences[1].bitmap_count), (2, 1));
    let sp = &bm.sequences[2].sprites;
    assert_eq!(sp.len(), 2);
    assert_eq!(
        (
            sp[0].bitmap_index,
            sp[0].left,
            sp[0].right,
            sp[0].top,
            sp[0].bottom,
            sp[0].registration
        ),
        (1, 0.25, 0.5, 0.125, 0.75, [0.1, 0.2])
    );

    // The frame rule.
    assert_eq!(bm.frame(0, 0).map(|f| f.0), Some(0));
    assert_eq!(bm.frame(0, 3).map(|f| f.0), Some(1)); // frames wrap
    assert_eq!(bm.frame(1, 5).map(|f| f.0), Some(2));
    assert_eq!(bm.frame(4, 0).map(|f| f.0), Some(2)); // 4 mod 3 = 1
    let (page, sprite) = bm.frame(2, 1).unwrap();
    assert_eq!((page, sprite.is_some()), (1, true));
    assert_eq!(bm.frame(2, 2).map(|f| f.0), Some(2)); // no such sprite: the frame is the index
    assert_eq!(bm.frame(2, 7), None); // outside the images
    assert_eq!(bm.frame(-1, 1).map(|f| f.0), Some(1));
    assert_eq!(bm.frame(0, -1), None);
}

#[test]
fn bad_tags_are_refused_with_context() {
    let ok = element(4, 4, 1, 0, 2, 0x89, 0, OFF, 128);
    let refused = |images: Vec<Vec<u8>>| -> String {
        let map = bitmap_map(images, vec![], &[0; 128]);
        Bitmap::read(&map, bitm(&map)).unwrap_err().to_string()
    };
    for (el, needle) in [
        (element(4, 4, 1, 0, 4, 0x89, 0, OFF, 128), "pixel format 4"),
        (element(4, 4, 1, 0, 12, 0x89, 0, OFF, 128), "pixel format 12"),
        (element(4, 4, 1, 5, 2, 0x89, 0, OFF, 128), "image type 5"),
        (element(4, 4, 1, 0, 2, 0x89, 9, OFF, 128), "mipmap count"),
        (element(-4, 4, 1, 0, 2, 0x89, 0, OFF, 128), "width"),
        (element(4, 4, 2, 0, 2, 0x89, 0, OFF, 128), "depth 2"),
    ] {
        let e = refused(vec![ok.clone(), el]);
        assert!(e.contains(needle), "{e}");
        assert!(e.contains("bitm `bitmaps\\test`"), "{e}");
    }
    let mut bad_sig = ok.clone();
    bad_sig[0] ^= 1;
    assert!(refused(vec![bad_sig]).contains("signature"));

    // Pixels past the image, not a bitmap, an image index past the block.
    let map = bitmap_map(vec![element(4, 4, 1, 0, 2, 0x89, 0, 0x10_0000, 128)], vec![], &[0; 128]);
    let img = read_image(&map, bitm(&map), 0).unwrap();
    assert!(img.pixels(&map).unwrap_err().to_string().contains("past the image"));
    assert!(read_image(&map, bitm(&map), 1).is_err());
    assert!(read_image(&map, map.scenario(), 0).is_err());
    // A pixels size too small for the levels.
    let map = bitmap_map(vec![element(4, 4, 1, 0, 11, 0x89, 0, OFF, 32)], vec![], &[0; 128]);
    assert!(read_image(&map, bitm(&map), 0).unwrap().pixels(&map).is_err());
}

#[test]
fn decoding_is_thread_safe() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<MapFile>();
    assert_send_sync::<Bitmap>();
    assert_send_sync::<blam_formats::bitmap::DecodedImage>();
    let mut pixels = sixteen();
    pixels.resize(128, 0);
    let map = bitmap_map(vec![element(4, 4, 1, 0, 11, 0x89, 0, OFF, 128)], vec![], &pixels);
    let want = decode_image(&map, bitm(&map), 0).unwrap();
    std::thread::scope(|s| {
        let hs: Vec<_> = (0..4)
            .map(|_| s.spawn(|| decode_image(&map.clone(), bitm(&map), 0).unwrap()))
            .collect();
        for h in hs {
            assert_eq!(h.join().unwrap(), want);
        }
    });
}
