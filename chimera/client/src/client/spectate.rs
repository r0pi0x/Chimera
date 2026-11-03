//! Spectating while dead, as CS:S does it: a death cam turning to your
//! killer (or looking down at your body), then the freeze cam or, without
//! one, `DEATH_CAM_ALONE` after the death, the camera follows a living
//! player. Jump cycles the mode (first person
//! through the target's eyes with its view model, a chase camera orbited
//! with the mouse, free roaming), attack and attack2 the next and previous
//! target; `mp_forcecamera 1` keeps you on your own team while it has
//! anyone alive. The spectator's HUD is the game's front end's (CS:S's
//! bars from its `spectator.res`, or a plain panel:
//! `games::cs_source::ui::spectator_hud`); in first person the target's crosshair and scope (`hud.rs`). Back to
//! the eye when you live again (respawn, new round). A game may have its
//! own dead view instead (`core::OrbitDeadView`, HL2:DM's): its own
//! ragdoll from behind along the view until it lives again.
//!
//! Under rules that respawn the dead on a timer (`core::RespawnDue`: a
//! minigame's `respawn.delay`, 2 s in the gun game) the death cam and the
//! freeze cam are fitted into the wait (`fit_to_respawn`: at most 40 % of
//! it death cam, 15 % travel, the rest the held picture), so the picture
//! is reached and seen before the respawn; without one (plain CS:S) the
//! timings below stand.
//!
//! Client-side only: it reads the simulation and moves the local camera.
//! The state machine (`Spectator::step`) is pure; `SpectateStatePlugin`
//! runs it without a window (tests), `SpectatePlugin` adds the camera
//! and input.
//!
//! Spectating while dead or on the spectator team (CS:S style;
//! `core::Spectating`, joined with `spectate`/`jointeam 1` in `rules`: no death
//! cam, starts flying free, anyone may be watched, `spec_player <name>` picks
//! one): `Spectator` (a pure state machine: death cam on the killer or your
//! body for `DEATH_CAM_SECONDS`, then, killed by someone still alive, the
//! freeze cam (`FreezeCamSettings`: `cl_disablefreezecam`, the server's
//! replicated `spec_freeze_traveltime` 0.7 s and `spec_freeze_time` 5 s; the
//! camera zooms toward the killer and holds), else the death cam until
//! `DEATH_CAM_ALONE` (6.5 s), then the first allowed living player in order
//! (specs/cs_source/spectating.md); jump / `spec_mode
//! [4\|5\|6]` cycles first person, chase and free look; attack/attack2 /
//! `spec_next`/`spec_prev` the target; a dead target lingers
//! `TARGET_DEATH_HOLD` (3 s) then the next; back to the eye on living again),
//! `mp_forcecamera` (`ForceCamera`: 1 only teammates, none once all are dead;
//! 2, a game's own: teammates, anyone once all are dead),
//! `SpecView` (the camera pose `follow_eye` uses; the first-person target,
//! whose body is hidden, whose zoom the camera takes and whose view model the
//! anchor draws through `map::ViewModelSource`), the spectator bars (the
//! front end's `spectator_hud`) in the
//! game's layout (`GameMenus::spectator`, CS:S's `spectator.res`: black top and
//! bottom bars across the window, team scores, "Map: de_dust2" and the round
//! clock at the top right, the target's "name (health)" in its team colour at
//! the bottom, the mode where `bottomspectator.res`'s view list sits;
//! `SpectatorBarsUp`: while they show, the HUD hides its round clock and money
//! and puts the death notices under the top bar), else a plain bottom panel
//! ("Spectating: Bot 3 [100]", weapon, mode; "Killed by ..." in the death cam);
//! in first person the crosshair and scope overlay are the target's (`hud.rs`);
//! the radar follows the target; `SpectateStatePlugin` runs the state without a
//! window (tests), `SpectateCameraPlugin` the camera's pose too.
//!
//! Known shortcuts (docs/tech-debt.md): Spectating (`client::spectate`) is
//! measured only in part (specs/cs_source/spectating.md, from bots in a
//! SourceTV demo: the 6.5 s without a freeze cam, the 3 s on a dead target,
//! the first target in order not the nearest, no enemies under
//! `mp_forcecamera 1`); the rest is general CS:S knowledge: the death cam's 2 s
//! before the freeze cam, its rise (1.4 m back, 0.9 m up over 0.8 s) and look;
//! our order is by name (CS:S's by entity index, the first the next after
//! the watcher's own: measured 2026-10-18); with no teammate left the camera
//! holds where it was (`SpecMode::Fixed`; CS:S's sits at the last watched
//! player's eye where it died); chase distance 96 units,
//! starting 15 degrees above the target's view; free look stays allowed under
//! `mp_forcecamera 1`; the mode is kept across deaths, starting in first
//! person; no auto-director (its menu entry greyed) or HLTV modes; the map overview (`client::overview`, `overview_mode`): its
//! boxes' places and sizes (small: 192 x 144 at the top left under the top bar;
//! large: between the bars), the defaults (off; not turning, names, health and
//! tracks on), zoom 1 fitting the whole picture, zooming toward the watched
//! player, the dots, health bars and 12 track points every 0.5 s are ours (no
//! capture), and the radar hides while it shows. The spectator team
//! (`rules::join_spectators`): a spectator starts flying free (no spec or
//! capture says CS:S's starting mode; kept), may watch anyone (`mp_forcecamera`
//! only binds the dead on a team), no death is counted when a living player
//! joins it, the bomb and kit drop and the other weapons go; free look is
//! client-side (the server keeps the body where it left play); the spectator
//! menu (duck) draws `bottomspectator.res`'s controls with the GameUI dialogs'
//! widgets and scheme (CS:S's are the client scheme's), its lists open upward,
//! a submenu beside its entry, a ticked toggle marked with a check sign; there
//! is no "join a team" in it (M opens the team menu, as in CS:S). In first
//! person the target's view model has no bob or sway (`ViewModelOffset` is the
//! local player's only) and no sniper scope overlay or crosshair; flash and
//! blast senses stay the local player's. Why it's OK for now: Plays like CS:S's
//! spectator; a spec session can pin the numbers.

use avian3d::prelude::*;
use bevy::{input::mouse::AccumulatedMouseMotion, prelude::*, window::CursorOptions};

use super::view::{CameraMode, FreeCam, camera_offset};
use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Dead, Died, Health, Intent, LocalPlayer, MovementState, Npc, Spectating, Team},
    map::{
        ViewModelAnchor, ViewModelSource,
        interp::{EyeView, RenderedView},
        ragdoll::Ragdoll,
    },
    weapon::{ViewPunch, Zoomed},
};

