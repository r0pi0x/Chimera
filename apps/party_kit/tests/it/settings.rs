//! Settings and the Escape menu in the Signal look (`party_kit::settings`,
//! `esc_signal`): with Signal's theme the Escape menu's keys move its
//! selection and press it, its Options open Settings over the match with
//! the options' pages (Mouse becoming Controls, with the binds), a step
//! sets the cvar at once and the bar counts it, Revert puts it back,
//! Apply saves the config, leaving applies; a bind takes the next key; a
//! display-mode change goes back by itself unless kept. Without the
//! theme (Lucker Party, the plain apps) nothing of it shows.

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
        .insert_resource(Cvars([("chimera_fullscreen", "0".to_string()), ("fov_desired", "90".to_string())].into()))
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
fn a_step_applies_at_once_revert_puts_it_back_apply_saves() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    let row = row_of(&sim.app, "fov_desired");
    let before = cvar(&mut sim.app, "fov_desired");
    send(&mut sim.app, SettingsAction::Step { row, dir: -1 });
    let after = cvar(&mut sim.app, "fov_desired");
    assert_ne!(before, after, "set at once");
    assert_eq!(set(&mut sim.app).baseline.len(), 1, "the bar counts it");
    send(&mut sim.app, SettingsAction::Revert);
    assert_eq!(cvar(&mut sim.app, "fov_desired"), before, "Revert puts it back");
    assert!(set(&mut sim.app).baseline.is_empty());
    send(&mut sim.app, SettingsAction::Step { row, dir: -1 });
    let saves = sim.app.world().resource::<Saves>().0;
    send(&mut sim.app, SettingsAction::Apply);
    assert_eq!(sim.app.world().resource::<Saves>().0, saves + 1, "Apply writes the config");
    assert!(set(&mut sim.app).baseline.is_empty());
    // A change left unapplied is saved on leaving.
    send(&mut sim.app, SettingsAction::Step { row, dir: 1 });
    tap(&mut sim.app, KeyCode::Escape);
    sim.app.update();
    assert_eq!(sim.app.world().resource::<Saves>().0, saves + 2, "leaving applies");
}

#[test]
fn a_display_mode_goes_back_unless_kept() {
    let mut sim = party(true);
    open_settings(&mut sim.app);
    let row = row_of(&sim.app, "chimera_fullscreen");
    send(&mut sim.app, SettingsAction::Pick { row, option: 2 });
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "2");
    assert!(set(&mut sim.app).keep.is_some(), "asks to be kept");
    sim.app.world_mut().resource_mut::<UiClock>().step(16.0);
    sim.app.update();
    sim.app.update();
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "0", "15 s on: back by itself");
    assert!(set(&mut sim.app).keep.is_none());
    send(&mut sim.app, SettingsAction::Pick { row, option: 1 });
    send(&mut sim.app, SettingsAction::Keep);
    sim.app.world_mut().resource_mut::<UiClock>().step(16.0);
    sim.app.update();
    assert_eq!(cvar(&mut sim.app, "chimera_fullscreen"), "1", "kept");
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
