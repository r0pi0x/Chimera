//! specs/combat_arms/world_objects.md's synthetic test cases on the
//! object world (`game::objects::runtime::ObjectWorld`) built from map
//! entities written here (the keyvalues `game::world` passes through:
//! vectors in entity axes, x z y of the file's), driven at the server's
//! 20 Hz frames (dt 0.05 s): sliding and rotating motion and their
//! waveforms, reversal, blocking, door links, auto-close, triggers'
//! counts, delays and weights, bots and triggers, command strings,
//! `when`, event counters, breakables. The maps' cases: `heavy::
//! world_objects`; the objects in play on a converted world (use key,
//! bullets, volumes): `world_objects_play`.

use bevy::prelude::*;
use combat_arms::game::objects::{
    commands::{Ctx, Who},
    movers::{CharBox, State},
    runtime::{Kind, ObjectWorld, keys},
};

pub(crate) const DT: f32 = 0.05;

/// A vector property as the map entity carries it (entity axes).
pub(crate) fn v(x: f32, y: f32, z: f32) -> String {
    format!("{x} {z} {y}")
}

/// An object: class, name, properties.
pub(crate) fn obj(class: &str, name: &str, props: &[(&str, String)]) -> (Vec<(String, String)>, Option<Vec3>) {
    let mut kv = vec![
        ("classname".to_string(), class.to_string()),
        ("targetname".to_string(), name.to_string()),
    ];
    kv.extend(props.iter().map(|(k, v)| (k.to_string(), v.clone())));
    (kv, None)
}

/// A world model at `centre` with box half-extents `half`, solid and
/// ray-hit.
pub(crate) fn wm(
    class: &str,
    name: &str,
    centre: Vec3,
    half: Vec3,
    props: &[(&str, String)],
) -> (Vec<(String, String)>, Option<Vec3>) {
    let (mut kv, _) = obj(class, name, props);
    kv.push((keys::HALF.into(), v(half.x, half.y, half.z)));
    kv.push((keys::SOLID.into(), "1".into()));
    kv.push((keys::RAYHIT.into(), "1".into()));
    (kv, Some(centre))
}

/// The object world of `objects`, at time 0.
pub(crate) struct T {
    pub(crate) w: ObjectWorld,
    pub(crate) t: f64,
}

impl T {
    pub(crate) fn new(objects: Vec<(Vec<(String, String)>, Option<Vec3>)>) -> Self {
        Self {
            w: ObjectWorld::build(&objects),
            t: 0.0,
        }
    }

    pub(crate) fn id(&self, name: &str) -> u32 {
        self.w.named_all(name)[0]
    }

    /// One server frame at the current time, then the clock on.
    pub(crate) fn frame(&mut self) {
        self.w.frame(self.t, DT);
        self.t += DT as f64;
    }

    pub(crate) fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.frame();
        }
    }

    /// A message now (before this frame's updates), from nobody.
    pub(crate) fn send(&mut self, name: &str, message: &str) {
        self.w.now = self.t;
        let id = self.id(name);
        self.w.deliver(id, message, Ctx::default());
    }

    /// A message from character `who`.
    pub(crate) fn send_from(&mut self, name: &str, message: &str, who: Who) {
        self.w.now = self.t;
        let id = self.id(name);
        self.w.deliver(
            id,
            message,
            Ctx {
                sender: who,
                target: None,
                ..Ctx::default()
            },
        );
    }

    pub(crate) fn run(&mut self, text: &str) {
        self.w.now = self.t;
        self.w.run(text, Ctx::default());
    }

    pub(crate) fn pos(&self, name: &str) -> Vec3 {
        self.w.objects[self.id(name) as usize].pose.pos
    }

    pub(crate) fn state(&self, name: &str) -> State {
        self.w.objects[self.id(name) as usize].awm().unwrap().state
    }

    pub(crate) fn yaw(&self, name: &str) -> f32 {
        self.w.objects[self.id(name) as usize]
            .awm()
            .unwrap()
            .angles
            .y
            .to_degrees()
    }

    pub(crate) fn var(&self, name: &str) -> Option<i32> {
        self.w.cmds.int(name)
    }

    pub(crate) fn on(&self, name: &str) -> bool {
        match &self.w.objects[self.id(name) as usize].kind {
            Kind::Switchable { on, .. } => *on,
            _ => panic!("{name} isn't switchable"),
        }
    }
}

