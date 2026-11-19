//! specs/combat_arms/lobby.md's test cases for the lobby service and the
//! client's checks (`game::lobby`): the room list (cases 1 to 8), room
//! creation (9 to 17), the waiting room's teams and options (18 to 30),
//! ready, start and the countdown (31 to 38), joining a match in progress
//! (39 to 42), the host, leaving and chat (43 to 47) and the result and
//! the way back (58, 60, 61). The service is a state machine, so these
//! need no world; cases 48 to 57 (the loadout) are `lobby_loadout.rs`, the
//! lobby over the network `lobby_net.rs`, the screens tests/ui/lobby.rs.

use combat_arms::game::lobby::{
    check::{self, CreateForm, Primary},
    client::{ClientLobby, countdown_text},
    data::{self, LobbyData, RANDOM_MAP},
    list::{Column, Direction, Joining, RoomListView},
    room::*,
    service::Lobby,
};

fn lobby() -> Lobby {
    Lobby::builtin()
}

fn mode(l: &Lobby, key: &str) -> u16 {
    l.data.mode_by_key(key).unwrap().id
}

fn hello(l: &mut Lobby, id: PlayerId) {
    l.request(id, Request::Hello { name: format!("P{id}") }).unwrap();
}

/// A create request for a mode key with its defaults and `players`.
fn spec(l: &Lobby, key: &str, players: u32) -> CreateSpec {
    let m = l.data.mode_by_key(key).unwrap();
    CreateSpec {
        name: "Room".into(),
        password: String::new(),
        mode: m.id,
        pure_combat: false,
        map: None,
        goal: m.goal_default(),
        players,
        arms: [0, 0],
    }
}

/// Player 1 hosts a room of `key` for `players`; players 2..=n enter in
/// order. Returns the room's id.
fn room_with(l: &mut Lobby, key: &str, players: u32, joiners: u64) -> RoomId {
    for p in 1..=joiners + 1 {
        hello(l, p);
    }
    let s = spec(l, key, players);
    l.request(1, Request::Create(s)).unwrap();
    let id = l.room_of(1).unwrap();
    for p in 2..=joiners + 1 {
        l.request(
            p,
            Request::Enter {
                room: id,
                password: String::new(),
            },
        )
        .unwrap();
    }
    id
}

fn team(l: &Lobby, room: RoomId, p: PlayerId) -> Team {
    l.room(room).unwrap().seat(p).unwrap().team
}

fn state(l: &Lobby, room: RoomId, p: PlayerId) -> PlayerState {
    l.room(room).unwrap().seat(p).unwrap().state
}

fn ready(l: &mut Lobby, p: PlayerId) {
    l.request(p, Request::Ready(true)).unwrap();
}

fn settings(l: &Lobby, room: RoomId) -> Settings {
    l.room(room).unwrap().settings.clone()
}

fn change(l: &mut Lobby, p: PlayerId, what: Setting) -> Result<(), Refusal> {
    let room = l.room_of(p).unwrap();
    let mode = l.room(room).unwrap().settings.mode;
    l.request(p, Request::Change { mode, what })
}

fn summary(
    id: RoomId,
    name: &str,
    state: RoomState,
    users: u32,
    max: u32,
    join_mid: bool,
    locked: bool,
) -> RoomSummary {
    RoomSummary {
        id,
        name: name.into(),
        locked,
        map: 0,
        mode: 0,
        pure_combat: false,
        super_soldiers: false,
        arms: [0, 0],
        users,
        max,
        state,
        join_mid,
        moderator: 0,
    }
}

/// The room list of case 1: A waiting 4/16, B in game 16/16, C in game
/// 3/16 with Join Mid Combat off, D waiting 2/8 with a password.
fn case_list() -> RoomListView {
    let mut v = RoomListView::new(LobbyData::built_in());
    v.set_all(vec![
        summary(0, "Alpha", RoomState::Waiting, 4, 16, true, false),
        summary(1, "bravo", RoomState::InGame, 16, 16, true, false),
        summary(2, "Charlie", RoomState::InGame, 3, 16, false, false),
        summary(3, "delta", RoomState::Waiting, 2, 8, true, true),
    ]);
    v
}

fn titles(v: &RoomListView) -> Vec<String> {
    v.visible().iter().map(|r| r.name.clone()).collect()
}

// ---------------------------------------------------------------------------
// L: the room list

#[test]
fn case_1_rows_show_numbers_status_and_greyed_rooms() {
    let v = case_list();
    let rows = v.rows();
    assert_eq!(
        rows.iter().map(|r| r.num.as_str()).collect::<Vec<_>>(),
        ["1", "2", "3", "4"]
    );
    assert_eq!(
        rows.iter().map(|r| r.players.as_str()).collect::<Vec<_>>(),
        ["4/16", "16/16", "3/16", "2/8"]
    );
    assert_eq!(
        rows.iter().map(|r| r.status.as_str()).collect::<Vec<_>>(),
        ["Waiting", "In game", "In game", "Waiting"]
    );
    assert_eq!(
        rows.iter().map(|r| r.disabled).collect::<Vec<_>>(),
        [false, true, true, false]
    );
    assert_eq!(
        rows.iter().map(|r| r.locked).collect::<Vec<_>>(),
        [false, false, false, true]
    );
}

#[test]
fn case_2_joinable_and_no_password_filters() {
    let mut v = case_list();
    v.filters.joinable = true;
    v.rebuild();
    assert_eq!(titles(&v), ["Alpha", "delta"]);
    v.filters.no_password = true;
    v.rebuild();
    assert_eq!(titles(&v), ["Alpha"]);
}

/// Corrections 2026-10-22 (4): a map change re-sorts a list sorted by Map.
#[test]
fn corr4_a_map_change_resorts_the_list() {
    let mut v = RoomListView::new(LobbyData::built_in());
    let mut a = summary(0, "A", RoomState::Waiting, 1, 8, true, false);
    a.map = 1; // JUNK FLEA
    let mut b = summary(1, "B", RoomState::Waiting, 1, 8, true, false);
    b.map = 0; // OIL RIG
    v.set_all(vec![a.clone(), b]);
    v.click(Column::Map);
    assert_eq!(titles(&v), ["A", "B"]);
    a.map = 2; // WAREHOUSE
    v.changed(a);
    assert_eq!(titles(&v), ["B", "A"], "re-sorted at once");
}

/// Corrections 2026-10-22 (4): filters apply only on a rebuild: a room that
/// starts failing vanishes at once, one passing again stays hidden.
#[test]
fn corr4_filters_apply_on_rebuild_only() {
    let mut v = case_list();
    v.filters.joinable = true;
    v.rebuild();
    assert_eq!(titles(&v), ["Alpha", "delta"]);
    // Alpha becomes full: gone at once.
    v.changed(summary(0, "Alpha", RoomState::Waiting, 16, 16, true, false));
    assert_eq!(titles(&v), ["delta"]);
    // It has room again: still hidden until a rebuild.
    v.changed(summary(0, "Alpha", RoomState::Waiting, 15, 16, true, false));
    assert_eq!(titles(&v), ["delta"]);
    v.click(Column::Num);
    assert_eq!(titles(&v), ["Alpha", "delta"]);
    // A name change rebuilds nothing.
    v.changed(summary(0, "Renamed", RoomState::Waiting, 15, 16, true, false));
    assert_eq!(titles(&v), ["Renamed", "delta"]);
}

#[test]
fn case_3_title_sort_ignores_case_and_the_direction_is_shared() {
    let mut v = case_list();
    v.click(Column::Title);
    assert_eq!(titles(&v), ["Alpha", "bravo", "Charlie", "delta"]);
    v.click(Column::Title);
    assert_eq!(titles(&v), ["delta", "Charlie", "bravo", "Alpha"]);
    // The spec's text says the Map click after these is descending, but
    // its own sort rule (each click flips the one direction, quirk 2) makes
    // it ascending here: implemented as the rule.
    v.click(Column::Map);
    assert_eq!(v.direction, Direction::Ascending);
}

