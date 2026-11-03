//! HL2:DM's inventory rules (specs/hl2dm/weapons.md 1-4, 7.6, 8-9).
//!
//! On every HL2:DM weapon (the guns' part, wherever they are):
//!
//! - `Hl2Item` (`hl2dm:inventory`): the script's facts the rules read:
//!   bucket position, `default_clip`, `autoswitchto`/`autoswitchfrom`, the
//!   ammo type; with a clip, `Hl2Holster`.
//! - **Holstered auto-reload** (`holster_reload`): a carried weapon not in
//!   hand for more than `sk_auto_reload_time` (3 s) since it was last put
//!   away fills its clip from the reserve with no animation, and counts
//!   from then again; a weapon never yet in hand counts from time 0, so it
//!   tops up on the next tick.
//!
//! HL2:DM's game (`Hl2WeaponsPlugin`, its own app):
//!
//! - **The spawn loadout** (`arm`): the starting weapons (pistol, SMG1,
//!   frag, gravity gun) with the melee weapon of the player's model kind first
//!   (`melee_for`: a stunstick for the Combine and metro police, a crowbar
//!   for citizens; without a model, the DM fallback `models/
//!   combine_soldier.mdl`), the reserves of section 3 (Pistol 150, SMG1 45,
//!   1 grenade, Buckshot 6, 357 6: each type's count waits in `Hl2Ammo` for
//!   a weapon that uses it; the frag's 1 and the grant's make 2), the
//!   gravity gun in hand and the SMG1 the last weapon.
//! - **Ammo per player, not per weapon**: a weapon carries only its clip
//!   (a map's AR2 also the 30 over its clip, `default_clip` 60); taken new
//!   it brings the player's count of its type into its reserve
//!   (`on_pickup`), plus what it carried; dropped it keeps its clip only
//!   (`loose_ammo`).
//! - **The next best weapon** (`next_best`): a weapon in hand with no ammo
//!   at all whose timers have passed, with no button or attack held,
//!   switches to the first other one of the same weight with ammo, else
//!   the heaviest with ammo (melee weapons always have it); weapons with
//!   `autoswitchto` 0 and a weapon in hand with `autoswitchfrom` 0 are
//!   never part of it; the empty weapon's next primary goes 0.3 s on.
//! - Pickup and drop (`weapon::drop::PickupRules`): touchable 1 s after a
//!   drop, gone 31 s after it, several weapons to a bucket, the dead drop
//!   the weapon in hand (melee weapons never: `undroppable`), touching a
//!   weapon of a kind carried takes its rounds and removes it.
//!
//! Known shortcuts: `cl_defaultweapon` isn't read (the gravity gun ends
//! in hand as with the default); a dropped weapon falls
//! where its owner died (the death force and the 400 in/s cap, the spin,
//! aren't modelled); `mp_weaponstay` and the placed weapons' 20 s respawn
//! are M7's (placed weapons are put back at a round restart, Chimera's
//! rule); the selection commands (`hud_fastswitch`, `phys_swap`) are the
//! shell's slot keys and `lastinv` (M9's HUD); the ammo pickup switch
//! (section 2, "Switch on ammo pickup") and the "ammo denied" notice wait
//! for ammo items (M7).

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use chimera_sim::weapon::{
    Inventory, Magazine, Melee, NetPart, Weapon, WeaponState,
    drop::{Loose, PickedUp},
    equip::MapWeapon,
    spawn_weapon,
};

use crate::core::{Died, Health, Intent, SimClock};

/// `sk_auto_reload_time`: seconds holstered before a clip tops up.
pub const AUTO_RELOAD_TIME: f64 = 3.0;

/// The old weapon's next primary after an empty auto-switch.
pub const AUTOSWITCH_HOLD: f64 = 0.3;

/// `hl2dm:inventory`'s parameters: the script's keys the rules read.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InventoryParams {
    pub position: u8,
    #[serde(default)]
    pub default_clip: Option<u32>,
    #[serde(default = "yes")]
    pub autoswitchto: bool,
    #[serde(default = "yes")]
    pub autoswitchfrom: bool,
}

