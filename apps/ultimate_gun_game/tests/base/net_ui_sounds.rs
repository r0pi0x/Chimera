//! The UI sound kit on remote clients of a dedicated server (`NetSim`:
//! the server has no local player; two clients, the party's simulation
//! on the greybox, the Ultimate Gun Game's Signal theme and sound table):
//! every cue the shell's screens send (`chimera_client::client::ui_cues`)
//! comes from what a client receives (the replicated rung and ladder, the
//! lobby, the match's effects and results, the vote), so each client
//! hears its own cues once per event and none of another player's: a
//! kill's rank-up only on the killer's client, the knife on the last
//! rung, no cue for a rung down, the lobby's ready, the others readying,
//! the countdown and the go, the air strike's alert, pulses, chip and
//! impact (also over a slow link; none of what passed for a client that
//! joins after it began), the match end's reveal, a vote cast, the others' votes and
//! the lock; `snd_uivolume` sets the bus on a client. What plays is read
//! from each client's `UiVoices` (the files are silent).

use std::time::Duration;

use bevy::prelude::*;
use chimera_client::client::{
    audio::SoundOptionsPlugin,
    logo_pass::LogoPassPlugin,
    match_effects::{EffectsView, MatchEffectsPlugin},
    match_end::{MatchEndPlugin, MatchEndView},
    prematch_lobby::PreMatchLobbyPlugin,
    ui_cues::UiCuesPlugin,
    ui_sound::{UiSoundPlugin, UiSounds, UiVoices},
};
use chimera_core::{
    console::Console,
    core::{Damage, DamageKind, Dead, God, Hitgroup},
};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::minigame::{
    ladder::{self, Ladder, Rung},
    vote::MapVote,
};
use chimera_ui::{ChimeraUiPlugin, UiClock};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use party_roster::PartyPartsPlugin;
use ultimate_gun_game::{MINIGAME, UltimateGunGamePlugin, signal};

const LINK: LinkConditions = LinkConditions {
    latency: Duration::from_millis(20),
    jitter: Duration::ZERO,
    loss: 0.0,
};

/// Every app: the party, UGG, the lobby, and the client's screens that
/// send cues with the bus (the server runs them too; with no local
/// player and no screens up it sends none).
fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin, UltimateGunGamePlugin::default()));
    chimera_net::prematch::enable(app);
    app.insert_resource(chimera_ui::Theme::from_toml(&signal::THEME).unwrap());
    app.add_plugins((ChimeraUiPlugin, LogoPassPlugin, PreMatchLobbyPlugin, MatchEndPlugin, MatchEffectsPlugin));
    app.add_plugins((SoundOptionsPlugin, UiSoundPlugin, UiCuesPlugin));
    if !app.world().contains_resource::<Assets<Image>>() {
        app.init_asset::<Image>();
    }
    app.insert_resource(UiClock::manual());
    app.insert_resource(UiSounds::silent(&signal::UI_SOUND_TABLE).unwrap());
}

/// A dedicated server and `clients` clients, with or without the
/// pre-match lobby (ready-up on, two players, 3 s of countdown, started
/// by everyone being ready rather than the host).
fn sim(clients: usize, lobby: bool) -> NetSim {
    let mut sim = NetSim::new(CaSim, LINK, 7, clients, setup);
    sim.set_frame(TICK_INTERVAL);
    let lines: &[&str] =
        if lobby { &["mp_lobby 1", "mp_lobby_host_start 0", "mp_lobby_ready_up 1", "mp_lobby_min_players 2", "mp_lobby_countdown 3"] } else { &["mp_lobby 0", "mp_vote_all_voted 3"] };
    for l in lines {
        con(&mut sim, l);
    }
    chimera_net::minigame_net::change_match(sim.server.app.world_mut(), MINIGAME, Some("greybox")).unwrap();
    sim.until_joined(900);
    if lobby {
        // Up once the logo pass has played on each client.
        step_until(&mut sim, 200, |s| (0..clients).all(|i| !played(s, i).is_empty()));
        step(&mut sim, 60);
    } else {
        step(&mut sim, 60);
    }
    sim
}