/// A switchable object (a `SoundFX`) to see commands arrive.
pub(crate) fn lamp(name: &str) -> (Vec<(String, String)>, Option<Vec3>) {
    obj("SoundFX", name, &[("StartOn", "0".into())])
}

pub(crate) fn player(id: u64, min: Vec3, max: Vec3) -> CharBox {
    CharBox {
        who: Who::Character(id),
        min,
        max,
        player: true,
        alive: true,
        using: false,
    }
}

pub(crate) fn close(a: f32, b: f32, tol: f32, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} vs {b}");
}

/// A sliding world model: 100 u along +Y over 1 s with `wave`.
fn slider(wave: &str) -> T {
    T::new(vec![wm(
        "SlidingWorldModel",
        "Lift",
        Vec3::ZERO,
        Vec3::new(50.0, 10.0, 50.0),
        &[
            ("MoveDir", v(0.0, 1.0, 0.0)),
            ("MoveDist", "100".into()),
            ("PowerOnTime", "1".into()),
            ("Waveform", wave.into()),
        ],
    )])
}

/// Case 5: sliding waveforms.
#[test]
fn t05_sliding_waveforms() {
    let mut t = slider("Linear");
    t.send("Lift", "TRIGGER");
    for n in 1..=20 {
        t.frame();
        close(t.pos("Lift").y, 5.0 * n as f32, 1e-3, &format!("linear frame {n}"));
        assert_eq!(t.state("Lift") == State::On, n == 20, "on after 20 moving frames");
    }
    let mut t = slider("Sine");
    t.send("Lift", "TRIGGER");
    t.frame();
    assert_eq!(t.pos("Lift").y, 0.0, "a sine move's first frame moves 0");
    t.frames(9);
    close(t.pos("Lift").y, 45.97, 0.01, "sine after 10 frames");
    t.frames(30);
    assert_eq!(t.pos("Lift").y, 100.0);
    assert_eq!(t.state("Lift"), State::On);
}

/// Case 6: a rotating door reversed half-way.
#[test]
fn t06_rotating_door_reversed() {
    let mut t = T::new(vec![wm(
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
            ("ClosedCommand", "add closed 1".into()),
        ],
    )]);
    t.run("int closed 0");
    t.send("Door", "TRIGGER");
    t.frames(10);
    close(t.yaw("Door"), 45.0, 1e-3, "after 10 frames");
    // The hinge stays put: the centre swings about Pos.
    let c = t.pos("Door");
    close(c.distance(Vec3::new(-32.0, 0.0, 0.0)), 32.0, 1e-3, "about the hinge");
    t.send("Door", "TRIGGER");
    assert_eq!(t.state("Door"), State::PowerOff);
    t.frames(9);
    assert_eq!(t.state("Door"), State::PowerOff);
    t.frame();
    close(t.yaw("Door"), 0.0, 1e-4, "closed");
    assert_eq!(t.state("Door"), State::Off);
    assert_eq!(t.var("closed"), Some(1), "ClosedCommand ran once");
    t.frames(5);
    assert_eq!(t.var("closed"), Some(1));
}

/// Case 7: a door blocked by a standing player waits (no damage, nothing
/// pushed) and goes on once they leave; a bot stops it too (the engine).
#[test]
fn t07_blocked_door() {
    for bot in [false, true] {
        let mut t = T::new(vec![wm(
            "SlidingWorldModel",
            "Door",
            Vec3::ZERO,
            Vec3::new(50.0, 50.0, 10.0),
            &[
                ("MoveDir", v(1.0, 0.0, 0.0)),
                ("MoveDist", "100".into()),
                ("PowerOnTime", "1".into()),
            ],
        )]);
        // The door's right face reaches 77 u from its 6th moving frame.
        let mut c = player(1, Vec3::new(77.0, -40.0, -20.0), Vec3::new(110.0, 40.0, 20.0));
        c.player = !bot;
        t.w.chars = vec![c];
        t.send("Door", "TRIGGER");
        t.frames(10);
        close(t.pos("Door").x, 25.0, 1e-3, "stopped at 25 u");
        let tau = t.w.objects[0].awm().unwrap().tau;
        close(tau, 0.25, 1e-4, "the timer waits");
        assert!(
            t.w.fx
                .iter()
                .all(|f| !matches!(f, combat_arms::game::objects::runtime::Fx::Carry { .. }))
        );
        t.w.chars.clear();
        t.frames(15);
        close(t.pos("Door").x, 100.0, 1e-3, "open once they left");
        assert_eq!(t.state("Door"), State::On);
    }
}

