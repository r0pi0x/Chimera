//! What a game type does to the objects a game creates
//! (specs/halo_ce/multiplayer_rules.md 4.6, 5.3): the weapon set's
//! substitution of a weapon (`substitute`), of a grenade or powerup item
//! (`substitute_item`), the starting grenade counts (`starting_grenades`)
//! and the starting-grenade cap by the players a game began with
//! (`grenade_cap`). Pure functions over the weapons' ids and the
//! pickups' `Item`; the rules call them at a spawn, the pickups' item clock
//! through `ItemRules`.

use bevy::prelude::*;
use blam_formats::game_type::{GameFlags, Rules, WeaponSet};

use super::super::weapons::{
    ASSAULT_RIFLE, FLAMETHROWER, NEEDLER, PISTOL, PLASMA_PISTOL, PLASMA_RIFLE, ROCKET_LAUNCHER, SHOTGUN, SNIPER_RIFLE,
    grenades::{FRAG, PLASMA},
    pickups::{Item, Powerup},
};

/// The table's columns, in section 5.3's order (the flamethrower's
/// column is never reached: it is a rocket launcher before the table).
const COLUMNS: [&str; 9] = [ASSAULT_RIFLE, FLAMETHROWER, NEEDLER, PISTOL, PLASMA_PISTOL, PLASMA_RIFLE, ROCKET_LAUNCHER, SHOTGUN, SNIPER_RIFLE];

const AR: &str = ASSAULT_RIFLE;
const PP: &str = PLASMA_PISTOL;
const PR: &str = PLASMA_RIFLE;
const RL: &str = ROCKET_LAUNCHER;
const SG: &str = SHOTGUN;
const SR: &str = SNIPER_RIFLE;
const ND: &str = NEEDLER;
const PI: &str = PISTOL;

/// Section 5.3's table, a row per weapon set (sets 10 and 12 are set 0's).
fn row(set: WeaponSet) -> [&'static str; 9] {
    match set {
        WeaponSet::Normal | WeaponSet::NoGrenades | WeaponSet::Classic => [AR, RL, ND, PI, PP, PR, RL, SG, SR],
        WeaponSet::Pistols => [PI, PP, PP, PP, PI, PP, PP, PI, PI],
        WeaponSet::Rifles => [AR, PR, PR, PR, AR, PR, PR, AR, AR],
        WeaponSet::PlasmaWeapons => [PR, PR, PP, PP, PP, PR, PR, PR, PR],
        WeaponSet::Sniper => [SR, SR, SR, PI, SR, SR, SR, SR, SR],
        WeaponSet::NoSniping => [AR, RL, ND, AR, PP, PR, RL, SG, SG],
        WeaponSet::RocketLaunchers | WeaponSet::Heavy => [RL; 9],
        WeaponSet::Shotguns => [SG; 9],
        WeaponSet::ShortRange => [SG, RL, SG, SG, PP, PR, RL, SG, SG],
        WeaponSet::Human => [AR, RL, AR, PI, PI, AR, RL, SG, SR],
        WeaponSet::Covenant => [PR, ND, ND, PP, PP, PR, ND, ND, ND],
    }
}

/// Weapon `id` as the weapon set makes it: the flamethrower first a
/// rocket launcher, then the set's column; a weapon not in the table is
/// unchanged.
pub fn substitute(set: WeaponSet, id: &'static str) -> &'static str {
    let id = if id == FLAMETHROWER { ROCKET_LAUNCHER } else { id };
    match COLUMNS.iter().position(|c| *c == id) {
        Some(i) => row(set)[i],
        None => id,
    }
}

/// The starting-grenade cap of a game that began with `players` (4.6: 1
/// with 9 or more, 2 with 5 to 8, else 4).
pub fn grenade_cap(players: u32) -> u8 {
    match players {
        9.. => 1,
        5..=8 => 2,
        _ => 4,
    }
}

/// The grenades a unit starts with (4.6 step 3): frag 4, plasma 0; the
/// matched starting equipment entry's flags (`equipment_flags`: bit 0 no
/// grenades, bit 1 plasma for frag); infinite grenades both at the cap;
/// the weapon set's (plasma, human, no grenades); each limited to the cap.
pub fn starting_grenades(r: &Rules, players: u32, equipment_flags: Option<u32>) -> [u8; 2] {
    let cap = grenade_cap(players) as i32;
    let (mut frag, mut plasma) = (4i32, 0i32);
    if let Some(f) = equipment_flags {
        if f & 1 != 0 {
            frag = 0;
            plasma = 0;
        }
        if f & 2 != 0 {
            plasma += frag;
            frag = 0;
        }
    }
    let infinite = r.flags.contains(GameFlags::INFINITE_GRENADES);
    if infinite {
        frag = cap;
        plasma = cap;
    }
    match r.weapon_set {
        WeaponSet::PlasmaWeapons => {
            plasma += frag;
            frag = 0;
        }
        WeaponSet::Human => {
            frag += plasma;
            plasma = 0;
        }
        WeaponSet::NoGrenades if !infinite => {
            frag = 0;
            plasma = 0;
        }
        _ => {}
    }
    let mut out = [0u8; 2];
    out[FRAG] = frag.clamp(0, cap) as u8;
    out[PLASMA] = plasma.clamp(0, cap) as u8;
    out
}

/// An item as the game type makes it when the item clock creates it (5.3):
/// a weapon by the weapon set; a grenade by the set (plasma: both plasma;
/// human: both frag; no grenades: none), none with infinite grenades, and
/// in a game that began with 5 or more players kept only when `draw` (a
/// uniform number in [0, 1)) is at most 0.55 (9 or more: 0.3); an
/// overshield none with no shields, camouflage none with invisible
/// players. None: the entry creates nothing this period.
pub fn substitute_item(r: &Rules, players: u32, item: Item, draw: impl FnOnce() -> f32) -> Option<Item> {
    match item {
        Item::Weapon { id, loaded, reserve, age } => {
            let to = substitute(r.weapon_set, id);
            if to == id {
                Some(Item::Weapon { id, loaded, reserve, age })
            } else {
                // A new weapon of the other kind, with its own rounds.
                super::super::weapons::pickups::new_weapon(to)
            }
        }
        Item::Grenade(k) => {
            let k = match r.weapon_set {
                WeaponSet::PlasmaWeapons => PLASMA,
                WeaponSet::Human => FRAG,
                WeaponSet::NoGrenades => return None,
                _ => k,
            };
            if r.flags.contains(GameFlags::INFINITE_GRENADES) {
                return None;
            }
            let keep = match players {
                9.. => 0.30,
                5..=8 => 0.55,
                _ => return Some(Item::Grenade(k)),
            };
            (draw() <= keep).then_some(Item::Grenade(k))
        }
        Item::Powerup(Powerup::Overshield) if r.flags.contains(GameFlags::NO_SHIELDS) => None,
        Item::Powerup(Powerup::Camouflage) if r.flags.contains(GameFlags::INVISIBLE_PLAYERS) => None,
        other => Some(other),
    }
}

/// What the pickups' item clock asks of the game type in play (its
/// presence: a Halo match runs): the rules and the players its game began
/// with, and the base game type whose entries are eligible.
#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ItemRules {
    pub rules: Rules,
    pub players: u32,
    pub game: blam_formats::scenario::GameType,
}