/// One frame everywhere, the clients' UI clocks moving with it.
fn step(sim: &mut NetSim, n: u64) {
    for _ in 0..n {
        sim.step();
        for c in &mut sim.clients {
            c.app.world_mut().resource_mut::<UiClock>().step(TICK_INTERVAL as f32);
        }
    }
}

/// Step (as `step`) until `done`, at most `max` frames.
fn step_until(sim: &mut NetSim, max: u64, mut done: impl FnMut(&mut NetSim) -> bool) -> bool {
    for _ in 0..max {
        if done(sim) {
            return true;
        }
        step(sim, 1);
    }
    done(sim)
}

fn con(sim: &mut NetSim, line: &str) {
    sim.server.app.world_mut().resource_mut::<Console>().submit(line);
}

/// What client `i` played, in order.
fn played(sim: &NetSim, i: usize) -> Vec<String> {
    sim.clients[i].app.world().resource::<UiVoices>().played.iter().map(|p| p.event.clone()).collect()
}

fn count(sim: &NetSim, i: usize, event: &str) -> usize {
    played(sim, i).iter().filter(|e| *e == event).count()
}

/// What client `i` played since `from` plays.
fn since(sim: &NetSim, i: usize, from: usize) -> Vec<String> {
    played(sim, i)[from..].to_vec()
}

