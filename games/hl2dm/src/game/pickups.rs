//! The map's pickups and wall chargers (specs/hl2dm/suit.md "Pickups",
//! "Wall chargers"; maps_entities.md 1, 2, 4), from the map's entities:
//!
//! - **Items**: `item_healthkit` (+25 health), `item_healthvial` (+10),
//!   `item_battery` (+15 armour, at most 100) and the ammo boxes (data/
//!   items.toml through `ammo::ItemData`: Pistol 20, SMG1 45, ... the
//!   "large" ones 100, 225, 100, 20; aliases too) are physics objects
//!   (players walk through them; they fall and come to rest) taken by
//!   touching them (the item's box grown 24 sideways and 12 up) with a
//!   clear line from the item to the eye. Each tick of such a touch fires
//!   the item's `OnCacheInteraction` (taken or not); one that would give
//!   nothing stays. An ammo box adds what fits of its count to the pool of
//!   its type, no weapon of it needed, and is used up whole even when only
//!   part fitted (`BaseCombatCharacter.AmmoPickup`, the `ammo_pickup`
//!   event); a box of a type HL2:DM lacks (flares, sniper rounds) is never
//!   taken. A taken item fires `OnPlayerTouch` and comes back 30 s later
//!   at the spot where it first came to rest, with `AlyxEMP.Charge`
//!   (Chimera's `weapon::respawn`, `sv_hl2mp_item_respawn_time`), or with
//!   spawnflag 1 << 30 is removed; one moved more than 64 units away
//!   goes back at its 30 s check. Placed weapons come back the same way
//!   (20 s, `sv_hl2mp_weapon_respawn_time`; at once with `mp_weaponstay`,
//!   when owners of a weapon can't take it).
//! - **Chargers**: `item_suitcharger` (75 armour; the citadel flag 8192:
//!   200, +2 armour and +5 health a charge, armour up to 200; the Kleiner
//!   flag 16384: 25) and `item_healthcharger` (50 health) give a point
//!   every 7 ticks (strictly more than 0.1 s apart) while `+use` holds
//!   them, deny when empty (at most every 0.62 s) or when the player is
//!   full (once a press), switch off 0.25 s after the last use and fill up
//!   again 30 s (suit; citadel 60 s) or 60 s (health) after running empty.
//!   Their outputs: `OnPlayerUse` (a run's first charge), `OnHalfEmpty`
//!   (a charge taking the pool to half or below), `OnEmpty`, `OnFull` (at
//!   map load and on each refill; suit chargers).
//!
//! Server-side.
//!
//! Known shortcuts:
//! - Items' and chargers' models aren't read: an item is a 16-unit cube
//!   (its origin at the bottom), its touch box that cube grown; a charger
//!   is found when the view's ray from the eye passes within 24 units of
//!   its origin, at most 80 units away (not Source's `+use` search). Items
//!   aren't drawn yet; the respawn light isn't made.
//! - Chargers' dial (the model's cycle) and the loop sound's stop aren't
//!   modelled (no model is drawn). `Recharge`/`SetCharge` arrive as
//!   `MapEntityInput`s (`set_pieces` registers the classes); the value
//!   outputs go out as `FireEntityOutputValue`s.
//! - A held item's bigger touch box (the gravity gun's feeding) waits for
//!   the gravity gun (M6).

use avian3d::prelude::{Collider, CollisionLayers, LayerMask, LinearDamping, Mass, RigidBody};
use bevy::prelude::*;
use chimera_sim::weapon::respawn::{self, MapPickup, PickupKind, Respawning, RespawnRules};

use super::{
    ammo::{self, AmmoLimits, ItemData},
    suit::{Suit, clear_line},
};
use crate::{
    core::{Health, Intent, RoundRestarts, SimClock, SimSet},
    map::{
        MapEntities, PlaySound,
        entities::{FireEntityOutput, FireEntityOutputValue, MapEntityInput},
    },
    weapon::Armor,
};

