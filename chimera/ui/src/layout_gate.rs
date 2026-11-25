//! Bevy's UI layout runs only in frames where something it reads changed.
//! `bevy_ui`'s `ui_layout_system` walks every node of every root each
//! frame (taffy's rounding and Bevy's geometry pass, docs/performance.md
//! "The HUD"), changed or not, and in a match nothing it reads changes in
//! most frames: the F10 reports b33ceded (dod_flash) and afd90ba5
//! (de_dust2) had it in every frame spike. A condition on its set
//! (`UiSystems::Layout`, which holds only it) lets it run when a node, its
//! content size (text), transform, outline, scroll, layout config, font
//! sizes, target (window size, scale factor) or the hierarchy changed, a
//! node or its children went, or the rem size changed; otherwise its
//! outputs would come out as they are, and it is skipped.
//!
//! `UiLayoutAlways(true)` (a resource) turns the gate off, for comparing.
//!
//! Known shortcuts: `GhostNode`s (Bevy's `ghost_nodes` feature, off in
//! this workspace) are not watched.

use std::sync::atomic::{AtomicU64, Ordering};

use bevy::{
    prelude::*,
    text::{EmSize, RemSize},
    ui::{ComputedUiRenderTargetInfo, ContentSize, FixedNode, IgnoreScroll, LayoutConfig, UiSystems},
};

/// The layout runs every frame (the gate off).
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct UiLayoutAlways(pub bool);

/// Frames the layout ran and was skipped (tests, measurements); counted
/// by the condition, which may only read.
#[derive(Resource, Debug, Default)]
pub struct UiLayoutRuns {
    ran: AtomicU64,
    skipped: AtomicU64,
}

impl UiLayoutRuns {
    pub fn ran(&self) -> u64 {
        self.ran.load(Ordering::Relaxed)
    }

    pub fn skipped(&self) -> u64 {
        self.skipped.load(Ordering::Relaxed)
    }
}

pub fn add(app: &mut App) {
    app.init_resource::<UiLayoutAlways>()
        .init_resource::<UiLayoutRuns>()
        .configure_sets(PostUpdate, UiSystems::Layout.run_if(layout_needed));
}

type Changed1 = Or<(
    Changed<Node>,
    Changed<ContentSize>,
    Changed<UiTransform>,
    Changed<ComputedUiRenderTargetInfo>,
    Changed<EmSize>,
    Changed<Children>,
    Changed<ChildOf>,
    Changed<Outline>,
)>;
type Changed2 = Or<(Changed<ScrollPosition>, Changed<LayoutConfig>, Changed<IgnoreScroll>, Changed<FixedNode>)>;

/// Something the layout reads changed since it last looked (module doc).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn layout_needed(
    always: Res<UiLayoutAlways>,
    rem: Option<Res<RemSize>>,
    changed: Query<(), (With<Node>, Changed1)>,
    changed2: Query<(), (With<Node>, Changed2)>,
    mut gone: (RemovedComponents<Node>, RemovedComponents<Children>, RemovedComponents<ChildOf>, RemovedComponents<FixedNode>),
    runs: Res<UiLayoutRuns>,
) -> bool {
    // Every reader read through (none may short-circuit past the others).
    let removed = gone.0.read().count() + gone.1.read().count() + gone.2.read().count() + gone.3.read().count() > 0;
    let need = always.0 || removed || rem.is_some_and(|r| r.is_changed()) || !changed.is_empty() || !changed2.is_empty();
    if need { &runs.ran } else { &runs.skipped }.fetch_add(1, Ordering::Relaxed);
    need
}