/// Seconds the death cam shows your killer before the freeze cam takes
/// over (a guess: specs/cs_source/spectating.md, open questions).
pub const DEATH_CAM_SECONDS: f64 = 2.0;
/// Seconds from the death to spectating when no freeze cam follows (the
/// killer dead or none, `cl_disablefreezecam 1`): CS:S's bots' 6.48 to
/// 6.54 s (specs/cs_source/spectating.md, measured on a SourceTV demo).
pub const DEATH_CAM_ALONE: f64 = 6.5;
/// `spec_freeze_traveltime` and `spec_freeze_time` defaults, seconds: the
/// CS:S dedicated server's (measured 2026-10-15), server cvars replicated
/// to clients (`core::SpectatorRules`, `console::SERVER_NAMES`).
pub use crate::core::{FREEZE_HOLD, FREEZE_TRAVEL};
use crate::core::{FREEZE_TRAVEL_MIN, RespawnDue, SimClock, SpectatorRules};
/// How far from the killer's eye the freeze cam stops, CS:S units (at
/// most; nearer when the death cam was nearer). A guess.
const FREEZE_DISTANCE: f32 = 128.0;
const UNIT: f32 = 0.0254;
/// Seconds the camera stays on a target that died before moving on:
/// CS:S's 3.0 s (200 ticks; specs/cs_source/spectating.md, measured).
pub const TARGET_DEATH_HOLD: f64 = 3.0;
/// Chase camera distance behind the target's eye, CS:S units (a guess
/// near CS:S's).
const CHASE_DISTANCE: f32 = 96.0;
/// Chase camera's starting pitch below the target's view, degrees.
const CHASE_PITCH: f32 = -15.0;
/// The death cam's rise: back from the body (away from the killer) and up,
/// meters, and how long it takes to get there, seconds.
const DEATH_CAM_BACK: f32 = 1.4;
const DEATH_CAM_UP: f32 = 0.9;
const DEATH_CAM_MOVE: f32 = 0.8;
/// Radius of the sphere swept to keep spectator cameras out of walls.
const CAMERA_RADIUS: f32 = 0.2;
const PITCH_LIMIT: f32 = 89f32.to_radians();

/// How the camera follows the target.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SpecMode {
    /// Through the target's eyes, with its view angles and view model.
    #[default]
    InEye,
    /// Behind the target, orbited with the mouse.
    Chase,
    /// Flying free (WASD where you look, shift faster).
    Roaming,
    /// Held where the camera was, no target: the dead on a team with
    /// nobody left to watch under `mp_forcecamera 1` (CS:S's mode 3,
    /// measured 2026-10-18: also without a round end). Never chosen.
    Fixed,
}

impl SpecMode {
    /// The next mode on jump, as CS:S cycles them.
    pub fn next(self) -> Self {
        match self {
            SpecMode::InEye => SpecMode::Chase,
            SpecMode::Chase => SpecMode::Roaming,
            SpecMode::Roaming | SpecMode::Fixed => SpecMode::InEye,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            SpecMode::InEye => "First Person",
            SpecMode::Chase => "Chase Camera",
            SpecMode::Roaming => "Free Look",
            SpecMode::Fixed => "Fixed View",
        }
    }

    /// `spec_mode` numbers, CS:S's observer modes: 4 in-eye, 5 chase, 6
    /// roaming.
    pub fn from_number(n: u32) -> Option<Self> {
        match n {
            4 => Some(SpecMode::InEye),
            5 => Some(SpecMode::Chase),
            6 => Some(SpecMode::Roaming),
            _ => None,
        }
    }

    pub fn number(self) -> u32 {
        match self {
            SpecMode::InEye => 4,
            SpecMode::Chase => 5,
            SpecMode::Roaming => 6,
            SpecMode::Fixed => 3,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum SpecPhase {
    /// Playing: the camera is at your eye.
    #[default]
    Alive,
    /// Just died: looking at the killer (or your body) since `since`.
    DeathCam { since: f64, killer: Option<Entity> },
    /// Then CS:S's freeze cam (`cl_disablefreezecam 0`, killed by someone
    /// alive): since `since` the camera travels toward the killer for
    /// `travel` seconds, then the picture holds still `hold` seconds.
    FreezeCam {
        since: f64,
        killer: Entity,
        travel: f32,
        hold: f32,
    },
    /// Following a target (or roaming).
    Watching,
}

/// A living character that could be watched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Candidate {
    pub entity: Entity,
    pub team: Option<Team>,
    pub position: Vec3,
}

/// What the local player is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Me {
    pub dead: bool,
    /// Killed (a `Died`), not just put out of play (team switch): a death
    /// cam first.
    pub killed: bool,
    pub killer: Option<Entity>,
    pub team: Option<Team>,
    pub position: Vec3,
    /// The freeze cam's travel and hold, seconds; None: disabled.
    pub freeze: Option<(f32, f32)>,
    /// On the spectator team (`core::Spectating`): watching starts free
    /// (no death cam), and anyone may be watched (`team` is None).
    pub spectator: bool,
    /// Seconds until the rules respawn you on a timer (`core::RespawnDue`:
    /// a minigame's `respawn.delay`); None: no timed respawn, CS:S's own
    /// timings. The death cam and freeze cam are fitted into the wait
    /// (`fit_to_respawn`).
    pub respawn_in: Option<f64>,
}

/// Spectator requests from keys and console commands.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SpecInput {
    pub next_mode: bool,
    pub set_mode: Option<SpecMode>,
    /// +1 next target, -1 previous.
    pub target_step: i32,
    /// Watch this one (`spec_player`, the spectator menu's list), when it
    /// may be watched.
    pub pick: Option<Entity>,
}

/// The local player's spectator state.
#[derive(Resource, Clone, Debug, Default)]
pub struct Spectator {
    pub phase: SpecPhase,
    /// Kept across deaths, as CS:S keeps your last observer mode.
    pub mode: SpecMode,
    pub target: Option<Entity>,
    /// When the target was last seen no longer watchable (dead).
    target_lost: Option<f64>,
    /// Who may be watched now, in cycling order (the spectator menu's
    /// list).
    pub watchable: Vec<Entity>,
    /// Requests waiting for the next step.
    pub pending: SpecInput,
    /// Dead on a team whose players are all dead under `mp_forcecamera`:
    /// nobody to watch, the camera held (`SpecMode::Fixed`).
    pub stranded: bool,
    /// This death's death cam length, seconds (`DEATH_CAM_SECONDS`, or
    /// shorter fitted to a timed respawn); 0 before the first.
    pub death_cam_length: f64,
}

/// A timed respawn's wait (`budget`, seconds from the death) fitted with
/// the death view: the death cam takes at most `FIT_DEATH_CAM` of it
/// (never more than `DEATH_CAM_SECONDS`), the freeze cam's travel at most
/// `FIT_TRAVEL` (never more than its own), and the picture holds for what
/// is left (never more than its own hold), so the freeze is reached and
/// seen before the respawn. A wait as long as the whole view (7.7 s with
/// CS:S's defaults) changes nothing. Returns the death cam's length and
/// the freeze cam's travel and hold (None stays None: disabled).
pub fn fit_to_respawn(budget: f64, freeze: Option<(f32, f32)>) -> (f64, Option<(f32, f32)>) {
    let budget = budget.max(0.0);
    let death_cam = DEATH_CAM_SECONDS.min(budget * FIT_DEATH_CAM);
    let freeze = freeze.map(|(travel, hold)| {
        let travel = travel.min((budget * FIT_TRAVEL) as f32).max(FREEZE_TRAVEL_MIN);
        let left = (budget - death_cam) as f32 - travel;
        (travel, hold.min(left.max(0.0)))
    });
    (death_cam, freeze)
}

