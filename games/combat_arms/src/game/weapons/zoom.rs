//! Combat Arms' zoom and scope (specs/combat_arms/weapon_classes.md,
//! section 7; numbers below are its sections): the `combat_arms:zoom` hook
//! (`CaZoom`) on any weapon with zoom fields, zooming when its scope (the
//! sight attachment fitted, by default its `DefaultScopeAttach`:
//! `attachments` sets `CaZoom::scope` from it) has levels.
//!
//! - The key (7.2): attack2's press only, a toggle through 0, 1, ...,
//!   L_max, 0. A press is dropped while a transition runs, while
//!   reloading (either kind), and on a `ReloadAfterFire` weapon in its fire
//!   animation and not zoomed (`bolt_busy`: set by a shot, cleared on the
//!   first update whose weapon step sees the animation over). The level
//!   changes on the press; the field of view then moves at a constant
//!   speed (7.4: in at (90 - fov_L) / `ZoomTime`, out as if from 20
//!   degrees) and is clamped on the frame it arrives. "Zoomed" (`zoomed`)
//!   is the level above 0 or a transition running: from the press, the
//!   zoomed spread values (`weapons`), the scoped speed limit (movement's,
//!   through `Zoomed`) and the mouse scale 1 / (1 + 3L) (7.7,
//!   `Zoomed::sensitivity`) apply. The owner's `Zoomed` (Chimera's: the
//!   camera's FOV, the scope overlay, the HUD's "no crosshair") is kept here
//!   (the weapon is `OwnZoom`): present while zoomed, so the overlay stays
//!   until the zoom-out ends (7.10).
//! - Leaving (7.5): switching weapons or dying resets at once (no
//!   transition, a pending bolt zoom-out and the "reload after zoom" flag
//!   cleared: Q10's recommendation). A reload request while zoomed (the key,
//!   or the automatic one at clip 0) is ignored during a transition, does
//!   nothing when the reload would be refused, and otherwise zooms out (a
//!   `ReloadAfterFire` weapon arms its 300 ms zoom-out instead) and starts
//!   the reload on the frame the zoom-out completes. Chimera's frame is held
//!   from reloading meanwhile (`WeaponState::reload_held`).
//! - The bolt zoom-out (7.6): a `ReloadAfterFire` weapon's shot while the
//!   last transition was a zoom-in arms it; on a later update, zoomed and
//!   more than 300 whole ms on, it is one zoom press (so a two-level scope at
//!   level 1 steps in first, Q3), retried each update while dropped.
//! - Sway (7.8): each update while zoomed and alive, the aim drifts by
//!   (S / 1000) sin(t F / 10) dt on each axis (t the game time), added to
//!   the owner's `ViewPunch`, which the shots and the camera follow and
//!   nothing takes back (Q9).
//!
//! - Sounds (Follow-up (2026-11-14) 1-2): each accepted press, the
//!   automatic zoom-outs included, writes the scope part's `ZoomInSound`
//!   (a level up) or `ZoomOutSound` (back to 0) as `CaZoomSound`, and so
//!   does the instant reset of a weapon that was zoomed (its own part's
//!   zoom-out sound); a refused press or a reset while not zoomed is
//!   silent. `effects` plays it as a local sound on the owner's machine
//!   only.
//!
//! - The focus blur (Follow-up (2026-11-14) 3): `CaFocusBlur`, on while
//!   the local player's weapon is zoomed (a transition included) and the
//!   `Shader` option is on, through the scope part's `FocusTex`
//!   (`focus_blur`; drawn by `ui::scope`).
//!
//! Per tick: presses, resets and reload requests in `core::Predict::Select`
//! (after selection, before movement, so the speed limit changes on the
//! press's tick); sway, the transition step and the reload after a zoom-out
//! in `Predict::Weapons` before the frame; the bolt zoom-out after it. A
//! network client predicts `CaZoom` with the weapon (`weapon::NetPart`) and
//! the owner's `Zoomed` and `ViewPunch` (Chimera's predicted parts).
//!
//! Known shortcuts:
//! - A first-person spectator doesn't hear the watched player's zoom
//!   sounds (no spectator mirroring, below); the scope's overlay
//!   picture (`Scope::sprite`, `Scope::sprite_scale`) is drawn by
//!   `ui::scope` (a card scaled with the screen's height, 7.10), and
//!   Chimera's placeholder is drawn while it loads or when the install has
//!   none (the range readout and gauge are the HUD's, `ui::hud`). `ZoomedMovePenalty` is read and
//!   unused (Q4).
//! - The camera's vertical FOV keeps Chimera's rule (the client's); 7.3's
//!   proportional rule is `vertical_fov` (FovX/FovY 90/75, not settable).
//! - No breath perk (perks spec), no cinematic or external cameras, no
//!   dot-sight (`Type` 2) scopes, no spectator mirroring; the LOD offset
//!   (presentation) isn't applied.
//! - A reload after zoom of a shell-by-shell weapon isn't started here
//!   (no such weapon has a scope); the frame's own reload then waits for a
//!   new request.
//! - Updates are the game's ticks: "the first update after" is the next
//!   tick, the frame time is the tick (capped at 0.1 s).

