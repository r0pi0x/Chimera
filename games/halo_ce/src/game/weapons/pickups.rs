//! Halo's map pickups (specs/halo_ce/multiplayer_rules.md 5, weapons.md 3,
//! objects.md "Equipment"; health_shields.md "The overshield", "Active
//! camouflage", "Health"): the scenario's netgame equipment as items on
//! the item clock, and what walking over (or holding the action button by)
//! an item does.
//!
//! From the map to the simulation: the loader (`add_entities`) writes each
//! netgame equipment entry eligible for free-for-all slayer as one map
//! entity (`ENTRY_CLASS`: its position, levitate flag, own and collection
//! spawn times, and its item collection's weighted items by tag), only
//! when the map's game is the gameplay's (`map::KitLoad::pickups`: a
//! party's Halo map has none, as HL2:DM's: a ladder decides the guns), and
//! the level's material types (`materials`). `spawn_entries` makes the
//! entries from the map's entities.
//!
//! The item clock (`clock`): game time is ticks since the entries were
//! made; an entry creates its item whenever game time is a multiple of its
//! period (its own spawn time, else its collection's, else 30 s, times
//! 30), whether or not the last one was taken: one draw from its
//! collection (the integer r in [0, total) from the game-state generator,
//! the first permutation whose weight takes r below 0), the flamethrower
//! and gravity rifle made rocket launchers (the weapon set's rule); a
//! created item's "last owned" time is advanced by its period less 900
//! ticks, and every item not held for 900 ticks is deleted (so a map item
//! untaken is replaced, the two overlapping one tick; a dropped weapon
//! lives 901 ticks). A dead unit drops its Halo weapons where it died
//! (`drop_on_death`), on a map whose pickups play.
//!
//! Taking (`take`, each tick, an item within a living unit's bounding
//! sphere): a powerup is used at once unless its effect refuses it
//! (`Vitality::overshield`, `camouflage`, `heal`); a grenade adds one of its
//! kind below the maximum (4); a weapon of a tag the unit holds gives its
//! spare rounds (`params::take_ammunition`, deleted at 0 spare); any other
//! weapon is taken by holding the action (use) key for 7 ticks: added when
//! the unit holds fewer than two weapons, else swapped for the one in hand,
//! which drops where the unit stands.
//!
//! Known shortcuts: items do not fall (a map item stays where the entry
//! puts it, levitating or not; a dropped weapon where it was dropped); the
//! weapon set substitution is set 0's (normal) and only the flamethrower
//! and gravity rifle rule applies; the game type's options (no shields,
//! invisible players, infinite grenades) are not read; a swap with a weapon
//! of the same tag (a fresher plasma weapon) is not offered; a dead
//! player's weapons drop where its body stands, at once (no throw), and its
//! grenades are not dropped.

use blam_formats::{
    cache::{MapFile, class},
    scenario::{GameType, ItemCollection, Scenario},
};

use crate::map::{MapData, MapEntity};

/// A netgame equipment entry's map entity.
pub const ENTRY_CLASS: &str = "halo_ce_netgame_equipment";

/// The map's entities for the simulation: the level's material types and,
/// when the map's game is the gameplay's (`pickups`), its netgame
/// equipment entries eligible for free-for-all slayer.
pub fn add_entities(file: &MapFile, scenario: &Scenario, map: &mut MapData, pickups: bool) {
    let codes = map
        .extras
        .get::<super::materials::LevelCodes>()
        .map(|c| c.0.clone())
        .unwrap_or_default();
    map.extras.remove::<super::materials::LevelCodes>();
    map.entities.push(super::materials::entity(&codes));
    map.entity_scale = 1.0;
    if !pickups {
        return;
    }
    for (i, e) in scenario.netgame_equipment.iter().enumerate() {
        if !e.eligible_for(GameType::Slayer) {
            continue;
        }
        let collection = blam_formats::weapon::ref_id(&e.item_collection)
            .and_then(|id| file.tag(id))
            .filter(|t| t.class == class::ITEM_COLLECTION)
            .and_then(|t| ItemCollection::read(file, t).ok());
        let Some(collection) = collection else { continue };
        let items: Vec<String> = collection
            .permutations
            .iter()
            .filter_map(|p| {
                let tag = blam_formats::weapon::ref_id(&p.item).and_then(|id| file.tag(id))?;
                Some(format!("{}={}", p.weight, tag.path))
            })
            .collect();
        let at = super::super::maps::space::point(e.position);
        let origin = crate::map::entities::engine_to_entity(at, 1.0);
        map.entities.push(MapEntity {
            keyvalues: vec![
                ("classname".into(), ENTRY_CLASS.into()),
                ("index".into(), i.to_string()),
                ("origin".into(), format!("{} {} {}", origin.x, origin.y, origin.z)),
                ("levitate".into(), (e.levitate() as u8).to_string()),
                ("spawn_time".into(), e.spawn_time.to_string()),
                ("collection_spawn_time".into(), collection.spawn_time.to_string()),
                ("items".into(), items.join("|")),
            ],
            ..Default::default()
        });
    }
}

