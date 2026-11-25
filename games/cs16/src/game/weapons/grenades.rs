//! Counter-Strike 1.6's grenades (specs/cs16/weapons.md, "Grenades";
//! damage.md, "Blast damage"), the `cs16:grenade` hook on Chimera's
//! shared grenade (`chimera_sim::weapon::grenade`): the pin on the press
//! (no throw before 0.5 s), the throw on the release, the pitch bent
//! −10 + p × 80/90 looking up and × 100/90 down, the speed
//! (90 − p') × 6 up to 750 units/s plus the thrower's velocity, from 16
//! units ahead of the eye; the 1.5 s fuse checked every 0.1 s; gravity
//! 0.55 (HE) or 0.5 × `sv_gravity`; then:
//!
//! - **HE**: 100 at the point pulled 45.6 units off the ground under it,
//!   falling linearly to 0 at 350, through walls; the blast's armour rule
//!   (ratio 0.5 of the damage to health, half the rest from armour);
//! - **flash**: everyone within 1500 who sees it and whom it sees,
//!   strength s = 4 − d × 4/1500: facing it white (255) held s / 1.5 s
//!   then fading over 3s s; behind, 200 held s / 3.5 then 1.75 s s
//!   (`flash_model`; the fade of view_and_hud.md);
//! - **smoke**: once resting after the fuse, a cloud for about 20 s.
//!
//! Known shortcuts:
//! - The bounce (how much speed a contact keeps) is CS:S's shared flight
//!   (weapons.md open question 4), not GoldSrc's engine bounce; water and
//!   the ten-bounce stop are the shared rules'.
//! - A blast that uses more armour than is left takes `D − 2A` from health
//!   (the shared armour's rule), not the original's `D − A`.
//! - A second flash on a blinded player keeps the stronger of the two
//!   (`core::Blinded::raised`), not the original's sum of holds; the
//!   flash's line of sight is the shared one (the eye, not a random body
//!   point).
//! - The smoke cloud's size and timing (weapons.md open question 5) and
//!   the HE's and smoke's looks are not drawn yet (no sprites).

use bevy::prelude::*;
use chimera_sim::weapon::{
    data::WeaponData,
    grenade::{Blast, Flash, Flight, GrenadeEffect, Smoke, ThrowRule, Throwable},
};

/// The hook's name.
pub const HOOK: &str = "cs16:grenade";

const UNIT: f32 = super::super::METERS_PER_UNIT;
/// Damage points as health.
const HP: f32 = 0.01;
/// `sv_gravity`, units/s².
const GRAVITY: f32 = 800.0;

/// `cs16:grenade`'s parameters: which grenade, how many are carried and
/// its bounce sound (a wave under `sound/`).
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GrenadeParams {
    pub effect: Kind,
    pub max: u32,
    pub bounce_sound: String,
}

#[derive(serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    He,
    Flash,
    Smoke,
}

/// The throw (module doc).
pub fn throw_rule() -> ThrowRule {
    ThrowRule {
        pitch_lift: -10.0,
        pitch_scale: 100.0 / 90.0,
        pitch_scale_up: 80.0 / 90.0,
        speed_per_deg: 6.0 * UNIT,
        speed_max: 750.0 * UNIT,
        forward: 16.0 * UNIT,
    }
}

/// The flight, at `gravity` × `sv_gravity`.
pub fn flight(gravity: f32) -> Flight {
    Flight {
        half: 2.0 * UNIT,
        gravity: gravity * GRAVITY * UNIT,
        elasticity: 0.45,
        player_elasticity: 0.3,
        elasticity_cap: 0.9,
        stop_speed: 30.0 * UNIT,
        floor_normal: 0.7,
        breakable_damage: 10.0 * HP,
        breakable_slowdown: 0.4,
        water_slowdown: 0.5,
        water_entry: 0.5,
        max_velocity: 2000.0 * UNIT,
        check_interval: 0.1,
        spin_roll: 600f32.to_radians(),
        spin_pitch: 1200f32.to_radians(),
    }
}

/// Where a blast meets a player: its box's centre, as a share of its
/// eye's height above its feet (36 of 53 units).
pub const BOX_CENTRE: f32 = 36.0 / 53.0;

/// The HE blast (module doc; damage.md, "Blast damage").
pub fn he_blast() -> Blast {
    Blast {
        damage: 100.0 * HP,
        radius: 350.0 * UNIT,
        bell_sigma: None,
        legacy: false,
        // Health takes 0.5 of a covered hit: the armour's 0.5 × 1.
        armor_ratio: Some(1.0),
        quantum: HP,
        // A trace 40 units down from 8 above; the point pulled out by
        // (100 − 24) × 0.6.
        probe_up: 8.0 * UNIT,
        probe_down: 32.0 * UNIT,
        pull_out: 45.6 * UNIT,
        src_lift: 0.0,
        // A player's origin, its box's centre: 36 of the eye's 53 units
        // above the feet (movement.md).
        body_target: (BOX_CENTRE, BOX_CENTRE),
        force: 0.0,
        force_jitter: (1.0, 1.0),
        max_push_speed: 2000.0 * UNIT,
        ragdolls: None,
        scorch: None,
        sound: Some(HE_SOUND.into()),
        hearing: None,
        shake: None,
        shield: None,
        through_walls: true,
        radius_rule: None,
    }
}

