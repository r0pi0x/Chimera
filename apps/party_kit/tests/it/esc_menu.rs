//! The party's Escape menu (`party_kit::esc_menu`) and its server side
//! (`party_kit::host`): Esc opens and closes it in place of Combat Arms'
//! menu, above the match-end screen and the scoreboard; the game gets no
//! input while it is open (nor from keys held across its closing); its
//! options set the player's cvars; the host settings show only on the
//! host and set its server's cvars and the next match's rules; and the
//! vote to skip the map, over the network with three clients, passes with
//! a majority of the people (bots don't vote) and moves the server to the
//! rotation's next map, fails without one, and runs one at a time.

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use chimera_client::client::front_end::{AppMenu, MenuStatus};
use chimera_core::{
    console::{Console, ConsoleAppExt},
    core::{Bot, Intent},
};
use chimera_net::{harness::NetSim, net::vote::{VoteView, Votes}};
use chimera_sim::minigame::{ActiveMinigame, ladder::Ladder, overrides::MinigameOverrides};
use chimera_world::{harness::Sim, map::LoadedMapName};
use party_kit::{
    esc_menu::{self, Action, EscMenu, EscMenuPlugin, Page, VotePanel},
    host,
};

use crate::{
    controls::local_party_with,
    net_party::party_and,
    rotation::{Levels, ROTATING, client_loads, server_loads, FakeLoad, SERVER_LOAD},
};

fn tap(app: &mut App, key: KeyCode) {
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(key);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(key);
    keys.clear();
    app.update();
}

fn act(app: &mut App, a: Action) {
    app.world_mut().write_message(a);
    app.update();
    app.update();
}

fn menu(app: &App) -> &EscMenu {
    app.world().resource::<EscMenu>()
}

/// The local party with the Escape menu and Combat Arms' own in-game menu
/// (the apps have both), a test's `host_writeconfig` counting saves.
fn local_with_menu() -> (Sim, Entity) {
    let mut sim = Sim::new(
        combat_arms::harness::CaSim,
        (
            chimera_world::greybox::GreyboxMapPlugin,
            party_kit::PartyPartsPlugin,
            EscMenuPlugin,
            combat_arms::ui::menu::CaMenuStatePlugin,
            chimera_client::client::swap_effect::SwapEffectPlugin,
        ),
    );
    crate::gun_game::plain_rungs(sim.app.world_mut());
    chimera_sim::minigame::start(sim.app.world_mut(), party_kit::DEFAULT_MINIGAME).expect("the gun game starts");
    let (mut sim, a) = local_party_with(sim);
    sim.app
        // The shell's: a captured key is forgotten once up.
        .add_systems(PreUpdate, chimera_client::client::binds::release_captured)
        .init_resource::<Saves>()
        .console_command("host_writeconfig", "a test's: counted", |w, _| {
            w.resource_mut::<Saves>().0 += 1;
            Ok(None)
        });
    sim.app.world_mut().resource_mut::<Console>().submit("bind w +forward; bind mouse1 +attack; bind 2 slot2");
    sim.ticks(2);
    (sim, a)
}

#[derive(Resource, Default)]
struct Saves(u32);

fn grabbed(sim: &mut Sim) -> bool {
    let mut q = sim.app.world_mut().query::<&CursorOptions>();
    q.iter(sim.app.world()).any(|c| c.grab_mode != CursorGrabMode::None)
}

