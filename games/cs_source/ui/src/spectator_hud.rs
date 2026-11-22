//! CS:S's spectator HUD while dead or spectating (the shell's
//! `client::spectate` runs the state and the camera): with the game's
//! layout (`GameMenus::spectator`, CS:S's `spectator.res`) black top and
//! bottom bars across the window, team scores, "Map: de_dust2" and the
//! round clock at the top right, the target's "name (health)" in its team
//! colour at the bottom, the mode where `bottomspectator.res`'s view list
//! sits (`SpectatorBarsUp`: while they show, the HUD hides its round
//! clock and money and puts the death notices under the top bar); else a
//! plain bottom panel ("Spectating: Bot 3 [100]", weapon, mode; "Killed
//! by ..." in the death cam).

use bevy::prelude::*;

use crate::{
    client::spectate::{SpecMode, SpecPhase, Spectator, place_camera},
    core::{Health, Team},
    weapon::{Inventory, Weapon},
};

/// The spectator's HUD (after the shell's `SpectatePlugin`).
pub struct SpectatorHudPlugin;

impl Plugin for SpectatorHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpectatorBarsUp>().add_systems(
            Update,
            (spectator_panel.after(place_camera).after(crate::client::front_end::FrontEndSync),)
                .distributive_run_if(crate::gameplay::hud_is(crate::game::GAME)),
        );
    }
}

#[derive(Component)]
struct SpectatorPanel;

/// The game-look bars' root (the whole window).
#[derive(Component)]
struct SpectatorBars;

/// What the spectator HUD says: the watched (or killing) player's name
/// and health and their team, the mode's name, the weapon.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpecHudText {
    pub target: Option<(String, f32, Option<Team>)>,
    pub mode: Option<SpecMode>,
    pub weapon: String,
    pub killed_by: bool,
}

impl SpecHudText {
    /// The plain panel's text: "Spectating: Bot 3 [100]   AK47" and the
    /// mode, or "Killed by ..." during the death cam.
    pub fn plain(&self) -> String {
        match (&self.target, self.mode, self.killed_by) {
            (Some((name, _, _)), _, true) => format!("Killed by {name}"),
            (None, _, true) => String::new(),
            (Some((name, hp, _)), Some(mode), false) => {
                format!("Spectating: {name} [{hp:.0}]   {}\n{}", self.weapon, mode.name())
            }
            (None, Some(mode), false) => mode.name().to_string(),
            (_, None, false) => String::new(),
        }
    }
}

/// What to show for the spectator's state.
#[allow(clippy::type_complexity)]
fn hud_text(
    spec: &Spectator,
    who: &Query<(Option<&Name>, Option<&Health>, Option<&Inventory>, Option<&Team>)>,
    weapons: &Query<&Weapon>,
) -> SpecHudText {
    let about = |e: Entity| {
        let (name, health, _, team) = who.get(e).unwrap_or((None, None, None, None));
        let name = name.map_or_else(|| "Player".to_string(), |n| n.as_str().to_string());
        let hp = health.map_or(0.0, |h| (h.current * 100.0).ceil().max(0.0));
        (name, hp, team.copied())
    };
    match spec.phase {
        SpecPhase::Alive => SpecHudText::default(),
        SpecPhase::FreezeCam { killer, .. } => SpecHudText {
            target: who.contains(killer).then(|| about(killer)),
            killed_by: true,
            ..default()
        },
        SpecPhase::DeathCam { killer, .. } => SpecHudText {
            target: killer.filter(|k| who.contains(*k)).map(about),
            killed_by: true,
            ..default()
        },
        SpecPhase::Watching => {
            let mode = spec.effective_mode();
            let target = spec.target.filter(|_| mode != SpecMode::Roaming);
            let weapon = target
                .and_then(|t| who.get(t).ok())
                .and_then(|(_, _, i, _)| i?.active)
                .and_then(|w| weapons.get(w).ok())
                .map(|w| weapon_name(w.id))
                .unwrap_or_default();
            SpecHudText {
                target: target.map(about),
                mode: Some(mode),
                weapon,
                killed_by: false,
            }
        }
    }
}

