//! TF2's player animation state (specs/tf2/player_animation.md 1a-6, 8)
//! on Source's shared half (`source_engine::player_anim`, as HL2:DM's):
//! one main sequence from the movement state (stand, crouch, run,
//! crouch-walk, swim, airwalk, a jump's start played once then its float)
//! asked kind-less and translated by the held gun's kind table
//! (`translate`, 2: the suffix, a reload's before its `_LOOP` / `_END`,
//! the deployed activities only in the primary table; `Kind::of_class`:
//! `PRIMARY`, `SECONDARY` or `MELEE`, the Shotgun the Engineer's primary
//! and the Soldier's, Pyro's and Heavy's secondary; the class
//! `Tf2PlayerAnim::class`, kept by `follow_class` from the rung's drawn
//! class); the sequence drawn among those of the activity by |weight|
//! (a negative weight keeps the current one), changed only when the
//! activity does (1a, 3 step 6). The Heavy's Minigun winding up or spun
//! (`follow_spin`, the gun's networked spin; never a Soldier) plays the
//! deployed set (stand `ACT_MP_DEPLOYED_IDLE`, moving `ACT_MP_DEPLOYED`,
//! crouched idle and swim; a crouch-walk the plain one; no jump or
//! airwalk), the PREFIRE gesture as it starts and POSTFIRE as it stops,
//! and a shot doesn't restart a fire gesture still playing (6); the spin
//! layers are full-body poses the file's autolayers ramp in on a spline
//! (`map::anim`). Ours (open question 13): a holder whose model lacks the
//! deployed set (a Scout spinning the Minigun) keeps his kind's normal
//! one, a kind it lacks its primary one (the game shows the reference
//! pose). The 3 x 3 move grid by `move_x` / `move_y` (the velocity's
//! direction against the eyes pushed out to the unit square, scaled by
//! the speed over the grid's ground speed there, never above 1; playback
//! rate 1, 3); aim by `body_pitch` (minus the eye pitch, -45 to 90) and
//! `body_yaw` (minus the eyes against the feet, clamped to 45; standing,
//! the feet's goal steps 45 degrees toward the eyes once they are more
//! than 45 off it, moving it is the eyes; the feet turn at 720
//! degrees/s, slower in the last 60), each parameter by name, the main
//! sequences' own aim-matrix autolayers adding the pose with the feet
//! kept where the main sequence put them (`Animator::ik_locks`, 4a);
//! `r_hand_grip` and `r_arm` at 0 (the constants; the engine's default
//! for a range from 0 is its middle, a special item's pose); fire,
//! reload (start, a loop per round, end), the spin and landing gestures
//! by the stance, one per slot (attack, jump) in slot order (5). Each
//! side runs it on its own values (`PlayerAnimPlugin` drawn,
//! `SimAnimPlugin` for a server's hitboxes) and a client poses others as
//! the server's tick pose had them (`source_engine::anim_net`).
//! `hold_weapons` puts the gun's `c_` model in their hands (bone-merged
//! on `weapon_bone`).
//!
//! Known shortcuts: the left hand isn't pulled onto the Minigun by the
//! aim cells' IK rule (4a; blends of aim cells may drift it a little);
//! the scoped Sniper's deployed set and its 2 s hold aren't played; the
//! main-activity rules are the earlier reading (airwalk whenever airborne
//! without a jump, not above 300 units/s up; the jump's start until its
//! end, not 0.5 s; no restarts on landing or entering water: 3); no
//! flinch on damage (open question 7) and no air-jump sequence (open
//! question 4: `jump_double_primary` has no activity).

use bevy::prelude::*;
use source_engine::player_anim::{Inputs, PlayerAnimRules, normalize};

use crate::{
    map::anim::{AnimSet, Animator, Layer},
    weapon::Inventory,
};

/// Horizontal speed above which a body counts as moving (units/s).
pub const MOVING_SPEED: f32 = 0.5;
/// Feet turn rate (degrees/s) and the angle below which it slows.
pub const FEET_TURN_RATE: f32 = 720.0;
pub const FEET_FADE: f32 = 60.0;
/// Standing: a torso twist above this turns the feet to the eyes.
pub const FEET_MAX_TORSO: f32 = 45.0;
/// `body_yaw`'s and `body_pitch`'s ranges (4).
pub const BODY_YAW: f32 = 45.0;
pub const PITCH_UP: f32 = 90.0;
pub const PITCH_DOWN: f32 = -45.0;
/// Overlay layers a body may have.
pub const MAX_GESTURES: usize = 8;

/// A weapon's kind (2): the suffix of its activities.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Melee,
}