// ---------------------------------------------------------------------------
// The simulation's side.

use std::sync::Arc;

use bevy::prelude::*;
use chimera_sim::weapon::{Inventory, Magazine, Weapon, give};

use super::{
    guns,
    grenades::{FRAG, GrenadeData, PLASMA},
    params::take_ammunition,
    random::HaloRng,
    systems::{HaloHands, HaloWeapon, halo_ticks},
    tick::HaloGun,
};
use crate::{
    core::{Health, Intent, SimClock},
    map::MapEntities,
};

/// Ticks an unheld item lives past its last owner (multiplayer_rules.md
/// 5.4: deleted when now - 900 > last owned).
pub const ITEM_LIFE: i64 = 900;
/// Ticks the action button is held to swap for a weapon (globals: 7).
pub const SWAP_HOLD_TICKS: u32 = 7;
/// The flamethrower's and gravity rifle's tags, made rocket launchers on
/// every map (5.3).
pub const MADE_ROCKET_LAUNCHERS: [&str; 2] = ["weapons\\flamethrower\\flamethrower", "weapons\\gravity rifle\\gravity rifle"];

/// A netgame equipment entry: its block index, where (metres), its
/// period in ticks, its collection's (weight, item tag) list.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct NetgameEntry {
    pub index: usize,
    pub at: Vec3,
    pub period: u32,
    pub items: Vec<(f32, String)>,
    pub levitate: bool,
}

impl NetgameEntry {
    /// The period of an entry with its own and its collection's spawn
    /// times (seconds; 0: unset): 30 x the first set, else 900.
    pub fn period_of(spawn_time: i16, collection_spawn_time: i16) -> u32 {
        let s = if spawn_time != 0 { spawn_time } else { collection_spawn_time };
        if s > 0 { s as u32 * 30 } else { 900 }
    }

    /// The draw's range: the weights' sum truncated (5.2).
    pub fn total(&self) -> u32 {
        let sum: f32 = self.items.iter().map(|i| i.0).sum();
        if sum.is_finite() && sum > 0.0 { sum as u32 } else { 0 }
    }

    /// The item an integer draw `r` from [0, total) picks: the first whose
    /// weight takes it below 0.
    pub fn pick(&self, r: u32) -> Option<&str> {
        let mut r = r as f32;
        for (w, tag) in &self.items {
            r -= w;
            if r < 0.0 {
                return Some(tag);
            }
        }
        None
    }

    /// One read from a map entity.
    pub fn from_entity(e: &MapEntity) -> Option<Self> {
        if e.classname() != ENTRY_CLASS {
            return None;
        }
        let num = |k: &str| e.get(k).and_then(|v| v.trim().parse::<i64>().ok()).unwrap_or(0);
        let items = e
            .get("items")
            .unwrap_or("")
            .split('|')
            .filter_map(|i| {
                let (w, t) = i.split_once('=')?;
                Some((w.parse().ok()?, t.to_string()))
            })
            .collect();
        Some(NetgameEntry {
            index: num("index") as usize,
            at: crate::map::entities::entity_to_engine(e.vector("origin"), 1.0),
            period: Self::period_of(num("spawn_time") as i16, num("collection_spawn_time") as i16),
            items,
            levitate: num("levitate") != 0,
        })
    }
}

/// A powerup's kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Powerup {
    Overshield,
    Camouflage,
    Health,
}

/// What an item is.
#[derive(Clone, Debug, PartialEq)]
pub enum Item {
    /// A weapon with its rounds (loaded, spare) and battery.
    Weapon { id: &'static str, loaded: i16, reserve: i16, age: f32 },
    /// One grenade of a kind.
    Grenade(usize),
    Powerup(Powerup),
}

/// An item lying in the world (metres: its `Transform`).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct HaloItem {
    pub item: Item,
    /// Game time it was last held (or created, advanced by its entry's
    /// period less 900).
    pub last_owned: i64,
    /// The netgame entry that made it.
    pub entry: Option<usize>,
}

