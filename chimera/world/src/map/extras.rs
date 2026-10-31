//! A map's typed extensions (`MapData::extras`): features only some
//! games' maps have, kept apart from `MapData` so a game's own map
//! features don't widen Chimera's struct (docs/plans/active/
//! architecture-review.md, A2 3).
//!
//! One value per type, keyed by its `TypeId` in a `BTreeMap` (a stable
//! order). A loader fills a value with `MapExtras::entry`, a reader asks
//! with `get`; a map without the value reads as absent. `SourceEffects` is
//! the first: Source's entity-driven effect lists (steam jets, tesla arcs,
//! strike beams, embers, smoke stacks, dust motes, detail sprites), filled
//! by the Source map loader and read by the renderer.
//!
//! The converted-map cache (`cache`) serializes the values: each as its
//! type's name and its postcard bytes. Reading one back needs its type in
//! `KNOWN` (a type missing there makes the entry unreadable, so the map
//! is converted again: a miss, never a wrong map).

use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::{MapDetailProps, MapDust, emitters, steam};

/// What an extension type is called in a serialized map.
fn name_of<T>() -> &'static str {
    std::any::type_name::<T>()
}

type Read = fn(&[u8]) -> Option<(TypeId, Box<dyn Extra>)>;

fn read<T: Any + Clone + Send + Sync + Serialize + DeserializeOwned>(bytes: &[u8]) -> Option<(TypeId, Box<dyn Extra>)> {
    let value: T = postcard::from_bytes(bytes).ok()?;
    Some((TypeId::of::<T>(), Box::new(value)))
}

/// The extension types a serialized map can hold (`cache`): add a new
/// one here.
fn known() -> [(&'static str, Read); 1] {
    [(name_of::<SourceEffects>(), read::<SourceEffects>)]
}

/// One extension value: any `Clone` type that serializes, cloneable
/// behind the trait.
trait Extra: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn Extra>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error>;
}

impl<T: Any + Clone + Send + Sync + Serialize> Extra for T {
    fn clone_box(&self) -> Box<dyn Extra> {
        Box::new(self.clone())
    }

    fn to_bytes(&self) -> Result<Vec<u8>, postcard::Error> {
        postcard::to_allocvec(self)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A map's extensions, one value per type.
#[derive(Default)]
pub struct MapExtras(BTreeMap<TypeId, (&'static str, Box<dyn Extra>)>);

impl MapExtras {
    /// The value of type `T`, if the map has one.
    pub fn get<T: Any + Clone + Send + Sync>(&self) -> Option<&T> {
        self.0.get(&TypeId::of::<T>())?.1.as_any().downcast_ref()
    }

    /// The value of type `T` to change, if the map has one.
    pub fn get_mut<T: Any + Clone + Send + Sync>(&mut self) -> Option<&mut T> {
        self.0.get_mut(&TypeId::of::<T>())?.1.as_any_mut().downcast_mut()
    }

    /// The value of type `T`, added (`T::default()`) when the map has none.
    pub fn entry<T: Any + Clone + Default + Send + Sync + Serialize>(&mut self) -> &mut T {
        let slot = self
            .0
            .entry(TypeId::of::<T>())
            .or_insert_with(|| (std::any::type_name::<T>(), Box::new(T::default())));
        slot.1.as_any_mut().downcast_mut().expect("keyed by its type")
    }

    /// Set the value of type `T`.
    pub fn insert<T: Any + Clone + Send + Sync + Serialize>(&mut self, value: T) {
        self.0
            .insert(TypeId::of::<T>(), (std::any::type_name::<T>(), Box::new(value)));
    }

    /// Take the value of type `T` out.
    pub fn remove<T: Any + Clone + Send + Sync>(&mut self) {
        self.0.remove(&TypeId::of::<T>());
    }

    /// No extensions.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Clone for MapExtras {
    fn clone(&self) -> Self {
        Self(self.0.iter().map(|(k, (n, v))| (*k, (*n, (**v).clone_box()))).collect())
    }
}

impl Serialize for MapExtras {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut out = Vec::with_capacity(self.0.len());
        for (name, value) in self.0.values() {
            out.push((*name, value.to_bytes().map_err(serde::ser::Error::custom)?));
        }
        out.serialize(s)
    }
}

impl<'de> Deserialize<'de> for MapExtras {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let values: Vec<(String, Vec<u8>)> = Vec::deserialize(d)?;
        let known = known();
        let mut out = BTreeMap::new();
        for (name, bytes) in values {
            let (static_name, read) = known
                .iter()
                .find(|(n, _)| *n == name)
                .ok_or_else(|| serde::de::Error::custom(format!("unknown map extension {name}")))?;
            let (id, value) = read(&bytes).ok_or_else(|| serde::de::Error::custom(format!("map extension {name}")))?;
            out.insert(id, (*static_name, value));
        }
        Ok(Self(out))
    }
}

impl std::fmt::Debug for MapExtras {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.values().map(|(name, _)| name)).finish()
    }
}

