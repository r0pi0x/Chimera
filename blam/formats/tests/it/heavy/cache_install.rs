//! The map cache reader on the user's Halo disc image, against section
//! (b) of specs/halo_ce/cache_file.md and its per-map table: each map is
//! read out of the image through `blam_formats::disc`. Each test passes
//! with a note when there is no image (`$CHIMERA_HALO_CE_ROOT`, else
//! `[games.halo_ce] path` in chimera.local.toml).

use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

use blam_formats::cache::{ClassCode, MapFile, MapType, SpaceId, TAG_BASE, TAG_CACHE_END, TagEntry, TagId, class};
use blam_formats::disc::DiscImage;

use super::disc_install::{SKIP, image_path};

fn disc() -> Option<&'static DiscImage> {
    static DISC: OnceLock<Option<DiscImage>> = OnceLock::new();
    let d = DISC
        .get_or_init(|| image_path().map(|p| DiscImage::open(&p).unwrap()))
        .as_ref();
    if d.is_none() {
        eprintln!("{SKIP}");
    }
    d
}

/// A map's file bytes and the map loaded from them, with the load's time.
fn load(name: &str) -> Option<(Vec<u8>, MapFile, Duration)> {
    let file = disc()?.read_file(&format!("maps/{name}.map")).unwrap();
    let t = Instant::now();
    let map = MapFile::load(&file).unwrap();
    Some((file, map, t.elapsed()))
}

fn bloodgulch() -> Option<&'static MapFile> {
    static MAP: OnceLock<Option<MapFile>> = OnceLock::new();
    MAP.get_or_init(|| load("bloodgulch").map(|(_, m, _)| m)).as_ref()
}

/// Enough of the scenario's main struct for the fields the spec names.
const SCENARIO_VIEW: usize = 0x5B0;

fn c(s: &str) -> ClassCode {
    ClassCode::from_text(s)
}

fn entry(map: &MapFile, i: usize) -> (String, &str) {
    let t = &map.tags()[i];
    (t.class.to_string(), t.path.as_str())
}

#[test]
fn bloodgulch_header_and_load_time() {
    let Some((file, map, took)) = load("bloodgulch") else {
        return;
    };
    eprintln!("bloodgulch.map: MapFile::load took {took:?}");
    assert_eq!(file.len(), 22_571_008);
    let h = map.header();
    assert_eq!(
        (h.version, h.decompressed_size, h.compressed_padding),
        (5, 0x02A4_6800, 1585)
    );
    assert_eq!((h.tag_data_offset, h.tag_data_size), (0x0233_B600, 0x0070_B060));
    assert_eq!(h.name, "bloodgulch");
    assert_eq!(h.build, "01.10.12.2276");
    assert_eq!(h.map_type, MapType::Multiplayer);
    assert_eq!(h.checksum, 0x035E_2DA3);
    assert_eq!(map.image().len(), 44_328_960);
    // Compressed stream length = file - header - padding.
    assert_eq!(file.len() - 0x800 - 1585, 22_567_375);
    assert_eq!(&file[0x800..0x802], [0x78, 0x9C]);
    let th = map.tag_header();
    assert_eq!(th.index_address, 0x803A_6024);
    assert_eq!(th.scenario_id, TagId(0xE174_0000));
    assert_eq!(th.checksum, 0xEF03_1F2E);
    assert_eq!(th.tag_count, 1806);
    assert_eq!((th.vertex_buffer_count, th.vertex_buffer_address), (359, 0x803C_6374));
    assert_eq!((th.index_buffer_count, th.index_buffer_address), (359, 0x8064_8680));
    assert_eq!(map.model_vertex_buffers().len(), 359);
    assert_eq!(map.model_index_buffers().len(), 359);
    // Their data lies in the tag data (the spec: vertex data from 0x803C7460,
    // index data up to the first tag struct 0x8068C318).
    let space = map.tag_space();
    for &a in map.model_vertex_buffers().iter().chain(map.model_index_buffers()) {
        assert!(space.offset_of(a, 1).is_some(), "{a:#x}");
    }
    assert_eq!(map.model_vertex_buffers().iter().min(), Some(&0x803C_7460));
}

