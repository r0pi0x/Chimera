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
//! - `icons`: the fallback gun set and the marks as data, rasterized
//!   per pixel height (`IconImages`) or tessellated; `IconFor`, `choose`.
//! - `icon_ink`: any icon fitted by its visible ink (an image's trimmed
//!   alpha, a glyph's outline, the fallback's polygons): `fit_contain`,
//!   `IconInk` (cached by asset).
//! - `materials`: `SlantMaterial`, `TickBarMaterial`, `PassMaterial`,
//!   `LinesMaterial`, `AmbientMaterial`.
//! - `style`: `UiStyle` (`style!`), `UiText`, `UiSlant`: the look book's
//!   CSS subset in cq and tokens, resolved through `UiUnits`.
//! - `widgets`: tags (YOU, HOST, the fused YOU+HOST), the bot slot,
//!   your row, the hover tick, key caps, the ping meter.
//! - `sound`: `UiCue` (a UI event to sound, by its name in the app's
//!   sound table), `UiMusic` (the loop a screen wants) and `SoundTable`
//!   (the app's event table, TOML); played by the
//!   shell's `ui_sound` under a theme with `flags.sound.ui`.
//! - `controls`: the menus' form controls (buttons, sub-tabs, rows,
//!   toggle, segmented, slider, stepper, dropdown, key-bind field, key
//!   caps, key/value lines, modal plates), their one state vocabulary
//!   (hover tick and tint, accent focus, white selected) and the 12°
//!   wipe for moves inside a screen.
//! - `scroll`: the scroll container (`Ui::scroll`: a view, its content
//!   and a Signal scroll bar), eased wheel steps, the right-button pan
//!   with a fling, `ScrollIntoView`; state by key in `ScrollViews`,
//!   tuned by `[layout.scroll]`, cut down by `ReducedMotion`.
//! - `parity`: `UiParityId`, a node's name in the look book, and its
//!   measured box for the UI parity check (`tools/ui-parity/`).
//! - `pointer_lead`: `PointerLead`, whether the keys or the pointer drive
//!   a list's selection (a navigation key takes it, a real pointer move
//!   gives it back): one rule for every list hover and keys both select in.
//!
//! `ChimeraUiPlugin` keeps `UiUnits` from the window, steps the clock and
//! every `Timeline` (`UiKitSystems`, in `PostUpdate` before Bevy's UI
//! layout) and registers the materials. Add it after Bevy's plugins.
//! Without a `Theme` resource it inserts `Theme::shell_default`.

pub mod controls;
pub mod fonts;
pub mod icon_ink;
pub mod icons;
pub mod materials;
pub mod motion;
pub mod parity;
pub mod scroll;
pub mod pointer_lead;
pub mod sound;
pub mod theme;
pub mod style;
pub mod units;
pub mod widgets;

use bevy::{prelude::*, ui::UiSystems};

pub use fonts::{FontFiles, KitFonts, Role, Styled};
pub use icon_ink::IconInk;
pub use icons::{IconFor, IconImages, IconSource, Icons};
pub use materials::{
    AmbientMaterial, EdgeMaterial, KitMaterialsPlugin, LinesMaterial, PassMaterial, RingMaterial, ShadeMaterial, SlantMaterial,
    TickBarMaterial,
};
pub use motion::{Ease, Prop, Timeline, Track, UiClock};
pub use parity::UiParityId;
pub use pointer_lead::PointerLead;
pub use scroll::{ReducedMotion, ScrollIntoView, ScrollParts, ScrollViews, scroll_into_view};
pub use sound::{SoundTable, UiCue, UiMusic};
pub use theme::{Theme, Tone};
pub use style::{PlainText, UiSlant, UiStyle, UiText};
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
            .init_resource::<icons::IconImages>()
            .init_resource::<icon_ink::IconInk>()
            .init_resource::<UiMusic>()
            .add_message::<UiCue>()
            .init_resource::<UiClock>()
            .add_plugins((KitMaterialsPlugin, widgets::HoverTicks, controls::ControlsPlugin, scroll::ScrollPlugin))
            .configure_sets(PostUpdate, UiKitSystems.before(UiSystems::Prepare));
        pointer_lead::add(app);
        app
            .add_systems(
                PostUpdate,
                (
                    units::track_window,
                    // Before the timelines, which write over its sizes.
                    style::resolve_styles,
                    motion::advance_clock,
                    motion::step_timelines,
                    motion::apply_timelines,
                )
                    .chain()
                    .in_set(UiKitSystems),
            );
    }
}
