//! CS:S in the debug tools: the debug UI's Bots and Rounds tabs
//! (`client::debug_ui::GameDebug`: the bots' skill presets, each bot's
//! role and activity, the round's state) and the bots' overlays drawn
//! with gizmos (`chimera_drawbots 1`: sites and their approaches; per bot
//! its role ring, target, lead, route, goal, hold spot and watched
//! approaches, look target and the corner it checks, the teammate it
//! answers, planned grenade arc, target and burst point, a flash it looks
//! away from, its radio order and its last radio call (4 s) with a label
//! over its head: "following Player", "Cover Me!"; `bot_debug 1`: an
//! on-screen list of team plans and each bot's role, site, activity,
//! health, radio order, last radio call and how long ago). Added beside
//! the shell's own overlays (`client::debug_views`).

use bevy::prelude::*;

use crate::{
    bot::{Bot, Role, Tactics, radio::Order},
    client::debug_ui::GameDebug,
    console::resource_cvar,
    core::{Health, Intent, LocalPlayer, Team},
    map::radio::RadioCommands,
    rules::rounds::{Phase, RoundState},
    weapon::grenade::GrenadeKind,
};

/// The bots' overlays (after the shell's `DebugViewsPlugin`) and what
/// the debug UI shows of CS:S.
pub struct BotDebugPlugin;

impl Plugin for BotDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BotViews>()
            .insert_resource(GameDebug {
                bot_skills: Some(bot_skills),
                bot_state: Some(bot_state),
                round_status: Some(round_status),
            })
            .add_systems(Startup, spawn_bot_list)
            .add_systems(Update, (draw_bots, order_labels, bot_list).distributive_run_if(crate::gameplay::hud_is(crate::game::GAME)));
        resource_cvar::<BotViews, u8>(
            app,
            "chimera:drawbots",
            "1: each bot's role, target, last known enemy position, route, goal, hold spot, look target, grenade arc and flash it avoids.",
            |d| &mut d.bots,
        );
        resource_cvar::<BotViews, u8>(
            app,
            "bot_debug",
            "1: list every bot's team, role, site, activity and health on screen.",
            |d| &mut d.bot_list,
        );
    }
}

/// Lift off the floor, m (as the shell's `debug_views`).
const LIFT: f32 = 0.05;

#[derive(Resource, Default)]
struct BotViews {
    bots: u8,
    bot_list: u8,
}

/// The bots' skill presets (`bot::DIFFICULTIES`) as console lines.
fn bot_skills() -> Vec<(String, String)> {
    crate::bot::DIFFICULTIES
        .iter()
        .map(|(name, reaction, aim, turn)| {
            (
                name.to_string(),
                format!("bot_reaction {reaction}; bot_aim_error {aim}; bot_turn_rate {turn}"),
            )
        })
        .collect()
}

/// A bot's role and activity (None: not one of CS:S's bots).
fn bot_state(world: &World, e: Entity) -> Option<(String, String)> {
    let b = world.get::<Bot>(e)?;
    let activity = format!("{:?}", b.activity());
    // The debug UI's row offers "unfreeze" for an activity starting "frozen".
    Some((format!("{:?}", b.role()), if b.frozen { format!("frozen ({activity})") } else { activity }))
}

/// The round's state for the Rounds tab.
fn round_status(world: &World) -> Option<String> {
    let r = world.get_resource::<RoundState>()?;
    let now = world.resource::<Time>().elapsed_secs_f64();
    let phase = match r.phase {
        Phase::Off => "off (deathmatch)".to_string(),
        Phase::Freeze { until } => format!("freeze time, {:.0} s left", until - now),
        Phase::Live { ends, .. } => format!("live, {:.0} s left", ends - now),
        Phase::Over { until, winner } => format!(
            "over ({}), next in {:.0} s",
            match winner.map(|t| t.0) {
                Some(1) => "terrorists won",
                Some(2) => "counter-terrorists won",
                _ => "draw",
            },
            until - now
        ),
    };
    Some(format!("round {}: {phase}; wins T {} CT {}", r.number, r.wins[0], r.wins[1]))
}

/// Radio-driven orders in `chimera_drawbots`.
const ORDER_COLOR: Color = Color::srgb(1.0, 0.3, 1.0);

