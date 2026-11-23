//! DoD:S's player animation (docs/plans/active/dods.md, "Body animation"),
//! on Source's shared half (`source_engine::player_anim`, as HL2:DM's and
//! TF2's): one main sequence and one gesture layer on each DoD:S body, from
//! the shared model's own activities (`models/player/dod_player_shared.mdl`,
//! specs/dods/install.md, "Models"). No spec covers DoD:S's animation
//! state; what is built is what the model's names and lengths answer, and
//! every choice beyond them is ours (Known shortcuts).
//!
//! - **Which set**: the activities end in the held gun's suffix
//!   (`ACT_DOD_STAND_IDLE_TOMMY`), from the weapon's data (`BODY_ANIM_HOOK`,
//!   `BodyAnim`: the script's `anim_prefix` as the model names it, and its
//!   `reload_anim_prefix` where the model has that reload); another game's
//!   gun, or a gun with none, plays the Thompson's (`FALLBACK`).
//! - **The main sequence** by stance and movement: going prone
//!   `ACT_GET_DOWN_STAND` (or `_CROUCH` when begun ducked) and getting up
//!   `ACT_GET_UP_STAND`, played over the stance's own time
//!   (`stance::GO_PRONE`, `GET_UP`; the files' 37 frames at 30 fps are the
//!   measured 1.2 s); prone deployed `ACT_DOD_PRONE_DEPLOY_<gun>`; prone
//!   `ACT_DOD_PRONE_AIM_<gun>` still and `ACT_DOD_PRONEWALK_IDLE_<gun>`
//!   crawling (sights up: `PRONE_ZOOM` and `PRONE_ZOOM_FORWARD`); in the air
//!   after a jump `ACT_HOP` until the ground; then `CROUCH` / `CROUCHWALK` /
//!   `STAND` / `WALK` / `RUN`, each `_IDLE_`, or with the sights up `_ZOOM_`
//!   where the model has it, else `_AIM_` (a gun with no idle, the knife,
//!   spade and grenades, plays its aim). Walk below the midpoint of the
//!   walk's and the run's ground speeds (`walk_or_run`), run above.
//! - **Pose parameters**: `move_x` / `move_y` on the 3 x 3 move grids and a
//!   crawl's rate by its speed over its ground speed (`move_grid`, TF2's),
//!   `body_pitch` from the eyes, `body_yaw` from the eyes against the feet,
//!   which follow them by TF2's rules (`turn_feet`, `FEET`); the main
//!   sequences' own aim-matrix autolayers add the aim, per gun set
//!   (`aimmatrix_idle_mg42` ...), the feet kept under them by the files'
//!   IK locks and the hands by their IK rules.
//! - **Gestures**, one layer: a shot or swing plays
//!   `ACT_DOD_PRIMARYATTACK_<gun>` (prone: `_PRONE_`, deployed:
//!   `_PRONE_DEPLOYED_`, crouched: `_CROUCH_` where the model has it), a
//!   reload `ACT_DOD_RELOAD_<reload>` (`_CROUCH_`, `_PRONE_`,
//!   `_PRONE_DEPLOYED_`), each from cycle 0 to its end.
//! - **Death**: the model's only death activity is `ACT_DIERAGDOLL`; a
//!   dead body is Chimera's ragdoll (`map::ragdoll`), and the state starts
//!   afresh on the next life.
//!
//! The animations place the guns' bones (`ValveBiped.weapon_bone`,
//! `weapon_bone_left`) in the hands, so the held gun (`bodies::
//! hold_weapons`) follows them. `follow` hands each state the player's
//! stance (`DodsStance`), the sights (`AltModes` past its first mode) and
//! the gun's `BodyAnim`, drawn and on a server's tick (`SimAnimState`); a
//! client poses others as the server's tick pose had them
//! (`source_engine::anim_net`).
//!
//! Known shortcuts (each needs a spec; docs/plans/active/dods.md, "Needs a
//! spec"):
//! - Idle against aim: the model has both (`ACT_DOD_STAND_IDLE_*`,
//!   `ACT_DOD_STAND_AIM_*`); ours plays the aim only with the sights up.
//! - Walk against run and the sprint set (`ACT_DOD_SPRINT_IDLE_*`): sprint
//!   is unmeasured (movement.md), so no sprint sequence plays; walk and run
//!   split at the midpoint of their ground speeds.
//! - The feet and the torso twist are TF2's rules (45 degrees, 720/s);
//!   the deployed yaw limits and `body_height` (the ledge height) are
//!   unmeasured: `body_height` stays at the file's default.
//! - The jump plays `ACT_HOP` from the take-off to the ground, with no
//!   landing; no hand signals (`ACT_DOD_HS_*`), the SMGs' stock strike
//!   (`ACT_DOD_SECONDARYATTACK_*`), the bazooka's shouldering or zoomed
//!   reload, the TNT's plant and defuse, or flinches.
//! - The MG42's deployed and reloading world models (`deployedmodel`,
//!   `reloadmodel`) aren't swapped in.