/// The shares of a timed respawn's wait the death cam and the freeze
/// cam's travel may take (ours: with a 2 s wait 0.8 s of death cam, the
/// whole of its rise, 0.3 s of travel and 0.9 s of held picture).
pub const FIT_DEATH_CAM: f64 = 0.4;
pub const FIT_TRAVEL: f64 = 0.15;

/// The player's own freeze cam setting: `cl_disablefreezecam`
/// (Multiplayer > Advanced "Disable freeze cam", default 0). Its timings
/// are the server's (`core::SpectatorRules`: `spec_freeze_traveltime`,
/// `spec_freeze_time`), as is `mp_forcecamera`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct FreezeCamSettings {
    pub disabled: u8,
}

impl FreezeCamSettings {
    /// Travel and hold under the server's `rules`, None when disabled.
    pub fn timing(&self, rules: &SpectatorRules) -> Option<(f32, f32)> {
        (self.disabled == 0).then_some((rules.freeze_travel.max(FREEZE_TRAVEL_MIN), rules.freeze_hold.max(0.0)))
    }
}

/// The candidates you may watch: everyone living, or with `force` 1 (and
/// a team) only your living teammates, none once they are all dead (CS:S
/// put its dead on no one rather than an enemy: specs/cs_source/
/// spectating.md); with `force` 2 your living teammates, everyone once
/// they are all dead (Combat Arms' rounds: specs/combat_arms/
/// game_modes.md M2).
pub fn allowed(living: &[Candidate], team: Option<Team>, force: u8) -> Vec<Entity> {
    let mates: Vec<Entity> = living
        .iter()
        .filter(|c| force == 0 || team.is_none() || c.team == team)
        .map(|c| c.entity)
        .collect();
    if force == 2 && mates.is_empty() {
        return living.iter().map(|c| c.entity).collect();
    }
    mates
}

/// The target `step` places from `current` in `allowed` (wrapping); from
/// none (or one not in the list) the first or, going back, the last.
pub fn cycle(allowed: &[Entity], current: Option<Entity>, step: i32) -> Option<Entity> {
    let n = allowed.len() as i32;
    if n == 0 {
        return None;
    }
    let i = match current.and_then(|c| allowed.iter().position(|e| *e == c)) {
        Some(i) => (i as i32 + step).rem_euclid(n),
        None if step < 0 => n - 1,
        None => 0,
    };
    Some(allowed[i as usize])
}

impl Spectator {
    pub fn active(&self) -> bool {
        self.phase != SpecPhase::Alive
    }

    /// The camera mode in effect: free roaming without a target, the chase
    /// camera on a target that just died.
    pub fn effective_mode(&self) -> SpecMode {
        match self.target {
            None if self.stranded && self.mode != SpecMode::Roaming => SpecMode::Fixed,
            None => SpecMode::Roaming,
            Some(_) if self.target_lost.is_some() && self.mode == SpecMode::InEye => SpecMode::Chase,
            Some(_) => self.mode,
        }
    }

    /// Advance the state by a frame at `now` (seconds): `living` are the
    /// characters that could be watched, in cycling order.
    pub fn step(&mut self, now: f64, me: &Me, living: &[Candidate], force: u8, input: SpecInput) {
        if !me.dead {
            self.phase = SpecPhase::Alive;
            self.target = None;
            self.target_lost = None;
            return;
        }
        if self.phase == SpecPhase::Alive {
            self.target = None;
            self.target_lost = None;
            // Joining the spectators: flying free where you were (ours;
            // CS:S's starting mode not checked).
            if me.spectator {
                self.mode = SpecMode::Roaming;
            }
            self.phase = if me.killed && !me.spectator {
                SpecPhase::DeathCam {
                    since: now,
                    killer: me.killer,
                }
            } else {
                SpecPhase::Watching
            };
        }
        if let SpecPhase::DeathCam { since, killer } = self.phase {
            // A timed respawn's wait (from the death) fits the view.
            let (length, freeze) = match me.respawn_in {
                Some(left) => fit_to_respawn(left + (now - since), me.freeze),
                None => (DEATH_CAM_SECONDS, me.freeze),
            };
            self.death_cam_length = length;
            // The death cam plays out; keys wait for it.
            if now - since < length {
                return;
            }
            // The freeze cam, on a killer still alive; without one the
            // death cam lasts CS:S's `DEATH_CAM_ALONE`.
            let alive = |k: Entity| living.iter().any(|c| c.entity == k);
            match (freeze, killer.filter(|k| alive(*k))) {
                (Some((travel, hold)), Some(killer)) => {
                    self.phase = SpecPhase::FreezeCam {
                        since: now,
                        killer,
                        travel,
                        hold,
                    };
                    return;
                }
                _ if now - since < DEATH_CAM_ALONE => return,
                _ => self.phase = SpecPhase::Watching,
            }
        }
        if let SpecPhase::FreezeCam {
            since, travel, hold, ..
        } = self.phase
        {
            if now - since < (travel + hold) as f64 {
                return;
            }
            self.phase = SpecPhase::Watching;
        }
        if let Some(m) = input.set_mode {
            self.mode = m;
        } else if input.next_mode {
            self.mode = self.mode.next();
        }
        let allowed = allowed(living, me.team, force);
        self.watchable.clone_from(&allowed);
        self.stranded = allowed.is_empty() && force != 0 && me.team.is_some() && !me.spectator;
        match self.target {
            Some(t) if allowed.contains(&t) => self.target_lost = None,
            Some(_) => {
                // Gone (dead, or not allowed now): linger, then move on.
                let lost = *self.target_lost.get_or_insert(now);
                if now - lost >= TARGET_DEATH_HOLD || input.target_step != 0 {
                    self.target = None;
                    self.target_lost = None;
                }
            }
            None => {}
        }
        if let Some(p) = input.pick.filter(|p| allowed.contains(p)) {
            self.target = Some(p);
            self.target_lost = None;
            // Picking someone to watch while flying free follows them.
            if self.mode == SpecMode::Roaming {
                self.mode = SpecMode::InEye;
            }
        } else if input.target_step != 0 {
            self.target = cycle(&allowed, self.target, input.target_step).or(self.target);
            self.target_lost = None;
        } else if self.target.is_none() {
            // The first in order, not the nearest (CS:S's: the lowest
            // entity index; ours: by name).
            self.target = allowed.first().copied();
        }
    }
}

