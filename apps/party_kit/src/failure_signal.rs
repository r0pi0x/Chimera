//! The join failure box in the Signal look (docs/plans/active/ugg-ui.md,
//! "Spectators"), for an app whose theme lays its pages out
//! (`[layout.pages]`): why a join was refused or a game ended (the
//! server is full, no room for another spectator, a bad password, the
//! host left), in the kit's modal over the menu, with Close as its one
//! button (Enter, Esc or a click; `MainMenuAction::CloseFailure`).
//! Without the theme `main_menu`'s VGUI box says it.
//!
//! The words are `main_menu::failure_text`'s (the game's own messages
//! where it has one, else the server's). The title is "Disconnected"
//! when the game ended after the join (the server dropped us), else
//! "Couldn't join". `mainmenu_failure <text>` stages it.

use bevy::prelude::*;
use chimera_net::net::client::{JoinFailure, JoinProgress};
use chimera_ui::{
    KitFonts, Role, Theme, Tone, UiStyle, UiUnits,
    controls::{Kind, SENTENCE, Size, State, Ui},
};

use crate::main_menu::{MainMenuAction, PartyMenu, Screen};

/// The box's root.
#[derive(Component)]
pub struct FailureSignalRoot;

pub struct FailureSignalPlugin;

impl Plugin for FailureSignalPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, draw.after(crate::main_menu::MainMenuSystems));
    }
}

/// The title over the words.
fn title_of(join: Option<&JoinProgress>) -> &'static str {
    match join.and_then(|j| j.failure.as_ref()).map(|(k, _)| k) {
        Some(JoinFailure::Dropped) => "Disconnected",
        _ => "Couldn't join",
    }
}

/// What the box was drawn for: the words and title, the units.
#[derive(Default, PartialEq)]
struct Key {
    shown: Option<(String, &'static str)>,
    units: (u32, u32),
}

#[allow(clippy::too_many_arguments)]
fn draw(
    menu: Res<PartyMenu>,
    theme: Option<Res<Theme>>,
    kit: Option<Res<KitFonts>>,
    units: Option<Res<UiUnits>>,
    join: Option<Res<JoinProgress>>,
    camera: Query<Entity, With<crate::main_menu::MenuUiCamera>>,
    roots: Query<Entity, With<FailureSignalRoot>>,
    mut drawn: Local<Key>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme, units) else { return };
    let themed = crate::signal_pages::themed(Some(&theme));
    let shown = (themed && menu.shown() && menu.screen == Screen::MainMenu)
        .then(|| menu.failure.clone().map(|f| (f, title_of(join.as_deref()))))
        .flatten();
    let key = Key { shown, units: ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32) };
    if *drawn == key {
        return;
    }
    for r in &roots {
        commands.entity(r).despawn();
    }
    *drawn = key;
    let Some((text, title)) = drawn.shown.clone() else { return };
    let ui = Ui::new(&theme, kit.as_deref(), *units);
    let mut root = commands.spawn((
        FailureSignalRoot,
        Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), justify_content: JustifyContent::Center, align_items: AlignItems::Center, ..default() },
        BackgroundColor(ui.color(Tone::Ink, 0.72)),
        bevy::ui::FocusPolicy::Block,
        GlobalZIndex(crate::main_menu::MENU_Z + 1),
    ));
    if let Some(cam) = camera.iter().next() {
        root.insert(UiTargetCamera(cam));
    }
    let root = root.id();
    let c = &mut commands;
    let m = ui.modal(c, root, 32.0, false);
    ui.text(c, m, Role::DisplayBold, 1.6, Tone::Fg, title);
    let mark = ui.boxed(c, m, UiStyle { w_pct: Some(100.0), gap: Some(0.6), margin: Some([0.8, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
    ui.mark(c, mark, Tone::Threat, 0.42, 0.42);
    let tx = ui.boxed(c, mark, UiStyle { grow: Some(1.0), ..UiStyle::column() });
    ui.paragraph(c, tx, Role::Text, SENTENCE, Tone::Fg, &text);
    let acts = ui.boxed(c, m, UiStyle { gap: Some(0.7), margin: Some([1.4, 0.0, 0.0, 0.0].into()), ..UiStyle::row() }.justify(JustifyContent::FlexEnd));
    let close = ui.button(c, acts, Kind::Primary, "Close", Some("Enter"), Size::Regular, State::focused(true));
    c.entity(close).insert(MainMenuAction::CloseFailure);
}