use bevy::prelude::*;
use serde::Deserialize;
use source_engine::player_anim::{FeetRules, Inputs, PlayerAnimRules, SimAnimState, move_grid, normalize, turn_feet};

use super::stance::{DodsStance, GET_UP, GO_PRONE, Stance};
use crate::map::anim::{AnimSet, Animator, Layer};
use chimera_sim::weapon::{AltModes, Inventory};

/// The weapon data's hook naming the body's animation set:
/// `[hooks."dods:body_anim"] suffix = "RIFLE", reload = "BAR"`.
pub const BODY_ANIM_HOOK: &str = "dods:body_anim";

/// The suffix a gun without a `BodyAnim` plays (the Thompson's: it holds
/// the gun on `weapon_bone`, as most guns of other games are held).
pub const FALLBACK: &str = "TOMMY";

/// Horizontal speed above which a body counts as moving (units/s).
pub const MOVING_SPEED: f32 = 0.5;

/// The feet's rules (TF2's, a stand-in: module doc).
pub const FEET: FeetRules = FeetRules {
    max_torso: 45.0,
    rate: 720.0,
    fade: 60.0,
    clamp: 45.0,
};

/// A gun's body animation set (`BODY_ANIM_HOOK`): the shared model's
/// activity suffix for everything (`TOMMY`) and, where its reload has
/// sequences of its own, the reload's (`BAR`).
#[derive(Component, Clone, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BodyAnim {
    pub suffix: String,
    #[serde(default)]
    pub reload: Option<String>,
}

/// What a body's player is doing beyond `Inputs` (`follow`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Doing {
    pub stance: Stance,
    /// Going prone from a ducked eye.
    pub from_crouch: bool,
    /// The seconds into going prone or getting up.
    pub elapsed: f32,
    pub deployed: bool,
    /// The sights or scope up.
    pub sighted: bool,
    /// The gun in hand's set (None: `FALLBACK`).
    pub anim: Option<BodyAnim>,
}

/// The main sequence's state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Main {
    Stand,
    Walk,
    Run,
    Crouch,
    CrouchWalk,
    Jump,
    GoingDown,
    GettingUp,
    Prone,
    Crawl,
    Deployed,
}

impl Main {
    /// Whether the sequence is a timed transition (played over the
    /// stance's own time).
    pub fn transition(self) -> bool {
        matches!(self, Main::GoingDown | Main::GettingUp)
    }
}

/// The gesture layer: a shot or a reload.
#[derive(Clone, Debug, PartialEq)]
pub struct Gesture {
    pub sequence: usize,
    pub cycle: f32,
}

/// One DoD:S body's animation state.
#[derive(Component, Debug, Clone)]
pub struct DodsPlayerAnim {
    reset: bool,
    snap: bool,
    snap_offset: f32,
    pub feet_yaw: f32,
    goal_yaw: f32,
    /// The render and hitbox yaw: the feet before this update's turn.
    pub render_yaw: f32,
    pub main: Option<Main>,
    /// The main sequence's activity as played.
    pub activity: String,
    /// In the air after a jump.
    jumping: bool,
    pub doing: Doing,
    pub gesture: Option<Gesture>,
    /// The gesture's activity.
    pub gesture_activity: String,
}

impl Default for DodsPlayerAnim {
    fn default() -> Self {
        Self {
            reset: true,
            snap: true,
            snap_offset: 0.0,
            feet_yaw: 0.0,
            goal_yaw: 0.0,
            render_yaw: 0.0,
            main: None,
            activity: String::new(),
            jumping: false,
            doing: Doing::default(),
            gesture: None,
            gesture_activity: String::new(),
        }
    }
}

/// The first of `asked` the model has, with its sequence.
fn first(set: &AnimSet, asked: &[String]) -> Option<(String, usize)> {
    asked.iter().find_map(|a| set.activity(a).map(|s| (a.clone(), s)))
}

