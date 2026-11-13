//! What CS:S player bodies play: the player animation state of
//! specs/cs_source/animation.md §12 (the SDK template the CS:S data
//! follows) driving each character's `Animator`, on CS:S's bodies only:
//! CS:S's rules (`PlayerAnim`, sequences by the weapon's suffix) on
//! Source's shared half (`source_engine::player_anim`, lift S9: the
//! inputs, the drawn and per-tick server updates, the hands' IK locks).
//! The plugin also drives hostages (`hostage_anim`) and puts the held
//! weapon's world model in their hands (`hold_weapons`).
//!
//! Known shortcuts (docs/tech-debt.md): Player animation follows the SDK
//! template (spec §12): ground speeds 100/250/85 make feet slide against CS:S's
//! authored ≈107/150/67; no death layer; hand IK is our reading of the files'
//! IK rules (kinds 1 lock, 4 release; the last rule over the layers wins;
//! target the grip bones, a 0.2 s fade; no foot IK, no rule ramps); the grenade
//! layer picks `<Move>_Shoot_GREN1/2` by activity (the spec names
//! `idle_shoot_gren1/2`). Why it's OK for now: CS:S's own state isn't public;
//! check against a demo (spec open questions).

use bevy::prelude::*;
pub use source_engine::player_anim::{Inputs, normalize};
use source_engine::player_anim::{PlayerAnimRules, PlayerAnimSystems};

use crate::{
    map::anim::{AnimSet, Animator, Fading, Layer},
    weapon::{Inventory, Weapon},
};

const RUN_THRESHOLD: f32 = 175.0;
const MOVING_MIN_SPEED: f32 = 0.5;
const MAX_BODY_YAW: f32 = 90.0;
const FEET_YAW_RATE: f32 = 720.0;
const FEET_TURN_FADE: f32 = 45.0;
const FACE_FRONT_TIME: f64 = 3.0;
const JUMP_GROUND_IGNORE: f64 = 0.2;

/// Characters CS:S animates otherwise (hostages: `hostage_anim`).
type NotHostages = Without<crate::objectives::hostages::Hostage>;

pub struct PlayerAnimPlugin;

impl Plugin for PlayerAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(source_engine::player_anim::PlayerAnimPlugin::<PlayerAnim, NotHostages>::default())
            .init_resource::<HandIk>()
            .add_systems(
                Update,
                (
                    super::hostage_anim::drive.in_set(crate::map::DriveAnimation),
                    hold_weapons,
                    hands_off.after(PlayerAnimSystems).in_set(crate::map::DriveAnimation),
                ),
            );
        crate::console::resource_cvar::<HandIk, u8>(
            app,
            "chimera:hand_ik",
            "1: hands locked onto the weapon as the player models' IK rules say (drawn bodies); 0: off, the animation's own arms (for comparison).",
            |h| &mut h.0,
        );
    }
}

/// `chimera_hand_ik 0`: the drawn bodies' hands left to the animation.
fn hands_off(hand_ik: Res<HandIk>, mut bodies: Query<&mut Animator, With<PlayerAnim>>) {
    if hand_ik.0 == 0 {
        for mut a in &mut bodies {
            a.ik.clear();
        }
    }
}

/// `chimera_hand_ik`: drawn bodies lock their hands onto their weapon
/// (`PlayerAnim::lock_hands`); 0 turns it off.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HandIk(pub u8);

