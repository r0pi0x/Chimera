//! The over-the-shoulder third-person camera, every game's
//! (docs/plans/active/third-person-camera.md, M1): `thirdperson` /
//! `firstperson` or the bindable `chimera:toggle_thirdperson` (unbound)
//! turn it on and off; middle mouse (`chimera:shoulder`, where the game
//! leaves the key free) swaps the shoulder.
//!
//! - **The camera** sits `chimera:cam_up` above the eye, `chimera:cam_side`
//!   to the shoulder (`chimera:cam_shoulder`, 1 right, -1 left; the swap
//!   slides) and `chimera:cam_dist` back, metres. A sphere is swept from
//!   the eye to the pivot, then back from it: a wall pulls the camera in at
//!   once, and it eases back out once clear (`ShoulderSettings::ease_out_secs`).
//!   The server's `chimera:thirdperson_maxoffset` caps its distance from the
//!   eye; its `chimera:allow_thirdperson` (0 by default) keeps a networked
//!   player in first person (`ThirdPersonRules`).
//! - **Aim through the crosshair.** The mouse turns the camera
//!   (`Shoulder::look`). Each frame `aim_through_crosshair` casts the
//!   camera's centre ray from the point on it nearest the eye (so nothing
//!   between the camera and the character counts), takes its first hit (or
//!   the range) as the aim point, and writes the eye-to-aim-point angles
//!   into the command (`Intent::yaw`/`pitch`): the shot still leaves the
//!   eye by each game's rules, and the server traces from its own eye along
//!   those angles as always, so it takes nothing new from the client.
//!   Movement keys are turned so W walks where the camera faces.
//! - **The blocked-shot marker.** When something stands between the eye
//!   and the aim point (`blocked_point`, the shot trace's layers), a dot
//!   marks where the shot would land instead (`Shoulder::blocked`). Melee
//!   and thrown weapons show none.
//! - **Right mouse** on a gun whose right click zooms or aims down sights
//!   (`weapon::Zoom`, a game's `OwnZoom`): a tap (shorter than
//!   `tap_secs`) passes the click to the gun and shows first person while
//!   its zoom lasts; held, the camera zooms over the shoulder (closer,
//!   narrower FOV, slower mouse) and the gun gets no click. Any other
//!   right click (TF2's Minigun spin, a knife's stab) is the gun's.
//!
//! - **Scaled to the body.** The metre settings are for a character of
//!   `REF_EYE` (CS:S's); the distances scale with the standing eye height
//!   seen (`Shoulder::scale`), so a Spartan and a CS:S player frame alike.
//! - **Glide.** The camera moves between the eye and its spot over
//!   `transition_secs` (`Shoulder::blend`), the look slerping with it; input
//!   and the command's aim follow the target mode at once, never blocked.
//!   Death, spectating, map cameras and the free cams take over at once.
//! - **Recoil and body fade.** The view punch shakes the camera (the aim
//!   stays put). The body's parts fade out as the camera nears the eye
//!   (`BodyFade`, a dither band, `ShoulderSettings::fade_hidden`/`fade_whole`),
//!   so a wall at your back or the glide shows no body in the lens; the gun
//!   fades with it.
//! - **HUD.** The blocked-shot marker dims the shared crosshair
//!   (`Shoulder::dim`); target IDs look along the crosshair's ray
//!   (`aim::crosshair_ray`, from `Shoulder::ray`).
//!
//! - **Follow lag and the scope** (M3). The pivot hangs off an anchor that
//!   trails the eye (`follow_step`, a critically damped spring,
//!   `chimera:cam_smooth` seconds, at most `MAX_LAG` behind); the look is
//!   never smoothed, so the aim stays one-to-one. A gun's scope
//!   (`Zoomed::scope`) takes the view to first person at once, for its
//!   overlay, and the camera glides back when it ends.
//! - **Free look** (`+freelook`, Left Alt, held; rebindable in Controls).
//!   The mouse orbits the camera about the character (centred over it, the
//!   spring arm's sweeps as ever) and turns nothing else: the aim and the
//!   character's facing stay put, and the move keys walk along that facing.
//!   The crosshair dims and the marker hides (the camera isn't aiming).
//!   Let go, the camera eases back behind the shoulder
//!   (`Shoulder::ease_free`, `free_return_secs`). Where third person isn't
//!   allowed it is first person's free look about the eye.
//! - **Data** (M4). `data/third_person_camera.toml` holds the defaults
//!   (`ShoulderSettings::default`), the marker's look and each game's
//!   factors on the distances (`CameraData::games`); the archived cvars
//!   `chimera:cam_*` and `chimera:thirdperson` change them. Each game's
//!   HUD dims its own crosshair by `Shoulder::dim` (Combat Arms, TF2,
//!   HL2:DM, Halo; their `crosshair_dim` inputs).
//!
//! Known shortcuts (docs/tech-debt.md): Entering third person hides the view
//! model at once (the body fades in with the glide); leaving shows it when
//! the glide ends. The own-hull and near-plane margins rely on the sphere
//! sweep's radius. A game with half-weight first key presses
//! (`HalfFirstPress`) turns that tick's move share unturned. The sight's
//! range readout and the defuse prompt still use the eye's ray. The
//! camera's recoil shake isn't in the marker's aim. Why it's OK for now:
//! M2 to M4 of the plan are in; the plan lists what is left.

use avian3d::prelude::*;
use bevy::{camera::visibility::VisibilityRange, ecs::system::SystemParam, prelude::*};

use super::{FirstPersonCamera, view::CameraMode};
use crate::{
    console::{ConsoleAppExt, resource_cvar},
    core::{Dead, Intent, LocalPlayer, MovementState, NetRole, SOLID_LAYERS, ThirdPersonRules},
};

