//! What a map's load worked out for the game that plays it, kept with the
//! map (`MapData::memos`) and, once it spawns, as the world's `MapMemos`
//! resource until the map is unloaded (map-scoped): Combat Arms' rigs,
//! body groups, held models' sockets and effect library, which its
//! systems read while the map plays. One value per type, keyed by its
//! `TypeId`, as `MapExtras`; unlike those they are not in the
//! converted-map cache (a loader adds them after a cache hit, as it adds
//! the game's kit) and a clone is a copy (each world changes its own).
//!
//! What lives longer than a map (an app's, a process's install data) is
//! not here: an app's is in `sources::LoadContext`, an install's on the
//! game's own data (docs/DESIGN.md, "A game's loader and memos").

use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
};

use bevy::prelude::*;

/// One memo value: any `Clone` type, cloneable behind the trait.
trait Memo: Any + Send + Sync {
    fn clone_box(&self) -> Box<dyn Memo>;
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl<T: Any + Clone + Send + Sync> Memo for T {
    fn clone_box(&self) -> Box<dyn Memo> {
        Box::new(self.clone())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// A map's memos, one value per type (module doc).
#[derive(Resource, Default)]
pub struct MapMemos(BTreeMap<TypeId, (&'static str, Box<dyn Memo>)>);

impl MapMemos {
    /// The value of type `T`, if the map has one.
    pub fn get<T: Any + Clone + Send + Sync>(&self) -> Option<&T> {
        self.0.get(&TypeId::of::<T>())?.1.as_any().downcast_ref()
    }

    /// The value of type `T` to change, if the map has one.
    pub fn get_mut<T: Any + Clone + Send + Sync>(&mut self) -> Option<&mut T> {
        self.0.get_mut(&TypeId::of::<T>())?.1.as_any_mut().downcast_mut()
    }

    /// The value of type `T`, added (`T::default()`) when the map has none.
    pub fn entry<T: Any + Clone + Default + Send + Sync>(&mut self) -> &mut T {
        let slot = self
            .0
            .entry(TypeId::of::<T>())
            .or_insert_with(|| (std::any::type_name::<T>(), Box::new(T::default())));
        slot.1.as_any_mut().downcast_mut().expect("keyed by its type")
    }

    /// Set the value of type `T`.
    pub fn insert<T: Any + Clone + Send + Sync>(&mut self, value: T) {
        self.0.insert(TypeId::of::<T>(), (std::any::type_name::<T>(), Box::new(value)));
    }

    /// Take the value of type `T` out.
    pub fn remove<T: Any + Clone + Send + Sync>(&mut self) {
        self.0.remove(&TypeId::of::<T>());
    }

    /// No memos.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Clone for MapMemos {
    fn clone(&self) -> Self {
        Self(self.0.iter().map(|(k, (n, v))| (*k, (*n, (**v).clone_box()))).collect())
    }
}

impl std::fmt::Debug for MapMemos {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.0.values().map(|(name, _)| name)).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_clone_is_a_copy() {
        let mut a = MapMemos::default();
        a.entry::<Vec<u8>>().push(1);
        let mut b = a.clone();
        b.entry::<Vec<u8>>().push(2);
        assert_eq!(a.get::<Vec<u8>>(), Some(&vec![1]));
        assert_eq!(b.get::<Vec<u8>>(), Some(&vec![1, 2]));
        b.remove::<Vec<u8>>();
        assert!(b.is_empty() && b.get::<u8>().is_none());
    }
}