use bevy::prelude::*;
use serde::Deserialize;

use crate::{
    core::{FirstTimePredicted, Health, Intent, SimClock},
    map::interp::NetDrawn,
    weapon::{Inventory, Magazine, NetPart, ShellReload, ViewPunch, WeaponEvent, WeaponEventKind, WeaponState, OwnZoom, Zoom, Zoomed},
};

/// The hook's name.
pub const ZOOM: &str = "combat_arms:zoom";

/// A zoom step's sound (Follow-up (2026-11-14) 1-2): the scope part's
/// `ZoomInSound` or `ZoomOutSound`, for `owner`'s machine alone (a local
/// sound: `effects` plays it when `owner` is the local player). Written
/// the first time a tick is predicted only.
#[derive(Message, Clone, Debug, PartialEq)]
pub struct CaZoomSound {
    pub owner: Entity,
    /// The file inside the install's archives (never empty).
    pub file: String,
}

/// The sound of a step from level `before` to the weapon's level now (in:
/// up a level; out: back to 0; none: no change, or the part names none).
fn step_sound(z: &CaZoom, before: u8) -> Option<String> {
    let s = z.scope.as_ref()?;
    let file = if z.level > before {
        &s.zoom_in_sound
    } else if z.level < before {
        &s.zoom_out_sound
    } else {
        return None;
    };
    (!file.trim().is_empty()).then(|| file.clone())
}

/// The focus blur's mask before any scope part named one
/// (Follow-up (2026-11-14) 3).
pub const DEFAULT_FOCUS_MASK: &str = "shaders/image/ScopeFocus.dtx";

/// The focus blur on the local player's screen (Follow-up (2026-11-14)
/// 3): on while the local player's weapon is zoomed in the sense of Z
/// (level above 0 or a transition running, the zoom-out's included) and
/// the `Shader` option is on; the mask is the `FocusTex` of the local
/// weapon's scope part, looked up each frame, the last one kept while the
/// weapon in hand has none (`DEFAULT_FOCUS_MASK` at the start). The
/// front end draws it (`ui::scope`).
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct CaFocusBlur {
    pub on: bool,
    /// The mask's install path.
    pub mask: String,
}

impl Default for CaFocusBlur {
    fn default() -> Self {
        Self {
            on: false,
            mask: DEFAULT_FOCUS_MASK.into(),
        }
    }
}

/// `CaFocusBlur` from the local player's weapon and the `Shader` option.
pub fn focus_blur(
    settings: Option<Res<crate::game::options::settings::CaSettings>>,
    me: Query<&crate::weapon::Inventory, With<crate::core::LocalPlayer>>,
    zooms: Query<&CaZoom>,
    mut blur: ResMut<CaFocusBlur>,
) {
    let shader = settings.is_some_and(|s| s.bool("Shader"));
    let zoom = me.iter().next().and_then(|inv| inv.active).and_then(|w| zooms.get(w).ok());
    if let Some(mask) = zoom.and_then(|z| z.scope.as_ref()).map(|s| s.focus_tex.trim()).filter(|m| !m.is_empty())
        && blur.mask != mask
    {
        blur.mask = mask.to_string();
    }
    let on = shader && zoom.is_some_and(CaZoom::zoomed);
    if blur.on != on {
        blur.on = on;
    }
}

