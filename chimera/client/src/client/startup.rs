//! The startup fetch's window (docs/plans/active/asset-service.md,
//! "Playtest delivery"): when the files the games read at startup come
//! from the asset service and aren't here yet (`net::asset_startup::plan_here`:
//! a game not installed, a service set, the store lacking its startup
//! sets), `app::run` shows a small window first: the game's title,
//! "Downloading game files: X of Y MB", a bar, the speed and the time left.
//! When the files are in, the game starts in a new process
//! (`relaunch::start_game`, the same command line) and this one waits for
//! it: Bevy's window system opens once per process, so the game's own
//! window can't follow this one in it. A refused token, an unreachable
//! service or another certificate shows its reason in the window, with
//! "Try again" (R, Enter), "Start anyway" (S: the game as before, its
//! files on demand) and "Quit" (Esc).
//!
//! With `--offscreen` the window is drawn into an image as the game's is
//! (`offscreen`), `--dump <dir>` saving its frames in `<dir>/startup`; a
//! tool run (`--frames N`) that fails ends after N frames of the error
//! instead of waiting for a choice.
//!
//! Known shortcuts: the window closes before the game's opens (a few
//! seconds without one while the game loads, now from the disk); the
//! window's text is English only.

use std::time::Duration;

use bevy::{
    app::ScheduleRunnerPlugin,
    prelude::*,
    winit::{UpdateMode, WinitSettings},
};

use super::{Args, ClientArgs, app::GameApp, widgets::Interaction};
use crate::net::asset_startup::{self, StartupFetch, Status};

const BACKGROUND: Color = Color::srgb(0.07, 0.08, 0.09);
const TEXT: Color = Color::srgb(0.92, 0.92, 0.9);
const DIM: Color = Color::srgb(0.6, 0.62, 0.64);
const ACCENT: Color = Color::srgb(0.95, 0.6, 0.15);
const ERROR: Color = Color::srgb(1.0, 0.45, 0.4);

/// Show the startup fetch's window if this start needs it: Some (the
/// game's exit, once it has run) when it did.
pub fn run_if_needed(game: &'static GameApp, args: &Args) -> Option<AppExit> {
    let games = game.hooks.games.map_or_else(|| vec![game.info], |games| games());
    let (settings, wanted) = asset_startup::plan_here(&games)?;
    let from = settings.remote.as_ref().map(|r| r.url.clone()).unwrap_or_default();
    eprintln!(
        "assets: the files the games read at startup aren't here yet: fetching them from {from} first ({})",
        wanted.iter().map(|w| format!("{} {}", w.game, w.sets.join(", "))).collect::<Vec<_>>().join("; ")
    );
    let restart = super::relaunch::without(std::env::args().skip(1), &[]);
    let mut app = App::new();
    window_app(&mut app, args, game.title);
    app.insert_resource(Screen {
        fetch: StartupFetch::start(settings.clone(), wanted.clone()),
        settings,
        wanted,
        restart,
        last: None,
        tool_frames: args.offscreen.and(args.frames),
    });
    let exit = app.run();
    // The window's app is done; the game (a new process) is what runs now.
    drop(app);
    crate::metrics::memory::release_free();
    Some(super::relaunch::wait_for_restart().unwrap_or(exit))
}

/// The window's app: Bevy's plugins as the game takes them (no sound),
/// a calm frame rate, the screen.
fn window_app(app: &mut App, args: &Args, title: &str) {
    let mut group = super::app::default_plugins(args, title).disable::<bevy::audio::AudioPlugin>();
    if args.offscreen.is_some() {
        group = group.set(ScheduleRunnerPlugin::run_loop(Duration::from_millis(33)));
    }
    let mut own = args.clone();
    own.dump_dir = args.dump_dir.as_ref().map(|d| d.join("startup"));
    app.insert_resource(ClientArgs(own))
        .add_plugins(group)
        .add_plugins(super::offscreen::OffscreenPlugin)
        .insert_resource(WinitSettings {
            focused_mode: UpdateMode::reactive(Duration::from_millis(100)),
            unfocused_mode: UpdateMode::reactive_low_power(Duration::from_millis(250)),
        })
        .insert_resource(ClearColor(BACKGROUND))
        .add_plugins(ScreenPlugin);
}

/// The screen's systems (also on a headless app: the tests).
pub struct ScreenPlugin;

impl Plugin for ScreenPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_screen).add_systems(Update, (show, choose).chain());
    }
}

/// The fetch and what to do after it.
#[derive(Resource)]
pub struct Screen {
    pub fetch: StartupFetch,
    pub settings: asset_startup::AssetSettings,
    pub wanted: Vec<asset_startup::Wanted>,
    /// The game's command line.
    pub restart: Vec<String>,
    /// The status shown last.
    pub last: Option<Status>,
    /// A tool run's frames (`--offscreen --frames`): the error shown this
    /// long, then the end.
    pub tool_frames: Option<u32>,
}