/// The follow lag never trails the eye by more than this, metres.
const MAX_LAG: f32 = 0.5;
/// Radius of the sphere swept to place the camera.
const CAMERA_RADIUS: f32 = 0.2;
/// How much of its opacity the crosshair loses while the marker shows.
pub const DIM_AMOUNT: f32 = 0.65;
/// The marker shows when the shot would stop more than this short of the
/// aim point, metres.
const BLOCKED_TOLERANCE: f32 = 0.15;
/// After a tap, seconds the gun has to show its zoom before the camera
/// goes back to third person.
const TAP_GRACE_SECS: f64 = 0.35;
/// Aim range without a hit, game units (Source's trace length).
const RANGE_UNITS: f32 = 8192.0;
const PITCH_LIMIT: f32 = 89f32.to_radians();

/// The shoulder camera's tuning (metres, seconds; archived cvars), by
/// default from `data/third_person_camera.toml`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, serde::Deserialize)]
pub struct ShoulderSettings {
    /// 1 the right shoulder, -1 the left (`chimera:cam_shoulder`).
    pub shoulder: i8,
    /// Back from the pivot (`chimera:cam_dist`).
    pub dist: f32,
    /// Out to the shoulder (`chimera:cam_side`).
    pub side: f32,
    /// The pivot above the eye (`chimera:cam_up`).
    pub up: f32,
    /// While the aim zoom holds: the distance's fraction, the side's, and
    /// the field of view's.
    pub aim_dist: f32,
    pub aim_side: f32,
    pub aim_fov: f32,
    /// The third-person field of view, degrees at 4:3 (`chimera:cam_fov`);
    /// 0 keeps the player's own.
    pub fov: f32,
    /// Follow lag: seconds the pivot takes to settle (`chimera:cam_smooth`);
    /// 0 is off.
    pub smooth: f32,
    pub swap_secs: f32,
    pub zoom_secs: f32,
    pub ease_out_secs: f32,
    /// First to third person and back: the camera's glide, seconds.
    pub transition_secs: f32,
    /// Free look let go: seconds the camera takes to ease back behind the
    /// shoulder.
    pub free_return_secs: f32,
    /// The body fades as the camera nears the eye: hidden at this distance
    /// from it and whole again at `fade_whole`, metres for a character of
    /// `REF_EYE` (scaled with the eye height).
    pub fade_hidden: f32,
    pub fade_whole: f32,
    /// Right mouse let go sooner than this is a tap.
    pub tap_secs: f32,
    /// The blocked-shot marker: pixels across, and its colour.
    pub marker_size: f32,
    pub marker_color: [f32; 4],
}

/// One game's factors on the camera's distances (`CameraData::games`).
#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(default)]
pub struct GameCamera {
    pub dist: f32,
    pub side: f32,
    pub up: f32,
}

impl Default for GameCamera {
    fn default() -> Self {
        Self { dist: 1.0, side: 1.0, up: 1.0 }
    }
}

/// `data/third_person_camera.toml`: the defaults and each game's factors.
#[derive(Resource, Clone, Debug, PartialEq, serde::Deserialize)]
pub struct CameraData {
    pub camera: ShoulderSettings,
    #[serde(default)]
    pub games: std::collections::BTreeMap<String, GameCamera>,
}

impl CameraData {
    /// The embedded file.
    pub fn builtin() -> Self {
        static DATA: std::sync::OnceLock<CameraData> = std::sync::OnceLock::new();
        DATA.get_or_init(|| {
            toml::from_str(include_str!("../../data/third_person_camera.toml")).expect("data/third_person_camera.toml is valid")
        })
        .clone()
    }
}

impl Default for CameraData {
    fn default() -> Self {
        Self::builtin()
    }
}

/// The eye height the metre settings are for (CS:S's standing eye, 64
/// units); a character's distances scale with its own.
pub const REF_EYE: f32 = 1.626;

impl ShoulderSettings {
    /// The distances scaled for a character `scale` times the reference's
    /// eye height: each game's body frames alike.
    pub fn scaled(&self, scale: f32) -> Self {
        Self {
            dist: self.dist * scale,
            side: self.side * scale,
            up: self.up * scale,
            ..*self
        }
    }
}

impl Default for ShoulderSettings {
    fn default() -> Self {
        CameraData::builtin().camera
    }
}

/// Right mouse in third person on a gun that aims with it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AimButton {
    /// When it went down (None: up, or not ours).
    pub down_at: Option<f64>,
    /// Held past a tap: the over-the-shoulder zoom.
    pub hold_zoom: bool,
    /// A tap gave the gun its click at this time: first person while the
    /// gun's zoom lasts.
    pub tap_zoom: Option<f64>,
}

impl AimButton {
    /// One frame of the button: (held for the gun, a press to give the
    /// gun). While a tap's zoom shows, the button is the gun's.
    pub fn step(&mut self, now: f64, held: bool, went_down: bool, tap_secs: f32) -> (bool, bool) {
        if self.tap_zoom.is_some() {
            if went_down {
                self.tap_zoom = Some(now);
            }
            return (held, went_down);
        }
        if went_down {
            self.down_at = Some(now);
        }
        match self.down_at {
            Some(at) if held => {
                if now - at >= tap_secs as f64 {
                    self.hold_zoom = true;
                }
                (false, false)
            }
            Some(_) => {
                let tap = !self.hold_zoom;
                self.down_at = None;
                self.hold_zoom = false;
                if tap {
                    self.tap_zoom = Some(now);
                }
                (false, tap)
            }
            None => (false, false),
        }
    }

    /// The gun's zoom (`zoomed`) ended after a tap: back to third person.
    pub fn settle(&mut self, now: f64, zoomed: bool) {
        if let Some(at) = self.tap_zoom
            && !zoomed
            && now - at > TAP_GRACE_SECS
        {
            self.tap_zoom = None;
        }
    }
}