#[test]
fn escape_opens_the_partys_menu_and_the_game_gets_no_input_while_it_is_open() {
    let (mut sim, a) = local_with_menu();
    tap(&mut sim.app, KeyCode::Escape);
    let w = sim.app.world();
    assert!(menu(&sim.app).open && menu(&sim.app).page == Page::Main);
    assert!(!w.resource::<combat_arms::ui::menu::InGameMenu>().open, "Combat Arms' menu yields Esc");
    assert!(w.resource::<AppMenu>().open);
    assert!(w.resource::<MenuStatus>().open, "the shell sees a menu open: binds and clicks wait");
    assert!(!grabbed(&mut sim), "the mouse is free");
    // Above the HUDs, the match-end screen and the scoreboard; under the
    // console.
    assert!(esc_menu::MENU_Z > chimera_client::client::match_end::MATCH_END_Z);
    assert!(esc_menu::MENU_Z > chimera_client::client::minigame_scoreboard::SCOREBOARD_Z);
    assert!(esc_menu::VOTE_Z > chimera_client::client::match_end::MATCH_END_Z);
    // W, the mouse's turn and fire held: nothing reaches the player.
    let from = sim.position(a);
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    sim.app.world_mut().resource_mut::<AccumulatedMouseMotion>().delta = Vec2::new(300.0, 0.0);
    let yaw = sim.app.world().get::<Intent>(a).unwrap().yaw;
    for _ in 0..30 {
        sim.ticks(1);
        let i = sim.app.world().get::<Intent>(a).unwrap();
        assert!(!i.fire && i.move_axis == Vec2::ZERO, "{i:?}");
        assert_eq!(i.yaw, yaw, "the view stays");
    }
    sim.app.world_mut().resource_mut::<AccumulatedMouseMotion>().delta = Vec2::ZERO;
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().release(MouseButton::Left);
    assert!((sim.position(a) - from).xz().length() < 0.05, "the player stood");
    // Closed with W still held: the mouse is the game's again, the held W
    // stays dead until let go.
    tap(&mut sim.app, KeyCode::Escape);
    assert!(!menu(&sim.app).open && !sim.app.world().resource::<AppMenu>().open);
    assert!(grabbed(&mut sim), "the mouse is taken back");
    assert!(!sim.app.world().resource::<MenuStatus>().open);
    sim.ticks(30);
    assert!((sim.position(a) - from).xz().length() < 0.05, "a key held across the closing doesn't move");
    let mut keys = sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(KeyCode::KeyW);
    keys.clear();
    sim.ticks(2);
    sim.app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(KeyCode::KeyW);
    sim.seconds(0.5);
    assert!((sim.position(a) - from).xz().length() > 0.5, "pressed again, it runs");
}

#[test]
fn escape_goes_back_from_a_page_and_the_options_set_the_players_cvars() {
    let (mut sim, _a) = local_with_menu();
    tap(&mut sim.app, KeyCode::Escape);
    act(&mut sim.app, Action::Options);
    assert_eq!(menu(&sim.app).page, Page::Options(0));
    let tabs = esc_menu::option_tabs();
    let party = tabs.iter().position(|(n, _)| n == "Party").expect("a Party tab");
    act(&mut sim.app, Action::Tab(party));
    let index = tabs[party].1.iter().position(|s| s.cvar == "party_swap_effect").expect("the swap effect");
    let read = |sim: &mut Sim| party_kit::controls::cvar_value(sim.app.world_mut(), "party_swap_effect");
    assert_eq!(read(&mut sim).as_deref(), Some("0"));
    act(&mut sim.app, Action::Step { host: false, index, dir: 1 });
    assert_eq!(read(&mut sim).as_deref(), Some("1"));
    assert_eq!(menu(&sim.app).values.get("party_swap_effect").map(String::as_str), Some("1"), "the page shows it");
    // Every setting a tab names is a choice or a range of its own cvar.
    for (name, settings) in &tabs {
        assert!(!settings.is_empty(), "{name}");
    }
    // Esc: back to the first page, the config written once.
    tap(&mut sim.app, KeyCode::Escape);
    assert!(menu(&sim.app).open && menu(&sim.app).page == Page::Main);
    assert_eq!(sim.app.world().resource::<Saves>().0, 1, "saved on leaving the page");
    tap(&mut sim.app, KeyCode::Escape);
    assert!(!menu(&sim.app).open);
    assert_eq!(sim.app.world().resource::<Saves>().0, 1);
}

