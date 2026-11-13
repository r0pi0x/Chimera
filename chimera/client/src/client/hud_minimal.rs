//! The minimal HUD (`chimera:minimal`, the HUD slot `MINIMAL_HUD`): what a
//! game that lends no HUD shows when the app falls back to the shell's own
//! (`PartsRegistry::fallback_hud`). Plain text: health and armour (bottom
//! left), the held weapon's id and its clip and reserve (bottom right), a
//! cross crosshair. A playing minigame's panel is the shell's overlay's
//! (`client::minigame_panel`), as over every game's HUD.
//!
//! Its roots are `HudRoot(MINIMAL_HUD)`, so `HudSwitchPlugin` shows them
//! only while the gameplay's HUD is the minimal one and hides them when a
//! game's own HUD is up; while it is up it takes over the shell's status
//! texts and crosshair (`HudDraws`). An app adds `MinimalHudPlugin`.
//!
//! Known shortcuts: the weapon is called by its id (no display-name
//! registry in the shell); no kill feed or scoreboard (the opt-in `cl_hitmarker` works over it).

use bevy::prelude::*;
use chimera_core::game_parts::MINIMAL_HUD;

use super::hud::{GameDraws, HudDraws, HudRoot, HudSwitchPlugin};
use crate::{
    core::{Health, LocalPlayer},
    weapon::{Armor, Inventory, Magazine, Weapon},
};

/// The minimal HUD's nodes and what it takes over from the shell's.
pub struct MinimalHudPlugin;

impl Plugin for MinimalHudPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<HudSwitchPlugin>() {
            app.add_plugins(HudSwitchPlugin);
        }
        app.init_resource::<HudDraws>();
        app.world_mut().resource_mut::<HudDraws>().0.push((
            MINIMAL_HUD,
            GameDraws {
                crosshair: true,
                status: true,
                ..default()
            },
        ));
        app.add_systems(Startup, spawn).add_systems(Update, draw);
    }
}

#[derive(Component)]
pub struct MinimalStatus;
#[derive(Component)]
pub struct MinimalAmmo;
/// The mode switch's centre notice (`weapon::notice::CenterNotice`).
#[derive(Component)]
pub struct MinimalNotice;

const COLOR: Color = Color::srgb(0.95, 0.95, 0.95);

fn spawn(mut commands: Commands) {
    let text = |size: f32| (Text::default(), TextFont { font_size: FontSize::Px(size), ..default() }, TextColor(COLOR));
    commands
        .spawn((
            HudRoot(MINIMAL_HUD),
            Visibility::Hidden,
            GlobalZIndex(40),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                MinimalStatus,
                text(24.0),
                Node { position_type: PositionType::Absolute, bottom: px(14.0), left: px(20.0), ..default() },
            ));
            root.spawn((
                MinimalAmmo,
                text(24.0),
                TextLayout::justify(Justify::Right),
                Node { position_type: PositionType::Absolute, bottom: px(14.0), right: px(20.0), ..default() },
            ));
            // The weapon mode switch's notice, under the crosshair.
            root.spawn((
                MinimalNotice,
                text(20.0),
                TextLayout::justify(Justify::Center),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    width: percent(100.0),
                    top: percent(60.0),
                    ..default()
                },
            ));
            // The crosshair: two bars across the screen's middle.
            for (w, h) in [(14.0, 2.0), (2.0, 14.0)] {
                root.spawn((
                    BackgroundColor(COLOR),
                    Node {
                        position_type: PositionType::Absolute,
                        left: percent(50.0),
                        top: percent(50.0),
                        width: px(w),
                        height: px(h),
                        margin: UiRect { left: px(-w / 2.0), top: px(-h / 2.0), ..default() },
                        ..default()
                    },
                ));
            }
        });
}

#[allow(clippy::type_complexity)]
fn draw(
    player: Option<Single<(&Health, Option<&Armor>, Option<&Inventory>), With<LocalPlayer>>>,
    weapons: Query<(&Weapon, Option<&Magazine>)>,
    mut status: Single<
        &mut Text,
        (With<MinimalStatus>, Without<MinimalAmmo>, Without<MinimalNotice>),
    >,
    mut ammo: Single<
        &mut Text,
        (With<MinimalAmmo>, Without<MinimalStatus>, Without<MinimalNotice>),
    >,
    mut notice: Single<
        &mut Text,
        (With<MinimalNotice>, Without<MinimalStatus>, Without<MinimalAmmo>),
    >,
    center: Option<Res<crate::weapon::notice::CenterNotice>>,
    time: Res<Time>,
) {
    let wanted = center.and_then(|c| c.text(time.elapsed_secs()).map(str::to_string)).unwrap_or_default();
    if notice.0 != wanted {
        notice.0 = wanted;
    }
    let (mut s, mut a) = (String::new(), String::new());
    if let Some(p) = player {
        let (health, armor, inv) = *p;
        s = format!("+ {:.0}", (health.current * 100.0).ceil());
        if let Some(armor) = armor.filter(|a| a.amount > 0.0) {
            s += &format!("   armour {:.0}", (armor.amount * 100.0).round());
        }
        a = match inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok()) {
            Some((w, Some(m))) => format!("{}\n{} | {}", w.id, m.clip, m.reserve),
            Some((w, None)) => w.id.to_string(),
            None => String::new(),
        };
    }
    status.0 = s;
    ammo.0 = a;
}
