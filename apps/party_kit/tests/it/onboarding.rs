//! The first run (`party_kit::onboarding`; docs/plans/active/
//! onboarding.md), headless on a manual UI clock with the staged hub
//! answering in the same frame: an invite link redeemed through every
//! screen (the hub confirmed, a taken name answered with suggestions, a
//! suggestion taken, the avatar chosen, the recovery key saved) ends on
//! the main menu signed in with the toast; each error says what happened
//! and keeps the input; a bare code without a known hub asks for the
//! link; the entry never blocks (Enter works on the first frame and ends
//! the entry); the menus behind take no keys; a device link code signs
//! in without the name and key screens; recovery by the key shows the
//! new key; Skip writes its mark; every staged screen draws its words;
//! each error offers its one action as the hero (Enter clears a used
//! code, a certificate that isn't the invite's has no way on and copies
//! its details for the owner); the bar carries the game's own lockup; a
//! PC with no account sees the first run (a no-hub asset token aside)
//! until it signs in or skips.

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use chimera_core::console::Console;
use chimera_ui::{Timeline, UiClock};
use chimera_world::harness::Sim;
use party_kit::onboarding::{
    ChipRoot, HaveTab, NameField, ObAction, Onboarding, OnboardingBackend, OnboardingFiles, OnboardingLockup, OnboardingPlugin,
    OnboardingRoot, Problem, STAGED_KEY, Step, ToastRoot, first_run_due,
};

use super::signal_menu::step;

const THEME: &str = r#"
name = "test"
[shape]
slant_deg = 12.0
[layout.menu]
items_y = 23.4
[layout.esc]
"#;

const LINK: &str = "chimera-hub://hub.example.net:27080/3f9a0b71c2d45e6f3f9a0b71c2d45e6f3f9a0b71c2d45e6f3f9a0b71c2d45e6f?code=K7Q-M2P-9TR";

/// What `cl_avatar` was set to (the profile's cvar isn't in this sim).
#[derive(Resource, Default)]
struct AvatarAsked(Option<String>);

