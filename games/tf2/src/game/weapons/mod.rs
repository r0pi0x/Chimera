//! TF2's guns as data (docs/weapon-data.md): one TOML file per gun in
//! games/tf2/data/weapons (built in, parsed at run time: `tf2_weapons`)
//! with specs/tf2/weapons.md's public numbers: the Scattergun, the Rocket
//! Launcher, the Engineer's Shotgun and the Minigun (M2b of
//! docs/plans/active/tf2.md). The game's own weapon scripts override them
//! once their fields are specced (`script_numbers`: the seam, logging
//! which source served).
//!
//! The shared frame (`chimera_sim::weapon`) runs them: pellets in a fixed
//! cone (`SpreadShape::Template`: each axis the sum of two halves, open
//! question 1), a target's pellets of one shot summed into one damage
//! event (Chimera's delivery), shells one at a time stopped by a shot
//! (`ShellReload`, back to back), the deploy 0.5 s. What data can't say
//! is the hooks it names (`register_hooks`):
//!
//! - `tf2:gun` (`damage::Tf2Gun`): the distance modifier with the gun's
//!   near-range scale (`DamageEffect::by_distance`, `damage::rule`), the
//!   first pellet dead centre after 0.25 s without a shot
//!   (`Hitscan::centre_first`), the reload's start by the class whose
//!   arms hold it, the script it reads;
//! - `tf2:rocket` (`rocket`): the rocket's explosion on Chimera's flyers
//!   (`chimera:projectile` flies it): splash, self-damage, knockback;
//! - `tf2:minigun` (`minigun`): the spin, its slowed and grounded holder,
//!   the damage ramp, its sound loops.
//!
//! Random critical hits stay off (the party's choice: a gun game has no
//! damage history to roll on, and `tf_weapon_criticals` is the server's;
//! weapons.md 2.6); random damage spread is off as
//! `tf_damage_disablespread` 1, the server default (2.3).
//!
//! `Tf2GunsPlugin` is the guns alone: what TF2 adds to a game of several
//! (`super::party_sim`) and to its own simulation.
//!
//! Known shortcuts:
//! - Crits and the damage spread's random jitter aren't modelled (both
//!   off by default, above); the fixed spread grid
//!   (`tf_use_fixed_weaponspreads`) neither.
//! - Bullets push nobody (only the rocket's blast does): the knockback
//!   per point is open (weapons.md, open question 9).
//! - A network client redraws another's shot without its centred first
//!   pellet (its own and the server's have it).

pub mod damage;
pub mod minigun;
pub mod rocket;
pub mod script_numbers;

use bevy::prelude::*;
use chimera_sim::weapon::{
    WeaponFrame,
    data::{WeaponDataAppExt, WeaponSet},
    harm::HarmAppExt,
};

use crate::core::Predict;

macro_rules! data_file {
    ($name:literal) => {
        ($name, include_str!(concat!("../../../data/weapons/", $name)))
    };
}

/// The weapon documents, built in, in registration order (network
/// messages name weapons by registry index).
pub const DATA: &[(&str, &str)] = &[
    data_file!("_base.toml"),
    data_file!("_gun.toml"),
    data_file!("ammo.toml"),
    data_file!("scattergun.toml"),
    data_file!("rocketlauncher.toml"),
    data_file!("shotgun_primary.toml"),
    data_file!("minigun.toml"),
];

pub const SCATTERGUN: &str = "tf2:tf_weapon_scattergun";
pub const ROCKET_LAUNCHER: &str = "tf2:tf_weapon_rocketlauncher";
pub const SHOTGUN: &str = "tf2:tf_weapon_shotgun_primary";
pub const MINIGUN: &str = "tf2:tf_weapon_minigun";

/// The four guns, in the ladder's order.
pub const GUNS: [&str; 4] = [SCATTERGUN, ROCKET_LAUNCHER, SHOTGUN, MINIGUN];

/// The hooks the data names.
pub const GUN_HOOK: &str = "tf2:gun";
pub const ROCKET_HOOK: &str = "tf2:rocket";
pub const MINIGUN_HOOK: &str = "tf2:minigun";

/// The rocket's model key (`ShownItem`) and model.
pub const ROCKET_SHOWN: &str = "tf2:rocket";
pub const ROCKET_MODEL: &str = "models/weapons/w_models/w_rocket.mdl";

/// TF2's weapons as data, parsed (a fresh copy each call).
pub fn tf2_weapons() -> WeaponSet {
    WeaponSet::parse(DATA, &[]).unwrap_or_else(|e| panic!("TF2 weapon data: {e}"))
}

/// Each gun's hook parameters as its data gives them.
pub fn hook_params<T: serde::de::DeserializeOwned>(weapon: &str, hook: &str) -> T {
    let set = tf2_weapons();
    let data = set.weapons.iter().find(|w| w.id == weapon).unwrap_or_else(|| panic!("no {weapon}"));
    data.hooks
        .get(hook)
        .cloned()
        .unwrap_or_else(|| panic!("{weapon}: no {hook}"))
        .try_into()
        .unwrap_or_else(|e| panic!("{weapon} {hook}: {e}"))
}

