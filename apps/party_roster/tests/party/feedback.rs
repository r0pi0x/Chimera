//! The report dialog (`chimera_client::client::feedback`, drawn by
//! `party_roster::feedback_signal` under Signal's theme): the report key
//! takes the screenshot first and opens the dialog after it; typing goes
//! into the report (the game's keys reset), Tab switches the category,
//! Ctrl+Enter sends through the outbox, Esc cancels; the clipboard's
//! keys (paste, copy, cut, select all) work in the text; the Escape menu's
//! "Report a bug" closes the menu before the capture; the key is a row of
//! Settings > Controls (Shared) that rebinds.

use std::sync::{Arc, Mutex};

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use chimera_client::client::feedback::{
    Category, Feedback, FeedbackCapture, Outbox, Phase, Report, Submitted, captured, fake_frame,
};
use chimera_core::console::Console;
use chimera_ui::{Theme, UiClock, UiUnits};
use chimera_world::harness::Sim;
use party_roster::{
    esc_menu::{Action, EscMenu, EscMenuPlugin},
    feedback_signal::FeedbackSignalPlugin,
    settings::{self, Row, Settings, SettingsAction, SettingsPlugin},
};

use crate::local::local_party_with;

const SIGNAL: &str = include_str!("../../../ultimate_gun_game/data/theme.toml");

/// What the fake outbox got.
#[derive(Clone, Default)]
struct Got(Arc<Mutex<Vec<Report>>>);

impl Outbox for Got {
    fn destination(&self) -> String {
        "to the test's report service".into()
    }

    fn submit(&self, report: Report) -> Result<Submitted, String> {
        self.0.lock().unwrap().push(report);
        Ok(Submitted::Sent { id: "t".into() })
    }
}

/// The phase the capture found (it must come before the dialog).
#[derive(Resource, Default)]
struct Captures(Vec<Phase>);

fn capture(w: &mut World) {
    let phase = w.resource::<Feedback>().phase;
    let esc_open = w.resource::<EscMenu>().open;
    assert!(!esc_open, "the Escape menu is gone before the capture");
    // Nothing drawn reacts to the report before its frame is taken: the
    // lobby hid its whole panel for an overlay from the key press, so a
    // report from the lobby was a black frame (report 3025fbb1).
    let overlaid = w.resource::<chimera_client::client::prematch_lobby::LobbyOverlay>().0;
    assert!(!overlaid, "the lobby is drawn as it was in the frame the report takes");
    w.resource_mut::<Captures>().0.push(phase);
    captured(w, fake_frame());
}

fn party() -> (Sim, Got) {
    let mut sim = Sim::new(
        combat_arms::harness::CaSim,
        (chimera_world::greybox::GreyboxMapPlugin, party_roster::PartyPartsPlugin, EscMenuPlugin, SettingsPlugin, FeedbackSignalPlugin),
    );
    crate::play::plain_rungs(sim.app.world_mut());
    chimera_sim::minigame::start(sim.app.world_mut(), party_roster::DEFAULT_MINIGAME).expect("the gun game starts");
    let (mut sim, _) = local_party_with(sim);
    let got = Got::default();
    sim.app
        .insert_resource(Theme::from_toml(SIGNAL).unwrap())
        .insert_resource(UiUnits::new(1080.0, 1.0))
        .insert_resource(UiClock::manual())
        .insert_resource(FeedbackCapture(capture))
        .insert_resource(chimera_client::client::feedback::FeedbackOutbox(Arc::new(got.clone())))
        .init_resource::<Captures>();
    sim.app.world_mut().resource_mut::<Console>().submit("bind f10 feedback");
    sim.ticks(2);
    (sim, got)
}

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    app.update();
}

fn event(app: &mut App, key_code: KeyCode, logical_key: Key, state: ButtonState) {
    app.world_mut().write_message(KeyboardInput { key_code, logical_key, state, text: None, repeat: false, window: Entity::PLACEHOLDER });
}

/// A key typed (pressed and let go) as the window sends it.
fn key(app: &mut App, key_code: KeyCode, logical: Key) {
    event(app, key_code, logical.clone(), ButtonState::Pressed);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key_code);
    app.update();
    event(app, key_code, logical, ButtonState::Released);
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(key_code);
    app.update();
}

