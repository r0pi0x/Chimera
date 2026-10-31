//! specs/combat_arms/world_objects.md Follow-up (2026-10-31b) 1: the event
//! objects' cases 27-39 on the object world (`game::objects::events`),
//! driven at the server's 20 Hz like `world_objects`: `TimerTrigger`
//! (27-30), `LightActivateTrigger` (31, 32), `CaptionTrigger` (33),
//! `MissionTrigger` (34), the `TargetTrigger` marker's game type (36),
//! `Camera` (38, 39). The markers' icons (35, 37) are `world_markers`; the
//! stock `VipExitTimer` in a VIP Escort round (28's zone, game_modes_2.md
//! 54) is `vip_escort`.

use bevy::prelude::*;
use combat_arms::game::objects::{
    commands::Who,
    events::{Marker, MarkerTeam, Notice, ObjectiveState, UseAnswer},
    runtime::{Fx, Kind},
};

use crate::world_objects::{T, close, obj, player, v};

/// A timer named `X` with T `time`, `AutoStop` `auto`, `InTimeCommandTime`
/// `at`, its commands adding to `i`, `o` and `j`.
fn timer(time: f32, auto: bool, at: f32) -> T {
    let mut t = T::new(vec![obj(
        "TimerTrigger",
        "X",
        &[
            ("TimeAmount", time.to_string()),
            ("AutoStop", (auto as u8).to_string()),
            ("InTimeCommandTime", at.to_string()),
            ("InTime_Command", "add i 1".into()),
            ("OverTime_Command", "add o 1".into()),
            ("JustTime_Command", "add j 1".into()),
        ],
    )]);
    t.run("int i 0; int o 0; int j 0");
    t
}

fn ran(t: &T) -> (i32, i32, i32) {
    (t.var("i").unwrap(), t.var("o").unwrap(), t.var("j").unwrap())
}

/// Run frames until the clock reaches `s`.
fn until(t: &mut T, s: f64) {
    while t.t < s - 1e-6 {
        t.frame();
    }
}

fn notices(t: &T) -> Vec<Notice> {
    t.w.fx
        .iter()
        .filter_map(|f| match f {
            Fx::Notice { notice, .. } => Some(notice.clone()),
            _ => None,
        })
        .collect()
}

/// 27. Late stop: T 180, no auto-stop: `STOP` at 181 → the over-time
/// command once; clients told "on, 180" and "off". `STOP` at 100 → the
/// in-time command. A second `START` at 50 changes nothing.
#[test]
fn t27_timer_late_stop() {
    let mut t = timer(180.0, false, 0.0);
    t.send("X", "START");
    until(&mut t, 50.0);
    t.send("X", "START");
    until(&mut t, 181.0);
    t.send("X", "STOP");
    assert_eq!(ran(&t), (0, 1, 0), "over time once");
    assert_eq!(
        notices(&t),
        vec![Notice::Timer { on: true, seconds: 180.0 }, Notice::Timer { on: false, seconds: 0.0 }]
    );
    t.send("X", "STOP");
    assert_eq!(ran(&t), (0, 1, 0), "a stopped timer ignores STOP");

    let mut t = timer(180.0, false, 0.0);
    t.send("X", "START");
    until(&mut t, 100.0);
    t.send("X", "STOP");
    assert_eq!(ran(&t), (1, 0, 0), "in time");
}

/// 28. The stock `VipExitTimer` (T 60, auto-stop, in-time command at 54
/// s): the in-time command at the first update at or after 54 s, the
/// over-time one (not just-time) at the first at or after 61 s. A `STOP`
/// at 30 s runs nothing (in-time suppressed) and turns the clock off.
#[test]
fn t28_timer_auto_stop() {
    let mut t = timer(60.0, true, 54.0);
    t.send("X", "START");
    let (mut early, mut late) = (None, None);
    while t.t < 70.0 {
        let at = t.t;
        t.frame();
        let r = ran(&t);
        if r.0 == 1 && early.is_none() {
            early = Some(at);
        }
        if r.1 == 1 && late.is_none() {
            late = Some(at);
        }
    }
    close(early.unwrap() as f32, 54.0, 1e-3, "in time at 54 s");
    close(late.unwrap() as f32, 61.0, 1e-3, "over time at 61 s");
    assert_eq!(ran(&t), (1, 1, 0), "never the just-time command");

    let mut t = timer(60.0, true, 54.0);
    t.send("X", "START");
    until(&mut t, 30.0);
    t.send("X", "STOP");
    until(&mut t, 70.0);
    assert_eq!(ran(&t), (0, 0, 0), "nothing ran");
    assert!(matches!(&t.w.objects[0].kind, Kind::Timer(x) if x.started.is_none()), "clock off");
}

