//! The objectives on the HUD (specs/cs_source/objectives.md, HUD table):
//! the carried-bomb / defusal-kit / rescue-zone icon (`HudC4`,
//! `HudDefuser`, `HudHostageRescueZone`), the scenario icon (a planted
//! bomb, the hostages left: `HudScenarioIcon`), the defusing progress
//! bar (`HudProgressBar`; arming types on the C4's screen instead), the game's messages (centre text for
//! everyone, hints for the local player, chat lines for the team), the
//! game's deny sound when the local player's +use finds nothing (neither a
//! map door or button nor the bomb or a hostage; spec 5, step 4), and
//! `chimera_objectives 1`, a debug readout of the bomb and hostages.
//!
//! The objectives on the HUD: the carried-bomb / kit / rescue-zone icon
//! (`HudC4` etc., flashing red in a target), the scenario icon (planted bomb; a
//! glyph per hostage left; the bomb blinking in step with its beeps), both icon-font glyphs drawn as `client::hud_text`
//! draws them (additive for the scheme's additive `Icons`: see-through), the defusing progress bar (`HudProgressBar`;
//! defusing only, off with `cl_c4progressbar 0`: arming types on the C4's
//! screen instead, spec Q16; its fill `BombProgress` and the C4 screen's text
//! `bomb_fx::C4ScreenText` come from `BombHudStatePlugin`, headless for tests),
//! centre messages ("The bomb has been planted."), hints and team chat lines
//! from `objectives::ObjectiveEvent`; the deny sound
//! (`map::RoundSounds::use_deny`) when the local player's +use press finds
//! neither a map entity (`LogicWorld::use_presses`) nor an objective;
//! `chimera_objectives 1` shows the bomb's timer and defuse and the hostages'
//! state; the centre print also says what the local player's weapon switched
//! to (`weapon::ModeTexts`: "Switched to burst-fire mode").

use bevy::prelude::*;

use crate::client::{
    chat::{ChatLine, Hint},
    fonts::UiFonts,
    hud_text::{Align, GlyphFont, HudText},
};
use crate::{
    console::resource_cvar,
    core::{Health, LocalPlayer, MovementState, Team},
    map::hud::{ActiveHud, HudCoord},
    objectives::{
        MapKind, MapObjectives, ObjectiveEvent,
        bomb::{BombRules, BombState, Defusing, PlantedBomb, hull},
        hostages::{Hostage, HostagePenalty, HostageTally},
    },
    weapon::economy::DefuseKit,
};

pub struct ObjectivesHudPlugin;

impl Plugin for ObjectivesHudPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Overlay>()
            .init_resource::<Centre>()
            .add_plugins(BombHudStatePlugin)
            .add_message::<crate::weapon::drop::UsedPickup>()
            .add_systems(Startup, spawn)
            .add_systems(
                Update,
                (
                    messages.before(centre),
                    status,
                    progress.after(progress_share),
                    centre,
                    overlay,
                ),
            )
            .add_systems(
                FixedUpdate,
                use_deny
                    .after(crate::core::SimSet::Weapons)
                    .after(crate::logic::LogicSet::Pre),
            );
        resource_cvar::<Overlay, u8>(
            app,
            "chimera:objectives",
            "1: show the bomb's timer and defuse and the hostages' state (debug).",
            |o| &mut o.0,
        );
    }
}

/// What the bomb shows the local player, without drawing it (so
/// headless tests can read it): the progress bar's fill
/// (`BombProgress`, with `cl_c4progressbar`) and the code on the C4's
/// screen (`bomb_fx::C4ScreenText`); and the weapon mode switch's centre
/// print (`mode_texts`, `centre_text`).
pub struct BombHudStatePlugin;

impl Plugin for BombHudStatePlugin {
    fn build(&self, app: &mut App) {
        c4_bar_cvar(app);
        app.init_resource::<BombProgress>()
            .init_resource::<super::bomb_fx::C4ScreenText>()
            .init_resource::<Centre>()
            .add_message::<crate::weapon::WeaponEvent>()
            .add_systems(
                Update,
                (progress_share, super::bomb_fx::screen_text, mode_texts.before(centre)),
            );
    }
}

/// The progress bar's fill (0 to 1) while the local player defuses the
/// bomb; None: no bar.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct BombProgress(pub Option<f32>);

