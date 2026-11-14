//! The freeze cam's picture (`spectate::SpecPhase::FreezeCam`): once the
//! camera has travelled to the killer, the window's picture is taken and
//! held over the world (the world goes on underneath, unseen), a white
//! flash fades from it, the install's freeze sound plays
//! (`sound/ui/freeze_cam.wav`, `map::hud::UiSound::FreezeCam`), and the
//! game's freeze panel (`resource/ui/freezepanel_basic.res`) names the
//! killer. Its timings are the server's `spec_freeze_traveltime` (0.7 s)
//! and `spec_freeze_time` (5 s), CS:S's defaults (measured on its
//! dedicated server; replicated, `core::SpectatorRules`).
//!
//! Known shortcuts (docs/tech-debt.md): Freeze cam (`client::spectate`,
//! `client::freeze_cam`; the timings measured, the rest from the game's files
//! and general CS:S knowledge): it follows a 2 s death cam (a guess; no
//! capture of a human's freeze cam: specs/cs_source/spectating.md) only when
//! killed by someone still alive; the camera stops 128 units from the killer's
//! eye (no closer than the death cam was); the held picture is a screenshot of the window, so the
//! HUD of that moment is in it and live HUD parts drawn over it (chat, notices)
//! can double; the white flash (70 % fading over 0.35 s) is a guess; the panel
//! shows the killer's name, "KILLED YOU", "[F5] Save this moment" and the
//! killer's health gauge (its colours ours: the file gives none) but no avatar
//! or nemesis/revenge lines; keys wait until spectating starts;
//! the spectator bars hide during it (and so are not in its picture). Captures
//! to confirm: in CS:S, killed by a bot at a known spot (`bot_stop 1`,
//! `setpos`/`setang` both), frames every 0.1 s from the death to 8 s after
//! (when the zoom starts, how long it takes, how near it ends, the flash, how
//! long the picture holds), and `help spec_freeze_time` / `help
//! spec_freeze_traveltime`. Why it's OK for now: Reads like CS:S's; the numbers
//! are cvars.

use bevy::{
    prelude::*,
    render::view::screenshot::{Screenshot, ScreenshotCaptured},
    window::PrimaryWindow,
};

use crate::client::spectate::{SpecPhase, Spectator};
use crate::map::hud::ActiveHud;

/// The flash's starting opacity and how long it fades, seconds.
const FLASH_ALPHA: f32 = 0.7;
const FLASH_FADE: f32 = 0.35;

pub struct FreezeCamPlugin;

impl Plugin for FreezeCamPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Frozen>()
            .add_systems(
                Update,
                (
                    freeze.run_if(crate::gameplay::hud_is(crate::game::GAME)),
                    // Another game's HUD up (a party app's player holding
                    // that game's gun): the held picture goes.
                    release.run_if(not(crate::gameplay::hud_is(crate::game::GAME))),
                )
                    .after(crate::client::spectate::SpectateSet),
            );
    }
}

/// The held picture: for which freeze (its start), when it was asked
/// for, and the picture once taken.
#[derive(Resource, Default)]
pub struct Frozen {
    pub since: Option<f64>,
    pub at: f64,
    pub image: Option<Handle<Image>>,
    shown: bool,
}

#[derive(Component)]
#[require(crate::client::hud::HudPart)]
struct FreezeOverlay;

#[derive(Component)]
struct FreezeFlash;

fn release(mut frozen: ResMut<Frozen>, overlay: Query<Entity, With<FreezeOverlay>>, mut commands: Commands) {
    if frozen.since.is_some() {
        for e in &overlay {
            commands.entity(e).despawn();
        }
        *frozen = Frozen::default();
    }
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn freeze(
    time: Res<Time>,
    spec: Res<Spectator>,
    mut frozen: ResMut<Frozen>,
    overlay: Query<Entity, With<FreezeOverlay>>,
    mut flash: Query<&mut BackgroundColor, With<FreezeFlash>>,
    window: Query<&Window, With<PrimaryWindow>>,
    who: Query<(Option<&Name>, Option<&crate::core::Health>)>,
    hud: Option<Res<ActiveHud>>,
    fonts: Res<crate::client::fonts::UiFonts>,
    menu_ui: Option<Res<super::game_menu::MenuUi>>,
    sounds: Option<Res<Assets<crate::audio::clip_asset::SoundAsset>>>,
    clips: Option<ResMut<Assets<crate::audio::live_sound::LiveClip>>>,
    mut commands: Commands,
) {
    let now = time.elapsed_secs_f64();
    let holding = match spec.phase {
        SpecPhase::FreezeCam {
            since, travel, killer, ..
        } if now - since >= travel as f64 => Some((since, killer)),
        _ => None,
    };
    let Some((since, killer)) = holding else {
        if frozen.since.is_some() {
            for e in &overlay {
                commands.entity(e).despawn();
            }
            *frozen = Frozen::default();
        }
        return;
    };
    if frozen.since != Some(since) {
        // Take the picture now: it arrives a frame or two later.
        for e in &overlay {
            commands.entity(e).despawn();
        }
        *frozen = Frozen {
            since: Some(since),
            at: now,
            ..default()
        };
        commands
            .spawn(Screenshot::primary_window())
            .observe(|shot: On<ScreenshotCaptured>, mut images: ResMut<Assets<Image>>, mut frozen: ResMut<Frozen>| {
                frozen.image = Some(images.add(shot.image.clone()));
            });
        // The freeze sound.
        let sound = super::game_menu::ui_sound(
            menu_ui.as_deref(),
            sounds.as_deref(),
            crate::map::hud::UiSound::FreezeCam,
        );
        if let (Some(clip), Some(mut clips)) = (sound, clips) {
            let gains = std::sync::Arc::new(crate::audio::live_sound::Gains::new(1.0, 1.0));
            let handle = clips.add(crate::audio::live_sound::LiveClip::new(clip, gains, default()));
            commands.spawn((AudioPlayer(handle), PlaybackSettings::DESPAWN));
        }
        return;
    }
    if !frozen.shown
        && let Some(image) = frozen.image.clone()
    {
        frozen.shown = true;
        let root = commands
            .spawn((
                FreezeOverlay,
                ImageNode::new(image),
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100.0),
                    height: percent(100.0),
                    ..default()
                },
                GlobalZIndex(41),
            ))
            .id();
        commands.spawn((
            FreezeFlash,
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(Color::srgba(1.0, 1.0, 1.0, FLASH_ALPHA)),
            ChildOf(root),
        ));
        let height = window.single().map_or(480.0, |w| w.height());
        let width = window.single().map_or(640.0, |w| w.width());
        let (name, health) = who.get(killer).unwrap_or((None, None));
        let name = name.map_or("Player", |n| n.as_str()).to_string();
        let health = health.map_or(0.0, |h| h.current.clamp(0.0, 1.0));
        if let Some(hud) = hud.as_deref() {
            panel(&mut commands, root, hud, &fonts, Vec2::new(width, height), &name, health);
        }
    }
    // The flash fades from when the picture shows.
    let t = (now - frozen.at) as f32;
    let a = FLASH_ALPHA * (1.0 - t / FLASH_FADE).clamp(0.0, 1.0);
    for mut c in &mut flash {
        c.set_if_neq(BackgroundColor(Color::srgba(1.0, 1.0, 1.0, a)));
    }
}

