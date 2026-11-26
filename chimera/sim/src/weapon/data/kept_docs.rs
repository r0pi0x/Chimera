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
//! self-describing), so the entry holds each table as a tagged value
//! (`ValueRef` writes it, `Read` reads it: a variant per kind of value,
//! a table as its entries in order, a float as its bits), read straight
//! into TOML's own tables (through a value type of its own and a copy
//! into tables it cost 969 M instructions for Combat Arms' set, 345 M of
//! them the copy: docs/performance.md, "Start cost, fifth pass"); a set
//! whose tables don't come back as they went in isn't written. Sets
//! under `KEEP_FROM` bytes are parsed as before (a game's own documents:
//! no entry each).

use chimera_core::mount::assets::map_cache::{Key, MapCache, cached};
use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{self, EnumAccess, SeqAccess, VariantAccess, Visitor},
};

/// The entries' version: bump when the encoding (`ValueRef`, `Read`)
/// changes, or the TOML parser starts giving other tables for the same
/// text.
const VERSION: &str = "1";

/// The least text (bytes, all documents) a set is kept from.
pub const KEEP_FROM: usize = 256 * 1024;

/// The encoding's enum and its variants, in their order (a variant is
/// written as its index).
const ENUM: &str = "Kept";
const VARIANTS: &[&str] = &["String", "Integer", "Float", "Boolean", "Datetime", "Array", "Table"];

/// A TOML value as the entry holds it (module doc): a string, an
/// integer, a float's bits (NaN and the infinities read back as they
/// were), a boolean, a datetime as TOML writes it, an array's values, a
/// table's entries in order.
struct ValueRef<'a>(&'a toml::Value);

/// A table as its variant of `ValueRef`.
struct TableRef<'a>(&'a toml::Table);

/// A table's entries, an array's values: the variants' payloads.
struct Entries<'a>(&'a toml::Table);
struct Values<'a>(&'a [toml::Value]);

impl Serialize for ValueRef<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let variant = |i: u32| (ENUM, i, VARIANTS[i as usize]);
        match self.0 {
            toml::Value::String(v) => {
                let (e, i, n) = variant(0);
                s.serialize_newtype_variant(e, i, n, v)
            }
            toml::Value::Integer(v) => {
                let (e, i, n) = variant(1);
                s.serialize_newtype_variant(e, i, n, v)
            }
            toml::Value::Float(v) => {
                let (e, i, n) = variant(2);
                s.serialize_newtype_variant(e, i, n, &v.to_bits())
            }
            toml::Value::Boolean(v) => {
                let (e, i, n) = variant(3);
                s.serialize_newtype_variant(e, i, n, v)
            }
            toml::Value::Datetime(v) => {
                let (e, i, n) = variant(4);
                s.serialize_newtype_variant(e, i, n, &v.to_string())
            }
            toml::Value::Array(v) => {
                let (e, i, n) = variant(5);
                s.serialize_newtype_variant(e, i, n, &Values(v))
            }
            toml::Value::Table(v) => TableRef(v).serialize(s),
        }
    }
}

impl Serialize for TableRef<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_newtype_variant(ENUM, 6, VARIANTS[6], &Entries(self.0))
    }
}

impl Serialize for Entries<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(|(k, v)| (k, ValueRef(v))))
    }
}

impl Serialize for Values<'_> {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(ValueRef))
    }
}

/// A value read from the entry as TOML's own (`ValueRef`'s encoding).
struct Read(toml::Value);

/// The payloads read: a table's entries, an array's values.
struct ReadEntries(toml::Table);
struct ReadValues(Vec<toml::Value>);

impl<'de> Deserialize<'de> for Read {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Kinds;
        impl<'de> Visitor<'de> for Kinds {
            type Value = Read;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a kept TOML value")
            }

            fn visit_enum<A: EnumAccess<'de>>(self, data: A) -> Result<Read, A::Error> {
                let (kind, payload) = data.variant::<u32>()?;
                Ok(Read(match kind {
                    0 => toml::Value::String(payload.newtype_variant()?),
                    1 => toml::Value::Integer(payload.newtype_variant()?),
                    2 => toml::Value::Float(f64::from_bits(payload.newtype_variant()?)),
                    3 => toml::Value::Boolean(payload.newtype_variant()?),
                    4 => toml::Value::Datetime(
                        payload.newtype_variant::<String>()?.parse().map_err(|_| de::Error::custom("a kept datetime that doesn't parse"))?,
                    ),
                    5 => toml::Value::Array(payload.newtype_variant::<ReadValues>()?.0),
                    6 => toml::Value::Table(payload.newtype_variant::<ReadEntries>()?.0),
                    other => return Err(de::Error::invalid_value(de::Unexpected::Unsigned(other.into()), &"a kind of TOML value")),
                }))
            }
        }
        d.deserialize_enum(ENUM, VARIANTS, Kinds)
    }
}

