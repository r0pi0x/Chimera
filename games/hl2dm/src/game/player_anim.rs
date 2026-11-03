//! HL2:DM's player animation state (specs/hl2dm/player_models.md 3-8) on
//! Source's shared half (`source_engine::player_anim`, lift S9): one main
//! sequence from five activities (idle, run, crouch idle, crouch walk,
//! jump; no walk and no swim) translated by the weapon's kind from a fixed
//! table per weapon (`Kind::of`, §3; the script's `anim_prefix` plays no
//! part), restarted at cycle 0 on every change (§4); played at the
//! horizontal speed over the main sequence's authored ground speed, with
//! no clamp (§5); the legs' `move_yaw` from the gait yaw (§6.1),
//! `aim_pitch` from the eyes (§6.2), the feet planted until the view turns
//! past 45 degrees or 3 s pass, then turning at 360 degrees/s, slower in
//! the last 45 (§6.3), the rest of the upper body in `aim_yaw`; fire and
//! reload gestures on overlay layers (§8: cycle 0, weight 1 until done,
//! removed on the next advance, at most 15); the autoplay head and spine
//! sequences on every frame (§7 step 5). Pose parameters aren't networked:
//! each side runs this on its own values (`PlayerAnimPlugin` for the drawn
//! bodies, `SimAnimPlugin` for a server's hitboxes, which use the main
//! sequence alone, §7 step 3).
//!
//! `Hl2AnimPlugin` adds both for HL2:DM's bodies, and the client's head
//! and eyes (`look`: §6.4, the nearest player in front within 128 units)
//! and the held weapon's world model (`hold_weapons`).
//!
//! Known shortcuts: the server's state uses the command's own eye angles
//! where the game takes the previous command's (§6.2, §6.3: a tick's lag
//! of the hitboxes' aim); a weapon of another game in an HL2:DM body
//! (a party's melee) plays the SMG1's kind.

use bevy::prelude::*;
use source_engine::player_anim::{Inputs, PlayerAnimRules, normalize};

use crate::{
    map::anim::{AnimSet, Animator, Layer},
    weapon::Inventory,
};

/// Horizontal speed above which the playback rate follows the speed
/// (units/s, §5).
pub const MOVING_SPEED: f32 = 0.5;
/// 3D speed above which the feet follow the eyes at once (units/s).
pub const MOVING_SPEED_FEET: f32 = 1.0;
/// Feet turn rate (degrees/s; `mp_feetyawrate` is not used).
pub const FEET_TURN_RATE: f32 = 360.0;
/// Below this remaining angle the feet turn slower in proportion.
pub const FEET_FADE: f32 = 45.0;
/// Standing: a torso twist above this turns the feet to the eyes.
pub const FEET_MAX_TORSO: f32 = 45.0;
/// `aim_yaw`'s clamp before the pose range.
pub const AIM_YAW_CLAMP: f32 = 60.0;
/// `mp_facefronttime`: still this long after the last turn, the feet
/// face the eyes (s).
pub const FACE_FRONT_TIME: f64 = 3.0;
/// Overlay layers a body may have (§8).
pub const MAX_GESTURES: usize = 15;

/// A weapon's kind: the suffix of the activities it translates to (§3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Pistol,
    Shotgun,
    Smg1,
    Ar2,
    Gravgun,
    Grenade,
    Rpg,
    Crossbow,
    Melee,
    Slam,
}

impl Kind {
    /// The kind of weapon `id` (`hl2dm:weapon_pistol`) by the fixed table
    /// (§3); another game's weapon plays the SMG1's.
    pub fn of(id: &str) -> Kind {
        let name = id.rsplit(':').next().unwrap_or(id).trim_start_matches("weapon_");
        match name {
            "pistol" | "357" => Kind::Pistol,
            "shotgun" => Kind::Shotgun,
            "smg1" => Kind::Smg1,
            "ar2" => Kind::Ar2,
            "physcannon" => Kind::Gravgun,
            "frag" => Kind::Grenade,
            "rpg" => Kind::Rpg,
            "crossbow" => Kind::Crossbow,
            "crowbar" | "stunstick" => Kind::Melee,
            "slam" => Kind::Slam,
            _ => Kind::Smg1,
        }
    }

