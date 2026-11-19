//! specs/combat_arms/world_objects.md "Corrections (2026-10-22)" (1-16) on
//! the object world (`game::objects::runtime::ObjectWorld`), driven at
//! 20 Hz like `world_objects`: the reversal test of one frame's step, door
//! links, looping sounds stopping, `SOLID 0`, `SETPOWERONMULTIPLIER`,
//! command strings run only when their first statement is valid, `ABORT`,
//! nested runs clearing the context, `CommandTouch`, `VMSG`, `RAND`,
//! `REPEAT 0`, delays queued in the manager's walk, event counters
//! between steps, `REMEMBER`, and the volume nudge. The volumes' own
//! corrections (world_dat.md) are `volume_corrections`.

use bevy::prelude::*;
use combat_arms::game::objects::{
    commands::Who,
    movers::State,
    runtime::{Fx, Kind},
};

use crate::world_objects::{DT, T, close, lamp, obj, player, v, wm};

fn slider() -> T {
    T::new(vec![wm(
        "SlidingWorldModel",
        "Lift",
        Vec3::ZERO,
        Vec3::new(50.0, 10.0, 50.0),
        &[
            ("MoveDir", v(0.0, 1.0, 0.0)),
            ("MoveDist", "100".into()),
            ("PowerOnTime", "1".into()),
            ("PowerOffTime", "1".into()),
            ("Waveform", "Linear".into()),
        ],
    )])
}

/// 1: the reversal test steps one frame (5 u), not 10 % of the move.
#[test]
fn c01_reversal_tests_one_frames_step() {
    // The lift is at 50 u, its underside at 40. The activator's box top
    // lies `gap` below it; closing is down.
    for (gap, reverses) in [(7.0, true), (4.0, false)] {
        let mut t = slider();
        let p = Who::Character(1);
        t.send_from("Lift", "ON", p);
        t.frames(10);
        close(t.pos("Lift").y, 50.0, 1e-3, "half way");
        let top = 40.0 - gap;
        t.w.chars = vec![player(1, Vec3::new(-5.0, top - 80.0, -5.0), Vec3::new(5.0, top, 5.0))];
        t.send_from("Lift", "OFF", p);
        let want = if reverses { State::PowerOff } else { State::PowerOn };
        assert_eq!(t.state("Lift"), want, "gap {gap}");
    }
}

#[test]
fn c01_rotating_reversal_uses_the_current_angle() {
    let door = || {
        T::new(vec![wm(
            "RotatingDoor",
            "Door",
            Vec3::ZERO,
            Vec3::new(32.0, 64.0, 4.0),
            &[
                ("Pos", v(-32.0, 0.0, 0.0)),
                ("RotationAngles", v(0.0, 90.0, 0.0)),
                ("OpeningTime", "1".into()),
                ("ClosingTime", "1".into()),
                ("OpenAway", "0".into()),
            ],
        )])
    };
    let hinge = Vec3::new(-32.0, 0.0, 0.0);
    let mut open = door();
    open.send_from("Door", "ON", Who::Character(1));
    open.frames(9);
    let c40 = open.pos("Door");
    open.frames(1);
    let c45 = open.pos("Door");
    // Which way a negative angle about +Y turns the door's centre.
    let about = |c: Vec3, deg: f32, k: f32| hinge + Quat::from_rotation_y(k * deg.to_radians()) * (c - hinge);
    let k = if about(c45, -4.5, 1.0).distance(c40) < about(c45, -4.5, -1.0).distance(c40) {
        1.0
    } else {
        -1.0
    };
    // A point near the door's free end (62 u from the hinge) at yaw 36
    // degrees: inside the old test pose (45 - 9), outside the new
    // one (45 - 2.25), whose thickness is 8 u.
    let end = |t: &T| {
        let rot = t.w.objects[0].pose.rot;
        let c = t.pos("Door");
        let tip = [32.0, -32.0].map(|x| c + rot * Vec3::new(x, 0.0, 0.0));
        let far = if tip[0].distance(hinge) > tip[1].distance(hinge) {
            tip[0]
        } else {
            tip[1]
        };
        hinge + (far - hinge) * (62.0 / 64.0)
    };
    let p36 = about(end(&open), -9.0, k);
    for (at, reverses) in [(p36, true)] {
        let mut t = door();
        let p = Who::Character(1);
        t.send_from("Door", "ON", p);
        t.frames(10);
        t.w.chars = vec![player(
            1,
            at - Vec3::new(0.5, 30.0, 0.5),
            at + Vec3::new(0.5, 30.0, 0.5),
        )];
        t.send_from("Door", "OFF", p);
        let want = if reverses { State::PowerOff } else { State::PowerOn };
        assert_eq!(t.state("Door"), want, "a point where the old test (9 degrees) stood");
    }
    // And one inside the new test pose refuses.
    let p43 = about(end(&open), -2.25, k);
    let mut t = door();
    let p = Who::Character(1);
    t.send_from("Door", "ON", p);
    t.frames(10);
    t.w.chars = vec![player(
        1,
        p43 - Vec3::new(0.5, 30.0, 0.5),
        p43 + Vec3::new(0.5, 30.0, 0.5),
    )];
    t.send_from("Door", "OFF", p);
    assert_eq!(t.state("Door"), State::PowerOn, "inside the new pose it refuses");
}