/// The spectator HUD while dead: with the game's layout (CS:S's
/// `resource/ui/spectator.res`) its top and bottom bars across the
/// window: the team scores, the map's name and the round clock at the top
/// right, the watched player's "name (health)" in their team's colour at
/// the bottom, the camera mode where the spectator menu's view list sits
/// (`bottomspectator.res`'s `viewcombo`). Without it, a plain panel.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn spectator_panel(
    spec: Res<Spectator>,
    windows: Query<&Window>,
    who: Query<(Option<&Name>, Option<&Health>, Option<&Inventory>, Option<&Team>)>,
    weapons: Query<&Weapon>,
    mut panel: Query<(&mut Text, &mut TextFont, &mut Visibility), (With<SpectatorPanel>, Without<SpectatorBars>)>,
    mut bars: Query<(Entity, &mut Visibility), (With<SpectatorBars>, Without<SpectatorPanel>)>,
    (fonts, hud, map): (
        Res<crate::client::fonts::UiFonts>,
        Option<Res<crate::map::hud::ActiveHud>>,
        Option<Res<crate::map::LoadedMapName>>,
    ),
    (rounds, fixed): (Option<Res<crate::rules::rounds::RoundState>>, Res<Time<Fixed>>),
    panels: Option<Res<crate::client::front_end::Panels>>,
    mut last: Local<Option<BarsShot>>,
    mut shown: ResMut<SpectatorBarsUp>,
    mut commands: Commands,
) {
    let (Ok((mut text, mut font, mut vis)), Ok((root, mut bars_vis))) = (panel.single_mut(), bars.single_mut()) else {
        if panel.is_empty() {
            commands.spawn((
                SpectatorPanel,
                Text::default(),
                TextFont::default(),
                TextColor(Color::WHITE),
                TextShadow::default(),
                TextLayout::justify(Justify::Center),
                Node {
                    position_type: PositionType::Absolute,
                    bottom: Val::Percent(15.0),
                    width: Val::Percent(100.0),
                    justify_content: JustifyContent::Center,
                    ..default()
                },
                Visibility::Hidden,
                GlobalZIndex(44),
            ));
        }
        if bars.is_empty() {
            commands.spawn((
                SpectatorBars,
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.0),
                    height: percent(100.0),
                    ..default()
                },
                Visibility::Hidden,
                GlobalZIndex(43),
            ));
        }
        return;
    };
    let about = hud_text(&spec, &who, &weapons);
    let height = windows.iter().next().map_or(480.0, |w| w.height());
    let layouts = hud.as_ref().and_then(|h| h.0.menus.as_ref()).and_then(|m| {
        let bars = m.layouts.get(m.spectator.as_ref()?)?;
        Some((m, bars, m.spectator_menu.as_ref().and_then(|k| m.layouts.get(k))))
    });
    // The freeze cam shows its own panel over its picture: no bars (nor
    // in the picture it takes).
    let active = spec.active() && !matches!(spec.phase, SpecPhase::FreezeCam { .. });
    // The plain panel, without the game's layout.
    let line = if layouts.is_some() { String::new() } else { about.plain() };
    vis.set_if_neq(if line.is_empty() {
        Visibility::Hidden
    } else {
        Visibility::Inherited
    });
    if text.0 != line {
        text.0 = line;
    }
    // The client scheme's Default.
    let want = fonts.client("Default", height, 12.0);
    if *font != want {
        *font = want;
    }
    let (Some((menus, layout, menu_layout)), Some(hud), true) = (layouts, hud.as_deref(), active) else {
        bars_vis.set_if_neq(Visibility::Hidden);
        *last = None;
        shown.set_if_neq(SpectatorBarsUp(None));
        return;
    };
    bars_vis.set_if_neq(Visibility::Inherited);
    let top = layout.get("topbar").map_or(52.0, |c| c.tall) * height / 480.0;
    shown.set_if_neq(SpectatorBarsUp(Some(top)));
    let size = windows
        .iter()
        .next()
        .map_or(Vec2::new(640.0, 480.0), |w| Vec2::new(w.width(), w.height()));
    let map = map.map_or_else(String::new, |m| m.0.rsplit(':').next().unwrap_or(&m.0).to_string());
    let rounds = rounds.filter(|r| r.phase != crate::rules::rounds::Phase::Off);
    let shot = BarsShot {
        about,
        map,
        clock: rounds
            .as_ref()
            .and_then(|r| r.clock(fixed.elapsed_secs_f64()))
            .map(super::game_hud::clock_text),
        wins: rounds.map(|r| r.wins),
        size,
        menu_open: panels.is_some_and(|p| p.is_open("spectator")),
    };
    if last.as_ref() == Some(&shot) {
        return;
    }
    commands.entity(root).despawn_related::<Children>();
    let painter = crate::client::vgui::Painter::new(hud, &fonts, size.y);
    draw_bars(&mut commands, root, &painter, menus, layout, menu_layout, &shot);
    *last = Some(shot);
}

