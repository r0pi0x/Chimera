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
//! (both gun games) a rung's grenade comes back once thrown, so the count
//! is kept at the frag's maximum (4) at every Halo tick (as the ladder's
//! `refill` keeps a `Throwable` at its `max`); without it a new life, or
//! the arrival on the rung, gives the frag's start count once. No plasma
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
//! Known shortcuts: the power curve reads the pistol's numbers for it
//! (its band is the class's; the score only orders a band's members).

use bevy::prelude::*;
use serde::Deserialize;

use super::{PISTOL, grenades, systems::FRAG_GRENADE};

/// The hook its weapon document names.
pub const HOOK: &str = "halo_ce:grenade_rung";

/// The rung's weapon id (the frag grenade's).
pub const FRAG_RUNG: &str = FRAG_GRENADE;

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
}

/// On a grenade rung's weapon: the kind it throws, and whether the start
/// count was given (without the ladder's refill).
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct GrenadeRung {
    pub kind: usize,
    pub given: bool,
}

/// The hook: the rung's part and the engine's fact.
pub fn register(app: &mut App) {
    chimera_sim::weapon::data::WeaponDataAppExt::register_weapon_hook::<RungParams>(app, HOOK, |e, _, p| {
        let kind = if p.kind == "plasma" { grenades::PLASMA } else { grenades::FRAG };
        e.insert((
            GrenadeRung { kind, given: false },
            chimera_sim::weapon::AttackAction { action: p.action.clone() },
        ));
    });
}
