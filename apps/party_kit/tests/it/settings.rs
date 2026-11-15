//! Settings and the Escape menu in the Signal look (`party_kit::settings`,
//! `esc_signal`): with Signal's theme the Escape menu's keys move its
//! selection and press it, its Options open Settings over the match with
//! the options' pages (Mouse becoming Controls, with the binds); a change
//! is pending until Apply (which sets the cvars and saves the config),
//! Revert drops it, leaving with changes pending asks Apply / Discard /
//! Stay, Discard sets a previewed volume back, binds join the same model;
//! a bind takes the next key; an applied display-mode change goes back by
//! itself unless kept; the pointer focuses rows (and the Escape menu's
//! entries) until a key takes the lead, and again once it moves. Without the theme (Lucker Party, the plain apps)
//! nothing of it shows.

use bevy::prelude::*;
use chimera_core::console::{Console, ConsoleAppExt};
use chimera_ui::{Theme, UiClock, UiUnits};
use chimera_world::harness::Sim;
use party_kit::{
    esc_menu::{Action, EscMenu, EscMenuPlugin, Page},
    settings::{self, Context, Row, Settings, SettingsAction, SettingsPlugin},
};

use crate::controls::local_party_with;

const SIGNAL: &str = include_str!("../../../ultimate_gun_game/data/theme.toml");

#[derive(Resource, Default)]
struct Saves(u32);

/// Stand-ins for the client's cvars the sim lacks.
#[derive(Resource)]
struct Cvars(std::collections::BTreeMap<&'static str, String>);

/// The local party with the Escape menu and Settings; Signal's theme or
/// none; `host_writeconfig` counted; a stand-in `chimera_fullscreen`.
fn party(signal: bool) -> Sim {
    let mut sim = Sim::new(
        combat_arms::harness::CaSim,
        (chimera_world::greybox::GreyboxMapPlugin, party_kit::PartyPartsPlugin, EscMenuPlugin, SettingsPlugin),
    );
    crate::gun_game::plain_rungs(sim.app.world_mut());
    chimera_sim::minigame::start(sim.app.world_mut(), party_kit::DEFAULT_MINIGAME).expect("the gun game starts");
    let (mut sim, _) = local_party_with(sim);
    if signal {
        sim.app.insert_resource(Theme::from_toml(SIGNAL).unwrap());
    }
    sim.app
        .insert_resource(UiUnits::new(1080.0, 1.0))
        .insert_resource(UiClock::manual())
        .insert_resource(Cvars([("chimera_fullscreen", "0".to_string()), ("fov_desired", "90".to_string()), ("volume", "1".to_string())].into()))
        .init_resource::<Saves>()
        .console_command("host_writeconfig", "a test's: counted", |w, _| {
            w.resource_mut::<Saves>().0 += 1;
            Ok(None)
        })
        .console_cvar(
            "chimera_fullscreen",
            "a test's display mode",
            "0",
            |w| Some(w.resource::<Cvars>().0["chimera_fullscreen"].clone()),
            |w, v| {
                w.resource_mut::<Cvars>().0.insert("chimera_fullscreen", v.to_string());
                Ok(())
            },
        )
        .console_cvar(
            "fov_desired",
            "a test's field of view",
            "90",
            |w| Some(w.resource::<Cvars>().0["fov_desired"].clone()),
            |w, v| {
                w.resource_mut::<Cvars>().0.insert("fov_desired", v.to_string());
                Ok(())
            },
        )
        .console_cvar(
            "volume",
            "a test's game volume",
            "1",
            |w| Some(w.resource::<Cvars>().0["volume"].clone()),
            |w, v| {
                w.resource_mut::<Cvars>().0.insert("volume", v.to_string());
                Ok(())
            },
        );
    sim.ticks(2);
    sim
}

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    app.update();
}

/// `key` with Ctrl held.
fn ctrl(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::ControlLeft);
    tap(app, key);
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(KeyCode::ControlLeft);
    keys.clear();
    app.update();
}

fn saves(app: &App) -> u32 {
    app.world().resource::<Saves>().0
}

fn changes(app: &App) -> usize {
    app.world().resource::<Settings>().changes(app.world().resource::<Console>())
}

fn send(app: &mut App, a: SettingsAction) {
    app.world_mut().write_message(a);
    app.update();
    app.update();
}

fn cvar(app: &mut App, name: &str) -> String {
    party_kit::controls::cvar_value(app.world_mut(), name).unwrap_or_default()
}

fn set(app: &mut App) -> Settings {
    app.world().resource::<Settings>().clone()
}