/// The shoulder camera's state.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct Shoulder {
    /// The shoulder camera is what's drawn this frame.
    pub on: bool,
    /// The camera's own look (yaw, pitch, radians) while on.
    pub look: Option<Vec2>,
    /// `cam_idealyaw` orbits the camera (a debug view): the mouse turns
    /// the aim as in first person.
    pub orbit: bool,
    /// Where between the shoulders the camera is (-1 left .. 1 right).
    pub side: f32,
    /// The aim zoom, 0 .. 1.
    pub zoom: f32,
    /// Metres back from the pivot as placed last (pulled in by walls,
    /// easing back out).
    pub reach: Option<f32>,
    /// Pivot offset from the eye as placed last.
    pub pivot: Vec3,
    pub aim: AimButton,
    /// How far the camera has glided from the eye to its spot, 0 (first
    /// person) .. 1; moves over `transition_secs` while `on` changes.
    pub blend: f32,
    /// The look the camera had when it left (gliding back to the eye).
    pub exit_look: Option<Vec2>,
    /// The standing eye height seen (metres) and the scale it gives the
    /// distances.
    pub stand_eye: f32,
    /// The crosshair dims while the blocked-shot marker shows: 0 .. 1.
    pub dim: f32,
    /// The crosshair's ray as drawn (from the point on it nearest the
    /// eye): what the HUD's target IDs look along.
    pub ray: Option<(Vec3, Dir3)>,
    /// Where the crosshair's ray lands, and where the shot would stop
    /// short of it (the marker), world space; this frame's.
    pub aim_point: Option<Vec3>,
    pub blocked: Option<Vec3>,
    /// The follow lag's pivot anchor and its speed (world space): trails
    /// the eye (`ShoulderSettings::smooth`).
    pub follow: Option<(Vec3, Vec3)>,
    /// The game's factors on the distances (`CameraData::games`).
    pub game: GameCamera,
    /// Free look (`+freelook` held): the camera's orbit about the
    /// character as drawn (yaw, pitch offsets from `look`, radians), and,
    /// once let go, the offset it eases back from and the seconds since.
    pub free: Vec2,
    pub free_back: Option<(Vec2, f32)>,
}

impl Shoulder {
    /// The distances' scale for the character's eye height.
    pub fn scale(&self) -> f32 {
        if self.stand_eye > 0.0 { self.stand_eye / REF_EYE } else { 1.0 }
    }

    /// The camera is drawn off the eye this frame (on, or gliding back).
    pub fn drawn(&self) -> bool {
        self.on || self.blend > 0.0
    }

    /// `settings` with the distances scaled for the body (`scale`) and the
    /// game (`game`).
    pub fn tuned(&self, settings: &ShoulderSettings) -> ShoulderSettings {
        let s = settings.scaled(self.scale());
        ShoulderSettings {
            dist: s.dist * self.game.dist,
            side: s.side * self.game.side,
            up: s.up * self.game.up,
            ..s
        }
    }

    /// Free look orbits the camera (held, or easing back): the crosshair
    /// isn't the aim then.
    pub fn orbiting(&self) -> bool {
        self.free != Vec2::ZERO
    }

    /// The free look's orbit drawn this frame, given the held offset
    /// (`input::FreeLook`; zero when let go): it follows the held one at
    /// once; let go, it eases back to zero over `secs` (smoothstep).
    pub fn ease_free(&mut self, held: Vec2, secs: f32, dt: f32) -> Vec2 {
        if held != Vec2::ZERO {
            self.free = held;
            self.free_back = None;
            return held;
        }
        if self.free == Vec2::ZERO {
            self.free_back = None;
            return Vec2::ZERO;
        }
        let (from, t) = self.free_back.unwrap_or((self.free, 0.0));
        let t = t + dt;
        if secs <= 0.0 || t >= secs {
            self.free = Vec2::ZERO;
            self.free_back = None;
            return Vec2::ZERO;
        }
        let k = t / secs;
        let k = k * k * (3.0 - 2.0 * k);
        self.free = from * (1.0 - k);
        self.free_back = Some((from, t));
        self.free
    }

    /// The look as a rotation.
    pub fn rotation(&self) -> Option<Quat> {
        self.look.map(|l| Quat::from_euler(EulerRot::YXZ, l.x, l.y, 0.0))
    }
}

/// The eye's height above the character's feet: its offset from the
/// origin less the hull's bottom (the capsule's, when the movement has no
/// hull).
pub fn eye_height(state: &MovementState) -> f32 {
    let feet = if state.hull_min.y != 0.0 { state.hull_min.y } else { -crate::character::CAPSULE_HEIGHT / 2.0 };
    state.eye_offset.y - feet
}

/// The pivot's offset from the eye (up, then out to the shoulder `side`,
/// -1..1) and the camera's distance back from it, with the aim zoom
/// `zoom` (0..1), before walls.
pub fn wanted_offsets(s: &ShoulderSettings, side: f32, zoom: f32, look: Quat) -> (Vec3, f32) {
    let right = look * Vec3::X;
    let right = Vec3::new(right.x, 0.0, right.z).normalize_or(Vec3::X);
    let side_m = s.side * side * (1.0 + (s.aim_side - 1.0) * zoom);
    let dist = s.dist * (1.0 + (s.aim_dist - 1.0) * zoom);
    (Vec3::Y * s.up + right * side_m, dist.max(0.0))
}

/// The camera's offset from the eye: the pivot, then `reach` back along
/// the look; no further than `max` from the eye.
pub fn camera_from_eye(pivot: Vec3, reach: f32, look: Quat, max: f32) -> Vec3 {
    let o = pivot + look * Vec3::Z * reach;
    if max > 0.0 && o.length() > max { o * (max / o.length()) } else { o }
}

/// One step of a critically damped spring: `pos` and `vel` toward `target`
/// settling in about `smooth` seconds (no overshoot). 0: at the target.
pub fn follow_step(pos: Vec3, vel: Vec3, target: Vec3, smooth: f32, dt: f32) -> (Vec3, Vec3) {
    if smooth <= 0.0 || dt <= 0.0 {
        return (target, Vec3::ZERO);
    }
    let w = 2.0 / smooth;
    let e = (-w * dt).exp();
    let x = pos - target;
    let t = (vel + x * w) * dt;
    (target + (x + t) * e, (vel - t * w) * e)
}

