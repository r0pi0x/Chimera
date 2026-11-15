//! The map cache reader on synthetic maps: every row of test case (a) of
//! specs/halo_ce/cache_file.md (results and rejections), then the parts
//! the spec's file leaves out (BSP spaces, model buffers, external data,
//! every primitive type, block caps, the `tag_struct!` layer) on maps
//! built with `map_fixture`.

use std::io;

use blam_formats::cache::{
    Argb8, Bounds, ClassCode, Euler2d, Euler3d, MapFile, MapType, Plane2d, Plane3d, RealAhsv, RealArgb, RealHsv,
    RealRect2d, RealRect3d, RealRgb, Rect2d, Rgb8, SpaceId, TAG_BASE, TAG_CACHE_END, TagId, TagStruct, class,
};

use crate::map_fixture::{
    Bsp, CacheFile, HEADER, Struct, TagData, bsp_element, c, data_ref_bytes, file_from, null_ref, spec_a_tag_data,
};

fn spec_a() -> CacheFile {
    let mut f = CacheFile::new("tiny", 1, spec_a_tag_data());
    f.header = f.header.clone().u32(0x064, 0xDEAD_BEEF);
    f
}

fn load(f: &CacheFile) -> io::Result<MapFile> {
    MapFile::load(&f.file())
}

fn rejected(r: io::Result<MapFile>, needle: &str) {
    let e = r.expect_err("should be refused").to_string();
    assert!(e.contains(needle), "error {e:?} lacks {needle:?}");
}

/// The tag data's byte at relative offset `off`, changed.
fn with_tag_u32(off: usize, v: u32) -> CacheFile {
    let mut t = spec_a_tag_data();
    t[off..off + 4].copy_from_slice(&v.to_le_bytes());
    CacheFile::new("tiny", 1, t)
}

fn check_spec_a(map: &MapFile) {
    let h = map.header();
    assert_eq!(h.name, "tiny");
    assert_eq!(h.build, "01.10.12.2276");
    assert_eq!(h.map_type, MapType::Multiplayer);
    assert_eq!(h.checksum, 0xDEAD_BEEF);
    assert_eq!(h.decompressed_size, 0x900);
    assert_eq!(h.version, 5);
    assert_eq!((h.tag_data_offset, h.tag_data_size), (0x800, 0x100));
    let th = map.tag_header();
    assert_eq!(th.tag_count, 2);
    assert_eq!(th.checksum, 0x1234_5678);
    assert_eq!(map.tags().len(), 2);

    let t0 = &map.tags()[0];
    assert_eq!(t0.class, class::SCENARIO);
    assert_eq!((t0.parent, t0.grandparent), (None, None));
    assert_eq!(t0.id, TagId(0xE174_0000));
    assert_eq!(t0.path, "scenarios\\tiny");
    assert_eq!(t0.address, 0x803A_6090);
    assert_eq!(map.scenario(), t0);

    let t1 = &map.tags()[1];
    assert_eq!(t1.class, ClassCode::new(b"weap"));
    assert_eq!((t1.parent, t1.grandparent), (Some(class::ITEM), Some(class::OBJECT)));
    assert_eq!(t1.id, TagId(0xE175_0001));
    assert_eq!(t1.path, "weapons\\gun");
    assert!(t1.is_a(class::ITEM) && t1.is_a(class::OBJECT) && t1.is_a(class::WEAPON));
    assert!(!t1.is_a(class::UNIT));

    assert_eq!(map.find(class::WEAPON, "WEAPONS\\GUN"), Some(t1));
    assert_eq!(map.find(class::SCENARIO, "weapons\\gun"), None);
    assert_eq!(map.tag_space().offset_of(0x803A_60E8, 1), Some(0x8E8));

    let s = map.tag_view(t0, 0x30).unwrap();
    let block = s.block(0x00, 8).unwrap();
    assert_eq!(block.len(), 2);
    let elems: Vec<_> = block
        .iter()
        .map(|e| (e.i16(0).unwrap(), e.i16(2).unwrap(), e.f32(4).unwrap()))
        .collect();
    assert_eq!(elems, [(1, -1, 2.0), (2, 0, -0.5)]);
    assert_eq!(block.get(0).unwrap().index16(2).unwrap(), None);
    assert_eq!(block.get(1).unwrap().index16(2).unwrap(), Some(0));

    let r = s.tag_ref(0x0C).unwrap();
    assert_eq!(r.class, class::WEAPON);
    assert_eq!(r.path_address, 0x803A_6073);
    let target = s.reference(0x0C, class::ITEM).unwrap().unwrap();
    assert_eq!(target, t1);
    assert_eq!(target.path, "weapons\\gun");
    assert_eq!(s.data(0x1C).unwrap(), [1, 2, 3, 4, 5, 6, 7, 8]);

    let w = map.tag_view(t1, 0x14).unwrap();
    assert_eq!(w.f32(0).unwrap(), 1.5);
    let null = w.tag_ref(4).unwrap();
    assert!(null.is_null());
    assert_eq!(null.class, class::PROJECTILE);
    assert_eq!(w.reference(4, class::PROJECTILE).unwrap(), None);
    assert_eq!(map.tag(null.id), None);
}

