//! Settings > Account (`party_roster::account_page`; docs/plans/active/
//! onboarding.md, "The account page") and the base files up front
//! (`party_roster::base_download`), headless on a manual UI clock with the
//! staged hub answering in the same frame: the chip opens the profile;
//! a rename to a taken name offers the sign-up's free names and one of
//! them renames (the chip follows); the avatar changes at once and the
//! hub hears after a pause; a link code shows; sign out asks in its row
//! (Esc stays), then deletes this PC's account file and shows the first
//! run; an owner makes an invite (intent, recipient, role), copies its
//! link, sees it listed and revokes it, a player sees no Invites tab;
//! every staged screen draws its words; the first run's end starts the
//! base files' fetch, and its strip takes no clicks.

use std::sync::{
    Arc,
    atomic::{AtomicU32, Ordering},
};

use bevy::{
    input::{
        ButtonState,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};
use chimera_core::console::Console;
use chimera_ui::UiClock;
use chimera_world::harness::Sim;
use party_roster::{
    account_page::{AccountAction, AccountBackend, AccountPage, AccountPagePlugin, AccountRoot, Editing, Row, STAGED_LINK, Tab},
    base_download::{BaseDownload, BaseDownloadPlugin, BaseStarter, BaseStrip},
    onboarding::{AccountShown, Onboarding, OnboardingBackend, OnboardingFiles, OnboardingPlugin, Shown, Step},
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

/// What `cl_avatar` was set to.
#[derive(Resource, Default)]
struct AvatarAsked(Option<String>);

fn sim(owner: bool) -> Sim {
    let theme = chimera_ui::Theme::from_toml(THEME).unwrap();
    let mut sim = Sim::with(combat_arms::harness::CaSim, |app| {
        app.add_plugins((chimera_world::greybox::GreyboxMapPlugin, party_roster::PartyPartsPlugin, chimera_net::net::NetPlugin));
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
        app.add_plugins((OnboardingPlugin, AccountPagePlugin, BaseDownloadPlugin))
            .insert_resource(OnboardingBackend::staged())
            .insert_resource(AccountBackend::staged(owner))
            .insert_resource(BaseStarter(Arc::new(|_| None)))
            .insert_resource(chimera_ui::clipboard::Clipboard::memory(None));
    });
    sim.ticks(3);
    sim.app.world_mut().insert_resource(AccountShown(Some(Shown {
        display: "Sam Rocket".into(),
        username: "sam_rocket".into(),
        hub: "Friday Night Hub".into(),
    })));
    sim
}

fn open(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<Console>().submit("account_page 1");
    sim.ticks(3);
}

fn p(sim: &Sim) -> &AccountPage {
    sim.app.world().resource::<AccountPage>()
}

fn key(sim: &mut Sim, logical: Key, code: KeyCode) {
    sim.app.world_mut().write_message(KeyboardInput {
        key_code: code,
        logical_key: logical,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    sim.ticks(2);
}

fn typed(sim: &mut Sim, text: &str) {
    key(sim, Key::Character(text.into()), KeyCode::KeyA);
}

fn enter(sim: &mut Sim) {
    key(sim, Key::Enter, KeyCode::Enter);
}

fn act(sim: &mut Sim, a: AccountAction) {
    sim.app.world_mut().write_message(a);
    sim.ticks(2);
}

fn focus(sim: &mut Sim, row: Row) {
    let i = p(sim).rows().iter().position(|r| *r == row).unwrap();
    act(sim, AccountAction::Row(i));
}

/// The words on the page and the strip, joined.
fn words(sim: &mut Sim) -> String {
    let w = sim.app.world_mut();
    let mut stack: Vec<Entity> = w.query_filtered::<Entity, Or<(With<AccountRoot>, With<BaseStrip>)>>().iter(w).collect();
    let mut out = String::new();
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

#[test]
fn the_profile_renames_with_free_names_when_taken() {
    let mut sim = sim(false);
    open(&mut sim);
    assert!(p(&sim).open && p(&sim).me.as_ref().is_some_and(|m| m.username == "sam_rocket"));
    assert_eq!(p(&sim).tabs(), [Tab::Profile], "a player sees no Invites");
    let text = words(&mut sim);
    assert!(text.contains("@sam_rocket") && text.contains("Friday Night Hub"), "{text}");
    // Enter edits the name; a taken one offers free names.
    enter(&mut sim);
    assert_eq!(p(&sim).editing, Some(Editing::Name));
    for _ in 0.."sam_rocket".len() {
        key(&mut sim, Key::Backspace, KeyCode::Backspace);
    }
    typed(&mut sim, "rocket");
    step(&mut sim, 0.5);
    sim.ticks(2);
    let check = p(&sim).name_check.clone().unwrap();
    assert_eq!((check.available, check.suggestions.as_slice()), (false, &["rocket2".to_string(), "rocket_gg".into(), "rocket_10".into()][..]));
    let text = words(&mut sim).to_lowercase();
    assert!(text.contains("rocket_gg"), "the free names drawn: {text}");
    enter(&mut sim);
    assert_eq!(p(&sim).me.as_ref().unwrap().username, "sam_rocket", "a taken name doesn't rename");
    act(&mut sim, AccountAction::Suggest("rocket_gg".into()));
    step(&mut sim, 0.5);
    sim.ticks(2);
    assert!(p(&sim).name_ok());
    enter(&mut sim);
    sim.ticks(2);
    assert_eq!(p(&sim).me.as_ref().unwrap().username, "rocket_gg");
    assert_eq!(p(&sim).editing, None);
    assert_eq!(sim.app.world().resource::<AccountShown>().0.as_ref().unwrap().username, "rocket_gg", "the chip follows");
    assert!(words(&mut sim).contains("@rocket_gg"));
    // Esc closes the page.
    key(&mut sim, Key::Escape, KeyCode::Escape);
    assert!(!p(&sim).open);
}

#[test]
fn avatar_link_code_and_sign_out() {
    let mut sim = sim(false);
    let dir = std::env::temp_dir().join(format!("account-page-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("account.toml");
    std::fs::write(&file, "# staged\n").unwrap();
    sim.app.insert_resource(OnboardingFiles { account: Some(file.clone()), ..Default::default() });
    open(&mut sim);
    // The avatar: at once here, the hub told after a pause.
    focus(&mut sim, Row::Avatar);
    key(&mut sim, Key::ArrowRight, KeyCode::ArrowRight);
    assert_eq!(sim.app.world().resource::<AvatarAsked>().0.as_deref(), Some("14"));
    assert_eq!(p(&sim).me.as_ref().unwrap().avatar, Some(13), "not yet told");
    step(&mut sim, 0.7);
    sim.ticks(2);
    assert_eq!(p(&sim).me.as_ref().unwrap().avatar, Some(14));
    // Link another device: the code in cells.
    focus(&mut sim, Row::Link);
    enter(&mut sim);
    assert_eq!(p(&sim).link.as_ref().map(|l| l.0.as_str()), Some(STAGED_LINK));
    let text = words(&mut sim);
    assert!(text.contains("H|") && text.contains("Q|") && text.contains("VALID FOR 10 MINUTES"), "{text}");
    // Sign out asks in its row; Esc stays.
    focus(&mut sim, Row::SignOut);
    enter(&mut sim);
    assert!(p(&sim).confirm_sign_out && words(&mut sim).contains("This PC forgets @sam_rocket"));
    key(&mut sim, Key::Escape, KeyCode::Escape);
    assert!(p(&sim).open && !p(&sim).confirm_sign_out && file.exists());
    enter(&mut sim);
    enter(&mut sim);
    sim.ticks(2);
    assert!(!file.exists(), "this PC's account file deleted");
    assert!(!p(&sim).open && sim.app.world().resource::<AccountShown>().0.is_none());
    let o = sim.app.world().resource::<Onboarding>();
    assert!(o.active && o.step == Step::Welcome && o.intro.is_none(), "the first run, no intro");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_owner_makes_copies_lists_and_revokes_invites() {
    let mut sim = sim(true);
    open(&mut sim);
    assert_eq!(p(&sim).tabs(), [Tab::Profile, Tab::Invites]);
    assert_eq!(p(&sim).invites.as_ref().map(Vec::len), Some(3));
    typed(&mut sim, "e");
    assert_eq!(p(&sim).tab, Tab::Invites);
    // Make with no intent: asked for one.
    focus(&mut sim, Row::Make);
    enter(&mut sim);
    assert!(p(&sim).said.as_ref().is_some_and(|s| s.error) && p(&sim).editing == Some(Editing::Intent));
    typed(&mut sim, "sent on Discord");
    enter(&mut sim);
    assert_eq!((p(&sim).row(), p(&sim).editing), (Some(Row::Recipient), None));
    enter(&mut sim);
    typed(&mut sim, "Alex");
    enter(&mut sim);
    assert_eq!(p(&sim).row(), Some(Row::Role));
    key(&mut sim, Key::ArrowRight, KeyCode::ArrowRight);
    assert_eq!(p(&sim).role, 1, "host");
    key(&mut sim, Key::ArrowDown, KeyCode::ArrowDown);
    enter(&mut sim);
    sim.ticks(2);
    let made = p(&sim).made.clone().expect("made");
    assert_eq!(made.name, "Alex");
    let list = p(&sim).invites.clone().unwrap();
    assert_eq!((list.len(), list[0].name.as_str(), list[0].role.as_str()), (4, "Alex", "host"));
    assert!(p(&sim).intent.is_empty(), "the form cleared");
    let text = words(&mut sim);
    assert!(text.contains("SHOWN ONCE") && text.contains("?code=K7Q-M2P-9TR") && text.contains("4|"), "{text}");
    act(&mut sim, AccountAction::CopyLink);
    let copied = sim.app.world_mut().resource_mut::<chimera_ui::clipboard::Clipboard>().get().unwrap_or_default();
    assert_eq!(Some(copied), made.link);
    // Revoke the new one.
    act(&mut sim, AccountAction::Revoke(list[0].id));
    sim.ticks(2);
    let list = p(&sim).invites.clone().unwrap();
    assert!(list.len() == 3 && list.iter().all(|i| i.name != "Alex"));
    assert!(p(&sim).said.as_ref().is_some_and(|s| s.text.contains("Alex") && !s.error));
}

#[test]
fn every_staged_account_screen_draws_its_words() {
    let mut sim = sim(true);
    for scene in ["profile", "rename", "rename-taken", "avatar", "link", "sign-out", "invites", "invite-made", "player"] {
        sim.app.world_mut().resource_mut::<Console>().submit(format!("ui_scene account {scene}"));
        sim.ticks(3);
        let text = words(&mut sim);
        assert!(!text.is_empty() && !text.contains("[account."), "{scene}: {text}");
    }
    for scene in ["planning", "running", "done", "failed"] {
        sim.app.world_mut().resource_mut::<Console>().submit(format!("ui_scene downloads {scene}"));
        sim.ticks(3);
        assert!(words(&mut sim).contains("BASE FILES"), "{scene}");
    }
}

/// The first run's end starts the base files' fetch (its plan on a
/// thread); the strip shows the loading screen's lines on the main menu
/// and takes no clicks.
#[test]
fn the_first_runs_end_starts_the_base_files_and_the_strip_never_blocks() {
    let mut sim = sim(false);
    let calls = Arc::new(AtomicU32::new(0));
    let seen = calls.clone();
    sim.app.insert_resource(BaseStarter(Arc::new(move |_| {
        seen.fetch_add(1, Ordering::SeqCst);
        None
    })));
    sim.app.insert_resource(AccountShown(None));
    sim.app.world_mut().resource_mut::<Console>().submit("onboarding 1");
    sim.ticks(2);
    let link = "chimera-hub://hub.example.net:27080/3f9a0b71c2d45e6f3f9a0b71c2d45e6f3f9a0b71c2d45e6f3f9a0b71c2d45e6f?code=K7Q-M2P-9TR";
    typed(&mut sim, link);
    enter(&mut sim);
    enter(&mut sim);
    typed(&mut sim, "sam_rocket");
    step(&mut sim, 0.5);
    sim.ticks(2);
    enter(&mut sim);
    key(&mut sim, Key::Space, KeyCode::Space);
    enter(&mut sim);
    assert!(!sim.app.world().resource::<Onboarding>().active, "signed in");
    assert_eq!(sim.app.world().resource::<BaseDownload>().started, 1);
    for _ in 0..4000 {
        if calls.load(Ordering::SeqCst) == 1 && sim.app.world().resource::<BaseDownload>().status.is_none() {
            break;
        }
        chimera_world::harness::let_io_run();
        sim.ticks(1);
    }
    assert_eq!(calls.load(Ordering::SeqCst), 1, "the plan made once");
    assert_eq!(sim.app.world().resource::<BaseDownload>().status, None, "nothing lacking: no strip");
    sim.app.world_mut().resource_mut::<Console>().submit("ui_scene downloads running");
    sim.ticks(3);
    let text = words(&mut sim);
    assert!(text.contains("Downloading game files: 148.0 of 371.3 MB"), "{text}");
    let w = sim.app.world_mut();
    let picks: Vec<Pickable> = w.query_filtered::<&Pickable, With<BaseStrip>>().iter(w).cloned().collect();
    assert_eq!(picks, [Pickable::IGNORE], "the strip takes no clicks");
}

/// The hub's address is hidden until its eye key shows it, and hidden
/// again when the page opens anew; the avatar's 32 looks are in the list
/// (what you change), the username a form's field with its help line.
#[test]
fn the_hub_address_is_hidden_and_the_avatar_grid_is_in_the_list() {
    use chimera_ui::secret::Reveals;
    let mut sim = sim(false);
    open(&mut sim);
    let host = "hub.example.net";
    assert!(words(&mut sim).contains("How you sign in and how people find you"), "the username's help line");
    let avatars = {
        let w = sim.app.world_mut();
        w.query::<&AccountAction>().iter(w).filter(|a| matches!(a, AccountAction::Avatar(_))).count()
    };
    assert_eq!(avatars, 32, "the looks are in the list");
    focus(&mut sim, Row::Hub);
    assert!(!words(&mut sim).contains(host), "masked");
    sim.app.world_mut().resource_mut::<Reveals>().toggle(party_roster::account_page::REVEAL_HOST);
    sim.ticks(2);
    assert!(words(&mut sim).contains(host), "shown");
    sim.app.world_mut().resource_mut::<Console>().submit("account_page 0");
    sim.ticks(3);
    open(&mut sim);
    focus(&mut sim, Row::Hub);
    assert!(!words(&mut sim).contains(host), "hidden again");
}
