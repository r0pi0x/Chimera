//! The pre-match lobby drawn on the UI kit (`client::lobby_panel`) under a
//! theme that lays it out: one list with you first, the rows carry their
//! marks (HOST on the host's row, the robot on a bot's avatar, YOU as the
//! avatar's white frame), each status a progress bar (loaded is ready:
//! filled), free slots read "Open", the host's Start, the countdown's
//! ticks light as it runs, the session wins
//! (`LobbyRow::wins`) show as a trophy and count, the leader's marked; the shell's theme keeps
//! the old panel. The clock is stepped by hand (`UiClock::manual`).

use bevy::prelude::*;
use chimera_client::client::{
    lobby_panel::{CountdownBar, LobbyMark, LobbyPlayerRow, LobbyWins, OpenSlot},
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

/// The status bar of row `index` (the progress component).
fn progress(sim: &mut Sim, index: usize) -> chimera_ui::controls::Progress {
    use chimera_ui::controls::ProgressBar;
    let w = sim.app.world_mut();
    let bars: Vec<(Entity, ProgressBar)> = w.query::<(Entity, &ProgressBar)>().iter(w).map(|(e, b)| (e, *b)).collect();
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
    assert_eq!(w.query::<&OpenSlot>().iter(w).count(), 3);

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
fn lobby_status_is_the_progress_bar() {
    use chimera_ui::controls::Progress;
    let mut s = sim(Some(THEME));
    assert_eq!(progress(&mut s, 0), Progress::Ready);
    assert_eq!(progress(&mut s, 1), Progress::Ready, "on the map: loaded (no ready-up)");
    {
        let w = s.app.world_mut();
        let mut l = w.query::<&mut NetLobby>().single_mut(w).unwrap();
        l.rows[1].status = status::DOWNLOADING;
        l.rows[1].done = 42;
        l.rows[1].total = 100;
    }
    s.ticks(2);
    assert_eq!(progress(&mut s, 1), Progress::Running(0.42));
    set_status(&mut s, 1, status::CONNECTING);
    s.ticks(2);
    assert_eq!(progress(&mut s, 1), Progress::Connecting);
    let w = s.app.world_mut();
    use chimera_client::client::prematch_lobby::LobbyAction;
    let start = w.query::<&LobbyAction>().iter(w).filter(|a| **a == LobbyAction::Ready).count();
    assert_eq!(start, 1, "the host's Start");
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
