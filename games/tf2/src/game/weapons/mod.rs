//! TF2's guns as data (docs/weapon-data.md): one TOML file per gun in
//! games/tf2/data/weapons (built in, parsed at run time: `tf2_weapons`)
//! with the specs' public numbers: the Scattergun, the Rocket Launcher,
//! the Engineer's Shotgun and the Minigun (specs/tf2/weapons.md, M2b of
//! docs/plans/active/tf2.md), and the Scout's Pistol, the SMG, the Sniper
//! Rifle, the Revolver, the Grenade and Stickybomb Launchers, the Flame
//! Thrower and the Syringe Gun (specs/tf2/weapons_more.md, the plan's
//! "More guns"). The game's own weapon scripts override them
//! where a script is served (`script_numbers`, from
//! specs/tf2/weapon_scripts.md: the store's decrypted script or the
//! install's with the key), each gun's source logged; the documents
//! registered are the set's server documents (`SERVER_DOCS`), so a client
//! plays with the server's numbers.
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
//!   arms hold it, the view kick (`punch`, `recoil`), the script it reads;
//! - `tf2:rocket` (`rocket`): the rocket's explosion on Chimera's flyers
//!   (`chimera:projectile` flies it): splash, self-damage, knockback;
//! - `tf2:minigun` (`minigun`): the spin, its slowed and grounded holder,
//!   the damage ramp, its sound loops;
//! - `tf2:sniper` (`sniper`): the scope's timing after shots and jumps,
//!   the scoped charge and the scoped headshot (the scope's look is
//!   Chimera's `[alt] scope`);
//! - `tf2:grenade` (`grenade`): the grenade's direct hit, bounces and fuse;
//! - `tf2:sticky` (`sticky`): the charged launch, sticking, arming, the
//!   limit and attack2's detonation;
//! - `tf2:flame` (`flame`): the flame points, their heat and the
//!   afterburn; attack2's airblast (`airblast`: pushes, deflections);
//! - `tf2:needle` (`needle`): a needle's hit by distance;
//! - `tf2:flare` (`flare`): the flare guns' flare (no stock weapon: the
//!   unlocks' base, `FLAREGUN`), its hit, the Detonator's and Scorch
//!   Shot's explosion and the Scorch Shot's bounce and push;
//! - batch 2b's unlocks (specs/tf2/batch2b_mechanics.md; their classes
//!   have no stock weapon: abstract bases, `ABSTRACT_BASES`): `tf2:charge`
//!   (`charge`: the Huntsman's draw, the Loose Cannon's fuse), `tf2:shot`
//!   and `tf2:throw` (`shot`: the arrow, bolt, Bison ring, cleaver and
//!   the bats' balls on touch), `tf2:shove` and `tf2:caber` (`knock`, with
//!   the Force-a-Nature's knockback), the cannonball's impact
//!   (`grenade`);
//! - crit boosts' loop and glow on the held weapon (`crit_boost`,
//!   effects.md 14);
//! - `tf2:melee` (`melee`): the nine stock melee weapons
//!   (specs/tf2/weapons_stock.md; base `tf2:melee`, Chimera's delayed
//!   swing and backstab rule): their crits, the swing's view-model
//!   actions, the raised knife.
//!
//! The rocket, grenade and sticky explode alike (`blast`), and every hit
//! pushes (`blast::push`: the blasts and the bullets, weapons.md 5.4).
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
//! - A network client redraws another's shot without its centred first
//!   pellet (its own and the server's have it).

pub mod airblast;
pub mod blast;
pub mod blood;
pub mod charge;
pub mod crit_boost;
pub mod crit_hits;
pub mod damage;
pub mod dot;
pub mod flame;
pub mod flare;
pub mod grenade;
pub mod knock;
pub mod looks;
pub mod melee;
pub mod minigun;
pub mod needle;
pub mod recoil;
pub mod rocket;
pub mod script_numbers;
pub mod shot;
pub mod sniper;
pub mod sticky;

use chimera_core::game_data::GameData;
use bevy::prelude::*;
use chimera_sim::weapon::{
    WeaponFrame,
    data::{WeaponDataAppExt, WeaponSet},
    harm::HarmAppExt,
};

