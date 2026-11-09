//! `World::query` for exclusive systems and `&mut World` helpers, with the
//! query's state kept between calls (docs/performance.md, "Party
//! frame-rate pass").
//!
//! A new `QueryState` (`world.query::<..>()`) matches itself against every
//! archetype in the world, and a party match has over 2000 (both games'
//! characters, weapons, props, UI): tens of µs a call, and dozens of calls a
//! frame and a tick came to about 1.2 ms of the main thread's frame. A kept
//! state only checks the archetypes added since its last use.
//! `cached_query::<D, F, _>(world, |q, world| ..)` lends the kept state
//! (one per query type, in the world's `QueryCache` resource) to the
//! closure; its results are the same as a new state's. `first::<F>(world)`
//! is the first entity matching a filter (the local player, mostly).
//!
//! Known shortcuts: a call nested inside another of the same query type
//! gets a new state (the kept one is lent out), correct but not cached.
//! Only the code that showed in profiles keeps its states; about 270
//! other `world.query` calls (console commands, loads, rare events) still
//! make theirs. A kept state still costs more as archetypes pile up (Bevy
//! frees none; the characters' optional components make new ones at every
//! map change and death: docs/performance.md, "Map changes that slowed
//! the tick"): it steps over every empty table it matches each time it
//! iterates, and each new archetype makes it look again at those with its
//! rarest component. Fewer tables would need the characters' optional
//! components (spawn protection, the gun's game's movement and view
//! parts, the body model) as sparse sets, or fewer of them: done, the
//! session's tables stay few (docs/performance.md, "A session's frame
//! rate").

use std::any::{Any, TypeId};

use bevy::{
    ecs::query::{QueryData, QueryFilter},
    platform::collections::HashMap,
    prelude::*,
};

/// The kept query states, by query type.
#[derive(Resource, Default)]
pub struct QueryCache(HashMap<TypeId, Box<dyn Any + Send + Sync>>);

/// Runs `f` with this world's kept `QueryState<D, F>` (made on first use):
/// `world.query_filtered::<D, F>()` without matching every archetype again.
/// Iterate it with `q.iter(world)` (or `iter_mut`, `get`, `single`), which
/// first checks archetypes added since the last call.
pub fn cached_query<D, F, R>(world: &mut World, f: impl FnOnce(&mut QueryState<D, F>, &mut World) -> R) -> R
where
    D: QueryData + 'static,
    F: QueryFilter + 'static,
{
    let key = TypeId::of::<QueryState<D, F>>();
    let kept = world
        .get_resource_mut::<QueryCache>()
        .and_then(|mut c| c.0.remove(&key))
        .and_then(|b| b.downcast::<QueryState<D, F>>().ok());
    let mut state = match kept {
        Some(s) => s,
        None => Box::new(world.query_filtered::<D, F>()),
    };
    let out = f(&mut state, world);
    world.get_resource_or_init::<QueryCache>().0.insert(key, state);
    out
}

/// The first entity matching `F` (e.g. `first::<With<LocalPlayer>>`), from
/// a kept state: `world.query_filtered::<Entity, F>().iter(world).next()`.
pub fn first<F: QueryFilter + 'static>(world: &mut World) -> Option<Entity> {
    cached_query::<Entity, F, _>(world, |q, w| q.iter(w).next())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Component)]
    struct A(u32);
    #[derive(Component)]
    struct B;

    fn sum(world: &mut World) -> u32 {
        cached_query::<&A, Without<B>, _>(world, |q, w| q.iter(w).map(|a| a.0).sum())
    }

    #[test]
    fn a_kept_state_sees_what_a_new_one_would() {
        let mut world = World::new();
        world.spawn(A(1));
        assert_eq!(sum(&mut world), 1);
        // New entities in a known archetype, then a new archetype.
        world.spawn(A(2));
        world.spawn((A(4), Name::new("x")));
        world.spawn((A(100), B));
        assert_eq!(sum(&mut world), 7);
        let fresh: u32 = world.query_filtered::<&A, Without<B>>().iter(&world).map(|a| a.0).sum();
        assert_eq!(fresh, 7);
        // Mutable access through the kept state.
        cached_query::<&mut A, (), _>(&mut world, |q, w| q.iter_mut(w).for_each(|mut a| a.0 += 1));
        assert_eq!(sum(&mut world), 10);
        assert_eq!(world.resource::<QueryCache>().0.len(), 2);
        assert!(first::<With<B>>(&mut world).is_some());
        assert!(first::<(With<B>, Without<A>)>(&mut world).is_none());
    }

    #[test]
    fn nested_calls_of_one_type_still_work() {
        let mut world = World::new();
        world.spawn(A(3));
        let n = cached_query::<&A, Without<B>, _>(&mut world, |q, w| {
            let inner = sum(w);
            q.iter(w).count() as u32 + inner
        });
        assert_eq!(n, 4);
        assert_eq!(sum(&mut world), 3);
    }
}
