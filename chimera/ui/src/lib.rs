//! `chimera_ui`, the UI kit (docs/plans/active/ugg-ui.md, 2.1): what the
//! shell's overlays and the apps' screens are built from, with no game,
//! simulation or network in it.
//!
//! - `theme`: `Theme` (resource) from TOML: palette tokens (`Tone`), type
//!   roles, the slant, named easings and durations, layout numbers in cq.
//! - `fonts`: `KitFonts` (the app's faces from bytes), `Role` and
//!   `fonts::text` (a role at a size: `TextFont` with weight and OpenType
//!   features, letter spacing, hinting).
//! - `units`: cq to logical px (`UiUnits`), physical-pixel snapping,
//!   `TickLayout`.
//! - `motion`: `Ease`, `UiClock`, `Track`, `Timeline`.
//! - `materials`: `SlantMaterial`, `TickBarMaterial`, `PassMaterial`.
//!
//! `ChimeraUiPlugin` keeps `UiUnits` from the window, steps the clock and
//! every `Timeline` (`UiKitSystems`, in `PostUpdate` before Bevy's UI
//! layout) and registers the materials. Add it after Bevy's plugins.
//! Without a `Theme` resource it inserts `Theme::shell_default`.

pub mod fonts;
pub mod materials;
pub mod motion;
pub mod theme;
pub mod units;

use bevy::{prelude::*, ui::UiSystems};

pub use fonts::{FontFiles, KitFonts, Role, Styled};
pub use materials::{KitMaterialsPlugin, PassMaterial, SlantMaterial, TickBarMaterial};
pub use motion::{Ease, Prop, Timeline, Track, UiClock};
pub use theme::{Theme, Tone};
pub use units::{TickLayout, UiUnits};

/// The kit's per-frame work: units from the window, the clock, the
/// timelines, their writes to transforms and sizes.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct UiKitSystems;

/// The kit (module doc).
pub struct ChimeraUiPlugin;

impl Plugin for ChimeraUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<Theme>() {
            app.insert_resource(Theme::shell_default());
        }
        app.init_resource::<UiUnits>()
            .init_resource::<UiClock>()
            .add_plugins(KitMaterialsPlugin)
            .configure_sets(PostUpdate, UiKitSystems.before(UiSystems::Prepare))
            .add_systems(
                PostUpdate,
                (units::track_window, motion::advance_clock, motion::step_timelines, motion::apply_timelines)
                    .chain()
                    .in_set(UiKitSystems),
            );
    }
}