fn yes() -> bool {
    true
}

/// A weapon's script facts (`hl2dm:inventory`).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hl2Item {
    /// `bucket_position` (the bucket is the weapon's slot).
    pub position: u8,
    /// `default_clip`: a new weapon's rounds (None: its clip size).
    pub default_clip: Option<u32>,
    pub autoswitchto: bool,
    pub autoswitchfrom: bool,
    /// Its primary ammo type (None: a melee weapon).
    pub ammo: Option<&'static str>,
    /// `weight`.
    pub weight: i32,
}

impl Hl2Item {
    /// What a weapon placed on the map holds beyond a full clip (the
    /// AR2's 30).
    pub fn spare(&self, clip: u32) -> u32 {
        self.default_clip.map_or(0, |d| d.saturating_sub(clip))
    }
}

/// When a carried weapon was last put away (or last in hand).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Hl2Holster {
    pub since: f64,
}

impl NetPart for Hl2Holster {
    type Net = f64;
    fn net(&self) -> f64 {
        self.since
    }
    fn set_net(&mut self, since: f64) {
        self.since = since;
    }
}

/// The holstered auto-reload (weapons.md 7.6), after the weapon frame.
pub fn holster_reload(
    owners: Query<&Inventory, Without<crate::map::interp::NetDrawn>>,
    mut weapons: Query<(Entity, &Weapon, &mut Hl2Holster, &mut Magazine)>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for (e, weapon, mut holster, mut mag) in &mut weapons {
        let Some(inv) = weapon.owner.and_then(|o| owners.get(o).ok()) else { continue };
        if inv.active == Some(e) {
            holster.since = now;
            continue;
        }
        if now - holster.since <= AUTO_RELOAD_TIME {
            continue;
        }
        holster.since = now;
        let n = (mag.size - mag.clip).min(mag.reserve);
        if n > 0 {
            mag.clip += n;
            mag.reserve -= n;
        }
    }
}

/// A player model's kind (rules.md "Models": the path names it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelKind {
    Citizen,
    MetroPolice,
    Combine,
}

impl ModelKind {
    /// The kind of model `path` (`models/humans/group03/male_01.mdl`).
    pub fn of(path: &str) -> Option<ModelKind> {
        let p = path.to_ascii_lowercase().replace('\\', "/");
        if p.contains("models/human") {
            Some(ModelKind::Citizen)
        } else if p.contains("police") {
            Some(ModelKind::MetroPolice)
        } else if p.contains("combine") {
            Some(ModelKind::Combine)
        } else {
            None
        }
    }
}

/// The DM fallback model (rules.md "Models in DM").
pub const DEFAULT_MODEL: &str = "models/combine_soldier.mdl";

/// A player's HL2:DM model (its path); none: `DEFAULT_MODEL`: its body
/// (`bodies`, replicated) and the spawn's melee weapon.
#[derive(Component, Reflect, Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[reflect(Component)]
pub struct Hl2Model(pub String);

/// The melee weapon a model of `kind` spawns with.
pub fn melee_for(kind: Option<ModelKind>) -> &'static str {
    match kind {
        Some(ModelKind::Citizen) => super::CROWBAR,
        _ => super::STUNSTICK,
    }
}

/// Each ammo type's count with no weapon of it to hold it (`arm`,
/// `on_pickup`).
#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct Hl2Ammo(pub Vec<(String, u32)>);

impl Hl2Ammo {
    /// Take all of `ammo`'s count.
    pub fn take(&mut self, ammo: &str) -> u32 {
        match self.0.iter().position(|(a, _)| a == ammo) {
            Some(i) => self.0.remove(i).1,
            None => 0,
        }
    }
}

