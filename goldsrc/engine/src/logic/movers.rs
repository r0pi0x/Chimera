//! Moving brushes (specs/goldsrc/entities.md, "Moving brushes" and the
//! classes after it): the shared rules, then each class.
//!
//! - **The shared rules** (`Motion`): a linear mover's direction from its
//!   angles (`world::movedir`), its closed and open positions (the
//!   brush's size along the direction less 2, less `lip`), a linear or
//!   angular move at a speed that ends when the mover's own clock has
//!   advanced by its time (snapped to the goal, or moved again at 100
//!   units/s when more than 1/32 off), a clock that stands still while
//!   the mover is blocked (so its waits run late), and the push
//!   (`World::push`): players standing on it are carried, players its new
//!   place overlaps are shoved, and a player who would end inside it or
//!   the static world blocks the whole step (its class's blocked rule
//!   runs).
//! - `func_door` and `func_water` (touch opens only an unnamed door; a
//!   use acts when closed, or open with Toggle; arrivals fire `target`,
//!   and `netname` per Starts Open; `wait` −1 stays; blocked: crush
//!   `dmg`, reverse when `wait` ≥ 0, doors of the same name reversed
//!   with it), `func_door_rotating` (axis by flags, `distance`, swings
//!   away from where its activator heads unless One-way);
//! - `func_button` and `func_rot_button` (out, going in, in, coming out;
//!   frame 1 while in: the `+a` set; Toggle, stays pressed, touch and
//!   shootable buttons; the multisources its `target` names used on its
//!   return);
//! - `func_train` with `path_corner` (box centre on each corner, waits,
//!   retrigger, teleport corners, speed changes, the corner's `message`,
//!   stopped and resumed by fires; blocked: `dmg` at most every 0.5 s,
//!   it keeps pushing);
//! - `func_rotating` (full speed at once, or ramped every 0.1 s by
//!   `fanfriction`; Start ON 1.5 s after spawn; blocked: `dmg`) and
//!   `func_pendulum` (a constant acceleration towards the swing's centre,
//!   clamped to `speed`, damped; used: stops or starts);
//! - `func_pushable`, `func_conveyor`: see Known shortcuts and `world`.
//!
//! Known shortcuts:
//! - Pushes test the pushed player against the static world and the
//!   mover itself, not other movers or players; a rider is a player on
//!   the ground whose box a 2-unit drop would sink into the mover; a
//!   rotating mover carries its riders round its axis without turning
//!   their view (open question 9).
//! - `func_pushable` is a breakable (flag 128) that stays where it spawns:
//!   not pushed, pulled, floated or dropped (open question 15).
//! - Button sparks (flag 64), `func_rotating`'s Fan Pain and the sound's
//!   volume and pitch ramps, and `func_pendulum`'s touch damage are not
//!   made; `healthvalue` isn't given; locked and unlocked sentences are
//!   ignored (the spec's "ignore").
//! - The brush size is the hull 0 regions' bounds (the spec names the
//!   models lump's bounds; they agree on the worked examples).

use bevy::math::{Quat, Vec3};

use super::{
    tables,
    world::{Effect, Ent, Id, Kind, Solid, UseType, Who, World, direction_angles, forward, key, movedir, num, sound_key},
};
use crate::{
    core::DamageKind,
    map::{MapEntity, entities::entity_rotation},
};

/// `door_default_speed`, `door_size_shrink`.
pub const DOOR_SPEED: f32 = 100.0;
pub const SIZE_SHRINK: f32 = 2.0;
/// `button_default_speed`, `_wait`, `_lip`.
pub const BUTTON_SPEED: f32 = 40.0;
pub const BUTTON_WAIT: f32 = 1.0;
pub const BUTTON_LIP: f32 = 4.0;
/// `move_snap_tolerance`, `move_resnap_speed`.
pub const SNAP: f32 = 1.0 / 32.0;
pub const RESNAP_SPEED: f32 = 100.0;
/// `door_lock_sound_gap`, `button_lock_sound_gap`.
pub const DOOR_LOCK_GAP: f64 = 3.0;
pub const BUTTON_LOCK_GAP: f64 = 0.5;
/// `rotating_start_delay`, `rotating_ramp_step`.
pub const ROTATING_START: f64 = 1.5;
pub const RAMP_STEP: f64 = 0.1;
/// `train_default_speed`, `_dmg`, `train_block_gap`.
pub const TRAIN_SPEED: f32 = 100.0;
pub const TRAIN_DMG: f32 = 2.0;
pub const TRAIN_BLOCK_GAP: f64 = 0.5;

/// What runs when a mover's clock reaches `Motion::next`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Step {
    #[default]
    None,
    /// Its move ended.
    MoveDone,
    /// Its wait ended (doors close, buttons come out, trains go on).
    WaitDone,
    /// A start or a ramp step (trains placed, rotators, pendulums).
    Think,
}

