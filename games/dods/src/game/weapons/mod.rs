//! DoD:S's weapons as data (data/weapons; docs/weapon-data.md;
//! specs/dods/weapons.md, damage.md): M1a's guns (the rifles, sniper
//! rifles, SMGs, BAR and StG44, the two machine guns, the pistols and the
//! carbine) and the two melee weapons, each built from its weapon script
//! over the committed document (`script_numbers`: the script's numbers,
//! sounds and models when the install or the asset store serves it, the
//! spec's numbers otherwise), with DoD:S's own rules as hooks:
//! `dods:gun` (`gun`: the script's spread by sights and movement, the
//! sights blocking the trigger, the Garand's reload only when empty),
//! `dods:melee` (the script a knife or spade reads) and `dods:hit_slow`
//! (`hit_slow`: the victim slowed 0.5 s, a leg 2 s). Their view models
//! are the scripts' (`v_*.mdl`, arms and gun in one model), their held
//! models (`w_*.mdl`) on the default body's skeleton, and the explosives'
//! projectile models their data names (`add_guns`).
//!
//! The machine guns (`dods:mg`, `mg`: deployed while prone, `stance`) are
//! the ladders' `heavy` class; the explosives (`explosives`: frag and
//! stick grenades, rifle grenades, bazooka and Panzerschreck) the
//! ladders' `grenade` and `launcher` classes. Draw and empty-reload times
//! are weapons.md's measured table (the data's `draw` and `[magazine]
//! reload`).
//!
//! Known shortcuts (beside `gun`'s, `hit_slow`'s and `explosives`'): a
//! partial reload takes the empty one's time (unmeasured); no kill icons
//! or HUD (M3); the melee swing's reach and box and the backstab's cone
//! are CS:S's knife's (damage.md, open question 4).

use bevy::prelude::*;
use chimera_sim::weapon::data::{WeaponDataAppExt, WeaponSet};

use crate::core::{Predict, SimSet};
use source_engine::content::{material::MaterialLoader, props, surfaceprops::SurfaceProps};

use crate::map::GameKit;

pub mod explosives;
pub mod gun;
pub mod hit_slow;
pub mod mg;
pub mod script_numbers;

macro_rules! data_file {
    ($name:literal) => {
        ($name, chimera_core::game_data!("data/weapons/", $name))
    };
}

/// The weapon documents, built in, in registration order.
pub const DATA: &[(&str, chimera_core::game_data::GameData)] = &[
    data_file!("_base.toml"),
    data_file!("_gun.toml"),
    data_file!("garand.toml"),
    data_file!("k98.toml"),
    data_file!("spring.toml"),
    data_file!("k98_scoped.toml"),
    data_file!("thompson.toml"),
    data_file!("mp40.toml"),
    data_file!("bar.toml"),
    data_file!("mp44.toml"),
    data_file!("30cal.toml"),
    data_file!("mg42.toml"),
    data_file!("colt.toml"),
    data_file!("p38.toml"),
    data_file!("c96.toml"),
    data_file!("m1carbine.toml"),
    data_file!("amerknife.toml"),
    data_file!("spade.toml"),
    data_file!("frag_us.toml"),
    data_file!("frag_ger.toml"),
    data_file!("riflegren_us.toml"),
    data_file!("riflegren_ger.toml"),
    data_file!("bazooka.toml"),
    data_file!("pschreck.toml"),
];

/// The hooks' names.
pub const GUN_HOOK: &str = "dods:gun";
pub const MELEE_HOOK: &str = "dods:melee";
pub const HIT_SLOW_HOOK: &str = "dods:hit_slow";
pub const MG_HOOK: &str = "dods:mg";

pub const GARAND: &str = "dods:weapon_garand";
pub const K98: &str = "dods:weapon_k98";
pub const SPRING: &str = "dods:weapon_spring";
pub const K98_SCOPED: &str = "dods:weapon_k98_scoped";
pub const THOMPSON: &str = "dods:weapon_thompson";
pub const MP40: &str = "dods:weapon_mp40";
pub const BAR: &str = "dods:weapon_bar";
pub const MP44: &str = "dods:weapon_mp44";
pub const CAL30: &str = "dods:weapon_30cal";
pub const MG42: &str = "dods:weapon_mg42";
/// The Colt M1911.
pub const COLT: &str = "dods:weapon_colt";
pub const P38: &str = "dods:weapon_p38";
pub const C96: &str = "dods:weapon_c96";
pub const M1CARBINE: &str = "dods:weapon_m1carbine";
pub const KNIFE: &str = "dods:weapon_amerknife";
pub const SPADE: &str = "dods:weapon_spade";
pub const FRAG_US: &str = "dods:weapon_frag_us";
pub const FRAG_GER: &str = "dods:weapon_frag_ger";
pub const RIFLEGREN_US: &str = "dods:weapon_riflegren_us";
pub const RIFLEGREN_GER: &str = "dods:weapon_riflegren_ger";
pub const BAZOOKA: &str = "dods:weapon_bazooka";
pub const PSCHRECK: &str = "dods:weapon_pschreck";

/// Every gun of DoD:S's data (the melee weapons apart: `MELEE`).
pub const GUNS: &[&str] = &[GARAND, K98, SPRING, K98_SCOPED, THOMPSON, MP40, BAR, MP44, CAL30, MG42, COLT, P38, C96, M1CARBINE];

