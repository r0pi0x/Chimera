//! The shell's overlay layer (docs/DESIGN.md, "HUD layers"): what the
//! shell draws over whichever game's HUD is up (a minigame's panel and
//! callouts, the kill feed, the respawn countdown, the match-end screen,
//! the Tab scoreboard, the hit marker, the swap flash) hangs under one
//! engine-owned root (`OverlayRoot`, its entity in `OverlayLayer`), apart
//! from every game's HUD roots (`hud::HudRoot`), and is laid out by one
//! rule of its own: real pixels sized for a 720-line screen and scaled by
//! the window's height (`scale`), in one face (`font`: Trebuchet MS, else
//! the nearest installed sans), never a game's scheme fonts, colours,
//! virtual canvas or `GameDraws`. So it looks the same whichever game's
//! HUD the gun in hand puts up, at any aspect.
//!
//! A game's HUD scales under its own roots by its own rule (Combat Arms'
//! 640x480 canvas stretched to the screen, CS:S's scheme proportions):
//! node sizes it computes, never Bevy's global `UiScale`, which would
//! scale this layer too. `repo_checks`' architecture test
//! (`games_leave_the_overlay_alone`) fails a game crate that names this
//! module or `UiScale`, and `check_layer` (debug builds) logs a child of
//! the root that is a game's HUD root. The party apps' guard test lays
//! the layer out under every composed game's HUD at 16:9, 4:3 and 21:9
//! and requires one layout.
//!
//! A text cell whose text may run long (a gun's name) is fitted to its
//! width with the font (`fit`: condensed, then cut at a word with `…`),
//! never wrapped into the next row or column.
//!
//! The pre-match lobby (`prematch_lobby`) follows the same rule but stays
//! a root of its own: an app's menu moves it to its own camera
//! (`UiTargetCamera` works on roots only).

use bevy::prelude::*;

use super::fonts::UiFonts;

/// The overlay layer's root: full screen, no size or scale of its own.
#[derive(Component)]
pub struct OverlayRoot;

/// The overlay layer's root entity, for the shell's overlays to spawn
/// under (`ChildOf(layer.0)`).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OverlayLayer(pub Entity);

/// Puts the layer's root in (each overlay's plugin does too, through
/// `layer`, so one alone in a test has it).
pub struct OverlayPlugin;

impl Plugin for OverlayPlugin {
    fn build(&self, app: &mut App) {
        layer(app);
    }
}

/// The layer's root, spawned on first use.
pub fn layer(app: &mut App) -> Entity {
    if let Some(l) = app.world().get_resource::<OverlayLayer>() {
        return l.0;
    }
    let root = app
        .world_mut()
        .spawn((
            OverlayRoot,
            Name::new("Overlay layer"),
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                top: px(0.0),
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
        ))
        .id();
    app.insert_resource(OverlayLayer(root));
    if cfg!(debug_assertions) {
        app.add_systems(PostUpdate, check_layer);
    }
    root
}

/// The layer's scale in a window `height` pixels tall: its sizes are for
/// a 720-line screen, never under a third of it. The one rule for every
/// game and aspect.
pub fn scale(height: f32) -> f32 {
    height.max(240.0) / 720.0
}

/// The layer's face, whichever game's HUD is up.
pub const FAMILY: &str = "trebuchet ms";

/// The layer's font at `em_px` (already scaled): `FAMILY` where the
/// faces are loaded, else Bevy's default face.
pub fn font(fonts: Option<&UiFonts>, em_px: f32, bold: bool) -> TextFont {
    match fonts {
        Some(f) => f.em(FAMILY, bold, em_px),
        None => TextFont {
            font_size: bevy::text::FontSize::Px(em_px),
            ..default()
        },
    }
}

/// `text` made to fit `max_px` in `font`, measured with the face
/// (`UiFonts::width`; without the faces, half an em a character): as it
/// is, else condensed, without its variant tail, without its skin, then
/// cut at a word with `…` (`chimera_sim::weapon::names::fit`). The
/// overlay's text cells call it for names that may run long (guns); it
/// never wraps. The words are `rules`' (the games' `GunNameRules`), else
/// the shipped list read for the call.
pub fn fit(
    fonts: Option<&UiFonts>,
    rules: Option<&chimera_sim::weapon::names::GunNameRules>,
    font: &TextFont,
    text: &str,
    max_px: f32,
) -> String {
    let none = UiFonts::default();
    let fonts = fonts.unwrap_or(&none);
    let fits = |t: &str| fonts.width(font, t) <= max_px;
    if fits(text) {
        return text.to_string();
    }
    let shipped;
    let words = match rules {
        Some(r) => r.words(),
        None => {
            shipped = chimera_sim::weapon::names::NameWords::shipped();
            &shipped
        }
    };
    chimera_sim::weapon::names::fit(text, fits, words)
}

/// A game's HUD root put under the layer would show and hide with that
/// HUD and take its place: said once.
fn check_layer(
    layer: Option<Res<OverlayLayer>>,
    children: Query<&Children>,
    huds: Query<&super::hud::HudRoot>,
    mut said: Local<bool>,
) {
    let Some(layer) = layer else { return };
    if *said {
        return;
    }
    for c in children.iter_descendants(layer.0) {
        if let Ok(h) = huds.get(c) {
            *said = true;
            error!("overlay: {c} under the overlay layer is game `{}`'s HUD root; a game's HUD has roots of its own", h.0);
        }
    }
}
