//! The event objects of world_objects.md Open question 8, as its
//! Follow-up (2026-10-31b) 1 gives them: message receivers with no box,
//! touch or model (`TimerTrigger`, `CaptionTrigger`, `MessageTrigger`,
//! `MissionTrigger`), the hold-to-use spot `LightActivateTrigger`, map
//! cameras and their points (`Camera`, `CameraPoint`), and what a
//! `TargetTrigger` adds to a trigger: its marker (team, game type, on or
//! off; its icons are the client's, `markers`).
//!
//! - `TimerTrigger` (1.1): `START` (ignored while running) fixes the
//!   run-out at `TimeAmount` + 1 s (`TIMER_RUN_OUT`); a timer with
//!   `AutoStop` or a non-zero `InTimeCommandTime` at load updates every
//!   server frame (decided once), running `InTime_Command` once at
//!   `InTimeCommandTime` and stopping itself at the first frame at or
//!   after the run-out. Stopping runs exactly one of `JustTime_Command`
//!   (elapsed = T), `OverTime_Command` (more) and `InTime_Command` (less,
//!   only while `InTimeCommandTime` is exactly 0).
//! - `LightActivateTrigger` (1.2): players working on it, each with a
//!   start time; one working longer than `TimeAmount` completes (the
//!   activation limit reached: it locks itself instead); `WorkCommand`
//!   when the user count leaves 0, `ReleaseCommand` when it returns there.
//! - `CaptionTrigger`, `MessageTrigger` (1.3, 1.4): `PLAY <id> [team]`
//!   shows text id on every client for `Showtime` s.
//! - `MissionTrigger` (1.5): the objective list (`ADD`, `ADDTEAM`,
//!   `ADD_POINT_MISSION`, `UPDATE_POINT_MISSION`, `CHECK`, `FAIL`,
//!   `DELETE`), each change told to every client.
//! - `Camera` (1.7): `ON` (live, for `ActiveTime` s when > 0), `OFF` and
//!   the timed turn-off (its cleanup command, removed when `OneTime`),
//!   `SKIP`, `MOVETO` a `CameraPoint` (a jump), `FOV x y [t]` (clients
//!   blend over t s: `Camera::fov_at`).
//!
//! What the server tells the clients is a `Notice` (`Fx::Notice`), which
//! the bridge writes as `CaObjectNotice` and logs; the cameras' live flag
//! and field of view and the markers' state reach clients with the
//! objects (`net`).
//!
//! Known shortcuts: how a client shows the timer, captions, messages and
//! the objective list isn't in the material (world_objects.md Open
//! question 15): they are notices on the server and lines in its log; the
//! caption, message and objective texts are their ids (the co-op mission
//! file's texts aren't read) and a counted objective's maximum comes from
//! `ObjectWorld::mission_points` (none read yet: 0); the co-op parts (the
//! sub-mission time, the mission manager's registration and successes)
//! aren't there; a `LightActivateTrigger`'s start and end requests are a
//! fresh press of the use key inside its box and letting go or leaving it
//! (the client's request path isn't in the material), its wall clock is
//! the server's clock, and no mode sets a wait-until time; a live camera
//! doesn't move the players' view positions on the server (ours sends
//! every object to every client).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    commands::{Ctx, ObjId, Who},
    movers::{CharBox, Pose},
    runtime::{Fx, Kind, ObjectWorld, Props},
};

/// A timer runs out this long after its `TimeAmount` (s).
pub const TIMER_RUN_OUT: f32 = 1.0;
/// A camera's field of view without `FovX`/`FovY` (degrees).
pub const CAMERA_FOV: Vec2 = Vec2::new(90.0, 78.0);

/// What the server tells every client about an event object.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Notice {
    /// A timer started (its T) or stopped.
    Timer { on: bool, seconds: f32 },
    /// Show caption text `id` for `showtime` s to `team` (0: everyone).
    Caption { id: String, showtime: f32, team: u8 },
    HideCaption,
    /// Show message text `id`.
    Message { id: String, showtime: f32, team: u8 },
    /// An objective's state (added or changed), or its removal.
    Objective(Objective),
    ObjectiveRemoved { id: String },
    /// A use spot's state for the clients (its prompt and gauge):
    /// locked or not.
    UseLocked { locked: bool },
    /// To one player working on a use spot.
    Use { who: Who, answer: UseAnswer },
    /// A camera's field of view, blended over `seconds`.
    Fov { fov: Vec2, seconds: f32 },
}