const UNIT: f32 = 0.0254;
/// Health and armour are normalized: 1.0 is 100 points.
const POINTS: f32 = 100.0;
/// `sk_healthkit`, `sk_healthvial`, `sk_battery` (skill.cfg).
pub const KIT: f32 = 25.0;
pub const VIAL: f32 = 10.0;
pub const BATTERY: f32 = 15.0;
/// `sv_hl2mp_item_respawn_time`, `sv_hl2mp_weapon_respawn_time`, seconds.
pub const ITEM_RESPAWN: f64 = 30.0;
pub const WEAPON_RESPAWN: f64 = 20.0;
/// Relocation distance (units) and the rest checks' period (s).
pub const RELOCATE: f32 = 64.0;
pub const SETTLE_EVERY: f64 = 0.1;
/// The no-respawn spawnflag (bit 30).
pub const NO_RESPAWN: i64 = 1 << 30;
/// An item's box (half size) and how far its touch box grows: sideways,
/// up (Source units).
pub const ITEM_HALF: f32 = 8.0;
const TOUCH_SIDE: f32 = 24.0;
const TOUCH_UP: f32 = 12.0;
/// An item's mass (kg).
const ITEM_MASS: f32 = 2.0;
/// Chargers: pools, the armour caps, the charge gap, switch-off, deny
/// repeat, the loop's delay, refills (suit.md "Constants").
pub const SUIT_POOL: f32 = 75.0;
pub const CITADEL_POOL: f32 = 200.0;
pub const KLEINER_POOL: f32 = 25.0;
pub const HEALTH_POOL: f32 = 50.0;
const ARMOUR_CAP: f32 = 100.0;
const CITADEL_CAP: f32 = 200.0;
const GAP: f64 = 0.1;
const IDLE_OFF: f64 = 0.25;
const DENY_REPEAT: f64 = 0.62;
const LOOP_DELAY: f64 = 0.56;
pub const SUIT_REFILL: f64 = 30.0;
pub const CITADEL_REFILL: f64 = 60.0;
pub const HEALTH_REFILL: f64 = 60.0;
/// `+use` reach, and how near the view's ray passes a charger (units).
const USE_REACH: f32 = 80.0;
const CHARGER_RADIUS: f32 = 24.0;
/// The clock's float noise.
const SAME: f64 = 1e-6;

/// What an item gives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Item {
    HealthKit,
    HealthVial,
    Battery,
    /// `count` rounds of ammo type `ammo`.
    Ammo { ammo: String, count: u32 },
}

/// A map item (`item_healthkit`, `item_battery`, an ammo box): its map
/// entity and what it gives. Taken ones are `respawn::Respawning`.
#[derive(Component, Clone, Debug)]
pub struct Pickup {
    pub item: Item,
    pub map_index: usize,
}

/// The respawn rules' cvars (`sv_hl2mp_item_respawn_time`,
/// `sv_hl2mp_weapon_respawn_time`, `mp_weaponstay`).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct RespawnCvars {
    pub item: f32,
    pub weapon: f32,
    pub weapon_stay: u8,
}

impl Default for RespawnCvars {
    fn default() -> Self {
        Self {
            item: ITEM_RESPAWN as f32,
            weapon: WEAPON_RESPAWN as f32,
            weapon_stay: 0,
        }
    }
}

/// The respawn rules from the cvars (`weapon::respawn`): `mp_weaponstay`
/// above 0 brings a weapon's copy at once and keeps owners from taking
/// it.
pub fn rules_of(c: &RespawnCvars) -> RespawnRules {
    RespawnRules {
        item_delay: c.item.max(0.0) as f64,
        weapon_relocate_every: c.weapon.max(0.0) as f64,
        weapon_delay: if c.weapon_stay > 0 { 0.0 } else { c.weapon.max(0.0) as f64 },
        relocate: Some(RELOCATE * UNIT),
        settle_every: SETTLE_EVERY,
        sound: Some("AlyxEMP.Charge"),
    }
}