/// A mover's motion and clock.
#[derive(Clone, Debug, Default)]
pub struct Motion {
    /// Moved by `World::run_mover` (a mover class).
    pub mover: bool,
    /// Its own clock: advances only by the time it moved.
    pub ltime: f64,
    pub velocity: Vec3,
    /// Degrees a second (pitch, yaw, roll).
    pub avelocity: Vec3,
    /// When (on `ltime`) `step` runs.
    pub next: Option<f64>,
    pub step: Step,
    /// Where the move ends (origin, or angles when `turn`).
    pub goal: Vec3,
    pub turn: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DoorState {
    Closed,
    Opening,
    Open,
    Closing,
}

#[derive(Clone, Debug)]
pub struct Door {
    pub rotating: bool,
    /// `func_water`: never solid, silent.
    pub water: bool,
    /// Closed and open origins (linear) or angles (rotating).
    pub pos1: Vec3,
    pub pos2: Vec3,
    pub speed: f32,
    pub wait: f32,
    pub dmg: f32,
    pub state: DoorState,
    pub activator: Option<Who>,
    pub move_sound: Option<String>,
    pub stop_sound: Option<String>,
    pub locked_sound: Option<String>,
    pub unlocked_sound: Option<String>,
    pub lock_sound_at: f64,
    pub touchable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonState {
    Out,
    GoingIn,
    In,
    ComingOut,
    /// Its master turned false while it went in: in for good.
    Stuck,
}

#[derive(Clone, Debug)]
pub struct Button {
    pub rotating: bool,
    pub pos1: Vec3,
    pub pos2: Vec3,
    pub speed: f32,
    pub wait: f32,
    pub state: ButtonState,
    pub activator: Option<Who>,
    pub sound: Option<String>,
    pub locked_sound: Option<String>,
    pub unlocked_sound: Option<String>,
    pub lock_sound_at: f64,
    pub shootable: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrainState {
    /// Not placed yet.
    Spawned,
    Waiting,
    Moving,
    /// Stopped by a fire on its way to `next`.
    Stopped,
}

#[derive(Clone, Debug)]
pub struct Train {
    pub speed: f32,
    pub dmg: f32,
    pub move_sound: Option<String>,
    pub stop_sound: Option<String>,
    pub volume: f32,
    pub state: TrainState,
    /// The corner it is at or came from, and the one it heads for.
    pub at: Option<Id>,
    pub next: Option<Id>,
    pub hurt_at: f64,
}

#[derive(Clone, Debug)]
pub struct Rotating {
    pub speed: f32,
    /// As angle components (yaw (0 1 0), ...).
    pub axis: Vec3,
    /// The ramp step's share of full speed.
    pub friction: f32,
    pub accel: bool,
    pub dmg: f32,
    pub sound: Option<String>,
    /// 1 spinning up, −1 down, 0 neither.
    pub ramp: i8,
}

#[derive(Clone, Debug)]
pub struct Pendulum {
    pub max_speed: f32,
    pub distance: f32,
    pub damp: f32,
    pub dmg: f32,
    pub axis: Vec3,
    pub start: Vec3,
    pub centre: Vec3,
    pub accel: f32,
    /// Its angular speed along the axis now, and the clamp.
    pub speed: f32,
    pub clamp: f32,
    pub swinging: bool,
}

fn sound(index: f32, table: fn(i32) -> Option<String>) -> Option<String> {
    table(index as i32)
}

/// A rotating class's axis by its flags (`x`: the roll flag, `y`: the
/// pitch flag, `reverse`).
fn axis(flags: u32, x: u32, y: u32, reverse: u32) -> Vec3 {
    let a = if flags & x != 0 {
        Vec3::Z
    } else if flags & y != 0 {
        Vec3::X
    } else {
        Vec3::Y
    };
    if reverse != 0 && flags & reverse != 0 { -a } else { a }
}

/// The travel of a linear mover: the brush's size along `dir`, less 2 per
/// axis, less `lip`.
pub fn travel(dir: Vec3, lo: Vec3, hi: Vec3, lip: f32) -> f32 {
    let s = hi - lo;
    dir.x.abs() * (s.x - SIZE_SHRINK) + dir.y.abs() * (s.y - SIZE_SHRINK) + dir.z.abs() * (s.z - SIZE_SHRINK) - lip
}

/// A mover class's state from its keys (None: not a mover).
pub fn spawn(ent: &mut Ent, e: &MapEntity) -> Option<Kind> {
    let flags = ent.flags;
    let class = ent.class.clone();
    let kind = match class.as_str() {
        "func_door" | "func_water" | "func_door_rotating" => {
            let rotating = class == "func_door_rotating";
            let water = class == "func_water";
            let speed = if num(e, "speed") == 0.0 { DOOR_SPEED } else { num(e, "speed") };
            let (mut pos1, mut pos2);
            if rotating {
                let mut ax = axis(flags, 64, 128, 2);
                pos1 = direction_angles(e);
                if key(e, "angle").is_some() && key(e, "movedir").is_none() && key(e, "angles").is_none() {
                    // A rotating door ignores `angle` for its axis.
                    pos1 = Vec3::ZERO;
                }
                pos2 = pos1 + ax * num(e, "distance");
                ent.angles = pos1;
                if flags & 1 != 0 {
                    ent.angles = pos2;
                    std::mem::swap(&mut pos1, &mut pos2);
                    ax = -ax;
                }
                let _ = ax;
            } else {
                let dir = movedir(direction_angles(e));
                pos1 = ent.origin;
                pos2 = pos1 + dir * travel(dir, ent.lo, ent.hi, num(e, "lip"));
                if flags & 1 != 0 {
                    ent.origin = pos2;
                    std::mem::swap(&mut pos1, &mut pos2);
                }
            }
            if water || flags & 8 != 0 {
                ent.solid = false;
            }
            let quiet = |s: Option<String>| if water { None } else { s };
            Kind::Door(Box::new(Door {
                rotating,
                water,
                pos1,
                pos2,
                speed,
                wait: num(e, "wait"),
                dmg: num(e, "dmg"),
                state: DoorState::Closed,
                activator: None,
                move_sound: quiet(sound(num(e, "movesnd"), tables::door_move)),
                stop_sound: quiet(sound(num(e, "stopsnd"), tables::door_stop)),
                locked_sound: quiet(sound(num(e, "locked_sound"), tables::button)),
                unlocked_sound: quiet(sound(num(e, "unlocked_sound"), tables::button)),
                lock_sound_at: f64::MIN,
                touchable: flags & 256 == 0,
            }))
        }
        "func_button" | "func_rot_button" => {
            let rotating = class == "func_rot_button";
            let speed = if num(e, "speed") == 0.0 { BUTTON_SPEED } else { num(e, "speed") };
            let wait = if num(e, "wait") == 0.0 { BUTTON_WAIT } else { num(e, "wait") };
            let (pos1, pos2) = if rotating {
                let a = direction_angles(e);
                ent.angles = a;
                if flags & 1 != 0 {
                    ent.solid = false;
                }
                (a, a + axis(flags, 64, 128, 2) * num(e, "distance"))
            } else {
                let lip = if num(e, "lip") == 0.0 { BUTTON_LIP } else { num(e, "lip") };
                let dir = movedir(direction_angles(e));
                let d = travel(dir, ent.lo, ent.hi, lip);
                let p1 = ent.origin;
                (p1, if d < 1.0 || flags & 1 != 0 { p1 } else { p1 + dir * d })
            };
            Kind::Button(Box::new(Button {
                rotating,
                pos1,
                pos2,
                speed,
                wait,
                state: ButtonState::Out,
                activator: None,
                sound: sound(num(e, "sounds"), tables::button),
                locked_sound: sound(num(e, "locked_sound"), tables::button),
                unlocked_sound: sound(num(e, "unlocked_sound"), tables::button),
                lock_sound_at: f64::MIN,
                shootable: num(e, "health") > 0.0,
            }))
        }
        "func_train" => {
            if flags & 8 != 0 {
                ent.solid = false;
            }
            ent.motion.next = Some(0.1);
            ent.motion.step = Step::Think;
            Kind::Train(Box::new(Train {
                speed: if num(e, "speed") == 0.0 { TRAIN_SPEED } else { num(e, "speed") },
                dmg: if num(e, "dmg") == 0.0 { TRAIN_DMG } else { num(e, "dmg") },
                move_sound: sound(num(e, "movesnd"), tables::plat_move),
                stop_sound: sound(num(e, "stopsnd"), tables::plat_stop),
                volume: if num(e, "volume") == 0.0 { 0.85 } else { num(e, "volume") },
                state: TrainState::Spawned,
                at: None,
                next: None,
                hurt_at: f64::MIN,
            }))
        }
        "path_corner" => {
            ent.solid = false;
            ent.visible = false;
            return Some(Kind::PathCorner { wait: num(e, "wait"), speed: num(e, "speed"), fired: false });
        }
        "func_rotating" => {
            if flags & 64 != 0 {
                ent.solid = false;
            }
            ent.angles = direction_angles(e);
            if flags & 1 != 0 {
                ent.motion.next = Some(ROTATING_START);
                ent.motion.step = Step::Think;
            }
            let friction = num(e, "fanfriction");
            let message = ent.message.trim().to_string();
            Kind::Rotating(Box::new(Rotating {
                speed: num(e, "speed").max(0.0),
                axis: axis(flags, 4, 8, 2),
                friction: if friction <= 0.0 { 1.0 } else { friction.min(100.0) / 100.0 },
                accel: flags & 16 != 0,
                dmg: num(e, "dmg"),
                sound: if message.is_empty() {
                    let s = num(e, "sounds") as i32;
                    (1..=5).contains(&s).then(|| format!("fans/fan{s}.wav"))
                } else {
                    Some(message)
                },
                ramp: 0,
            }))
        }
        "func_pendulum" => {
            if flags & 8 != 0 {
                ent.solid = false;
            }
            let start = direction_angles(e);
            ent.angles = start;
            let ax = axis(flags, 64, 128, 0);
            let max_speed = if num(e, "speed") == 0.0 { 100.0 } else { num(e, "speed") };
            let distance = num(e, "distance");
            if flags & 1 != 0 {
                ent.motion.next = Some(0.1);
                ent.motion.step = Step::WaitDone;
            }
            Kind::Pendulum(Box::new(Pendulum {
                max_speed,
                distance,
                damp: num(e, "damp").clamp(0.0, 1000.0) * 0.001,
                dmg: num(e, "dmg"),
                axis: ax,
                start,
                centre: start + ax * distance / 2.0,
                accel: if distance == 0.0 { 0.0 } else { max_speed * max_speed / (2.0 * distance.abs()) },
                speed: 0.0,
                clamp: max_speed,
                swinging: false,
            }))
        }
        _ => return None,
    };
    ent.motion.mover = true;
    Some(kind)
}

impl World {
    /// Start a linear move of `id` to `goal` at `speed` (at once when
    /// there).
    pub fn linear_move(&mut self, id: Id, goal: Vec3, speed: f32) {
        let e = &mut self.ents[id];
        let m = &mut e.motion;
        m.goal = goal;
        m.turn = false;
        m.avelocity = Vec3::ZERO;
        let delta = goal - e.origin;
        if delta == Vec3::ZERO || speed <= 0.0 {
            m.velocity = Vec3::ZERO;
            m.next = None;
            self.move_done(id);
            return;
        }
        let t = delta.length() / speed;
        m.velocity = delta / t;
        m.next = Some(m.ltime + f64::from(t));
        m.step = Step::MoveDone;
    }

