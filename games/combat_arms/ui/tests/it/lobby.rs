//! specs/combat_arms/lobby.md on the screens (`ui::lobby`, headless on
//! `CaSim` with the screens' state plugin and no window): the controls the
//! layouts name (the spec's names), the room list and its header, filter
//! and join clicks (L), the creation popup (C), the waiting room (R, O,
//! T, S), the message and password boxes, the inventory (E) and the way a
//! match starts and ends on them (F, S, G, X). The rules themselves are
//! tests/it/lobby.rs's.
//!
//! Case 59 (Esc in a match stops the player and the match goes on) is the
//! UI system's `t26_input_under_the_esc_menu` (ui_system.rs): the lobby
//! adds only EXIT's way back, tested here (case 60).

use bevy::prelude::*;
use chimera_core::core::{Connecting, LocalPlayer};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{game::{
        lobby::{
            CaLobbyClient, CaLobbyHost,
            data::LobbyData,
            host::CaInMatch,
            room::*,
        },
        rules::{CaMatch, Phase},
    }, harness::CaSim};
use combat_arms_ui::{
        CaUi,
        lobby::{CaLobbyStatePlugin, LobbyScreen, Page},
        menu::{CaMenuStatePlugin, InGameMenu, MenuPointer},
    };

fn sim() -> Sim {
    let mut s = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaMenuStatePlugin, CaLobbyStatePlugin));
    });
    {
        let mut host = s.app.world_mut().resource_mut::<CaLobbyHost>();
        host.enabled = true;
        host.tick_secs = 0.1;
        host.result_secs = 0.3;
        host.drop_leavers = false;
    }
    let me = s.spawn_character(chimera_world::greybox::SPAWNS[0], combat_arms::game::movement::ID);
    s.app.world_mut().entity_mut(me).insert(LocalPlayer);
    s.seconds(0.5);
    s
}

/// `sim()` with the options screen's state too.
fn sim_with_options() -> Sim {
    let mut s = Sim::with(CaSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CaMenuStatePlugin, CaLobbyStatePlugin, combat_arms_ui::options::CaOptionsStatePlugin));
    });
    {
        let mut host = s.app.world_mut().resource_mut::<CaLobbyHost>();
        host.enabled = true;
        host.tick_secs = 0.1;
        host.result_secs = 0.3;
        host.drop_leavers = false;
    }
    combat_arms::game::options::flow::load(s.app.world_mut(), None);
    let me = s.spawn_character(chimera_world::greybox::SPAWNS[0], combat_arms::game::movement::ID);
    s.app.world_mut().entity_mut(me).insert(LocalPlayer);
    s.seconds(0.5);
    s
}

fn screen(sim: &Sim) -> &LobbyScreen {
    sim.app.world().resource::<LobbyScreen>()
}

fn ui(sim: &Sim) -> CaUi {
    sim.app.world().resource::<CaUi>().clone()
}

fn client(sim: &Sim) -> &combat_arms::game::lobby::client::ClientLobby {
    &sim.app.world().resource::<CaLobbyClient>().0
}

fn host(sim: &mut Sim) -> Mut<'_, CaLobbyHost> {
    sim.app.world_mut().resource_mut::<CaLobbyHost>()
}

