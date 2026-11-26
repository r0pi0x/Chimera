//! Counter-Strike 1.6's weapons as data (data/weapons; docs/weapon-data.md;
//! specs/cs16/weapons.md, damage.md): the 24 guns, the knife and the three
//! grenades, one document each, in the spec's numbers (`DATA`), with the
//! game's rules as hooks: `cs16:gun` (`gun`: accuracy, spread by state,
//! kick, the modes' damage, bursts, GoldSrc's penetration with
//! data/penetration.toml, the shotguns' pellets) and `cs16:grenade`
//! (`grenades`). The knife is plain data (Chimera's melee: the slash, the
//! stab and its backstab by the attacker's level view, `Backstab::Aim`).
//! Their models are the install's `v_*.mdl` (view, with the hands),
//! `p_*.mdl` (held, on the player skeleton by bone name) and `w_*.mdl`
//! (dropped and thrown: the held model's loose form, `map::loose::
//! loose_key`), loaded with a map (`add_guns`:
//! `goldsrc_engine::content::weapon_models`, with the muzzle flashes their
//! events name), what the view models play by `view` (`view::rules`),
//! their sounds GoldSrc's waves by path
//! (`goldsrc_engine::content::sounds`: the data's entries, the view
//! models' sound events, the melee swings' and the grenades'); the HE's
//! and smoke's looks from the install's sprites (`looks`) and each shot's
//! spent case (`shells`: data/shells.toml, the cases' models from the
//! install).
//!
//! Each gun's speed is its data's `max_speed` (units/s; a scope's zoomed
//! speed its `[alt] max_speed`), what the movement reads.
//!
//! Known shortcuts (beside `gun`'s, `grenades`' and `view`'s):
//! - The cases' flight and the late shells' origin: `shells`' own.
//! - The bodies' sequence set per weapon (`[hooks."cs16:body_anim"]`,
//!   `bodies::BodyAnim`) is ours (`bodies`).

use bevy::prelude::*;
use chimera_sim::weapon::data::{WeaponData, WeaponDataAppExt, WeaponSet};
use goldsrc_engine::content::{sounds, weapon_models};

use crate::{
    map::{MapData, MapModel},
    mount::Mount,
};

pub mod grenades;
pub mod gun;
pub mod looks;
pub mod penetration;
pub mod shells;
pub mod view;

macro_rules! data_file {
    ($name:literal) => {
        ($name, chimera_core::game_data!("data/weapons/", $name))
    };
}

/// The weapon documents, built in, in registration order (the knife, the
/// guns in the spec's order, the grenades).
pub const DATA: &[(&str, chimera_core::game_data::GameData)] = &[
    data_file!("_base.toml"),
    data_file!("_gun.toml"),
    data_file!("ammo.toml"),
    data_file!("knife.toml"),
    data_file!("glock18.toml"),
    data_file!("usp.toml"),
    data_file!("p228.toml"),
    data_file!("deagle.toml"),
    data_file!("elite.toml"),
    data_file!("fiveseven.toml"),
    data_file!("m3.toml"),
    data_file!("xm1014.toml"),
    data_file!("mac10.toml"),
    data_file!("tmp.toml"),
    data_file!("mp5navy.toml"),
    data_file!("ump45.toml"),
    data_file!("p90.toml"),
    data_file!("galil.toml"),
    data_file!("famas.toml"),
    data_file!("ak47.toml"),
    data_file!("m4a1.toml"),
    data_file!("sg552.toml"),
    data_file!("aug.toml"),
    data_file!("scout.toml"),
    data_file!("awp.toml"),
    data_file!("g3sg1.toml"),
    data_file!("sg550.toml"),
    data_file!("m249.toml"),
    data_file!("hegrenade.toml"),
    data_file!("flashbang.toml"),
    data_file!("smokegrenade.toml"),
];

