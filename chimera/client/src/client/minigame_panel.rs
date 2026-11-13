//! A playing minigame's panel (`chimera_sim::minigame::hud::MinigameHud`:
//! its name, the rules' status lines, the gun game's rung, next gun and
//! leader among them, the match's time): top centre, on the shell's
//! overlay layer (`client::overlay`: its root, its scale by the window's
//! height, its face), the same whichever game's HUD is up (the gun game
//! swaps HUDs with the gun; no game's HUD draws a panel of its own).
//! Part of `ClientPlugins`; nothing shows without a playing minigame. A
//! line's `{player}` is the name the scoreboard shows, its `{weapon}` the
//! gun's name in `WeaponNames` (what a game read from its install's
//! strings), else the Tab scoreboard's (`minigame_scoreboard::gun_name`:
//! the app's `GunNames`, the data's `hud.name`, the id's last part in
//! capitals). A finished match's results (`MinigameHud::results`: the
//! winner, then a line a player) come under the status lines. The
//! minigame's moments (a rung up, the lead, the win) are the shell's
//! callout centre screen (`client::callouts`), so the panel lists no
//! notices; while the shell's match-end screen is up
//! (`MinigameHud::results_shown`) the panel draws nothing.
//!
//! Known shortcuts: the panel's place, sizes and colours are ours (these
//! games have no such panel; nothing to measure).

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_sim::minigame::hud::{HudLine, MinigameHud};

use chimera_sim::weapon::data::WeaponCatalog;

use crate::client::{
    fonts::UiFonts,
    minigame_scoreboard::{GunNames, gun_name},
    overlay::{self, OverlayLayer},
};

/// The minigame's panel on the overlay layer.
pub struct MinigamePanelPlugin;

impl Plugin for MinigamePanelPlugin {
    fn build(&self, app: &mut App) {
        overlay::layer(app);
        app.init_resource::<WeaponNames>().add_systems(Update, draw);
        super::text_warm::TextWarmAppExt::warm_texts(app, warm_fonts);
    }
}

/// The panel's two fonts at the window's size, laid out ahead
/// (`text_warm`) with the guns' names.
fn warm_fonts(windows: Query<&Window>, fonts: Option<Res<UiFonts>>, names: Res<WeaponNames>) -> Vec<super::text_warm::WarmText> {
    let s = overlay::scale(windows.iter().next().map_or(720.0, |w| w.height()));
    let names: String = names.0.values().flat_map(|v| v.chars()).collect();
    [(TITLE_SIZE, true), (LINE_SIZE, true)]
        .iter()
        .map(|(size, bold)| super::text_warm::WarmText { font: overlay::font(fonts.as_deref(), size * s, *bold), extra: names.clone() })
        .collect()
}

/// The title's (and the winner's) and the other lines' size on a
/// 720-line screen (`overlay::scale`).
pub const TITLE_SIZE: f32 = 26.0;
pub const LINE_SIZE: f32 = 19.0;

/// The panel's top edge on a 720-line screen.
pub const TOP: f32 = 14.0;

const TITLE: Color = Color::srgb(1.0, 200.0 / 255.0, 60.0 / 255.0);
const WINNER: Color = Color::srgb(1.0, 230.0 / 255.0, 120.0 / 255.0);

/// The guns' names a game read from its install's strings (weapon id to name).
#[derive(Resource, Clone, Debug, Default)]
pub struct WeaponNames(pub BTreeMap<String, String>);

/// The panel's root.
#[derive(Component)]
pub struct MinigamePanel;

/// The text of one line of the panel (title, status, result).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelLine {
    Title,
    Status,
    /// The finished match's results: the winner, then a line a player.
    Result,
}

/// A weapon id as a readable name when nothing names it: its last part
/// in capitals, a Source game's `weapon_` left off, `_` as spaces.
pub fn plain_name(id: &str) -> String {
    let last = id.rsplit(':').next().unwrap_or(id);
    let last = last.strip_prefix("weapon_").unwrap_or(last);
    last.chars().map(|c| if c == '_' { ' ' } else { c.to_ascii_uppercase() }).collect()
}