impl<'de> Deserialize<'de> for ReadEntries {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Rows;
        impl<'de> Visitor<'de> for Rows {
            type Value = ReadEntries;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a kept table's entries")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ReadEntries, A::Error> {
                let mut table = toml::Table::new();
                while let Some((key, value)) = seq.next_element::<(String, Read)>()? {
                    table.insert(key, value.0);
                }
                Ok(ReadEntries(table))
            }
        }
        d.deserialize_seq(Rows)
    }
}

impl<'de> Deserialize<'de> for ReadValues {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct Items;
        impl<'de> Visitor<'de> for Items {
            type Value = ReadValues;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a kept array's values")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<ReadValues, A::Error> {
                let mut values = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(4096));
                while let Some(value) = seq.next_element::<Read>()? {
                    values.push(value.0);
                }
                Ok(ReadValues(values))
            }
        }
        d.deserialize_seq(Items)
    }
}

/// A set's tables as an entry: written by `ValueRef`'s encoding, read
/// back as the tables themselves.
struct Tables(Vec<toml::Table>);

impl Serialize for Tables {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_seq(self.0.iter().map(TableRef))
    }
}

impl<'de> Deserialize<'de> for Tables {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Vec::<Read>::deserialize(d)?
            .into_iter()
            .map(|v| match v.0 {
                toml::Value::Table(t) => Ok(t),
                _ => Err(de::Error::custom("a kept document that isn't a table")),
            })
            .collect::<Result<_, _>>()
            .map(Tables)
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
    // Whether a miss parsed (its tables are then what `cached` gives, as
    // parsed: not through the encoding and back).
    let mut missed = false;
    let kept = cached(disk, &chimera_core::mount::Mount::default(), key, || {
        missed = true;
        let tables = Tables(parse(docs)?);
        let back = postcard::to_allocvec(&tables).ok().and_then(|b| postcard::from_bytes::<Tables>(&b).ok());
        if back.is_some_and(|b| b.0 == tables.0) { Ok(tables) } else { Err(String::new()) }
    });
    match kept {
        Ok((tables, _)) if tables.0.len() == docs.len() => Ok(tables.0),
        // Tables that don't read back the same (not written): parsed again.
        Err(e) if missed && e.is_empty() => parse(docs),
        Ok(_) => parse(docs),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DOC: &str = "id = \"t:gun\"\nslot = 1\ndraw = 0.5\nwhen = 1979-05-27T07:32:00Z\n[damage]\namount = inf\nfalloff = [1.0, -0.0, nan]\n\
                       [hooks.\"t:hook\"]\nnames = [\"a\", \"b\"]\nrows = [{ x = 1, on = true }, { x = -2 }]\n";

    /// The encoding as a derived type has it (what the entries written
    /// before `ValueRef` hold): the same bytes.
    #[derive(Serialize, Deserialize, PartialEq, Debug)]
    enum Kept {
        String(String),
        Integer(i64),
        Float(u64),
        Boolean(bool),
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
                toml::Value::Table(t) => Self::Table(t.iter().map(|(k, v)| (k.clone(), Self::of(v))).collect()),
            }
        }
    }

    /// A table comes back from the cache's encoding as it went in: every
    /// kind of value, the keys' order, the floats' bits; and the encoding
    /// is the derived enum's, byte for byte (an entry written through it
    /// reads back).
    #[test]
    fn a_table_reads_back_as_it_was_parsed() {
        let table: toml::Table = toml::from_str(DOC).unwrap();
        let bytes = postcard::to_allocvec(&Tables(vec![table.clone()])).unwrap();
        let derived = vec![Kept::of(&toml::Value::Table(table.clone()))];
        assert_eq!(bytes, postcard::to_allocvec(&derived).unwrap(), "the derived enum's bytes");
        assert_eq!(postcard::from_bytes::<Vec<Kept>>(&bytes).unwrap(), derived);
        let back = postcard::from_bytes::<Tables>(&bytes).unwrap().0.remove(0);
        // NaN isn't equal to itself: by the texts they write.
        assert_eq!(toml::to_string(&back).unwrap(), toml::to_string(&table).unwrap());
        assert_eq!(back.keys().collect::<Vec<_>>(), table.keys().collect::<Vec<_>>());
        let bits = |t: &toml::Table| t["damage"]["falloff"].as_array().unwrap().iter().map(|f| f.as_float().unwrap().to_bits()).collect::<Vec<_>>();
        assert_eq!(bits(&back), bits(&table), "-0.0 and NaN by their bits");
        // A document that isn't a table, a kind that isn't one: refused.
        assert!(postcard::from_bytes::<Tables>(&postcard::to_allocvec(&vec![Kept::Integer(1)]).unwrap()).is_err());
        assert!(postcard::from_bytes::<Tables>(&[1, 7, 0]).is_err());
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
