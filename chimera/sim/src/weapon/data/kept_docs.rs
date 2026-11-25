//! Weapon documents' tables kept on disk: a big set's TOML (Combat Arms'
//! install merged over ours: 2,363 documents, 16 MB) took 3.1 G
//! instructions to parse at every start (docs/performance.md, "Start
//! cost, third pass"). `tables` reads the parsed tables back from the
//! converted cache (chimera_assets' `map_cache`; the kept dressings'
//! layer, `map::kept::disk_cache`) when the same texts were parsed
//! before, else parses and writes them. The key is every document's name
//! and text and `VERSION`; no file is read, so an entry has no inputs.
//!
//! The cache's encoding (postcard) can't read a `toml::Value` (it isn't
//! self-describing), so the entry holds each table as a `Kept`, the same
//! values in the same order; a set whose tables don't come back as they
//! went in isn't written. Sets under `KEEP_FROM` bytes are parsed as
//! before (a game's own documents: no entry each).

use chimera_core::mount::assets::map_cache::{Key, MapCache, cached};
use serde::{Deserialize, Serialize};

/// The entries' version: bump when `Kept` changes, or the TOML parser
/// starts giving other tables for the same text.
const VERSION: &str = "1";

/// The least text (bytes, all documents) a set is kept from.
pub const KEEP_FROM: usize = 256 * 1024;

/// A TOML value as the cache keeps it (module doc).
#[derive(Serialize, Deserialize, PartialEq)]
enum Kept {
    String(String),
    Integer(i64),
    /// The bits: NaN and the infinities read back as they were.
    Float(u64),
    Boolean(bool),
    /// As TOML writes it.
    Datetime(String),
    Array(Vec<Kept>),
    Table(Vec<(String, Kept)>),
}

impl Kept {
    fn of(v: &toml::Value) -> Self {
        match v {
            toml::Value::String(s) => Self::String(s.clone()),
            toml::Value::Integer(i) => Self::Integer(*i),
            toml::Value::Float(f) => Self::Float(f.to_bits()),
            toml::Value::Boolean(b) => Self::Boolean(*b),
            toml::Value::Datetime(d) => Self::Datetime(d.to_string()),
            toml::Value::Array(a) => Self::Array(a.iter().map(Self::of).collect()),
            toml::Value::Table(t) => Self::of_table(t),
        }
    }

    fn of_table(t: &toml::Table) -> Self {
        Self::Table(t.iter().map(|(k, v)| (k.clone(), Self::of(v))).collect())
    }

    /// None: a datetime that doesn't parse (never written by `of`).
    fn value(self) -> Option<toml::Value> {
        Some(match self {
            Self::String(s) => toml::Value::String(s),
            Self::Integer(i) => toml::Value::Integer(i),
            Self::Float(f) => toml::Value::Float(f64::from_bits(f)),
            Self::Boolean(b) => toml::Value::Boolean(b),
            Self::Datetime(d) => toml::Value::Datetime(d.parse().ok()?),
            Self::Array(a) => toml::Value::Array(a.into_iter().map(Self::value).collect::<Option<_>>()?),
            Self::Table(_) => toml::Value::Table(self.table()?),
        })
    }

    fn table(self) -> Option<toml::Table> {
        let Self::Table(entries) = self else { return None };
        entries.into_iter().map(|(k, v)| Some((k, v.value()?))).collect()
    }
}

/// The cache `WeaponSet::parse` keeps `docs`' tables in: the kept
/// dressings' disk layer when the set is big enough (`KEEP_FROM`).
pub fn disk(docs: &[(&str, &str)]) -> Option<MapCache> {
    (docs.iter().map(|(_, t)| t.len()).sum::<usize>() >= KEEP_FROM)
        .then(chimera_world::map::kept::disk_cache)
        .flatten()
}

fn parse(docs: &[(&str, &str)]) -> Result<Vec<toml::Table>, String> {
    docs.iter().map(|(name, text)| toml::from_str(text).map_err(|e| format!("{name}: {e}"))).collect()
}