/// Case 23: a door link.
#[test]
fn t23_door_link() {
    let door = |name: &str, x: f32, link: &str| {
        wm(
            "SlidingDoor",
            name,
            Vec3::new(x, 0.0, 0.0),
            Vec3::new(30.0, 60.0, 4.0),
            &[("DoorLink", link.into()), ("MoveDist", "100".into())],
        )
    };
    let mut t = T::new(vec![door("L", -40.0, "R"), door("R", 40.0, "")]);
    let p = Who::Character(7);
    t.w.chars = vec![player(
        7,
        Vec3::new(-20.0, -50.0, -200.0),
        Vec3::new(20.0, 50.0, -150.0),
    )];
    t.send_from("L", "ACTIVATE", p);
    assert_eq!(
        (t.state("L"), t.state("R")),
        (State::PowerOn, State::PowerOn),
        "both open"
    );
    t.frames(30);
    assert_eq!((t.state("L"), t.state("R")), (State::On, State::On));
    t.send_from("R", "ACTIVATE", p);
    assert_eq!(
        (t.state("L"), t.state("R")),
        (State::On, State::PowerOff),
        "closing isn't linked"
    );
    // A bot opens only its own.
    let mut t = T::new(vec![door("L", -40.0, "R"), door("R", 40.0, "")]);
    let mut bot = player(8, Vec3::new(-20.0, -50.0, -200.0), Vec3::new(20.0, 50.0, -150.0));
    bot.player = false;
    t.w.chars = vec![bot];
    t.send_from("L", "ACTIVATE", Who::Character(8));
    assert_eq!((t.state("L"), t.state("R")), (State::PowerOn, State::Off));
}

/// Case 24: auto-close.
#[test]
fn t24_auto_close() {
    let make = || {
        T::new(vec![wm(
            "SlidingDoor",
            "Door",
            Vec3::ZERO,
            Vec3::new(30.0, 60.0, 4.0),
            &[
                ("RemainOpen", "0".into()),
                ("TriggerClose", "0".into()),
                ("OpenWaitTime", "3".into()),
                ("OpeningTime", "1".into()),
            ],
        )])
    };
    let closing_at = |t: &mut T, trigger_at: Option<f64>| {
        for _ in 0..200 {
            if trigger_at.is_some_and(|at| (t.t - at).abs() < 1e-6) {
                t.send("Door", "TRIGGER");
            }
            t.frame();
            if t.state("Door") == State::PowerOff {
                return t.t - DT as f64;
            }
        }
        panic!("never closed");
    };
    let mut t = make();
    t.send("Door", "TRIGGER");
    let at = closing_at(&mut t, None);
    assert!(at > 3.95 && at < 4.06, "closing at {at}");
    let mut t = make();
    t.send("Door", "TRIGGER");
    let at = closing_at(&mut t, Some(3.0));
    assert!(at > 5.95 && at < 6.06, "re-armed: closing at {at}");
}

/// Case 25: no crush: a forced door closing down onto a crouched player
/// stops above them; nothing deals damage.
#[test]
fn t25_no_crush() {
    let mut t = T::new(vec![wm(
        "SlidingDoor",
        "Shutter",
        Vec3::new(0.0, 150.0, 0.0),
        Vec3::new(60.0, 50.0, 4.0),
        &[
            ("MoveDir", v(0.0, -1.0, 0.0)),
            ("MoveDist", "150".into()),
            ("ForceMove", "1".into()),
            ("PlayerActivate", "0".into()),
        ],
    )]);
    t.w.chars = vec![player(1, Vec3::new(-16.0, 0.0, -16.0), Vec3::new(16.0, 60.0, 16.0))];
    t.send("Shutter", "TRIGGER");
    t.frames(60);
    let bottom = t.pos("Shutter").y - 50.0;
    assert!(
        bottom >= 60.0 - 1e-3 && bottom < 70.0,
        "stopped above the player: {bottom}"
    );
    assert_eq!(t.state("Shutter"), State::PowerOn);
    assert!(
        t.w.fx
            .iter()
            .all(|f| !matches!(f, combat_arms::game::objects::runtime::Fx::Explode { .. }))
    );
}

