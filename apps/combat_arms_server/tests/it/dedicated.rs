//! The dedicated server on the greybox (`ServerPlugin`, its command line
//! `+ca_mode TDM +ca_goal 2`) with two headless clients of Combat Arms'
//! simulation (as the app runs it, without the window) joined over UDP on
//! loopback: they come in on opposite teams, one shoots the other with
//! its rifle (its own aim and trigger, the server's hit), and the kill
//! scores on the server and both clients. Bots: `+MaxPlayer 2` fills the
//! server with two.

use std::time::Duration;

use bevy::{prelude::*, time::TimeUpdateStrategy};
use chimera_core::core::{Dead, Intent, LocalPlayer, MovementState, NetRole, SpawnPoint, Team};
use chimera_net::net::{self, NetCharacter, NetPlugin, NetSettings, client::Joined};
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};
use combat_arms::{
    game::{
        TICK_INTERVAL,
        bots::CaBot,
        rules::{CaMatch, CaMatchView, CaStats, Phase},
    },
    harness::CaSim,
};
use combat_arms_server::{Parsed, ServerPlugin, StartMap};

fn frame(sim: &mut Sim) {
    sim.app
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f64(TICK_INTERVAL)));
}

/// The server from its command line (`args`), on the greybox, with a row
/// of start points a team 8 m apart, listening on loopback.
fn server(args: &[&str]) -> (Sim, std::net::SocketAddr) {
    let mut args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    args.extend(["+ip", "127.0.0.1", "-port", "0"].map(String::from));
    let Ok(Parsed::Run(options)) = combat_arms_server::parse(args) else { panic!("options") };
    let start = StartMap::load(&combat_arms_server::SERVER, &options).unwrap();
    let mut sim = Sim::with((), |app| {
        app.add_plugins(ServerPlugin::new(&combat_arms_server::SERVER, start, &options));
        let w = app.world_mut();
        for (k, team) in [1u8, 2].into_iter().enumerate() {
            for i in 0..4 {
                w.spawn((
                    Name::new(format!("Spawn {}", k * 4 + i)),
                    SpawnPoint { team: Some(Team(team)) },
                    Transform::from_translation(Vec3::new(-6.0 + 3.0 * i as f32, 1.0, if team == 1 { 4.0 } else { -4.0 })),
                ));
            }
        }
    });
    frame(&mut sim);
    // The command line's console lines run.
    sim.app.update();
    let addr = combat_arms_server::listen(sim.app.world_mut()).expect("listen");
    assert!(addr.ip().is_loopback() && addr.port() != 0, "{addr}");
    assert_eq!(*sim.app.world().resource::<NetRole>(), NetRole::Server);
    (sim, addr)
}

/// A client of Combat Arms' simulation on the greybox (the app's, without
/// the window), connected to `addr`.
fn client(addr: std::net::SocketAddr) -> Sim {
    let mut sim = Sim::with(CaSim, |app| {
        app.add_plugins((NetPlugin, GreyboxMapPlugin));
    });
    frame(&mut sim);
    let to = std::net::SocketAddr::from(([127, 0, 0, 1], addr.port()));
    net::client::connect(sim.app.world_mut(), to).expect("connect");
    sim
}

fn step(server: &mut Sim, clients: &mut [Sim]) {
    server.app.update();
    for c in clients.iter_mut() {
        c.app.update();
    }
    std::thread::sleep(Duration::from_millis(1));
}

fn until(server: &mut Sim, clients: &mut [Sim], frames: usize, done: impl Fn(&mut Sim, &mut [Sim]) -> bool) -> bool {
    for _ in 0..frames {
        step(server, clients);
        if done(server, clients) {
            return true;
        }
    }
    false
}

fn local(sim: &mut Sim) -> Option<Entity> {
    let w = sim.app.world_mut();
    w.query_filtered::<Entity, With<LocalPlayer>>().iter(w).next()
}

/// The server's character of the client `c`.
fn character_of(server: &mut Sim, c: &mut Sim) -> Entity {
    let me = local(c).unwrap();
    let id = c.app.world().get::<NetCharacter>(me).unwrap().owner;
    let w = server.app.world_mut();
    w.query::<(Entity, &NetCharacter)>().iter(w).find(|(_, n)| n.owner == id).unwrap().0
}

fn view(c: &Sim) -> CaMatchView {
    c.app.world().resource::<CaMatchView>().clone()
}