fn centre(sim: &Sim, name: &str) -> IVec2 {
    let r = screen(sim).rect_of(name, &ui(sim)).unwrap_or_else(|| panic!("no control {name}"));
    IVec2::new((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2)
}

fn press_release(sim: &mut Sim) {
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().press(MouseButton::Left);
    sim.app.update();
    {
        let mut b = sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
        b.clear();
        b.release(MouseButton::Left);
    }
    sim.app.update();
    sim.app.world_mut().resource_mut::<ButtonInput<MouseButton>>().clear();
}

/// A left-click on a control's centre.
fn click(sim: &mut Sim, name: &str) {
    let p = centre(sim, name);
    sim.app.world_mut().resource_mut::<MenuPointer>().0 = Some(p);
    sim.app.update();
    press_release(sim);
}

fn text(sim: &Sim, name: &str) -> String {
    screen(sim).text_of(name, &ui(sim)).unwrap_or_else(|| panic!("no text {name}"))
}

fn label(sim: &Sim, name: &str) -> String {
    screen(sim).label_of(name, &ui(sim)).unwrap_or_else(|| panic!("no button {name}"))
}

fn shown(sim: &Sim, name: &str) -> bool {
    screen(sim).shown(name).unwrap_or_else(|| panic!("no control {name}"))
}

fn type_text(sim: &mut Sim, s: &str) {
    sim.app.world_mut().resource_mut::<LobbyScreen>().type_text(s);
    sim.app.update();
}

fn press_key(sim: &mut Sim, k: combat_arms_ui::lobby::Typed) {
    sim.app.world_mut().resource_mut::<LobbyScreen>().press(k);
    sim.app.update();
}

/// A made-up player in the lobby (the screen's player is player 0).
fn other(sim: &mut Sim, id: PlayerId) {
    host(sim).lobby.request(id, Request::Hello { name: format!("P{id}") }).unwrap();
}

fn create_spec(mode: &str, players: u32, name: &str, password: &str) -> CreateSpec {
    let data = LobbyData::built_in();
    let m = data.mode_by_key(mode).unwrap();
    CreateSpec {
        name: name.into(),
        password: password.into(),
        mode: m.id,
        pure_combat: false,
        map: None,
        goal: m.goal_default(),
        players,
        arms: [0, 0],
    }
}

/// Room `id` made by a made-up player.
fn their_room(sim: &mut Sim, id: PlayerId, spec: CreateSpec) -> RoomId {
    other(sim, id);
    host(sim).lobby.request(id, Request::Create(spec)).unwrap();
    let room = host(sim).lobby.room_of(id).unwrap();
    sim.seconds(0.3);
    room
}

/// Click the inventory list's row for `name`, paging to it.
fn pick_item(sim: &mut Sim, name: &str) {
    for _ in 0..6 {
        let items = combat_arms_ui::lobby::eligible(screen(sim));
        let at = items.iter().position(|(_, n)| n == name).unwrap_or_else(|| panic!("no item {name}"));
        let (page, row) = (at / 12, at % 12);
        if screen(sim).inv_page == page {
            click(sim, &format!("item_{row}"));
            return;
        }
        click(sim, if screen(sim).inv_page < page { "list_next" } else { "list_prev" });
    }
    panic!("could not page to {name}");
}

fn enter_room_by_clicks(sim: &mut Sim, row: usize) {
    click(sim, &format!("row_{row}"));
    click(sim, "button_Join");
    sim.seconds(0.3);
}

// ---------------------------------------------------------------------------

#[test]
fn our_layouts_have_every_control_the_screens_bind() {
    let s = sim();
    let screen = screen(&s);
    let names = [
        // GAMELOBBY.XML
        "gameLobby", "button_create", "button_Join", "Check_join", "Check_pw", "Check_official", "mode_combo",
        "map_combo", "Sort_Num", "Sort_Title", "Sort_Map", "Sort_Mode", "Sort_Mutation", "Sort_Arms",
        "Sort_Players", "Sort_Status", "row_0", "row_17", "button_lobby", "button_gameroom", "button_character",
        // GAMEROOM.XML
        "gameroom", "Btn_Alpha", "Btn_Bravo", "button_teamChange", "button_game_ready", "button_game_start",
        "button_invite", "alpha_0", "alpha_7", "bravo_0", "bravo_7", "game setting", "drop_mode", "drop_map",
        "drop_goal", "drop_players", "drop_arms", "drop_second_arms", "Join_On", "Join_Off", "TeamChange_On",
        "FriendlyFire_On", "KillCam_Off", "Perk_On", "Mutation_Off",
        // POPUP_CREATEROOM.XML
        "CreateWnd", "tab_multi", "tab_single", "Edit_RoomName", "UsePW", "Edit_RoomPW", "Combo_Players",
        "CreateRoom_ok", "button_cancel",
        // POPUP_LOCKROOM.XML
        "LockGameRoom", "input_password", "button_ok",
        // INVENTORY.XML and its panels
        "inventory", "tab_Normal", "tab_Official", "equip_primary", "equip_secondary", "equip_Melee",
        "equip_Support", "equip_backpackA", "equip_backpackD", "equip_head", "equip_perk0", "equip_perk3",
    ];
    for n in names {
        assert!(screen.find(n).is_some(), "control {n}");
    }
}

#[test]
fn the_screen_opens_on_the_room_list_when_the_lobby_answers() {
    let s = sim();
    assert!(client(&s).active);
    assert!(screen(&s).open);
    assert_eq!(screen(&s).page, Page::List);
    assert!(shown(&s, "button_create"));
    assert!(screen(&s).desk.sheets.iter().filter(|x| x.shown).count() >= 1);
    // The player waits out of the match.
    let me = s.app.world().resource::<CaLobbyClient>().0.me;
    assert_eq!(me, 0);
}

#[test]
fn case_1_rows_show_the_rooms_greyed_when_they_cannot_be_entered() {
    let mut s = sim();
    let a = their_room(&mut s, 11, create_spec("TDM", 16, "Alpha", ""));
    their_room(&mut s, 12, create_spec("FFA", 8, "bravo", "pw"));
    assert_eq!(text(&s, "row_0_c0"), "1");
    assert_eq!(text(&s, "row_0_c1"), "Alpha");
    assert_eq!(text(&s, "row_0_c2"), "OIL RIG");
    assert_eq!(text(&s, "row_0_c3"), "Elimination");
    assert_eq!(text(&s, "row_0_c6"), "1/16");
    assert_eq!(text(&s, "row_0_c7"), "Waiting");
    assert_eq!(text(&s, "row_1_c1"), "[L] bravo", "the lock");
    assert_eq!(text(&s, "row_1_c3"), "One Man Army");
    assert!(!shown(&s, "row_2"), "no third room");
    assert_eq!(a, 0);
}

#[test]
fn case_2_and_3_filters_and_the_header_sort_by_click() {
    let mut s = sim();
    their_room(&mut s, 11, create_spec("TDM", 16, "bravo", ""));
    their_room(&mut s, 12, create_spec("FFA", 8, "Alpha", "pw"));
    click(&mut s, "Sort_Title");
    assert_eq!(text(&s, "row_0_c1"), "[L] Alpha");
    click(&mut s, "Sort_Title");
    assert_eq!(text(&s, "row_0_c1"), "bravo", "the second click flips it");
    click(&mut s, "Check_pw");
    assert!(!shown(&s, "row_1"), "No Password hides the locked room");
    assert_eq!(text(&s, "list_count"), "1 rooms");
    assert_eq!(screen(&s).selected("Check_pw"), Some(true));
    click(&mut s, "Check_pw");
    click(&mut s, "mode_combo");
    assert_eq!(label(&s, "mode_combo"), "Elimination");
    assert_eq!(text(&s, "list_count"), "1 rooms");
}

#[test]
fn case_6_join_with_no_selection_or_a_full_room_shows_a_message_box() {
    let mut s = sim();
    click(&mut s, "button_Join");
    assert!(!screen(&s).enabled("button_Join").unwrap(), "JOIN needs a selected row");
    // A room the client refuses.
    their_room(&mut s, 11, create_spec("TDM", 2, "tiny", ""));
    other(&mut s, 12);
    host(&mut s).lobby.request(12, Request::Enter { room: 0, password: String::new() }).unwrap();
    s.seconds(0.3);
    click(&mut s, "row_0");
    click(&mut s, "button_Join");
    assert!(client(&s).message.as_deref().unwrap().starts_with("You cannot enter the room"));
    assert!(screen(&s).desk.sheets.iter().any(|x| x.shown && x.name == "MessageBox"));
    assert_eq!(text(&s, "message_text"), "You cannot enter the room that you have currently selected.");
    click(&mut s, "button_msg_ok");
    assert!(client(&s).message.is_none());
    assert_eq!(screen(&s).page, Page::List);
}

#[test]
fn case_7_a_locked_room_asks_for_the_password() {
    let mut s = sim();
    their_room(&mut s, 11, create_spec("TDM", 16, "locked", "ab"));
    click(&mut s, "row_0");
    click(&mut s, "button_Join");
    assert!(screen(&s).password_room.is_some(), "the box opens");
    type_text(&mut s, "AB");
    assert_eq!(label(&s, "input_password"), "**");
    press_key(&mut s, combat_arms_ui::lobby::Typed::Enter);
    s.seconds(0.3);
    assert_eq!(client(&s).message.as_deref(), Some("You enter the wrong password,please retry."));
    assert_eq!(screen(&s).page, Page::List, "still in the list");
    click(&mut s, "button_msg_ok");
    click(&mut s, "button_Join");
    type_text(&mut s, "ab");
    press_key(&mut s, combat_arms_ui::lobby::Typed::Enter);
    s.seconds(0.3);
    assert_eq!(screen(&s).page, Page::Room, "the waiting room opens");
    assert_eq!(client(&s).room.as_ref().unwrap().settings.name, "locked");
}

#[test]
fn case_9_the_creation_popup_opens_on_the_modes_defaults() {
    let mut s = sim();
    click(&mut s, "button_create");
    assert!(screen(&s).desk.sheets.iter().any(|x| x.shown && x.name == "CreateWnd"));
    s.seconds(0.1);
    assert_eq!(label(&s, "drop_mode"), "Elimination");
    assert_eq!(label(&s, "drop_map"), "OIL RIG");
    assert_eq!(label(&s, "drop_goal"), "100 Kills (Default)");
    assert_eq!(label(&s, "Combo_Players"), "8 vs 8");
    assert_eq!(label(&s, "drop_arms"), "All Permitted");
    assert_eq!(label(&s, "drop_second_arms"), "-");
    assert_eq!(screen(&s).enabled("drop_second_arms"), Some(false));
    assert_eq!(screen(&s).selected("UsePW"), Some(false));
    assert!(LobbyData::built_in().titles.contains(&label(&s, "Edit_RoomName").trim_end_matches('_').to_string()));
}

#[test]
fn case_10_and_11_choosing_modes_in_the_popup() {
    let mut s = sim();
    click(&mut s, "button_create");
    // Cycle the Mode drop-down to Fireteam (third).
    click(&mut s, "drop_mode");
    click(&mut s, "drop_mode");
    s.seconds(0.1);
    assert_eq!(label(&s, "drop_mode"), "Fireteam");
    assert_eq!(text(&s, "goal_label"), "Difficulty");
    assert_eq!(label(&s, "drop_goal"), "HARD");
    assert_eq!(label(&s, "Combo_Players"), "8 Player(s)");
    assert_eq!(screen(&s).enabled("drop_arms"), Some(false), "1st Arms is disabled at All Permitted");
    click(&mut s, "drop_mode");
    s.seconds(0.1);
    assert_eq!(label(&s, "drop_mode"), "One Man Army");
    assert_eq!(label(&s, "drop_map"), "JUNK FLEA");
}

#[test]
fn case_12_a_blank_name_opens_the_message_box_and_sends_nothing() {
    let mut s = sim();
    click(&mut s, "button_create");
    click(&mut s, "Edit_RoomName");
    for _ in 0..40 {
        press_key(&mut s, combat_arms_ui::lobby::Typed::Backspace);
    }
    type_text(&mut s, "   ");
    click(&mut s, "CreateRoom_ok");
    assert_eq!(client(&s).message.as_deref(), Some("Please enter the room name."));
    assert!(screen(&s).create.is_some(), "the popup stays open");
    s.seconds(0.3);
    assert!(host(&mut s).lobby.rooms.is_empty(), "nothing was sent");
}

#[test]
fn case_13_ok_creates_the_room_and_the_player_is_its_host_on_alpha() {
    let mut s = sim();
    click(&mut s, "button_create");
    click(&mut s, "drop_mode");
    click(&mut s, "drop_mode");
    click(&mut s, "drop_mode");
    click(&mut s, "drop_mode");
    s.seconds(0.1);
    assert_eq!(label(&s, "drop_mode"), "Capture the Flag");
    click(&mut s, "UsePW");
    click(&mut s, "Edit_RoomPW");
    type_text(&mut s, "pw");
    click(&mut s, "CreateRoom_ok");
    s.seconds(0.3);
    assert_eq!(screen(&s).page, Page::Room);
    let room = client(&s).room.clone().unwrap();
    assert_eq!(room.host, 0);
    assert_eq!(room.seat(0).unwrap().team, Team::Alpha);
    assert_eq!(room.settings.password, "pw");
    assert_eq!(label(&s, "drop_goal"), "5 Points (Default)");
    assert_eq!(screen(&s).selected("Join_On"), Some(true));
    assert_eq!(screen(&s).selected("FriendlyFire_Off"), Some(true));
    assert_eq!(screen(&s).selected("KillCam_Off"), Some(true));
    assert_eq!(screen(&s).selected("Perk_On"), Some(true));
    assert_eq!(text(&s, "alpha_0_c0"), "* Player", "the host's crown");
}

#[test]
fn case_17_the_single_tab_hides_the_password_and_arms_rows() {
    let mut s = sim();
    click(&mut s, "button_create");
    click(&mut s, "tab_single");
    s.seconds(0.1);
    assert_eq!(label(&s, "drop_mode"), "Elimination (bot play)");
    assert_eq!(label(&s, "Combo_Players"), "1 vs 8");
    assert_eq!(text(&s, "arms_label"), "Difficulty");
    assert_eq!(label(&s, "drop_arms"), "HARD");
    assert_eq!(label(&s, "drop_goal"), "40 Kills (Default)");
    assert!(!shown(&s, "Edit_RoomPW") && !shown(&s, "UsePW") && !shown(&s, "drop_second_arms"));
}

/// The player in a room of `mode` with `joiners` made-up players (ids
/// from 21), the player as the host.
fn host_room(s: &mut Sim, mode: &str, joiners: u64) -> RoomId {
    click(s, "button_create");
    s.app.world_mut().resource_mut::<LobbyScreen>().create.as_mut().unwrap().set_mode(LobbyData::built_in().mode_by_key(mode).unwrap().id);
    click(s, "CreateRoom_ok");
    s.seconds(0.3);
    let room = client(s).room.as_ref().unwrap().id;
    for p in 21..21 + joiners {
        other(s, p);
        host(s).lobby.request(p, Request::Enter { room, password: String::new() }).unwrap();
    }
    s.seconds(0.3);
    room
}

#[test]
fn the_waiting_room_shows_both_teams_and_the_host_sees_start() {
    let mut s = sim();
    host_room(&mut s, "TDM", 2);
    assert_eq!(text(&s, "alpha_0_c0"), "* Player");
    assert_eq!(text(&s, "bravo_0_c0"), "P21");
    assert_eq!(text(&s, "alpha_1_c0"), "P22");
    assert!(!shown(&s, "alpha_2"));
    assert!(shown(&s, "button_game_start") && !shown(&s, "button_game_ready"));
    // A non-host sees READY.
    host(&mut s).lobby.request(0, Request::EntrustHost(21)).unwrap();
    s.seconds(0.3);
    assert!(shown(&s, "button_game_ready") && !shown(&s, "button_game_start"));
    assert_eq!(screen(&s).enabled("drop_mode"), Some(false), "only the host changes the settings");
    assert_eq!(screen(&s).enabled("Join_On"), Some(false));
}

#[test]
fn case_20_to_22_team_buttons_and_the_one_team_rooms() {
    let mut s = sim();
    host_room(&mut s, "TDM", 3);
    assert!(shown(&s, "Btn_Alpha"));
    click(&mut s, "Btn_Bravo");
    s.seconds(0.3);
    assert_eq!(client(&s).seat().unwrap().team, Team::Bravo);
    click(&mut s, "button_teamChange");
    s.seconds(0.3);
    assert_eq!(client(&s).seat().unwrap().team, Team::Alpha);
    // One-team modes: the team buttons are hidden and Team Change is
    // disabled.
    let mut s = sim();
    host_room(&mut s, "FFA", 1);
    assert!(!shown(&s, "Btn_Alpha") && !shown(&s, "Btn_Bravo"));
    assert_eq!(screen(&s).enabled("button_teamChange"), Some(false));
}

#[test]
fn case_23_to_26_the_host_changes_options_by_click() {
    let mut s = sim();
    let room = host_room(&mut s, "TDM", 1);
    host(&mut s).lobby.request(21, Request::Ready(true)).unwrap();
    s.seconds(0.3);
    click(&mut s, "FriendlyFire_On");
    s.seconds(0.3);
    assert!(client(&s).room.as_ref().unwrap().settings.friendly_fire);
    assert_eq!(screen(&s).selected("FriendlyFire_On"), Some(true));
    assert_eq!(client(&s).room.as_ref().unwrap().seat(21).unwrap().state, PlayerState::Ready, "ready states stay");
    // The map: cycles to the next open one, ready kept.
    click(&mut s, "drop_map");
    s.seconds(0.3);
    assert_ne!(client(&s).room.as_ref().unwrap().settings.map, 0);
    assert_eq!(client(&s).room.as_ref().unwrap().seat(21).unwrap().state, PlayerState::Ready);
    // 1st Arms un-readies everyone and says so in the room chat.
    click(&mut s, "drop_arms");
    s.seconds(0.3);
    assert_eq!(client(&s).room.as_ref().unwrap().seat(21).unwrap().state, PlayerState::Waiting);
    assert!(client(&s).chat.iter().any(|l| l.text.starts_with("Arms Setting changed to : ") && l.system));
    // Options the mode refuses are disabled (One Man Army: no friendly fire).
    let mut s = sim();
    host_room(&mut s, "FFA", 1);
    assert_eq!(screen(&s).enabled("FriendlyFire_On"), Some(false));
    assert_eq!(screen(&s).enabled("Join_On"), Some(true));
    let _ = room;
}

#[test]
fn case_28_a_mode_with_too_few_seats_shows_the_clients_message() {
    let mut s = sim();
    host_room(&mut s, "TDM", 9);
    // Cycle the Mode to Fireteam (max 8): refused by the client.
    click(&mut s, "drop_mode");
    click(&mut s, "drop_mode");
    s.seconds(0.2);
    assert_eq!(
        client(&s).message.as_deref(),
        Some("You have too many players to change the Mode to Fireteam (Max: 8 players)")
    );
    assert_eq!(client(&s).room.as_ref().unwrap().settings.mode, 1, "the first click went through to Search & Destroy");
}

#[test]
fn case_31_to_36_start_the_countdown_and_go() {
    let mut s = sim();
    let room = host_room(&mut s, "TDM", 1);
    // Not enough ready players: the message box.
    click(&mut s, "button_game_start");
    assert_eq!(client(&s).message.as_deref(), Some("There aren't enough players to start the game."));
    click(&mut s, "button_msg_ok");
    host(&mut s).lobby.request(21, Request::Ready(true)).unwrap();
    s.seconds(0.3);
    click(&mut s, "button_game_start");
    s.seconds(0.3);
    assert_eq!(client(&s).room.as_ref().unwrap().state, RoomState::Starting);
    assert_eq!(screen(&s).enabled("button_game_start"), Some(false), "buttons disabled while starting");
    // The countdown in the room chat, then the player goes in.
    s.seconds(0.1);
    assert!(client(&s).chat.iter().any(|l| l.text == "Game starting in... 5 seconds." && l.system));
    s.seconds(1.5);
    assert!(!screen(&s).open, "in the match the lobby screen is gone");
    let me = s.app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().single(s.app.world()).unwrap();
    assert!(s.app.world().get::<CaInMatch>(me).is_some());
    assert!(s.app.world().get::<Connecting>(me).is_none());
    let _ = room;
}

#[test]
fn case_52_ready_with_a_loadout_the_room_refuses_shows_the_message() {
    let mut s = sim();
    // A made-up host makes a Sniper Only room; the screen's player joins.
    let mut spec = create_spec("TDM", 16, "snipers", "");
    spec.arms = [6, 0];
    their_room(&mut s, 11, spec);
    enter_room_by_clicks(&mut s, 0);
    assert_eq!(screen(&s).page, Page::Room);
    assert!(shown(&s, "button_game_ready"));
    click(&mut s, "button_game_ready");
    let m = client(&s).message.clone().unwrap();
    assert!(m.contains("sniper rifle") && m.contains("Sniper Only"), "{m}");
    assert_eq!(client(&s).seat().unwrap().state, PlayerState::Waiting, "READY was not sent");
    // With a sniper rifle in the primary slot it can ready.
    click(&mut s, "button_msg_ok");
    click(&mut s, "button_character");
    click(&mut s, "equip_primary");
    pick_item(&mut s, "L96A1");
    click(&mut s, "button_equip");
    assert_eq!(label(&s, "equip_primary"), "L96A1");
    click(&mut s, "button_inv_back");
    s.seconds(0.2);
    click(&mut s, "button_game_ready");
    s.seconds(0.3);
    assert_eq!(client(&s).seat().unwrap().state, PlayerState::Ready);
}

#[test]
fn case_48_to_50_the_inventory_screen() {
    let mut s = sim();
    click(&mut s, "button_character");
    s.seconds(0.1);
    assert_eq!(screen(&s).page, Page::Inventory);
    assert_eq!(label(&s, "equip_primary"), "M16A3");
    assert_eq!(label(&s, "equip_secondary"), "Beretta M92FS");
    assert_eq!(label(&s, "equip_Melee"), "M9");
    assert_eq!(label(&s, "equip_Support"), "M67");
    assert_eq!(label(&s, "equip_head"), "Black Cap");
    assert_ne!(label(&s, "equip_gear_backpack"), "-", "the Standard Backpack");
    assert_eq!(label(&s, "equip_backpackA"), "-");
    assert_eq!(label(&s, "equip_backpackB"), "(no slot)", "B to D are unusable");
    assert_eq!(screen(&s).enabled("equip_backpackB"), Some(false));
    // Release the primary: refused, still M16A3.
    click(&mut s, "equip_primary");
    click(&mut s, "button_release");
    assert_eq!(text(&s, "inv_note"), "A weapon slot cannot be empty.");
    assert_eq!(label(&s, "equip_primary"), "M16A3");
    // A knife into backpack A is not in its list; a pistol is accepted.
    click(&mut s, "equip_backpackA");
    let items = combat_arms_ui::lobby::eligible(screen(&s));
    assert!(items.iter().all(|(_, n)| !n.contains("Knife") && n != "M9"), "no knives for the backpack");
    pick_item(&mut s, "Desert Eagle");
    click(&mut s, "button_equip");
    assert_eq!(label(&s, "equip_backpackA"), "Desert Eagle");
    // The same pistol in the secondary: refused.
    click(&mut s, "equip_secondary");
    pick_item(&mut s, "Desert Eagle");
    click(&mut s, "button_equip");
    assert_eq!(text(&s, "inv_note"), "You cannot equip two of the same item at the same time.");
    assert_eq!(label(&s, "equip_secondary"), "Beretta M92FS");
    // The Pure Combat tab is its own set, with no backpack.
    click(&mut s, "tab_Official");
    assert_eq!(label(&s, "equip_backpackA"), "(no slot)");
    // Abilities: the list is the perks; the choice reaches the gear's
    // settings.
    click(&mut s, "tab_Normal");
    click(&mut s, "equip_perk0");
    click(&mut s, "item_0");
    click(&mut s, "button_equip");
    assert_ne!(label(&s, "equip_perk0"), "-");
    click(&mut s, "button_inv_back");
    let perks = s.app.world().resource::<combat_arms::game::gear::CaOwnGear>().perks.clone();
    assert!(!perks.is_empty(), "ca_perks was set: {perks}");
    let items = s.app.world().resource::<combat_arms::game::gear::CaOwnGear>().items.clone();
    assert!(items.contains("20"), "the equipped items: {items}");
    // The lobby has the loadout for the next match.
    s.seconds(0.3);
    assert!(host(&mut s).lobby.loadouts.contains_key(&0));
    assert_eq!(screen(&s).page, Page::List);
}

#[test]
fn case_56_the_expired_notice_on_the_next_room_visit() {
    let mut s = sim();
    their_room(&mut s, 11, create_spec("TDM", 16, "r", ""));
    enter_room_by_clicks(&mut s, 0);
    host(&mut s).lobby.items_expired(0, &[911]);
    s.seconds(0.3);
    assert!(client(&s).message.as_deref().unwrap().starts_with("These items have expired: "));
}

/// A click on a button of the in-game menu.
fn click_menu(sim: &mut Sim, name: &str) {
    let p = {
        let m = sim.app.world().resource::<InGameMenu>();
        let id = m.layout().find(name).unwrap_or_else(|| panic!("no menu button {name}"));
        let r = m.layout().hit_rect(id, &ui(sim).ctx());
        IVec2::new((r.x0 + r.x1) / 2, (r.y0 + r.y1) / 2)
    };
    sim.app.world_mut().resource_mut::<MenuPointer>().0 = Some(p);
    sim.app.update();
    press_release(sim);
}

#[test]
fn case_58_and_60_the_match_ends_and_exit_returns_to_the_room() {
    let mut s = sim();
    host_room(&mut s, "TDM", 1);
    host(&mut s).lobby.request(21, Request::Ready(true)).unwrap();
    s.seconds(0.3);
    click(&mut s, "button_game_start");
    s.seconds(2.0);
    assert!(!screen(&s).open);
    // Esc's menu: EXIT returns to the room; the game does not quit.
    s.app.world_mut().resource_mut::<InGameMenu>().set_open(true);
    s.app.update();
    click_menu(&mut s, "Lobby");
    s.seconds(0.4);
    assert!(!s.app.world().resource::<InGameMenu>().open);
    assert!(screen(&s).open, "the room shows again");
    assert_eq!(screen(&s).page, Page::Room);
    assert_eq!(client(&s).seat().unwrap().state, PlayerState::Waiting);
    let me = s.app.world_mut().query_filtered::<Entity, With<LocalPlayer>>().single(s.app.world()).unwrap();
    assert!(s.app.world().get::<CaInMatch>(me).is_none(), "held out of the match again");
    assert!(s.app.world().get::<Connecting>(me).is_some());
    // The match ends: the same room, waiting, ready states lost.
    s.app.world_mut().resource_mut::<CaMatch>().phase = Phase::MatchOver;
    s.seconds(1.0);
    assert_eq!(client(&s).room.as_ref().unwrap().state, RoomState::Waiting);
    assert!(client(&s).room.as_ref().unwrap().seats.iter().all(|x| x.state == PlayerState::Waiting));
    // And the room starts another.
    host(&mut s).lobby.request(21, Request::Ready(true)).unwrap();
    s.seconds(0.3);
    click(&mut s, "button_game_start");
    s.seconds(2.0);
    assert!(!screen(&s).open);
    assert!(s.app.world().get::<CaInMatch>(me).is_some());
}

#[test]
fn leaving_the_room_goes_back_to_the_list() {
    let mut s = sim();
    host_room(&mut s, "TDM", 1);
    click(&mut s, "button_lobby_room");
    s.seconds(0.3);
    assert_eq!(screen(&s).page, Page::List);
    assert!(client(&s).room.is_none());
    // The room (the made-up player's) is still in the list or gone with
    // its host; either way the list answers.
    assert!(shown(&s, "button_create"));
}

#[test]
fn room_chat_goes_through_the_repeat_lock() {
    let mut s = sim();
    host_room(&mut s, "TDM", 1);
    click(&mut s, "chat_input");
    for _ in 0..3 {
        type_text(&mut s, "hello");
        press_key(&mut s, combat_arms_ui::lobby::Typed::Enter);
        s.seconds(0.3);
    }
    let said = client(&s).chat.iter().filter(|l| l.text.ends_with("hello") && !l.system).count();
    assert_eq!(said, 3);
    // The fourth identical line is locked by the chat's own rule.
    type_text(&mut s, "hello");
    press_key(&mut s, combat_arms_ui::lobby::Typed::Enter);
    s.seconds(0.3);
    let said = client(&s).chat.iter().filter(|l| l.text.ends_with("hello") && !l.system).count();
    assert_eq!(said, 3);
}

/// Corrections 2026-10-22 (options 1): the lobby's start menu opens the
/// options screen as a modal over the room, and closing it changes nothing
/// of the player's room state.
#[test]
fn corr_options_open_from_the_start_menu_without_touching_the_room() {
    let mut s = sim_with_options();
    their_room(&mut s, 11, create_spec("TDM", 16, "room", ""));
    enter_room_by_clicks(&mut s, 0);
    click(&mut s, "button_game_ready");
    s.seconds(0.3);
    assert_eq!(client(&s).seat().unwrap().state, PlayerState::Ready);
    assert!(!screen(&s).start_menu, "the menu is closed");
    click(&mut s, "StartButton");
    assert!(screen(&s).start_menu);
    click(&mut s, "button_start_option");
    s.seconds(0.2);
    let opts = s.app.world().resource::<combat_arms::game::options::CaOptions>();
    assert!(opts.open, "the options screen is up");
    assert_eq!(opts.tab, combat_arms::game::options::Tab::Graphic);
    assert!(!screen(&s).start_menu);
    // The room underneath takes no clicks while it is up.
    let before = client(&s).outbox.len();
    click(&mut s, "button_game_ready");
    assert_eq!(client(&s).outbox.len(), before, "modal");
    combat_arms::game::options::flow::cancel(s.app.world_mut());
    s.seconds(0.3);
    assert_eq!(screen(&s).page, Page::Room);
    assert_eq!(client(&s).seat().unwrap().state, PlayerState::Ready, "still ready");
}

/// The same from the room list.
#[test]
fn corr_options_open_from_the_room_list() {
    let mut s = sim_with_options();
    assert_eq!(screen(&s).page, Page::List);
    click(&mut s, "StartButton");
    click(&mut s, "button_start_option");
    s.seconds(0.2);
    assert!(s.app.world().resource::<combat_arms::game::options::CaOptions>().open);
}

#[test]
fn the_password_box_takes_eight_characters_and_cancel_forgets_them() {
    let mut s = sim();
    their_room(&mut s, 11, create_spec("TDM", 16, "locked", "ab"));
    click(&mut s, "row_0");
    click(&mut s, "button_Join");
    type_text(&mut s, "abcdefghij");
    assert_eq!(label(&s, "input_password"), "********");
    click(&mut s, "button_cancel");
    assert!(screen(&s).password_room.is_none());
    click(&mut s, "button_Join");
    assert_eq!(screen(&s).password_box.input, "", "the box opens empty");
}