/// Sweep the camera's sphere from `from` along `to - from`: how far it gets.
fn sweep(spatial: &SpatialQuery, filter: &SpatialQueryFilter, from: Vec3, to: Vec3) -> Vec3 {
    let Ok((dir, len)) = Dir3::new_and_length(to - from) else {
        return to;
    };
    let config = ShapeCastConfig {
        max_distance: len,
        ignore_origin_penetration: true,
        ..default()
    };
    let d = spatial
        .cast_shape(&Collider::sphere(CAMERA_RADIUS), from, Quat::IDENTITY, dir, &config, filter)
        .map_or(len, |h| h.distance);
    from + dir * d
}

/// The pivot (from the eye) and how far back the camera fits, against what
/// characters collide with (no characters).
pub fn fit(spatial: &SpatialQuery, filter: &SpatialQueryFilter, eye: Vec3, pivot: Vec3, dist: f32, look: Quat) -> (Vec3, f32) {
    let p = sweep(spatial, filter, eye, eye + pivot);
    let back = look * Vec3::Z;
    let c = sweep(spatial, filter, p, p + back * dist);
    (p - eye, c.distance(p))
}

/// The angles (yaw, pitch) of a direction (`Intent`'s: yaw 0 faces -Z).
pub fn angles_of(dir: Vec3) -> Vec2 {
    let d = dir.normalize_or(Vec3::NEG_Z);
    Vec2::new((-d.x).atan2(-d.z).rem_euclid(std::f32::consts::TAU), d.y.clamp(-1.0, 1.0).asin())
}

/// Where the camera's centre ray (`camera`, `dir`) lands: cast from its
/// point nearest the eye (what lies between the camera and the character
/// never counts), on the shot's layers, leaving out `exclude` (the
/// shooter); the point `range` metres along when nothing is hit.
pub fn aim_point(spatial: &SpatialQuery, camera: Vec3, dir: Dir3, eye: Vec3, exclude: &[Entity], range: f32) -> Vec3 {
    let start = (eye - camera).dot(*dir).max(0.0);
    let from = camera + *dir * start;
    let filter = SpatialQueryFilter::from_excluded_entities(exclude.iter().copied()).with_mask(SOLID_LAYERS);
    let d = spatial.cast_ray(from, dir, range, true, &filter).map_or(range, |h| h.distance);
    from + *dir * d
}

/// Where a shot from `eye` toward `aim` stops first, when that is short of
/// `aim` (by more than `BLOCKED_TOLERANCE`): the blocked-shot marker.
pub fn blocked_point(spatial: &SpatialQuery, eye: Vec3, aim: Vec3, exclude: &[Entity]) -> Option<Vec3> {
    let (dir, len) = Dir3::new_and_length(aim - eye).ok()?;
    let filter = SpatialQueryFilter::from_excluded_entities(exclude.iter().copied()).with_mask(SOLID_LAYERS);
    let hit = spatial.cast_ray(eye, dir, len, true, &filter)?;
    (hit.distance < len - BLOCKED_TOLERANCE).then(|| eye + dir * hit.distance)
}

/// A move (right, forward) meant along the camera's yaw, as the same
/// world direction along the aim's yaw.
pub fn turn_move(axis: Vec2, camera_yaw: f32, aim_yaw: f32) -> Vec2 {
    let fwd = |y: f32| Vec2::new(-y.sin(), -y.cos());
    let right = |y: f32| Vec2::new(y.cos(), -y.sin());
    let wish = right(camera_yaw) * axis.x + fwd(camera_yaw) * axis.y;
    Vec2::new(wish.dot(right(aim_yaw)), wish.dot(fwd(aim_yaw))).clamp(Vec2::NEG_ONE, Vec2::ONE)
}

/// What `input::write_local_intent` needs of the shoulder camera.
#[derive(SystemParam)]
pub struct ShoulderInput<'w, 's> {
    pub state: Option<ResMut<'w, Shoulder>>,
    settings: Option<Res<'w, ShoulderSettings>>,
    inventory: Query<'w, 's, &'static crate::weapon::Inventory, With<LocalPlayer>>,
    aims: Query<'w, 's, (), Or<(With<crate::weapon::Zoom>, With<crate::weapon::OwnZoom>)>>,
}

impl ShoulderInput<'_, '_> {
    /// The camera's look the mouse turns (the shoulder camera on, not
    /// orbiting).
    pub fn turning(&mut self) -> Option<&mut Vec2> {
        let s = self.state.as_mut()?;
        if !s.on || s.orbit {
            return None;
        }
        s.look.as_mut()
    }