/// Whether the game-look spectator bars are up, and the top bar's height
/// in pixels: the HUD hides what they replace (round clock, money; health
/// and ammo are gone while dead) and moves the death notices below the
/// top bar.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SpectatorBarsUp(pub Option<f32>);

/// What the bars show; redrawn when it changes.
#[derive(Clone, Debug, PartialEq)]
struct BarsShot {
    about: SpecHudText,
    map: String,
    clock: Option<String>,
    wins: Option<[u32; 2]>,
    size: Vec2,
    /// The spectator menu is open: its drop-downs take the bottom bar
    /// (the player's name and the mode are in them).
    menu_open: bool,
}

/// The bars' text by control name (lower case), None: as the layout has
/// it.
pub fn bar_text(name: &str, about: &SpecHudText, map: &str, clock: Option<&str>, wins: Option<[u32; 2]>, menus: &crate::map::hud::GameMenus) -> Option<String> {
    Some(match name {
        "playerlabel" => about
            .target
            .as_ref()
            .map(|(name, hp, _)| {
                menus
                    .string("Spec_PlayerItem_Health", "%s1 (%s2)")
                    .replace("%s1", name)
                    .replace("%s2", &format!("{hp:.0}"))
            })
            .unwrap_or_default(),
        "extrainfo" => menus.string("Spec_Map", "Map: %s1").replace("%s1", map),
        "timerlabel" => clock.unwrap_or("").to_string(),
        "ctscorevalue" => wins.map_or(0, |w| w[1]).to_string(),
        "terscorevalue" => wins.map_or(0, |w| w[0]).to_string(),
        _ => return None,
    })
}

