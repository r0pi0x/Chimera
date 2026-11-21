//! `chimera_ui`, the UI kit (docs/plans/active/ugg-ui.md, 2.1): what the
//! shell's overlays and the apps' screens are built from, with no game,
//! simulation or network in it.
//!
//! - `theme`: `Theme` (resource) from TOML: palette tokens (`Tone`), type
//!   roles, the slant, named easings and durations, layout numbers in cq.
//! - `fonts`: `KitFonts` (our faces from bytes, `fonts::OWN`; `install`
//!   also makes them Bevy's default font), `Role` (`Role::Text`: the
//!   sentence face) and `fonts::text` (a role at a size: `TextFont` with
//!   weight and OpenType features, letter spacing, hinting, line height,
//!   case).
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
//! - `modal`: modal layers (`Modal` on a dialog's or overlay's root,
//!   `Modals` the stack): only the top layer and what is drawn above it
//!   take the pointer (wheel, clicks, hover, drag); key systems ask
//!   `Modals` whose the keys are.
//! - `scroll`: the scroll container (`Ui::scroll`: a view, its content
//!   and a Signal scroll bar), eased wheel steps, the right-button pan
//!   with a fling, `ScrollIntoView`; state by key in `ScrollViews`,
//!   tuned by `[layout.scroll]`, cut down by `ReducedMotion`.
//! - `vlist`: the recycling list (`Ui::virtual_list`: a pool of the
//!   lines in view plus two spare, rebound in place on `Changed<VSlot>`;
//!   a list or a tile grid virtualised by row, folded sections, keys,
//!   select on a press and the primary action on a double press:
//!   `VListEvent`), its state by key in `VirtualLists`.
//! - `picker`: the one picker for catalogues (maps, guns): items grouped
//!   by game in folding sections, a search and filter chips, multi-select
//!   with on / off / some boxes or a single pick, rows or cards sized by
//!   the frame's width (`picker::spawn`, `picker::dialog`).
//! - `chat`: the one chat feed's look (`FeedLine`, `LineKind`: player,
//!   team, radio lines, notices and the server's word; `line_alpha`, the
//!   fade after `HOLD` s; `stamp`; `name_color`; `Ui::chat_line`,
//!   `chat_plate`, `chat_field`), placed and fed by the shell's
//!   `signal_chat`.
//! - `clipboard`: `Clipboard` (the system's through arboard, or one of
//!   its own for tests) that text fields copy to and paste from.
//! - `parity`: `UiParityId`, a node's name in the look book, and its
//!   measured box for the UI parity check (`tools/ui-parity/`).
//! - `pointer_lead`: `PointerLead`, whether the keys or the pointer drive
//!   a list's selection (a navigation key takes it, a real pointer move
//!   gives it back): one rule for every list hover and keys both select in.
//! - `text_fit`: `ellipsize` and `Ellipsize` (a text cut to a width with
//!   `…`, measured in the kit's faces: `KitFonts::width`), for the names
//!   in the Signal screens.
//!
//! `ChimeraUiPlugin` keeps `UiUnits` from the window, steps the clock and
//! every `Timeline` (`UiKitSystems`, in `PostUpdate` before Bevy's UI
//! layout) and registers the materials. Add it after Bevy's plugins.
//! Without a `Theme` resource it inserts `Theme::shell_default`; it
//! installs our faces (`KitFonts::install`).

pub mod avatar;
pub mod chat;
pub mod clipboard;
pub mod controls;
pub mod fonts;
pub mod icon_ink;
pub mod icons;
pub mod materials;
pub mod modal;
pub mod motion;
pub mod parity;
pub mod picker;
pub mod scroll;
pub mod shell;
pub mod pointer_lead;
pub mod sound;
pub mod theme;
pub mod style;
pub mod text_edit;
pub mod text_fit;
pub mod units;
pub mod vlist;
pub mod widgets;

use bevy::{prelude::*, ui::UiSystems};

pub use fonts::{FontFiles, KitFonts, Role, Styled};
pub use icon_ink::IconInk;
pub use icons::{IconFor, IconImages, IconSource, Icons};
pub use materials::{
    AmbientMaterial, EdgeMaterial, KitMaterialsPlugin, LinesMaterial, PassMaterial, RingMaterial, ShadeMaterial, SlantMaterial,
    TickBarMaterial,
};
pub use modal::{Modal, Modals};
pub use motion::{Ease, Prop, Timeline, Track, UiClock};
pub use parity::UiParityId;
pub use pointer_lead::PointerLead;
pub use scroll::{ReducedMotion, ScrollIntoView, ScrollParts, ScrollViews, scroll_into_view};
pub use sound::{SoundTable, UiCue, UiMusic};
pub use theme::{Theme, Tone};
pub use style::{KeepCase, UiSlant, UiStyle, UiText};
pub use text_fit::{Ellipsize, ellipsize, ellipsize_with};
pub use units::{TickLayout, UiUnits};
pub use vlist::{VCursor, VHit, VListEvent, VListState, VNav, VSection, VShape, VSlot, VirtualList, VirtualLists};

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
        KitFonts::install(app.world_mut());
        app.init_resource::<UiUnits>()
            .init_resource::<icons::IconImages>()
            .init_resource::<icon_ink::IconInk>()
            .init_resource::<UiMusic>()
            .add_message::<UiCue>()
            .init_resource::<UiClock>()
            .add_plugins((KitMaterialsPlugin, controls::ControlsPlugin, modal::ModalPlugin, scroll::ScrollPlugin, vlist::VListPlugin, picker::PickerPlugin, shell::ShellPlugin))
            .configure_sets(PostUpdate, UiKitSystems.before(UiSystems::Prepare));
        pointer_lead::add(app);
        app
            .add_systems(
                PostUpdate,
                (
                    units::track_window,
                    // Before the timelines, which write over its sizes.
                    style::case_texts,
                    style::resolve_styles,
                    // On the styles' fonts and the last layout's widths.
                    text_fit::fit_texts,
                    motion::advance_clock,
                    motion::step_timelines,
                    motion::apply_timelines,
                )
                    .chain()
                    .in_set(UiKitSystems),
            );
    }
}
