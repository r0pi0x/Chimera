//! HL2:DM's weapons as data (docs/weapon-data.md): one TOML file per
//! weapon in games/hl2dm/data/weapons (built in, parsed at run time:
//! `hl2_weapons`) with specs/hl2dm/weapons.md's numbers (H3): the
//! pistol, .357, SMG1 and AR2, the shotgun (single and double shot), the
//! crowbar and the stunstick; and the projectile weapons
//! (specs/hl2dm/projectiles.md, H4): the SMG1's grenade and the AR2's ball
//! (`alt_fire`), the crossbow (`crossbow`, its bolts Chimera's
//! `chimera:projectile`), the RPG (`rpg`), the frag (`frag`) and the
//! S.L.A.M. (`slam`); and the gravity gun (`physcannon`,
//! specs/hl2dm/gravity_gun.md, H5, on Chimera's held bodies), on Chimera's flyers (`weapon::projectile`, lift C1)
//! with HL2's blast (`launch`). The install's own scripts (`scripts/weapon_*.txt`,
//! plain text) are read by `game::scripts` and checked against the data.
//!
//! The shared frame (`chimera_sim::weapon`) runs them with HL2:DM's parts:
//! no penetration or falloff, the script's damage against everything,
//! head x3, the cone spread (`SpreadShape::Cone`), the shot along view +
//! punch, holding attack on an empty clip reloads, the shotgun's double
//! shot (`AltShot`, drawn by other clients with its 12 pellets) and shells
//! in as their insert starts, a call's odd bullets as ±3 boxes
//! (`Hitscan::odd_box`, weapons.md 5), the melee box hit refined to a line
//! to twice its distance and its corners (`Swing::refine`, 11), and lag
//! compensation's choice of whom to rewind (`[lag_compensation]`,
//! `weapon::lagcomp::RewindChoice`, 12: attack held or a shot in the last
//! 5 commands; within 1.5 x their max speed, `rewind_speeds`, x
//! `sv_maxunlag`, or 45 degrees of the view). What data can't
//! say is the hooks it names (`register_hooks`):
//!
//! - `hl2dm:inventory` (`inventory::Hl2Item`): the script's facts, and the
//!   holstered auto-reload (`inventory::Hl2Holster`);
//! - `hl2dm:pistol` (`pistol::Hl2Pistol`): the accuracy ramp, the click
//!   refire, attack2's block, the dry fire's wait;
//! - `hl2dm:punch`, `hl2dm:machine_gun` (`recoil`): the kicks on HL2's
//!   punch spring;
//! - `hl2dm:shotgun` (`shotgun::Hl2Shotgun`): the pump;
//! - `hl2dm:dry_under_water` (`under_water::Hl2DryUnderWater`): the guns
//!   that won't fire with the eyes under water (the .357, the machine
//!   guns, the shotgun);
//! - `hl2dm:vent` (`vent::Hl2Vent`): the AR2's vents following its shots.
//!
//! `Hl2GunsPlugin` is the weapons alone (data, hooks, recoil, the damage
//! shove of HL2:DM players: `shove`; what a game of several games adds);
//! `Hl2WeaponsPlugin` is HL2:DM's game around them: the spawn loadout,
//! ammo per player, the next best weapon, pickup and drop rules
//! (`inventory`).
//!
//! Known shortcuts: an
//! odd pellet's box meets a turned hitbox as that hitbox grown 3 units on
//! its own axes (exact for one lined up); another client draws a tick's
//! catch-up rounds as lines (each comes as its own round); a melee swing's
//! lines hit players' hitboxes as bullets do (weapons.md 11: their
//! collision box); the tracer counter, impacts and sounds are M10's.

pub mod alt_fire;
pub mod crossbow;
pub mod frag;
pub mod inventory;
pub mod launch;
pub mod physcannon;
pub mod pistol;
pub mod recoil;
pub mod rpg;
pub mod shotgun;
pub mod shove;
pub mod slam;
pub mod under_water;
pub mod vent;

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
    data_file!("crossbow.toml"),
    data_file!("rpg.toml"),
    data_file!("frag.toml"),
    data_file!("slam.toml"),
    data_file!("physcannon.toml"),
];