/// Walk or run at `speed` (units/s) with suffix `s`: run above the
/// midpoint of the walk's and the run's forward ground speeds (ours).
pub fn walk_or_run(set: &AnimSet, s: &str, speed: f32) -> Main {
    let forward = |act: &str| {
        let seq = set
            .activity(&format!("ACT_DOD_{act}_IDLE_{s}"))
            .or_else(|| set.activity(&format!("ACT_DOD_{act}_AIM_{s}")))?;
        let mut p = set.default_params();
        if let (Some(x), Some(y)) = (set.param("move_x"), set.param("move_y")) {
            p[x] = set.params[x].encode(1.0);
            p[y] = set.params[y].encode(0.0);
        }
        Some(set.ground_motion(seq, &p))
    };
    match (forward("WALK"), forward("RUN")) {
        (Some(w), Some(r)) if speed < (w + r) / 2.0 => Main::Walk,
        (Some(_), None) => Main::Walk,
        _ => Main::Run,
    }
}

impl DodsPlayerAnim {
    fn suffix(&self) -> &str {
        self.doing.anim.as_ref().map_or(FALLBACK, |a| a.suffix.as_str())
    }

    /// The main state from the inputs and the stance.
    pub fn pick(&mut self, i: &Inputs, set: &AnimSet) -> Main {
        let moving = i.velocity.length() > MOVING_SPEED;
        match self.doing.stance {
            Stance::GoingDown => return Main::GoingDown,
            Stance::GettingUp => return Main::GettingUp,
            Stance::Prone if self.doing.deployed => return Main::Deployed,
            Stance::Prone => return if moving { Main::Crawl } else { Main::Prone },
            Stance::Up => {}
        }
        if i.jumped {
            self.jumping = true;
        }
        if self.jumping {
            if i.on_ground || i.swimming {
                self.jumping = false;
            } else {
                return Main::Jump;
            }
        }
        match (i.ducked, moving) {
            (true, true) => Main::CrouchWalk,
            (true, false) => Main::Crouch,
            (false, false) => Main::Stand,
            (false, true) => walk_or_run(set, self.suffix(), i.velocity.length()),
        }
    }

    /// The activities `main` asks for, best first: the gun's set, then
    /// the fallback's.
    pub fn asked(&self, main: Main) -> Vec<String> {
        let modes: &[&str] = if self.doing.sighted {
            &["ZOOM", "AIM", "IDLE"]
        } else {
            &["IDLE", "AIM"]
        };
        let mut out = Vec::new();
        let fixed = match main {
            Main::Jump => Some("ACT_HOP"),
            Main::GoingDown if self.doing.from_crouch => Some("ACT_GET_DOWN_CROUCH"),
            Main::GoingDown => Some("ACT_GET_DOWN_STAND"),
            Main::GettingUp => Some("ACT_GET_UP_STAND"),
            _ => None,
        };
        if let Some(f) = fixed {
            return vec![f.to_string()];
        }
        for s in [self.suffix(), FALLBACK] {
            match main {
                Main::Deployed => {
                    out.push(format!("ACT_DOD_PRONE_DEPLOY_{s}"));
                    out.push(format!("ACT_DOD_PRONE_AIM_{s}"));
                }
                Main::Prone => {
                    if self.doing.sighted {
                        out.push(format!("ACT_DOD_PRONE_ZOOM_{s}"));
                    }
                    out.push(format!("ACT_DOD_PRONE_AIM_{s}"));
                }
                Main::Crawl => {
                    if self.doing.sighted {
                        out.push(format!("ACT_DOD_PRONE_ZOOM_FORWARD_{s}"));
                    }
                    out.push(format!("ACT_DOD_PRONEWALK_IDLE_{s}"));
                    out.push(format!("ACT_DOD_PRONEWALK_AIM_{s}"));
                }
                _ => {
                    let stance = match main {
                        Main::Walk => "WALK",
                        Main::Run => "RUN",
                        Main::Crouch => "CROUCH",
                        Main::CrouchWalk => "CROUCHWALK",
                        _ => "STAND",
                    };
                    for m in modes {
                        out.push(format!("ACT_DOD_{stance}_{m}_{s}"));
                    }
                }
            }
        }
        out
    }

    /// The fire gesture's activities, best first.
    fn fire_asked(&self, ducked: bool) -> Vec<String> {
        let s = self.suffix();
        let mut out = Vec::new();
        match self.doing.stance {
            Stance::Prone if self.doing.deployed => {
                out.push(format!("ACT_DOD_PRIMARYATTACK_PRONE_DEPLOYED_{s}"));
                out.push(format!("ACT_DOD_PRIMARYATTACK_DEPLOYED_{s}"));
                out.push(format!("ACT_DOD_PRIMARYATTACK_PRONE_{s}"));
            }
            Stance::Prone | Stance::GoingDown | Stance::GettingUp => {
                out.push(format!("ACT_DOD_PRIMARYATTACK_PRONE_{s}"))
            }
            Stance::Up if ducked => out.push(format!("ACT_DOD_PRIMARYATTACK_CROUCH_{s}")),
            Stance::Up => {}
        }
        out.push(format!("ACT_DOD_PRIMARYATTACK_{s}"));
        out
    }