#[test]
fn bloodgulch_index() {
    let Some(map) = bloodgulch() else { return };
    let first = [
        ("scnr", "levels\\test\\bloodgulch\\bloodgulch"),
        ("sky ", "levels\\test\\bloodgulch\\bloodgulch"),
        ("mode", "sky\\mp clear afternoon\\mp clear afternoon"),
        ("sotr", "sky\\clear afternoon\\shaders\\threshold clear afternoon"),
        ("bitm", "sky\\planets\\threshold clear afternoon"),
    ];
    for (i, (cl, path)) in first.iter().enumerate() {
        assert_eq!(entry(map, i), (cl.to_string(), *path));
        assert_eq!(map.tags()[i].id, TagId(((0xE174 + i as u32) << 16) | i as u32));
    }
    let s = map.scenario();
    assert_eq!((s.path_address, s.address), (0x803B_41E4, 0x8068_C318));
    let t3 = &map.tags()[3];
    assert_eq!((t3.parent, t3.grandparent), (Some(class::SHADER), None));
    let last = &map.tags()[1805];
    assert_eq!(
        (last.class, last.path.as_str(), last.id),
        (
            class::SOUND_ENVIRONMENT,
            "sound\\sound environments\\mountains",
            TagId(0xE883_070D)
        )
    );
    let g = map.find(class::GLOBALS, "globals\\globals").unwrap();
    assert_eq!((g.index(), g.id), (1378, TagId(0xE6D6_0562)));
    let ar = map
        .find(class::WEAPON, "weapons\\assault rifle\\assault rifle")
        .unwrap();
    assert_eq!((ar.index(), ar.id), (237, TagId(0xE261_00ED)));
    let mp = map.find(class::BIPED, "characters\\cyborg_mp\\cyborg_mp").unwrap();
    assert_eq!((mp.parent, mp.grandparent), (Some(class::UNIT), Some(class::OBJECT)));
    assert_eq!(
        map.find(class::WEAPON, "Weapons\\Pistol\\PISTOL").unwrap().path,
        "weapons\\pistol\\pistol"
    );
    for (cl, n) in [
        ("bitm", 368),
        ("snd!", 315),
        ("weap", 15),
        ("effe", 318),
        ("DeLa", 30),
        ("mode", 63),
        ("soso", 106),
        ("proj", 16),
        ("eqip", 14),
        ("sbsp", 1),
    ] {
        assert_eq!(map.of_class(c(cl)).count(), n, "{cl}");
    }
    // Objects: every bipd, vehi, weap, eqip, garb, mach, ctrl, lifi, proj,
    // scen, ssce (2+3+15+14+0+0+0+0+16+19+1).
    assert_eq!(map.of_kind(class::OBJECT).count(), 70);
    assert_eq!(map.of_kind(class::SHADER).count(), 14 + 106 + 26 + 20 + 6 + 7 + 1);
}

#[test]
fn bloodgulch_scenario() {
    let Some(map) = bloodgulch() else { return };
    let s = map.tag_view(map.scenario(), SCENARIO_VIEW).unwrap();
    // The obsolete references at 0x00, 0x10, 0x20.
    for (off, cl) in [(0x00, "sbsp"), (0x10, "sbsp"), (0x20, "sky ")] {
        let r = s.tag_ref(off).unwrap();
        assert!(r.is_null() && r.path_address == 0, "{off:#x}");
        assert_eq!(r.class, c(cl));
    }
    // The sky block: elements of one tag reference.
    let sky = s.block(0x30, 0x10).unwrap();
    assert_eq!(sky.len(), 1);
    let e = sky.get(0).unwrap();
    let r = e.tag_ref(0).unwrap();
    assert_eq!((r.path_address, e.i32(8).unwrap()), (0x803B_4206, 0));
    let t = e.reference(0, class::SKY).unwrap().unwrap();
    assert_eq!(t.index(), 1);
    assert_eq!(t.path_address, 0x803B_4206);

    let counts: Vec<i32> = [0x354, 0x378, 0x384]
        .iter()
        .map(|&o| s.raw_block(o).unwrap().count)
        .collect();
    assert_eq!(counts, [71, 191, 35]);
    let spawn = s.block(0x354, 0x34).unwrap().get(0).unwrap();
    let [x, y, z] = spawn.point3(0).unwrap();
    let close = |a: f32, b: f32, eps: f32| (a - b).abs() < eps;
    assert!(
        close(x, 98.4934, 1e-3) && close(y, -157.639, 1e-3) && close(z, 1.70473, 1e-4),
        "{x} {y} {z}"
    );
    assert!(close(spawn.f32(0xC).unwrap(), 1.328281, 1e-5));

    let bsps = map.bsps().unwrap();
    assert_eq!(bsps.len(), 1);
    let b = &bsps[0];
    assert_eq!((b.file_offset, b.size, b.load_address), (0x800, 0x13_C800, 0x8186_9800));
    let bt = map.tag(b.tag.id).unwrap();
    assert_eq!(
        (bt.class, bt.path.as_str(), bt.id),
        (
            class::SCENARIO_STRUCTURE_BSP,
            "levels\\test\\bloodgulch\\bloodgulch",
            TagId(0xE85A_06E4)
        )
    );
    assert_eq!(bt.address, 0);
    let h = b.header;
    assert_eq!(
        (
            h.struct_address,
            h.list_a_count,
            h.list_a_address,
            h.list_b_count,
            h.list_b_address
        ),
        (0x818A_2120, 0x4F, 0x8186_9818, 0x4F, 0x8186_9BCC)
    );
    assert_eq!((b.buffers_a.len(), b.buffers_b.len()), (0x4F, 0x4F));
    let space = map.space(SpaceId::Bsp(0)).unwrap();
    assert_eq!(space.end(), TAG_CACHE_END as u64);
    assert_eq!(space.end(), 0x819A_6000);
    let v = map.bsp_view(0, 4).unwrap();
    assert_eq!(v.file_offset(), 0x800 + (0x818A_2120u32 - 0x8186_9800) as usize);
}