/// A use spot's answers to a player.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum UseAnswer {
    CanStart,
    Success,
    Locked,
}

/// `TimerTrigger` (1.1).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timer {
    /// T (s).
    pub time: f32,
    pub in_time: String,
    pub over_time: String,
    pub just_time: String,
    pub auto_stop: bool,
    pub in_time_at: f32,
    /// It updates every server frame (decided at load).
    pub updates: bool,
    /// When `START` started it (running).
    pub started: Option<f64>,
    /// Its run-out time, fixed at `START`.
    pub run_out: f64,
    /// `InTime_Command` already ran since `START`.
    pub early_done: bool,
}

impl Timer {
    pub fn of(p: &Props) -> Self {
        let auto_stop = p.bool_or("AutoStop", false);
        let in_time_at = p.real_or("InTimeCommandTime", 0.0);
        Self {
            time: p.real_or("TimeAmount", 15.0),
            in_time: p.string("InTime_Command"),
            over_time: p.string("OverTime_Command"),
            just_time: p.string("JustTime_Command"),
            auto_stop,
            in_time_at,
            updates: auto_stop || in_time_at != 0.0,
            ..default()
        }
    }
}

/// One player on a `LightActivateTrigger`'s list.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UseUser {
    pub who: Who,
    /// When they started working (the server's clock, s).
    pub start: f64,
    pub working: bool,
}

/// `LightActivateTrigger` (1.2).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LightUse {
    /// Half-extents (u).
    pub dims: Vec3,
    pub time: f32,
    /// Completed uses allowed (0: unlimited).
    pub limit: u32,
    pub locked: bool,
    pub work: String,
    pub release: String,
    pub activate: String,
    pub users: Vec<UseUser>,
    /// Players working now.
    pub count: u32,
    /// At the last update.
    pub last_count: u32,
    pub completions: u32,
    /// The last player to complete a use.
    pub last_player: Who,
    /// Who held the use key inside at the last update (a fresh press
    /// starts work).
    pub held: Vec<Who>,
}

impl LightUse {
    pub fn of(p: &Props) -> Self {
        Self {
            dims: p.vector("Dims").map(|d| d.abs()).unwrap_or(super::runtime::TRIGGER_DIMS),
            time: p.real_or("TimeAmount", 10.0),
            limit: p.real_or("NumberOfActivations", 0.0).max(0.0) as u32,
            locked: p.bool_or("Locked", false),
            work: p.string("WorkCommand"),
            release: p.string("ReleaseCommand"),
            activate: p.string("ActivateCommand"),
            ..default()
        }
    }

    fn used_up(&self) -> bool {
        self.limit > 0 && self.completions >= self.limit
    }
}

/// An objective's state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectiveState {
    #[default]
    Normal,
    InProgress,
    Success,
    Failed,
}

/// One objective of a `MissionTrigger`'s list.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Objective {
    pub id: String,
    pub team: u8,
    pub state: ObjectiveState,
    /// A counted objective's count and maximum.
    pub count: Option<(i32, i32)>,
}

/// `Camera` (1.7).
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// s; −1: until turned off.
    pub active_time: f32,
    pub one_time: bool,
    pub can_skip: bool,
    pub cleanup: String,
    pub skip_cleanup_only: bool,
    pub live: bool,
    pub off_at: Option<f64>,
    /// Field of view (degrees), the one it blends from, when the blend
    /// started (the server's clock) and over how long.
    pub fov: Vec2,
    pub fov_from: Vec2,
    pub fov_since: f64,
    pub fov_over: f32,
}

impl Camera {
    pub fn of(p: &Props) -> Self {
        let fov = Vec2::new(p.real_or("FovX", CAMERA_FOV.x), p.real_or("FovY", CAMERA_FOV.y));
        Self {
            active_time: p.real_or("ActiveTime", -1.0),
            one_time: p.bool_or("OneTime", true),
            can_skip: p.bool_or("CanSkip", true),
            cleanup: p.string("CleanupCommand"),
            skip_cleanup_only: p.bool_or("OnSkipCleanupOnly", false),
            live: false,
            off_at: None,
            fov,
            fov_from: fov,
            fov_since: 0.0,
            fov_over: 0.0,
        }
    }

