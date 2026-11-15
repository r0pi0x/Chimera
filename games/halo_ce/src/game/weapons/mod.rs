//! Halo's weapons (specs/halo_ce/weapons.md, projectiles.md; M2 and M3 of
//! docs/plans/active/halo-ce.md): the nine held multiplayer weapons (the
//! assault rifle, pistol, plasma rifle, shotgun, sniper rifle, rocket
//! launcher, needler, plasma pistol and flamethrower) and the frag and
//! plasma grenades as data (data/weapons/*.toml: every number in the
//! `halo_ce:weapon` hook's table, `params`; data/grenades.toml), run by
//! Halo's own per-tick rules, not the engine's stock frame
//! (`weapon::OwnFrame`):
//!
//! - `tick`: one weapon's 30 Hz update in the spec's order (the draw by
//!   the ready animation's frames, the trigger's idle counter and the
//!   rate-of-fire ramp, the error growing and decaying, the cone's draws
//!   in order, the shotgun's 15 pellets, magazines, reloads and their
//!   interruption, the shotgun's shell loop and chambering, heat and
//!   battery, the zoom levels, charging and the second trigger: the
//!   plasma pistol);
//! - `random`: the game-state generator and the 1026 directions;
//! - `flight`: a projectile's flight one update per tick (timers and
//!   arming, guidance and its wander, deceleration, gravity, range, hits
//!   and the material responses by the level's or the body's material
//!   type: disappear, detonate, reflect, overpenetrate, attach): no
//!   hitscan, a bullet's first update traces its whole tick's travel on
//!   the firing tick;
//! - `materials`: the level's material types from the map to the traces;
//! - `explode`: a detonation's area damage (falloff, sight by the core
//!   probes) and push;
//! - `grenades`: the counts carried and the throw on the grenade button;
//! - `pickups`: the netgame equipment on the item clock, powerups,
//!   grenades, ammunition from duplicates and the weapon swap;
//! - `systems`: the engine side: the holders' ticks (switches, the throw,
//!   the player melee by its key frame through the seam's
//!   `HaloHit::melee`), the projectiles (server: free, stuck in a body,
//!   or in a thrower's hand), their direct hits through
//!   `damage::HaloHit::direct`, detonations through `HaloHit::area` and
//!   the push through `movement::push`, the needles' supercombine, lag
//!   compensation of the firing tick, prediction (`HaloGun` and `Magazine`
//!   with the inventory, `HaloHands` with the player).
//!
//! **How each special rule is expressed.** Data: every number (rates,
//! errors, magazines, heat, battery, zoom, charging, animation frames,
//! projectiles, material responses, the detonation effects' damage
//! effects, the throw, the explosions' constants). Stock parts: `Weapon`
//! and `WeaponState` (selection and the deploy event), `Magazine` (the
//! rounds, which HUDs, `impulse 101` and the network already know: the
//! weapon keeps it equal to its own), `Zoomed` for the camera,
//! `WeaponEvent`. Halo's own parts: `HaloGun` (the state), `HaloWeapon`
//! (the numbers), `HaloHands` (the melee, the grenades, the throw),
//! `HaloProjectile`, `pickups::HaloItem`. One engine extension point:
//! `weapon::OwnFrame`.
//!
//! **The tick rate.** The spec's numbers are 30 Hz ticks and counted as
//! such (`systems::halo_ticks`): exact at Halo's own tick, where one Halo
//! tick runs per engine tick; under another game's tick (the party apps'
//! 64 Hz) a Halo tick runs on each engine tick that crosses a 30 Hz
//! boundary of simulated time, so the rates, reload and draw lengths stay
//! the spec's to within one engine tick (a press between two Halo ticks is
//! latched for the next). The other games keep seconds and compare them
//! with the engine's clock; integer tick counts are what Halo's rules are
//! (the idle counter, frame counts), so they are kept and paced instead.
//!
//! **For presentation** (no drawing here):
//! - `WeaponEvent`s on the weapon: `Deployed` (the engine's, on a switch;
//!   the draw lasts `params::Animations::ready` ticks), `Action("fire")`,
//!   `Action("fire_last")` (the magazine's last round), `Action("misfire")`,
//!   `Action("fire_charged")` (the plasma pistol's charged bolt),
//!   `Action("charging")`, `Action("charged")` (the first-person charged
//!   state), `DryFire` (the empty click, once a pull), `ReloadStarted` (a
//!   magazine, or the shotgun's first shell: its `enter`), `ShellInserting`
//!   (each further shell), `Action("shell_in")` (a shell went in),
//!   `Reloaded` (a magazine's rounds went in), `Action("chamber")` (the
//!   shotgun's pump), `Action("overheated")`, `Action("overheat_cleared")`,
//!   `Action("zoom_in")`, `Action("zoom_out")`, `Action("melee")` (a melee
//!   began), `Swing` (its hit tick: what it struck), `Action("throw_grenade")`
//!   (a throw began: the first-person and holder's throw animations);
//! - `systems::HaloFired`: each round with its trigger (1: the charged
//!   bolt), the firing effect entry (the plasma rifle's barrel), last round,
//!   misfire, origin, every projectile's direction and tracer flag, heat and
//!   battery after it, a guided projectile's target;
//! - `systems::HaloImpact` (server): each projectile segment, what it met
//!   (a character, the level), the normal, the material type, its tracer;
//! - `systems::HaloStuck` (server): a needle or plasma grenade stuck in a
//!   body or the level (the needles a body carries, the plasma grenade's
//!   detonation-started effect);
//! - `systems::HaloDetonation` (server): a rocket's, grenade's, flame's or
//!   needle's detonation, where, the supercombine, its damage effects (the
//!   explosions' effects and shock waves);
//! - `systems::HaloPush` (server): a push on a character;
//! - `HaloProjectile` entities (server): rockets, needles, flames, bolts
//!   and grenades in flight, held or stuck (the grenades are drawn and
//!   reach the clients as `fx::objects` shows them; the rest are flown
//!   again from the shot, `fx::shots`);
//! - `pickups::HaloItem` entities (server): items lying on the map, with
//!   what they are (weapon, grenade, powerup), shown by `fx::objects`;
//!   `pickups::HaloTaken`: one taken, by whom (its pickup sound);
//! - `tick::HaloGun` on the weapon: `heat`, `age` (the battery is
//!   `1 - age`), `overheated`, `overheat_exit`, `error`, `rate_of_fire`,
//!   `zoom`, `overcharged`, `HaloGun::charge` (the charged fraction), the
//!   magazine's state; `Zoomed` on the holder (the field of view: the nominal
//!   70 degrees over the magnification, as the camera's angle,
//!   `view::camera_fov`); `systems::HaloHands::melee`,
//!   `HaloHands::throw` (the grenade counts, the throw's frame).
//!
//! The autoaim (weapons.md 15, `aim`): the target a guided projectile
//! (the needle, the charged bolt) follows is picked for every player; the
//! shot bent toward it within the deviation cone (with the convergence
//! correction) only with `aim::Bend` on (`halo_autoaim 1`: a controller's
//! behaviour; off by default in every app);
//! `params::AimAssist::enabled` is the data's switch, on for all nine.
//! The magnetism (look stickiness) is left off.
//!
//! Known shortcuts: see `tick`, `flight`, `systems`, `grenades`,
//! `pickups`, `explode`, `random` and `aim`; the magnetism is not
//! implemented; inventory weapons only tick while
//! carried by a holder (a dropped plasma rifle does not cool on the
//! ground); the grenade button is the named action `grenade`, which the
//! front end binds (`ui::binds`, G in `DEFAULT_BINDS` and the bundle's).

