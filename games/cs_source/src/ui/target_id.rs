//! CS:S's target ID: the name of the player under the crosshair, in that
//! player's team colour: "Friend: <name> Health: <n>%" for a teammate,
//! "Enemy: <name>" for anyone else, "Hostage: Health: <n>%" for a hostage
//! (the game's `Cstrike_playerid_sameteam`, `_diffteam` and `_hostage`
//! strings from its localization; their `%s2`, after the name, is left
//! empty).
//! `hud_centerid 1` (Multiplayer > Advanced "Center player names", on by
//! default in `cfg/user.scr`) puts it in the middle under the crosshair;
//! 0 at the left. `mp_playerid` (the server's) limits it: 1 teammates
//! only, 2 nobody. A ray from the eye along the view finds the player
//! (the world and props block it, so does smoke); range and placement
//! are guesses (docs/tech-debt.md).
//!
//! `target_text` is the pure rule; `TargetIdPlugin` finds the target and
//! draws it.
//!
//! CS:S's target ID: a ray from the eye finds the player under the crosshair
//! (not through smoke); its name in the game's words and its team's colour
//! ("Friend: name Health: n%", "Enemy: name", "Hostage: Health: n%"), centred
//! (`hud_centerid 1`) or at the left; `mp_playerid` (`core::PlayerIdMode`)
//! limits whose; `TargetIdStatePlugin` finds it without a window (tests).
//!
//! Known shortcuts (docs/tech-debt.md): Target ID (`client::target_id`): range
//! 8192 units (a guess), shown only while the ray from the eye along the view
//! (with punch) hits the player first (CS:S may keep it a moment after); hidden
//! through smoke (`SightBlocker`, a guess at CS:S's); placed 260 lines down
//! centred (`hud_centerid 1`) or at (10, 420) at the left (0), guesses:
//! `hudlayout.res` gives `TargetID` the whole screen; the forms' `%s2` (after
//! the name) is left empty; team colours are the scheme's `T_Red`/`CT_Blue`;
//! hostages named "Hostage: Health: n%" in grey; no names while dead (the
//! spectator bars name the target). Captures to confirm: a teammate and an
//! enemy at 2-3 m under the crosshair with `hud_centerid 1` and `0`, and an
//! enemy through a smoke cloud. Why it's OK for now: Reads like CS:S's.

use avian3d::prelude::SpatialQuery;
use bevy::{prelude::*, window::PrimaryWindow};

use crate::{
    console::resource_cvar,
    core::{Health, Intent, LocalPlayer, MovementState, PlayerIdMode, SightBlocker, Team},
    map::{hud::ActiveHud, interp::RenderedView},
    objectives::hostages::Hostage,
    rules::Dead,
};

/// `hud_centerid`: 1 (CS:S's `cfg/user.scr` default) the name centred
/// under the crosshair, 0 at the left.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CenterId(pub u8);

impl Default for CenterId {
    fn default() -> Self {
        Self(1)
    }
}

/// How far the target ID looks, CS:S units (a guess: no spec gives it).
pub const RANGE: f32 = 8192.0;
const UNIT: f32 = 0.0254;

/// Where the text goes (in the virtual 640x480 screen: lines down from the
/// top, the left margin when not centred): guesses, below the crosshair,
/// and at the left above the health panel (`scripts/hudlayout.res` gives
/// the target ID the whole screen and no place).
const CENTER_Y: f32 = 260.0;
const LEFT_X: f32 = 10.0;
const LEFT_Y: f32 = 420.0;

/// The player under the crosshair, as the local player sees it.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TargetId {
    pub target: Option<Entity>,
    /// The words shown (empty: nothing).
    pub text: String,
    pub team: Option<Team>,
    /// Centred (`hud_centerid 1`).
    pub centered: bool,
}

/// The game's forms (`%s1` the name, `%s2` empty, `%s3` the health in
/// percent; the hostage's `%s1` its health).
const SAME_TEAM: (&str, &str) = ("#Cstrike_playerid_sameteam", "Friend: %s1%s2 Health: %s3");
const DIFF_TEAM: (&str, &str) = ("#Cstrike_playerid_diffteam", "Enemy: %s1%s2");
const HOSTAGE: (&str, &str) = ("#Cstrike_playerid_hostage", "Hostage: Health: %s1");

