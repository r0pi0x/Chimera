//! The bitmap reader and decoders on the user's Halo disc image, against
//! section (b) of specs/halo_ce/bitmaps.md: the named bloodgulch bitmaps
//! (sizes, offsets, stated pixel values), the lightmap pages of both maps,
//! and a sweep that decodes every bitmap image of bloodgulch.map and
//! hangemhigh.map. Each test passes with a note when there is no image.

use std::{collections::BTreeMap, sync::OnceLock, time::Instant};

use blam_formats::bitmap::{Bitmap, BitmapType, DecodedImage, ImageType, PixelFormat, Usage};
use blam_formats::cache::{MapFile, class};
use blam_formats::disc::DiscImage;

use super::disc_install::{SKIP, image_path};

fn load(name: &str) -> Option<MapFile> {
    let path = image_path().or_else(|| {
        eprintln!("{SKIP}");
        None
    })?;
    let disc = DiscImage::open(&path).unwrap();
    Some(MapFile::load(&disc.read_file(&format!("maps/{name}.map")).unwrap()).unwrap())
}

fn bloodgulch() -> Option<&'static MapFile> {
    static MAP: OnceLock<Option<MapFile>> = OnceLock::new();
    MAP.get_or_init(|| load("bloodgulch")).as_ref()
}

fn tag(map: &MapFile, path: &str) -> Bitmap {
    let t = map
        .find(class::BITMAP, path)
        .unwrap_or_else(|| panic!("no bitmap `{path}`"));
    Bitmap::read(map, t).unwrap()
}

fn texel(d: &DecodedImage, face: usize, level: usize, x: usize, y: usize) -> [u8; 4] {
    let l = &d.faces[face][level];
    let i = (y * l.width as usize + x) * 4;
    l.rgba[i..i + 4].try_into().unwrap()
}

fn near(got: [u8; 4], want: [u8; 4], tol: u8, what: &str) {
    for k in 0..4 {
        assert!(got[k].abs_diff(want[k]) <= tol, "{what}: got {got:?}, want {want:?}");
    }
}

#[test]
fn blood_ground_dxt3() {
    let Some(map) = bloodgulch() else { return };
    let b = tag(map, "levels\\test\\bloodgulch\\bitmaps\\blood ground");
    assert_eq!(b.images.len(), 1);
    let i = &b.images[0];
    assert_eq!(
        (i.width, i.height, i.format, i.mipmap_count),
        (512, 512, PixelFormat::Dxt3, 9)
    );
    assert_eq!((i.flags.0, i.pixels_offset, i.pixels_size), (0x83, 0x15A_1A00, 0x55580));
    let d = b.decode_image(map, 0).unwrap();
    assert_eq!(d.stored_levels(), 8);
    near(texel(&d, 0, 0, 0, 0), [107, 93, 74, 221], 1, "(0,0)");
    near(texel(&d, 0, 0, 200, 300), [132, 101, 66, 85], 1, "(200,300)");
    near(texel(&d, 0, 0, 256, 256), [198, 134, 82, 255], 1, "(256,256)");
    near(texel(&d, 0, 0, 511, 511), [101, 87, 68, 221], 1, "(511,511)");
    near(texel(&d, 0, 1, 0, 0), [99, 85, 66, 238], 1, "level 1 (0,0)");
    near(texel(&d, 0, 1, 128, 128), [198, 138, 82, 255], 1, "level 1 (128,128)");
    // The stored blocks as the GPU takes them.
    let blocks = i.blocks(map).unwrap().unwrap();
    assert_eq!(blocks.stored_levels(), 8);
    assert_eq!(blocks.faces[0][0].data.len(), 128 * 128 * 16);
}

#[test]
fn blood_cliff_rock_dxt1() {
    let Some(map) = bloodgulch() else { return };
    let b = tag(map, "levels\\test\\bloodgulch\\bitmaps\\blood cliff rock");
    let i = &b.images[0];
    assert_eq!(
        (i.width, i.height, i.format, i.mipmap_count),
        (256, 512, PixelFormat::Dxt1, 9)
    );
    assert_eq!((i.flags.0, i.pixels_offset, i.pixels_size), (0x83, 0x14D_2800, 0x15580));
    let d = b.decode_image(map, 0).unwrap();
    assert_eq!(d.stored_levels(), 8);
    let last = d.faces[0].last().unwrap();
    assert_eq!((last.width, last.height), (2, 4));
    near(texel(&d, 0, 0, 0, 0), [118, 65, 38, 255], 1, "(0,0)");
    near(texel(&d, 0, 0, 130, 77), [104, 52, 29, 255], 1, "(130,77)");
    near(texel(&d, 0, 0, 255, 511), [107, 65, 41, 255], 1, "(255,511)");
    assert_eq!((d.faces[0][1].width, d.faces[0][1].height), (128, 256));
    near(texel(&d, 0, 1, 0, 0), [132, 77, 41, 255], 1, "level 1 (0,0)");
}