use crate::core::Predict;

macro_rules! data_file {
    ($name:literal) => {
        ($name, chimera_core::game_data!("data/weapons/", $name))
    };
}

/// The weapon documents, built in, in registration order (network
/// messages name weapons by registry index).
pub const DATA: &[(&str, GameData)] = &[
    data_file!("_base.toml"),
    data_file!("_gun.toml"),
    data_file!("ammo.toml"),
    data_file!("scattergun.toml"),
    data_file!("rocketlauncher.toml"),
    data_file!("shotgun_primary.toml"),
    data_file!("minigun.toml"),
    data_file!("pistol_scout.toml"),
    data_file!("smg.toml"),
    data_file!("sniperrifle.toml"),
    data_file!("revolver.toml"),
    data_file!("grenadelauncher.toml"),
    data_file!("pipebomblauncher.toml"),
    data_file!("flamethrower.toml"),
    data_file!("syringegun_medic.toml"),
    data_file!("flaregun.toml"),
    data_file!("compound_bow.toml"),
    data_file!("crossbow.toml"),
    data_file!("raygun.toml"),
    data_file!("cannon.toml"),
    data_file!("cleaver.toml"),
    data_file!("handgun_scout_primary.toml"),
    data_file!("lunchbox.toml"),
    data_file!("lunchbox_drink.toml"),
    data_file!("jar.toml"),
    data_file!("_melee.toml"),
    data_file!("bat.toml"),
    data_file!("shovel.toml"),
    data_file!("fireaxe.toml"),
    data_file!("bottle.toml"),
    data_file!("fists.toml"),
    data_file!("wrench.toml"),
    data_file!("bonesaw.toml"),
    data_file!("club.toml"),
    data_file!("knife.toml"),
    data_file!("bat_wood.toml"),
    data_file!("bat_giftwrap.toml"),
    data_file!("stickbomb.toml"),
    data_file!("buff_item.toml"),
    data_file!("demoshield.toml"),
    data_file!("wearable.toml"),
    data_file!("invis.toml"),
    data_file!("rocketlauncher_fireball.toml"),
    data_file!("flaregun_revenge.toml"),
    data_file!("parachute.toml"),
    data_file!("rocketpack.toml"),
    data_file!("particle_cannon.toml"),
];

pub const SCATTERGUN: &str = "tf2:tf_weapon_scattergun";
pub const ROCKET_LAUNCHER: &str = "tf2:tf_weapon_rocketlauncher";
pub const SHOTGUN: &str = "tf2:tf_weapon_shotgun_primary";
pub const MINIGUN: &str = "tf2:tf_weapon_minigun";
pub const PISTOL_SCOUT: &str = "tf2:tf_weapon_pistol_scout";
pub const SMG: &str = "tf2:tf_weapon_smg";
pub const SNIPER_RIFLE: &str = "tf2:tf_weapon_sniperrifle";
pub const REVOLVER: &str = "tf2:tf_weapon_revolver";
pub const GRENADE_LAUNCHER: &str = "tf2:tf_weapon_grenadelauncher";
pub const STICKY_LAUNCHER: &str = "tf2:tf_weapon_pipebomblauncher";
pub const FLAMETHROWER: &str = "tf2:tf_weapon_flamethrower";
pub const SYRINGE_GUN: &str = "tf2:tf_weapon_syringegun_medic";
/// The flare gun class: no stock weapon, the base of the Flare Gun, the
/// Detonator and the Scorch Shot (data/weapons/flaregun.toml, abstract).
pub const FLAREGUN: &str = "tf2:tf_weapon_flaregun";
/// The batch-2b unlocks' classes with no stock weapon: abstract bases
/// (specs/tf2/batch2b_mechanics.md; data/weapons/<class>.toml).
pub const COMPOUND_BOW: &str = "tf2:tf_weapon_compound_bow";
pub const CROSSBOW: &str = "tf2:tf_weapon_crossbow";
pub const RAYGUN: &str = "tf2:tf_weapon_raygun";
pub const CANNON: &str = "tf2:tf_weapon_cannon";
pub const CLEAVER: &str = "tf2:tf_weapon_cleaver";
pub const HANDGUN_SCOUT_PRIMARY: &str = "tf2:tf_weapon_handgun_scout_primary";
pub const BAT_WOOD: &str = "tf2:tf_weapon_bat_wood";
pub const BAT_GIFTWRAP: &str = "tf2:tf_weapon_bat_giftwrap";
pub const STICKBOMB: &str = "tf2:tf_weapon_stickbomb";
/// The abstract bases (no weapon of their own; data/items.toml's
/// `[bases]` name them).
pub const ABSTRACT_BASES: [&str; 22] = [
    FLAREGUN,
    COMPOUND_BOW,
    CROSSBOW,
    RAYGUN,
    CANNON,
    CLEAVER,
    HANDGUN_SCOUT_PRIMARY,
    BAT_WOOD,
    BAT_GIFTWRAP,
    STICKBOMB,
    LUNCHBOX,
    DRINK,
    JAR,
    BUFF_ITEM,
    DEMOSHIELD,
    WEARABLE,
    WATCH,
    FIREBALL,
    FLAREGUN_REVENGE,
    PARACHUTE,
    ROCKETPACK,
    PARTICLE_CANNON,
];