/// An item taken (for presentation: its pickup sound for the taker): by
/// whom, the item's tag, where it lay (metres).
#[derive(Message, Clone, Debug, PartialEq)]
pub struct HaloTaken {
    pub unit: Entity,
    pub tag: String,
    pub at: Vec3,
}

/// An item's object tag (what it is drawn as and its pickup sound): a
/// weapon's tag, a grenade's, a powerup's equipment.
pub fn item_tag(item: &Item, grenades: &GrenadeData, spartan: &super::super::vitality::Spartan) -> String {
    match item {
        Item::Weapon { id, .. } => guns().into_iter().find(|(i, _)| i == id).map(|(_, g)| g.tag).unwrap_or_default(),
        Item::Grenade(k) => if *k == FRAG { grenades.frag.tag.clone() } else { grenades.plasma.tag.clone() },
        Item::Powerup(p) => {
            let p2 = &spartan.powerups;
            match p {
                Powerup::Overshield => p2.overshield.tag.clone(),
                Powerup::Camouflage => p2.active_camouflage.tag.clone(),
                Powerup::Health => p2.health_pack.tag.clone(),
            }
        }
    }
}

/// The item clock: game time in Halo ticks since the entries were made,
/// and the generator its draws come from.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct ItemClock {
    pub time: i64,
    pub rng: u32,
    /// The map entities the entries came from.
    seen: usize,
}

/// The characters' action-button holds (ticks) for the swap (a resource:
/// no part comes and goes on the characters).
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct UseHolds(pub std::collections::BTreeMap<Entity, u32>);

/// What a tag names as an item, at its creation: a weapon (the
/// flamethrower and gravity rifle as rocket launchers) new from its data,
/// a grenade, a powerup (by the vitality's and grenades' tags).
pub fn item_of(tag: &str, grenades: &GrenadeData, spartan: &super::super::vitality::Spartan) -> Option<Item> {
    let tag = if MADE_ROCKET_LAUNCHERS.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
        "weapons\\rocket launcher\\rocket launcher"
    } else {
        tag
    };
    if let Some((id, g)) = guns().into_iter().find(|(_, g)| g.tag.eq_ignore_ascii_case(tag)) {
        let id = super::GUNS.into_iter().find(|i| *i == id)?;
        let (loaded, reserve) = g.magazine.map_or((0, 0), |m| m.new_rounds());
        return Some(Item::Weapon { id, loaded, reserve, age: 0.0 });
    }
    if grenades.frag.tag.eq_ignore_ascii_case(tag) {
        return Some(Item::Grenade(FRAG));
    }
    if grenades.plasma.tag.eq_ignore_ascii_case(tag) {
        return Some(Item::Grenade(PLASMA));
    }
    let p = &spartan.powerups;
    [(&p.overshield.tag, Powerup::Overshield), (&p.active_camouflage.tag, Powerup::Camouflage), (&p.health_pack.tag, Powerup::Health)]
        .into_iter()
        .find(|(t, _)| t.eq_ignore_ascii_case(tag))
        .map(|(_, k)| Item::Powerup(k))
}

/// The entries from the map's entities (made again when they change; the
/// clock starts at 0).
#[allow(clippy::type_complexity)]
fn spawn_entries(
    entities: Option<Res<MapEntities>>,
    old: Query<Entity, Or<(With<NetgameEntry>, With<HaloItem>)>>,
    mut clock: ResMut<ItemClock>,
    mut commands: Commands,
) {
    let Some(map) = entities else { return };
    let key = Arc::as_ptr(&map.entities) as usize;
    if clock.seen == key {
        return;
    }
    clock.seen = key;
    clock.time = 0;
    for e in &old {
        commands.entity(e).despawn();
    }
    for e in map.entities.iter().filter_map(NetgameEntry::from_entity) {
        commands.spawn((Name::new(format!("halo_ce netgame equipment {}", e.index)), e));
    }
}