#[test]
fn spec_a_parses() {
    let f = spec_a();
    let file = f.file();
    assert_eq!(file.len() % 2048, 0);
    let map = MapFile::load(&file).unwrap();
    check_spec_a(&map);
    // The scenario is far too small for a BSP list: the map loads, the
    // list is refused.
    assert!(map.bsps().is_err());
}

#[test]
fn spec_a_from_the_image() {
    let image = spec_a().image();
    assert_eq!(image.len(), 0x900);
    check_spec_a(&MapFile::from_image(image).unwrap());
}

#[test]
fn rejects_a_bad_head_magic() {
    let mut f = spec_a();
    f.header.0[0] ^= 1;
    rejected(load(&f), "header magic");
}

#[test]
fn rejects_a_bad_foot_magic() {
    let mut f = spec_a();
    f.header.0[0x7FC] ^= 1;
    rejected(load(&f), "footer magic");
}

#[test]
fn rejects_version_7() {
    let mut f = spec_a();
    f.header = f.header.clone().u32(4, 7);
    rejected(load(&f), "cache version 7");
}

#[test]
fn rejects_a_missing_padding_byte() {
    let f = spec_a();
    let mut file = f.file();
    file.pop();
    rejected(MapFile::load(&file), "padding");
    // And the header's P one too small with the bytes present.
    let p = u32::from_le_bytes(file[0xC..0x10].try_into().unwrap());
    let mut file = f.file();
    file[0xC..0x10].copy_from_slice(&(p - 1).to_le_bytes());
    rejected(MapFile::load(&file), "padding");
}

#[test]
fn rejects_nonzero_padding() {
    let mut file = spec_a().file();
    *file.last_mut().unwrap() = 1;
    rejected(MapFile::load(&file), "not zero");
}

#[test]
fn rejects_a_truncated_stream() {
    let f = spec_a();
    let stream = f.stream();
    let short = &stream[..stream.len() - 1];
    let p = (2048 - (HEADER + short.len()) % 2048) % 2048;
    let file = file_from(&f.header.clone().u32(0xC, p as u32), short, p);
    assert!(MapFile::load(&file).is_err());
    // With no padding at all the inflater runs out of input.
    let file = file_from(&f.header.clone().u32(0xC, 0), short, 0);
    rejected(MapFile::load(&file), "zlib stream");
}

#[test]
fn rejects_a_body_of_the_wrong_size() {
    let mut f = spec_a();
    f.header = f.header.clone().u32(0x8, 0x8FC);
    assert!(load(&f).is_err());
    let mut f = spec_a();
    f.header = f.header.clone().u32(0x8, 0x904);
    rejected(load(&f), "inflates to");
}

#[test]
fn rejects_tag_data_beyond_the_image() {
    let mut f = spec_a();
    f.header = f.header.clone().u32(0x14, 0x200);
    rejected(load(&f), "beyond the image");
    let mut f = spec_a();
    f.header = f.header.clone().u32(0x10, 0x10);
    rejected(load(&f), "inside the header");
}

#[test]
fn rejects_a_name_without_nul() {
    let mut f = spec_a();
    f.header = f.header.clone().put(0x20, &[b'x'; 32]);
    rejected(load(&f), "name has no NUL");
}

