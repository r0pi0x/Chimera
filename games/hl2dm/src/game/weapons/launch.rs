//! What HL2:DM's projectiles share (specs/hl2dm/projectiles.md 1-3), on
//! Chimera's flyers (`weapon::projectile`, lift C1):
//!
//! - `hl2_blast`: every explosion's rule (2): linear falloff from the
//!   blast point lifted 1 unit, characters aimed at their eye with the
//!   distance to where that line enters their box, the world (static
//!   props, brushes) blocking whole, a body between shielding mass/350
//!   (85 kg for a player, 0.25 for anything else), the force 300 per point
//!   of the victim's own damage up to 30000 (kg·in/s) with a 0.85-1.15
//!   jitter, blast damage through the suit at two armour per point
//!   (`suit`), the shake; the shove of the victims is `shove`'s, from the
//!   blast point (`weapon::Inflicted`); under water (the point and 24 above
//!   it in water) it reaches only those with their feet in water, dry only
//!   those not wholly under; doors on the line block it whole; the hearing
//!   (2.2): shock past a health part of 30, else ringing within 240 units
//!   (the shared presets, `weapon::grenade`).
//! - `Hl2Rounds`: a weapon's own count of a second ammo type (the SMG1's
//!   grenades, the AR2's balls) or of its only one (rockets, grenades,
//!   S.L.A.M.s), predicted with the weapon, with its carry limit.
//! - `aim`: the eye, the view (with or without the punch) and its basis
//!   in engine space; `to_engine`/`to_source` for Source vectors.
//! - `fill_up`: `impulse 101` fills them (`weapon::FilledUp`);
//!   `loadout_rounds`: a weapon a minigame's rules hand out (a gun game's
//!   rung, `minigame::LoadoutWeapon`) gets its data's `loadout` of them.
//! - `mark_under_water`: `LaunchUnderWater` on players whose eyes are under
//!   water (the crossbow's slower bolt, the refusals of attack2).
//! - The projectiles' models (`MODELS`): drawn by key on clients
//!   (`ShownItem`), loaded with the weapons' world models.
//!
//! Known shortcuts: a prop's water level in the blast's rule is its box's
//! bottom and top at its centre (a push on a loose body, its centre alone);
//! the spawn grace of rules.md (0.5 s) waits for M7.

use std::sync::Arc;

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    NetPart, ViewPunch, aim_of,
    grenade::{Blast, BlastHearing, EAR_RING_DISTANCE, EXPLOSION_RING, SHOCK_DAMAGE, SHOCK_MUFFLE, Shake, SourceRadius},
    projectile::LaunchUnderWater,
};
use source_engine::source_movement::SourceMovement;

use crate::core::{Intent, MovementState};

/// Metres per Source unit.
pub const U: f32 = 0.0254;
/// Health per hit point (normalized).
pub const HP: f32 = 0.01;

/// Engine space (Y up, metres) from Source's (Z up, units), as the maps
/// convert.
pub fn to_engine(v: Vec3) -> Vec3 {
    Vec3::new(v.x, v.z, -v.y) * U
}

/// Source's (Z up, units) from engine space.
pub fn to_source(v: Vec3) -> Vec3 {
    Vec3::new(v.x, -v.z, v.y) / U
}

