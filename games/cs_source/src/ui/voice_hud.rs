//! Voice chat's HUD, CS:S's panels from `scripts/hudlayout.res`:
//! `HudVoiceSelfStatus` (the `voice_self` icon, `voice/icntlk_local`,
//! while this player talks) and `HudVoiceStatus` (a row per other player
//! talking: their name in their team's colour, `scoreboard_dead`'s icon
//! when dead: the layout's `item_tall`, `item_spacing`, `text_xpos`,
//! `dead_xpos`/`dead_ypos`/`dead_wide`/`dead_tall`). Who talks is
//! `net::voice::VoiceActivity`.
//!
//! Voice chat's HUD: `HudVoiceSelfStatus`'s `voice_self` icon while talking;
//! `HudVoiceStatus`'s rows of who else talks (team colour, dead icon).
//!
//! Known shortcuts (docs/tech-debt.md): Voice HUD (`ui::voice_hud`): rows
//! stack up from the panel's bottom, the newest at the bottom, on a
//! translucent black box (guesses: capture C-voice); no Steam avatars
//! (`show_avatar`), no friend marks, no fade (`fade_in_time`,
//! `fade_out_time`); no icon over talking players' heads
//! (`voice/icntlk_pl`).

use bevy::{prelude::*, window::PrimaryWindow};

use super::game_hud::{panel_rect, scaled};
use crate::{
    core::{Dead, LocalPlayer, Team},
    map::hud::{ActiveHud, HudPanel},
    net::{NetCharacter, voice::VoiceActivity},
};

pub struct VoiceHudPlugin;

impl Plugin for VoiceHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VoiceActivity>()
            .add_systems(
                Update,
                (self_icon, rows)
                    .run_if(resource_exists::<ActiveHud>)
                    .run_if(crate::gameplay::hud_is(crate::game::GAME)),
            )
            // The look put away (another game's HUD up in a party app): the
            // icon and rows go with it, built again when it is back.
            .add_systems(Update, teardown.run_if(resource_removed::<ActiveHud>));
    }
}

#[derive(Component)]
struct SelfIcon;

/// A row of `HudVoiceStatus` (rebuilt when who talks changes).
#[derive(Component)]
struct Row;

fn teardown(parts: Query<Entity, Or<(With<SelfIcon>, With<Row>)>>, mut commands: Commands) {
    for e in &parts {
        commands.entity(e).despawn();
    }
}

/// An image of a HUD sprite (its sheet and rectangle), if the HUD has it.
fn sprite_image(hud: &ActiveHud, name: &str) -> Option<ImageNode> {
    let s = hud.0.sprites.get(name)?;
    let image = hud.1.get(&s.texture)?.clone();
    let [x, y, w, h] = s.rect;
    Some(ImageNode {
        image,
        rect: Some(Rect::new(x, y, x + w, y + h)),
        ..default()
    })
}

fn window_size(window: &Query<&Window, With<PrimaryWindow>>) -> (f32, f32) {
    window.single().map_or((1280.0, 720.0), |w| (w.width(), w.height()))
}

/// The talking player's own icon, in `HudVoiceSelfStatus`'s box.
fn self_icon(
    hud: Option<Res<ActiveHud>>,
    activity: Res<VoiceActivity>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut icon: Query<(&mut Node, &mut Visibility), With<SelfIcon>>,
    mut commands: Commands,
) {
    // Gone mid-frame (the map unloaded: back to the menu).
    let Some(hud) = hud else { return };
    let Some(panel) = hud.0.panels.get("HudVoiceSelfStatus") else { return };
    let Ok((mut node, mut vis)) = icon.single_mut() else {
        if let Some(image) = sprite_image(&hud, "voice_self") {
            commands.spawn((
                SelfIcon,
                image,
                Node {
                    position_type: PositionType::Absolute,
                    ..default()
                },
                Visibility::Hidden,
                GlobalZIndex(43),
            ));
        }
        return;
    };
    let (w, h) = window_size(&window);
    let r = panel_rect(panel, w, h);
    crate::client::ui_write::edit(&mut node, |node| {
        node.left = px(r.min.x);
        node.top = px(r.min.y);
        node.width = px(r.width());
        node.height = px(r.height());
    });
    vis.set_if_neq(if activity.local { Visibility::Inherited } else { Visibility::Hidden });
}