pub const CROWBAR: &str = "hl2dm:weapon_crowbar";
pub const STUNSTICK: &str = "hl2dm:weapon_stunstick";
pub const PISTOL: &str = "hl2dm:weapon_pistol";
pub const MAGNUM: &str = "hl2dm:weapon_357";
pub const SMG1: &str = "hl2dm:weapon_smg1";
pub const AR2: &str = "hl2dm:weapon_ar2";
pub const SHOTGUN: &str = "hl2dm:weapon_shotgun";
pub const CROSSBOW: &str = "hl2dm:weapon_crossbow";
pub const RPG: &str = "hl2dm:weapon_rpg";
pub const FRAG: &str = "hl2dm:weapon_frag";
pub const SLAM: &str = "hl2dm:weapon_slam";
pub const PHYSCANNON: &str = "hl2dm:weapon_physcannon";

/// The hooks the data names.
pub const INVENTORY: &str = "hl2dm:inventory";
pub const PISTOL_HOOK: &str = "hl2dm:pistol";
pub const PUNCH: &str = "hl2dm:punch";
pub const MACHINE_GUN: &str = "hl2dm:machine_gun";
pub const SHOTGUN_HOOK: &str = "hl2dm:shotgun";
pub const SMG1_GRENADE_HOOK: &str = "hl2dm:smg1_grenade";
pub const BALL_HOOK: &str = "hl2dm:combine_ball";
pub const RPG_HOOK: &str = "hl2dm:rpg";
pub const CROSSBOW_HOOK: &str = "hl2dm:crossbow";
pub const FRAG_HOOK: &str = "hl2dm:frag";
pub const SLAM_HOOK: &str = "hl2dm:slam";
pub const PHYSCANNON_HOOK: &str = "hl2dm:physcannon";
pub const DRY_UNDER_WATER: &str = "hl2dm:dry_under_water";
pub const VENT_HOOK: &str = "hl2dm:vent";

/// The weapons every player spawns with besides the melee weapon of its
/// model (`inventory::arm`), in giving order (weapons.md 3): the last
/// given, the gravity gun, ends in hand.
pub const STARTING: &[&str] = &[PISTOL, SMG1, FRAG, PHYSCANNON];

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
    })
    .register_weapon_hook::<alt_fire::SmgGrenadeParams>(SMG1_GRENADE_HOOK, |e, _, p| {
        e.insert((alt_fire::Hl2SmgGrenade(p.clone()), p.rounds.rounds()));
    })
    .register_weapon_hook::<alt_fire::BallParams>(BALL_HOOK, |e, _, p| {
        e.insert((
            alt_fire::Hl2Ball {
                params: p.clone(),
                launch_at: None,
            },
            p.rounds.rounds(),
        ));
    })
    .register_weapon_hook::<rpg::RpgParams>(RPG_HOOK, |e, _, p| {
        e.insert((
            rpg::Hl2Rpg {
                params: p.clone(),
                missile: false,
                reload_until: None,
                missile_entity: None,
            },
            p.rounds.rounds(),
        ));
    })
    .register_weapon_hook::<crossbow::CrossbowParams>(CROSSBOW_HOOK, |e, _, p| {
        e.insert((
            crossbow::Hl2Crossbow {
                params: *p,
                fired_at: None,
                secondary_held: false,
                zoomed: false,
            },
            chimera_sim::weapon::OwnZoom,
        ));
    })
    .register_weapon_hook::<frag::FragParams>(FRAG_HOOK, |e, _, p| {
        e.insert((
            frag::Hl2Frag {
                params: p.clone(),
                stage: frag::READY,
                secondary: false,
                kind: frag::THROW,
                since: 0.0,
                released: false,
            },
            p.rounds.rounds(),
        ));
    })
    .register_weapon_hook::<slam::SlamParams>(SLAM_HOOK, |e, _, p| {
        e.insert((
            slam::Hl2Slam {
                params: p.clone(),
                mine_mode: false,
                transition_until: 0.0,
                detonator: false,
                action: slam::IDLE,
                action_end: 0.0,
            },
            p.rounds.rounds(),
        ));
    })
    .register_weapon_hook::<physcannon::PhysParams>(PHYSCANNON_HOOK, |e, _, p| {
        e.insert(physcannon::Hl2Physcannon::new(p.clone()));
    })
    .register_weapon_hook::<under_water::Hl2DryUnderWater>(DRY_UNDER_WATER, |e, _, p| {
        e.insert(*p);
    })
    .register_weapon_hook::<vent::VentParams>(VENT_HOOK, |e, _, p| {
        e.insert(vent::Hl2Vent { params: p.clone(), value: 0.0 });
    });
}