impl Default for HandIk {
    fn default() -> Self {
        Self(1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Activity {
    Idle,
    Walk,
    Run,
    CrouchIdle,
    CrouchWalk,
    Hop,
}

impl Activity {
    fn name(self) -> &'static str {
        match self {
            Activity::Idle => "ACT_IDLE",
            Activity::Walk => "ACT_WALK",
            Activity::Run => "ACT_RUN",
            Activity::CrouchIdle => "ACT_CROUCHIDLE",
            Activity::CrouchWalk => "ACT_RUN_CROUCH",
            Activity::Hop => "ACT_HOP",
        }
    }

    /// Full-pose speed (units/s) of the activity's sequence (§12.2).
    fn ground_speed(self) -> f32 {
        match self {
            Activity::Idle | Activity::Walk => 100.0,
            Activity::Run => 250.0,
            Activity::CrouchWalk => 85.0,
            _ => 0.0,
        }
    }
}

/// Upper-body sequences fading into each other (§12.6): (sequence, time
/// it became current).
#[derive(Default, Debug, Clone)]
struct Queue(Vec<(usize, f64)>);

impl Queue {
    /// Make `s` current; the two layers' (sequence, weight).
    fn update(&mut self, set: &AnimSet, s: usize, now: f64) -> [Option<(usize, f32)>; 2] {
        if self.0.last().is_none_or(|(last, _)| *last != s) {
            self.0.push((s, now));
        }
        loop {
            match self.0.as_slice() {
                [] => return [None, None],
                [(a, _)] => return [Some((*a, 1.0)), None],
                [(a, _), (b, since), ..] => {
                    let fade = Fading {
                        sequence: *a,
                        cycle: 0.0,
                        playback: 1.0,
                        stopped: *since,
                        fade: set.sequences[*a].fade_out.min(set.sequences[*b].fade_in),
                    };
                    let u = fade.weight(now);
                    if u <= 0.0 {
                        self.0.remove(0);
                        continue;
                    }
                    return [Some((*a, u)), Some((*b, 1.0 - u))];
                }
            }
        }
    }
}

/// One character's animation state.
#[derive(Component, Debug, Clone)]
#[component(storage = "SparseSet")]
pub struct PlayerAnim {
    /// The next update is the first (spawn, respawn).
    reset: bool,
    pub feet_yaw: f32,
    goal_yaw: f32,
    last_turn: f64,
    gait_yaw: f32,
    jumping: Option<f64>,
    restart: bool,
    pub activity: Activity,
    idle: Queue,
    moving: Queue,
    /// The fire layer: sequence and cycle.
    fire: Option<(usize, f32)>,
    /// The reload layer: sequence and cycle.
    reload: Option<(usize, f32)>,
    /// The grenade layer: sequence, cycle, and whether it is the throw.
    grenade: Option<(usize, f32, bool)>,
    /// A throw waits for the prime to reach its end.
    throw_wanted: bool,
    /// How far each hand (`HAND_CHAINS`) is locked onto its grip, 0 to 1.
    pub hand_locks: [f32; 2],
    /// The next lock update snaps (spawn).
    reset_hands: bool,
    /// The weapon suffix of the last update.
    suffix: &'static str,
}

impl Default for PlayerAnim {
    fn default() -> Self {
        Self {
            reset: true,
            feet_yaw: 0.0,
            goal_yaw: 0.0,
            last_turn: 0.0,
            gait_yaw: 0.0,
            jumping: None,
            restart: false,
            activity: Activity::Idle,
            idle: Queue::default(),
            moving: Queue::default(),
            fire: None,
            reload: None,
            grenade: None,
            throw_wanted: false,
            hand_locks: [0.0; 2],
            reset_hands: true,
            suffix: "",
        }
    }
}

/// What the state decided: pose parameters by name and the render yaw.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Outputs {
    pub move_x: f32,
    pub move_y: f32,
    pub body_yaw: f32,
    pub body_pitch: f32,
    pub rate: f32,
}

impl PlayerAnim {
    /// Feet yaw, torso twist (§12.4); returns `body_yaw`.
    pub fn turn(&mut self, eye: f32, moving: bool, dt: f32, now: f64) -> f32 {
        if self.reset {
            self.feet_yaw = eye;
            self.goal_yaw = eye;
            self.last_turn = 0.0;
        } else {
            if moving || now - self.last_turn > FACE_FRONT_TIME {
                self.goal_yaw = eye;
            } else {
                let e = normalize(self.goal_yaw - eye);
                if e.abs() > MAX_BODY_YAW {
                    self.goal_yaw += if e > 0.0 { -MAX_BODY_YAW } else { MAX_BODY_YAW };
                }
            }
            self.goal_yaw = normalize(self.goal_yaw);
            if self.feet_yaw != self.goal_yaw {
                let diff = normalize(self.goal_yaw - self.feet_yaw);
                let a = diff.abs();
                let scale = if a <= FEET_TURN_FADE {
                    (a / FEET_TURN_FADE).clamp(0.01, 1.0)
                } else {
                    1.0
                };
                let mut step = FEET_YAW_RATE * dt * scale;
                if a > MAX_BODY_YAW {
                    step = a - MAX_BODY_YAW;
                }
                self.feet_yaw = if a < step {
                    self.goal_yaw
                } else {
                    normalize(self.feet_yaw + step * diff.signum())
                };
                self.last_turn = now;
            }
        }
        self.reset = false;
        normalize(eye - self.feet_yaw)
    }