/// The spectator camera's result for this frame: where the local camera
/// goes (world space) and whose eyes it looks through.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct SpecView {
    pub pose: Option<(Vec3, Quat)>,
    /// The target watched in first person (its body hidden, its view model
    /// and zoom shown).
    pub in_eye: Option<Entity>,
}

/// The state machine and its console commands, without a window.
pub struct SpectateStatePlugin;

impl Plugin for SpectateStatePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Spectator>()
            // The server's rules (their cvars are the game rules': CS:S's
            // in `games::cs_source::rules`); defaults without them.
            .init_resource::<SpectatorRules>()
            .init_resource::<KilledBy>()
            .init_resource::<FreezeCamSettings>()
            .add_systems(Update, (remember_killer, track).chain().in_set(SpectateSet));
        resource_cvar::<FreezeCamSettings, u8>(
            app,
            "cl_disablefreezecam",
            "1: no freeze cam after you are killed (the death cam goes straight to spectating).",
            |f| &mut f.disabled,
        );
        app.world_mut()
            .resource_mut::<crate::console::Console>()
            .archive("cl_disablefreezecam");
        app.console_command(
            "spec_mode",
            "spec_mode [4|5|6]: next spectator mode, or first person (4), chase (5), free look (6).",
            |w, a| {
                let mut s = w.resource_mut::<Spectator>();
                match a.first() {
                    None => s.pending.next_mode = true,
                    Some(n) => {
                        let m = n
                            .parse()
                            .ok()
                            .and_then(SpecMode::from_number)
                            .ok_or("spec_mode 4|5|6 (first person, chase, free look)")?;
                        s.pending.set_mode = Some(m);
                    }
                }
                Ok(None)
            },
        )
        .console_command("spec_next", "Spectate the next player.", |w, _| {
            w.resource_mut::<Spectator>().pending.target_step = 1;
            Ok(None)
        })
        .console_command("spec_prev", "Spectate the previous player.", |w, _| {
            w.resource_mut::<Spectator>().pending.target_step = -1;
            Ok(None)
        })
        .console_command("spec_player", "spec_player <name>: spectate that player.", |w, a| {
            let want = a.join(" ");
            if want.is_empty() {
                return Err("spec_player <name>".into());
            }
            let found = w
                .query_filtered::<(Entity, &Name), (With<Intent>, Without<LocalPlayer>, Without<Npc>)>()
                .iter(w)
                .find(|(_, n)| n.as_str().eq_ignore_ascii_case(&want))
                .map(|(e, _)| e)
                .ok_or_else(|| format!("no player called {want}"))?;
            w.resource_mut::<Spectator>().pending.pick = Some(found);
            Ok(None)
        });
    }
}

/// The spectator systems (state, then camera), before `follow_eye`.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SpectateSet;

/// The state and where the camera goes (`SpecView`), without a window
/// (tests).
pub struct SpectateCameraPlugin;

impl Plugin for SpectateCameraPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpectateStatePlugin)
            .init_resource::<SpecView>()
            .init_resource::<ChaseOrbit>()
            .add_systems(
                Update,
                (place_camera, show_through_eyes).chain().after(track).in_set(SpectateSet),
            );
    }
}

/// State, keys and camera; a game's front end draws the spectator's HUD
/// (CS:S's: `games::cs_source::ui::spectator_hud`).
pub struct SpectatePlugin;

impl Plugin for SpectatePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(SpectateCameraPlugin).add_systems(
            Update,
            (keys.before(super::input::grab_cursor).before(track),),
        );
    }
}

/// Who killed the local player last (from `Died`), until it lives again.
#[derive(Resource, Default)]
struct KilledBy(Option<Option<Entity>>);

fn remember_killer(
    mut died: MessageReader<Died>,
    local: Option<Single<(Entity, Has<Dead>), With<LocalPlayer>>>,
    mut killed: ResMut<KilledBy>,
    mut was_dead: Local<bool>,
) {
    let Some(local) = local else {
        died.clear();
        return;
    };
    let (me, dead) = *local;
    // Forget the killer on living again (`Died` may come a tick before
    // `Dead` is on the player).
    if *was_dead && !dead {
        killed.0 = None;
    }
    *was_dead = dead;
    for d in died.read() {
        if d.entity == me {
            killed.0 = Some(d.attacker.filter(|a| *a != me));
        }
    }
}

type Living<'a> = (
    Entity,
    &'a Transform,
    Option<&'a Team>,
    Option<&'a Name>,
    Option<&'a Health>,
    Has<Dead>,
);

#[allow(clippy::type_complexity)]
fn track(
    time: Res<Time>,
    (due, clock): (Option<Res<RespawnDue>>, Option<Res<SimClock>>),
    local: Option<Single<(&Transform, Option<&Team>, Has<Dead>, Has<Spectating>), With<LocalPlayer>>>,
    characters: Query<Living, (With<Intent>, Without<LocalPlayer>, Without<Npc>)>,
    killed: Res<KilledBy>,
    rules: Res<SpectatorRules>,
    freeze: Res<FreezeCamSettings>,
    mut spec: ResMut<Spectator>,
) {
    let Some(local) = local else { return };
    let (at, team, dead, spectator) = *local;
    let me = Me {
        dead,
        killed: killed.0.is_some(),
        killer: killed.0.flatten(),
        team: team.copied().filter(|_| !spectator),
        position: at.translation,
        freeze: freeze.timing(&rules),
        spectator,
        respawn_in: due
            .and_then(|d| d.0)
            .zip(clock)
            .map(|(d, c)| (d.at - c.now).max(0.0)),
    };
    let mut living: Vec<(String, Candidate)> = characters
        .iter()
        .filter(|(.., health, dead)| !dead && health.is_none_or(|h| h.current > 0.0))
        .map(|(e, t, team, name, ..)| {
            (
                name.map_or_else(String::new, |n| n.as_str().to_string()),
                Candidate {
                    entity: e,
                    team: team.copied(),
                    position: t.translation,
                },
            )
        })
        .collect();
    // A stable order to cycle in: by name ("Bot 2" before "Bot 10").
    living.sort_by(|(a, ca), (b, cb)| natural(a).cmp(&natural(b)).then(ca.entity.cmp(&cb.entity)));
    let living: Vec<Candidate> = living.into_iter().map(|(_, c)| c).collect();
    let input = std::mem::take(&mut spec.pending);
    spec.step(time.elapsed_secs_f64(), &me, &living, rules.force_camera, input);
}

/// A sort key that orders the numbers in names by value.
pub fn natural(name: &str) -> (String, u64) {
    let digits: String = name.chars().rev().take_while(|c| c.is_ascii_digit()).collect();
    let n = digits.chars().rev().collect::<String>().parse().unwrap_or(0);
    (name[..name.len() - digits.len()].to_string(), n)
}