fn rows(app: &App) -> Vec<Row> {
    let s = app.world().resource::<Settings>();
    settings::rows(s.ctx.unwrap(), &party_kit::esc_menu::shown_tabs(app.world()), s.tab, s.bind_game)
}

fn row_of(app: &App, cvar: &str) -> usize {
    rows(app).iter().position(|r| matches!(r, Row::Setting { s, .. } if s.cvar == cvar)).unwrap()
}

/// Esc, then ↓ and Enter on Options: Settings over the match, its pages
/// the options' with Mouse as Controls.
fn open_settings(app: &mut App) {
    tap(app, KeyCode::Escape);
    tap(app, KeyCode::ArrowDown);
    assert_eq!(app.world().resource::<EscMenu>().focus, 1, "↓ moves the selection");
    tap(app, KeyCode::Enter);
    app.update();
    assert_eq!(app.world().resource::<EscMenu>().page, Page::Options(0), "Enter presses Options");
    assert_eq!(set(app).ctx, Some(Context::Game));
}

#[test]
fn esc_menu_keys_open_settings_with_the_options_pages() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    let tabs = party_kit::esc_menu::shown_tabs(sim.app.world());
    assert_eq!(settings::pages(Context::Game, &tabs), ["Video", "Audio", "Controls", "Party"]);
    let roots = sim.app.world_mut().query_filtered::<(), With<settings::SettingsRoot>>().iter(sim.app.world()).count();
    assert_eq!(roots, 1, "drawn");
    // Q/E change the page; Controls has the mouse rows and the binds.
    tap(&mut sim.app, KeyCode::KeyE);
    tap(&mut sim.app, KeyCode::KeyE);
    assert_eq!(set(&mut sim.app).tab, 2);
    let r = rows(&sim.app);
    assert!(r.iter().any(|r| matches!(r, Row::Setting { s, .. } if s.cvar == "sensitivity")));
    assert!(r.iter().any(|r| matches!(r, Row::Bind { command: "+jump", .. })));
    // Esc goes back to the Escape menu's first page.
    tap(&mut sim.app, KeyCode::Escape);
    assert_eq!(sim.app.world().resource::<EscMenu>().page, Page::Main);
    assert_eq!(set(&mut sim.app).ctx, None);
}

#[test]
fn a_change_is_pending_until_apply_writes_it() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    let row = row_of(&sim.app, "fov_desired");
    let before = cvar(&mut sim.app, "fov_desired");
    let saves0 = saves(&sim.app);
    send(&mut sim.app, SettingsAction::Step { row, dir: -1 });
    assert_eq!(cvar(&mut sim.app, "fov_desired"), before, "pending: the cvar waits");
    let shown = set(&mut sim.app).values["fov_desired"].clone();
    assert_ne!(shown, before, "the row shows the new value");
    assert_eq!(set(&mut sim.app).pending.get("fov_desired"), Some(&shown), "marked changed");
    assert_eq!(changes(&sim.app), 1, "the bar counts it");
    // A second step builds on the pending value, not the cvar.
    send(&mut sim.app, SettingsAction::Step { row, dir: -1 });
    let shown2 = set(&mut sim.app).values["fov_desired"].clone();
    assert_ne!(shown2, shown);
    // Ctrl+S applies: the cvar set, the config written.
    ctrl(&mut sim.app, KeyCode::KeyS);
    assert_eq!(cvar(&mut sim.app, "fov_desired"), shown2, "Apply sets the cvar");
    assert_eq!(saves(&sim.app), saves0 + 1, "Apply writes the config");
    assert_eq!(changes(&sim.app), 0);
    // Nothing pending: Esc leaves at once, writing nothing.
    tap(&mut sim.app, KeyCode::Escape);
    sim.app.update();
    assert_eq!(set(&mut sim.app).ctx, None);
    assert_eq!(saves(&sim.app), saves0 + 1);
}

#[test]
fn revert_drops_the_pending_changes() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    let row = row_of(&sim.app, "fov_desired");
    let before = cvar(&mut sim.app, "fov_desired");
    send(&mut sim.app, SettingsAction::Step { row, dir: -1 });
    assert_eq!(changes(&sim.app), 1);
    ctrl(&mut sim.app, KeyCode::KeyZ);
    assert_eq!(changes(&sim.app), 0, "Ctrl+Z drops it");
    assert_eq!(set(&mut sim.app).values["fov_desired"], before, "the row shows the cvar again");
    assert_eq!(cvar(&mut sim.app, "fov_desired"), before);
    // Stepping back to the applied value is no change either.
    send(&mut sim.app, SettingsAction::Step { row, dir: -1 });
    send(&mut sim.app, SettingsAction::Step { row, dir: 1 });
    assert_eq!(changes(&sim.app), 0);
}