/// Every HL2:DM explosion (projectiles.md 2): `damage` points in `radius`
/// units, the ground probed `probe` (up, down, pulled out: units) under
/// the point (the frag's), `sound` played there.
pub fn hl2_blast(damage: f32, radius: f32, probe: Option<(f32, f32, f32)>, sound: Option<&str>) -> Blast {
    let (up, down, out) = probe.unwrap_or((0.0, 0.0, 0.0));
    Blast {
        damage: damage * HP,
        radius: radius * U,
        bell_sigma: None,
        legacy: false,
        // The suit's own rule (`suit::suit_armour`: 80 % taken, blasts at
        // two armour a point) with ratio 1: health takes 20 %.
        armor_ratio: Some(1.0),
        quantum: 0.0,
        probe_up: up * U,
        probe_down: down * U,
        pull_out: out * U,
        src_lift: 1.0 * U,
        // The eye.
        body_target: (1.0, 1.0),
        force: 0.0,
        force_jitter: (0.85, 1.15),
        max_push_speed: 2000.0 * U,
        ragdolls: None,
        scorch: Some("Scorch".into()),
        // An empty entry is no server sound (the rocket's and the
        // satchel's boom is each client's own: `projectile_fx`).
        sound: sound.filter(|s| !s.is_empty()).map(str::to_string),
        // After the suit: a health part of 30 shocks, else within 240 of
        // the blast rings (2.2), CS:S's presets.
        hearing: Some(BlastHearing {
            after_armour: true,
            shock_damage: SHOCK_DAMAGE * HP,
            shock: SHOCK_MUFFLE,
            ring_distance: EAR_RING_DISTANCE * U,
            ring: EXPLOSION_RING,
        }),
        shake: Some(Shake {
            amplitude: 25.0 * U,
            frequency: 150.0,
            duration: 1.0,
            radius: 750.0 * U,
        }),
        shield: None,
        through_walls: false,
        radius_rule: Some(SourceRadius {
            shield_mass: 350.0,
            shield_other: 0.25,
            character_mass: 85.0,
            // 300 kg·in/s per point, at most 30000.
            per_damage: 300.0 * U / HP,
            force_cap: 30000.0 * U,
            water_lift: 24.0 * U,
            stoppers: &["prop_door_rotating", "func_door", "func_door_rotating"],
        }),
    }
}

/// `hl2_blast` shared.
pub fn blast(damage: f32, radius: f32, probe: Option<(f32, f32, f32)>, sound: Option<&str>) -> Arc<Blast> {
    Arc::new(hl2_blast(damage, radius, probe, sound))
}

/// A weapon's hook parameters as its data gives them (`weapon`'s
/// `hook`): for tests and tools.
pub fn hook_params<T: serde::de::DeserializeOwned>(weapon: &str, hook: &str) -> T {
    let set = super::hl2_weapons();
    let data = set
        .weapons
        .iter()
        .find(|w| w.id == weapon)
        .unwrap_or_else(|| panic!("no {weapon}"));
    data.hooks
        .get(hook)
        .cloned()
        .unwrap_or_else(|| panic!("{weapon}: no {hook}"))
        .try_into()
        .unwrap_or_else(|e| panic!("{weapon} {hook}: {e}"))
}

/// A weapon's own rounds of an ammo type (`ammo`), at most `max`; a
/// weapon a minigame's rules hand out (`minigame::LoadoutWeapon`: a gun
/// game's rung) gets `loadout` of them (`loadout_rounds`), none: its own
/// `count`.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct Hl2Rounds {
    pub ammo: &'static str,
    pub count: u32,
    pub max: u32,
    pub loadout: Option<u32>,
}

impl NetPart for Hl2Rounds {
    type Net = u32;
    fn net(&self) -> u32 {
        self.count
    }
    fn set_net(&mut self, count: u32) {
        self.count = count;
    }
}

/// Parameters naming a weapon's own rounds (`Hl2Rounds`).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct RoundsParams {
    pub ammo: String,
    #[serde(default)]
    pub count: u32,
    pub max: u32,
    /// How many a minigame's rules give with the weapon (a gun game's
    /// rung: ours, the user's call 2026-11-12; HL2:DM itself has no such
    /// rules).
    #[serde(default)]
    pub loadout: Option<u32>,
}

impl RoundsParams {
    pub fn rounds(&self) -> Hl2Rounds {
        Hl2Rounds {
            ammo: Box::leak(self.ammo.clone().into_boxed_str()),
            count: self.count.min(self.max),
            max: self.max,
            loadout: self.loadout,
        }
    }
}

