//! Where the view model and its effects go (specs/combat_arms/
//! view_models.md 4, 5, 9 and 10), as plain maths in the game's frame
//! (left-handed, +Y up, units; camera space +X right, +Y up, +Z ahead):
//! the model's camera-space origin, the shot jiggle, the world flash point
//! and breach point, the fields of view and the muzzle flash's timer,
//! and how dynamic lights fall on models (the view model included).
//!
//! Known shortcuts: the drawn falloff is Chimera's prop shader's (1 −
//! d²/r²) × cosine (`render::prop.wesl`'s `dynamic_light`), not the
//! game's 1 / (1 + 19·d²/r²) (`model_light_share`), and Chimera's light
//! count, not the game's per-model choice of the strongest; the effect
//! lights reach the view model's layer (`ui::effects`), placed in the
//! world, which the view model's camera draws at the eye.

use bevy::math::Vec3;

/// `pv_fov_y`, `world_fov_y` (degrees, vertical).
pub const PV_FOV_Y: f32 = 50.0;
pub const WORLD_FOV_Y: f32 = 75.0;
/// `pv_near`, `pv_far` (units).
pub const PV_NEAR: f32 = 0.1;
pub const PV_FAR: f32 = 7.0;
/// `flash_time`: the stock first-person muzzle effect's length (s).
pub const FLASH_TIME: f64 = 0.05;
/// The stock flash particles' offset from the `Flash` socket (socket
/// axes, units; `AK47_PV_Muzz`, 9).
pub const FLASH_SOCKET_OFFSET: Vec3 = Vec3::new(0.0, 0.0, 0.5);

/// A horizontal field of view from a vertical one and the aspect ratio
/// (width / height): 2 atan(a tan(fov_y / 2)), degrees.
pub fn fov_x(fov_y: f32, aspect: f32) -> f32 {
    2.0 * (aspect * (fov_y.to_radians() / 2.0).tan()).atan().to_degrees()
}

/// The world camera's horizontal FOV (5.2, Open question 14): `FovY`
/// stays 75°, `FovX` is recomputed from the aspect only when the camera is
/// reset (client start, each world start); a window change in between
/// keeps it (the picture stretches). The view model's own projection
/// takes the live aspect at every draw (`fov_x(PV_FOV_Y, a)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldFov {
    pub fov_x: f32,
}

impl WorldFov {
    /// A camera reset at aspect `a`.
    pub fn reset(a: f32) -> Self {
        Self {
            fov_x: fov_x(WORLD_FOV_Y, a),
        }
    }

    /// The window changed: nothing until the next reset.
    pub fn resize(&mut self, _a: f32) {}
}

/// The view model's camera-space origin (4): `Pos` + the sway (w, h, 0)
/// + the jiggle j on all three axes (Quirk 2).
pub fn model_origin(pos: Vec3, sway: (f32, f32), jiggle: f32) -> Vec3 {
    pos + Vec3::new(sway.0, sway.1, 0.0) + Vec3::splat(jiggle)
}

/// The shot jiggle's three draws, one per axis, uniform in +-`recoil`
/// (`rolls` in [0, 1)); zero without a shot.
pub fn jiggle_draws(recoil: Vec3, rolls: [f32; 3]) -> Vec3 {
    Vec3::new(
        recoil.x * (2.0 * rolls[0] - 1.0),
        recoil.y * (2.0 * rolls[1] - 1.0),
        recoil.z * (2.0 * rolls[2] - 1.0),
    )
}

/// The flash offset o (4): `Pos` + `MuzzlePos` + the sway + the draws per
/// axis.
pub fn flash_offset(pos: Vec3, muzzle: Vec3, sway: (f32, f32), draws: Vec3) -> Vec3 {
    pos + muzzle + Vec3::new(sway.0, sway.1, 0.0) + draws
}

/// The camera's axes (right, up, forward) in world space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraAxes {
    pub right: Vec3,
    pub up: Vec3,
    pub forward: Vec3,
}

impl CameraAxes {
    /// A camera-space vector in world space.
    pub fn apply(&self, v: Vec3) -> Vec3 {
        self.right * v.x + self.up * v.y + self.forward * v.z
    }
}

/// The world flash point F = c + o along the camera's axes (4).
pub fn flash_point(camera: Vec3, axes: &CameraAxes, o: Vec3) -> Vec3 {
    camera + axes.apply(o)
}

/// The breach point B = F + `BreachOffset` along the camera's axes (10).
pub fn breach_point(flash: Vec3, axes: &CameraAxes, breach: Vec3) -> Vec3 {
    flash + axes.apply(breach)
}

/// The first-person muzzle flash (9): shown on each weapon update while
/// the clock is before the shot's time plus the effect's length, the
/// model visible, the player alive and the camera not under water (unless
/// the weapon allows it).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlashTimer {
    pub shot_at: Option<f64>,
    pub shown: bool,
}

impl FlashTimer {
    pub fn shot(&mut self, now: f64) {
        self.shot_at = Some(now);
    }

    pub fn update(&mut self, now: f64, length: f64, visible: bool, alive: bool, under_water: bool) -> bool {
        self.shown = visible && alive && !under_water && self.shot_at.is_some_and(|t| now < t + length);
        self.shown
    }
}

/// How much of a dynamic light's colour reaches a model point `d` units
/// from it, the light's radius `r` (view_models.md Follow-up (2026-10-31b)
/// 1): 1 / (1 + 19·d²/r²), none at or beyond r. Every model is lit so, the
/// view model too (its light placed relative to the camera).
pub fn model_light_share(d: f32, r: f32) -> f32 {
    if r <= 0.0 || d >= r {
        return 0.0;
    }
    1.0 / (1.0 + 19.0 * d * d / (r * r))
}

/// A world light at `light` as the view model sees it: in the camera's
/// space (the camera at `eye` with rotation `rot`), so it falls on the
/// model where it really is relative to the camera.
pub fn light_in_camera(light: Vec3, eye: Vec3, rot: bevy::math::Quat) -> Vec3 {
    rot.inverse() * (light - eye)
}