#[test]
fn the_host_settings_set_the_servers_cvars_and_the_next_matchs_rules() {
    let (mut sim, _a) = local_with_menu();
    tap(&mut sim.app, KeyCode::Escape);
    act(&mut sim.app, Action::Host);
    assert_eq!(menu(&sim.app).page, Page::Host, "the host (no server: this game) sees them");
    let settings = host::settings();
    let at = |cvar: &str| settings.iter().position(|s| s.cvar == cvar).unwrap();
    let knife = at("mp_knife_finale");
    // Minigame's (on) -> On -> Off.
    act(&mut sim.app, Action::Step { host: true, index: knife, dir: 1 });
    act(&mut sim.app, Action::Step { host: true, index: knife, dir: 1 });
    let w = sim.app.world();
    assert_eq!(w.resource::<MinigameOverrides>().keys.get("rules.knife_finale").map(String::as_str), Some("false"));
    assert!(w.resource::<Ladder>().knife_finale, "not before the next match");
    let bots = at("MaxPlayer");
    let before = party_kit::controls::cvar_value(sim.app.world_mut(), "MaxPlayer").unwrap();
    act(&mut sim.app, Action::Step { host: true, index: bots, dir: 1 });
    let after = party_kit::controls::cvar_value(sim.app.world_mut(), "MaxPlayer").unwrap();
    assert_eq!(after.parse::<u32>().unwrap(), before.parse::<u32>().unwrap() + 1, "bots at once");
    // The next match plays without the knife finale.
    let id = sim.app.world().resource::<ActiveMinigame>().def.id.clone();
    sim.app.world_mut().resource_mut::<Console>().submit(format!("minigame {id}"));
    sim.ticks(2);
    let l = sim.app.world().resource::<Ladder>();
    assert!(!l.knife_finale && l.melee_rungs == 0, "the next match's rules: no finale");
    // Back to the minigame's own: the match after plays as written.
    act(&mut sim.app, Action::Step { host: true, index: knife, dir: 1 });
    assert!(sim.app.world().resource::<MinigameOverrides>().keys.is_empty());
    sim.app.world_mut().resource_mut::<Console>().submit(format!("minigame {id}"));
    sim.ticks(2);
    assert!(sim.app.world().resource::<Ladder>().knife_finale, "the definition's own again: the finale");
}

/// Three clients on a server (no host) with the Escape menu, joined to
/// `ROTATING`'s ladder, the server's changes of level the test's, and
/// bots with them.
fn three() -> NetSim {
    let mut sim = party_and(3, Some(ROTATING), |app| {
        app.add_plugins(EscMenuPlugin);
    });
    sim.server.app.init_resource::<Levels>().add_systems(Update, server_loads);
    sim.server.app.console_command("changelevel", "changelevel <map>: a test's.", |w, a| {
        let map = a.first().cloned().unwrap_or_default();
        w.resource_mut::<Levels>().0.push(map.clone());
        w.insert_resource(LoadedMapName(map.clone()));
        w.insert_resource(FakeLoad(map, SERVER_LOAD));
        Ok(None)
    });
    for c in &mut sim.clients {
        c.app.add_systems(Update, client_loads);
    }
    sim.server.app.world_mut().resource_mut::<Console>().submit("MaxPlayer 2");
    sim.until_joined(900);
    sim.ticks(120);
    sim
}

fn voting(sim: &NetSim) -> bool {
    sim.server.app.world().resource::<Votes>().active.is_some()
}

fn client_act(sim: &mut NetSim, i: usize, a: Action) {
    sim.clients[i].app.world_mut().write_message(a);
    sim.ticks(2);
}

fn open_menu(sim: &mut NetSim, i: usize) {
    sim.clients[i].app.world_mut().resource_mut::<EscMenu>().set_open(true);
    sim.ticks(1);
}

fn panel(sim: &NetSim, i: usize) -> Vec<String> {
    sim.clients[i].app.world().resource::<VotePanel>().0.clone()
}

