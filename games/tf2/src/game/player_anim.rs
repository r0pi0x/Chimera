//! TF2's player animation state (specs/tf2/player_animation.md 2-6, 8) on
//! Source's shared half (`source_engine::player_anim`, as HL2:DM's): one
//! main sequence from the movement state (stand, crouch, run, crouch-walk,
//! swim, airwalk, a jump's start played once then its float) translated by
//! the held gun's kind (`Kind::of_class`: `PRIMARY`, `SECONDARY` or
//! `MELEE`, the gun's slot for its holder's class, 2: the Shotgun is the
//! Engineer's primary and the Soldier's, Pyro's and Heavy's secondary;
//! the class `Tf2PlayerAnim::class`, kept by `follow_class` from the
//! rung's drawn class) into `ACT_MP_<STATE>_<KIND>`; the 3 x 3
//! move grid by `move_x` / `move_y` (the velocity's direction against the
//! feet pushed out to the unit square, scaled by the speed over the grid's
//! ground speed there, never above 1; playback rate 1, 3); aim by
//! `body_pitch` (minus the eye pitch, -45 to 90) and `body_yaw` (the eyes
//! against the feet, which stay until the eyes turn past 45 degrees
//! standing and follow them while moving, at 720 degrees/s, slower in the
//! last 60), the main sequences' own aim-matrix autolayers adding the
//! pose (4); fire, reload (start, a loop per round, end) and landing
//! gestures by the stance on overlay layers (5). Each side runs it on its
//! own values (`PlayerAnimPlugin` drawn, `SimAnimPlugin` for a server's
//! hitboxes) and a client poses others as the server's tick pose had them
//! (`source_engine::anim_net`). `hold_weapons` puts the gun's `c_` model
//! in their hands (bone-merged on `weapon_bone`).
//!
//! Known shortcuts: the Heavy's spun ("deployed") main sequences and
//! spin gestures (6) aren't played (the anim state sees the gun, not its
//! spin); no flinch on damage (open question 7) and no air-jump sequence
//! (open question 4: `jump_double_primary` has no activity); several
//! sequences of one activity take the first, not a weighted pick (the
//! Soldier's five floats); the feet-yaw rules are cs_source's §13 reading
//! (open question 2).

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

/// An overlay layer (a gesture, 5).
#[derive(Clone, Debug, PartialEq)]
pub struct Gesture {
    pub sequence: usize,
    pub cycle: f32,
    pub weight: f32,
    /// A reload's loop: restarted per round, kept until the end.
    pub held: bool,
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
    /// The feet toward the eyes (4) for eye yaw `eye`; returns `body_yaw`.
    pub fn turn(&mut self, eye: f32, moving: bool, dt: f32) -> f32 {
        self.render_yaw = self.feet_yaw;
        if self.reset {
            self.feet_yaw = eye;
            self.goal_yaw = eye;
            self.render_yaw = eye;
        }
        if moving || normalize(eye - self.feet_yaw).abs() > FEET_MAX_TORSO {
            self.goal_yaw = eye;
        }
        let diff = normalize(self.goal_yaw - self.feet_yaw);
        let a = diff.abs();
        let scale = (a / FEET_FADE).clamp(0.01, 1.0);
        let step = FEET_TURN_RATE * dt * scale;
        self.feet_yaw = if a <= step { self.goal_yaw } else { normalize(self.feet_yaw + step * diff.signum()) };
        // Never more than the twist's range behind the eyes.
        let twist = normalize(eye - self.feet_yaw);
        if twist.abs() > FEET_MAX_TORSO {
            self.feet_yaw = normalize(eye - FEET_MAX_TORSO * twist.signum());
        }
        normalize(eye - self.feet_yaw).clamp(-BODY_YAW, BODY_YAW)
    }