impl Kind {
    /// The kind of gun `id` for its own class (2: no stock gun has an
    /// `anim_slot`, so it is the item's slot): the secondaries (the
    /// Pistol, SMG, Revolver, Stickybomb Launcher, Medi Gun) and melee
    /// weapons by name, every other gun primary (another game's gun in a
    /// TF2 body, open question 1: primary).
    pub fn of(id: &str) -> Kind {
        let name = id.rsplit(':').next().unwrap_or(id).trim_start_matches("tf_weapon_");
        match name {
            "pistol" | "pistol_scout" | "smg" | "revolver" | "pipebomblauncher" | "medigun" | "shotgun_soldier" | "shotgun_hwg"
            | "shotgun_pyro" => Kind::Secondary,
            "bat" | "shovel" | "fireaxe" | "bottle" | "fists" | "wrench" | "bonesaw" | "club" | "knife" => Kind::Melee,
            _ => Kind::Primary,
        }
    }

    /// The kind of gun `id` in `class`'s hands (None: its own class's,
    /// `of`): the Shotgun is a secondary but for the Engineer.
    pub fn of_class(id: &str, class: Option<&str>) -> Kind {
        let name = id.rsplit(':').next().unwrap_or(id).trim_start_matches("tf_weapon_");
        match (name, class) {
            ("shotgun_primary", Some(c)) if c != "engineer" => Kind::Secondary,
            _ => Kind::of(id),
        }
    }

    pub fn suffix(self) -> &'static str {
        match self {
            Kind::Primary => "PRIMARY",
            Kind::Secondary => "SECONDARY",
            Kind::Melee => "MELEE",
        }
    }
}

/// The main sequence's movement state (3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Stand,
    Crouch,
    Run,
    CrouchWalk,
    Swim,
    Airwalk,
    JumpStart,
    JumpFloat,
}

impl State {
    fn activity(self) -> &'static str {
        match self {
            State::Stand => "ACT_MP_STAND",
            State::Crouch => "ACT_MP_CROUCH",
            State::Run => "ACT_MP_RUN",
            State::CrouchWalk => "ACT_MP_CROUCHWALK",
            State::Swim => "ACT_MP_SWIM",
            State::Airwalk => "ACT_MP_AIRWALK",
            State::JumpStart => "ACT_MP_JUMP_START",
            State::JumpFloat => "ACT_MP_JUMP_FLOAT",
        }
    }
}

/// A gesture slot (5): one gesture each, layered in this order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Slot {
    /// Fire, reload, the Minigun's pre- and post-fire.
    Attack,
    /// Land, air jump.
    Jump,
    Flinch,
}

/// An overlay layer (a gesture, 5).
#[derive(Clone, Debug, PartialEq)]
pub struct Gesture {
    pub sequence: usize,
    pub cycle: f32,
    pub weight: f32,
    /// A reload's loop: restarted per round, kept until the end.
    pub held: bool,
    pub slot: Slot,
}

/// The activity `asked` (kind-less, `ACT_MP_...`) in the table of kind
/// `k` (2, "One rule for every kind"): the movement states, fire and
/// reload gestures and the flinch take the kind's suffix (a reload's
/// before its `_LOOP` / `_END`); the deployed activities only in the
/// primary table (`ACT_MP_DEPLOYED` -> `..._PRIMARY`, the swim one too;
/// the idles stay kind-less); anything else is looked up as asked.
pub fn translate(asked: &str, k: Kind) -> String {
    const MOVES: [&str; 12] = [
        "ACT_MP_STAND",
        "ACT_MP_CROUCH",
        "ACT_MP_RUN",
        "ACT_MP_WALK",
        "ACT_MP_AIRWALK",
        "ACT_MP_CROUCHWALK",
        "ACT_MP_JUMP",
        "ACT_MP_JUMP_START",
        "ACT_MP_JUMP_FLOAT",
        "ACT_MP_JUMP_LAND",
        "ACT_MP_SWIM",
        "ACT_MP_DOUBLEJUMP",
    ];
    const GESTURES: [&str; 5] = ["ACT_MP_ATTACK_STAND", "ACT_MP_ATTACK_CROUCH", "ACT_MP_ATTACK_SWIM", "ACT_MP_ATTACK_AIRWALK", "ACT_MP_GESTURE_FLINCH_CHEST"];
    let suffix = k.suffix();
    match asked {
        "ACT_MP_DEPLOYED" if k == Kind::Primary => return "ACT_MP_DEPLOYED_PRIMARY".into(),
        "ACT_MP_SWIM_DEPLOYED" if k == Kind::Primary => return "ACT_MP_SWIM_DEPLOYED_PRIMARY".into(),
        a if MOVES.contains(&a) || GESTURES.contains(&a) => return format!("{a}_{suffix}"),
        _ => {}
    }
    for stance in ["STAND", "CROUCH", "SWIM", "AIRWALK"] {
        let base = format!("ACT_MP_RELOAD_{stance}");
        if asked == base {
            return format!("{base}_{suffix}");
        }
        for part in ["_LOOP", "_END"] {
            if asked.strip_prefix(base.as_str()) == Some(part) {
                return format!("{base}_{suffix}{part}");
            }
        }
    }
    asked.to_string()
}

