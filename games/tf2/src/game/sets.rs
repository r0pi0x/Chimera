//! What TF2's asset sets hold (docs/plans/active/tf2.md, section 3,
//! decision 4), for `chimera_pack sets tf2` and the pack test: per gun
//! (`M2_WEAPONS`) its `weapon/<id>` set (`<id>` the gun's id past `tf2:`,
//! as a match's offer names a gun's set: `weapon/tf_weapon_scattergun`),
//! read by loading the gun as a match does (`maps::add_guns`: its view
//! model, the class's arms with their animations include and the `c_`
//! weapon model bone-merged, the held model, the rocket's model) through
//! the mount and noting every file read (`Mount::track`), with the waves
//! of its sound entries (specs/tf2/weapons.md 8) and its script
//! (`scripts/tf_weapon_<id>.ctx`, packed decrypted: `scripts::decrypting`);
//! per class `players/<class>` (`classes::set_of`, `class_files`): its
//! body as a match loads it (`bodies::load`: the player model, its
//! animation includes, its materials and textures), the model as a view
//! model reads it, and its first-person arms (`c_<arms>_arms.mdl` with
//! their animations), less what every class reads; `players` (offered
//! with every TF2 match, `resolve::MATCH_SETS`) what every class reads,
//! the HUD's look (`hud::load`, the class portraits too), the kill feed's
//! icons (`hud::kill_icon_pictures`: `scripts/mod_textures.txt` and its
//! sheets) and the guns' looks on any map (`maps::add_guest_looks`: the
//! particle files and their materials, the impact decals and effect
//! materials) with the surfaces' bullet-impact sounds. The match's offer names the class sets
//! its TF2 guns can make (`bodies::asset_names`, with the classes they
//! need: data/classes.toml's `needs`).
//!
//! Known shortcuts:
//! - The sound entries are the spec's lists (weapons.md 8.3), not read
//!   from the scripts.

use source_engine::content::{material::MaterialLoader, props, sound::SoundScripts};

use super::scripts;
use crate::mount::{Access, Mount};

/// One gun of M2 and what its set holds.
#[derive(Clone, Copy, Debug)]
pub struct WeaponSet {
    /// The set's id, `weapon/<id>`: the gun's id past `tf2:`.
    pub id: &'static str,
    /// The script's name (`scripts/<script>.ctx`).
    pub script: &'static str,
    /// The `c_` weapon model.
    pub model: &'static str,
    /// The classes whose arms hold it (plan, decision 6).
    pub classes: &'static [&'static str],
    /// Its sound entries (weapons.md 8).
    pub sounds: &'static [&'static str],
    /// Other models it uses (its projectile).
    pub models: &'static [&'static str],
}