    /// The field of view a client shows at time `t` (s on the same clock
    /// as `fov_since`): linear from the previous values over the blend's
    /// time.
    pub fn fov_at(&self, t: f64) -> Vec2 {
        if self.fov_over <= 0.0 {
            return self.fov;
        }
        let k = (((t - self.fov_since) / self.fov_over as f64) as f32).clamp(0.0, 1.0);
        self.fov_from.lerp(self.fov, k)
    }
}

/// Whose a `TargetTrigger`'s marker is (1.6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkerTeam {
    /// `CheckTeam` false.
    All,
    /// `CheckTeam` true and `<none>`.
    Nobody,
    /// `CheckTeam` true and a team name (`JSF`, `Creed`).
    Team(String),
}

/// A `TargetTrigger`'s marker.
#[derive(Clone, Debug, PartialEq)]
pub struct Marker {
    pub team: MarkerTeam,
    /// The mode it exists in for clients (`GameType`).
    pub game_type: String,
    pub on: bool,
}

impl Marker {
    pub fn of(p: &Props) -> Self {
        let team = p.string("Team");
        let team = if !p.bool_or("CheckTeam", true) {
            MarkerTeam::All
        } else if team.is_empty() || team.eq_ignore_ascii_case("<none>") {
            MarkerTeam::Nobody
        } else {
            MarkerTeam::Team(team)
        };
        Self {
            team,
            game_type: p.string("GameType"),
            on: false,
        }
    }

    /// Whether clients hear of it while mode family `family` (a mode's
    /// `GameType` name) is playing.
    pub fn exists_in(&self, family: Option<&str>) -> bool {
        family.is_some_and(|f| f.eq_ignore_ascii_case(&self.game_type))
    }
}

impl ObjectWorld {
    pub(super) fn notice(&mut self, id: ObjId, n: Notice) {
        self.fx.push(Fx::Notice { id, notice: n });
    }