/// One TF2 body's animation state.
#[derive(Component, Debug, Clone)]
pub struct Tf2PlayerAnim {
    reset: bool,
    /// The feet yaw and its goal (degrees, Source's axes).
    pub feet_yaw: f32,
    goal_yaw: f32,
    /// The render and hitbox yaw: the feet before this update's turn.
    pub render_yaw: f32,
    pub state: Option<State>,
    /// In the air after a jump (its start, then its float).
    jumping: bool,
    airborne: bool,
    weapon: Option<String>,
    kind: Option<Kind>,
    /// The holder's class (`follow_class`; None: the gun's own).
    pub class: Option<String>,
    pub gestures: Vec<Gesture>,
    /// The last `move_x`, `move_y`, `body_yaw`, `body_pitch` set.
    pub pose: [f32; 4],
    /// The held Minigun winding up or spun (`follow_spin`, 6): the
    /// "aiming" state's deployed main sequences.
    pub spun: bool,
    /// `spun` as the main sequence and the spin gestures last saw it.
    was_spun: bool,
    /// The weighted picks' random state (1a).
    seed: u32,
}

impl Default for Tf2PlayerAnim {
    fn default() -> Self {
        Self {
            reset: true,
            feet_yaw: 0.0,
            goal_yaw: 0.0,
            render_yaw: 0.0,
            state: None,
            jumping: false,
            airborne: false,
            weapon: None,
            kind: None,
            class: None,
            gestures: Vec::new(),
            pose: [0.0; 4],
            spun: false,
            was_spun: false,
            seed: 0x9e37_79b9,
        }
    }
}

/// `(cos d, -sin d)` pushed out to the unit square (3).
pub fn square(d_degrees: f32) -> Vec2 {
    let d = d_degrees.to_radians();
    let v = Vec2::new(d.cos(), -d.sin());
    let m = v.x.abs().max(v.y.abs());
    if m > 1e-6 { v / m } else { Vec2::ZERO }
}

impl Tf2PlayerAnim {
    /// The feet toward the eyes (4) for eye yaw `eye`, moving or not;
    /// returns `body_yaw` (minus the eyes against the feet, clamped).
    pub fn turn(&mut self, eye: f32, moving: bool, dt: f32) -> f32 {
        self.render_yaw = self.feet_yaw;
        if self.reset {
            self.feet_yaw = eye;
            self.goal_yaw = eye;
            self.render_yaw = eye;
        }
        if moving {
            self.goal_yaw = eye;
        } else if normalize(self.goal_yaw - eye).abs() > FEET_MAX_TORSO {
            // Standing: the goal steps 45 degrees toward the eyes.
            self.goal_yaw = normalize(self.goal_yaw + FEET_MAX_TORSO * normalize(eye - self.goal_yaw).signum());
        }
        if self.feet_yaw != self.goal_yaw {
            // The slow-down on the unwrapped gap (Quirks).
            let k = ((self.goal_yaw - self.feet_yaw).abs() / FEET_FADE).clamp(0.01, 1.0);
            let step = FEET_TURN_RATE * dt * k;
            let diff = normalize(self.goal_yaw - self.feet_yaw);
            self.feet_yaw = if diff.abs() <= step { self.goal_yaw } else { normalize(self.feet_yaw + step * diff.signum()) };
        }
        (-normalize(eye - self.feet_yaw)).clamp(-BODY_YAW, BODY_YAW)
    }

    /// The next roll in [0, 1) for a weighted pick (1a).
    fn roll(&mut self) -> f32 {
        // xorshift32
        let mut x = self.seed;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.seed = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// Whether the holder is in the "aiming" state (2): his Minigun spins,
    /// and he is not a Soldier.
    fn aiming(&self) -> bool {
        self.spun && self.class.as_deref() != Some("soldier")
    }

    /// The main sequence's state from the inputs (3, "Choosing").
    fn pick(&mut self, i: &Inputs, a: &Animator) -> State {
        let moving = i.velocity.length() > MOVING_SPEED;
        // Spun: no jump or airwalk (6).
        let spun = self.aiming();
        if i.jumped && !spun {
            self.jumping = true;
            return State::JumpStart;
        }
        if i.swimming {
            self.jumping = false;
            return State::Swim;
        }
        if !i.on_ground && !spun {
            if self.jumping {
                // The start plays once, then the float.
                let done = self.state == Some(State::JumpStart) && a.cycle >= 0.99;
                return if self.state == Some(State::JumpFloat) || done { State::JumpFloat } else { State::JumpStart };
            }
            return State::Airwalk;
        }
        self.jumping = false;
        match (i.ducked, moving) {
            (true, true) => State::CrouchWalk,
            (true, false) => State::Crouch,
            (false, true) => State::Run,
            (false, false) => State::Stand,
        }
    }

    /// The kind-less main activity for `state`, spun or not (2, 6): spun,
    /// the deployed ones (a crouch-walk keeps the plain one).
    fn main_activity(state: State, spun: bool) -> &'static str {
        match (spun, state) {
            (true, State::Stand | State::JumpStart | State::JumpFloat | State::Airwalk) => "ACT_MP_DEPLOYED_IDLE",
            (true, State::Run) => "ACT_MP_DEPLOYED",
            (true, State::Swim) => "ACT_MP_SWIM_DEPLOYED",
            (true, State::Crouch) => "ACT_MP_CROUCH_DEPLOYED_IDLE",
            _ => state.activity(),
        }
    }

