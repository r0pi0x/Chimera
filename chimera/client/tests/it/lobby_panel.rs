//! The pre-match lobby drawn on the UI kit (`client::lobby_panel`, the
//! starting grid) under a theme that lays it out: one list with you
//! first, the lanes carry their marks (HOST on the host's lane, the robot
//! as a bot's avatar, YOU as the avatar's white frame), each load a kit
//! load line (joining the sweep, downloading stripes, loading the white
//! fill, ready no bar), the host's Auto/Hold, Start now, Add bot and Kick
//! on the lane under the pointer, the waiting mark, the countdown's
//! ticks light as it runs, the session wins
//! (`LobbyRow::wins`) show as a trophy and count, the leader's marked; the shell's theme keeps
//! the old panel. The clock is stepped by hand (`UiClock::manual`).

use bevy::prelude::*;
use chimera_client::client::{
    lobby_panel::{CountdownBar, LobbyKickSwap, LobbyMark, LobbyPlayerRow, LobbyWait, LobbyWins},
    prematch_lobby::{LobbyView, PreMatchLobbyPlugin},
};
use chimera_core::core::{LocalPlayer, NetRole};
use chimera_net::{
    net::{HOST_ID, NetSettings, client::LocalClientId},
    prematch::{LobbyRow, NetLobby, status},
};
use chimera_sim::SimBasePlugins;
use chimera_ui::{ChimeraUiPlugin, Theme, UiClock};
use chimera_world::harness::Sim;

/// A theme with the lobby's layout (the Ultimate Gun Game's numbers).
const THEME: &str = r#"
name = "test"
[type.display_bold]
face = "display"
weight = 700
upper = true
[layout.lobby]
mode_w = 18.0
map_w = 27.4
card_h = 13.0
big = 3.0
map_big = 2.0
row_h = 2.9
avatar = 2.3
status_w = 11.2
ping_w = 4.6
name = 0.9
tag = 0.62
ticks_h = 1.4
countdown_ticks = 48.0
countdown_hot = 3.0
"#;

const JOINER: u64 = 5;

fn row(id: Option<u64>, name: &str, status: u8) -> LobbyRow {
    LobbyRow { id, name: name.into(), status, done: 0, total: 0, step: chimera_net::prematch::NO_STEP, permille: 0, ping_ms: id.filter(|&i| i != HOST_ID).map(|_| 31), wins: 0, avatar: 0, party: 0 }
}

/// A listen host's lobby: the host, a joiner not ready, a bot; 6 slots.
fn sim(theme: Option<&str>) -> Sim {
    let theme = theme.map(|t| Theme::from_toml(t).unwrap());
    let mut sim = Sim::with(SimBasePlugins, move |app| {
        if let Some(t) = theme {
            app.insert_resource(t);
        }
        app.add_plugins((ChimeraUiPlugin, PreMatchLobbyPlugin)).insert_resource(UiClock::manual());
        if !app.world().contains_resource::<Assets<Image>>() {
            app.init_asset::<Image>();
        }
    });
    sim.app.insert_resource(NetRole::Server).insert_resource(NetSettings { maxplayers: 6, ..default() });
    sim.app.world_mut().spawn(LocalPlayer);
    sim.app.world_mut().spawn(NetLobby {
        map: "cs_source:de_dust2".into(),
        mode: "Ultimate Gun Game".into(),
        rows: vec![
            row(Some(HOST_ID), "kestrel", status::READY),
            row(Some(JOINER), "nadia", status::NOT_READY),
            row(None, "Rook", status::READY),
        ],
        waiting: 1,
        wait_left: Some(60),
        countdown: None,
        host_start: false,
        settings: vec![("Length".into(), "5 min".into()), ("Cross-game balance".into(), "On".into())],
    });
    sim.ticks(2);
    sim
}

fn marks(sim: &mut Sim) -> Vec<LobbyMark> {
    let w = sim.app.world_mut();
    let mut m: Vec<LobbyMark> = w.query::<&LobbyMark>().iter(w).copied().collect();
    m.sort_by_key(|m| *m as u8);
    m
}

