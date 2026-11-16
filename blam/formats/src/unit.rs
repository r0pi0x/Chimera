//! A unit cut out of a map (specs/halo_ce/characters.md, section 8 "Each
//! character as a unit to fetch"): the closed set of tags a biped needs,
//! measured as the spec measures it and written as a small map file that
//! `MapFile::load` reads unchanged.
//!
//! - `Spans`: each tag's bytes in the tag data, from its main struct's
//!   address to the next tag's (tags lie contiguously in tag index order;
//!   `sbsp` tags have no struct here).
//! - `references`: the tag references inside a tag's span, found by their
//!   shape (16 bytes: a class the target is or derives from, the target's
//!   own path address, its exact id with the salt), so no class layout is
//!   needed to walk any tag.
//! - `closure`: every tag reachable from the roots, the tags `enter`
//!   refuses neither kept nor walked (the spec's exclusions,
//!   `character_excluded`: actor variants and actors, weapons and HUD
//!   interfaces).
//! - `measure`: the spec's size: tag data (the spans) plus raw data
//!   (bitmaps' images' pixels, sounds' permutations' samples, models'
//!   parts' 32-byte vertices and 2-byte strip indices), with the ranges
//!   each occupies (`raw_ranges`).
//! - `cut`: a map file holding only a unit: the original header, tag
//!   header, the whole tag index with every path string and the model
//!   buffer record arrays (so every id, path and address keeps its
//!   meaning), the unit's tag spans and raw ranges at their own addresses
//!   and file offsets, and zeros everywhere else. The image ends after the
//!   last raw byte kept, the tag data (whole: every index entry's struct
//!   address must lie in it) moved there (tag addresses are the tag
//!   cache's, not file offsets, so nothing in a tag changes); the body is
//!   one zlib stream at a fixed
//!   level. A tag outside the unit still has its index entry and path but
//!   zero bytes: a reference to it reads as an empty struct (blocks of
//!   count 0, null references), and the scenario's BSP list is empty.
//!
//! Deterministic: the same map gives the same bytes on any machine
//! (`BTreeSet` order, no floats, flate2's pure-Rust backend at a fixed
//! level; a change of that backend's output is a new converter version for
//! the caller).
//!
//! Known shortcuts:
//! - The image keeps the original file offsets of the raw data, so a unit
//!   whose sounds sit late in a large map inflates to that size in memory
//!   (zeros up to its last sample) although it is small on disk.

use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, Write},
};

use crate::{
    bitmap::Bitmap,
    cache::{
        ClassCode, HEADER_SIZE, MapFile, TAG_BASE, TAG_ENTRY_SIZE, TAG_HEADER_SIZE, TagEntry, TagId, class,
    },
    model::Model,
    sound::Sound,
};

/// The classes a character's unit never holds nor walks into (the spec's
/// closure: actor tags are the AI's, weapons are units of their own, the
/// HUD is the Spartan's).
pub const CHARACTER_EXCLUDED: [ClassCode; 6] = [
    class::ACTOR_VARIANT,
    class::ACTOR,
    class::WEAPON,
    class::UNIT_HUD_INTERFACE,
    class::WEAPON_HUD_INTERFACE,
    class::GRENADE_HUD_INTERFACE,
];

/// Whether a character's closure enters `t` (`CHARACTER_EXCLUDED`).
pub fn character_excluded(t: &TagEntry) -> bool {
    CHARACTER_EXCLUDED.contains(&t.class)
}

/// Every tag's span in the tag data (module doc).
pub struct Spans {
    /// (struct address, index) of every tag with one, by address.
    starts: Vec<(u32, u16)>,
    /// One past the tag data's last address.
    end: u32,
}

impl Spans {
    pub fn of(map: &MapFile) -> Spans {
        let mut starts: Vec<(u32, u16)> =
            map.tags().iter().filter(|t| t.address != 0).map(|t| (t.address, t.index())).collect();
        starts.sort_unstable();
        Spans { starts, end: TAG_BASE + map.header().tag_data_size }
    }