    /// Turn the camera's look (pitch within the limit).
    pub fn turn(look: &mut Vec2, turn: Vec2) {
        look.x = (look.x + turn.x).rem_euclid(std::f32::consts::TAU);
        look.y = (look.y + turn.y).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// The turn multiplier while the aim zoom narrows the view.
    pub fn zoom_scale(&self, mouse: &super::input::MouseSettings) -> f32 {
        match (&self.state, &self.settings) {
            (Some(s), Some(t)) if s.on && s.zoom > 0.0 => mouse.zoom_scale(Some(if t.fov > 0.0 { t.fov } else { 90.0 } * (1.0 + (t.aim_fov - 1.0) * s.zoom))),
            _ => 1.0,
        }
    }

    /// Right mouse: (held for the gun, a press for the gun), given the
    /// keys' (held, went down). Unchanged unless the shoulder camera is
    /// on (or a tap's zoom shows) and the gun aims with it.
    pub fn secondary(&mut self, now: f64, held: bool, went_down: bool) -> (bool, bool) {
        let aims = self
            .inventory
            .iter()
            .next()
            .and_then(|i| i.active)
            .is_some_and(|w| self.aims.contains(w));
        let tap = self.settings.as_ref().map_or(0.2, |s| s.tap_secs);
        let Some(s) = self.state.as_mut() else {
            return (held, went_down);
        };
        if !aims || (!s.on && s.aim.tap_zoom.is_none()) {
            s.aim.down_at = None;
            s.aim.hold_zoom = false;
            return (held, went_down);
        }
        s.aim.step(now, held, went_down, tap)
    }
}

pub struct ThirdPersonPlugin;

impl Plugin for ThirdPersonPlugin {
    fn build(&self, app: &mut App) {
        use super::input::LocalInputSystems;
        super::input::configure_local_input(app);
        app.init_resource::<Shoulder>()
            .init_resource::<CameraMode>()
            .init_resource::<super::input::FreeLook>()
            .init_resource::<ShoulderSettings>()
            .init_resource::<CameraData>()
            .init_resource::<ThirdPersonRules>()
            .add_systems(
                RunFixedMainLoop,
                (
                    update_shoulder
                        .in_set(LocalInputSystems::Write)
                        .before(super::input::write_local_intent),
                    aim_through_crosshair
                        .in_set(LocalInputSystems::Write)
                        .after(super::input::write_local_intent),
                ),
            )
            .add_systems(Update, (draw_marker, fade_body).after(super::follow_eye));
        console(app);
    }
}

fn console(app: &mut App) {
    // Scenes and scripts have no mouse: turn the held free look by degrees.
    app.console_command(
        "chimera:freelook_turn",
        "chimera:freelook_turn <yaw> <pitch>: turn the free look by degrees (while +freelook is held; for scenes).",
        |w, a| {
            let deg = |i: usize| a.get(i).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0).to_radians();
            let mut f = w.resource_mut::<super::input::FreeLook>();
            f.yaw = (f.yaw + deg(0) + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI;
            f.pitch = (f.pitch + deg(1)).clamp(-PITCH_LIMIT, PITCH_LIMIT);
            Ok(None)
        },
    );
    app.console_command("chimera:shoulder", "Swap the third-person camera's shoulder.", |w, _| {
        let mut s = w.resource_mut::<ShoulderSettings>();
        s.shoulder = if s.shoulder < 0 { 1 } else { -1 };
        Ok(None)
    })
    .console_command(
        "chimera:toggle_thirdperson",
        "Switch between first person and the over-the-shoulder camera.",
        |w, _| {
            let mut m = w.resource_mut::<CameraMode>();
            m.third_person = !m.third_person;
            Ok(None)
        },
    );
    resource_cvar::<ShoulderSettings, i8>(
        app,
        "chimera:cam_shoulder",
        "Third-person camera's shoulder: 1 right, -1 left.",
        |s| &mut s.shoulder,
    );
    resource_cvar::<ShoulderSettings, f32>(
        app,
        "chimera:cam_dist",
        "Third-person camera's distance behind the shoulder, metres.",
        |s| &mut s.dist,
    );
    resource_cvar::<ShoulderSettings, f32>(
        app,
        "chimera:cam_side",
        "Third-person camera's distance out to the shoulder, metres.",
        |s| &mut s.side,
    );
    resource_cvar::<ShoulderSettings, f32>(
        app,
        "chimera:cam_up",
        "Third-person camera's height above the eye, metres.",
        |s| &mut s.up,
    );
    resource_cvar::<ShoulderSettings, f32>(
        app,
        "chimera:cam_fov",
        "Third-person camera's field of view, degrees at 4:3 (0: yours).",
        |s| &mut s.fov,
    );
    resource_cvar::<ShoulderSettings, f32>(
        app,
        "chimera:cam_smooth",
        "Third-person camera's follow lag, seconds (0: none).",
        |s| &mut s.smooth,
    );
    app.console_cvar(
        "chimera:thirdperson",
        "The over-the-shoulder camera on (1) or off (0): the same as thirdperson / firstperson.",
        "0",
        |w| w.get_resource::<CameraMode>().map(|m| (m.third_person as u8).to_string()),
        |w, v| {
            let on = match v.trim() {
                "0" => false,
                "1" => true,
                _ => return Err(format!("bad value \"{v}\"")),
            };
            w.get_resource_mut::<CameraMode>().ok_or("not available")?.third_person = on;
            Ok(())
        },
    );
    let mut c = app.world_mut().resource_mut::<crate::console::Console>();
    c.set_range("chimera:cam_shoulder", -1.0, 1.0);
    c.set_range("chimera:cam_smooth", 0.0, 0.5);
    c.set_range("chimera:cam_fov", 0.0, 140.0);
    c.set_range("chimera:thirdperson", 0.0, 1.0);
    for name in [
        "chimera:cam_shoulder",
        "chimera:cam_dist",
        "chimera:cam_side",
        "chimera:cam_up",
        "chimera:cam_fov",
        "chimera:cam_smooth",
        "chimera:thirdperson",
    ] {
        c.archive(name);
    }
}

/// Whether the shoulder camera is on; its look starts from the aim; the
/// shoulder swap, the aim zoom and the glide to and from the eye slide.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn update_shoulder(
    mut mode: ResMut<CameraMode>,
    mut state: ResMut<Shoulder>,
    settings: Res<ShoulderSettings>,
    rules: Res<ThirdPersonRules>,
    role: Option<Res<NetRole>>,
    time: Res<Time>,
    data: Res<CameraData>,
    active: Option<Res<crate::map::hud::ActiveHud>>,
    player: Option<
        Single<(&Intent, &MovementState, Has<Dead>, Has<crate::core::MapView>, Option<&crate::weapon::Zoomed>, Option<&crate::slots::GameIdentity>), With<LocalPlayer>>,
    >,
    others: (
        Option<Res<super::view::FreeCam>>,
        Option<Res<super::view::Watch>>,
        Option<Res<super::spectate::SpecView>>,
    ),
) {
    let now = time.elapsed_secs_f64();
    let zoomed = player.as_ref().is_some_and(|p| p.4.is_some());
    // A scope's overlay is first person's: the camera leaves for it.
    let scoped = player.as_ref().is_some_and(|p| p.4.is_some_and(|z| z.scope));
    state.aim.settle(now, zoomed);
    let game = player
        .as_ref()
        .and_then(|p| p.5.map(|g| g.0.as_str()))
        .or(active.as_ref().map(|a| a.0.game.as_str()))
        .and_then(|g| data.games.get(g).copied())
        .unwrap_or_default();
    if state.game != game {
        state.game = game;
    }
    let allowed = rules.allows(role.map_or(NetRole::Standalone, |r| *r));
    let held_off = !allowed || state.aim.tap_zoom.is_some() || scoped;
    if mode.held_off != held_off {
        mode.held_off = held_off;
    }
    let (free, watch, spec) = others;
    let elsewhere = free.is_some_and(|f| f.mode != 0)
        || watch.is_some_and(|w| w.0 != 0)
        || spec.is_some_and(|s| s.pose.is_some());
    // Death, spectating and map cameras take over at once.
    let usable = !elsewhere && player.as_ref().is_some_and(|p| !p.2 && !p.3);
    let on = mode.third() && usable;
    let orbit = mode.ideal_yaw != 0.0;
    let s = &mut *state;
    s.orbit = orbit;
    let dt = time.delta_secs();
    let step = |at: f32, to: f32, secs: f32, span: f32| {
        let rate = if secs > 0.0 { span / secs * dt } else { f32::INFINITY };
        at + (to - at).clamp(-rate, rate)
    };
    if !usable || (!on && s.blend <= 0.0) {
        if s.on || s.look.is_some() || s.blend > 0.0 || s.exit_look.is_some() {
            s.on = false;
            s.look = None;
            s.exit_look = None;
            s.blend = 0.0;
            s.reach = None;
            s.zoom = 0.0;
            s.aim.down_at = None;
            s.aim.hold_zoom = false;
            s.blocked = None;
            s.aim_point = None;
            s.ray = None;
            s.dim = 0.0;
            s.free = Vec2::ZERO;
            s.free_back = None;
        }
        return;
    }
    // The standing eye: the highest seen since the camera came on.
    let was_drawn = s.drawn();
    if let Some(p) = player.as_ref() {
        let h = eye_height(p.1);
        s.stand_eye = if was_drawn { s.stand_eye.max(h) } else { h };
    }
    // The scope takes the view at once (no glide); leaving it glides out.
    let secs = if scoped { 0.0 } else { settings.transition_secs };
    s.blend = step(s.blend, if on { 1.0 } else { 0.0 }, secs, 1.0);
    let to_zoom = if on && s.aim.hold_zoom { 1.0 } else { 0.0 };
    s.zoom = step(s.zoom, to_zoom, settings.zoom_secs, 1.0);
    if !on {
        // Gliding back to the eye: the look it left with, until it's there.
        if s.on {
            s.on = false;
            s.exit_look = s.look;
            s.aim.down_at = None;
            s.aim.hold_zoom = false;
            s.blocked = None;
        }
        return;
    }
    s.exit_look = None;
    let intent = player.as_ref().map(|p| p.0);
    if s.look.is_none() || orbit {
        s.look = intent.map(|i| Vec2::new(i.yaw, i.pitch));
        if !was_drawn {
            s.side = settings.shoulder.signum() as f32;
        }
    }
    s.on = true;
    // Free look orbits about the character: the camera centres over it.
    let to_side = if s.orbiting() {
        0.0
    } else if settings.shoulder < 0 {
        -1.0
    } else {
        1.0
    };
    s.side = step(s.side, to_side, settings.swap_secs, 2.0);
}

