//! Being hit by a bullet in CS:S (specs/cs_source/weapons.md, measured
//! M18): the victim slows down and its view flinches.
//!
//! - **Slowdown.** Every hit sets the victim's velocity modifier
//!   (`QuakeMovement::velocity_modifier`) to the gun's value
//!   (`HitSlowdown`, from the gun's data, `cs_source:hit_slowdown`: 0.65 rifles,
//!   snipers and shotguns, 0.5 the rest; 0.5 for a leg hit from any gun).
//!   The movement multiplies its max speed by it and lets it climb back
//!   to 1 by 1/3 a second, so a player running faster slows by friction
//!   (250 → 235 the next tick). A shotgun's pellets on one player are one
//!   hit with the last pellet's hitgroup (`weapon::BulletHit`), so the
//!   last pellet decides, as measured.
//! - **Flinch.** The view punch (`weapon::ViewPunch`) is set on the hit
//!   (`flinch`): a head without a helmet (−12, 0, ±9 roll, the sign from
//!   the shared random); an unarmoured chest, stomach or arm hit pitch
//!   −min(damage × 0.1, 4) (damage before truncation); none for a leg hit
//!   or a hit the vest took part of. It decays with recoil's rule
//!   (`weapons::decay_punch`, over all three angles).
//!
//! Only hits on the living count. The slowdown comes only with a hit that
//! is taken (not refused by team rules, noclip or map filters, not under
//! god mode); a noclipping player's hit, refused, still flinches by the same
//! rule, armour counting as covering though it takes nothing; a teammate's
//! with friendly fire off does neither (measured 2026-10-17,
//! specs/cs_source/weapons.md "Refused hits"). The server decides hits: the slowdown
//! and punch reach the victim's predicting client with its own state
//! (`QuakeMovement` and `ViewPunch` are predicted and sent to the owner),
//! where the first state after the hit corrects the prediction once and
//! the commands after it re-run with the new values. Bots move by the
//! same movement, so they slow down too.
//!
//! Known shortcuts (docs/tech-debt.md): Hit slowdown and flinch
//! (`games/cs_source/hit_reaction.rs`): the hit lands in the weapons step,
//! after every player's movement that tick, so the first slowed move is the
//! victim's next tick (CS:S's can be the same tick, by the order players
//! run). A helmeted head hit gives no punch (measured: CS:S gives none); punch
//! is set rather than added to recoil already on the view (only measured on
//! an unpunched victim); a shotgun's flinch uses all its pellets' damage on
//! the player. The landing roll stays on the movement's spring (the
//! shared code's), not this decay. Why it's OK for now: The measured cases
//! are reproduced (games/cs_source/tests/it/hit_reaction.rs); the rest needs the probe.

use bevy::prelude::*;

use super::movement::QuakeMovement;
use crate::{
    slots::GameIdentity,
    core::{DamageRules, Died, God, Health, Hitgroup, Intent, Refusal, SimSet},
    weapon::{BulletHit, ViewPunch},
};

/// The velocity modifier a gun's hit sets (M18): `legs` for a leg hit,
/// `body` for any other. Weapon data's `cs_source:hit_slowdown` hook.
#[derive(Component, Clone, Copy, Debug, PartialEq, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HitSlowdown {
    pub body: f32,
    pub legs: f32,
}

impl HitSlowdown {
    pub fn of(&self, group: Hitgroup) -> f32 {
        if is_leg(group) { self.legs } else { self.body }
    }
}

fn is_leg(group: Hitgroup) -> bool {
    matches!(group, Hitgroup::LeftLeg | Hitgroup::RightLeg)
}

/// The head flinch, degrees (Source's pitch, roll; measured M18).
const HEAD_PITCH: f32 = -12.0;
const HEAD_ROLL: f32 = 9.0;
/// The body flinch: pitch −damage × this, at most `BODY_MAX` (M18).
const BODY_PER_DAMAGE: f32 = 0.1;
const BODY_MAX: f32 = 4.0;

/// The view punch a hit sets, as Source angles in degrees (pitch, yaw,
/// roll; negative pitch looks up), or None: `damage` in hit points before
/// truncation, `covered` when armour took part of it, `helmet` when the
/// victim wore one, `roll_sign` ±1 for a head hit.
pub fn flinch(group: Hitgroup, damage: f32, covered: bool, helmet: bool, roll_sign: f32) -> Option<Vec3> {
    match group {
        // Helmeted: not measured; none (our choice).
        Hitgroup::Head if helmet => None,
        Hitgroup::Head => Some(Vec3::new(HEAD_PITCH, 0.0, HEAD_ROLL * roll_sign)),
        g if is_leg(g) || covered => None,
        _ => Some(Vec3::new(-(damage * BODY_PER_DAMAGE).min(BODY_MAX), 0.0, 0.0)),
    }
}

/// Source angles (degrees) as a `ViewPunch` (pitch up, yaw left, roll;
/// radians).
pub fn to_view_punch(source: Vec3) -> ViewPunch {
    ViewPunch(Vec3::new(-source.x, source.y, source.z) * std::f32::consts::PI / 180.0)
}

pub struct HitReactionPlugin;