fn role_color(role: Role) -> Color {
    match role {
        Role::Attack => Color::srgb(1.0, 0.45, 0.1),
        Role::Defend => Color::srgb(0.2, 0.55, 1.0),
        Role::Roam => Color::srgb(0.7, 0.7, 0.7),
    }
}

fn draw_bots(
    views: Res<BotViews>,
    bots: Query<(&Bot, &GlobalTransform, &Intent, &Health)>,
    targets: Query<&GlobalTransform>,
    tactics: Option<Res<Tactics>>,
    mut gizmos: Gizmos,
) {
    if views.bots == 0 {
        return;
    }
    let flat = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
    // The sites (a yellow ring) and their approaches (orange: the
    // attackers', blue: the defenders').
    if let Some(t) = tactics.as_ref() {
        for site in &t.sites {
            let at = site.point + Vec3::Y * LIFT;
            gizmos.circle(
                Isometry3d::new(at, flat),
                crate::bot::tactics::ARRIVE_RADIUS,
                Color::srgb(1.0, 0.9, 0.2),
            );
            for (side, color) in [(0, role_color(Role::Attack)), (1, role_color(Role::Defend))] {
                for a in &site.approaches[side] {
                    gizmos.sphere(Isometry3d::from_translation(*a + Vec3::Y * 0.5), 0.5, color);
                }
            }
        }
    }
    for (bot, at, intent, health) in &bots {
        if health.current <= 0.0 {
            continue;
        }
        let eye = at.translation() + Vec3::Y * 0.6;
        let feet = at.translation() - Vec3::Y * 0.9 + Vec3::Y * LIFT;
        // Its role: a ring at its feet.
        let role = role_color(bot.role());
        gizmos.circle(Isometry3d::new(feet, flat), 0.5, role);
        // Where it looks, and at what.
        gizmos.line(eye, eye + intent.look_rotation() * Vec3::NEG_Z * 1.5, Color::WHITE);
        if bot.target.is_none()
            && let Some(look) = bot.look_target()
        {
            gizmos.line(eye, look, Color::srgba(1.0, 1.0, 0.3, 0.35));
        }
        if let Some(spot) = bot.checking() {
            gizmos.sphere(
                Isometry3d::from_translation(spot + Vec3::Y * 0.2),
                0.2,
                Color::srgb(1.0, 1.0, 0.3),
            );
        }
        if let Some(t) = bot.target.and_then(|t| targets.get(t).ok()) {
            gizmos.line(eye, t.translation(), Color::srgb(1.0, 0.2, 0.2));
        }
        if let Some((lead, _)) = bot.lead {
            gizmos.sphere(
                Isometry3d::from_translation(lead + Vec3::Y * 0.3),
                0.3,
                Color::srgb(1.0, 0.6, 0.0),
            );
            gizmos.line(eye, lead + Vec3::Y * 0.3, Color::srgba(1.0, 0.6, 0.0, 0.4));
        }
        if let Some(call) = bot.assisting() {
            gizmos.line(eye, call + Vec3::Y * 1.0, Color::srgb(0.2, 1.0, 0.6));
        }
        // A teammate's radio command (magenta): a line to whom it follows
        // or where it goes, a ring where it holds.
        if let Some(o) = bot.order() {
            let to = o.point() + Vec3::Y * LIFT;
            match o.order {
                Order::Hold { .. } => {
                    gizmos.circle(Isometry3d::new(to, flat), crate::bot::radio::HOLD_NEAR, ORDER_COLOR);
                }
                _ => {
                    gizmos.line(eye, to + Vec3::Y, ORDER_COLOR);
                    gizmos.sphere(Isometry3d::from_translation(to + Vec3::Y * 0.3), 0.3, ORDER_COLOR);
                }
            }
        }
        let (route, next) = bot.route();
        let ahead = route.iter().skip(next).map(|p| *p + Vec3::Y * 0.1);
        gizmos.linestrip(
            std::iter::once(at.translation()).chain(ahead),
            Color::srgb(0.3, 0.9, 1.0),
        );
        if let Some(goal) = bot.goal() {
            gizmos.sphere(
                Isometry3d::from_translation(goal + Vec3::Y * 0.3),
                0.25,
                Color::srgb(0.6, 0.4, 1.0),
            );
        }
        // Its hold spot (a box in its role's colour) and what it watches.
        if let Some(hold) = bot.hold() {
            gizmos.cube(
                Transform::from_translation(hold.spot + Vec3::Y * 0.25).with_scale(Vec3::splat(0.5)),
                role,
            );
            for w in &hold.watch {
                gizmos.line(hold.spot + Vec3::Y * 0.5, *w + Vec3::Y * 0.5, role.with_alpha(0.3));
            }
        }
        if let Some(plan) = bot.grenade_plan() {
            let color = match plan.kind {
                GrenadeKind::Blast => Color::srgb(1.0, 0.25, 0.1),
                GrenadeKind::Flash => Color::srgb(1.0, 1.0, 0.6),
                GrenadeKind::Smoke => Color::srgb(0.7, 0.7, 0.7),
            };
            gizmos.linestrip(plan.points.iter().copied(), color);
            // The target (a cross on the floor) and where it should go off.
            let t = plan.target + Vec3::Y * LIFT;
            gizmos.line(t - Vec3::X * 0.4, t + Vec3::X * 0.4, Color::srgb(0.2, 1.0, 0.3));
            gizmos.line(t - Vec3::Z * 0.4, t + Vec3::Z * 0.4, Color::srgb(0.2, 1.0, 0.3));
            gizmos.sphere(Isometry3d::from_translation(plan.pop), 0.2, color);
            if bot.throwing() {
                gizmos.line(eye, plan.start, color);
            }
        }
        if let Some(from) = bot.averting() {
            gizmos.line(eye, from, Color::srgba(1.0, 1.0, 1.0, 0.3));
            gizmos.sphere(Isometry3d::from_translation(from), 0.15, Color::WHITE);
        }
    }
}