/// The consumables' classes (specs/tf2/batch2b_mechanics.md 11-13; no
/// stock weapon: abstract bases, `items::consumables`).
pub const LUNCHBOX: &str = "tf2:tf_weapon_lunchbox";
pub const DRINK: &str = "tf2:tf_weapon_lunchbox_drink";
pub const JAR: &str = "tf2:tf_weapon_jar";
/// Batch 3's classes (specs/tf2/batch3_mechanics.md 1, 2; abstract bases):
/// the banners (`items::meters`' rage), the shields (`items::shields`)
/// and a worn item (the boots).
pub const BUFF_ITEM: &str = "tf2:tf_weapon_buff_item";
pub const DEMOSHIELD: &str = "tf2:tf_wearable_demoshield";
pub const WEARABLE: &str = "tf2:tf_wearable";
/// The Spy's watches' class (specs/tf2/batch3_mechanics.md 3; no stock
/// weapon of ours: an abstract base, `items::spy`).
pub const WATCH: &str = "tf2:tf_weapon_invis";
/// Batch 3b's classes (specs/tf2/batch3b_mechanics.md; abstract bases):
/// the Dragon's Fury, the Manmelter, the B.A.S.E. Jumper, the Thermal
/// Thruster and the Cow Mangler.
pub const FIREBALL: &str = "tf2:tf_weapon_rocketlauncher_fireball";
pub const FLAREGUN_REVENGE: &str = "tf2:tf_weapon_flaregun_revenge";
pub const PARACHUTE: &str = "tf2:tf_weapon_parachute";
pub const ROCKETPACK: &str = "tf2:tf_weapon_rocketpack";
pub const PARTICLE_CANNON: &str = "tf2:tf_weapon_particle_cannon";

/// The guns, in the ladder's order (M2b's four, then weapons_more.md's
/// eight).
pub const GUNS: [&str; 12] = [
    SCATTERGUN,
    ROCKET_LAUNCHER,
    SHOTGUN,
    MINIGUN,
    PISTOL_SCOUT,
    SMG,
    SNIPER_RIFLE,
    REVOLVER,
    GRENADE_LAUNCHER,
    STICKY_LAUNCHER,
    FLAMETHROWER,
    SYRINGE_GUN,
];

pub const BAT: &str = "tf2:tf_weapon_bat";
pub const SHOVEL: &str = "tf2:tf_weapon_shovel";
pub const FIREAXE: &str = "tf2:tf_weapon_fireaxe";
pub const BOTTLE: &str = "tf2:tf_weapon_bottle";
pub const FISTS: &str = "tf2:tf_weapon_fists";
pub const WRENCH: &str = "tf2:tf_weapon_wrench";
pub const BONESAW: &str = "tf2:tf_weapon_bonesaw";
pub const KUKRI: &str = "tf2:tf_weapon_club";
pub const KNIFE: &str = "tf2:tf_weapon_knife";