fn link_door(name: &str, x: f32, link: &str) -> (Vec<(String, String)>, Option<Vec3>) {
    wm(
        "SlidingDoor",
        name,
        Vec3::new(x, 0.0, 0.0),
        Vec3::new(30.0, 60.0, 4.0),
        &[
            ("DoorLink", link.into()),
            ("MoveDist", "100".into()),
            ("OpeningTime", "1".into()),
            ("ClosingTime", "1".into()),
        ],
    )
}

/// 2: the link fires when a closing door is reversed, and on map-command
/// triggers; only the first object of the name.
#[test]
fn c02_door_link_on_reversal_and_commands() {
    let mut t = T::new(vec![link_door("L", -40.0, "R"), link_door("R", 40.0, "")]);
    let p = Who::Character(7);
    t.w.chars = vec![player(
        7,
        Vec3::new(-20.0, -50.0, -200.0),
        Vec3::new(20.0, 50.0, -150.0),
    )];
    t.send_from("L", "ACTIVATE", p);
    t.frames(30);
    // Both closing, half way.
    t.send_from("L", "ACTIVATE", p);
    t.send_from("R", "ACTIVATE", p);
    t.frames(10);
    assert_eq!((t.state("L"), t.state("R")), (State::PowerOff, State::PowerOff));
    t.send_from("L", "ACTIVATE", p);
    assert_eq!(
        (t.state("L"), t.state("R")),
        (State::PowerOn, State::PowerOn),
        "reversed together"
    );
    // A map command: both open.
    let mut t = T::new(vec![link_door("L", -40.0, "R"), link_door("R", 40.0, "")]);
    t.run("msg L trigger");
    assert_eq!((t.state("L"), t.state("R")), (State::PowerOn, State::PowerOn));
    // Two objects named R: the first only.
    let mut t = T::new(vec![
        link_door("L", -40.0, "R"),
        link_door("R", 40.0, ""),
        link_door("R", 120.0, ""),
    ]);
    t.run("msg L trigger");
    assert_eq!(t.w.objects[1].awm().unwrap().state, State::PowerOn);
    assert_eq!(t.w.objects[2].awm().unwrap().state, State::Off);
}

fn sounds(t: &T) -> Vec<String> {
    t.w.fx
        .iter()
        .filter_map(|f| match f {
            Fx::Sound { file, looping, .. } => Some(format!("{file}{}", if *looping { " loop" } else { "" })),
            Fx::StopSound { .. } => Some("stop".into()),
            _ => None,
        })
        .collect()
}