/// 29. A timer that never updates (decided at load): `INTIMECOMMANDTIME 5`
/// then `START` at 0 → nothing at 5 s; `STOP` at 6 → nothing (in-time
/// suppressed by the new value).
#[test]
fn t29_timer_without_updates() {
    let mut t = timer(10.0, false, 0.0);
    t.send("X", "INTIMECOMMANDTIME 5");
    t.send("X", "START");
    until(&mut t, 6.0);
    assert_eq!(ran(&t), (0, 0, 0), "nothing at 5 s");
    t.send("X", "STOP");
    assert_eq!(ran(&t), (0, 0, 0), "nothing at the stop");
}

/// 30. `TIMEAMOUNT 30` at 5 s on a running auto-stop timer (T 10): it
/// still stops itself at the first update at or after 11 s, running the
/// in-time command (11 < 30).
#[test]
fn t30_timeamount_while_running() {
    let mut t = timer(10.0, true, 0.0);
    t.send("X", "START");
    until(&mut t, 5.0);
    t.send("X", "TIMEAMOUNT 30");
    until(&mut t, 10.95);
    t.frame();
    assert_eq!(ran(&t), (0, 0, 0), "running at 10.95 s");
    t.frame();
    assert_eq!(ran(&t), (1, 0, 0), "stopped at 11 s, in time");
}

/// A use spot `U`: 5 s, `limit` uses, its commands adding to `w`, `r`
/// and `a`.
fn use_spot(limit: u32) -> T {
    let mut t = T::new(vec![obj(
        "LightActivateTrigger",
        "U",
        &[
            ("Dims", v(32.0, 32.0, 32.0)),
            ("TimeAmount", "5".into()),
            ("NumberOfActivations", limit.to_string()),
            ("WorkCommand", "add w 1".into()),
            ("ReleaseCommand", "add r 1".into()),
            ("ActivateCommand", "add a 1".into()),
        ],
    )]);
    t.run("int w 0; int r 0; int a 0");
    t
}

fn wra(t: &T) -> (i32, i32, i32) {
    (t.var("w").unwrap(), t.var("r").unwrap(), t.var("a").unwrap())
}

/// Character `id` at the spot, holding use or not.
fn user(id: u64, using: bool, alive: bool) -> combat_arms::game::objects::movers::CharBox {
    let mut c = player(id, Vec3::new(-10.0, -10.0, -10.0), Vec3::new(10.0, 10.0, 10.0));
    c.using = using;
    c.alive = alive;
    c
}

/// 31. One use allowed: P from 0 s → `WorkCommand` at the next update;
/// at 5.05 s it completes (`ActivateCommand`, success) and the release
/// runs. Q from 6 s → no work command (used up); at 11.05 s the spot locks
/// itself, no activate, the release runs.
#[test]
fn t31_light_activate_use() {
    let mut t = use_spot(1);
    t.w.chars = vec![user(1, true, true)];
    t.frame();
    assert_eq!(wra(&t), (1, 0, 0), "work at the start");
    until(&mut t, 5.0);
    assert_eq!(wra(&t), (1, 0, 0), "still working at 5 s");
    t.frame();
    assert_eq!(wra(&t), (1, 1, 1), "completed at 5.05 s, released");
    assert!(notices(&t).contains(&Notice::Use { who: Who::Character(1), answer: UseAnswer::Success }));
    t.w.chars = vec![user(1, false, true)];
    until(&mut t, 6.0);
    t.w.chars = vec![user(2, true, true)];
    t.frame();
    assert_eq!(wra(&t), (1, 1, 1), "no work command when used up");
    until(&mut t, 11.05);
    t.frame();
    assert_eq!(wra(&t), (1, 2, 1), "locked, no activate, released");
    assert!(matches!(&t.w.objects[0].kind, Kind::LightUse(u) if u.locked));
    assert!(notices(&t).contains(&Notice::UseLocked { locked: true }));
}