/// A label over a bot carrying out a radio command (`chimera_drawbots`).
#[derive(Component)]
struct OrderLabel(Entity);

/// What a bot does for a teammate's radio command ("following Player",
/// "pressing on"), or None.
fn order_text(bot: &Bot, now: f64, name_of: impl Fn(Entity) -> String) -> Option<String> {
    match bot.order() {
        Some(o) => Some(o.describe(&name_of(o.from))),
        None if bot.urgent(now) => Some("pressing on".into()),
        None => None,
    }
}

/// Seconds a bot's radio call stays over its head (`chimera_drawbots`).
const SAID_SHOWN: f64 = 4.0;

/// A radio command's text ("Go go go!"), else its name.
fn call_text(radio: Option<&RadioCommands>, call: &str) -> String {
    radio
        .and_then(|r| r.get(call))
        .and_then(|c| c.variants.first())
        .map_or_else(|| call.to_string(), |v| v.1.clone())
}

/// `chimera_drawbots`: each bot's radio-driven order and its latest radio
/// call as text over its head.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn order_labels(
    views: Res<BotViews>,
    radio: Option<Res<RadioCommands>>,
    time: Res<Time<Fixed>>,
    bots: Query<(Entity, &Bot, &GlobalTransform, &Health)>,
    names: Query<(Option<&Name>, Has<LocalPlayer>)>,
    camera: Query<(&Camera, &GlobalTransform), With<crate::client::FirstPersonCamera>>,
    mut labels: Query<(Entity, &OrderLabel, &mut Text, &mut Node, &mut Visibility)>,
    fonts: Res<crate::client::fonts::UiFonts>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let name_of = |e: Entity| match names.get(e) {
        Ok((_, true)) => "Player".to_string(),
        Ok((Some(n), _)) => n.to_string(),
        _ => e.to_string(),
    };
    let cam = camera.iter().next();
    let mut shown: Vec<Entity> = Vec::new();
    if views.bots > 0 {
        for (e, bot, at, health) in &bots {
            let said = bot
                .last_call()
                .filter(|(_, at)| now - at < SAID_SHOWN)
                .map(|(call, _)| format!("\"{}\"", call_text(radio.as_deref(), call)));
            let lines: Vec<String> = order_text(bot, now, name_of).into_iter().chain(said).collect();
            if lines.is_empty() || health.current <= 0.0 {
                continue;
            }
            let text = lines.join("\n");
            let Some(p) = cam.and_then(|(c, ct)| c.world_to_viewport(ct, at.translation() + Vec3::Y * 1.2).ok()) else {
                continue;
            };
            shown.push(e);
            match labels.iter_mut().find(|l| l.1.0 == e) {
                Some((_, _, mut t, mut node, mut vis)) => {
                    if t.0 != text {
                        t.0 = text;
                    }
                    node.left = px(p.x);
                    node.top = px(p.y);
                    *vis = Visibility::Visible;
                }
                None => {
                    commands.spawn((
                        OrderLabel(e),
                        Text::new(text),
                        fonts.debug(13.0),
                        TextColor(ORDER_COLOR),
                        Node {
                            position_type: PositionType::Absolute,
                            left: px(p.x),
                            top: px(p.y),
                            ..default()
                        },
                        GlobalZIndex(40),
                    ));
                }
            }
        }
    }
    for (label, l, ..) in &labels {
        if !shown.contains(&l.0) {
            commands.entity(label).despawn();
        }
    }
}