    /// The reload gesture's activities, best first: the reload's own set,
    /// then the gun's.
    fn reload_asked(&self, ducked: bool) -> Vec<String> {
        let s = self.suffix().to_string();
        let r = self.doing.anim.as_ref().and_then(|a| a.reload.clone());
        let mut out = Vec::new();
        for x in r.iter().chain(std::iter::once(&s)) {
            match self.doing.stance {
                Stance::Prone if self.doing.deployed => {
                    out.push(format!("ACT_DOD_RELOAD_PRONE_DEPLOYED_{x}"));
                    out.push(format!("ACT_DOD_RELOAD_DEPLOYED_{x}"));
                    out.push(format!("ACT_DOD_RELOAD_PRONE_{x}"));
                }
                Stance::Prone | Stance::GoingDown | Stance::GettingUp => out.push(format!("ACT_DOD_RELOAD_PRONE_{x}")),
                Stance::Up if ducked => out.push(format!("ACT_DOD_RELOAD_CROUCH_{x}")),
                Stance::Up => {}
            }
            out.push(format!("ACT_DOD_RELOAD_{x}"));
        }
        out
    }

    /// The feet toward the eyes (`turn_feet`); returns `body_yaw`.
    pub fn turn(&mut self, eye: f32, moving: bool, dt: f32) -> f32 {
        self.render_yaw = self.feet_yaw;
        if self.snap {
            let feet = normalize(eye + self.snap_offset);
            (self.feet_yaw, self.goal_yaw, self.render_yaw) = (feet, feet, feet);
            self.snap = false;
            self.snap_offset = 0.0;
        }
        turn_feet(&mut self.feet_yaw, &mut self.goal_yaw, eye, moving, dt, &FEET)
    }

    /// One update with the gun in hand `weapon` (its id).
    pub fn step(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        let set = a.set.clone();
        a.ik_locks = true;
        a.ik_rules = true;
        let _ = weapon;
        if self.reset {
            self.main = None;
            self.gesture = None;
            self.jumping = false;
            a.main = None;
            self.reset = false;
        }
        let main = self.pick(i, &set);
        let asked = self.asked(main);
        let found = first(&set, &asked);
        if let Some((act, s)) = found {
            let changed = act != self.activity || a.main.is_none();
            if changed {
                a.play(s, now);
                // A jump and a transition start at their first frame.
                if main == Main::Jump || main.transition() {
                    a.cycle = 0.0;
                }
                self.activity = act;
            }
        }
        self.main = Some(main);
        // Gestures: a shot (or swing, or throw) and a reload.
        let gesture = if i.fired || i.threw {
            Some(self.fire_asked(i.ducked))
        } else if i.reloaded {
            Some(self.reload_asked(i.ducked))
        } else {
            None
        };
        if let Some((act, s)) = gesture.and_then(|g| first(&set, &g)) {
            self.gesture = Some(Gesture {
                sequence: s,
                cycle: 0.0,
            });
            self.gesture_activity = act;
        }
        // The pose parameters by name.
        let speed3 = (i.velocity.length_squared() + i.vertical * i.vertical).sqrt();
        let body_yaw = self.turn(i.eye_yaw, speed3 > 1.0, dt);
        if let Some(p) = set.param("body_yaw") {
            a.set_param(p, body_yaw);
        }
        if let Some(p) = set.param("body_pitch") {
            a.set_param(p, normalize(-i.eye_pitch));
        }
        move_grid(a, i.velocity, i.eye_yaw, MOVING_SPEED);
        // The playback rate: a transition over the stance's time; a crawl
        // (one cell) by its speed over its ground speed; else 1.
        a.playback = 1.0;
        if let Some(m) = a.main {
            let length = set.duration(m);
            match main {
                Main::GoingDown if length > 0.0 => a.playback = length / GO_PRONE,
                Main::GettingUp if length > 0.0 => a.playback = length / GET_UP,
                Main::Crawl => {
                    let g = set.ground_motion(m, &a.params);
                    if g > 0.0 {
                        a.playback = i.velocity.length() / g;
                    }
                }
                _ => {}
            }
        }
        a.advance(dt, now);
        if let Some(g) = &mut self.gesture {
            g.cycle += dt * set.cycle_rate(g.sequence, &a.params);
            if g.cycle >= 1.0 {
                self.gesture = None;
                self.gesture_activity.clear();
            }
        }
        a.layers = self
            .gesture
            .iter()
            .map(|g| {
                Some(Layer {
                    sequence: g.sequence,
                    cycle: g.cycle,
                    weight: 1.0,
                })
            })
            .collect();
    }
}

