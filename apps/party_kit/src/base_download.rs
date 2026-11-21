//! The base game files up front (feature `present`; docs/plans/active/
//! onboarding.md, decision 12): the sets every game reads at startup
//! (`chimera_assets::sets::STARTUP_SETS`) come from the hub's asset
//! service as soon as this PC is signed in, in the background, never
//! blocking the menu.
//!
//! - **When**: a start already signed in fetches them from the start
//!   (`chimera_client::client::app::run`, `net::assets::StartupFiles`);
//!   the first run's end (`start_after_sign_in`, from
//!   `onboarding::finish`) starts the same fetch then, its plan made on a
//!   thread (`asset_startup::plan_here` asks the service for newer
//!   manifests, up to 5 s) through `BaseStarter` (the real one:
//!   `app::start_startup_fetch` for the running app's games, `RunningApp`;
//!   tests give their own). Nothing lacking, or no service: nothing shows.
//! - **The strip** (`show_strip`): on the main menu, at the bottom left,
//!   the kit's progress bar with the two lines the loading screen says
//!   (`asset_startup::progress_lines`); "Game files ready" for
//!   `DONE_SECS` once done, the reason when it failed (a join fetches
//!   them again). It takes no clicks and no keys (`Pickable::IGNORE`).
//!   A join that starts meanwhile takes the fetch over (it waits for it,
//!   its bytes on the loading screen: `StartupFiles` removed), and the
//!   strip goes.
//! - `ui_scene downloads <planning|running|done|failed|off>` stages it.
//!
//! Known shortcuts: the games whose files arrived are read from the
//! store at the next join (its bind remounts them) or the next start, not
//! at once; a hub's own customisation and the bulk sets (decision 12's
//! future note) aren't fetched.

use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use bevy::prelude::*;
use chimera_client::client::app::{GameApp, RunningApp};
use chimera_net::net::{
    asset_startup::{self, StartupFetch, Status},
    assets::StartupFiles,
};
use chimera_ui::{
    Role, Tone, UiStyle,
    controls::{Progress, Ui},
    motion::UiClock,
};

use crate::main_menu::MenuUiCamera;

/// Seconds "Game files ready" stays.
pub const DONE_SECS: f64 = 5.0;
/// Over the main menu, under its overlays (the account page, Settings).
pub const STRIP_Z: i32 = 90;

/// How the base files' fetch goes, as the strip shows it.
#[derive(Clone, Debug, PartialEq)]
pub enum BaseStatus {
    /// Its plan being made (the service asked what it has).
    Planning,
    /// The fetch's own (`asset_startup::Status`).
    Fetch(Status),
}

/// The base files' fetch as shown (None: nothing to show).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct BaseDownload {
    pub status: Option<BaseStatus>,
    /// When it ended (the UI clock): the strip goes `DONE_SECS` later.
    pub ended: Option<f64>,
    /// Fetches started after a sign-in (tests).
    pub started: u32,
    /// Its status comes from a running `StartupFiles` (not staged).
    pub live: bool,
}

/// What starts the fetch for the running app (None: nothing lacking or
/// no service). Called on a thread.
#[derive(Resource, Clone)]
pub struct BaseStarter(pub Arc<dyn Fn(Option<&'static GameApp>) -> Option<StartupFetch> + Send + Sync>);

impl Default for BaseStarter {
    fn default() -> Self {
        BaseStarter(Arc::new(|app| app.and_then(chimera_client::client::app::start_startup_fetch)))
    }
}

/// A plan under way on its thread: Some once made.
#[derive(Resource, Clone, Default)]
struct Planned(Arc<Mutex<Option<Option<StartupFetch>>>>);

#[derive(Component)]
pub struct BaseStrip;

pub struct BaseDownloadPlugin;

impl Plugin for BaseDownloadPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BaseDownload>().init_resource::<BaseStarter>().init_resource::<Planned>();
        chimera_client::client::ui_scene::UiSceneHooks::add(app.world_mut(), "downloads", scene);
        app.add_systems(Update, (poll, show_strip).chain());
    }
}