/// The projectile weapons' systems (`Hl2GunsPlugin`): attack2s, the RPG,
/// the crossbow, the frag and the S.L.A.M. on Chimera's flyers.
fn projectile_systems(app: &mut App) {
    use chimera_sim::weapon::{SelectWeapons, projectile::FlyerSet};
    app.add_observer(slam::leaver_satchels);
    app.add_observer(launch::loadout_rounds);
    app.add_message::<slam::SlamAction>()
        .add_message::<slam::SatchelPop>()
        .add_message::<crate::map::GameSound>()
        .add_message::<crate::map::logic_view::MapShake>()
        .add_message::<crate::map::PlaceDecal>()
        .add_systems(
            Predict::Weapons,
            (
                (alt_fire::before_frame, crossbow::before_frame, frag::before_frame, slam::before_frame).before(WeaponFrame),
                crossbow::after_frame.after(WeaponFrame),
            ),
        )
        .add_systems(Predict::Select, alt_fire::lock_charging.before(SelectWeapons))
        .add_systems(FixedUpdate, launch::mark_under_water.before(SimSet::Weapons))
        .add_systems(FixedPostUpdate, frag::air_drag.before(avian3d::prelude::PhysicsSystems::StepSimulation))
        .add_systems(FixedUpdate, launch::fill_up.run_if(crate::core::authoritative))
        .add_systems(
            FixedUpdate,
            (
                (alt_fire::ball_touches, alt_fire::ball_ends, alt_fire::grenade_stopped).in_set(FlyerSet::Touch),
                (slam::actions, slam::satchels, slam::mines, frag::think, frag::player_taps, slam::spent)
                    .chain()
                    .in_set(FlyerSet::Touch),
                frag::drop_live
                    .after(crate::core::apply_damage)
                    .before(chimera_sim::weapon::drop::drop_on_death),
            )
                .run_if(crate::core::authoritative),
        )
        .predicted_part::<launch::Hl2Rounds>()
        .predicted_part::<alt_fire::Hl2Ball>()
        .predicted_part::<crossbow::Hl2Crossbow>()
        .predicted_part::<frag::Hl2Frag>()
        .predicted_part::<slam::Hl2Slam>()
        .predicted_part::<physcannon::Hl2Physcannon>();
    rpg::plugin(app);
}

/// The gravity gun's systems (`Hl2GunsPlugin`): its frame (predicted),
/// the server's holds and the drop at its holder's death.
fn physcannon_systems(app: &mut App) {
    use chimera_sim::weapon::SelectWeapons;
    if !app.is_plugin_added::<crate::map::held::HeldPlugin>() {
        app.add_plugins(crate::map::held::HeldPlugin);
    }
    app.add_systems(Predict::Weapons, physcannon::frame.before(WeaponFrame))
        .add_systems(Predict::Select, physcannon::lock_holding.before(SelectWeapons))
        .add_systems(
            FixedUpdate,
            (
                physcannon::hold.after(SimSet::Weapons),
                physcannon::drop_on_death
                    .after(crate::core::apply_damage)
                    .before(chimera_sim::weapon::drop::drop_on_death),
            )
                .run_if(crate::core::authoritative),
        );
    use crate::console::ConsoleAppExt;
    app.console_command("hl2dm:phys_swap", "Drop what the gravity gun holds and switch to the last weapon, or select the gravity gun.", |w, _| {
        let e = crate::console::command_player(w)?;
        physcannon::phys_swap(w, e);
        Ok(None)
    });
    app.world_mut().resource_mut::<crate::console::Console>().player_command("hl2dm:phys_swap");
}