/// Every tag reference in the scenario's top level resolves: the fields
/// this spec names (the BSP list's and the sky block's references; the
/// three obsolete ones are null), and every 16-byte word shaped like a
/// reference (a disc class code, then a path address) in the struct's
/// first `SCENARIO_VIEW` bytes, whose id must name a tag of that class with
/// that path. (On bloodgulch there is none: the scenario's own reference
/// fields are past what this spec describes; scenario.md will list them.)
#[test]
fn bloodgulch_scenario_references_resolve() {
    let Some(map) = bloodgulch() else { return };
    let s = map.tag_view(map.scenario(), SCENARIO_VIEW).unwrap();
    let mut checked = 0;
    for off in (0..SCENARIO_VIEW - 16).step_by(4) {
        let r = s.tag_ref(off).unwrap();
        if class::class_info(r.class).is_none() || r.path_address == 0 || r.path_address < TAG_BASE {
            continue;
        }
        let t: &TagEntry = map
            .tag(r.id)
            .unwrap_or_else(|| panic!("+{off:#x}: {} does not resolve", r.id));
        assert_eq!((t.class, t.path_address), (r.class, r.path_address), "+{off:#x}");
        checked += 1;
    }
    eprintln!("{checked} reference-shaped words with a path in the scenario's first {SCENARIO_VIEW:#x} bytes");
    for b in map.bsps().unwrap() {
        assert!(map.tag(b.tag.id).is_some());
    }
    let sky = s.block(0x30, 0x10).unwrap();
    for e in sky.iter() {
        assert!(e.reference(0, class::SKY).unwrap().is_some());
    }
}

#[test]
fn hangemhigh() {
    let Some((file, map, _)) = load("hangemhigh") else {
        return;
    };
    let h = map.header();
    assert_eq!(
        (file.len(), h.decompressed_size, h.compressed_padding),
        (21_338_112, 0x029A_D000, 1029)
    );
    assert_eq!((h.tag_data_offset, h.tag_data_size), (0x022F_6200, 0x006B_6C08));
    assert_eq!(
        (h.name.as_str(), h.map_type, h.checksum),
        ("hangemhigh", MapType::Multiplayer, 0x8313_BD85)
    );
    let th = map.tag_header();
    assert_eq!((th.tag_count, th.checksum), (1681, 0x10B5_ACA4));
    assert_eq!((th.vertex_buffer_count, th.vertex_buffer_address), (323, 0x803C_3B2C));
    assert_eq!((th.index_buffer_count, th.index_buffer_address), (323, 0x8062_CE40));
    assert_eq!(entry(&map, 0), ("scnr".into(), "levels\\test\\hangemhigh\\hangemhigh"));
    assert_eq!(entry(&map, 1), ("sky ".into(), "levels\\test\\hangemhigh\\hangemhigh"));
    assert_eq!(entry(&map, 2).1, "sky\\mp clear afternoon\\mp clear afternoon");
    assert_eq!(entry(&map, 4).1, "sky\\planets\\threshold clear afternoon");
    assert_eq!(map.scenario().address, 0x8066_DCF8);
    assert_eq!(
        entry(&map, 1680),
        ("snde".into(), "sound\\sound environments\\concert_hall")
    );
    let b = &map.bsps().unwrap()[0];
    assert_eq!(
        (map.bsps().unwrap().len(), b.file_offset, b.size, b.load_address),
        (1, 0x800, 0x9_0000, 0x8191_6000)
    );
    assert_eq!(b.space().end(), 0x819A_6000);
    let s = map.tag_view(map.scenario(), SCENARIO_VIEW).unwrap();
    let counts: Vec<i32> = [0x354, 0x378, 0x384]
        .iter()
        .map(|&o| s.raw_block(o).unwrap().count)
        .collect();
    assert_eq!(counts, [67, 92, 45]);
    for (cl, n) in [
        ("bitm", 329),
        ("snd!", 314),
        ("scen", 2),
        ("ssce", 0),
        ("itmc", 13),
        ("coll", 25),
    ] {
        assert_eq!(map.of_class(c(cl)).count(), n, "{cl}");
    }
}