// Constants (the spec's "Constants").

/// `FovX`, `FovY` defaults: the view when not zoomed, degrees.
pub const FOV_X: f32 = 90.0;
pub const FOV_Y: f32 = 75.0;
/// Zoom-out speed is computed as if leaving this FOV, degrees.
pub const ZOOM_OUT_REFERENCE: f32 = 20.0;
/// Most `ZoomFov#` entries a scope has.
pub const MAX_ZOOM_LEVELS: usize = 10;
/// The mouse divisor is 1 + this x L.
pub const SENS_ZOOM_STEP: f32 = 3.0;
/// After a zoomed shot of a `ReloadAfterFire` weapon, zoom steps once
/// more after more than this, ms.
pub const BOLT_ZOOM_OUT_DELAY_MS: f64 = 300.0;
/// Sway phase is game time x this x frequency; its rate speed x the other.
pub const SWAY_TIME_SCALE: f32 = 0.1;
pub const SWAY_AMOUNT_SCALE: f32 = 0.001;
/// Longest frame time these rules see, s.
pub const FRAME_TIME_CAP: f32 = 0.1;
/// Timers due on a tick count on it (as the weapon frame does).
const SLACK: f64 = 1e-5;

/// The vertical FOV for a horizontal one (7.3: in proportion to the
/// angle, 90/75), degrees.
pub fn vertical_fov(fov_x: f32) -> f32 {
    fov_x * FOV_Y / FOV_X
}

/// `CameraSway*`: speeds and frequencies (defaults 12, 2, 6, 20).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Sway {
    pub x_speed: f32,
    pub y_speed: f32,
    pub x_freq: f32,
    pub y_freq: f32,
}

impl Default for Sway {
    fn default() -> Self {
        Self {
            x_speed: 12.0,
            y_speed: 2.0,
            x_freq: 6.0,
            y_freq: 20.0,
        }
    }
}

/// A scope attachment's zoom (the modifications file's `Mod#` record, as
/// `attachments::ModRecord::scope` gives it).
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct Scope {
    /// The record's `Name`.
    pub name: String,
    /// `ZoomFov0..`: each level's horizontal FOV, degrees (`ZoomLevel`
    /// entries; at most `MAX_ZOOM_LEVELS` are used).
    pub fov: Vec<f32>,
    /// `ZoomSpriteName`: the overlay picture (an install path; empty:
    /// none, the shell's stand-in is drawn).
    pub sprite: String,
    /// `ZoomSpriteScale` (0: missing, `DEFAULT_SPRITE_SCALE`).
    pub sprite_scale: f32,
    /// `ZoomInSound`, `ZoomOutSound` (an install path; empty: silent).
    pub zoom_in_sound: String,
    pub zoom_out_sound: String,
    /// `FocusTex`: the focus blur's mask (an install path; empty: none).
    pub focus_tex: String,
}

/// `ZoomSpriteScale` when the record has none (weapon_classes.md 7.10).
pub const DEFAULT_SPRITE_SCALE: f32 = 0.001;

impl Scope {
    /// The overlay picture's scale, the default where the record has none.
    pub fn sprite_scale(&self) -> f32 {
        if self.sprite_scale > 0.0 { self.sprite_scale } else { DEFAULT_SPRITE_SCALE }
    }
}

/// `combat_arms:zoom`: a weapon's zoom fields (its scope is the fitted
/// sight's, `CaZoom::scope`).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct ZoomParams {
    /// `ZoomTime`, s.
    pub zoom_time: f32,
    pub sway: Sway,
    /// `ReloadAfterFire`: leave zoom after a zoomed shot, no zooming in
    /// during the fire animation.
    pub reload_after_fire: bool,
    /// `ZoomedMovePenalty`: read, never used (Q4).
    pub zoomed_move_penalty: f32,
}