/// The nine stock melee weapons (specs/tf2/weapons_stock.md 1), by class
/// index order of their classes' table rows: Scout, Soldier, Pyro,
/// Demoman, Heavy, Engineer, Medic, Sniper, Spy.
pub const MELEE: [&str; 9] = [BAT, SHOVEL, FIREAXE, BOTTLE, FISTS, WRENCH, BONESAW, KUKRI, KNIFE];

/// The hooks the data names.
pub const GUN_HOOK: &str = "tf2:gun";
pub const ROCKET_HOOK: &str = "tf2:rocket";
pub const MINIGUN_HOOK: &str = "tf2:minigun";
pub const SNIPER_HOOK: &str = "tf2:sniper";
pub const GRENADE_HOOK: &str = "tf2:grenade";
pub const STICKY_HOOK: &str = "tf2:sticky";
/// TF2's air density (`tf2:air_density`, weapons_more.md 4.4).
pub const AIR_DENSITY: f32 = 2.0;
pub const FLAME_HOOK: &str = "tf2:flame";
pub const NEEDLE_HOOK: &str = "tf2:needle";
pub const FLARE_HOOK: &str = "tf2:flare";
pub const CHARGE_HOOK: &str = "tf2:charge";
pub const MELEE_HOOK: &str = "tf2:melee";
/// Chimera's flyers' hook (a projectile's launch and flight).
pub const PROJECTILE_HOOK: &str = "chimera:projectile";

/// The rocket's model key (`ShownItem`) and model.
pub const ROCKET_SHOWN: &str = "tf2:tf_weapon_rocketlauncher/rocket";
pub const ROCKET_MODEL: &str = "models/weapons/w_models/w_rocket.mdl";

/// Each gun's projectile models: (gun, model key, model)
/// (weapons_more.md 9.1). A key starts with its gun's (`<gun>/<part>`): a
/// map keeps the held models of the guns its match offers by key prefix
/// (`GameKit::stay`), and a projectile's goes with its gun.
pub const PROJECTILES: &[(&str, &str, &str)] = &[
    (ROCKET_LAUNCHER, ROCKET_SHOWN, ROCKET_MODEL),
    (GRENADE_LAUNCHER, "tf2:tf_weapon_grenadelauncher/grenade", "models/weapons/w_models/w_grenade_grenadelauncher.mdl"),
    (STICKY_LAUNCHER, sticky::SHOWN, sticky::MODEL),
    (SYRINGE_GUN, "tf2:tf_weapon_syringegun_medic/syringe", "models/weapons/w_models/w_syringe_proj.mdl"),
    (FLAREGUN, "tf2:tf_weapon_flaregun/flare", "models/weapons/w_models/w_flaregun_shell.mdl"),
    // Batch 2b's (`shot`, `charge`): their classes' abstract bases'.
    (COMPOUND_BOW, "tf2:tf_weapon_compound_bow/arrow", "models/weapons/w_models/w_arrow.mdl"),
    (CROSSBOW, "tf2:tf_weapon_crossbow/bolt", "models/weapons/w_models/w_syringe_proj.mdl"),
    (CANNON, "tf2:tf_weapon_cannon/ball", "models/weapons/w_models/w_cannonball.mdl"),
    (CLEAVER, "tf2:tf_weapon_cleaver/cleaver", "models/workshop_partner/weapons/c_models/c_sd_cleaver/c_sd_cleaver.mdl"),
    (BAT_WOOD, "tf2:tf_weapon_bat_wood/ball", "models/weapons/w_models/w_baseball.mdl"),
    (BAT_GIFTWRAP, "tf2:tf_weapon_bat_giftwrap/ornament", "models/weapons/c_models/c_xms_festive_ornament.mdl"),
    // The Candy Cane's dropped kit (`items::effects`), with the unlock.
    (super::items::effects::KIT, super::items::effects::KIT_SHOWN, super::items::effects::KIT_MODEL),
];