#[test]
fn ui() {
    let Some((_, map, _)) = load("ui") else { return };
    let h = map.header();
    assert_eq!((h.decompressed_size, h.compressed_padding), (0x0200_6C00, 27));
    assert_eq!((h.tag_data_offset, h.tag_data_size), (0x01E7_5C00, 0x0019_0F04));
    assert_eq!((h.map_type, h.checksum), (MapType::UserInterface, 0xCDC1_A39A));
    let th = map.tag_header();
    assert_eq!(
        (th.tag_count, th.checksum, th.vertex_buffer_count, th.index_buffer_count),
        (983, 0x6B0B_0215, 34, 34)
    );
    assert_eq!(map.scenario().path, "levels\\ui\\ui");
    let bsps = map.bsps().unwrap();
    assert_eq!(bsps.len(), 1);
    let b = &bsps[0];
    assert_eq!((b.size, b.file_offset, b.load_address), (0x800, 0x800, 0x819A_5800));
    assert_eq!((b.header.list_a_count, b.header.list_b_count), (0, 0));
}

#[test]
fn a10() {
    let Some((_, map, took)) = load("a10") else { return };
    eprintln!("a10.map: MapFile::load took {took:?}");
    let h = map.header();
    assert_eq!((h.decompressed_size, h.compressed_padding), (0x1060_1C00, 666));
    assert_eq!((h.tag_data_offset, h.tag_data_size), (0x0F88_1200, 0x00D8_08B0));
    assert_eq!((h.map_type, h.checksum), (MapType::Campaign, 0xE6AC_8E18));
    let th = map.tag_header();
    assert_eq!(
        (th.tag_count, th.checksum, th.vertex_buffer_count, th.index_buffer_count),
        (3357, 0x99A1_B67E, 623, 623)
    );
    let bsps = map.bsps().unwrap();
    assert_eq!(bsps.len(), 9);
    let path = |b: &blam_formats::cache::Bsp| map.tag(b.tag.id).unwrap().path.clone();
    assert_eq!(
        (
            path(&bsps[0]).as_str(),
            bsps[0].file_offset,
            bsps[0].size,
            bsps[0].load_address
        ),
        ("levels\\a10\\a10a", 0x800, 0x5F_1000, 0x813B_5000)
    );
    assert_eq!(
        (path(&bsps[8]).as_str(), bsps[8].file_offset, bsps[8].size),
        ("levels\\a10\\x10hangar", 0x31F_9800, 0x2A_2000)
    );
    for b in bsps {
        assert_eq!(b.load_address as u64 + b.size as u64, 0x819A_6000);
    }
}