/// A talker as a row shows it.
#[derive(Clone, Debug, PartialEq)]
struct Talker {
    name: String,
    team: Option<Team>,
    dead: bool,
}

/// `HudVoiceStatus`'s numbers (480-line units).
struct Layout {
    item_tall: f32,
    spacing: f32,
    text_x: f32,
    dead: Rect,
}

impl Layout {
    fn of(p: &HudPanel) -> Self {
        let n = |k: &str, d: f32| p.num(k).unwrap_or(d);
        let (x, y) = (n("dead_xpos", 0.0), n("dead_ypos", 0.0));
        Self {
            item_tall: n("item_tall", 16.0),
            spacing: n("item_spacing", 2.0),
            text_x: n("text_xpos", 42.0),
            dead: Rect::new(x, y, x + n("dead_wide", 16.0), y + n("dead_tall", 16.0)),
        }
    }
}

/// The rows of `HudVoiceStatus`: who else talks.
#[allow(clippy::too_many_arguments)]
fn rows(
    hud: Option<Res<ActiveHud>>,
    activity: Res<VoiceActivity>,
    time: Res<Time<Real>>,
    fonts: Res<crate::client::fonts::UiFonts>,
    window: Query<&Window, With<PrimaryWindow>>,
    who: Query<(Option<&NetCharacter>, Option<&Name>, Option<&Team>, Has<Dead>), Without<LocalPlayer>>,
    old: Query<Entity, With<Row>>,
    mut shown: Local<(Vec<Talker>, (f32, f32))>,
    mut commands: Commands,
) {
    // Gone mid-frame (the map unloaded: back to the menu).
    let Some(hud) = hud else { return };
    let Some(panel) = hud.0.panels.get("HudVoiceStatus") else { return };
    let talkers: Vec<Talker> = activity
        .talking(time.elapsed_secs_f64())
        .into_iter()
        .filter_map(|e| {
            let (character, name, team, dead) = who.get(e).ok()?;
            let name = character.map(|c| c.name.clone()).or_else(|| name.map(|n| n.to_string()))?;
            Some(Talker {
                name,
                team: team.copied(),
                dead,
            })
        })
        .collect();
    let size = window_size(&window);
    if shown.0 == talkers && shown.1 == size {
        return;
    }
    *shown = (talkers.clone(), size);
    for e in &old {
        commands.entity(e).despawn();
    }
    let (w, h) = size;
    let s = h / 480.0;
    let r = panel_rect(panel, w, h);
    let layout = Layout::of(panel);
    let font = fonts.client("Default", h, 10.0);
    let dead_icon = sprite_image(&hud, "scoreboard_dead");
    let row_h = scaled(layout.item_tall, s);
    let step = scaled(layout.item_tall + layout.spacing, s);
    for (i, t) in talkers.iter().enumerate() {
        // From the panel's bottom up, the last talker lowest (a guess).
        let top = r.max.y - row_h - step * (talkers.len() - 1 - i) as f32;
        let color = crate::client::chat::run_color(Some(&hud), crate::map::radio::ChatColor::Team, t.team);
        let mut row = commands.spawn((
            Row,
            Node {
                position_type: PositionType::Absolute,
                left: px(r.min.x),
                top: px(top),
                width: px(r.width()),
                height: px(row_h),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.5)),
            GlobalZIndex(43),
        ));
        row.with_children(|row| {
            if t.dead
                && let Some(icon) = dead_icon.clone()
            {
                row.spawn((
                    icon,
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(scaled(layout.dead.min.x, s)),
                        top: px(scaled(layout.dead.min.y, s)),
                        width: px(scaled(layout.dead.width(), s)),
                        height: px(scaled(layout.dead.height(), s)),
                        ..default()
                    },
                ));
            }
            row.spawn((
                Text::new(t.name.clone()),
                font.clone(),
                TextColor(color),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(scaled(layout.text_x, s)),
                    ..default()
                },
            ));
        });
    }
}