/// HL2:DM's weapons alone: the data, its hooks, the recoil and the pump
/// around the shared frame, the holstered auto-reload and the damage
/// shove of HL2:DM players. What an app of several games adds.
pub struct Hl2GunsPlugin;

impl Plugin for Hl2GunsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::character::CharacterPartsExt;
        app.on_every_character::<recoil::Hl2Punch>();
        register_hooks(app);
        // Its guns' short names (data/display_names.toml).
        chimera_sim::weapon::names::DisplayNamesAppExt::add_display_names(
            app,
            "hl2dm data/display_names.toml",
            include_str!("../../../data/display_names.toml"),
        );
        projectile_systems(app);
        physcannon_systems(app);
        if let Err(e) = app.add_weapon_data(DATA) {
            panic!("HL2:DM weapon data: {e}");
        }
        // A rung's player model and melee weapon (`rungs`).
        app.add_plugins(super::rungs::Hl2RungsPlugin);
        app.add_systems(
            Predict::Weapons,
            (
                (
                    recoil::decay_punch,
                    pistol::before_frame,
                    shotgun::before_frame,
                    under_water::before_frame.after(shotgun::before_frame).after(alt_fire::before_frame),
                    // At the start of the AR2's frame (weapons.md 7.3).
                    vent::before_frame.before(alt_fire::before_frame),
                )
                    .before(WeaponFrame),
                (recoil::kick, pistol::after_frame, shotgun::after_frame, inventory::holster_reload).after(WeaponFrame),
                recoil::nudge_view.after(recoil::kick),
            ),
        )
        .add_systems(
            FixedUpdate,
            (
                shove::shove_players
                    .after(SimSet::Weapons)
                    .before(crate::core::apply_damage)
                    .run_if(crate::core::authoritative),
                rewind_speeds.before(SimSet::Weapons).run_if(crate::core::authoritative),
            ),
        )
        .add_systems(Update, recoil::apply_view_nudge)
        .init_resource::<recoil::ViewNudge>()
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
                weapon_stay: false,
                death_push: Some(inventory::DEATH_PUSH),
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
            .add_systems(Predict::Weapons, inventory::switch_from_empty.after(WeaponFrame))
            .add_systems(
                Predict::Select,
                inventory::mark_unselectable.before(chimera_sim::weapon::SelectWeapons),
            )
            .init_resource::<inventory::DefaultWeaponSetting>();
        crate::console::resource_cvar::<inventory::DefaultWeaponSetting, String>(
            app,
            "hl2dm:cl_defaultweapon",
            "The weapon in hand after spawning, by class name (sent to the server as userinfo; weapon_physcannon by default).",
            |s| &mut s.0,
        );
        let mut console = app.world_mut().resource_mut::<crate::console::Console>();
        console.archive("hl2dm:cl_defaultweapon");
        console.userinfo("hl2dm:cl_defaultweapon");
    }
}

/// Each HL2:DM character's current max speed (`SourceSpeed`, its speed
/// selection's) for lag compensation's choice (weapons.md 12,
/// `weapon::lagcomp::RewindSpeed`); the server's.
fn rewind_speeds(
    q: Query<(Entity, &source_engine::source_movement::SourceSpeed, Option<&chimera_sim::weapon::lagcomp::RewindSpeed>)>,
    mut commands: Commands,
) {
    use chimera_sim::weapon::lagcomp::RewindSpeed;
    for (e, speed, have) in &q {
        let want = (speed.0 > 0.0).then(|| RewindSpeed(speed.0 * 0.0254));
        if want != have.copied() {
            match want {
                Some(s) => commands.entity(e).insert(s),
                None => commands.entity(e).remove::<RewindSpeed>(),
            };
        }
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
        chimera_sim::weapon::projectile::register_launcher(&mut app);
        app.add_weapon_data(DATA).unwrap();
        assert_eq!(hl2_weapons().weapons.len(), 12);
        assert_eq!(view_models().len(), 12);
    }
}