/// `cl_c4progressbar` (archived).
pub(crate) fn c4_bar_cvar(app: &mut App) {
    app.init_resource::<C4ProgressBar>();
    resource_cvar::<C4ProgressBar, u8>(
        app,
        "cl_c4progressbar",
        "1: a progress bar while you defuse the bomb (Multiplayer > Advanced).",
        |o| &mut o.0,
    );
    app.world_mut()
        .resource_mut::<crate::console::Console>()
        .archive("cl_c4progressbar");
}

#[derive(Resource, Default)]
struct Overlay(u8);

/// `cl_c4progressbar` (CS:S's default 1): the progress bar while
/// defusing; 0 hides it (arming has none).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct C4ProgressBar(pub u8);

impl Default for C4ProgressBar {
    fn default() -> Self {
        Self(1)
    }
}

/// The centre message and until when it shows.
#[derive(Resource, Default)]
struct Centre(Option<(String, f32)>);

/// Seconds a centre message stays.
const CENTRE_SECONDS: f32 = 4.0;

#[derive(Component)]
enum Part {
    Status,
    Scenario,
    Frame,
    Centre,
    Overlay,
}

fn spawn(mut commands: Commands, fonts: Res<UiFonts>) {
    let abs = || Node {
        position_type: PositionType::Absolute,
        ..default()
    };
    // Drawn as `HudText` (`status`).
    for part in [Part::Status, Part::Scenario] {
        commands.spawn((part, abs(), Visibility::Hidden, GlobalZIndex(40)));
    }
    commands
        .spawn((
            Part::Frame,
            abs(),
            BorderColor::all(Color::srgb_u8(255, 30, 13)),
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.3)),
            Visibility::Hidden,
            GlobalZIndex(42),
        ))
        .with_children(|f| {
            f.spawn((
                FillMarker,
                Node {
                    height: Val::Percent(100.0),
                    ..default()
                },
                BackgroundColor(Color::srgb_u8(255, 30, 13)),
            ));
        });
    commands.spawn((
        Part::Centre,
        Text::default(),
        TextColor(Color::WHITE),
        TextShadow::default(),
        TextLayout::justify(Justify::Center),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(22.0),
            width: Val::Percent(100.0),
            justify_content: JustifyContent::Center,
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(45),
    ));
    commands.spawn((
        Part::Overlay,
        Text::default(),
        fonts.debug(13.0),
        TextColor(Color::srgb(1.0, 1.0, 0.6)),
        TextShadow::default(),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(8.0),
            top: Val::Percent(44.0),
            ..default()
        },
        Visibility::Hidden,
        GlobalZIndex(60),
    ));
}

/// The game's words for what happened: centre text for everyone, hints
/// to the local player, chat lines to the terrorists. Being given the
/// bomb and a hostage following are automatic hints (`rules::hints`).
#[allow(clippy::too_many_arguments)]
fn messages(
    mut events: MessageReader<ObjectiveEvent>,
    mut penalties: MessageReader<HostagePenalty>,
    local: Option<Single<(Entity, Option<&Team>), With<LocalPlayer>>>,
    names: Query<(Option<&Name>, Has<LocalPlayer>, Option<&crate::net::NetCharacter>)>,
    settings: Option<Res<crate::net::NetSettings>>,
    rules: Res<BombRules>,
    mut centre: ResMut<Centre>,
    mut hints: MessageWriter<Hint>,
    mut chat: MessageWriter<ChatLine>,
    time: Res<Time>,
) {
    let now = time.elapsed_secs();
    let (me, team) = local.map_or((Entity::PLACEHOLDER, None), |l| (l.0, l.1.copied()));
    let name = |e: Entity| match names.get(e) {
        Ok((name, local, net)) => crate::client::shown_name(e, local, net, name, settings.as_deref()),
        _ => format!("{e}"),
    };
    let carriers = team == Some(rules.carrier_team);
    let mut say = |text: &str| centre.0 = Some((text.to_string(), now + CENTRE_SECONDS));
    for ev in events.read() {
        use ObjectiveEvent as E;
        let hint: Option<String> = match ev {
            E::PickedUpBomb { who } => {
                if carriers && *who != me {
                    chat.write(ChatLine::plain(format!("{} picked up the bomb.", name(*who))));
                }
                (*who == me).then(|| "You picked up the bomb.".into())
            }
            E::DroppedBomb { who } => {
                if carriers {
                    chat.write(ChatLine::plain(format!("{} dropped the bomb.", name(*who))));
                }
                None
            }
            E::PlantRefused { who, reason } | E::DefuseRefused { who, reason } if *who == me => {
                Some(reason.to_string())
            }
            E::AbortPlant { who, left_zone: true } if *who == me => {
                Some("Arming sequence canceled. C4 can only be placed at a bomb target.".into())
            }
            E::Planted { .. } => {
                say("The bomb has been planted.");
                None
            }
            E::BeginDefuse { who, kit } if *who == me => Some(if *kit {
                "Defusing bomb WITH defuse kit.".into()
            } else {
                "Defusing bomb WITHOUT defuse kit.".into()
            }),
            E::Defused { .. } => {
                say("The bomb has been defused.");
                None
            }
            E::PickedUpKit { who } if *who == me => Some("You picked up a defuse kit!".into()),
            E::HostageRefused { who } if *who == me => Some("Only Counter-Terrorists can move the hostages.".into()),
            E::HostageHurt { attacker, .. } if *attacker == Some(me) => Some("You injured a hostage!".into()),
            E::HostageKilled { attacker, .. } if *attacker == Some(me) => Some("You killed a hostage!".into()),
            _ => None,
        };
        if let Some(h) = hint {
            hints.write(Hint(h));
        }
    }
    for p in penalties.read() {
        match p {
            HostagePenalty::Warning { who } if *who == me => {
                hints.write(Hint(
                    "If you kill one more hostage, you will be removed from the server.".into(),
                ));
            }
            HostagePenalty::Removed { who } => {
                chat.write(ChatLine::plain(format!(
                    "{} was removed for killing too many hostages.",
                    name(*who)
                )));
            }
            _ => {}
        }
    }
}