/// The command's angles from the eye to where the crosshair points, and
/// the move keys turned to the camera's facing.
#[allow(clippy::type_complexity)]
fn aim_through_crosshair(
    mut player: Option<Single<(Entity, &Transform, &mut Intent, &MovementState), With<LocalPlayer>>>,
    state: Res<Shoulder>,
    settings: Res<ShoulderSettings>,
    rules: Res<ThirdPersonRules>,
    mode: Res<CameraMode>,
    free: Res<super::input::FreeLook>,
    spatial: SpatialQuery,
) {
    let Some((me, at, intent, ms)) = player.as_deref_mut() else { return };
    let (Some(look), Some(rot)) = (state.look, state.rotation()) else { return };
    if !state.on || state.orbit || *free != super::input::FreeLook::default() {
        return;
    }
    let eye = at.translation + crate::map::interp::EyeView::of(intent, ms).eye_offset;
    let (pivot, dist) = wanted_offsets(&state.tuned(&settings), state.side, state.zoom, rot);
    let pivot = if state.reach.is_some() { state.pivot } else { pivot };
    let camera = eye + camera_from_eye(pivot, state.reach.unwrap_or(dist).min(dist), rot, rules.max_offset);
    let Ok(dir) = Dir3::new(rot * Vec3::NEG_Z) else { return };
    let point = aim_point(&spatial, camera, dir, eye, &[*me], RANGE_UNITS * mode.meters_per_unit);
    // Too near the eye to turn by: along the camera's look.
    let aim = if point.distance(eye) > 0.3 { angles_of(point - eye) } else { look };
    let aim = Vec2::new(aim.x, aim.y.clamp(-PITCH_LIMIT, PITCH_LIMIT));
    intent.move_axis = turn_move(intent.move_axis, look.x, aim.x);
    intent.yaw = aim.x;
    intent.pitch = aim.y;
}

/// What `follow_eye` needs to place the shoulder camera.
#[derive(SystemParam)]
pub struct ShoulderView<'w> {
    pub state: Option<ResMut<'w, Shoulder>>,
    settings: Option<Res<'w, ShoulderSettings>>,
    rules: Option<Res<'w, ThirdPersonRules>>,
    time: Res<'w, Time>,
}