#[test]
fn leaving_with_changes_pending_asks() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    let row = row_of(&sim.app, "fov_desired");
    send(&mut sim.app, SettingsAction::Step { row, dir: -1 });
    let next = set(&mut sim.app).values["fov_desired"].clone();
    let saves0 = saves(&sim.app);
    // Esc asks instead of leaving.
    tap(&mut sim.app, KeyCode::Escape);
    sim.app.update();
    assert_eq!(set(&mut sim.app).ctx, Some(Context::Game), "still open");
    assert_eq!(set(&mut sim.app).leave, Some(0), "the prompt, Apply focused");
    assert_eq!(sim.app.world().resource::<EscMenu>().page, Page::Options(0), "the Escape menu left Esc alone");
    // Esc again stays.
    tap(&mut sim.app, KeyCode::Escape);
    sim.app.update();
    assert_eq!(set(&mut sim.app).leave, None, "Stay");
    assert_eq!(set(&mut sim.app).ctx, Some(Context::Game));
    assert_eq!(changes(&sim.app), 1, "still pending");
    // The Back button asks too; Enter on Apply applies and leaves.
    send(&mut sim.app, SettingsAction::Back);
    assert_eq!(set(&mut sim.app).leave, Some(0));
    tap(&mut sim.app, KeyCode::Enter);
    sim.app.update();
    assert_eq!(cvar(&mut sim.app, "fov_desired"), next, "applied");
    assert_eq!(saves(&sim.app), saves0 + 1, "saved");
    assert_eq!(set(&mut sim.app).ctx, None, "and left");
    assert_eq!(sim.app.world().resource::<EscMenu>().page, Page::Main);
}

#[test]
fn discard_sets_a_previewed_volume_back() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    send(&mut sim.app, SettingsAction::Tab(1));
    let row = row_of(&sim.app, "volume");
    let fov = row_of_tab(&mut sim.app, 0, "fov_desired");
    let saves0 = saves(&sim.app);
    // A drag along the track: heard at once.
    // (0 to 200 %: a quarter of the track is half volume.)
    send(&mut sim.app, SettingsAction::SetFrac { row, frac: 0.25 });
    assert_eq!(cvar(&mut sim.app, "volume"), "0.5", "a volume previews live");
    assert_eq!(changes(&sim.app), 1, "and is pending");
    // A setting that doesn't preview waits.
    send(&mut sim.app, SettingsAction::Tab(0));
    let fov_before = cvar(&mut sim.app, "fov_desired");
    send(&mut sim.app, SettingsAction::Step { row: fov, dir: -1 });
    assert_eq!(cvar(&mut sim.app, "fov_desired"), fov_before);
    assert_eq!(changes(&sim.app), 2, "pending across pages");
    // Leave: the prompt; → Discard, Enter.
    tap(&mut sim.app, KeyCode::Escape);
    tap(&mut sim.app, KeyCode::ArrowRight);
    assert_eq!(set(&mut sim.app).leave, Some(1));
    tap(&mut sim.app, KeyCode::Enter);
    sim.app.update();
    assert_eq!(cvar(&mut sim.app, "volume"), "1", "Discard sets the preview back");
    assert_eq!(cvar(&mut sim.app, "fov_desired"), fov_before);
    assert_eq!(saves(&sim.app), saves0, "nothing saved");
    assert_eq!(set(&mut sim.app).ctx, None, "left");
}

/// The row of `cvar` on page `tab`.
fn row_of_tab(app: &mut App, tab: usize, cvar: &str) -> usize {
    let tabs = party_kit::esc_menu::shown_tabs(app.world());
    settings::rows(Context::Game, &tabs, tab, 0).iter().position(|r| matches!(r, Row::Setting { s, .. } if s.cvar == cvar)).unwrap()
}