fn type_text(app: &mut App, text: &str) {
    for ch in text.chars() {
        match ch {
            ' ' => key(app, KeyCode::Space, Key::Space),
            c => key(app, KeyCode::KeyA, Key::Character(c.to_string().into())),
        }
    }
}

fn ctrl_enter(app: &mut App) {
    event(app, KeyCode::ControlLeft, Key::Control, ButtonState::Pressed);
    app.update();
    key(app, KeyCode::Enter, Key::Enter);
    event(app, KeyCode::ControlLeft, Key::Control, ButtonState::Released);
    app.update();
}

fn fb(app: &App) -> &Feedback {
    app.world().resource::<Feedback>()
}

fn open_with_f10(app: &mut App) {
    tap(app, KeyCode::F10);
    for _ in 0..6 {
        app.update();
    }
    assert_eq!(fb(app).phase, Phase::Open, "the dialog opens after the capture");
}

#[test]
fn the_key_captures_first_then_opens_and_send_submits() {
    let (mut sim, got) = party();
    assert!(fb(&sim.app).dialog, "Signal draws the dialog");
    open_with_f10(&mut sim.app);
    let caps = &sim.app.world().resource::<Captures>().0;
    assert_eq!(caps.len(), 1, "one capture");
    assert!(matches!(caps[0], Phase::Capturing(_)), "captured before the dialog opened: {:?}", caps[0]);
    // The frame is kept (its thumbnail needs `Assets<Image>`, which the
    // headless sim has not).
    assert!(fb(&sim.app).shot.is_some() && fb(&sim.app).include_screenshot);
    assert!(fb(&sim.app).snapshot.contains("Chimera build"), "the snapshot from the key press");

    // Typing goes into the report and the game's keys stay up.
    type_text(&mut sim.app, "fell through");
    assert_eq!(fb(&sim.app).text, "fell through");
    assert!(!sim.app.world().resource::<ButtonInput<KeyCode>>().pressed(KeyCode::KeyA), "the game sees no key");
    key(&mut sim.app, KeyCode::Enter, Key::Enter);
    type_text(&mut sim.app, "x");
    assert_eq!(fb(&sim.app).text, "fell through\nx", "Enter is a new line");
    key(&mut sim.app, KeyCode::Backspace, Key::Backspace);
    // Tab switches the category.
    key(&mut sim.app, KeyCode::Tab, Key::Tab);
    assert_eq!(fb(&sim.app).category, Category::Feedback);
    // Esc stays the dialog's: the Escape menu doesn't open... (Ctrl+Enter
    // sends first here.)
    ctrl_enter(&mut sim.app);
    sim.app.update();
    let reports = got.0.lock().unwrap();
    assert_eq!(reports.len(), 1, "Ctrl+Enter sent it");
    assert_eq!(reports[0].text, "fell through");
    assert_eq!(reports[0].category, Category::Feedback);
    assert!(reports[0].screenshot.is_some());
    drop(reports);
    assert_eq!(fb(&sim.app).phase, Phase::Closed);
    let notice = fb(&sim.app).notice.clone().expect("a notice");
    assert_eq!(notice.text, "Report sent");
}

#[test]
fn esc_cancels_and_an_empty_report_is_refused() {
    let (mut sim, got) = party();
    open_with_f10(&mut sim.app);
    ctrl_enter(&mut sim.app);
    assert!(fb(&sim.app).refused, "nothing typed: refused");
    assert_eq!(fb(&sim.app).phase, Phase::Open);
    // The screenshot toggled off is left out.
    sim.app.world_mut().write_message(chimera_client::client::feedback::FeedbackAction::ToggleScreenshot);
    sim.app.update();
    assert!(!fb(&sim.app).include_screenshot);
    key(&mut sim.app, KeyCode::Escape, Key::Escape);
    sim.app.update();
    assert_eq!(fb(&sim.app).phase, Phase::Closed, "Esc cancels");
    assert!(!sim.app.world().resource::<EscMenu>().open, "the Esc was the dialog's");
    assert!(got.0.lock().unwrap().is_empty(), "nothing sent");
}

/// A key with Ctrl (or Shift) held, as the window sends it.
fn with_mod(app: &mut App, (m, ml): (KeyCode, Key), key_code: KeyCode, logical: Key) {
    event(app, m, ml.clone(), ButtonState::Pressed);
    app.update();
    key(app, key_code, logical);
    event(app, m, ml, ButtonState::Released);
    app.update();
}

