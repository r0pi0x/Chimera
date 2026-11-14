//! The Signal menu's backdrops (docs/plans/active/ugg-ui.md, M5), in the
//! right two-thirds of the menu (`signal_menu`): Ladder, Assembly and the
//! Ident, swapping every 16 s.
//!
//! Known shortcuts: not drawn yet (the ambient ground alone).

use bevy::prelude::*;

/// The Ladder's whole climb: 13 beats of 1.6 s to the knife, its landing
/// at .95 s into the last, a 2.2 s hold and a .55 s fade out.
pub const LADDER_PERIOD: f32 = 13.0 * 1.6 + 0.95 + 2.2 + 0.55;

/// Each backdrop's still frame (reduced motion; a staged scene's default).
pub fn still(which: usize) -> f32 {
    match which {
        0 => 13.0 * 1.6 + 0.95 + 1.2,
        1 => 12.6,
        _ => 7.0,
    }
}

/// The backdrops' systems.
pub fn add(_app: &mut App) {}

/// The backdrops' layer under the page (its first children over the
/// ambient).
pub fn spawn_layer(_c: &mut Commands, _root: Entity) {}