impl Default for ZoomParams {
    fn default() -> Self {
        Self {
            zoom_time: 0.5,
            sway: Sway::default(),
            reload_after_fire: false,
            zoomed_move_penalty: 0.7,
        }
    }
}

/// A field-of-view change under way: toward `target` at `speed`
/// degrees/s.
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Transition {
    pub target: f32,
    pub speed: f32,
}

/// A weapon's zoom (`combat_arms:zoom`): the data and its state.
#[derive(Component, Clone, Debug)]
pub struct CaZoom {
    pub params: ZoomParams,
    /// The fitted sight's scope (`attachments`); none: the zoom key does
    /// nothing.
    pub scope: Option<Scope>,
    /// The fitted parts' sway additions (x speed, y speed, x freq, y freq).
    pub sway_add: [f32; 4],
    /// The zoom level L (0: not zoomed).
    pub level: u8,
    /// The horizontal FOV now, degrees.
    pub fov: f32,
    pub transition: Option<Transition>,
    /// The last transition was a zoom-in.
    pub last_in: bool,
    /// When the bolt zoom-out was armed (7.6).
    pub bolt_armed: Option<f64>,
    /// A reload waits for the zoom-out (7.5).
    pub reload_after: bool,
    /// A `ReloadAfterFire` weapon is in its fire animation (7.2).
    pub bolt_busy: bool,
}

impl CaZoom {
    pub fn new(params: ZoomParams) -> Self {
        Self {
            params,
            scope: None,
            sway_add: [0.0; 4],
            level: 0,
            fov: FOV_X,
            transition: None,
            last_in: false,
            bolt_armed: None,
            reload_after: false,
            bolt_busy: false,
        }
    }

    /// L_max: the scope's levels (0: can't zoom).
    pub fn levels(&self) -> u8 {
        self.scope.as_ref().map_or(0, |s| s.fov.len().min(MAX_ZOOM_LEVELS) as u8)
    }

    /// "Zoomed" (Z): a level above 0 or a transition running.
    pub fn zoomed(&self) -> bool {
        self.level > 0 || self.transition.is_some()
    }

    /// The FOV of `level` (0: `FOV_X`), degrees.
    pub fn level_fov(&self, level: u8) -> f32 {
        match (level, &self.scope) {
            (0, _) | (_, None) => FOV_X,
            (l, Some(s)) => s.fov.get(l as usize - 1).copied().unwrap_or(FOV_X),
        }
    }

    /// The mouse scale at this level (7.7): 1 / (1 + 3L).
    pub fn sensitivity(&self) -> f32 {
        1.0 / (1.0 + SENS_ZOOM_STEP * self.level as f32)
    }

    fn zoom_time(&self) -> f32 {
        self.params.zoom_time
    }

    /// Instantly back to normal (7.5: switching, dying).
    pub fn reset(&mut self) {
        self.level = 0;
        self.fov = FOV_X;
        self.transition = None;
        self.last_in = false;
        self.bolt_armed = None;
        self.reload_after = false;
        self.bolt_busy = false;
    }

    /// Whether the weapon is all reset (nothing to do on a reset).
    fn is_reset(&self) -> bool {
        self.level == 0
            && self.transition.is_none()
            && self.fov == FOV_X
            && self.bolt_armed.is_none()
            && !self.reload_after
            && !self.bolt_busy
            && !self.last_in
    }

    /// Start the zoom-out to level 0 (7.2's last press; 7.5's reload).
    pub fn zoom_out(&mut self) {
        self.level = 0;
        let speed = (FOV_X - ZOOM_OUT_REFERENCE) / self.zoom_time();
        self.transition = Some(Transition { target: FOV_X, speed });
        self.last_in = false;
        self.bolt_armed = None;
    }

    /// One zoom press (7.2): dropped (false) without a scope, during a
    /// transition or when `blocked` (reloading, mid-bolt and not zoomed).
    pub fn press(&mut self, blocked: bool) -> bool {
        let levels = self.levels();
        if levels == 0 || self.transition.is_some() || blocked {
            return false;
        }
        let next = (self.level + 1) % (levels + 1);
        if next > self.level {
            self.level = next;
            let target = self.level_fov(next);
            let speed = (FOV_X - target) / self.zoom_time();
            self.transition = Some(Transition { target, speed });
            self.last_in = true;
        } else {
            self.zoom_out();
        }
        true
    }

