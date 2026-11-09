//! HL2:DM's rules over the network (`NetSim` on `Hl2Sim`, the greybox,
//! the rules started on the server as a map load starts them): a client
//! joins and is on a team (TDM: Combine or Rebels), changes team with
//! `jointeam` (sent to the server, which runs it for its player, a
//! suicide charged to the new team), and comes back after its death by
//! pressing a button more than 3 s on.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::{
    console::Console,
    core::{Damage, DamageKind, Dead, Hitgroup, Intent, Score},
};
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::greybox::GreyboxMapPlugin;
use hl2dm::{
    game::rules::{self, COMBINE, Hl2Player, Hl2Rules, REBELS},
    harness::Hl2Sim,
};

fn tdm() -> NetSim {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(Hl2Sim, link, 7, 1, |app: &mut App| {
        app.add_plugins(GreyboxMapPlugin);
    });
    {
        let w = sim.server.app.world_mut();
        w.resource_mut::<Hl2Rules>().teamplay = 1;
        rules::start(w);
    }
    sim.until_joined(600);
    sim.ticks(60);
    sim
}

#[test]
fn join_team_and_respawn() {
    let mut sim = tdm();
    let theirs = sim.character_of(0).expect("the client's player on the server");
    let team = sim.server.app.world().get::<Hl2Player>(theirs).expect("in HL2:DM's game").team;
    assert!(team == COMBINE || team == REBELS, "joined a team: {team}");
    let other = if team == COMBINE { REBELS } else { COMBINE };

    // `jointeam` from the client's console, run by the server.
    sim.ticks(400);
    let line = format!("hl2dm:jointeam {other}");
    sim.clients[0].app.world_mut().resource_mut::<Console>().submit(line);
    sim.ticks(30);
    let w = sim.server.app.world();
    assert_eq!(w.get::<Hl2Player>(theirs).unwrap().team, other, "changed team");
    assert!(w.get::<Dead>(theirs).is_some(), "a suicide");
    assert_eq!(w.get::<Score>(theirs).map(|s| (s.kills, s.deaths)), Some((-1, 1)));

    // Back after more than 3 s, on a press from the client.
    sim.ticks(240);
    let me = sim.local_player(0).unwrap();
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = true;
    sim.ticks(20);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = false;
    assert!(sim.server.app.world().get::<Dead>(theirs).is_none(), "respawned");

    // Killed by the world: dead, back again on a press.
    sim.server.app.world_mut().write_message(Damage {
        target: theirs,
        attacker: None,
        amount: 10.0,
        point: Vec3::ZERO,
        dir: Vec3::NEG_Y,
        hitgroup: Hitgroup::Generic,
        kind: DamageKind::Generic,
        weapon: None,
        force: Vec3::ZERO,
    });
    sim.ticks(10);
    assert!(sim.server.app.world().get::<Dead>(theirs).is_some());
    sim.ticks(100);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = true;
    sim.ticks(20);
    assert!(sim.server.app.world().get::<Dead>(theirs).is_some(), "not before 3 s");
    sim.ticks(120);
    sim.clients[0].app.world_mut().get_mut::<Intent>(me).unwrap().fire = false;
    assert!(sim.server.app.world().get::<Dead>(theirs).is_none(), "respawned after 3 s");
}

/// Models (rules.md): the server rewrites a remote client's
/// `cl_playermodel` to the model it resolved (an invalid setting, a TDM
/// team's replacement) and back after a change refused within 5 s.
#[test]
fn the_server_rewrites_a_clients_playermodel() {
    use hl2dm::game::rules::players::PlayerModelSetting;
    let mut sim = tdm();
    let theirs = sim.character_of(0).unwrap();
    let model = sim.server.app.world().get::<Hl2Player>(theirs).unwrap().model.clone();
    let setting = |sim: &NetSim| sim.clients[0].app.world().resource::<PlayerModelSetting>().0.clone();
    assert!(setting(&sim).eq_ignore_ascii_case(&model), "told its model: {} vs {model}", setting(&sim));
    // A change at once after the spawn's model is refused: put back.
    sim.ticks(400);
    let other = if model.contains("human") { "models/police.mdl" } else { "models/humans/group03/male_01.mdl" };
    sim.server.app.world_mut().get_mut::<Hl2Player>(theirs).unwrap().model_free_at = f64::MAX;
    sim.clients[0].app.world_mut().resource_mut::<PlayerModelSetting>().0 = other.into();
    sim.ticks(60);
    assert_eq!(sim.server.app.world().get::<Hl2Player>(theirs).unwrap().model, model, "refused");
    assert!(setting(&sim).eq_ignore_ascii_case(&model), "put back: {}", setting(&sim));
}

/// The suit over the network: a client's `impulse 100` lights its player
/// on the server, and the client sees it; `toggle_zoom` from its console
/// zooms it (suit.md "Flashlight", "Suit zoom").
#[test]
fn a_clients_flashlight_and_toggle_zoom() {
    use hl2dm::game::suit::{FlashlightAllowed, FlashlightOn, Suit};
    let mut sim = tdm();
    sim.server.app.world_mut().insert_resource(FlashlightAllowed(true));
    let theirs = sim.character_of(0).unwrap();
    let me = sim.local_player(0).unwrap();
    sim.clients[0].app.world_mut().resource_mut::<Console>().submit("impulse 100".to_string());
    sim.ticks(30);
    assert!(sim.server.app.world().get::<FlashlightOn>(theirs).is_some(), "on, on the server");
    assert!(sim.clients[0].app.world().get::<FlashlightOn>(me).is_some(), "and replicated");
    sim.clients[0].app.world_mut().resource_mut::<Console>().submit("impulse 100".to_string());
    sim.ticks(30);
    assert!(sim.server.app.world().get::<FlashlightOn>(theirs).is_none(), "off");
    assert!(sim.clients[0].app.world().get::<FlashlightOn>(me).is_none());

    sim.clients[0].app.world_mut().resource_mut::<Console>().submit("hl2dm:toggle_zoom".to_string());
    sim.ticks(30);
    let zoom = sim.server.app.world().get::<Suit>(theirs).unwrap().zoom;
    assert!(zoom.is_some_and(|z| z.to < 75.0), "zooming: {zoom:?}");
}

/// The history's server messages (hud.md 9): an item's class and "ammo
/// denied" reach only the taker's client.
#[test]
fn pickup_notes_reach_their_player() {
    use hl2dm::game::hud_notes::{HudPickup, PickupNote};
    let mut sim = tdm();
    let theirs = sim.character_of(0).unwrap();
    let me = sim.local_player(0).unwrap();
    sim.server.app.world_mut().write_message(HudPickup { player: theirs, note: PickupNote::Item("item_battery".into()) });
    sim.server.app.world_mut().write_message(HudPickup { player: theirs, note: PickupNote::AmmoDenied("hl2dm:smg1".into()) });
    let mut got = Vec::new();
    let mut cursor = bevy::ecs::message::MessageCursor::<HudPickup>::default();
    for _ in 0..30 {
        sim.ticks(1);
        let msgs = sim.clients[0].app.world().resource::<Messages<HudPickup>>();
        got.extend(cursor.read(msgs).cloned());
    }
    assert_eq!(
        got,
        vec![
            HudPickup { player: me, note: PickupNote::Item("item_battery".into()) },
            HudPickup { player: me, note: PickupNote::AmmoDenied("hl2dm:smg1".into()) },
        ]
    );
}