/// TF2's weapons as data, parsed (a fresh copy each call).
pub fn tf2_weapons() -> WeaponSet {
    WeaponSet::parse(&chimera_core::game_data::texts(DATA), &[]).unwrap_or_else(|e| panic!("TF2 weapon data: {e}"))
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

/// Each gun and melee weapon and the class whose arms hold it (`tf2:gun`'s
/// or `tf2:melee`'s `arms`; the plan's decision 6).
pub fn arms_by_gun() -> Vec<(&'static str, String)> {
    let guns = GUNS.iter().map(|g| (*g, hook_params::<damage::GunParams>(g, GUN_HOOK).arms));
    guns.chain(MELEE.iter().map(|m| (*m, hook_params::<melee::MeleeParams>(m, MELEE_HOOK).arms))).collect()
}

/// The guns and the melee weapons (what loads view and held models).
pub fn all_weapons() -> impl Iterator<Item = &'static str> {
    GUNS.into_iter().chain(MELEE)
}

/// The guns' display names (the HUD's, the ladder's), by id.
pub fn display_names(mount: &crate::mount::Mount) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = tf2_weapons()
        .weapons
        .into_iter()
        .filter_map(|w| Some((w.id, w.hud.name?)))
        .collect();
    // The unlocks the pack on `mount` gives (`items`).
    if let Some(pack) = super::items::read_pack(mount) {
        let data = super::items::ItemsData::builtin();
        for (name, def) in &data.items {
            if let Some(r) = pack.get(def.index).filter(|_| !def.wearable) {
                out.push((format!("tf2:{name}"), super::items::display_name(r)));
            }
        }
    }
    out
}

/// Register the behaviour TF2's weapon data names.
pub fn register_hooks(app: &mut App) {
    app.register_weapon_hook::<damage::GunParams>(GUN_HOOK, |e, _, p| damage::attach(e, p))
        .register_weapon_hook::<rocket::RocketParams>(ROCKET_HOOK, |e, _, p| {
            e.insert(rocket::Tf2Rocket(p.clone()));
        })
        .register_weapon_hook::<minigun::MinigunParams>(MINIGUN_HOOK, |e, _, p| minigun::attach(e, p))
        .register_weapon_hook::<sniper::SniperParams>(SNIPER_HOOK, |e, _, p| sniper::attach(e, p))
        .register_weapon_hook::<blast::BlastParams>(GRENADE_HOOK, |_, _, _| {})
        .register_weapon_hook::<sticky::StickyParams>(STICKY_HOOK, |e, _, p| sticky::attach(e, p))
        .register_weapon_hook::<flame::FlameParams>(FLAME_HOOK, |e, _, p| flame::attach(e, p))
        .register_weapon_hook::<airblast::AirblastParams>(airblast::AIRBLAST_HOOK, |e, _, p| airblast::attach(e, p))
        .register_weapon_hook::<needle::NeedleParams>(NEEDLE_HOOK, |_, _, _| {})
        .register_weapon_hook::<flare::FlareParams>(FLARE_HOOK, |_, _, _| {})
        .register_weapon_hook::<charge::ChargeParams>(CHARGE_HOOK, |e, _, p| charge::attach(e, p))
        .register_weapon_hook::<shot::ShotParams>(shot::SHOT_HOOK, |_, _, _| {})
        .register_weapon_hook::<shot::ThrowParams>(shot::THROW_HOOK, |e, _, p| shot::attach(e, p))
        .register_weapon_hook::<knock::ShoveParams>(knock::SHOVE_HOOK, |e, _, p| knock::attach_shove(e, p))
        .register_weapon_hook::<blast::BlastParams>(knock::CABER_HOOK, |e, _, p| knock::attach_caber(e, p))
        .register_weapon_hook::<melee::MeleeParams>(MELEE_HOOK, |e, _, p| melee::attach(e, p))
        .register_weapon_hook::<looks::LooksParams>(looks::LOOKS_HOOK, |_, _, _| {})
        .register_weapon_hook::<std::collections::BTreeMap<String, f32>>(super::player_anim::POSEPARAM_HOOK, |e, _, p| {
            super::player_anim::attach_pose_params(e, p)
        });
    // The physics world's air density (weapons_more.md 4.4: 2.0), which
    // scales its grenades' and stickies' air drag.
    chimera_sim::weapon::flight::register_air_density(app, super::GAME, AIR_DENSITY);
    // Whether each hurts a player (an automatic ladder draws only those
    // that do): the blasts, the flames, the needles; the guns' damage is
    // their `[damage]`.
    let hurts = |p: &chimera_sim::weapon::harm::HookParams| chimera_sim::weapon::harm::num(p, "damage").is_some_and(|d| d > 0.0);
    for hook in [ROCKET_HOOK, GRENADE_HOOK, STICKY_HOOK, FLAME_HOOK, NEEDLE_HOOK, FLARE_HOOK, shot::SHOT_HOOK] {
        app.add_harm_reader(hook, hurts);
    }
    // What a drawn ladder's power curve reads of them: the distance
    // modifier on a pellet, the rocket's blast, the Minigun's spin-up.
    use chimera_sim::minigame::power::PowerAppExt;
    app.add_power_reader(GUN_HOOK, power_gun)
        .add_power_reader(ROCKET_HOOK, power_rocket)
        .add_power_reader(MINIGUN_HOOK, power_minigun)
        .add_power_reader(SNIPER_HOOK, power_sniper)
        .add_power_reader(GRENADE_HOOK, power_grenade)
        .add_power_reader(STICKY_HOOK, power_rocket)
        .add_power_reader(FLAME_HOOK, power_flame)
        .add_power_reader(NEEDLE_HOOK, power_rocket)
        .add_power_reader(FLARE_HOOK, power_grenade)
        .add_power_reader(shot::SHOT_HOOK, power_grenade);
}