/// Riders (world_objects.md 10, server): a player standing on a lift
/// rises with it, by the lift's own displacement; a world model that
/// doesn't collide (HGOILRIG's sea, `Solid` false) carries nobody: it
/// passes through the player above it.
#[test]
fn riders_only_on_solid_world_models() {
    use combat_arms::game::objects::runtime::Fx;
    let carried = |solid: bool| {
        let (mut kv, centre) = wm(
            "SlidingWorldModel",
            "Lift",
            Vec3::ZERO,
            Vec3::new(50.0, 10.0, 50.0),
            // Forced, as the sea is (a lift that isn't stops at the
            // player it would push: blocking, case 7).
            &[
                ("MoveDir", v(0.0, 1.0, 0.0)),
                ("MoveDist", "100".into()),
                ("PowerOnTime", "1".into()),
                ("ForceMove", "1".into()),
            ],
        );
        for (k, val) in &mut kv {
            if k == keys::SOLID {
                *val = (solid as u8).to_string();
            }
        }
        let mut t = T::new(vec![(kv, centre)]);
        // Standing on its top (y 10).
        t.w.chars = vec![player(1, Vec3::new(-16.0, 10.0, -16.0), Vec3::new(16.0, 110.0, 16.0))];
        t.send("Lift", "TRIGGER");
        let mut up = 0.0;
        for _ in 0..30 {
            t.frame();
            for f in t.w.fx.drain(..) {
                if let Fx::Carry { delta, .. } = f {
                    up += delta.y;
                }
            }
        }
        assert_eq!(t.state("Lift"), State::On);
        up
    };
    close(carried(true), 100.0, 1e-2, "carried up with a solid lift");
    assert_eq!(
        carried(false),
        0.0,
        "nothing stands on a world model that doesn't collide"
    );
}

/// A trigger relay with `props`, its first command `msg <out> toggle`.
fn trigger(name: &str, props: &[(&str, String)]) -> (Vec<(String, String)>, Option<Vec3>) {
    obj("Trigger", name, props)
}

/// Case 16: activation and fire counts.
#[test]
fn t16_trigger_counting() {
    for (limit, want) in [("2", vec![2, 4]), ("0", vec![2, 4, 6])] {
        let mut t = T::new(vec![trigger(
            "T",
            &[
                ("ActivationCount", "2".into()),
                ("NumberOfActivations", limit.into()),
                ("Command1", "add n 1".into()),
            ],
        )]);
        t.run("int n 0");
        let mut ran = Vec::new();
        for k in 1..=6 {
            let before = t.var("n");
            t.send("T", "TRIGGER");
            t.frame();
            if t.var("n") != before {
                ran.push(k);
            }
        }
        let want: Vec<i32> = want;
        assert_eq!(ran, want, "NumberOfActivations {limit}");
    }
}

/// Case 17: send and trigger delays; the last toucher before activation.
#[test]
fn t17_trigger_delays() {
    let mut t = T::new(vec![trigger(
        "T",
        &[
            ("SendDelay", "1".into()),
            ("Dims", v(50.0, 50.0, 50.0)),
            ("Command1", "msg Sender on".into()),
        ],
    )]);
    let (a, b) = (Who::Character(1), Who::Character(2));
    t.w.fire(t.id("T"), a, true);
    t.frames(10);
    t.w.now = t.t;
    t.w.fire(t.id("T"), b, true);
    t.w.fx.clear();
    let mut fired_at = None;
    for _ in 0..20 {
        let now = t.t;
        t.frame();
        let to: Vec<_> =
            t.w.fx
                .drain(..)
                .filter_map(|f| match f {
                    combat_arms::game::objects::runtime::Fx::CharacterMessage { who, .. } => Some(who),
                    _ => None,
                })
                .collect();
        if !to.is_empty() {
            assert_eq!(to, [b], "the second toucher is the sender");
            fired_at = Some(now);
            break;
        }
    }
    let at = fired_at.expect("fired");
    assert!((at - 1.0).abs() < 1e-6, "at the first update with t >= 1: {at}");

    // TriggerDelay 2: touches at 0, 1, 2.5 fire at 0 and 2.5.
    let mut t = T::new(vec![trigger(
        "T",
        &[
            ("TriggerDelay", "2".into()),
            ("NumberOfActivations", "0".into()),
            ("Command1", "add n 1".into()),
        ],
    )]);
    t.run("int n 0");
    let mut fires = Vec::new();
    for k in 0..60 {
        let now = k as f64 * DT as f64;
        if [0.0, 1.0, 2.5].iter().any(|x| (x - now).abs() < 1e-6) {
            t.w.now = t.t;
            t.w.fire(t.id("T"), a, true);
        }
        let before = t.var("n");
        t.frame();
        if t.var("n") != before {
            fires.push(now);
        }
    }
    assert_eq!(fires.len(), 2, "{fires:?}");
    assert!(fires[0].abs() < 1e-6 && (fires[1] - 2.5).abs() < 1e-6, "{fires:?}");
}