fn apply_cvars(
    cvars: Res<RespawnCvars>,
    rules: Option<ResMut<RespawnRules>>,
    pickup: Option<ResMut<crate::weapon::drop::PickupRules>>,
    mut commands: Commands,
) {
    let want = rules_of(&cvars);
    match rules {
        Some(mut r) if *r != want => *r = want,
        Some(_) => {}
        None => commands.insert_resource(want),
    }
    if let Some(mut p) = pickup {
        let stay = cvars.weapon_stay > 0;
        if p.weapon_stay != stay {
            p.weapon_stay = stay;
        }
    }
}

/// What a charger charges.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChargerKind {
    Suit,
    /// Spawn flag 8192.
    Citadel,
    /// Spawn flag 16384.
    Kleiner,
    Health,
}

/// A wall charger (`item_suitcharger`, `item_healthcharger`).
#[derive(Component, Clone, Debug)]
pub struct Charger {
    pub kind: ChargerKind,
    pub map_index: usize,
    pub pool: f32,
    pub max: f32,
    last_charge: f64,
    off_at: Option<f64>,
    running: Option<f64>,
    looping: bool,
    press_only: bool,
    last_deny: f64,
    refill_at: Option<f64>,
}

impl Charger {
    pub fn new(kind: ChargerKind, map_index: usize) -> Self {
        let max = match kind {
            ChargerKind::Suit => SUIT_POOL,
            ChargerKind::Citadel => CITADEL_POOL,
            ChargerKind::Kleiner => KLEINER_POOL,
            ChargerKind::Health => HEALTH_POOL,
        };
        Self {
            kind,
            map_index,
            pool: max,
            max,
            last_charge: f64::NEG_INFINITY,
            off_at: None,
            running: None,
            looping: false,
            press_only: false,
            last_deny: f64::NEG_INFINITY,
            refill_at: None,
        }
    }

    fn suit(&self) -> bool {
        self.kind != ChargerKind::Health
    }

    fn sounds(&self) -> (&'static str, &'static str, &'static str) {
        if self.suit() {
            ("SuitRecharge.Start", "SuitRecharge.ChargingLoop", "SuitRecharge.Deny")
        } else {
            ("WallHealth.Start", "WallHealth.LoopingContinueCharge", "WallHealth.Deny")
        }
    }
}

/// The map's items and chargers came from this map (made again with it).
#[derive(Component, Clone, Copy, Debug)]
pub struct MapItem;

pub struct PickupsPlugin;

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        use crate::console::resource_cvar;
        app.add_message::<PlaySound>()
            .add_message::<FireEntityOutput>()
            .add_message::<FireEntityOutputValue>()
            .add_message::<MapEntityInput>()
            .init_resource::<RespawnCvars>()
            .insert_resource(rules_of(&RespawnCvars::default()))
            .insert_resource(AmmoLimits::hl2())
            .insert_resource(ItemData::hl2())
            .add_systems(
                FixedUpdate,
                (
                    apply_cvars,
                    spawn_items,
                    charger_inputs,
                    chargers.before(SimSet::Movement),
                    touch.after(SimSet::Movement).before(SimSet::Weapons),
                    super::ammo_crates::crates.after(SimSet::Weapons),
                )
                    .chain()
                    .run_if(crate::core::authoritative),
            );
        resource_cvar::<RespawnCvars, f32>(
            app,
            "hl2dm:sv_hl2mp_item_respawn_time",
            "Seconds before a taken item (kit, battery, ammo box) comes back.",
            |c| &mut c.item,
        );
        resource_cvar::<RespawnCvars, f32>(
            app,
            "hl2dm:sv_hl2mp_weapon_respawn_time",
            "Seconds before a taken placed weapon comes back.",
            |c| &mut c.weapon,
        );
        resource_cvar::<RespawnCvars, u8>(
            app,
            "hl2dm:mp_weaponstay",
            "1: placed weapons come back at once, and owners of a weapon can't take another.",
            |c| &mut c.weapon_stay,
        );
        super::ammo_crates::plugin(app);
        if !app.is_plugin_added::<respawn::MapRespawnPlugin>() {
            app.add_plugins(respawn::MapRespawnPlugin);
        }
    }
}