impl Plugin for HitReactionPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<BulletHit>().add_message::<Died>().add_systems(
            FixedUpdate,
            react
                .after(SimSet::Weapons)
                .after(crate::core::apply_damage)
                .run_if(crate::core::authoritative),
        );
    }
}

/// Health points per normalized unit (`Health` 1.0 = 100).
const HP_PER_UNIT: f32 = 100.0;

#[allow(clippy::type_complexity)]
fn react(
    mut hits: MessageReader<BulletHit>,
    mut died: MessageReader<Died>,
    rules: DamageRules,
    guns: Query<&HitSlowdown>,
    mut victims: Query<(
        &Health,
        Has<God>,
        Option<&Intent>,
        Option<&mut QuakeMovement>,
        Option<&mut ViewPunch>,
        Option<&GameIdentity>,
    )>,
    mut commands: Commands,
) {
    for (n, hit) in hits.read().enumerate() {
        let d = &hit.damage;
        let Ok((health, god, intent, movement, punch, identity)) = victims.get_mut(d.target) else {
            continue;
        };
        // A character of another game's identity (a party app's player
        // holding that game's gun) has its own game's hit reaction.
        if identity.is_some_and(|i| !i.0.eq_ignore_ascii_case(super::GAME)) {
            continue;
        }
        // By the living (this runs after the damage is applied). A hit
        // taken slows and flinches; a noclipping player's (refused) only
        // flinches; any other refused one does neither (measured
        // 2026-10-17, weapons.md "Refused hits").
        if health.current <= 0.0 {
            continue;
        }
        let refusal = rules.refusal(d);
        if god || refusal.is_some_and(|r| r != Refusal::Noclip) {
            continue;
        }
        if refusal.is_none()
            && let Ok(slow) = guns.get(hit.weapon)
            && let Some(mut m) = movement
        {
            m.velocity_modifier = slow.of(d.hitgroup);
        }
        // The roll's sign: a shared random of the victim's command and the
        // hit's place in the tick (server-side only; any fixed rule would
        // do, as CS:S's generator isn't known).
        let roll = intent.map_or(0.0, |i| {
            crate::weapon::random::shared_random(i.command, "flinch", n as i32).next()
        });
        let sign = if roll < 0.5 { -1.0 } else { 1.0 };
        let Some(angles) = flinch(d.hitgroup, hit.raw * HP_PER_UNIT, hit.covered, hit.helmet, sign) else {
            continue;
        };
        let new = to_view_punch(angles);
        match punch {
            Some(mut p) => *p = new,
            None => {
                commands.entity(d.target).insert(new);
            }
        }
    }
    // A new life starts at full speed.
    for d in died.read() {
        if let Ok((_, _, _, Some(mut m), ..)) = victims.get_mut(d.entity) {
            m.velocity_modifier = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flinch_follows_m18() {
        assert_eq!(
            flinch(Hitgroup::Head, 140.0, false, false, 1.0),
            Some(Vec3::new(-12.0, 0.0, 9.0))
        );
        assert_eq!(
            flinch(Hitgroup::Head, 140.0, true, false, -1.0),
            Some(Vec3::new(-12.0, 0.0, -9.0))
        );
        assert_eq!(flinch(Hitgroup::Head, 140.0, true, true, 1.0), None);
        let pitch = |g, d| flinch(g, d, false, false, 1.0).map(|a| a.x);
        assert!((pitch(Hitgroup::Chest, 21.0).unwrap() + 2.1).abs() < 1e-5);
        assert!((pitch(Hitgroup::Stomach, 37.0).unwrap() + 3.7).abs() < 1e-5);
        assert_eq!(pitch(Hitgroup::LeftArm, 44.0), Some(-4.0));
        assert_eq!(pitch(Hitgroup::Chest, 115.0), Some(-4.0));
        assert_eq!(pitch(Hitgroup::LeftLeg, 30.0), None);
        assert_eq!(flinch(Hitgroup::Chest, 30.0, true, false, 1.0), None);
    }

    #[test]
    fn punch_decays_as_m18_measured() {
        // Head (-12, 0, 9) → (-11.79, 0, 8.8425) a tick later; body 4 → 3.82.
        let p = to_view_punch(Vec3::new(-12.0, 0.0, 9.0)).0;
        let next = super::super::weapons::decay_punch(p, 0.015) * 180.0 / std::f32::consts::PI;
        assert!(
            (next - Vec3::new(11.79, 0.0, 8.8425)).abs().max_element() < 1e-4,
            "{next}"
        );
        let p = to_view_punch(Vec3::new(-4.0, 0.0, 0.0)).0;
        let next = super::super::weapons::decay_punch(p, 0.015).x * 180.0 / std::f32::consts::PI;
        assert!((next - 3.82).abs() < 1e-4, "{next}");
    }

    #[test]
    fn legs_take_the_leg_value() {
        let s = HitSlowdown { body: 0.65, legs: 0.5 };
        assert_eq!(s.of(Hitgroup::RightLeg), 0.5);
        assert_eq!(s.of(Hitgroup::Chest), 0.65);
        assert_eq!(s.of(Hitgroup::Head), 0.65);
    }
}