/// A weapon the minigame's rules handed out (`minigame::LoadoutWeapon`):
/// its own rounds' `loadout` count (at most its `max`), when its data
/// gives one.
pub fn loadout_rounds(add: On<Add<chimera_sim::minigame::LoadoutWeapon>>, mut rounds: Query<&mut Hl2Rounds>) {
    if let Ok(mut r) = rounds.get_mut(add.entity)
        && let Some(n) = r.loadout
    {
        r.count = n.min(r.max);
    }
}

/// `impulse 101`: every weapon's own rounds full (`weapon::FilledUp`).
pub fn fill_up(
    mut filled: MessageReader<chimera_sim::weapon::FilledUp>,
    owners: Query<&chimera_sim::weapon::Inventory>,
    mut rounds: Query<&mut Hl2Rounds>,
) {
    for f in filled.read() {
        let Ok(inv) = owners.get(f.player) else { continue };
        for w in &inv.weapons {
            if let Ok(mut r) = rounds.get_mut(*w) {
                r.count = r.max;
            }
        }
    }
}

/// A character's eye and view in engine space: (eye, forward, right, up),
/// the view with the punch (`punched`) or without.
pub fn aim(
    at: &Transform,
    state: &MovementState,
    intent: &Intent,
    punch: Option<&ViewPunch>,
    punched: bool,
) -> (Vec3, Vec3, Vec3, Vec3) {
    let kick = if punched {
        punch.map_or(Vec2::ZERO, |p| p.0.truncate())
    } else {
        Vec2::ZERO
    };
    let q = aim_of(intent.yaw + kick.y, intent.pitch + kick.x);
    (
        at.translation + state.eye_offset,
        q * Vec3::NEG_Z,
        q * Vec3::X,
        q * Vec3::Y,
    )
}

/// `LaunchUnderWater` follows the eyes (the movement's water level 3).
pub fn mark_under_water(q: Query<(Entity, &SourceMovement, Has<LaunchUnderWater>)>, mut commands: Commands) {
    for (e, m, marked) in &q {
        let under = m.water_level >= 3;
        if under && !marked {
            commands.entity(e).insert(LaunchUnderWater);
        } else if !under && marked {
            commands.entity(e).remove::<LaunchUnderWater>();
        }
    }
}

/// The projectiles' model keys (`ShownItem`), their files and the weapon
/// they come from (a map's kit loads them with it).
pub const SMG1_GRENADE_MODEL: &str = "hl2dm:grenade_ar2";
pub const BALL_MODEL: &str = "hl2dm:prop_combine_ball";
pub const MISSILE_MODEL: &str = "hl2dm:rpg_missile";
pub const BOLT_MODEL: &str = "hl2dm:crossbow_bolt";
pub const SATCHEL_MODEL: &str = "hl2dm:npc_satchel";
pub const MINE_MODEL: &str = "hl2dm:npc_tripmine";
pub const MODELS: &[(&str, &str, &str)] = &[
    (SMG1_GRENADE_MODEL, "models/Weapons/ar2_grenade.mdl", super::SMG1),
    (BALL_MODEL, "models/effects/combineball.mdl", super::AR2),
    (MISSILE_MODEL, "models/weapons/w_missile.mdl", super::RPG),
    (BOLT_MODEL, "models/crossbow_bolt.mdl", super::CROSSBOW),
    (SATCHEL_MODEL, "models/weapons/w_slam.mdl", super::SLAM),
    (MINE_MODEL, "models/weapons/w_slam.mdl", super::SLAM),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_round_trip() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert!((to_source(to_engine(v)) - v).length() < 1e-4);
        assert!((to_engine(Vec3::Z) - Vec3::Y * U).length() < 1e-6);
    }

    #[test]
    fn the_blast_is_in_meters_and_health() {
        let b = hl2_blast(150.0, 250.0, Some((8.0, 32.0, 0.6)), None);
        assert!((b.damage - 1.5).abs() < 1e-6);
        assert!((b.radius - 6.35).abs() < 1e-5);
        let r = b.radius_rule.unwrap();
        // 300 kg·in/s per point.
        assert!((r.per_damage * 0.01 - 300.0 * U).abs() < 1e-4);
    }
}