#[test]
fn case_3_quirk_2_title_then_map_is_descending() {
    let mut v = RoomListView::new(LobbyData::built_in());
    let mut a = summary(0, "a", RoomState::Waiting, 1, 8, true, false);
    a.map = 0; // OIL RIG
    let mut b = summary(1, "b", RoomState::Waiting, 1, 8, true, false);
    b.map = 1; // JUNK FLEA
    let mut c = summary(2, "c", RoomState::Waiting, 1, 8, true, false);
    c.map = 2; // WAREHOUSE
    v.set_all(vec![a, b, c]);
    v.click(Column::Title);
    assert_eq!(v.direction, Direction::Ascending);
    v.click(Column::Map);
    assert_eq!(v.direction, Direction::Descending);
    let maps: Vec<String> = v.rows().iter().map(|r| r.map.clone()).collect();
    assert_eq!(maps, ["WAREHOUSE", "OIL RIG", "JUNK FLEA"]);
}

#[test]
fn case_4_player_sort_is_max_then_count() {
    let mut v = case_list();
    v.click(Column::Players);
    assert_eq!(titles(&v), ["delta", "Charlie", "Alpha", "bravo"]);
}

#[test]
fn case_5_a_new_room_is_sorted_in_and_the_scroll_stays() {
    let mut v = case_list();
    v.click(Column::Title);
    v.scroll = 2;
    v.created(summary(4, "Bob", RoomState::Waiting, 1, 8, true, false));
    assert_eq!(titles(&v), ["Alpha", "Bob", "bravo", "Charlie", "delta"]);
    assert_eq!(v.scroll, 2);
}

#[test]
fn case_6_a_room_the_client_refuses_sends_nothing() {
    let mut c = ClientLobby::new(LobbyData::built_in(), 1, "me");
    c.list = case_list();
    assert_eq!(c.join(Some(2)), None);
    assert!(
        c.message
            .as_deref()
            .unwrap()
            .starts_with("You cannot enter the room that you have currently selected.")
    );
    assert!(c.outbox.is_empty());
    // No selection.
    c.message = None;
    assert_eq!(c.join(None), None);
    assert_eq!(c.message.as_deref(), Some("Select a game room that you wish to enter."));
}

#[test]
fn case_7_the_password_is_compared_exactly() {
    let mut l = lobby();
    for p in 1..=3 {
        hello(&mut l, p);
    }
    let mut s = spec(&l, "TDM", 16);
    s.password = "ab".into();
    l.request(1, Request::Create(s)).unwrap();
    let room = l.room_of(1).unwrap();
    // The client asks for the password of a locked row.
    let mut c = ClientLobby::new(LobbyData::built_in(), 2, "me");
    c.list.set_all(l.summaries());
    assert_eq!(c.join(Some(room)), Some(Joining::Password(room)));
    c.enter(room, "ab");
    assert_eq!(
        c.take_outbox(),
        [Request::Enter {
            room,
            password: "ab".into()
        }]
    );
    assert_eq!(
        l.request(
            2,
            Request::Enter {
                room,
                password: "ab".into()
            }
        ),
        Ok(())
    );
    assert!(l.room_of(2).is_some());
    // "AB" is wrong: the message, and still in the list.
    let r = l.request(
        3,
        Request::Enter {
            room,
            password: "AB".into(),
        },
    );
    assert_eq!(r, Err(Refusal::WrongPassword));
    assert_eq!(
        Refusal::WrongPassword.text(),
        "You enter the wrong password,please retry."
    );
    assert_eq!(l.room_of(3), None);
}

#[test]
fn case_8_the_mode_filter_matches_the_pure_combat_flag() {
    let mut v = RoomListView::new(LobbyData::built_in());
    let tmm = 1;
    let mut a = summary(0, "plain", RoomState::Waiting, 1, 16, true, false);
    a.mode = tmm;
    let mut b = summary(1, "pcm", RoomState::Waiting, 1, 16, true, false);
    b.mode = tmm;
    b.pure_combat = true;
    let c = summary(2, "elim", RoomState::Waiting, 1, 16, true, false);
    v.set_all(vec![a, b, c]);
    v.filter_mode(Some((tmm, false)));
    assert_eq!(titles(&v), ["plain"]);
    assert_eq!(v.rows()[0].mode, "Search & Destroy");
    v.filter_mode(Some((tmm, true)));
    assert_eq!(v.rows()[0].mode, "[PCM] Search & Destroy");
}

#[test]
fn the_list_stays_live_and_a_change_does_not_resort() {
    let mut l = lobby();
    for p in 1..=2 {
        hello(&mut l, p);
    }
    l.take_notices();
    let s = spec(&l, "TDM", 16);
    l.request(1, Request::Create(s)).unwrap();
    let notices = l.take_notices();
    let created = notices.iter().find_map(|(a, n)| match (a, n) {
        (Audience::Channel, Notice::RoomCreated(r)) => Some(r.clone()),
        _ => None,
    });
    let created = created.expect("a room-created notice to the channel");
    let mut v = RoomListView::new(LobbyData::built_in());
    v.created(created.clone());
    let room = created.id;
    l.request(
        2,
        Request::Enter {
            room,
            password: String::new(),
        },
    )
    .unwrap();
    let changed = l.take_notices().into_iter().find_map(|(_, n)| match n {
        Notice::RoomChanged(r) => Some(r),
        _ => None,
    });
    v.changed(changed.expect("a room-changed notice"));
    assert_eq!(v.rows()[0].players, "2/16");
    l.request(2, Request::Leave).unwrap();
    l.request(1, Request::Leave).unwrap();
    let gone = l
        .take_notices()
        .into_iter()
        .any(|(_, n)| n == Notice::RoomDestroyed(room));
    assert!(gone, "the last player leaving destroys the room");
}

// ---------------------------------------------------------------------------
// C: room creation

fn form(single: bool) -> CreateForm {
    CreateForm::open(LobbyData::built_in(), single, 1234, 16)
}

#[test]
fn case_9_the_popup_opens_with_the_modes_defaults() {
    let f = form(false);
    assert_eq!(f.mode_rec().name, "Elimination");
    assert_eq!(f.map_text(), "OIL RIG");
    let goals = f.goals();
    assert_eq!(
        goals.iter().find(|(g, _)| *g == f.goal).unwrap().1,
        "100 Kills (Default)"
    );
    assert_eq!(
        f.player_options().iter().find(|(p, _)| *p == f.players).unwrap().1,
        "8 vs 8"
    );
    let arms = f.arms_options();
    assert_eq!(arms.iter().find(|(id, _)| *id == f.arms[0]).unwrap().1, "All Permitted");
    assert!(f.arms_enabled());
    assert!(!f.second_arms_enabled(), "2nd Arms is '-' and disabled offline");
    assert!(!f.use_password);
    let titles = &LobbyData::built_in().titles;
    assert!(titles.contains(&f.name), "one of the five default titles");
    assert_eq!(titles.len(), 5);
}