    /// Start an angular move of `id` to angles `goal` at `speed` °/s.
    pub fn angular_move(&mut self, id: Id, goal: Vec3, speed: f32) {
        let e = &mut self.ents[id];
        let m = &mut e.motion;
        m.goal = goal;
        m.turn = true;
        m.velocity = Vec3::ZERO;
        let delta = goal - e.angles;
        if delta == Vec3::ZERO || speed <= 0.0 {
            m.avelocity = Vec3::ZERO;
            m.next = None;
            self.move_done(id);
            return;
        }
        let t = delta.length() / speed;
        m.avelocity = delta / t;
        m.next = Some(m.ltime + f64::from(t));
        m.step = Step::MoveDone;
    }

    /// A move ended: put the mover at its goal (or move on to it when a
    /// push drifted it), then its class's arrival.
    fn move_done(&mut self, id: Id) {
        let e = &mut self.ents[id];
        let goal = e.motion.goal;
        if e.motion.turn {
            e.angles = goal;
        } else {
            if e.origin.distance(goal) > SNAP {
                let speed = RESNAP_SPEED;
                let t = e.origin.distance(goal) / speed;
                e.motion.velocity = (goal - e.origin) / t;
                e.motion.next = Some(e.motion.ltime + f64::from(t));
                e.motion.step = Step::MoveDone;
                return;
            }
            e.origin = goal;
        }
        e.motion.velocity = Vec3::ZERO;
        e.motion.avelocity = Vec3::ZERO;
        e.motion.next = None;
        match &e.kind {
            Kind::Door(d) => match d.state {
                DoorState::Opening => self.door_hit_top(id),
                DoorState::Closing => self.door_hit_bottom(id),
                _ => {}
            },
            Kind::Button(b) => match b.state {
                ButtonState::GoingIn => self.button_in(id),
                ButtonState::ComingOut => self.button_out(id),
                _ => {}
            },
            Kind::Train(_) => self.train_arrived(id),
            _ => {}
        }
    }