    /// The main sequence for `state` with kind `kind` (2, 1a): the
    /// translated activity; ours (open question 13) where the holder's
    /// model lacks it: spun, the kind's normal set; a kind the model
    /// lacks, its primary one; else the reference pose.
    fn main_sequence(&mut self, set: &AnimSet, state: State, kind: Kind, current: Option<usize>) -> (String, usize) {
        let spun = self.aiming();
        let mut asked = vec![translate(Self::main_activity(state, spun), kind)];
        if spun {
            asked.push(translate(state.activity(), kind));
        }
        asked.push(translate(state.activity(), Kind::Primary));
        for act in asked {
            if let Some(s) = set.select_activity(&act, current, self.roll()) {
                return (act, s);
            }
        }
        (String::new(), 0)
    }

    /// Start a gesture of `activity` (kind-less, translated by `kind`) in
    /// `slot` (5): one per slot; the same activity again restarts it,
    /// another replaces it; nothing if the model has none. `keep`: not
    /// restarted while it plays (the Minigun's fire gesture, 6).
    #[allow(clippy::too_many_arguments)]
    fn gesture(&mut self, set: &AnimSet, activity: &str, kind: Kind, slot: Slot, held: bool, keep: bool) {
        let act = translate(activity, kind);
        let current = self.gestures.iter().find(|g| g.slot == slot).map(|g| g.sequence);
        let same = current.filter(|&c| set.sequences.get(c).is_some_and(|s| s.activity.eq_ignore_ascii_case(&act)));
        if keep && same.is_some() {
            return;
        }
        let roll = self.roll();
        let Some(s) = same.or_else(|| set.select_activity(&act, current, roll)) else { return };
        self.gestures.retain(|g| g.slot != slot);
        self.gestures.push(Gesture { sequence: s, cycle: 0.0, weight: 1.0, held, slot });
        self.gestures.sort_by_key(|g| g.slot);
    }

