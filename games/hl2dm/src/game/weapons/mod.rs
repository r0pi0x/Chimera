//! HL2:DM's weapons as data (docs/weapon-data.md): one TOML file per
//! weapon in games/hl2dm/data/weapons (built in, parsed at run time:
//! `hl2_weapons`) with specs/hl2dm/weapons.md's numbers (H3): the
//! pistol, .357, SMG1 and AR2 (their attack2s, the grenade and the energy
//! ball, are M5's), the shotgun (single and double shot), the crowbar and
//! the stunstick. The install's own scripts (`scripts/weapon_*.txt`,
//! plain text) are read by `game::scripts` and checked against the data.
//!
//! The shared frame (`chimera_sim::weapon`) runs them with HL2:DM's parts:
//! no penetration or falloff, the script's damage against everything,
//! head x3, the cone spread (`SpreadShape::Cone`), the shot along view +
//! punch, holding attack on an empty clip reloads, the shotgun's double
//! shot (`AltShot`) and shells in as their insert starts. What data can't
//! say is the hooks it names (`register_hooks`):
//!
//! - `hl2dm:inventory` (`inventory::Hl2Item`): the script's facts, and the
//!   holstered auto-reload (`inventory::Hl2Holster`);
//! - `hl2dm:pistol` (`pistol::Hl2Pistol`): the accuracy ramp, the click
//!   refire, attack2's block, the dry fire's wait;
//! - `hl2dm:punch`, `hl2dm:machine_gun` (`recoil`): the kicks on HL2's
//!   punch spring;
//! - `hl2dm:shotgun` (`shotgun::Hl2Shotgun`): the pump.
//!
//! `Hl2GunsPlugin` is the weapons alone (data, hooks, recoil, the damage
//! shove of HL2:DM players: `shove`; what a game of several games adds);
//! `Hl2WeaponsPlugin` is HL2:DM's game around them: the spawn loadout,
//! ammo per player, the next best weapon, pickup and drop rules
//! (`inventory`).
//!
//! Known shortcuts: no weapon refuses to fire under water (Chimera's
//! characters don't tell how deep they are yet; the movement is M3's); the
//! odd pellets of a multi-pellet shot are lines, not the spec's 6-unit
//! boxes; the melee box's re-trace to twice its hit and its corners isn't
//! done (the box's own hit is used); another client draws a double shot's
//! seven pellets, not twelve; lag compensation rewinds every player, not
//! HL2:DM's choice by distance and view (weapons.md 12); the tracer
//! counter, impacts and sounds are M10's.

pub mod inventory;
pub mod pistol;
pub mod recoil;
pub mod shotgun;
pub mod shove;

use bevy::prelude::*;
use chimera_sim::{
    minigame::power::{GunStats, HookParams, PowerAppExt, ReadAt},
    weapon::{
        CarriedAppExt, Magazine, StartingWeapons, WeaponFrame,
        data::{WeaponDataAppExt, WeaponSet},
        drop::{DeathDrops, PickupRules},
    },
};

use crate::core::{Predict, PredictedAppExt, SimSet};

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
    data_file!("crowbar.toml"),
    data_file!("stunstick.toml"),
    data_file!("pistol.toml"),
    data_file!("357.toml"),
    data_file!("smg1.toml"),
    data_file!("ar2.toml"),
    data_file!("shotgun.toml"),
];

pub const CROWBAR: &str = "hl2dm:weapon_crowbar";
pub const STUNSTICK: &str = "hl2dm:weapon_stunstick";
pub const PISTOL: &str = "hl2dm:weapon_pistol";
pub const MAGNUM: &str = "hl2dm:weapon_357";
pub const SMG1: &str = "hl2dm:weapon_smg1";
pub const AR2: &str = "hl2dm:weapon_ar2";
pub const SHOTGUN: &str = "hl2dm:weapon_shotgun";