    /// One frame of a mover: it moves (unless blocked), its clock
    /// advances by the time it moved, and its step runs when due.
    pub(super) fn run_mover(&mut self, id: Id, dt: f64, solid: &dyn Solid) {
        let old = self.ents[id].motion.ltime;
        let mut movetime = dt;
        if let Some(n) = self.ents[id].motion.next
            && n < old + dt
        {
            movetime = (n - old).max(0.0);
        }
        let m = &self.ents[id].motion;
        if movetime > 0.0 && (m.velocity != Vec3::ZERO || m.avelocity != Vec3::ZERO) {
            match self.push(id, movetime as f32, solid) {
                Ok(()) => self.ents[id].motion.ltime += movetime,
                Err(k) => self.blocked(id, k),
            }
        } else {
            self.ents[id].motion.ltime += movetime;
        }
        let m = &self.ents[id].motion;
        if let Some(n) = m.next
            && n <= m.ltime + 1e-9
            && !self.ents[id].removed
        {
            let step = m.step;
            self.ents[id].motion.next = None;
            match step {
                Step::MoveDone => self.move_done(id),
                Step::WaitDone => self.wait_done(id),
                Step::Think => self.mover_think(id),
                Step::None => {}
            }
        }
    }

    /// Move entity `id` by its velocities for `t` seconds, pushing and
    /// carrying players; Err(player index) when one blocks it (nothing
    /// moved).
    pub fn push(&mut self, id: Id, t: f32, solid: &dyn Solid) -> Result<(), usize> {
        let (o0, a0) = (self.ents[id].origin, self.ents[id].angles);
        let (o1, a1) = (o0 + self.ents[id].motion.velocity * t, a0 + self.ents[id].motion.avelocity * t);
        if !self.ents[id].solid {
            self.ents[id].origin = o1;
            self.ents[id].angles = a1;
            return Ok(());
        }
        let before = self.brushes(id);
        self.ents[id].origin = o1;
        self.ents[id].angles = a1;
        let after = self.brushes(id);
        let turn = entity_rotation(a1) * entity_rotation(a0).inverse();
        let mut moved: Vec<(usize, Vec3)> = Vec::new();
        for k in 0..self.players.len() {
            let p = &self.players[k];
            if !p.alive {
                continue;
            }
            let (c, h) = (p.origin + (p.mins + p.maxs) / 2.0, (p.maxs - p.mins) / 2.0);
            let rider = p.on_ground && before.iter().any(|b| b.overlaps_box(c - Vec3::Z * 2.0, h, 0.01));
            let hit = after.iter().any(|b| b.overlaps_box(c, h, 0.01));
            if !rider && !hit {
                continue;
            }
            let to = o1 + turn * (p.origin - o0);
            let c2 = to + (p.mins + p.maxs) / 2.0;
            let stuck = after.iter().any(|b| b.overlaps_box(c2, h, 0.01))
                || solid.blocked(c2, h - Vec3::splat(1.0));
            if stuck {
                for (j, was) in moved {
                    self.players[j].origin = was;
                }
                self.ents[id].origin = o0;
                self.ents[id].angles = a0;
                return Err(k);
            }
            moved.push((k, p.origin));
            let p = &mut self.players[k];
            p.origin = to;
            p.moved = true;
        }
        Ok(())
    }

    /// A mover blocked by player `k` this frame.
    fn blocked(&mut self, id: Id, k: usize) {
        let player = self.players[k].id;
        let now = self.time;
        let crush = |points: f32| Effect::Damage { player, points, kind: DamageKind::Crush, by: Some(id) };
        match &mut self.ents[id].kind {
            Kind::Door(d) => {
                let (dmg, wait, state) = (d.dmg, d.wait, d.state);
                if dmg != 0.0 {
                    self.effects.push(crush(dmg));
                }
                if wait < 0.0 {
                    return;
                }
                let closing = state == DoorState::Closing;
                self.door_reverse(id, closing);
                // Doors of the same name go with it.
                let name = self.ents[id].targetname.clone();
                if name.is_empty() {
                    return;
                }
                let (vel, avel, origin, angles) = {
                    let e = &self.ents[id];
                    (e.motion.velocity, e.motion.avelocity, e.origin, e.angles)
                };
                for other in self.named(&name) {
                    if other == id {
                        continue;
                    }
                    let Kind::Door(o) = &self.ents[other].kind else { continue };
                    if o.wait < 0.0 {
                        continue;
                    }
                    let closing = o.state == DoorState::Closing;
                    let oe = &mut self.ents[other];
                    if oe.motion.velocity == vel && oe.motion.avelocity == avel {
                        oe.origin = origin;
                        oe.angles = angles;
                        oe.motion.velocity = Vec3::ZERO;
                        oe.motion.avelocity = Vec3::ZERO;
                    }
                    self.door_reverse(other, closing);
                }
            }
            Kind::Train(tr) => {
                if now >= tr.hurt_at + TRAIN_BLOCK_GAP {
                    tr.hurt_at = now;
                    let dmg = tr.dmg;
                    self.effects.push(crush(dmg));
                }
            }
            Kind::Rotating(r) => {
                if r.dmg != 0.0 {
                    let dmg = r.dmg;
                    self.effects.push(crush(dmg));
                }
            }
            Kind::Pendulum(p) => {
                if p.dmg != 0.0 {
                    let dmg = p.dmg;
                    self.effects.push(crush(dmg));
                }
            }
            _ => {}
        }
    }