/// What the target ID says about `name` (team `theirs`, `health` in
/// percent, a hostage or not) to a player of team `mine` with
/// `mp_playerid` `mode`; None: nothing shown. `string` looks up a
/// localization token.
pub fn target_text(
    string: &dyn Fn(&str, &str) -> String,
    mine: Option<Team>,
    theirs: Option<Team>,
    name: &str,
    health: f32,
    hostage: bool,
    mode: u8,
) -> Option<String> {
    if mode >= 2 {
        return None;
    }
    let health = format!("{:.0}%", health.ceil().max(0.0));
    if hostage {
        let (token, fallback) = HOSTAGE;
        return Some(string(token, fallback).replace("%s1", &health).trim().to_string());
    }
    let mate = mine.is_some() && mine == theirs;
    if !mate && mode == 1 {
        return None;
    }
    let (token, fallback) = if mate { SAME_TEAM } else { DIFF_TEAM };
    let form = string(token, fallback);
    Some(
        form.replace("%s1", name)
            .replace("%s2", "")
            .replace("%s3", &health)
            .trim()
            .to_string(),
    )
}

/// Finding the target and the cvar, without a window (tests).
pub struct TargetIdStatePlugin;

impl Plugin for TargetIdStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CenterId>()
            .init_resource::<TargetId>()
            .init_resource::<PlayerIdMode>()
            .add_systems(Update, find.in_set(FindTarget));
        resource_cvar::<CenterId, u8>(
            app,
            "hud_centerid",
            "1: the name of the player under the crosshair centred below it; 0: at the left.",
            |c| &mut c.0,
        );
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .archive("hud_centerid");
    }
}

/// `TargetIdStatePlugin`'s system.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct FindTarget;

/// The target ID drawn.
pub struct TargetIdPlugin;

impl Plugin for TargetIdPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(TargetIdStatePlugin)
            .add_systems(
                Update,
                (
                    draw.run_if(crate::gameplay::hud_is(crate::game::GAME)),
                    // Another game's HUD up (a party app's player holding
                    // that game's gun): no names from CS:S's.
                    put_away.run_if(not(crate::gameplay::hud_is(crate::game::GAME))),
                )
                    .after(FindTarget),
            );
    }
}

type Who<'a> = (Option<&'a Name>, Option<&'a Health>, Option<&'a Team>, Has<Dead>, Has<Hostage>);

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn find(
    local: Option<
        Single<(Entity, &Transform, &Intent, &MovementState, Option<&RenderedView>, Option<&Team>, Has<Dead>), With<LocalPlayer>>,
    >,
    who: Query<Who, With<Intent>>,
    parents: Query<&ChildOf>,
    smoke: Query<&SightBlocker>,
    spatial: SpatialQuery,
    hud: Option<Res<ActiveHud>>,
    mode: Res<PlayerIdMode>,
    center: Res<CenterId>,
    shoulder: Option<Res<crate::client::third_person::Shoulder>>,
    mut out: ResMut<TargetId>,
) {
    let mut next = TargetId {
        centered: center.0 != 0,
        ..default()
    };
    if let Some(local) = local {
        let (me, at, intent, state, view, team, dead) = *local;
        let alive = !dead && who.get(me).ok().and_then(|w| w.1).is_none_or(|h| h.current > 0.0);
        if alive {
            let ray = crate::client::aim::crosshair_ray(shoulder.as_deref(), crate::client::aim::view_ray(at, intent, state, view));
            let eye = ray.eye;
            let target = crate::client::aim::character_under(
                &spatial,
                ray,
                RANGE * UNIT,
                me,
                |e| who.contains(e),
                &parents,
            );
            if let Some((target, point)) = target
                && let Ok((name, health, theirs, their_dead, hostage)) = who.get(target)
                && !their_dead
                && health.is_none_or(|h| h.current > 0.0)
                && !smoke.iter().any(|s| s.blocks(eye, point))
            {
                let menus = hud.as_ref().and_then(|h| h.0.menus.as_ref());
                let string = |t: &str, f: &str| menus.map_or_else(|| f.to_string(), |m| m.string(t, f).to_string());
                let name = name.map_or("Player", |n| n.as_str());
                let hp = health.map_or(100.0, |h| h.current * 100.0);
                if let Some(text) = target_text(&string, team.copied(), theirs.copied(), name, hp, hostage, mode.0) {
                    next.target = Some(target);
                    next.text = text;
                    next.team = theirs.copied();
                }
            }
        }
    }
    out.set_if_neq(next);
}