    /// One update with the gun in hand `weapon`.
    pub fn step(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        let set = a.set.clone();
        a.ik_locks = true;
        let kind = weapon.map_or(Kind::Primary, |w| Kind::of_class(w, self.class.as_deref()));
        if self.reset || weapon != self.weapon.as_deref() || self.kind != Some(kind) {
            if self.reset {
                self.gestures.clear();
            }
            // Another gun: its reload's and spin's gestures stop with it.
            self.gestures.retain(|g| g.slot != Slot::Attack);
            self.weapon = weapon.map(str::to_string);
            self.kind = Some(kind);
            self.state = None;
            self.was_spun = self.spun;
            a.main = None;
        }
        // The stance a gesture starts in (5).
        let stance = if i.swimming {
            "SWIM"
        } else if i.ducked {
            "CROUCH"
        } else {
            "STAND"
        };
        // Landing: the additive land gesture.
        if self.airborne && i.on_ground && !i.swimming {
            self.gesture(&set, "ACT_MP_JUMP_LAND", kind, Slot::Jump, false, false);
        }
        self.airborne = !i.on_ground && !i.swimming;
        // The wind-up's and wind-down's gestures for the stance (6).
        let spin_changed = self.aiming() != self.was_spun;
        if spin_changed {
            let part = if self.aiming() { "PREFIRE" } else { "POSTFIRE" };
            self.gesture(&set, &format!("ACT_MP_ATTACK_{stance}_{part}"), kind, Slot::Attack, false, false);
            self.was_spun = self.aiming();
        }
        let state = self.pick(i, a);
        if self.state != Some(state) || a.main.is_none() || spin_changed {
            let (act, s) = self.main_sequence(&set, state, kind, a.main);
            // The main sequence changes only with its activity (3 step 6).
            let same = a.main.and_then(|m| set.sequences.get(m)).is_some_and(|m| !act.is_empty() && m.activity.eq_ignore_ascii_case(&act));
            if !same && !set.sequences.is_empty() {
                a.play(s, now);
            }
            if state == State::JumpStart && self.state != Some(State::JumpStart) {
                a.cycle = 0.0;
            }
            self.state = Some(state);
        }
        if i.fired {
            // Spun, a shot doesn't restart the fire gesture still playing (6).
            let keep = self.aiming();
            self.gesture(&set, &format!("ACT_MP_ATTACK_{stance}"), kind, Slot::Attack, false, keep);
        }
        if i.reloaded {
            let reload_stance = if !i.on_ground && !i.swimming { "AIRWALK" } else { stance };
            match i.reload_part {
                "_loop" => self.gesture(&set, &format!("ACT_MP_RELOAD_{reload_stance}_LOOP"), kind, Slot::Attack, true, false),
                "_end" => self.gesture(&set, &format!("ACT_MP_RELOAD_{reload_stance}_END"), kind, Slot::Attack, false, false),
                _ => self.gesture(&set, &format!("ACT_MP_RELOAD_{reload_stance}"), kind, Slot::Attack, false, false),
            }
        }
        // The pose parameters (3, 4), by name.
        let speed = i.velocity.length();
        let moving = speed > MOVING_SPEED;
        let body_yaw = self.turn(i.eye_yaw, (i.velocity.length_squared() + i.vertical * i.vertical).sqrt() > 1.0, dt);
        let body_pitch = normalize(-i.eye_pitch).clamp(PITCH_DOWN, PITCH_UP);
        let mut mv = Vec2::ZERO;
        if moving {
            // The movement direction against the eyes (3).
            let d = normalize(i.velocity.y.atan2(i.velocity.x).to_degrees() - i.eye_yaw);
            mv = square(d);
            // The grid's ground speed in that direction, at the full cell.
            let (px, py) = (set.param("move_x"), set.param("move_y"));
            if let (Some(px), Some(py), Some(main)) = (px, py, a.main) {
                a.set_param(px, mv.x);
                a.set_param(py, mv.y);
                let g = set.ground_motion(main, &a.params);
                if g > 0.0 && speed < g {
                    mv *= speed / g;
                }
            }
        }
        let values = [mv.x, mv.y, body_yaw, body_pitch];
        for (name, v) in ["move_x", "move_y", "body_yaw", "body_pitch"].into_iter().zip(values) {
            if let Some(p) = set.param(name) {
                a.set_param(p, v);
            }
        }
        // The hand- and arm-pose libraries at their first pose (the
        // constants: 0 for the guns). Left to the engine's default, a
        // parameter whose range starts at 0 sits at its middle: the
        // Engineer's arms took a special item's pose and held his pistol
        // turned at the belt (playtest 2026-11-18).
        for name in ["r_hand_grip", "r_arm"] {
            if let Some(p) = set.param(name) {
                a.set_param(p, 0.0);
            }
        }
        self.pose = values;
        a.playback = 1.0;
        a.advance(dt, now);
        // Gestures: played once (rate 1); a reload's loop holds its last
        // frame until the next round or the end.
        for g in &mut self.gestures {
            g.cycle += dt * set.cycle_rate(g.sequence, &a.params);
            if g.cycle >= 1.0 {
                if g.held {
                    g.cycle = 1.0;
                } else {
                    g.weight = 0.0;
                }
            }
        }
        self.gestures.retain(|g| g.weight > 0.0);
        a.layers = self
            .gestures
            .iter()
            .map(|g| Some(Layer { sequence: g.sequence, cycle: g.cycle.min(1.0), weight: g.weight }))
            .collect();
        self.reset = false;
    }
}

impl PlayerAnimRules for Tf2PlayerAnim {
    const GAME: &'static str = super::GAME;

    fn update(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        self.step(a, i, weapon, dt, now);
    }

    fn feet_yaw(&self) -> f32 {
        self.render_yaw
    }

    fn dead(&mut self) {
        self.reset = true;
        self.gestures.clear();
    }
}

/// TF2's bodies hold their active weapon's `c_` model.
fn hold_weapons(
    characters: Query<(Entity, Option<&Inventory>, Option<&crate::map::Held>), With<Tf2PlayerAnim>>,
    weapons: Query<&crate::weapon::Weapon>,
    mut commands: Commands,
) {
    for (e, inventory, held) in &characters {
        let id = inventory
            .and_then(|i| i.active)
            .and_then(|w| weapons.get(w).ok())
            .map(|w| w.id.to_string());
        if held.is_none_or(|h| h.0 != id) {
            commands.entity(e).insert(crate::map::Held(id));
        }
    }
}