/// 32. Two users (unlimited): P from 0, Q from 1 s → one work command;
/// P completes at about 5 s, Q at about 6 s (two activates), one release
/// after Q's. P dead at 3 s → no activate for P.
#[test]
fn t32_light_activate_two_users() {
    let mut t = use_spot(0);
    t.w.chars = vec![user(1, true, true), user(2, false, true)];
    t.frame();
    until(&mut t, 1.0);
    t.w.chars = vec![user(1, true, true), user(2, true, true)];
    until(&mut t, 5.1);
    assert_eq!(wra(&t), (1, 0, 1), "P completed");
    until(&mut t, 6.1);
    assert_eq!(wra(&t), (1, 1, 2), "Q completed, released once");

    let mut t = use_spot(0);
    t.w.chars = vec![user(1, true, true), user(2, false, true)];
    t.frame();
    until(&mut t, 1.0);
    t.w.chars = vec![user(1, true, true), user(2, true, true)];
    until(&mut t, 3.0);
    t.w.chars = vec![user(1, true, false), user(2, true, true)];
    until(&mut t, 6.1);
    assert_eq!(wra(&t), (1, 1, 1), "only Q's activate");
}

/// 33. `CapTRG01` (`Showtime` 4): `PLAY Cap404` → every client gets it
/// for 4 s, team 0; after `LOCK` nothing; `CLEAR` still hides.
#[test]
fn t33_caption() {
    let mut t = T::new(vec![obj("CaptionTrigger", "CapTRG01", &[("Showtime", "4".into())])]);
    t.run("msg CapTRG01 (play Cap404)");
    t.send("CapTRG01", "LOCK");
    t.run("msg CapTRG01 (play Cap404)");
    t.send("CapTRG01", "CLEAR");
    assert_eq!(
        notices(&t),
        vec![
            Notice::Caption {
                id: "Cap404".into(),
                showtime: 4.0,
                team: 0
            },
            Notice::HideCaption
        ]
    );
    let mut m = T::new(vec![obj("MessageTrigger", "MsgTRG01", &[("Showtime", "4".into())])]);
    m.run("msg MsgTRG01 (play Msg1 2)");
    assert_eq!(
        notices(&m),
        vec![Notice::Message {
            id: "Msg1".into(),
            showtime: 4.0,
            team: 2
        }]
    );
}

/// 34. The objective list: `ADD 0` twice, `ADD_POINT_MISSION 2 99` (the
/// data's maximum 5), `UPDATE_POINT_MISSION 2 3`, `CHECK 0`, `FAIL 7` →
/// [0 success; 2 in progress, 3 of 5]; four changes sent.
#[test]
fn t34_mission_list() {
    let mut t = T::new(vec![obj("MissionTrigger", "MishTRG01", &[])]);
    t.w.mission_points.insert("2".into(), 5);
    for m in ["ADD 0", "ADD 0", "ADD_POINT_MISSION 2 99", "UPDATE_POINT_MISSION 2 3", "CHECK 0", "FAIL 7"] {
        t.send("MishTRG01", m);
    }
    let Kind::Mission(list) = &t.w.objects[0].kind else { panic!() };
    assert_eq!(list.len(), 2);
    assert_eq!((list[0].id.as_str(), list[0].state), ("0", ObjectiveState::Success));
    assert_eq!((list[1].id.as_str(), list[1].state, list[1].count), ("2", ObjectiveState::InProgress, Some((3, 5))));
    assert_eq!(notices(&t).len(), 4, "{:?}", notices(&t));
}

/// 36. A marker of `GameType` `HeadQuaters` exists for clients in Seize
/// and Secure only (TDM: never heard of); `ON` and `OFF` switch it. The
/// team: `CheckTeam` false → all teams, `<none>` → nobody, `JSF` → JSF.
#[test]
fn t36_marker_game_type() {
    let mut t = T::new(vec![obj(
        "TargetTrigger",
        "Indicator_A00",
        &[("GameType", "HeadQuaters".into()), ("Team", "JSF".into()), ("CheckTeam", "1".into())],
    )]);
    let marker = |t: &T| match &t.w.objects[0].kind {
        Kind::Trigger(tr) => tr.marker.clone().unwrap(),
        _ => panic!(),
    };
    let m = marker(&t);
    assert!(!m.on, "starts off");
    assert_eq!(m.team, MarkerTeam::Team("JSF".into()));
    assert!(m.exists_in(Some("HeadQuaters")), "Seize and Secure");
    assert!(!m.exists_in(Some("Team Death Match")), "TDM");
    t.send("Indicator_A00", "ON");
    assert!(marker(&t).on);
    t.send("Indicator_A00", "OFF");
    assert!(!marker(&t).on);
    let of = |props: &[(&str, &str)]| {
        let (kv, _) = obj("TargetTrigger", "M", &props.iter().map(|(k, v)| (*k, v.to_string())).collect::<Vec<_>>());
        Marker::of(&combat_arms::game::objects::runtime::Props(kv)).team
    };
    assert_eq!(of(&[("CheckTeam", "0"), ("Team", "JSF")]), MarkerTeam::All);
    assert_eq!(of(&[("CheckTeam", "1"), ("Team", "<none>")]), MarkerTeam::Nobody);
}