/// The (width, height) -> count table of a lightmap tag's pages.
fn page_sizes(b: &Bitmap) -> BTreeMap<(u16, u16), usize> {
    let mut m = BTreeMap::new();
    for i in &b.images {
        *m.entry((i.width, i.height)).or_insert(0) += 1;
    }
    m
}

fn sizes(list: &[((u16, u16), usize)]) -> BTreeMap<(u16, u16), usize> {
    list.iter().copied().collect()
}

#[test]
fn bloodgulch_lightmap_pages() {
    let Some(map) = bloodgulch() else { return };
    let b = tag(map, "levels\\test\\bloodgulch\\bloodgulch");
    assert_eq!((b.usage, b.bitmap_type), (Usage::LightMap, BitmapType::Textures2d));
    assert_eq!(b.images.len(), 15);
    assert_eq!(
        page_sizes(&b),
        sizes(&[
            ((256, 128), 1),
            ((128, 256), 1),
            ((128, 128), 2),
            ((128, 64), 1),
            ((64, 128), 1),
            ((64, 64), 1),
            ((64, 32), 2),
            ((32, 32), 1),
            ((32, 16), 3),
            ((16, 32), 1),
            ((8, 8), 1)
        ])
    );
    assert_eq!(b.images.iter().map(|i| u64::from(i.pixels_size)).sum::<u64>(), 252_032);
    for i in &b.images {
        assert_eq!((i.format, i.flags.0, i.mipmap_count), (PixelFormat::R5g6b5, 0x89, 0));
        assert_eq!(i.pixels_size, u32::from(i.width) * u32::from(i.height) * 2);
    }
    let (p0, p1) = (&b.images[0], &b.images[1]);
    assert_eq!(
        (p0.width, p0.height, p0.pixels_offset, p0.pixels_size),
        (256, 128, 0x149_4E00, 0x10000)
    );
    assert_eq!(
        (p1.width, p1.height, p1.pixels_offset, p1.pixels_size),
        (128, 256, 0x14A_4E00, 0x10000)
    );
    let d0 = b.decode_image(map, 0).unwrap();
    near(texel(&d0, 0, 0, 0, 0), [222, 207, 181, 255], 0, "page 0 (0,0)");
    near(texel(&d0, 0, 0, 100, 60), [255, 247, 222, 255], 0, "page 0 (100,60)");
    near(texel(&d0, 0, 0, 255, 127), [255, 255, 0, 255], 0, "page 0 filler");
    let d1 = b.decode_image(map, 1).unwrap();
    near(texel(&d1, 0, 0, 0, 0), [255, 239, 214, 255], 0, "page 1 (0,0)");
    near(texel(&d1, 0, 0, 60, 100), [41, 44, 33, 255], 0, "page 1 (60,100)");
}

#[test]
fn hangemhigh_lightmap_pages() {
    let Some(map) = load("hangemhigh") else { return };
    let b = tag(&map, "levels\\test\\hangemhigh\\hangemhigh");
    assert_eq!(b.usage, Usage::LightMap);
    assert_eq!(page_sizes(&b), sizes(&[((256, 256), 1), ((512, 512), 1)]));
    assert_eq!(b.images.iter().map(|i| u64::from(i.pixels_size)).sum::<u64>(), 655_360);
}

#[test]
fn metal_flat_generic_bump() {
    let Some(map) = bloodgulch() else { return };
    let b = tag(map, "levels\\b30\\bitmaps\\metal flat generic bump");
    let i = &b.images[0];
    assert_eq!(
        (i.width, i.height, i.format, i.mipmap_count),
        (256, 256, PixelFormat::P8Bump, 8)
    );
    assert_eq!((i.flags.0, i.pixels_offset, i.pixels_size), (0x8D, 0x41_CC00, 0x15580));
    let d = b.decode_image(map, 0).unwrap();
    assert_eq!(texel(&d, 0, 0, 0, 0), [128, 128, 255, 255]);
    assert_eq!(texel(&d, 0, 0, 188, 79), [122, 126, 255, 255]);
    let flat = d.faces[0][0]
        .rgba
        .chunks(4)
        .filter(|p| *p == [128, 128, 255, 255])
        .count();
    assert_eq!(flat, 23_043);
    assert_eq!(d.stored_levels(), 9);
}

