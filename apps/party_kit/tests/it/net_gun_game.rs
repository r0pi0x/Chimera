//! The gun game over the network (docs/plans/active/multiplayer.md,
//! `minigame_net`): a server playing `originals:gun_game_dust2` and two
//! clients that only `connect` (on `NetSim`'s in-memory link, the party's
//! simulation on the greybox). The clients learn the minigame from the
//! server, show its HUD (rung, next gun, leader, the match's state), and
//! follow a player up the ladder: each rung's gun, CS:S's and Combat
//! Arms', drawn on both clients with rounds in it, to the winner and the
//! match over.

use std::time::Duration;

use bevy::{ecs::message::MessageCursor, prelude::*};
use chimera_core::{
    console::Console,
    core::{Dead, God, Score},
    gameplay::Gameplay,
};
use chimera_net::{harness::NetSim, minigame_net::NetMinigame, net::memory::LinkConditions};
use chimera_sim::{
    minigame::{self, MINIGAME, hud::MinigameHud, ladder::Rung, runtime::InMinigame},
    mode::{Match, Phase},
    testkit,
    weapon::{Magazine, WeaponEvent, WeaponEventKind, WeaponSounds},
};
use chimera_world::{greybox::GreyboxMapPlugin, map::PlaySound};
use combat_arms::{
    game::TICK_INTERVAL,
    harness::CaSim,
};
use party_kit::{DEFAULT_MINIGAME, PartyPartsPlugin};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, PartyPartsPlugin));
}

/// A server playing the gun game and two clients that came in with no
/// minigame of their own.
pub(crate) fn joined() -> NetSim {
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        setup,
    );
    sim.set_frame(TICK_INTERVAL);
    crate::gun_game::no_finale(sim.server.app.world_mut());
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    sim
}

fn hud_lines(sim: &NetSim, i: usize) -> Vec<String> {
    let hud = sim.clients[i].app.world().resource::<MinigameHud>();
    hud.lines.iter().map(|l| l.text.clone()).collect()
}

pub(crate) fn held(sim: &mut NetSim, i: usize) -> Option<&'static str> {
    let me = sim.local_player(i)?;
    testkit::active_id(sim.clients[i].app.world(), me)
}

/// Client `i`'s own rounds in the clip of the gun in hand.
pub(crate) fn clip(sim: &mut NetSim, i: usize) -> Option<u32> {
    let me = sim.local_player(i)?;
    let w = sim.clients[i].app.world();
    w.get::<Magazine>(testkit::active(w, me)?).map(|m| m.clip)
}

pub(crate) fn server_alive(sim: &NetSim, e: Entity) -> bool {
    sim.server.app.world().get::<Dead>(e).is_none()
}

#[test]
fn a_client_that_only_connects_plays_the_servers_minigame() {
    let mut sim = joined();
    for i in 0..2 {
        let w = sim.clients[i].app.world();
        assert_eq!(w.resource::<Gameplay>().rules, MINIGAME, "client {i} plays the minigame's rules");
        assert_eq!(w.resource::<minigame::ActiveMinigame>().def.id, DEFAULT_MINIGAME);
        assert!(w.contains_resource::<minigame::ladder::Ladder>(), "client {i} has the ladder");
        let me = sim.local_player(i).unwrap();
        let w = sim.clients[i].app.world();
        assert!(w.get::<InMinigame>(me).is_some(), "client {i}'s player is in the minigame");
        assert_eq!(w.get::<Rung>(me).map(|r| r.0), Some(0));
        assert_eq!(held(&mut sim, i), Some(cs_source::game::weapons::GLOCK), "the first rung's Glock on client {i}");
        assert!(clip(&mut sim, i).is_some_and(|c| c > 0), "rounds in client {i}'s clip");
    }
    // The match's state: live (both are in), a minigame entity for each.
    let m = sim.clients[0].app.world().resource::<Match>();
    assert_eq!(m.phase, Phase::Live);
    assert!(m.round_ends.is_some());
    let lines = hud_lines(&sim, 0);
    assert!(lines.iter().any(|l| l.starts_with("Rung 1/10")), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("Next:")), "{lines:?}");
    assert!(lines.iter().any(|l| l.starts_with("Time ")), "the match's time: {lines:?}");
    assert!(!lines.iter().any(|l| l.contains("Waiting")), "{lines:?}");
    let w = sim.clients[1].app.world_mut();
    let n = w.query::<&NetMinigame>().iter(w).count();
    assert_eq!(n, 1, "one replicated match");
}