    /// The main sequence's state from the inputs (3, "Choosing").
    fn pick(&mut self, i: &Inputs, a: &Animator) -> State {
        let moving = i.velocity.length() > MOVING_SPEED;
        if i.jumped {
            self.jumping = true;
            return State::JumpStart;
        }
        if i.swimming {
            self.jumping = false;
            return State::Swim;
        }
        if !i.on_ground {
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

    /// A gesture of `activity` for the kind (none if the model has none).
    fn gesture(&mut self, set: &AnimSet, activity: &str, held: bool) {
        let Some(s) = set.activity(activity).filter(|&s| s > 0) else { return };
        // A new reload part ends the last one's.
        if held || activity.contains("RELOAD") {
            self.gestures.retain(|g| !g.held);
        }
        if let Some(g) = self.gestures.iter_mut().find(|g| g.sequence == s) {
            g.cycle = 0.0;
            g.weight = 1.0;
            return;
        }
        if self.gestures.len() < MAX_GESTURES {
            self.gestures.push(Gesture { sequence: s, cycle: 0.0, weight: 1.0, held });
        }
    }

    /// One update with the gun in hand `weapon`.
    pub fn step(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        let set = a.set.clone();
        let kind = weapon.map_or(Kind::Primary, |w| Kind::of_class(w, self.class.as_deref()));
        let k = kind.suffix();
        if self.reset || weapon != self.weapon.as_deref() || self.kind != Some(kind) {
            if self.reset {
                self.gestures.clear();
            }
            // Another gun: its reload's gestures stop with it.
            self.gestures.retain(|g| !g.held);
            self.weapon = weapon.map(str::to_string);
            self.kind = Some(kind);
            self.state = None;
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
            self.gesture(&set, &format!("ACT_MP_JUMP_LAND_{k}"), false);
        }
        self.airborne = !i.on_ground && !i.swimming;
        let state = self.pick(i, a);
        if self.state != Some(state) || a.main.is_none() {
            let s = set
                .activity(&format!("{}_{k}", state.activity()))
                .or_else(|| set.activity(&format!("{}_PRIMARY", state.activity())))
                .unwrap_or(0);
            if a.main != Some(s) && !set.sequences.is_empty() {
                a.play(s, now);
                if state == State::JumpStart {
                    a.cycle = 0.0;
                }
            }
            self.state = Some(state);
        }
        if i.fired {
            self.gesture(&set, &format!("ACT_MP_ATTACK_{stance}_{k}"), false);
        }
        if i.reloaded {
            let reload_stance = if !i.on_ground && !i.swimming { "AIRWALK" } else { stance };
            match i.reload_part {
                "_loop" => self.gesture(&set, &format!("ACT_MP_RELOAD_{reload_stance}_{k}_LOOP"), true),
                "_end" => self.gesture(&set, &format!("ACT_MP_RELOAD_{reload_stance}_{k}_END"), false),
                _ => self.gesture(&set, &format!("ACT_MP_RELOAD_{reload_stance}_{k}"), false),
            }
        }
        // The pose parameters (3, 4).
        let speed = i.velocity.length();
        let moving = speed > MOVING_SPEED;
        let body_yaw = self.turn(i.eye_yaw, moving, dt);
        let body_pitch = normalize(-i.eye_pitch).clamp(PITCH_DOWN, PITCH_UP);
        let mut mv = Vec2::ZERO;
        if moving {
            let d = normalize(i.velocity.y.atan2(i.velocity.x).to_degrees() - self.feet_yaw);
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
        self.pose = values;
        a.playback = 1.0;
        a.advance(dt, now);
        // Gestures: played once; a reload's loop holds its last frame
        // until the next round or the end.
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
/// drawn class when the gun can be it), on every machine.
fn follow_class(
    mut bodies: Query<(&mut Tf2PlayerAnim, Option<&Inventory>, Option<&super::classes::RungClass>)>,
    weapons: Query<&crate::weapon::Weapon>,
    table: Option<Res<super::classes::Classes>>,
) {
    let Some(table) = table else { return };
    for (mut anim, inv, drawn) in &mut bodies {
        let held = inv.and_then(|i| i.active).and_then(|w| weapons.get(w).ok());
        let class = held.and_then(|w| table.held(&w.id, drawn.map(|d| d.0.as_str()))).map(|c| c.name.as_str());
        if anim.class.as_deref() != class {
            anim.class = class.map(str::to_string);
        }
    }
}

/// TF2's bodies animated: drawn (`PlayerAnimPlugin`), on a server per tick
/// (`SimAnimPlugin`), sent to clients (`AnimNetPlugin`), and what they hold.
pub struct Tf2AnimPlugin;

impl Plugin for Tf2AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            source_engine::player_anim::PlayerAnimPlugin::<Tf2PlayerAnim>::default(),
            source_engine::player_anim::SimAnimPlugin::<Tf2PlayerAnim>::default(),
            source_engine::anim_net::AnimNetPlugin::<Tf2PlayerAnim>::new("tf2_anim"),
        ))
        .add_systems(Update, (hold_weapons, follow_class));
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

    /// 4, tests 13-14: the pitch's sign and clamp; the feet stay until the
    /// eyes pass 45 degrees standing.
    #[test]
    fn aim_and_feet() {
        let mut s = Tf2PlayerAnim::default();
        assert_eq!(s.turn(0.0, false, 0.015), 0.0);
        s.reset = false;
        let y = s.turn(30.0, false, 0.015);
        assert!((y - 30.0).abs() < 1e-3, "{y}");
        for _ in 0..200 {
            s.turn(80.0, false, 0.015);
        }
        assert!((s.feet_yaw - 80.0).abs() < 1.0, "turned to {}", s.feet_yaw);
        assert_eq!(normalize(89.0f32).clamp(PITCH_DOWN, PITCH_UP), 89.0);
        assert_eq!(normalize(-60.0f32).clamp(PITCH_DOWN, PITCH_UP), -45.0);
    }
}