/// Case 18: a weighted trigger runs exactly one of its first two
/// commands, the first a quarter of the time; the third always.
#[test]
fn t18_weighted_trigger() {
    let mut t = T::new(vec![trigger(
        "T",
        &[
            ("WeightedTrigger", "1".into()),
            ("Message1Weight", "0.25".into()),
            ("NumberOfActivations", "0".into()),
            ("Command1", "add a 1".into()),
            ("Command2", "add b 1".into()),
            ("Command3", "add c 1".into()),
        ],
    )]);
    t.run("int a 0; int b 0; int c 0");
    let n = 4000;
    for _ in 0..n {
        t.send("T", "TRIGGER");
    }
    let (a, b, c) = (t.var("a").unwrap(), t.var("b").unwrap(), t.var("c").unwrap());
    assert_eq!(a + b, n, "never both");
    assert_eq!(c, n);
    close(a as f32 / n as f32, 0.25, 0.03, "Command1's share");
}

/// Case 19: bots never fire a trigger by touch; players do.
#[test]
fn t19_bots_and_triggers() {
    let mut t = T::new(vec![trigger(
        "T",
        &[
            ("AITriggerable", "1".into()),
            ("Dims", v(50.0, 50.0, 50.0)),
            ("Command1", "add n 1".into()),
        ],
    )]);
    t.run("int n 0");
    let mut bot = player(1, Vec3::splat(-10.0), Vec3::splat(10.0));
    bot.player = false;
    t.w.chars = vec![bot];
    t.w.touches();
    t.frame();
    assert_eq!(t.var("n"), Some(0), "a bot");
    t.w.chars = vec![player(2, Vec3::splat(-10.0), Vec3::splat(10.0))];
    t.w.touches();
    t.frame();
    assert_eq!(t.var("n"), Some(1), "a player");
}

/// Case 20: command syntax: synchronous delivery, real-time delays,
/// groups of targets, `if`, `rand`, unknown targets, failures.
#[test]
fn t20_command_syntax() {
    let mut t = T::new(vec![
        wm(
            "SlidingDoor",
            "Door01",
            Vec3::ZERO,
            Vec3::new(30.0, 60.0, 4.0),
            &[("MoveDist", "100".into()), ("OpeningTime", "10".into())],
        ),
        wm("WorldModel", "A", Vec3::new(500.0, 0.0, 0.0), Vec3::splat(10.0), &[]),
        wm("WorldModel", "B", Vec3::new(600.0, 0.0, 0.0), Vec3::splat(10.0), &[]),
        lamp("L"),
        lamp("M"),
    ]);
    t.run("msg Door01 trigger; delay 2 (msg Door01 trigger); msg (A B) (visible 0)");
    assert_eq!(t.state("Door01"), State::PowerOn, "now");
    assert!(!t.w.objects[1].visible && !t.w.objects[2].visible, "A and B now");
    // The frame it was queued in counts (its real time), so 2 s later is
    // the 40th frame.
    t.frames(39);
    assert_eq!(t.state("Door01"), State::PowerOn, "not yet");
    t.frame();
    assert_eq!(t.state("Door01"), State::PowerOff, "2 s later: reversed");
    t.run("int Check 0; int TRound 1");
    t.run("if ((Check == 0) AND (TRound == 1)) then (set TRound 0)");
    assert_eq!(t.var("TRound"), Some(0));
    t.run("rand 0.5 (msg L on) (msg M on)");
    assert!(t.on("L") != t.on("M"), "exactly one");
    let mut t = T::new(vec![lamp("A")]);
    t.run("msg Unknown on; msg A on");
    assert!(t.on("A"), "an unknown target is no failure");
    let mut t = T::new(vec![lamp("A")]);
    t.run("set Undeclared 1; msg A on");
    assert!(!t.on("A"), "a failure stops the string");
}