    /// `[start, end)` of tag `index`'s bytes (None: no struct here).
    pub fn span(&self, map: &MapFile, index: u16) -> Option<(u32, u32)> {
        let t = map.tags().get(index as usize)?;
        if t.address == 0 {
            return None;
        }
        let at = self.starts.partition_point(|&(a, _)| a < t.address);
        // Tags sharing an address (none on the disc) all get the span up to
        // the next distinct address.
        let next = self.starts[at..].iter().find(|&&(a, _)| a > t.address).map_or(self.end, |&(a, _)| a);
        Some((t.address, next))
    }
}

/// The tags referenced from tag `index`'s span (module doc), in order of
/// first appearance, without repeats.
pub fn references(map: &MapFile, spans: &Spans, index: u16) -> Vec<u16> {
    let Some((start, end)) = spans.span(map, index) else { return Vec::new() };
    let base = map.header().tag_data_offset as usize;
    let image = map.image();
    let at = |addr: u32| base + (addr - TAG_BASE) as usize;
    let Some(bytes) = image.get(at(start)..at(end)) else { return Vec::new() };
    let word = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().expect("4 bytes"));
    let mut out = Vec::new();
    let mut o = 0;
    while o + 16 <= bytes.len() {
        let id = TagId(word(o + 12));
        if let Some(t) = map.tag(id)
            && t.path_address == word(o + 4)
            && t.is_a(ClassCode::from_u32(word(o)))
            && !out.contains(&t.index())
        {
            out.push(t.index());
        }
        o += 4;
    }
    out
}

/// Every tag reachable from `roots` through references, `enter` false
/// for the tags neither kept nor walked.
pub fn closure(map: &MapFile, roots: &[u16], enter: impl Fn(&TagEntry) -> bool) -> BTreeSet<u16> {
    let spans = Spans::of(map);
    let mut seen = BTreeSet::new();
    let mut todo: Vec<u16> = roots.to_vec();
    while let Some(i) = todo.pop() {
        let Some(t) = map.tags().get(i as usize) else { continue };
        if !enter(t) || !seen.insert(i) {
            continue;
        }
        todo.extend(references(map, &spans, i).into_iter().rev());
    }
    seen
}

/// Where a tag's raw data lies: absolute file offsets (bitmaps' pixels,
/// sounds' samples) and tag data addresses (models' vertices and strips),
/// each with its length.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawRanges {
    pub file: Vec<(usize, usize)>,
    pub tags: Vec<(u32, usize)>,
}

/// Tag `t`'s raw data (module doc); none for a class without any.
pub fn raw_ranges(map: &MapFile, t: &TagEntry) -> io::Result<RawRanges> {
    let mut r = RawRanges::default();
    if t.class == class::BITMAP {
        for i in Bitmap::read(map, t)?.images {
            r.file.push((i.pixels_offset as usize, i.pixels_size as usize));
        }
    } else if t.class == class::SOUND {
        for range in Sound::read(map, t)?.pitch_ranges {
            for p in range.permutations {
                r.file.push((p.sample_offset, p.sample_size));
            }
        }
    } else if t.class == class::MODEL {
        for g in Model::read(map, t)?.geometries {
            for p in g.parts {
                r.tags.push((p.vertex_address, p.vertices.len() * 32));
                r.tags.push((p.index_address, p.strip.len() * 2));
            }
        }
    }
    r.file.retain(|&(_, n)| n > 0);
    r.tags.retain(|&(_, n)| n > 0);
    Ok(r)
}

/// A unit's tags and sizes as the spec counts them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Unit {
    pub tags: BTreeSet<u16>,
    /// The tags' spans, summed.
    pub tag_bytes: u64,
    /// The raw data, summed (each range once).
    pub raw_bytes: u64,
    /// Of the total, the sounds' (their tag data and samples: the spec's
    /// "of which sound").
    pub sound_bytes: u64,
    /// The tags per class (four-letter code), counted.
    pub classes: BTreeMap<String, usize>,
    pub raw: RawRanges,
}

impl Unit {
    pub fn total(&self) -> u64 {
        self.tag_bytes + self.raw_bytes
    }
}