/// Each TF2 body's class for its stance (`Tf2PlayerAnim::class`): the
/// one its gun in hand makes it (`classes::Classes::held`, the rung's
/// drawn class when the gun can be it), on every machine, in the drawn
/// state and in a server's own (`SimAnimState`; before, a server posed a
/// Soldier's Shotgun as a primary for everyone else).
#[allow(clippy::type_complexity)]
fn follow_class(
    mut bodies: Query<(
        Option<&mut Tf2PlayerAnim>,
        Option<&mut source_engine::player_anim::SimAnimState<Tf2PlayerAnim>>,
        Option<&Inventory>,
        Option<&super::classes::RungClass>,
    ), Or<(With<Tf2PlayerAnim>, With<source_engine::player_anim::SimAnimState<Tf2PlayerAnim>>)>>,
    weapons: Query<&crate::weapon::Weapon>,
    table: Option<Res<super::classes::Classes>>,
) {
    let Some(table) = table else { return };
    for (anim, sim, inv, drawn) in &mut bodies {
        if anim.is_none() && sim.is_none() {
            continue;
        }
        let held = inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
        let class = held.and_then(|w| table.held(&w.id, drawn.map(|d| d.0.as_str()))).map(|c| c.name.as_str());
        // The drawn state, and a server's own, whose pose clients draw
        // others with (`anim_net`).
        if let Some(mut a) = anim
            && a.class.as_deref() != class
        {
            a.class = class.map(str::to_string);
        }
        if let Some(mut s) = sim
            && s.0.class.as_deref() != class
        {
            s.0.class = class.map(str::to_string);
        }
    }
}

/// Each TF2 body's spin (`Tf2PlayerAnim::spun`): its Minigun in hand
/// winding up or spun, in the drawn state and in a server's own
/// (`SimAnimState`, whose pose clients draw others with: `anim_net`).
#[allow(clippy::type_complexity)]
fn follow_spin(
    mut bodies: Query<
        (Option<&mut Tf2PlayerAnim>, Option<&mut source_engine::player_anim::SimAnimState<Tf2PlayerAnim>>, Option<&Inventory>),
        Or<(With<Tf2PlayerAnim>, With<source_engine::player_anim::SimAnimState<Tf2PlayerAnim>>)>,
    >,
    guns: Query<&super::weapons::minigun::Tf2Minigun>,
) {
    use super::weapons::minigun::Spin;
    for (drawn, sim, inv) in &mut bodies {
        if drawn.is_none() && sim.is_none() {
            continue;
        }
        let spun = inv
            .and_then(|i| i.active)
            .and_then(|w| guns.get(w).ok())
            .is_some_and(|g| matches!(g.spin, Spin::Up { .. } | Spin::Spun { .. }));
        if let Some(mut a) = drawn
            && a.spun != spun
        {
            a.spun = spun;
        }
        if let Some(mut s) = sim
            && s.0.spun != spun
        {
            s.0.spun = spun;
        }
    }
}

/// TF2's bodies animated: drawn (`PlayerAnimPlugin`), on a server per tick
/// (`SimAnimPlugin`), sent to clients (`AnimNetPlugin`), what they hold and
/// their faces (`faces`).
pub struct Tf2AnimPlugin;