impl PlayerAnimRules for DodsPlayerAnim {
    const GAME: &'static str = super::GAME;

    fn update(&mut self, a: &mut Animator, i: &Inputs, weapon: Option<&str>, dt: f32, now: f64) {
        self.step(a, i, weapon, dt, now);
    }

    fn feet_yaw(&self) -> f32 {
        self.render_yaw
    }

    fn dead(&mut self) {
        self.reset = true;
        self.gesture = None;
    }

    fn snap_feet(&mut self, offset: f32) {
        self.snap = true;
        self.snap_offset = offset;
    }
}

/// What the player is doing, from its stance and the gun in hand.
pub fn doing(stance: Option<&DodsStance>, sighted: bool, anim: Option<&BodyAnim>) -> Doing {
    let st = stance.copied().unwrap_or_default();
    Doing {
        stance: st.stance,
        from_crouch: st.from_eye < (super::movement::EYE_STAND + super::movement::EYE_DUCK) / 2.0,
        elapsed: st.elapsed,
        deployed: st.deployed,
        sighted,
        anim: anim.cloned(),
    }
}

/// Each DoD:S body's `Doing`, in the drawn state and a server's own.
#[allow(clippy::type_complexity)]
pub fn follow(
    mut bodies: Query<
        (
            Option<&mut DodsPlayerAnim>,
            Option<&mut SimAnimState<DodsPlayerAnim>>,
            Option<&DodsStance>,
            Option<&Inventory>,
        ),
        Or<(With<DodsPlayerAnim>, With<SimAnimState<DodsPlayerAnim>>)>,
    >,
    weapons: Query<(Option<&BodyAnim>, Option<&AltModes>)>,
) {
    for (drawn, sim, stance, inv) in &mut bodies {
        let (anim, alt) = inv
            .and_then(|i| i.active)
            .and_then(|w| weapons.get(w).ok())
            .unwrap_or((None, None));
        let d = doing(stance, alt.is_some_and(|m| m.current > 0), anim);
        if let Some(mut a) = drawn
            && a.doing != d
        {
            a.doing = d.clone();
        }
        if let Some(mut s) = sim
            && s.0.doing != d
        {
            s.0.doing = d;
        }
    }
}

/// DoD:S's bodies animated: drawn (`PlayerAnimPlugin`), on a server per
/// tick (`SimAnimPlugin`), sent to clients (`AnimNetPlugin`), each told
/// what its player is doing (`follow`).
pub struct DodsAnimPlugin;

impl Plugin for DodsAnimPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            source_engine::player_anim::PlayerAnimPlugin::<DodsPlayerAnim>::default(),
            source_engine::player_anim::SimAnimPlugin::<DodsPlayerAnim>::default(),
            source_engine::anim_net::AnimNetPlugin::<DodsPlayerAnim>::new("dods_anim"),
        ))
        .add_systems(Update, follow.before(source_engine::player_anim::PlayerAnimStates))
        .add_systems(FixedLast, follow.before(crate::map::SimPose));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gun_s_set_then_the_fallback() {
        let mut s = DodsPlayerAnim::default();
        s.doing.anim = Some(BodyAnim {
            suffix: "RIFLE".into(),
            reload: Some("BAR".into()),
        });
        assert_eq!(
            s.asked(Main::Stand),
            [
                "ACT_DOD_STAND_IDLE_RIFLE",
                "ACT_DOD_STAND_AIM_RIFLE",
                "ACT_DOD_STAND_IDLE_TOMMY",
                "ACT_DOD_STAND_AIM_TOMMY"
            ]
        );
        s.doing.sighted = true;
        assert_eq!(
            s.asked(Main::Prone)[..2],
            ["ACT_DOD_PRONE_ZOOM_RIFLE", "ACT_DOD_PRONE_AIM_RIFLE"]
        );
        assert_eq!(
            s.reload_asked(true)[..2],
            ["ACT_DOD_RELOAD_CROUCH_BAR", "ACT_DOD_RELOAD_BAR"]
        );
        s.doing.stance = Stance::Prone;
        s.doing.deployed = true;
        assert_eq!(s.fire_asked(false)[0], "ACT_DOD_PRIMARYATTACK_PRONE_DEPLOYED_RIFLE");
        assert_eq!(s.asked(Main::GoingDown), ["ACT_GET_DOWN_STAND"]);
    }
}