#[test]
fn rejects_a_wrong_base() {
    rejected(load(&with_tag_u32(0x00, 0x803A_6028)), "base");
}

#[test]
fn rejects_a_bad_tags_magic() {
    rejected(load(&with_tag_u32(0x20, 0x7461_6774)), "tag header magic");
}

#[test]
fn rejects_an_index_entry_naming_another_index() {
    rejected(load(&with_tag_u32(0x44 + 0xC, 0xE175_0000)), "names entry 0");
}

#[test]
fn rejects_a_scenario_id_not_entry_0() {
    rejected(load(&with_tag_u32(0x04, 0xE175_0001)), "scenario id");
}

#[test]
fn rejects_a_pc_class() {
    rejected(load(&with_tag_u32(0x44, c("mod2"))), "not a class");
}

#[test]
fn rejects_a_path_outside_the_tag_data() {
    rejected(load(&with_tag_u32(0x44 + 0x10, 0x803A_6100)), "outside the tag data");
}

#[test]
fn a_salt_mismatch_resolves_to_nothing() {
    let map = load(&with_tag_u32(0x44 + 0xC, 0xE176_0001)).unwrap();
    let s = map.tag_view(map.scenario(), 0x30).unwrap();
    let r = s.tag_ref(0x0C).unwrap();
    assert!(!r.is_null());
    assert_eq!(map.tag(r.id), None);
    assert_eq!(s.reference(0x0C, class::WEAPON).unwrap(), None);
    assert!(map.tag(TagId(0xE176_0001)).is_some());
    assert_eq!(map.tag(TagId(0xE176_0002)), None, "index past the count");
    assert_eq!(map.tag(TagId::NONE), None);
}

#[test]
fn a_block_outside_the_tag_data_is_refused() {
    let map = load(&with_tag_u32(0x90, 6)).unwrap();
    let s = map.tag_view(map.scenario(), 0x30).unwrap();
    let e = s.block(0, 8).unwrap_err().to_string();
    assert!(e.contains("outside the tag data"), "{e}");
    assert!(e.contains("scnr `scenarios\\tiny`"), "names the tag: {e}");
    assert!(e.contains("0x803a6090+0x0"), "names the place: {e}");
}

#[test]
fn a_negative_block_count_is_refused() {
    let map = load(&with_tag_u32(0x90, 0xFFFF_FFFF)).unwrap();
    let s = map.tag_view(map.scenario(), 0x30).unwrap();
    assert!(s.block(0, 8).unwrap_err().to_string().contains("negative block count"));
}

#[test]
fn a_block_count_over_the_cap_is_refused() {
    let map = load(&spec_a()).unwrap();
    let s = map.tag_view(map.scenario(), 0x30).unwrap();
    let e = s.block_max(0, 8, 1).unwrap_err().to_string();
    assert!(e.contains("over the maximum 1"), "{e}");
    let map = load(&with_tag_u32(0x90, 0x7FFF_FFFF)).unwrap();
    let s = map.tag_view(map.scenario(), 0x30).unwrap();
    assert!(s.block(0, 1).unwrap_err().to_string().contains("over the maximum"));
}

#[test]
fn reads_past_a_view_are_errors_not_panics() {
    let map = load(&spec_a()).unwrap();
    let w = map.tag_view(&map.tags()[1], 0x14).unwrap();
    assert!(w.u32(0x12).is_err());
    assert!(w.f32(usize::MAX - 1).is_err());
    assert!(w.tag_ref(0x8).is_err());
    assert!(w.sub(0x10, 8).is_err());
    // A view itself must lie in the space.
    assert!(map.tag_view(&map.tags()[1], 0x41).is_err());
    assert!(map.view(SpaceId::Tags, TAG_BASE - 4, 8).is_err());
    assert!(map.view(SpaceId::Tags, 0, 4).is_err());
    assert!(map.view(SpaceId::Bsp(0), TAG_CACHE_END - 4, 4).is_err());
    // The weapon's reference expects a projectile: a weapon is refused.
    let s = map.tag_view(map.scenario(), 0x30).unwrap();
    let e = s.reference(0x0C, class::PROJECTILE).unwrap_err().to_string();
    assert!(e.contains("expected `proj`"), "{e}");
    assert!(s.block(0, 8).unwrap().get(2).is_err());
}