    fn door_reverse(&mut self, id: Id, closing: bool) {
        if closing { self.door_go_up(id) } else { self.door_go_down(id) }
    }

    /// A step due on a mover's clock after a wait.
    fn wait_done(&mut self, id: Id) {
        match &self.ents[id].kind {
            Kind::Door(_) => self.door_go_down(id),
            Kind::Button(_) => self.button_come_out(id),
            Kind::Train(_) => self.train_next(id),
            Kind::Pendulum(_) => self.pendulum_use(id),
            _ => {}
        }
    }

    /// A mover's start or ramp step.
    fn mover_think(&mut self, id: Id) {
        match &self.ents[id].kind {
            Kind::Train(t) if t.state == TrainState::Spawned => self.train_place(id),
            Kind::Rotating(r) => {
                if r.ramp == 0 {
                    // Start ON.
                    self.rotating_use(id);
                } else {
                    self.rotating_ramp(id);
                }
            }
            Kind::Pendulum(_) => self.pendulum_swing(id),
            _ => {}
        }
    }

    /// A mover class used.
    pub(super) fn mover_use(&mut self, id: Id, activator: Option<Who>, _caller: Option<Id>, _use_type: UseType) {
        match &self.ents[id].kind {
            Kind::Door(_) => self.door_use(id, activator),
            Kind::Button(_) => self.button_press(id, activator),
            Kind::Train(_) => self.train_use(id),
            Kind::Rotating(_) => self.rotating_use(id),
            Kind::Pendulum(_) => self.pendulum_use(id),
            _ => {}
        }
    }

    fn sound_at(&mut self, id: Id, path: Option<String>, volume: f32) {
        if let Some(path) = path {
            let at = self.centre(id);
            self.effects.push(Effect::Sound { path, at, entity: Some(id), volume });
        }
    }

    fn move_loop(&mut self, id: Id, path: Option<String>, volume: f32) {
        let at = self.centre(id);
        self.effects.push(Effect::Loop { key: sound_key(id, 0), path, at, entity: Some(id), volume });
    }

    fn centre(&self, id: Id) -> Vec3 {
        let (lo, hi) = self.bounds(id);
        if lo.x > hi.x { self.ents[id].origin } else { (lo + hi) / 2.0 }
    }

    // ---- func_door, func_door_rotating, func_water ----

    fn door(&self, id: Id) -> &Door {
        match &self.ents[id].kind {
            Kind::Door(d) => d,
            _ => unreachable!("not a door"),
        }
    }

    fn door_mut(&mut self, id: Id) -> &mut Door {
        match &mut self.ents[id].kind {
            Kind::Door(d) => d,
            _ => unreachable!("not a door"),
        }
    }

    fn door_lock_sound(&mut self, id: Id, unlocked: bool) {
        let now = self.time;
        let d = self.door_mut(id);
        let s = if unlocked { d.unlocked_sound.clone() } else { d.locked_sound.clone() };
        if s.is_none() || now < d.lock_sound_at + DOOR_LOCK_GAP {
            return;
        }
        d.lock_sound_at = now;
        self.sound_at(id, s, 1.0);
    }

    pub(super) fn door_touch(&mut self, id: Id, k: usize) {
        let flags = self.ents[id].flags;
        if !self.door(id).touchable || flags & 256 != 0 {
            return;
        }
        if !self.ents[id].targetname.is_empty() || !self.master_ok(&self.ents[id].master) {
            self.door_lock_sound(id, false);
            return;
        }
        let who = Who::Player(self.players[k].id);
        self.door_mut(id).activator = Some(who);
        self.door_activate(id);
    }

    fn door_use(&mut self, id: Id, activator: Option<Who>) {
        let toggle = self.ents[id].flags & 32 != 0;
        let state = self.door(id).state;
        if !(state == DoorState::Closed || (state == DoorState::Open && toggle)) {
            return;
        }
        self.door_mut(id).activator = activator;
        if !self.master_ok(&self.ents[id].master) {
            return;
        }
        self.door_activate(id);
    }

    fn door_activate(&mut self, id: Id) {
        let toggle = self.ents[id].flags & 32 != 0;
        if toggle && self.door(id).state == DoorState::Open {
            self.door_go_down(id);
        } else {
            self.door_lock_sound(id, true);
            self.door_go_up(id);
        }
    }

    fn door_go_up(&mut self, id: Id) {
        let d = self.door(id);
        let moving = matches!(d.state, DoorState::Opening | DoorState::Closing);
        if !moving {
            let s = d.move_sound.clone();
            if s.is_some() {
                self.move_loop(id, s, 1.0);
            }
        }
        let d = self.door_mut(id);
        d.state = DoorState::Opening;
        d.touchable = false;
        let (rotating, pos2, speed, activator) = (d.rotating, d.pos2, d.speed, d.activator);
        if rotating {
            let goal = if self.swings_back(id, activator) { -pos2 } else { pos2 };
            self.angular_move(id, goal, speed);
        } else {
            self.linear_move(id, pos2, speed);
        }
    }