fn sim() -> Sim {
    let theme = chimera_ui::Theme::from_toml(THEME).unwrap();
    let mut sim = Sim::with(combat_arms::harness::CaSim, |app| {
        app.add_plugins((chimera_world::greybox::GreyboxMapPlugin, party_kit::PartyPartsPlugin, chimera_net::net::NetPlugin));
        app.insert_resource(theme).add_plugins(chimera_ui::ChimeraUiPlugin).insert_resource(UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
        super::main_menu::menu_bits(app, true);
        app.init_resource::<AvatarAsked>();
        let _ = chimera_core::console::ConsoleAppExt::console_command(app, "cl_avatar", "a test's", |w, a| {
            w.resource_mut::<AvatarAsked>().0 = a.first().cloned();
            Ok(None)
        });
        if !app.world().contains_resource::<chimera_client::client::ui_scene::UiScene>() {
            app.add_plugins(chimera_client::client::ui_scene::UiScenePlugin);
        }
        app.add_plugins(OnboardingPlugin)
            .insert_resource(OnboardingBackend::staged())
            .insert_resource(chimera_ui::clipboard::Clipboard::memory(None));
    });
    sim.ticks(3);
    start(&mut sim);
    sim
}

fn start(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<Console>().submit("onboarding 1");
    sim.ticks(2);
}

fn o(sim: &Sim) -> &Onboarding {
    sim.app.world().resource::<Onboarding>()
}

fn key(sim: &mut Sim, logical: Key, code: KeyCode) {
    let window = Entity::PLACEHOLDER;
    sim.app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: logical,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    sim.ticks(1);
}

fn typed(sim: &mut Sim, text: &str) {
    key(sim, Key::Character(text.into()), KeyCode::KeyA);
}

fn enter(sim: &mut Sim) {
    key(sim, Key::Enter, KeyCode::Enter);
    sim.ticks(1);
}

fn act(sim: &mut Sim, a: ObAction) {
    sim.app.world_mut().write_message(a);
    sim.ticks(2);
}

/// The words on the screen (upper or own case), joined.
fn words(sim: &mut Sim) -> String {
    let w = sim.app.world_mut();
    let roots: Vec<Entity> = w.query_filtered::<Entity, Or<(With<OnboardingRoot>, With<ToastRoot>, With<ChipRoot>)>>().iter(w).collect();
    let mut out = String::new();
    let mut stack = roots;
    while let Some(e) = stack.pop() {
        if let Some(t) = w.get::<Text>(e) {
            out.push_str(&t.0);
            out.push('|');
        }
        if let Some(ch) = w.get::<Children>(e) {
            stack.extend(ch.iter());
        }
    }
    out
}

fn wait_name(sim: &mut Sim) {
    step(sim, 0.5);
    sim.ticks(2);
}

#[test]
fn an_invite_link_goes_through_every_screen_to_the_menu_signed_in() {
    let mut sim = sim();
    assert!(o(&sim).active && o(&sim).step == Step::Welcome);
    assert!(words(&mut sim).contains("PASTE YOUR INVITE"));
    // Enter works at once: no blocking entry, and it ends the entry.
    typed(&mut sim, LINK);
    enter(&mut sim);
    assert_eq!(o(&sim).step, Step::Hub, "{:?}", o(&sim).problem);
    let text = words(&mut sim);
    assert!(text.contains("FRIDAY NIGHT HUB") && text.contains("Gun game every Friday"), "{text}");
    enter(&mut sim);
    assert_eq!(o(&sim).step, Step::Name);
    // A taken name: suggestions, Continue off.
    typed(&mut sim, "Rocket");
    wait_name(&mut sim);
    let n = o(&sim).name.clone().unwrap();
    assert!(!n.available && n.suggestions.contains(&"rocket3".to_string()), "{n:?}");
    enter(&mut sim);
    assert_eq!(o(&sim).step, Step::Name, "a taken name doesn't go on");
    assert!(words(&mut sim).contains("SOMEONE ON FRIDAY NIGHT HUB HAS @ROCKET"));
    act(&mut sim, ObAction::Suggest("rocket3".into()));
    wait_name(&mut sim);
    assert!(o(&sim).name_ok());
    // The display name and the avatar (Tab, then the arrows).
    key(&mut sim, Key::Tab, KeyCode::Tab);
    typed(&mut sim, "Rocket Three");
    key(&mut sim, Key::Tab, KeyCode::Tab);
    assert_eq!(o(&sim).field, NameField::Avatar);
    let before = o(&sim).avatar;
    key(&mut sim, Key::ArrowRight, KeyCode::ArrowRight);
    assert_eq!(o(&sim).avatar, (before + 1) % 32);
    enter(&mut sim);
    assert_eq!(o(&sim).step, Step::Key);
    assert_eq!(o(&sim).recovery.as_deref(), Some(STAGED_KEY));
    // Continue waits for "I saved it".
    enter(&mut sim);
    assert_eq!(o(&sim).step, Step::Key);
    act(&mut sim, ObAction::Copy);
    let got = sim.app.world_mut().resource_mut::<chimera_ui::clipboard::Clipboard>().get();
    assert_eq!(got.as_deref(), Some(STAGED_KEY));
    key(&mut sim, Key::Space, KeyCode::Space);
    assert!(o(&sim).saved);
    enter(&mut sim);
    let ob = o(&sim).clone();
    assert!(!ob.active, "the menu");
    assert_eq!(ob.signed_in.as_ref().map(|f| (f.username.as_str(), f.display_name.as_str())), Some(("rocket3", "Rocket Three")));
    sim.ticks(2);
    let avatar = sim.app.world().resource::<AvatarAsked>().0.clone();
    assert_eq!(avatar, Some(((before + 1) % 32).to_string()), "the avatar chosen is the player's");
    assert!(words(&mut sim).contains("WELCOME, ROCKET THREE"), "the toast");
    step(&mut sim, 6.5);
    sim.ticks(2);
    assert!(!words(&mut sim).contains("WELCOME"), "the toast goes");
}

#[test]
fn each_error_says_what_happened_and_keeps_the_input() {
    let mut sim = sim();
    for (code, want, line) in [
        ("AAA-AAA-AAA", Problem::Used, "THIS INVITE WAS ALREADY USED"),
        ("BBB-BBB-BBB", Problem::Unknown, "THIS CODE DOESN'T MATCH ANY INVITE"),
        ("RRR-RRR-RRR", Problem::Unreachable, "CAN'T REACH THE HUB"),
        ("CER-T00-000", Problem::Certificate { offered: Some("77d0e2c4a51b19af".into()) }, "THIS ISN'T THE HUB YOUR INVITE NAMES"),
    ] {
        act(&mut sim, ObAction::Clear);
        typed(&mut sim, &LINK.replace("K7Q-M2P-9TR", code));
        enter(&mut sim);
        assert_eq!((o(&sim).step, o(&sim).problem.clone()), (Step::Welcome, Some(want)), "{code}");
        assert!(o(&sim).input.contains(code), "the input is kept");
        assert!(words(&mut sim).contains(line), "{code}");
    }
    // Not an invite; a bare code with no hub known.
    act(&mut sim, ObAction::Clear);
    typed(&mut sim, "see you friday!");
    enter(&mut sim);
    assert_eq!(o(&sim).problem, Some(Problem::NotInvite));
    act(&mut sim, ObAction::Clear);
    typed(&mut sim, "k7q m2p 9tr");
    enter(&mut sim);
    assert_eq!(o(&sim).problem, Some(Problem::NeedsHub));
    // The menus behind take no keys while it shows.
    assert!(party_kit::onboarding::takes_keys(Some(o(&sim))));
}

/// Each error's hero is the one action that helps (the mockups' 03b-d).
#[test]
fn each_error_offers_its_one_action() {
    let mut sim = sim();
    // Used: Try another (Enter) clears the field; I already have an account.
    typed(&mut sim, &LINK.replace("K7Q-M2P-9TR", "AAA-AAA-AAA"));
    enter(&mut sim);
    let text = words(&mut sim);
    assert!(text.contains("TRY ANOTHER") && text.contains("I ALREADY HAVE AN ACCOUNT"), "{text}");
    enter(&mut sim);
    assert_eq!((o(&sim).input.as_str(), o(&sim).problem.clone()), ("", None), "Enter tries another");
    // Unreachable: Retry, and playing without an account.
    typed(&mut sim, &LINK.replace("K7Q-M2P-9TR", "RRR-RRR-RRR"));
    enter(&mut sim);
    let text = words(&mut sim);
    assert!(text.contains("RETRY") && text.contains("PLAY WITHOUT AN ACCOUNT"), "{text}");
    // Another certificate: no field, no way on; the details for the owner.
    act(&mut sim, ObAction::Clear);
    typed(&mut sim, &LINK.replace("K7Q-M2P-9TR", "CER-T00-000"));
    enter(&mut sim);
    let text = words(&mut sim);
    assert!(text.contains("COPY DETAILS FOR THE OWNER") && !text.contains("INVITE · LINK, WEB ADDRESS OR CODE"), "{text}");
    enter(&mut sim);
    assert!(matches!(o(&sim).problem, Some(Problem::Certificate { .. })), "Enter doesn't go on");
    act(&mut sim, ObAction::CopyDetails);
    let got = sim.app.world_mut().resource_mut::<chimera_ui::clipboard::Clipboard>().get().unwrap_or_default();
    assert!(got.contains("3F9A 0B71") && got.contains("77D0 E2C4") && got.contains("hub.example.net"), "{got}");
    key(&mut sim, Key::Escape, KeyCode::Escape);
    assert_eq!((o(&sim).input.as_str(), o(&sim).problem.clone()), ("", None), "Esc goes back");
}

/// The bar carries the game's own lockup (the main menu's mark and
/// wordmark), not a stand-in.
#[test]
fn the_bar_carries_the_games_lockup() {
    let mut sim = sim();
    let w = sim.app.world_mut();
    let lockups: Vec<Entity> = w.query_filtered::<Entity, With<OnboardingLockup>>().iter(w).collect();
    assert_eq!(lockups.len(), 1);
    let mut stack = lockups;
    let mut mark = false;
    while let Some(e) = stack.pop() {
        mark |= w.get::<chimera_ui::controls::KitIcon>(e).is_some_and(|k| k.key == "mark");
        if let Some(ch) = w.get::<Children>(e) {
            stack.extend(ch.iter());
        }
    }
    assert!(mark, "the mark");
    let text = words(&mut sim);
    assert!(text.contains("GUN GAME") && !text.contains("LUCKER PARTY"), "{text}");
}

#[test]
fn the_entry_slides_in_but_never_blocks() {
    let mut sim = sim();
    let n = {
        let w = sim.app.world_mut();
        w.query_filtered::<(), With<Timeline>>().iter(w).count()
    };
    assert!(n > 3, "the parts slide in ({n})");
    let since = o(&sim).since;
    typed(&mut sim, "x");
    assert!(o(&sim).since < since - 5.0, "a key ends the entry");
    assert_eq!(o(&sim).input, "x", "and still types");
}

#[test]
fn a_link_code_signs_in_without_the_name_and_key_screens() {
    let mut sim = sim();
    act(&mut sim, ObAction::Have);
    assert_eq!(o(&sim).step, Step::Have);
    // No hub known: the code alone asks for the link.
    typed(&mut sim, "H4K-92Q");
    enter(&mut sim);
    assert_eq!(o(&sim).problem, Some(Problem::NeedsHub));
    act(&mut sim, ObAction::HaveField(true));
    typed(&mut sim, "chimera-hub://hub.example.net:27080/");
    enter(&mut sim);
    sim.ticks(3);
    let ob = o(&sim).clone();
    assert!(!ob.active, "{:?} {:?}", ob.step, ob.problem);
    assert_eq!(ob.signed_in.map(|f| f.username), Some("sam_rocket".into()));
}

#[test]
fn recovering_shows_the_new_key_and_skip_writes_its_mark() {
    let mut sim = sim();
    sim.app.world_mut().resource_mut::<Onboarding>().last_hub =
        Some(chimera_assets::account::HubLink { host: "hub.example.net:27080".into(), fingerprint: None, code: None });
    act(&mut sim, ObAction::Have);
    act(&mut sim, ObAction::Tab(HaveTab::Recover));
    typed(&mut sim, "0000");
    enter(&mut sim);
    assert!(matches!(o(&sim).problem, Some(Problem::Other(_))), "a wrong key: {:?}", o(&sim).problem);
    act(&mut sim, ObAction::Tab(HaveTab::Recover));
    typed(&mut sim, "RK7F-2M9Q-XA4T-8WLC");
    enter(&mut sim);
    assert_eq!((o(&sim).step, o(&sim).recovery.as_deref()), (Step::Key, Some(STAGED_KEY)));

    let dir = std::env::temp_dir().join(format!("onboarding-skip-{}", std::process::id()));
    let mark = dir.join("onboarding-skipped");
    let mut sim = self::sim();
    sim.app.insert_resource(OnboardingFiles { skip_mark: Some(mark.clone()), ..Default::default() });
    act(&mut sim, ObAction::Skip);
    assert!(!o(&sim).active && mark.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// A PC with no account and no skip mark gets the first run, whatever
/// else it holds (a token for an asset service with no hub beside the
/// account file is no way onto a hub); an account file or the skip mark
/// ends it.
#[test]
fn a_pc_with_no_account_sees_the_first_run() {
    let dir = std::env::temp_dir().join(format!("onboarding-due-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let files = OnboardingFiles {
        account: Some(dir.join("account.toml")),
        skip_mark: Some(dir.join("onboarding-skipped")),
        key_dir: None,
    };
    let mut access = chimera_assets::access::Access::default();
    access.set_token(&format!("chimera1_{}", "0".repeat(64))).unwrap();
    access.save_to(&dir.join("asset-access.toml")).unwrap();
    assert!(first_run_due(&files), "no account: the first run");
    let file = chimera_assets::account::AccountFile {
        url: "https://hub.example.net:27080".into(),
        account: "00".repeat(16),
        device: "11".repeat(16),
        username: "sam".into(),
        device_key: "03".repeat(32),
        ..Default::default()
    };
    file.save_to(files.account.as_ref().unwrap()).unwrap();
    assert!(!first_run_due(&files), "signed in");
    std::fs::remove_file(files.account.as_ref().unwrap()).unwrap();
    std::fs::write(files.skip_mark.as_ref().unwrap(), "").unwrap();
    assert!(!first_run_due(&files), "skipped");
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every staged screen draws, with all its words in the data file.
#[test]
fn every_staged_screen_draws_its_words() {
    let mut sim = sim();
    for s in [
        "welcome", "clipboard", "checking", "used", "unknown", "unreachable", "certificate", "hub", "name", "name-bad", "name-taken",
        "key", "have", "recover", "done",
    ] {
        sim.app.world_mut().resource_mut::<Console>().submit(&format!("ui_scene onboarding {s}"));
        sim.ticks(3);
        let text = words(&mut sim);
        assert!(!text.is_empty(), "{s} draws");
        let want = match s {
            "hub" => "YOU'RE JOINING",
            "key" => "SAVE YOUR RECOVERY KEY",
            "name-taken" => "FREE:",
            "have" => "I ALREADY HAVE AN ACCOUNT",
            "done" => "WELCOME, SAM ROCKET",
            _ => "",
        };
        assert!(text.contains(want), "{s}: {text}");
        assert!(!text.contains('['), "{s}: a word missing from data/onboarding.toml: {text}");
    }
    let _ = sim.app.world().resource::<UiClock>();
}

/// A returning player: no screen, the account chip on the main menu;
/// with the hub unreachable the chip turns amber and the banner says
/// offline play goes on.
#[test]
fn a_returning_player_sees_the_chip_and_the_offline_banner() {
    let mut sim = sim();
    sim.app.world_mut().resource_mut::<Console>().submit("ui_scene onboarding returning");
    sim.ticks(3);
    assert!(!o(&sim).active, "no sign-in screen");
    let text = words(&mut sim);
    assert!(text.contains("Sam Rocket") && text.contains("@SAM_ROCKET · FRIDAY NIGHT HUB"), "{text}");
    assert!(!text.contains("CAN'T REACH"));
    sim.app.world_mut().resource_mut::<Console>().submit("ui_scene onboarding offline");
    sim.ticks(3);
    let text = words(&mut sim);
    assert!(text.contains("OFFLINE · FRIDAY NIGHT HUB") && text.contains("CAN'T REACH FRIDAY NIGHT HUB"), "{text}");
    // The first run's end gives the chip its names.
    let mut sim = self::sim();
    typed(&mut sim, LINK);
    enter(&mut sim);
    enter(&mut sim);
    typed(&mut sim, "sam_rocket");
    wait_name(&mut sim);
    enter(&mut sim);
    key(&mut sim, Key::Space, KeyCode::Space);
    enter(&mut sim);
    sim.ticks(2);
    let shown = sim.app.world().resource::<party_kit::onboarding::AccountShown>().0.clone().unwrap();
    assert_eq!((shown.username.as_str(), shown.hub.as_str()), ("sam_rocket", "Friday Night Hub"));
}