pub mod aim;
pub mod explode;
pub mod flight;
pub mod grenades;
pub mod materials;
pub mod params;
pub mod pickups;
pub mod random;
pub mod systems;
pub mod tick;

use std::sync::Arc;

use bevy::prelude::*;
use chimera_sim::weapon::{
    Magazine, NetPart, OwnFrame, OwnZoom, WeaponFrame,
    data::WeaponDataAppExt,
    sync::CarriedAppExt,
};

use crate::core::{ActionAppExt, Predict, PredictedAppExt, SimSet};

pub use systems::{HaloFired, HaloHands, HaloImpact, HaloProjectile, HaloWeapon, halo_ticks};
pub use tick::HaloGun;

/// Metres per world unit (cache_file.md "The world").
pub const METERS_PER_WU: f32 = super::METERS_PER_UNIT;

/// The hook every Halo weapon's numbers are read by.
pub const WEAPON_HOOK: &str = "halo_ce:weapon";

pub const ASSAULT_RIFLE: &str = "halo_ce:assault_rifle";
pub const PISTOL: &str = "halo_ce:pistol";
pub const PLASMA_RIFLE: &str = "halo_ce:plasma_rifle";
pub const SHOTGUN: &str = "halo_ce:shotgun";
pub const SNIPER_RIFLE: &str = "halo_ce:sniper_rifle";
pub const ROCKET_LAUNCHER: &str = "halo_ce:rocket_launcher";
pub const NEEDLER: &str = "halo_ce:needler";
pub const PLASMA_PISTOL: &str = "halo_ce:plasma_pistol";
pub const FLAMETHROWER: &str = "halo_ce:flamethrower";