/// The game's freeze panel: its frame and, in it, the killer's name, the
/// words over it, the screenshot hint (`ScreenshotLabel`, "[F5] Save this
/// moment": F5 is bound to `jpeg`) and the killer's health (`KillerHealth`,
/// a gauge `health` 0..1 full). The avatar is left out (no avatars), as are
/// the nemesis icon (no domination counts) and the avatar's glow (its
/// picture isn't in the install).
fn panel(
    commands: &mut Commands,
    root: Entity,
    hud: &ActiveHud,
    fonts: &crate::client::fonts::UiFonts,
    size: Vec2,
    killer: &str,
    health: f32,
) {
    let Some(menus) = hud.0.menus.as_ref() else { return };
    let Some((outer, inner)) = menus
        .freeze_panel
        .as_ref()
        .and_then(|(o, i)| Some((menus.layouts.get(o)?, menus.layouts.get(i)?)))
    else {
        return;
    };
    let painter = crate::client::vgui::Painter::new(hud, fonts, size.y);
    let rects = outer.rects(Rect::from_corners(Vec2::ZERO, size), painter.scale);
    let Some((frame, r)) = outer
        .controls
        .iter()
        .zip(&rects)
        .find(|(c, _)| c.name.eq_ignore_ascii_case("FreezePanelBG"))
    else {
        return;
    };
    let bg = frame.bg.map_or(Color::srgba_u8(0, 0, 0, 192), |[r, g, b, a]| Color::srgba_u8(r, g, b, a));
    let e = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(r.min.x),
                top: px(r.min.y),
                width: px(r.width()),
                height: px(r.height()),
                ..default()
            },
            BackgroundColor(bg),
            ChildOf(root),
        ))
        .id();
    let words = freeze_words(&|t, f| menus.string(t, f).to_string(), killer);
    let mut shown = |c: &crate::map::hud::UiControl| {
        let mut s = crate::client::vgui::Shown::of(c);
        match c.name.to_lowercase().as_str() {
            "killername" => s.text = Some(words.0.clone()),
            "infolabel1" => s.text = Some(words.1.clone()),
            "infolabel2" => s.text = Some(words.2.clone()),
            "screenshotlabel" => {}
            _ => s.visible = false,
        }
        s
    };
    painter.spawn(commands, e, r.size(), inner, crate::client::vgui::VguiMenu::Spectator, &mut shown);
    // The health gauge (a `HorizontalGauge`, which the painter doesn't
    // draw): the panel's foreground colour over a dark track (ours).
    let inner_rects = inner.rects(Rect::from_corners(Vec2::ZERO, r.size()), painter.scale);
    if let Some((_, g)) = inner
        .controls
        .iter()
        .zip(&inner_rects)
        .find(|(c, _)| c.name.eq_ignore_ascii_case("KillerHealth") && c.visible)
    {
        let fg = frame.fg.map_or(Color::srgba_u8(251, 176, 59, 255), |[r, g, b, a]| Color::srgba_u8(r, g, b, a));
        let track = commands
            .spawn((
                KillerHealthGauge(health),
                Node {
                    position_type: PositionType::Absolute,
                    left: px(g.min.x),
                    top: px(g.min.y),
                    width: px(g.width()),
                    height: px(g.height().max(1.0)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.6)),
                ZIndex(2),
                ChildOf(e),
            ))
            .id();
        commands.spawn((
            Node {
                width: percent(health * 100.0),
                height: percent(100.0),
                ..default()
            },
            BackgroundColor(fg),
            ChildOf(track),
        ));
    }
}

/// The freeze panel's killer health gauge, how full (0..1).
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct KillerHealthGauge(pub f32);

/// The freeze panel's three lines: the killer's name, then the game's
/// `FreezePanel_Killer1` and `FreezePanel_Killer2` ("KILLED YOU", "").
/// (Nemesis and revenge need the domination counts CS:S keeps; we don't.)
pub fn freeze_words(string: &dyn Fn(&str, &str) -> String, killer: &str) -> (String, String, String) {
    (
        killer.to_string(),
        string("#FreezePanel_Killer1", "KILLED YOU"),
        string("#FreezePanel_Killer2", ""),
    )
}