use chimera_sim::{
    minigame::power::{GunStats, ReadAt},
    weapon::harm::{HookParams, num},
};

/// A gun's ramp k (`tf2:gun`), 1 without.
fn ramp_of(at: &ReadAt) -> f32 {
    damage::ramp_of(at.data.hooks.get(GUN_HOOK))
}

/// A pellet's damage at the distance: the distance modifier on it.
fn power_gun(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    s.damage *= damage::modifier(at.distance, damage::ramp_of(Some(p)));
}

/// The rocket's blast on the one it meets, at the distance.
fn power_rocket(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    s.damage = num(p, "damage").unwrap_or(0.0) * at.data.units.damage * damage::modifier(at.distance, ramp_of(at));
    s.pellets = 1;
}

/// The grenade's flat blast.
fn power_grenade(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    s.damage = num(p, "damage").unwrap_or(0.0) * at.data.units.damage;
    s.pellets = 1;
}

/// The rifle's uncharged body shot, flat (its charge waits on the
/// scope).
fn power_sniper(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    s.damage = num(p, "base").unwrap_or(0.0) * at.data.units.damage;
}

/// A flame hit at three quarters of its heat ramp, every hit interval
/// (the curve's distances are beyond its reach: it scores as if within
/// it, as the other games' flamethrowers do).
fn power_flame(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    s.damage = num(p, "damage").unwrap_or(0.0) * 0.75 * at.data.units.damage;
    s.pellets = 1;
    s.interval = num(p, "hit_interval").unwrap_or(s.interval);
}

/// The spin-up before the first shot (the wind-up and its step to the
/// shot).
fn power_minigun(p: &HookParams, _: &ReadAt, s: &mut GunStats) {
    s.latency += num(p, "wind_up").unwrap_or(0.0) + num(p, "fire_step").unwrap_or(0.0);
}

/// The server documents' name of TF2's weapon set (`server_docs`).
pub const SERVER_DOCS: &str = "tf2:weapons";

/// The projectile rules read from `set` (the hooks' parameters).
fn insert_rules(world: &mut World, set: &WeaponSet) {
    // The guns' looks, the unlocks' (their base's hooks) too.
    world.insert_resource(looks::GunLooks::from_set(set));
    world.insert_resource(rocket::RocketRules::from_set(set));
    world.insert_resource(grenade::GrenadeRules(rocket::RocketRules::of_hook(set, GRENADE_HOOK)));
    let stickies = sticky::StickyRules::from_set(set);
    stickies.draw_late(&mut world.get_resource_or_init::<chimera_world::map::loose::DrawnLate>());
    world.insert_resource(stickies);
    world.insert_resource(needle::NeedleRules::from_set(set));
    world.insert_resource(flare::FlareRules::from_set(set));
    world.insert_resource(shot::ShotRules::from_set(set));
    world.insert_resource(super::items::effects::ItemRules::from_set(set));
    world.insert_resource(super::items::consumables::ConsumableRules::from_set(set));
}