/// The map's items and chargers as entities, made again when the map's
/// entities change or the game restarts (`core::RoundRestarts`).
fn spawn_items(
    entities: Option<Res<MapEntities>>,
    restarts: Option<Res<RoundRestarts>>,
    mut seen: Local<u32>,
    old: Query<Entity, With<MapItem>>,
    data: Res<ItemData>,
    clock: Res<SimClock>,
    mut commands: Commands,
    mut outputs: MessageWriter<FireEntityOutput>,
) {
    let Some(map) = entities else { return };
    let restarted = restarts.as_ref().is_some_and(|r| r.0 != *seen);
    if !map.is_changed() && !restarted {
        return;
    }
    *seen = restarts.map_or(0, |r| r.0);
    for e in &old {
        commands.entity(e).despawn();
    }
    for (i, e) in map.entities.iter().enumerate() {
        let class = e.classname().to_ascii_lowercase();
        let origin = crate::map::entities::entity_to_engine(e.origin(), map.scale);
        let turn = crate::map::entities::rotation_to_engine(crate::map::entities::entity_rotation(e.angles()));
        let flags: i64 = e.get("spawnflags").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        let item = match class.as_str() {
            "item_healthkit" => Some(Item::HealthKit),
            "item_healthvial" => Some(Item::HealthVial),
            "item_battery" => Some(Item::Battery),
            c => data.ammo_box(c).map(|b| Item::Ammo {
                ammo: b.ammo.clone(),
                count: b.count,
            }),
        };
        if let Some(item) = item {
            let half = ITEM_HALF * UNIT;
            // A physics object from the start (maps_entities.md 1), its
            // origin at the bottom of its box.
            commands.spawn((
                Name::new(class.clone()),
                MapItem,
                Transform::from_translation(origin + Vec3::Y * half).with_rotation(turn),
                Visibility::default(),
                Pickup { item, map_index: i },
                MapPickup {
                    since: Some(clock.tick),
                    ..MapPickup::new(PickupKind::Item, flags & NO_RESPAWN != 0)
                },
                RigidBody::Dynamic,
                Collider::cuboid(2.0 * half, 2.0 * half, 2.0 * half),
                CollisionLayers::new(crate::core::ITEM_LAYER, LayerMask::ALL),
                Mass(ITEM_MASS),
                LinearDamping(0.1),
            ));
            continue;
        }
        let kind = match class.as_str() {
            "item_suitcharger" if flags & 8192 != 0 => ChargerKind::Citadel,
            "item_suitcharger" if flags & 16384 != 0 => ChargerKind::Kleiner,
            "item_suitcharger" => ChargerKind::Suit,
            "item_healthcharger" => ChargerKind::Health,
            _ => continue,
        };
        commands.spawn((Name::new(class.clone()), MapItem, Transform::from_translation(origin), Charger::new(kind, i)));
        // A suit charger's pool goes from 0 to full at its spawn:
        // `OnFull` at map load (maps_entities.md 4).
        if kind != ChargerKind::Health {
            outputs.write(FireEntityOutput {
                map_index: i,
                output: "OnFull".into(),
                activator: None,
            });
        }
    }
}

fn sound(play: &mut MessageWriter<PlaySound>, entry: &str, at: Vec3, source: Entity) {
    play.write(PlaySound {
        entry: entry.into(),
        at: Some(at),
        volume: None,
        pitch: None,
        source: Some(source),
        channel: None,
    });
}