#[test]
fn a_client_cant_start_or_restart_the_minigame_itself() {
    let mut sim = joined();
    for line in ["minigame originals:gun_game_dust2", "minigame_restart"] {
        sim.clients[0].app.world_mut().resource_mut::<Console>().submit(line);
        sim.ticks(2);
    }
    let out: Vec<String> =
        sim.clients[0].app.world().resource::<Console>().output.iter().map(|l| l.text.clone()).collect();
    assert_eq!(out.iter().filter(|l| l.contains("the server picks the minigame")).count(), 2, "{out:?}");
    assert_eq!(
        sim.clients[0].app.world().resource::<Match>().phase,
        Phase::Live,
        "the match wasn't started over"
    );
}

/// Client 0 (character `a`) shoots client 1 (`b`) dead with rung `kill`'s
/// gun, which each client sees in its owner's hand with rounds in it.
pub(crate) fn kill_once(sim: &mut NetSim, kill: usize, a: Entity, b: Entity, ladder: &[&'static str]) {
    assert_eq!(held(sim, 0), Some(ladder[kill]), "rung {kill}'s gun in client 0's hand");
    assert!(clip(sim, 0).is_some_and(|c| c > 0), "rung {kill}: {} has rounds on its client", ladder[kill]);
    // The victim back in play and unprotected, 4 m in front of the shooter.
    assert!(
        sim.until(600, |s| server_alive(s, b) && s.server.app.world().get::<God>(b).is_none()),
        "the victim is back"
    );
    sim.ticks(30);
    assert_eq!(held(sim, 1), Some(ladder[0]), "the victim, back, has the first rung's gun on its client");
    let at = sim.server.position(a) + Vec3::new(0.0, 0.0, -4.0);
    {
        let w = sim.server.app.world_mut();
        w.get_mut::<Transform>(b).unwrap().translation = at;
        w.get_mut::<avian3d::prelude::Position>(b).unwrap().0 = at;
    }
    sim.ticks(90);
    let me = sim.local_player(0).unwrap();
    let mut dead = false;
    for t in 0..(10.0 / TICK_INTERVAL) as u64 {
        let target = sim.server.position(b);
        testkit::aim_at(&mut sim.clients[0], me, target);
        sim.clients[0].intent(me).fire = t % 2 == 0;
        sim.step();
        if !server_alive(sim, b) {
            dead = true;
            break;
        }
    }
    sim.clients[0].intent(me).fire = false;
    assert!(dead, "kill {kill}: {} killed in 10 s", ladder[kill]);
    sim.ticks(40);
}

/// Client `i`'s minigame panel: the rules' status lines, then the match's
/// own line (its time, waiting or over).
fn panel(sim: &NetSim, i: usize) -> (Vec<String>, String) {
    let mut lines = hud_lines(sim, i);
    let phase = lines.pop().unwrap_or_default();
    (lines, phase)
}

/// What the server's rules tell player `who` (a server entity), as HUD text
/// (`{weapon}` and `{player}` left in).
fn server_status(sim: &mut NetSim, who: Entity) -> Vec<String> {
    let w = sim.server.app.world();
    w.get::<minigame::hud::MinigameStatus>(who).map(|s| s.0.iter().map(|l| l.text.clone()).collect()).unwrap_or_default()
}

/// The first line of the server's status for `who` without its `{...}`
/// placeholders' words: what a client's line must start with.
fn stem(line: &str) -> &str {
    line.split('{').next().unwrap_or(line).trim_end_matches([' ', ':'])
}

/// Client 0 kills client 1 with each of the first rungs' guns. Before each
/// kill and after the last, each client's minigame panel says what the
/// server's rules say to its own player (rung, next gun, leader), the time
/// on it is the server's, the match is live (never "waiting"), and the HUD
/// in use is the game of the gun in hand (CS:S's and Combat Arms'
/// alternate on this ladder).
#[test]
fn each_clients_minigame_panel_follows_the_servers_state_and_the_held_gun() {
    let mut sim = joined();
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let ladder = sim.server.app.world().resource::<minigame::ladder::Ladder>().rungs.clone();
    for kill in 0..=4usize {
        for (i, who) in [(0usize, a), (1, b)] {
            let (lines, phase) = panel(&sim, i);
            let want = server_status(&mut sim, who);
            assert_eq!(lines.len(), want.len(), "kill {kill}, client {i}: {lines:?} against the server's {want:?}");
            for (got, want) in lines.iter().zip(&want) {
                assert!(got.starts_with(stem(want)), "kill {kill}, client {i}: {got:?} against {want:?}");
            }
            let rung = if i == 0 { kill } else { 0 };
            assert!(lines[0].starts_with(&format!("Rung {}/10", rung + 1)), "kill {kill}, client {i}: {lines:?}");
            assert_eq!(
                lines.last().is_some_and(|l| l.starts_with("Leader:")),
                i == 1 && kill > 0,
                "kill {kill}, client {i}: only the one behind is told who leads: {lines:?}"
            );
            // The match: live, with the server's time.
            let time = phase.strip_prefix("Time ").unwrap_or_else(|| panic!("kill {kill}, client {i}: {phase:?}"));
            let (m, s) = time.split_once(':').unwrap();
            let shown = m.parse::<f64>().unwrap() * 60.0 + s.parse::<f64>().unwrap();
            let w = sim.server.app.world();
            let left = w.resource::<Match>().round_ends.unwrap() - w.resource::<chimera_core::core::SimClock>().now;
            assert!((shown - left).abs() < 3.0, "kill {kill}, client {i}: {phase} against {left:.1} s left");
            // The HUD in use is the held gun's game's.
            // (The one just killed holds nothing until it respawns.)
            let Some(gun) = held(&mut sim, i) else {
                assert_eq!((i, kill > 0), (1, true), "kill {kill}: client {i} holds a gun");
                continue;
            };
            let w = sim.clients[i].app.world();
            let game = w.resource::<chimera_core::game_registry::Games>().get(gun.split(':').next().unwrap()).map(|g| g.id);
            assert_eq!(Some(w.resource::<Gameplay>().hud), game, "kill {kill}, client {i}: the HUD of {gun}");
        }
        if kill < 4 {
            kill_once(&mut sim, kill, a, b, &ladder);
        }
    }
}

/// Client 0 shoots client 1 with every rung's gun to the win. Each client
/// draws the rung's gun with rounds in it, the HUD follows (rung, next
/// gun, leader), and the match ends with the shooter the winner on both.
#[test]
fn a_ladder_climbed_by_a_client_shows_on_both_clients() {
    let mut sim = joined();
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let ladder = sim.server.app.world().resource::<minigame::ladder::Ladder>().rungs.clone();
    assert_eq!(ladder.len(), 10);
    for kill in 0..10usize {
        kill_once(&mut sim, kill, a, b, &ladder);
        if kill < 9 {
            let rung = |sim: &mut NetSim, i: usize| {
                let me = sim.local_player(i).unwrap();
                sim.clients[i].app.world().get::<Rung>(me).map(|r| r.0)
            };
            assert_eq!(rung(&mut sim, 0), Some(kill + 1), "client 0 sees its rung up");
            assert_eq!(rung(&mut sim, 1), Some(0), "client 1 sees its own rung");
            let lines = hud_lines(&sim, 0);
            assert!(lines.iter().any(|l| l.starts_with(&format!("Rung {}/10", kill + 2))), "{lines:?}");
            let theirs = hud_lines(&sim, 1);
            assert!(theirs.iter().any(|l| l.starts_with("Leader:") || l.starts_with("Tied")), "{theirs:?}");
        }
    }
    let team_a = sim.server.app.world().get::<chimera_core::core::Team>(a).unwrap().0;
    assert_eq!(sim.server.app.world().resource::<Match>().winner, Some(team_a));
    assert_eq!(sim.server.app.world().get::<Score>(a).unwrap().kills, 10);
    // The match ends: over on both clients, the HUD says so.
    sim.until(1200, |s| s.clients.iter().all(|c| c.app.world().resource::<Match>().phase == Phase::MatchOver));
    for i in 0..2 {
        assert_eq!(sim.clients[i].app.world().resource::<Match>().phase, Phase::MatchOver, "client {i}");
        let lines = hud_lines(&sim, i);
        assert!(lines.iter().any(|l| l == "Match over"), "client {i}: {lines:?}");
    }
    let kills = {
        let me = sim.local_player(0).unwrap();
        sim.clients[0].app.world().get::<Score>(me).map(|s| s.kills)
    };
    assert_eq!(kills, Some(10), "the scoreboard's kills on the winner's own client");
}

/// A swap from Combat Arms' gun to CS:S's (the server's rung change) ends
/// Combat Arms' recoil on the server and on the client predicting the
/// player alike (`weapon::impact::RecoilGame`, predicted): the punch is
/// zero on both and doesn't drift down on the client.
#[test]
fn a_swap_ends_the_old_games_recoil_on_server_and_client_alike() {
    use chimera_sim::weapon::{ViewPunch, impact::RecoilGame};
    let mut sim = joined();
    let a = sim.character_of(0).unwrap();
    let set = |sim: &mut NetSim, r: usize| {
        let w = sim.server.app.world_mut();
        let l = w.resource::<minigame::ladder::Ladder>().clone();
        minigame::ladder::set_rung(w, a, r, &l);
    };
    set(&mut sim, 7);
    assert!(sim.until(300, |s| held(s, 0) == Some("combat_arms:m4a1")), "the M4A1 on the client");
    sim.ticks(60);
    let me = sim.local_player(0).unwrap();
    let ahead = sim.server.position(a) + Vec3::new(0.0, 1.5, -20.0);
    for t in 0..40u64 {
        testkit::aim_at(&mut sim.clients[0], me, ahead);
        sim.clients[0].intent(me).fire = t % 2 == 0;
        sim.step();
    }
    sim.clients[0].intent(me).fire = false;
    let punch = |w: &World, e: Entity| w.get::<ViewPunch>(e).map_or(Vec3::ZERO, |p| p.0);
    assert!(punch(sim.clients[0].app.world(), me).x > 0.01, "kicked on the client");
    set(&mut sim, 6);
    assert!(sim.until(300, |s| held(s, 0) == Some("cs_source:weapon_ak47")), "the AK-47 on the client");
    sim.ticks(10);
    for _ in 0..60 {
        sim.step();
        let (c, s) = (punch(sim.clients[0].app.world(), me), punch(sim.server.app.world(), a));
        assert!(c.length() < 1e-6 && s.length() < 1e-6, "no punch left or drifting: client {c:?}, server {s:?}");
    }
    let game = |w: &World, e: Entity| w.get::<RecoilGame>(e).map(|g| g.0.clone());
    assert_eq!(game(sim.server.app.world(), a).as_deref(), Some("cs_source"));
    assert_eq!(game(sim.clients[0].app.world(), me).as_deref(), Some("cs_source"));
}

/// Reload starts client 0's own weapons showed (`WeaponEvent::shown`: what
/// the view models play a reload animation for).
#[derive(Resource, Default)]
struct ShownReloads(u32);

fn count_shown_reloads(
    mut events: MessageReader<chimera_sim::weapon::WeaponEvent>,
    me: Query<(), With<chimera_core::core::LocalPlayer>>,
    mut n: ResMut<ShownReloads>,
) {
    for e in events.read() {
        if e.shown() && me.contains(e.owner) && matches!(e.kind, chimera_sim::weapon::WeaponEventKind::ReloadStarted) {
            n.0 += 1;
        }
    }
}

/// On client 0, rung `rung`'s gun emptied by firing (fire held after the
/// last round until the reload is over, or let go): one reload shown, one predicted reload, and the
/// server's one, over the next seconds.
fn empty_and_reload_on_a_client(sim: &mut NetSim, rung: usize, hold: bool) {
    let a = sim.character_of(0).unwrap();
    {
        let w = sim.server.app.world_mut();
        let l = w.resource::<minigame::ladder::Ladder>().clone();
        minigame::ladder::set_rung(w, a, rung, &l);
    }
    let gun = sim.server.app.world().resource::<minigame::ladder::Ladder>().rungs[rung];
    assert!(sim.until(300, |s| held(s, 0) == Some(gun)), "{gun} on the client");
    sim.ticks(150);
    if clip(sim, 0).is_none() {
        // A knife or a grenade: nothing to reload.
        return;
    }
    let me = sim.local_player(0).unwrap();
    let ahead = sim.server.position(a) + Vec3::new(0.0, 6.0, -20.0);
    let reloading = |w: &World, e: Entity| {
        testkit::active(w, e)
            .and_then(|g| w.get::<chimera_sim::weapon::WeaponState>(g))
            .is_some_and(|s| s.reloading())
    };
    sim.clients[0].app.world_mut().resource_mut::<ShownReloads>().0 = 0;
    // Fire (pressed again and again for a semi-automatic) until empty.
    let mut t = 0u64;
    while clip(sim, 0).is_some_and(|c| c > 0) {
        testkit::aim_at(&mut sim.clients[0], me, ahead);
        sim.clients[0].intent(me).fire = t % 2 == 0;
        sim.step();
        t += 1;
        assert!(t < 2000, "{gun} emptied");
    }
    let (mut client, mut server) = (0, 0);
    let (mut was_c, mut was_s) = (false, false);
    // Held until the clip has rounds again (a shell-by-shell reload: until
    // it starts, as a shot ends it; CS:S's guns wait for the trigger let
    // go: a second at most).
    let shells = {
        let w = sim.clients[0].app.world();
        testkit::active(w, me).is_some_and(|g| w.get::<chimera_sim::weapon::ShellReload>(g).is_some())
    };
    for n in 0..(8.0 / TICK_INTERVAL) as u64 {
        testkit::aim_at(&mut sim.clients[0], me, ahead);
        let refilled = client > 0 && (shells || clip(sim, 0).is_some_and(|c| c > 0));
        let waits = client == 0 && n as f64 * TICK_INTERVAL > 1.0;
        sim.clients[0].intent(me).fire = hold && !refilled && !waits;
        sim.step();
        let c = reloading(sim.clients[0].app.world(), me);
        let s = reloading(sim.server.app.world(), a);
        client += (c && !was_c) as u32;
        server += (s && !was_s) as u32;
        (was_c, was_s) = (c, s);
    }
    sim.clients[0].intent(me).fire = false;
    let shown = sim.clients[0].app.world().resource::<ShownReloads>().0;
    assert_eq!(
        (shown, client, server),
        (1, 1, 1),
        "{gun}, fire held {hold}: reloads shown, predicted, on the server"
    );
    assert!(clip(sim, 0).is_some_and(|c| c > 0), "{gun} refilled");
}

/// Emptying a gun on a client reloads it once: one reload shown (one
/// reload animation), predicted and on the server, fire held or not, with
/// the ladder's ammo refill on, on a link that jitters and loses packets
/// (corrections re-run commands): CS:S's Glock, Combat Arms' M92FS, 870
/// (shell by shell) and M4A1.
#[test]
fn an_emptied_gun_reloads_once_on_a_client() {
    let mut sim = joined();
    sim.set_conditions(
        0,
        LinkConditions {
            latency: Duration::from_millis(30),
            jitter: Duration::from_millis(15),
            loss: 0.05,
        },
    );
    sim.clients[0].app.init_resource::<ShownReloads>().add_systems(Update, count_shown_reloads);
    let ladder = sim.server.app.world().resource::<minigame::ladder::Ladder>().rungs.clone();
    for gun in [
        cs_source::game::weapons::GLOCK,
        "combat_arms:beretta_m92fs",
        "combat_arms:remington_870",
        "combat_arms:m4a1",
    ] {
        let rung = ladder.iter().position(|g| *g == gun).unwrap();
        for hold in [false, true] {
            empty_and_reload_on_a_client(&mut sim, rung, hold);
        }
    }
}

/// Rounds fired by `who` this far (first runs only, not a correction's
/// replays) and the fire sounds `entry` heard, counted tick by tick.
#[derive(Default)]
struct Shots {
    rounds: usize,
    heard: usize,
}

fn count_shots(
    sim: &mut NetSim,
    app: usize,
    who: Entity,
    entry: &str,
    cursors: &mut (MessageCursor<WeaponEvent>, MessageCursor<PlaySound>),
    into: &mut Shots,
) {
    let w = if app == usize::MAX { sim.server.app.world() } else { sim.clients[app].app.world() };
    into.rounds += cursors
        .0
        .read(w.resource::<Messages<WeaponEvent>>())
        .filter(|e| e.owner == who && !e.replay && matches!(e.kind, WeaponEventKind::Fired { .. }))
        .count();
    into.heard += cursors.1.read(w.resource::<Messages<PlaySound>>()).filter(|s| s.entry == entry).count();
}

/// Client 0's own killing round, predicted, sounds once on it (the server's
/// echo doesn't sound it again, nor does the swap that follows silence it)
/// and once on the other client, which sees it as someone else's.
#[test]
fn a_clients_own_killing_shot_sounds_once() {
    let mut sim = joined();
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    let me = sim.local_player(0).unwrap();
    let ladder = sim.server.app.world().resource::<minigame::ladder::Ladder>().rungs.clone();
    let entry = {
        let w = sim.clients[0].app.world();
        let sounds = w.get::<WeaponSounds>(testkit::active(w, me).unwrap());
        sounds.and_then(|s| s.fire.clone()).expect("the first rung's gun has a fire sound")
    };
    assert_eq!(held(&mut sim, 0), Some(ladder[0]));
    assert!(
        sim.until(600, |s| server_alive(s, b) && s.server.app.world().get::<God>(b).is_none()),
        "the victim is in play"
    );
    sim.ticks(30);
    let at = sim.server.position(a) + Vec3::new(0.0, 0.0, -4.0);
    {
        let w = sim.server.app.world_mut();
        w.get_mut::<Transform>(b).unwrap().translation = at;
        w.get_mut::<avian3d::prelude::Position>(b).unwrap().0 = at;
    }
    sim.ticks(90);
    let cursors = |w: &World| (
        w.resource::<Messages<WeaponEvent>>().get_cursor_current(),
        w.resource::<Messages<PlaySound>>().get_cursor_current(),
    );
    let id0 = sim.client_id(0);
    let a_seen = NetSim::owned_by(&mut sim.clients[1], Some(id0)).expect("client 1 sees player a");
    let mut own = cursors(sim.clients[0].app.world());
    let mut other = cursors(sim.clients[1].app.world());
    let mut server = cursors(sim.server.app.world());
    let (mut mine, mut theirs, mut served) = (Shots::default(), Shots::default(), Shots::default());
    let mut dead_for = 0;
    for t in 0..(10.0 / TICK_INTERVAL) as u64 {
        let target = sim.server.position(b);
        testkit::aim_at(&mut sim.clients[0], me, target);
        sim.clients[0].intent(me).fire = dead_for == 0 && t % 2 == 0;
        sim.step();
        count_shots(&mut sim, 0, me, &entry, &mut own, &mut mine);
        count_shots(&mut sim, 1, a_seen, &entry, &mut other, &mut theirs);
        count_shots(&mut sim, usize::MAX, a, &entry, &mut server, &mut served);
        if !server_alive(&sim, b) {
            dead_for += 1;
            if dead_for > 6 {
                break;
            }
        }
    }
    sim.clients[0].intent(me).fire = false;
    assert!(dead_for > 0, "the victim died");
    // Let the swap, the echoes and any corrections arrive.
    for _ in 0..60 {
        sim.step();
        count_shots(&mut sim, 0, me, &entry, &mut own, &mut mine);
        count_shots(&mut sim, 1, a_seen, &entry, &mut other, &mut theirs);
        count_shots(&mut sim, usize::MAX, a, &entry, &mut server, &mut served);
    }
    assert_ne!(held(&mut sim, 0), Some(ladder[0]), "the kill swapped the gun");
    assert!(mine.rounds > 1, "rounds fired: {}", mine.rounds);
    assert_eq!(mine.heard, mine.rounds, "the shooter hears each of its own rounds once, the killing one too");
    assert_eq!(theirs.heard, served.rounds, "the other client hears each round the server fired once");
}

/// The announcer over the network (specs/combat_arms/kill_streaks.md A1,
/// A3): the server counts client 0's head kill of client 1 and sends its
/// facts with the kill; client 0 (the killer) shows the server's pack's
/// FIRST KILL with the skull over it and plays that one voice only (one
/// event and one voice per kill: kill_streaks.md case 36); client 1 (the
/// victim) neither.
#[test]
fn the_killers_client_announces_the_servers_facts() {
    use chimera_client::client::announcer::{AnnouncerPlugin, AnnouncerView};
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        |app| {
            setup(app);
            app.add_plugins(AnnouncerPlugin);
        },
    );
    sim.set_frame(TICK_INTERVAL);
    minigame::start(sim.server.app.world_mut(), DEFAULT_MINIGAME).expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    assert!(sim.until(600, |s| server_alive(s, b) && s.server.app.world().get::<God>(b).is_none()));
    let at = sim.server.position(b);
    sim.server.app.world_mut().write_message(chimera_core::core::Damage {
        target: b,
        attacker: Some(a),
        amount: 5.0,
        point: at,
        dir: Vec3::NEG_Z,
        hitgroup: chimera_core::core::Hitgroup::Head,
        kind: chimera_core::core::DamageKind::Bullet,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(20);
    let view = sim.clients[0].app.world().resource::<AnnouncerView>();
    assert_eq!(view.voices, ["UI_SND/FirstKill.wav"]);
    let shown = view.player.shown.as_ref().expect("a banner on the killer's client");
    assert_eq!(shown.request.image, "UI_HUD_Message/Effect/FirstKill.dtx");
    assert_eq!(shown.request.top.as_deref(), Some("UI_HUD_Message/Effect/Head.dtx"));
    let victim = sim.clients[1].app.world().resource::<AnnouncerView>();
    assert!(victim.voices.is_empty() && victim.player.shown.is_none(), "not on the victim's client");
}

/// A gun game with HL2:DM's guns over the network: the first rung,
/// HL2:DM's pistol, in client 0's hand with rounds in it (the client
/// predicting HL2:DM's weapon frame: its pistol hooks), the player
/// HL2:DM's on the server and on its client; its kill moves it to CS:S's
/// Glock on both.
#[test]
fn an_hl2dm_rung_plays_over_the_network() {
    use crate::hl2dm_parts::{LADDER, mixed_text};
    let text = mixed_text(false);
    let setup_hl2 = move |app: &mut App| {
        setup(app);
        chimera_sim::minigame::MinigameAppExt::add_minigames(app, &[("hl2dm.toml", &text)]).unwrap();
    };
    let mut sim = NetSim::new(
        CaSim,
        LinkConditions {
            latency: Duration::from_millis(20),
            jitter: Duration::ZERO,
            loss: 0.0,
        },
        7,
        2,
        setup_hl2,
    );
    sim.set_frame(TICK_INTERVAL);
    crate::gun_game::no_finale(sim.server.app.world_mut());
    minigame::start(sim.server.app.world_mut(), "test:gun_game_hl2dm").expect("the gun game starts");
    sim.until_joined(900);
    sim.ticks(300);
    let (a, b) = (sim.character_of(0).unwrap(), sim.character_of(1).unwrap());
    assert!(sim.until(300, |s| held(s, 0) == Some(LADDER[0])), "HL2:DM's pistol on the client");
    let identity = |w: &World, e: Entity| w.get::<chimera_core::slots::GameIdentity>(e).map(|g| g.0.clone());
    assert_eq!(identity(sim.server.app.world(), a).as_deref(), Some("hl2dm"));
    let me = sim.local_player(0).unwrap();
    assert_eq!(identity(sim.clients[0].app.world(), me).as_deref(), Some("hl2dm"), "replicated");
    kill_once(&mut sim, 0, a, b, &LADDER);
    assert!(sim.until(300, |s| held(s, 0) == Some(LADDER[1])), "the Glock on the client after the kill");
}
