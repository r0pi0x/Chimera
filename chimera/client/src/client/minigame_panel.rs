//! A playing minigame on a game's HUD, for an app whose minigames may show
//! it (`chimera_sim::minigame::hud::MinigameHud`: its name, the rules' status
//! lines, the gun game's rung, next gun and leader among them, the match's
//! time): top centre, in the client scheme's fonts and the HUD's colour, as a
//! Source game prints its centre messages. `MinigamePanelPlugin(game)` puts
//! it on that game's HUD (CS:S's, `cs_source::ui::minigame_hud`; HL2:DM's). A
//! line's `{player}` is the name the scoreboard shows, its `{weapon}` the
//! gun's name in `WeaponNames` (what a game read from its install's
//! strings), else the id's last part in capitals. A finished match's
//! results (`MinigameHud::results`: the winner, then a line a player) come
//! under the status lines. The minigame's moments (a rung up, the lead, the
//! win) are the shell's callout centre screen (`client::callouts`), so the
//! panel lists no notices (they said the same, a second banner); while the
//! shell's match-end screen is up (`MinigameHud::results_shown`) the panel
//! draws nothing. Its root is a `client::hud::HudRoot`: down while another
//! game's HUD is the gameplay's.
//!
//! Known shortcuts: the panel's place, sizes and colours are ours (these
//! games have no such panel; nothing to measure).

use std::collections::BTreeMap;

use bevy::prelude::*;
use chimera_sim::minigame::hud::{HudLine, MinigameHud};

use crate::{
    client::{fonts::UiFonts, hud::HudRoot},
    core::LocalPlayer,
    gameplay::Gameplay,
    map::hud::ActiveHud,
};

/// The minigame's panel on game `.0`'s HUD.
pub struct MinigamePanelPlugin(pub &'static str);

impl Plugin for MinigamePanelPlugin {
    fn build(&self, app: &mut App) {
        let first = !app.world().contains_resource::<PanelGames>();
        app.init_resource::<WeaponNames>().init_resource::<PanelGames>();
        app.world_mut().resource_mut::<PanelGames>().0.push(self.0);
        if first {
            app.add_systems(Update, draw);
            super::text_warm::TextWarmAppExt::warm_texts(app, warm_fonts);
        }
    }

    fn is_unique(&self) -> bool {
        false
    }
}

/// The panel's two fonts at the window's size, laid out ahead
/// (`text_warm`) with the guns' names.
fn warm_fonts(windows: Query<&Window>, fonts: Option<Res<UiFonts>>, names: Res<WeaponNames>) -> Vec<super::text_warm::WarmText> {
    // The window `draw` sizes the panel by.
    let h = windows.iter().next().map_or(480.0, |w| w.height());
    let names: String = names.0.values().flat_map(|v| v.chars()).collect();
    PANEL_FONTS
        .iter()
        .map(|(name, fallback)| super::text_warm::WarmText { font: panel_font(fonts.as_deref(), name, h, *fallback), extra: names.clone() })
        .collect()
}

/// The scheme fonts the panel's lines are in (titles and the first
/// result; status lines and the other results), with their sizes without
/// the scheme.
const PANEL_FONTS: [(&str, f32); 2] = [("CenterPrintText", 18.0), ("ChatFont", 14.0)];

fn panel_font(fonts: Option<&UiFonts>, name: &str, h: f32, fallback: f32) -> TextFont {
    match fonts {
        Some(f) => f.client(name, h, fallback),
        None => TextFont {
            font_size: bevy::text::FontSize::Px(fallback),
            ..default()
        },
    }
}

/// The games whose HUD shows the panel.
#[derive(Resource, Clone, Debug, Default)]
pub struct PanelGames(pub Vec<&'static str>);

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
    gameplay: Option<Res<Gameplay>>,
    games: Res<PanelGames>,
    minigame: Option<Res<MinigameHud>>,
    names: Res<WeaponNames>,
    who: Query<(Option<&Name>, Has<LocalPlayer>, Option<&crate::net::NetCharacter>)>,
    settings: Option<Res<crate::net::NetSettings>>,
    (fonts, hud, windows): (Option<Res<UiFonts>>, Option<Res<ActiveHud>>, Query<&Window>),
    panel: Query<Entity, With<MinigamePanel>>,
    mut last: Local<Option<(Vec<(PanelLine, String)>, f32, &'static str)>>,
    mut commands: Commands,
) {
    // No gameplay (a test's): the first game's.
    let Some(game) = games.0.iter().copied().find(|g| gameplay.as_ref().is_none_or(|p| p.hud == *g)) else { return };
    let lines = minigame.as_deref().map_or_else(Vec::new, |m| {
        panel_lines_with(
            m,
            |e| match who.get(e) {
                Ok((name, local, net)) => crate::client::shown_name(e, local, net, name, settings.as_deref()),
                Err(_) => e.to_string(),
            },
            |w| names.0.get(w).cloned().unwrap_or_else(|| plain_name(w)),
            |r| r.name.clone(),
        )
    });
    let h = windows.iter().next().map_or(480.0, |w| w.height());
    let now = (lines.clone(), h, game);
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
    let colour = hud
        .as_deref()
        .and_then(|h| h.0.color("FgColor"))
        .unwrap_or(Color::srgba_u8(255, 176, 0, 255));
    let font = |name: &str, fallback: f32| panel_font(fonts.as_deref(), name, h, fallback);
    let root = commands
        .spawn((
            MinigamePanel,
            HudRoot(game),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(2.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                ..default()
            },
            GlobalZIndex(42),
        ))
        .id();
    let mut first_result = true;
    for (kind, text) in lines {
        let (f, c) = match kind {
            PanelLine::Title => (font(PANEL_FONTS[0].0, PANEL_FONTS[0].1), colour),
            PanelLine::Status => (font(PANEL_FONTS[1].0, PANEL_FONTS[1].1), Color::WHITE),
            PanelLine::Result if first_result => {
                first_result = false;
                (font(PANEL_FONTS[0].0, PANEL_FONTS[0].1), Color::srgba_u8(255, 230, 120, 255))
            }
            PanelLine::Result => (font(PANEL_FONTS[1].0, PANEL_FONTS[1].1), Color::WHITE),
        };
        commands.spawn((kind, Text::new(text), f, TextColor(c), TextShadow::default(), ChildOf(root)));
    }
}