pub const KNIFE: &str = "cs16:weapon_knife";
pub const GLOCK: &str = "cs16:weapon_glock18";
pub const USP: &str = "cs16:weapon_usp";
pub const P228: &str = "cs16:weapon_p228";
pub const DEAGLE: &str = "cs16:weapon_deagle";
pub const ELITE: &str = "cs16:weapon_elite";
pub const FIVESEVEN: &str = "cs16:weapon_fiveseven";
pub const M3: &str = "cs16:weapon_m3";
pub const XM1014: &str = "cs16:weapon_xm1014";
pub const MAC10: &str = "cs16:weapon_mac10";
pub const TMP: &str = "cs16:weapon_tmp";
pub const MP5: &str = "cs16:weapon_mp5navy";
pub const UMP45: &str = "cs16:weapon_ump45";
pub const P90: &str = "cs16:weapon_p90";
pub const GALIL: &str = "cs16:weapon_galil";
pub const FAMAS: &str = "cs16:weapon_famas";
pub const AK47: &str = "cs16:weapon_ak47";
pub const M4A1: &str = "cs16:weapon_m4a1";
pub const SG552: &str = "cs16:weapon_sg552";
pub const AUG: &str = "cs16:weapon_aug";
pub const SCOUT: &str = "cs16:weapon_scout";
pub const AWP: &str = "cs16:weapon_awp";
pub const G3SG1: &str = "cs16:weapon_g3sg1";
pub const SG550: &str = "cs16:weapon_sg550";
pub const M249: &str = "cs16:weapon_m249";
pub const HEGRENADE: &str = "cs16:weapon_hegrenade";
pub const FLASHBANG: &str = "cs16:weapon_flashbang";
pub const SMOKEGRENADE: &str = "cs16:weapon_smokegrenade";

/// Every gun (bullets), in the spec's order.
pub const GUNS: &[&str] = &[
    GLOCK, USP, P228, DEAGLE, ELITE, FIVESEVEN, M3, XM1014, MAC10, TMP, MP5, UMP45, P90, GALIL, FAMAS, AK47, M4A1, SG552, AUG,
    SCOUT, AWP, G3SG1, SG550, M249,
];

/// The grenades.
pub const GRENADES: &[&str] = &[HEGRENADE, FLASHBANG, SMOKEGRENADE];

/// The game's weapons as data, parsed (a fresh copy each call: no state
/// outlives an app).
pub fn weapons() -> WeaponSet {
    WeaponSet::parse(&chimera_core::game_data::texts(DATA), &[]).unwrap_or_else(|e| panic!("Counter-Strike 1.6 weapon data: {e}"))
}

/// A weapon's data by id.
pub fn data(id: &str) -> Option<WeaponData> {
    weapons().weapons.into_iter().find(|w| w.id == id)
}

/// Every weapon's display name by id (its data's `hud.name`).
pub fn display_names(_: &Mount) -> Vec<(String, String)> {
    weapons().weapons.into_iter().filter_map(|w| Some((w.id, w.hud.name?))).collect()
}

/// Every sound entry a weapon's data names (fire, empty, the swings', a
/// grenade's bounce, blast and flash): `goldsrc_engine::content::sounds`
/// names, waves by path.
pub fn data_sounds(w: &WeaponData) -> Vec<String> {
    let mut out: Vec<String> = w.sounds.iter().flat_map(|s| s.entries()).map(str::to_string).collect();
    for s in w.melee.iter().flat_map(|m| [m.primary.as_ref(), m.secondary.as_ref()]).flatten() {
        out.extend([&s.sound_hit, &s.sound_hit_world, &s.sound_miss, &s.sound_swing].into_iter().flatten().cloned());
    }
    if let Some(p) = w.hook::<grenades::GrenadeParams>(grenades::HOOK) {
        out.push(p.bounce_sound.clone());
        out.extend(grenades::effect_sounds(p.effect).iter().map(|s| s.to_string()));
    }
    if let Some(chimera_sim::weapon::data::AltData::Scope { sound: Some(s), .. }) = &w.alt {
        out.push(s.clone());
    }
    out
}