/// Each gun and the class whose arms hold it (`tf2:gun`'s `arms`; the
/// plan's decision 6).
pub fn arms_by_gun() -> Vec<(&'static str, String)> {
    GUNS.iter().map(|g| (*g, hook_params::<damage::GunParams>(g, GUN_HOOK).arms)).collect()
}

/// The guns' display names (the HUD's, the ladder's), by id.
pub fn display_names(_: &crate::mount::Mount) -> Vec<(String, String)> {
    tf2_weapons()
        .weapons
        .into_iter()
        .filter_map(|w| Some((w.id, w.hud.name?)))
        .collect()
}

/// Register the behaviour TF2's weapon data names.
pub fn register_hooks(app: &mut App) {
    app.register_weapon_hook::<damage::GunParams>(GUN_HOOK, |e, _, p| damage::attach(e, p))
        .register_weapon_hook::<rocket::RocketParams>(ROCKET_HOOK, |e, _, p| {
            e.insert(rocket::Tf2Rocket(p.clone()));
        })
        .register_weapon_hook::<minigun::MinigunParams>(MINIGUN_HOOK, |e, _, p| minigun::attach(e, p));
    // Whether each hurts a player (an automatic ladder draws only those
    // that do): the rocket's blast; the guns' damage is their `[damage]`.
    app.add_harm_reader(ROCKET_HOOK, |p| chimera_sim::weapon::harm::num(p, "damage").is_some_and(|d| d > 0.0));
    // What a drawn ladder's power curve reads of them: the distance
    // modifier on a pellet, the rocket's blast, the Minigun's spin-up.
    use chimera_sim::minigame::power::PowerAppExt;
    app.add_power_reader(GUN_HOOK, power_gun)
        .add_power_reader(ROCKET_HOOK, power_rocket)
        .add_power_reader(MINIGUN_HOOK, power_minigun);
}

use chimera_sim::{
    minigame::power::{GunStats, ReadAt},
    weapon::harm::{HookParams, num},
};

/// A gun's ramp k (`tf2:gun`), 1 without.
fn ramp_of(at: &ReadAt) -> f32 {
    at.data.hooks.get(GUN_HOOK).and_then(|g| num(g, "ramp")).unwrap_or(1.0)
}

/// A pellet's damage at the distance: the distance modifier on it.
fn power_gun(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    s.damage *= damage::modifier(at.distance, num(p, "ramp").unwrap_or(1.0));
}

/// The rocket's blast on the one it meets, at the distance.
fn power_rocket(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    s.damage = num(p, "damage").unwrap_or(0.0) * at.data.units.damage * damage::modifier(at.distance, ramp_of(at));
    s.pellets = 1;
}

/// The spin-up before the first shot.
fn power_minigun(p: &HookParams, _: &ReadAt, s: &mut GunStats) {
    s.latency += num(p, "wind_up").unwrap_or(0.0);
}

/// TF2's guns: their data, hooks and systems.
pub struct Tf2GunsPlugin;

impl Plugin for Tf2GunsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_sim::weapon::{CarriedAppExt, SelectWeapons, projectile::FlyerSet};
        register_hooks(app);
        if let Err(e) = app.add_weapon_data(DATA) {
            panic!("TF2 weapon data: {e}");
        }
        app.insert_resource(rocket::RocketRules::from_set(&tf2_weapons()))
            .add_message::<crate::map::GameSound>()
            .add_message::<crate::map::PredictedSound>()
            .add_systems(
                Predict::Weapons,
                (damage::centre_first.before(WeaponFrame), damage::note_shots.after(WeaponFrame)),
            )
            .add_systems(Predict::Select, minigun::spin.before(SelectWeapons))
            .add_systems(
                FixedUpdate,
                rocket::explode.in_set(FlyerSet::Touch).run_if(crate::core::authoritative),
            )
            .predicted_part::<damage::Tf2Gun>()
            .predicted_part::<minigun::Tf2Minigun>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file in data/weapons is built in, and the four guns load
    /// with their classes for the automatic ladder.
    #[test]
    fn the_data_is_built_in_and_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weapons");
        let files = WeaponSet::read_dir(&dir).unwrap();
        for (name, text) in &files {
            let built_in = DATA.iter().find(|(n, _)| n == name);
            assert_eq!(built_in.map(|d| d.1), Some(text.as_str()), "{name} not in `DATA`");
        }
        assert_eq!(files.len(), DATA.len());
        let set = tf2_weapons();
        let ids: Vec<&str> = set.weapons.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, GUNS);
        use chimera_core::game_parts::GunClass;
        let class = |id: &str| set.weapons.iter().find(|w| w.id == id).and_then(|w| w.class);
        assert_eq!(class(SCATTERGUN), Some(GunClass::Shotgun));
        assert_eq!(class(SHOTGUN), Some(GunClass::Shotgun));
        assert_eq!(class(ROCKET_LAUNCHER), Some(GunClass::Launcher));
        assert_eq!(class(MINIGUN), Some(GunClass::Heavy));
        let arms: Vec<String> = arms_by_gun().into_iter().map(|(_, a)| a).collect();
        assert_eq!(arms, ["scout", "soldier", "engineer", "heavy"]);
    }
}