    /// The activities' suffix (`ACT_HL2MP_IDLE_PHYSGUN`).
    pub fn suffix(self) -> &'static str {
        match self {
            Kind::Pistol => "PISTOL",
            Kind::Shotgun => "SHOTGUN",
            Kind::Smg1 => "SMG1",
            Kind::Ar2 => "AR2",
            Kind::Gravgun => "PHYSGUN",
            Kind::Grenade => "GRENADE",
            Kind::Rpg => "RPG",
            Kind::Crossbow => "CROSSBOW",
            Kind::Melee => "MELEE",
            Kind::Slam => "SLAM",
        }
    }

    /// Whether a shot plays the fire gesture (the crossbow and the gravity
    /// gun never ask for one).
    pub fn fire_gesture(self) -> bool {
        !matches!(self, Kind::Crossbow | Kind::Gravgun)
    }

    /// Whether a reload plays the reload gesture (the shotgun's shell by
    /// shell reload never asks; the rest have no sequence).
    pub fn reload_gesture(self) -> bool {
        matches!(self, Kind::Pistol | Kind::Smg1 | Kind::Ar2)
    }
}

/// The untranslated activities of the main sequence (§4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ideal {
    Idle,
    Run,
    CrouchIdle,
    CrouchWalk,
    Jump,
}

impl Ideal {
    fn activity(self) -> &'static str {
        match self {
            Ideal::Idle => "ACT_HL2MP_IDLE",
            Ideal::Run => "ACT_HL2MP_RUN",
            Ideal::CrouchIdle => "ACT_HL2MP_IDLE_CROUCH",
            Ideal::CrouchWalk => "ACT_HL2MP_WALK_CROUCH",
            Ideal::Jump => "ACT_HL2MP_JUMP",
        }
    }
}

/// A request to the main sequence (§4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Request {
    IdleOrMoving,
    Jump,
}

/// An overlay layer (a fire or reload gesture, §8).
#[derive(Clone, Debug, PartialEq)]
pub struct Gesture {
    pub sequence: usize,
    pub cycle: f32,
    pub weight: f32,
}

/// One HL2:DM body's animation state.
#[derive(Component, Debug, Clone)]
pub struct Hl2PlayerAnim {
    /// The next update starts afresh (spawn, a new model).
    reset: bool,
    /// The feet yaw F and its goal Γ (degrees).
    pub feet_yaw: f32,
    pub goal_yaw: f32,
    /// The render and hitbox yaw: F before this update's turn (§6.3).
    pub render_yaw: f32,
    /// When the view or the feet last turned; None: moving.
    last_turn: Option<f64>,
    last_eye_yaw: f32,
    /// The upper body's twist (`aim_yaw` before its clamps).
    pub torso: f32,
    pub gait_yaw: f32,
    /// The untranslated activity; None: cleared (spawn, weapon switch).
    pub ideal: Option<Ideal>,
    /// The playback rate for the next advance (§5).
    pub rate: f32,
    /// The weapon of the last update.
    weapon: Option<String>,
    pub gestures: Vec<Gesture>,
}

impl Default for Hl2PlayerAnim {
    fn default() -> Self {
        Self {
            reset: true,
            feet_yaw: 0.0,
            goal_yaw: 0.0,
            render_yaw: 0.0,
            last_turn: None,
            last_eye_yaw: 0.0,
            torso: 0.0,
            gait_yaw: 0.0,
            ideal: None,
            rate: 1.0,
            weapon: None,
            gestures: Vec::new(),
        }
    }
}

impl Hl2PlayerAnim {
    /// The feet and the torso twist for eye yaw `eye` (§6.3); `speed3`:
    /// the 3D speed.
    pub fn turn(&mut self, eye: f32, speed3: f32, dt: f32, now: f64) {
        self.render_yaw = self.feet_yaw;
        if speed3 > MOVING_SPEED_FEET {
            self.last_turn = None;
            self.feet_yaw = eye;
            self.goal_yaw = eye;
            self.torso = 0.0;
            return;
        }
        let mut t = match self.last_turn {
            None => {
                self.last_eye_yaw = eye;
                self.feet_yaw = eye;
                self.goal_yaw = eye;
                now
            }
            Some(t) => t,
        };
        if eye != self.last_eye_yaw {
            t = now;
            self.last_eye_yaw = eye;
        }
        if self.goal_yaw != self.feet_yaw {
            t = now;
        }
        // Converge: the fade uses the gap before normalizing (a quirk
        // the spec keeps).
        let d = normalize(self.goal_yaw - self.feet_yaw);
        let a = (self.goal_yaw - self.feet_yaw).abs();
        let scale = if a <= FEET_FADE { (a / FEET_FADE).clamp(0.01, 1.0) } else { 1.0 };
        let step = FEET_TURN_RATE * dt * scale;
        self.feet_yaw = if d.abs() < step {
            self.goal_yaw
        } else {
            normalize(self.feet_yaw + step * d.signum())
        };
        let delta = normalize(eye - self.feet_yaw);
        if delta.abs() > FEET_MAX_TORSO || now > t + FACE_FRONT_TIME {
            self.goal_yaw = eye;
            t = now;
        }
        self.torso = delta;
        self.last_turn = Some(t);
    }