/// The panel's lines this frame: `{player}` and `{weapon}` filled in.
pub fn panel_lines(
    m: &MinigameHud,
    player: impl Fn(Entity) -> String,
    weapon: impl Fn(&str) -> String,
) -> Vec<(PanelLine, String)> {
    panel_lines_with(m, player, weapon, |r| r.name.clone())
}

/// `panel_lines` with `row_name` wording a result's player who isn't
/// a character on this side (a network client's results).
pub fn panel_lines_with(
    m: &MinigameHud,
    player: impl Fn(Entity) -> String,
    weapon: impl Fn(&str) -> String,
    row_name: impl Fn(&chimera_sim::minigame::results::ResultRow) -> String,
) -> Vec<(PanelLine, String)> {
    // The shell's match-end screen up: it has the results and the
    // panel would draw into it.
    if !m.playing() || m.results_shown {
        return Vec::new();
    }
    let word = |l: &HudLine| {
        let mut text = l.text.clone();
        if let Some(p) = l.player {
            text = text.replace("{player}", &player(p));
        }
        if let Some(w) = l.weapon {
            text = text.replace("{weapon}", &weapon(w));
        }
        text
    };
    let mut out = vec![(PanelLine::Title, m.title.clone())];
    out.extend(m.lines.iter().map(|l| (PanelLine::Status, word(l))));
    if let Some(r) = &m.results {
        out.extend(
            r.lines(|row| row.player.map_or_else(|| row_name(row), &player)).into_iter().map(|t| (PanelLine::Result, t)),
        );
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn draw(
    layer: Res<OverlayLayer>,
    minigame: Option<Res<MinigameHud>>,
    (names, guns, catalog): (Res<WeaponNames>, Option<Res<GunNames>>, Option<Res<WeaponCatalog>>),
    who: Query<(Option<&Name>, Has<crate::core::LocalPlayer>, Option<&crate::net::NetCharacter>)>,
    settings: Option<Res<crate::net::NetSettings>>,
    (fonts, windows): (Option<Res<UiFonts>>, Query<&Window>),
    panel: Query<Entity, With<MinigamePanel>>,
    mut last: Local<Option<(Vec<(PanelLine, String)>, f32)>>,
    mut commands: Commands,
) {
    let lines = minigame.as_deref().map_or_else(Vec::new, |m| {
        panel_lines_with(
            m,
            |e| match who.get(e) {
                Ok((name, local, net)) => crate::client::shown_name(e, local, net, name, settings.as_deref()),
                Err(_) => e.to_string(),
            },
            |w| names.0.get(w).cloned().unwrap_or_else(|| gun_name(w, guns.as_deref(), catalog.as_deref())),
            |r| r.name.clone(),
        )
    });
    let h = windows.iter().next().map_or(720.0, |w| w.height());
    let now = (lines.clone(), h);
    if last.as_ref() == Some(&now) && (lines.is_empty() || !panel.is_empty()) {
        return;
    }
    *last = Some(now);
    for e in &panel {
        commands.entity(e).despawn();
    }
    if lines.is_empty() {
        return;
    }
    let s = overlay::scale(h);
    let font = |size: f32| overlay::font(fonts.as_deref(), size * s, true);
    let root = commands
        .spawn((
            MinigamePanel,
            Pickable::IGNORE,
            Node {
                position_type: PositionType::Absolute,
                top: px(TOP * s),
                width: percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(2.0 * s),
                ..default()
            },
            GlobalZIndex(42),
            ChildOf(layer.0),
        ))
        .id();
    let mut first_result = true;
    for (kind, text) in lines {
        let (f, c) = match kind {
            PanelLine::Title => (font(TITLE_SIZE), TITLE),
            PanelLine::Status => (font(LINE_SIZE), Color::WHITE),
            PanelLine::Result if first_result => {
                first_result = false;
                (font(TITLE_SIZE), WINNER)
            }
            PanelLine::Result => (font(LINE_SIZE), Color::WHITE),
        };
        commands.spawn((kind, Text::new(text), f, TextColor(c), TextShadow::default(), ChildOf(root)));
    }
}