    /// The event objects' messages (true: handled).
    pub(super) fn event_message(&mut self, id: ObjId, kind: &mut Kind, word: &str, args: &[String]) -> bool {
        let now = self.now;
        let number = |i: usize| args.get(i).and_then(|a| a.parse::<f32>().ok());
        match kind {
            Kind::Timer(t) => {
                match word {
                    "START" if t.started.is_none() => {
                        t.started = Some(now);
                        t.run_out = now + (t.time + TIMER_RUN_OUT) as f64;
                        t.early_done = false;
                        let seconds = t.time;
                        self.notice(id, Notice::Timer { on: true, seconds });
                    }
                    "STOP" => {
                        let run = timer_stop(t, now);
                        if let Some(run) = run {
                            self.timer_stopped(id, run);
                        }
                    }
                    "TIMEAMOUNT" => {
                        if let Some(x) = number(0) {
                            t.time = x;
                        }
                    }
                    "INTIMECOMMANDTIME" => {
                        if let Some(x) = number(0) {
                            t.in_time_at = x;
                        }
                    }
                    _ => {}
                }
                true
            }
            Kind::LightUse(u) => {
                match word {
                    "LOCK" | "UNLOCK" => {
                        u.locked = word == "LOCK";
                        let locked = u.locked;
                        self.notice(id, Notice::UseLocked { locked });
                    }
                    _ => {}
                }
                true
            }
            Kind::Caption { showtime, locked } => {
                match word {
                    "PLAY" if !*locked => {
                        if let Some(text) = args.first() {
                            let team = number(1).map_or(0, |t| t as u8);
                            let n = Notice::Caption {
                                id: text.clone(),
                                showtime: *showtime,
                                team,
                            };
                            self.notice(id, n);
                        }
                    }
                    "CLEAR" => self.notice(id, Notice::HideCaption),
                    "LOCK" => *locked = true,
                    "UNLOCK" => *locked = false,
                    _ => {}
                }
                true
            }
            Kind::MessageTrigger { showtime } => {
                if word == "PLAY"
                    && let Some(text) = args.first()
                {
                    let team = number(1).map_or(0, |t| t as u8);
                    let n = Notice::Message {
                        id: text.clone(),
                        showtime: *showtime,
                        team,
                    };
                    self.notice(id, n);
                }
                true
            }
            Kind::Mission(list) => {
                let key = args.first().cloned().unwrap_or_default();
                let at = list.iter().position(|o| o.id == key);
                let changed: Option<Notice> = match (word, at) {
                    ("ADD" | "ADDTEAM", None) if !key.is_empty() => {
                        let team = if word == "ADDTEAM" { number(1).map_or(0, |t| t as u8) } else { 0 };
                        list.push(Objective {
                            id: key.clone(),
                            team,
                            ..default()
                        });
                        list.last().cloned().map(Notice::Objective)
                    }
                    ("ADD_POINT_MISSION", None) if !key.is_empty() => {
                        // The maximum is the mission data's (the message's
                        // own number is ignored).
                        let max = self.mission_points.get(&key).copied().unwrap_or(0);
                        list.push(Objective {
                            id: key.clone(),
                            team: 0,
                            state: ObjectiveState::InProgress,
                            count: Some((0, max)),
                        });
                        list.last().cloned().map(Notice::Objective)
                    }
                    ("UPDATE_POINT_MISSION", Some(i)) => {
                        let n = number(1).map_or(0, |n| n as i32);
                        let o = &mut list[i];
                        let max = o.count.map_or(0, |c| c.1);
                        o.count = Some((n, max));
                        o.state = ObjectiveState::InProgress;
                        Some(Notice::Objective(o.clone()))
                    }
                    ("CHECK" | "FAIL", Some(i)) => {
                        list[i].state = if word == "CHECK" { ObjectiveState::Success } else { ObjectiveState::Failed };
                        Some(Notice::Objective(list[i].clone()))
                    }
                    ("DELETE", Some(i)) => {
                        list.remove(i);
                        Some(Notice::ObjectiveRemoved { id: key })
                    }
                    _ => None,
                };
                if let Some(n) = changed {
                    self.notice(id, n);
                }
                true
            }
            Kind::Camera(c) => {
                match word {
                    "ON" => {
                        c.live = true;
                        c.off_at = (c.active_time > 0.0).then(|| now + c.active_time as f64);
                        self.objects[id as usize].dirty = true;
                    }
                    "OFF" => {
                        self.camera_off(id, c, false);
                    }
                    "SKIP" if c.can_skip && c.live => {
                        self.camera_off(id, c, c.skip_cleanup_only);
                    }
                    "MOVETO" => {
                        let point = args.first().and_then(|n| self.named_all(n).first().copied());
                        if let Some(p) = point
                            && self.objects[p as usize].class == "CameraPoint"
                        {
                            let to = self.objects[p as usize].pose;
                            let half = self.objects[id as usize].pose.half;
                            self.set_pose(
                                id,
                                Pose {
                                    pos: to.pos,
                                    rot: to.rot,
                                    half,
                                },
                            );
                        }
                    }
                    "FOV" => {
                        if let (Some(x), Some(y)) = (number(0), number(1)) {
                            c.fov_from = c.fov_at(now);
                            c.fov = Vec2::new(x, y);
                            c.fov_since = now;
                            c.fov_over = number(2).unwrap_or(0.0).max(0.0);
                            let (fov, seconds) = (c.fov, c.fov_over);
                            self.objects[id as usize].dirty = true;
                            self.notice(id, Notice::Fov { fov, seconds });
                        }
                    }
                    _ => {}
                }
                true
            }
            _ => false,
        }
    }