/// Jump cycles the mode, attack and attack2 the target (while the mouse
/// is captured, so the click that captures it doesn't count).
fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    cursor: Single<&CursorOptions>,
    console: Res<crate::console::Console>,
    mut spec: ResMut<Spectator>,
) {
    if spec.phase != SpecPhase::Watching || !super::input::cursor_grabbed(&cursor) {
        return;
    }
    let pressed = |c: &str| super::binds::just_pressed(&console.binds, &keys, &mouse, c);
    if pressed("+jump") {
        spec.pending.next_mode = true;
    }
    if pressed("+attack") {
        spec.pending.target_step = 1;
    } else if pressed("+attack2") {
        spec.pending.target_step = -1;
    }
}

/// The chase camera's orbit (yaw, pitch, radians) and the roaming camera.
#[derive(Resource, Default)]
pub struct ChaseOrbit {
    orbit: Vec2,
    /// The target and mode the orbit was set up for.
    set_for: Option<(Entity, SpecMode)>,
    roam: FreeCam,
}

type Watched<'a> = (
    &'a Transform,
    &'a Intent,
    &'a MovementState,
    Option<&'a ViewPunch>,
    Option<&'a RenderedView>,
);

/// Where a character is drawn this frame and its eye (eased between
/// ticks, `map::interp`; the simulation's when not eased).
fn drawn((t, i, s, punch, view): (&Transform, &Intent, &MovementState, Option<&ViewPunch>, Option<&RenderedView>)) -> (Vec3, EyeView) {
    let eye = match view {
        Some(v) => v.now,
        None => EyeView {
            punch: punch.map_or(Vec3::ZERO, |p| p.0),
            ..EyeView::of(i, s)
        },
    };
    (t.translation, eye)
}

/// Where the spectator camera goes this frame.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn place_camera(
    time: Res<Time>,
    spec: Res<Spectator>,
    mut view: ResMut<SpecView>,
    mut chase: ResMut<ChaseOrbit>,
    local: Option<Single<(Entity, &Transform, &Intent, &MovementState, Option<&RenderedView>), With<LocalPlayer>>>,
    watched: Query<Watched>,
    ragdolls: Query<&Ragdoll>,
    bodies: Query<&GlobalTransform>,
    spatial: SpatialQuery,
    characters: Query<Entity, With<Intent>>,
    (orbit, spectating): (Option<Res<crate::core::OrbitDeadView>>, Query<(), With<crate::core::Spectating>>),
    controls: (
        Option<Res<ButtonInput<KeyCode>>>,
        Option<Res<AccumulatedMouseMotion>>,
        Option<Res<super::input::MouseSettings>>,
        Option<Single<&CursorOptions>>,
        Option<Res<ButtonInput<MouseButton>>>,
        Option<Res<crate::console::Console>>,
    ),
) {
    let (Some(local), true) = (local, spec.active()) else {
        chase.set_for = None;
        chase.roam.at = None;
        view.set_if_neq(SpecView::default());
        return;
    };
    let (me, at, intent, state, own_view) = *local;
    let own = crate::map::interp::eye_view(own_view, intent, state);
    // A game's own dead view (HL2:DM's): around its ragdoll until it lives.
    if let (Some(orbit), false) = (orbit.as_deref(), spectating.contains(me)) {
        let root = ragdolls
            .iter()
            .find(|r| r.owner == me && !r.placed)
            .and_then(|r| r.bodies.first())
            .and_then(|b| bodies.get(*b).ok())
            .map(|g| g.translation());
        let look = Quat::from_euler(EulerRot::YXZ, intent.yaw, intent.pitch, 0.0);
        let pose = orbit_pose(orbit, root, at.translation + own.eye_offset, look, &spatial, &characters);
        view.set_if_neq(SpecView { pose: Some(pose), in_eye: None });
        return;
    }
    let (keys, motion, settings, cursor, mouse, console) = controls;
    let grabbed = cursor.is_some_and(|c| super::input::cursor_grabbed(&c));
    let turn = match (grabbed, motion, settings) {
        (true, Some(m), Some(s)) => s.look_delta(m.delta),
        _ => Vec2::ZERO,
    };
    let mode = spec.effective_mode();
    let pose = match spec.phase {
        SpecPhase::Alive => None,
        SpecPhase::DeathCam { since, killer } => {
            let dc = death_cam(me, at, own.eye_offset, intent, killer, &watched, &ragdolls, &bodies, &spatial, &characters);
            // The rise, within a death cam fitted shorter.
            let rise = match spec.death_cam_length as f32 {
                l if l > 0.0 => DEATH_CAM_MOVE.min(l),
                _ => DEATH_CAM_MOVE,
            };
            let s = ((time.elapsed_secs_f64() - since) as f32 / rise).clamp(0.0, 1.0);
            Some(dc.at(s))
        }
        SpecPhase::FreezeCam {
            since, killer, travel, ..
        } => {
            let dc = death_cam(me, at, own.eye_offset, intent, Some(killer), &watched, &ragdolls, &bodies, &spatial, &characters);
            let (from, _) = dc.at(1.0);
            let s = ((time.elapsed_secs_f64() - since) as f32 / travel.max(1e-3)).clamp(0.0, 1.0);
            Some(freeze_pose(from, dc.focus, s, &spatial, &characters))
        }
        SpecPhase::Watching => {
            let target = spec.target.and_then(|t| watched.get(t).ok()).map(drawn);
            let set_for = spec.target.map(|t| (t, mode));
            if set_for != chase.set_for {
                // A new target or mode: the orbit starts behind its view.
                if let Some((_, e)) = target {
                    chase.orbit = Vec2::new(e.yaw, CHASE_PITCH.to_radians());
                }
                chase.set_for = set_for;
            }
            if mode != SpecMode::Roaming {
                chase.roam.at = None;
            }
            match (mode, target) {
                (SpecMode::InEye, Some((t, e))) => {
                    let p = e.punch;
                    let look = Quat::from_euler(EulerRot::YXZ, e.yaw + p.y, e.pitch + p.x, 0.0);
                    let fp = CameraMode::default();
                    let offset = camera_offset(&fp, t, e.eye_offset, look, &spatial, &characters);
                    Some((t + offset, look))
                }
                (SpecMode::Chase, Some((t, e))) => {
                    chase.orbit.x = (chase.orbit.x + turn.x).rem_euclid(std::f32::consts::TAU);
                    chase.orbit.y = (chase.orbit.y + turn.y).clamp(-PITCH_LIMIT, PITCH_LIMIT);
                    let look = Quat::from_euler(EulerRot::YXZ, chase.orbit.x, chase.orbit.y, 0.0);
                    let third = CameraMode {
                        third_person: true,
                        ideal_dist: CHASE_DISTANCE,
                        ideal_yaw: 0.0,
                        pivot_up: 0.0,
                        meters_per_unit: UNIT,
                    };
                    let offset = camera_offset(&third, t, e.eye_offset, look, &spatial, &characters);
                    Some((t + offset, look))
                }
                (SpecMode::Fixed, _) => {
                    // Held where it was (CS:S's sat at the last watched
                    // player's eye where it died).
                    chase.roam.at = None;
                    Some(view.pose.unwrap_or((
                        at.translation + own.eye_offset,
                        Quat::from_euler(EulerRot::YXZ, intent.yaw, intent.pitch, 0.0),
                    )))
                }
                _ => {
                    // Roaming: from wherever the camera was.
                    if chase.roam.at.is_none() {
                        let (p, q) = view
                            .pose
                            .unwrap_or((at.translation + own.eye_offset, Quat::from_euler(EulerRot::YXZ, intent.yaw, intent.pitch, 0.0)));
                        let (yaw, pitch, _) = q.to_euler(EulerRot::YXZ);
                        chase.roam.at = Some((p, yaw, pitch));
                    }
                    // The movement keys (binds) fly it.
                    let held = |c: &str| {
                        grabbed
                            && match (&keys, &mouse, &console) {
                                (Some(k), Some(m), Some(con)) => super::binds::pressed(&con.binds, k, m, c),
                                _ => false,
                            }
                    };
                    let axis = |a: &str, b: &str| held(a) as i8 as f32 - held(b) as i8 as f32;
                    let input = Vec3::new(axis("+moveright", "+moveleft"), axis("+forward", "+back"), 0.0);
                    let speed = if held("+speed") { 12.0 } else { 4.0 };
                    chase.roam.fly(input, turn, speed, time.delta_secs());
                    chase.roam.rotation()
                }
            }
        }
    };
    let in_eye = (spec.phase == SpecPhase::Watching && mode == SpecMode::InEye)
        .then_some(spec.target)
        .flatten();
    view.set_if_neq(SpecView { pose, in_eye });
}

