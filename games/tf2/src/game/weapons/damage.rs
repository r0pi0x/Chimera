//! TF2's shared gun rules (specs/tf2/weapons.md 2, 4, 6), the hook
//! `tf2:gun` (`GunParams`, the weapon part `Tf2Gun`):
//!
//! - **The distance modifier** (2.1-2.3, spread off: `tf_damage_disablespread`
//!   1): t = min(d, 921.6) / 1024 with d the distance between the
//!   shooter's and the target's centres (units), M0 = 1 + 0.5 (1 - 2 S(t)),
//!   S the smoothstep 3t^2 - 2t^3, its near half scaled by the gun's `ramp`
//!   k (`modifier`): 1.75 at 0 for the Scattergun (k 1.5), 1.25 for the
//!   Rocket Launcher (0.5), 1.5 for the others; 52.8 % at and beyond 921.6.
//!   A target's pellets of one shot are summed by Chimera's delivery,
//!   then scaled and rounded to whole points, halves up (`rule`,
//!   `DamageEffect::by_distance`; open question 3).
//! - **The first pellet dead centre** (6.1): a gun that hasn't fired for
//!   0.25 s puts its next shot's first pellet exactly along the aim
//!   (`centre_first`, before the frame; `note_shots` after it).
//!   `flat` leaves it out (the Sniper Rifle, weapons_more.md 3.2).
//! - **The reload by class** (4; weapons_more.md 1): `reload_start` per
//!   class (the Shotgun's 0.3333 for the Engineer, 0.4667 the Soldier,
//!   0.5 the Pyro, 0.3333 the Heavy) and a whole clip's `reload` per class
//!   (the Pistol's 1.005 for the Scout, 1.035 the Engineer): the gun's own
//!   class's (`arms`) at attach, its holder's class's while held
//!   (`for_class`, from `super::super::movement::select_class`).
//!
//! Known shortcuts: the distance jitter of `tf_damage_disablespread` 0
//! isn't modelled (the rule is the default's).

use bevy::prelude::*;
use serde::Deserialize;

use chimera_sim::weapon::{ByDistance, DamageEffect, Hitscan, Magazine, NetPart, ShellReload, WeaponEvent, WeaponEventKind};

use crate::core::SimClock;

/// Metres per unit, health per point.
pub const U: f32 = 0.0254;
pub const HP: f32 = 0.01;

/// `tf_damage_range`: the modifier's half-width (R).
pub const DAMAGE_RANGE: f32 = 0.5;
/// The far end (units): t = 1.
pub const DIST_FAR: f32 = 1024.0;
/// With the spread off, farther distances count as this (units).
pub const SPREAD_OFF_CLAMP: f32 = 921.6;
/// Seconds without a shot after which the first pellet goes dead centre.
pub const FIRST_PELLET_PAUSE: f32 = 0.25;

/// The smoothstep.
pub fn smooth(t: f32) -> f32 {
    3.0 * t * t - 2.0 * t * t * t
}

/// The near-range "scale" that stands for the short-range-reduced
/// modifier M_s (weapons_more.md "Units and conventions": the Stickybomb
/// Launcher's and the Syringe Gun's; `tf2:gun` `near = "compressed"`).
pub const COMPRESSED: f32 = -1.0;

/// The modifier at `d` units with near-range scale `k` (2.1, spread off);
/// `COMPRESSED`: M_s, the near half's distance compressed toward the
/// middle (0.35 + 0.3 t) before the curve (121.8 % at 0).
pub fn modifier(d: f32, k: f32) -> f32 {
    let t = d.max(0.0).min(SPREAD_OFF_CLAMP) / DIST_FAR;
    let m0 = |t: f32| 1.0 + DAMAGE_RANGE * (1.0 - 2.0 * smooth(t));
    if k == COMPRESSED {
        return m0(if t < 0.5 { 0.35 + 0.3 * t } else { t });
    }
    let m = m0(t);
    if m > 1.0 { 1.0 + k * (m - 1.0) } else { m }
}

/// A gun's ramp from its `tf2:gun` parameters: `ramp` (1 without), or
/// `COMPRESSED` when `near` is "compressed".
pub fn ramp_of(gun: Option<&toml::Value>) -> f32 {
    let Some(g) = gun else { return 1.0 };
    if g.get("near").and_then(toml::Value::as_str) == Some("compressed") {
        return COMPRESSED;
    }
    chimera_sim::weapon::harm::num(g, "ramp").unwrap_or(1.0)
}