fn you_rows(sim: &mut Sim) -> Vec<usize> {
    let w = sim.app.world_mut();
    w.query::<&LobbyPlayerRow>().iter(w).filter(|r| r.you).map(|r| r.index).collect()
}

/// The load line of lane `index` (the kit's `load_line`).
fn progress(sim: &mut Sim, index: usize) -> chimera_ui::load::Load {
    use chimera_ui::load::LoadLine;
    let w = sim.app.world_mut();
    let bars: Vec<(Entity, LoadLine)> = w.query::<(Entity, &LoadLine)>().iter(w).map(|(e, b)| (e, *b)).collect();
    for (e, b) in bars {
        let mut at = e;
        while let Some(parent) = w.get::<ChildOf>(at).map(|c| c.parent()) {
            if let Some(r) = w.get::<LobbyPlayerRow>(parent)
                && r.index == index
            {
                return b.0;
            }
            at = parent;
        }
    }
    panic!("no status on row {index}");
}

fn step(sim: &mut Sim, secs: f32) {
    sim.app.world_mut().resource_mut::<UiClock>().step(secs);
    sim.ticks(1);
}

fn set_status(sim: &mut Sim, index: usize, s: u8) {
    let w = sim.app.world_mut();
    let mut l = w.query::<&mut NetLobby>().single_mut(w).unwrap();
    l.rows[index].status = s;
}

#[test]
fn lobby_rows_carry_host_bot_you() {
    let mut s = sim(Some(THEME));
    // The host's own row: HOST; the bot's robot; the host's row is YOU
    // (its avatar's white frame), first in the list.
    assert_eq!(marks(&mut s), vec![LobbyMark::Host, LobbyMark::Bot]);
    assert_eq!(you_rows(&mut s), vec![0]);
    let w = s.app.world_mut();
    assert_eq!(w.query::<&chimera_ui::avatar::YouFrame>().iter(w).count(), 2, "your row's avatar and the bar's");
    let v = s.app.world().resource::<LobbyView>().clone();
    assert_eq!((v.game.as_str(), v.slots), ("", Some(6)), "no games registered here; the host's maxplayers");
    // Three slots free: three rows reading "Open".
    let w = s.app.world_mut();

    // The joiner's view: HOST alone on the host's row, YOU on its own.
    s.app.insert_resource(NetRole::Client).insert_resource(LocalClientId(JOINER));
    s.ticks(2);
    assert_eq!(marks(&mut s), vec![LobbyMark::Host, LobbyMark::Bot]);
    assert_eq!(you_rows(&mut s), vec![1]);
    // Your row comes first.
    let w = s.app.world_mut();
    let first = w.query::<(&LobbyPlayerRow, &ChildOf)>().iter(w).map(|(r, _)| r.index).next();
    assert_eq!(first, Some(1), "you lead the list");
}