    /// The main activity (§12.1); also tracks jumping.
    fn pick(&mut self, i: &Inputs, speed: f32, now: f64) -> Activity {
        if i.jumped {
            self.jumping = Some(now);
            self.restart = true;
        }
        if let Some(start) = self.jumping
            && now - start > JUMP_GROUND_IGNORE
            && i.on_ground
        {
            self.jumping = None;
            self.restart = true;
        }
        let moving = speed > MOVING_MIN_SPEED;
        if self.jumping.is_some() {
            Activity::Hop
        } else if i.ducked {
            if moving {
                Activity::CrouchWalk
            } else {
                Activity::CrouchIdle
            }
        } else if moving {
            if speed > RUN_THRESHOLD {
                Activity::Run
            } else {
                Activity::Walk
            }
        } else {
            Activity::Idle
        }
    }

    /// One update (§ "Per-frame order") for a body with weapon animation
    /// `suffix`.
    pub fn update(&mut self, a: &mut Animator, i: &Inputs, suffix: &'static str, dt: f32, now: f64) -> Outputs {
        let set = a.set.clone();
        // Another weapon drawn: the old one's shot and reload gestures stop
        // with it (switching cancels a reload, weapons.md T11); they were
        // played out on the new weapon's arms, up to the reload's 2 s
        // (playtest 2026-10-10: others' weapon draw looked wrong). The
        // grenade's throw plays out: the thrower draws the next weapon
        // while it does.
        if suffix != self.suffix {
            self.fire = None;
            self.reload = None;
            self.suffix = suffix;
        }
        let speed = i.velocity.length();
        let moving = speed > MOVING_MIN_SPEED;
        // Main sequence.
        let activity = self.pick(i, speed, now);
        if let Some(s) = set.activity(activity.name()) {
            if activity != self.activity || a.main.is_none() {
                a.play(s, now);
            }
            if self.restart {
                a.cycle = 0.0;
            }
        }
        self.restart = false;
        self.activity = activity;
        let ground = activity.ground_speed();
        let rate = if !moving {
            1.0
        } else if ground < 0.001 {
            0.01
        } else {
            (speed / ground).clamp(0.01, 10.0)
        };
        // Pose parameters.
        let body_pitch = normalize(i.eye_pitch).clamp(-90.0, 90.0);
        let body_yaw = self.turn(i.eye_yaw, moving, dt, now);
        if moving {
            self.gait_yaw = i.velocity.y.atan2(i.velocity.x).to_degrees();
        }
        let d = normalize(-(i.eye_yaw - self.gait_yaw)).to_radians();
        let (move_x, move_y) = if moving {
            (d.cos() * rate, -d.sin() * rate)
        } else {
            (0.0, 0.0)
        };
        for (name, v) in [
            ("move_x", move_x),
            ("move_y", move_y),
            ("body_yaw", body_yaw),
            ("body_pitch", body_pitch),
        ] {
            if let Some(p) = set.param(name) {
                a.set_param(p, v);
            }
        }
        a.advance(dt, now);
        let cycle = a.cycle;
        // Upper-body layers 1-4, phase-locked to the legs.
        a.layers.resize(7, None);
        let seq = |name: String| set.sequence(&name);
        let idle_name = if activity == Activity::CrouchIdle {
            format!("Crouch_Idle_Upper_{suffix}")
        } else {
            format!("Idle_Upper_{suffix}")
        };
        let put = |slot: usize, pair: [Option<(usize, f32)>; 2], scale: f32, layers: &mut Vec<Option<Layer>>| {
            for (k, e) in pair.into_iter().enumerate() {
                layers[slot + k] = e.map(|(sequence, w)| Layer {
                    sequence,
                    cycle,
                    weight: (w * scale).clamp(0.0, 1.0),
                });
            }
        };
        let mut layers = std::mem::take(&mut a.layers);
        match seq(idle_name) {
            Some(s) => put(0, self.idle.update(&set, s, now), 1.0, &mut layers),
            None => put(0, [None, None], 1.0, &mut layers),
        }
        let moving_name = match activity {
            Activity::Run => Some(format!("Run_Upper_{suffix}")),
            Activity::Walk => Some(format!("Walk_Upper_{suffix}")),
            Activity::CrouchWalk => Some(format!("Crouch_Walk_Upper_{suffix}")),
            _ => None,
        };
        match moving_name.filter(|_| moving).and_then(seq) {
            Some(s) => put(2, self.moving.update(&set, s, now), rate, &mut layers),
            None => {
                self.moving = Queue::default();
                put(2, [None, None], 1.0, &mut layers);
            }
        }
        let others: f32 = layers[1..4].iter().flatten().map(|l| l.weight).sum();
        if let Some(l) = &mut layers[0] {
            l.weight = (1.0 - others).max(0.0);
        }
        // Layers under a later full-weight one change nothing visible.
        if let Some(full) = (0..4).rev().find(|k| layers[*k].is_some_and(|l| l.weight > 0.99)) {
            layers[..full].fill(None);
        }
        // Fire layer (order 5).
        if i.fire_stopped && !i.fired {
            self.fire = None;
        } else if i.fired {
            let name = match activity {
                Activity::Run => "Run_Shoot_",
                Activity::Walk => "Walk_Shoot_",
                Activity::CrouchIdle => "Crouch_Idle_Shoot_",
                Activity::CrouchWalk => "Crouch_Walk_Shoot_",
                _ => "Idle_Shoot_",
            };
            // Dual pistols shoot from either hand (`<Move>_Shoot_ELITES_L`
            // and `_R`, spec 14).
            self.fire = set
                .sequence(&format!("{name}{suffix}{}", i.shot_side))
                .or_else(|| set.sequence(&format!("{name}{suffix}")))
                .map(|s| (s, 0.0));
        } else if let Some((s, c)) = &mut self.fire {
            *c += set.cycle_rate(*s, &a.params) * dt;
            if *c > 1.0 {
                self.fire = None;
            }
        }
        layers[4] = self.fire.map(|(sequence, cycle)| Layer {
            sequence,
            cycle,
            weight: 1.0,
        });
        // Reload layer (order 6). The spec's generic `reload_<suffix>`
        // doesn't exist in CS:S's data; it has `<Move>_Reload_<suffix>`
        // by the main activity instead (spec 12, Open questions).
        if i.reloaded {
            let name = match activity {
                Activity::Run => "Run_Reload_",
                Activity::Walk => "Walk_Reload_",
                Activity::CrouchIdle => "Crouch_Idle_Reload_",
                Activity::CrouchWalk => "Crouch_Walk_Reload_",
                _ => "Idle_Reload_",
            };
            // Shotguns load shell by shell: `<Move>_reload_<X>_start`,
            // `_loop` per shell, `_end` (spec 14).
            let part = i.reload_part;
            self.reload = set
                .sequence(&format!("{name}{suffix}{part}"))
                .or_else(|| set.sequence(&format!("Reload_{suffix}{part}")))
                .or_else(|| set.sequence(&format!("Idle_Reload_{suffix}{part}")))
                .map(|s| (s, 0.0));
        } else if let Some((s, c)) = &mut self.reload {
            *c += set.cycle_rate(*s, &a.params) * dt;
            if *c > 1.0 {
                self.reload = None;
            }
        }
        layers[5] = self.reload.map(|(sequence, cycle)| Layer {
            sequence,
            cycle,
            weight: 1.0,
        });
        // Grenade layer (order 7; specs/cs_source/grenades.md 8): the
        // prime (`<Move>_Shoot_GREN1`) plays and holds at its end while the
        // pin is out; once it has reached its end a release plays the throw
        // (`<Move>_Shoot_GREN2`) out. Always the grenade suffix: the
        // thrower may hold something else by then.
        self.throw_wanted |= i.threw;
        let gren = |n: u8| {
            let name = match activity {
                Activity::Run => "Run_Shoot_GREN",
                Activity::Walk => "Walk_Shoot_GREN",
                Activity::CrouchIdle => "Crouch_Idle_Shoot_GREN",
                Activity::CrouchWalk => "Crouch_Walk_Shoot_GREN",
                _ => "Idle_Shoot_GREN",
            };
            set.sequence(&format!("{name}{n}"))
                .or_else(|| set.sequence(&format!("Idle_Shoot_GREN{n}")))
        };
        self.grenade = match self.grenade {
            Some((s, c, true)) => {
                let c = c + set.cycle_rate(s, &a.params) * dt;
                (c <= 1.0).then_some((s, c, true))
            }
            prime if i.primed || self.throw_wanted => {
                let (s, c) = match prime {
                    Some((s, c, false)) => (s, (c + set.cycle_rate(s, &a.params) * dt).min(1.0)),
                    _ => match gren(1) {
                        Some(s) => (s, 0.0),
                        None => (usize::MAX, 1.0),
                    },
                };
                if self.throw_wanted && c >= 1.0 {
                    self.throw_wanted = false;
                    gren(2).map(|s| (s, 0.0, true))
                } else {
                    (s != usize::MAX).then_some((s, c, false))
                }
            }
            _ => None,
        };
        layers[6] = self.grenade.map(|(sequence, cycle, _)| Layer {
            sequence,
            cycle,
            weight: 1.0,
        });
        a.layers = layers;
        self.lock_hands(a, suffix, dt);
        Outputs {
            move_x,
            move_y,
            body_yaw,
            body_pitch,
            rate,
        }
    }
}

impl PlayerAnim {
    /// The hands' IK locks from the files' rules (`player_anim::
    /// lock_hands`). Guns' hand poses lock both hands to the weapon, the
    /// knife's release them, dual pistols lock the right hand only;
    /// reloads lock them to the grips as they move. Grenades hang on the
    /// hand itself and their grips aren't where the hands are (the right
    /// one 7.7 units and 162 degrees off): no locks.
    fn lock_hands(&mut self, a: &mut Animator, suffix: &str, dt: f32) {
        source_engine::player_anim::lock_hands(a, &mut self.hand_locks, self.reset_hands, suffix != "GREN", dt);
        self.reset_hands = false;
    }
}

impl PlayerAnimRules for PlayerAnim {
    const GAME: &'static str = super::GAME;