    /// The gait yaw for velocity `v` (§6.1) and `move_yaw` against body
    /// yaw `body`.
    pub fn gait(&mut self, v: Vec2, body: f32, dt: f32) -> f32 {
        if v == Vec2::ZERO {
            let e = normalize(body - self.gait_yaw);
            let k = if dt < 0.25 { 4.0 * dt } else { dt };
            self.gait_yaw = (self.gait_yaw + e * k) % 360.0;
        } else {
            self.gait_yaw = v.y.atan2(v.x).to_degrees();
        }
        normalize(-(body - self.gait_yaw))
    }

    /// Handle request `r` (§4 steps 1-5).
    fn request(&mut self, a: &mut Animator, set: &AnimSet, r: Request, i: &Inputs, kind: Option<Kind>, now: f64, tick: bool) {
        let (r, speed) = if i.frozen { (Request::IdleOrMoving, 0.0) } else { (r, i.velocity.length()) };
        let ideal = match r {
            Request::Jump => Ideal::Jump,
            Request::IdleOrMoving if !i.on_ground && self.ideal == Some(Ideal::Jump) => Ideal::Jump,
            Request::IdleOrMoving => match (i.ducked, speed > 0.0) {
                (true, true) => Ideal::CrouchWalk,
                (true, false) => Ideal::CrouchIdle,
                (false, true) => Ideal::Run,
                (false, false) => Ideal::Idle,
            },
        };
        self.ideal = Some(ideal);
        let base = ideal.activity();
        let s = kind
            .and_then(|k| set.activity(&format!("{base}_{}", k.suffix())))
            .or_else(|| set.activity(base))
            .unwrap_or(0);
        if a.main == Some(s) || set.sequences.is_empty() {
            return;
        }
        a.play(s, now);
        if tick {
            // The server's bones use the current main sequence only.
            a.fading.clear();
        }
        a.cycle = 0.0;
        a.playback = 1.0;
        self.rate = 1.0;
    }

    /// A gesture for `activity`: an active one playing it restarts, else a
    /// new layer if the model has a sequence for it (§8).
    fn gesture(&mut self, set: &AnimSet, activity: &str) {
        let Some(s) = set.activity(activity).filter(|&s| s > 0) else { return };
        if let Some(g) = self.gestures.iter_mut().find(|g| g.sequence == s && g.weight > 0.0) {
            g.cycle = 0.0;
            return;
        }
        if self.gestures.len() < MAX_GESTURES {
            self.gestures.push(Gesture {
                sequence: s,
                cycle: 0.0,
                weight: 1.0,
            });
        }
    }