/// The unit of `tags` measured (module doc).
pub fn measure(map: &MapFile, tags: BTreeSet<u16>) -> io::Result<Unit> {
    let spans = Spans::of(map);
    let mut u = Unit::default();
    let (mut file, mut tag_ranges) = (BTreeSet::new(), BTreeSet::new());
    for &i in &tags {
        let t = &map.tags()[i as usize];
        *u.classes.entry(t.class.to_string()).or_default() += 1;
        if let Some((a, b)) = spans.span(map, i) {
            u.tag_bytes += u64::from(b - a);
            if t.class == class::SOUND {
                u.sound_bytes += u64::from(b - a);
            }
        }
        let r = raw_ranges(map, t).map_err(|e| io::Error::new(e.kind(), format!("{} `{}`: {e}", t.class, t.path)))?;
        for range in r.file {
            if file.insert(range) && t.class == class::SOUND {
                u.sound_bytes += range.1 as u64;
            }
        }
        tag_ranges.extend(r.tags);
    }
    u.raw_bytes = file.iter().map(|r| r.1 as u64).sum::<u64>() + tag_ranges.iter().map(|r| r.1 as u64).sum::<u64>();
    u.raw = RawRanges { file: file.into_iter().collect(), tags: tag_ranges.into_iter().collect() };
    u.tags = tags;
    Ok(u)
}

/// The zlib level `cut` writes at.
pub const LEVEL: u32 = 9;

/// A map file holding only `unit` of `map` (module doc).
pub fn cut(map: &MapFile, unit: &Unit) -> io::Result<Vec<u8>> {
    let header = map.header();
    let th = map.tag_header();
    let image = map.image();
    let old_base = header.tag_data_offset as usize;
    let rel = |addr: u32| addr.wrapping_sub(TAG_BASE) as usize;
    // What of the tag data is kept, relative to its start.
    let mut keep: Vec<(usize, usize)> = vec![(0, TAG_HEADER_SIZE)];
    keep.push((rel(th.index_address), th.tag_count as usize * TAG_ENTRY_SIZE));
    for t in map.tags() {
        keep.push((rel(t.path_address), t.path.len() + 1));
    }
    keep.push((rel(th.vertex_buffer_address), th.vertex_buffer_count as usize * 12));
    keep.push((rel(th.index_buffer_address), th.index_buffer_count as usize * 12));
    let spans = Spans::of(map);
    for &i in &unit.tags {
        if let Some((a, b)) = spans.span(map, i) {
            keep.push((rel(a), (b - a) as usize));
        }
    }
    keep.extend(unit.raw.tags.iter().map(|&(a, n)| (rel(a), n)));
    // An empty range (a buffer array of count 0, address 0) keeps nothing.
    keep.retain(|&(_, n)| n > 0);
    let size = header.tag_data_size as usize;
    if let Some(bad) = keep.iter().find(|&&(o, n)| o.checked_add(n).is_none_or(|e| e > size)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{}: {} bytes at tag data offset {:#x} outside it", header.name, bad.1, bad.0),
        ));
    }
    // Whole: every index entry's struct address must stay inside it.
    let tag_end = size;
    // Raw data stays at its file offsets; the tag data follows the last of
    // it (the header and nothing else when there is none).
    let raw_end = unit.raw.file.iter().map(|&(o, n)| o + n).max().unwrap_or(HEADER_SIZE).max(HEADER_SIZE);
    if raw_end > image.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{}: raw data past the image", header.name)));
    }
    let new_base = raw_end.next_multiple_of(16);
    let total = new_base + tag_end;
    let mut out = vec![0u8; total];
    out[..HEADER_SIZE].copy_from_slice(&image[..HEADER_SIZE]);
    for &(o, n) in &unit.raw.file {
        out[o..o + n].copy_from_slice(&image[o..o + n]);
    }
    for &(o, n) in &keep {
        out[new_base + o..new_base + o + n].copy_from_slice(&image[old_base + o..old_base + o + n]);
    }
    let put = |out: &mut [u8], at: usize, v: u32| out[at..at + 4].copy_from_slice(&v.to_le_bytes());
    put(&mut out, 0x08, total as u32);
    put(&mut out, 0x0C, 0);
    put(&mut out, 0x10, new_base as u32);
    put(&mut out, 0x14, tag_end as u32);
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(LEVEL));
    z.write_all(&out[HEADER_SIZE..])?;
    let body = z.finish()?;
    let mut file = out[..HEADER_SIZE].to_vec();
    file.extend_from_slice(&body);
    Ok(file)
}