    fn update(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        PlayerAnim::update(self, a, i, suffix(weapon), dt, now);
    }

    fn feet_yaw(&self) -> f32 {
        self.feet_yaw
    }

    fn dead(&mut self) {
        self.reset = true;
    }

    /// Dual pistols shoot from either hand.
    fn shot_side(w: &Weapon, m: Option<&crate::weapon::Magazine>) -> &'static str {
        match m {
            Some(m) if super::weapons::alternate_hands(w.id) => {
                if super::weapons::elite_right_hand(m.clip) {
                    "_R"
                } else {
                    "_L"
                }
            }
            _ => "",
        }
    }
}

/// The weapon's player animation suffix (`PlayerAnimationExtension`).
pub fn suffix(weapon: Option<&str>) -> &'static str {
    let Some(id) = weapon else { return "Pistol" };
    let name = id.rsplit(['_', ':']).next().unwrap_or(id);
    match name {
        "ak47" => "AK",
        "aug" => "AUG",
        "awp" => "AWP",
        "c4" => "C4",
        "elite" => "ELITES",
        "famas" => "FAMAS",
        "g3sg1" => "G3",
        "galil" => "GALIL",
        "m249" => "M249",
        "m3" => "M3S90",
        "m4a1" => "M4",
        "mac10" => "MAC10",
        "mp5navy" => "MP5",
        "p90" => "P90",
        "scout" => "SCOUT",
        "sg550" => "SG550",
        "sg552" => "SG552",
        "tmp" => "TMP",
        "ump45" => "UMP45",
        "xm1014" => "XM1014",
        "knife" => "KNIFE",
        "hegrenade" | "flashbang" | "smokegrenade" => "GREN",
        _ => "PISTOL",
    }
}