    /// Whether a rotating door opens the other way, away from where its
    /// activator heads (entities.md, "Opening direction").
    pub fn swings_back(&self, id: Id, activator: Option<Who>) -> bool {
        let e = &self.ents[id];
        let Kind::Door(d) = &e.kind else { return false };
        let yaw = (d.pos2 - d.pos1).normalize_or_zero();
        if e.flags & 16 != 0 || yaw.x != 0.0 || yaw.z != 0.0 || yaw.y == 0.0 {
            return false;
        }
        let Some((origin, view)) = activator.and_then(|a| self.activator_pose(a)) else { return false };
        let v = origin - e.origin;
        let w = origin + forward(view) * 10.0 - e.origin;
        v.x * w.y - v.y * w.x < 0.0
    }

    fn door_hit_top(&mut self, id: Id) {
        let flags = self.ents[id].flags;
        let d = self.door_mut(id);
        let stop = d.stop_sound.clone();
        let had_loop = d.move_sound.is_some();
        d.state = DoorState::Open;
        let (wait, activator) = (d.wait, d.activator);
        if had_loop {
            self.move_loop(id, None, 0.0);
        }
        self.sound_at(id, stop, 1.0);
        if flags & 32 != 0 {
            if flags & 256 == 0 {
                self.door_mut(id).touchable = true;
            }
        } else if wait >= 0.0 {
            let m = &mut self.ents[id].motion;
            m.next = Some(m.ltime + f64::from(wait));
            m.step = Step::WaitDone;
        }
        let (netname, delay) = (self.ents[id].netname.clone(), self.ents[id].delay);
        if flags & 1 != 0 {
            self.fire_with(id, &netname, "", delay, activator, UseType::Toggle);
        }
        self.fire_targets(id, activator, UseType::Toggle);
    }

    fn door_go_down(&mut self, id: Id) {
        let d = self.door(id);
        let moving = matches!(d.state, DoorState::Opening | DoorState::Closing);
        if !moving {
            let s = d.move_sound.clone();
            if s.is_some() {
                self.move_loop(id, s, 1.0);
            }
        }
        let d = self.door_mut(id);
        d.state = DoorState::Closing;
        d.touchable = false;
        let (rotating, pos1, speed) = (d.rotating, d.pos1, d.speed);
        if rotating {
            self.angular_move(id, pos1, speed);
        } else {
            self.linear_move(id, pos1, speed);
        }
    }

    fn door_hit_bottom(&mut self, id: Id) {
        let flags = self.ents[id].flags;
        let d = self.door_mut(id);
        let stop = d.stop_sound.clone();
        let had_loop = d.move_sound.is_some();
        d.state = DoorState::Closed;
        d.touchable = flags & 256 == 0;
        let activator = d.activator;
        if had_loop {
            self.move_loop(id, None, 0.0);
        }
        self.sound_at(id, stop, 1.0);
        self.fire_targets(id, activator, UseType::Toggle);
        let (netname, delay) = (self.ents[id].netname.clone(), self.ents[id].delay);
        if flags & 1 == 0 {
            self.fire_with(id, &netname, "", delay, activator, UseType::Toggle);
        }
    }

    // ---- func_button, func_rot_button ----

    fn button_mut(&mut self, id: Id) -> &mut Button {
        match &mut self.ents[id].kind {
            Kind::Button(b) => b,
            _ => unreachable!("not a button"),
        }
    }

    fn button_lock_sound(&mut self, id: Id, unlocked: bool) {
        let now = self.time;
        let b = self.button_mut(id);
        let s = if unlocked { b.unlocked_sound.clone() } else { b.locked_sound.clone() };
        if s.is_none() || now < b.lock_sound_at + BUTTON_LOCK_GAP {
            return;
        }
        b.lock_sound_at = now;
        self.sound_at(id, s, 1.0);
    }

    pub(super) fn button_touch(&mut self, id: Id, k: usize) {
        if self.ents[id].flags & 256 == 0 {
            return;
        }
        let who = Who::Player(self.players[k].id);
        if self.button_mut(id).state == ButtonState::Out {
            self.button_press(id, Some(who));
        }
    }

    /// Any damage presses a shootable button that is out; a Toggle one
    /// that is in comes out and fires at once.
    pub(super) fn button_damaged(&mut self, id: Id) {
        let toggle = self.ents[id].flags & 32 != 0;
        let b = self.button_mut(id);
        if !b.shootable {
            return;
        }
        match b.state {
            ButtonState::Out => self.button_press(id, None),
            ButtonState::In if toggle => {
                let activator = b.activator;
                self.button_come_out(id);
                self.fire_targets(id, activator, UseType::Toggle);
            }
            _ => {}
        }
    }

    fn button_press(&mut self, id: Id, activator: Option<Who>) {
        let toggle = self.ents[id].flags & 32 != 0;
        let b = self.button_mut(id);
        match b.state {
            ButtonState::Out => {}
            ButtonState::In if toggle => {
                let s = b.sound.clone();
                self.sound_at(id, s, 1.0);
                self.button_come_out(id);
                return;
            }
            _ => return,
        }
        let s = b.sound.clone();
        b.activator = activator;
        self.sound_at(id, s, 1.0);
        if !self.master_ok(&self.ents[id].master) {
            self.button_lock_sound(id, false);
            return;
        }
        self.button_lock_sound(id, true);
        let b = self.button_mut(id);
        b.state = ButtonState::GoingIn;
        let (rotating, pos2, speed) = (b.rotating, b.pos2, b.speed);
        if rotating {
            self.angular_move(id, pos2, speed);
        } else {
            self.linear_move(id, pos2, speed);
        }
    }