/// The dead view of `OrbitDeadView`: O = the ragdoll's root (or `eye`) +
/// `height` up, the camera `distance` back from O along `look`, pulled in
/// where the world is in the way.
fn orbit_pose(
    orbit: &crate::core::OrbitDeadView,
    root: Option<Vec3>,
    eye: Vec3,
    look: Quat,
    spatial: &SpatialQuery,
    characters: &Query<Entity, With<Intent>>,
) -> (Vec3, Quat) {
    let o = root.map_or(eye, |r| r + Vec3::Y * orbit.height * UNIT);
    let back = look * Vec3::Z * orbit.distance * UNIT;
    (sweep(spatial, o, back, characters), look)
}

/// The death cam's path: from the eye back (away from the killer, or
/// behind where you faced) and up, turning from where you looked to the
/// killer's eye (or your body).
struct DeathCamPath {
    eye: Vec3,
    to: Vec3,
    start: Quat,
    focus: Vec3,
}

impl DeathCamPath {
    /// The pose `s` (0..1, eased) of the way along.
    fn at(&self, s: f32) -> (Vec3, Quat) {
        let s = s * s * (3.0 - 2.0 * s);
        let p = self.eye.lerp(self.to, s);
        let end = if self.focus.distance(p) > 1e-3 {
            Transform::from_translation(p).looking_at(self.focus, Vec3::Y).rotation
        } else {
            self.start
        };
        (p, self.start.slerp(end, s))
    }
}

#[allow(clippy::too_many_arguments)]
fn death_cam(
    me: Entity,
    at: &Transform,
    eye_offset: Vec3,
    intent: &Intent,
    killer: Option<Entity>,
    watched: &Query<Watched>,
    ragdolls: &Query<&Ragdoll>,
    bodies: &Query<&GlobalTransform>,
    spatial: &SpatialQuery,
    characters: &Query<Entity, With<Intent>>,
) -> DeathCamPath {
    let eye = at.translation + eye_offset;
    let start = Quat::from_euler(EulerRot::YXZ, intent.yaw, intent.pitch, 0.0);
    let killer = killer.filter(|k| *k != me).and_then(|k| watched.get(k).ok()).map(drawn);
    let (away, focus) = match killer {
        Some((t, e)) => {
            let focus = t + e.eye_offset;
            ((eye - focus).with_y(0.0).normalize_or(start * Vec3::Z), focus)
        }
        None => {
            let body = ragdolls
                .iter()
                .find(|r| r.owner == me)
                .and_then(|r| r.bodies.first())
                .and_then(|b| bodies.get(*b).ok())
                .map_or(at.translation, |g| g.translation());
            ((start * Vec3::Z).with_y(0.0).normalize_or(Vec3::Z), body)
        }
    };
    let to = sweep(spatial, eye, away * DEATH_CAM_BACK + Vec3::Y * DEATH_CAM_UP, characters);
    DeathCamPath { eye, to, start, focus }
}

/// The freeze cam `s` (0..1) of the way from the death cam's end toward
/// the killer's eye (`freeze_end`, pulled in where the world is in the
/// way), looking at it.
fn freeze_pose(
    from: Vec3,
    focus: Vec3,
    s: f32,
    spatial: &SpatialQuery,
    characters: &Query<Entity, With<Intent>>,
) -> (Vec3, Quat) {
    let to = freeze_end(from, focus);
    let to = sweep(spatial, from, to - from, characters);
    let s = s * s * (3.0 - 2.0 * s);
    let p = from.lerp(to, s);
    let look = if focus.distance(p) > 1e-3 {
        Transform::from_translation(p).looking_at(focus, Vec3::Y).rotation
    } else {
        Quat::IDENTITY
    };
    (p, look)
}

/// Where the freeze cam stops: on the line from `from` to the killer's
/// eye `focus`, `FREEZE_DISTANCE` from it (no farther than `from` was).
pub fn freeze_end(from: Vec3, focus: Vec3) -> Vec3 {
    let d = FREEZE_DISTANCE * UNIT;
    let gap = from.distance(focus);
    if gap <= d {
        return from;
    }
    focus + (from - focus) / gap * d
}

/// `from + offset`, pulled in if the world is in the way.
fn sweep(spatial: &SpatialQuery, from: Vec3, offset: Vec3, characters: &Query<Entity, With<Intent>>) -> Vec3 {
    let Ok(dir) = Dir3::new(offset) else { return from };
    let filter = SpatialQueryFilter::from_excluded_entities(characters).with_mask(crate::core::SOLID_LAYERS);
    let config = ShapeCastConfig {
        max_distance: offset.length(),
        ignore_origin_penetration: true,
        ..default()
    };
    let d = spatial
        .cast_shape(&Collider::sphere(CAMERA_RADIUS), from, Quat::IDENTITY, dir, &config, &filter)
        .map_or(offset.length(), |h| h.distance);
    from + dir * d
}

