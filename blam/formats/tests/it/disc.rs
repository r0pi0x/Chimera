//! The disc image reader on synthetic images: the test cases of
//! specs/halo_ce/disc_image.md section (a), each corruption case, the path
//! rules, the partition probe, and the `FileSource` and install seams.

use std::{io::ErrorKind, sync::Arc};

use blam_formats::disc::{DirEntry, DiscImage, fold_cmp, open_install};
use chimera_vfs::{FileSource, Mount};

use crate::fixture::*;

fn open(bytes: Vec<u8>) -> std::io::Result<DiscImage> {
    DiscImage::from_bytes(Arc::from(bytes))
}

fn spec() -> DiscImage {
    open(spec_image()).unwrap()
}

fn names(list: &[DirEntry]) -> Vec<(&str, bool, u64)> {
    list.iter().map(|e| (e.name.as_str(), e.is_dir, e.size)).collect()
}

#[test]
fn lists_the_root_in_tree_order() {
    let d = spec();
    assert_eq!(
        names(d.list("").unwrap()),
        [
            ("Maps", true, 2048),
            ("readme.txt", false, 5),
            ("Zeta.bin", false, 3000)
        ]
    );
    assert_eq!(d.list("/").unwrap(), d.list("").unwrap());
    assert_eq!(d.base(), 0);
    assert_eq!((d.file_count(), d.dir_count()), (3, 1));
    assert_eq!(d.volume_time(), 0);
}

#[test]
fn lists_a_subdirectory() {
    let d = spec();
    assert_eq!(names(d.list("Maps").unwrap()), [("A_B.map", false, 4)]);
    assert_eq!(names(d.list("maps/").unwrap()), [("A_B.map", false, 4)]);
}

#[test]
fn lists_every_file() {
    let d = spec();
    let all: Vec<_> = d.entries().into_iter().map(|e| (e.path, e.size)).collect();
    assert_eq!(
        all,
        [
            ("maps/a_b.map".to_string(), 4),
            ("readme.txt".to_string(), 5),
            ("zeta.bin".to_string(), 3000)
        ]
    );
    assert_eq!(all.iter().map(|e| e.1).sum::<u64>(), 3009);
}

#[test]
fn reads_files_case_insensitively_with_either_separator() {
    let d = spec();
    for p in [
        "readme.txt",
        "README.TXT",
        "/readme.txt",
        "./readme.txt",
        "\\Readme.Txt",
    ] {
        assert_eq!(d.read_file(p).unwrap(), b"HELLO", "{p}");
    }
    assert_eq!(d.read_file("maps/a_b.MAP").unwrap(), [1, 2, 3, 4]);
    assert_eq!(d.read_file("Maps\\A_B.map").unwrap(), [1, 2, 3, 4]);
    let z = d.read_file("zeta.BIN").unwrap();
    assert_eq!(z.len(), 3000);
    assert_eq!(z[2999], 238);
    assert!(z.iter().enumerate().all(|(i, &b)| b == zeta_byte(i)));
}

