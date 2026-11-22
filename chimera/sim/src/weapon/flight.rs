//! A flyer's tumble and air drag, for any game (`projectile::Flyer::spin`,
//! `Flyer::drag`): the data's `LaunchSpin` (a spin drawn at launch from the
//! shot's seed: a fixed part plus a whole number in ± a range per axis,
//! added `times` over; TF2's grenades and stickies, specs/tf2/
//! weapons_more.md 4.1: (600, r, 0) degrees/s added twice) and `DragParams`
//! (a physics engine's air drag on a body: each tick, after gravity, the
//! velocity scaled by 1 − ρ Σ c_i |v_i| and the spin by 1 − ρ Σ a_i |ω_i|,
//! v and ω in the body's own axes, ρ the game's air density; TF2's,
//! weapons_more.md 4.4) with, for a body whose inertia differs per axis
//! (`DragParams::inertia`), the physics engine's turning of ω in the body
//! (torque-free precession, `precess`: after the drag, in 1 + floor(|ω| dt /
//! (1/12 rad)) forward-Euler sub-steps from the tick's starting |ω|, whose
//! growth of |ω| is part of how TF2's grenade tumbles, weapons_more.md 4.4).
//!
//! The data's vectors are in its own axes, z up and right-handed (x
//! forward, y left, z up for a Source model): the engine's (x, z, −y).
//! The air density is per game (`AirDensity`, the console's
//! `<game>:air_density`, registered by the game with its default:
//! `register_air_density`); a game that registers none has 1 (the data's
//! coefficients as written).
//!
//! Known shortcuts: the body's axes are its drawn model's (`Flyer::turn`
//! from its model's nose); without the model (a headless test) they are
//! its launch's facing (−Z forward), so a model whose drag differs per axis
//! drags as if turned. No speed caps here (a physics engine's, specs/tf2/
//! weapons_more.md 5.1: TF2's sticky caps its own launch, `sticky.rs`).

use bevy::prelude::*;
use serde::Deserialize;

use crate::console::ConsoleAppExt;

/// A launch's spin (data: degrees/s about the model's own axes, the data's
/// axes): `base` plus, per axis, a whole number uniform in ±`random`, the
/// sum added `times` over (TF2's grenade: base (600, 0, 0), random (0,
/// 1200, 0), twice: (1200, 2r, 0)).
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LaunchSpin {
    #[serde(default)]
    pub base: [f32; 3],
    #[serde(default)]
    pub random: [f32; 3],
    #[serde(default = "one")]
    pub times: u32,
}

fn one() -> u32 {
    1
}

/// Mixed into the shot's seed for the spin's draws.
const SPIN_SALT: u32 = 0x5e1f_0a2b;

impl LaunchSpin {
    /// The spin (rad/s about the engine's model axes) with each axis's
    /// random term picked by `r` (in [−1, 1]).
    pub fn with(&self, r: [f32; 3]) -> Vec3 {
        let d: [f32; 3] = std::array::from_fn(|i| (self.base[i] + (r[i].clamp(-1.0, 1.0) * self.random[i]).round()) * self.times as f32);
        Vec3::new(d[0], d[2], -d[1]) * std::f32::consts::PI / 180.0
    }

    /// The spin of the shot with `seed` (its own draws: deterministic).
    pub fn draw(&self, seed: u32) -> Vec3 {
        let mut rng = super::random::Rng::new(seed ^ SPIN_SALT);
        self.with(std::array::from_fn(|_| rng.range(-1.0, 1.0)))
    }
}

/// Air drag (data): the fraction of speed lost per `per` seconds per
/// (data length/s) along each of the body's axes (`lin`), and of spin per
/// (degree/s) about each (`ang`), both × the air density (TF2's sticky:
/// (1.43, 1.41, 1.45) × 10⁻⁶ and 7.3 × 10⁻⁷ per 0.015 s tick).
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DragParams {
    pub lin: [f32; 3],
    #[serde(default)]
    pub ang: [f32; 3],
    pub per: f32,
    /// The body's moments of inertia about its axes (relative; none: the
    /// same about every axis, so ω keeps its direction in the body):
    /// TF2's grenade (0.288, 1, 1), its long axis x (weapons_more.md 4.4).
    #[serde(default)]
    pub inertia: Option<[f32; 3]>,
}