impl ShoulderView<'_> {
    /// The camera's offset from the character's origin and its look, when
    /// the shoulder camera is drawn (`look` from the intent when it orbits:
    /// `orbit_look`; `kick` is the view punch: pitch, yaw, roll as
    /// `follow_eye` has it, and `fp_look` the first-person look the camera
    /// glides to and from). Walls pull it in at once; it eases back out.
    #[allow(clippy::too_many_arguments)]
    pub fn place(
        &mut self,
        origin: Vec3,
        eye: Vec3,
        orbit_look: Quat,
        fp_look: Quat,
        kick: Vec3,
        free: Vec2,
        spatial: &SpatialQuery,
        filter: &SpatialQueryFilter,
    ) -> Option<(Vec3, Quat)> {
        let dt = self.time.delta_secs();
        let settings = self.settings.as_deref().copied().unwrap_or_default();
        let max = self.rules.as_deref().map_or(0.0, |r| r.max_offset);
        let s = self.state.as_mut()?;
        if !s.drawn() {
            s.follow = None;
            return None;
        }
        // Free look: the held orbit, or easing back once let go.
        let free = s.ease_free(free, settings.free_return_secs, dt);
        let look = if s.orbit {
            orbit_look
        } else {
            let l = if s.on { s.look } else { s.exit_look.or(s.look) }?;
            // The recoil shakes the camera as it does the first-person
            // view (the aim, and so the crosshair, stays put).
            Quat::from_euler(
                EulerRot::YXZ,
                l.x + free.x + kick.y,
                (l.y + free.y + kick.x).clamp(-PITCH_LIMIT, PITCH_LIMIT),
                -kick.z,
            )
        };
        let settings = s.tuned(&settings);
        let (pivot, dist) = wanted_offsets(&settings, s.side, s.zoom, look);
        // Follow lag: the pivot hangs off an anchor that trails the eye (a
        // critically damped spring); the look is never smoothed. Crouching,
        // landing and stairs ease with it.
        let eye_at = origin + eye;
        let lag = match s.follow {
            Some((pos, vel)) if pos.distance(eye_at) < 4.0 => {
                let (pos, vel) = follow_step(pos, vel, eye_at, settings.smooth, dt);
                let lag = (pos - eye_at).clamp_length_max(MAX_LAG);
                s.follow = Some((eye_at + lag, vel));
                lag
            }
            _ => {
                s.follow = Some((eye_at, Vec3::ZERO));
                Vec3::ZERO
            }
        };
        let (pivot, fits) = fit(spatial, filter, eye_at + lag, pivot, dist, look);
        let pivot = pivot + lag;
        let reach = match s.reach {
            Some(r) if fits >= r => {
                let rate = if settings.ease_out_secs > 0.0 { dist / settings.ease_out_secs * dt } else { f32::INFINITY };
                (r + rate).min(fits)
            }
            _ => fits,
        };
        s.reach = Some(reach);
        s.pivot = pivot;
        let spot = eye + camera_from_eye(pivot, reach, look, max);
        // Gliding between the eye and the spot.
        let t = s.blend.clamp(0.0, 1.0);
        let t = t * t * (3.0 - 2.0 * t);
        Some((eye.lerp(spot, t), fp_look.slerp(look, t)))
    }
}

/// Marks a mesh of the local body that fades with the camera's distance.
#[derive(Component)]
struct BodyFade;

/// The body fades as the camera nears the eye (a wall pulled it in, or it
/// glides to and from the eye): a dither band (`VisibilityRange`, as the
/// props') hidden at `fade_hidden` from the eye and whole at `fade_whole`,
/// scaled with the eye height. A part's own distance is the camera's to its
/// origin (what the shader measures), so each part's band is shifted by how much farther it is than
/// the eye: every part fades together, the gun with the body.
#[allow(clippy::type_complexity)]
fn fade_body(
    state: Res<Shoulder>,
    settings: Res<ShoulderSettings>,
    local: Option<Single<(&Children, &Transform, &MovementState), With<LocalPlayer>>>,
    camera: Query<&GlobalTransform, With<FirstPersonCamera>>,
    bodies: Query<(), With<crate::render::bodies::CharacterBody>>,
    children: Query<&Children>,
    meshes: Query<(Option<&VisibilityRange>, Has<BodyFade>, &GlobalTransform), With<Mesh3d>>,
    mut commands: Commands,
) {
    let Some((kids, at, ms)) = local.map(|l| *l) else { return };
    let Some(cam) = camera.iter().next().map(|c| c.translation()) else { return };
    let eye = at.translation + ms.eye_offset;
    let from_eye = cam.distance(eye);
    let s = state.scale();
    let (hidden, whole) = (settings.fade_hidden * s, settings.fade_whole * s);
    for body in kids.iter().filter(|c| bodies.contains(*c)) {
        for d in children.iter_descendants(body).chain(std::iter::once(body)) {
            let Ok((range, ours, gt)) = meshes.get(d) else { continue };
            if !state.drawn() {
                if ours {
                    commands.entity(d).remove::<(VisibilityRange, BodyFade)>();
                }
                continue;
            }
            // The shader measures from the mesh's origin.
            let shift = cam.distance(gt.translation()) - from_eye;
            let band = VisibilityRange {
                start_margin: hidden + shift..whole + shift,
                end_margin: f32::MAX..f32::MAX,
                use_aabb: false,
            };
            if range != Some(&band) {
                commands.entity(d).insert((band, BodyFade));
            }
        }
    }
}

#[derive(Component)]
struct Marker;

