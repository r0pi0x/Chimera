//! Halo's weapons (specs/halo_ce/weapons.md, projectiles.md; M2 of
//! docs/plans/active/halo-ce.md): the assault rifle, the pistol, the
//! plasma rifle and the shotgun as data (data/weapons/*.toml: every
//! number in the `halo_ce:weapon` hook's table, `params`), run by Halo's
//! own per-tick rules, not the engine's stock frame (`weapon::OwnFrame`):
//!
//! - `tick`: one weapon's 30 Hz update in the spec's order (the draw by
//!   the ready animation's frames, the trigger's idle counter and the
//!   rate-of-fire ramp, the error growing and decaying, the cone's draws
//!   in order, the shotgun's 15 pellets, magazines, reloads and their
//!   interruption, the shotgun's shell loop and chambering, the plasma
//!   rifle's heat and battery, the pistol's zoom);
//! - `random`: the game-state generator and the 1026 directions;
//! - `flight`: a projectile's flight (deceleration, the slowed bolt's end,
//!   gravity, range, hits) one update per tick: no hitscan, a bullet's
//!   first update traces its whole tick's travel on the firing tick;
//! - `systems`: the engine side: the holders' ticks (switches, the player
//!   melee by its key frame through the seam's `HaloHit::melee`), the
//!   projectiles (server) and their direct hits through
//!   `damage::HaloHit::direct` (the amount between the effect's bounds by
//!   the speed left and a draw), lag compensation of the firing tick,
//!   prediction (`HaloGun` and `Magazine` with the inventory, `HaloHands`
//!   with the player).
//!
//! **How each special rule is expressed.** Data: every number (rates,
//! errors, magazines, heat, battery, zoom, animation frames, projectiles,
//! responses). Stock parts: `Weapon` and `WeaponState` (selection and the
//! deploy event), `Magazine` (the rounds, which HUDs, `impulse 101` and
//! the network already know: the weapon keeps it equal to its own),
//! `Zoomed` for the camera, `WeaponEvent`. Halo's own parts: `HaloGun`
//! (the state), `HaloWeapon` (the numbers), `HaloHands` (the melee),
//! `HaloProjectile`. One engine extension point: `weapon::OwnFrame`.
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
//!   `DryFire` (the empty click, once a pull), `ReloadStarted` (a magazine,
//!   or the shotgun's first shell: its `enter`), `ShellInserting` (each
//!   further shell), `Action("shell_in")` (a shell went in), `Reloaded`
//!   (a magazine's rounds went in), `Action("chamber")` (the shotgun's
//!   pump), `Action("overheated")`, `Action("overheat_cleared")`,
//!   `Action("zoom_in")`, `Action("zoom_out")`, `Action("melee")` (a melee
//!   began) and `Swing` (its hit tick: what it struck);
//! - `systems::HaloFired`: each round with its trigger, the firing effect
//!   entry (the plasma rifle's barrel), last round, misfire, origin, every
//!   projectile's direction and tracer flag, heat and battery after it;
//! - `systems::HaloImpact` (server): each projectile segment, what it met
//!   (a character, the level), the normal, its tracer;
//! - `tick::HaloGun` on the weapon: `heat`, `age` (the battery is
//!   `1 - age`), `overheated`, `overheat_exit`, `error`, `rate_of_fire`,
//!   `zoom`, the magazine's state; `Zoomed` on the holder (the field of
//!   view, 70 degrees over the magnification); `systems::HaloHands::melee`.
//!
//! Aim assist is a controller feature the port leaves off (weapons.md
//! 15): its numbers are data and `params::AimAssist::enabled` is the
//! switch, false for the four; with it off, only the convergence
//! correction would remain, which is the identity while shots start on
//! the camera ray (as here).
//!
//! Ammunition at spawn and from a duplicate weapon on the ground
//! (multiplayer_rules.md 4.6, 5.5): `params::Magazine::new_rounds`,
//! `params::take_ammunition`; the pickups themselves are M3's.
//!
//! Known shortcuts: see `tick`, `flight`, `systems` and `random`; aim
//! assist and magnetism are not implemented beyond the switch; inventory
//! weapons only tick while carried by a holder (a dropped plasma rifle
//! does not cool on the ground).

pub mod flight;
pub mod params;
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

/// The four, in the order they register.
pub const GUNS: [&str; 4] = [ASSAULT_RIFLE, PISTOL, PLASMA_RIFLE, SHOTGUN];

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
    let Some(fx) = params::effect_id(&pr.impact_damage).and_then(|e| effects.get(e).cloned()) else { return };
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
        app.intent_action(systems::MELEE_ACTION).intent_action(systems::ZOOM_ACTION);
        app.on_every_character::<HaloHands>();
        app.add_message::<HaloFired>()
            .add_message::<HaloImpact>()
            .add_message::<super::damage::HaloHit>()
            .add_message::<super::damage::HaloHurt>()
            .add_systems(Predict::Weapons, systems::run.before(WeaponFrame))
            .add_systems(
                FixedUpdate,
                (
                    systems::fly.in_set(SimSet::Weapons).before(WeaponFrame),
                    (systems::camo_on_fire, systems::unzoom_when_hurt).after(SimSet::Weapons),
                )
                    .run_if(crate::core::authoritative),
            )
            .predicted_part::<HaloGun>()
            .predicted_net::<HaloHands>();
    }
}