#[test]
fn binds_join_the_pending_changes() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    send(&mut sim.app, SettingsAction::Tab(2));
    let row = rows(&sim.app).iter().position(|r| matches!(r, Row::Bind { command: "+use", .. })).unwrap();
    let keys = |app: &App| party_kit::controls::keys_on(app.world().resource::<Console>(), 0, "+use");
    let before = keys(&sim.app);
    assert_ne!(before, ["g"]);
    let saves0 = saves(&sim.app);
    send(&mut sim.app, SettingsAction::Bind(row));
    tap(&mut sim.app, KeyCode::KeyG);
    sim.app.update();
    assert_eq!(keys(&sim.app), ["g"], "the page shows the new key");
    assert!(changes(&sim.app) > 0, "pending");
    assert_eq!(saves(&sim.app), saves0, "not saved");
    // Revert puts the keys back.
    send(&mut sim.app, SettingsAction::Revert);
    assert_eq!(keys(&sim.app), before, "Revert");
    assert_eq!(changes(&sim.app), 0);
    // Rebind and leave: the prompt; Discard puts them back.
    send(&mut sim.app, SettingsAction::Bind(row));
    tap(&mut sim.app, KeyCode::KeyG);
    sim.app.update();
    tap(&mut sim.app, KeyCode::Escape);
    assert_eq!(set(&mut sim.app).leave, Some(0), "leaving asks");
    send(&mut sim.app, SettingsAction::LeaveDiscard);
    assert_eq!(keys(&sim.app), before, "Discard");
    assert_eq!(set(&mut sim.app).ctx, None);
    // Again, applied this time: kept and saved.
    tap(&mut sim.app, KeyCode::Enter);
    sim.app.update();
    assert_eq!(set(&mut sim.app).ctx, Some(Context::Game), "Options again");
    send(&mut sim.app, SettingsAction::Tab(2));
    send(&mut sim.app, SettingsAction::Bind(row));
    tap(&mut sim.app, KeyCode::KeyG);
    sim.app.update();
    send(&mut sim.app, SettingsAction::Apply);
    assert_eq!(keys(&sim.app), ["g"], "kept");
    assert_eq!(saves(&sim.app), saves0 + 1, "saved");
    assert_eq!(changes(&sim.app), 0);
}

#[test]
fn an_applied_display_mode_goes_back_unless_kept() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    let row = row_of(&sim.app, "chimera_fullscreen");
    let saves0 = saves(&sim.app);
    send(&mut sim.app, SettingsAction::Pick { row, option: 2 });
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "0", "pending");
    assert!(set(&mut sim.app).keep.is_none(), "nothing to keep before Apply");
    send(&mut sim.app, SettingsAction::Apply);
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "2", "applied");
    assert!(set(&mut sim.app).keep.is_some(), "asks to be kept");
    sim.app.world_mut().resource_mut::<UiClock>().step(16.0);
    sim.app.update();
    sim.app.update();
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "0", "15 s on: back by itself");
    assert!(set(&mut sim.app).keep.is_none());
    assert_eq!(saves(&sim.app), saves0 + 2, "the config written again with the old mode");
    send(&mut sim.app, SettingsAction::Pick { row, option: 1 });
    send(&mut sim.app, SettingsAction::Apply);
    send(&mut sim.app, SettingsAction::Keep);
    sim.app.world_mut().resource_mut::<UiClock>().step(16.0);
    sim.app.update();
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "1", "kept");
    // Applied from the leave prompt: the keep first, Keep then leaves.
    send(&mut sim.app, SettingsAction::Pick { row, option: 2 });
    send(&mut sim.app, SettingsAction::Back);
    send(&mut sim.app, SettingsAction::LeaveApply);
    assert!(set(&mut sim.app).keep.is_some());
    assert_eq!(set(&mut sim.app).ctx, Some(Context::Game), "stays for the keep");
    send(&mut sim.app, SettingsAction::Keep);
    assert_eq!(set(&mut sim.app).ctx, None, "Keep leaves");
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "2");
}

#[test]
fn a_bind_row_takes_the_next_key() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    send(&mut sim.app, SettingsAction::Tab(2));
    let row = rows(&sim.app).iter().position(|r| matches!(r, Row::Bind { command: "+use", .. })).unwrap();
    send(&mut sim.app, SettingsAction::Bind(row));
    assert_eq!(set(&mut sim.app).capture, Some(row));
    tap(&mut sim.app, KeyCode::KeyG);
    assert_eq!(set(&mut sim.app).capture, None);
    let keys = party_kit::controls::keys_on(sim.app.world().resource::<Console>(), 0, "+use");
    assert_eq!(keys, ["g"]);
    // Esc while listening cancels and leaves the page open.
    send(&mut sim.app, SettingsAction::Bind(row));
    tap(&mut sim.app, KeyCode::Escape);
    assert_eq!(set(&mut sim.app).capture, None);
    assert_eq!(set(&mut sim.app).ctx, Some(Context::Game), "Esc was the capture's");
}

#[test]
fn without_the_theme_nothing_of_it_shows() {
    let mut sim = party(false);
    tap(&mut sim.app, KeyCode::Escape);
    sim.app.world_mut().write_message(Action::Options);
    sim.app.update();
    sim.app.update();
    assert_eq!(sim.app.world().resource::<EscMenu>().page, Page::Options(0));
    assert_eq!(set(&mut sim.app).ctx, None, "the plain look keeps its own Options");
    // ↓ moves no selection without the Signal look.
    tap(&mut sim.app, KeyCode::Escape);
    tap(&mut sim.app, KeyCode::ArrowDown);
    assert_eq!(sim.app.world().resource::<EscMenu>().focus, 0);
}