#[test]
fn case_10_changing_to_one_man_army_keeps_a_valid_player_count() {
    let mut f = form(false);
    assert_eq!(f.players, 16);
    let ffa = LobbyData::built_in().mode_by_key("FFA").unwrap().id;
    f.set_mode(ffa);
    assert_eq!(f.map_text(), "JUNK FLEA");
    assert_eq!(
        f.goals().iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
        ["15 Kills", "30 Kills (Default)", "45 Kills"]
    );
    let players = f.player_options();
    assert_eq!(players.first().unwrap(), &(2, "2 Player(s)".to_string()));
    assert_eq!(players.last().unwrap(), &(16, "16 Player(s)".to_string()));
    assert_eq!(players.len(), 15, "2 to 16 in steps of 1");
    assert_eq!(f.players, 16, "16 is valid for the new mode, so it stays");
    // Opened fresh with One Man Army picked first.
    let fresh = CreateForm::with_mode(LobbyData::built_in(), false, ffa, 16);
    assert_eq!(fresh.players, 8);
    assert_eq!(
        fresh.player_options().iter().find(|(p, _)| *p == 8).unwrap().1,
        "8 Player(s)"
    );
}

#[test]
fn case_11_fireteam_has_a_difficulty_instead_of_a_goal() {
    let mut f = form(false);
    let coop = LobbyData::built_in().mode_by_key("COOP").unwrap().id;
    f.set_mode(coop);
    assert_eq!(f.goal_title(), "Difficulty");
    assert_eq!(
        f.goals().iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
        ["NORMAL", "HARD", "EXTREME"]
    );
    assert_eq!(f.goal, 1, "HARD");
    assert_eq!(f.player_options(), [(8, "8 Player(s)".to_string())]);
    assert_eq!(f.players, 8);
    assert!(!f.arms_enabled());
    assert_eq!(f.arms[0], 0, "All Permitted");
}

#[test]
fn case_12_a_blank_name_is_refused_before_anything_is_sent() {
    let mut f = form(false);
    f.name = "   ".into();
    assert_eq!(f.submit(&|_| false), Err(Refusal::NameEmpty));
    assert_eq!(Refusal::NameEmpty.text(), "Please enter the room name.");
    // A name the filter rejects (unless it is still a default title).
    f.name = "badword".into();
    assert_eq!(f.submit(&|n| n.contains("badword")), Err(Refusal::NameBad));
    f.name = "Weapons Hot!".into();
    assert!(f.submit(&|_| true).is_ok(), "a default title skips the filter");
}

#[test]
fn case_13_capture_the_flag_with_four_players() {
    let mut l = lobby();
    let ctf = l.data.mode_by_key("CTF").unwrap().id;
    let mut f = form(false);
    f.set_mode(ctf);
    f.set_players(4);
    f.name = "My CTF".into();
    let spec = f.submit(&|_| false).unwrap();
    hello(&mut l, 1);
    l.request(1, Request::Create(spec)).unwrap();
    let room = l.room_of(1).unwrap();
    assert_eq!(l.room(room).unwrap().host, 1);
    assert_eq!(team(&l, room, 1), Team::Alpha);
    let s = settings(&l, room);
    assert_eq!(s.goal, 5);
    assert_eq!(l.data.mode(s.mode).unwrap().goal_text(5), "5 Points (Default)");
    assert_eq!(
        (s.join_mid, s.friendly_fire, s.kill_cam, s.abilities),
        (true, false, false, true)
    );
    assert_eq!(s.max_players, 4);
}

#[test]
fn case_14_a_goal_outside_the_range_is_silently_the_default() {
    let mut l = lobby();
    hello(&mut l, 1);
    let mut s = spec(&l, "TDM", 16);
    s.goal = 25;
    l.request(1, Request::Create(s)).unwrap();
    assert_eq!(settings(&l, l.room_of(1).unwrap()).goal, 100);
}

#[test]
fn case_15_an_odd_player_count_in_a_two_team_mode_is_refused() {
    let mut l = lobby();
    hello(&mut l, 1);
    let s = spec(&l, "TDM", 7);
    assert_eq!(l.request(1, Request::Create(s)), Err(Refusal::Players));
    assert_eq!(l.room_of(1), None);
}

#[test]
fn case_16_a_mode_the_lobby_has_no_rules_for_is_refused() {
    let mut l = lobby();
    hello(&mut l, 1);
    let mut s = spec(&l, "TDM", 16);
    s.mode = 20; // Big Head
    assert_eq!(l.request(1, Request::Create(s)), Err(Refusal::ModeNoRules));
    // And the popup never lists it.
    assert!(form(false).modes().iter().all(|m| m.id < 20));
}

#[test]
fn case_17_the_single_tab() {
    let f = form(true);
    assert_eq!(f.mode_rec().name, "Elimination (bot play)");
    assert_eq!(
        f.player_options().iter().map(|(_, t)| t.as_str()).collect::<Vec<_>>(),
        ["1 vs 5", "1 vs 6", "1 vs 7", "1 vs 8"]
    );
    assert_eq!(f.players, 8);
    assert_eq!(f.difficulty, 1, "HARD");
    assert_eq!(f.goals().iter().map(|(g, _)| *g).collect::<Vec<_>>(), [20, 30, 40, 50]);
    assert_eq!(f.goal, 40);
    assert!(f.goals().iter().any(|(_, t)| t == "40 Kills (Default)"));
    assert!(!f.rows_shown(), "the password and arms rows are hidden");
    let modes: Vec<&str> = f.modes().iter().map(|m| m.key.as_str()).collect();
    assert_eq!(modes, ["BDM", "SCOP"]);
    // The lobby makes no room for bot play: the client builds it.
    let mut l = lobby();
    hello(&mut l, 1);
    let s = f.submit(&|_| false).unwrap();
    assert_eq!(l.request(1, Request::Create(s)), Err(Refusal::ModeNoRules));
}

#[test]
fn create_refusals_and_silent_replacements() {
    let mut l = lobby();
    hello(&mut l, 1);
    // A map the mode does not open.
    let mut s = spec(&l, "FFA", 8);
    s.map = Some(4); // Blood Money
    assert_eq!(l.request(1, Request::Create(s)), Err(Refusal::MapNotOpen));
    // The random map is some open map.
    let mut s = spec(&l, "FFA", 8);
    s.map = Some(RANDOM_MAP);
    l.request(1, Request::Create(s)).unwrap();
    let room = l.room_of(1).unwrap();
    assert!(
        l.data
            .map_open(settings(&l, room).map, l.data.mode_by_key("FFA").unwrap())
    );
    l.request(1, Request::Leave).unwrap();
    // An invalid arms value is All Permitted; a name over 32 characters
    // leaves the room unnamed.
    let mut s = spec(&l, "TDM", 16);
    s.arms = [11, 0];
    s.name = "x".repeat(33);
    l.request(1, Request::Create(s)).unwrap();
    let st = settings(&l, l.room_of(1).unwrap());
    assert_eq!((st.arms, st.name.as_str()), ([0, 0], ""));
    // Too many players for the channel.
    l.request(1, Request::Leave).unwrap();
    let s = spec(&l, "TDM", 18);
    assert_eq!(l.request(1, Request::Create(s)), Err(Refusal::Players));
    // Pure Combat rooms are not offered offline.
    let mut s = spec(&l, "TDM", 16);
    s.pure_combat = true;
    assert_eq!(l.request(1, Request::Create(s)), Err(Refusal::PureCombat));
    // The cup forces its own arms option.
    let s = spec(&l, "ESL", 10);
    l.request(1, Request::Create(s)).unwrap();
    assert_eq!(settings(&l, l.room_of(1).unwrap()).arms, [data::CUP_ARMS, 0]);
}

#[test]
fn rooms_take_the_lowest_free_id_up_to_a_hundred() {
    let mut l = lobby();
    for p in 1..=101u64 {
        hello(&mut l, p);
    }
    for p in 1..=100u64 {
        let s = spec(&l, "TDM", 16);
        l.request(p, Request::Create(s)).unwrap();
        assert_eq!(l.room_of(p), Some((p - 1) as RoomId));
    }
    let s = spec(&l, "TDM", 16);
    assert_eq!(l.request(101, Request::Create(s)), Err(Refusal::NoFreeRoom));
    l.request(5, Request::Leave).unwrap();
    let s = spec(&l, "TDM", 16);
    l.request(101, Request::Create(s)).unwrap();
    assert_eq!(l.room_of(101), Some(4));
}