#[test]
fn the_report_text_takes_paste_copy_cut_and_select_all() {
    use chimera_ui::clipboard::Clipboard;
    let ctrl = (KeyCode::ControlLeft, Key::Control);
    let shift = (KeyCode::ShiftLeft, Key::Shift);
    let (mut sim, _) = party();
    sim.app.insert_resource(Clipboard::memory(Some("fell\r\nthrough")));
    open_with_f10(&mut sim.app);
    type_text(&mut sim.app, "I ");
    with_mod(&mut sim.app, ctrl.clone(), KeyCode::KeyV, Key::Character("v".into()));
    assert_eq!(fb(&sim.app).text, "I fell\nthrough", "Ctrl+V pastes, its line breaks kept");
    with_mod(&mut sim.app, shift.clone(), KeyCode::Insert, Key::Insert);
    assert_eq!(fb(&sim.app).text, "I fell\nthroughfell\nthrough", "Shift+Insert pastes");
    // Nothing selected: copy leaves the clipboard.
    sim.app.world_mut().insert_resource(Clipboard::memory(Some("kept")));
    with_mod(&mut sim.app, ctrl.clone(), KeyCode::KeyC, Key::Character("c".into()));
    assert_eq!(sim.app.world_mut().resource_mut::<Clipboard>().get().as_deref(), Some("kept"));
    // Ctrl+A selects the whole text, drawn as a selection; Ctrl+C copies it.
    with_mod(&mut sim.app, ctrl.clone(), KeyCode::KeyA, Key::Character("a".into()));
    assert!(fb(&sim.app).selected);
    sim.app.update();
    let drawn = sim.app.world_mut().query_filtered::<(), With<chimera_ui::controls::TextSelection>>().iter(sim.app.world()).count();
    assert!(drawn > 0, "the selection is drawn");
    with_mod(&mut sim.app, ctrl.clone(), KeyCode::KeyC, Key::Character("c".into()));
    assert_eq!(sim.app.world_mut().resource_mut::<Clipboard>().get().as_deref(), Some("I fell\nthroughfell\nthrough"));
    assert_eq!(fb(&sim.app).text, "I fell\nthroughfell\nthrough", "copy keeps the text");
    // Typing replaces the selection.
    type_text(&mut sim.app, "x");
    assert_eq!(fb(&sim.app).text, "x");
    assert!(!fb(&sim.app).selected);
    // Ctrl+X cuts it; Shift+Delete too.
    with_mod(&mut sim.app, ctrl.clone(), KeyCode::KeyA, Key::Character("a".into()));
    with_mod(&mut sim.app, ctrl.clone(), KeyCode::KeyX, Key::Character("x".into()));
    assert_eq!(fb(&sim.app).text, "");
    assert_eq!(sim.app.world_mut().resource_mut::<Clipboard>().get().as_deref(), Some("x"));
    type_text(&mut sim.app, "yz");
    with_mod(&mut sim.app, ctrl, KeyCode::KeyA, Key::Character("a".into()));
    with_mod(&mut sim.app, shift, KeyCode::Delete, Key::Delete);
    assert_eq!(fb(&sim.app).text, "");
    assert_eq!(sim.app.world_mut().resource_mut::<Clipboard>().get().as_deref(), Some("yz"));
}

#[test]
fn the_escape_menu_entry_closes_the_menu_before_the_capture() {
    let (mut sim, got) = party();
    tap(&mut sim.app, KeyCode::Escape);
    assert!(sim.app.world().resource::<EscMenu>().open);
    let entries = party_roster::esc_menu::shown_entries(true, false, false);
    assert!(entries.iter().any(|e| e.label == "Report a bug" && e.action == Some(Action::Feedback)), "{entries:?}");
    sim.app.world_mut().write_message(Action::Feedback);
    for _ in 0..8 {
        sim.app.update();
    }
    assert!(!sim.app.world().resource::<EscMenu>().open);
    assert_eq!(sim.app.world().resource::<Captures>().0.len(), 1);
    assert_eq!(fb(&sim.app).phase, Phase::Open);
    // The key with the menu open closes it first too (`capture` asserts).
    key(&mut sim.app, KeyCode::Escape, Key::Escape);
    sim.app.update();
    tap(&mut sim.app, KeyCode::Escape);
    assert!(sim.app.world().resource::<EscMenu>().open);
    tap(&mut sim.app, KeyCode::F10);
    for _ in 0..8 {
        sim.app.update();
    }
    assert_eq!(fb(&sim.app).phase, Phase::Open);
    assert_eq!(sim.app.world().resource::<Captures>().0.len(), 2);
    assert!(got.0.lock().unwrap().is_empty());
}

