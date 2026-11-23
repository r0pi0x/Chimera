//! DoD:S's HUD while a DoD:S weapon is in hand (M3b of
//! docs/plans/active/dods.md): the health (bottom left: the number over a
//! bar), the stamina under it (a bar, `dods::game::stamina`), the weapon's
//! name and its rounds (bottom right: clip and reserve), under its own root
//! (`HudRoot(GAME)`), shown only while DoD:S's HUD is the gameplay's. The
//! crosshair is the shell's (its gap from the spread); the kill feed is the
//! app's.
//!
//! - `Frame`: what the HUD shows, from the local player's state (`frame`;
//!   tests read it without a window);
//! - the Bevy side: `spawn_root`, `collect` (the frame) and `sync` (the
//!   nodes).
//!
//! Known shortcuts:
//! - No spec of DoD:S's HUD: the layout, colours and font are ours (plain
//!   panels in the HUD's corners), not the install's `hudlayout.res` and
//!   scheme; no class portrait, no grenade or rifle-grenade count of its
//!   own (they show as the weapon's rounds).
//! - No kill icons: DoD:S's guns are named in the app's kill feed (the
//!   scripts' `TextureData` death notices are unspecified).

use bevy::prelude::*;
use chimera_client::client::hud::{GameDraws, HudRoot};
use chimera_core::core::{Health, LocalPlayer};
use chimera_sim::weapon::{Inventory, Magazine, Weapon};

use dods::game::{GAME, stamina::DodsStamina};

/// What this HUD draws instead of the shell's: health and ammo (the
/// crosshair, hit marker and kill feed stay the shell's).
pub const DRAWS: GameDraws = GameDraws { crosshair: false, status: true, hit_marker: false, killfeed: false };

/// What the HUD shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Frame {
    /// Health, 0-100 (rounded up), none without a local player.
    pub health: Option<u32>,
    /// Stamina, 0-1 (none: not a DoD:S player).
    pub stamina: Option<f32>,
    /// The weapon in hand's name.
    pub weapon: String,
    /// Its clip and reserve, when it has a magazine.
    pub ammo: Option<(u32, u32)>,
}

/// The frame from the local player's health, stamina and weapon (its id
/// and, with a magazine, its clip and reserve; `name`: its display name,
/// else its id).
pub fn frame(
    health: Option<&Health>,
    stamina: Option<&DodsStamina>,
    weapon: Option<(&str, Option<(u32, u32)>)>,
    name: impl Fn(&str) -> Option<String>,
) -> Frame {
    Frame {
        health: health.map(|h| (h.current * 100.0).ceil().max(0.0) as u32),
        stamina: stamina.map(|s| (s.value / dods::game::stamina::MAX).clamp(0.0, 1.0)),
        weapon: weapon.map(|(id, _)| name(id).unwrap_or_else(|| id.to_string())).unwrap_or_default(),
        ammo: weapon.and_then(|(_, m)| m),
    }
}

/// The HUD's last frame.
#[derive(Resource, Default)]
pub struct DodsHudState {
    pub frame: Frame,
}

/// The weapons' display names (their data's `hud.name`).
#[derive(Resource)]
pub struct Names(pub Vec<(String, String)>);

impl Default for Names {
    fn default() -> Self {
        Names(dods::game::weapons::dods_weapons().weapons.into_iter().filter_map(|w| Some((w.id, w.hud.name?))).collect())
    }
}

#[derive(Component)]
pub struct HealthText;
#[derive(Component)]
pub struct HealthBar;
#[derive(Component)]
pub struct StaminaBar;
#[derive(Component)]
pub struct AmmoText;
#[derive(Component)]
pub struct WeaponText;

const TEXT: Color = Color::srgb(0.93, 0.89, 0.76);
const PANEL: Color = Color::srgba(0.0, 0.0, 0.0, 0.45);
const HEALTH: Color = Color::srgb(0.62, 0.71, 0.38);
const LOW: Color = Color::srgb(0.80, 0.25, 0.18);
const STAMINA: Color = Color::srgb(0.85, 0.75, 0.45);
/// Health at or below this draws in the warning colour (ours).
const LOW_HEALTH: u32 = 25;
const BAR_W: f32 = 180.0;