    /// A camera turned off (`OFF`, the timed turn-off, `SKIP`): its
    /// cleanup command (even when it wasn't live); unless `cleanup_only`,
    /// not live and removed when `OneTime`.
    fn camera_off(&mut self, id: ObjId, c: &mut Camera, cleanup_only: bool) {
        if !c.cleanup.is_empty() {
            let cmd = c.cleanup.clone();
            self.run(&cmd, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
        }
        if cleanup_only {
            return;
        }
        c.live = false;
        c.off_at = None;
        self.objects[id as usize].dirty = true;
        if c.one_time {
            self.remove(id);
        }
    }

    /// A timer's stop: which command runs (if any), the clock off and the
    /// clients told.
    fn timer_stopped(&mut self, id: ObjId, run: String) {
        self.notice(id, Notice::Timer { on: false, seconds: 0.0 });
        if !run.is_empty() {
            self.run(&run, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
        }
    }

    /// The event objects' updates (true: handled).
    pub(super) fn event_update(&mut self, id: ObjId, kind: &mut Kind) -> bool {
        let now = self.now;
        match kind {
            Kind::Timer(t) => {
                if !t.updates {
                    return true;
                }
                let Some(start) = t.started else { return true };
                let e = now - start;
                if t.in_time_at > 0.0 && !t.early_done && e + 1e-9 >= t.in_time_at as f64 {
                    t.early_done = true;
                    let c = t.in_time.clone();
                    if !c.is_empty() {
                        self.run(&c, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
                    }
                }
                if t.auto_stop
                    && t.started.is_some()
                    && now + 1e-9 >= t.run_out
                    && let Some(run) = timer_stop(t, now)
                {
                    self.timer_stopped(id, run);
                }
                true
            }
            Kind::LightUse(u) => {
                self.light_use_update(id, u);
                true
            }
            Kind::Camera(c) => {
                if c.live && c.off_at.is_some_and(|t| now + 1e-9 >= t) {
                    self.camera_off(id, c, false);
                }
                true
            }
            _ => false,
        }
    }

    /// One update of a `LightActivateTrigger` (1.2).
    fn light_use_update(&mut self, id: ObjId, u: &mut LightUse) {
        let now = self.now;
        let pos = self.objects[id as usize].pose.pos;
        let dims = u.dims;
        let inside = |c: &CharBox| c.min.cmplt(pos + dims).all() && c.max.cmpgt(pos - dims).all();
        // The requests (ours: a fresh press of use inside starts, letting go
        // or leaving ends).
        let holding: Vec<Who> =
            self.chars.iter().filter(|c| c.alive && c.player && c.using && inside(c)).map(|c| c.who).collect();
        for who in holding.iter().copied() {
            if u.held.contains(&who) {
                continue;
            }
            match u.users.iter_mut().find(|x| x.who == who) {
                Some(x) if x.working => {}
                Some(x) => {
                    x.working = true;
                    x.start = now;
                    u.count += 1;
                    self.notice(id, Notice::Use { who, answer: UseAnswer::CanStart });
                }
                None => {
                    u.users.push(UseUser { who, start: now, working: true });
                    u.count += 1;
                    self.notice(id, Notice::Use { who, answer: UseAnswer::CanStart });
                }
            }
        }
        for x in u.users.iter_mut() {
            if x.working && !holding.contains(&x.who) && self.chars.iter().any(|c| c.who == x.who && c.alive) {
                x.working = false;
                u.count -= 1;
            }
        }
        u.held = holding;
        // The per-player pass.
        let mut i = 0;
        let mut runs: Vec<(String, Who)> = Vec::new();
        while i < u.users.len() {
            let x = u.users[i];
            let Some(c) = self.chars.iter().find(|c| c.who == x.who) else {
                // Disconnected.
                if x.working {
                    u.count -= 1;
                }
                u.users.remove(i);
                continue;
            };
            i += 1;
            if !x.working {
                continue;
            }
            if !c.alive {
                u.users[i - 1].working = false;
                u.count -= 1;
                continue;
            }
            if u.locked {
                u.users[i - 1].working = false;
                u.count -= 1;
                self.notice(id, Notice::Use { who: x.who, answer: UseAnswer::Locked });
                continue;
            }
            if now - x.start > u.time as f64 + 1e-9 {
                // Completes.
                u.users[i - 1].working = false;
                u.count -= 1;
                if u.used_up() {
                    u.locked = true;
                    self.notice(id, Notice::UseLocked { locked: true });
                    continue;
                }
                u.completions += 1;
                u.last_player = x.who;
                self.active_player = x.who;
                if !u.activate.is_empty() {
                    runs.push((u.activate.clone(), x.who));
                    self.notice(id, Notice::Use { who: x.who, answer: UseAnswer::Success });
                }
            }
        }
        for (c, _) in runs {
            self.run(&c, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
        }
        // Work and release.
        if u.count != u.last_count {
            if u.last_count == 0 && u.count > 0 {
                if !u.locked && !u.used_up() && !u.work.is_empty() {
                    let c = u.work.clone();
                    self.run(&c, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
                }
            } else if u.count == 0 && !u.release.is_empty() {
                let c = u.release.clone();
                self.run(&c, Ctx { sender: Who::Object(id), target: Some(id), ..Ctx::default() });
            }
            u.last_count = u.count;
        }
    }
}

/// A running timer stops at `now`: the command its elapsed time picks
/// (None: it wasn't running). The clock is turned off.
fn timer_stop(t: &mut Timer, now: f64) -> Option<String> {
    let start = t.started.take()?;
    let e = now - start;
    let time = t.time as f64;
    let run = if (e - time).abs() < 1e-9 {
        t.just_time.clone()
    } else if e > time {
        t.over_time.clone()
    } else if t.in_time_at == 0.0 {
        t.in_time.clone()
    } else {
        String::new()
    };
    Some(run)
}
