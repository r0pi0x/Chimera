//! Radio and chat over the wire (specs/combat_arms/radio_chat.md S1): a
//! server and two clients on `NetSim`'s in-memory link with Combat Arms'
//! simulation on the greybox. The cases of tests/it/radio_chat.rs, on the
//! real path: a client's request, the server's limiter and relay, the
//! receiving client's own say.

use std::time::Duration;

use bevy::prelude::*;
use chimera_core::core::Team;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::greybox::GreyboxMapPlugin;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        chat::{
            self, CaChat, CaRadio, CaSayRequest,
            client::{InputKey, Tab},
            radio::{self, RadioData},
            show::colours,
        },
    },
    harness::CaSim,
};

/// The radio file of the spec's test cases (an answer everyone hears).
const RADIO_FILE: &str = r#"
[SoundTemplate]
Template0 = "Male"
[RadioClass]
RadioClass0 = "Report"
RadioClass1 = "Answer"
[Report0]
ListText = "Enemy spotted"
Male_Snd0 = "male0.wav"
Chat0 = "Enemy spotted!"
Chat1 = "Contact!"
SendToEnemy = 0
[Answer0]
SendToEnemy = 1
3Dsound = 0
Male_Snd0 = "male_roger.wav"
Chat0 = "Roger"
"#;

fn joined() -> NetSim {
    let link = LinkConditions {
        latency: Duration::ZERO,
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 11, 2, |app| {
        app.add_plugins(GreyboxMapPlugin);
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(240);
    for c in &mut sim.clients {
        c.app
            .world_mut()
            .insert_resource(CaRadio(RadioData::parse(RADIO_FILE).unwrap()));
    }
    sim
}

fn lines(sim: &NetSim, i: usize) -> Vec<(String, [u8; 4])> {
    sim.clients[i]
        .app
        .world()
        .resource::<CaChat>()
        .log
        .lines
        .iter()
        .map(|l| (l.text.clone(), l.colour))
        .collect()
}

fn say(sim: &mut NetSim, i: usize, tab: Tab, text: &str) {
    let w = sim.clients[i].app.world_mut();
    {
        let mut c = w.resource_mut::<CaChat>();
        c.tab = tab;
        c.sent_this_frame = false;
    }
    assert!(chat::open_enter(w), "client {i}'s input opens");
    w.resource_mut::<CaChat>().type_text(text);
    chat::input_key(w, InputKey::Enter);
    sim.ticks(30);
}

fn team(sim: &mut NetSim, i: usize) -> u8 {
    let me = sim.local_player(i).unwrap();
    sim.clients[i].app.world().get::<Team>(me).unwrap().0
}

/// Chat and radio from a client reach the right clients through the
/// server: all chat everyone (teammate and enemy colours), team chat the
/// team, a team radio call the team, an answer everyone; ten identical
/// lines show nine (the server drops the tenth).
#[test]
fn chat_and_radio_over_the_wire() {
    let mut sim = joined();
    assert_ne!(
        team(&mut sim, 0),
        team(&mut sim, 1),
        "the two clients are on different teams"
    );
    // All chat.
    say(&mut sim, 0, Tab::All, "hi");
    let (a, b) = (lines(&sim, 0), lines(&sim, 1));
    assert!(a.last().is_some_and(|l| l.0.ends_with(" : hi")), "{a:?}");
    assert_eq!(a.last().unwrap().1, colours::ALL_TEAMMATE);
    assert!(b.last().is_some_and(|l| l.0.ends_with(" : hi")), "{b:?}");
    assert_eq!(b.last().unwrap().1, colours::ALL_ENEMY);
    // Team chat: the sender's team only.
    let (before0, before1) = (lines(&sim, 0).len(), lines(&sim, 1).len());
    say(&mut sim, 0, Tab::Team, "push");
    assert_eq!(lines(&sim, 0).len(), before0 + 1);
    assert_eq!(lines(&sim, 0).last().unwrap().1, colours::OWN_TEAM);
    assert_eq!(lines(&sim, 1).len(), before1, "the enemy reads no team chat");
    // A team radio call (report): the team only.
    let before1 = lines(&sim, 1).len();
    let w = sim.clients[0].app.world_mut();
    chat::radio_key(w, radio::REPORT);
    assert!(chat::radio_number_with(w, 1, Some(1)));
    sim.ticks(30);
    let a = lines(&sim, 0);
    let last = &a.last().unwrap().0;
    assert!(
        last.starts_with("[Report] ") && last.ends_with(" : Enemy spotted!"),
        "{a:?}"
    );
    assert_eq!(a.last().unwrap().1, colours::RADIO);
    assert_eq!(lines(&sim, 1).len(), before1, "the enemy hears no team call");
    // An answer goes to everyone (after the 2000 ms radio cooldown).
    sim.ticks(150);
    let w = sim.clients[0].app.world_mut();
    chat::radio_key(w, radio::ANSWER);
    assert!(chat::radio_number_with(w, 1, Some(0)));
    sim.ticks(30);
    let b = lines(&sim, 1);
    assert!(b.last().unwrap().0.ends_with(" : Roger"), "{b:?}");
    assert_eq!(b.last().unwrap().1, colours::RADIO);
    // The server's limiter: ten identical lines (a client without the
    // sender's own limits) show nine.
    let before = lines(&sim, 1).len();
    for _ in 0..10 {
        sim.clients[0].app.world_mut().write_message(CaSayRequest {
            team: false,
            text: "spam".into(),
        });
        sim.ticks(2);
    }
    sim.ticks(30);
    assert_eq!(
        lines(&sim, 1).len() - before,
        9,
        "the tenth identical line is dropped by the server"
    );
}