/// The hooks the data names.
pub const INVENTORY: &str = "hl2dm:inventory";
pub const PISTOL_HOOK: &str = "hl2dm:pistol";
pub const PUNCH: &str = "hl2dm:punch";
pub const MACHINE_GUN: &str = "hl2dm:machine_gun";
pub const SHOTGUN_HOOK: &str = "hl2dm:shotgun";

/// The weapons every player spawns with besides the melee weapon of its
/// model (`inventory::arm`), in giving order.
pub const STARTING: &[&str] = &[PISTOL, SMG1];

/// HL2:DM's weapons as data, parsed (a fresh copy each call: a map's
/// kit and the tests read it; an app's `WeaponCatalog` holds its own).
pub fn hl2_weapons() -> WeaponSet {
    WeaponSet::parse(DATA, &[]).unwrap_or_else(|e| panic!("HL2:DM weapon data: {e}"))
}

/// World models (`playermodel`) by weapon.
pub fn world_models() -> Vec<(String, String)> {
    hl2_weapons()
        .weapons
        .into_iter()
        .filter_map(|w| Some((w.id, w.models.world?)))
        .collect()
}

/// View models (`viewmodel`) by weapon: HL2's are right-handed and never
/// mirrored (specs/hl2dm/player_models.md 12.1).
pub fn view_models() -> Vec<(String, String)> {
    hl2_weapons()
        .weapons
        .into_iter()
        .filter_map(|w| Some((w.id, w.models.view?)))
        .collect()
}

/// Every HL2:DM weapon's display name by weapon id, from the install's
/// strings (`resource/hl2mp_english.txt`): the text of its data's
/// `hud.name` token (`#HL2_Pistol`: "9MM PISTOL"), its line breaks as
/// spaces. Empty without the file.
pub fn display_names(mount: &crate::mount::Mount) -> Vec<(String, String)> {
    use source_formats::localization::{decode, localization};
    let mut strings = std::collections::BTreeMap::new();
    for bytes in mount.read_every(&crate::mount::normalize("resource/hl2mp_english.txt")) {
        for (k, v) in localization(&decode(&bytes)) {
            strings.entry(k).or_insert(v);
        }
    }
    hl2_weapons()
        .weapons
        .into_iter()
        .filter_map(|w| {
            let token = w.hud.name.as_deref()?.trim_start_matches('#').to_lowercase();
            let text = strings.get(&token)?;
            // Line breaks (the AR2's and SMG1's two-line names) as spaces.
            let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty()).then_some((w.id, text))
        })
        .collect()
}

/// A shot's spread for the gun game's power curve: the pistol's first
/// shot's cone and its follow-ups' after a quarter of the ramp
/// (`ReadAt::firing`).
fn power_pistol(p: &HookParams, at: &ReadAt, s: &mut GunStats) {
    let Ok(p) = p.clone().try_into::<pistol::PistolParams>() else { return };
    s.spread = p.spread[0];
    s.follow = Some(p.spread[0] + (p.spread[1] - p.spread[0]) * at.firing);
}

/// Register the behaviour HL2:DM's weapon data names.
pub fn register_hooks(app: &mut App) {
    app.add_power_reader(PISTOL_HOOK, power_pistol);
    app.register_weapon_hook::<inventory::InventoryParams>(INVENTORY, |e, d, p| {
        let item = inventory::Hl2Item {
            position: p.position,
            default_clip: p.default_clip,
            autoswitchto: p.autoswitchto,
            autoswitchfrom: p.autoswitchfrom,
            ammo: d.magazine.as_ref().map(|m| m.ammo.as_str()),
            weight: d.weight.unwrap_or(0),
        };
        e.insert(item);
        if e.contains::<Magazine>() {
            e.insert(inventory::Hl2Holster::default());
        }
    })
    .register_weapon_hook::<pistol::PistolParams>(PISTOL_HOOK, |e, _, p| {
        e.insert(pistol::Hl2Pistol::new(*p));
    })
    .register_weapon_hook::<recoil::Hl2Kick>(PUNCH, |e, _, p| {
        e.insert(*p);
    })
    .register_weapon_hook::<recoil::Hl2MachineGun>(MACHINE_GUN, |e, _, p| {
        e.insert(*p);
    })
    .register_weapon_hook::<shotgun::ShotgunParams>(SHOTGUN_HOOK, |e, _, p| {
        e.insert(shotgun::Hl2Shotgun {
            params: p.clone(),
            pump_pending: false,
            reload_after_pump: false,
        });
    });
}