/// The blocked-shot marker: where the shot stops short of the crosshair's
/// point, from the camera as drawn this frame.
#[allow(clippy::type_complexity)]
fn draw_marker(
    mut state: ResMut<Shoulder>,
    settings: Res<ShoulderSettings>,
    mode: Res<CameraMode>,
    time: Res<Time>,
    player: Option<Single<(Entity, &Transform, &Intent, &MovementState, Option<&crate::weapon::Inventory>), With<LocalPlayer>>>,
    camera: Query<(&Camera, &Transform), With<FirstPersonCamera>>,
    no_marker: Query<(), Or<(With<crate::weapon::Melee>, With<crate::weapon::grenade::Throwable>)>>,
    spatial: SpatialQuery,
    mut marker: Query<(&mut Node, &mut Visibility), With<Marker>>,
    mut commands: Commands,
) {
    let mut shown = None;
    let mut aim = None;
    let mut ray = None;
    // Free look: the camera isn't aiming, so no marker or crosshair ray
    // (the crosshair dims instead).
    let orbiting = state.orbiting();
    if state.on
        && !orbiting
        && let Some((me, at, intent, ms, inv)) = player.as_deref().copied()
        && let Some((cam, t)) = camera.iter().next()
    {
        let view = GlobalTransform::from(at.mul_transform(*t));
        let eye = at.translation + crate::map::interp::EyeView::of(intent, ms).eye_offset;
        let point = aim_point(&spatial, view.translation(), view.forward(), eye, &[me], RANGE_UNITS * mode.meters_per_unit);
        aim = Some(point);
        let dir = view.forward();
        let start = (eye - view.translation()).dot(*dir).max(0.0);
        ray = Some((view.translation() + *dir * start, dir));
        let marks = !inv.and_then(|i| i.active).is_some_and(|w| no_marker.contains(w));
        if marks && let Some(hit) = blocked_point(&spatial, eye, point, &[me]) {
            shown = Some(hit);
            if let Ok(p) = cam.world_to_viewport(&view, hit)
                && let Ok((mut node, mut vis)) = marker.single_mut()
            {
                node.left = px(p.x - settings.marker_size / 2.0);
                node.top = px(p.y - settings.marker_size / 2.0);
                vis.set_if_neq(Visibility::Visible);
            }
        }
    }
    if state.blocked != shown || state.aim_point != aim || state.ray != ray {
        state.blocked = shown;
        state.aim_point = aim;
        state.ray = ray;
    }
    // The crosshair dims while the marker shows.
    let dim = (shown.is_some() || (state.on && orbiting)) as u8 as f32;
    let rate = time.delta_secs() * 8.0;
    let eased = state.dim + (dim - state.dim).clamp(-rate, rate);
    if eased != state.dim {
        state.dim = eased;
    }
    match marker.single_mut() {
        Ok((_, mut vis)) if shown.is_none() => {
            vis.set_if_neq(Visibility::Hidden);
        }
        Err(_) if state.on => {
            let c = settings.marker_color;
            commands.spawn((
                Marker,
                Name::new("Blocked-shot marker"),
                Node {
                    position_type: PositionType::Absolute,
                    width: px(settings.marker_size),
                    height: px(settings.marker_size),
                    border: UiRect::all(px(2.0)),
                    border_radius: BorderRadius::MAX,
                    ..default()
                },
                BackgroundColor(Color::srgba(c[0], c[1], c[2], c[3])),
                BorderColor::all(Color::srgba(0.0, 0.0, 0.0, 0.8)),
                GlobalZIndex(5),
                Visibility::Hidden,
            ));
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_tap_gives_the_gun_its_click_and_a_hold_zooms_over_the_shoulder() {
        let mut b = AimButton::default();
        // Down, up 0.1 s later: a tap; the gun gets one press.
        assert_eq!(b.step(0.0, true, true, 0.2), (false, false));
        assert_eq!(b.step(0.1, false, false, 0.2), (false, true));
        assert_eq!(b.tap_zoom, Some(0.1));
        // While its zoom shows, the button is the gun's.
        assert_eq!(b.step(0.5, true, true, 0.2), (true, true));
        b.settle(1.0, false);
        assert_eq!(b.tap_zoom, None, "the zoom ended: back to third person");
        // Held past the tap: the camera's zoom, nothing for the gun.
        b.step(2.0, true, true, 0.2);
        assert_eq!(b.step(2.3, true, false, 0.2), (false, false));
        assert!(b.hold_zoom);
        assert_eq!(b.step(2.5, false, false, 0.2), (false, false));
        assert!(!b.hold_zoom && b.tap_zoom.is_none());
    }

    #[test]
    fn angles_point_along_the_direction() {
        let a = angles_of(Vec3::NEG_Z);
        assert!(a.length() < 1e-6);
        let a = angles_of(Vec3::new(-1.0, 1.0, 0.0));
        assert!((a.x - std::f32::consts::FRAC_PI_2).abs() < 1e-5 && (a.y - std::f32::consts::FRAC_PI_4).abs() < 1e-5);
        let q = Quat::from_euler(EulerRot::YXZ, a.x, a.y, 0.0);
        assert!((q * Vec3::NEG_Z).distance(Vec3::new(-1.0, 1.0, 0.0).normalize()) < 1e-5);
    }

    #[test]
    fn the_walk_keeps_the_camera_s_facing() {
        // Camera faces yaw 0 (-Z), the aim is 10 degrees left: W still
        // walks -Z.
        let m = turn_move(Vec2::Y, 0.0, 10f32.to_radians());
        let aim = 10f32.to_radians();
        let world = Vec2::new(aim.cos(), -aim.sin()) * m.x + Vec2::new(-aim.sin(), -aim.cos()) * m.y;
        assert!(world.distance(Vec2::new(0.0, -1.0)) < 1e-5, "{world}");
    }

    #[test]
    fn the_shoulder_sits_right_above_and_behind() {
        let s = ShoulderSettings::default();
        let (pivot, dist) = wanted_offsets(&s, 1.0, 0.0, Quat::IDENTITY);
        assert!(pivot.x > 0.5 && pivot.y > 0.0 && dist > 2.0);
        let (left, _) = wanted_offsets(&s, -1.0, 0.0, Quat::IDENTITY);
        assert!(left.x < -0.5);
        let (_, near) = wanted_offsets(&s, 1.0, 1.0, Quat::IDENTITY);
        assert!(near < dist * 0.6, "the aim zoom comes closer");
        let o = camera_from_eye(pivot, dist, Quat::IDENTITY, 1.0);
        assert!((o.length() - 1.0).abs() < 1e-5, "the server's cap");
    }
}