/// Loaded is ready: a filled bar; a player still downloading a hollow
/// bar filling with its share; the host's hero is Start and nothing asks
/// to ready up.
#[test]
fn lobby_status_is_the_load_line() {
    use chimera_ui::load::{Load as Progress, LoadTrack};
    let mut s = sim(Some(THEME));
    assert_eq!(progress(&mut s, 0), Progress::Ready);
    assert_eq!(progress(&mut s, 1), Progress::Ready, "on the map: loaded (no ready-up)");
    assert_eq!(progress(&mut s, 2), Progress::Ready, "a bot is ready");
    let w = s.app.world_mut();
    assert_eq!(w.query::<&LoadTrack>().iter(w).count(), 0, "ready: no bar at all");
    {
        let w = s.app.world_mut();
        let mut l = w.query::<&mut NetLobby>().single_mut(w).unwrap();
        l.rows[1].status = status::DOWNLOADING;
        l.rows[1].done = 42;
        l.rows[1].total = 100;
    }
    s.ticks(2);
    assert_eq!(progress(&mut s, 1), Progress::Downloading(0.42));
    {
        let w = s.app.world_mut();
        let mut l = w.query::<&mut NetLobby>().single_mut(w).unwrap();
        l.rows[1].status = status::LOADING;
        (l.rows[1].step, l.rows[1].permille) = (2, 620);
    }
    s.ticks(2);
    assert_eq!(progress(&mut s, 1), Progress::Loading(0.62));
    let w = s.app.world_mut();
    assert_eq!(w.query::<&LoadTrack>().iter(w).count(), 1, "one bar: the one still loading");
    set_status(&mut s, 1, status::CONNECTING);
    s.ticks(2);
    assert_eq!(progress(&mut s, 1), Progress::Joining);
    let w = s.app.world_mut();
    assert_eq!(w.query::<&LobbyWait>().single(w).unwrap(), &LobbyWait::Waiting, "waiting for nadia");
    let w = s.app.world_mut();
    use chimera_client::client::prematch_lobby::LobbyAction;
    let start = w.query::<&LobbyAction>().iter(w).filter(|a| **a == LobbyAction::Ready).count();
    assert_eq!(start, 1, "the host's Start now");
    let w = s.app.world_mut();
    let acts: Vec<LobbyAction> = w.query::<&LobbyAction>().iter(w).copied().collect();
    assert!(acts.contains(&LobbyAction::Hold(false)) && acts.contains(&LobbyAction::Hold(true)) && acts.contains(&LobbyAction::AddBot), "{acts:?}");
}

#[test]
fn the_countdown_lights_its_ticks() {
    let mut s = sim(Some(THEME));
    let lit = |s: &mut Sim| {
        let now = s.app.world().resource::<UiClock>().now;
        let w = s.app.world_mut();
        w.query::<&CountdownBar>().single(w).unwrap().lit(now)
    };
    let w = s.app.world_mut();
    assert_eq!(w.query::<&CountdownBar>().iter(w).count(), 0, "no standby before the countdown");
    let set = |s: &mut Sim, c| {
        let w = s.app.world_mut();
        w.query::<&mut NetLobby>().single_mut(w).unwrap().countdown = Some(c);
    };
    set(&mut s, 6);
    step(&mut s, 0.0);
    assert_eq!(lit(&mut s), (0, 24), "6 s: none lit yet; the last 3 s hot");
    step(&mut s, 0.5);
    assert_eq!(lit(&mut s).0, 4, "half a second in, on the clock");
    set(&mut s, 3);
    step(&mut s, 0.0);
    assert_eq!(lit(&mut s).0, 24);
    set(&mut s, 1);
    step(&mut s, 0.9);
    let (n, hot) = lit(&mut s);
    assert!(n > hot && n <= 48, "the end lit in the accent ({n} lit, hot from {hot})");
}

#[test]
fn the_shell_theme_keeps_the_old_panel() {
    let mut s = sim(None);
    assert!(s.app.world().resource::<LobbyView>().shown);
    assert!(marks(&mut s).is_empty());
    let w = s.app.world_mut();
    assert_eq!(w.query::<&LobbyPlayerRow>().iter(w).count(), 0);
}

/// The session wins the server sends on the rows (`LobbyRow::wins`):
/// a trophy and the count beside each name with any, the session's
/// leader marked; none without.
#[test]
fn session_wins_show_in_the_lobby() {
    let mut sim = sim(Some(THEME));
    let wins = |sim: &mut Sim| {
        let w = sim.app.world_mut();
        let mut v: Vec<LobbyWins> = w.query::<&LobbyWins>().iter(w).copied().collect();
        v.sort_by_key(|l| l.wins);
        v
    };
    assert!(wins(&mut sim).is_empty(), "nobody has won yet");
    {
        let w = sim.app.world_mut();
        let mut l = w.query::<&mut NetLobby>().single_mut(w).unwrap();
        l.rows[1].wins = 1;
        l.rows[2].wins = 3;
    }
    sim.ticks(2);
    assert_eq!(wins(&mut sim), [LobbyWins { wins: 1, leader: false }, LobbyWins { wins: 3, leader: true }]);
    let v = sim.app.world().resource::<LobbyView>().rows.iter().map(|r| r.wins).collect::<Vec<_>>();
    assert_eq!(v, [0, 1, 3], "the view carries them");
}

