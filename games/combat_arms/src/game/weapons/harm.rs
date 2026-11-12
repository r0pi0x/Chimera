//! Combat Arms' answers to whether a weapon can damage a player
//! (`chimera_sim::weapon::harm`, `HarmAppExt::add_harm_reader`): each of
//! its hooks' damage paths, from the numbers the hook's data gives
//! (specs/combat_arms/special_weapons.md 1-4, 6), and its data's draw
//! exclusions (data/ladder_exclusions.toml, `auto::DrawExclusions`).
//!
//! - `combat_arms:damage`: the ray's `InstDamage` above 0 of a type other
//!   than 0 (a type 0 deals nothing, 1.3).
//! - `combat_arms:knife`: the primary swing's or the alternate's
//!   (`Alt_InstDamage`) damage above 0.
//! - `combat_arms:projectile`: a blast (`AreaDamage` and its radius above
//!   0; the M136_AT4's is 0, Quirk 1) or a burning sphere (`ProgDamage`
//!   above 0 for a time, in a radius; the RMS12's is 0, 4).
//! - `combat_arms:bolt`: the bolt's damage, or the explosive bolt's blast.
//! - `combat_arms:grenade`: an explosion's damage (not a flash's or a
//!   smoke's) or a sphere's burn.
//! - `combat_arms:device`: a blast, the claymore's cone, the caltrops'
//!   bleed, the turret's gun or an air strike shell's blast or sphere
//!   (the nerve mine, the kits and the SA-18's smoke marker hurt nobody, 6).

use chimera_sim::{
    minigame::auto::DrawExclusionsAppExt,
    weapon::harm::{HarmAppExt, HookParams, num},
};

/// The draw exclusions, as shipped.
pub const LADDER_EXCLUSIONS: &str = include_str!("../../../data/ladder_exclusions.toml");

fn positive(p: &HookParams, key: &str) -> bool {
    num(p, key).is_some_and(|x| x > 0.0)
}

/// A ray's damage table (`DamageParams`).
fn ray(p: &HookParams) -> bool {
    positive(p, "damage") && num(p, "damage_type").is_none_or(|t| t != 0.0)
}

/// A blast (`area_damage` in `area_radius`).
fn blast(p: &HookParams) -> bool {
    positive(p, "area_damage") && positive(p, "area_radius")
}

/// A lasting sphere (`OverTimeParams`).
fn sphere(p: &HookParams) -> bool {
    p.get("over_time").is_some_and(|o| positive(o, "rate") && positive(o, "lifetime") && positive(o, "radius"))
}

fn projectile(p: &HookParams) -> bool {
    blast(p) || sphere(p)
}

fn knife(p: &HookParams) -> bool {
    ["primary", "secondary"].iter().any(|k| p.get(*k).is_some_and(|a| positive(a, "damage")))
}

fn bolt(p: &HookParams) -> bool {
    p.get("damage").is_some_and(ray) || p.get("explosive").is_some_and(blast)
}

fn grenade(p: &HookParams) -> bool {
    let explodes = p.get("area_type").and_then(|t| t.as_str()).is_none_or(|t| t == "explode");
    (explodes && positive(p, "area_damage")) || sphere(p)
}

fn device(p: &HookParams) -> bool {
    blast(p)
        || p.get("arc").is_some_and(|a| positive(a, "damage") && positive(a, "radius"))
        || p.get("caltrops").is_some_and(|c| positive(c, "rate"))
        || p.get("turret").and_then(|t| t.get("gun")).is_some_and(ray)
        || p.get("strike").and_then(|s| s.get("shell")).is_some_and(projectile)
}

/// Combat Arms' readers and exclusions registered.
pub fn plugin(app: &mut bevy::prelude::App) {
    app.add_harm_reader(super::DAMAGE, ray)
        .add_harm_reader(super::super::inventory::KNIFE_HOOK, knife)
        .add_harm_reader(super::super::special::data::PROJECTILE, projectile)
        .add_harm_reader(super::super::special::data::BOLT, bolt)
        .add_harm_reader(super::super::grenades::HOOK, grenade)
        .add_harm_reader(super::super::special::data::DEVICE, device)
        .add_draw_exclusions("combat_arms data/ladder_exclusions.toml", LADDER_EXCLUSIONS);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(text: &str) -> HookParams {
        toml::Value::Table(toml::from_str(text).unwrap())
    }

    #[test]
    fn each_path_reads_its_numbers() {
        assert!(ray(&t("damage = 35.0\ndamage_type = 2")));
        assert!(!ray(&t("damage = 0.0\ndamage_type = 2")), "the M4 DEVGRU's InstDamage 0");
        assert!(!ray(&t("damage = 35.0\ndamage_type = 0")), "type 0 deals nothing");
        assert!(projectile(&t("area_damage = 40.0\narea_radius = 550.0")), "the LAW");
        assert!(!projectile(&t("area_damage = 0.0\narea_radius = 550.0")), "the M136_AT4");
        let flame = "area_damage = 0.0\narea_radius = 0.0\n[over_time]\nkind = \"burn\"\nduration = 1.0\nlifetime = 2.0\nradius = 75.0\n";
        assert!(projectile(&t(&format!("{flame}rate = 12.0"))), "the Flamethrower");
        assert!(!projectile(&t(&format!("{flame}rate = 0.0"))), "the RMS12");
        assert!(grenade(&t("area_damage = 60.0\narea_type = \"explode\"")));
        assert!(!grenade(&t("area_damage = 1.0\narea_type = \"smoke\"")), "a smoke marker");
        assert!(knife(&t("[primary]\ndamage = 0.0\n[secondary]\ndamage = 65.0")));
        assert!(!device(&t("area_damage = 0.0\n[kit]\nheal = 5.0")), "a first-aid kit");
        assert!(device(&t("area_damage = 0.0\n[caltrops]\nrate = 3.0")));
    }

    #[test]
    fn the_shipped_exclusions_read() {
        let list = chimera_sim::minigame::auto::parse_exclusions(LADDER_EXCLUSIONS).expect("reads");
        assert!(list.iter().any(|(id, _)| id == "combat_arms:asw338lmspunk"));
        assert!(list.iter().all(|(id, r)| id.starts_with("combat_arms:") && !r.is_empty()));
    }
}