/// Case 21: `when`.
#[test]
fn t21_when() {
    let mut t = T::new(vec![obj("CommandTrigger", "C", &[("Command0", "add hits 1".into())])]);
    t.run("int X 0; int hits 0");
    t.run("when (X == 3) then (msg C trigger)");
    t.run("add X 1");
    t.run("add X 1");
    assert_eq!(t.var("hits"), Some(0));
    t.run("add X 1");
    assert_eq!(t.var("hits"), Some(1), "A on once");
    t.run("sub X 1; add X 1; set X 3");
    assert_eq!(t.var("hits"), Some(1), "only once");
}

/// Case 22: an event counter.
#[test]
fn t22_event_counter() {
    let mut t = T::new(vec![
        obj(
            "EventCounter",
            "E",
            &[
                ("StartingValue", "0".into()),
                ("Event1Value", "3".into()),
                ("Event1IncToValCmd", "msg A on".into()),
                ("Event1DecToValCmd", "msg A off".into()),
                ("Event2Value", "2".into()),
                ("Event2DecToValCmd", "add down 1".into()),
            ],
        ),
        lamp("A"),
    ]);
    t.run("int down 0");
    let value = |t: &T| match &t.w.objects[0].kind {
        Kind::EventCounter { value, .. } => *value,
        _ => unreachable!(),
    };
    t.send("E", "INC 2");
    assert!(!t.on("A"));
    t.send("E", "INC");
    assert!(t.on("A"), "on at 3");
    t.send("E", "DEC");
    assert_eq!(value(&t), 2);
    assert_eq!(
        t.var("down"),
        Some(1),
        "each single step down runs the event at its new count"
    );
    t.send("E", "DEC 5");
    assert_eq!(value(&t), 0, "never below 0");
}

/// Delays count each frame's time; a delay of 0 runs on the next frame;
/// `abort` cancels by id; 512 slots, then at once.
#[test]
fn delays_slots_and_abort() {
    let mut t = T::new(vec![lamp("A"), lamp("B")]);
    t.run("delay 0 (msg A on)");
    assert!(!t.on("A"), "not at once");
    t.frame();
    assert!(t.on("A"), "the next frame");
    t.run("delayid Wup 1 (msg B on); abort wup");
    t.frames(30);
    assert!(!t.on("B"), "aborted");
    t.run("abort Wup");
    let mut t = T::new(vec![lamp("A")]);
    t.run("int n 0");
    for _ in 0..512 {
        t.run("delay 100 (add n 1)");
    }
    assert_eq!(t.w.cmds.pending_count(), 512);
    t.run("delay 100 (msg A on)");
    assert!(t.on("A"), "a full table runs it at once");
    // Repeats and loops.
    let mut t = T::new(vec![]);
    t.run("int r 0; int l 0; repeat 3 3 0.5 0.5 (add r 1); loopid L 1 1 (add l 1)");
    t.frames(80);
    assert_eq!(t.var("r"), Some(3));
    let l = t.var("l").unwrap();
    assert!((3..=4).contains(&l), "a loop every second: {l}");
    t.run("abort L");
    t.frames(40);
    assert_eq!(t.var("l"), Some(l));
}