// ---------------------------------------------------------------------------
// R, T, O: the waiting room

#[test]
fn case_18_a_two_team_room_fills_the_smaller_team_alpha_on_ties() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 3);
    assert_eq!(team(&l, room, 1), Team::Alpha);
    assert_eq!(team(&l, room, 2), Team::Bravo);
    assert_eq!(team(&l, room, 3), Team::Alpha);
    assert_eq!(team(&l, room, 4), Team::Bravo);
}

#[test]
fn case_19_a_one_team_room_fills_alpha_to_eight_and_has_no_team_buttons() {
    let mut l = lobby();
    let room = room_with(&mut l, "FFA", 16, 9);
    for p in 1..=8 {
        assert_eq!(team(&l, room, p), Team::Alpha, "player {p}");
    }
    assert_eq!(team(&l, room, 9), Team::Bravo);
    assert_eq!(team(&l, room, 10), Team::Bravo);
    let detail = l.room(room).unwrap().detail();
    assert_eq!(check::switch_check(&l.data, &detail, 3), Err(Refusal::NoTeamChange));
    // The service refuses a forged request too.
    assert!(l.request(3, Request::SetTeam(Team::Bravo)).is_err());
}

#[test]
fn case_20_a_full_team_refuses_a_switch() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 4, 3);
    // Alpha: 1, 3; Bravo: 2, 4.
    let r = l.request(3, Request::SetTeam(Team::Bravo));
    assert_eq!(r, Err(Refusal::TeamSwitch));
    assert_eq!(team(&l, room, 3), Team::Alpha);
    // A refused switch shows a message box whose text is a single space
    // (quirk 5).
    assert_eq!(Refusal::TeamSwitch.text(), " ");
}

#[test]
fn case_21_a_ready_player_cannot_switch() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 2);
    ready(&mut l, 2);
    assert_eq!(team(&l, room, 2), Team::Bravo);
    assert_eq!(l.request(2, Request::SetTeam(Team::Alpha)), Err(Refusal::TeamSwitch));
    // Not ready, it can.
    l.request(2, Request::Ready(false)).unwrap();
    l.request(2, Request::SetTeam(Team::Alpha)).unwrap();
    assert_eq!(team(&l, room, 2), Team::Alpha);
}

#[test]
fn case_22_the_client_refuses_a_switch_that_unbalances_a_match_in_progress() {
    let data = LobbyData::built_in();
    let seat = |id: u64, team: Team| Seat {
        id,
        name: format!("P{id}"),
        team,
        state: PlayerState::InGame,
        extra: Extra::None,
        order: id,
    };
    let room = |alpha: u64, bravo: u64| RoomDetail {
        id: 0,
        settings: combat_arms::game::lobby::service::defaults(data.mode(0).unwrap(), false),
        host: 100,
        state: RoomState::InGame,
        seats: (1..=alpha)
            .map(|i| seat(i, Team::Alpha))
            .chain((10..10 + bravo).map(|i| seat(i, Team::Bravo)))
            .collect(),
    };
    // Alpha 3 (P1 among them), Bravo 4: 3 - 1 = 2 < 4.
    assert_eq!(check::switch_check(&data, &room(3, 4), 1), Err(Refusal::TeamImbalance));
    assert_eq!(
        Refusal::TeamImbalance.text(),
        "Unable to switch teams due to team imbalance."
    );
    // Alpha 5, Bravo 4: the request is sent.
    assert_eq!(check::switch_check(&data, &room(5, 4), 1), Ok(()));
}

#[test]
fn case_23_friendly_fire_keeps_ready_states() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 2);
    ready(&mut l, 2);
    l.take_notices();
    change(&mut l, 1, Setting::FriendlyFire(true)).unwrap();
    assert!(settings(&l, room).friendly_fire);
    assert_eq!(state(&l, room, 2), PlayerState::Ready);
    let notices = l.take_notices();
    assert!(
        notices
            .iter()
            .any(|(a, n)| *a == Audience::Room(room) && matches!(n, Notice::Settings(s) if s.friendly_fire))
    );
    // The clients show it.
    let mut c = ClientLobby::new(LobbyData::built_in(), 2, "me");
    c.apply(Notice::Entered(l.room(room).unwrap().detail()), 0.0);
    c.apply(Notice::Settings(settings(&l, room)), 0.0);
    assert!(c.room.as_ref().unwrap().settings.friendly_fire);
}

#[test]
fn case_24_friendly_fire_is_unavailable_in_one_man_army() {
    let mut l = lobby();
    let room = room_with(&mut l, "FFA", 8, 1);
    let detail = l.room(room).unwrap().detail();
    let enabled = check::option_enabled(&l.data, &detail, 1);
    assert!(!enabled[1], "the control is disabled");
    assert_eq!(
        change(&mut l, 1, Setting::FriendlyFire(true)),
        Err(Refusal::OptionRefused)
    );
    assert!(!settings(&l, room).friendly_fire);
}

#[test]
fn case_25_a_map_change_resets_the_goal_and_keeps_ready_states() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    change(&mut l, 1, Setting::Goal(60)).unwrap();
    ready(&mut l, 2);
    change(&mut l, 1, Setting::Map(2)).unwrap();
    let s = settings(&l, room);
    assert_eq!((s.map, s.goal), (2, 100));
    assert_eq!(state(&l, room, 2), PlayerState::Ready);
    // A map the mode does not open, and no change.
    assert_eq!(change(&mut l, 1, Setting::Map(99)), Err(Refusal::MapNotOpen));
    assert_eq!(change(&mut l, 1, Setting::Map(2)), Err(Refusal::NoChange));
}

#[test]
fn case_26_an_arms_change_unreadies_everyone() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    ready(&mut l, 2);
    l.take_notices();
    change(&mut l, 1, Setting::Arms(1, 0)).unwrap();
    assert_eq!(state(&l, room, 2), PlayerState::Waiting);
    let line = l.take_notices().into_iter().find_map(|(_, n)| match n {
        Notice::System(t) => Some(t),
        _ => None,
    });
    assert_eq!(line.as_deref(), Some("Arms Setting changed to : Pistols Only"));
    // Both values must be below 11, and a real change.
    assert_eq!(change(&mut l, 1, Setting::Arms(11, 0)), Err(Refusal::OptionRefused));
    assert_eq!(change(&mut l, 1, Setting::Arms(1, 0)), Err(Refusal::NoChange));
}

#[test]
fn case_27_a_mode_change_unreadies_moves_bravo_and_raises_the_maximum() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 11);
    assert_eq!(l.room(room).unwrap().team_count(Team::Alpha), 6);
    assert_eq!(l.room(room).unwrap().team_count(Team::Bravo), 6);
    for p in 2..=12 {
        ready(&mut l, p);
    }
    l.take_notices();
    let ffa = mode(&l, "FFA");
    change(&mut l, 1, Setting::Mode(ffa)).unwrap();
    let r = l.room(room).unwrap();
    assert_eq!(r.settings.mode, ffa);
    assert_eq!(r.team_count(Team::Alpha), 8);
    assert_eq!(r.team_count(Team::Bravo), 4);
    // The first Bravo players in join order moved.
    assert_eq!(team(&l, room, 2), Team::Alpha);
    assert_eq!(team(&l, room, 4), Team::Alpha);
    assert_eq!(team(&l, room, 6), Team::Bravo);
    assert_eq!(r.settings.max_players, 12, "the default 8 raised to the 12 players");
    assert!(r.seats.iter().all(|s| s.state == PlayerState::Waiting));
    // Goal and options are the new mode's defaults.
    assert_eq!(r.settings.goal, 30);
    let line = l.take_notices().into_iter().find_map(|(_, n)| match n {
        Notice::System(t) => Some(t),
        _ => None,
    });
    assert_eq!(line.as_deref(), Some("The game has been changed to One Man Army mode."));
}