#[test]
fn two_clients_play_a_team_death_match_kill() {
    let (mut server, addr) = server(&["+map", "greybox", "+ca_mode", "TDM", "+ca_goal", "2", "+maxplayers", "4"]);
    let mut clients = vec![client(addr), client(addr)];
    let joined = until(&mut server, &mut clients, 1000, |_, cs| {
        cs.iter_mut().all(|c| c.app.world().contains_resource::<Joined>() && local(c).is_some())
    });
    assert!(joined, "both joined over UDP");
    let status = net::status(server.app.world_mut());
    assert!(status.contains("players : 2 (4 max)"), "{status}");
    let live = until(&mut server, &mut clients, 600, |s, cs| {
        s.app.world().resource::<CaMatch>().phase == Phase::Live && cs.iter().all(|c| view(c).phase == Phase::Live)
    });
    assert!(live, "live everywhere: {:?}", server.app.world().resource::<CaMatch>().phase);
    for c in &clients {
        assert_eq!(view(c).mode, "TDM");
        assert_eq!(view(c).goal, 2);
        assert_eq!(view(c).team_counts, [1, 1], "one player a team");
    }
    let (a, b) = (character_of(&mut server, &mut clients[0]), character_of(&mut server, &mut clients[1]));
    let team = |s: &Sim, e| s.app.world().get::<Team>(e).unwrap().0;
    let (ta, tb) = (team(&server, a), team(&server, b));
    assert!(ta != tb, "opposite teams: {ta} {tb}");
    // Past the spawn protection (4 s), client 0 aims at b and holds fire.
    for _ in 0..(4.3 / TICK_INTERVAL) as usize {
        step(&mut server, &mut clients);
    }
    let mut killed = false;
    for _ in 0..(10.0 / TICK_INTERVAL) as usize {
        let target = server.app.world().get::<Transform>(b).unwrap().translation;
        let me = local(&mut clients[0]).unwrap();
        let w = clients[0].app.world_mut();
        let eye = w.get::<Transform>(me).unwrap().translation + w.get::<MovementState>(me).unwrap().eye_offset;
        let d = target + Vec3::Y * 0.2 - eye;
        let mut intent = w.get_mut::<Intent>(me).unwrap();
        intent.yaw = (-d.x).atan2(-d.z);
        intent.pitch = d.y.atan2(d.xz().length());
        intent.fire = true;
        step(&mut server, &mut clients);
        if server.app.world().get::<Dead>(b).is_some() {
            killed = true;
            break;
        }
    }
    assert!(killed, "client 0 shot client 1 dead");
    let side = ta as usize - 1;
    let scored = until(&mut server, &mut clients, 100, |s, cs| {
        s.app.world().resource::<CaMatch>().score[side] == 1 && cs.iter().all(|c| view(c).score[side] == 1)
    });
    assert!(scored, "the kill scores everywhere: server {:?}", server.app.world().resource::<CaMatch>().score);
    assert_eq!(server.app.world().get::<CaStats>(a).map(|s| s.kills), Some(1), "the killer's statistics");
    let me = local(&mut clients[0]).unwrap();
    assert_eq!(clients[0].app.world().get::<CaStats>(me).map(|s| s.kills), Some(1), "as its client sees them");
    let dead = local(&mut clients[1]).unwrap();
    assert!(clients[1].app.world().get::<Dead>(dead).is_some(), "the victim's client sees itself dead");
    // `quit` hangs up on both.
    server.app.world_mut().resource_mut::<chimera_core::console::Console>().submit("quit");
    let gone = until(&mut server, &mut clients, 200, |_, cs| {
        cs.iter().all(|c| c.app.world().get_resource::<NetRole>().is_none_or(|r| *r == NetRole::Standalone))
    });
    assert!(gone, "clients hear the server shut down");
}

#[test]
fn max_player_fills_the_server_with_bots() {
    let (mut server, _) = server(&["+ca_mode", "TDM", "+MaxPlayer", "2", "+BotLevel", "1"]);
    assert_eq!(server.app.world().resource::<NetSettings>().maxplayers, 8, "eight players unless told");
    let filled = until(&mut server, &mut [], 400, |s, _| {
        let w = s.app.world_mut();
        w.query_filtered::<(), With<CaBot>>().iter(w).count() == 2
    });
    assert!(filled, "two bots");
    net::disconnect(server.app.world_mut(), "done");
}