/// What touching an item would give: whether it is taken (and so, for
/// kits and batteries, what changed).
#[allow(clippy::type_complexity)]
type Touchers<'w, 's> = Query<
    'w,
    's,
    (Entity, &'static Transform, &'static crate::core::MovementState, &'static Health, &'static Armor, &'static Suit),
    Without<Pickup>,
>;

/// Items touched (suit.md "Pickups", maps_entities.md 1), after the
/// movement: `OnCacheInteraction` each tick of a touch in sight; a taken
/// item goes away to come back (`respawn::taken`).
fn touch(
    items: Query<(Entity, &Transform, &Pickup), Without<Respawning>>,
    players: Touchers,
    spatial: avian3d::prelude::SpatialQuery,
    mut outputs: MessageWriter<FireEntityOutput>,
    mut commands: Commands,
) {
    let mut taken_by: Vec<Entity> = Vec::new();
    for (item, t, p) in &items {
        let half = Vec3::new(ITEM_HALF + TOUCH_SIDE, 0.0, ITEM_HALF + TOUCH_SIDE) * UNIT;
        let bottom = t.translation - Vec3::Y * ITEM_HALF * UNIT;
        let lo = bottom - Vec3::new(half.x, 0.0, half.z);
        let hi = bottom + Vec3::new(half.x, (2.0 * ITEM_HALF + TOUCH_UP) * UNIT, half.z);
        let centre = t.translation;
        for (who, pt, state, health, armor, suit) in &players {
            if health.current <= 0.0 || taken_by.contains(&item) {
                continue;
            }
            let (plo, phi) = (pt.translation + state.hull_min, pt.translation + state.hull_max);
            if !(plo.cmple(hi).all() && lo.cmple(phi).all()) {
                continue;
            }
            let eye = pt.translation + state.eye_offset;
            if !clear_line(&spatial, centre, eye, &[who, item]) {
                continue;
            }
            outputs.write(FireEntityOutput {
                map_index: p.map_index,
                output: "OnCacheInteraction".into(),
                activator: Some(who),
            });
            // Whether it would give anything: kits and batteries here, ammo
            // in the world (the pools).
            let useful = match p.item {
                Item::HealthKit | Item::HealthVial => (health.current * POINTS).round() < (health.max * POINTS).round(),
                Item::Battery => suit.worn && (armor.amount * POINTS).round() < ARMOUR_CAP,
                Item::Ammo { .. } => true,
            };
            if !useful {
                continue;
            }
            let (item_e, it, index) = (item, p.item.clone(), p.map_index);
            if !matches!(it, Item::Ammo { .. }) {
                taken_by.push(item);
            }
            commands.queue(move |w: &mut World| take(w, item_e, who, &it, index));
            if !matches!(p.item, Item::Ammo { .. }) {
                break;
            }
        }
    }
}

/// `who` takes `item` if it still can (an ammo box: if anything fits).
fn take(world: &mut World, item: Entity, who: Entity, what: &Item, index: usize) {
    if world.get::<Respawning>(item).is_some() || world.get_entity(item).is_err() {
        return;
    }
    let at = world.get::<Transform>(who).map_or(Vec3::ZERO, |t| t.translation);
    let entry = match what {
        Item::HealthKit | Item::HealthVial => {
            let give = if *what == Item::HealthKit { KIT } else { VIAL };
            let Some(mut h) = world.get_mut::<Health>(who) else { return };
            h.current = (h.current + give / POINTS).min(h.max);
            if *what == Item::HealthKit { "HealthKit.Touch" } else { "HealthVial.Touch" }
        }
        Item::Battery => {
            let Some(mut a) = world.get_mut::<Armor>(who) else { return };
            a.amount = (a.amount + BATTERY / POINTS).min(ARMOUR_CAP / POINTS);
            "ItemBattery.Touch"
        }
        Item::Ammo { ammo: kind, count } => {
            let added = ammo::give(world, who, kind, *count);
            if added == 0 {
                return;
            }
            let total = ammo::carried(world, who, kind);
            crate::rules::ammo_pickup(world, who, kind, added, total);
            "BaseCombatCharacter.AmmoPickup"
        }
    };
    world.write_message(PlaySound {
        entry: entry.into(),
        at: Some(at),
        volume: None,
        pitch: None,
        source: Some(who),
        channel: None,
    });
    world.write_message(FireEntityOutput {
        map_index: index,
        output: "OnPlayerTouch".into(),
        activator: Some(who),
    });
    respawn::taken(world, item);
}

/// The charger a player's `+use` finds: in front within reach, the
/// nearest along the view.
fn found(eye: Vec3, dir: Vec3, chargers: &[(Entity, Vec3)]) -> Option<Entity> {
    chargers
        .iter()
        .filter_map(|(e, at)| {
            let along = (*at - eye).dot(dir);
            let off = (*at - eye - dir * along).length();
            (along > 0.0 && along <= USE_REACH * UNIT && off <= CHARGER_RADIUS * UNIT).then_some((*e, along))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(e, _)| e)
}

/// Wall chargers (suit.md "Wall chargers"): each tick a living suited
/// player's held `+use` finds one, it is used.
#[allow(clippy::type_complexity)]
fn chargers(
    mut q: Query<(Entity, &Transform, &mut Charger)>,
    mut players: Query<
        (
            Entity,
            &Transform,
            &Intent,
            &crate::core::MovementState,
            &source_engine::source_movement::SourceMovement,
            &mut Health,
            &mut Armor,
            &Suit,
        ),
        Without<Charger>,
    >,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
    mut outputs: MessageWriter<FireEntityOutput>,
    mut values: MessageWriter<FireEntityOutputValue>,
) {
    let now = clock.now;
    let places: Vec<(Entity, Vec3)> = q.iter().map(|(e, t, _)| (e, t.translation)).collect();
    for (who, pt, intent, state, me, mut health, mut armor, suit) in &mut players {
        if !intent.use_key || health.current <= 0.0 {
            continue;
        }
        let fresh = !me.use_held;
        let eye = pt.translation + state.eye_offset;
        let (yaw, pitch) = (intent.yaw, intent.pitch);
        let dir = Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0) * Vec3::NEG_Z;
        let Some(c) = found(eye, dir, &places) else { continue };
        let Ok((ce, ct, mut ch)) = q.get_mut(c) else { continue };
        let at = ct.translation;
        let (start, looping, deny) = ch.sounds();
        if fresh {
            ch.press_only = false;
        }
        if ch.press_only {
            continue;
        }
        // 1. A suit charger needs the suit.
        if ch.suit() && !suit.worn {
            if now >= ch.last_deny + DENY_REPEAT - SAME {
                ch.last_deny = now;
                sound(&mut play, deny, at, ce);
            }
            continue;
        }
        // 2. Empty.
        if ch.pool <= 0.0 {
            switch_off(&mut ch, now);
            if now >= ch.last_deny + DENY_REPEAT - SAME {
                ch.last_deny = now;
                sound(&mut play, deny, at, ce);
            }
            continue;
        }
        // 3. A full player: one deny a press.
        let cap = if ch.kind == ChargerKind::Citadel { CITADEL_CAP } else { ARMOUR_CAP } / POINTS;
        // Whole points: 0.9 and ten 0.01 steps come to a hair under 1.0.
        let at_least = |v: f32, cap: f32| (v * POINTS).round() >= (cap * POINTS).round();
        let health_full = at_least(health.current, health.max);
        let armour_full = at_least(armor.amount, cap);
        let full = match ch.kind {
            ChargerKind::Health => health_full,
            ChargerKind::Citadel => armour_full && health_full,
            _ => armour_full,
        };
        if full {
            ch.press_only = true;
            sound(&mut play, deny, at, ce);
            continue;
        }
        // 4. Switch off a while after the last use.
        ch.off_at = Some(now + IDLE_OFF);
        // 5. One charge every 0.1 s (strictly).
        if now <= ch.last_charge + GAP + SAME {
            continue;
        }
        // 6. A run's first charge, and its loop.
        match ch.running {
            None => {
                ch.running = Some(now);
                ch.looping = false;
                sound(&mut play, start, at, ce);
                outputs.write(FireEntityOutput {
                    map_index: ch.map_index,
                    output: "OnPlayerUse".into(),
                    activator: Some(who),
                });
            }
            Some(since) if !ch.looping && now >= since + LOOP_DELAY - SAME => {
                ch.looping = true;
                sound(&mut play, looping, at, ce);
            }
            _ => {}
        }
        // 7. Charge.
        let before = ch.pool;
        match ch.kind {
            ChargerKind::Health => {
                if !health_full {
                    health.current = (health.current + 1.0 / POINTS).min(health.max);
                    ch.pool -= 1.0;
                }
            }
            kind => {
                let step = if kind == ChargerKind::Citadel { 2.0 } else { 1.0 };
                if !armour_full {
                    ch.pool -= step;
                    armor.amount = (armor.amount + step / POINTS).min(cap);
                }
                if kind == ChargerKind::Citadel && !health_full {
                    health.current = (health.current + 5.0 / POINTS).min(health.max);
                }
            }
        }
        ch.last_charge = now;
        // The charge left, as a fraction (maps_entities.md 4).
        values.write(FireEntityOutputValue {
            map_index: ch.map_index,
            output: if ch.suit() { "OutRemainingCharge" } else { "OutRemainingHealth" }.into(),
            activator: Some(who),
            value: ch.pool / ch.max,
        });
        // The pool's outputs (maps_entities.md 4).
        let half = (ch.max / 2.0).floor();
        let index = ch.map_index;
        let suit_charger = ch.suit();
        if suit_charger && before > half && ch.pool <= half {
            outputs.write(FireEntityOutput {
                map_index: index,
                output: "OnHalfEmpty".into(),
                activator: None,
            });
        }
        if suit_charger && before > 0.0 && ch.pool <= 0.0 {
            outputs.write(FireEntityOutput {
                map_index: index,
                output: "OnEmpty".into(),
                activator: None,
            });
        }
    }
    // 8. Switching off and filling up again.
    for (ce, ct, mut ch) in &mut q {
        if ch.off_at.is_some_and(|off| now >= off - SAME) {
            switch_off(&mut ch, now);
        }
        if let Some(at) = ch.refill_at
            && now >= at - SAME
        {
            ch.refill_at = None;
            ch.pool = ch.max;
            let entry = if ch.suit() { "SuitRecharge.Start" } else { "WallHealth.Recharge" };
            sound(&mut play, entry, ct.translation, ce);
            if ch.suit() {
                outputs.write(FireEntityOutput {
                    map_index: ch.map_index,
                    output: "OnFull".into(),
                    activator: None,
                });
            }
        }
    }
}

/// The map's inputs to chargers (maps_entities.md 4): `Recharge` (a suit
/// charger's pool to full now, `OnFull` if it was below) and `SetCharge`
/// (the pool and its maximum).
fn charger_inputs(
    mut inputs: MessageReader<MapEntityInput>,
    mut q: Query<(Entity, &Transform, &mut Charger)>,
    mut play: MessageWriter<PlaySound>,
    mut outputs: MessageWriter<FireEntityOutput>,
) {
    for i in inputs.read() {
        let Some((ce, ct, mut ch)) = q.iter_mut().find(|(_, _, c)| Some(c.map_index) == i.map_index) else { continue };
        match i.input.as_str() {
            "recharge" if ch.suit() => {
                let was_below = ch.pool < ch.max;
                ch.pool = ch.max;
                ch.refill_at = None;
                ch.off_at = None;
                sound(&mut play, "SuitRecharge.Start", ct.translation, ce);
                if was_below {
                    outputs.write(FireEntityOutput {
                        map_index: ch.map_index,
                        output: "OnFull".into(),
                        activator: None,
                    });
                }
            }
            "setcharge" if ch.suit() => {
                let n = i.value.trim().parse::<f32>().unwrap_or(0.0).trunc();
                ch.pool = n;
                ch.max = n;
            }
            _ => {}
        }
    }
}

/// A charger stops its run; empty, it fills up again after its time.
fn switch_off(ch: &mut Charger, now: f64) {
    ch.off_at = None;
    ch.running = None;
    ch.looping = false;
    if ch.pool <= 0.0 && ch.refill_at.is_none() {
        let wait = match ch.kind {
            ChargerKind::Health => HEALTH_REFILL,
            ChargerKind::Citadel => CITADEL_REFILL,
            _ => SUIT_REFILL,
        };
        ch.refill_at = Some(now + wait);
    }
}
