//! Map pickups that come back (docs/plans/active/hl2dm.md, lift C3; the
//! rules are HL2:DM's, specs/hl2dm/maps_entities.md 2, weapons.md 10, as
//! data any game may give): a game that wants it puts in `RespawnRules`
//! (without it, placed weapons are put back at round restarts only, as
//! CS:S's are: `equip`).
//!
//! - **Spots** (`MapPickup`): a map's pickup (a game's item, or a placed
//!   weapon's loose entity, `equip::place_weapons`) checks every
//!   `settle_every` from when it appeared whether it is at rest (its body
//!   asleep, or it has none) and on the first such check records where it
//!   lies as its **spot** (not where the map put it) and joins the
//!   relocation list, first checked that same tick.
//! - **An item taken** (`taken`, the game's touch calls it): hidden, its
//!   body off, moved to its spot (the world origin if it never rested), and
//!   back `item_delay` later (`Respawning`; a delay lands on the nearest
//!   tick, as Source's thinks do), the same entity; one marked `once` is
//!   removed instead.
//! - **A placed weapon taken** (`weapon_taken`: picked up new, or used up
//!   for its rounds; `drop::pick_up` calls it): a copy of the same weapon
//!   waits at the taken one's spot, turned as the taken one was
//!   (`WeaponCopy`), and appears `weapon_delay` later, a fresh weapon with
//!   its own spot where it appears; the copy fires no map outputs
//!   (`Copied`). The taken one leaves the list.
//! - **Relocation** (`relocate`): each listed pickup, at its check time
//!   (now at or past it), sets the next one (`item_delay` or `weapon_delay`
//!   on, as the rules say now) and, when it lies more than `relocate` from
//!   its spot and is at rest, is put back there (velocity kept, woken).
//! - Each return (an item back, a copy appearing, a relocation) is told as
//!   `PickupBack` and plays `RespawnRules::sound` there.
//!
//! The game adds `MapRespawnPlugin` (the systems) with its rules; an app
//! without them doesn't pay for them.
//!
//! Known shortcuts: a pickup taken before it ever rested comes back at the
//! world origin without the drop to a floor below it (maps_entities.md
//! "Edge cases"); a copy appears where the taken one rested, not dropped
//! again to the floor; the respawn light (a flash at the model's first
//! attachment) is left to the presentation, which hears `PickupBack`.

use avian3d::prelude::{AngularVelocity, ColliderDisabled, LinearVelocity, Position, RigidBodyDisabled, Rotation, Sleeping};
use bevy::prelude::*;

use super::{Weapon, WeaponRegistry, WeaponState, drop::Loose, equip::MapWeapon};
use crate::{
    core::SimClock,
    map::{PlaySound, loose::LooseItem},
};

/// How a game's map pickups come back (its presence turns this on).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct RespawnRules {
    /// Seconds a taken item stays away (HL2:DM's
    /// `sv_hl2mp_item_respawn_time`, 30), and between its relocation
    /// checks.
    pub item_delay: f64,
    /// Seconds before a taken placed weapon's copy appears
    /// (`sv_hl2mp_weapon_respawn_time`, 20; 0 with `mp_weaponstay`), and
    /// between its relocation checks.
    pub weapon_relocate_every: f64,
    pub weapon_delay: f64,
    /// Farther than this (m) from its spot and at rest, a pickup is put
    /// back at its check; None: never.
    pub relocate: Option<f32>,
    /// Seconds between rest checks of a pickup without a spot yet.
    pub settle_every: f64,
    /// The sound entry played where a pickup comes back.
    pub sound: Option<&'static str>,
}

/// What kind of map pickup: an item (the game's) or a placed weapon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupKind {
    Item,
    Weapon,
}

/// A map pickup that comes back.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct MapPickup {
    pub kind: PickupKind,
    /// Where it first came to rest; None: not yet.
    pub spot: Option<Transform>,
    /// The tick it appeared (its rest checks count from it).
    pub since: Option<u64>,
    /// Its next relocation check (s); None: not in the list.
    pub next_check: Option<f64>,
    /// Taken, it never comes back (HL2:DM's spawnflag 1 << 30).
    pub once: bool,
}

impl MapPickup {
    pub fn new(kind: PickupKind, once: bool) -> Self {
        Self {
            kind,
            spot: None,
            since: None,
            next_check: None,
            once,
        }
    }
}

/// A taken item, hidden until this tick.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Respawning {
    pub at: u64,
}

/// A taken placed weapon's copy, waiting to appear at `at` (tick).
#[derive(Component, Clone, Debug, PartialEq)]
pub struct WeaponCopy {
    pub id: &'static str,
    /// The map entity the first one came from (`MapWeapon`).
    pub index: usize,
    pub at: u64,
    pub spot: Transform,
}

/// On a copy (weapon and loose entity): no map entity's outputs.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Copied;

/// A pickup came back: an item reappeared, a copy appeared, or one was
/// relocated to its spot (`relocated`).
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct PickupBack {
    pub entity: Entity,
    pub at: Vec3,
    pub relocated: bool,
}