/// Source's entity-driven effect lists on a map: what a map made for
/// another game doesn't have (`MapData::source_fx`).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SourceEffects {
    /// Detail props' sprites (grass, weeds): `detail`.
    pub detail_props: Option<MapDetailProps>,
    /// Dust motes (`func_dustmotes`, `func_dustcloud`).
    pub dust: Vec<MapDust>,
    /// Steam jets (env_steam).
    pub steam: Vec<steam::MapSteam>,
    /// Smoke stacks (env_smokestack): `emitters`.
    pub smokestacks: Vec<emitters::MapSmokeStack>,
    /// point_tesla arcs (`emitters`).
    pub teslas: Vec<emitters::MapTesla>,
    /// Strike-generator env_beams' looks (`emitters::StrikeEmitter`).
    pub strikes: Vec<emitters::MapStrike>,
    /// env_embers volumes (`emitters::EmbersEmitter`).
    pub embers: Vec<emitters::MapEmbers>,
}

impl SourceEffects {
    /// A map with none: what `MapData::source_fx` reads for a map that
    /// has no `SourceEffects`.
    pub const NONE: SourceEffects = SourceEffects {
        detail_props: None,
        dust: Vec::new(),
        steam: Vec::new(),
        smokestacks: Vec::new(),
        teslas: Vec::new(),
        strikes: Vec::new(),
        embers: Vec::new(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq, Serialize)]
    struct A(u32);

    #[derive(Clone, Debug, Default, PartialEq, Serialize)]
    struct B(String);

    #[test]
    fn one_value_per_type() {
        let mut x = MapExtras::default();
        assert!(x.is_empty() && x.get::<A>().is_none());
        x.entry::<A>().0 = 4;
        x.insert(B("b".into()));
        assert_eq!(x.get::<A>(), Some(&A(4)));
        x.entry::<A>().0 += 1;
        let copy = x.clone();
        x.remove::<A>();
        assert!(x.get::<A>().is_none());
        assert_eq!(copy.get::<A>(), Some(&A(5)), "a clone has its own values");
        assert_eq!(copy.get::<B>(), Some(&B("b".into())));
        assert!(format!("{copy:?}").matches("::").count() >= 2);
    }

    #[test]
    fn known_extensions_serialize_and_unknown_ones_fail() {
        let mut x = MapExtras::default();
        x.entry::<SourceEffects>();
        let back: MapExtras = postcard::from_bytes(&postcard::to_allocvec(&x).unwrap()).unwrap();
        assert!(back.get::<SourceEffects>().is_some_and(|e| e.dust.is_empty()));
        x.insert(A(1));
        let bytes = postcard::to_allocvec(&x).unwrap();
        assert!(postcard::from_bytes::<MapExtras>(&bytes).is_err(), "A isn't in `known`");
    }

    #[test]
    fn a_map_without_the_effects_reads_none() {
        let map = crate::map::MapData::default();
        assert!(map.source_fx().dust.is_empty() && map.source_fx().detail_props.is_none());
    }
}
