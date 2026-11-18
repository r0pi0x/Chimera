//! HL2:DM's view punch (specs/hl2dm/weapons.md 5, 7): the punch angle with
//! a velocity on a spring (`Hl2Punch`, decayed every tick before the weapon
//! frame: angle += velocity dt, velocity x max(0, 1 - 9 dt), velocity -=
//! angle x clamp(65 dt, 0, 2), both zeroed once their squares are under
//! 0.001), kicked after the shots by the weapon's rule: `Hl2Kick`
//! (`hl2dm:punch`: a "view punch" of random angles per shot, velocity +=
//! angles x 20, optionally zeroing the punch first, the pistol's; the
//! shotgun's double shot its own angles) or `Hl2MachineGun`
//! (`hl2dm:machine_gun`: per firing call, growing with how long attack has
//! been held, clipped to +-24/3/1 degrees, velocity += kick x 10). The
//! punch moves the shot (`Hitscan::punch_scale` 1, the pistol's 0: its
//! punch is zeroed before each shot).
//!
//! Angles here are Source's: degrees (pitch, yaw, roll), pitch positive
//! looking down; Chimera's `ViewPunch` is radians with pitch up
//! (`to_view`, `from_view`). The punch is HL2:DM's to decay only while
//! an HL2:DM weapon is in hand (`weapon::impact::RecoilGame`).
//!
//! The .357's nudge (`Hl2Kick::nudge`, weapons.md 7.2): on the machine
//! that plays the shooter (`LocalPlayer`; a dedicated server has none),
//! the first time a shot is predicted, its real view angles turn by a whole
//! -1, 0 or +1 degree in pitch and in yaw (`nudge_view`; applied to the
//! local `Intent` in the frame, `apply_view_nudge`, so the next command
//! carries it): a lasting turn of the aim, not a punch.
//!
//! Known shortcuts: the randoms are Chimera's generator seeded per command
//! and use (weapons.md Q4: the ranges and seeding, not the game's
//! numbers); the nudge's numbers don't shift the client's punch yaw as the
//! game's extra draws do (its punch is the server's here too).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    ViewPunch, WeaponEvent, WeaponEventKind, WeaponState, impact::RecoilGame,
    random::{Rng, shared_random, shot_seed},
};

use crate::core::{Intent, LocalPlayer, SimClock};

/// Source's punch spring (shared with CS:S: weapons.md constants).
pub const DAMPING: f32 = 9.0;
pub const SPRING: f32 = 65.0;
/// A "view punch" of angles a adds a x this to the velocity.
pub const IMPULSE_SCALE: f32 = 20.0;
/// The machine guns' kick floor and clip (weapons.md constants).
pub const KICK_MIN: Vec3 = Vec3::new(0.2, 0.2, 0.1);
pub const KICK_CLIP: Vec3 = Vec3::new(24.0, 3.0, 1.0);
/// The punch is zeroed before a machine-gun kick when its size and its
/// velocity's together are at most this.
pub const KICK_RESET: f32 = 10.0;

/// The punch's velocity on a character (Source degrees per second);
/// the angle itself is its `ViewPunch`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hl2Punch {
    pub velocity: Vec3,
}

/// `hl2dm:punch`: a view punch per shot, Source degrees, each from U(lo,
/// hi); `reset`: the punch and its velocity zeroed first (the pistol);
/// `alt_*`: the double shot's (the shotgun).
#[derive(Component, Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Hl2Kick {
    pub pitch: [f32; 2],
    pub yaw: [f32; 2],
    #[serde(default)]
    pub reset: bool,
    #[serde(default)]
    pub alt_pitch: Option<[f32; 2]>,
    #[serde(default)]
    pub alt_yaw: Option<[f32; 2]>,
    /// The shooter's own machine turns its real view by a whole -1, 0 or
    /// +1 degree in pitch and in yaw on each shot (the .357, weapons.md
    /// 7.2: `nudge_view`).
    #[serde(default)]
    pub nudge: bool,
}

/// `hl2dm:machine_gun`: the kick per call with `max` degrees of extra
/// vertical kick reached after `slide` seconds of attack held (SMG1 1/2,
/// AR2 8/5).
#[derive(Component, Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Hl2MachineGun {
    pub max: f32,
    pub slide: f32,
}

/// Source angles (degrees, pitch down) as Chimera's `ViewPunch` (radians,
/// pitch up).
pub fn to_view(p: Vec3) -> Vec3 {
    Vec3::new(-p.x.to_radians(), p.y.to_radians(), p.z.to_radians())
}

/// A `ViewPunch` as Source angles.
pub fn from_view(v: Vec3) -> Vec3 {
    Vec3::new(-v.x.to_degrees(), v.y.to_degrees(), v.z.to_degrees())
}

