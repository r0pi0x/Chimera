//! The crash notice in the Signal look (feature `present`; docs/plans/
//! active/feedback-reports.md, P3b): when the last session died (a crash,
//! a freeze, a power loss) and `cl_crash_reports` is `ask`
//! (`chimera_client::client::crash::CrashReports::offer`), a small plate
//! at the foot of the main menu: the mark, "The last session ended
//! unexpectedly. Send a report?" (or "... crashed ..."), what it holds
//! (the log's last lines, memory and frame times; nothing else), and
//! **Send**, **Always send** (sets `cl_crash_reports always`) and **Not
//! now** (`CrashAction`s). After a choice the plate says what became of
//! it for a few seconds. Under a theme without the Signal look nothing
//! is drawn (the console's `crash_report` still works).
//!
//! `ui_scene crash [ended|crashed|sent|off]` stages it for screenshots.

use bevy::{prelude::*, ui::DeprecatedInteraction as Interaction};
use chimera_client::client::crash::{CrashAction, CrashReports};
use chimera_ui::{
    Role, Tone, UiStyle,
    controls::{Kind, NOTE, SENTENCE, State, Ui},
};

use crate::main_menu::{MENU_Z, MenuPage, MenuUiCamera, PartyMenu, Screen};

/// Seconds the word after a choice stays.
const SAID_SECS: f64 = 5.0;

/// The plate's width, cq.
const WIDTH: f32 = 50.0;

/// What the plate says it sends.
pub const HOLDS: &str = "The log's last lines, memory and frame times, the map. No passwords, tokens or game files.";

#[derive(Component)]
struct CrashNoticeRoot;

pub struct CrashSignalPlugin;

impl Plugin for CrashSignalPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<CrashReports>() {
            app.add_plugins(chimera_client::client::crash::CrashPlugin);
        }
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "crash", scene_crash);
        app.add_systems(Update, (clicks, show).chain());
    }
}

fn clicks(pressed: Query<(&Interaction, &CrashAction), Changed<Interaction>>, mut out: MessageWriter<CrashAction>) {
    for (i, a) in &pressed {
        if *i == Interaction::Pressed {
            out.write(*a);
        }
    }
}

/// What the plate shows: the offer, or the word after a choice.
#[derive(Clone, PartialEq)]
enum Shown {
    Offer { crashed: bool },
    Said(String),
}

#[allow(clippy::type_complexity)]
fn show(
    reports: Res<CrashReports>,
    menu: Option<Res<PartyMenu>>,
    time: Res<Time<Real>>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<CrashNoticeRoot>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    mut said_at: Local<Option<(String, f64)>>,
    mut shown: Local<Option<(Shown, (u32, u32), Option<Entity>)>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let now = time.elapsed_secs_f64();
    if let Some(s) = reports.said.as_ref().filter(|s| said_at.as_ref().is_none_or(|(t, _)| t != *s)) {
        *said_at = Some((s.clone(), now));
    }
    let on_menu = menu.as_deref().is_none_or(|m| m.screen == Screen::MainMenu && m.page == MenuPage::Main);
    let live = if !crate::esc_menu::signal(Some(theme)) || !on_menu {
        None
    } else if reports.offer {
        Some(Shown::Offer { crashed: reports.crashed })
    } else {
        said_at.as_ref().filter(|(_, at)| now - at < SAID_SECS).map(|(s, _)| Shown::Said(s.clone()))
    };
    let cam = camera.iter().next();
    let key = live.clone().map(|l| (l, ((units.height * 8.0) as u32, (units.scale_factor * 1000.0) as u32), cam));
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((what, _, cam)) = &key {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let r = spawn(&mut commands, &ui, what, &reports);
        if let Some(cam) = cam {
            commands.entity(r).insert(UiTargetCamera(*cam));
        }
    }
    *shown = key;
}

/// The plate under a root of its own, at the foot of the screen.
fn spawn(c: &mut Commands, ui: &Ui, what: &Shown, reports: &CrashReports) -> Entity {
    let root = c
        .spawn((
            CrashNoticeRoot,
            GlobalZIndex(MENU_Z + 2),
            Node {
                position_type: PositionType::Absolute,
                width: percent(100.0),
                bottom: ui.units.val(6.5),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .id();
    let plate = ui.boxed(
        c,
        root,
        UiStyle { w: Some(WIDTH), gap: Some(0.6), pad: Some([1.0, 1.4].into()), ..UiStyle::column() }.bg(Tone::Ink2).outline(Tone::Fg4, 1.0),
    );
    c.entity(plate).insert(bevy::ui::FocusPolicy::Block);
    ui.focus_bar(c, plate, 0.22);
    match what {
        Shown::Offer { crashed } => {
            let head = ui.boxed(c, plate, UiStyle { gap: Some(0.7), ..UiStyle::row() });
            ui.mark(c, head, Tone::Threat, 0.5, 0.7);
            ui.text(c, head, Role::DisplayBold, 0.82, Tone::Fg, reports.headline());
            let _ = crashed;
            ui.sentence(c, plate, NOTE, Tone::Fg3, HOLDS);
            let foot = ui.boxed(c, plate, UiStyle { gap: Some(0.5), margin: Some([0.6, 0.0, 0.0, 0.0].into()), ..UiStyle::row() });
            ui.sentence(c, foot, NOTE, Tone::Fg3, "Sent to this playtest's host");
            ui.boxed(c, foot, UiStyle::row().grow(1.0));
            let not_now = ui.button(c, foot, Kind::Ghost, "Not now", None, 2.2, State::NONE);
            c.entity(not_now).insert(CrashAction::Dismiss);
            let always = ui.button(c, foot, Kind::Secondary, "Always send", None, 2.2, State::NONE);
            c.entity(always).insert(CrashAction::Always);
            let send = ui.button(c, foot, Kind::Primary, "Send", None, 2.2, State::focused(true));
            c.entity(send).insert(CrashAction::Send);
        }
        Shown::Said(text) => {
            let row = ui.boxed(c, plate, UiStyle { gap: Some(0.7), ..UiStyle::row() });
            ui.mark(c, row, Tone::Acc, 0.5, 0.7);
            ui.sentence(c, row, SENTENCE, Tone::Fg, text);
        }
    }
    root
}

/// `ui_scene crash [ended|crashed|sent|off]`: the notice staged (no
/// session needed).
fn scene_crash(w: &mut World, args: &[String]) -> Result<(), String> {
    let mut r = w.resource_mut::<CrashReports>();
    match args.first().map(String::as_str) {
        Some("off") => {
            r.offer = false;
            r.said = None;
        }
        Some("sent") => {
            r.offer = false;
            r.said = Some("Crash report queued: it sends with the other reports".into());
        }
        Some("crashed") => {
            r.offer = true;
            r.crashed = true;
        }
        None | Some("ended") => {
            r.offer = true;
            r.crashed = false;
        }
        Some(other) => return Err(format!("ui_scene crash {other}: ended, crashed, sent or off")),
    }
    Ok(())
}