#[test]
fn case_28_too_many_players_for_the_new_mode() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 9);
    let coop = mode(&l, "COOP");
    let detail = l.room(room).unwrap().detail();
    let r = check::mode_change_check(&l.data, &detail, coop);
    assert_eq!(
        r.unwrap_err().text(),
        "You have too many players to change the Mode to Fireteam (Max: 8 players)"
    );
    // The lobby refuses it too.
    assert!(matches!(
        change(&mut l, 1, Setting::Mode(coop)),
        Err(Refusal::TooManyPlayers { .. })
    ));
    assert_eq!(settings(&l, room).mode, 0);
}

#[test]
fn case_29_a_non_host_cannot_change_settings() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    assert_eq!(change(&mut l, 2, Setting::Goal(60)), Err(Refusal::NotHost));
    assert_eq!(settings(&l, room).goal, 100);
}

#[test]
fn case_30_a_stale_request_is_refused() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    let r = l.request(
        1,
        Request::Change {
            mode: 1,
            what: Setting::Goal(60),
        },
    );
    assert_eq!(r, Err(Refusal::Stale));
    assert_eq!(settings(&l, room).goal, 100);
}

#[test]
fn settings_only_while_waiting_and_players_must_fit() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 5);
    // Alpha 3, Bravo 3: 4 players cannot hold 3 a side... 6 can.
    assert_eq!(change(&mut l, 1, Setting::Players(4)), Err(Refusal::Players));
    assert_eq!(
        change(&mut l, 1, Setting::Players(5)),
        Err(Refusal::Players),
        "odd in a two-team mode"
    );
    change(&mut l, 1, Setting::Players(6)).unwrap();
    assert_eq!(settings(&l, room).max_players, 6);
    // A goal outside the lobby's range is refused here (quirk 12).
    assert_eq!(change(&mut l, 1, Setting::Goal(25)), Err(Refusal::BadValue));
    // The name and the password.
    change(&mut l, 1, Setting::Name("New".into())).unwrap();
    change(&mut l, 1, Setting::Password("secret".into())).unwrap();
    assert_eq!(
        change(&mut l, 1, Setting::Password("123456789".into())),
        Err(Refusal::BadValue)
    );
    assert!(l.summaries()[0].locked);
    change(&mut l, 1, Setting::Password(String::new())).unwrap();
    assert!(!l.summaries()[0].locked);
    // Not while starting.
    for p in 2..=6 {
        ready(&mut l, p);
    }
    l.request(1, Request::Start).unwrap();
    assert_eq!(change(&mut l, 1, Setting::Goal(60)), Err(Refusal::NotWaiting));
}

#[test]
fn option_rules_by_mode() {
    let mut l = lobby();
    let room = room_with(&mut l, "HG", 16, 2);
    // Hunted: Kill Cam on, Abilities off and locked.
    let s = settings(&l, room);
    assert!(s.kill_cam && !s.abilities);
    assert_eq!(change(&mut l, 1, Setting::Abilities(true)), Err(Refusal::OptionRefused));
    // Elimination: all three changeable; Super Soldiers too.
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    change(&mut l, 1, Setting::KillCam(true)).unwrap();
    change(&mut l, 1, Setting::Abilities(false)).unwrap();
    change(&mut l, 1, Setting::SuperSoldiers(true)).unwrap();
    change(&mut l, 1, Setting::JoinMid(false)).unwrap();
    let s = settings(&l, room);
    assert!(s.kill_cam && !s.abilities && s.super_soldiers && !s.join_mid);
    // Search & Destroy has no Super Soldiers; Fireteam no arms.
    let mut l = lobby();
    room_with(&mut l, "TMM", 16, 0);
    assert_eq!(
        change(&mut l, 1, Setting::SuperSoldiers(true)),
        Err(Refusal::OptionRefused)
    );
    let mut l = lobby();
    room_with(&mut l, "COOP", 8, 0);
    assert_eq!(change(&mut l, 1, Setting::Arms(1, 0)), Err(Refusal::OptionRefused));
}

// ---------------------------------------------------------------------------
// S: ready, start, countdown

#[test]
fn case_31_start_needs_a_ready_player_on_each_team() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    let detail = l.room(room).unwrap().detail();
    let e = check::start_check(&l.data, &detail).unwrap_err();
    assert_eq!(e.text(), "There aren't enough players to start the game.");
    // The lobby refuses it as well.
    assert_eq!(l.request(1, Request::Start), Err(Refusal::NotEnoughPlayers));
    ready(&mut l, 2);
    let detail = l.room(room).unwrap().detail();
    assert_eq!(check::start_check(&l.data, &detail), Ok(()));
    assert_eq!(l.request(1, Request::Start), Ok(()));
    assert_eq!(l.room(room).unwrap().state, RoomState::Starting);
}

#[test]
fn case_32_search_and_destroy_the_lobby_needs_three_a_side() {
    let mut l = lobby();
    let room = room_with(&mut l, "TMM", 16, 5);
    // Alpha: 1, 3, 5; Bravo: 2, 4, 6. Ready 3, 5 and 2, 4.
    for p in [3, 5, 2, 4] {
        ready(&mut l, p);
    }
    let detail = l.room(room).unwrap().detail();
    assert_eq!(
        check::start_check(&l.data, &detail),
        Ok(()),
        "the client accepts: each team >= 1"
    );
    assert_eq!(
        l.request(1, Request::Start),
        Err(Refusal::NotEnoughPlayers),
        "the lobby needs 3 and 3"
    );
    ready(&mut l, 6);
    assert_eq!(l.request(1, Request::Start), Ok(()));
}

#[test]
fn case_33_elimination_pro_needs_three_a_side_on_the_client() {
    let mut l = lobby();
    let room = room_with(&mut l, "TSV", 16, 4);
    // Alpha: 1, 3, 5; Bravo: 2, 4. Ready 3 and 2, 4: Alpha H + 1, Bravo 2.
    for p in [3, 2, 4] {
        ready(&mut l, p);
    }
    let detail = l.room(room).unwrap().detail();
    assert_eq!(check::start_check(&l.data, &detail), Err(Refusal::NotEnoughPlayers));
}

#[test]
fn case_34_hunted_needs_three() {
    let mut l = lobby();
    let room = room_with(&mut l, "HG", 16, 3);
    ready(&mut l, 2);
    let detail = l.room(room).unwrap().detail();
    assert_eq!(check::start_check(&l.data, &detail), Err(Refusal::NotEnoughPlayers));
    assert_eq!(l.request(1, Request::Start), Err(Refusal::NotEnoughPlayers));
    ready(&mut l, 3);
    let detail = l.room(room).unwrap().detail();
    assert_eq!(check::start_check(&l.data, &detail), Ok(()));
    assert_eq!(l.request(1, Request::Start), Ok(()));
}

/// Start the room and step the lobby at the times the case gives (ms),
/// the game server becoming ready at `ready_at`; the notices each tick
/// gave.
fn run_ticks(l: &mut Lobby, room: RoomId, times: &[u64], ready_at: Option<u64>) -> Vec<(u64, Vec<Notice>)> {
    l.take_notices();
    let mut out = Vec::new();
    for &t in times {
        if ready_at.is_some_and(|r| t >= r) {
            l.server_ready(room);
        }
        l.tick();
        out.push((t, l.take_notices().into_iter().map(|(_, n)| n).collect()));
    }
    out
}

