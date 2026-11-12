//! Data formats (docs/plans/active/multiplayer.md, "Server data
//! documents"): what shape of data this build reads, so a client and a
//! server of different builds find out at the join, before anything is
//! downloaded, that the server's documents won't load here (chimera_net's
//! `net::formats`).
//!
//! - **A format** is a text describing a type's serde shape, made by
//!   `describe::<T>()`, which drives `T`'s own `Deserialize` with a tracing
//!   deserializer (written here): each struct's field names, each enum's
//!   variants, the kinds of the values (bool, integer widths, strings,
//!   options, lists, maps), by path. A field added to a hook's parameters
//!   (`deny_unknown_fields`: an older build can't read the newer data)
//!   changes it. Nothing is written by hand, so a change can't forget to
//!   change it.
//! - **What has one** (`DataFormats`, `formats`): the weapon document
//!   (`chimera:weapon`: `WeaponData`, `AmmoData`, `Units`, recorded by
//!   `add_weapon_set`: `weapon::data::weapon_format`), every weapon hook's
//!   parameters by the hook's name (`register_weapon_hook` records it),
//!   and every other server document set by its name (`DocApply::With`
//!   names its format). `fingerprint` hashes a format (FNV-1a, the same on
//!   every platform and compiler).
//! - **Tracing**: a value the type can't take from the tracer (a
//!   validation, an internally tagged enum, something read with
//!   `deserialize_any`) fails that pass; the last value given (`""` or
//!   0, then `"x"` or 1), else the innermost choice open there (an option,
//!   a list, a map, an enum's variant, an `any`'s kind) is taken the other
//!   way in the next pass (None, empty, the next variant, the next kind),
//!   up to `MAX_PASSES`. The format is every line any pass
//!   recorded, sorted.
//!
//! Known shortcuts: only one variant of an enum is traced into (the first
//! that traces; the variants' names are all there); an internally tagged
//! or untagged enum reads its content through serde's own buffer, so only
//! its place is recorded, not its fields; a hand-written parser's checks
//! (Combat Arms' `parse_mods`) beyond the type it reads into aren't seen.

use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fmt,
};

use bevy::prelude::*;
use serde::de::{
    self, DeserializeOwned, DeserializeSeed, EnumAccess, IntoDeserializer, MapAccess, SeqAccess, VariantAccess, Visitor,
};

/// Passes a trace takes at most.
pub const MAX_PASSES: usize = 256;
/// Nesting a trace follows at most (a recursive type stops there).
const MAX_DEPTH: usize = 32;

/// The name of the weapon document's format.
pub const WEAPON: &str = "chimera:weapon";

/// Formats this app reads its served data in, by name (the weapon
/// document's, a hook's, a set's).
#[derive(Resource, Clone, Debug, Default)]
pub struct DataFormats(pub BTreeMap<String, String>);

/// The serde shape of `T`, as text (deterministic: the same type gives the
/// same text in any build).
pub fn describe<T: DeserializeOwned>() -> String {
    let state = RefCell::new(State::default());
    for _ in 0..MAX_PASSES {
        {
            let mut s = state.borrow_mut();
            s.failed = None;
            s.open.clear();
            s.last_leaf = None;
        }
        let ok = T::deserialize(Tracer {
            st: &state,
            path: String::new(),
            depth: 0,
        })
        .is_ok();
        let mut s = state.borrow_mut();
        if ok {
            break;
        }
        match s.failed.take() {
            Some(Some(site)) => *s.taken.entry(site).or_default() += 1,
            // No choice left: what was recorded is the format.
            _ => break,
        }
    }
    let s = state.into_inner();
    s.lines.into_iter().collect::<Vec<_>>().join("\n")
}

