//! The spectator's plate (docs/plans/active/ugg-ui.md, "Spectators"):
//! while the local player watches a minigame (`core::Spectating`, a
//! minigame playing; a game's own spectator panel, CS:S's bars, keeps
//! the plain apps), bottom centre on the shell's overlay layer: "WATCHING"
//! and the watched player's name (none flying free), the camera's mode,
//! and the keys that drive it, each the key bound now (the console's
//! binds under the player's game's layer, `rung_hint::bound_key`): jump
//! the mode (first person, chase, free), attack and attack2 the next and
//! previous player (`spectate`).
//!
//! Under the Signal look (`signal::signal_on`) the kit's plate tone with
//! its key caps (`chimera_ui::widgets::kbd`) as the rung hint's; without
//! it one plain line in the overlay's face. `SpectatorPlateView` is the
//! state (headless too); `ui_scene spectate` stages it.

use bevy::prelude::*;
use chimera_sim::minigame::hud::MinigameHud;
use chimera_ui::{KitFonts, Role, Theme, Tone, UiUnits};

use super::spectate::{SpecMode, Spectator};
use crate::core::{LocalPlayer, Spectating};

/// Its bottom above the window's bottom, cq (where the rung hint sits;
/// a spectator has none).
pub const BOTTOM_CQ: f32 = super::rung_hint::BOTTOM_CQ;
/// The key lockups' size (Mono, cq).
pub const SIZE_CQ: f32 = 0.56;

/// What the plate shows now.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct SpectatorPlateView {
    pub shown: bool,
    /// The watched player's name (empty: flying free).
    pub target: String,
    /// The camera's mode in words.
    pub mode: String,
    /// The keys bound now: mode, next, previous.
    pub keys: [String; 3],
    /// A staged plate (`ui_scene spectate`).
    pub staged: bool,
}

impl SpectatorPlateView {
    /// The plain line.
    pub fn text(&self) -> String {
        let who = if self.target.is_empty() { String::new() } else { format!(" {}", self.target) };
        format!(
            "Watching{who}  ({})   {}: mode   {}: next   {}: previous",
            self.mode, self.keys[0], self.keys[1], self.keys[2]
        )
    }
}

#[derive(Component)]
struct PlateRoot;

/// The plate.
pub struct SpectatorPlatePlugin;

impl Plugin for SpectatorPlatePlugin {
    fn build(&self, app: &mut App) {
        super::overlay::layer(app);
        app.init_resource::<SpectatorPlateView>().add_systems(Startup, spawn).add_systems(Update, (gather, draw).chain());
    }
}

/// Stages the plate (`on`) or clears it.
pub fn stage(world: &mut World, on: bool) {
    if let Some(mut v) = world.get_resource_mut::<SpectatorPlateView>() {
        v.staged = on;
    }
}

fn spawn(layer: Res<super::overlay::OverlayLayer>, mut commands: Commands) {
    commands.spawn((
        PlateRoot,
        ChildOf(layer.0),
        Visibility::Hidden,
        Pickable::IGNORE,
        Node { position_type: PositionType::Absolute, left: px(0.0), width: percent(100.0), justify_content: JustifyContent::Center, ..default() },
    ));
}

fn gather(
    hud: Option<Res<MinigameHud>>,
    spec: Option<Res<Spectator>>,
    me: Query<(), (With<LocalPlayer>, With<Spectating>)>,
    names: Query<(Entity, Option<&crate::net::NetCharacter>, Option<&Name>, Has<LocalPlayer>)>,
    settings: Option<Res<crate::net::NetSettings>>,
    console: Option<Res<chimera_core::console::Console>>,
    identity: Query<&crate::slots::GameIdentity, With<LocalPlayer>>,
    mut view: ResMut<SpectatorPlateView>,
) {
    let game = identity.iter().next().map(|i| i.0.as_str());
    let keys = ["jump", "attack", "attack2"].map(|a| super::rung_hint::bound_key(console.as_deref(), game, a));
    let next = if view.staged {
        SpectatorPlateView { shown: true, target: "nadia.exe".into(), mode: SpecMode::InEye.name().into(), keys, staged: true }
    } else {
        let on = !me.is_empty() && hud.as_deref().is_some_and(|h| h.playing());
        let (target, mode) = match spec.as_deref() {
            Some(s) => {
                let target = s
                    .target
                    .filter(|_| s.mode != SpecMode::Roaming)
                    .and_then(|t| names.get(t).ok())
                    .map(|(e, ch, n, l)| super::shown_name(e, l, ch, n, settings.as_deref()))
                    .unwrap_or_default();
                (target, s.mode.name().to_string())
            }
            None => (String::new(), String::new()),
        };
        SpectatorPlateView { shown: on, target, mode, keys, staged: false }
    };
    view.set_if_neq(next);
}