impl Plugin for Tf2AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            source_engine::player_anim::PlayerAnimPlugin::<Tf2PlayerAnim>::default(),
            source_engine::player_anim::SimAnimPlugin::<Tf2PlayerAnim>::default(),
            source_engine::anim_net::AnimNetPlugin::<Tf2PlayerAnim>::new("tf2_anim"),
        ))
        .insert_resource(super::faces::IdleFaces::builtin())
        .add_systems(
            Update,
            (hold_weapons, follow_class, follow_spin, super::faces::faces.in_set(crate::map::DriveAnimation)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 3, test cases 2-4: forward (1, 0), left (0, -1), forward-right
    /// pushed to the corner (1, 1).
    #[test]
    fn the_move_grid_is_the_unit_square() {
        let near = |a: Vec2, b: Vec2| (a - b).length() < 1e-4;
        assert!(near(square(0.0), Vec2::new(1.0, 0.0)));
        assert!(near(square(90.0), Vec2::new(0.0, -1.0)));
        assert!(near(square(-45.0), Vec2::new(1.0, 1.0)));
        assert!(near(square(180.0), Vec2::new(-1.0, 0.0)));
    }

    #[test]
    fn kinds_by_slot() {
        assert_eq!(Kind::of("tf2:tf_weapon_scattergun"), Kind::Primary);
        assert_eq!(Kind::of("tf2:tf_weapon_minigun"), Kind::Primary);
        assert_eq!(Kind::of("tf2:tf_weapon_shotgun_primary"), Kind::Primary);
        assert_eq!(Kind::of("tf2:tf_weapon_pistol"), Kind::Secondary);
        assert_eq!(Kind::of("tf2:tf_weapon_revolver"), Kind::Secondary);
        // The Shotgun in a drawn class's hands: the Engineer's primary,
        // the others' secondary.
        assert_eq!(Kind::of_class("tf2:tf_weapon_shotgun_primary", Some("engineer")), Kind::Primary);
        assert_eq!(Kind::of_class("tf2:tf_weapon_shotgun_primary", Some("pyro")), Kind::Secondary);
        assert_eq!(Kind::of_class("tf2:tf_weapon_shotgun_primary", None), Kind::Primary);
        assert_eq!(Kind::of_class("tf2:tf_weapon_pistol_scout", Some("engineer")), Kind::Secondary);
    }

    /// The constants: `r_hand_grip` and `r_arm` at 0 for the guns, not
    /// at their range's middle (the engine's default for a range that
    /// starts at 0): the Engineer held his pistol turned at the belt.
    #[test]
    fn the_hand_and_arm_poses_are_the_first() {
        use crate::map::anim::PoseParam;
        let p = |name: &str, end: f32| PoseParam { name: name.into(), start: 0.0, end, looping: 0.0 };
        let set = std::sync::Arc::new(AnimSet { params: vec![p("r_hand_grip", 16.0), p("r_arm", 3.0)], ..Default::default() });
        let mut a = Animator::new(set.clone());
        assert_eq!(a.params, vec![0.5, 0.5], "the engine's default");
        let mut s = Tf2PlayerAnim::default();
        s.step(&mut a, &Inputs { on_ground: true, ..Default::default() }, Some("tf2:tf_weapon_pistol_scout"), 0.015, 0.0);
        assert_eq!(a.params, vec![0.0, 0.0]);
    }

    /// 4, tests 13-14, 16c, 16d: the pitch's sign and clamp; the feet
    /// stay inside 45 degrees standing, then their goal steps 45 toward
    /// the eyes; body_yaw is minus the eyes against the feet.
    #[test]
    fn aim_and_feet() {
        let mut s = Tf2PlayerAnim::default();
        assert_eq!(s.turn(0.0, false, 0.015), 0.0);
        s.reset = false;
        let y = s.turn(30.0, false, 0.015);
        assert!((y + 30.0).abs() < 1e-3, "16d: {y}");
        assert_eq!(s.feet_yaw, 0.0);
        // 16c: the eyes at 50: the goal 45, the first step at 540 degrees/s.
        let mut s = Tf2PlayerAnim { reset: false, ..Default::default() };
        s.turn(50.0, false, 0.01);
        assert!((s.feet_yaw - 5.4).abs() < 1e-3, "{}", s.feet_yaw);
        for _ in 0..200 {
            s.turn(50.0, false, 0.015);
        }
        assert!((s.feet_yaw - 45.0).abs() < 1e-3, "turned to {}", s.feet_yaw);
        // Moving: onto the eyes.
        for _ in 0..200 {
            s.turn(50.0, true, 0.015);
        }
        assert!((s.feet_yaw - 50.0).abs() < 1e-3, "turned to {}", s.feet_yaw);
        assert_eq!(normalize(89.0f32).clamp(PITCH_DOWN, PITCH_UP), 89.0);
        assert_eq!(normalize(-60.0f32).clamp(PITCH_DOWN, PITCH_UP), -45.0);
    }

    /// 2: the kind tables and the deployed exception.
    #[test]
    fn kind_tables() {
        assert_eq!(translate("ACT_MP_RUN", Kind::Secondary), "ACT_MP_RUN_SECONDARY");
        assert_eq!(translate("ACT_MP_RELOAD_STAND_LOOP", Kind::Primary), "ACT_MP_RELOAD_STAND_PRIMARY_LOOP");
        assert_eq!(translate("ACT_MP_RELOAD_CROUCH", Kind::Secondary), "ACT_MP_RELOAD_CROUCH_SECONDARY");
        assert_eq!(translate("ACT_MP_DEPLOYED_IDLE", Kind::Primary), "ACT_MP_DEPLOYED_IDLE");
        assert_eq!(translate("ACT_MP_DEPLOYED", Kind::Primary), "ACT_MP_DEPLOYED_PRIMARY");
        assert_eq!(translate("ACT_MP_DEPLOYED", Kind::Melee), "ACT_MP_DEPLOYED");
        assert_eq!(translate("ACT_MP_SWIM_DEPLOYED", Kind::Primary), "ACT_MP_SWIM_DEPLOYED_PRIMARY");
        assert_eq!(translate("ACT_MP_ATTACK_STAND_PREFIRE", Kind::Primary), "ACT_MP_ATTACK_STAND_PREFIRE");
        assert_eq!(translate("ACT_MP_ATTACK_CROUCH", Kind::Primary), "ACT_MP_ATTACK_CROUCH_PRIMARY");
    }

    fn made_up(names: &[&str]) -> std::sync::Arc<AnimSet> {
        std::sync::Arc::new(made_up_set(names))
    }

    fn made_up_set(names: &[&str]) -> AnimSet {
        use crate::map::anim::{Animation, Sequence};
        let seq = |activity: &str| Sequence {
            name: activity.to_lowercase(),
            activity: activity.into(),
            activity_weight: 1,
            looping: true,
            grid: (1, 1),
            anims: vec![0],
            ..Default::default()
        };
        AnimSet {
            sequences: names.iter().map(|n| seq(n)).collect(),
            animations: vec![Animation { name: "a".into(), fps: 30.0, frames: 30, ..Default::default() }],
            ..Default::default()
        }
    }

    /// 6, tests 10, 12, 12c: a spun Minigun plays the deployed idle with
    /// the PREFIRE gesture (no jump while spun); shots don't restart the
    /// fire gesture still playing; stopping, the plain stand with POSTFIRE.
    #[test]
    fn a_spun_minigun_plays_the_deployed_set() {
        let set = made_up(&["", "ACT_MP_STAND_PRIMARY", "ACT_MP_DEPLOYED_IDLE", "ACT_MP_ATTACK_STAND_PREFIRE", "ACT_MP_ATTACK_STAND_POSTFIRE", "ACT_MP_ATTACK_STAND_PRIMARY", "ACT_MP_JUMP_START_PRIMARY"]);
        let mut a = Animator::new(set);
        let mut s = Tf2PlayerAnim::default();
        let gun = Some("tf2:tf_weapon_minigun");
        let still = Inputs { on_ground: true, ..Default::default() };
        let mut t = 0.0;
        let mut step = |s: &mut Tf2PlayerAnim, a: &mut Animator, i: &Inputs| {
            t += 0.015;
            s.step(a, i, gun, 0.015, t);
        };
        step(&mut s, &mut a, &still);
        assert_eq!(a.main, Some(1));
        s.spun = true;
        step(&mut s, &mut a, &still);
        assert_eq!(a.main, Some(2), "the deployed idle");
        assert!(s.gestures.iter().any(|g| g.sequence == 3), "the wind-up's gesture: {:?}", s.gestures);
        step(&mut s, &mut a, &Inputs { jumped: true, ..still });
        assert_eq!(a.main, Some(2), "12c: no jump while spun");
        // Shots while spun: the fire gesture runs on, not restarted.
        step(&mut s, &mut a, &Inputs { fired: true, ..still });
        let c = s.gestures.iter().find(|g| g.sequence == 5).map(|g| g.cycle).expect("the fire gesture");
        step(&mut s, &mut a, &Inputs { fired: true, ..still });
        assert!(s.gestures.iter().find(|g| g.sequence == 5).is_some_and(|g| g.cycle > c), "not restarted");
        s.spun = false;
        step(&mut s, &mut a, &still);
        assert_eq!(a.main, Some(1), "the plain stand again");
        assert!(s.gestures.iter().any(|g| g.sequence == 4), "the wind-down's gesture");
        assert_eq!(s.gestures.len(), 1, "one gesture in the attack slot: {:?}", s.gestures);
    }

    /// Open question 13 (ours): a holder without deployed sequences (a
    /// Scout with the Minigun) keeps his kind's normal set when spun.
    #[test]
    fn spun_without_a_deployed_set_keeps_the_normal_one() {
        let set = made_up(&["", "ACT_MP_STAND_PRIMARY", "ACT_MP_RUN_PRIMARY"]);
        let mut a = Animator::new(set);
        let mut s = Tf2PlayerAnim { spun: true, ..Default::default() };
        s.step(&mut a, &Inputs { on_ground: true, ..Default::default() }, Some("tf2:tf_weapon_minigun"), 0.015, 0.0);
        assert_eq!(a.main, Some(1));
        s.step(&mut a, &Inputs { on_ground: true, velocity: Vec2::new(100.0, 0.0), ..Default::default() }, Some("tf2:tf_weapon_minigun"), 0.015, 0.1);
        assert_eq!(a.main, Some(2));
    }

    /// 1a, test 25: the weighted pick, 2 : 1 over many draws; a negative
    /// weight keeps the current sequence.
    #[test]
    fn weighted_picks() {
        let mut set = made_up_set(&["", "ACT_X", "ACT_X"]);
        set.sequences[1].activity_weight = 2;
        let mut s = Tf2PlayerAnim::default();
        let n = 30_000;
        let a = (0..n).filter(|_| set.select_activity("ACT_X", None, s.roll()) == Some(1)).count();
        let share = a as f32 / n as f32;
        assert!((share - 2.0 / 3.0).abs() < 0.01, "{share}");
        set.sequences[2].activity_weight = -1;
        assert!((0..100).all(|_| set.select_activity("ACT_X", Some(2), s.roll()) == Some(2)));
    }
}