/// The pointer over the node carrying `which` (the focus system sets
/// this each frame in the app; headless the test does), none elsewhere.
fn hover_on<A: Component + PartialEq>(app: &mut App, which: &A) {
    let all: Vec<(Entity, bool)> =
        app.world_mut().query::<(Entity, &A)>().iter(app.world()).map(|(e, a)| (e, a == which)).collect();
    for (e, on) in all {
        let want = if on { Interaction::Hovered } else { Interaction::None };
        let mut w = app.world_mut().entity_mut(e);
        if w.get::<Interaction>() != Some(&want) {
            w.insert(want);
        }
    }
    app.update();
}

/// A real pointer move: the window's cursor somewhere new.
fn move_pointer(app: &mut App, to: Vec2) {
    let w = app.world_mut();
    if w.query::<&Window>().iter(w).next().is_none() {
        w.spawn(Window::default());
    }
    let mut q = w.query::<&mut Window>();
    q.iter_mut(w).next().unwrap().set_cursor_position(Some(to));
    app.update();
}

#[test]
fn keys_take_the_focus_from_a_still_pointer_until_it_moves() {
    let mut sim = party(true);
    let app = &mut sim.app;
    move_pointer(app, Vec2::new(400.0, 300.0));
    open_settings(app);
    let live: Vec<usize> = rows(app).iter().enumerate().filter(|(_, r)| r.focusable()).map(|(i, _)| i).collect();
    assert!(live.len() >= 5, "enough rows: {live:?}");
    let (on, two_down) = (live[2], live[4]);
    // The pointer brought onto the third row (a move, then its hover).
    move_pointer(app, Vec2::new(400.0, 320.0));
    hover_on(app, &SettingsAction::Row(on));
    assert_eq!(set(app).focus, on, "the pointer focuses the row under it");
    tap(app, KeyCode::ArrowDown);
    tap(app, KeyCode::ArrowDown);
    assert_eq!(set(app).focus, two_down, "↓ twice moves on from it");
    // The pointer is still; its row's hover again (a row redrawn under
    // it) doesn't take the focus back.
    hover_on(app, &SettingsAction::Row(two_down));
    hover_on(app, &SettingsAction::Row(on));
    for _ in 0..3 {
        app.update();
    }
    assert_eq!(set(app).focus, two_down, "a still pointer doesn't pin the focus");
    // A real move hands it back to the row under the pointer.
    move_pointer(app, Vec2::new(402.0, 321.0));
    assert_eq!(set(app).focus, on, "the moved pointer focuses its row");
}

#[test]
fn the_pointer_moves_the_esc_menus_selection_and_keys_go_on_from_it() {
    let mut sim = party(true);
    let app = &mut sim.app;
    tap(app, KeyCode::Escape);
    move_pointer(app, Vec2::new(300.0, 300.0));
    hover_on(app, &Action::Options);
    assert_eq!(app.world().resource::<EscMenu>().focus, 1, "Options under the pointer is the selection");
    tap(app, KeyCode::ArrowDown);
    let after = app.world().resource::<EscMenu>().focus;
    assert!(after > 1, "↓ goes on from Options: {after}");
    hover_on(app, &Action::Resume);
    hover_on(app, &Action::Options);
    assert_eq!(app.world().resource::<EscMenu>().focus, after, "a still pointer doesn't pin it");
}