/// Whether `body` is one of CS:S's models (or of no game in particular).
fn ours(models: Option<&crate::map::CharacterModels>, body: Option<&crate::map::BodyModel>) -> bool {
    models.is_none_or(|m| m.is_of(body, super::GAME))
}

/// Characters hold their active weapon's world model (in CS:S's bodies).
#[allow(clippy::type_complexity)]
fn hold_weapons(
    characters: Query<
        (Entity, Option<&Inventory>, Option<&crate::map::Held>, Option<&crate::map::BodyModel>),
        (With<Animator>, Without<crate::objectives::hostages::Hostage>),
    >,
    weapons: Query<(&Weapon, Option<&crate::weapon::AltModes>)>,
    models: Option<Res<crate::map::CharacterModels>>,
    mut commands: Commands,
) {
    for (e, inventory, held, body) in &characters {
        if !ours(models.as_deref(), body) {
            continue;
        }
        // With the silencer on, its own world model.
        let id = inventory
            .and_then(|i| i.active)
            .and_then(|w| weapons.get(w).ok())
            .map(|(w, modes)| {
                let silenced = modes.is_some_and(|m| m.current == 1)
                    && super::weapons::data(w.id).is_some_and(|d| d.models.world_alt.is_some());
                if silenced {
                    super::weapons::silenced_key(w.id)
                } else {
                    w.id.to_string()
                }
            });
        if held.is_none_or(|h| h.0 != id) {
            commands.entity(e).insert(crate::map::Held(id));
        }
    }
}