/// Watching through a target's eyes: its body hidden (the camera is
/// inside it) and its view model drawn at the local camera.
#[allow(clippy::type_complexity)]
fn show_through_eyes(
    view: Res<SpecView>,
    mut hidden: Local<Option<Entity>>,
    children: Query<&Children>,
    mut bodies: Query<&mut Visibility, With<crate::render::bodies::CharacterBody>>,
    anchors: Query<(Entity, Option<&ViewModelSource>), With<ViewModelAnchor>>,
    mut commands: Commands,
) {
    let mut set = |owner: Entity, want: Visibility| {
        if let Ok(kids) = children.get(owner) {
            let mut it = bodies.iter_many_mut(kids).matched();
            while let Some(mut v) = it.fetch_next() {
                v.set_if_neq(want);
            }
        }
    };
    if *hidden != view.in_eye
        && let Some(old) = hidden.take()
    {
        set(old, Visibility::Inherited);
    }
    if let Some(t) = view.in_eye {
        // Every frame: a body re-made (team change) comes back visible.
        set(t, Visibility::Hidden);
        *hidden = Some(t);
    }
    for (anchor, source) in &anchors {
        let want = view.in_eye.map(ViewModelSource);
        if source.copied() != want {
            match want {
                Some(s) => commands.entity(anchor).insert(s),
                None => commands.entity(anchor).remove::<ViewModelSource>(),
            };
        }
    }
}