impl DragParams {
    /// In meters and radians, per second, in the engine's axes (`unit` m
    /// per data length).
    pub fn drag(&self, unit: f32) -> AirDrag {
        let (l, a) = (self.lin, self.ang);
        AirDrag {
            lin: Vec3::new(l[0], l[2], l[1]) / (self.per * unit),
            ang: Vec3::new(a[0], a[2], a[1]) / self.per * (180.0 / std::f32::consts::PI),
            inertia: self.inertia.map(|i| Vec3::new(i[0], i[2], i[1])),
        }
    }
}

/// A flyer's air drag (`Flyer::drag`): per second, per m/s along each of
/// its model's axes (`lin`) and per rad/s about each (`ang`), × the air
/// density; `inertia` its moments about its axes (`DragParams::inertia`,
/// the engine's axes).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirDrag {
    pub lin: Vec3,
    pub ang: Vec3,
    pub inertia: Option<Vec3>,
}

/// The physics engine's sub-step of a body's rotation: at most this many
/// radians of |ω| dt a sub-step (1/12 rad, 4.775°; weapons_more.md 4.4).
pub const PRECESSION_STEP: f32 = 1.0 / 12.0;

/// `spin` (rad/s, the body's axes) after `dt` of torque-free rotation of
/// a body with moments `inertia` (its axes), as the physics engine steps
/// it: n = 1 + floor(|ω| dt / `PRECESSION_STEP`) forward-Euler sub-steps
/// of Euler's equations, each from the sub-step's old ω (so |ω| grows by
/// about θ² / 2n a tick: the engine's, kept; weapons_more.md 4.4, test
/// 29f). `speed` is |ω| at the tick's start (before its drag).
pub fn precess(spin: Vec3, inertia: Vec3, speed: f32, dt: f32) -> Vec3 {
    let n = 1 + (speed * dt / PRECESSION_STEP).floor().max(0.0) as u32;
    let h = dt / n as f32;
    let i = inertia.max(Vec3::splat(1e-9));
    let k = Vec3::new((i.y - i.z) / i.x, (i.z - i.x) / i.y, (i.x - i.y) / i.z);
    let mut w = spin;
    for _ in 0..n {
        w += Vec3::new(k.x * w.y * w.z, k.y * w.z * w.x, k.z * w.x * w.y) * h;
    }
    w
}

impl AirDrag {
    /// One step of `dt` at density `rho` for a body turned `rotation`
    /// going `velocity` (world, m/s; gravity already applied) spinning
    /// `spin` (rad/s, its own axes): both scaled down, their directions
    /// kept; then, with `inertia`, the spin precessed (`precess`, its
    /// sub-steps from the tick's starting spin; also without air).
    pub fn step(&self, velocity: Vec3, spin: Vec3, rotation: Quat, rho: f32, dt: f32) -> (Vec3, Vec3) {
        let (mut v, mut w) = (velocity, spin);
        if rho > 0.0 {
            let local = rotation.inverse() * velocity;
            let f_lin = rho * dt * self.lin.dot(local.abs());
            let f_ang = rho * dt * self.ang.dot(spin.abs());
            (v, w) = (velocity * (1.0 - f_lin).max(0.0), spin * (1.0 - f_ang).max(0.0));
        }
        if let Some(i) = self.inertia {
            w = precess(w, i, spin.length(), dt);
        }
        (v, w)
    }
}