/// The nine held multiplayer weapons, in the order they register.
pub const GUNS: [&str; 9] = [ASSAULT_RIFLE, PISTOL, PLASMA_RIFLE, SHOTGUN, SNIPER_RIFLE, ROCKET_LAUNCHER, NEEDLER, PLASMA_PISTOL, FLAMETHROWER];

macro_rules! doc {
    ($name:literal) => {
        ($name, include_str!(concat!("../../../data/weapons/", $name)))
    };
}

/// The weapon documents, base first.
pub const DATA: &[(&str, &str)] = &[
    doc!("_base.toml"),
    doc!("assault_rifle.toml"),
    doc!("pistol.toml"),
    doc!("plasma_rifle.toml"),
    doc!("shotgun.toml"),
    doc!("sniper_rifle.toml"),
    doc!("rocket_launcher.toml"),
    doc!("needler.toml"),
    doc!("plasma_pistol.toml"),
    doc!("flamethrower.toml"),
];

/// Its guns' names (data/display_names.toml).
pub const DISPLAY_NAMES: &str = include_str!("../../../data/display_names.toml");

impl NetPart for HaloGun {
    type Net = HaloGun;
    fn net(&self) -> Self::Net {
        self.clone()
    }
    fn set_net(&mut self, net: Self::Net) {
        *self = net;
    }
}

/// Each weapon's numbers by id, read from `DATA` as the game loads them
/// (bases merged): for tests and tools. Bad built-in data panics.
pub fn guns() -> Vec<(String, params::Gun)> {
    let set = chimera_sim::weapon::data::WeaponSet::parse(DATA, &[]).unwrap_or_else(|e| panic!("Halo weapon data: {e}"));
    GUNS.iter()
        .map(|id| {
            let d = set.get(id).unwrap_or_else(|| panic!("{id} in the data"));
            let table = d.hooks.get(WEAPON_HOOK).cloned().unwrap_or_else(|| panic!("{id}: no {WEAPON_HOOK}"));
            let gun: params::Gun = table.try_into().unwrap_or_else(|e| panic!("{id}: {e}"));
            (id.to_string(), gun)
        })
        .collect()
}

/// One weapon's numbers (`guns`).
pub fn gun(id: &str) -> params::Gun {
    guns().into_iter().find(|(i, _)| i == id).map(|(_, g)| g).unwrap_or_else(|| panic!("no Halo weapon {id}"))
}

/// A weapon built from its data: its numbers, a new weapon's state, the
/// stock frame kept off it, its zoom its own, and its rounds as the
/// engine's `Magazine` where it has a magazine of rounds.
fn build(e: &mut EntityWorldMut, gun: &params::Gun) {
    e.insert((HaloWeapon(Arc::new(gun.clone())), HaloGun::new(gun), OwnFrame, OwnZoom));
    if let Some(m) = gun.magazine.filter(|m| m.rounds_loaded_maximum > 0) {
        let (loaded, reserve) = m.new_rounds();
        e.insert(Magazine {
            clip: loaded as u32,
            size: m.rounds_loaded_maximum as u32,
            reserve: reserve as u32,
            reserve_max: m.rounds_total_maximum as u32,
            reload_time: gun.animations.reload_empty.unwrap_or(0) as f32 / 30.0,
            reload_while_held: false,
            auto_reload: false,
        });
    }
}