#[test]
fn vector_normalization_cube() {
    let Some(map) = bloodgulch() else { return };
    let b = tag(map, "rasterizer\\vector normalization");
    assert_eq!(b.images.len(), 2);
    let (i0, i1) = (&b.images[0], &b.images[1]);
    assert_eq!(
        (i0.image_type, i0.width, i0.height, i0.format),
        (ImageType::CubeMap, 64, 64, PixelFormat::X8r8g8b8)
    );
    assert_eq!(
        (i0.mipmap_count, i0.flags.0, i0.pixels_offset, i0.pixels_size),
        (6, 0x89, 0xFD_A000, 0x20100)
    );
    assert_eq!(
        (i1.image_type, i1.width, i1.height, i1.mipmap_count),
        (ImageType::CubeMap, 4, 4, 2)
    );
    assert_eq!((i1.pixels_offset, i1.pixels_size), (0xFF_A200, 0x300));
    let d = b.decode_image(map, 0).unwrap();
    assert_eq!(d.faces.len(), 6);
    let centre = [
        [255, 127, 127],
        [1, 127, 129],
        [129, 255, 129],
        [129, 1, 127],
        [129, 127, 255],
        [127, 127, 1],
    ];
    let corner = [
        [202, 201, 201],
        [54, 201, 55],
        [55, 202, 55],
        [55, 54, 201],
        [55, 201, 202],
        [201, 201, 54],
    ];
    for f in 0..6 {
        let c = texel(&d, f, 0, 32, 32);
        assert_eq!(c[..3], centre[f], "face {f} centre");
        let k = texel(&d, f, 0, 0, 0);
        assert_eq!(k[..3], corner[f], "face {f} (0,0)");
    }
    assert_eq!(texel(&d, 0, 0, 63, 63)[..3], [202, 55, 55]);
}

/// Every image of every bitmap tag of a map: layout, offsets, decode.
fn sweep(map: &MapFile, images: usize, tags: Option<usize>) {
    let t0 = Instant::now();
    let mut spans = Vec::new();
    let (mut n_images, mut n_tags, mut bytes) = (0usize, 0usize, 0usize);
    for t in map.of_class(class::BITMAP) {
        n_tags += 1;
        let b = Bitmap::read(map, t).unwrap();
        // The colour plate and pixel data references are stripped.
        let v = map.tag_view(t, 0x6C).unwrap();
        for off in [0x1C, 0x30] {
            let r = v.data_ref(off).unwrap();
            assert_eq!((r.flags, r.address), (0, 0), "{} data ref {off:#x}", t.path);
        }
        for (k, i) in b.images.iter().enumerate() {
            let what = format!("{} image {k}", t.path);
            let layout = i.layout().unwrap();
            assert_eq!(layout.size, i.pixels_size as usize, "{what}: size model");
            assert_eq!(i.pixels_offset % 512, 0, "{what}: offset");
            assert!(
                i.pixels_offset as usize + i.pixels_size as usize <= map.image().len(),
                "{what}"
            );
            spans.push((i.pixels_offset, i.pixels_size, what.clone()));
            let d = b.decode_image(map, k).unwrap();
            assert_eq!(d.faces.len(), layout.faces, "{what}");
            for face in &d.faces {
                assert_eq!(face.len(), layout.stored_levels(), "{what}");
                for (l, lv) in face.iter().enumerate() {
                    let want = (lv.width * lv.height * lv.depth * 4) as usize;
                    assert_eq!(lv.rgba.len(), want, "{what} level {l}");
                    assert_eq!(lv.width, (u32::from(i.width) >> l).max(1), "{what} level {l}");
                    bytes += want;
                }
            }
            if let Some(blocks) = i.blocks(map).unwrap() {
                assert_eq!(blocks.stored_levels(), layout.stored_levels(), "{what}");
            }
            n_images += 1;
        }
    }
    spans.sort();
    for w in spans.windows(2) {
        assert!(
            u64::from(w[0].0) + u64::from(w[0].1) <= u64::from(w[1].0),
            "{} overlaps {}",
            w[0].2,
            w[1].2
        );
    }
    eprintln!(
        "{}: {n_images} images in {n_tags} bitmap tags, {bytes} RGBA bytes, swept in {:?}",
        map.header().name,
        t0.elapsed()
    );
    assert_eq!(n_images, images);
    if let Some(t) = tags {
        assert_eq!(n_tags, t);
    }
}

#[test]
fn sweep_bloodgulch() {
    let Some(map) = bloodgulch() else { return };
    sweep(map, 516, Some(368));
}

#[test]
fn sweep_hangemhigh() {
    let Some(map) = load("hangemhigh") else { return };
    sweep(&map, 462, None);
}