/// The rows sit in the kit's scroll container; ↓ marks the focused row to
/// be scrolled into view (`chimera_ui::ScrollIntoView`), the pointer
/// moving the focus doesn't, and a page change starts at the top.
#[test]
fn the_keys_scroll_the_focused_row_into_view() {
    let mut sim = party(true);
    sim.app.init_resource::<chimera_ui::ScrollViews>();
    open_settings(&mut sim.app);
    tap(&mut sim.app, KeyCode::KeyE);
    tap(&mut sim.app, KeyCode::KeyE);
    let w = sim.app.world_mut();
    let views: Vec<String> = w.query::<&chimera_ui::scroll::ScrollView>().iter(w).map(|v| v.key.clone()).collect();
    assert_eq!(views, [settings::SCROLL_KEY], "Controls' rows in one scroll view");
    for _ in 0..12 {
        tap(&mut sim.app, KeyCode::ArrowDown);
    }
    let s = set(&mut sim.app);
    assert!(s.reveal, "the keys moved the focus");
    let w = sim.app.world_mut();
    let marked: Vec<SettingsAction> =
        w.query_filtered::<&SettingsAction, With<chimera_ui::ScrollIntoView>>().iter(w).copied().collect();
    assert_eq!(marked, [SettingsAction::Row(s.focus)], "the focused row is scrolled to");
    // The pointer on another row, moved (`PointerLead` gives it back):
    // focus moves, nothing scrolls.
    move_pointer(&mut sim.app, Vec2::new(400.0, 300.0));
    let w = sim.app.world_mut();
    let (other, to) = w
        .query::<(Entity, &SettingsAction)>()
        .iter(w)
        .find_map(|(e, a)| match a {
            SettingsAction::Row(r) if *r != s.focus => Some((e, *r)),
            _ => None,
        })
        .expect("another row");
    w.entity_mut(other).insert(bevy::ui::DeprecatedInteraction::Hovered);
    sim.app.update();
    sim.app.update();
    assert_eq!(set(&mut sim.app).focus, to, "the pointer moved the focus");
    assert!(!set(&mut sim.app).reveal);
    let w = sim.app.world_mut();
    assert_eq!(w.query_filtered::<(), With<chimera_ui::ScrollIntoView>>().iter(w).count(), 0, "and didn't scroll");
    // A page change: the view back at the top.
    sim.app.world_mut().resource_mut::<chimera_ui::ScrollViews>().state(settings::SCROLL_KEY).offset = 9.0;
    tap(&mut sim.app, KeyCode::KeyQ);
    assert_eq!(sim.app.world().resource::<chimera_ui::ScrollViews>().get(settings::SCROLL_KEY).unwrap().offset, 0.0);
}

/// Counts the slider's cues (`ui_slider`).
#[derive(Resource, Default)]
struct SliderCues(usize);

fn count_slider_cues(mut cues: MessageReader<chimera_ui::UiCue>, mut n: ResMut<SliderCues>) {
    n.0 += cues.read().filter(|c| c.event == "ui_slider").count();
}

fn slider_cues(app: &App) -> usize {
    app.world().resource::<SliderCues>().0
}

/// Settings on the Audio page, counting slider cues, the left button held.
fn audio_page() -> (Sim, usize) {
    let mut sim = party(true);
    sim.app.init_resource::<SliderCues>().add_systems(Update, count_slider_cues.after(settings::SettingsSystems));
    open_settings(&mut sim.app);
    send(&mut sim.app, SettingsAction::Tab(1));
    let row = row_of(&sim.app, "volume");
    (sim, row)
}

/// The volume slider's track as now drawn, the pointer `frac` along it.
fn point_at(app: &mut App, row: usize, frac: f32) -> Entity {
    let w = app.world_mut();
    let track = w
        .query::<(Entity, &SettingsAction)>()
        .iter(w)
        .find(|(_, a)| **a == SettingsAction::Slide(row))
        .map(|(e, _)| e)
        .expect("the slider's track");
    w.entity_mut(track).insert(bevy::ui::RelativeCursorPosition { cursor_over: true, normalized: Some(Vec2::new(frac - 0.5, 0.0)) });
    track
}

fn mouse(app: &mut App) -> Mut<'_, ButtonInput<MouseButton>> {
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>()
}

/// A press on the track jumps there and drags, the page drawn again under
/// the pointer; the value shows and previews while dragging and becomes
/// pending once, at the release, with one cue.
#[test]
fn a_slider_drags_and_its_release_commits_once() {
    let (mut sim, row) = audio_page();
    let app = &mut sim.app;
    // The volume's track is 0 to 2 (200 %): a fraction is twice its value.
    let track = point_at(app, row, 0.1);
    mouse(app).press(MouseButton::Left);
    app.world_mut().entity_mut(track).insert(bevy::ui::DeprecatedInteraction::Pressed);
    app.update();
    mouse(app).clear();
    app.update();
    assert_eq!(set(app).values["volume"], "0.2", "jumped to the press");
    assert!(set(app).slide.is_some(), "dragging");
    for f in [0.15, 0.225, 0.3, 0.35] {
        // A new track each time: the page is drawn again as the value moves.
        point_at(app, row, f);
        app.update();
    }
    assert_eq!(set(app).values["volume"], "0.7", "followed the pointer");
    app.update();
    assert_eq!(cvar(app, "volume"), "0.7", "a volume previews while dragged");
    assert!(set(app).pending.is_empty(), "nothing pending mid-drag");
    assert_eq!(slider_cues(app), 0, "no ticks while dragging");
    mouse(app).release(MouseButton::Left);
    app.update();
    app.update();
    assert!(set(app).slide.is_none());
    assert_eq!(set(app).pending.get("volume").map(String::as_str), Some("0.7"), "pending at the release");
    assert_eq!(changes(app), 1);
    assert_eq!(slider_cues(app), 1, "one cue, at the release");
    // Revert puts the previewed volume back.
    ctrl(app, KeyCode::KeyZ);
    assert_eq!(cvar(app, "volume"), "1");
}

