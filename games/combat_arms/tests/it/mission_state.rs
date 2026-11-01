//! The mission display's state (`game::objects::mission::CaMission`; ours,
//! not from the original, world_objects.md Open question 15): the notices
//! of the timer, caption, message and mission triggers applied in order,
//! and over the network (`NetCaMission`) a late joiner getting the
//! running timer, the caption still showing and the current objectives.

use std::time::Duration;

use bevy::prelude::*;
use chimera_net::{harness::NetSim, net::memory::LinkConditions};
use chimera_world::map::MapSimPlugin;
use combat_arms::{
    game::{
        TICK_INTERVAL,
        objects::{
            CaObjectMessage,
            events::{Notice, Objective, ObjectiveState},
            mission::CaMission,
        },
        world,
    },
    harness::CaSim,
};
use lithtech_formats::world::{Object, Value};

fn objective(id: &str, team: u8, state: ObjectiveState, count: Option<(i32, i32)>) -> Objective {
    Objective {
        id: id.into(),
        team,
        state,
        count,
    }
}

/// The notices applied in order: a timer on and off, a second start moving
/// its end, captions and messages replacing and clearing, objectives
/// added, changed in place and removed.
#[test]
fn notices_become_state() {
    let mut m = CaMission::default();
    m.apply(7, "VipExitTimer", &Notice::Timer { on: true, seconds: 60.0 }, 10.0);
    m.apply(9, "Other", &Notice::Timer { on: true, seconds: 30.0 }, 12.0);
    assert_eq!(m.timers.iter().map(|t| (t.object, t.ends)).collect::<Vec<_>>(), [(7, 70.0), (9, 42.0)]);
    m.apply(9, "Other", &Notice::Timer { on: false, seconds: 0.0 }, 20.0);
    assert_eq!(m.timers.len(), 1, "off removes only its own");

    m.apply(1, "Cap", &Notice::Caption { id: "Cap1".into(), showtime: 4.0, team: 0 }, 5.0);
    m.apply(1, "Cap", &Notice::Caption { id: "Cap2".into(), showtime: 4.0, team: 2 }, 6.0);
    let c = m.caption.clone().unwrap();
    assert_eq!((c.text.as_str(), c.until, c.team), ("Cap2", 10.0, 2), "a new caption replaces");
    m.apply(2, "Msg", &Notice::Message { id: "Msg1".into(), showtime: 3.0, team: 0 }, 6.0);
    m.expire(9.5);
    assert!(m.caption.is_some() && m.message.is_none(), "the message ran out first");
    m.apply(1, "Cap", &Notice::HideCaption, 9.6);
    assert!(m.caption.is_none());

    m.apply(3, "Mis", &Notice::Objective(objective("Find", 0, ObjectiveState::Normal, None)), 1.0);
    m.apply(3, "Mis", &Notice::Objective(objective("Kill", 1, ObjectiveState::InProgress, Some((0, 5)))), 1.0);
    m.apply(3, "Mis", &Notice::Objective(objective("Kill", 1, ObjectiveState::InProgress, Some((2, 5)))), 2.0);
    m.apply(3, "Mis", &Notice::Objective(objective("Find", 0, ObjectiveState::Success, None)), 3.0);
    assert_eq!(m.objectives.len(), 2, "changed in place by id");
    assert_eq!(m.objectives[0].state, ObjectiveState::Success);
    assert_eq!(m.objectives[1].count, Some((2, 5)));
    m.apply(3, "Mis", &Notice::ObjectiveRemoved { id: "Find".into() }, 4.0);
    assert_eq!(m.objectives.len(), 1);

    // A timer whose stop notice never came goes a while after its end.
    m.expire(80.0);
    assert!(m.timers.is_empty());
}

fn real(v: f32) -> Value {
    Value::Real(v)
}