/// 3: a looping transition sound stops at every transition and on a
/// locked use.
#[test]
fn c03_looping_sounds_stop() {
    let mut t = T::new(vec![wm(
        "SlidingDoor",
        "Door",
        Vec3::ZERO,
        Vec3::new(30.0, 60.0, 4.0),
        &[
            ("LoopSounds", "1".into()),
            ("OpeningSound", "open.wav".into()),
            ("OpeningTime", "1".into()),
            ("RemainOpen", "1".into()),
        ],
    )]);
    t.w.fx.clear();
    t.send("Door", "TRIGGER");
    assert_eq!(sounds(&t), ["stop", "open.wav loop"]);
    t.w.fx.clear();
    t.frames(19);
    assert!(sounds(&t).is_empty(), "silent while it moves");
    t.frame();
    assert_eq!(t.state("Door"), State::On);
    assert_eq!(sounds(&t), ["stop"], "silent from the frame it reaches On");
    // A locked use of a moving looping door.
    let mut t = T::new(vec![wm(
        "SlidingDoor",
        "Door",
        Vec3::ZERO,
        Vec3::new(30.0, 60.0, 4.0),
        &[
            ("LoopSounds", "1".into()),
            ("OpeningSound", "open.wav".into()),
            ("OpeningTime", "1".into()),
        ],
    )]);
    t.send("Door", "TRIGGER");
    t.frames(3);
    t.send("Door", "LOCK");
    t.w.fx.clear();
    t.send("Door", "TRIGGER");
    assert_eq!(sounds(&t), ["stop"]);
    // A one-shot sound isn't stopped.
    let mut t = T::new(vec![wm(
        "SlidingDoor",
        "Door",
        Vec3::ZERO,
        Vec3::new(30.0, 60.0, 4.0),
        &[("OpeningSound", "open.wav".into())],
    )]);
    t.send("Door", "TRIGGER");
    assert_eq!(sounds(&t), ["open.wav"]);
}

/// 4: `SOLID 0` clears ray-hit; `HIDDEN 0` never clears a flag.
#[test]
fn c04_solid_zero_clears_rayhit() {
    let mut t = T::new(vec![wm("WorldModel", "Wall", Vec3::ZERO, Vec3::splat(10.0), &[])]);
    assert!(t.w.objects[0].solid && t.w.objects[0].rayhit);
    t.send("Wall", "solid 0");
    assert!(!t.w.objects[0].solid && !t.w.objects[0].rayhit, "bullets pass");
    t.send("Wall", "solid 1");
    assert!(t.w.objects[0].solid && t.w.objects[0].rayhit, "stops bullets again");
    let mut t = T::new(vec![wm("WorldModel", "Wall", Vec3::ZERO, Vec3::splat(10.0), &[])]);
    t.w.objects[0].visible = false;
    t.send("Wall", "visible 1");
    t.send("Wall", "hidden 1");
    assert!(!t.w.objects[0].visible);
    t.send("Wall", "hidden 0");
    assert!(t.w.objects[0].visible, "stays visible");
}

/// 5: `SETPOWERONMULTIPLIER` replaces.
#[test]
fn c05_power_on_multiplier_replaces() {
    let mut t = slider();
    t.send("Lift", "SETPOWERONMULTIPLIER 2");
    t.send("Lift", "SETPOWERONMULTIPLIER 3");
    t.send("Lift", "TRIGGER");
    t.frames(55);
    assert_eq!(t.state("Lift"), State::PowerOn);
    t.frames(10);
    assert_eq!(t.state("Lift"), State::On, "3 s, not 6");
}