#[test]
fn a_skip_vote_passes_with_a_majority_of_the_people_and_goes_to_the_next_map() {
    let mut sim = three();
    let bots = sim.server.app.world_mut().query_filtered::<(), With<Bot>>().iter(sim.server.app.world()).count();
    assert!(bots > 0, "bots in the game");
    // A client sees no host settings: it isn't the host.
    open_menu(&mut sim, 1);
    assert!(!host::is_host(sim.clients[1].app.world()));
    client_act(&mut sim, 1, Action::Host);
    assert_eq!(sim.clients[1].app.world().resource::<EscMenu>().page, Page::Main, "the page isn't a client's");
    // Client 0 calls it from its menu: everyone's panel shows it.
    open_menu(&mut sim, 0);
    client_act(&mut sim, 0, Action::SkipMap);
    assert!(sim.until(60, |s| voting(s)), "the vote started");
    assert!(sim.until(60, |s| (0..3).all(|i| !panel(s, i).is_empty())), "the panel on every client");
    for i in 0..3 {
        let lines = panel(&sim, i);
        assert!(lines[0].ends_with("wants to skip the map"), "client {i}: {lines:?}");
        let v = sim.clients[i].app.world().resource::<VoteView>().active.clone().unwrap();
        assert_eq!(v.eligible, 3, "client {i}: the people, not the bots");
        assert!(lines[2].contains("2 needed"), "client {i}: a majority of three: {lines:?}");
        assert_eq!(lines[1], if i == 0 { "You voted yes" } else { "F1: yes    F2: no" }, "client {i}");
    }
    // One at a time: another call is refused, and the menu says why.
    open_menu(&mut sim, 2);
    client_act(&mut sim, 2, Action::SkipMap);
    assert!(sim.until(40, |s| s.clients[2].app.world().resource::<EscMenu>().notice.is_some()));
    let notice = sim.clients[2].app.world().resource::<EscMenu>().notice.clone().unwrap();
    assert!(notice.contains("already running"), "{notice}");
    // Yes from the menu (client 1), no (client 2): 2 of 3, passed.
    client_act(&mut sim, 1, Action::VoteYes);
    sim.clients[2].app.world_mut().resource_mut::<Console>().submit("vote option2");
    assert!(sim.until(60, |s| !voting(s)), "the vote ended");
    assert_eq!(sim.server.app.world().resource::<Votes>().passed, 1);
    assert!(sim.until(60, |s| panel(s, 1).first().is_some_and(|l| l == "Vote passed: skipping the map")));
    assert!(sim.until(300, |s| s.server.app.world().resource::<Levels>().0 == ["test:next"]), "the next map");
}

#[test]
fn a_skip_vote_without_a_majority_fails_and_keeps_the_map() {
    let mut sim = three();
    open_menu(&mut sim, 0);
    client_act(&mut sim, 0, Action::SkipMap);
    assert!(sim.until(60, |s| voting(s)));
    sim.ticks(10);
    for i in [1, 2] {
        sim.clients[i].app.world_mut().resource_mut::<Console>().submit("vote option2");
    }
    assert!(sim.until(60, |s| !voting(s)));
    let w = sim.server.app.world();
    assert_eq!((w.resource::<Votes>().passed, w.resource::<Votes>().failures), (0, 1));
    assert!(sim.until(60, |s| panel(s, 0).first().is_some_and(|l| l.starts_with("Vote failed"))));
    sim.ticks(200);
    assert!(sim.server.app.world().resource::<Levels>().0.is_empty(), "the map stays");
}

/// Under Signal's theme (`esc_signal`): ↑↓ move the selection over the
/// entries that act, Enter presses it; the host's Disconnect asks first
/// with Stay selected, so a second Enter stays; drawn on the kit.
#[test]
fn signal_keys_move_the_selection_and_stay_comes_first() {
    let (mut sim, _a) = local_with_menu();
    sim.app
        .insert_resource(chimera_ui::Theme::from_toml(include_str!("../../../ultimate_gun_game/data/theme.toml")).unwrap())
        .insert_resource(chimera_ui::UiUnits::new(1080.0, 1.0))
        .init_resource::<chimera_ui::IconImages>();
    tap(&mut sim.app, KeyCode::Escape);
    assert_eq!(menu(&sim.app).focus, 0);
    tap(&mut sim.app, KeyCode::ArrowUp);
    let entries = esc_menu::shown_entries(true, false, false);
    assert_eq!(menu(&sim.app).focus, entries.len() - 1, "↑ wraps to the last entry");
    assert_eq!(entries.last().unwrap().action, Some(Action::Quit));
    tap(&mut sim.app, KeyCode::ArrowDown);
    tap(&mut sim.app, KeyCode::ArrowDown);
    assert_eq!(menu(&sim.app).focus, 1);
    tap(&mut sim.app, KeyCode::Enter);
    assert_eq!(menu(&sim.app).page, Page::Options(0), "Enter pressed Options");
    // The vote entry is off while a vote runs.
    let voting = esc_menu::shown_entries(true, false, true);
    assert!(voting.iter().any(|e| e.action.is_none() && e.label.contains("Vote")));
    // The confirm: Stay first.
    sim.app.world_mut().resource_mut::<EscMenu>().page = Page::ConfirmDisconnect(2);
    sim.app.world_mut().resource_mut::<EscMenu>().focus = 0;
    tap(&mut sim.app, KeyCode::Enter);
    assert_eq!(menu(&sim.app).page, Page::Main, "Enter on Stay stays");
    assert!(menu(&sim.app).open);
}