#[allow(clippy::too_many_arguments)]
fn draw(
    view: Res<SpectatorPlateView>,
    theme: Option<Res<Theme>>,
    kit: Option<Res<KitFonts>>,
    units: Option<Res<UiUnits>>,
    fonts: Option<Res<super::fonts::UiFonts>>,
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    mut root: Query<(Entity, &mut Node, &mut Visibility), With<PlateRoot>>,
    mut commands: Commands,
    mut built: Local<Option<(SpectatorPlateView, bool, u32)>>,
) {
    let Ok((root, mut node, mut vis)) = root.single_mut() else { return };
    vis.set_if_neq(if view.shown { Visibility::Inherited } else { Visibility::Hidden });
    if !view.shown {
        return;
    }
    let height = window.iter().next().map_or(720.0, |w| w.height());
    let units = units.as_deref().copied().unwrap_or_default();
    let bottom = Val::Px(units.snap(units.cq(BOTTOM_CQ)));
    if node.bottom != bottom {
        node.bottom = bottom;
    }
    let signal = super::signal::signal_on(theme.as_deref());
    let key = (view.clone(), signal, height as u32);
    if built.as_ref() == Some(&key) {
        return;
    }
    *built = Some(key);
    commands.entity(root).despawn_related::<Children>();
    match (signal, theme.as_deref()) {
        (true, Some(theme)) => {
            let plate = commands
                .spawn((
                    ChildOf(root),
                    Pickable::IGNORE,
                    Node { column_gap: units.val(0.9), align_items: AlignItems::Center, padding: UiRect::axes(units.val(0.9), units.val(0.5)), ..default() },
                    BackgroundColor(theme.tone(Tone::Plate)),
                ))
                .id();
            let text = |c: &mut Commands, role: Role, size: f32, tone: Tone, s: &str| {
                let st = chimera_ui::fonts::text(theme, kit.as_deref(), role, units.text_px(size));
                c.spawn((ChildOf(plate), Pickable::IGNORE, Text::new(s.to_string()), st.bundle(), TextColor(theme.tone(tone))));
            };
            text(&mut commands, Role::Mono, SIZE_CQ, Tone::AccText, "WATCHING");
            if !view.target.is_empty() {
                text(&mut commands, Role::DisplayBold, 0.9, Tone::Fg, &view.target.to_uppercase());
            }
            text(&mut commands, Role::Mono, SIZE_CQ, Tone::Fg3, &view.mode.to_uppercase());
            for (k, verb) in view.keys.iter().zip(["Mode", "Next", "Prev"]) {
                let pair = commands.spawn((ChildOf(plate), Pickable::IGNORE, Node { column_gap: units.val(0.35), align_items: AlignItems::Center, ..default() })).id();
                let cap = chimera_ui::widgets::spawn_pair(&mut commands, chimera_ui::widgets::kbd(theme, kit.as_deref(), &units, k, SIZE_CQ));
                commands.entity(cap).insert((ChildOf(pair), Pickable::IGNORE));
                let st = chimera_ui::fonts::text(theme, kit.as_deref(), Role::Mono, units.text_px(SIZE_CQ));
                commands.spawn((ChildOf(pair), Pickable::IGNORE, Text::new(verb.to_uppercase()), st.bundle(), TextColor(theme.tone(Tone::Fg2))));
            }
        }
        _ => {
            let size = super::callouts::LINE_SIZES[1] * super::overlay::scale(height);
            commands.spawn((
                ChildOf(root),
                Pickable::IGNORE,
                Node { padding: UiRect::axes(px(size * 0.6), px(size * 0.3)), ..default() },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.45)),
                children![(Text::new(view.text()), super::overlay::font(fonts.as_deref(), size, false), TextColor(Color::WHITE))],
            ));
        }
    }
}