/// A camera and a point and a trigger to move it to.
fn camera(one_time: bool, can_skip: bool) -> T {
    let mut t = T::new(vec![
        obj(
            "Camera",
            "Camera0",
            &[
                ("FovY", "75".into()),
                ("OneTime", (one_time as u8).to_string()),
                ("CanSkip", (can_skip as u8).to_string()),
                ("CleanupCommand", "add c 1".into()),
                ("origin", v(0.0, 0.0, 0.0)),
            ],
        ),
        obj("CameraPoint", "A_Exp_Cam01", &[("origin", v(100.0, 50.0, -20.0)), ("Rotation", v(0.0, 1.0, 0.0))]),
        obj("Trigger", "Trig", &[("origin", v(7.0, 7.0, 7.0))]),
    ]);
    t.run("int c 0");
    t
}

/// 38. `MOVETO A_Exp_Cam01`: the point's position and rotation at once;
/// naming a `Trigger` does nothing. `FOV 55 41.25 2`: clients blend from
/// 90 x 75 to 55 x 41.25 over 2 s (72.5 x 58.125 half way).
#[test]
fn t38_camera_move() {
    let mut t = camera(false, true);
    t.run("msg Camera0 (moveto A_Exp_Cam01)");
    let (cam, point) = (t.w.objects[0].pose, t.w.objects[1].pose);
    assert_eq!((cam.pos, cam.rot), (point.pos, point.rot), "at the point");
    t.run("msg Camera0 (moveto Trig)");
    assert_eq!(t.w.objects[0].pose.pos, point.pos, "not to a trigger");
    t.run("msg Camera0 (fov 55 41.25 2)");
    let Kind::Camera(c) = &t.w.objects[0].kind else { panic!() };
    let half = c.fov_at(c.fov_since + 1.0);
    close(half.x, 72.5, 1e-4, "x half way");
    close(half.y, 58.125, 1e-4, "y half way");
    assert_eq!(c.fov_at(c.fov_since + 2.5), Vec2::new(55.0, 41.25));
    assert!(notices(&t).contains(&Notice::Fov { fov: Vec2::new(55.0, 41.25), seconds: 2.0 }));
}

/// 39. `OFF` on a camera that isn't live still runs its cleanup; it
/// stays (`OneTime` false) or goes (`OneTime` true); `SKIP` with
/// `CanSkip` false does nothing; `ActiveTime` turns a live camera off.
#[test]
fn t39_camera_off() {
    let mut t = camera(false, false);
    t.send("Camera0", "OFF");
    assert_eq!(t.var("c"), Some(1), "cleanup");
    assert!(!t.w.objects[0].removed, "stays");
    t.send("Camera0", "ON");
    t.send("Camera0", "SKIP");
    assert!(matches!(&t.w.objects[0].kind, Kind::Camera(c) if c.live), "SKIP refused");
    assert_eq!(t.var("c"), Some(1));

    let mut t = camera(true, true);
    t.send("Camera0", "OFF");
    assert!(t.w.objects[0].removed, "a one-time camera goes");

    let mut t = T::new(vec![obj("Camera", "Cam", &[("ActiveTime", "2".into()), ("OneTime", "0".into())])]);
    t.send("Cam", "ON");
    until(&mut t, 2.0);
    assert!(matches!(&t.w.objects[0].kind, Kind::Camera(c) if c.live), "live through 1.95 s");
    t.frame();
    assert!(matches!(&t.w.objects[0].kind, Kind::Camera(c) if !c.live), "off after 2 s");
}