/// The root and its panels, hidden until DoD:S's HUD is the gameplay's.
pub fn spawn_root(mut commands: Commands) {
    let text = |size: f32| (Text::default(), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(TEXT));
    let bar = |h: f32| Node { width: px(BAR_W), height: px(h), ..default() };
    commands
        .spawn((
            HudRoot(GAME),
            Visibility::Hidden,
            GlobalZIndex(40),
            Node { position_type: PositionType::Absolute, width: percent(100.0), height: percent(100.0), ..default() },
        ))
        .with_children(|root| {
            root.spawn((
                BackgroundColor(PANEL),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(20.0),
                    bottom: px(16.0),
                    padding: UiRect::all(px(8.0)),
                    flex_direction: FlexDirection::Column,
                    row_gap: px(4.0),
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((HealthText, text(30.0)));
                p.spawn((BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)), bar(8.0))).with_children(|b| {
                    b.spawn((HealthBar, BackgroundColor(HEALTH), Node { width: percent(100.0), height: percent(100.0), ..default() }));
                });
                p.spawn((BackgroundColor(Color::srgba(1.0, 1.0, 1.0, 0.12)), bar(5.0))).with_children(|b| {
                    b.spawn((StaminaBar, BackgroundColor(STAMINA), Node { width: percent(100.0), height: percent(100.0), ..default() }));
                });
            });
            root.spawn((
                BackgroundColor(PANEL),
                Node {
                    position_type: PositionType::Absolute,
                    right: px(20.0),
                    bottom: px(16.0),
                    padding: UiRect::all(px(8.0)),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::FlexEnd,
                    ..default()
                },
            ))
            .with_children(|p| {
                p.spawn((WeaponText, text(16.0)));
                p.spawn((AmmoText, text(30.0)));
            });
        });
}

/// The local player's state into the frame.
#[allow(clippy::type_complexity)]
pub fn collect(
    player: Option<Single<(&Health, Option<&DodsStamina>, Option<&Inventory>), With<LocalPlayer>>>,
    weapons: Query<(&Weapon, Option<&Magazine>)>,
    names: Res<Names>,
    mut state: ResMut<DodsHudState>,
) {
    let f = match player {
        Some(p) => {
            let (health, stamina, inv) = *p;
            let held = inv
                .and_then(|i| i.active)
                .and_then(|w| weapons.get(w).ok())
                .map(|(w, m)| (w.id.as_ref(), m.map(|m| (m.clip, m.reserve))));
            frame(Some(health), stamina, held, |id| names.0.iter().find(|(i, _)| i == id).map(|(_, n)| n.clone()))
        }
        None => Frame::default(),
    };
    if state.frame != f {
        state.frame = f;
    }
}

/// The frame into the nodes (only when it changed).
#[allow(clippy::type_complexity)]
pub fn sync(
    state: Res<DodsHudState>,
    mut texts: Query<(&mut Text, Option<&mut TextColor>, Has<HealthText>, Has<AmmoText>), Or<(With<HealthText>, With<AmmoText>, With<WeaponText>)>>,
    mut bars: Query<(&mut Node, &mut BackgroundColor, Has<HealthBar>), Or<(With<HealthBar>, With<StaminaBar>)>>,
) {
    if !state.is_changed() {
        return;
    }
    let f = &state.frame;
    let low = f.health.is_some_and(|h| h <= LOW_HEALTH);
    for (mut text, colour, health, ammo) in &mut texts {
        let want = if health {
            f.health.map(|h| h.to_string()).unwrap_or_default()
        } else if ammo {
            f.ammo.map(|(c, r)| format!("{c} | {r}")).unwrap_or_default()
        } else {
            f.weapon.clone()
        };
        if text.0 != want {
            text.0 = want;
        }
        if health && let Some(mut c) = colour {
            let want = if low { LOW } else { TEXT };
            if c.0 != want {
                c.0 = want;
            }
        }
    }
    for (mut node, mut colour, health) in &mut bars {
        let share = if health { f.health.unwrap_or(0) as f32 / 100.0 } else { f.stamina.unwrap_or(0.0) };
        node.width = percent(share.clamp(0.0, 1.0) * 100.0);
        if health {
            colour.0 = if low { LOW } else { HEALTH };
        }
    }
}

/// DoD:S's HUD: its state, root and nodes, drawing while its HUD is the
/// gameplay's.
pub struct DodsHudPlugin;

impl Plugin for DodsHudPlugin {
    fn build(&self, app: &mut App) {
        app.world_mut().get_resource_or_init::<chimera_client::client::hud::HudDraws>().0.push((GAME, DRAWS));
        // Its kill feed is the app's: it shows none of its own.
        chimera_client::client::kill_feed::GameKillFeedAppExt::register_game_kill_feed(app, GAME, |_| 0);
        app.init_resource::<DodsHudState>()
            .init_resource::<Names>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, (collect, sync).chain().run_if(chimera_core::gameplay::hud_is(GAME)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_frame_reads_health_stamina_and_rounds() {
        let f = frame(
            Some(&Health { current: 0.734, max: 1.0 }),
            Some(&DodsStamina { value: 40.0, sprinting: false }),
            Some(("dods:weapon_garand", Some((8, 80)))),
            |_| Some("M1 Garand".into()),
        );
        assert_eq!(f, Frame { health: Some(74), stamina: Some(0.4), weapon: "M1 Garand".into(), ammo: Some((8, 80)) });
        assert_eq!(frame(None, None, None, |_| None), Frame::default());
    }
}