/// The rules read again when TF2's set changed (a server's documents put
/// in, or this machine's back).
fn refresh_rules(catalog: Res<chimera_sim::weapon::data::WeaponCatalog>, mut commands: Commands) {
    let Some(set) = catalog.sets.iter().rev().find(|s| s.get(SCATTERGUN).is_some()).copied() else {
        return;
    };
    commands.insert_resource(looks::GunLooks::from_set(set));
    commands.insert_resource(rocket::RocketRules::from_set(set));
    commands.insert_resource(grenade::GrenadeRules(rocket::RocketRules::of_hook(set, GRENADE_HOOK)));
    let stickies = sticky::StickyRules::from_set(set);
    commands.queue({
        let stickies = stickies.clone();
        move |world: &mut World| stickies.draw_late(&mut world.get_resource_or_init::<chimera_world::map::loose::DrawnLate>())
    });
    commands.insert_resource(stickies);
    commands.insert_resource(needle::NeedleRules::from_set(set));
    commands.insert_resource(flare::FlareRules::from_set(set));
    commands.insert_resource(shot::ShotRules::from_set(set));
    commands.insert_resource(super::items::effects::ItemRules::from_set(set));
    commands.insert_resource(super::items::consumables::ConsumableRules::from_set(set));
}

/// TF2's guns: their data, hooks and systems.
pub struct Tf2GunsPlugin;

