//! The damage shove (specs/hl2dm/projectiles.md 3, correcting weapons.md
//! 6 and 11): every hit on a living, walking HL2:DM player that names an
//! attacker and costs at least a whole health point pushes the player away
//! from the attacker's centre (lowered 10 units) by 5 units/s per point
//! of the damage, times 32 x 32 x 72 over its box's volume (twice when
//! ducked), at most 1000 units/s, its upward part at most 250. Bullets and
//! melee name the shooter (a pistol body hit shoves 40 units/s, a .357's
//! 375, a crowbar's 125). A player is HL2:DM's when its identity is
//! (`slots::GameIdentity`, the gun game's), or with no identity in a game
//! whose rules are HL2:DM's (its own app). A shove of more than 140
//! units/s upward lifts the player off the ground. The server's
//! (`core::authoritative`), after the weapons and before the damage
//! lands.
//!
//! Known shortcuts: the shove reads the damage as the weapon sent it,
//! after the armour's share (the spec's is before the suit: M3's C2 brings
//! the suit's own rule); "a whole health point" is the hit's amount of at
//! least one point (the suit's carried fraction is M3's); a ladder isn't
//! told apart from walking (`MovementState::on_ladder` is; noclip is).

use bevy::prelude::*;

use chimera_core::{gameplay::Gameplay, slots::{GameIdentity, MovementSlot}};

use crate::core::{Damage, Health, MovementState, Velocity};

const UNIT: f32 = 0.0254;

/// Units/s per point of damage, on a standing player's box.
pub const PER_POINT: f32 = 5.0;
/// The shove's cap and its upward part's (units/s).
pub const CAP: f32 = 1000.0;
pub const UP_CAP: f32 = 250.0;
/// How far the attacker's centre is lowered before aiming (units).
pub const AIM_DROP: f32 = 10.0;
/// A standing player's box (units).
pub const STANDING: Vec3 = Vec3::new(32.0, 32.0, 72.0);
/// Above this upward shove (units/s) the player leaves the ground.
pub const LIFT: f32 = 140.0;

/// The shove in units/s (Source axes: x, y level, z up) of `damage` points
/// on a victim whose box is `size` (units), its centre `victim`, the
/// attacker's `attacker`.
pub fn shove(damage: f32, size: Vec3, victim: Vec3, attacker: Vec3) -> Vec3 {
    let volume = (size.x * size.y * size.z).max(1.0);
    let f = (damage * PER_POINT * (STANDING.x * STANDING.y * STANDING.z) / volume).min(CAP);
    let toward = ((attacker - Vec3::Z * AIM_DROP) - victim).normalize_or_zero();
    let mut dv = -toward * f;
    dv.z = dv.z.min(UP_CAP);
    dv
}

/// Engine space (Y up, metres) to Source's (Z up, units) as the maps
/// convert (`source_engine::content::to_engine`), and back.
fn to_source(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y) / UNIT
}

fn to_engine(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y) * UNIT
}

/// A character's box size and centre (units, Source axes).
fn bounds(at: &Transform, state: Option<&MovementState>) -> (Vec3, Vec3) {
    match state {
        Some(s) if s.hull_max != s.hull_min => {
            let size = (s.hull_max - s.hull_min).abs() / UNIT;
            let centre = at.translation + (s.hull_min + s.hull_max) / 2.0;
            (Vec3::new(size.x, size.z, size.y), to_source(centre))
        }
        // No hull: the standing box (crouched: half its height), centred
        // on the origin (the placeholder's capsule centre).
        _ => {
            let crouched = state.is_some_and(|s| s.crouching);
            let size = if crouched { STANDING * Vec3::new(1.0, 1.0, 0.5) } else { STANDING };
            (size, to_source(at.translation))
        }
    }
}

/// Whether `identity` makes a character HL2:DM's here.
pub fn is_hl2dm(identity: Option<&GameIdentity>, gameplay: Option<&Gameplay>) -> bool {
    match identity {
        Some(i) => i.0 == super::super::GAME,
        None => gameplay.is_some_and(|g| g.rules == super::super::GAME),
    }
}

/// Each damage on an HL2:DM player shoves it (module doc).
#[allow(clippy::type_complexity)]
pub fn shove_players(
    mut damage: MessageReader<Damage>,
    mut victims: Query<(
        &Transform,
        &Health,
        &mut Velocity,
        Option<&mut MovementState>,
        Option<&GameIdentity>,
        Option<&MovementSlot>,
    )>,
    others: Query<&Transform>,
    gameplay: Option<Res<Gameplay>>,
) {
    for d in damage.read() {
        let Some(attacker) = d.attacker.filter(|a| *a != d.target) else { continue };
        if d.amount < 0.01 {
            continue;
        }
        let from = match victims.get(attacker) {
            Ok((a_at, _, _, a_state, ..)) => bounds(a_at, a_state).1,
            Err(_) => match others.get(attacker) {
                Ok(a_at) => bounds(a_at, None).1,
                Err(_) => continue,
            },
        };
        let Ok((at, health, mut velocity, state, identity, slot)) = victims.get_mut(d.target) else { continue };
        if health.current <= 0.0 || !is_hl2dm(identity, gameplay.as_deref()) {
            continue;
        }
        if slot.is_some_and(|s| s.0 == chimera_sim::movement::noclip::ID) || state.as_ref().is_some_and(|s| s.on_ladder) {
            continue;
        }
        let (size, centre) = bounds(at, state.as_deref());
        let dv = shove(d.amount * 100.0, size, centre, from);
        velocity.0 += to_engine(dv);
        if dv.z > LIFT
            && let Some(mut s) = state
        {
            s.on_ground = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// projectiles.md test 13 and corrections: a pistol body hit 40, a
    /// .357 375, a .357 head shot capped at 1000, ducked twice.
    #[test]
    fn shoves_by_the_damage() {
        let victim = Vec3::new(0.0, 0.0, 36.0);
        let attacker = Vec3::new(-100.0, 0.0, 46.0);
        let pistol = shove(8.0, STANDING, victim, attacker);
        assert!((pistol - Vec3::new(40.0, 0.0, 0.0)).length() < 1e-3, "{pistol}");
        assert!((shove(75.0, STANDING, victim, attacker).x - 375.0).abs() < 1e-2);
        assert!((shove(225.0, STANDING, victim, attacker).x - 1000.0).abs() < 1e-2);
        let ducked = shove(8.0, STANDING * Vec3::new(1.0, 1.0, 0.5), victim, attacker);
        assert!((ducked.x - 80.0).abs() < 1e-3, "{ducked}");
        // From below, the upward part is capped.
        let up = shove(225.0, STANDING, victim, Vec3::new(-10.0, 0.0, -400.0));
        assert!(up.z <= UP_CAP + 1e-3 && up.z > 200.0, "{up}");
    }

    #[test]
    fn source_and_engine_axes_round_trip() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!((to_source(to_engine(v)) - v).length() < 1e-4);
        // Engine up is Source's z.
        assert!((to_source(Vec3::Y * UNIT) - Vec3::Z).length() < 1e-4);
    }
}