    fn button_in(&mut self, id: Id) {
        let master = self.ents[id].master.clone();
        if !master.is_empty() && !self.master_ok(&master) {
            self.button_mut(id).state = ButtonState::Stuck;
            return;
        }
        let toggle = self.ents[id].flags & 32 != 0;
        let b = self.button_mut(id);
        b.state = ButtonState::In;
        let (activator, wait) = (b.activator, b.wait);
        self.ents[id].frame = 1;
        self.fire_targets(id, activator, UseType::Toggle);
        if !(toggle || wait < 0.0) {
            let m = &mut self.ents[id].motion;
            m.next = Some(m.ltime + f64::from(wait));
            m.step = Step::WaitDone;
        }
    }

    fn button_come_out(&mut self, id: Id) {
        let b = self.button_mut(id);
        b.state = ButtonState::ComingOut;
        let (rotating, pos1, speed) = (b.rotating, b.pos1, b.speed);
        self.ents[id].frame = 0;
        if rotating {
            self.angular_move(id, pos1, speed);
        } else {
            self.linear_move(id, pos1, speed);
        }
    }

    fn button_out(&mut self, id: Id) {
        let toggle = self.ents[id].flags & 32 != 0;
        let b = self.button_mut(id);
        b.state = ButtonState::Out;
        let activator = b.activator;
        if toggle {
            self.fire_targets(id, activator, UseType::Toggle);
        }
        let target = self.ents[id].target.clone();
        for ms in self.named(&target) {
            if matches!(self.ents[ms].kind, Kind::Multisource(_)) {
                self.use_entity(ms, activator, Some(id), UseType::Toggle);
            }
        }
    }

    // ---- func_train, path_corner ----

    fn train_mut(&mut self, id: Id) -> &mut Train {
        match &mut self.ents[id].kind {
            Kind::Train(t) => t,
            _ => unreachable!("not a train"),
        }
    }

    /// Where the train's origin is when its box centre is on corner `c`.
    fn train_spot(&self, id: Id, c: Id) -> Vec3 {
        let e = &self.ents[id];
        self.ents[c].origin - (e.lo + e.hi) / 2.0
    }

    fn train_place(&mut self, id: Id) {
        let target = self.ents[id].target.clone();
        let first = self.named(&target).into_iter().find(|&c| matches!(self.ents[c].kind, Kind::PathCorner { .. }));
        let Some(first) = first else {
            self.train_mut(id).state = TrainState::Waiting;
            return;
        };
        self.ents[id].origin = self.train_spot(id, first);
        let t = self.train_mut(id);
        t.at = Some(first);
        t.state = TrainState::Waiting;
        if self.ents[id].targetname.is_empty() {
            self.train_next(id);
        }
    }

    fn corner_next(&self, c: Id) -> Option<Id> {
        let target = &self.ents[c].target;
        self.named(target).into_iter().find(|&n| matches!(self.ents[n].kind, Kind::PathCorner { .. }))
    }

    /// Leave the corner it is at for the next one.
    fn train_next(&mut self, id: Id) {
        let Some(at) = self.train_mut(id).at else { return };
        let Some(next) = self.corner_next(at) else {
            self.train_mut(id).state = TrainState::Waiting;
            return;
        };
        if let Kind::PathCorner { speed, .. } = self.ents[at].kind
            && speed != 0.0
        {
            self.train_mut(id).speed = speed;
        }
        self.train_go(id, next);
    }

    fn train_go(&mut self, id: Id, next: Id) {
        let spot = self.train_spot(id, next);
        let t = self.train_mut(id);
        t.next = Some(next);
        t.state = TrainState::Moving;
        let (speed, s, v) = (t.speed, t.move_sound.clone(), t.volume);
        if self.ents[next].flags & 2 != 0 {
            // A teleport corner: jumped to.
            self.ents[id].origin = spot;
            self.ents[id].motion.goal = spot;
            self.ents[id].motion.turn = false;
            self.move_done(id);
            return;
        }
        if s.is_some() {
            self.move_loop(id, s, v);
        }
        self.linear_move(id, spot, speed);
    }

    fn train_arrived(&mut self, id: Id) {
        let t = self.train_mut(id);
        let Some(c) = t.next.take() else { return };
        t.at = Some(c);
        let (stop, volume, has_loop) = (t.stop_sound.clone(), t.volume, t.move_sound.is_some());
        let flags = self.ents[c].flags;
        let (wait, fired) = match &mut self.ents[c].kind {
            Kind::PathCorner { wait, fired, .. } => {
                let f = *fired;
                *fired = true;
                (*wait, f)
            }
            _ => (0.0, false),
        };
        let message = self.ents[c].message.clone();
        if !message.is_empty() && !(flags & 4 != 0 && fired) {
            self.fire(&message, Some(Who::Ent(id)), Some(id), UseType::Toggle);
        }
        let halt = |w: &mut World| {
            if has_loop {
                w.move_loop(id, None, 0.0);
            }
            w.sound_at(id, stop.clone(), volume);
        };
        if flags & 1 != 0 {
            halt(self);
            self.train_mut(id).state = TrainState::Waiting;
        } else if wait != 0.0 {
            halt(self);
            self.train_mut(id).state = TrainState::Waiting;
            if wait > 0.0 {
                let m = &mut self.ents[id].motion;
                m.next = Some(m.ltime + f64::from(wait));
                m.step = Step::WaitDone;
            }
        } else {
            self.train_next(id);
        }
    }

