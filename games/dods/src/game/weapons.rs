//! DoD:S's guns as data (data/weapons; docs/weapon-data.md). M0 has one
//! stand-in, the Colt (`COLT`: the script's damage, clip and fire delay
//! from specs/dods/weapons.md, semi-automatic, the head and limb
//! multipliers of damage.md), so a DoD:S body can be worn in the party
//! apps, where a body follows the gun in hand's game. Its view model is
//! the install's `v_colt.mdl` (arms and gun in one model), its held model
//! `w_colt.mdl` on the default body's skeleton (`add_guns`).
//!
//! Known shortcuts (M0; M1 replaces them): no sounds, recoil, movement
//! penalty, view-model animation rules or numbers read from the install's
//! scripts; the reload time is a stand-in.

use bevy::prelude::*;
use chimera_sim::weapon::data::{WeaponDataAppExt, WeaponSet};
use source_engine::content::{material::MaterialLoader, props, surfaceprops::SurfaceProps};

use crate::map::GameKit;

macro_rules! data_file {
    ($name:literal) => {
        ($name, chimera_core::game_data!("data/weapons/", $name))
    };
}

/// The weapon documents, built in, in registration order.
pub const DATA: &[(&str, chimera_core::game_data::GameData)] = &[data_file!("_base.toml"), data_file!("colt.toml")];

/// The Colt M1911.
pub const COLT: &str = "dods:weapon_colt";

/// Every gun of DoD:S's data.
pub const GUNS: &[&str] = &[COLT];

/// DoD:S's weapons as data, parsed (a fresh copy each call).
pub fn dods_weapons() -> WeaponSet {
    WeaponSet::parse(&chimera_core::game_data::texts(DATA), &[]).unwrap_or_else(|e| panic!("DoD:S weapon data: {e}"))
}

/// Every gun's display name by id (its data's `hud.name`).
pub fn display_names(_: &crate::mount::Mount) -> Vec<(String, String)> {
    dods_weapons().weapons.into_iter().filter_map(|w| Some((w.id, w.hud.name?))).collect()
}

/// DoD:S's guns that `wants`, into `into`: their view models (right-handed,
/// never mirrored) and their world models held on the default body's
/// skeleton.
pub fn add_guns(
    materials: &mut MaterialLoader,
    surfaces: &SurfaceProps,
    wants: &dyn Fn(&str) -> bool,
    into: &mut GameKit,
    warnings: &mut Vec<String>,
) {
    let set = dods_weapons();
    let guns: Vec<_> = set.weapons.iter().filter(|w| wants(&w.id)).collect();
    if guns.is_empty() {
        return;
    }
    let skeleton = match into.characters.iter().find(|c| c.game == Some(super::GAME) && c.name.is_none()) {
        Some(c) => Some(c.bones.clone()),
        None => super::bodies::load(materials, surfaces, super::bodies::DEFAULT_MODEL).ok().map(|c| c.bones),
    };
    for w in guns {
        if let Some(view) = &w.models.view {
            match props::load_view_model(materials, view, &w.id, true) {
                Ok(mut v) => {
                    v.allow_flipping = false;
                    into.view_models.push(v);
                }
                Err(e) => warnings.push(e),
            }
        }
        if let (Some(world), Some(skeleton)) = (&w.models.world, &skeleton) {
            match props::load_held(materials, world, &w.id, skeleton) {
                Ok(h) => into.held.push(h),
                Err(e) => warnings.push(e),
            }
        }
    }
}

/// DoD:S's guns in an app's weapon registry.
pub struct DodsGunsPlugin;

impl Plugin for DodsGunsPlugin {
    fn build(&self, app: &mut App) {
        if let Err(e) = app.add_weapon_data(&chimera_core::game_data::texts(DATA)) {
            panic!("DoD:S weapon data: {e}");
        }
    }
}