fn panel_box(hud: Option<&ActiveHud>, name: &str, fallback: (HudCoord, HudCoord, f32, f32), w: f32, h: f32) -> Rect {
    let scale = h / 480.0;
    let (x, y, wide, tall) = hud
        .and_then(|hud| hud.0.panels.get(name))
        .map_or(fallback, |p| (p.x, p.y, p.wide, p.tall));
    let min = Vec2::new(x.resolve(w, scale), y.resolve(h, scale));
    Rect::from_corners(min, min + Vec2::new(wide, tall) * scale)
}

/// The status icon and the scenario icon.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn status(
    hud: Option<Res<ActiveHud>>,
    fonts: Res<UiFonts>,
    windows: Query<&Window>,
    local: Option<
        Single<
            (
                Entity,
                &Transform,
                Option<&MovementState>,
                Option<&Team>,
                Option<&Health>,
                Has<DefuseKit>,
            ),
            With<LocalPlayer>,
        >,
    >,
    world_bits: (Res<MapObjectives>, Res<BombState>, Res<HostageTally>, Res<BombRules>),
    inventories: Query<&crate::weapon::Inventory>,
    bombs: Query<(), With<crate::objectives::bomb::C4>>,
    mut parts: Query<(Entity, &Part, &mut Visibility, Option<&mut HudText>)>,
    time: Res<Time>,
    mut events: MessageReader<ObjectiveEvent>,
    mut beeps: Local<(f64, f64)>,
    mut commands: Commands,
) {
    let (objectives, bomb, tally, rules) = world_bits;
    // The planted bomb's beeps (this clock): the last two.
    for ev in events.read() {
        if let ObjectiveEvent::Beep { .. } = ev {
            *beeps = (beeps.1, time.elapsed_secs_f64());
        }
    }
    if bomb.planted.is_none() {
        *beeps = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    }
    let Some(window) = windows.iter().next() else { return };
    let (w, h) = (window.width(), window.height());
    let hud_ref = hud.as_deref();
    let color = |name: &str, fallback: Color| hud_ref.and_then(|h| h.0.color(name)).unwrap_or(fallback);
    // The icon font's glyph as Source draws it: `Icons` is additive in
    // the scheme, so the scene shows through the icon (playtest
    // 2026-10-14: ours was opaque).
    let glyph = |name: &str| -> Option<(GlyphFont, f32, char)> {
        let (font, ch) = hud_ref?.0.icons.get(name)?.clone();
        let (font, tall) = fonts.hud_font(&font, h)?;
        Some((font, tall, ch))
    };
    let flash = (time.elapsed_secs() * 4.0) as u32 % 2 == 0;
    let green = color("HudIcon_Green", Color::srgb_u8(0, 160, 0));
    let red = color("HudIcon_Red", Color::srgb_u8(160, 0, 0));
    // Status: the bomb (red-flashing in a target), else the kit, else the
    // rescue zone (flashing) for a defender standing in one.
    let status = local.as_ref().and_then(|l| {
        let (e, t, state, team, health, kit) = **l;
        if health.is_some_and(|h| h.current <= 0.0) {
            return None;
        }
        let (lo, hi) = hull(t, state);
        let carrying = inventories
            .get(e)
            .is_ok_and(|inv| inv.weapons.iter().any(|w| bombs.contains(*w)));
        if carrying {
            let inside = objectives.bomb_target_at(lo, hi).is_some();
            return Some(("HudC4", "c4", if inside && flash { red } else { green }));
        }
        if kit {
            return Some(("HudDefuser", "defuser", green));
        }
        if team == Some(&rules.defuser_team) && objectives.rescue_zones.iter().any(|z| z.overlaps(lo, hi)) {
            return Some((
                "HudHostageRescueZone",
                "hostage_rescue",
                if flash { red } else { green },
            ));
        }
        None
    });
    // Scenario: the planted bomb, or a glyph per hostage left.
    let scenario = if bomb.planted.is_some() {
        // Blinking with the beeps, faster as they speed up (playtest
        // 2026-10-14, as CS:S).
        let since = (time.elapsed_secs_f64() - beeps.1) as f32;
        let interval = (beeps.1 - beeps.0) as f32;
        glyph("scenario_c4")
            .filter(|_| bomb_icon_lit(since, interval))
            .map(|g| (g, 1))
    } else if objectives.kind() == MapKind::Hostage && tally.total > 0 {
        let left = tally.total.saturating_sub(tally.rescued + tally.killed) as usize;
        glyph("scenario_hostage").map(|g| (g, left))
    } else {
        None
    };
    for (e, part, mut vis, shown) in &mut parts {
        let want = match part {
            Part::Status => status.and_then(|(panel, icon, c)| {
                let (handle, tall, ch) = glyph(icon)?;
                let r = panel_box(
                    hud_ref,
                    panel,
                    (HudCoord::Start(16.0), HudCoord::Start(240.0), 40.0, 40.0),
                    w,
                    h,
                );
                Some((r, handle, tall, ch.to_string(), c))
            }),
            Part::Scenario => scenario.as_ref().map(|((handle, tall, ch), n)| {
                let r = panel_box(
                    hud_ref,
                    "HudScenarioIcon",
                    (HudCoord::Centre(110.0), HudCoord::Start(443.0), 40.0, 44.0),
                    w,
                    h,
                );
                (
                    r,
                    handle.clone(),
                    *tall,
                    ch.to_string().repeat(*n),
                    color("Hostage_Yellow", Color::srgba_u8(255, 176, 0, 120)),
                )
            }),
            _ => continue,
        };
        let Some((r, font, tall, text, color)) = want else {
            vis.set_if_neq(Visibility::Hidden);
            continue;
        };
        vis.set_if_neq(Visibility::Inherited);
        let next = HudText {
            font,
            tall,
            text,
            color,
            at: r.min,
            align: Align::Left,
        };
        match shown {
            Some(mut t) => {
                t.set_if_neq(next);
            }
            None => {
                commands.entity(e).insert(next);
            }
        }
    }
}

