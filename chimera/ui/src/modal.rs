//! Modal layers: a dialog, a picker's dialog, a confirm, the Escape menu,
//! any overlay that sits over a screen takes all of its input; the UI
//! behind it gets none.
//!
//! - **Mark** a layer's root (its scrim) with `Modal`. The kit's builders
//!   do it: `Ui::modal` marks the plate's parent (the shade or overlay
//!   root it is centred in), `picker::dialog` its scrim. An overlay's own
//!   root component can `#[require(Modal)]`.
//! - **The stack** (`Modals`, a resource): the visible `Modal`s, back to
//!   front by Bevy's UI stack (`ComputedStackIndex`; a modal under another
//!   is above it), kept in `PreUpdate` after Bevy's UI focus
//!   (`ModalSystems`). A node takes input when no modal is up, or it is
//!   the top modal or under it, or it is drawn above it (`reaches`):
//!   a tooltip, a dropdown's list, a screen opened over the modal.
//! - **Pointer** (`gate_pointer`): every node the top modal hides has its
//!   `Interaction` put back to `None` (no hover, no press, no drag) and
//!   its `RelativeCursorPosition` to "not over", right after Bevy's focus
//!   wrote them, so the wheel (the kit's scroll views read the cursor
//!   from it), clicks, hover lifts and sliders behind a modal see nothing,
//!   whether or not the modal's scrim covers the window.
//! - **Keys**: the keyboard has no hit test, so key systems ask the stack:
//!   a screen's keys run only while `Modals::reaches(its root)` (or
//!   `keys_to`), a dialog's only while it is the top (`is_top`), so Esc,
//!   Enter, Tab and the arrows drive the top layer only and Esc closes
//!   the top dialog alone.
//!
//! Known shortcuts: the stack is the last layout's (a modal spawned this
//! frame counts from the next `PreUpdate`), so a screen that opens a
//! dialog keeps its own "open" flag for that first frame; the keys are
//! by convention (each key system asks), not taken from the screens
//! behind.

use bevy::{
    ecs::entity::EntityHashSet,
    prelude::*,
    ui::{ComputedStackIndex, DeprecatedInteraction as Interaction, RelativeCursorPosition, UiSystems},
};

/// A modal layer's root (module doc).
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Modal;

/// The open modals, back to front (module doc).
#[derive(Resource, Clone, Debug, Default)]
pub struct Modals {
    stack: Vec<Entity>,
    /// The top modal's UI stack index.
    top_index: u32,
    /// The top modal and every node under it.
    inside: EntityHashSet,
}

impl Modals {
    /// The modals, back to front.
    pub fn stack(&self) -> &[Entity] {
        &self.stack
    }

    /// The front modal.
    pub fn top(&self) -> Option<Entity> {
        self.stack.last().copied()
    }

    /// A modal is up.
    pub fn is_open(&self) -> bool {
        !self.stack.is_empty()
    }

    /// `e` is the front modal.
    pub fn is_top(&self, e: Entity) -> bool {
        self.top() == Some(e)
    }

    /// Node `e` takes input: no modal is up, or `e` is the top modal or
    /// under it, or drawn above it (`stack_index`, its
    /// `ComputedStackIndex`, when known).
    pub fn reaches_at(&self, e: Entity, stack_index: Option<u32>) -> bool {
        self.stack.is_empty() || self.inside.contains(&e) || stack_index.is_some_and(|i| i > self.top_index)
    }

    /// Node `e` takes input (`reaches_at`, by the tree only).
    pub fn reaches(&self, e: Entity) -> bool {
        self.reaches_at(e, None)
    }

    /// The keys go to `layer`: no modal up for a screen (`None`), or
    /// `layer` the front modal.
    pub fn keys_to(&self, layer: Option<Entity>) -> bool {
        self.top() == layer
    }
}