/// Case 10 and 11's rules on a breakable built here: 8 + 8 damage kills
/// at 10 HP (≤ 1 HP), the death command runs, it hides (not removed); a
/// dead one takes nothing; `memorize`/`remember` bring it back visible
/// and solid but still dead (Quirk 9); 9 damage on a fresh one leaves 1
/// HP: dead.
#[test]
fn breakable_death_and_reset() {
    let make = || {
        T::new(vec![
            wm(
                "Breakable",
                "Drum",
                Vec3::ZERO,
                Vec3::splat(20.0),
                &[
                    ("HitPoints", "10".into()),
                    ("MaxHitPoints", "10".into()),
                    ("RemoveOnDeath", "0".into()),
                    ("DeathCommand", "msg FX on; msg Boom on".into()),
                ],
            ),
            lamp("FX"),
            (
                {
                    let (mut kv, _) = obj(
                        "Explosion",
                        "Boom",
                        &[("MaxDamage", "5".into()), ("DamageRadius", "200".into())],
                    );
                    kv.push(("origin".into(), v(10.0, 0.0, 0.0)));
                    kv
                },
                None,
            ),
        ])
    };
    let mut t = make();
    let id = t.id("Drum");
    t.w.damage(id, 8.0, Who::Character(1), true);
    let hp = |t: &T| t.w.objects[0].hp.as_ref().unwrap().hp;
    assert_eq!(hp(&t), 2.0);
    t.w.damage(id, 8.0, Who::Character(1), true);
    let d = t.w.objects[0].hp.as_ref().unwrap();
    assert!(d.dead && d.hp == 0.0);
    assert!(t.on("FX"));
    assert!(
        t.w.fx.iter().any(|f| matches!(f, combat_arms::game::objects::runtime::Fx::Explode { damage, radius, .. } if *damage == 5.0 && *radius == 200.0)),
        "the explosion went off"
    );
    let o = &t.w.objects[0];
    assert!(!o.visible && !o.solid && !o.rayhit && !o.removed, "hidden, not removed");
    t.w.damage(id, 50.0, Who::Character(1), true);
    assert_eq!(hp(&t), 0.0);
    // Reset: memorize at the start (alive), remember after it broke.
    let mut t = make();
    t.run("msg Drum memorize");
    t.w.damage(id, 20.0, Who::Character(1), true);
    t.run("msg Drum (visible 1); msg Drum (solid 1); msg Drum remember");
    let o = &t.w.objects[0];
    assert!(o.visible && o.solid && o.rayhit);
    assert!(o.hp.as_ref().unwrap().dead, "still dead: can't break again");
    let mut t = make();
    t.w.damage(id, 9.0, Who::Character(1), true);
    assert!(t.w.objects[0].hp.as_ref().unwrap().dead, "1 HP left is dead");
}

/// A `StartupCommand` runs on the first frame and is gone; a volume takes
/// `on`/`off`.
#[test]
fn startup_command_switches_a_volume() {
    let mut t = T::new(vec![
        obj(
            "StartupCommand",
            "Start",
            &[("Command", "msg Box_Kill01 off; delay 7 (msg Box_Kill01 on)".into())],
        ),
        obj("EndlessFall", "Box_Kill01", &[]),
    ]);
    let vol = t.id("Box_Kill01");
    assert!(t.w.objects[vol as usize].visible);
    t.frame();
    assert!(!t.w.objects[vol as usize].visible, "off at the first frame");
    assert!(t.w.objects[0].removed);
    t.frames(141);
    assert!(t.w.objects[vol as usize].visible, "on 7 s later");
}

/// A keyframer moving an object along two keys in time mode (case 13 on
/// the object world): it starts on `ON`, places its object with its
/// offset, and stops at the end.
#[test]
fn keyframer_moves_its_object() {
    use combat_arms::game::objects::keyframer::KEY_CHUNK_ID;
    let _ = KEY_CHUNK_ID;
    // Two keys: (0,0,0) at 0 s, (100,0,0) 1 s later; no tangents.
    let mut b = Vec::new();
    b.extend(2u32.to_le_bytes());
    for (x, stamp) in [(0.0f32, 0.0f32), (100.0, 1.0)] {
        b.extend(0u16.to_le_bytes());
        b.extend([0u8, 0u8]);
        for f in [x, 0.0, 0.0, 0.0, 0.0, 0.0, stamp, 0.0] {
            b.extend(f.to_le_bytes());
        }
    }
    let hex: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let mut t = T::new(vec![
        obj(
            "KeyFramer",
            "KF",
            &[
                ("ObjectName", "Snd".into()),
                ("BaseKeyName", "K".into()),
                (keys::KEYS, hex),
            ],
        ),
        {
            let (mut kv, _) = obj("SoundFX", "Snd", &[]);
            kv.push(("origin".into(), v(0.0, 10.0, 0.0)));
            (kv, None)
        },
    ]);
    t.frame();
    t.send("KF", "ON");
    t.frames(11);
    let p = t.pos("Snd");
    close(p.x, 50.0, 1e-3, "half-way");
    close(p.y, 10.0, 1e-4, "with its offset");
    t.frames(20);
    close(t.pos("Snd").x, 100.0, 1e-3, "on the last key");
    t.send("KF", "GOTO K0");
    close(t.pos("Snd").x, 0.0, 1e-3, "GOTO snaps to key 0");
}