fn countdowns(n: &[Notice]) -> Vec<u32> {
    n.iter()
        .filter_map(|n| if let Notice::Countdown(c) = n { Some(*c) } else { None })
        .collect()
}

#[test]
fn case_35_the_countdown_runs_five_ticks_then_go() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    ready(&mut l, 2);
    l.request(1, Request::Start).unwrap();
    // The launcher was asked for a game server.
    assert!(
        l.take_events()
            .iter()
            .any(|e| matches!(e, LobbyEvent::Launch { room: r, .. } if *r == room))
    );
    let times: Vec<u64> = (0..6).map(|i| 400 + 1000 * i).collect();
    let ticks = run_ticks(&mut l, room, &times, Some(2000));
    for (i, (t, n)) in ticks.iter().take(5).enumerate() {
        assert_eq!(countdowns(n), [i as u32], "at {t} ms");
        assert!(!n.iter().any(|n| matches!(n, Notice::Go { .. })));
    }
    assert_eq!(countdown_text(0), "Game starting in... 5 seconds.");
    assert_eq!(countdown_text(4), "Game starting in... 1 second.");
    let (t, last) = &ticks[5];
    assert_eq!(*t, 5400);
    assert!(countdowns(last).is_empty());
    assert_eq!(
        last.iter().filter(|n| matches!(n, Notice::Go { .. })).count(),
        2,
        "both players get go"
    );
    assert_eq!(l.room(room).unwrap().state, RoomState::InGame);
    assert_eq!(state(&l, room, 1), PlayerState::InGame);
    assert_eq!(state(&l, room, 2), PlayerState::InGame);
    // The game server is told to expect each, with their kit.
    let expects = l
        .take_events()
        .into_iter()
        .filter(|e| matches!(e, LobbyEvent::Expect { .. }))
        .count();
    assert_eq!(expects, 2);
    // The counter stopped: nothing more happens.
    l.tick();
    assert!(l.take_notices().is_empty());
}

#[test]
fn case_36_no_game_server_stops_the_launcher_then_returns_to_waiting() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    ready(&mut l, 2);
    l.request(1, Request::Start).unwrap();
    l.take_events();
    // The client of player 2 hears the countdown and times out.
    let mut c = ClientLobby::new(LobbyData::built_in(), 2, "me");
    c.apply(Notice::Entered(l.room(room).unwrap().detail()), 0.0);
    let times: Vec<u64> = (0..18).map(|i| 400 + 1000 * i).collect();
    let mut stop_at = None;
    let mut waiting_at = None;
    for &t in &times {
        l.tick();
        let notices = l.take_notices();
        for (a, n) in &notices {
            if *a == Audience::Room(room) || *a == Audience::Player(2) {
                c.apply(n.clone(), t as f64);
            }
        }
        if l.take_events()
            .iter()
            .any(|e| matches!(e, LobbyEvent::StopLauncher(r) if *r == room))
        {
            stop_at = Some(t);
        }
        if l.room(room).unwrap().state == RoomState::Waiting && waiting_at.is_none() {
            waiting_at = Some(t);
        }
    }
    assert_eq!(stop_at, Some(15_400), "the launcher is told to stop at count 15");
    assert_eq!(waiting_at, Some(17_400), "the room waits again at count 17");
    assert_eq!(state(&l, room, 2), PlayerState::Waiting);
    assert_eq!(state(&l, room, 1), PlayerState::Waiting);
    // The client's own timer: 15000 ms after the last countdown notice
    // (4400), still shown though the room waits again (quirk 14).
    c.tick(19_399.0);
    assert!(c.message.is_none());
    c.tick(19_400.0);
    assert_eq!(
        c.message.as_deref(),
        Some("Failed to join the battle. Connection timed out.")
    );
    assert_eq!(c.take_outbox(), [Request::ReturnToRoom(ReturnReason::Timeout)]);
    // The lobby ignores that return: they already wait.
    assert_eq!(l.request(2, Request::ReturnToRoom(ReturnReason::Timeout)), Ok(()));
    assert_eq!(state(&l, room, 2), PlayerState::Waiting);
}

#[test]
fn case_37_only_the_ready_players_go() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 2);
    ready(&mut l, 2);
    l.request(1, Request::Start).unwrap();
    assert_eq!(state(&l, room, 2), PlayerState::Starting);
    assert_eq!(state(&l, room, 3), PlayerState::Waiting);
    l.server_ready(room);
    for _ in 0..6 {
        l.tick();
    }
    assert_eq!(state(&l, room, 2), PlayerState::InGame);
    assert_eq!(state(&l, room, 3), PlayerState::Waiting, "P3 stays in the room");
    // P3 sees START (to join the match in progress).
    let detail = l.room(room).unwrap().detail();
    assert_eq!(check::primary_button(&detail, 3), Primary::Start);
    assert_eq!(check::primary_button(&detail, 1), Primary::Start);
}

#[test]
fn case_38_f5_is_start_for_the_host_and_ready_for_the_others() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    ready(&mut l, 2);
    let detail = l.room(room).unwrap().detail();
    let mut host = ClientLobby::new(LobbyData::built_in(), 1, "H");
    host.apply(Notice::Entered(detail.clone()), 0.0);
    host.f5(Ok(()));
    assert_eq!(host.take_outbox(), [Request::Start]);
    let mut other = ClientLobby::new(LobbyData::built_in(), 3, "P");
    let mut detail = detail;
    detail.seats.push(Seat {
        id: 3,
        name: "P".into(),
        team: Team::Alpha,
        state: PlayerState::Waiting,
        extra: Extra::None,
        order: 9,
    });
    other.apply(Notice::Entered(detail.clone()), 0.0);
    other.f5(Ok(()));
    assert_eq!(other.take_outbox(), [Request::Ready(true)]);
    // The lobby's reply (here a refusal) frees the next press.
    other.apply(Notice::Refused(Refusal::FinalRound), 0.0);
    other.message = None;
    // A failed loadout check sends nothing and offers the message.
    other.f5(Err(
        "You do not have a sniper rifle equipped for 'Sniper Only' mode.".into()
    ));
    assert!(other.take_outbox().is_empty());
    assert!(other.message.as_deref().unwrap().contains("Sniper Only"));
    // A ready player's F5 un-readies.
    detail.seats.last_mut().unwrap().state = PlayerState::Ready;
    other.apply(Notice::Entered(detail), 0.0);
    other.f5(Ok(()));
    assert_eq!(other.take_outbox(), [Request::Ready(false)]);
}

#[test]
fn a_host_cannot_ready_and_the_countdown_texts_are_the_clients() {
    let mut l = lobby();
    room_with(&mut l, "TDM", 16, 1);
    assert_eq!(l.request(1, Request::Ready(true)), Err(Refusal::HostCannotReady));
    // The room chat prints the countdown as a red system line.
    let mut c = ClientLobby::new(LobbyData::built_in(), 2, "me");
    c.apply(Notice::Countdown(3), 0.0);
    assert_eq!(c.chat.last().unwrap().text, "Game starting in... 2 seconds.");
    assert!(c.chat.last().unwrap().system);
}

// ---------------------------------------------------------------------------
// J: joining a match in progress

fn room_in_game(join_mid: bool) -> (Lobby, RoomId) {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 2);
    if !join_mid {
        change(&mut l, 1, Setting::JoinMid(false)).unwrap();
    }
    ready(&mut l, 2);
    l.request(1, Request::Start).unwrap();
    l.server_ready(room);
    for _ in 0..6 {
        l.tick();
    }
    assert_eq!(l.room(room).unwrap().state, RoomState::InGame);
    l.take_notices();
    l.take_events();
    (l, room)
}