/// HL2:DM's weapons alone: the data, its hooks, the recoil and the pump
/// around the shared frame, the holstered auto-reload and the damage
/// shove of HL2:DM players. What an app of several games adds.
pub struct Hl2GunsPlugin;

impl Plugin for Hl2GunsPlugin {
    fn build(&self, app: &mut App) {
        register_hooks(app);
        if let Err(e) = app.add_weapon_data(DATA) {
            panic!("HL2:DM weapon data: {e}");
        }
        // A rung's player model and melee weapon (`rungs`).
        app.add_plugins(super::rungs::Hl2RungsPlugin);
        app.add_systems(
            Predict::Weapons,
            (
                (recoil::decay_punch, pistol::before_frame, shotgun::before_frame).before(WeaponFrame),
                (recoil::kick, pistol::after_frame, shotgun::after_frame, inventory::holster_reload).after(WeaponFrame),
            ),
        )
        .add_systems(
            FixedUpdate,
            shove::shove_players
                .after(SimSet::Weapons)
                .before(crate::core::apply_damage)
                .run_if(crate::core::authoritative),
        )
        .predicted_net::<recoil::Hl2Punch>()
        .predicted_part::<pistol::Hl2Pistol>()
        .predicted_part::<shotgun::Hl2Shotgun>()
        .predicted_part::<inventory::Hl2Holster>();
    }
}

/// HL2:DM's weapons as its game plays them (`Hl2GunsPlugin` and
/// `inventory`'s rules): the starting weapons (`STARTING`, the melee
/// weapon by model, the reserves), ammo per player, the next best
/// weapon, HL2:DM's pickup and drop rules.
pub struct Hl2WeaponsPlugin;

impl Plugin for Hl2WeaponsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(Hl2GunsPlugin)
            .insert_resource(StartingWeapons {
                team: Vec::new(),
                all: STARTING.to_vec(),
            })
            .insert_resource(DeathDrops(true))
            .insert_resource(PickupRules {
                touch_delay: 1.0,
                one_per_slot: false,
                owner_pickup_removes: true,
                death_drop_held: true,
                loose_life: Some(31.0),
            })
            .add_systems(
                FixedUpdate,
                (
                    (inventory::arm, inventory::loose_ammo, inventory::on_pickup)
                        .after(SimSet::Weapons)
                        .run_if(crate::core::authoritative),
                    inventory::disarm.run_if(crate::core::authoritative),
                ),
            )
            .add_systems(Predict::Weapons, inventory::switch_from_empty.after(WeaponFrame));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file in data/weapons is built in, and the data loads with
    /// HL2:DM's hooks.
    #[test]
    fn the_data_is_built_in_and_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weapons");
        let files = WeaponSet::read_dir(&dir).unwrap();
        for (name, text) in &files {
            let built_in = DATA.iter().find(|(n, _)| n == name);
            assert_eq!(built_in.map(|d| d.1), Some(text.as_str()), "{name} not in `DATA`");
        }
        assert_eq!(files.len(), DATA.len());
        let mut app = App::new();
        register_hooks(&mut app);
        app.add_weapon_data(DATA).unwrap();
        assert_eq!(hl2_weapons().weapons.len(), 7);
        assert_eq!(view_models().len(), 7);
    }
}