/// A click without a drag: one cue; ←→: a cue each step.
#[test]
fn a_click_cues_once_and_keys_cue_each_step() {
    let (mut sim, row) = audio_page();
    let app = &mut sim.app;
    send(app, SettingsAction::SlideStart { row, frac: 0.2 });
    send(app, SettingsAction::SlideEnd(row));
    assert_eq!(set(app).pending.get("volume").map(String::as_str), Some("0.4"));
    assert_eq!(slider_cues(app), 1, "a click-jump: once");
    send(app, SettingsAction::Row(row));
    tap(app, KeyCode::ArrowRight);
    tap(app, KeyCode::ArrowRight);
    assert_eq!(slider_cues(app), 3, "each key step");
}

/// Clicking the value box types a number (a volume in percent, 160 for
/// 1.6, the box opened with it all selected); a negative volume or one
/// past 200 % refused with why, Esc puts the value back.
#[test]
fn a_typed_value_is_kept_past_the_range_and_checked() {
    let (mut sim, row) = audio_page();
    let app = &mut sim.app;
    send(app, SettingsAction::EditValue(row));
    assert_eq!(set(app).edit.as_ref().map(|e| e.text.as_str()), Some("100"), "the value to edit, in percent");
    assert_eq!(set(app).edit.as_ref().map(|e| (e.anchor, e.caret)), Some((0, 3)), "all of it selected");
    app.world_mut().resource_mut::<Console>().submit("settings_type 160");
    app.update();
    tap(app, KeyCode::Enter);
    assert!(set(app).edit.is_none(), "Enter keeps it");
    assert_eq!(set(app).pending.get("volume").map(String::as_str), Some("1.6"), "160 % is 1.6");
    // Refused: past 200 %, a negative volume, and not a number.
    send(app, SettingsAction::EditValue(row));
    app.world_mut().resource_mut::<Console>().submit("settings_type 250%");
    app.update();
    tap(app, KeyCode::Enter);
    assert_eq!(set(app).edit.and_then(|e| e.error).as_deref(), Some("At most 200"));
    app.world_mut().resource_mut::<Console>().submit("settings_type -50");
    app.update();
    tap(app, KeyCode::Enter);
    let e = set(app).edit.expect("still typing");
    assert_eq!(e.error.as_deref(), Some("At least 0"), "why, beside the box");
    let texts: Vec<String> = app.world_mut().query::<&Text>().iter(app.world()).map(|t| t.0.clone()).collect();
    assert!(texts.iter().any(|t| t == "AT LEAST 0"), "shown: {texts:?}");
    app.world_mut().resource_mut::<Console>().submit("settings_type abc");
    app.update();
    tap(app, KeyCode::Enter);
    assert_eq!(set(app).edit.and_then(|e| e.error).as_deref(), Some("Not a number"));
    // Esc: the value as it was, the page still up.
    tap(app, KeyCode::Escape);
    app.update();
    assert!(set(app).edit.is_none());
    assert_eq!(set(app).ctx, Some(Context::Game), "Esc only closed the box");
    assert_eq!(set(app).pending.get("volume").map(String::as_str), Some("1.6"));
}

/// The volumes step in 5 %: ←→ moves one, a press on the track lands on
/// the 5 % grid, the help pane says so in percent (never the raw 0.05).
#[test]
fn volumes_step_in_five_percent() {
    let (mut sim, row) = audio_page();
    let app = &mut sim.app;
    send(app, SettingsAction::Row(row));
    tap(app, KeyCode::ArrowRight);
    assert_eq!(set(app).pending.get("volume").map(String::as_str), Some("1.05"), "→: 100 % to 105 %");
    tap(app, KeyCode::ArrowLeft);
    tap(app, KeyCode::ArrowLeft);
    assert_eq!(set(app).pending.get("volume").map(String::as_str), Some("0.95"), "←←: 95 %");
    // A press between the grid's marks lands on one (0 to 200 %).
    send(app, SettingsAction::SlideStart { row, frac: 0.513 });
    send(app, SettingsAction::SlideEnd(row));
    assert_eq!(set(app).pending.get("volume").map(String::as_str), Some("1.05"), "51.3 % of the track: 102.6 % lands on 105 %");
    let tabs = party_kit::esc_menu::shown_tabs(app.world());
    for cvar in ["volume", "snd_musicvolume", "snd_uivolume"] {
        let Some(s) = tabs.iter().flat_map(|(_, s)| s.iter()).find(|s| s.cvar == cvar) else { continue };
        assert!(matches!(s.pick, party_kit::host::Pick::Range { step, .. } if (step - 0.05).abs() < 1e-6), "{cvar}: 5 % steps");
        assert_eq!(settings::range_text(s), "0% – 200%, steps of 5%", "{cvar}");
    }
}