/// A player's name is written as it is, and stays so across updates (a
/// status change rebuilds the rows; the text kit's upper-casing and the
/// ellipsis must not flip it between capitals and as written).
#[test]
fn player_names_keep_their_case_across_updates() {
    let mut s = sim(Some(THEME));
    let names = |s: &mut Sim| -> Vec<String> {
        let w = s.app.world_mut();
        let mut v: Vec<String> = w
            .query::<(&Text, &chimera_ui::UiText)>()
            .iter(w)
            .filter(|(t, u)| u.role == chimera_ui::Role::DisplayBold && ["kestrel", "nadia", "Rook"].iter().any(|n| t.0.eq_ignore_ascii_case(n)))
            .map(|(t, _)| t.0.clone())
            .collect();
        v.sort();
        v
    };
    let first = names(&mut s);
    assert_eq!(first, ["Rook", "kestrel", "nadia"], "as written");
    for st in [status::READY, status::NOT_READY, status::READY] {
        set_status(&mut s, 1, st);
        for _ in 0..6 {
            step(&mut s, 0.1);
            assert_eq!(names(&mut s), first, "the same on every frame after an update");
        }
    }
}

/// A joiner's lobby (docs/plans/active/lobby-between-maps.md, step 5): the
/// match's settings are told in the mode's column and the kit's Leave
/// button is on the action bar (the host has Start instead), so a
/// player can go before being put into a match it doesn't want.
#[test]
fn a_joiners_lobby_tells_the_settings_and_has_leave() {
    use chimera_client::client::prematch_lobby::LobbyAction;
    let texts = |s: &mut Sim| -> Vec<String> {
        let w = s.app.world_mut();
        w.query::<&Text>().iter(w).map(|t| t.0.to_lowercase()).collect()
    };
    let leaves = |s: &mut Sim| {
        let w = s.app.world_mut();
        w.query::<&LobbyAction>().iter(w).filter(|a| **a == LobbyAction::Leave).count()
    };
    // The host: its settings are shown, no Leave button (Start instead).
    let mut s = sim(Some(THEME));
    let t = texts(&mut s);
    assert!(t.iter().any(|t| t == "cross-game balance") && t.iter().any(|t| t == "5 min"), "{t:?}");
    assert_eq!(leaves(&mut s), 0);
    // A joiner.
    let mut j = sim(Some(THEME));
    let host_player = {
        let w = j.app.world_mut();
        w.query_filtered::<Entity, With<LocalPlayer>>().iter(w).collect::<Vec<_>>()
    };
    for e in host_player {
        j.app.world_mut().despawn(e);
    }
    j.app.insert_resource(NetRole::Client).insert_resource(LocalClientId(JOINER));
    j.ticks(3);
    let t = texts(&mut j);
    assert!(t.iter().any(|t| t == "cross-game balance") && t.iter().any(|t| t == "on"), "{t:?}");
    assert_eq!(leaves(&mut j), 1);
    assert!(t.iter().any(|t| t == "leave"), "{t:?}");
}