/// The target ID's text node.
#[derive(Component)]
#[require(crate::client::hud::HudPart)]
pub struct TargetIdText;

fn put_away(mut text: Query<&mut Visibility, With<TargetIdText>>) {
    for mut v in &mut text {
        v.set_if_neq(Visibility::Hidden);
    }
}

#[allow(clippy::type_complexity)]
fn draw(
    id: Res<TargetId>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<crate::client::fonts::UiFonts>,
    window: Query<&Window, With<PrimaryWindow>>,
    mut text: Query<(Entity, &mut Text, &mut TextColor, &mut TextFont, &mut Node, &mut Visibility), With<TargetIdText>>,
    mut commands: Commands,
) {
    let Ok((_, mut t, mut color, mut font, mut node, mut vis)) = text.single_mut() else {
        commands.spawn((
            TargetIdText,
            Text::default(),
            TextFont::default(),
            TextColor(Color::WHITE),
            TextLayout::justify(Justify::Center),
            Node {
                position_type: PositionType::Absolute,
                ..default()
            },
            Visibility::Hidden,
            GlobalZIndex(42),
        ));
        return;
    };
    if id.text.is_empty() {
        vis.set_if_neq(Visibility::Hidden);
        return;
    }
    vis.set_if_neq(Visibility::Inherited);
    if t.0 != id.text {
        t.0 = id.text.clone();
    }
    let c = crate::client::chat::run_color(hud.as_deref(), crate::map::radio::ChatColor::Team, id.team);
    color.set_if_neq(TextColor(c));
    let (w, h) = window.single().map_or((1280.0, 720.0), |w| (w.width(), w.height()));
    // The client scheme's TargetID font, its size where it lacks one.
    let want = fonts.client("TargetID", h, 12.0);
    if *font != want {
        *font = want;
    }
    let s = h / 480.0;
    let want = if id.centered {
        Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            width: px(w),
            top: px(CENTER_Y * s),
            justify_content: JustifyContent::Center,
            ..default()
        }
    } else {
        Node {
            position_type: PositionType::Absolute,
            left: px(LEFT_X * s),
            top: px(LEFT_Y * s),
            ..default()
        }
    };
    if *node != want {
        *node = want;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(_: &str, f: &str) -> String {
        f.to_string()
    }

    #[test]
    fn friend_and_enemy_forms() {
        let (t, ct) = (Some(Team(1)), Some(Team(2)));
        assert_eq!(
            target_text(&plain, ct, ct, "Bot 3", 74.2, false, 0).as_deref(),
            Some("Friend: Bot 3 Health: 75%")
        );
        assert_eq!(target_text(&plain, ct, t, "Bot 4", 10.0, false, 0).as_deref(), Some("Enemy: Bot 4"));
        assert_eq!(
            target_text(&plain, ct, None, "Hostage", 100.0, true, 1).as_deref(),
            Some("Hostage: Health: 100%")
        );
        // mp_playerid 1: teammates only; 2: nobody.
        assert_eq!(target_text(&plain, ct, t, "Bot 4", 10.0, false, 1), None);
        assert!(target_text(&plain, ct, ct, "Bot 3", 10.0, false, 1).is_some());
        assert_eq!(target_text(&plain, ct, ct, "Bot 3", 10.0, false, 2), None);
    }
}
