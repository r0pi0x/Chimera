//! Whether a weapon can damage a player, from its own data
//! (`can_damage_player`): a gun game's drawn ladder leaves out the ones
//! that can't (docs/DESIGN.md "Gun game rules"; `minigame::auto::drawable`),
//! such as Combat Arms' `RMS12` (its flames' burn is 0 HP/s) and
//! `M136_AT4` (its rocket's blast is 0).
//!
//! Every damage path the weapon's data names is asked:
//! - Chimera's own: the ray's `[damage]` amount, a melee swing's damage
//!   (primary or secondary), a flyer's direct damage (`chimera:projectile`);
//! - each game's own hooks, through the readers it registered
//!   (`HarmAppExt::add_harm_reader`): the hook's parameters say whether
//!   that path hurts (Combat Arms: the ray, the alt swing, a blast, a burn
//!   sphere, a bolt, a grenade's blast or sphere, a device's blast, cone,
//!   caltrops or turret gun).
//!
//! Any path that hurts: yes. Paths named, none hurting: no. No path the
//! engine or a reader knows (a game hook no reader reads, HL2:DM's RPG):
//! unknown, which a draw keeps. CS:S reads its grenades' (the HE hurts,
//! the flash and smoke don't).

use bevy::prelude::*;

use super::data::WeaponData;

/// A hook's parameters as the weapon's data gives them.
pub type HookParams = toml::Value;

/// A game's reader of one of its hooks: whether that damage path can
/// hurt a player.
pub type HarmReader = fn(&HookParams) -> bool;

/// The games' readers, by hook name (`HarmAppExt::add_harm_reader`);
/// kept in `minigame::power::PowerReaders::harm`, not a resource of its
/// own (a new resource is a new entity, shifting later entities' ids).
#[derive(Clone, Default)]
pub struct HarmReaders(pub Vec<(&'static str, HarmReader)>);

/// Implemented by `minigame::power`, whose `PowerReaders` keeps the
/// readers (this layer may not name it).
pub trait HarmAppExt {
    /// `reader` says whether the damage path of hook `hook` hurts, for a
    /// weapon whose data names that hook.
    fn add_harm_reader(&mut self, hook: &'static str, reader: HarmReader) -> &mut Self;
}


/// A number of a hook's table (float or integer).
pub fn num(v: &HookParams, key: &str) -> Option<f32> {
    let x = v.get(key)?;
    x.as_float().map(|f| f as f32).or_else(|| x.as_integer().map(|i| i as f32))
}

/// Whether `data` can damage a player: Some(true) when a damage path of
/// its data hurts, Some(false) when it names damage paths and none does,
/// None when no path is known (no damage table, no swing, no hook a
/// reader reads).
pub fn can_damage_player(data: &WeaponData, readers: &HarmReaders) -> Option<bool> {
    let mut known = false;
    let mut hurts = false;
    if let Some(d) = &data.damage {
        known = true;
        hurts |= d.amount > 0.0;
    }
    if let Some(m) = &data.melee {
        known = true;
        hurts |= m.primary.damage > 0.0 || m.secondary.as_ref().is_some_and(|s| s.damage > 0.0);
    }
    if let Some(p) = data.hooks.get(super::projectile::LAUNCHER) {
        known = true;
        hurts |= num(p, "damage").is_some_and(|d| d > 0.0);
    }
    for (hook, read) in &readers.0 {
        if let Some(p) = data.hooks.get(*hook) {
            known = true;
            hurts |= read(p);
        }
    }
    known.then_some(hurts)
}

/// Whether `data` surely can't damage a player (its paths named, none
/// hurting).
pub fn harmless(data: &WeaponData, readers: &HarmReaders) -> bool {
    can_damage_player(data, readers) == Some(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::weapon::data::WeaponSet;

    fn weapon(text: &str) -> WeaponData {
        let set = WeaponSet::parse(&[("w.toml", text)], &[]).expect("parses");
        set.weapons.into_iter().next().expect("a weapon")
    }

    fn burns(p: &HookParams) -> bool {
        num(p, "rate").is_some_and(|r| r > 0.0)
    }

    #[test]
    fn every_path_is_asked() {
        let readers = HarmReaders(vec![("test:burn", burns)]);
        let base = "id = \"t:w\"\nunits = { length = 1.0, damage = 1.0 }\nslot = 1\ndraw = 1.0\n";
        let ray = weapon(&format!("{base}[damage]\namount = 30.0\nfalloff = 1.0\nfalloff_step = 1.0\nimpulse = 1.0\nhitgroups = {{ head = 4.0, chest = 1.0, stomach = 1.25, arm = 1.0, leg = 0.75 }}\n"));
        assert_eq!(can_damage_player(&ray, &readers), Some(true));
        let burn = weapon(&format!("{base}[hooks.\"test:burn\"]\nrate = 12.0\n"));
        assert_eq!(can_damage_player(&burn, &readers), Some(true));
        let cold = weapon(&format!("{base}[hooks.\"test:burn\"]\nrate = 0.0\n"));
        assert_eq!(can_damage_player(&cold, &readers), Some(false));
        assert!(harmless(&cold, &readers));
        let flyer = weapon(&format!("{base}[hooks.\"chimera:projectile\"]\ndamage = 0.0\n"));
        assert_eq!(can_damage_player(&flyer, &readers), Some(false), "a flyer with no direct damage");
        let unknown = weapon(&format!("{base}[hooks.\"test:other\"]\nx = 1\n"));
        assert_eq!(can_damage_player(&unknown, &readers), None, "no reader: unknown");
        assert!(!harmless(&unknown, &readers));
    }
}