#[derive(Component)]
struct BotList;

fn spawn_bot_list(mut commands: Commands, fonts: Res<crate::client::fonts::UiFonts>) {
    commands.spawn((
        BotList,
        Text::default(),
        fonts.debug(13.0),
        TextColor(Color::srgb(0.85, 1.0, 0.85)),
        Node {
            position_type: PositionType::Absolute,
            top: px(480.0),
            left: px(8.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

/// `bot_debug 1`: each team's plan, then one line per bot (name, team,
/// role, site, activity, `*` for the group leader, health, a teammate's
/// radio command it carries out, its last radio call and how long ago).
#[allow(clippy::too_many_arguments)]
fn bot_list(
    views: Res<BotViews>,
    radio: Option<Res<RadioCommands>>,
    bots: Query<(Entity, &Bot, &Team, &Health, Option<&Name>)>,
    names: Query<(Option<&Name>, Has<LocalPlayer>)>,
    time: Res<Time<Fixed>>,
    tactics: Option<Res<Tactics>>,
    mut text: Query<(&mut Text, &mut Visibility), With<BotList>>,
) {
    let Ok((mut text, mut vis)) = text.single_mut() else {
        return;
    };
    let show = views.bot_list > 0;
    let want = if show { Visibility::Visible } else { Visibility::Hidden };
    if *vis != want {
        *vis = want;
    }
    if !show {
        return;
    }
    let mut out = String::new();
    if let Some(t) = tactics.as_ref() {
        out += &format!("{:?}, {} sites\n", t.scenario, t.sites.len());
        for plan in &t.teams {
            let site = plan.site.map_or("-", |s| t.site_name(s));
            let wait = if plan.leader_wait { " (leader waits)" } else { "" };
            out += &format!("team {}: {:?} {site}{wait}\n", plan.team.0, plan.role);
        }
    }
    let mut rows: Vec<_> = bots.iter().collect();
    rows.sort_by_key(|r| (r.2.0, r.0));
    for (e, bot, team, health, name) in rows {
        let site = match (bot.site(), tactics.as_ref()) {
            (Some(s), Some(t)) => t.site_name(s).to_string(),
            _ => "-".to_string(),
        };
        let leader = tactics
            .as_ref()
            .and_then(|t| t.team(*team))
            .is_some_and(|p| p.leader == Some(e));
        let name = name.map_or_else(|| e.to_string(), |n| n.as_str().to_string());
        let order = order_text(bot, time.elapsed_secs_f64(), |e| match names.get(e) {
            Ok((_, true)) => "Player".to_string(),
            Ok((Some(n), _)) => n.to_string(),
            _ => e.to_string(),
        });
        let now = time.elapsed_secs_f64();
        let said = bot.last_call().map_or_else(String::new, |(call, at)| {
            format!("  said \"{}\" {:.0} s ago", call_text(radio.as_deref(), call), now - at)
        });
        out += &format!(
            "{name:<8} t{} {:<7} {site:<10} {:<10}{} hp {:.0}{}{said}\n",
            team.0,
            format!("{:?}", bot.role()),
            format!("{:?}", bot.activity()),
            if leader { " *" } else { "  " },
            health.current * 100.0,
            order.map_or_else(String::new, |o| format!("  [{o}]")),
        );
    }
    if text.0 != out {
        text.0 = out;
    }
}