/// The returns' systems, for a game whose pickups come back (with its
/// `RespawnRules`); apps without one don't run them.
pub struct MapRespawnPlugin;

impl Plugin for MapRespawnPlugin {
    fn build(&self, app: &mut App) {
        plugin(app);
    }
}

fn plugin(app: &mut App) {
    app.add_message::<PickupBack>().add_message::<PlaySound>().add_systems(
        FixedUpdate,
        (settle, come_back, relocate)
            .chain()
            .after(crate::core::SimSet::Weapons)
            .run_if(resource_exists::<RespawnRules>)
            .run_if(crate::core::authoritative),
    );
}

/// Ticks a think `secs` ahead lands on (the nearest tick, as Source's
/// thinks: 20 s at 0.015 s is 1333).
pub fn think_ticks(secs: f64, dt: f64) -> u64 {
    if dt <= 0.0 { 0 } else { (secs / dt).round().max(0.0) as u64 }
}

fn at_rest(sleeping: bool, has_body: bool, still: bool) -> bool {
    sleeping || !has_body || still
}

/// Speeds (m/s, rad/s) under which a body counts as at rest even awake
/// (Source's sleep is the physics engine's; a body this slow is about to).
const STILL: f32 = 0.01;

type Body<'a> = (
    Option<&'a LinearVelocity>,
    Option<&'a AngularVelocity>,
    Has<Sleeping>,
    Has<avian3d::prelude::RigidBody>,
);

fn still(body: &(Option<&LinearVelocity>, Option<&AngularVelocity>, bool, bool)) -> bool {
    let (lin, ang, sleeping, has_body) = *body;
    let slow = lin.is_none_or(|v| v.0.length() < STILL) && ang.is_none_or(|v| v.0.length() < STILL);
    at_rest(sleeping, has_body, slow)
}

/// Rest checks: a pickup without a spot, every `settle_every` from when it
/// appeared, records where it lies on the first check that finds it at
/// rest, and joins the relocation list (checked this tick).
fn settle(
    mut pickups: Query<(&mut MapPickup, &Transform, Body), Without<Respawning>>,
    rules: Res<RespawnRules>,
    clock: Res<SimClock>,
) {
    let every = think_ticks(rules.settle_every, clock.delta.as_secs_f64()).max(1);
    for (mut p, t, body) in &mut pickups {
        if p.spot.is_some() {
            continue;
        }
        let since = *p.since.get_or_insert(clock.tick);
        let age = clock.tick - since;
        if age == 0 || !age.is_multiple_of(every) || !still(&body) {
            continue;
        }
        p.spot = Some(*t);
        p.next_check = Some(clock.now);
    }
}

/// Items whose time has come reappear; weapon copies whose time has come
/// appear.
fn come_back(
    items: Query<(Entity, &Respawning, &Transform)>,
    copies: Query<(Entity, &WeaponCopy)>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    for (e, r, t) in &items {
        if clock.tick < r.at {
            continue;
        }
        let at = t.translation;
        commands.queue(move |w: &mut World| {
            let Ok(mut ent) = w.get_entity_mut(e) else { return };
            ent.remove::<(Respawning, RigidBodyDisabled, ColliderDisabled)>();
            ent.insert(Visibility::Inherited);
            back(w, e, at, false);
        });
    }
    for (e, c) in &copies {
        if clock.tick < c.at {
            continue;
        }
        let c = c.clone();
        commands.queue(move |w: &mut World| {
            w.despawn(e);
            appear(w, &c);
        });
    }
}

/// A copy appears: a fresh weapon lying at its spot, its own spot there.
fn appear(world: &mut World, c: &WeaponCopy) {
    let since = super::drop::tick_time(world) - world.get_resource::<super::drop::PickupRules>().copied().unwrap_or_default().touch_delay;
    let Some((weapon, loose)) = place_one(world, c.id, c.index, c.spot, since) else { return };
    world.entity_mut(weapon).insert(Copied);
    let now = world.resource::<SimClock>().now;
    world.entity_mut(loose).insert((
        Copied,
        MapPickup {
            kind: PickupKind::Weapon,
            spot: Some(c.spot),
            since: None,
            next_check: Some(now),
            once: false,
        },
    ));
    back(world, loose, c.spot.translation, false);
}

/// A placed weapon `id` (map entity `index`) lying at `at`, touchable from
/// `since`: its weapon and loose entities.
pub fn place_one(world: &mut World, id: &str, index: usize, at: Transform, since: f64) -> Option<(Entity, Entity)> {
    let def = world.resource::<WeaponRegistry>().find(id)?;
    let (id, build) = (def.id, def.build.clone());
    let mut w = world.spawn((Name::new(id.to_string()), WeaponState::default(), MapWeapon(index)));
    build(&mut w);
    let weapon = w.id();
    let loose = world
        .spawn((
            Name::new(format!("Map {id}")),
            LooseItem(id.to_string()),
            Loose {
                weapon,
                dropper: None,
                since,
            },
            MapWeapon(index),
            at,
            LinearVelocity(Vec3::ZERO),
            AngularVelocity(Vec3::ZERO),
        ))
        .id();
    Some((weapon, loose))
}