impl Plugin for Tf2GunsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_sim::weapon::{CarriedAppExt, SelectWeapons, projectile::FlyerSet};
        use chimera_sim::server_docs::DocsAppExt;
        register_hooks(app);
        // The unlocks' hook (`tf2:item`) and their holders' rules.
        super::items::mods::add(app);
        // The numbers: the harness's built-in ones, else the scripts'
        // over the data (`script_numbers`), each gun's source logged.
        let numbers = app
            .world()
            .get_resource::<script_numbers::GunNumbers>()
            .cloned()
            .unwrap_or_else(script_numbers::GunNumbers::from_scripts);
        // Said once the app runs: a dedicated server's log is set up after
        // its plugins are built (`chimera_net::dedicated::run`).
        let said = numbers.clone();
        app.add_systems(Startup, move || said.log());
        let docs: Vec<(&str, &str)> = numbers.docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let set = app.add_weapon_data(&docs).unwrap_or_else(|e| panic!("TF2 weapon data: {e}"));
        // A client whose own differ (no scripts, the fallback) plays with
        // the server's.
        app.server_weapon_docs(SERVER_DOCS, set, numbers.docs.clone());
        // The unlocks' classes (`items`), and their holders' rules.
        {
            let mut classes = app.world_mut().get_resource_or_init::<super::classes::Classes>();
            for item in &numbers.items {
                classes.add_gun(&item.id, &item.classes, &item.base);
            }
        }
        app.insert_resource(numbers);
        insert_rules(app.world_mut(), set);
        app.add_systems(
            PreUpdate,
            refresh_rules.run_if(resource_changed::<chimera_sim::weapon::data::WeaponCatalog>),
        )
        .add_message::<crate::map::GameSound>()
            .add_message::<crate::map::PredictedSound>()
            .add_message::<blast::Tf2Explosion>()
            .add_message::<airblast::Airblast>()
            .add_message::<airblast::AirblastLook>()
            .add_message::<chimera_sim::weapon::grenade::BlastDealt>()
            .add_systems(
                Predict::Weapons,
                (
                    (damage::centre_first, recoil::decay, melee::prepare, (flame::blocked, airblast::fire).chain()).before(WeaponFrame),
                    (damage::note_shots, flame::ammo, flame::sounds, recoil::kick, melee::after).chain().after(WeaponFrame),
                ),
            )
            .add_systems(Predict::Select, (minigun::spin, sticky::charge, charge::charge, sniper::scope).before(SelectWeapons))
            // An unlock's switch times (`items::mods::holster`, 4.9).
            .add_systems(Predict::Select, super::items::mods::holster.after(SelectWeapons))
            // Every Minigun's barrel before the state step and its sound
            // after the tick, once per tick (not in replays).
            .add_message::<crate::map::PlaySound>()
            .add_message::<crate::map::StopSound>()
            .add_message::<crate::map::FadeSound>()
            .add_systems(
                FixedUpdate,
                (
                    minigun::barrels.before(SelectWeapons),
                    minigun::sounds.after(chimera_sim::weapon::WeaponFrame).after(crate::core::SimSet::Weapons),
                ),
            )
            .add_systems(
                FixedUpdate,
                (
                    sticky::launch.in_set(FlyerSet::Launch),
                    shot::throw.in_set(crate::core::SimSet::Weapons).after(chimera_sim::weapon::WeaponFrame).before(FlyerSet::Launch),
                    airblast::deflect.in_set(FlyerSet::Think),
                    (
                        blast::blast_jump_lands,
                        rocket::explode,
                        grenade::touch,
                        grenade::fuse,
                        sticky::touch,
                        needle::touch,
                        flare::touch,
                        flare::detonate,
                        shot::touch,
                        sticky::detonate,
                        flame::flames,
                        flame::afterburn,
                        blast::bullet_push,
                        knock::knockback,
                        knock::shove,
                        knock::caber,
                    )
                        .chain()
                        .in_set(FlyerSet::Touch),
                )
                    .run_if(crate::core::authoritative),
            )
            .predicted_part::<damage::Tf2Gun>()
            .predicted_part::<minigun::Tf2Minigun>()
            .predicted_part::<sniper::Tf2Sniper>()
            .predicted_part::<sticky::Tf2Sticky>()
            .predicted_part::<flame::Tf2Flame>()
            .predicted_part::<airblast::Tf2Airblast>()
            .predicted_part::<melee::Tf2Melee>()
            .predicted_part::<charge::Tf2Charge>()
            .init_resource::<shot::Rehits>()
            .init_resource::<shot::Spent>()
            .init_resource::<melee::MeleeCrits>()
            .add_systems(Update, melee::backstab_ready);
        looks::add(app);
        crit_boost::add(app);
        crit_hits::add(app);
        app.add_systems(
            FixedUpdate,
            sniper::pierce
                .after(crate::core::SimSet::Weapons)
                .before(super::items::effects::crits)
                .run_if(crate::core::authoritative),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file in data/weapons is built in, and the twelve guns load
    /// with their classes for the automatic ladder.
    #[test]
    fn the_data_is_built_in_and_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weapons");
        let files = WeaponSet::read_dir(&dir).unwrap();
        for (name, text) in &files {
            let built_in = DATA.iter().find(|(n, _)| n == name);
            assert_eq!(built_in.map(|d| d.1.text()), Some(text.as_str()), "{name} not in `DATA`");
        }
        assert_eq!(files.len(), DATA.len());
        let set = tf2_weapons();
        let ids: Vec<&str> = set.weapons.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, all_weapons().collect::<Vec<_>>());
        use chimera_core::game_parts::GunClass;
        let class = |id: &str| set.weapons.iter().find(|w| w.id == id).and_then(|w| w.class);
        assert_eq!(class(SCATTERGUN), Some(GunClass::Shotgun));
        assert_eq!(class(SHOTGUN), Some(GunClass::Shotgun));
        assert_eq!(class(ROCKET_LAUNCHER), Some(GunClass::Launcher));
        assert_eq!(class(MINIGUN), Some(GunClass::Heavy));
        assert_eq!(class(SNIPER_RIFLE), Some(GunClass::Sniper));
        assert_eq!(class(REVOLVER), Some(GunClass::HeavyPistol));
        assert_eq!(class(FLAMETHROWER), Some(GunClass::Flamethrower));
        assert_eq!(class(STICKY_LAUNCHER), Some(GunClass::Launcher));
        assert!(MELEE.iter().all(|m| class(m).is_none()), "melee weapons are no ladder guns");
        let arms: Vec<String> = arms_by_gun().into_iter().map(|(_, a)| a).collect();
        assert_eq!(
            arms,
            [
                "scout", "soldier", "engineer", "heavy", "scout", "sniper", "sniper", "spy", "demo", "demo", "pyro", "medic", "scout",
                "soldier", "pyro", "demo", "heavy", "engineer", "medic", "sniper", "spy"
            ]
        );
    }
}
