//! The HUD on network clients (docs/plans/active/combat-arms.md, "Network
//! part"): a server and two clients on chimera_net's `NetSim`, each
//! client with the HUD's state (`CaHudStatePlugin`). The hit indicator
//! comes from the client's own predicted shot test, before the server
//! has the shot (hud_crosshair.md A10, Corrections 1); a client deals no
//! damage of its own, so its damage feedback is the server's report; the
//! armour shown is the server's.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::{Health, Intent};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_sim::weapon::{Inventory, Magazine};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{game::TICK_INTERVAL, harness::CaSim};
use combat_arms_ui::hud::{CaHud, CaHudStatePlugin, HudMemory};

fn client_intent(sim: &mut NetSim, i: usize) -> Mut<'_, Intent> {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world_mut().get_mut::<Intent>(me).unwrap()
}

/// The server puts client `i` at `at`; its client looks along `yaw`.
fn place(sim: &mut NetSim, i: usize, at: Vec3, yaw: f32) {
    let e = sim.character_of(i).unwrap();
    let world = sim.server.app.world_mut();
    world.get_mut::<Transform>(e).unwrap().translation = at;
    if let Some(mut p) = world.get_mut::<avian3d::prelude::Position>(e) {
        p.0 = at;
    }
    *client_intent(sim, i) = Intent { yaw, ..default() };
}

#[test]
fn hits_damage_and_armour_come_from_the_server() {
    let link = LinkConditions {
        latency: Duration::from_millis(100),
        ..default()
    };
    let mut sim = NetSim::new(CaSim, link, 51, 2, |app| {
        app.add_plugins((GreyboxMapPlugin, CaHudStatePlugin));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    place(&mut sim, 0, Vec3::new(0.0, 1.0, 12.0), 0.0);
    place(&mut sim, 1, Vec3::new(0.0, 1.0, 6.0), std::f32::consts::PI);
    sim.ticks(120);
    let me0 = sim.local_player(0).unwrap();
    let rifle = sim.clients[0].app.world().get::<Inventory>(me0).unwrap().active.unwrap();
    let clip = sim.clients[0].app.world().get::<Magazine>(rifle).unwrap().clip;
    client_intent(&mut sim, 0).pitch = -0.08;
    client_intent(&mut sim, 0).fire = true;
    let (mut indicated, mut hurt, mut felt) = (None, None, false);
    let target = sim.character_of(1).unwrap();
    for step in 0..120 {
        sim.step();
        if sim.clients[0].app.world().get::<Magazine>(rifle).unwrap().clip <= clip - 2 {
            client_intent(&mut sim, 0).fire = false;
        }
        if indicated.is_none() && sim.clients[0].app.world().resource::<HudMemory>().hit.last.is_some() {
            indicated = Some(step);
        }
        if hurt.is_none() && sim.server.app.world().get::<Health>(target).unwrap().current < 1.0 {
            hurt = Some(step);
        }
        felt |= sim.clients[1].app.world().resource::<HudMemory>().damage.is_some();
    }
    let me1 = sim.local_player(1).unwrap();
    let w1 = sim.clients[1].app.world();
    assert!(w1.get::<Health>(me1).unwrap().current < 1.0, "client 1 was hurt");
    let (indicated, hurt) = (indicated.expect("client 0's hit indicator lit"), hurt.expect("the server hurt client 1"));
    assert!(indicated < hurt, "lit by the client's own shot test ({indicated}) before the server's hit ({hurt})");
    assert!(felt, "client 1's damage feedback from the server's report");
    let points = w1.get::<chimera_sim::weapon::Armor>(me1).map(combat_arms::game::weapons::points).unwrap();
    assert!(points < 100.0);
    let shown = w1.resource::<CaHud>().status.clone().expect("client 1's status block");
    assert_eq!(shown.ap, (points.max(0.0) as u32).to_string(), "the server's armour shown");
}
