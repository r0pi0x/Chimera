//! The pre-match lobby drawn on the UI kit (`client::lobby_panel`) under a
//! theme that lays it out: the rows carry their marks (YOU and HOST fused
//! on the host's own row, HOST alone on the host's row seen by another
//! player, the robot on a bot's, YOU's row outlined), a block turning
//! ready pops back to its size on the kit's clock, free slots read
//! "Open", the countdown's ticks light as it runs, the session wins
//! (`LobbyRow::wins`) show as a trophy and count, the leader's marked; the shell's theme keeps
//! the old panel. The clock is stepped by hand (`UiClock::manual`).

use bevy::prelude::*;
use chimera_client::client::{
    lobby_panel::{CountdownBar, LobbyMark, LobbyPlayerRow, LobbyWins, OpenSlot, ReadyBlock},
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
[motion.dur.lobby]
ready_pop = 0.32
[layout.lobby]
margin = 4.6
header_y = 3.8
lockup = 1.05
title = 1.2
rule_y = 7.6
col_y = 10.6
left_w = 50.0
right_x = 60.4
big = 3.0
map_big = 2.4
row_h = 3.2
open_h = 2.9
col_pos = 2.6
col_tag = 7.4
col_status = 12.6
name = 0.92
label = 0.66
head = 0.56
tag = 0.62
kv = 0.64
key = 0.62
bot = 0.94
block_w = 1.25
block_h = 0.85
map_h = 11.4
ticks_h = 1.4
countdown_ticks = 48.0
countdown_hot = 3.0
foot_y = 3.4
corner = 2.4
"#;

const JOINER: u64 = 5;

fn row(id: Option<u64>, name: &str, status: u8) -> LobbyRow {
    LobbyRow { id, name: name.into(), status, done: 0, total: 0, ping_ms: id.filter(|&i| i != HOST_ID).map(|_| 31), wins: 0 }
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

/// The status block of row `index`: its state and drawn height scale.
fn block(sim: &mut Sim, index: usize) -> (bool, f32) {
    let w = sim.app.world_mut();
    let blocks: Vec<(Entity, ReadyBlock, f32)> =
        w.query::<(Entity, &ReadyBlock, &UiTransform)>().iter(w).map(|(e, b, t)| (e, *b, t.scale.y)).collect();
    for (e, b, s) in blocks {
        let mut at = e;
        while let Some(parent) = w.get::<ChildOf>(at).map(|c| c.parent()) {
            if let Some(r) = w.get::<LobbyPlayerRow>(parent)
                && r.index == index
            {
                return (b.ready, s);
            }
            at = parent;
        }
    }
    panic!("no block on row {index}");
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
    // The host's own row: one fused chip; the bot's robot; the host's row is YOU.
    assert_eq!(marks(&mut s), vec![LobbyMark::YouHost, LobbyMark::Bot]);
    assert_eq!(you_rows(&mut s), vec![0]);
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
}

#[test]
fn lobby_ready_block_flips() {
    let mut s = sim(Some(THEME));
    assert_eq!(block(&mut s, 1), (false, 1.0), "not ready: an outline at its size");
    assert_eq!(block(&mut s, 0), (true, 1.0));
    set_status(&mut s, 1, status::READY);
    step(&mut s, 0.0);
    let (ready, scale) = block(&mut s, 1);
    assert!(ready);
    assert!(scale > 1.5, "it pops (scale y {scale})");
    step(&mut s, 0.1);
    let (_, mid) = block(&mut s, 1);
    assert!(mid < scale && mid > 1.0, "settling ({mid})");
    step(&mut s, 0.3);
    assert_eq!(block(&mut s, 1), (true, 1.0), "back to its size after ready_pop");
    assert_eq!(block(&mut s, 0), (true, 1.0), "the others never popped");
}

#[test]
fn the_countdown_lights_its_ticks() {
    let mut s = sim(Some(THEME));
    let lit = |s: &mut Sim| {
        let now = s.app.world().resource::<UiClock>().now;
        let w = s.app.world_mut();
        w.query::<&CountdownBar>().single(w).unwrap().lit(now)
    };
    assert_eq!(lit(&mut s).0, 0, "nothing lit before the countdown");
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