#[derive(Component)]
struct Head;
#[derive(Component)]
struct Detail;
#[derive(Component)]
struct Note;
#[derive(Component)]
struct BarFill;
#[derive(Component)]
struct Bar;
#[derive(Component)]
struct Choices;

/// A button of the error's.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Choice {
    Retry,
    Start,
    Quit,
}

/// What the screen says for `status`: the head line, the detail line, the
/// bar's fill (None: no bar) and whether it is an error (the choices
/// show).
pub fn lines(status: &Status, from: &str) -> (String, String, Option<f32>, bool) {
    match status {
        Status::Connecting => (
            "Connecting to the asset service...".into(),
            format!("{from}: checking which game files are needed"),
            Some(0.0),
            false,
        ),
        Status::Downloading { done, total, elapsed } => {
            let (head, detail) = asset_startup::progress_lines(*done, *total, *elapsed);
            let fill = if *total == 0 { 1.0 } else { (*done as f32 / *total as f32).clamp(0.0, 1.0) };
            (head, detail, Some(fill), false)
        }
        Status::Done(_) => ("Game files ready.".into(), "Starting the game...".into(), Some(1.0), false),
        Status::Failed(why) => ("Couldn't download the game files".into(), why.clone(), None, true),
    }
}

fn spawn_screen(mut commands: Commands, title: Option<Res<super::app::RunningApp>>, windows: Query<&Window>) {
    commands.spawn(Camera2d);
    let title = title
        .map(|t| t.0.title.to_string())
        .or_else(|| windows.iter().next().map(|w| w.title.clone()))
        .unwrap_or_default();
    let text = |s: &str, size: f32, color: Color| {
        (
            Text::new(s),
            TextFont { font_size: FontSize::Px(size), ..default() },
            TextColor(color),
            TextLayout::justify(Justify::Center),
        )
    };
    let button = |choice: Choice, label: &str| {
        (
            super::widgets::clickable(),
            choice,
            Node {
                padding: UiRect::axes(Val::Px(18.0), Val::Px(8.0)),
                margin: UiRect::horizontal(Val::Px(8.0)),
                border: UiRect::all(Val::Px(1.0)),
                ..default()
            },
            BorderColor::all(DIM),
            BackgroundColor(Color::srgb(0.14, 0.15, 0.17)),
            children![text(label, 20.0, TEXT)],
        )
    };
    commands.spawn((
        Node {
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            justify_content: JustifyContent::Center,
            row_gap: Val::Px(14.0),
            padding: UiRect::all(Val::Px(24.0)),
            ..default()
        },
        children![
            text(&title, 40.0, TEXT),
            (Head, text("", 26.0, TEXT), Node { max_width: Val::Px(760.0), ..default() }),
            (
                Bar,
                Node {
                    width: Val::Px(560.0),
                    height: Val::Px(18.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BorderColor::all(DIM),
                children![(
                    BarFill,
                    Node { width: Val::Percent(0.0), height: Val::Percent(100.0), ..default() },
                    BackgroundColor(ACCENT),
                )],
            ),
            (Detail, text("", 20.0, DIM), Node { max_width: Val::Px(760.0), ..default() }),
            (
                Note,
                text(
                    "The first start fetches the games' files once; later starts read them from this computer.",
                    16.0,
                    DIM,
                ),
                Node { max_width: Val::Px(760.0), margin: UiRect::top(Val::Px(18.0)), ..default() },
            ),
            (
                Choices,
                Node { display: Display::None, margin: UiRect::top(Val::Px(10.0)), ..default() },
                children![
                    button(Choice::Retry, "Try again (R)"),
                    button(Choice::Start, "Start anyway (S)"),
                    button(Choice::Quit, "Quit (Esc)"),
                ],
            ),
        ],
    ));
}

/// The fetch's status on screen; once done, the game started and this
/// app ended.
#[allow(clippy::type_complexity)]
fn show(
    mut screen: ResMut<Screen>,
    mut head: Query<(&mut Text, &mut TextColor), (With<Head>, Without<Detail>)>,
    mut detail: Query<&mut Text, (With<Detail>, Without<Head>)>,
    mut fill: Query<&mut Node, (With<BarFill>, Without<Bar>, Without<Choices>)>,
    mut bar: Query<&mut Node, (With<Bar>, Without<BarFill>, Without<Choices>)>,
    mut choices: Query<&mut Node, (With<Choices>, Without<Bar>, Without<BarFill>)>,
    mut exit: MessageWriter<AppExit>,
) {
    let status = screen.fetch.status();
    if screen.last.as_ref() != Some(&status) {
        let (h, d, f, failed) = lines(&status, &screen.fetch.from);
        for (mut t, mut c) in &mut head {
            t.0 = h.clone();
            c.0 = if failed { ERROR } else { TEXT };
        }
        for mut t in &mut detail {
            t.0 = d.clone();
        }
        for mut n in &mut fill {
            n.width = Val::Percent(100.0 * f.unwrap_or(0.0));
        }
        for mut n in &mut bar {
            n.display = if f.is_some() { Display::Flex } else { Display::None };
        }
        for mut n in &mut choices {
            n.display = if failed { Display::Flex } else { Display::None };
        }
        match &status {
            Status::Failed(why) => eprintln!("assets: couldn't download the game files: {why}"),
            Status::Done(summary) => {
                eprintln!("assets: {summary}");
                start(&screen.restart, &mut exit);
            }
            _ => {}
        }
        screen.last = Some(status);
    }
}

/// Start the game with `args` and end this app.
fn start(args: &[String], exit: &mut MessageWriter<AppExit>) {
    match super::relaunch::start_game(args) {
        Ok(id) => info!("starting the game (process {id})"),
        Err(e) => error!("couldn't start the game ({e}): start it again yourself"),
    }
    exit.write(AppExit::Success);
}

/// After an error: try again, start the game anyway, or quit.
fn choose(
    mut screen: ResMut<Screen>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    buttons: Query<(&Interaction, &Choice), Changed<Interaction>>,
    mut exit: MessageWriter<AppExit>,
    mut shown: Local<u32>,
) {
    if !matches!(screen.last, Some(Status::Failed(_))) {
        *shown = 0;
        return;
    }
    *shown += 1;
    if screen.tool_frames.is_some_and(|n| *shown >= n) {
        exit.write(AppExit::error());
        return;
    }
    let key = keys.and_then(|k| {
        if k.just_pressed(KeyCode::KeyR) || k.just_pressed(KeyCode::Enter) {
            Some(Choice::Retry)
        } else if k.just_pressed(KeyCode::KeyS) {
            Some(Choice::Start)
        } else if k.just_pressed(KeyCode::Escape) {
            Some(Choice::Quit)
        } else {
            None
        }
    });
    let clicked = buttons.iter().find(|(i, _)| **i == Interaction::Pressed).map(|(_, c)| *c);
    match key.or(clicked) {
        Some(Choice::Retry) => {
            screen.fetch = StartupFetch::start(screen.settings.clone(), screen.wanted.clone());
            screen.last = None;
        }
        Some(Choice::Start) => start(&screen.restart, &mut exit),
        Some(Choice::Quit) => {
            exit.write(AppExit::Success);
        }
        None => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_screen_says_what_is_going_on() {
        let (head, _, bar, failed) = lines(&Status::Connecting, "https://h:1");
        let (_, detail, _, _) = lines(&Status::Connecting, "https://h:1");
        assert!(head.contains("Connecting") && detail.contains("https://h:1") && bar == Some(0.0) && !failed);
        let (head, detail, bar, _) = lines(
            &Status::Downloading { done: 100_000_000, total: 400_000_000, elapsed: Duration::from_secs(20) },
            "",
        );
        assert_eq!(head, "Downloading game files: 100.0 of 400.0 MB");
        assert_eq!(detail, "5.0 MB/s, about 1m00s left");
        assert_eq!(bar, Some(0.25));
        let (head, detail, bar, failed) = lines(&Status::Failed("the asset service refused your token".into()), "");
        assert!(head.contains("Couldn't") && detail.contains("refused") && bar.is_none() && failed);
    }

    /// The screen on a headless app: an unreachable service's reason on
    /// it, the choices shown, and a tool run ending by itself.
    #[test]
    fn an_unreachable_service_is_shown_and_a_tool_run_ends() {
        use asset_startup::{AssetSettings, RemoteConfig, Store, Wanted};
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap();
        let dir = std::env::temp_dir().join(format!("chimera_startup_screen_{}", std::process::id()));
        let settings = AssetSettings {
            store: Some(Store::new(dir.clone())),
            use_install: true,
            builds: Default::default(),
            remote: Some(RemoteConfig {
                url: format!("https://{closed}"),
                token: "chimera1_test".into(),
                key: None,
                fingerprint: Some("ab".repeat(32)),
            }),
            lock: None,
        };
        let wanted = vec![Wanted { game: "g".into(), sets: vec!["base".into()] }];
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, ScreenPlugin)).insert_resource(Screen {
            fetch: StartupFetch::start(settings.clone(), wanted.clone()),
            settings,
            wanted,
            restart: Vec::new(),
            last: None,
            tool_frames: Some(3),
        });
        let start = std::time::Instant::now();
        while app.should_exit().is_none() {
            assert!(start.elapsed() < Duration::from_secs(30), "the tool run ended");
            app.update();
            std::thread::sleep(Duration::from_millis(5));
        }
        let world = app.world_mut();
        let head = world.query_filtered::<&Text, With<Head>>().single(world).unwrap().0.clone();
        let detail = world.query_filtered::<&Text, With<Detail>>().single(world).unwrap().0.clone();
        assert!(head.contains("Couldn't download"), "{head}");
        assert!(detail.contains("can't reach the asset service"), "{detail}");
        let choices = world.query_filtered::<&Node, With<Choices>>().single(world).unwrap().display;
        assert_eq!(choices, Display::Flex);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
