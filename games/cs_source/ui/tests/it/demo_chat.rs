//! A demo playing says only what the demo says: its players coming in as
//! puppets (`game::demo`) make no "X is joining the ... force" lines,
//! which a character joining a team in play does (`client::chat`).

use std::{collections::HashMap, sync::Arc};

use bevy::{ecs::message::MessageCursor, prelude::*};
use source_formats::dem::{
    Header,
    playback::{Frame, Timeline},
};

use crate::stack::{
    client::chat::{ChatLine, GameMessagesPlugin},
    core::{LocalPlayer, Team},
    games::cs_source::{
        TICK_INTERVAL,
        demo::{self, CsDemo, PlayerSnap, Thing},
        weapons::CsWeaponsPlugin,
    },
    greybox::GreyboxMapPlugin,
    harness::{CsSim, Sim},
    map::radio::{RadioCommands, SayFormats},
    movement::placeholder,
};

/// Two players walking for 100 ticks.
fn built() -> CsDemo {
    let player = |x: f32, team: u8| {
        Thing::Player(PlayerSnap {
            origin: [x, 0.0, 0.0],
            team,
            class: if team == 2 { 1 } else { 5 },
            health: 100,
            alive: true,
            flags: 1,
            ..Default::default()
        })
    };
    let frames = (0..=100)
        .step_by(4)
        .map(|tick| Frame {
            tick,
            server_tick: 1000 + tick as u32,
            entities: vec![(1, player(tick as f32, 2)), (2, player(-(tick as f32), 3))],
            events: Vec::new(),
        })
        .collect();
    CsDemo {
        timeline: Timeline {
            header: Header {
                demo_protocol: 3,
                network_protocol: 24,
                server_name: "test".into(),
                client_name: "SourceTV Demo".into(),
                map: "greybox".into(),
                game_dir: "cstrike".into(),
                playback_time: 1.5,
                ticks: 100,
                frames: 26,
                signon_length: 0,
            },
            tick_interval: TICK_INTERVAL as f32,
            map: "greybox".into(),
            frames,
            errors: Vec::new(),
        },
        classes: Vec::new(),
        models: Vec::new(),
        names: HashMap::from([(1, "Ann".to_string()), (2, "Bob".to_string())]),
        sounds: Vec::new(),
        recorder: None,
        cvars: Vec::new(),
    }
}

/// Chat lines over `n` updates.
fn heard(sim: &mut Sim, cursor: &mut MessageCursor<ChatLine>, n: usize) -> Vec<String> {
    let mut out = Vec::new();
    for _ in 0..n {
        sim.app.update();
        let messages = sim.app.world().resource::<Messages<ChatLine>>();
        out.extend(
            cursor
                .read(messages)
                .map(|l| l.0.iter().map(|(_, t)| t.as_str()).collect::<String>()),
        );
    }
    out
}

#[test]
fn demo_puppets_join_without_a_chat_line() {
    let mut sim = Sim::with(CsSim, |app| {
        app.add_plugins((GreyboxMapPlugin, CsWeaponsPlugin, demo::DemoPlugin, GameMessagesPlugin))
            .insert_resource(RadioCommands {
                say: SayFormats {
                    joins: vec![
                        (1, "%s1 is joining the Terrorist force".into()),
                        (2, "%s1 is joining the Counter-Terrorist force".into()),
                    ],
                    ..default()
                },
                ..default()
            });
    });
    sim.set_tick_interval(TICK_INTERVAL);
    let mut cursor = sim.app.world().resource::<Messages<ChatLine>>().get_cursor();
    // In play, a character on a team is announced.
    let me = sim.spawn_character(Vec3::ZERO, placeholder::ID);
    sim.app
        .world_mut()
        .entity_mut(me)
        .insert((LocalPlayer, Name::new("Watcher"), Team(2)));
    let lines = heard(&mut sim, &mut cursor, 5);
    assert!(
        lines
            .iter()
            .any(|l| l.ends_with("is joining the Counter-Terrorist force")),
        "{lines:?}"
    );

    // The demo's players come in as puppets on their teams: no lines.
    demo::start(sim.app.world_mut(), Arc::new(built()));
    let lines = heard(&mut sim, &mut cursor, 30);
    let puppets = sim
        .app
        .world_mut()
        .query::<&demo::DemoPuppet>()
        .iter(sim.app.world())
        .count();
    assert_eq!(puppets, 2);
    assert!(!lines.iter().any(|l| l.contains("is joining")), "{lines:?}");

    // Stopped, play announces again.
    demo::stop(sim.app.world_mut());
    let other = sim.spawn_character(Vec3::X * 2.0, placeholder::ID);
    sim.app
        .world_mut()
        .entity_mut(other)
        .insert((Name::new("Late"), Team(1)));
    let lines = heard(&mut sim, &mut cursor, 5);
    assert!(
        lines.iter().any(|l| l == "Late is joining the Terrorist force"),
        "{lines:?}"
    );
}