/// After the first run signed this PC in: the base files' fetch starts
/// (its plan on a thread), unless one runs already.
pub fn start_after_sign_in(world: &mut World) {
    if world.contains_resource::<StartupFiles>() || world.resource::<BaseDownload>().status.is_some() {
        return;
    }
    let app = world.get_resource::<RunningApp>().map(|a| a.0);
    let starter = world.resource::<BaseStarter>().0.clone();
    let out = world.resource::<Planned>().0.clone();
    let mut d = world.resource_mut::<BaseDownload>();
    d.status = Some(BaseStatus::Planning);
    d.ended = None;
    d.started += 1;
    let run = move || {
        let fetch = starter(app);
        *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(fetch);
    };
    if std::thread::Builder::new().name("base files' plan".into()).spawn(run).is_err() {
        world.resource_mut::<BaseDownload>().status = None;
    }
}

/// The plan's answer into `StartupFiles`, and the fetch's status into
/// `BaseDownload`.
fn poll(
    planned: Res<Planned>,
    files: Option<ResMut<StartupFiles>>,
    mut d: ResMut<BaseDownload>,
    clock: Option<Res<UiClock>>,
    mut commands: Commands,
) {
    let now = clock.map_or(0.0, |c| c.now);
    let made = planned.0.lock().unwrap_or_else(|e| e.into_inner()).take();
    if let Some(fetch) = made {
        match fetch {
            Some(f) => commands.insert_resource(StartupFiles(f)),
            None => d.status = None,
        }
    }
    match files.map(|mut f| f.0.status()) {
        Some(s) => {
            let ended = matches!(s, Status::Done(_) | Status::Failed(_));
            if ended && d.ended.is_none() {
                d.ended = Some(now);
            }
            let s = Some(BaseStatus::Fetch(s));
            if d.status != s {
                d.status = s;
            }
            if !d.live {
                d.live = true;
            }
        }
        // A join took the fetch over.
        None if d.live && d.ended.is_none() => {
            d.status = None;
            d.live = false;
        }
        None => {}
    }
    if d.ended.is_some_and(|t| now - t > DONE_SECS) {
        d.status = None;
        d.ended = None;
        d.live = false;
    }
}

/// The strip's words, bar and tone for `s` (None: not shown).
pub fn strip_view(s: &BaseStatus) -> (String, String, Progress, bool) {
    match s {
        BaseStatus::Planning => ("Checking your hub's game files".into(), "The menu stays yours meanwhile".into(), Progress::Connecting, false),
        BaseStatus::Fetch(Status::Connecting) => ("Reaching the hub's game files".into(), "Starting...".into(), Progress::Connecting, false),
        BaseStatus::Fetch(Status::Downloading { done, total, elapsed }) => {
            let (head, rest) = asset_startup::progress_lines(*done, *total, *elapsed);
            let share = if *total == 0 { 0.0 } else { *done as f32 / *total as f32 };
            (head, rest, Progress::Running(share.clamp(0.0, 1.0)), false)
        }
        BaseStatus::Fetch(Status::Done(_)) => ("Game files ready".into(), "Every game's base files are on this PC".into(), Progress::Ready, false),
        BaseStatus::Fetch(Status::Failed(why)) => ("Game files didn't download".into(), format!("{why} · a join tries again"), Progress::Pending, true),
    }
}

