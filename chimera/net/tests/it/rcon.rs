//! RCON over the connection (`net::rcon`): a player who isn't an admin
//! can't change a server setting; one who logs in with the server's
//! `rcon_password` can (and sees the settings the app lists for admins);
//! a wrong password fails, and too many get the address banned and the
//! client kicked.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    console::{Console, resource_cvar},
    slots::Loadout,
};
use chimera_net::{
    harness::NetSim,
    net::{
        LastDisconnect,
        memory::LinkConditions,
        rcon::{AdminCvars, BANNED, RconAdmin, RconBans},
    },
};
use chimera_sim::{SimBasePlugins, movement::placeholder};
use chimera_world::greybox::GreyboxMapPlugin;

/// A server setting (a host setting's stand-in).
#[derive(Resource, Default)]
struct Rule(u32);

fn setup(app: &mut App) {
    app.add_plugins(GreyboxMapPlugin)
        .insert_resource(Loadout {
            movement: placeholder::ID,
        })
        .init_resource::<Rule>();
    resource_cvar::<Rule, u32>(app, "mp_test_rule", "A server rule.", |r| &mut r.0);
    app.world_mut().resource_mut::<AdminCvars>().0.insert("mp_test_rule".into());
}

fn submit(sim: &mut NetSim, app: Option<usize>, line: &str) {
    let app = match app {
        Some(i) => &mut sim.clients[i].app,
        None => &mut sim.server.app,
    };
    app.world_mut().resource_mut::<Console>().submit(line);
}

fn joined(clients: usize) -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(20),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(SimBasePlugins, link, 7, clients, setup);
    sim.until_joined(600);
    sim.ticks(30);
    sim
}

fn rule(sim: &NetSim) -> u32 {
    sim.server.app.world().resource::<Rule>().0
}

fn console_has(sim: &NetSim, i: usize, text: &str) -> bool {
    sim.clients[i].app.world().resource::<Console>().output.iter().any(|l| l.text.contains(text))
}

fn admin(sim: &NetSim, i: usize) -> bool {
    sim.clients[i].app.world().resource::<RconAdmin>().admin
}

#[test]
fn only_an_admin_with_the_password_changes_server_settings() {
    let mut sim = joined(2);
    submit(&mut sim, None, "rcon_password s3cret; mp_test_rule 1");
    sim.ticks(10);
    // Not an admin: the cvar is the server's, and rcon wants a password.
    submit(&mut sim, Some(0), "mp_test_rule 5; rcon mp_test_rule 5");
    sim.ticks(60);
    assert_eq!(rule(&sim), 1);
    assert!(console_has(&sim, 0, "set rcon_password first"));
    assert!(!admin(&sim, 0));
    // A wrong password: refused, not an admin, counted.
    submit(&mut sim, Some(0), "rcon_password nope");
    assert!(sim.until(300, |s| console_has(s, 0, "Bad RCON password.")), "told it was wrong");
    submit(&mut sim, Some(0), "rcon mp_test_rule 5");
    sim.ticks(60);
    assert_eq!(rule(&sim), 1, "a wrong password changes nothing");
    assert!(!admin(&sim, 0));
    assert_eq!(sim.server.app.world().resource::<RconBans>().failures.values().sum::<u32>(), 2);
    // The right one: an admin, with the server's values.
    submit(&mut sim, Some(1), "rcon_password s3cret");
    assert!(sim.until(300, |s| admin(s, 1)), "logged in");
    assert_eq!(
        sim.clients[1].app.world().resource::<RconAdmin>().values.get("mp_test_rule").map(String::as_str),
        Some("1")
    );
    submit(&mut sim, Some(1), "rcon mp_test_rule 7");
    assert!(sim.until(300, |s| rule(s) == 7), "the admin's change reached the server");
    // A change made elsewhere reaches the admin's copy.
    submit(&mut sim, None, "mp_test_rule 9");
    assert!(sim.until(300, |s| s.clients[1].app.world().resource::<RconAdmin>().values.get("mp_test_rule")
        == Some(&"9".to_string())));
    // A new server password: no admins any more, and the old one fails.
    submit(&mut sim, None, "rcon_password other");
    assert!(sim.until(300, |s| !admin(s, 1)), "dropped");
    submit(&mut sim, Some(1), "rcon mp_test_rule 3");
    sim.ticks(60);
    assert_eq!(rule(&sim), 9);
}

#[test]
fn too_many_wrong_passwords_ban_and_kick() {
    let mut sim = joined(1);
    submit(&mut sim, None, "rcon_password s3cret; sv_rcon_maxfailures 3");
    sim.ticks(10);
    for (n, guess) in ["a", "b", "c"].iter().enumerate() {
        submit(&mut sim, Some(0), &format!("rcon_password {guess}; rcon mp_test_rule 4"));
        let bans = |s: &mut NetSim| {
            let b = s.server.app.world().resource::<RconBans>();
            b.failures.values().sum::<u32>() as usize >= (n + 1) * 2 || !b.banned.is_empty()
        };
        assert!(sim.until(300, bans), "attempt {n} counted");
    }
    assert!(!sim.server.app.world().resource::<RconBans>().banned.is_empty(), "banned");
    assert!(
        sim.until(600, |s| s.clients[0]
            .app
            .world()
            .get_resource::<LastDisconnect>()
            .is_some_and(|d| d.0.contains(BANNED))),
        "kicked"
    );
    assert_eq!(rule(&sim), 0);
}