#[allow(clippy::too_many_arguments)]
fn draw_bars(
    commands: &mut Commands,
    root: Entity,
    painter: &crate::client::vgui::Painter,
    menus: &crate::map::hud::GameMenus,
    layout: &crate::map::hud::UiLayout,
    menu_layout: Option<&crate::map::hud::UiLayout>,
    shot: &BarsShot,
) {
    let bg = painter.color("BgColor", [0, 0, 0, 196]);
    let rects = layout.rects(Rect::from_corners(Vec2::ZERO, shot.size), painter.scale);
    // The bars span the window (the layout's are 640 wide).
    for name in ["topbar", "bottombarblank"] {
        if let Some(i) = layout.controls.iter().position(|c| c.name.eq_ignore_ascii_case(name)) {
            let r = rects[i];
            commands.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: px(0.0),
                    top: px(r.min.y),
                    width: percent(100.0),
                    height: px(r.height()),
                    ..default()
                },
                BackgroundColor(bg),
                ZIndex(-1),
                ChildOf(root),
            ));
        }
    }
    let team_color = |team: Option<Team>| match team.map(|t| t.0) {
        Some(1) => painter.color("T_Red", [255, 64, 64, 255]),
        Some(2) => painter.color("CT_Blue", [153, 204, 255, 255]),
        _ => painter.color("team0", [204, 204, 204, 255]),
    };
    let mut shown = |c: &crate::map::hud::UiControl| {
        let mut s = crate::client::vgui::Shown::of(c);
        let name = c.name.to_lowercase();
        if matches!(name.as_str(), "topbar" | "bottombarblank") {
            s.visible = false;
        }
        s.text = bar_text(
            &name,
            &shot.about,
            &shot.map,
            shot.clock.as_deref(),
            shot.wins,
            menus,
        );
        if name == "playerlabel" {
            s.visible = shot.about.target.is_some() && !shot.menu_open;
        }
        if matches!(name.as_str(), "timerclock" | "timerlabel") && shot.clock.is_none() {
            s.visible = false;
        }
        s
    };
    let drawn = painter.spawn(commands, root, shot.size, layout, crate::client::vgui::VguiMenu::Spectator, &mut shown);
    // The name in its team's colour.
    if let (Some((label, _)), Some((_, _, team))) = (drawn.get("playerlabel"), &shot.about.target) {
        let (label, color) = (*label, team_color(*team));
        // The label's text node is its child.
        commands.queue(move |w: &mut World| {
            let kids: Vec<Entity> = w.get::<Children>(label).map(|c| c.iter().collect()).unwrap_or_default();
            for k in kids {
                if let Some(mut c) = w.get_mut::<TextColor>(k) {
                    c.0 = color;
                }
            }
        });
    }
    // The camera mode, where the spectator menu's view list sits.
    if let Some(mode) = shot.about.mode.filter(|_| !shot.menu_open) {
        let bar = menu_layout.and_then(|l| {
            let rects = l.rects(Rect::from_corners(Vec2::ZERO, shot.size), painter.scale);
            let frame = l.controls.iter().position(|c| c.name.eq_ignore_ascii_case("specmenu"))?;
            let view = l.controls.iter().position(|c| c.name.eq_ignore_ascii_case("viewcombo"))?;
            Some(Rect::from_corners(
                rects[view].min + Vec2::Y * rects[frame].min.y,
                rects[view].max + Vec2::Y * rects[frame].min.y,
            ))
        });
        if let Some(r) = bar {
            commands
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(r.min.x),
                        top: px(r.min.y),
                        width: px(r.width()),
                        height: px(r.height()),
                        display: Display::Flex,
                        align_items: AlignItems::Center,
                        ..default()
                    },
                    ChildOf(root),
                ))
                .with_child((
                    Text::new(spec_mode_text(menus, mode)),
                    painter.font(None),
                    TextColor(painter.color("Label.TextColor", [255, 176, 0, 255])),
                ));
        }
    }
}

/// The mode's name as the game words it (`#Spec_Mode3`... in the
/// spectator menu's list: first person, chase camera, free look; fixed
/// view, `#Spec_Mode2`).
fn spec_mode_text(menus: &crate::map::hud::GameMenus, mode: SpecMode) -> String {
    let token = match mode {
        SpecMode::InEye => "Spec_Mode3",
        SpecMode::Chase => "Spec_Mode4",
        SpecMode::Roaming => "Spec_Mode5",
        SpecMode::Fixed => "Spec_Mode2",
    };
    menus.string(token, mode.name()).to_string()
}

/// `cs_source:weapon_ak47` -> `AK47`.
fn weapon_name(id: &str) -> String {
    let id = id.rsplit(':').next().unwrap_or(id);
    id.strip_prefix("weapon_").unwrap_or(id).to_uppercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bars_fill_in_the_game_strings() {
        let menus = crate::map::hud::GameMenus {
            strings: std::collections::HashMap::from([
                ("spec_map".to_string(), "Map: %s1".to_string()),
                ("spec_playeritem_health".to_string(), "%s1 (%s2)".to_string()),
            ]),
            ..default()
        };
        let about = SpecHudText {
            target: Some(("Bot 3".into(), 76.0, Some(Team(2)))),
            mode: Some(SpecMode::Chase),
            weapon: "AK47".into(),
            killed_by: false,
        };
        let text = |name| bar_text(name, &about, "de_dust2", Some("1:45"), Some([3, 5]), &menus);
        assert_eq!(text("playerlabel").as_deref(), Some("Bot 3 (76)"));
        assert_eq!(text("extrainfo").as_deref(), Some("Map: de_dust2"));
        assert_eq!(text("timerlabel").as_deref(), Some("1:45"));
        assert_eq!(text("ctscorevalue").as_deref(), Some("5"));
        assert_eq!(text("terscorevalue").as_deref(), Some("3"));
        assert_eq!(text("ctscorelabel"), None, "as the layout has it");
        assert_eq!(about.plain(), "Spectating: Bot 3 [76]   AK47\nChase Camera");
        let killed = SpecHudText {
            killed_by: true,
            ..about
        };
        assert_eq!(killed.plain(), "Killed by Bot 3");
    }
}
