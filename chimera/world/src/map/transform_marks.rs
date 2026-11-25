//! Marking which transform trees changed, on the schedule's own thread.
//!
//! Before Bevy propagates transforms it marks every ancestor of a changed
//! transform (`TransformTreeChanged`), so the propagation can skip the
//! trees nothing in changed (the map's static parts). Bevy's marking
//! (`mark_dirty_trees`) always starts a task per compute thread plus a
//! consumer and waits for all of them, however little changed: on the
//! client the render world keeps those threads busy, and the waiting
//! main thread runs the render world's queued tasks inside it. It ran
//! once a frame and twice a tick (the physics' propagation before each
//! step, the interpolation's at each tick's start), and in the F10 reports
//! b33ceded (dod_flash, 183 dynamic bodies) and afd90ba5 (de_dust2) its
//! physics run was in nearly every frame spike, 3.9 ms a spike.
//!
//! `mark_changed_trees` does the same marking in a plain walk up from each
//! changed or orphaned entity, stopping at a node already marked in this
//! run, with no task. Where Bevy's marking runs (`PostUpdate`, Avian's
//! `FixedPostUpdate`), it runs just before it and turns Bevy's into a no-op
//! for that run (`StaticTransformOptimizations::Disabled`, put back to
//! `Enabled` before the propagation, which reads it to skip unchanged
//! trees); the interpolation's tick start (`map::interp`) runs it in
//! place of Bevy's. The same trees are marked: the propagation's results
//! are the same. `CHIMERA_TRANSFORM_MARKS=bevy` leaves Bevy's marking in
//! `PostUpdate` and `FixedPostUpdate` (the A/B).

use bevy::{
    prelude::*,
    transform::{
        TransformSystems,
        components::TransformTreeChanged,
        systems::{StaticTransformOptimizations, mark_dirty_trees, propagate_parent_transforms},
    },
};

/// Mark every ancestor of a changed or orphaned transform (module doc).
#[allow(clippy::type_complexity)]
pub fn mark_changed_trees(
    changed: Query<Entity, Or<(Changed<Transform>, Changed<ChildOf>, Added<GlobalTransform>)>>,
    mut orphaned: RemovedComponents<ChildOf>,
    mut trees: Query<&mut TransformTreeChanged>,
    parents: Query<&ChildOf>,
    optimizations: Option<Res<StaticTransformOptimizations>>,
    mut marked: Local<Marked>,
) {
    if optimizations.is_some_and(|o| !o.is_enabled()) {
        return;
    }
    marked.clear();
    for from in changed.iter().chain(orphaned.read()) {
        let mut at = from;
        // Up to the root, or to a node this run already marked (and so
        // everything above it).
        while marked.insert(at) {
            let Ok(mut tree) = trees.get_mut(at) else { break };
            tree.set_changed();
            let Ok(parent) = parents.get(at) else { break };
            at = parent.parent();
        }
    }
}

/// Entities marked in a run, by index: a bit each, and the words set so
/// far, to clear only those.
#[derive(Default)]
pub struct Marked {
    bits: Vec<u64>,
    words: Vec<u32>,
}

impl Marked {
    fn clear(&mut self) {
        for &w in &self.words {
            self.bits[w as usize] = 0;
        }
        self.words.clear();
    }

    /// Whether `e` was not marked yet (it is now).
    fn insert(&mut self, e: Entity) -> bool {
        let i = e.index_u32() as usize;
        let (w, bit) = (i / 64, 1u64 << (i % 64));
        if w >= self.bits.len() {
            self.bits.resize(w + 1, 0);
        }
        let word = &mut self.bits[w];
        if *word & bit != 0 {
            return false;
        }
        if *word == 0 {
            self.words.push(w as u32);
        }
        *word |= bit;
        true
    }
}

/// Bevy's marking a no-op for its run (it reads the setting).
fn hold_bevy_marking(mut o: ResMut<StaticTransformOptimizations>) {
    *o = StaticTransformOptimizations::Disabled;
}

/// The propagation skips unchanged trees again.
fn release_bevy_marking(mut o: ResMut<StaticTransformOptimizations>) {
    *o = StaticTransformOptimizations::Enabled;
}

/// Ours before Bevy's marking in `PostUpdate` (Bevy's transform plugin)
/// and `FixedPostUpdate` (Avian's propagation before the physics step).
pub struct TransformMarksPlugin;

impl Plugin for TransformMarksPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<StaticTransformOptimizations>();
        if chimera_core::env::var("CHIMERA_TRANSFORM_MARKS").as_deref() == Ok("bevy") {
            return;
        }
        app.add_systems(
            PostUpdate,
            ((mark_changed_trees, hold_bevy_marking).chain().before(mark_dirty_trees), release_bevy_marking.after(mark_dirty_trees).before(propagate_parent_transforms))
                .in_set(TransformSystems::Propagate),
        );
        app.add_systems(
            FixedPostUpdate,
            ((mark_changed_trees, hold_bevy_marking).chain().before(mark_dirty_trees), release_bevy_marking.after(mark_dirty_trees).before(propagate_parent_transforms))
                .in_set(avian3d::physics_transform::PhysicsTransformSystems::Propagate),
        );
    }
}