/// The host's Kick (the user's review, 2026-11-22): on every lane but
/// the host's own, shown only under the pointer, the ping in its place
/// otherwise; `ui_lobby_kick 0` hides it; a joiner has none. A bot shows
/// its robot, no BOT word.
#[test]
fn the_host_kicks_from_the_lane_under_the_pointer() {
    use bevy::ui::DeprecatedInteraction as Interaction;
    use chimera_client::client::prematch_lobby::{LobbyAction, LobbyKick};
    let mut s = sim(Some(THEME));
    let kicks = |s: &mut Sim| {
        let w = s.app.world_mut();
        let mut v: Vec<u8> = w.query::<&LobbyAction>().iter(w).filter_map(|a| if let LobbyAction::Kick(i) = a { Some(*i) } else { None }).collect();
        v.sort();
        v
    };
    assert_eq!(kicks(&mut s), [1, 2], "nadia and the bot; not the host");
    let shown = |s: &mut Sim| {
        let w = s.app.world_mut();
        w.query::<(&LobbyKickSwap, &Visibility)>().iter(w).filter(|(k, v)| k.kick && **v != Visibility::Hidden).count()
    };
    assert_eq!(shown(&mut s), 0, "no Kick at rest");
    let lane = {
        let w = s.app.world_mut();
        w.query::<(Entity, &LobbyPlayerRow)>().iter(w).find(|(_, r)| r.index == 1).unwrap().0
    };
    *s.app.world_mut().get_mut::<Interaction>(lane).unwrap() = Interaction::Hovered;
    s.ticks(1);
    assert!(shown(&mut s) >= 1, "Kick under the pointer");
    let texts: Vec<String> = {
        let w = s.app.world_mut();
        w.query::<&Text>().iter(w).map(|t| t.0.clone()).collect()
    };
    assert!(!texts.iter().any(|t| t == "BOT"), "no BOT word: {texts:?}");
    s.app.insert_resource(LobbyKick(0));
    s.ticks(2);
    assert!(kicks(&mut s).is_empty(), "the host hid Kick");
}

/// Hold (the user's call, 2026-11-22: Auto the default, the host may hold
/// for a player who'll be right back): the segmented choice shows it and
/// the waiting mark says the host holds; counting down, the ticks.
#[test]
fn the_waiting_mark_tells_held_and_counting() {
    let mut s = sim(Some(THEME));
    let wait = |s: &mut Sim| {
        let w = s.app.world_mut();
        *w.query::<&LobbyWait>().single(w).unwrap()
    };
    assert_eq!(wait(&mut s), LobbyWait::Waiting);
    {
        let w = s.app.world_mut();
        w.query::<&mut NetLobby>().single_mut(w).unwrap().host_start = true;
    }
    s.ticks(2);
    assert_eq!(wait(&mut s), LobbyWait::Held);
    let w = s.app.world_mut();
    assert!(w.query::<&chimera_ui::load::BusyTick>().iter(w).count() > 0, "the busy squares");
    {
        let w = s.app.world_mut();
        let mut l = w.query::<&mut NetLobby>().single_mut(w).unwrap();
        l.host_start = false;
        l.countdown = Some(3);
    }
    s.ticks(2);
    assert_eq!(wait(&mut s), LobbyWait::Counting);
}

/// Parties: your party's lanes first, joined and edged in the accent,
/// another party together after, then the rest.
#[test]
fn parties_are_grouped_and_edged() {
    use chimera_client::client::lobby_panel::{PartyEdge, list_order};
    let mut s = sim(Some(THEME));
    {
        let w = s.app.world_mut();
        let mut l = w.query::<&mut NetLobby>().single_mut(w).unwrap();
        let mut a = row(Some(7), "ally", status::READY);
        a.party = 1;
        let mut o1 = row(Some(8), "other1", status::READY);
        o1.party = 2;
        let mut o2 = row(Some(9), "other2", status::READY);
        o2.party = 2;
        l.rows[0].party = 1;
        l.rows.insert(1, o1);
        l.rows.push(a);
        l.rows.push(o2);
    }
    s.ticks(2);
    let v = s.app.world().resource::<LobbyView>().clone();
    let names: Vec<&str> = list_order(&v.rows).iter().map(|&i| v.rows[i].name.as_str()).collect();
    assert_eq!(names, ["kestrel", "ally", "other1", "other2", "nadia", "Rook"]);
    let w = s.app.world_mut();
    let mut edges: Vec<u8> = w.query::<&PartyEdge>().iter(w).map(|e| e.0).collect();
    edges.sort();
    assert_eq!(edges, [1, 1, 2, 2]);
}