/// The game's guns that `wants`, into `map`'s kit (module doc): their
/// view models, their held models on the player skeleton (`skeleton`: its
/// bone names), their textures appended to the map's, and every sound
/// they play into its sound table. `decode` off: the textures' sizes
/// only.
pub fn add_guns(mount: &Mount, wants: &dyn Fn(&str) -> bool, skeleton: &[&str], decode: bool, map: &mut MapData) {
    let mut entries: Vec<String> = Vec::new();
    let mut flashes: Vec<String> = Vec::new();
    for w in weapons().weapons.iter().filter(|w| wants(&w.id)) {
        if let Some(view) = &w.models.view {
            match weapon_models::view_model(mount, view, &w.id, decode) {
                Ok((mut v, textures, waves)) => {
                    for code in v.animations.as_deref().map(weapon_models::flash_codes).unwrap_or_default() {
                        if !flashes.contains(&code) {
                            flashes.push(code);
                        }
                    }
                    offset(&mut v.model, map.textures.len());
                    map.textures.extend(textures);
                    map.kit.view_models.push(v);
                    entries.extend(waves);
                }
                Err(e) => map.warnings.push(e),
            }
        }
        if let Some(world) = &w.models.world {
            match weapon_models::held(mount, world, &w.id, skeleton, decode) {
                Ok((mut h, textures)) => {
                    let base = map.textures.len();
                    offset(&mut h.model, base);
                    if let Some(own) = h.own.as_mut() {
                        offset(own, base);
                    }
                    for (_, m) in &mut h.merged {
                        offset(m, base);
                    }
                    map.textures.extend(textures);
                    map.kit.held.push(h);
                }
                Err(e) => map.warnings.push(e),
            }
        }
        // Its dropped model, the held one's loose form (`dropped_model`).
        if let Some(path) = dropped_model(w).filter(|p| mount.contains(p)) {
            match weapon_models::dropped(mount, &path, &w.id, decode) {
                Ok((mut h, textures)) => {
                    offset(&mut h.model, map.textures.len());
                    map.textures.extend(textures);
                    map.kit.held.push(h);
                }
                Err(e) => map.warnings.push(e),
            }
        }
        // The sprites its look draws (`looks`).
        for sprite in looks::sprites_of(&w.id) {
            goldsrc_engine::content::sprites::particle_material(mount, sprite, decode, map);
        }
        entries.extend(data_sounds(w));
    }
    // The muzzle flashes their view models' events name (`view::effect`).
    for code in flashes {
        let kind = weapon_models::flash_kind(super::GAME, &code);
        if map.kit.muzzle_flash_kinds.iter().any(|(k, _)| *k == kind) {
            continue;
        }
        if let Some((mut flash, texture)) = weapon_models::muzzle_flash(mount, &code, decode) {
            flash.texture = Some(map.textures.len());
            map.textures.push(texture);
            map.kit.muzzle_flash_kinds.push((kind, flash));
        }
    }
    // The spent cases of the guns wanted (`shells`).
    let mut types = Vec::new();
    shells::kit(
        wants,
        |model| match weapon_models::shell(mount, &format!("models/{model}.mdl"), decode) {
            Ok((mut m, textures)) => {
                offset(&mut m, map.textures.len());
                map.textures.extend(textures);
                Some(m)
            }
            Err(e) => {
                map.warnings.push(e);
                None
            }
        },
        &mut entries,
        &mut types,
    );
    map.kit.shells.extend(types);
    let table = std::sync::Arc::make_mut(&mut map.sounds);
    for e in entries {
        if !sounds::add_entry(mount, &e, table) {
            map.warnings.push(format!("sound {e}: not in the install"));
        }
    }
}

/// A model's meshes' texture indices moved past `base` (their textures
/// appended to a list of that length).
fn offset(model: &mut MapModel, base: usize) {
    for m in &mut model.meshes {
        for t in [m.texture.as_mut(), m.layer.as_mut().and_then(|l| l.texture.as_mut())].into_iter().flatten() {
            *t += base;
        }
    }
}