#[test]
fn map_file_is_send_sync_and_shared() {
    fn assert_send_sync<T: Send + Sync + Clone>() {}
    assert_send_sync::<MapFile>();
    let map = load(&spec_a()).unwrap();
    let other = map.clone();
    assert!(std::ptr::eq(map.image(), other.image()));
    std::thread::scope(|s| {
        s.spawn(|| assert_eq!(other.tags().len(), 2));
    });
}

// Maps built with the fixture builder.

blam_formats::tag_struct! {
    /// A block element of the example class.
    pub struct Point: 0x10 {
        0x00 position: [f32; 3],
        0x0C kind: i16,
    }
}

blam_formats::tag_struct! {
    /// An example class's main struct, as a class reader would declare it.
    pub struct Example: 0x40 {
        0x00 name: String,
        0x20 speed: f32,
        0x24 points: Vec<Point>,
        0x30 projectile: blam_formats::cache::TagRef,
    }
}

/// A scenario holding an example struct, and the projectile it names.
fn example_map() -> MapFile {
    let mut td = TagData::new(&[
        ("scnr", "levels\\example\\example"),
        ("weap", "weapons\\gun"),
        ("proj", "weapons\\gun\\bullet"),
    ]);
    td.set_struct(2, &Struct::new(4).f32(0, 9.0).0);
    let points: Vec<Vec<u8>> = (0..3)
        .map(|i| Struct::new(0x10).f32s(0, &[i as f32, 1.0, 2.0]).i16(0xC, i).into())
        .collect();
    let block = td.block(&points);
    let proj = td.tag_ref(2);
    td.set_struct(
        1,
        &Struct::new(0x40)
            .str(0, "gun")
            .f32(0x20, 3.5)
            .put(0x24, &block)
            .put(0x30, &proj)
            .0,
    );
    td.set_struct(0, &Struct::new(0x10).put(0, &td.tag_ref(1)).0);
    MapFile::load(&CacheFile::new("example", 1, td.finish()).file()).unwrap()
}

#[test]
fn a_class_reader_by_hand() {
    let map = example_map();
    let gun = map.find(class::WEAPON, "weapons\\gun").unwrap();
    let v = map.tag_view(gun, 0x40).unwrap();
    assert_eq!(v.tag_string(0).unwrap(), "gun");
    assert_eq!(v.f32(0x20).unwrap(), 3.5);
    let points = v.block(0x24, 0x10).unwrap();
    let kinds: Vec<i16> = points.iter().map(|p| p.i16(0xC)).collect::<io::Result<_>>().unwrap();
    assert_eq!(kinds, [0, 1, 2]);
    let bullet = v.reference(0x30, class::PROJECTILE).unwrap().unwrap();
    assert_eq!(bullet.path, "weapons\\gun\\bullet");
    assert_eq!(map.tag_view(bullet, 4).unwrap().f32(0).unwrap(), 9.0);
}

#[test]
fn a_class_reader_by_tag_struct() {
    let map = example_map();
    let gun = map.find(class::WEAPON, "WEAPONS\\Gun").unwrap();
    let ex: Example = map.read_tag(gun).unwrap();
    assert_eq!(ex.name, "gun");
    assert_eq!(ex.speed, 3.5);
    assert_eq!(ex.points.len(), 3);
    assert_eq!(ex.points[2].position, [2.0, 1.0, 2.0]);
    assert_eq!(ex.points[1].kind, 1);
    assert_eq!(map.tag(ex.projectile.id).unwrap().class, class::PROJECTILE);
    assert_eq!(Example::SIZE, 0x40);
    // The scenario's reference, through a generic `reference`.
    let s = map.tag_view(map.scenario(), 0x10).unwrap();
    assert_eq!(s.reference(0, class::OBJECT).unwrap(), Some(gun));
    // Reading elements at the wrong size is refused.
    let v = map.tag_view(gun, 0x40).unwrap();
    assert!(v.block(0x24, 8).unwrap().read_all::<Point>().is_err());
}