/// Each document's table, in `docs`' order: read back from `disk` when
/// these texts were parsed before, else parsed (and written, when they
/// read back the same). The error is the parser's, by document name.
pub fn tables(docs: &[(&str, &str)], disk: Option<&MapCache>) -> Result<Vec<toml::Table>, String> {
    if disk.is_none() {
        return parse(docs);
    }
    let mut key = Key::new("weapon_docs", VERSION);
    for (name, text) in docs {
        key = key.part(name, text);
    }
    // What a miss parsed, as it is: not through its encoding and back.
    let mut parsed = None;
    let kept = cached(disk, &chimera_core::mount::Mount::default(), key, || {
        let tables = parse(docs)?;
        let kept: Vec<Kept> = tables.iter().map(Kept::of_table).collect();
        let back: Option<Vec<toml::Table>> = postcard::to_allocvec(&kept)
            .ok()
            .and_then(|b| postcard::from_bytes::<Vec<Kept>>(&b).ok())
            .and_then(|k| k.into_iter().map(Kept::table).collect());
        let same = back.as_ref() == Some(&tables);
        parsed = Some(tables);
        if same { Ok(kept) } else { Err(String::new()) }
    });
    match (parsed, kept) {
        (Some(tables), _) => Ok(tables),
        (None, Ok((kept, _))) => match kept.into_iter().map(Kept::table).collect::<Option<Vec<_>>>() {
            Some(tables) if tables.len() == docs.len() => Ok(tables),
            _ => parse(docs),
        },
        (None, Err(e)) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "id = \"t:gun\"\nslot = 1\ndraw = 0.5\nwhen = 1979-05-27T07:32:00Z\n[damage]\namount = inf\nfalloff = [1.0, -0.0, nan]\n\
                       [hooks.\"t:hook\"]\nnames = [\"a\", \"b\"]\nrows = [{ x = 1, on = true }, { x = -2 }]\n";

    /// A table comes back from the cache's encoding as it went in: every
    /// kind of value, the keys' order, the floats' bits.
    #[test]
    fn a_table_reads_back_as_it_was_parsed() {
        let table: toml::Table = toml::from_str(DOC).unwrap();
        let bytes = postcard::to_allocvec(&Kept::of_table(&table)).unwrap();
        let back = postcard::from_bytes::<Kept>(&bytes).unwrap().table().unwrap();
        // NaN isn't equal to itself: by the texts they write.
        assert_eq!(toml::to_string(&back).unwrap(), toml::to_string(&table).unwrap());
        assert_eq!(back.keys().collect::<Vec<_>>(), table.keys().collect::<Vec<_>>());
    }

    /// A set read back from the cache is the set parsed: the first call
    /// writes its entry, the second reads it and writes none; another
    /// text is another entry, and a text that doesn't parse gives the
    /// parser's error.
    #[test]
    fn a_set_read_back_is_the_set_parsed() {
        let dir = std::env::temp_dir().join(format!("chimera-kept-docs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let cache = MapCache::at(&dir, 1 << 30);
        let docs = [("a.toml", "id = \"t:a\"\nslot = 1\ndraw = 0.5\n"), ("b.toml", "id = \"t:b\"\nbase = \"t:a\"\ndraw = 1.5\n[units]\nlength = 0.5\n")];
        let plain = parse(&docs).unwrap();
        assert_eq!(tables(&docs, Some(&cache)).unwrap(), plain, "a miss gives what was parsed");
        let entries = || std::fs::read_dir(&dir).unwrap().filter(|e| e.as_ref().unwrap().path().extension().is_some_and(|x| x == "map")).count();
        assert_eq!(entries(), 1, "the miss wrote its entry");
        assert_eq!(tables(&docs, Some(&cache)).unwrap(), plain, "a hit gives the same tables");
        assert_eq!(entries(), 1, "the hit wrote none");
        // Another text is another set; a bad one's error is the parser's.
        let other = [("a.toml", "id = \"t:a\"\nslot = 2\ndraw = 0.5\n")];
        assert_eq!(tables(&other, Some(&cache)).unwrap(), parse(&other).unwrap());
        assert_eq!(entries(), 2);
        let bad = [("bad.toml", "id = ")];
        assert_eq!(tables(&bad, Some(&cache)).unwrap_err(), parse(&bad).unwrap_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