/// The item clock's tick (5.1, 5.4): each entry whose period divides the
/// game time creates its item; items unheld past their life are deleted.
#[allow(clippy::too_many_arguments)]
pub fn clock(
    entries: Query<&NetgameEntry>,
    items: Query<(Entity, &HaloItem)>,
    mut clock: ResMut<ItemClock>,
    sim: Res<SimClock>,
    grenades: Res<GrenadeData>,
    spartan: Res<super::super::vitality::Spartan>,
    mut commands: Commands,
) {
    for _ in 0..halo_ticks(&sim) {
        let now = clock.time;
        let mut list: Vec<&NetgameEntry> = entries.iter().collect();
        list.sort_by_key(|e| e.index);
        let mut rng = HaloRng(clock.rng);
        for e in list {
            if now % e.period as i64 != 0 {
                continue;
            }
            let total = e.total();
            if total == 0 {
                continue;
            }
            let r = rng.int_range(0, total as i32) as u32;
            let Some(item) = e.pick(r).and_then(|t| item_of(t, &grenades, &spartan)) else { continue };
            debug!("halo_ce: entry {} makes {:?} at {}", e.index, item, now);
            commands.spawn((
                Name::new("halo_ce item"),
                Transform::from_translation(e.at),
                HaloItem { item, last_owned: now + e.period as i64 - ITEM_LIFE, entry: Some(e.index) },
            ));
        }
        clock.rng = rng.0;
        for (i, item) in &items {
            if now - ITEM_LIFE > item.last_owned {
                commands.entity(i).despawn();
            }
        }
        clock.time += 1;
    }
}

/// What a unit overlapping items takes (5.5), each Halo tick (exclusive:
/// a swap gives and removes weapons).
pub fn take(world: &mut World) {
    let n = world.get_resource::<SimClock>().map_or(0, halo_ticks);
    if n == 0 {
        return;
    }
    let Some(spartan) = world.get_resource::<super::super::vitality::Spartan>().cloned() else { return };
    let Some(grenades) = world.get_resource::<GrenadeData>().cloned() else { return };
    // The clock has stepped: the time items are held at.
    let now = world.get_resource::<ItemClock>().map_or(0, |c| c.time - 1);
    let reach = grenades.explosions.biped_bounding_radius * super::METERS_PER_WU;
    let mut units = world.query_filtered::<(Entity, &Transform, &Health, &Intent), With<Inventory>>();
    let units: Vec<(Entity, Vec3, bool, bool)> = units.iter(world).map(|(e, t, h, i)| (e, t.translation, h.current > 0.0, i.use_key)).collect();
    let mut items = world.query::<(Entity, &Transform, &HaloItem)>();
    let lying: Vec<(Entity, Vec3, HaloItem)> = items.iter(world).map(|(e, t, i)| (e, t.translation, i.clone())).collect();
    let mut gone: Vec<Entity> = Vec::new();
    for (unit, centre, alive, use_key) in units {
        let mut holds = world.get_resource_or_insert_with(UseHolds::default);
        let hold = if use_key { holds.0.get(&unit).copied().unwrap_or(0) + 1 } else { 0 };
        if hold == 0 {
            holds.0.remove(&unit);
        } else {
            holds.0.insert(unit, hold);
        }
        if !alive {
            continue;
        }
        for (ie, at, item) in &lying {
            if gone.contains(ie) || at.distance(centre) > reach {
                continue;
            }
            let taken = match &item.item {
                Item::Powerup(p) => {
                    let Some(mut v) = world.get_mut::<super::super::vitality::Vitality>(unit) else { continue };
                    match p {
                        Powerup::Overshield => v.overshield(&spartan),
                        Powerup::Camouflage => v.camouflage(&spartan),
                        Powerup::Health => v.heal(),
                    }
                }
                Item::Grenade(k) => {
                    let Some(mut h) = world.get_mut::<HaloHands>(unit) else { continue };
                    if h.throw.counts[*k] < grenades.throw.maximum[*k] {
                        h.throw.counts[*k] += 1;
                        true
                    } else {
                        false
                    }
                }
                Item::Weapon { id, loaded, reserve, age } => {
                    let held = world.get::<Inventory>(unit).map(|i| i.weapons.clone()).unwrap_or_default();
                    let same = held.iter().copied().find(|w| world.get::<Weapon>(*w).is_some_and(|x| x.id == *id));
                    match same {
                        Some(w) => {
                            // Its spare rounds into the held one's.
                            let max = world.get::<HaloWeapon>(w).and_then(|h| h.0.magazine).map_or(0, |m| m.rounds_total_maximum);
                            let Some(mut m) = world.get_mut::<Magazine>(w) else { continue };
                            let (h, g, delete) = take_ammunition(m.reserve as i16, max, *reserve);
                            if h == m.reserve as i16 {
                                continue;
                            }
                            m.reserve = h as u32;
                            if let Some(mut gun) = world.get_mut::<HaloGun>(w) {
                                gun.reserve = h;
                            }
                            if !delete
                                && let Some(mut x) = world.get_mut::<HaloItem>(*ie)
                                && let Item::Weapon { reserve, .. } = &mut x.item
                            {
                                *reserve = g;
                            }
                            delete
                        }
                        None if hold == SWAP_HOLD_TICKS => {
                            swap(world, unit, *at, id, *loaded, *reserve, *age, now);
                            true
                        }
                        None => false,
                    }
                }
            };
            if taken {
                debug!("halo_ce: {unit:?} took {:?}", item.item);
                gone.push(*ie);
                world.despawn(*ie);
                let tag = item_tag(&item.item, &grenades, &spartan);
                world.write_message(HaloTaken { unit, tag, at: *at });
            }
        }
    }
}

