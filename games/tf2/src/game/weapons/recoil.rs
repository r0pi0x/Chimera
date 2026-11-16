//! TF2's view kick (specs/tf2/weapon_scripts.md `PunchAngle`, weapons.md
//! 6.3, their corrections from the SDK): after each shot of a gun whose
//! `tf2:gun` `punch` P is above 0 (the script's `PunchAngle`; the data's
//! 0, the wiki's "no recoil", where no script is served), its owner's view
//! punch pitch moves up by a random whole number of degrees from P - 1 to
//! P + 1 (`kick`, after the frame; the shared seed, so the shooter's
//! client and the server agree); the punch then decays on Source's shared
//! spring (`decay`, before the frame: `source_movement::punch_spring`,
//! its velocity in `Tf2Punch`) while a TF2 gun is in hand
//! (`impact::RecoilGame`). The next shot includes it (`Hitscan::
//! punch_scale` 1 on TF2's guns).
//!
//! Known shortcuts: the Scattergun and Shotgun family kick on every shot
//! in the game even at P 0 (a -1 to 1 whole degree); here only P > 0 kicks.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    ViewPunch, WeaponEvent, WeaponEventKind, impact::RecoilGame,
    random::shared_random,
};
use source_engine::source_movement::punch_spring;

use super::damage::Tf2Gun;
use crate::core::{Intent, SimClock};

/// The punch's velocity (Source degrees per second, pitch down positive
/// as Source's), the angle itself being the character's `ViewPunch`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[component(storage = "SparseSet")]
pub struct Tf2Punch {
    pub velocity: Vec3,
}

/// A `ViewPunch` (radians, pitch up) as Source degrees (pitch down).
fn from_view(v: Vec3) -> Vec3 {
    Vec3::new(-v.x.to_degrees(), v.y.to_degrees(), v.z.to_degrees())
}

fn to_view(p: Vec3) -> Vec3 {
    Vec3::new(-p.x.to_radians(), p.y.to_radians(), p.z.to_radians())
}

/// The kick (whole degrees, up) for punch `p` with the roll `r` in [0, 1).
pub fn kick_degrees(p: f32, r: f32) -> f32 {
    let (lo, hi) = ((p - 1.0).ceil(), (p + 1.0).floor());
    (lo + (r * (hi - lo + 1.0)).floor()).min(hi)
}

/// Before the weapon frame: each character's punch on the spring while
/// TF2's recoil owns it.
pub fn decay(
    mut owners: Query<(Entity, &mut ViewPunch, Option<&mut Tf2Punch>, Option<&RecoilGame>)>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let dt = clock.dt();
    for (e, mut punch, velocity, game) in &mut owners {
        if game.is_none() || !RecoilGame::is(game, super::super::GAME) {
            continue;
        }
        let v = velocity.as_deref().map_or(Vec3::ZERO, |v| v.velocity);
        let (p, v) = punch_spring(from_view(punch.0), v, dt);
        let next = to_view(p);
        if punch.0 != next {
            punch.0 = next;
        }
        match velocity {
            Some(mut vel) if vel.velocity != v => vel.velocity = v,
            None if v != Vec3::ZERO => {
                commands.entity(e).insert(Tf2Punch { velocity: v });
            }
            _ => {}
        }
    }
}

/// After the weapon frame: each TF2 gun with a kick that fired kicks its
/// owner's punch up, once per shot.
pub fn kick(
    mut events: MessageReader<WeaponEvent>,
    guns: Query<&Tf2Gun>,
    mut owners: Query<(&Intent, Option<&mut ViewPunch>)>,
    mut commands: Commands,
) {
    for e in events.read() {
        if !matches!(e.kind, WeaponEventKind::Fired { .. }) {
            continue;
        }
        let Ok(gun) = guns.get(e.weapon) else { continue };
        if gun.punch <= 0.0 {
            continue;
        }
        let Ok((intent, punch)) = owners.get_mut(e.owner) else { continue };
        let r = shared_random(intent.command, "tf2:punch", 0).next();
        let up = kick_degrees(gun.punch, r).to_radians();
        match punch {
            Some(mut p) => p.0.x += up,
            None => {
                commands.entity(e.owner).insert(ViewPunch(Vec3::new(up, 0.0, 0.0)));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// P - 1 to P + 1, whole degrees, each equally likely.
    #[test]
    fn the_kick_is_a_whole_degree_around_the_punch() {
        assert_eq!(kick_degrees(2.0, 0.0), 1.0);
        assert_eq!(kick_degrees(2.0, 0.5), 2.0);
        assert_eq!(kick_degrees(2.0, 0.99), 3.0);
        assert_eq!(kick_degrees(2.5, 0.0), 2.0);
        assert_eq!(kick_degrees(2.5, 0.99), 3.0);
    }

    /// The spring brings a kick back to 0 within a second.
    #[test]
    fn the_spring_brings_it_back() {
        let (mut p, mut v) = (Vec3::new(-3.0, 0.0, 0.0), Vec3::ZERO);
        for _ in 0..67 {
            (p, v) = punch_spring(p, v, 0.015);
        }
        assert!(p.length() < 0.5, "{p}");
    }
}