    /// One update with the weapon's kind `kind` (none: no weapon); `tick`:
    /// a server's per-tick update.
    pub fn step(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64, tick: bool) {
        let set = a.set.clone();
        let kind = weapon.map(Kind::of);
        let still = i.velocity == Vec2::ZERO;
        // Spawn or a weapon switch: the main sequence cleared, one request
        // from the state (§4).
        if self.reset || weapon != self.weapon.as_deref() {
            if self.reset {
                self.gestures.clear();
                self.last_turn = None;
            }
            self.weapon = weapon.map(str::to_string);
            self.ideal = None;
            a.main = None;
            let r = if still || i.on_ground || i.swimming { Request::IdleOrMoving } else { Request::Jump };
            self.request(a, &set, r, i, kind, now, tick);
            self.reset = false;
        }
        if i.jumped {
            self.request(a, &set, Request::Jump, i, kind, now, tick);
        }
        if let Some(k) = kind {
            if i.fired && k.fire_gesture() {
                self.gesture(&set, &format!("ACT_HL2MP_GESTURE_RANGE_ATTACK_{}", k.suffix()));
            }
            if i.threw && k == Kind::Grenade {
                self.gesture(&set, "ACT_HL2MP_GESTURE_RANGE_ATTACK_GRENADE");
            }
            let whole = i.reload_part.is_empty();
            if i.reloaded && whole && k.reload_gesture() {
                self.gesture(&set, &format!("ACT_HL2MP_GESTURE_RELOAD_{}", k.suffix()));
            }
        }
        // The post-think request: none while moving in the air.
        if still || i.on_ground || i.swimming {
            self.request(a, &set, Request::IdleOrMoving, i, kind, now, tick);
        }
        // Frame advance: the main cycle at the last rate, the gestures.
        a.playback = self.rate;
        a.advance(dt, now);
        self.gestures.retain(|g| g.weight > 0.0);
        for g in &mut self.gestures {
            g.cycle += dt * set.cycle_rate(g.sequence, &a.params);
            if g.cycle >= 1.0 {
                g.weight = 0.0;
            }
        }
        a.layers = self
            .gestures
            .iter()
            .map(|g| {
                Some(Layer {
                    sequence: g.sequence,
                    cycle: g.cycle,
                    weight: g.weight,
                })
            })
            .collect();
        // The pose parameters (§6) and the next playback rate (§5).
        let body = normalize(i.eye_yaw);
        let move_yaw = self.gait(i.velocity, body, dt);
        let mut pitch = i.eye_pitch;
        if pitch > 180.0 {
            pitch -= 360.0;
        }
        let speed3 = i.velocity.extend(i.vertical).length();
        self.turn(body, speed3, dt, now);
        let aim_yaw = self.torso.clamp(-AIM_YAW_CLAMP, AIM_YAW_CLAMP);
        for (name, v) in [("move_yaw", move_yaw), ("aim_pitch", pitch.clamp(-90.0, 90.0)), ("aim_yaw", aim_yaw)] {
            if let Some(p) = set.param(name) {
                a.set_param(p, v);
            }
        }
        let speed = i.velocity.length();
        let ground = a.main.map_or(0.0, |s| set.ground_motion(s, &a.params));
        self.rate = if speed > MOVING_SPEED && ground > 0.0 { speed / ground } else { 1.0 };
        a.playback = self.rate;
    }
}

impl PlayerAnimRules for Hl2PlayerAnim {
    const GAME: &'static str = super::GAME;
    const AUTOPLAY: bool = true;

    fn update(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        self.step(a, i, weapon, dt, now, false);
    }

    fn update_tick(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        self.step(a, i, weapon, dt, now, true);
    }

    fn feet_yaw(&self) -> f32 {
        self.render_yaw
    }

    fn dead(&mut self) {
        self.reset = true;
        self.gestures.clear();
    }
}

/// Characters hold their active weapon's world model.
fn hold_weapons(
    characters: Query<(Entity, Option<&Inventory>, Option<&crate::map::Held>, Option<&crate::map::BodyModel>), With<Hl2PlayerAnim>>,
    weapons: Query<&crate::weapon::Weapon>,
    mut commands: Commands,
) {
    for (e, inventory, held, _) in &characters {
        let id = inventory
            .and_then(|i| i.active)
            .and_then(|w| weapons.get(w).ok())
            .map(|w| w.id.to_string());
        if held.is_none_or(|h| h.0 != id) {
            commands.entity(e).insert(crate::map::Held(id));
        }
    }
}

/// HL2:DM's bodies animated: drawn (`PlayerAnimPlugin`), on a server per
/// tick (`SimAnimPlugin`), the client's heads and eyes (`look`), and what
/// they hold.
pub struct Hl2AnimPlugin;

impl Plugin for Hl2AnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            source_engine::player_anim::PlayerAnimPlugin::<Hl2PlayerAnim>::default(),
            source_engine::player_anim::SimAnimPlugin::<Hl2PlayerAnim>::default(),
        ))
        .add_systems(
            Update,
            (
                hold_weapons,
                super::look::look
                    .after(source_engine::player_anim::PlayerAnimSystems)
                    .in_set(crate::map::DriveAnimation),
            ),
        );
    }
}