/// How far a flash reaches, units.
pub const FLASH_RADIUS: f32 = 1500.0;

/// The flash's blindness at `distance` (m) and `facing` (the cosine of the
/// angle between the view and the grenade): (alpha 0-1, hold s, fade s);
/// None out of reach (module doc).
pub fn flash_model(distance: f32, facing: f32) -> Option<(f32, f32, f32)> {
    let d = distance / UNIT;
    let s = 4.0 - d * 4.0 / FLASH_RADIUS;
    if s <= 0.0 {
        return None;
    }
    Some(if facing >= 0.0 { (1.0, s / 1.5, 3.0 * s) } else { (200.0 / 255.0, s / 3.5, 1.75 * s) })
}

/// The smoke cloud (weapons.md, "Smoke grenade": puffs for about 20 s;
/// its size ours).
pub fn smoke() -> Smoke {
    Smoke {
        max_radius: 150.0 * UNIT,
        expand_time: 1.0,
        fade_start: 18.0,
        fade_end: 21.0,
        pop_speed: 0.1 * UNIT,
        sight_radius: 0.75,
        sound: Some(SMOKE_SOUND.into()),
    }
}

/// The waves a grenade's effect plays (an entry of
/// `goldsrc_engine::content::sounds`).
pub fn effect_sounds(kind: Kind) -> &'static [&'static str] {
    match kind {
        Kind::He => &[HE_SOUND],
        Kind::Flash => &[FLASH_SOUND],
        Kind::Smoke => &[SMOKE_SOUND],
    }
}

const HE_SOUND: &str = "weapons/hegrenade-1.wav;weapons/hegrenade-2.wav";
const FLASH_SOUND: &str = "weapons/flashbang-1.wav;weapons/flashbang-2.wav";
const SMOKE_SOUND: &str = "weapons/sg_explode.wav";

/// The hook: the throw, flight and effect on a grenade (the data's `draw`
/// is also the redraw after a throw).
pub fn throwable(e: &mut EntityWorldMut, d: &'static WeaponData, p: &GrenadeParams) {
    let (effect, gravity) = match p.effect {
        Kind::He => (GrenadeEffect::Blast(he_blast()), 0.55),
        Kind::Flash => (
            GrenadeEffect::Flash(Flash {
                model: flash_model,
                sound: Some(FLASH_SOUND.into()),
                hearing: None,
                through_smoke: 1.0,
            }),
            0.5,
        ),
        Kind::Smoke => (GrenadeEffect::Smoke(smoke()), 0.5),
    };
    e.insert(Throwable {
        count: 1,
        max: p.max,
        // No throw before 0.5 s of the pin pull.
        pin_time: 0.5,
        throw_delay: 0.0,
        // The next primary attack 0.5 s after the throw.
        throw_time: 0.5,
        redraw_time: d.draw,
        fuse: 1.5,
        throw: throw_rule(),
        flight: flight(gravity),
        effect,
        // The pin's sound is the view model's `pullpin` event.
        pin_sounds: Vec::new(),
        bounce_sound: Some(p.bounce_sound.clone()),
        water_sounds: None,
        pin: false,
        throw_at: None,
        redraw_at: None,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// At 0 units facing it: white, held 2.67 s, fading 12 s.
    #[test]
    fn flash_at_zero_facing() {
        let (a, hold, fade) = flash_model(0.0, 1.0).unwrap();
        assert_eq!(a, 1.0);
        assert!((hold - 8.0 / 3.0).abs() < 1e-4 && (fade - 12.0).abs() < 1e-4);
        let (a, hold, fade) = flash_model(0.0, -1.0).unwrap();
        assert!((a - 200.0 / 255.0).abs() < 1e-6 && (hold - 4.0 / 3.5).abs() < 1e-4 && (fade - 7.0).abs() < 1e-4);
        assert!(flash_model(1500.0 * UNIT, 1.0).is_none());
    }

    /// Looking level: 10° up at 600 units/s.
    #[test]
    fn a_level_throw() {
        let r = throw_rule();
        assert!((r.pitch(0.0) + 10.0).abs() < 1e-5);
        assert!((r.speed(r.pitch(0.0)) - 600.0 * UNIT).abs() < 1e-4);
    }
}