/// Whether the planted bomb's scenario icon shows `since` seconds after
/// a beep that came `interval` after the one before: lit for the first
/// half of each beep's interval (a 1 s one before a second beep is
/// heard, `objectives::bomb::beep_interval`'s start), so it blinks in
/// step with the beeps (our reading of a playtest, no spec).
pub fn bomb_icon_lit(since: f32, interval: f32) -> bool {
    let interval = if interval.is_finite() && interval > 0.0 { interval } else { 1.0 };
    !since.is_finite() || since < interval / 2.0 || since > interval * 1.5
}

/// How far the local player's defuse is (`BombProgress`). Arming shows
/// no bar: the C4's own screen types the code (`bomb_fx`), where the bar
/// would cover it, and `cl_c4progressbar` is "Show a progress bar when
/// defusing the C4" (spec HUD table; whether CS:S draws one while
/// planting is the spec's Q16).
fn progress_share(
    local: Option<Single<(Entity, Option<&Defusing>), With<LocalPlayer>>>,
    bombs: Query<&PlantedBomb>,
    fixed: Res<Time<Fixed>>,
    clock: Res<crate::core::SimClock>,
    defuse_bar: Option<Res<C4ProgressBar>>,
    mut out: ResMut<BombProgress>,
) {
    let defuse_bar = defuse_bar.is_none_or(|b| b.0 != 0);
    let now = fixed.elapsed_secs_f64();
    let share = local.as_ref().filter(|_| defuse_bar).and_then(|l| {
        let (me, defusing) = **l;
        // Our own defuse is on the simulation's clock (a network client
        // predicts it).
        if let Some(d) = defusing {
            return Some(((clock.now - d.started) / (d.ends - d.started).max(1e-6)) as f32);
        }
        bombs
            .iter()
            .filter_map(|b| b.defuse)
            .find(|d| d.who == me)
            .map(|d| ((now - d.started) / (d.ends - d.started)) as f32)
    });
    out.set_if_neq(BombProgress(share.map(|s| s.clamp(0.0, 1.0))));
}