/// A format's hash (FNV-1a, 32 bits).
pub fn fingerprint(format: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in format.bytes() {
        h ^= u32::from(b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// Every format this world reads served data in, by name.
pub fn formats(world: &World) -> BTreeMap<String, String> {
    world.get_resource::<DataFormats>().map(|f| f.0.clone()).unwrap_or_default()
}

#[derive(Default)]
struct State {
    lines: BTreeSet<String>,
    /// Choice sites by path, and how many times each was taken another way.
    taken: BTreeMap<String, u32>,
    /// This pass's choices open now: (path, ways it can go).
    open: Vec<(String, u32)>,
    /// This pass failed: the choice to take another way next (None: none
    /// can).
    failed: Option<Option<String>>,
    /// The last value given (a leaf: two ways, e.g. "" and "x"), which a
    /// check after it (`deserialize_with`) may have refused.
    last_leaf: Option<String>,
}

impl State {
    fn way(&self, site: &str) -> u32 {
        self.taken.get(site).copied().unwrap_or(0)
    }

    /// A value failed: note the innermost open choice that can still go
    /// another way (once per pass: the first failure is the innermost).
    fn fail(&mut self) {
        if self.failed.is_some() {
            return;
        }
        if let Some(leaf) = self.last_leaf.take()
            && self.way(&leaf) == 0
        {
            self.failed = Some(Some(leaf));
            return;
        }
        let site = self
            .open
            .iter()
            .rev()
            .find(|(site, ways)| self.way(site) + 1 < *ways)
            .map(|(site, _)| site.clone());
        self.failed = Some(site);
    }
}

#[derive(Debug)]
struct TraceError(String);

impl fmt::Display for TraceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for TraceError {}

impl de::Error for TraceError {
    fn custom<T: fmt::Display>(msg: T) -> Self {
        Self(msg.to_string())
    }
}

#[derive(Clone)]
struct Tracer<'a> {
    st: &'a RefCell<State>,
    path: String,
    depth: usize,
}

impl<'a> Tracer<'a> {
    fn at(&self, part: &str) -> Self {
        Self {
            st: self.st,
            path: format!("{}{part}", self.path),
            depth: self.depth + 1,
        }
    }

    fn line(&self, kind: &str) {
        self.st.borrow_mut().lines.insert(format!(
            "{} {kind}",
            if self.path.is_empty() { "." } else { &self.path }
        ));
    }

    /// Open a choice of `ways` at `site`; the way to take this pass.
    fn open(&self, site: &str, ways: u32) -> u32 {
        let mut s = self.st.borrow_mut();
        s.open.push((site.to_string(), ways));
        s.way(site).min(ways - 1)
    }

    /// A leaf value: which of its two to give this pass.
    fn leaf(&self) -> u32 {
        let site = format!("{}=", self.path);
        let mut s = self.st.borrow_mut();
        let way = s.way(&site).min(1);
        s.last_leaf = Some(site);
        way
    }

    fn close(&self) {
        self.st.borrow_mut().open.pop();
    }

    /// A visit's result: a failure noted.
    fn seen<T>(&self, r: Result<T, TraceError>) -> Result<T, TraceError> {
        if r.is_err() {
            self.st.borrow_mut().fail();
        }
        r
    }

    fn deep(&self) -> Result<(), TraceError> {
        if self.depth > MAX_DEPTH {
            self.st.borrow_mut().fail();
            return Err(TraceError("too deep".into()));
        }
        Ok(())
    }
}

macro_rules! primitive {
    ($($method:ident => $kind:literal, $visit:ident($a:expr, $b:expr);)*) => {
        $(
            fn $method<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
                self.line($kind);
                let r = if self.leaf() == 0 { visitor.$visit($a) } else { visitor.$visit($b) };
                self.seen(r)
            }
        )*
    };
}

impl<'de, 'a> de::Deserializer<'de> for Tracer<'a> {
    type Error = TraceError;

    primitive! {
        deserialize_bool => "bool", visit_bool(false, true);
        deserialize_i8 => "i8", visit_i8(0, 1);
        deserialize_i16 => "i16", visit_i16(0, 1);
        deserialize_i32 => "i32", visit_i32(0, 1);
        deserialize_i64 => "i64", visit_i64(0, 1);
        deserialize_i128 => "i128", visit_i128(0, 1);
        deserialize_u8 => "u8", visit_u8(0, 1);
        deserialize_u16 => "u16", visit_u16(0, 1);
        deserialize_u32 => "u32", visit_u32(0, 1);
        deserialize_u64 => "u64", visit_u64(0, 1);
        deserialize_u128 => "u128", visit_u128(0, 1);
        deserialize_f32 => "f32", visit_f32(0.0, 1.0);
        deserialize_f64 => "f64", visit_f64(0.0, 1.0);
        deserialize_char => "char", visit_char('a', 'b');
        deserialize_str => "str", visit_str("", "x");
        deserialize_string => "str", visit_str("", "x");
        deserialize_bytes => "bytes", visit_bytes(&[], b"x");
        deserialize_byte_buf => "bytes", visit_bytes(&[], b"x");
        deserialize_identifier => "ident", visit_str("", "x");
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.line("unit");
        let r = visitor.visit_unit();
        self.seen(r)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        let r = visitor.visit_unit();
        self.seen(r)
    }

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deep()?;
        self.line("any");
        let site = format!("{}#any", self.path);
        let r = match self.open(&site, 6) {
            0 => visitor.visit_map(Entries {
                tracer: self.clone(),
                fields: &[],
                at: 0,
                any: true,
            }),
            1 => visitor.visit_str(""),
            2 => visitor.visit_i64(0),
            3 => visitor.visit_f64(0.0),
            4 => visitor.visit_bool(false),
            _ => visitor.visit_seq(Items {
                tracer: self.at("[]"),
                left: 0,
            }),
        };
        let r = self.seen(r);
        self.close();
        r
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deep()?;
        self.line("option");
        let site = format!("{}?", self.path);
        let r = match self.open(&site, 2) {
            0 => visitor.visit_some(self.at("")),
            _ => visitor.visit_none(),
        };
        let r = self.seen(r);
        self.close();
        r
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(self, _: &'static str, visitor: V) -> Result<V::Value, Self::Error> {
        self.line("unit");
        let r = visitor.visit_unit();
        self.seen(r)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(self, _: &'static str, visitor: V) -> Result<V::Value, Self::Error> {
        self.deep()?;
        let r = visitor.visit_newtype_struct(self.at(""));
        self.seen(r)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deep()?;
        self.line("seq");
        let site = format!("{}[]", self.path);
        let left = if self.open(&site, 2) == 0 { 1 } else { 0 };
        let r = visitor.visit_seq(Items {
            tracer: self.at("[]"),
            left,
        });
        let r = self.seen(r);
        self.close();
        r
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error> {
        self.deep()?;
        self.line(&format!("tuple{len}"));
        let r = visitor.visit_seq(Items {
            tracer: self.at("()"),
            left: len,
        });
        self.seen(r)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deserialize_tuple(len, visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Self::Error> {
        self.deep()?;
        self.line("map");
        let site = format!("{}{{}}", self.path);
        let one = self.open(&site, 2) == 0;
        let r = visitor.visit_map(Entries {
            tracer: self.clone(),
            fields: &[],
            at: if one { 0 } else { 1 },
            any: false,
        });
        let r = self.seen(r);
        self.close();
        r
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deep()?;
        self.line(&format!("struct {}", fields.join(",")));
        let r = visitor.visit_map(Entries {
            tracer: self.clone(),
            fields,
            at: 0,
            any: false,
        });
        self.seen(r)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        self.deep()?;
        self.line(&format!("enum {}", variants.join(",")));
        if variants.is_empty() {
            return self.seen(Err(TraceError("no variants".into())));
        }
        let site = format!("{}::", self.path);
        let way = self.open(&site, variants.len() as u32);
        let r = visitor.visit_enum(Variant {
            tracer: self.at(&format!("::{}", variants[way as usize])),
            index: way,
        });
        let r = self.seen(r);
        self.close();
        r
    }
}

/// A list's items: `left` more, each traced.
struct Items<'a> {
    tracer: Tracer<'a>,
    left: usize,
}

impl<'de, 'a> SeqAccess<'de> for Items<'a> {
    type Error = TraceError;

    fn next_element_seed<T: DeserializeSeed<'de>>(&mut self, seed: T) -> Result<Option<T::Value>, Self::Error> {
        if self.left == 0 {
            return Ok(None);
        }
        self.left -= 1;
        seed.deserialize(self.tracer.clone()).map(Some)
    }
}

/// A struct's fields (each by name) or a map's one entry (`at` 0) or none.
struct Entries<'a> {
    tracer: Tracer<'a>,
    fields: &'static [&'static str],
    at: usize,
    /// An `any` read as a map: no entries.
    any: bool,
}

impl<'de, 'a> MapAccess<'de> for Entries<'a> {
    type Error = TraceError;

    fn next_key_seed<K: DeserializeSeed<'de>>(&mut self, seed: K) -> Result<Option<K::Value>, Self::Error> {
        if self.any {
            return Ok(None);
        }
        if self.fields.is_empty() {
            // A map: one entry, its key traced.
            if self.at > 0 {
                return Ok(None);
            }
            return seed.deserialize(self.tracer.at("{key}")).map(Some);
        }
        let Some(name) = self.fields.get(self.at) else {
            return Ok(None);
        };
        let key: de::value::StrDeserializer<'_, TraceError> = name.into_deserializer();
        seed.deserialize(key).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Self::Error> {
        let tracer = if self.fields.is_empty() {
            self.tracer.at("{}")
        } else {
            self.tracer.at(&format!(".{}", self.fields[self.at]))
        };
        self.at += 1;
        seed.deserialize(tracer)
    }
}

/// An enum's variant (`index`), its content traced.
struct Variant<'a> {
    tracer: Tracer<'a>,
    index: u32,
}