/// The stack and the pointer's gate (`PreUpdate`, after Bevy's UI focus;
/// before anything that reads the pointer from the nodes: the scroll
/// views' `ScrollInput`).
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ModalSystems;

/// Hidden by its own or an ancestor's `Visibility::Hidden` or
/// `Display::None` (Bevy's inherited visibility needs its plugin; this
/// reads the tree).
fn hidden(e: Entity, up: &Query<(Option<&ChildOf>, Option<&Visibility>, Option<&Node>)>) -> bool {
    let mut at = Some(e);
    while let Some(a) = at {
        let Ok((parent, vis, node)) = up.get(a) else { break };
        if vis == Some(&Visibility::Hidden) || node.is_some_and(|n| n.display == Display::None) {
            return true;
        }
        at = parent.map(ChildOf::parent);
    }
    false
}

fn is_under(e: Entity, ancestor: Entity, up: &Query<(Option<&ChildOf>, Option<&Visibility>, Option<&Node>)>) -> bool {
    let mut at = up.get(e).ok().and_then(|(p, ..)| p.map(ChildOf::parent));
    while let Some(a) = at {
        if a == ancestor {
            return true;
        }
        at = up.get(a).ok().and_then(|(p, ..)| p.map(ChildOf::parent));
    }
    false
}

/// Keeps `Modals` (module doc).
pub fn track_modals(
    mut modals: ResMut<Modals>,
    marked: Query<(Entity, Option<&ComputedStackIndex>), With<Modal>>,
    up: Query<(Option<&ChildOf>, Option<&Visibility>, Option<&Node>)>,
    children: Query<&Children>,
) {
    let mut open: Vec<(Entity, u32)> = marked.iter().filter(|(e, _)| !hidden(*e, &up)).map(|(e, i)| (e, i.map_or(0, |i| i.0))).collect();
    // Back to front: the UI stack, then (the same index: no layout yet) a
    // modal under another above it, then the later spawned.
    open.sort_by(|a, b| {
        a.1.cmp(&b.1).then_with(|| {
            if is_under(a.0, b.0, &up) {
                std::cmp::Ordering::Greater
            } else if is_under(b.0, a.0, &up) {
                std::cmp::Ordering::Less
            } else {
                a.0.index().cmp(&b.0.index())
            }
        })
    });
    let stack: Vec<Entity> = open.iter().map(|(e, _)| *e).collect();
    let top_index = open.last().map_or(0, |t| t.1);
    if modals.stack != stack || modals.top_index != top_index {
        modals.stack = stack;
        modals.top_index = top_index;
    }
    let mut inside = EntityHashSet::default();
    if let Some(top) = modals.top() {
        inside.insert(top);
        inside.extend(children.iter_descendants(top));
    }
    if modals.inside != inside {
        modals.inside = inside;
    }
}

/// The pointer's gate (module doc).
#[allow(clippy::type_complexity)]
pub fn gate_pointer(
    modals: Res<Modals>,
    mut nodes: Query<
        (Entity, Option<&ComputedStackIndex>, Option<&mut Interaction>, Option<&mut RelativeCursorPosition>),
        Or<(With<Interaction>, With<RelativeCursorPosition>)>,
    >,
) {
    if !modals.is_open() {
        return;
    }
    for (e, index, interaction, rel) in &mut nodes {
        if modals.reaches_at(e, index.map(|i| i.0)) {
            continue;
        }
        if let Some(mut i) = interaction {
            i.set_if_neq(Interaction::None);
        }
        if let Some(mut r) = rel {
            r.set_if_neq(RelativeCursorPosition { cursor_over: false, normalized: None });
        }
    }
}

/// The modal layers' systems (in the kit's plugin).
pub struct ModalPlugin;

impl Plugin for ModalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Modals>()
            .configure_sets(PreUpdate, ModalSystems.after(UiSystems::Focus))
            .add_systems(PreUpdate, (track_modals, gate_pointer).chain().in_set(ModalSystems));
    }
}