/// The spawn's reserves (weapons.md 3: Pistol 255 clamped to 150, SMG1
/// 45, 1 grenade, Buckshot 6, 357 6).
pub const SPAWN_AMMO: &[(&str, u32)] = &[
    ("hl2dm:pistol", 150),
    ("hl2dm:smg1", 45),
    ("hl2dm:grenade", 1),
    ("hl2dm:buckshot", 6),
    ("hl2dm:357", 6),
];

/// On a player whose life's loadout is done.
#[derive(Component, Clone, Copy, Debug)]
pub struct Hl2Armed;

/// A player with the starting weapons and no loadout yet: its melee
/// weapon first, the reserves, the SMG1 in hand and the pistol last.
#[allow(clippy::type_complexity)]
pub fn arm(
    players: Query<(Entity, &Inventory, Option<&Hl2Model>), (With<Health>, Without<Hl2Armed>, Without<crate::map::interp::NetDrawn>)>,
    mut commands: Commands,
) {
    for (e, inv, model) in &players {
        if inv.weapons.is_empty() {
            continue;
        }
        let kind = ModelKind::of(model.map_or(DEFAULT_MODEL, |m| m.0.as_str()));
        commands.queue(move |world: &mut World| arm_one(world, e, kind));
    }
}

fn arm_one(world: &mut World, e: Entity, kind: Option<ModelKind>) {
    let Ok(mut player) = world.get_entity_mut(e) else { return };
    if player.contains::<Hl2Armed>() {
        return;
    }
    player.insert(Hl2Armed);
    let mut pools = Hl2Ammo(SPAWN_AMMO.iter().map(|(a, n)| (a.to_string(), *n)).collect());
    let melee = melee_for(kind);
    let has_melee = world
        .get::<Inventory>(e)
        .is_some_and(|inv| inv.weapons.iter().any(|w| world.get::<Melee>(*w).is_some()));
    if !has_melee && let Some(w) = spawn_weapon(world, melee, e) {
        if let Some(mut inv) = world.get_mut::<Inventory>(e) {
            inv.weapons.insert(0, w);
        }
    }
    let weapons: Vec<Entity> = world.get::<Inventory>(e).map(|i| i.weapons.clone()).unwrap_or_default();
    for w in &weapons {
        // A weapon counting its own rounds (the frag) takes its type's.
        if let Some(ammo) = world.get::<super::launch::Hl2Rounds>(*w).map(|r| r.ammo) {
            let n = pools.take(ammo);
            if let Some(mut r) = world.get_mut::<super::launch::Hl2Rounds>(*w) {
                r.count = (r.count + n).min(r.max);
            }
        }
        let Some(ammo) = world.get::<Hl2Item>(*w).and_then(|i| i.ammo) else { continue };
        let n = pools.take(ammo);
        if let Some(mut mag) = world.get_mut::<Magazine>(*w) {
            mag.reserve = n.min(mag.reserve_max);
        }
    }
    // The weapons in giving order switched by weight: the last given is
    // in hand, the one before it the last weapon.
    // The frag and the gravity gun never switch: the SMG1 is the last
    // weapon.
    let smg1 = weapons.iter().copied().find(|w| world.get::<Weapon>(*w).is_some_and(|x| x.id == super::SMG1));
    if let Some(mut inv) = world.get_mut::<Inventory>(e)
        && smg1.is_some()
        && inv.last != smg1
        && inv.active != smg1
    {
        inv.last = smg1;
    }
    world.entity_mut(e).insert(pools);
}

/// A death ends the life's loadout (the next spawn arms again).
pub fn disarm(mut died: MessageReader<Died>, mut commands: Commands) {
    for d in died.read() {
        if let Ok(mut e) = commands.get_entity(d.entity) {
            e.remove::<(Hl2Armed, Hl2Ammo)>();
        }
    }
}

