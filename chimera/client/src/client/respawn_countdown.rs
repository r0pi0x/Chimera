//! The respawn countdown: while the local player is dead and the rules
//! will bring it back on a timer (`core::RespawnDue`: a minigame's
//! `respawn.delay`), "Respawning in N" (whole seconds, rounded up;
//! "Respawning" once due, until the server's respawn arrives) in the
//! upper middle of the screen. It is the shell's own overlay (not a
//! `HudRoot`), so it looks the same on CS:S's HUD, Combat Arms' and the
//! minimal one; nothing shows when nothing respawns on a timer (plain
//! CS:S's and Combat Arms' rules leave `RespawnDue` unset, Combat Arms'
//! death screen has its own countdown). Sized for a 720-line screen and
//! scaled with the window's height (`SIZE`, `fonts::overlay_scale`), in
//! the HUD's face (`fonts::overlay_font`), on a band shaded as it would be
//! blended in display space (`widgets::display_alpha`). Its ticks sound
//! through the match's music director (`minigame::moments`' `respawn_in`:
//! Combat Arms' party cues). Part of `ClientPlugins`.
//!
//! Known shortcuts: plain text on a dark band, no gauge.

use bevy::prelude::*;

use crate::core::{Dead, LocalPlayer, RespawnDue, SimClock};

/// Adds the countdown's node and its system.
pub struct RespawnCountdownPlugin;

impl Plugin for RespawnCountdownPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RespawnDue>().add_systems(Startup, spawn).add_systems(Update, (draw, size));
    }
}

/// The countdown's root (hidden while there is nothing to count).
#[derive(Component)]
pub struct RespawnCountdownRoot;

/// The countdown's text node.
#[derive(Component)]
pub struct RespawnCountdownText;

/// The countdown's font size on a 720-line screen, scaled with the
/// window (`fonts::overlay_scale`).
pub const SIZE: f32 = 24.0;

/// What the countdown says `left` seconds before the respawn.
pub fn text(left: f64) -> String {
    match left.ceil() as i64 {
        n if n <= 0 => "Respawning".into(),
        n => format!("Respawning in {n}"),
    }
}

fn spawn(mut commands: Commands) {
    commands
        .spawn((
            RespawnCountdownRoot,
            Visibility::Hidden,
            GlobalZIndex(70),
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                width: percent(100.0),
                top: percent(24.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|c| {
            c.spawn((
                RespawnCountdownText,
                Text::new(""),
                TextFont {
                    font_size: FontSize::Px(SIZE),
                    ..default()
                },
                TextColor(Color::WHITE),
                BackgroundColor(super::widgets::display_alpha(Color::srgba(0.0, 0.0, 0.0, 0.55))),
                Node {
                    padding: UiRect::axes(px(16.0), px(6.0)),
                    ..default()
                },
            ));
        });
}

fn draw(
    due: Option<Res<RespawnDue>>,
    clock: Option<Res<SimClock>>,
    local: Query<Has<Dead>, With<LocalPlayer>>,
    mut root: Query<&mut Visibility, With<RespawnCountdownRoot>>,
    mut label: Query<&mut Text, With<RespawnCountdownText>>,
) {
    let dead = local.iter().next().unwrap_or(false);
    let shown = due.and_then(|d| d.0).filter(|_| dead).zip(clock).map(|(d, c)| text(d.at - c.now));
    for mut v in &mut root {
        v.set_if_neq(if shown.is_some() { Visibility::Inherited } else { Visibility::Hidden });
    }
    if let Some(s) = shown {
        for mut t in &mut label {
            if t.0 != s {
                t.0 = s.clone();
            }
        }
    }
}

/// The text's font and band for the window's height, in the HUD's face
/// (`fonts::overlay_font`): again when the window or the fonts change.
fn size(
    window: Query<&Window, With<bevy::window::PrimaryWindow>>,
    fonts: Option<Res<super::fonts::UiFonts>>,
    mut label: Query<(&mut TextFont, &mut Node), With<RespawnCountdownText>>,
    mut sized: Local<Option<u32>>,
) {
    let height = window.iter().next().map_or(720.0, |w| w.height());
    let s = super::fonts::overlay_scale(height);
    let key = (s * 1000.0) as u32;
    if *sized == Some(key) && !fonts.as_ref().is_some_and(|f| f.is_changed()) {
        return;
    }
    *sized = Some(key);
    for (mut font, mut node) in &mut label {
        *font = super::fonts::overlay_font(fonts.as_deref(), SIZE * s, true);
        node.padding = UiRect::axes(px(16.0 * s), px(6.0 * s));
    }
}