    /// The transition's step over `dt` s (7.4): returns true when a
    /// zoom-out to level 0 ended on it.
    pub fn step(&mut self, dt: f32) -> bool {
        let Some(t) = self.transition else { return false };
        let dt = dt.min(FRAME_TIME_CAP);
        let to = t.target;
        let step = if t.speed.is_finite() { t.speed * dt } else { f32::INFINITY };
        let arrived = if self.fov > to {
            self.fov -= step;
            self.fov <= to + 1e-4
        } else {
            self.fov += step;
            self.fov >= to - 1e-4
        };
        if arrived {
            self.fov = to;
            self.transition = None;
            return self.level == 0;
        }
        false
    }

    /// The owner's view while zoomed.
    pub fn view(&self) -> Option<Zoomed> {
        self.zoomed().then(|| Zoomed {
            fov: self.fov,
            scope: true,
            sensitivity: Some(self.sensitivity()),
        })
    }
}

impl NetPart for CaZoom {
    /// The level, FOV, transition, last direction, bolt and reload flags
    /// (the parameters are the data's).
    type Net = (u8, f32, Option<Transition>, bool, Option<f64>, bool, bool);
    fn net(&self) -> Self::Net {
        (
            self.level,
            self.fov,
            self.transition,
            self.last_in,
            self.bolt_armed,
            self.reload_after,
            self.bolt_busy,
        )
    }
    fn set_net(&mut self, (level, fov, transition, last_in, bolt_armed, reload_after, bolt_busy): Self::Net) {
        self.level = level;
        self.fov = fov;
        self.transition = transition;
        self.last_in = last_in;
        self.bolt_armed = bolt_armed;
        self.reload_after = reload_after;
        self.bolt_busy = bolt_busy;
    }
}

/// Put `want` on `owner` as its `Zoomed` (none: remove), when it differs.
fn sync_view(commands: &mut Commands, owner: Entity, have: Option<&Zoomed>, want: Option<Zoomed>) {
    if have.copied() == want {
        return;
    }
    match want {
        Some(z) => commands.entity(owner).insert(z),
        None => commands.entity(owner).remove::<Zoomed>(),
    };
}

/// Whether a reload would start now (refused at a full clip or none in
/// reserve, or while one runs).
fn reload_allowed(mag: Option<&Magazine>, st: &WeaponState) -> bool {
    mag.is_some_and(|m| m.clip < m.size && m.reserve > 0) && !st.reloading()
}