/// A network server animates CS:S's bodies again in the fixed tick
/// (`source_engine::player_anim::SimAnimPlugin`), and its clients pose
/// other players' bodies from that (`source_engine::anim_net`).
pub struct SimAnimPlugin;

impl Plugin for SimAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(source_engine::player_anim::SimAnimPlugin::<PlayerAnim, NotHostages>::default());
        // Its clients pose others' bodies as that pose was at the tick
        // they draw them at.
        if !app.is_plugin_added::<source_engine::anim_net::AnimNetPlugin<PlayerAnim>>() {
            app.add_plugins(source_engine::anim_net::AnimNetPlugin::<PlayerAnim>::new("cs_source_anim"));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn turned(state: &mut PlayerAnim, eye: f32, moving: bool, now: f64) -> f32 {
        state.turn(eye, moving, 0.01, now)
    }

    #[test]
    fn feet_follow_the_eyes() {
        let mut s = PlayerAnim::default();
        assert_eq!(turned(&mut s, 30.0, false, 0.0), 0.0);
        assert_eq!(s.feet_yaw, 30.0);

        let fresh = || PlayerAnim {
            reset: false,
            last_turn: 1.0,
            ..default()
        };
        let mut s = fresh();
        assert_eq!(turned(&mut s, 30.0, false, 1.5), 30.0);
        assert_eq!(s.feet_yaw, 0.0);

        let mut s = fresh();
        let twist = turned(&mut s, 120.0, false, 1.5);
        assert_eq!(s.goal_yaw, 90.0);
        assert!((s.feet_yaw - 7.2).abs() < 1e-4, "{}", s.feet_yaw);
        assert!((twist - 112.8).abs() < 1e-4);

        let mut s = fresh();
        let twist = turned(&mut s, 100.0, true, 1.5);
        assert!((s.feet_yaw - 10.0).abs() < 1e-4, "{}", s.feet_yaw);
        assert!((twist - 90.0).abs() < 1e-4);
        turned(&mut s, 100.0, true, 1.51);
        assert!((s.feet_yaw - 17.2).abs() < 1e-4, "{}", s.feet_yaw);

        let mut s = fresh();
        turned(&mut s, 10.0, true, 1.5);
        assert!((s.feet_yaw - 1.6).abs() < 1e-4, "{}", s.feet_yaw);

        let mut s = fresh();
        turned(&mut s, 50.0, false, 4.5);
        assert_eq!(s.goal_yaw, 50.0);
        assert!((s.feet_yaw - 7.2).abs() < 1e-4, "{}", s.feet_yaw);
    }

    #[test]
    fn activities_from_speed() {
        let mut s = PlayerAnim::default();
        let at = |s: &mut PlayerAnim, v: Vec2, ducked: bool| {
            let i = Inputs {
                velocity: v,
                ducked,
                on_ground: true,
                ..default()
            };
            s.pick(&i, v.length(), 0.0)
        };
        assert_eq!(at(&mut s, Vec2::new(0.0, 250.0), false), Activity::Run);
        assert_eq!(at(&mut s, Vec2::new(130.0, 0.0), false), Activity::Walk);
        assert_eq!(at(&mut s, Vec2::new(42.5, 0.0), true), Activity::CrouchWalk);
        assert_eq!(at(&mut s, Vec2::ZERO, true), Activity::CrouchIdle);
        assert_eq!(at(&mut s, Vec2::ZERO, false), Activity::Idle);
    }

    #[test]
    fn suffixes() {
        assert_eq!(suffix(Some("cs_source:weapon_ak47")), "AK");
        assert_eq!(suffix(Some("cs_source:weapon_knife")), "KNIFE");
        assert_eq!(suffix(Some("cs_source:weapon_usp")), "PISTOL");
        assert_eq!(suffix(Some("cs_source:weapon_glock")), "PISTOL");
        assert_eq!(suffix(Some("cs_source:weapon_deagle")), "PISTOL");
        assert_eq!(suffix(Some("cs_source:weapon_m4a1")), "M4");
        assert_eq!(suffix(Some("cs_source:weapon_awp")), "AWP");
        assert_eq!(suffix(None), "Pistol");
    }
}
