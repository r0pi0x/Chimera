//! TF2's player animation state (specs/tf2/player_animation.md 1a-6, 8)
//! on Source's shared half (`source_engine::player_anim`, as HL2:DM's):
//! one main sequence from the movement state (3: airwalk only rising above
//! 300 units/s or already in it, until the ground or water; a jump's start
//! for 0.5 s, then its float, until water or the ground after 0.2 s; then
//! crouch, swim, run or stand; the jump, landing and entering water
//! restart it at cycle 0, landing from a jump or airwalk plays the land
//! gesture)
//! asked kind-less and translated by the held gun's kind table
//! (`translate`, 2: the suffix, a reload's before its `_LOOP` / `_END`,
//! the deployed activities only in the primary table; `Kind::of_class`:
//! `PRIMARY`, `SECONDARY` or `MELEE`, the Shotgun the Engineer's primary
//! and the Soldier's, Pyro's and Heavy's secondary, the Grenade Launcher
//! a secondary and the Stickybomb Launcher a primary (their items'
//! animation slots, weapons_more.md 1); the class
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
//! degrees/s, slower in the last 60; the goal snaps to the eyes only when
//! the state is made and on a snap, `map::anim::SnapFeet`: `setang_exact`,
//! a scene's `target set <n> feet`, never on a death, respawn or
//! teleport, 4c 5), each parameter by name, the main
//! sequences' own aim-matrix autolayers adding the pose with the feet
//! kept where the main sequence put them and the knees bent toward the
//! include's knee directions (`Animator::ik_locks`, 4a) and
//! the left hand held on the Minigun by the aim cells' IK rule
//! (`Animator::ik_rules`, 4a);
//! every pose parameter at 0 in its own units on each spawn and a new
//! model (4b: `r_hand_grip` and `r_arm` the first cells of the hand and
//! arm libraries, which act only where a host sequence layers them, the
//! Engineer's pistol's `r_handposes_engineer`; the engine's default for
//! a range from 0 is its middle, a special item's pose), then the held
//! item's player pose parameters (`POSEPARAM_HOOK`, the item schema's
//! `player_poseparam`; no stock weapon has one), the old item's back to 0
//! on a switch; fire,
//! reload (start, a loop per round, end), the spin and landing gestures
//! by the stance, one per slot (attack, jump) in slot order (5). Each
//! side runs it on its own values (`PlayerAnimPlugin` drawn,
//! `SimAnimPlugin` for a server's hitboxes) and a client poses others as
//! the server's tick pose had them (`source_engine::anim_net`).
//! `hold_weapons` puts the active gun's `c_` model in their hands
//! (bone-merged on `weapon_bone`), in the frame of a switch; nothing else
//! they carry is drawn (7a: no holstered weapon).
//!
//! Known shortcuts: the
//! scoped Sniper's deployed set and its 2 s hold aren't played; the
//! class data's `DontDoAirwalk` / `DontDoNewJump` and the Scout's air-dash
//! crouch aren't read (3); no
//! flinch on damage (open question 7) and no air-jump sequence (open
//! question 4: `jump_double_primary` has no activity); no Spy disguise,
//! cloak, stun or ghost mode, so 7a's reasons to hide the active weapon
//! are only the owner's first-person view (the body hidden); other
//! clients don't keep an item's pose parameters past a switch (4b's
//! quirk: they write the new weapon's list over a reset).

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
/// Rising faster than this (units/s) is airwalk (3 step 1).
pub const AIRWALK_SPEED: f32 = 300.0;
/// The jump's start plays this long (s), then its float (3 step 2).
pub const JUMP_START_TIME: f64 = 0.5;
/// The jump state ends on the ground only after this long (s).
pub const JUMP_LAND_AFTER: f64 = 0.2;
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
    /// The kind of gun `id` for its own class (2: the weapon's own type,
    /// or the item's animation slot where it has one: the Grenade
    /// Launcher animates as a secondary and the Stickybomb Launcher as a
    /// primary, weapons_more.md 1, the reverse of their item slots): the
    /// secondaries (the Pistol, SMG, Revolver, Grenade Launcher, Medi
    /// Gun, the class shotguns) and melee weapons by name, every other gun
    /// primary (another game's gun in a TF2 body, open question 1:
    /// primary). With the Demoman's primary set the Grenade Launcher's
    /// `weapon_bone_1` (its barrel) took the Stickybomb Launcher's pose,
    /// across his body (7: bone-merged by name).
    pub fn of(id: &str) -> Kind {
        let name = id.rsplit(':').next().unwrap_or(id).trim_start_matches("tf_weapon_");
        match name {
            "pistol" | "pistol_scout" | "smg" | "revolver" | "grenadelauncher" | "medigun" | "shotgun_soldier" | "shotgun_hwg"
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
    /// The feet (and their goal) go to the eyes on the next update: only
    /// when the state is made and on `setang_exact` (`snap_feet`), not on
    /// a respawn or a teleport (4, 4c 5).
    snap: bool,
    /// Where a snap puts the feet against the eyes (degrees; 0 but for a
    /// scene lining its feet up with a reference's, `map::anim::SnapFeet`).
    snap_offset: f32,
    /// The animation set the pose parameters were zeroed for (its
    /// address): a new model zeroes them again (4b).
    zeroed_for: usize,
    /// The held weapon's player pose parameters (the item schema's
    /// `player_poseparam`, 4b; `PlayerPoseParams` on the weapon), written
    /// each update; `follow_items` keeps it.
    pub item_params: Vec<(String, f32)>,
    /// The names the last weapon's list set, zeroed on a switch (4b).
    item_names: Vec<String>,
    /// The feet yaw and its goal (degrees, Source's axes).
    pub feet_yaw: f32,
    goal_yaw: f32,
    /// The render and hitbox yaw: the feet before this update's turn.
    pub render_yaw: f32,
    pub state: Option<State>,
    /// The jump state's start time (3 step 2: its start, then its float).
    jump_t0: Option<f64>,
    /// In airwalk (3 step 1).
    airwalk: bool,
    /// In water at the last update (entering it restarts the main sequence).
    in_water: bool,
    weapon: Option<String>,
    kind: Option<Kind>,
    /// The holder's class (`follow_class`; None: the gun's own).
    pub class: Option<String>,
    /// The active weapon's base when it is an unlock (`follow_items`:
    /// it stands as that weapon).
    pub base: Option<String>,
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
            snap: true,
            snap_offset: 0.0,
            zeroed_for: 0,
            item_params: Vec::new(),
            item_names: Vec::new(),
            feet_yaw: 0.0,
            goal_yaw: 0.0,
            render_yaw: 0.0,
            state: None,
            jump_t0: None,
            airwalk: false,
            in_water: false,
            weapon: None,
            kind: None,
            class: None,
            base: None,
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
        if self.snap {
            let feet = normalize(eye + self.snap_offset);
            self.feet_yaw = feet;
            self.goal_yaw = feet;
            self.render_yaw = feet;
            self.snap = false;
            self.snap_offset = 0.0;
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

    /// Snap the feet and their goal to the eyes (plus `offset` degrees,
    /// counter-clockwise) on the next update (4: a "snap yaw", which TF2
    /// sends only on `setang_exact`; an offset lines a scene's feet up
    /// with a reference's).
    pub fn snap_feet(&mut self, offset: f32) {
        self.snap = true;
        self.snap_offset = offset;
    }

    /// The feet's goal yaw (degrees, 4).
    pub fn goal_yaw(&self) -> f32 {
        self.goal_yaw
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

    /// The main sequence's state from the inputs (3, "Choosing"), with
    /// whether the main sequence restarts at cycle 0 and whether the land
    /// gesture plays: airwalk (not ducked, rising above 300 units/s or
    /// already in it) until the ground or water; then the jump state (its
    /// start for 0.5 s, then its float; ended by water, or by the ground
    /// after 0.2 s); then crouch, swim, run or stand. A spun Heavy skips
    /// airwalk and the jump (6).
    fn pick(&mut self, i: &Inputs, set: &AnimSet, kind: Kind, now: f64) -> (State, bool, bool) {
        let moving = i.velocity.length() > MOVING_SPEED;
        // "Ducked": the duck flag, if the kind has a crouch-walk.
        let ducked = i.ducked && set.activity(&translate("ACT_MP_CROUCHWALK", kind)).is_some();
        let (mut restart, mut land) = (false, false);
        let entered_water = i.swimming && !self.in_water;
        self.in_water = i.swimming;
        if !self.aiming() {
            if !ducked && (i.vertical > AIRWALK_SPEED || self.airwalk) {
                if i.on_ground {
                    self.airwalk = false;
                    (restart, land) = (true, true);
                } else if i.swimming {
                    self.airwalk = false;
                    restart = true;
                } else {
                    self.airwalk = true;
                    return (State::Airwalk, restart, land);
                }
            }
            if i.jumped {
                self.jump_t0 = Some(now);
                restart = true;
            }
            if let Some(t0) = self.jump_t0 {
                let t = now - t0;
                if i.swimming {
                    self.jump_t0 = None;
                    restart = true;
                } else if t > JUMP_LAND_AFTER && i.on_ground {
                    self.jump_t0 = None;
                    (restart, land) = (true, true);
                } else {
                    let state = if t <= JUMP_START_TIME { State::JumpStart } else { State::JumpFloat };
                    return (state, restart, land);
                }
            }
        }
        let state = match (ducked, i.swimming, moving) {
            (true, _, true) => State::CrouchWalk,
            (true, _, false) => State::Crouch,
            (false, true, _) => State::Swim,
            (false, false, true) => State::Run,
            (false, false, false) => State::Stand,
        };
        (state, restart || (entered_water && state == State::Swim), land)
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
        a.ik_rules = true;
        // An unlock stands as its base (`follow_items`).
        let kind = weapon.map_or(Kind::Primary, |w| Kind::of_class(self.base.as_deref().unwrap_or(w), self.class.as_deref()));
        // Every pose parameter at 0 in its own units when the model is set
        // and on each spawn (4b): left to the engine's default, one whose
        // range starts at 0 sat at its middle, a special item's pose (the
        // Engineer's pistol turned at the belt, playtest 2026-11-18).
        let set_id = std::sync::Arc::as_ptr(&set) as usize;
        if self.reset || self.zeroed_for != set_id {
            for p in 0..set.params.len() {
                a.set_param(p, 0.0);
            }
            self.zeroed_for = set_id;
            self.item_names.clear();
        }
        // Another item's list: the old one's names back to 0 first (4b).
        if self.item_names.iter().ne(self.item_params.iter().map(|(n, _)| n)) {
            for name in &self.item_names {
                if let Some(p) = set.param(name) {
                    a.set_param(p, 0.0);
                }
            }
            self.item_names = self.item_params.iter().map(|(n, _)| n.clone()).collect();
        }
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
        // The wind-up's and wind-down's gestures for the stance (6).
        let spin_changed = self.aiming() != self.was_spun;
        if spin_changed {
            let part = if self.aiming() { "PREFIRE" } else { "POSTFIRE" };
            self.gesture(&set, &format!("ACT_MP_ATTACK_{stance}_{part}"), kind, Slot::Attack, false, false);
            self.was_spun = self.aiming();
        }
        let (state, restart, land) = self.pick(i, &set, kind, now);
        if land {
            // Landing from a jump or airwalk: the additive land gesture.
            self.gesture(&set, "ACT_MP_JUMP_LAND", kind, Slot::Jump, false, false);
        }
        if self.state != Some(state) || a.main.is_none() || spin_changed || restart {
            let (act, s) = self.main_sequence(&set, state, kind, a.main);
            // The main sequence changes only with its activity (3 step 6).
            let same = a.main.and_then(|m| set.sequences.get(m)).is_some_and(|m| !act.is_empty() && m.activity.eq_ignore_ascii_case(&act));
            if !same && !set.sequences.is_empty() {
                a.play(s, now);
            }
            // The jump, landing and entering water restart it (3).
            if restart {
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
            let reload_stance = if self.airwalk { "AIRWALK" } else { stance };
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
        // The held item's player pose parameters (4b; none for a stock
        // weapon: `r_hand_grip` and `r_arm` stay at 0, the first cells).
        // A name the model lacks is skipped.
        for (name, v) in &self.item_params {
            if let Some(p) = set.param(name) {
                a.set_param(p, *v);
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

    /// Death restarts the state but keeps the feet's goal (4c 5).
    fn dead(&mut self) {
        self.reset = true;
        self.gestures.clear();
    }

    fn snap_feet(&mut self, offset: f32) {
        Tf2PlayerAnim::snap_feet(self, offset);
    }
}

/// The weapon data's hook naming an item's player pose parameters (the
/// item schema's `visuals` -> `player_poseparam`, 4b):
/// `[hooks."tf2:player_poseparam"] r_hand_grip = 16`. No stock weapon
/// has one.
pub const POSEPARAM_HOOK: &str = "tf2:player_poseparam";

/// On a weapon: the player pose parameters its holder's model takes
/// while it is active (`POSEPARAM_HOOK`), by name.
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct PlayerPoseParams(pub Vec<(String, f32)>);

/// `POSEPARAM_HOOK`'s parameters onto the weapon.
pub fn attach_pose_params(e: &mut EntityWorldMut, p: &std::collections::BTreeMap<String, f32>) {
    e.insert(PlayerPoseParams(p.iter().map(|(k, v)| (k.clone(), *v)).collect()));
}

/// Each TF2 body's item pose parameters (`Tf2PlayerAnim::item_params`):
/// its active weapon's `PlayerPoseParams` (none: an empty list), in the
/// drawn state and a server's own.
#[allow(clippy::type_complexity)]
fn follow_items(
    mut bodies: Query<
        (Option<&mut Tf2PlayerAnim>, Option<&mut source_engine::player_anim::SimAnimState<Tf2PlayerAnim>>, Option<&Inventory>),
        Or<(With<Tf2PlayerAnim>, With<source_engine::player_anim::SimAnimState<Tf2PlayerAnim>>)>,
    >,
    items: Query<&PlayerPoseParams>,
    unlocks: Query<&super::items::mods::Tf2Item>,
) {
    for (drawn, sim, inv) in &mut bodies {
        let active = inv.and_then(|i| i.active);
        let list = active.and_then(|w| items.get(w).ok()).map_or(&[][..], |p| p.0.as_slice());
        let base = active.and_then(|w| unlocks.get(w).ok()).map(|i| i.0.base.as_str()).filter(|b| !b.is_empty());
        if let Some(mut a) = drawn {
            if a.item_params != list {
                a.item_params = list.to_vec();
            }
            if a.base.as_deref() != base {
                a.base = base.map(str::to_string);
            }
        }
        if let Some(mut s) = sim {
            if s.0.item_params != list {
                s.0.item_params = list.to_vec();
            }
            if s.0.base.as_deref() != base {
                s.0.base = base.map(str::to_string);
            }
        }
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
    for (drawn, sim, inv) in &mut bodies {
        if drawn.is_none() && sim.is_none() {
            continue;
        }
        let spun = inv
            .and_then(|i| i.active)
            .and_then(|w| guns.get(w).ok())
            .is_some_and(|g| g.state.turning());
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
            (
                // In the animation set, so the drawn body takes the new
                // weapon in the frame of the switch (7a; `attach_held`
                // runs after it).
                hold_weapons.in_set(crate::map::DriveAnimation),
                follow_class,
                follow_spin,
                follow_items,
                super::faces::faces.in_set(crate::map::DriveAnimation),
            ),
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
        // The items' animation slots: the reverse of their item slots.
        assert_eq!(Kind::of("tf2:tf_weapon_grenadelauncher"), Kind::Secondary);
        assert_eq!(Kind::of("tf2:tf_weapon_pipebomblauncher"), Kind::Primary);
        // The Shotgun in a drawn class's hands: the Engineer's primary,
        // the others' secondary.
        assert_eq!(Kind::of_class("tf2:tf_weapon_shotgun_primary", Some("engineer")), Kind::Primary);
        assert_eq!(Kind::of_class("tf2:tf_weapon_shotgun_primary", Some("pyro")), Kind::Secondary);
        assert_eq!(Kind::of_class("tf2:tf_weapon_shotgun_primary", None), Kind::Primary);
        assert_eq!(Kind::of_class("tf2:tf_weapon_pistol_scout", Some("engineer")), Kind::Secondary);
    }

    /// 4b, test 42: every pose parameter at 0 in its own units at spawn
    /// (`r_hand_grip` and `r_arm` the first cells, not the range's middle,
    /// the engine's default for a range that starts at 0: the Engineer
    /// held his pistol turned at the belt), and again on a new model.
    #[test]
    fn pose_parameters_start_at_zero_42() {
        use crate::map::anim::PoseParam;
        let p = |name: &str, start: f32, end: f32| PoseParam { name: name.into(), start, end, looping: 0.0 };
        let params = vec![p("body_pitch", -45.0, 90.0), p("body_yaw", -45.0, 45.0), p("r_hand_grip", 0.0, 16.0), p("r_arm", 0.0, 3.0), p("move_x", -1.0, 1.0), p("move_y", -1.0, 1.0)];
        let set = std::sync::Arc::new(AnimSet { params: params.clone(), ..Default::default() });
        let mut a = Animator::new(set.clone());
        assert_eq!(a.params[2..4], [0.5, 0.5], "the engine's default");
        let mut s = Tf2PlayerAnim::default();
        let still = Inputs { on_ground: true, ..Default::default() };
        s.step(&mut a, &still, Some("tf2:tf_weapon_pistol_scout"), 0.015, 0.0);
        let value = |a: &Animator, i: usize| a.params[i] * (params[i].end - params[i].start) + params[i].start;
        for i in 0..params.len() {
            assert!(value(&a, i).abs() < 1e-4, "{}: {}", params[i].name, value(&a, i));
        }
        // A new model: zeroed again.
        let other = std::sync::Arc::new(AnimSet { params: params.clone(), ..Default::default() });
        let mut b = Animator::new(other);
        s.step(&mut b, &still, Some("tf2:tf_weapon_pistol_scout"), 0.015, 0.1);
        assert!(value(&b, 2).abs() < 1e-4 && value(&b, 3).abs() < 1e-4);
    }

    /// 4b, tests 47, 48: an item's player pose parameters from the frame
    /// it is active; back to 0 when a weapon without them is; a name the
    /// model lacks is skipped. No stock weapon lists any.
    #[test]
    fn item_pose_parameters_47_48() {
        use crate::map::anim::PoseParam;
        let p = |name: &str, end: f32| PoseParam { name: name.into(), start: 0.0, end, looping: 0.0 };
        let set = std::sync::Arc::new(AnimSet { params: vec![p("r_hand_grip", 16.0), p("r_arm", 3.0)], ..Default::default() });
        let mut a = Animator::new(set);
        let mut s = Tf2PlayerAnim::default();
        let still = Inputs { on_ground: true, ..Default::default() };
        s.item_params = vec![("r_hand_grip".into(), 16.0), ("r_handposes_engineer".into(), 1.0)];
        s.step(&mut a, &still, Some("tf2:drink"), 0.015, 0.0);
        assert_eq!(a.params, vec![1.0, 0.0], "47: the drink's grip");
        s.item_params.clear();
        s.step(&mut a, &still, Some("tf2:tf_weapon_bat"), 0.015, 0.1);
        assert_eq!(a.params, vec![0.0, 0.0], "47: the Bat's: 0 again");
        s.item_params = vec![("r_hand_grip".into(), 15.0), ("r_arm".into(), 3.0)];
        s.step(&mut a, &still, Some("tf2:hot_hand"), 0.015, 0.2);
        assert_eq!(a.params, vec![15.0 / 16.0, 1.0]);
        for w in crate::game::weapons::tf2_weapons().weapons {
            assert!(!w.has_hook(POSEPARAM_HOOK), "48: {} lists player pose parameters", w.id);
        }
    }

    /// 4, tests 13-14, 16c, 16d: the pitch's sign and clamp; the feet
    /// stay inside 45 degrees standing, then their goal steps 45 toward
    /// the eyes; body_yaw is minus the eyes against the feet.
    #[test]
    fn aim_and_feet() {
        let mut s = Tf2PlayerAnim::default();
        assert_eq!(s.turn(0.0, false, 0.015), 0.0);
        let y = s.turn(30.0, false, 0.015);
        assert!((y + 30.0).abs() < 1e-3, "16d: {y}");
        assert_eq!(s.feet_yaw, 0.0);
        // 16c: the eyes at 50: the goal 45, the first step at 540 degrees/s.
        let mut s = Tf2PlayerAnim { reset: false, snap: false, ..Default::default() };
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

    /// Run `turn` standing at eye yaw `eye` for `frames` frames of 15 ms.
    fn stand(s: &mut Tf2PlayerAnim, eye: f32, frames: usize) -> f32 {
        let mut y = 0.0;
        for _ in 0..frames {
            y = s.turn(eye, false, 0.015);
        }
        y
    }

    /// 4c 5, test 53: a teleport (no motion) keeps the feet's goal; it
    /// steps 45 degrees a frame toward the eyes while more than 45 off
    /// (at exactly 180 toward larger yaw), so from 90 to eyes at 270 it
    /// rests at 225 (steps 135, 180, 225) and body_yaw is -45.
    #[test]
    fn the_feet_keep_their_goal_over_a_teleport_53() {
        let mut s = Tf2PlayerAnim::default();
        stand(&mut s, 90.0, 1);
        assert_eq!(s.goal_yaw(), 90.0);
        let goals: Vec<f32> = (0..3).map(|_| {
            s.turn(-90.0, false, 0.015);
            s.goal_yaw()
        }).collect();
        assert_eq!(goals, vec![135.0, 180.0, -135.0], "53: steps 135, 180, 225");
        let y = stand(&mut s, -90.0, 200);
        assert!((y + 45.0).abs() < 1e-3, "53: body_yaw {y}");
        // The others of 4c 5: 0 -> 270 rests at 315 (+45), 330 -> 270 at
        // 285 (+15), 300 -> 270 stays (+30).
        for (from, goal, body) in [(0.0, -45.0, 45.0), (-30.0, -75.0, 15.0), (-60.0, -60.0, 30.0)] {
            let mut s = Tf2PlayerAnim::default();
            stand(&mut s, from, 1);
            let y = stand(&mut s, -90.0, 300);
            assert!((s.goal_yaw() - goal).abs() < 1e-3 && (y - body).abs() < 1e-3, "{from}: goal {} body_yaw {y}", s.goal_yaw());
        }
    }

    /// 4c 5, test 54: death and the respawn keep the goal: goal 0 at
    /// death, respawned facing 270 standing, it steps once to 315 and
    /// body_yaw is +45 (no snap to the eyes).
    #[test]
    fn the_feet_keep_their_goal_over_a_respawn_54() {
        let set = made_up(&["ACT_MP_STAND_PRIMARY"]);
        let mut a = Animator::new(set);
        let mut s = Tf2PlayerAnim::default();
        let still = |yaw: f32| Inputs { on_ground: true, eye_yaw: yaw, ..Default::default() };
        for k in 0..10 {
            s.step(&mut a, &still(0.0), Some("tf2:tf_weapon_rocketlauncher"), 0.015, k as f64 * 0.015);
        }
        PlayerAnimRules::dead(&mut s);
        for k in 0..200 {
            s.step(&mut a, &still(-90.0), Some("tf2:tf_weapon_rocketlauncher"), 0.015, 1.0 + k as f64 * 0.015);
        }
        assert!((s.goal_yaw() + 45.0).abs() < 1e-3, "54: goal {}", s.goal_yaw());
        assert!((s.pose[2] - 45.0).abs() < 1e-3, "54: body_yaw {}", s.pose[2]);
    }

    /// 4, 4c 5, test 55: the first update of a new state, and a snap
    /// (`setang_exact`), put the goal and the feet on the eyes; a snap with
    /// an offset puts them that far off (a scene lining up with TF2).
    #[test]
    fn the_feet_snap_to_the_eyes_55() {
        let mut s = Tf2PlayerAnim::default();
        assert_eq!(s.turn(123.0, false, 0.015), 0.0);
        assert_eq!((s.goal_yaw(), s.feet_yaw), (123.0, 123.0));
        stand(&mut s, -90.0, 100);
        assert_ne!(s.goal_yaw(), -90.0);
        s.snap_feet(0.0);
        assert_eq!(s.turn(-90.0, false, 0.015), 0.0, "55: body_yaw 0");
        assert_eq!((s.goal_yaw(), s.feet_yaw), (-90.0, -90.0));
        s.snap_feet(45.0);
        assert_eq!(s.turn(-90.0, false, 0.015), 45.0);
        assert_eq!(stand(&mut s, -90.0, 50), 45.0, "kept");
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

    const MOVES: [&str; 7] = ["", "ACT_MP_STAND_PRIMARY", "ACT_MP_AIRWALK_PRIMARY", "ACT_MP_JUMP_START_PRIMARY", "ACT_MP_JUMP_FLOAT_PRIMARY", "ACT_MP_JUMP_LAND_PRIMARY", "ACT_MP_SWIM_PRIMARY"];

    /// Steps `s` with `i` at `t`, 15 ms after the last.
    fn at(s: &mut Tf2PlayerAnim, a: &mut Animator, i: &Inputs, t: f64) {
        s.step(a, i, Some("tf2:tf_weapon_scattergun"), 0.015, t);
    }

    /// 3 step 1 (2026-11-15): airwalk only rising above 300 units/s or
    /// already in it; off a ledge (falling) the ground activity goes on;
    /// landing from airwalk restarts the main sequence and plays the land
    /// gesture.
    #[test]
    fn airwalk_only_above_300_units_per_second() {
        let mut a = Animator::new(made_up(&MOVES));
        let mut s = Tf2PlayerAnim::default();
        let ground = Inputs { on_ground: true, ..Default::default() };
        let mut t = 0.0;
        for _ in 0..20 {
            t += 0.015;
            at(&mut s, &mut a, &ground, t);
        }
        assert_eq!(a.main, Some(1));
        // Walked off a ledge, falling: still the stand, no land gesture after.
        for v in [-50.0, -300.0, 289.0, 300.0] {
            t += 0.015;
            at(&mut s, &mut a, &Inputs { on_ground: false, vertical: v, ..Default::default() }, t);
            assert_eq!(a.main, Some(1), "{v} units/s up: no airwalk");
        }
        t += 0.015;
        at(&mut s, &mut a, &ground, t);
        assert!(s.gestures.is_empty(), "no land gesture off a ledge: {:?}", s.gestures);
        // A blast: rising at 301 is airwalk, and stays so falling.
        t += 0.015;
        at(&mut s, &mut a, &Inputs { vertical: 301.0, ..Default::default() }, t);
        assert_eq!(a.main, Some(2));
        for _ in 0..30 {
            t += 0.015;
            at(&mut s, &mut a, &Inputs { vertical: -400.0, ..Default::default() }, t);
        }
        assert_eq!(a.main, Some(2), "airwalk until the ground");
        // Ducked: no airwalk.
        let mut d = Tf2PlayerAnim::default();
        let mut b = Animator::new(made_up(&["", "ACT_MP_STAND_PRIMARY", "ACT_MP_AIRWALK_PRIMARY", "ACT_MP_CROUCH_PRIMARY", "ACT_MP_CROUCHWALK_PRIMARY"]));
        at(&mut d, &mut b, &Inputs { vertical: 600.0, ducked: true, ..Default::default() }, 0.0);
        assert_eq!(b.main, Some(3), "ducked: the crouch");
        // Landing: the stand from cycle 0 and the land gesture.
        t += 0.015;
        at(&mut s, &mut a, &ground, t);
        assert_eq!(a.main, Some(1));
        assert!(a.cycle < 0.02, "restarted: {}", a.cycle);
        assert!(s.gestures.iter().any(|g| g.sequence == 5 && g.slot == Slot::Jump), "{:?}", s.gestures);
    }

    /// 3 step 2 (2026-11-15): the jump's start for 0.5 s whatever its
    /// length, then its float; the ground ends it only after 0.2 s, with a
    /// restart and the land gesture.
    #[test]
    fn the_jump_start_is_cut_at_half_a_second() {
        let mut set = made_up_set(&MOVES);
        // A 2 s start: the cut, not its end, ends it.
        set.animations[0].frames = 60;
        let mut a = Animator::new(std::sync::Arc::new(set));
        let mut s = Tf2PlayerAnim::default();
        let ground = Inputs { on_ground: true, ..Default::default() };
        at(&mut s, &mut a, &ground, 0.0);
        at(&mut s, &mut a, &Inputs { on_ground: true, jumped: true, vertical: 289.0, ..Default::default() }, 1.0);
        assert_eq!(a.main, Some(3));
        assert_eq!(s.state, Some(State::JumpStart));
        // Still on the ground at 0.1 s (the ground is checked after 0.2 s).
        at(&mut s, &mut a, &ground, 1.1);
        assert_eq!(s.state, Some(State::JumpStart));
        let air = Inputs { vertical: 100.0, ..Default::default() };
        at(&mut s, &mut a, &air, 1.49);
        assert_eq!(s.state, Some(State::JumpStart), "0.49 s");
        at(&mut s, &mut a, &air, 1.51);
        assert_eq!(s.state, Some(State::JumpFloat), "0.51 s");
        assert_eq!(a.main, Some(4));
        at(&mut s, &mut a, &ground, 1.8);
        assert_eq!(s.state, Some(State::Stand));
        assert!(a.cycle < 0.02, "restarted on landing: {}", a.cycle);
        assert!(s.gestures.iter().any(|g| g.sequence == 5), "the land gesture: {:?}", s.gestures);
    }

    /// 3 steps 2 and 4 (2026-11-15): entering water restarts the main
    /// sequence (and ends a jump without a land gesture).
    #[test]
    fn entering_water_restarts_the_main_sequence() {
        let mut a = Animator::new(made_up(&MOVES));
        let mut s = Tf2PlayerAnim::default();
        let ground = Inputs { on_ground: true, ..Default::default() };
        let mut t = 0.0;
        for _ in 0..40 {
            t += 0.015;
            at(&mut s, &mut a, &ground, t);
        }
        assert!(a.cycle > 0.5);
        t += 0.015;
        at(&mut s, &mut a, &Inputs { swimming: true, ..Default::default() }, t);
        assert_eq!(a.main, Some(6));
        assert!(a.cycle < 0.02, "restarted: {}", a.cycle);
        for _ in 0..40 {
            t += 0.015;
            at(&mut s, &mut a, &Inputs { swimming: true, ..Default::default() }, t);
        }
        assert!(a.cycle > 0.5, "staying in water doesn't restart it: {}", a.cycle);
        // A jump that ends in water: restarted, no land gesture.
        t += 0.015;
        at(&mut s, &mut a, &Inputs { on_ground: true, jumped: true, ..Default::default() }, t);
        t += 0.3;
        at(&mut s, &mut a, &Inputs { vertical: 50.0, ..Default::default() }, t);
        assert_eq!(s.state, Some(State::JumpStart));
        t += 0.015;
        at(&mut s, &mut a, &Inputs { swimming: true, ..Default::default() }, t);
        assert_eq!(s.state, Some(State::Swim));
        assert!(a.cycle < 0.02);
        assert!(s.gestures.is_empty(), "{:?}", s.gestures);
    }
}