#[test]
fn case_39_a_waiting_player_joins_the_match_in_progress() {
    let (mut l, room) = room_in_game(true);
    assert_eq!(l.request(3, Request::Start), Ok(()));
    assert_eq!(state(&l, room, 3), PlayerState::InGame);
    let events = l.take_events();
    let expect = events.iter().find_map(|e| match e {
        LobbyEvent::Expect { player: 3, weapons, .. } => Some(weapons.clone()),
        _ => None,
    });
    // Their normal set: the base loadout.
    assert_eq!(expect, Some(vec![20, 24, 0, 7]));
    let notices = l.take_notices();
    assert!(
        notices
            .iter()
            .any(|(a, n)| *a == Audience::Player(3) && matches!(n, Notice::Go { .. }))
    );
}

#[test]
fn case_40_without_join_mid_combat_the_start_is_refused() {
    let (mut l, room) = room_in_game(false);
    let r = l.request(3, Request::Start);
    assert_eq!(r, Err(Refusal::FinalRound));
    assert_eq!(
        Refusal::FinalRound.text(),
        "Please wait... the game is currently in the final round or is just ending."
    );
    assert_eq!(state(&l, room, 3), PlayerState::Waiting);
    // Nor can a new player enter the in-game room.
    hello(&mut l, 9);
    assert_eq!(
        l.request(
            9,
            Request::Enter {
                room,
                password: String::new()
            }
        ),
        Err(Refusal::NoMidCombat)
    );
}

#[test]
fn case_41_a_closing_room_cannot_be_entered() {
    let (mut l, room) = room_in_game(true);
    l.closing(room);
    hello(&mut l, 9);
    let summary = l.room(room).unwrap().summary();
    assert!(!summary.can_enter(), "greyed");
    assert_eq!(
        l.request(
            9,
            Request::Enter {
                room,
                password: String::new()
            }
        ),
        Err(Refusal::Closing)
    );
    // A waiting player's START is refused too.
    assert_eq!(l.request(3, Request::Start), Err(Refusal::FinalRound));
    // The room list still says "In game".
    assert_eq!(summary.state.text(), "In game");
}

#[test]
fn case_42_a_late_joiner_of_a_starting_room_goes_in_with_the_others() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    ready(&mut l, 2);
    l.request(1, Request::Start).unwrap();
    hello(&mut l, 3);
    l.request(
        3,
        Request::Enter {
            room,
            password: String::new(),
        },
    )
    .unwrap();
    l.request(3, Request::Ready(true)).unwrap();
    assert_eq!(state(&l, room, 3), PlayerState::Starting);
    l.server_ready(room);
    for _ in 0..6 {
        l.tick();
    }
    assert_eq!(state(&l, room, 3), PlayerState::InGame);
    // With Join Mid Combat off the room cannot even be entered.
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    change(&mut l, 1, Setting::JoinMid(false)).unwrap();
    ready(&mut l, 2);
    l.request(1, Request::Start).unwrap();
    hello(&mut l, 3);
    assert_eq!(
        l.request(
            3,
            Request::Enter {
                room,
                password: String::new()
            }
        ),
        Err(Refusal::NoMidCombat)
    );
}

/// A client in `room`'s detail as player `id`.
fn client_in(l: &Lobby, room: RoomId, id: PlayerId) -> ClientLobby {
    let mut c = ClientLobby::new(LobbyData::built_in(), id, "me");
    c.apply(Notice::Entered(l.room(room).unwrap().detail()), 0.0);
    c
}

/// Corrections 2026-10-22 (1): START while the room is in game is a join,
/// the host's too: no player count check, no "not enough players".
#[test]
fn corr1_the_host_rejoins_a_running_match_with_start() {
    let (mut l, room) = room_in_game(true);
    // The host left the match with EXIT and waits in the room.
    l.request(1, Request::ReturnToRoom(ReturnReason::Normal)).unwrap();
    assert_eq!(state(&l, room, 1), PlayerState::Waiting);
    l.take_notices();
    let mut host = client_in(&l, room, 1);
    host.f5(Ok(()));
    assert!(host.message.is_none(), "{:?}", host.message);
    assert_eq!(host.take_outbox(), [Request::Start]);
    assert_eq!(l.request(1, Request::Start), Ok(()));
    assert_eq!(state(&l, room, 1), PlayerState::InGame);
    let notices = l.take_notices();
    assert!(
        notices
            .iter()
            .any(|(a, n)| *a == Audience::Player(1) && matches!(n, Notice::Go { .. }))
    );
}

/// The same with Join Mid Combat off: the final-round message.
#[test]
fn corr1_without_join_mid_combat_the_hosts_start_is_refused() {
    let (mut l, room) = room_in_game(false);
    l.request(1, Request::ReturnToRoom(ReturnReason::Normal)).unwrap();
    assert_eq!(l.request(1, Request::Start), Err(Refusal::FinalRound));
    assert_eq!(state(&l, room, 1), PlayerState::Waiting);
}

/// Corrections 2026-10-22 (2): a join runs the loadout checks first.
#[test]
fn corr2_a_mid_match_join_runs_the_arms_check() {
    let (l, room) = room_in_game(true);
    let mut c = client_in(&l, room, 3);
    let why = "You do not have a pistol equipped for 'Pistols Only' mode.";
    c.f5(Err(why.into()));
    assert!(c.take_outbox().is_empty(), "nothing is sent");
    assert_eq!(c.message.as_deref(), Some(why));
    c.message = None;
    c.f5(Ok(()));
    assert_eq!(c.take_outbox(), [Request::Start]);
}

/// Corrections 2026-10-22 (3): READY flips the flag at once, one request in
/// flight; the arms check runs on every press.
#[test]
fn corr3_ready_allows_one_request_at_a_time() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    let mut c = client_in(&l, room, 2);
    c.f5(Ok(()));
    assert!(c.is_ready(), "the flag follows the first press");
    c.f5(Ok(()));
    c.f5(Ok(()));
    assert_eq!(c.take_outbox(), [Request::Ready(true)], "exactly one request");
    // The reply: the seat is ready; the next press un-readies.
    c.apply(Notice::State(2, PlayerState::Ready, Extra::None), 1.0);
    assert!(c.is_ready());
    c.f5(Ok(()));
    assert_eq!(c.take_outbox(), [Request::Ready(false)]);
    c.apply(Notice::State(2, PlayerState::Waiting, Extra::None), 2.0);
    assert!(!c.is_ready());
    // A refusal turns the flag back.
    c.f5(Ok(()));
    assert!(c.is_ready());
    c.apply(Notice::Refused(Refusal::FinalRound), 3.0);
    assert!(!c.is_ready());
    c.take_outbox();
    // The check runs when un-readying too.
    c.apply(Notice::State(2, PlayerState::Ready, Extra::None), 4.0);
    c.f5(Err("no".into()));
    assert!(c.take_outbox().is_empty());
    assert_eq!(c.message.as_deref(), Some("no"));
}

// ---------------------------------------------------------------------------
// K: host, leaving, chat

fn host_room() -> (Lobby, RoomId) {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 3);
    (l, room)
}

#[test]
fn case_43_the_host_passes_to_the_first_free_player() {
    let (mut l, room) = host_room();
    l.request(2, Request::Extra(Extra::Outfitting)).unwrap();
    l.request(4, Request::Extra(Extra::Idle)).unwrap();
    l.request(1, Request::Leave).unwrap();
    assert_eq!(
        l.room(room).unwrap().host,
        3,
        "P2 of the case: the first with extra state none"
    );
}

#[test]
fn case_44_else_the_first_outfitting() {
    let (mut l, room) = host_room();
    l.request(2, Request::Extra(Extra::Outfitting)).unwrap();
    l.request(3, Request::Extra(Extra::Outfitting)).unwrap();
    l.request(4, Request::Extra(Extra::Idle)).unwrap();
    l.request(1, Request::Leave).unwrap();
    assert_eq!(l.room(room).unwrap().host, 2);
}

