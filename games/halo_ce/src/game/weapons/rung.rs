//! Halo's grenade rung in a party ladder (the user's decision,
//! 2026-11-15): one more part Halo lends (`parts::PARTS`, class grenade,
//! id `systems::FRAG_GRENADE`), so an automatic ladder deals it in its
//! grenade band as it deals the other games' grenades, and holding it
//! makes the character Halo's as any Halo rung does. Its data
//! (data/weapons/frag_grenade.toml) is the pistol's (`base`: view and held
//! models, animations, crosshair, HUD) with no round loaded, none in
//! reserve and none to be had, and its `halo_ce:grenade_rung` hook
//! (`RungParams`): the grenade kind it throws and the action that throws
//! it; and the party's rung hint, opt-in in its document (`rung_hint`:
//! the grenade action, "throw"), which no other Halo weapon declares.
//!
//! What the holder has (`systems::run`, while a weapon with `GrenadeRung`
//! is carried): the empty pistol (its rounds kept at 0 loaded and 0 in
//! reserve whatever writes its `Magazine`: nothing refills it; the fire
//! button dry-fires as an empty pistol does), frag grenades by the
//! ladder's grenade-band supply rule whatever `PartyGrenades` says (that
//! switch governs the other Halo rungs): with the ladder's `refill_ammo`
//! (both gun games) the supply is TIMED, the rung's own (user, 2026-11-15:
//! a thrown grenade is not given back at once, so it cannot be spammed):
//! the player arrives, and every new life starts, with `max` frags (4); a
//! throw spends one as any throw does; while the count is below `max` the
//! server counts Halo ticks (`ticks`, from the 30 Hz tick counting, so the
//! same seconds at any engine tick rate) and gives one frag back every
//! `refill_seconds` (5.0, the hook's data), one per interval from the
//! moment the count dropped below `max` (not from each throw), never above
//! `max`. The timer is the server's alone: the count (`HaloHands`) is
//! what replicates, so a client's HUD follows it. Without `refill_ammo` a
//! new life, or the arrival on the rung, gives the frag's start count once.
//! No plasma
//! grenades; the rotate-grenades button does nothing. The throw is
//! Halo's own on the grenade button (`grenades::Throw`), with the throw
//! animation the pistol's view model plays.
//!
//! What it states to the engine (as `systems::battery_rounds` does for the
//! battery guns): its `Magazine`'s rounds loaded are the frags it carries
//! (what a bot reads as what it can fire), a throw's release is a shot made
//! (`WeaponEventKind::ShotMade`), and its attack is the named grenade
//! action (`weapon::AttackAction`: a bot presses it where it would press
//! fire). Halo's HUD reads the gun's
//! own rounds (0 and 0: the empty pistol's ammunition block) and the
//! grenade counts (the frag plate with its count).
//!
//! **The Hunter's rung** (`HUNTER_RUNG`, the owner's wish, 2026-11-16):
//! the Hunter's arm cannon (data/weapons/hunter_fuel_rod.toml, every
//! number its tag's on d20) as one more gun Halo lends (`parts::PARTS`,
//! class launcher): its holder always wears the Hunter
//! (`campaign::CampaignData::own_gun`: data/campaign.toml gives the gun to
//! the Hunter alone), whatever `halo_ce:campaign_bodies` says; a drawn
//! ladder deals it only with that setting on (`campaign::own_guns_drawn`:
//! off, it is a draw exclusion), the sandbox lists it always. The weapon
//! is Halo's ordinary one, run by the same rules as the others (its
//! charge, its rod, the rod's detonation: `tick`, `flight`, `explode`), so
//! a bot fires it as it fires any Halo gun (it fires on the press: a rate
//! above 0 with one trigger; a shot is a `WeaponEventKind::ShotMade`). The
//! rod leaves from the eye's line moved by the trigger's first-person
//! offset, which is 0, as every Halo gun's shot does: not from the
//! Hunter's `primary trigger` marker on its right forearm (the spec leaves
//! open which the game uses: characters.md, Open questions 2), so it
//! comes out of the middle of the view. It has no model: in third person
//! the Hunter's own arm is the cannon (nothing is held), in first person
//! the body's own arm is drawn (`campaign::first_person`).
//!
//! Known shortcuts: the power curve reads the pistol's numbers for the
//! grenade rung (its band is the class's; the score only orders a band's
//! members); the Hunter's rod is drawn in flight only where its
//! projectile's looks are loaded (the donor map has none: `fx::shots`), so
//! on another game's map its shot shows its detonation's damage but no
//! rod, no muzzle flash and no impact effect.

use bevy::prelude::*;
use serde::Deserialize;

use super::{PISTOL, grenades, systems::FRAG_GRENADE};

/// The hook its weapon document names.
pub const HOOK: &str = "halo_ce:grenade_rung";

/// The rung's weapon id (the frag grenade's).
pub const FRAG_RUNG: &str = FRAG_GRENADE;

/// The Hunter's rung: its arm cannon (module doc).
pub const HUNTER_RUNG: &str = "halo_ce:hunter_fuel_rod";

/// The rung weapons whose looks are another gun's (view and held models):
/// (rung weapon, gun).
pub const LOOKS: [(&str, &str); 1] = [(FRAG_RUNG, PISTOL)];

/// The hook's parameters.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct RungParams {
    /// "frag" or "plasma".
    pub kind: String,
    /// The named action that throws.
    pub action: String,
    /// The most frags the rung's holder has (the frag's own maximum caps
    /// it).
    pub max: u8,
    /// Seconds of simulation time between one frag coming back and the
    /// next, while the count is below `max` (the party ladder's refill).
    pub refill_seconds: f32,
}

/// On a grenade rung's weapon: the kind it throws, and whether the start
/// count was given (without the ladder's refill).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrenadeRung {
    pub kind: usize,
    pub given: bool,
    /// The most frags carried.
    pub max: u8,
    /// Halo ticks (30 Hz) between two frags coming back (at least 1).
    pub refill_ticks: u32,
    /// Halo ticks counted toward the next frag: the server's alone (the
    /// count is what replicates); 0 while the count is full.
    pub ticks: u32,
}

/// The hook: the rung's part and the engine's fact.
pub fn register(app: &mut App) {
    chimera_sim::weapon::data::WeaponDataAppExt::register_weapon_hook::<RungParams>(app, HOOK, |e, _, p| {
        let kind = if p.kind == "plasma" { grenades::PLASMA } else { grenades::FRAG };
        e.insert((
            GrenadeRung {
                kind,
                given: false,
                max: p.max,
                refill_ticks: ((p.refill_seconds as f64 / crate::game::TICK_INTERVAL).round() as u32).max(1),
                ticks: 0,
            },
            chimera_sim::weapon::AttackAction { action: p.action.clone() },
        ));
    });
}