/// 6: a string runs only when its first statement is valid.
#[test]
fn c06_first_statement_must_be_valid() {
    let mut t = T::new(vec![
        lamp("A"),
        lamp("B"),
        obj("CommandTrigger", "Trig", &[("Command0", "mgs A on; msg B on".into())]),
    ]);
    t.send("Trig", "TRIGGER");
    assert!(!t.on("A") && !t.on("B"), "typo first: nothing");
    t.run("msg B on; mgs A on");
    assert!(t.on("B") && !t.on("A"), "valid first: runs");
    let mut t = T::new(vec![lamp("A"), lamp("B")]);
    t.run("delay 1 (mgs A on); msg B on");
    assert!(!t.on("B"), "the failed delay stops its string");
    assert_eq!(t.w.cmds.pending_count(), 0, "nothing queued");
    t.run("repeat 1 1 1 1 (mgs A on); msg B on");
    t.run("loop 1 1 (mgs A on); msg B on");
    assert!(!t.on("B"));
    assert_eq!(t.w.cmds.pending_count(), 0);
}

/// 7: `ABORT` with nothing pending fails; ids longer than 15 characters
/// are never found.
#[test]
fn c07_abort_fails_when_nothing_is_pending() {
    let mut t = T::new(vec![lamp("A")]);
    t.run("abort Wup; msg A on");
    assert!(!t.on("A"));
    t.run("msg A on; abort Wup");
    assert!(t.on("A"));
    let mut t = T::new(vec![lamp("A")]);
    let long = "AVeryLongCommandId";
    t.run(&format!("delayid {long} 5 (msg A on); abort {long}; msg A off"));
    assert_eq!(t.w.cmds.pending_count(), 1, "never aborted");
    // A full table: the command runs at once and counts as a failure.
    let mut t = T::new(vec![lamp("A"), lamp("B")]);
    t.run("int n 0");
    for _ in 0..512 {
        t.run("delay 100 (add n 1)");
    }
    t.run("delay 100 (msg A on); msg B on");
    assert!(t.on("A") && !t.on("B"));
}

/// 8: a nested run clears the outer string's sender.
#[test]
fn c08_nested_run_clears_the_sender() {
    let make = |opening: &str| {
        let mut props = vec![("MoveDist", "100".to_string())];
        if !opening.is_empty() {
            props.push(("OpeningCommand", opening.to_string()));
        }
        let door_b = wm(
            "RotatingDoor",
            "DoorB",
            Vec3::new(200.0, 0.0, 0.0),
            Vec3::new(30.0, 60.0, 4.0),
            &[("Pos", v(170.0, 0.0, 0.0))],
        );
        let mut t = T::new(vec![
            obj(
                "Trigger",
                "Trig",
                &[("Command1", "msg DoorA trigger; msg DoorB on".into())],
            ),
            wm(
                "SlidingDoor",
                "DoorA",
                Vec3::new(-200.0, 0.0, 0.0),
                Vec3::new(30.0, 60.0, 4.0),
                &props,
            ),
            door_b,
            lamp("X"),
        ]);
        t.run("int n 0");
        t
    };
    let p = Who::Character(7);
    let activator_b = |t: &T| t.w.objects[2].awm().unwrap().activator;
    let mut t = make("add n 1");
    let trig = t.id("Trig");
    t.w.fire(trig, p, false);
    assert_eq!(t.var("n"), Some(1), "DoorA's opening command ran");
    assert_eq!(activator_b(&t), Who::Nobody, "no sender after a nested run");
    let mut t = make("");
    let trig = t.id("Trig");
    t.w.fire(trig, p, false);
    assert_eq!(activator_b(&t), p);
}

/// 9: `CommandTouch` targets the toucher.
#[test]
fn c09_command_touch_targets_the_toucher() {
    let mut t = T::new(vec![obj(
        "Trigger",
        "Pad",
        &[
            ("TriggerTouch", "1".into()),
            ("CommandTouch", "msg () (visible 0)".into()),
        ],
    )]);
    let p = Who::Character(7);
    t.w.fire(0, p, true);
    assert!(
        t.w.fx
            .iter()
            .any(|f| matches!(f, Fx::CharacterMessage { who, message } if *who == p && message == "visible 0")),
        "{:?}",
        t.w.fx
    );
}