/// Counts the hover cues.
#[derive(Resource, Default)]
struct HoverCues(usize);

fn count_hover_cues(mut cues: MessageReader<chimera_ui::UiCue>, mut n: ResMut<HoverCues>) {
    n.0 += cues.read().filter(|c| c.event == "ui_hover").count();
}

fn hover_cues(app: &App) -> usize {
    app.world().resource::<HoverCues>().0
}

fn later(app: &mut App, secs: f32) {
    app.world_mut().resource_mut::<UiClock>().step(secs);
}

/// The hover cue belongs to the row, not its parts: across the field of
/// view row's label, slider, value box and background it plays once; a
/// fast sweep over ten rows plays a few (90 ms apart at least); none
/// while the button is held (a drag); ↑↓ cue each step.
#[test]
fn hover_cues_once_per_row_spaced_and_never_while_dragging() {
    let mut sim = party(true);
    sim.app.init_resource::<HoverCues>().add_systems(Update, count_hover_cues.after(settings::SettingsSystems));
    let app = &mut sim.app;
    open_settings(app);
    let fov = row_of(app, "fov_desired");
    later(app, 1.0);
    app.update();
    let n0 = hover_cues(app);
    for a in [
        SettingsAction::Row(fov),
        SettingsAction::Slide(fov),
        SettingsAction::Row(fov),
        SettingsAction::EditValue(fov),
        SettingsAction::Row(fov),
        SettingsAction::Slide(fov),
    ] {
        hover_on(app, &a);
        later(app, 0.2);
    }
    assert_eq!(hover_cues(app) - n0, 1, "one row: one cue");
    // Another row, then back: each a new item.
    let other = rows(app).iter().position(|r| matches!(r, Row::Setting { s, .. } if s.cvar != "fov_desired")).unwrap();
    hover_on(app, &SettingsAction::Row(other));
    later(app, 0.2);
    hover_on(app, &SettingsAction::Slide(fov));
    assert_eq!(hover_cues(app) - n0, 3, "a different row cues, and coming back does");

    // A fast sweep down Controls' rows, 30 ms each.
    send(app, SettingsAction::Tab(2));
    later(app, 1.0);
    let live: Vec<usize> = rows(app).iter().enumerate().filter(|(_, r)| r.focusable()).map(|(i, _)| i).take(10).collect();
    assert_eq!(live.len(), 10);
    let n1 = hover_cues(app);
    for r in &live {
        hover_on(app, &SettingsAction::Row(*r));
        later(app, 0.03);
    }
    let swept = hover_cues(app) - n1;
    assert!((1..=4).contains(&swept), "ten rows in 0.3 s: a few cues, not ten ({swept})");

    // Held: none.
    later(app, 1.0);
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    let n2 = hover_cues(app);
    for r in &live[..4] {
        hover_on(app, &SettingsAction::Row(*r));
        later(app, 0.2);
    }
    assert_eq!(hover_cues(app), n2, "no hover cue while the button is held");
    app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
    app.update();

    // The keys: each step, gap or not.
    let n3 = hover_cues(app);
    for _ in 0..3 {
        tap(app, KeyCode::ArrowDown);
    }
    assert_eq!(hover_cues(app) - n3, 3, "↓ three times: three cues");
}

/// A toggle flipped slides its slab (the row and the time kept for the
/// drawing).
#[test]
fn a_flipped_toggle_slides() {
    let mut sim = party(true);
    let app = &mut sim.app;
    open_settings(app);
    send(app, SettingsAction::Tab(3));
    let row = row_of(app, "party_swap_effect");
    send(app, SettingsAction::Step { row, dir: 1 });
    assert_eq!(set(app).flip.map(|(r, _)| r), Some(row));
    let w = app.world_mut();
    let sliding = w.query_filtered::<(), (With<chimera_ui::controls::ToggleSlab>, With<chimera_ui::Timeline>)>().iter(w).count();
    assert_eq!(sliding, 1, "its slab slides");
}