/// The progress bar (`BombProgress`).
fn progress(
    hud: Option<Res<ActiveHud>>,
    windows: Query<&Window>,
    share: Res<BombProgress>,
    mut frame: Query<(&Part, &mut Node, &mut Visibility)>,
    mut fill: Query<&mut Node, (With<FillMarker>, Without<Part>)>,
) {
    let share = share.0;
    let Some(window) = windows.iter().next() else { return };
    let (w, h) = (window.width(), window.height());
    let r = panel_box(
        hud.as_deref(),
        "HudProgressBar",
        (HudCoord::Centre(-150.0), HudCoord::Start(300.0), 300.0, 15.0),
        w,
        h,
    );
    for (part, mut node, mut vis) in &mut frame {
        if !matches!(part, Part::Frame) {
            continue;
        }
        *vis = if share.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        node.left = Val::Px(r.min.x);
        node.top = Val::Px(r.min.y);
        node.width = Val::Px(r.width());
        node.height = Val::Px(r.height());
        node.border = UiRect::all(Val::Px((h / 480.0).max(1.0)));
    }
    for mut node in &mut fill {
        node.width = Val::Percent(share.unwrap_or(0.0) * 100.0);
    }
}

#[derive(Component)]
struct FillMarker;

fn centre(
    mut state: ResMut<Centre>,
    windows: Query<&Window>,
    mut parts: Query<(&Part, &mut Text, &mut TextFont, &mut Visibility)>,
    time: Res<Time>,
    fonts: Res<UiFonts>,
) {
    let now = time.elapsed_secs();
    if state.0.as_ref().is_some_and(|(_, until)| now > *until) {
        state.0 = None;
    }
    // Centre print: the client scheme's CenterPrintText (Trebuchet).
    let want = fonts.client("CenterPrintText", windows.iter().next().map_or(480.0, |w| w.height()), 18.0);
    for (part, mut text, mut font, mut vis) in &mut parts {
        if !matches!(part, Part::Centre) {
            continue;
        }
        match &state.0 {
            Some((s, _)) => {
                if text.0 != *s {
                    text.0 = s.clone();
                }
                if *font != want {
                    *font = want.clone();
                }
                *vis = Visibility::Inherited;
            }
            None => *vis = Visibility::Hidden,
        }
    }
}

/// The local player's weapon stepping to another mode prints what it
/// switched to (`weapon::ModeTexts`, the install's strings: the Glock's
/// and FAMAS's "Switched to burst-fire mode"), in the centre print.
fn mode_texts(
    mut events: MessageReader<crate::weapon::WeaponEvent>,
    local: Query<(), With<LocalPlayer>>,
    texts: Query<&crate::weapon::ModeTexts>,
    hud: Option<Res<ActiveHud>>,
    mut centre: ResMut<Centre>,
    time: Res<Time>,
) {
    for ev in events.read().filter(|e| e.shown() && local.contains(e.owner)) {
        let crate::weapon::WeaponEventKind::ModeChanged { mode } = ev.kind else {
            continue;
        };
        let Some(token) = texts.get(ev.weapon).ok().and_then(|t| t.0.get(mode as usize)) else {
            continue;
        };
        let text = hud
            .as_ref()
            .and_then(|h| h.0.menus.as_ref())
            .and_then(|m| m.strings.get(&token.trim_start_matches('#').to_lowercase()))
            .cloned()
            .unwrap_or_else(|| token.to_string());
        centre.0 = Some((text, time.elapsed_secs() + CENTRE_SECONDS));
    }
}

/// The centre print's text now (tests).
pub fn centre_text(world: &World) -> Option<String> {
    world.get_resource::<Centre>().and_then(|c| c.0.as_ref().map(|(s, _)| s.clone()))
}