#[test]
fn lookups_by_class_and_kind() {
    let map = example_map();
    let objects: Vec<_> = map.of_kind(class::OBJECT).map(|t| t.path.as_str()).collect();
    assert_eq!(objects, ["weapons\\gun", "weapons\\gun\\bullet"]);
    assert_eq!(map.of_class(class::ITEM).count(), 0);
    assert_eq!(map.of_kind(class::ITEM).count(), 1);
    assert_eq!(map.of_class(class::PROJECTILE).count(), 1);
    assert!(map.find(class::PROJECTILE, "weapons\\gun").is_none());
}

#[test]
fn every_primitive_type() {
    let mut td = TagData::new(&[("scnr", "s")]);
    let s = Struct::new(0x100)
        .u8(0, 0xFE)
        .i16(2, -2)
        .u16(4, 0xFFFE)
        .i32(8, -3)
        .str(0x0C, "abc")
        .u32(0x2C, c("bitm"))
        .i16(0x30, 3)
        .i16(0x32, -4)
        .f32s(0x34, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0])
        .put(0x4C, &[0x10, 0x20, 0x30, 0x40])
        .put(0x50, &[0x11, 0x22, 0x33, 0x44])
        .i16(0x54, -1)
        .i16(0x56, -2)
        .u8(0x58, 0xFF)
        .i32(0x5C, -1)
        .str(0x60, "abcdefgh")
        .str(0xE0, &"x".repeat(32));
    let at = td.set_struct(0, &s.0);
    let map = MapFile::from_image(CacheFile::new("prims", 0, td.finish()).image()).unwrap();
    let v = map.tag_view(map.scenario(), 0x100).unwrap();
    assert_eq!(v.addr(), at);
    assert_eq!(v.file_offset(), HEADER + (at - TAG_BASE) as usize);
    assert_eq!((v.u8(0).unwrap(), v.i8(0).unwrap()), (0xFE, -2));
    assert_eq!(
        (v.i16(2).unwrap(), v.u16(4).unwrap(), v.i32(8).unwrap()),
        (-2, 0xFFFE, -3)
    );
    assert_eq!(v.enum16(2).unwrap(), -2);
    assert!(v.enum16_below(0x30, 3).is_err());
    assert_eq!(v.enum16_below(0x30, 4).unwrap(), 3);
    assert_eq!((v.flags8(0).unwrap(), v.flags16(4).unwrap()), (0xFE, 0xFFFE));
    assert_eq!(v.tag_string(0x0C).unwrap(), "abc");
    assert_eq!(v.class(0x2C).unwrap(), class::BITMAP);
    assert_eq!(v.class(0x2C).unwrap().to_string(), "bitm");
    assert_eq!(v.point2d(0x30).unwrap(), [3, -4]);
    assert_eq!(v.short_bounds(0x30).unwrap(), Bounds { lower: 3, upper: -4 });
    assert_eq!(
        v.rect2d(0x30).unwrap(),
        Rect2d {
            top: 3,
            left: -4,
            bottom: 0,
            // The high half of 1.0f32 at 0x34.
            right: 0x3F80
        }
    );
    let f = 0x34;
    assert_eq!(v.point2(f).unwrap(), [1.0, 2.0]);
    assert_eq!(v.point3(f).unwrap(), [1.0, 2.0, 3.0]);
    assert_eq!(v.vector3(f).unwrap(), [1.0, 2.0, 3.0]);
    assert_eq!(v.vector2(f).unwrap(), [1.0, 2.0]);
    assert_eq!(v.quaternion(f).unwrap(), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(v.euler2d(f).unwrap(), Euler2d { yaw: 1.0, pitch: 2.0 });
    assert_eq!(
        v.euler3d(f).unwrap(),
        Euler3d {
            yaw: 1.0,
            pitch: 2.0,
            roll: 3.0
        }
    );
    assert_eq!(
        v.plane2d(f).unwrap(),
        Plane2d {
            normal: [1.0, 2.0],
            d: 3.0
        }
    );
    assert_eq!(
        v.plane3d(f).unwrap(),
        Plane3d {
            normal: [1.0, 2.0, 3.0],
            d: 4.0
        }
    );
    assert_eq!(v.real_rgb(f).unwrap(), RealRgb { r: 1.0, g: 2.0, b: 3.0 });
    assert_eq!(
        v.real_argb(f).unwrap(),
        RealArgb {
            a: 1.0,
            r: 2.0,
            g: 3.0,
            b: 4.0
        }
    );
    assert_eq!(v.real_hsv(f).unwrap(), RealHsv { h: 1.0, s: 2.0, v: 3.0 });
    assert_eq!(
        v.real_ahsv(f).unwrap(),
        RealAhsv {
            a: 1.0,
            h: 2.0,
            s: 3.0,
            v: 4.0
        }
    );
    let b = |lower, upper| Bounds { lower, upper };
    assert_eq!(v.real_bounds(f).unwrap(), b(1.0, 2.0));
    assert_eq!(
        v.real_rect2d(f).unwrap(),
        RealRect2d {
            x: b(1.0, 2.0),
            y: b(3.0, 4.0)
        }
    );
    assert_eq!(
        v.real_rect3d(f).unwrap(),
        RealRect3d {
            x: b(1.0, 2.0),
            y: b(3.0, 4.0),
            z: b(5.0, 6.0)
        }
    );
    // Bytes B, G, R, (ignored | A).
    assert_eq!(
        v.rgb8(0x4C).unwrap(),
        Rgb8 {
            r: 0x30,
            g: 0x20,
            b: 0x10
        }
    );
    assert_eq!(
        v.argb8(0x50).unwrap(),
        Argb8 {
            a: 0x44,
            r: 0x33,
            g: 0x22,
            b: 0x11
        }
    );
    assert_eq!(v.index16(0x54).unwrap(), None);
    assert!(v.index16(0x56).is_err(), "-2 is neither an index nor NONE");
    assert_eq!(v.index8(0x58).unwrap(), None);
    assert_eq!(v.index32(0x5C).unwrap(), None);
    assert_eq!(v.index16(0x30).unwrap(), Some(3));
    assert_eq!(v.string(0x60, 4).unwrap(), "abcd");
    assert!(v.tag_string(0xE0).is_err(), "no NUL in its 32 bytes");
}