/// Each game's air density (`<game>:air_density`; unregistered: 1).
#[derive(Resource, Clone, Debug, Default)]
pub struct AirDensity(pub Vec<(&'static str, f32)>);

impl AirDensity {
    /// For a flyer of weapon `weapon` (its game: the id's namespace).
    pub fn of(&self, weapon: &'static str) -> f32 {
        let game = super::impact::weapon_game(weapon);
        self.0.iter().find(|(g, _)| Some(*g) == game).map_or(1.0, |(_, d)| *d)
    }

    fn slot(&mut self, game: &str) -> Option<&mut f32> {
        self.0.iter_mut().find(|(g, _)| *g == game).map(|(_, d)| d)
    }
}

/// Register `game`'s air density, `default` (its cvar `<game>:air_density`).
pub fn register_air_density(app: &mut App, game: &'static str, default: f32) {
    let mut air = app.world_mut().get_resource_or_init::<AirDensity>();
    match air.slot(game) {
        Some(d) => *d = default,
        None => air.0.push((game, default)),
    }
    app.console_cvar(
        &format!("{game}:air_density"),
        "The air's density: scales projectiles' air drag (0: none).",
        &default.to_string(),
        move |w| w.get_resource_mut::<AirDensity>()?.bypass_change_detection().slot(game).map(|d| d.to_string()),
        move |w, v| {
            let d: f32 = v.trim().trim_matches('"').parse().map_err(|_| format!("bad value \"{v}\""))?;
            let mut air = w.get_resource_mut::<AirDensity>().ok_or("not available")?;
            *air.slot(game).ok_or("not available")? = d.max(0.0);
            Ok(())
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const U: f32 = 0.0254;

    fn sticky() -> AirDrag {
        DragParams { lin: [1.43e-6, 1.41e-6, 1.45e-6], ang: [7.27e-7, 7.36e-7, 7.3e-7], per: 0.015, inertia: None }.drag(U)
    }

    fn grenade() -> AirDrag {
        DragParams { lin: [0.75e-6, 1.91e-6, 1.90e-6], ang: [9.45e-7, 3.98e-7, 3.98e-7], per: 0.015, inertia: Some([0.288, 1.0, 1.0]) }.drag(U)
    }

    /// The data's (x, y, z) degrees/s as the engine's rad/s, and back.
    fn eng(d: [f32; 3]) -> Vec3 {
        Vec3::new(d[0], d[2], -d[1]) * std::f32::consts::PI / 180.0
    }
    fn data(w: Vec3) -> [f32; 3] {
        [w.x, -w.z, w.y].map(f32::to_degrees)
    }

    /// Test 29f: no air, ω (1200, 107.9, −24.5): 4 sub-steps, ω_x kept,
    /// (ω_y, ω_z) turned −12.81° (+y toward −z), its length × 1.00626.
    #[test]
    fn a_grenade_precesses_in_4_sub_steps_29f() {
        let w0 = [1200.0, 107.9, -24.5];
        let (_, w) = grenade().step(Vec3::X, eng(w0), Quat::IDENTITY, 0.0, 0.015);
        let w = data(w);
        assert!((w[0] - 1200.0).abs() < 1e-2, "{w:?}");
        let (a0, a1) = (w0[2].atan2(w0[1]), w[2].atan2(w[1]));
        assert!(((a1 - a0).to_degrees() + 12.81).abs() < 0.02, "turned {}", (a1 - a0).to_degrees());
        let (l0, l1) = (w0[1].hypot(w0[2]), w[1].hypot(w[2]));
        assert!((l1 / l0 - 1.00626).abs() < 2e-5, "× {}", l1 / l0);
    }

    /// The sub-steps by |ω| (measured: 4 at 1205, 5 up to 1590, 6 from
    /// 1594, 7 up to 2227, 8 from 2233 degrees/s) and the growth of ω⊥ a
    /// tick with no drag at ω_x 1200 (measured × 1.00626, 1.00502,
    /// 1.00418, 1.00358, 1.00313 at n = 4-8).
    #[test]
    fn the_sub_steps_and_their_growth_by_n() {
        for (deg, n) in [(1205.0f32, 4u32), (1590.0, 5), (1594.0, 6), (2227.0, 7), (2233.0, 8)] {
            assert_eq!(1 + (deg.to_radians() * 0.015 / PRECESSION_STEP).floor() as u32, n, "{deg}");
        }
        for (total, want) in [(1205.0f32, 1.00626f32), (1450.0, 1.00502), (1800.0, 1.00418), (2100.0, 1.00358), (2400.0, 1.00313)] {
            let perp = (total * total - 1200.0 * 1200.0).sqrt();
            let w0 = [1200.0, perp, 0.0];
            let (_, w) = grenade().step(Vec3::X, eng(w0), Quat::IDENTITY, 0.0, 0.015);
            let w = data(w);
            let grew = w[1].hypot(w[2]) / perp;
            assert!((grew - want).abs() < 3e-5, "|ω| {total}: × {grew}, want {want}");
        }
    }

    /// Test 29g: ρ = 2, ω (1195.0, −2302.3, 521.6): drag f = 0.00451 from
    /// the tick's start, then 9 sub-steps; ω_x → 1189.6.
    #[test]
    fn a_grenade_drags_then_precesses_29g() {
        let w0 = [1195.0, -2302.3, 521.6];
        let s = eng(w0);
        assert_eq!(1 + (s.length() * 0.015 / PRECESSION_STEP).floor() as u32, 9);
        let (_, w) = grenade().step(Vec3::X, s, Quat::IDENTITY, 2.0, 0.015);
        let w = data(w);
        assert!((w[0] - 1189.6).abs() < 0.05, "{w:?}");
        let f = 2.0 * (9.45e-7 * 1195.0 + 3.98e-7 * (2302.3 + 521.6));
        assert!((f - 0.00451f32).abs() < 1e-5);
    }

    /// specs/tf2/weapons_more.md 4.1, test 29b: r = 187 drawn, (600, r, 0)
    /// added twice: (1200, 374, 0) about forward, left, up (the engine's x,
    /// −z, y).
    #[test]
    fn the_spin_is_added_times_over_29b() {
        let s = LaunchSpin { base: [600.0, 0.0, 0.0], random: [0.0, 1200.0, 0.0], times: 2 };
        let w = s.with([0.0, 187.0 / 1200.0, 0.0]).to_array().map(f32::to_degrees);
        assert!((w[0] - 1200.0).abs() < 1e-2 && w[1].abs() < 1e-3 && (w[2] + 374.0).abs() < 1e-2, "{w:?}");
        for seed in 0..300 {
            let w = s.draw(seed).to_array().map(f32::to_degrees);
            assert!((w[0] - 1200.0).abs() < 1e-2 && w[1] == 0.0, "{w:?}");
            let y = w[2].round();
            assert!((w[2] - y).abs() < 1e-2 && y.abs() <= 2400.0 && y as i32 % 2 == 0, "even, within ±2400: {w:?}");
        }
        assert_eq!(s.draw(7), s.draw(7), "from the seed");
    }

    /// Test 29c: ρ = 2, one tick, along the body's x and along its
    /// diagonal; the spin (1200, 1600, 0).
    #[test]
    fn a_sticky_loses_speed_and_spin_per_tick_29c() {
        let d = sticky();
        let dt = 0.015;
        let along = |v_src: Vec3| {
            // The data's (forward, left, up) in the engine's model axes.
            let v = Vec3::new(v_src.x, v_src.z, -v_src.y) * U;
            let (after, _) = d.step(v, Vec3::ZERO, Quat::IDENTITY, 2.0, dt);
            after.length() / v.length()
        };
        assert!((along(Vec3::new(1000.0, 0.0, 0.0)) - (1.0 - 0.00286)).abs() < 2e-6);
        assert!((along(Vec3::splat(577.0)) - (1.0 - 0.00495)).abs() < 2e-5, "{}", along(Vec3::splat(577.0)));
        let w = LaunchSpin { base: [1200.0, 1600.0, 0.0], random: [0.0; 3], times: 1 }.with([0.0; 3]);
        let (v, after) = d.step(Vec3::X, w, Quat::IDENTITY, 2.0, dt);
        let k = 1.0 - 2.0 * (7.27e-7 * 1200.0 + 7.36e-7 * 1600.0);
        assert!((after.length() / w.length() - k).abs() < 1e-6, "× (1 − 0.00410)");
        assert!((after.normalize() - w.normalize()).length() < 1e-6, "its direction kept");
        assert!(v.length() < 1.0, "the velocity too");
        // The body turned: the velocity is taken in its own axes.
        let turned = Quat::from_rotation_y(0.3);
        let (a, _) = d.step(turned * Vec3::X * 25.4, Vec3::ZERO, turned, 2.0, dt);
        assert!((a.length() / 25.4 - (1.0 - 0.00286)).abs() < 2e-6);
    }

    /// Test 29d: density 0, nothing lost.
    #[test]
    fn no_air_no_drag_29d() {
        let w = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(sticky().step(Vec3::splat(20.0), w, Quat::IDENTITY, 0.0, 0.015), (Vec3::splat(20.0), w));
        // A sticky's ω keeps its direction in the body (no inertia given).
        let (_, s) = sticky().step(Vec3::X, eng([1200.0, 1600.0, 0.0]), Quat::IDENTITY, 2.0, 0.015);
        assert!((s.normalize() - eng([1200.0, 1600.0, 0.0]).normalize()).length() < 1e-6);
    }

    #[test]
    fn densities_by_game() {
        let mut app = App::new();
        register_air_density(&mut app, "tf2", 2.0);
        let air = app.world().resource::<AirDensity>();
        assert_eq!((air.of("tf2:x"), air.of("hl2dm:y")), (2.0, 1.0));
        let c = app.world().resource::<crate::console::Console>().cvar("tf2:air_density").map(|c| c.default.clone());
        assert_eq!(c.as_deref(), Some("2"));
    }
}