/// 10: `VMSG` with an unknown variable goes on. 11: a failing `RAND`
/// branch stops the string. 12: `REPEAT` with a roll of 0 runs once.
#[test]
fn c10_to_c12_vmsg_rand_repeat() {
    let mut t = T::new(vec![lamp("A"), lamp("B")]);
    t.run("vmsg Nope (on); msg A on");
    assert!(t.on("A"));
    t.run("rand 1 (set Undeclared 1) (msg A off); msg B on");
    assert!(!t.on("B"), "the failing branch stops the outer string");
    t.run("rand 1 (msg A off) (msg A on); msg B on");
    assert!(t.on("B") && !t.on("A"));
    let mut t = T::new(vec![lamp("A")]);
    t.run("repeat 0 0 1 1 (msg A on)");
    t.frames(10);
    assert!(!t.on("A"));
    t.frames(11);
    assert!(t.on("A"), "once, 1 s later");
    assert_eq!(t.w.cmds.pending_count(), 0);
}

/// 13: a delay queued during the manager's walk into a later slot runs in
/// the same walk.
#[test]
fn c13_delays_queued_in_the_walk() {
    let mut t = T::new(vec![lamp("A")]);
    t.run("delay 0.05 (delay 0 (msg A on))");
    assert!(!t.on("A"));
    t.frame();
    assert!(t.on("A"), "both delays in one walk");
}

/// 14: an event counter runs each step's commands before the next; a
/// `LOCK` stops the rest.
#[test]
fn c14_counter_steps_run_between() {
    let mut t = T::new(vec![
        obj(
            "EventCounter",
            "Counter",
            &[
                ("StartingValue", "0".into()),
                ("Event1Value", "1".into()),
                ("Event1IncToValCmd", "msg Counter lock".into()),
                ("Event2Value", "2".into()),
                ("Event2IncToValCmd", "msg A on".into()),
            ],
        ),
        lamp("A"),
    ]);
    t.send("Counter", "INC 2");
    let value = match &t.w.objects[0].kind {
        Kind::EventCounter { value, .. } => *value,
        _ => unreachable!(),
    };
    assert_eq!(value, 1, "locked after the first step");
    assert!(!t.on("A"));
    // 2.5 is 2 steps.
    let mut t = T::new(vec![
        obj(
            "EventCounter",
            "Counter",
            &[("Event2Value", "2".into()), ("Event2IncToValCmd", "msg A on".into())],
        ),
        lamp("A"),
    ]);
    t.send("Counter", "INC 2.5");
    assert!(t.on("A"));
}

/// 15: `REMEMBER` restores the logic state, not the pose or the flags.
#[test]
fn c15_remember_doesnt_move() {
    let mut t = slider();
    t.send("Lift", "MEMORIZE");
    t.send("Lift", "TRIGGER");
    t.frames(30);
    close(t.pos("Lift").y, 100.0, 1e-3, "open");
    t.w.objects[0].visible = false;
    t.send("Lift", "REMEMBER");
    close(t.pos("Lift").y, 100.0, 1e-3, "stays where it is");
    assert!(!t.w.objects[0].visible, "flags stay");
    assert_eq!(t.state("Lift"), State::Off);
    t.send("Lift", "TRIGGER");
    t.frames(30);
    close(t.pos("Lift").y, 200.0, 1e-3, "slides a further MoveDist");
}

/// 16: switching a volume on nudges the players, touch-firing the
/// triggers they stand in.
#[test]
fn c16_volume_on_nudges_players() {
    let mut t = T::new(vec![
        obj(
            "TouchTrigger",
            "TT",
            &[("NumberOfActivations", "0".into()), ("Command0", "add n 1".into())],
        ),
        obj("PoisonGas", "Gas", &[]),
    ]);
    t.run("int n 0");
    t.w.chars = vec![player(1, Vec3::splat(-5.0), Vec3::splat(5.0))];
    t.send("Gas", "on");
    assert_eq!(t.var("n"), Some(1));
    // Off does not.
    t.send("Gas", "off");
    assert_eq!(t.var("n"), Some(1));
    let _ = DT;
}