/// Tell a return and play its sound.
fn back(world: &mut World, entity: Entity, at: Vec3, relocated: bool) {
    world.write_message(PickupBack { entity, at, relocated });
    if let Some(sound) = world.get_resource::<RespawnRules>().and_then(|r| r.sound) {
        world.write_message(PlaySound {
            entry: sound.into(),
            at: Some(at),
            volume: None,
            pitch: None,
            source: Some(entity),
            channel: None,
        });
    }
}

/// Relocation checks (after the tick's thinks): a listed pickup at its
/// check time gets its next, and lying at rest farther than `relocate`
/// from its spot goes back there.
#[allow(clippy::type_complexity)]
fn relocate(
    mut pickups: Query<(Entity, &mut MapPickup, &Transform, Body), Without<Respawning>>,
    rules: Res<RespawnRules>,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    let now = clock.now;
    for (e, mut p, t, body) in &mut pickups {
        let (Some(next), Some(spot)) = (p.next_check, p.spot) else { continue };
        if now + 1e-9 < next {
            continue;
        }
        let every = match p.kind {
            PickupKind::Item => rules.item_delay,
            PickupKind::Weapon => rules.weapon_relocate_every,
        };
        p.next_check = Some(now + every);
        let Some(limit) = rules.relocate else { continue };
        if t.translation.distance(spot.translation) <= limit || !still(&body) {
            continue;
        }
        commands.queue(move |w: &mut World| {
            put(w, e, spot);
            w.entity_mut(e).remove::<Sleeping>();
            back(w, e, spot.translation, true);
        });
    }
}

/// Move `e` to `to` (its transform and its body's pose).
fn put(world: &mut World, e: Entity, to: Transform) {
    let Ok(mut ent) = world.get_entity_mut(e) else { return };
    if let Some(mut t) = ent.get_mut::<Transform>() {
        t.translation = to.translation;
        t.rotation = to.rotation;
    }
    if let Some(mut p) = ent.get_mut::<Position>() {
        p.0 = to.translation;
    }
    if let Some(mut r) = ent.get_mut::<Rotation>() {
        *r = Rotation::from(to.rotation);
    }
}

/// The game's item `e` was taken: removed if it comes back `once`, else
/// hidden, its body off and still, at its spot (the world origin if it
/// never rested) until `item_delay` from now.
pub fn taken(world: &mut World, e: Entity) {
    let Some(p) = world.get::<MapPickup>(e).cloned() else { return };
    if p.once {
        world.despawn(e);
        return;
    }
    let Some(rules) = world.get_resource::<RespawnRules>().copied() else { return };
    let clock = *world.resource::<SimClock>();
    let at = clock.tick + think_ticks(rules.item_delay, clock.delta.as_secs_f64());
    put(world, e, p.spot.unwrap_or_default());
    let mut ent = world.entity_mut(e);
    ent.insert((Respawning { at }, Visibility::Hidden, RigidBodyDisabled, ColliderDisabled));
    if let Some(mut v) = ent.get_mut::<LinearVelocity>() {
        v.0 = Vec3::ZERO;
    }
    if let Some(mut v) = ent.get_mut::<AngularVelocity>() {
        v.0 = Vec3::ZERO;
    }
}

/// The placed weapon lying as `loose` is being taken (picked up new, or
/// used up for its rounds): its copy waits at its spot (the world origin if
/// it never rested), turned as it lies now, for `weapon_delay`. Call it
/// before `loose` goes.
pub fn weapon_taken(world: &mut World, loose: Entity) {
    let Some(rules) = world.get_resource::<RespawnRules>().copied() else { return };
    let Some(p) = world.get::<MapPickup>(loose).cloned() else { return };
    if p.once {
        return;
    }
    let Some(index) = world.get::<MapWeapon>(loose).map(|m| m.0) else { return };
    let Some(weapon) = world.get::<Loose>(loose).map(|l| l.weapon) else { return };
    let Some(id) = world.get::<Weapon>(weapon).map(|w| w.id) else { return };
    let rotation = world.get::<Transform>(loose).map_or(Quat::IDENTITY, |t| t.rotation);
    let clock = *world.resource::<SimClock>();
    let at = clock.tick + think_ticks(rules.weapon_delay, clock.delta.as_secs_f64()).max(1);
    let spot = Transform::from_translation(p.spot.map_or(Vec3::ZERO, |s| s.translation)).with_rotation(rotation);
    world.entity_mut(loose).remove::<MapPickup>();
    world.spawn((Name::new(format!("Copy of {id}")), WeaponCopy { id, index, at, spot }, MapWeapon(index)));
}

#[cfg(test)]
mod tests {
    #[test]
    fn thinks_land_on_the_nearest_tick() {
        assert_eq!(super::think_ticks(20.0, 0.015), 1333);
        assert_eq!(super::think_ticks(30.0, 0.015), 2000);
        assert_eq!(super::think_ticks(0.1, 0.015), 7);
        assert_eq!(super::think_ticks(10.0, 0.015), 667);
    }
}