/// One tick of the spring on (angle, velocity), Source degrees.
pub fn decay(p: Vec3, v: Vec3, dt: f32) -> (Vec3, Vec3) {
    if p.length_squared() <= 0.001 && v.length_squared() <= 0.001 {
        return (Vec3::ZERO, Vec3::ZERO);
    }
    let p = (p + v * dt).clamp(Vec3::splat(-89.0), Vec3::splat(89.0));
    let v = v * (1.0 - DAMPING * dt).max(0.0);
    let v = v - p * (SPRING * dt).clamp(0.0, 2.0);
    (p, v)
}

/// The machine guns' kick for one call (weapons.md 7.3): with `held`
/// seconds of attack, its punch `punch` and velocity `velocity` (Source
/// degrees), signs from `seed`: the new (punch, velocity).
pub fn machine_gun_kick(gun: &Hl2MachineGun, held: f32, punch: Vec3, velocity: Vec3, seed: u32) -> (Vec3, Vec3) {
    let k = held.min(gun.slide) / gun.slide;
    let up = KICK_MIN.x + gun.max * k;
    let mut delta = Vec3::new(-up, up / 3.0, KICK_MIN.z + gun.max * k / 8.0);
    let (mut punch, mut velocity) = (punch, velocity);
    if (punch.length_squared() + velocity.length_squared()).sqrt() <= KICK_RESET {
        punch = Vec3::ZERO;
        velocity = Vec3::ZERO;
    }
    // Two whole numbers of {-1, 0, 1}: each sign flips when it is >= 0.
    let mut rng = Rng::new(seed);
    let mut whole = || (rng.next() * 3.0).floor() as i32 - 1;
    if whole() >= 0 {
        delta.y = -delta.y;
    }
    if whole() >= 0 {
        delta.z = -delta.z;
    }
    let clipped = (punch + delta).clamp(-KICK_CLIP, KICK_CLIP) - punch;
    velocity += clipped * 0.5 * IMPULSE_SCALE;
    (punch, velocity)
}

/// Before the weapon frame: each character's punch on its spring while
/// HL2:DM's recoil owns it.
pub fn decay_punch(
    mut owners: Query<(Entity, &mut ViewPunch, Option<&mut Hl2Punch>, Option<&RecoilGame>), Without<crate::map::interp::NetDrawn>>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    for (e, mut punch, velocity, game) in &mut owners {
        if !RecoilGame::is(game, super::super::GAME) || game.is_none() {
            continue;
        }
        let v = velocity.as_deref().map_or(Vec3::ZERO, |v| v.velocity);
        let (p, v) = decay(from_view(punch.0), v, dt);
        let next = to_view(p);
        if punch.0 != next {
            punch.0 = next;
        }
        match velocity {
            Some(mut vel) => {
                if vel.velocity != v {
                    vel.velocity = v;
                }
            }
            None if v != Vec3::ZERO => {
                commands.entity(e).insert(Hl2Punch { velocity: v });
            }
            None => {}
        }
    }
}

/// After the weapon frame: each HL2:DM weapon that fired kicks its
/// owner's punch, once per firing call (a tick's catch-up rounds are one
/// call).
#[allow(clippy::type_complexity)]
pub fn kick(
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<(&WeaponState, Option<&Hl2Kick>, Option<&Hl2MachineGun>)>,
    mut owners: Query<(&Intent, Option<&mut ViewPunch>, Option<&mut Hl2Punch>)>,
    mut commands: Commands,
    mut done: Local<Vec<Entity>>,
) {
    done.clear();
    for e in events.read() {
        let WeaponEventKind::Fired { mode, .. } = e.kind else { continue };
        if done.contains(&e.weapon) {
            continue;
        }
        done.push(e.weapon);
        let Ok((state, fixed, gun)) = weapons.get(e.weapon) else { continue };
        if fixed.is_none() && gun.is_none() {
            continue;
        }
        let Ok((intent, view, vel)) = owners.get_mut(e.owner) else { continue };
        let mut p = view.as_deref().map_or(Vec3::ZERO, |v| from_view(v.0));
        let mut v = vel.as_deref().map_or(Vec3::ZERO, |v| v.velocity);
        if let Some(k) = fixed {
            if k.reset {
                p = Vec3::ZERO;
                v = Vec3::ZERO;
            }
            let alt = mode > 0;
            let (pitch, yaw) = match (alt, k.alt_pitch) {
                (true, Some(ap)) => (ap, k.alt_yaw.unwrap_or([0.0, 0.0])),
                _ => (k.pitch, k.yaw),
            };
            let roll_label = if alt { "hl2dm:punch_alt" } else { "hl2dm:punch" };
            let mut dice = shared_random(intent.command, roll_label, 0);
            let angles = Vec3::new(dice.range(pitch[0], pitch[1]), dice.range(yaw[0], yaw[1]), 0.0);
            v += angles * IMPULSE_SCALE;
        }
        if let Some(gun) = gun {
            (p, v) = machine_gun_kick(gun, state.fire_duration, p, v, shot_seed(intent.command));
        }
        match view {
            Some(mut view) => view.0 = to_view(p),
            None => {
                commands.entity(e.owner).insert(ViewPunch(to_view(p)));
            }
        }
        match vel {
            Some(mut vel) => vel.velocity = v,
            None => {
                commands.entity(e.owner).insert(Hl2Punch { velocity: v });
            }
        }
    }
}