/// The melee weapons.
pub const MELEE: &[&str] = &[KNIFE, SPADE];

/// The explosives (M1b: `explosives`).
pub const EXPLOSIVES: &[&str] = &[FRAG_US, FRAG_GER, RIFLEGREN_US, RIFLEGREN_GER, BAZOOKA, PSCHRECK];

/// The server documents' name of DoD:S's weapon set (`server_docs`).
pub const SERVER_DOCS: &str = "dods:weapons";

/// DoD:S's weapons as data, the committed documents (the spec's numbers;
/// a fresh copy each call).
pub fn dods_weapons() -> WeaponSet {
    script_numbers::GunNumbers::builtin().parse().unwrap_or_else(|e| panic!("DoD:S weapon data: {e}"))
}

/// Every gun's display name by id (its data's `hud.name`).
pub fn display_names(_: &crate::mount::Mount) -> Vec<(String, String)> {
    dods_weapons().weapons.into_iter().filter_map(|w| Some((w.id, w.hud.name?))).collect()
}

/// DoD:S's guns that `wants`, into `into`: their view models (right-handed,
/// never mirrored), their world models held on the default body's
/// skeleton, and the explosives' projectile models (their data's `model`,
/// under `explosives::projectile_key`).
pub fn add_guns(
    materials: &mut MaterialLoader,
    surfaces: &SurfaceProps,
    wants: &dyn Fn(&str) -> bool,
    into: &mut GameKit,
    warnings: &mut Vec<String>,
) {
    // The models are the scripts' (read through the map's mount).
    let set = script_numbers::GunNumbers::from_mount(Some(materials.mount()))
        .parse()
        .unwrap_or_else(|_| dods_weapons());
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
        // An explosive's projectile, by its own key (never the weapon's
        // world model): in its bone's frame, as a held model (flyers fly
        // nose first by its nose, `MapHeldModel::nose`).
        if let (Some(model), Some(skeleton)) = (explosives::projectile_model(w), &skeleton) {
            match props::load_held(materials, model, &explosives::projectile_key(&w.id), skeleton) {
                Ok(h) => into.held.push(h),
                Err(e) => warnings.push(e),
            }
        }
    }
}

/// DoD:S's hooks, on an app.
pub fn register_hooks(app: &mut App) {
    app.register_weapon_hook::<gun::DodsGun>(GUN_HOOK, |e, _, p| gun::insert(e, p))
        .register_weapon_hook::<MeleeParams>(MELEE_HOOK, |_, _, _| {})
        .register_weapon_hook::<hit_slow::HitSlow>(HIT_SLOW_HOOK, |e, _, p| {
            e.insert(*p);
        })
        .register_weapon_hook::<mg::DodsMg>(MG_HOOK, |e, _, p| {
            e.insert(*p);
        })
        .register_weapon_hook::<super::player_anim::BodyAnim>(super::player_anim::BODY_ANIM_HOOK, |e, _, p| {
            e.insert(p.clone());
        });
    explosives::register_hooks(app);
}

/// `dods:melee`'s parameters: the script it reads (`script_numbers`).
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MeleeParams {
    pub script: String,
}

/// DoD:S's weapons in an app's weapon registry: their data (the scripts'
/// numbers over the committed ones, `script_numbers`), hooks and systems.
pub struct DodsGunsPlugin;

impl Plugin for DodsGunsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_sim::server_docs::DocsAppExt;
        use chimera_sim::weapon::WeaponFrame;
        register_hooks(app);
        // The harness's built-in numbers, else the scripts' over the data.
        let numbers = app
            .world()
            .get_resource::<script_numbers::GunNumbers>()
            .cloned()
            .unwrap_or_else(script_numbers::GunNumbers::from_scripts);
        // Said once the app runs (a dedicated server's log is set up after
        // its plugins are built).
        let said = numbers.clone();
        app.add_systems(Startup, move || said.log());
        let docs: Vec<(&str, &str)> = numbers.docs.iter().map(|(n, t)| (n.as_str(), t.as_str())).collect();
        let set = app.add_weapon_data(&docs).unwrap_or_else(|e| panic!("DoD:S weapon data: {e}"));
        // A client without the scripts plays with the server's numbers.
        app.server_weapon_docs(SERVER_DOCS, set, numbers.docs.clone());
        app.insert_resource(numbers);
        chimera_core::character::CharacterPartsExt::on_every_character::<hit_slow::DodsSlowed>(app);
        app.add_systems(
            Predict::Weapons,
            (
                (gun::before_frame, explosives::shoulders).before(WeaponFrame),
                explosives::pins.after(chimera_sim::weapon::grenade::ThrowFrame),
                gun::sights.after(WeaponFrame),
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                explosives::rifle_shots.after(WeaponFrame).in_set(SimSet::Weapons).run_if(crate::core::authoritative),
                explosives::looks.after(SimSet::Weapons),
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                hit_slow::on_hit.after(SimSet::Weapons).after(crate::core::apply_damage),
                super::stamina::mg_shots.after(SimSet::Weapons),
            )
                .run_if(crate::core::authoritative),
        );
    }
}