/// The guns: M2's four (weapons.md 8) and weapons_more.md's eight (9).
pub const M2_WEAPONS: &[WeaponSet] = &[
    WeaponSet {
        id: "tf_weapon_scattergun",
        script: "tf_weapon_scattergun",
        model: "models/weapons/c_models/c_scattergun.mdl",
        classes: &["scout"],
        sounds: &[
            "Weapon_Scatter_Gun.Single",
            "Weapon_Scatter_Gun.SingleCrit",
            "Weapon_Scatter_Gun.Empty",
            "Weapon_Scatter_Gun.Reload",
            "Weapon_Scatter_Gun.WorldReload",
            "Weapon_Scatter_Gun.Pump",
            "Weapon_Scatter_Gun.Draw",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_rocketlauncher",
        script: "tf_weapon_rocketlauncher",
        model: "models/weapons/c_models/c_rocketlauncher/c_rocketlauncher.mdl",
        classes: &["soldier"],
        sounds: &[
            "Weapon_RPG.Single",
            "Weapon_RPG.SingleCrit",
            "Weapon_RPG.Reload",
            "Weapon_RPG.WorldReload",
            "Weapon_RPG.Draw",
            "BaseExplosionEffect.Sound",
        ],
        models: &["models/weapons/w_models/w_rocket.mdl"],
    },
    WeaponSet {
        id: "tf_weapon_shotgun_primary",
        script: "tf_weapon_shotgun_primary",
        model: "models/weapons/c_models/c_shotgun/c_shotgun.mdl",
        classes: &["engineer"],
        sounds: &[
            "Weapon_Shotgun.Single",
            "Weapon_Shotgun.SingleCrit",
            "Weapon_Shotgun.Empty",
            "Weapon_Shotgun.Reload",
            "Weapon_Shotgun.WorldReload",
            "Weapon_Shotgun.Pump",
            "Weapon_Shotgun.Draw",
            "Weapon_ShotgunEngineer.Cock_Back",
            "Weapon_ShotgunEngineer.Cock_Forward",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_minigun",
        script: "tf_weapon_minigun",
        model: "models/weapons/c_models/c_minigun/c_minigun.mdl",
        classes: &["heavy"],
        sounds: &[
            "Weapon_Minigun.Fire",
            "Weapon_Minigun.FireCrit",
            "Weapon_Minigun.ClipEmpty",
            "Weapon_Minigun.Draw",
            "Weapon_Minigun.WindUp",
            "Weapon_Minigun.Spin",
            "Weapon_Minigun.WindDown",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_pistol_scout",
        script: "tf_weapon_pistol_scout",
        model: "models/weapons/c_models/c_pistol/c_pistol.mdl",
        classes: &["scout"],
        sounds: &[
            "Weapon_Pistol.Single",
            "Weapon_Pistol.SingleCrit",
            "Weapon_Pistol.ClipEmpty",
            "Weapon_Pistol.ReloadScout",
            "Weapon_Pistol.WorldReload",
            "Weapon_Pistol.Draw",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_smg",
        script: "tf_weapon_smg",
        model: "models/weapons/c_models/c_smg/c_smg.mdl",
        classes: &["sniper"],
        sounds: &[
            "Weapon_SMG.Single",
            "Weapon_SMG.SingleCrit",
            "Weapon_SMG.ClipEmpty",
            "Weapon_SMG.ClipOut",
            "Weapon_SMG.ClipIn",
            "Weapon_SMG.WorldReload",
            "Weapon_SMG.Draw",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_sniperrifle",
        script: "tf_weapon_sniperrifle",
        model: "models/weapons/c_models/c_sniperrifle/c_sniperrifle.mdl",
        classes: &["sniper"],
        sounds: &[
            "Weapon_SniperRifle.Single",
            "Weapon_SniperRifle.SingleCrit",
            "Weapon_SniperRifle.ClipEmpty",
            "Weapon_SniperRifle.BoltBack",
            "Weapon_SniperRifle.BoltForward",
            "Weapon_SniperRifle.WorldReload",
            "Weapon_SniperRifle.Draw",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_revolver",
        script: "tf_weapon_revolver",
        model: "models/weapons/c_models/c_revolver/c_revolver.mdl",
        classes: &["spy"],
        sounds: &[
            "Weapon_Revolver.Single",
            "Weapon_Revolver.SingleCrit",
            "Weapon_Revolver.ClipEmpty",
            "Weapon_Revolver.Reload1",
            "Weapon_Revolver.Reload2",
            "Weapon_Revolver.Reload3",
            "Weapon_Revolver.WorldReload",
            "Weapon_Revolver.Draw",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_grenadelauncher",
        script: "tf_weapon_grenadelauncher",
        model: "models/weapons/c_models/c_grenadelauncher/c_grenadelauncher.mdl",
        classes: &["demo"],
        sounds: &[
            "Weapon_GrenadeLauncher.Single",
            "Weapon_GrenadeLauncher.SingleCrit",
            "Weapon_GrenadeLauncher.DrumOpen",
            "Weapon_GrenadeLauncher.DrumLoad",
            "Weapon_GrenadeLauncher.DrumClose",
            "Weapon_GrenadeLauncher.WorldReload",
            "Weapon_GrenadeLauncher.Draw",
            "Weapon_Grenade_Pipebomb.Bounce",
            "Weapon_Grenade_Pipebomb.Explode",
        ],
        models: &["models/weapons/w_models/w_grenade_grenadelauncher.mdl"],
    },
    WeaponSet {
        id: "tf_weapon_pipebomblauncher",
        script: "tf_weapon_pipebomblauncher",
        model: "models/weapons/c_models/c_stickybomb_launcher/c_stickybomb_launcher.mdl",
        classes: &["demo"],
        sounds: &[
            "Weapon_StickyBombLauncher.Single",
            "Weapon_StickyBombLauncher.SingleCrit",
            "Weapon_StickyBombLauncher.BoltBack",
            "Weapon_StickyBombLauncher.BoltForward",
            "Weapon_StickyBombLauncher.WorldReload",
            "Weapon_StickyBombLauncher.ChargeUp",
            "Weapon_StickyBombLauncher.ModeSwitch",
            "Weapon_Grenade_Pipebomb.Explode",
        ],
        models: &["models/weapons/w_models/w_stickybomb.mdl"],
    },
    WeaponSet {
        id: "tf_weapon_flamethrower",
        script: "tf_weapon_flamethrower",
        model: "models/weapons/c_models/c_flamethrower/c_flamethrower.mdl",
        classes: &["pyro"],
        sounds: &[
            "Weapon_FlameThrower.FireStart",
            "Weapon_FlameThrower.FireLoop",
            "Weapon_FlameThrower.FireLoopCrit",
            "Weapon_FlameThrower.FireEnd",
            "Weapon_FlameThrower.FireHit",
            "Weapon_FlameThrower.PilotLoop",
            "Weapon_FlameThrower.Draw",
        ],
        models: &[],
    },
    WeaponSet {
        id: "tf_weapon_syringegun_medic",
        script: "tf_weapon_syringegun_medic",
        model: "models/weapons/c_models/c_syringegun/c_syringegun.mdl",
        classes: &["medic"],
        sounds: &[
            "Weapon_SyringeGun.Single",
            "Weapon_SyringeGun.SingleCrit",
            "Weapon_SyringeGun.ClipEmpty",
            "Weapon_SyringeGun.Reload1",
            "Weapon_SyringeGun.Reload2",
            "Weapon_SyringeGun.Reload3",
            "Weapon_SyringeGun.WorldReload",
            "Weapon_SyringeGun.Draw",
        ],
        models: &["models/weapons/w_models/w_syringe_proj.mdl"],
    },
];

/// The nine classes' player models (specs/tf2/classes.md).
pub fn player_models() -> Vec<String> {
    super::classes::classes().classes.into_iter().map(|c| c.model).collect()
}

/// A class's arms model.
pub fn arms_model(class: &str) -> String {
    format!("models/weapons/c_models/c_{class}_arms.mdl")
}

/// The files `load` reads of `mount` that it has, sorted.
fn read_by(mount: &Mount, load: impl FnOnce()) -> Vec<String> {
    let tracking = mount.track();
    load();
    let seen = tracking.finish();
    seen.into_iter()
        .filter_map(|a| match a {
            Access::File(p) | Access::Every(p) => Some(p),
            Access::Folder(_) | Access::Held(_) => None,
        })
        .filter(|p| mount.contains(p))
        .collect()
}

/// The files of gun `w`'s set (module doc). `scripts`: the sound scripts
/// (`SoundScripts::load_with`).
pub fn weapon_files(mount: &Mount, w: &WeaponSet, sound_scripts: &SoundScripts) -> Vec<String> {
    let gun = format!("{}:{}", super::GAME, w.id);
    let mut files = read_by(mount, || {
        let mut materials = MaterialLoader::guest(mount, 0, 0);
        let (mut kit, mut warnings) = (crate::map::GameKit::default(), Vec::new());
        super::maps::add_guns(&mut materials, &|g| g == gun, &mut kit, &mut warnings);
        for name in w.sounds {
            for wave in sound_scripts.waves(name) {
                let _ = mount.read(&wave);
            }
        }
    });
    files.push(scripts::path_of(w.script));
    files.sort();
    files.dedup();
    files
}

/// The sound scripts as the sets read them.
pub fn sound_scripts(mount: &Mount) -> SoundScripts {
    SoundScripts::load_with(&mut MaterialLoader::guest(mount, 0, 0), "", true)
}

/// The files class `class`'s body and arms read (module doc), sorted.
pub fn class_files(mount: &Mount, class: &super::classes::Class) -> Vec<String> {
    let default = super::classes::classes().get(super::classes::DEFAULT).map(|c| c.model.clone()).unwrap_or_default();
    read_by(mount, || {
        let mut materials = MaterialLoader::guest(mount, 0, 0);
        let surfaces = source_engine::content::surfaceprops::SurfaceProps::load(&mut materials);
        let _ = super::bodies::load(&mut materials, &surfaces, &class.model, &default);
        let _ = props::load_view_model(&mut materials, &class.model, &class.model, true);
        let _ = props::load_view_model(&mut materials, &arms_model(&class.arms), &class.arms, true);
    })
}

/// `players` and each class's `players/<class>` (module doc): the shared
/// files (every class's and the HUD's) apart, then each class's own.
pub fn player_pieces(mount: &Mount) -> Vec<(String, Vec<String>)> {
    let table = super::classes::classes();
    let per_class: Vec<(String, Vec<String>)> =
        table.classes.iter().map(|c| (super::classes::set_of(&c.name), class_files(mount, c))).collect();
    let hud = read_by(mount, || {
        let mut materials = MaterialLoader::guest(mount, 0, 0);
        let _ = super::hud::load(&mut materials);
        // The kill feed's icons (`ui::systems::register_kill_icons`).
        let _ = super::hud::kill_icon_pictures(mount);
        // The guns' looks on any map (`maps::add_guest_looks`: the
        // particle files of the explosion, muzzle flashes and flames with
        // their materials, the impact decals and effect materials), and
        // the surfaces' impact sounds.
        let mut map = crate::map::MapData::default();
        super::maps::add_guest_looks(&mut materials, &mut map);
        let surfaces = source_engine::content::surfaceprops::SurfaceProps::load(&mut materials);
        let scripts = SoundScripts::load_with(&mut materials, "", true);
        let impacts: std::collections::BTreeSet<String> =
            surfaces.names().filter_map(|n| surfaces.text(n, "bulletimpact")).collect();
        for wave in impacts.iter().flat_map(|e| scripts.waves(e)) {
            let _ = mount.read(&wave);
        }
    });
    // What every class reads (the surface properties, shared includes and
    // lightwarps): fetched with every TF2 match.
    let mut shared: std::collections::BTreeSet<String> = per_class.first().map(|(_, f)| f.iter().cloned().collect()).unwrap_or_default();
    for (_, files) in &per_class[1.min(per_class.len())..] {
        shared.retain(|p| files.contains(p));
    }
    let mut out = Vec::new();
    for (set, files) in per_class {
        out.push((set, files.into_iter().filter(|p| !shared.contains(p)).collect()));
    }
    shared.extend(hud);
    out.insert(0, ("players".into(), shared.into_iter().collect()));
    out
}

/// `weapon/<id>` for each M2 gun, `players` and `players/<class>`: each
/// set's name and files.
pub fn pieces(mount: &Mount) -> Vec<(String, Vec<String>)> {
    let sounds = sound_scripts(mount);
    let mut out: Vec<(String, Vec<String>)> =
        M2_WEAPONS.iter().map(|w| (format!("weapon/{}", w.id), weapon_files(mount, w, &sounds))).collect();
    out.extend(player_pieces(mount));
    out
}