/// The field of view to use while spectating in first person: the
/// target's zoom.
pub fn target_zoom(view: &SpecView, zoomed: &Query<&Zoomed>) -> Option<Option<f32>> {
    view.in_eye.map(|t| zoomed.get(t).ok().map(|z| z.fov))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    fn cand(i: u32, team: u8, x: f32) -> Candidate {
        Candidate {
            entity: e(i),
            team: Some(Team(team)),
            position: Vec3::X * x,
        }
    }

    fn dead_ct(killed: bool) -> Me {
        Me {
            dead: true,
            killed,
            killer: killed.then(|| e(9)),
            team: Some(Team(2)),
            position: Vec3::ZERO,
            freeze: None,
            spectator: false,
            respawn_in: None,
        }
    }

    /// A timed respawn fits the death view: with a 2 s wait the death cam
    /// is 0.8 s, the travel 0.3 s and the picture holds 0.9 s; a wait as
    /// long as the whole view leaves CS:S's timings; a short one never
    /// gives a negative hold.
    #[test]
    fn a_timed_respawn_fits_the_death_and_freeze_cams() {
        let cs = Some((FREEZE_TRAVEL, FREEZE_HOLD));
        let (dc, f) = fit_to_respawn(2.0, cs);
        let (travel, hold) = f.unwrap();
        assert!((dc - 0.8).abs() < 1e-9 && (travel - 0.3).abs() < 1e-6 && (hold - 0.9).abs() < 1e-5, "{dc} {f:?}");
        assert_eq!(fit_to_respawn(20.0, cs), (DEATH_CAM_SECONDS, cs));
        let (dc, f) = fit_to_respawn(0.1, cs);
        assert!(dc <= 0.1 && f.unwrap().1 >= 0.0);
        assert_eq!(fit_to_respawn(2.0, None).1, None, "disabled stays disabled");
    }

    /// Dead with a respawn 2 s away: the freeze cam starts at 0.8 s, holds
    /// until the respawn and the death view is over by then.
    #[test]
    fn the_freeze_cam_is_reached_before_a_timed_respawn() {
        let living = [cand(9, 1, 4.0), cand(2, 2, 9.0)];
        let mut me = dead_ct(true);
        me.freeze = Some((FREEZE_TRAVEL, FREEZE_HOLD));
        let mut s = Spectator::default();
        for i in 0..20 {
            let t = 10.0 + i as f64 * 0.1;
            me.respawn_in = Some((12.0 - t).max(0.0));
            s.step(t, &me, &living, 1, SpecInput::default());
            match i {
                0..=7 => assert!(matches!(s.phase, SpecPhase::DeathCam { .. }), "{t}: {:?}", s.phase),
                8 => {}
                _ => assert!(matches!(s.phase, SpecPhase::FreezeCam { .. }), "{t}: {:?}", s.phase),
            }
        }
        assert!((s.death_cam_length - 0.8).abs() < 1e-9);
        // Without a timed respawn, CS:S's 2 s.
        let mut s = Spectator::default();
        me.respawn_in = None;
        s.step(10.0, &me, &living, 1, SpecInput::default());
        s.step(11.0, &me, &living, 1, SpecInput::default());
        assert!(matches!(s.phase, SpecPhase::DeathCam { .. }));
    }

    /// CS:S: only first person hides the watched player (its body and the
    /// weapon in its hand); chase and free look show both, and the held
    /// weapon is never hidden on its own (playdemo review, 2026-10-15).
    #[test]
    fn only_first_person_hides_the_watched_body_and_its_weapon() {
        use crate::render::bodies::CharacterBody;
        let mut app = App::new();
        app.init_resource::<SpecView>().add_systems(Update, show_through_eyes);
        let w = app.world_mut();
        let target = w.spawn((Transform::default(), Visibility::default())).id();
        let body = w
            .spawn((
                CharacterBody {
                    model: 0,
                    skin: 0,
                    body: 0,
                    joints: Vec::new(),
                    held: None,
                    held_parts: Vec::new(),
                },
                Visibility::Inherited,
                ChildOf(target),
            ))
            .id();
        let held = w.spawn((Visibility::Inherited, ChildOf(body))).id();
        let shown = |app: &App| {
            (
                *app.world().get::<Visibility>(body).unwrap(),
                *app.world().get::<Visibility>(held).unwrap(),
            )
        };
        for (in_eye, body_vis) in [
            (Some(target), Visibility::Hidden),
            (None, Visibility::Inherited),
            (Some(target), Visibility::Hidden),
            (None, Visibility::Inherited),
        ] {
            app.world_mut().resource_mut::<SpecView>().in_eye = in_eye;
            app.update();
            // The weapon follows its body (Inherited): drawn whenever the
            // body is.
            assert_eq!(shown(&app), (body_vis, Visibility::Inherited), "in eye: {in_eye:?}");
        }
    }

    #[test]
    fn force_camera_keeps_you_on_your_team_while_it_lives() {
        let living = [cand(1, 1, 1.0), cand(2, 2, 5.0), cand(3, 2, 9.0)];
        assert_eq!(allowed(&living, Some(Team(2)), 1), vec![e(2), e(3)]);
        assert_eq!(allowed(&living, Some(Team(2)), 0), vec![e(1), e(2), e(3)]);
        // No teammate left: no one (CS:S's dead didn't take an enemy).
        assert!(allowed(&living[..1], Some(Team(2)), 1).is_empty());
        // The spectator team (no team): anyone.
        assert_eq!(allowed(&living, None, 1), vec![e(1), e(2), e(3)]);
        assert!(allowed(&[], Some(Team(2)), 1).is_empty());
    }

    /// Force 2 (Combat Arms' rounds): your teammates while any lives, then
    /// anyone.
    #[test]
    fn force_two_moves_on_to_enemies_when_your_team_is_dead() {
        let living = [cand(1, 1, 1.0), cand(2, 2, 5.0), cand(3, 2, 9.0)];
        assert_eq!(allowed(&living, Some(Team(2)), 2), vec![e(2), e(3)]);
        assert_eq!(allowed(&living[..1], Some(Team(2)), 2), vec![e(1)]);
        assert!(allowed(&[], Some(Team(2)), 2).is_empty());
    }

    #[test]
    fn targets_cycle_both_ways_and_wrap() {
        let list = [e(1), e(2), e(3)];
        assert_eq!(cycle(&list, Some(e(1)), 1), Some(e(2)));
        assert_eq!(cycle(&list, Some(e(3)), 1), Some(e(1)));
        assert_eq!(cycle(&list, Some(e(1)), -1), Some(e(3)));
        assert_eq!(cycle(&list, None, 1), Some(e(1)));
        assert_eq!(cycle(&list, Some(e(7)), -1), Some(e(3)));
        assert_eq!(cycle(&[], Some(e(1)), 1), None);
    }

    #[test]
    fn modes_cycle_first_person_chase_free_look() {
        let m = SpecMode::InEye;
        assert_eq!(m.next(), SpecMode::Chase);
        assert_eq!(m.next().next(), SpecMode::Roaming);
        assert_eq!(m.next().next().next(), SpecMode::InEye);
        for m in [SpecMode::InEye, SpecMode::Chase, SpecMode::Roaming] {
            assert_eq!(SpecMode::from_number(m.number()), Some(m));
        }
    }

    #[test]
    fn death_cam_then_the_first_teammate_in_order() {
        let living = [cand(1, 1, 1.0), cand(2, 2, 9.0), cand(3, 2, 4.0)];
        let mut s = Spectator::default();
        s.step(10.0, &dead_ct(true), &living, 1, SpecInput::default());
        assert_eq!(
            s.phase,
            SpecPhase::DeathCam {
                since: 10.0,
                killer: Some(e(9))
            }
        );
        // Keys wait for the death cam.
        let next = SpecInput {
            next_mode: true,
            target_step: 1,
            ..default()
        };
        s.step(11.0, &dead_ct(true), &living, 1, next);
        assert!(matches!(s.phase, SpecPhase::DeathCam { .. }) && s.target.is_none());
        assert_eq!(s.mode, SpecMode::InEye);
        // No freeze cam: the death cam lasts CS:S's 6.5 s.
        s.step(10.0 + DEATH_CAM_SECONDS, &dead_ct(true), &living, 1, SpecInput::default());
        assert!(matches!(s.phase, SpecPhase::DeathCam { .. }));
        s.step(10.0 + DEATH_CAM_ALONE - 0.1, &dead_ct(true), &living, 1, SpecInput::default());
        assert!(matches!(s.phase, SpecPhase::DeathCam { .. }));
        s.step(10.0 + DEATH_CAM_ALONE, &dead_ct(true), &living, 1, SpecInput::default());
        assert_eq!(s.phase, SpecPhase::Watching);
        // Not the nearer enemy nor the nearer teammate: the first teammate
        // in order.
        assert_eq!(s.target, Some(e(2)));
        // Attack: the next teammate, wrapping; never the enemy.
        let step = |n| SpecInput {
            target_step: n,
            ..default()
        };
        s.step(17.0, &dead_ct(true), &living, 1, step(1));
        assert_eq!(s.target, Some(e(3)));
        s.step(17.1, &dead_ct(true), &living, 1, step(1));
        assert_eq!(s.target, Some(e(2)));
        s.step(17.2, &dead_ct(true), &living, 1, step(-1));
        assert_eq!(s.target, Some(e(3)));
        // mp_forcecamera 0: the enemy too.
        s.step(17.3, &dead_ct(true), &living, 0, step(1));
        assert_eq!(s.target, Some(e(1)));
        // Jump: the next mode.
        s.step(
            17.4,
            &dead_ct(true),
            &living,
            1,
            SpecInput {
                next_mode: true,
                ..default()
            },
        );
        assert_eq!(s.mode, SpecMode::Chase);
    }

    #[test]
    fn put_out_of_play_without_dying_skips_the_death_cam() {
        let living = [cand(2, 2, 3.0)];
        let mut s = Spectator::default();
        s.step(1.0, &dead_ct(false), &living, 1, SpecInput::default());
        assert_eq!(s.phase, SpecPhase::Watching);
        assert_eq!(s.target, Some(e(2)));
    }

    #[test]
    fn a_dead_target_lingers_then_the_next_one() {
        let mut living = vec![cand(2, 2, 3.0), cand(3, 2, 5.0)];
        let mut s = Spectator {
            mode: SpecMode::InEye,
            ..default()
        };
        s.step(0.0, &dead_ct(false), &living, 1, SpecInput::default());
        assert_eq!(s.target, Some(e(2)));
        // Bot 2 dies: still on it (chase camera on the body) for a moment.
        living.remove(0);
        s.step(1.0, &dead_ct(false), &living, 1, SpecInput::default());
        assert_eq!(s.target, Some(e(2)));
        assert_eq!(s.effective_mode(), SpecMode::Chase);
        s.step(1.0 + TARGET_DEATH_HOLD, &dead_ct(false), &living, 1, SpecInput::default());
        assert_eq!(s.target, Some(e(3)));
        assert_eq!(s.effective_mode(), SpecMode::InEye);
        // Every teammate dead: a fixed view (CS:S's mode 3, measured
        // 2026-10-18 with no round end), free look on asking for it.
        s.step(5.0, &dead_ct(false), &[], 1, SpecInput::default());
        s.step(5.0 + TARGET_DEATH_HOLD, &dead_ct(false), &[], 1, SpecInput::default());
        assert_eq!(s.target, None);
        assert_eq!(s.effective_mode(), SpecMode::Fixed);
        s.step(9.0, &dead_ct(false), &[], 1, SpecInput {
            set_mode: Some(SpecMode::Roaming),
            ..default()
        });
        assert_eq!(s.effective_mode(), SpecMode::Roaming);
    }

    #[test]
    fn living_again_ends_spectating_but_keeps_the_mode() {
        let living = [cand(2, 2, 3.0)];
        let mut s = Spectator::default();
        s.step(0.0, &dead_ct(false), &living, 1, SpecInput {
            set_mode: Some(SpecMode::Chase),
            ..default()
        });
        assert_eq!(s.mode, SpecMode::Chase);
        s.step(1.0, &Me::default(), &living, 1, SpecInput::default());
        assert_eq!(s.phase, SpecPhase::Alive);
        assert!(!s.active() && s.target.is_none());
        assert_eq!(s.mode, SpecMode::Chase);
    }

    #[test]
    fn names_sort_by_their_numbers() {
        let mut names = vec!["Bot 10", "Bot 2", "Bot 1"];
        names.sort_by_key(|n| natural(n));
        assert_eq!(names, ["Bot 1", "Bot 2", "Bot 10"]);
    }
}