/// `chimera_objectives 1`: the bomb (where, time left, defuse progress)
/// and the hostages (leader, health), as text.
#[allow(clippy::type_complexity)]
fn overlay(
    show: Res<Overlay>,
    bombs: Query<(&PlantedBomb, &Transform)>,
    hostages: Query<(Entity, &Hostage, &Transform, &Health)>,
    carriers: Query<(Entity, Option<&Name>, &crate::weapon::Inventory)>,
    c4s: Query<(), With<crate::objectives::bomb::C4>>,
    state: (Res<BombState>, Res<HostageTally>),
    fixed: Res<Time<Fixed>>,
    mut parts: Query<(&Part, &mut Text, &mut Visibility)>,
) {
    let mut lines = Vec::new();
    if show.0 != 0 {
        let now = fixed.elapsed_secs_f64();
        let (bomb_state, tally) = state;
        for (e, name, inv) in &carriers {
            if inv.weapons.iter().any(|w| c4s.contains(*w)) {
                lines.push(format!(
                    "bomb carried by {}",
                    name.map_or(format!("{e}"), |n| n.to_string())
                ));
            }
        }
        for (b, t) in &bombs {
            let p = t.translation / 0.0254;
            lines.push(format!(
                "bomb planted at ({:.0} {:.0} {:.0}) site {:?}: {:.2} s left{}",
                p.x,
                -p.z,
                p.y,
                b.site,
                (b.explode_at - now).max(0.0),
                if b.defused { ", DEFUSED" } else { "" }
            ));
            if let Some(d) = b.defuse {
                lines.push(format!(
                    "  defusing {} ({}): {:.2} / {:.2} s",
                    d.who,
                    if d.kit { "kit" } else { "no kit" },
                    now - d.started,
                    d.ends - d.started
                ));
            }
        }
        if let Some(o) = bomb_state.outcome {
            lines.push(format!("bomb: {o:?}"));
        }
        if tally.total > 0 {
            lines.push(format!(
                "hostages: {} of {} rescued, {} killed",
                tally.rescued, tally.total, tally.killed
            ));
        }
        for (e, h, t, health) in &hostages {
            lines.push(format!(
                "  {e}: health {:.0}, leader {:?}, at {:.1} {:.1} {:.1}",
                health.current * 100.0,
                h.leader,
                t.translation.x,
                t.translation.y,
                t.translation.z
            ));
        }
    }
    for (part, mut text, mut vis) in &mut parts {
        if !matches!(part, Part::Overlay) {
            continue;
        }
        if lines.is_empty() {
            *vis = Visibility::Hidden;
            continue;
        }
        *vis = Visibility::Inherited;
        let s = lines.join("\n");
        if text.0 != s {
            text.0 = s;
        }
    }
}

/// Whether an objective event says `me` used something by pressing +use.
fn used(e: &ObjectiveEvent, me: Entity) -> bool {
    match e {
        ObjectiveEvent::BeginDefuse { who, .. }
        | ObjectiveEvent::DefuseRefused { who, .. }
        | ObjectiveEvent::HostageRefused { who } => *who == me,
        ObjectiveEvent::HostageFollows { leader, .. } | ObjectiveEvent::HostageStops { leader, .. } => *leader == me,
        _ => false,
    }
}

/// The deny sound for a +use press of the living local player that found
/// nothing: no map entity (`logic`'s use presses), no objective and no
/// weapon taken (`chimera_usepickup`).
#[allow(clippy::too_many_arguments)]
fn use_deny(
    local: Query<(Entity, &crate::core::Intent, Option<&Health>), With<LocalPlayer>>,
    logic: Option<Res<crate::logic::Logic>>,
    sounds: Option<Res<crate::map::RoundSounds>>,
    mut events: MessageReader<ObjectiveEvent>,
    mut picks: MessageReader<crate::weapon::drop::UsedPickup>,
    mut play: MessageWriter<crate::map::PlaySound>,
    mut was: Local<bool>,
) {
    let Some((me, intent, health)) = local.iter().next() else {
        events.clear();
        picks.clear();
        return;
    };
    let objective = events.read().fold(false, |found, e| found || used(e, me))
        | picks.read().fold(false, |found, p| found || p.who == me);
    let pressed = intent.use_key && !*was && health.is_none_or(|h| h.current > 0.0);
    *was = intent.use_key;
    if !pressed {
        return;
    }
    let map = logic.is_some_and(|l| l.world.use_presses.iter().any(|(e, found)| *e == me && *found));
    if !map
        && !objective
        && let Some(entry) = sounds.and_then(|s| s.use_deny.clone())
    {
        play.write(crate::map::PlaySound::ui(entry));
    }
}
