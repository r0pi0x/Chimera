//! Synthetic disc images built in code from specs/halo_ce/disc_image.md
//! (test case (a) and its variations), and a scratch folder for tests that
//! need real files.

use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

pub const SECTOR: usize = 2048;
pub const MAGIC: &[u8; 20] = b"MICROSOFT*XBOX*MEDIA";

/// One directory entry to write.
#[derive(Clone)]
pub struct E {
    pub left: u16,
    pub right: u16,
    pub sector: u32,
    pub size: u32,
    pub attr: u8,
    pub name: Vec<u8>,
}

impl E {
    pub fn new(name: &str, sector: u32, size: u32, attr: u8) -> Self {
        E {
            left: 0,
            right: 0,
            sector,
            size,
            attr,
            name: name.as_bytes().to_vec(),
        }
    }

    pub fn file(name: &str, sector: u32, size: u32) -> Self {
        Self::new(name, sector, size, 0x20)
    }

    pub fn dir(name: &str, sector: u32, size: u32) -> Self {
        Self::new(name, sector, size, 0x10)
    }

    pub fn links(mut self, left: u16, right: u16) -> Self {
        self.left = left;
        self.right = right;
        self
    }
}

/// The entries packed one after another, each padded with 0xFF to a
/// multiple of 4. Returns the bytes used.
pub fn table(entries: &[E]) -> Vec<u8> {
    let mut out = Vec::new();
    for e in entries {
        out.extend_from_slice(&e.left.to_le_bytes());
        out.extend_from_slice(&e.right.to_le_bytes());
        out.extend_from_slice(&e.sector.to_le_bytes());
        out.extend_from_slice(&e.size.to_le_bytes());
        out.push(e.attr);
        out.push(e.name.len() as u8);
        out.extend_from_slice(&e.name);
        while out.len() % 4 != 0 {
            out.push(0xFF);
        }
    }
    out
}

/// An image under construction: `sectors` zero sectors.
pub struct Image {
    pub bytes: Vec<u8>,
}

impl Image {
    pub fn new(sectors: usize) -> Self {
        Image {
            bytes: vec![0; sectors * SECTOR],
        }
    }

    pub fn put(&mut self, at: usize, data: &[u8]) {
        self.bytes[at..at + data.len()].copy_from_slice(data);
    }

    pub fn volume(&mut self, root_sector: u32, root_size: u32) {
        let at = 32 * SECTOR;
        self.put(at, MAGIC);
        self.put(at + 0x14, &root_sector.to_le_bytes());
        self.put(at + 0x18, &root_size.to_le_bytes());
        self.put(at + 0x7EC, MAGIC);
    }

    /// A directory table in `sector`: the entries, then 0xFF to the end of
    /// `sectors` sectors.
    pub fn dir_table(&mut self, sector: usize, sectors: usize, entries: &[E]) {
        let at = sector * SECTOR;
        self.bytes[at..at + sectors * SECTOR].fill(0xFF);
        self.put(at, &table(entries));
    }
}

/// Byte i of Zeta.bin.
pub fn zeta_byte(i: usize) -> u8 {
    (i % 251) as u8
}

/// Byte offsets in the spec image's root table.
pub const ROOT_AT: usize = 33 * SECTOR;
pub const README_ENTRY: usize = ROOT_AT;
pub const MAPS_ENTRY: usize = ROOT_AT + 24;
pub const ZETA_ENTRY: usize = ROOT_AT + 44;

/// The spec's test case (a): 39 sectors, root table in 33.
pub fn spec_image() -> Vec<u8> {
    let mut img = Image::new(39);
    img.volume(33, 68);
    img.dir_table(
        33,
        1,
        &[
            E::file("readme.txt", 35, 5).links(6, 11),
            E::dir("Maps", 34, 2048),
            E::file("Zeta.bin", 36, 3000),
        ],
    );
    img.dir_table(34, 1, &[E::file("A_B.map", 38, 4)]);
    img.put(35 * SECTOR, b"HELLO");
    let zeta: Vec<u8> = (0..3000).map(zeta_byte).collect();
    img.put(36 * SECTOR, &zeta);
    img.put(38 * SECTOR, &[1, 2, 3, 4]);
    img.bytes
}

/// A scratch folder, removed when dropped.
pub struct Scratch(pub PathBuf);

impl Scratch {
    pub fn new() -> Self {
        static N: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "blam_formats_test_{}_{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