/// The small world with a timer, a caption, a message and a mission list.
fn mission_world() -> lithtech_formats::world::World {
    let mut w = crate::world::small_world();
    let prop = crate::world::prop;
    let mut push = |class: &str, name: &str, extra: Vec<(&str, Value)>| {
        let mut properties = vec![prop("Name", Value::String(name.into())), prop("Pos", Value::Vector([0.0, 50.0, 200.0]))];
        properties.extend(extra.into_iter().map(|(n, v)| prop(n, v)));
        w.objects.push(Object { class: class.into(), properties });
    };
    push("TimerTrigger", "Clock", vec![("TimeAmount", real(300.0))]);
    push("CaptionTrigger", "Cap", vec![("Showtime", real(600.0))]);
    push("MessageTrigger", "Msg", vec![("Showtime", real(600.0))]);
    push("MissionTrigger", "Mis", vec![]);
    w
}

/// Over the network: the server starts the timer, shows a caption and
/// message and adds objectives; a client has them, and a client joining
/// later has them too, with the timer's remaining time about right.
#[test]
fn a_late_joiner_gets_the_running_timer_and_objectives() {
    let map = world::convert(&mission_world(), "mission", 0.01, &|_| None);
    let link = LinkConditions {
        latency: Duration::from_millis(40),
        jitter: Duration::ZERO,
        loss: 0.0,
    };
    let mut sim = NetSim::new(CaSim, link, 3, 1, move |app| {
        app.add_plugins(MapSimPlugin::new(map.clone()));
    });
    sim.set_frame(TICK_INTERVAL);
    sim.until_joined(600);
    sim.ticks(30);
    for (target, message) in [
        ("Clock", "start"),
        ("Cap", "play Cap404"),
        ("Msg", "play Msg1"),
        ("Mis", "ADDTEAM Find_the_intel 0"),
        ("Mis", "add_point_mission Kills"),
        ("Mis", "update_point_mission Kills 3"),
        ("Mis", "check Find_the_intel"),
    ] {
        sim.server.app.world_mut().write_message(CaObjectMessage {
            target: target.into(),
            message: message.into(),
        });
    }
    sim.ticks(40);
    let state = |app: &mut App| app.world().resource::<CaMission>().clone();
    let server = state(&mut sim.server.app);
    assert_eq!(server.timers.len(), 1, "{server:?}");
    assert_eq!(server.caption.as_ref().map(|c| c.text.as_str()), Some("Cap404"));
    assert_eq!(server.message.as_ref().map(|c| c.text.as_str()), Some("Msg1"));
    assert_eq!(server.objectives.len(), 2, "{server:?}");

    let same = |theirs: &CaMission, server: &CaMission| {
        assert_eq!(theirs.objectives, server.objectives);
        assert_eq!(theirs.caption.as_ref().map(|c| &c.text), server.caption.as_ref().map(|c| &c.text));
        assert_eq!(theirs.message.as_ref().map(|c| &c.text), server.message.as_ref().map(|c| &c.text));
        assert_eq!(theirs.timers.len(), 1);
    };
    same(&state(&mut sim.clients[0].app), &server);

    sim.add_client(|_| {});
    sim.until_joined(600);
    sim.ticks(40);
    let server = state(&mut sim.server.app);
    let late = state(&mut sim.clients[1].app);
    same(&late, &server);
    assert_eq!(late.objectives[0].state, ObjectiveState::Success);
    assert_eq!(late.objectives[1].count, Some((3, 0)), "the maximum is the mission data's, none read yet");
    // The timer's end on the joiner's clock is the server's, to the interpolation delay and a join's settling (the round timer's mapping, `ServerTime`).
    let left = |m: &CaMission, app: &App| m.timers[0].ends - app.world().resource::<chimera_core::core::SimClock>().now;
    let (a, b) = (left(&server, &sim.server.app), left(&late, &sim.clients[1].app));
    assert!((a - b).abs() < 2.0, "server {a} s left, late joiner {b} s");
    assert!(b > 200.0 && b <= 300.0, "{b}");
}