#[test]
fn the_report_key_is_a_shared_controls_row_that_rebinds() {
    let (mut sim, _) = party();
    tap(&mut sim.app, KeyCode::Escape);
    tap(&mut sim.app, KeyCode::ArrowDown);
    tap(&mut sim.app, KeyCode::Enter);
    sim.app.update();
    let s = sim.app.world().resource::<Settings>().clone();
    let ctx = s.ctx.expect("Settings open");
    // The Controls page (Mouse becomes Controls), Shared binds.
    let tabs = party_roster::esc_menu::shown_tabs(sim.app.world());
    let tab = tabs.iter().position(|(n, _)| n.eq_ignore_ascii_case("mouse") || n.eq_ignore_ascii_case("controls")).expect("a Controls tab");
    sim.app.world_mut().write_message(SettingsAction::Tab(tab));
    sim.app.update();
    sim.app.update();
    let s = sim.app.world().resource::<Settings>().clone();
    let rows = settings::rows(ctx, &tabs, s.tab, s.view());
    let row = rows.iter().position(|r| matches!(r, Row::Bind { command: "feedback", .. })).expect("a Report a bug row");
    assert_eq!(party_roster::controls::keys_on(party_roster::client::controls(), sim.app.world().resource::<Console>(), 0, "feedback"), ["f10"]);
    sim.app.world_mut().write_message(SettingsAction::Bind(row));
    sim.app.update();
    sim.app.update();
    assert!(fb(&sim.app).key_blocked, "the key is held off while a bind listens");
    tap(&mut sim.app, KeyCode::F7);
    assert_eq!(party_roster::controls::keys_on(party_roster::client::controls(), sim.app.world().resource::<Console>(), 0, "feedback"), ["f7"]);
    assert_eq!(fb(&sim.app).phase, Phase::Closed, "binding a key opens no report");
}