#[test]
fn data_references_in_each_case() {
    let mut td = TagData::new(&[("scnr", "s")]);
    let inline = td.data(&[9, 8, 7]);
    let s = Struct::new(0x60)
        .put(0x00, &inline)
        // External: 4 bytes at file offset 0x10 (the header's version etc.).
        .put(0x14, &data_ref_bytes(4, 0x4, 0))
        // Stripped: a size, no address.
        .put(0x28, &data_ref_bytes(0x68A7F, 0x1234, 0))
        // External past the image.
        .put(0x3C, &data_ref_bytes(16, 0x7FFF_FFF0, 0));
    td.set_struct(0, &s.0);
    let map = MapFile::from_image(CacheFile::new("data", 0, td.finish()).image()).unwrap();
    let v = map.tag_view(map.scenario(), 0x60).unwrap();
    assert_eq!(v.data(0).unwrap(), [9, 8, 7]);
    assert_eq!(v.data_ref(0).unwrap().size, 3);
    assert_eq!(v.external_data(0x14).unwrap(), 5u32.to_le_bytes());
    assert_eq!(v.data(0x28).unwrap(), b"");
    assert!(v.external_data(0x3C).is_err());
    assert_eq!(map.file_bytes(4, 4).unwrap(), 5u32.to_le_bytes());
    assert!(map.file_bytes(map.image().len() - 2, 4).is_err());
}

#[test]
fn model_buffers_from_the_tag_header() {
    let mut td = TagData::new(&[("scnr", "s")]);
    td.set_struct(0, &[0; 4]);
    td.model_buffers(&[TAG_BASE + 0x100, TAG_BASE + 0x200], &[TAG_BASE + 0x300]);
    let map = MapFile::from_image(CacheFile::new("buffers", 1, td.finish()).image()).unwrap();
    assert_eq!(map.model_vertex_buffers(), [TAG_BASE + 0x100, TAG_BASE + 0x200]);
    assert_eq!(map.model_index_buffers(), [TAG_BASE + 0x300]);
    assert_eq!(map.tag_header().vertex_buffer_count, 2);
}