#[allow(clippy::type_complexity)]
fn show_strip(
    d: Res<BaseDownload>,
    menu: Option<Res<crate::main_menu::PartyMenu>>,
    onboarding: Option<Res<crate::onboarding::Onboarding>>,
    camera: Query<Entity, With<MenuUiCamera>>,
    root: Query<Entity, With<BaseStrip>>,
    (theme, kit, units): (Option<Res<chimera_ui::Theme>>, Option<Res<chimera_ui::KitFonts>>, Option<Res<chimera_ui::UiUnits>>),
    mut shown: Local<Option<(BaseStatus, u32, Option<Entity>)>>,
    mut commands: Commands,
) {
    let (Some(theme), Some(units)) = (theme.as_deref(), units.as_deref()) else { return };
    let on_menu = menu.as_deref().is_some_and(|m| m.screen == crate::main_menu::Screen::MainMenu);
    let first_run = onboarding.as_deref().is_some_and(|o| o.active);
    let cam = camera.iter().next();
    // Redrawn at most every half megabyte's change of tenths (the bar's
    // share), not every frame's byte count.
    let key = d.status.clone().filter(|_| on_menu && !first_run).map(|s| {
        let s = match s {
            BaseStatus::Fetch(Status::Downloading { done, total, elapsed }) => BaseStatus::Fetch(Status::Downloading {
                done: done / 500_000 * 500_000,
                total,
                elapsed: Duration::from_secs(elapsed.as_secs()),
            }),
            s => s,
        };
        (s, (units.height * 8.0) as u32, cam)
    });
    if *shown == key {
        return;
    }
    for e in &root {
        commands.entity(e).despawn();
    }
    if let Some((s, _, cam)) = key.clone() {
        let ui = Ui::new(theme, kit.as_deref(), *units);
        let c = &mut commands;
        let (head, rest, progress, failed) = strip_view(&s);
        let r = c
            .spawn((
                BaseStrip,
                GlobalZIndex(STRIP_Z),
                UiStyle { w: Some(30.0), gap: Some(0.7), pad: Some([1.1, 1.4].into()), ..UiStyle::column() }
                    .abs([Some(4.6), None, None, Some(5.2)])
                    .bg_a(Tone::Ink2, 0.9)
                    .outline(if failed { Tone::Warn } else { Tone::Fg5 }, 1.0),
                Node::default(),
                Pickable::IGNORE,
            ))
            .id();
        let top = ui.boxed(c, r, UiStyle { gap: Some(0.7), align: Some(AlignItems::Center), ..UiStyle::row() });
        ui.mark(c, top, if failed { Tone::Warn } else { Tone::Acc }, 0.55, 0.55);
        ui.text_plain(c, top, Role::Mono, 0.56, if failed { Tone::Warn } else { Tone::AccText }, "BASE FILES");
        ui.text_plain(c, r, Role::DisplayBold, 0.9, Tone::Fg, &head);
        ui.progress(c, r, progress, "", 27.2, 0.7, 0.5, false);
        ui.text_plain(c, r, Role::Mono, 0.56, Tone::Fg3, &rest);
        if let Some(cam) = cam {
            c.entity(r).insert(UiTargetCamera(cam));
        }
    }
    *shown = key;
}

fn scene(w: &mut World, args: &[String]) -> Result<(), String> {
    let now = w.get_resource::<UiClock>().map_or(0.0, |c| c.now);
    let status = match args.first().map(String::as_str).unwrap_or("running") {
        "planning" => Some(BaseStatus::Planning),
        "running" => Some(BaseStatus::Fetch(Status::Downloading { done: 148_300_000, total: 371_300_000, elapsed: Duration::from_secs(21) })),
        "done" => Some(BaseStatus::Fetch(Status::Done("staged".into()))),
        "failed" => Some(BaseStatus::Fetch(Status::Failed("the hub at hub.example.net didn't answer".into()))),
        "off" => None,
        other => return Err(format!("ui_scene downloads {other}: planning, running, done, failed or off")),
    };
    w.remove_resource::<StartupFiles>();
    // Staged: held (no end) so a capture sees it.
    *w.resource_mut::<BaseDownload>() = BaseDownload { status, ..Default::default() };
    let _ = now;
    Ok(())
}