/// Health (normalized) rounded to whole points, halves up.
pub fn round_points(x: f32) -> f32 {
    // A hair over: 10.5 points summed in floats may come out a hair under.
    ((x / HP) + 1e-4).round() * HP
}

/// `DamageEffect::by_distance`'s rule: a target's summed damage of one
/// shot (normalized) at `metres` between the centres, with ramp `k`.
pub fn rule(sum: f32, metres: f32, k: f32) -> f32 {
    round_points(sum * modifier(metres / U, k))
}

/// The reload's start per class (`tf2:gun`'s `reload_start`), seconds.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClassTimes {
    #[serde(default)]
    pub scout: Option<f32>,
    #[serde(default)]
    pub soldier: Option<f32>,
    #[serde(default)]
    pub pyro: Option<f32>,
    #[serde(default)]
    pub heavy: Option<f32>,
    #[serde(default)]
    pub engineer: Option<f32>,
}

impl ClassTimes {
    pub fn of(&self, class: &str) -> Option<f32> {
        match class {
            "scout" => self.scout,
            "soldier" => self.soldier,
            "pyro" => self.pyro,
            "heavy" => self.heavy,
            "engineer" => self.engineer,
            _ => None,
        }
    }
}

/// `tf2:gun`'s parameters.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GunParams {
    /// The near half's scale k (2.1).
    pub ramp: f32,
    /// The class whose arms hold it (`c_<arms>_arms.mdl`).
    pub arms: String,
    /// Its script's name (`scripts/<script>.ctx`).
    pub script: String,
    /// The reload's start by class (4); the holder's class's is taken.
    #[serde(default)]
    pub reload_start: Option<ClassTimes>,
    /// A whole clip's reload by class (weapons_more.md 1: the Pistol's).
    #[serde(default)]
    pub reload: Option<ClassTimes>,
    /// Seconds without a shot before the first pellet goes dead centre.
    #[serde(default = "pause")]
    pub first_pellet_pause: f32,
    /// No distance modifier (the Sniper Rifle: 1 at every range).
    #[serde(default)]
    pub flat: bool,
    /// The view kick per shot, degrees (the script's `PunchAngle`; 0:
    /// none, `super::recoil`).
    #[serde(default)]
    pub punch: f32,
    /// "compressed": the short-range-reduced modifier M_s in place of the
    /// `ramp` (`COMPRESSED`).
    #[serde(default)]
    pub near: Option<String>,
}

impl GunParams {
    /// Its ramp (`COMPRESSED` for M_s).
    pub fn ramp_k(&self) -> f32 {
        if self.near.as_deref() == Some("compressed") { COMPRESSED } else { self.ramp }
    }
}

fn pause() -> f32 {
    FIRST_PELLET_PAUSE
}

/// A TF2 gun's own state: its ramp and when it last fired.
#[derive(Component, Clone, Debug)]
pub struct Tf2Gun {
    pub ramp: f32,
    pub pause: f32,
    pub last_shot: f64,
    /// The reload's start and a whole clip's reload by class.
    pub reload_start: Option<ClassTimes>,
    pub reload: Option<ClassTimes>,
    /// The view kick per shot, degrees (`super::recoil`).
    pub punch: f32,
}

impl Tf2Gun {
    /// The gun's reload times for a holder of `class`: the shells' start
    /// and a whole clip's reload where its data gives that class one.
    pub fn for_class(&self, class: &str, shells: Option<&mut ShellReload>, magazine: Option<&mut Magazine>) {
        if let (Some(t), Some(s)) = (self.reload_start.and_then(|t| t.of(class)), shells)
            && s.start != t
        {
            s.start = t;
        }
        if let (Some(t), Some(m)) = (self.reload.and_then(|t| t.of(class)), magazine)
            && m.reload_time != t
        {
            m.reload_time = t;
        }
    }
}

impl NetPart for Tf2Gun {
    type Net = f64;
    fn net(&self) -> f64 {
        self.last_shot
    }
    fn set_net(&mut self, net: f64) {
        self.last_shot = net;
    }
}