/// A weapon taken new brings the taker's count of its ammo type into its
/// reserve, with what it carried itself.
pub fn on_pickup(
    mut picked: MessageReader<PickedUp>,
    mut pools: Query<&mut Hl2Ammo>,
    mut weapons: Query<(&Hl2Item, &mut Magazine)>,
    mut rounds: Query<&mut super::launch::Hl2Rounds>,
) {
    for p in picked.read() {
        // A weapon counting its own rounds takes the pool of their type.
        if let Ok(mut r) = rounds.get_mut(p.weapon) {
            let pooled = pools.get_mut(p.who).map_or(0, |mut a| a.take(r.ammo));
            r.count = (r.count + pooled).min(r.max);
        }
        let Ok((item, mut mag)) = weapons.get_mut(p.weapon) else { continue };
        let Some(ammo) = item.ammo else { continue };
        let pooled = pools.get_mut(p.who).map_or(0, |mut a| a.take(ammo));
        mag.reserve = (mag.reserve + pooled).min(mag.reserve_max);
    }
}

/// A weapon lying in the world carries its clip only (the dead's reserve
/// isn't theirs to drop), a map's also its spare rounds (the AR2's 30).
#[allow(clippy::type_complexity)]
pub fn loose_ammo(
    loose: Query<(&Loose, Has<MapWeapon>), Added<Loose>>,
    mut weapons: Query<(&Hl2Item, &mut Magazine)>,
) {
    for (l, placed) in &loose {
        let Ok((item, mut mag)) = weapons.get_mut(l.weapon) else { continue };
        let spare = if placed { item.spare(mag.size) } else { 0 };
        if mag.reserve != spare {
            mag.reserve = spare;
        }
    }
}

/// Whether a weapon has any ammo (melee weapons always do).
fn has_ammo(mag: Option<&Magazine>) -> bool {
    mag.is_none_or(|m| m.clip > 0 || m.reserve > 0)
}

/// The next best weapon for `inv` (weapons.md 2): None when the one in
/// hand may not be switched from or nothing else has ammo.
pub fn next_best(
    inv: &Inventory,
    item: impl Fn(Entity) -> Option<(Hl2Item, Option<Magazine>)>,
) -> Option<Entity> {
    let active = inv.active?;
    let (held, _) = item(active)?;
    if !held.autoswitchfrom {
        return None;
    }
    let candidates: Vec<(Entity, Hl2Item)> = inv
        .weapons
        .iter()
        .copied()
        .filter(|w| *w != active)
        .filter_map(|w| item(w).filter(|(i, m)| i.autoswitchto && has_ammo(m.as_ref())).map(|(i, _)| (w, i)))
        .collect();
    if let Some((w, _)) = candidates.iter().find(|(_, i)| i.weight == held.weight) {
        return Some(*w);
    }
    candidates
        .iter()
        .fold(None::<(Entity, i32)>, |best, (w, i)| match best {
            Some((_, b)) if b >= i.weight => best,
            _ => Some((*w, i.weight)),
        })
        .map(|(w, _)| w)
}

/// A weapon in hand with no ammo at all switches to the next best once
/// its timers have passed (no buttons held, or attack after its dry fire).
#[allow(clippy::type_complexity)]
pub fn switch_from_empty(
    mut owners: Query<(&Intent, &mut Inventory), Without<crate::map::interp::NetDrawn>>,
    mut weapons: Query<(&Hl2Item, Option<&Magazine>, &mut WeaponState)>,
    clock: Res<SimClock>,
) {
    let now = clock.now;
    for (intent, mut inv) in &mut owners {
        let Some(active) = inv.active else { continue };
        if inv.wanted.is_some() || now < inv.next_attack {
            continue;
        }
        let Ok((_, mag, st)) = weapons.get(active) else { continue };
        if mag.is_none() || has_ammo(mag) || st.next_primary >= now || st.next_secondary >= now {
            continue;
        }
        if intent.secondary || intent.reload {
            continue;
        }
        let pick = next_best(&inv, |w| weapons.get(w).ok().map(|(i, m, _)| (*i, m.cloned())));
        let Some(pick) = pick else { continue };
        if let Ok((_, _, mut st)) = weapons.get_mut(active) {
            st.next_primary = now + AUTOSWITCH_HOLD;
        }
        inv.wanted = Some(pick);
    }
}