/// The swap (weapons.md 3): a unit with fewer than two Halo weapons adds
/// the new one, else the one in hand drops where the unit stands; the new
/// one is drawn with the ground weapon's rounds and battery.
#[allow(clippy::too_many_arguments)]
fn swap(world: &mut World, unit: Entity, at: Vec3, id: &'static str, loaded: i16, reserve: i16, age: f32, now: i64) {
    let held: Vec<Entity> = world
        .get::<Inventory>(unit)
        .map(|i| i.weapons.iter().copied().filter(|w| world.get::<HaloWeapon>(*w).is_some()).collect())
        .unwrap_or_default();
    if held.len() >= 2
        && let Some(active) = world.get::<Inventory>(unit).and_then(|i| i.active)
    {
        let dropped = world.get::<Weapon>(active).map(|w| w.id);
        let state = world.get::<HaloGun>(active).map(|g| (g.loaded, g.reserve, g.age));
        let size = world.get::<HaloWeapon>(active).and_then(|h| h.0.magazine).map_or(0, |m| m.rounds_loaded_maximum);
        if let Some(mut inv) = world.get_mut::<Inventory>(unit) {
            inv.weapons.retain(|w| *w != active);
            inv.active = None;
        }
        world.despawn(active);
        if let (Some(did), Some((l, r, a))) = (dropped, state) {
            // A weapon that cannot be fired is deleted at once (3, "Drop").
            let empty = a >= 1.0 || (size > 0 && l <= 0 && r <= 0);
            if !empty {
                world.spawn((
                    Name::new("halo_ce item"),
                    Transform::from_translation(at),
                    HaloItem { item: Item::Weapon { id: did, loaded: l, reserve: r, age: a }, last_owned: now, entry: None },
                ));
            }
        }
    }
    if let Some(w) = give(world, unit, id) {
        if let Some(mut g) = world.get_mut::<HaloGun>(w) {
            g.loaded = loaded;
            g.reserve = reserve;
            g.age = age;
        }
        if let Some(mut m) = world.get_mut::<Magazine>(w) {
            m.clip = loaded.max(0) as u32;
            m.reserve = reserve.max(0) as u32;
        }
    }
}

/// A dead unit's Halo weapons dropped where it died (multiplayer_rules.md
/// 5.4: unattached from then, so they live 901 ticks; one that cannot be
/// fired is deleted at once, weapons.md 3), on maps whose pickups play (a
/// netgame entry exists: a party's gun game drops nothing).
#[allow(clippy::type_complexity)]
pub fn drop_on_death(
    mut died: MessageReader<chimera_core::core::Died>,
    entries: Query<(), With<NetgameEntry>>,
    units: Query<(&Transform, &Inventory)>,
    guns: Query<(&Weapon, &HaloGun, &HaloWeapon)>,
    clock: Res<ItemClock>,
    mut commands: Commands,
) {
    let deaths: Vec<Entity> = died.read().map(|d| d.entity).collect();
    if entries.is_empty() {
        return;
    }
    for unit in deaths {
        let Ok((at, inv)) = units.get(unit) else { continue };
        for w in &inv.weapons {
            let Ok((weapon, g, hw)) = guns.get(*w) else { continue };
            let size = hw.0.magazine.map_or(0, |m| m.rounds_loaded_maximum);
            if g.age >= 1.0 || (size > 0 && g.loaded <= 0 && g.reserve <= 0) {
                continue;
            }
            commands.spawn((
                Name::new("halo_ce item"),
                Transform::from_translation(at.translation),
                HaloItem { item: Item::Weapon { id: weapon.id, loaded: g.loaded, reserve: g.reserve, age: g.age }, last_owned: clock.time, entry: None },
            ));
        }
    }
}

/// Spawn an entry (tests and tools): what the map's entity would make.
pub fn spawn_entry(world: &mut World, entry: NetgameEntry) -> Entity {
    world.spawn((Name::new(format!("halo_ce netgame equipment {}", entry.index)), entry)).id()
}

/// The pickups' systems, in order: the entries from the map, the clock,
/// the taking.
pub fn systems() -> bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::ScheduleSystem> {
    (spawn_entries, clock, take, drop_on_death).chain()
}