/// A map with two BSPs in the scenario's list.
fn bsp_map(tweak: impl FnOnce(&mut Vec<u8>)) -> io::Result<MapFile> {
    let mut td = TagData::new(&[
        ("scnr", "levels\\b\\b"),
        ("sbsp", "levels\\b\\b"),
        ("sbsp", "levels\\b\\c"),
    ]);
    let mut b0 = Bsp::new(0x1000);
    let inner = b0.heap.data(&[0xAB; 6]);
    let s0 = b0.set_struct(&Struct::new(0x20).u32(0, 77).put(4, &inner).0);
    b0.buffers(&[1, 2], &[3, 4]);
    let mut b1 = Bsp::new(0x800);
    b1.set_struct(&Struct::new(4).u32(0, 88).0);
    let elems = vec![
        bsp_element(0x800, 0x1000, td.tag_ref(1)),
        bsp_element(0x1800, 0x800, td.tag_ref(2)),
    ];
    let list = td.block(&elems);
    td.set_struct(0, &Struct::new(0x5B0).put(0x5A4, &list).0);
    let mut region = b0.finish();
    region.extend(b1.finish());
    tweak(&mut region);
    let map = MapFile::load(&CacheFile::with_bsps("bsps", 0, region, td.finish()).file())?;
    let b = &map.bsps()?[0];
    assert_eq!(b.header.struct_address, s0);
    Ok(map)
}

#[test]
fn bsp_spaces() {
    let map = bsp_map(|_| {}).unwrap();
    let bsps = map.bsps().unwrap();
    assert_eq!(bsps.len(), 2);
    let b0 = &bsps[0];
    assert_eq!(
        (b0.file_offset, b0.size, b0.load_address),
        (0x800, 0x1000, TAG_CACHE_END - 0x1000)
    );
    assert_eq!(b0.space().end(), TAG_CACHE_END as u64);
    assert_eq!(b0.header.list_a_count, 2);
    assert_eq!(b0.buffers_a, [1, 2]);
    assert_eq!(b0.buffers_b, [3, 4]);
    assert_eq!(map.tag(b0.tag.id).unwrap().path, "levels\\b\\b");
    let v = map.bsp_view(0, 0x20).unwrap();
    assert_eq!(v.u32(0).unwrap(), 77);
    assert_eq!(v.space().id, SpaceId::Bsp(0));
    assert_eq!(v.data(4).unwrap(), [0xAB; 6], "data in the BSP's own space");
    assert!(v.error(0, "x").to_string().contains("sbsp `levels\\b\\b` (bsp 0)"));
    let v1 = map.bsp_view(1, 4).unwrap();
    assert_eq!(v1.u32(0).unwrap(), 88);
    assert_eq!(v1.file_offset(), 0x1800 + 0x18);
    assert!(map.bsp_view(2, 4).is_err());
    assert!(map.bsp_view(1, 0x800).is_err(), "past the BSP's end");
    assert_eq!(map.space(SpaceId::Bsp(1)).unwrap().base, TAG_CACHE_END - 0x800);
}

#[test]
fn bsp_list_refusals() {
    let e = bsp_map(|r| r[0x14] = 0).unwrap_err().to_string();
    assert!(e.contains("sub-header magic"), "{e}");
    // A buffer list running out of the BSP.
    let e = bsp_map(|r| r[4..8].copy_from_slice(&0x1000u32.to_le_bytes()))
        .unwrap_err()
        .to_string();
    assert!(e.contains("buffer list A"), "{e}");
}

#[test]
fn class_table() {
    assert_eq!(class::CLASSES.len(), 73);
    let w = class::WEAPON.info().unwrap();
    assert_eq!(
        (w.name, w.parent, w.grandparent),
        ("weapon", Some(class::ITEM), Some(class::OBJECT))
    );
    assert!(class::SHADER.info().unwrap().abstract_only);
    assert_eq!(class::SOUND.to_string(), "snd!");
    assert_eq!(class::SKY, ClassCode::from_text("sky "));
    assert_eq!(class::SKY.as_u32(), 0x736B_7920);
    assert_eq!(format!("{:?}", class::UI_WIDGET_DEFINITION), "ClassCode(\"DeLa\")");
    assert!(class::is_known(ClassCode::new(b"cdmg")));
    assert!(!class::is_known(ClassCode::new(b"mod2")));
    assert_eq!(ClassCode::from_slot(0xFFFF_FFFF), None);
    for info in class::CLASSES {
        if let Some(p) = info.parent {
            assert!(p.info().unwrap().abstract_only, "{}", info.code);
        }
    }
    let _ = null_ref("shdr");
}