#[test]
fn refuses_what_the_path_rules_refuse() {
    let d = spec();
    assert_eq!(d.read_file("nothing.txt").unwrap_err().kind(), ErrorKind::NotFound);
    assert_eq!(d.read_file("Maps").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(d.read_file("readme.txt/x").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(d.list("readme.txt").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(d.list("Maps/..").unwrap_err().kind(), ErrorKind::InvalidInput);
    assert_eq!(
        d.read_file("../readme.txt").unwrap_err().kind(),
        ErrorKind::InvalidInput
    );
    assert_eq!(d.list("nowhere").unwrap_err().kind(), ErrorKind::NotFound);
    assert!(d.read_file(&"a".repeat(300)).is_err());
}

#[test]
fn is_a_file_source() {
    let d = spec();
    assert!(d.contains("maps/a_b.map") && d.contains("readme.txt"));
    assert!(!d.contains("maps") && !d.contains("nothing"));
    assert_eq!(d.files_in(""), ["readme.txt", "zeta.bin"]);
    assert_eq!(d.files_in("maps"), ["maps/a_b.map"]);
    assert!(d.files_in("nowhere").is_empty());
    assert_eq!(d.read("zeta.bin").unwrap().unwrap().len(), 3000);
    assert!(d.read("maps").is_none() && d.read("nothing").is_none());
    assert_eq!(d.fingerprint("zeta.bin"), None, "an image in memory has no fingerprint");
    let mut mount = Mount::default();
    mount.push(spec());
    assert_eq!(mount.read("Maps/A_B.map").unwrap(), [1, 2, 3, 4]);
}

#[test]
fn a_left_link_of_zero_just_makes_another_tree() {
    let mut b = spec_image();
    b[README_ENTRY..README_ENTRY + 2].copy_from_slice(&0u16.to_le_bytes());
    let d = open(b).unwrap();
    assert_eq!(
        names(d.list("").unwrap()),
        [("readme.txt", false, 5), ("Zeta.bin", false, 3000)]
    );
    assert!(d.read("maps/a_b.map").is_none());
}

#[test]
fn a_cycle_is_an_error() {
    let mut b = spec_image();
    b[MAPS_ENTRY..MAPS_ENTRY + 2].copy_from_slice(&11u16.to_le_bytes());
    b[ZETA_ENTRY..ZETA_ENTRY + 2].copy_from_slice(&6u16.to_le_bytes());
    let e = open(b).err().unwrap();
    assert_eq!(e.kind(), ErrorKind::InvalidData);
    assert!(e.to_string().contains("loops"), "{e}");
}

#[test]
fn a_link_past_the_table_is_an_error() {
    let mut b = spec_image();
    b[README_ENTRY..README_ENTRY + 2].copy_from_slice(&40u16.to_le_bytes());
    let e = open(b).err().unwrap();
    assert_eq!(e.kind(), ErrorKind::InvalidData);
    assert!(e.to_string().contains("table's end"), "{e}");
}

#[test]
fn an_entry_whose_name_passes_the_table_is_an_error() {
    let mut b = spec_image();
    // Zeta.bin's name length claims 40 bytes; the table ends at byte 68.
    b[ZETA_ENTRY + 13] = 40;
    assert!(open(b).is_err());
}

#[test]
fn a_link_into_the_fill_is_an_error() {
    let mut b = spec_image();
    // The root table now says 2048 bytes: byte 100 is 0xFF fill.
    b[32 * SECTOR + 0x18..32 * SECTOR + 0x1C].copy_from_slice(&2048u32.to_le_bytes());
    b[README_ENTRY..README_ENTRY + 2].copy_from_slice(&25u16.to_le_bytes());
    let e = open(b).err().unwrap();
    assert!(e.to_string().contains("fill"), "{e}");
}

#[test]
fn a_file_past_the_image_end_is_an_error() {
    let mut b = spec_image();
    b[ZETA_ENTRY + 4..ZETA_ENTRY + 8].copy_from_slice(&38u32.to_le_bytes());
    let e = open(b).err().unwrap();
    assert_eq!(e.kind(), ErrorKind::InvalidData);
    assert!(e.to_string().contains("passes the image's end"), "{e}");
}

#[test]
fn the_largest_sector_and_size_are_an_error() {
    let mut b = spec_image();
    b[ZETA_ENTRY + 4..ZETA_ENTRY + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    b[ZETA_ENTRY + 8..ZETA_ENTRY + 12].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(open(b).is_err());
}

#[test]
fn a_bad_magic_is_refused() {
    let mut b = spec_image();
    b[32 * SECTOR + 0x7EC] = b'X';
    let e = open(b).err().unwrap();
    assert_eq!(e.kind(), ErrorKind::InvalidData);
    assert!(e.to_string().contains("closing magic"), "{e}");
    let mut b = spec_image();
    b[32 * SECTOR] = b'X';
    assert!(open(b).err().unwrap().to_string().contains("not an Xbox disc image"));
    assert!(open(vec![0; 100]).is_err());
    assert!(open(Vec::new()).is_err());
}

#[test]
fn finds_a_partition_at_a_non_zero_base() {
    let base = 0x0209_0000;
    let mut b = vec![0u8; base];
    b.extend_from_slice(&spec_image());
    let d = open(b).unwrap();
    assert_eq!(d.base(), base as u64);
    assert_eq!(d.read_file("Maps/A_B.map").unwrap(), [1, 2, 3, 4]);
    assert_eq!(d.read_file("zeta.bin").unwrap()[2999], 238);
    assert_eq!(d.extent("readme.txt"), Some((base as u64 + 35 * 2048, 5)));
}

#[test]
fn finds_the_partition_at_0x18300000_in_an_image_file() {
    let base: u64 = 0x1830_0000;
    let scratch = Scratch::new();
    let path = scratch.path().join("full.iso");
    {
        use std::io::{Seek, SeekFrom, Write};
        let mut f = std::fs::File::create(&path).unwrap();
        f.seek(SeekFrom::Start(base)).unwrap();
        f.write_all(&spec_image()).unwrap();
    }
    let d = DiscImage::open(&path).unwrap();
    assert_eq!(d.base(), base);
    assert_eq!(d.read_file("README.TXT").unwrap(), b"HELLO");
    assert_eq!(d.read_file("zeta.bin").unwrap().len(), 3000);
    let fp = d.fingerprint("zeta.bin").unwrap();
    assert!(
        fp.starts_with("xiso ") && fp.ends_with(&format!("{} 3000", base + 36 * 2048)),
        "{fp}"
    );
    assert_eq!(d.fingerprint("nothing"), None);
}

#[test]
fn a_file_cut_short_is_an_error_on_read_and_not_a_panic() {
    let scratch = Scratch::new();
    let path = scratch.path().join("a.iso");
    std::fs::write(&path, spec_image()).unwrap();
    let d = DiscImage::open(&path).unwrap();
    // Shrink the image after the open: the file's bytes are gone.
    std::fs::write(&path, &spec_image()[..36 * SECTOR + 100]).unwrap();
    let e = d.read_file("zeta.bin").unwrap_err();
    assert_eq!(e.kind(), ErrorKind::UnexpectedEof, "{e}");
}

#[test]
fn the_underscore_sorts_after_the_letters() {
    use std::cmp::Ordering::*;
    assert_eq!(fold_cmp(b"AZ", b"A_"), Less);
    assert_eq!(fold_cmp(b"az", b"a_"), Less);
    assert_eq!(fold_cmp(b"A", b"AB"), Less);
    assert_eq!(fold_cmp(b"readme", b"README"), Equal);
    assert_eq!(fold_cmp(b"Maps", b"readme.txt"), Less);

    // "A_" is the root, "AZ" its left child: both are found.
    let mut img = Image::new(40);
    img.volume(33, 32);
    img.dir_table(33, 1, &[E::file("A_", 34, 1).links(4, 0), E::file("AZ", 35, 2)]);
    img.put(34 * SECTOR, b"u");
    img.put(35 * SECTOR, b"zz");
    let d = open(img.bytes).unwrap();
    assert_eq!(names(d.list("").unwrap()), [("AZ", false, 2), ("A_", false, 1)]);
    assert_eq!(d.read_file("az").unwrap(), b"zz");
    assert_eq!(d.read_file("a_").unwrap(), b"u");
}

fn one_entry(e: E) -> std::io::Result<DiscImage> {
    let mut img = Image::new(40);
    img.volume(33, 64);
    img.dir_table(33, 1, &[e]);
    open(img.bytes)
}

#[test]
fn bad_names_are_refused() {
    for name in [&b""[..], b"a/b", b"a\\b", b"a\0b"] {
        let e = one_entry(E {
            name: name.to_vec(),
            ..E::file("x", 34, 1)
        })
        .err()
        .unwrap();
        assert_eq!(e.kind(), ErrorKind::InvalidData, "{name:?}");
    }
    assert!(one_entry(E::file("fine", 34, 1)).is_ok());
}

#[test]
fn names_above_0x7f_list_as_latin_1() {
    let d = one_entry(E {
        name: vec![b'a', 0xE9],
        ..E::file("x", 34, 1)
    })
    .unwrap();
    assert_eq!(d.list("").unwrap()[0].name, "a\u{e9}");
}

#[test]
fn empty_directories_and_empty_files_need_no_data() {
    let mut img = Image::new(40);
    img.volume(33, 64);
    // "empty" takes 20 bytes, so the next entry is at link 5.
    img.dir_table(
        33,
        1,
        &[E::dir("empty", 0, 0).links(0, 5), E::file("zero.bin", 0x00FF_FFFF, 0)],
    );
    let d = open(img.bytes).unwrap();
    assert!(d.list("empty").unwrap().is_empty());
    assert_eq!(d.read_file("zero.bin").unwrap(), Vec::<u8>::new());
    assert_eq!(d.files_in(""), ["zero.bin"]);
    assert!(d.contains("zero.bin"));
}

#[test]
fn a_directory_leading_back_to_an_ancestor_is_refused() {
    // The root holds a directory whose table is the root's own.
    let mut img = Image::new(40);
    img.volume(33, 64);
    img.dir_table(33, 1, &[E::dir("up", 33, 64)]);
    let e = open(img.bytes).err().unwrap();
    assert!(e.to_string().contains("two directories"), "{e}");
}

fn chain(levels: usize) -> Vec<u8> {
    let mut img = Image::new(34 + levels + 1);
    img.volume(33, 32);
    for i in 0..levels {
        img.dir_table(33 + i, 1, &[E::dir("d", 34 + i as u32, 32)]);
    }
    img.dir_table(33 + levels, 1, &[E::file("end", 0, 0)]);
    img.bytes
}

#[test]
fn nesting_beyond_64_levels_is_refused() {
    let e = open(chain(70)).err().unwrap();
    assert!(e.to_string().contains("levels deep"), "{e}");
    let d = open(chain(60)).unwrap();
    let path = format!("{}end", "d/".repeat(60));
    assert!(d.read_file(&path).unwrap().is_empty());
}

#[test]
fn a_directory_past_the_image_end_is_refused() {
    let mut img = Image::new(40);
    img.volume(33, 64);
    img.dir_table(33, 1, &[E::dir("far", 39, 4096)]);
    assert!(
        open(img.bytes)
            .err()
            .unwrap()
            .to_string()
            .contains("passes the image's end")
    );
    let mut img = Image::new(40);
    img.volume(39, 4096);
    assert!(open(img.bytes).is_err());
}

#[test]
fn two_names_that_fold_together_are_refused() {
    let mut img = Image::new(40);
    img.volume(33, 64);
    img.dir_table(33, 1, &[E::file("Ab", 34, 1).links(4, 0), E::file("aB", 34, 1)]);
    assert!(open(img.bytes).err().unwrap().to_string().contains("fold"));
}

#[test]
fn an_install_is_an_image_or_a_folder() {
    let scratch = Scratch::new();
    let iso = scratch.path().join("game.iso");
    std::fs::write(&iso, spec_image()).unwrap();
    let mount = open_install(&iso).unwrap();
    assert_eq!(mount.read("MAPS/a_b.map").unwrap(), [1, 2, 3, 4]);
    assert!(mount.layers().next().unwrap().name().ends_with("game.iso"));

    let folder = scratch.path().join("files");
    std::fs::create_dir_all(folder.join("maps")).unwrap();
    std::fs::write(folder.join("maps").join("a_b.map"), [1, 2, 3, 4]).unwrap();
    let mount = open_install(&folder).unwrap();
    assert_eq!(mount.read("maps/a_b.map").unwrap(), [1, 2, 3, 4]);

    let e = open_install(&scratch.path().join("missing")).err().unwrap();
    assert_eq!(e.kind(), ErrorKind::NotFound);
    std::fs::write(scratch.path().join("junk.iso"), b"not a disc").unwrap();
    assert!(open_install(&scratch.path().join("junk.iso")).is_err());
}