    fn train_use(&mut self, id: Id) {
        let t = self.train_mut(id);
        match t.state {
            TrainState::Waiting => {
                self.ents[id].motion.next = None;
                self.train_next(id);
            }
            TrainState::Moving => {
                t.state = TrainState::Stopped;
                let (stop, volume, has_loop) = (t.stop_sound.clone(), t.volume, t.move_sound.is_some());
                let m = &mut self.ents[id].motion;
                m.velocity = Vec3::ZERO;
                m.next = None;
                if has_loop {
                    self.move_loop(id, None, 0.0);
                }
                self.sound_at(id, stop, volume);
            }
            TrainState::Stopped => {
                if let Some(next) = t.next {
                    self.train_go(id, next);
                }
            }
            TrainState::Spawned => {}
        }
    }

    // ---- func_rotating ----

    fn rotating_mut(&mut self, id: Id) -> &mut Rotating {
        match &mut self.ents[id].kind {
            Kind::Rotating(r) => r,
            _ => unreachable!("not a rotator"),
        }
    }

    fn rotating_use(&mut self, id: Id) {
        let spinning = self.ents[id].motion.avelocity != Vec3::ZERO;
        let r = self.rotating_mut(id);
        let (accel, s, full) = (r.accel, r.sound.clone(), r.axis * r.speed);
        if spinning {
            r.ramp = -1;
            let m = &mut self.ents[id].motion;
            m.next = Some(m.ltime + RAMP_STEP);
            m.step = Step::Think;
        } else if accel {
            r.ramp = 1;
            self.move_loop(id, s, 0.01);
            let m = &mut self.ents[id].motion;
            m.next = Some(m.ltime + RAMP_STEP);
            m.step = Step::Think;
        } else {
            r.ramp = 2;
            self.ents[id].motion.avelocity = full;
            self.move_loop(id, s, 1.0);
        }
    }

    fn rotating_ramp(&mut self, id: Id) {
        let r = self.rotating_mut(id);
        let (axis, speed, step, ramp, s) = (r.axis, r.speed, r.speed * r.friction, r.ramp, r.sound.clone());
        let m = &mut self.ents[id].motion;
        let now = m.avelocity.dot(axis);
        let next = now + step * f32::from(ramp.signum());
        if ramp > 0 && next >= speed {
            m.avelocity = axis * speed;
            self.rotating_mut(id).ramp = 2;
            self.move_loop(id, s, 1.0);
        } else if ramp < 0 && next <= 0.0 {
            m.avelocity = Vec3::ZERO;
            self.rotating_mut(id).ramp = 0;
            self.move_loop(id, None, 0.0);
        } else {
            m.avelocity = axis * next;
            m.next = Some(m.ltime + RAMP_STEP);
            m.step = Step::Think;
        }
    }

    // ---- func_pendulum ----

    fn pendulum_mut(&mut self, id: Id) -> &mut Pendulum {
        match &mut self.ents[id].kind {
            Kind::Pendulum(p) => p,
            _ => unreachable!("not a pendulum"),
        }
    }

    fn pendulum_use(&mut self, id: Id) {
        let auto_return = self.ents[id].flags & 16 != 0;
        let p = self.pendulum_mut(id);
        if p.distance == 0.0 {
            return;
        }
        if p.swinging {
            p.swinging = false;
            p.speed = 0.0;
            let (start, max) = (p.start, p.max_speed);
            self.ents[id].motion.avelocity = Vec3::ZERO;
            self.ents[id].motion.next = None;
            if auto_return {
                self.angular_move(id, start, max);
            }
        } else {
            p.swinging = true;
            p.clamp = p.max_speed;
            let m = &mut self.ents[id].motion;
            m.next = Some(m.ltime + RAMP_STEP);
            m.step = Step::Think;
        }
    }

    fn pendulum_swing(&mut self, id: Id) {
        let angles = self.ents[id].angles;
        let p = self.pendulum_mut(id);
        if !p.swinging {
            return;
        }
        let off = (angles - p.centre).dot(p.axis);
        let dt = RAMP_STEP as f32;
        p.speed -= off.signum() * p.accel * dt;
        if p.damp > 0.0 {
            p.clamp -= p.damp * p.clamp * dt;
        }
        p.speed = p.speed.clamp(-p.clamp, p.clamp);
        let (axis, speed, stop) = (p.axis, p.speed, p.damp > 0.0 && p.clamp < 30.0);
        if stop {
            p.swinging = false;
            let centre = p.centre;
            self.ents[id].angles = centre;
            self.ents[id].motion.avelocity = Vec3::ZERO;
            return;
        }
        let m = &mut self.ents[id].motion;
        m.avelocity = axis * speed;
        m.next = Some(m.ltime + RAMP_STEP);
        m.step = Step::Think;
    }
}

/// A rotation's quaternion in entity space (angles in degrees).
pub fn rotation(angles: Vec3) -> Quat {
    entity_rotation(angles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn travel_matches_the_worked_examples() {
        // cs_office door1: 16 × 304 × 176 up, lip 66: 108.
        let (lo, hi) = (Vec3::new(576.0, -2024.0, -336.0), Vec3::new(592.0, -1720.0, -160.0));
        assert_eq!(travel(Vec3::Z, lo, hi, 66.0), 108.0);
        // as_oilrig's elevator: height 22, lip −485: 505.
        assert_eq!(travel(Vec3::NEG_Z, Vec3::ZERO, Vec3::new(360.0, 344.0, 22.0), -485.0), 505.0);
        // de_nuke's pane: 52 wide, lip −12: 62.
        assert_eq!(travel(Vec3::NEG_Y, Vec3::ZERO, Vec3::new(4.0, 52.0, 92.0), -12.0), 62.0);
    }

    #[test]
    fn axes_by_flags() {
        assert_eq!(axis(0, 64, 128, 2), Vec3::Y);
        assert_eq!(axis(64, 64, 128, 2), Vec3::Z);
        assert_eq!(axis(128 | 2, 64, 128, 2), Vec3::NEG_X);
    }
}
