//! Death and bodies (specs/combat_arms/characters.md section 10): which
//! death animation plays, how a body that died in a jump is turned, and
//! when a player's body goes.

use bevy::prelude::*;

/// What killed (10, "Death animation choice").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeathKind {
    /// Bullets and anything not below.
    Other,
    /// Explosions, claymores and land mines ("blown off").
    Explosion,
    EndlessFall,
    Poison,
    Melee,
    /// A frozen player in the snowball mode.
    Frozen,
}

/// The death's damage kind (10, Open question 21): `Melee` the knife's,
/// blasts the blown-off set, an `EndlessFall` volume's damage (its weapon
/// `volumes::ENDLESS_FALL`) the endless fall, a thrown weapon's lasting
/// generic damage (a gas grenade's `Poison`) poison; the rest bullet-style.
pub fn kind_of(catalog: &crate::weapon::data::WeaponCatalog, kind: crate::core::DamageKind, weapon: Option<&str>) -> DeathKind {
    use crate::core::DamageKind as K;
    match kind {
        K::Blast | K::BlastSurface => DeathKind::Explosion,
        K::Melee => DeathKind::Melee,
        _ if weapon == Some(super::super::volumes::ENDLESS_FALL) => DeathKind::EndlessFall,
        K::Generic if weapon.is_some_and(|w| is_thrown(catalog, w)) => DeathKind::Poison,
        _ => DeathKind::Other,
    }
}

/// A thrown weapon (grenade data).
fn is_thrown(catalog: &crate::weapon::data::WeaponCatalog, id: &str) -> bool {
    catalog.get(id).is_some_and(|d| d.has_hook(super::super::grenades::HOOK))
}

/// The C library's generator (the Microsoft LCG) seeded with `seed`,
/// first draw mod 2 (Quirk 6: the coin, true for 1).
pub fn coin(seed: u32) -> bool {
    let next = seed.wrapping_mul(214_013).wrapping_add(2_531_011);
    ((next >> 16) & 0x7fff) % 2 == 1
}

/// The choice's inputs. Directions in the character's frame or the
/// world's, as long as all three agree.
#[derive(Clone, Debug)]
pub struct DeathInputs {
    pub kind: DeathKind,
    /// The direction the killing damage travelled.
    pub dir: Vec3,
    pub forward: Vec3,
    pub right: Vec3,
    /// The last hit sphere's location was the head (1) / the groin (5).
    pub head: bool,
    pub groin: bool,
    /// The coin c (true: 1).
    pub coin: bool,
    /// The player was in the jump state.
    pub jumping: bool,
    /// The model record's special death animation, if it names one.
    pub special: Option<String>,
}

/// The death animation's name; `exists` says whether the library has a
/// name (a missing one plays `DC`).
pub fn choose(i: &DeathInputs, exists: impl Fn(&str) -> bool) -> String {
    if let Some(s) = &i.special {
        return s.clone();
    }
    if i.kind == DeathKind::Frozen {
        return "SD".into();
    }
    let c45 = std::f32::consts::FRAC_1_SQRT_2;
    let (cf, cr) = (i.dir.dot(i.forward), i.dir.dot(i.right));
    let front = cf < 0.0;
    let left = cr > 0.0;
    let coin = i.coin && !i.jumping;
    let pick = |a: &str, b: &str| if coin { a.to_string() } else { b.to_string() };
    let name = match i.kind {
        DeathKind::Explosion => {
            if front && cf < -c45 {
                "D_BO_F".into()
            } else if !front && cf > c45 {
                "D_BO_B".into()
            } else if left {
                "D_BO_L".into()
            } else {
                "D_BO_R".into()
            }
        }
        DeathKind::EndlessFall => "D_W".into(),
        DeathKind::Poison => "D_P".into(),
        DeathKind::Melee => "D_S".into(),
        _ if i.groin => pick("NS1", "NS2"),
        _ if front && cf < -c45 => {
            if i.head {
                pick("DH", "DH_1")
            } else {
                pick("DT", "DT_1")
            }
        }
        // Front-left: the head case is `DLSH`; the body case looks up a
        // handle that is never set, so the missing-animation `DC` plays
        // (Corrections 2).
        _ if front && left => (if i.head { "DLSH" } else { "DC" }).into(),
        _ if front => (if i.head { "DRSH" } else { "DRT" }).into(),
        _ if cf > c45 => {
            if i.head {
                pick("DHB", "DHB_1")
            } else {
                pick("DTB", "DTB_1")
            }
        }
        _ if left => (if i.head { "DLSH" } else { "DLT" }).into(),
        _ => (if i.head { "DRSH" } else { "DRT" }).into(),
    };
    if exists(&name) { name } else { "DC".into() }
}

/// The yaw a body that died in a jump is turned to (spec frame: radians,
/// positive right, 0 along +Z): atan(d_x / d_z), +pi when d_z > 0, +-pi/2
/// by d_x's sign when d_z = 0.
pub fn jump_yaw(dir: Vec3) -> f32 {
    use std::f32::consts::{FRAC_PI_2, PI};
    if dir.z == 0.0 {
        FRAC_PI_2.copysign(dir.x)
    } else {
        let a = (dir.x / dir.z).atan();
        if dir.z > 0.0 { a + PI } else { a }
    }
}

/// When a body goes (10, "How long the body stays"): `delay` s after the
/// later of its owner's respawn and its death animation's end; None
/// before the respawn or in a mode that never removes bodies.
pub fn removal_time(anim_end: f64, respawned: Option<f64>, delay: Option<f32>) -> Option<f64> {
    Some(respawned?.max(anim_end) + delay? as f64)
}