impl<'de, 'a> EnumAccess<'de> for Variant<'a> {
    type Error = TraceError;
    type Variant = Self;

    fn variant_seed<V: DeserializeSeed<'de>>(self, seed: V) -> Result<(V::Value, Self), Self::Error> {
        let index: de::value::U32Deserializer<TraceError> = self.index.into_deserializer();
        let v = seed.deserialize(index)?;
        Ok((v, self))
    }
}

impl<'de, 'a> VariantAccess<'de> for Variant<'a> {
    type Error = TraceError;

    fn unit_variant(self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Self::Error> {
        seed.deserialize(self.tracer)
    }

    fn tuple_variant<V: Visitor<'de>>(self, len: usize, visitor: V) -> Result<V::Value, Self::Error> {
        de::Deserializer::deserialize_tuple(self.tracer, len, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Self::Error> {
        de::Deserializer::deserialize_struct(self.tracer, "", fields, visitor)
    }
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[allow(dead_code)]
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Old {
        a: f32,
        #[serde(default)]
        b: Option<Inner>,
        list: Vec<Inner>,
        kind: Kind,
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct New {
        a: f32,
        #[serde(default)]
        b: Option<Inner>,
        list: Vec<Inner>,
        kind: Kind,
        #[serde(default)]
        hidden_pieces: Vec<String>,
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct Inner {
        x: u8,
        name: String,
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct InnerMore {
        x: u8,
        name: String,
        y: bool,
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct Nested {
        list: Vec<InnerMore>,
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct NestedOld {
        list: Vec<Inner>,
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum Kind {
        One,
        Two(f32),
    }

    /// Fails on the tracer's empty string: the trace takes the option's
    /// None and still records the fields after it.
    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct Picky {
        #[serde(default, deserialize_with = "not_empty")]
        first: Option<String>,
        tagged: Option<Tagged>,
        last: Inner,
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    #[serde(tag = "kind")]
    enum Tagged {
        A { x: f32 },
    }

    fn not_empty<'de, D: de::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        let s = String::deserialize(d)?;
        if s.is_empty() {
            Err(de::Error::custom("empty"))
        } else {
            Ok(Some(s))
        }
    }

    #[allow(dead_code)]
    #[derive(Deserialize)]
    struct Tree {
        children: Vec<Tree>,
        v: i32,
    }

    #[test]
    fn a_field_added_changes_the_format() {
        let (old, new) = (describe::<Old>(), describe::<New>());
        assert_ne!(old, new);
        assert!(new.contains("hidden_pieces"), "{new}");
        assert!(
            old.contains(".list[].name str") && old.contains("enum one,two"),
            "{old}"
        );
        assert_eq!(describe::<Old>(), old, "the same type, the same text");
        assert_ne!(fingerprint(&old), fingerprint(&new));
        // Nested, inside a list.
        assert_ne!(describe::<Nested>(), describe::<NestedOld>());
    }

    #[test]
    fn a_trace_gets_past_values_it_cant_give() {
        let p = describe::<Picky>();
        assert!(p.contains("struct first,tagged,last"), "{p}");
        assert!(
            p.contains(".last.name str"),
            "the fields after the failures are traced: {p}"
        );
        // Recursion stops.
        let t = describe::<Tree>();
        assert!(t.contains(".v i32"), "{t}");
        // toml values (a hook's free table) trace as `any`.
        assert!(describe::<toml::Table>().contains("map"));
    }
}