/// What a gun game's power curve reads of a Halo weapon
/// (`chimera_sim::minigame::power`): a projectile's damage on an
/// unshielded body (the effect's mean at the scale its speed left at the
/// distance, its cyborg modifier, a Halo point 1/75 of the baseline's
/// health, as the damage seam counts it), the pellets, the ticks between
/// shots held at full rate (a battery just touched), the cone at error 0
/// and as held, the magazine and its reload; nothing beyond the
/// projectile's reach.
fn power(p: &chimera_sim::minigame::power::HookParams, at: &chimera_sim::minigame::power::ReadAt, s: &mut chimera_sim::minigame::power::GunStats) {
    let Ok(g) = p.clone().try_into::<params::Gun>() else { return };
    let t = &g.trigger;
    let pr = &g.projectile;
    let effects = super::vitality::DamageEffects::load();
    // The hit's damage: the impact's, else what a stuck projectile deals
    // (a needle), else its detonation's at the centre (a rocket, a flame).
    let named = |n: &str| params::effect_id(n).and_then(|e| effects.get(e).cloned()).filter(|f| f.lower + f.upper[1] > 0.0);
    let Some(fx) = named(&pr.impact_damage)
        .or_else(|| pr.attached_damage.as_deref().and_then(named))
        .or_else(|| pr.detonation.iter().find_map(|n| named(n)))
    else {
        return;
    };
    // The speed left at the distance (WU): full before the deceleration
    // starts, then at a constant rate down to the final speed.
    let d = at.distance;
    let (v0, vf, a) = (pr.initial_velocity, pr.final_velocity, pr.deceleration(pr.air_damage_range));
    let (_, step) = pr.deceleration_timer(pr.air_damage_range);
    let start = if step > 0.0 { ((1.0 / step).ceil() - 1.0) * v0 } else { 0.0 };
    let v = if a > 0.0 { (v0 * v0 - 2.0 * a * (d - start).max(0.0)).max(vf * vf).sqrt() } else { v0 };
    let slowed_end = a > 0.0 && pr.maximum_range == 0.0 && d > start + (v0 * v0 - vf * vf) / (2.0 * a) + 1.0;
    let beyond = (pr.maximum_range > 0.0 && d > pr.maximum_range) || slowed_end;
    let scale = pr.direct_hit_scale(v);
    let modifier = fx.modifiers.get(CYBORG_BODY).copied().unwrap_or(1.0);
    s.damage = if beyond { 0.0 } else { fx.amount(scale, 0.5) * modifier / 75.0 };
    s.pellets = t.projectiles_per_shot.max(1) as u32;
    let required = t.required_ticks(1.0, t.age_per_round, g.age.rate_of_fire_penalty);
    s.interval = required.ceil().max(1.0) / 30.0;
    s.latency = 0.0;
    s.spread = t.theta_max(0.0).tan();
    s.follow = Some(t.theta_max(at.firing).tan());
    let m = g.magazine.filter(|m| m.rounds_loaded_maximum > 0);
    s.clip = m.map_or(0, |m| m.rounds_loaded_maximum as u32);
    s.reload = g.animations.reload_empty.unwrap_or(0) as f32 / 30.0;
}

/// The cyborg body's material type (the Spartan's body; projectiles.md).
const CYBORG_BODY: usize = 21;

/// Halo's weapons: the hook, the data, the systems and their prediction.
pub struct HaloWeaponsPlugin;

impl Plugin for HaloWeaponsPlugin {
    fn build(&self, app: &mut App) {
        use chimera_core::character::CharacterPartsExt;
        chimera_sim::minigame::power::PowerAppExt::add_power_reader(app, WEAPON_HOOK, power);
        app.register_weapon_hook::<params::Gun>(WEAPON_HOOK, |e, _, p| build(e, p));
        if let Err(e) = app.add_weapon_data(DATA) {
            panic!("Halo weapon data: {e}");
        }
        chimera_sim::weapon::names::DisplayNamesAppExt::add_display_names(app, "halo_ce data/display_names.toml", DISPLAY_NAMES);
        app.intent_action(systems::MELEE_ACTION)
            .intent_action(systems::ZOOM_ACTION)
            .intent_action(grenades::GRENADE_ACTION)
            .intent_action(grenades::SWITCH_ACTION);
        app.insert_resource(grenades::GrenadeData::load()).init_resource::<pickups::ItemClock>().init_resource::<aim::Bend>();
        chimera_core::console::ConsoleAppExt::console_command(
            app,
            "halo_autoaim",
            "halo_autoaim <0|1>: bend Halo's shots toward the autoaim target and the look point (the original's behaviour, for a controller; off by default: mouse and keyboard). The guided projectiles' target is picked either way.",
            |w, args| {
                match args.first().map(String::as_str) {
                    Some("1") => w.insert_resource(aim::Bend(true)),
                    Some("0") => w.insert_resource(aim::Bend(false)),
                    _ => return Ok(Some(format!("halo_autoaim {}", u8::from(w.get_resource::<aim::Bend>().is_some_and(|b| b.0))))),
                }
                Ok(None)
            },
        );
        app.on_every_character::<HaloHands>();
        app.add_message::<HaloFired>()
            .add_message::<HaloImpact>()
            .add_message::<super::damage::HaloHit>()
            .add_message::<super::damage::HaloHurt>()
            .add_message::<systems::HaloDetonation>()
            .add_message::<systems::HaloStuck>()
            .add_message::<systems::HaloPush>()
            .add_message::<pickups::HaloTaken>()
            .add_systems(Predict::Weapons, systems::run.before(WeaponFrame))
            .add_systems(
                FixedUpdate,
                (
                    (materials::follow, systems::cut_throws, systems::fly).chain().in_set(SimSet::Weapons).before(WeaponFrame),
                    (systems::settle_new, systems::apply_pushes).chain().after(SimSet::Weapons).before(super::damage::deliver_hits),
                    (systems::camo_on_fire, systems::unzoom_when_hurt).after(SimSet::Weapons),
                    pickups::systems().in_set(SimSet::Weapons).after(WeaponFrame),
                )
                    .run_if(crate::core::authoritative),
            )
            .predicted_part::<HaloGun>()
            .predicted_net::<HaloHands>();
    }
}