/// Presses, resets and reload requests (7.2, 7.5), after selection.
#[allow(clippy::type_complexity)]
pub(super) fn zoom_input(
    owners: Query<(Entity, &Intent, &Inventory, Option<&Health>, Option<&Zoomed>), Without<NetDrawn>>,
    mut weapons: Query<(&mut CaZoom, &mut WeaponState, Option<&Magazine>)>,
    // Another game's zoom: the engine's (`Zoom`) or its own (`OwnZoom`:
    // Halo's guns keep their holder's `Zoomed` themselves).
    chimera_zoom: Query<(), Or<(With<Zoom>, With<OwnZoom>)>>,
    (clock, first): (Res<SimClock>, Res<FirstTimePredicted>),
    mut sounds: MessageWriter<CaZoomSound>,
    mut commands: Commands,
) {
    let now = clock.now;
    let due = now + SLACK;
    let mut sound = |owner: Entity, file: Option<String>| {
        if let Some(file) = file.filter(|_| first.0) {
            sounds.write(CaZoomSound { owner, file });
        }
    };
    for (owner, intent, inv, health, have) in &owners {
        let alive = health.is_none_or(|h| h.current > 0.0);
        for &w in &inv.weapons {
            let Ok((mut z, mut st, _)) = weapons.get_mut(w) else { continue };
            if (Some(w) != inv.active || !alive) && !z.is_reset() {
                // 7.5 corrected (Follow-up 2): the instant reset while
                // zoomed plays the zoom-out sound; silent when it was not.
                if z.zoomed() {
                    sound(owner, z.scope.as_ref().map(|s| s.zoom_out_sound.clone()).filter(|f| !f.trim().is_empty()));
                }
                z.reset();
                st.reload_held = false;
            }
        }
        let Some(active) = inv.active else { continue };
        let Ok((mut z, mut st, mag)) = weapons.get_mut(active) else {
            // Another game's zoom keeps its own view; none: no zoom.
            if have.is_some() && !chimera_zoom.contains(active) {
                commands.entity(owner).remove::<Zoomed>();
            }
            continue;
        };
        if !alive {
            sync_view(&mut commands, owner, have, None);
            continue;
        }
        // 7.2: attack2's press.
        if intent.secondary && !inv.prev_secondary() {
            let blocked = st.reloading() || (z.params.reload_after_fire && z.bolt_busy && !z.zoomed());
            let before = z.level;
            if z.press(blocked) {
                sound(owner, step_sound(&z, before));
            }
        }
        // 7.5: a reload request while zoomed.
        if z.zoomed() {
            let auto = mag.is_some_and(|m| m.auto_reload && m.clip == 0 && m.reserve > 0)
                && st.next_primary <= due
                && st.next_secondary <= due;
            if (intent.reload || auto) && z.transition.is_none() && reload_allowed(mag, &st) {
                if z.params.reload_after_fire {
                    if z.bolt_armed.is_none() {
                        z.bolt_armed = Some(now);
                    }
                } else {
                    // An automatic zoom-out is a press (Follow-up 2).
                    z.zoom_out();
                    sound(owner, step_sound(&z, 1));
                }
                z.reload_after = true;
            }
        }
        st.reload_held = z.zoomed();
        sync_view(&mut commands, owner, have, z.view());
    }
}

/// Sway, the transition step, and the reload waiting for a zoom-out (7.8,
/// 7.4, 7.5), before the weapon frame.
#[allow(clippy::type_complexity)]
pub(super) fn zoom_step(
    mut owners: Query<
        (
            Entity,
            &mut Inventory,
            Option<&Health>,
            Option<&Zoomed>,
            Option<&mut ViewPunch>,
            Option<&crate::game::perks::CaPerkEffects>,
        ),
        Without<NetDrawn>,
    >,
    mut weapons: Query<(&mut CaZoom, &mut WeaponState, Option<&Magazine>, Has<ShellReload>)>,
    mut events: MessageWriter<WeaponEvent>,
    (clock, first): (Res<SimClock>, Res<FirstTimePredicted>),
    mut commands: Commands,
) {
    let (now, dt) = (clock.now, clock.dt().min(FRAME_TIME_CAP));
    for (owner, mut inv, health, have, punch, perk) in &mut owners {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(active) = inv.active else { continue };
        let Ok((mut z, mut st, mag, shells)) = weapons.get_mut(active) else {
            continue;
        };
        // Steady Hand (gear_perks.md 14) stops the sway.
        if z.zoomed() && !perk.is_some_and(|p| p.steady) {
            // 7.8: the aim drifts (Combat Arms' pitch + is down and yaw +
            // right; the engine's view punch is pitch up, yaw left).
            // Plus the parts' additions (attachments.md: 0 in shipped data).
            let mut s = z.params.sway;
            s.x_speed += z.sway_add[0];
            s.y_speed += z.sway_add[1];
            s.x_freq += z.sway_add[2];
            s.y_freq += z.sway_add[3];
            let t = now as f32;
            let pitch = s.y_speed * SWAY_AMOUNT_SCALE * (t * s.y_freq * SWAY_TIME_SCALE).sin() * dt;
            let yaw = s.x_speed * SWAY_AMOUNT_SCALE * (t * s.x_freq * SWAY_TIME_SCALE).sin() * dt;
            let delta = Vec3::new(-pitch, -yaw, 0.0);
            match punch {
                Some(mut p) => p.0 += delta,
                None => {
                    commands.entity(owner).insert(ViewPunch(delta));
                }
            }
        }
        if z.step(dt) && z.reload_after {
            z.reload_after = false;
            if !shells
                && let Some(m) = mag
                && reload_allowed(Some(m), &st)
                && super::super::inventory::start_reload(&mut inv, &mut st, m, now)
            {
                events.write(WeaponEvent {
                    owner,
                    weapon: active,
                    kind: WeaponEventKind::ReloadStarted,
                    replay: !first.0,
                });
            }
        }
        st.reload_held = z.zoomed();
        sync_view(&mut commands, owner, have, z.view());
    }
}

