//! Voice chat over the network (docs/plans/active/voice-chat.md): a fake
//! microphone on one client, through the Opus encoder, the server's relay
//! and CS:S's hearing rule (`sv_alltalk`), to the other clients' fake
//! speakers.

use std::time::Duration;

use bevy::prelude::*;

use crate::stack::{
    client::voice::{FakeInput, FakeOutput, VoiceDevices, VoicePlugin},
    console::{execute, parse},
    core::{Dead, Spectating, Team, Unassigned},
    games::cs_source::movement::{self as source, SourceMovementPlugin},
    greybox::GreyboxMapPlugin,
    harness::{CsSim, NetSim},
    net::{
        NetCharacter,
        memory::LinkConditions,
        voice::{VoiceActivity, alive},
    },
    slots::Loadout,
};

fn setup(app: &mut App) {
    app.add_plugins((GreyboxMapPlugin, SourceMovementPlugin, VoicePlugin))
        .insert_resource(Loadout { movement: source::ID });
}

fn run(world: &mut World, line: &str) {
    for words in parse(line) {
        execute(world, &words, 0);
    }
}

/// Client 0 talks for `seconds` (a tone into its microphone, K held),
/// then lets go; the sim runs until it is all through.
fn talk(sim: &mut NetSim, mic: &FakeInput, seconds: f32) {
    run(sim.clients[0].app.world_mut(), "+voicerecord");
    mic.push_tone(300.0, seconds);
    sim.ticks(40);
    run(sim.clients[0].app.world_mut(), "-voicerecord");
    sim.ticks(40);
}

#[test]
fn a_teammate_hears_you_and_an_enemy_only_with_alltalk() {
    let link = LinkConditions {
        latency: Duration::from_millis(30),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CsSim, link, 77, 3, setup);
    sim.until_joined(600);
    sim.ticks(200);
    let mic = FakeInput::default();
    let speakers: Vec<FakeOutput> = (0..3).map(|_| FakeOutput::default()).collect();
    for (i, c) in sim.clients.iter_mut().enumerate() {
        let input = if i == 0 { mic.clone() } else { FakeInput::default() };
        c.app.insert_resource(VoiceDevices::fake(input, speakers[i].clone()));
    }
    sim.server
        .app
        .insert_resource(VoiceDevices::fake(FakeInput::default(), FakeOutput::default()));
    // Clients 0 and 1 terrorists, 2 a counter-terrorist, all alive.
    for (i, team) in [(0, 1), (1, 1), (2, 2)] {
        let c = sim.character_of(i).unwrap();
        let mut e = sim.server.app.world_mut().entity_mut(c);
        e.remove::<(Unassigned, Spectating, Dead)>().insert(Team(team));
    }
    sim.ticks(10);
    for i in 0..3 {
        let c = sim.character_of(i).unwrap();
        assert!(alive(sim.server.app.world(), c), "client {i}'s player is alive");
    }

    // sv_alltalk 0 (CS:S's default): the teammate hears, the enemy not.
    talk(&mut sim, &mic, 0.4);
    let heard = speakers[1].total();
    println!("alltalk 0: the teammate heard {heard} samples, the enemy {}", speakers[2].total());
    // 0.4 s at 48 kHz, a frame or two of padding or start-up aside.
    assert!(heard >= 18_000, "the teammate heard {heard} samples");
    assert_eq!(speakers[2].total(), 0, "the enemy heard nothing");
    assert_eq!(speakers[0].total(), 0, "no echo of one's own voice");
    // The teammate's client saw who talked (the HUD's list).
    assert_eq!(sim.clients[1].app.world().resource::<VoiceActivity>().heard.len(), 1);
    assert!(sim.server.app.world().resource::<VoiceActivity>().frames >= 18);

    // sv_alltalk 1: both hear.
    run(sim.server.app.world_mut(), "sv_alltalk 1");
    talk(&mut sim, &mic, 0.4);
    let (mate, enemy) = (speakers[1].total() - heard, speakers[2].total());
    println!("alltalk 1: the teammate heard {mate} samples, the enemy {enemy}");
    assert!(mate >= 18_000 && enemy >= 18_000, "{mate} {enemy}");

    // Client 2 mutes client 0 (by user id, as the player list does): it
    // neither hears nor shows them talking; the teammate still hears.
    let talker = sim.character_of(0).unwrap();
    let id = sim.server.app.world().get::<NetCharacter>(talker).unwrap().owner.unwrap();
    run(sim.clients[2].app.world_mut(), &format!("voice_mute {id}"));
    sim.clients[2].app.world_mut().resource_mut::<VoiceActivity>().heard.clear();
    let (mate, enemy) = (speakers[1].total(), speakers[2].total());
    talk(&mut sim, &mic, 0.4);
    assert!(speakers[1].total() - mate >= 18_000, "the teammate still hears");
    assert_eq!(speakers[2].total(), enemy, "muted: not played");
    assert!(sim.clients[2].app.world().resource::<VoiceActivity>().heard.is_empty(), "nor shown talking");
    run(sim.clients[2].app.world_mut(), &format!("voice_unmute {id}"));
    talk(&mut sim, &mic, 0.4);
    assert!(speakers[2].total() - enemy >= 18_000, "unmuted: heard again");

    // sv_voiceenable 0: nobody.
    run(sim.server.app.world_mut(), "sv_voiceenable 0");
    let before = speakers[2].total();
    talk(&mut sim, &mic, 0.2);
    assert_eq!(speakers[2].total(), before, "voice off on the server");
}