/// A weapon's dropped model (`models/w_<name>.mdl` beside its held
/// `p_<name>.mdl`).
pub fn dropped_model(w: &WeaponData) -> Option<String> {
    let held = w.models.world.as_deref()?;
    let (dir, file) = held.rsplit_once('/')?;
    Some(format!("{dir}/w_{}", file.strip_prefix("p_")?))
}

/// The player skeleton's bone names for the held models (the default
/// body's, `bodies::CLASS_MODELS`' first; any CS 1.6 body has the same
/// bones).
pub fn skeleton_names(map: &MapData) -> Vec<String> {
    map.kit
        .characters
        .iter()
        .find(|c| c.game == Some(super::GAME))
        .map(|c| c.bones.iter().map(|b| b.name.clone()).collect())
        .unwrap_or_else(|| SKELETON.iter().map(|s| s.to_string()).collect())
}

/// The hands (and what leads to them) of a CS 1.6 player skeleton, for a
/// kit without the bodies.
pub const SKELETON: &[&str] = &[
    "Bip01",
    "Bip01 Pelvis",
    "Bip01 Spine",
    "Bip01 Spine1",
    "Bip01 Spine2",
    "Bip01 Spine3",
    "Bip01 Neck",
    "Bip01 L Clavicle",
    "Bip01 L UpperArm",
    "Bip01 L Forearm",
    "Bip01 L Hand",
    "Bip01 R Clavicle",
    "Bip01 R UpperArm",
    "Bip01 R Forearm",
    "Bip01 R Hand",
];

/// The hooks the data names.
pub fn register_hooks(app: &mut App) {
    app.register_weapon_hook::<gun::GunParams>(gun::HOOK, |e, _, p| gun::insert(e, p))
        .register_weapon_hook::<grenades::GrenadeParams>(grenades::HOOK, grenades::throwable)
        .register_weapon_hook::<super::bodies::BodyAnim>(super::bodies::BODY_ANIM_HOOK, |e, _, p| {
            e.insert(p.clone());
        });
}

/// The game's weapons in an app's weapon registry: the data, its hooks,
/// the guns' rules around the weapon frame (predicted) and the view
/// models' rules.
pub struct Cs16GunsPlugin;

impl Plugin for Cs16GunsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_sim::weapon::{CarriedAppExt, WeaponFrame, view_anim::ViewAnimAppExt};
        register_hooks(app);
        if !app.is_plugin_added::<chimera_sim::weapon::view_anim::ViewAnimPlugin>() {
            app.add_plugins(chimera_sim::weapon::view_anim::ViewAnimPlugin);
        }
        app.insert_resource(penetration::Cs16Penetration(penetration::rules()));
        if let Err(e) = app.add_weapon_data(&chimera_core::game_data::texts(DATA)) {
            panic!("Counter-Strike 1.6 weapon data: {e}");
        }
        app.add_plugins(looks::Cs16GrenadeLooksPlugin);
        shells::add(app);
        app.add_message::<chimera_sim::weapon::WeaponEvent>()
            .predicted_part::<gun::Cs16Gun>()
            .view_anim_rules(super::GAME, view::rules())
            .add_systems(
                crate::core::Predict::Weapons,
                (gun::before_frame.before(WeaponFrame), gun::after_frame.after(WeaponFrame)),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every file in data/weapons is built in, and the data loads with the
    /// game's hooks.
    #[test]
    fn the_data_is_built_in_and_loads() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("data/weapons");
        let files = WeaponSet::read_dir(&dir).unwrap();
        for (name, text) in &files {
            let built_in = DATA.iter().find(|(n, _)| n == name);
            assert_eq!(built_in.map(|d| d.1.text()), Some(text.as_str()), "{name} not in `DATA`");
        }
        assert_eq!(files.len(), DATA.len());
        let mut app = App::new();
        register_hooks(&mut app);
        app.add_weapon_data(&chimera_core::game_data::texts(DATA)).unwrap();
        assert_eq!(weapons().weapons.len(), 28);
        for id in GUNS.iter().chain(GRENADES).chain([&KNIFE]) {
            assert!(data(id).is_some(), "{id}");
        }
    }
}