/// The spec's per-map table: name, type, file size, decompressed size,
/// tags, BSPs; and on every map: version, build, base, the scenario at tag
/// 0 with id 0xE1740000, stored parents as the class table has them.
#[test]
fn all_24_maps() {
    if disc().is_none() {
        return;
    }
    #[rustfmt::skip]
    let table: [(&str, i16, usize, u32, usize, usize); 24] = [
        ("a10", 0, 173_903_872, 274_734_080, 3357, 9),
        ("a30", 0, 152_610_816, 213_065_216, 3215, 2),
        ("a50", 0, 176_224_256, 250_138_624, 3311, 4),
        ("b30", 0, 162_877_440, 224_887_296, 3325, 2),
        ("b40", 0, 186_873_856, 279_763_456, 3647, 13),
        ("c10", 0, 166_926_336, 252_559_360, 3369, 6),
        ("c20", 0, 76_980_224, 142_009_856, 1860, 4),
        ("c40", 0, 166_295_552, 255_975_936, 3208, 13),
        ("d20", 0, 131_012_608, 205_501_952, 2743, 5),
        ("d40", 0, 170_405_888, 274_594_816, 2978, 10),
        ("beavercreek", 1, 22_122_496, 43_565_056, 1755, 1),
        ("bloodgulch", 1, 22_571_008, 44_328_960, 1806, 1),
        ("boardingaction", 1, 22_075_392, 45_346_304, 1735, 1),
        ("carousel", 1, 20_215_808, 42_367_488, 1716, 1),
        ("chillout", 1, 20_336_640, 41_967_104, 1702, 1),
        ("damnation", 1, 23_756_800, 47_968_768, 1758, 1),
        ("hangemhigh", 1, 21_338_112, 43_700_224, 1681, 1),
        ("longest", 1, 20_512_768, 42_141_696, 1696, 1),
        ("prisoner", 1, 21_323_776, 43_974_656, 1685, 1),
        ("putput", 1, 19_777_536, 40_668_672, 1675, 1),
        ("ratrace", 1, 23_840_768, 47_403_008, 1710, 1),
        ("sidewinder", 1, 24_823_808, 48_048_128, 1757, 1),
        ("wizard", 1, 19_687_424, 40_681_984, 1704, 1),
        ("ui", 2, 14_145_536, 33_582_080, 983, 1),
    ];
    for (name, ty, file_size, dsize, tags, bsps) in table {
        let (file, map, took) = load(name).unwrap();
        eprintln!("{name}: loaded in {took:?}");
        let h = map.header();
        assert_eq!(h.name, name);
        assert_eq!(h.build, "01.10.12.2276", "{name}");
        assert_eq!(h.version, 5);
        let want = match ty {
            0 => MapType::Campaign,
            1 => MapType::Multiplayer,
            _ => MapType::UserInterface,
        };
        assert_eq!(h.map_type, want, "{name}");
        assert_eq!((file.len(), h.decompressed_size), (file_size, dsize), "{name}");
        assert_eq!(file.len() % 2048, 0);
        assert_eq!(&file[0x800..0x802], [0x78, 0x9C], "{name}");
        assert_eq!(map.tags().len(), tags, "{name}");
        assert_eq!(map.tag_header().scenario_id, TagId(0xE174_0000));
        assert_eq!(map.tags()[0].class, class::SCENARIO);
        let sbsp = map.of_class(class::SCENARIO_STRUCTURE_BSP).count();
        assert_eq!(sbsp, bsps, "{name}: sbsp tags");
        assert_eq!(map.bsps().unwrap().len(), bsps, "{name}: BSP list");
        for t in map.tags() {
            match class::class_info(t.class) {
                Some(info) => {
                    assert_eq!(
                        (t.parent, t.grandparent),
                        (info.parent, info.grandparent),
                        "{name}: {}",
                        t.class
                    );
                    assert!(!info.abstract_only);
                }
                None => {
                    assert!(class::is_known(t.class), "{name}: {}", t.class);
                    eprintln!(
                        "{name}: tag {} `{}` of class {} (parents {:?} {:?}), absent from the spec's disc table",
                        t.index(),
                        t.path,
                        t.class,
                        t.parent,
                        t.grandparent
                    );
                }
            }
        }
        drop(map);
    }
}

#[test]
fn every_multiplayer_map_holds_the_donor_weapons() {
    if disc().is_none() {
        return;
    }
    let weapons = [
        "weapons\\assault rifle\\assault rifle",
        "weapons\\pistol\\pistol",
        "weapons\\sniper rifle\\sniper rifle",
        "weapons\\rocket launcher\\rocket launcher",
        "vehicles\\ghost\\ghost gun",
    ];
    for name in ["bloodgulch", "wizard", "chillout"] {
        let (_, map, _) = load(name).unwrap();
        assert_eq!(map.of_class(class::WEAPON).count(), 15, "{name}");
        assert_eq!(map.of_class(class::PROJECTILE).count(), 16, "{name}");
        assert_eq!(map.of_class(class::EQUIPMENT).count(), 14, "{name}");
        for w in weapons {
            assert!(map.find(class::WEAPON, w).is_some(), "{name}: {w}");
        }
        assert!(map.find(class::VEHICLE, "vehicles\\warthog\\warthog").is_some());
    }
}