/// The local player's view turn still to apply (Source degrees: pitch
/// down, yaw left), from `nudge_view`.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub struct ViewNudge(pub Vec2);

/// After the kicks: each shot of a nudging weapon (`Hl2Kick::nudge`) by
/// the local player, the first time it is predicted, turns its view by a
/// whole -1, 0 or +1 degree in pitch and in yaw.
pub fn nudge_view(
    mut events: MessageReader<WeaponEvent>,
    weapons: Query<&Hl2Kick>,
    owners: Query<&Intent, With<LocalPlayer>>,
    mut nudge: ResMut<ViewNudge>,
) {
    for e in events.read() {
        if e.replay || !matches!(e.kind, WeaponEventKind::Fired { .. }) {
            continue;
        }
        if !weapons.get(e.weapon).is_ok_and(|k| k.nudge) {
            continue;
        }
        let Ok(intent) = owners.get(e.owner) else { continue };
        let mut dice = shared_random(intent.command, "hl2dm:view_nudge", 0);
        let mut whole = || (dice.next() * 3.0).floor().min(2.0) - 1.0;
        let (pitch, yaw) = (whole(), whole());
        nudge.0 += Vec2::new(pitch, yaw);
    }
}

/// In the frame: the pending nudge turns the local player's view.
pub fn apply_view_nudge(mut nudge: ResMut<ViewNudge>, mut player: Query<&mut Intent, With<LocalPlayer>>) {
    if nudge.0 == Vec2::ZERO {
        return;
    }
    let turn = std::mem::take(&mut nudge.0);
    for mut i in &mut player {
        i.pitch = (i.pitch - turn.x.to_radians()).clamp(-89f32.to_radians(), 89f32.to_radians());
        i.yaw = (i.yaw + turn.y.to_radians()).rem_euclid(std::f32::consts::TAU);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// weapons.md test 12: the .357's kick (velocity -160 pitch) reaches
    /// -2.4 after a tick and -4.4409 after two.
    #[test]
    fn the_357s_punch_springs_as_the_spec_says() {
        let (mut p, mut v) = (Vec3::ZERO, Vec3::new(-160.0, 0.0, 0.0));
        let mut trace = Vec::new();
        for _ in 0..40 {
            (p, v) = decay(p, v, 0.015);
            trace.push(p.x);
        }
        assert!((trace[0] + 2.4).abs() < 1e-4, "{}", trace[0]);
        assert!((trace[1] + 4.4409).abs() < 1e-3, "{}", trace[1]);
        let peak = trace.iter().cloned().fold(0.0, f32::min);
        let at = trace.iter().position(|x| *x == peak).unwrap() + 1;
        assert!((peak + 10.47).abs() < 0.05 && at == 9, "peak {peak} at tick {at}");
        let back = trace.iter().position(|x| *x >= 0.0).unwrap() + 1;
        assert_eq!(back, 31, "back through 0 at tick {back}");
    }

    /// weapons.md tests 15-17.
    #[test]
    fn machine_gun_kicks_grow_with_the_hold_and_clip() {
        let smg = Hl2MachineGun { max: 1.0, slide: 2.0 };
        let (_, v) = machine_gun_kick(&smg, 0.015, Vec3::ZERO, Vec3::ZERO, 7);
        assert!((v.x + 2.075).abs() < 1e-4 && (v.y.abs() - 0.69167).abs() < 1e-4 && (v.z.abs() - 1.00938).abs() < 1e-4, "{v}");
        let (_, v) = machine_gun_kick(&smg, 2.5, Vec3::ZERO, Vec3::ZERO, 7);
        assert!((v.x + 12.0).abs() < 1e-4 && (v.y.abs() - 4.0).abs() < 1e-4 && (v.z.abs() - 2.25).abs() < 1e-4, "{v}");
        let ar2 = Hl2MachineGun { max: 8.0, slide: 5.0 };
        let mut rolls = Vec::new();
        for seed in 0..64 {
            let (p, v) = machine_gun_kick(&ar2, 6.0, Vec3::new(11.0, 0.0, 0.5), Vec3::ZERO, seed);
            assert_eq!(p, Vec3::new(11.0, 0.0, 0.5), "no reset above 10");
            assert!((v.x + 82.0).abs() < 1e-3 && (v.y.abs() - 27.333).abs() < 1e-2, "{v}");
            rolls.push((v.z * 100.0).round() as i32);
        }
        assert!(rolls.iter().all(|r| *r == 500 || *r == -1100), "{rolls:?}");
        assert!(rolls.contains(&500) && rolls.contains(&-1100));
    }
}