/// `tf2:gun` on a weapon (after its data's parts).
pub fn attach(e: &mut EntityWorldMut, p: &GunParams) {
    e.insert(Tf2Gun {
        ramp: p.ramp_k(),
        pause: p.first_pellet_pause,
        last_shot: f64::MIN,
        reload_start: p.reload_start,
        reload: p.reload,
        punch: p.punch,
    });
    if let Some(mut d) = e.get_mut::<DamageEffect>()
        && !p.flat
    {
        d.by_distance = Some(ByDistance { rule, param: p.ramp_k() });
    }
    if let Some(start) = p.reload_start.and_then(|t| t.of(&p.arms))
        && let Some(mut s) = e.get_mut::<ShellReload>()
    {
        s.start = start;
    }
    if let Some(t) = p.reload.and_then(|t| t.of(&p.arms))
        && let Some(mut m) = e.get_mut::<Magazine>()
    {
        m.reload_time = t;
    }
}

/// Before the frame: a gun that hasn't fired for its pause centres its
/// next shot's first pellet.
pub fn centre_first(mut guns: Query<(&Tf2Gun, &mut Hitscan)>, clock: Res<SimClock>) {
    for (g, mut scan) in &mut guns {
        let want = clock.now - g.last_shot + 1e-6 >= g.pause as f64;
        if scan.centre_first != want {
            scan.centre_first = want;
        }
    }
}

/// After the frame: the guns that fired, now.
pub fn note_shots(mut events: MessageReader<WeaponEvent>, mut guns: Query<&mut Tf2Gun>, clock: Res<SimClock>) {
    for e in events.read() {
        if matches!(e.kind, WeaponEventKind::Fired { .. })
            && let Ok(mut g) = guns.get_mut(e.weapon)
        {
            g.last_shot = clock.now;
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    /// weapons.md 2.2: the worked modifiers.
    #[test]
    fn the_worked_modifiers() {
        let near = |d: f32, k: f32| modifier(d, k);
        assert!((near(0.0, 1.5) - 1.75).abs() < 1e-6);
        assert!((near(0.0, 0.5) - 1.25).abs() < 1e-6);
        assert!((near(0.0, 1.0) - 1.5).abs() < 1e-6);
        assert!((near(256.0, 1.5) - 1.515625).abs() < 1e-5);
        assert!((near(256.0, 0.5) - 1.171875).abs() < 1e-5);
        assert!((near(512.0, 1.5) - 1.0).abs() < 1e-6);
        assert!((near(768.0, 1.5) - 0.65625).abs() < 1e-5);
        assert!((near(921.6, 1.0) - 0.528).abs() < 1e-5);
        assert!((near(5000.0, 1.0) - 0.528).abs() < 1e-5);
        // M_s (weapons_more.md): 1.21825, 1.11166, 1, 0.65625, 0.528.
        for (d, want) in [(0.0, 1.21825), (256.0, 1.11166), (512.0, 1.0), (768.0, 0.65625), (2000.0, 0.528)] {
            assert!((near(d, COMPRESSED) - want).abs() < 1e-4, "{d}: {}", near(d, COMPRESSED));
        }
    }

    /// Test cases 1-6, 10, 11 (10 pellets of 6), 12 (4 of 9), 15-16.
    #[test]
    fn summed_pellets_round_to_whole_points() {
        let pts = |sum: f32, d: f32, k: f32| (rule(sum * HP, d * U, k) / HP).round() as i32;
        assert_eq!(pts(60.0, 0.0, 1.5), 105);
        assert_eq!(pts(60.0, 512.0, 1.5), 60);
        assert_eq!(pts(60.0, 1024.0, 1.5), 32);
        assert_eq!(pts(60.0, 256.0, 1.5), 91);
        assert_eq!(pts(60.0, 768.0, 1.5), 39);
        assert_eq!(pts(60.0, 2000.0, 1.5), 32);
        assert_eq!(pts(60.0, 0.0, 1.0), 90);
        assert_eq!(pts(60.0, 256.0, 1.0), 81);
        assert_eq!(pts(36.0, 0.0, 1.0), 54);
        assert_eq!(pts(36.0, 1024.0, 1.0), 19);
        assert_eq!(pts(90.0, 512.0, 0.5), 90);
        assert_eq!(pts(90.0, 1024.0, 0.5), 48);
        assert_eq!(pts(90.0, 256.0, 0.5), 105);
    }
}