/// The user's outbox (`AssetsOutbox`) with no service: Send queues a zip
/// in `outbox/` with the words, the frame as JPEG and the game's state,
/// a secret typed into the console filtered out, and the notice says it
/// is kept on this PC until signed in.
#[test]
fn a_report_without_a_service_is_a_bundle_saved_on_this_pc() {
    use chimera_client::client::feedback::{AssetsOutbox, FeedbackOutbox};
    use chimera_core::mount::assets::feedback::{Targets, bundle};
    let (mut sim, _) = party();
    let dir = std::env::temp_dir().join(format!("chimera-feedback-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    sim.app.insert_resource(FeedbackOutbox(Arc::new(AssetsOutbox::at(dir.clone(), Targets::default()))));
    sim.app.world_mut().resource_mut::<Console>().submit("rcon_password PlantedPw55");
    sim.app.update();
    open_with_f10(&mut sim.app);
    assert_eq!(
        sim.app.world().resource::<FeedbackOutbox>().0.destination(),
        "Not signed in: reports kept on this PC"
    );
    type_text(&mut sim.app, "stuck on the ramp");
    ctrl_enter(&mut sim.app);
    sim.app.update();
    let notice = fb(&sim.app).notice.clone().expect("a notice");
    assert!(notice.text.starts_with("Sign in to send reports; kept on this PC"), "{}", notice.text);
    // The bundle is written off the main thread.
    let outbox = dir.join("outbox");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let zip = loop {
        let found = std::fs::read_dir(&outbox)
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .find(|p| p.extension().is_some_and(|x| x == "zip"));
        if let Some(z) = found {
            break z;
        }
        assert!(std::time::Instant::now() < deadline, "no zip in {}", outbox.display());
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    let bytes = std::fs::read(&zip).unwrap();
    let opened = bundle::open(&bytes).expect("a valid bundle");
    assert_eq!(opened.info.text, "stuck on the ramp");
    let names: Vec<&str> = opened.entries.iter().map(|e| e.name.as_str()).collect();
    for want in ["report.json", "screenshot.jpg", "game.txt", "system.txt", "cvars.txt", "console.txt"] {
        assert!(names.contains(&want), "{want} in {names:?}");
    }
    let all: String = opened.entries.iter().map(|e| String::from_utf8_lossy(&e.data).into_owned()).collect();
    assert!(!all.contains("PlantedPw55"), "the console's secret is filtered");
    assert!(all.contains("bind f10 \"feedback\""), "the changed bind is listed");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A service named by the cvars (`chimera:feedback_url`, `_token`): the
/// report reaches the asset service's `POST /feedback` (a real one on a
/// free port), and the notice turns from "Sending" to "Report sent".
#[test]
fn a_report_reaches_the_named_service_and_the_notice_says_sent() {
    use chimera_client::client::feedback::{AssetsOutbox, FeedbackOutbox};
    use chimera_core::mount::assets::{
        Store,
        accounts::Accounts,
        feedback::{Targets, inbox::Inbox},
        server,
    };
    let (mut sim, _) = party();
    let dir = std::env::temp_dir().join(format!("chimera-feedback-sent-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let store = Store::new(dir.join("server"));
    let service = server::Service::new(store.clone()).unwrap().with_feedback(Default::default(), None);
    let running = server::serve(service, "127.0.0.1:0", 2).unwrap();
    let path = Accounts::path(&store.root);
    let mut accounts = Accounts::load(&path).unwrap();
    let (_, token) = accounts.add("tester", None, "test").unwrap();
    accounts.save(&path).unwrap();
    sim.app.insert_resource(FeedbackOutbox(Arc::new(AssetsOutbox::at(dir.join("client"), Targets::default()))));
    let url = running.url();
    sim.app.world_mut().resource_mut::<Console>().submit(&format!("chimera:feedback_url \"{url}\"; chimera:feedback_token {token}"));
    sim.app.update();
    sim.app.update();
    open_with_f10(&mut sim.app);
    assert_eq!(sim.app.world().resource::<FeedbackOutbox>().0.destination(), "To the report service");
    type_text(&mut sim.app, "the gun vanished");
    ctrl_enter(&mut sim.app);
    sim.app.update();
    // "Sending report...", unless the local service answered already.
    let first = fb(&sim.app).notice.clone().expect("a notice").text;
    assert!(first == "Sending report..." || first == "Report sent", "{first}");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    while fb(&sim.app).notice.as_ref().is_none_or(|n| !n.sent) {
        assert!(std::time::Instant::now() < deadline, "no Sent: {:?}", fb(&sim.app).notice);
        std::thread::sleep(std::time::Duration::from_millis(50));
        sim.app.update();
    }
    assert_eq!(fb(&sim.app).notice.as_ref().unwrap().text, "Report sent");
    let got = Inbox::open(server::feedback_dir(&store)).list();
    assert_eq!(got.len(), 1, "stored once");
    assert!(got[0].account.contains("tester"), "{}", got[0].account);
    drop(running);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The report from the pre-match lobby: the lobby takes the report as an
/// overlay (its panel hidden under the dialog) only once the dialog shows,
/// so the frame taken is the lobby as the player saw it (`capture`
/// asserts it), from the key and from the lobby's Report button.
#[test]
fn the_lobby_stays_drawn_for_the_reports_frame() {
    use chimera_client::client::prematch_lobby::LobbyOverlay;
    let (mut sim, _) = party();
    let overlaid = |sim: &Sim| sim.app.world().resource::<LobbyOverlay>().0;
    assert!(!overlaid(&sim));
    tap(&mut sim.app, KeyCode::F10);
    assert!(!overlaid(&sim), "not while the frame is taken");
    for _ in 0..6 {
        sim.app.update();
    }
    assert_eq!(fb(&sim.app).phase, Phase::Open);
    assert!(overlaid(&sim), "the dialog over the lobby is its overlay");
    key(&mut sim.app, KeyCode::Escape, Key::Escape);
    sim.app.update();
    assert_eq!(fb(&sim.app).phase, Phase::Closed);
    assert!(!overlaid(&sim));
    // The lobby's Report a bug: the menu way (settling first).
    chimera_client::client::feedback::request(sim.app.world_mut(), true);
    for _ in 0..8 {
        sim.app.update();
    }
    assert_eq!(fb(&sim.app).phase, Phase::Open);
    assert_eq!(sim.app.world().resource::<Captures>().0.len(), 2);
}