/// Client `a`'s character kills client `b`'s on the server, once `b` is
/// back and unprotected.
fn kill(sim: &mut NetSim, a: usize, b: usize) {
    let (ea, eb) = (sim.character_of(a).unwrap(), sim.character_of(b).unwrap());
    assert!(
        step_until(sim, 900, |s| {
            let w = s.server.app.world();
            w.get::<God>(eb).is_none() && w.get::<Dead>(eb).is_none()
        }),
        "the victim is back"
    );
    let at = sim.server.position(eb);
    sim.server.app.world_mut().write_message(Damage {
        target: eb,
        attacker: Some(ea),
        amount: 500.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Head,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    step(sim, 60);
}

fn rung_of(sim: &mut NetSim, i: usize) -> usize {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world().get::<Rung>(me).unwrap().0
}

const RANK: [&str; 3] = ["rank_up", "rank_knife", "rank_chain"];

#[test]
fn a_kill_plays_the_killers_rank_up_once_on_its_client_and_nothing_on_the_others() {
    let mut sim = sim(2, false);
    let before = (played(&sim, 0).len(), played(&sim, 1).len());
    for k in 1..=3 {
        kill(&mut sim, 0, 1);
        step(&mut sim, 40);
        assert_eq!(rung_of(&mut sim, 0), k, "client 0 sees its rung");
        let mine = since(&sim, 0, before.0);
        assert_eq!(mine.iter().filter(|e| *e == "rank_up").count(), k, "one rank-up a kill on the killer's client: {mine:?}");
        let theirs = since(&sim, 1, before.1);
        assert!(!theirs.iter().any(|e| RANK.contains(&e.as_str())), "the victim's client hears no rank cue: {theirs:?}");
    }
    // The rank-up's render: the nearest to the rung's place on the ladder.
    let rungs = sim.clients[0].app.world().resource::<Ladder>().rungs.len();
    let last = sim.clients[0].app.world().resource::<UiVoices>().played.last().unwrap().file.clone();
    let want = format!("rank_p{:02}", (3.0 / (rungs - 1) as f32 * 16.0).round() as u32);
    assert_eq!(last, want, "rung 3 of {rungs}");
}

#[test]
fn the_last_rung_plays_the_knife_and_a_rung_down_plays_nothing() {
    let mut sim = sim(2, false);
    let a = sim.character_of(0).unwrap();
    let l = sim.server.app.world().resource::<Ladder>().clone();
    // Two below the last: one rank-up when set, then the kill to the last.
    ladder::set_rung(sim.server.app.world_mut(), a, l.rungs.len() - 3, &l);
    step(&mut sim, 120);
    let from = played(&sim, 0).len();
    ladder::set_rung(sim.server.app.world_mut(), a, l.rungs.len() - 2, &l);
    step(&mut sim, 120);
    assert_eq!(since(&sim, 0, from), ["rank_up"]);
    let from = played(&sim, 0).len();
    kill(&mut sim, 0, 1);
    assert_eq!(rung_of(&mut sim, 0), l.rungs.len() - 1);
    assert_eq!(since(&sim, 0, from), ["rank_knife"], "the last rung: the knife alone");
    // A rung down (as a knifing's or a suicide's): no cue.
    let from = played(&sim, 0).len();
    ladder::set_rung(sim.server.app.world_mut(), a, l.rungs.len() - 3, &l);
    step(&mut sim, 60);
    assert_eq!(rung_of(&mut sim, 0), l.rungs.len() - 3);
    assert!(!since(&sim, 0, from).iter().any(|e| RANK.contains(&e.as_str())), "{:?}", since(&sim, 0, from));
    assert!(played(&sim, 1).iter().all(|e| !RANK.contains(&e.as_str())), "{:?}", played(&sim, 1));
}

#[test]
fn the_lobby_and_countdown_cues_reach_each_client_once() {
    let mut sim = sim(2, true);
    // The lobby came up through the logo pass.
    for i in 0..2 {
        assert_eq!(played(&sim, i), ["pass_sweep"], "client {i}");
    }
    // Each readies: its own `lobby_ready`, the other's `lobby_peer`.
    for i in 0..2 {
        sim.clients[i].app.world_mut().resource_mut::<Console>().submit("lobby_ready 1");
        step_until(&mut sim, 100, |s| count(s, i, "lobby_ready") == 1 && count(s, 1 - i, "lobby_peer") == 1);
    }
    assert_eq!(played(&sim, 0)[1..3], ["lobby_ready", "lobby_peer"]);
    assert_eq!(played(&sim, 1)[1..3], ["lobby_peer", "lobby_ready"]);
    // The countdown (3 s): 3, 2, 1, then the go as the lobby goes
    // (through the pass); the plain tick is the vote's below.
    step_until(&mut sim, 700, |s| (0..2).all(|i| count(s, i, "count_go") == 1));
    step(&mut sim, 20);
    for i in 0..2 {
        assert_eq!(
            played(&sim, i)[3..],
            ["count_tick_3", "count_tick_2", "count_tick_1", "pass_sweep", "count_go"],
            "client {i}"
        );
    }
}

#[test]
fn the_strike_cues_reach_each_client_once() {
    let mut sim = sim(2, false);
    // The air strike: alert, a pulse a second to the impact, the chip,
    // the impact; once each on both clients.
    let from = (played(&sim, 0).len(), played(&sim, 1).len());
    con(&mut sim, "surprise airstrike 3 2");
    step_until(&mut sim, 900, |s| (0..2).all(|i| count(s, i, "air_impact") == 1));
    step(&mut sim, 20);
    for (i, f) in [(0, from.0), (1, from.1)] {
        assert_eq!(since(&sim, i, f), ["air_alert", "air_pulse", "air_pulse", "hud_chip", "air_impact"], "client {i}");
    }
}

#[test]
fn the_match_end_and_vote_cues_reach_each_client_once() {
    let mut sim = sim(2, false);
    // The match won by client 0: its knife, then the end on both.
    let a = sim.character_of(0).unwrap();
    let l = sim.server.app.world().resource::<Ladder>().clone();
    ladder::set_rung(sim.server.app.world_mut(), a, l.rungs.len() - 1, &l);
    step(&mut sim, 60);
    let from = (played(&sim, 0).len(), played(&sim, 1).len());
    kill(&mut sim, 0, 1);
    // The match end comes up with its vote on both clients (through the
    // logo pass); then both vote (so it closes early), client 1 for the second map.
    let shown = |s: &mut NetSim| {
        (0..2).all(|i| {
            let v = s.clients[i].app.world().resource::<MatchEndView>();
            v.shown && v.vote.as_ref().is_some_and(|v| v.open())
        })
    };
    assert!(step_until(&mut sim, 900, shown), "the vote shows on both clients");
    chimera_net::minigame_net::vote_map(sim.clients[1].app.world_mut(), 1).unwrap();
    chimera_net::minigame_net::vote_map(sim.clients[0].app.world_mut(), 0).unwrap();
    assert!(
        step_until(&mut sim, 3000, |s| s.clients[0].app.world().get_resource::<MapVote>().is_some_and(|v| v.winner.is_some())),
        "the vote closes"
    );
    step(&mut sim, 60);
    for (i, f) in [(0, from.0), (1, from.1)] {
        let heard = since(&sim, i, f);
        let once = |e: &str| heard.iter().filter(|h| *h == e).count();
        assert_eq!(once("end_reveal"), 1, "client {i}: {heard:?}");
        assert_eq!(once("vote_lock"), 1, "client {i}: {heard:?}");
        assert_eq!(once("vote_cast_2"), usize::from(i == 1), "client {i}: only the voter's cast: {heard:?}");
        assert_eq!(once("vote_cast_1"), usize::from(i == 0), "client {i}: only the voter's cast: {heard:?}");
        assert_eq!(once("vote_other"), 1, "client {i}: the other's vote: {heard:?}");
        assert_eq!(once("count_tick"), 3, "client {i}: the vote's last 3 s: {heard:?}");
        assert!(!heard.iter().any(|e| RANK.contains(&e.as_str())), "client {i}: the end isn't a rank-up: {heard:?}");
    }
}

#[test]
fn a_strike_heard_late_still_alerts_and_a_late_joiner_hears_none_of_its_past() {
    let mut sim = sim(1, false);
    // Client 0's link slow (0.6 s each way): the strike reaches it late.
    sim.set_conditions(0, LinkConditions { latency: Duration::from_millis(600), ..LINK });
    step(&mut sim, 60);
    let from = played(&sim, 0).len();
    con(&mut sim, "surprise airstrike 3 20");
    // Client 1 joins after the strike's start (its shells fall for 20 s).
    step(&mut sim, (2.0 / TICK_INTERVAL) as u64);
    let late = sim.add_client(|_| {});
    assert!(step_until(&mut sim, 900, |s| s.local_player(late).is_some()), "client 1 joins");
    let saw = |s: &mut NetSim| !s.clients[1].app.world().resource::<EffectsView>().shown.is_empty();
    assert!(step_until(&mut sim, 300, saw), "the late joiner sees the strike");
    step(&mut sim, 900);
    assert_eq!(since(&sim, 0, from), ["air_alert", "air_pulse", "air_pulse", "hud_chip", "air_impact"], "the slow client");
    let heard = played(&sim, late);
    assert!(!heard.iter().any(|e| e.starts_with("air_") || e == "hud_chip" || RANK.contains(&e.as_str())), "the late joiner: {heard:?}");
}

#[test]
fn snd_uivolume_sets_a_clients_bus() {
    let mut sim = sim(2, false);
    sim.clients[0].app.world_mut().resource_mut::<Console>().submit("snd_uivolume 0.5");
    step(&mut sim, 2);
    let (ea, eb) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    assert!(step_until(&mut sim, 900, |s| s.server.app.world().get::<God>(eb).is_none()), "the victim is unprotected");
    let at = sim.server.position(eb);
    sim.server.app.world_mut().write_message(Damage {
        target: eb,
        attacker: Some(ea),
        amount: 500.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: Hitgroup::Head,
        kind: DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    let playing = |s: &mut NetSim| s.clients[0].app.world().resource::<UiVoices>().voices.iter().any(|v| v.event == "rank_up");
    assert!(step_until(&mut sim, 120, playing), "the rank-up plays");
    let v = sim.clients[0].app.world().resource::<UiVoices>();
    let voice = v.voices.iter().find(|v| v.event == "rank_up").unwrap();
    assert!((voice.heard() - voice.gain * 0.5).abs() < 1e-5, "{} for {} at 0.5", voice.heard(), voice.gain);
    assert_eq!(count(&sim, 0, "rank_up"), 1);
}