/// After the weapon frame: the bolt's fire animation and its zoom-out
/// (7.2, 7.6).
#[allow(clippy::type_complexity)]
pub(super) fn bolt_zoom_out(
    owners: Query<(Entity, &Inventory, Option<&Health>, Option<&Zoomed>), Without<NetDrawn>>,
    mut weapons: Query<(&mut CaZoom, &WeaponState)>,
    mut fired: MessageReader<WeaponEvent>,
    (clock, first): (Res<SimClock>, Res<FirstTimePredicted>),
    mut sounds: MessageWriter<CaZoomSound>,
    mut commands: Commands,
) {
    let now = clock.now;
    let due = now + SLACK;
    let shots: Vec<Entity> = fired
        .read()
        .filter(|e| matches!(e.kind, WeaponEventKind::Fired { .. }))
        .map(|e| e.weapon)
        .collect();
    for (owner, inv, health, have) in &owners {
        if health.is_some_and(|h| h.current <= 0.0) {
            continue;
        }
        let Some(active) = inv.active else { continue };
        let Ok((mut z, st)) = weapons.get_mut(active) else { continue };
        if !z.params.reload_after_fire {
            continue;
        }
        if shots.contains(&active) {
            z.bolt_busy = true;
            if z.last_in && z.level > 0 {
                z.bolt_armed = Some(now);
            }
        } else if z.bolt_busy && due >= st.next_primary {
            // The weapon leaves its fire animation on this update.
            z.bolt_busy = false;
        }
        if let Some(t) = z.bolt_armed
            && z.zoomed()
            && ((now - t) * 1000.0 + 1e-6).floor() > BOLT_ZOOM_OUT_DELAY_MS
        {
            let before = z.level;
            if z.press(st.reloading())
                && first.0
                && let Some(file) = step_sound(&z, before)
            {
                sounds.write(CaZoomSound { owner, file });
            }
        }
        sync_view(&mut commands, owner, have, z.view());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scoped(fov: &[f32], zoom_time: f32) -> CaZoom {
        let mut z = CaZoom::new(ZoomParams { zoom_time, ..default() });
        z.scope = Some(Scope {
            name: String::new(),
            fov: fov.to_vec(),
            sprite: String::new(),
            sprite_scale: 0.0,
            ..Default::default()
        });
        z
    }

    /// Steps of the longest frame until the transition ends: whether a
    /// zoom-out ended it.
    fn settle(z: &mut CaZoom) -> bool {
        (0..10).any(|_| z.step(FRAME_TIME_CAP))
    }

    #[test]
    fn a_press_cycles_the_levels() {
        let mut z = scoped(&[30.0, 10.0], 0.16);
        assert!(z.press(false));
        assert_eq!(z.level, 1);
        assert!(!z.press(false), "during the transition");
        settle(&mut z);
        assert_eq!(z.fov, 30.0);
        assert!(z.press(false));
        settle(&mut z);
        assert_eq!((z.level, z.fov), (2, 10.0));
        assert!(z.press(false));
        assert!(settle(&mut z), "the zoom-out ends");
        assert_eq!((z.level, z.fov, z.zoomed()), (0, FOV_X, false));
    }

    #[test]
    fn no_scope_no_zoom() {
        let mut z = CaZoom::new(ZoomParams::default());
        assert!(!z.press(false));
        assert!(!z.zoomed());
    }

    #[test]
    fn vertical_fov_is_proportional() {
        assert!((vertical_fov(20.0) - 16.6667).abs() < 1e-3);
        assert!((vertical_fov(75.0) - 62.5).abs() < 1e-4);
    }
}