#[test]
fn case_45_else_the_first_player_and_the_last_one_destroys_the_room() {
    let (mut l, room) = host_room();
    l.request(2, Request::Leave).unwrap();
    l.request(3, Request::Leave).unwrap();
    l.request(4, Request::Extra(Extra::Idle)).unwrap();
    l.take_notices();
    l.request(1, Request::Leave).unwrap();
    assert_eq!(l.room(room).unwrap().host, 4);
    l.take_notices();
    l.request(4, Request::Leave).unwrap();
    assert!(l.room(room).is_none());
    let gone = l
        .take_notices()
        .into_iter()
        .any(|(a, n)| a == Audience::Channel && n == Notice::RoomDestroyed(room));
    assert!(gone);
}

#[test]
fn case_46_a_ready_new_host_is_quietly_unreadied() {
    let (mut l, room) = host_room();
    ready(&mut l, 2);
    l.take_notices();
    l.request(1, Request::Leave).unwrap();
    assert_eq!(l.room(room).unwrap().host, 2);
    assert_eq!(state(&l, room, 2), PlayerState::Waiting);
    let n = l.take_notices();
    assert!(n.iter().any(|(_, n)| *n == Notice::HostChanged(2)));
}

#[test]
fn case_47_a_host_without_the_moderator_item_cannot_kick() {
    let (mut l, room) = host_room();
    let r = l.request(1, Request::Kick { target: 2, reason: 0 });
    assert_eq!(r, Err(Refusal::NoModerator));
    assert!(l.room(room).unwrap().seat(2).is_some());
}

#[test]
fn a_moderator_host_kicks_and_the_kicked_cannot_return() {
    let mut l = lobby();
    l.config.host_kick = true;
    let room = room_with(&mut l, "TDM", 16, 2);
    l.request(1, Request::Kick { target: 2, reason: 3 }).unwrap();
    assert_eq!(l.room_of(2), None);
    let n = l.take_notices();
    assert!(
        n.iter()
            .any(|(a, n)| *a == Audience::Player(2) && *n == Notice::LeftRoom { kicked: true })
    );
    assert_eq!(
        l.request(
            2,
            Request::Enter {
                room,
                password: String::new()
            }
        ),
        Err(Refusal::Kicked)
    );
    // The host cannot kick itself or a player in the match.
    assert!(l.request(1, Request::Kick { target: 1, reason: 0 }).is_err());
}

#[test]
fn entrusting_the_host_unreadies_the_target() {
    let (mut l, room) = host_room();
    ready(&mut l, 3);
    l.request(1, Request::EntrustHost(3)).unwrap();
    assert_eq!(l.room(room).unwrap().host, 3);
    assert_eq!(state(&l, room, 3), PlayerState::Waiting);
    assert_eq!(l.request(1, Request::EntrustHost(2)), Err(Refusal::NotHost));
    assert_eq!(l.request(3, Request::EntrustHost(99)), Err(Refusal::BadTarget));
}

#[test]
fn chat_goes_to_the_room_or_the_lobby_and_invites_to_the_waiting() {
    let mut l = lobby();
    let room = room_with(&mut l, "TDM", 16, 1);
    for p in 5..=9 {
        hello(&mut l, p);
    }
    l.take_notices();
    l.request(2, Request::Chat("hi room".into())).unwrap();
    let n = l.take_notices();
    assert_eq!(n[0].0, Audience::Room(room));
    assert_eq!(l.recipients(n[0].0), [1, 2]);
    assert!(matches!(&n[0].1, Notice::Chat { from, text } if from == "P2" && text == "hi room"));
    // In the room list: the channel's players not in a room.
    l.request(5, Request::Chat("a".repeat(200))).unwrap();
    let n = l.take_notices();
    assert_eq!(l.recipients(n[0].0), [5, 6, 7, 8, 9]);
    assert!(matches!(&n[0].1, Notice::Chat { text, .. } if text.len() == 120));
    // The host invites up to five waiting players.
    for p in 10..=14 {
        hello(&mut l, p);
    }
    l.take_notices();
    l.request(1, Request::Invite).unwrap();
    let invites = l
        .take_notices()
        .into_iter()
        .filter(|(_, n)| matches!(n, Notice::Invite { .. }))
        .count();
    assert_eq!(invites, 5);
    assert_eq!(l.request(2, Request::Invite), Err(Refusal::NotHost));
}

#[test]
fn outfitting_unreadies_first_and_idle_follows_the_clock() {
    let (mut l, room) = host_room();
    let mut c = ClientLobby::new(LobbyData::built_in(), 2, "me");
    ready(&mut l, 2);
    c.apply(Notice::Entered(l.room(room).unwrap().detail()), 0.0);
    c.open_outfitting();
    assert_eq!(
        c.take_outbox(),
        [Request::Ready(false), Request::Extra(Extra::Outfitting)]
    );
    c.close_outfitting();
    assert_eq!(c.take_outbox(), [Request::Extra(Extra::None)]);
    // 60 s without input: idle; the next input: none.
    c.tick(59_999.0);
    assert!(c.take_outbox().is_empty());
    c.tick(60_000.0);
    assert_eq!(c.take_outbox(), [Request::Extra(Extra::Idle)]);
    c.input(61_000.0);
    assert_eq!(c.take_outbox(), [Request::Extra(Extra::None)]);
}

// ---------------------------------------------------------------------------
// G, X: the result and the way back

#[test]
fn case_58_the_result_screen_returns_to_the_room_after_a_minute() {
    let (mut l, room) = room_in_game(true);
    l.match_over(room);
    // Ready states are lost; everyone waits, in the same room.
    assert_eq!(l.room(room).unwrap().state, RoomState::Waiting);
    for p in 1..=3 {
        assert_eq!(state(&l, room, p), PlayerState::Waiting);
    }
    let mut c = ClientLobby::new(LobbyData::built_in(), 2, "me");
    c.show_result(1000.0);
    assert!(!c.result_timed_out(60_999.0));
    assert!(c.result_timed_out(61_000.0));
    assert!(c.exit_result(61_000.0), "the room shows again");
    assert!(!c.exit_result(61_001.0));
}

#[test]
fn case_60_leaving_the_match_returns_to_the_room() {
    let (mut l, room) = room_in_game(true);
    let mut c = ClientLobby::new(LobbyData::built_in(), 2, "me");
    c.leave_match();
    let out = c.take_outbox();
    assert_eq!(out, [Request::ReturnToRoom(ReturnReason::Normal)]);
    l.request(2, out[0].clone()).unwrap();
    assert_eq!(state(&l, room, 2), PlayerState::Waiting);
    // The match is told the player dropped out of it.
    assert!(
        l.take_events()
            .iter()
            .any(|e| matches!(e, LobbyEvent::Dropped { player: 2, .. }))
    );
    assert!(l.room_of(2).is_some(), "still in the room");
}

#[test]
fn case_61_start_again_sends_them_back_into_the_same_match() {
    let (mut l, room) = room_in_game(true);
    l.request(2, Request::ReturnToRoom(ReturnReason::Normal)).unwrap();
    l.take_events();
    l.request(2, Request::Start).unwrap();
    assert_eq!(state(&l, room, 2), PlayerState::InGame);
    assert!(
        l.take_events()
            .iter()
            .any(|e| matches!(e, LobbyEvent::Expect { player: 2, .. }))
    );
}

#[test]
fn a_dropped_connection_leaves_the_room() {
    let (mut l, room) = host_room();
    l.disconnect(1);
    assert_eq!(l.room(room).unwrap().host, 2);
    assert!(!l.has_player(1));
}
