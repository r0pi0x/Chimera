//! The suit's pickups and wall chargers (specs/hl2dm/suit.md "Pickups",
//! "Wall chargers"), from the map's entities: `item_healthkit` (+25
//! health), `item_healthvial` (+10) and `item_battery` (+15 armour, at
//! most 100) are taken by touching them with a clear line from the item to
//! the eye, refused when they would give nothing, and come back 30 s later
//! (`sv_hl2mp_item_respawn_time`) with `AlyxEMP.Charge`;
//! `item_suitcharger` (75 armour; the citadel flag 8192: 200, +2 armour
//! and +5 health a charge, armour up to 200; the Kleiner flag 16384: 25)
//! and `item_healthcharger` (50 health) give a point every 7 ticks
//! (strictly more than 0.1 s apart) while `+use` holds them, deny when
//! empty (at most every 0.62 s) or when the player is full (once a press),
//! switch off 0.25 s after the last use and fill up again 30 s (suit) or
//! 60 s (health) after running empty. Server-side.
//!
//! Known shortcuts:
//! - Items' and chargers' models aren't read: an item's touch box is a
//!   16-unit cube on its origin grown 24 sideways and 12 up; a charger is
//!   found when the view's ray from the eye passes within 24 units of its
//!   origin, at most 80 units away (not Source's `+use` search). Items are
//!   not drawn yet and don't fall to rest (H10's respawn spot, M7).
//! - Chargers' value outputs (`OutRemainingCharge`, `OnHalfEmpty`, ...),
//!   their dial, the loop sound's stop and the `Recharge`/`SetCharge`
//!   inputs aren't modelled; `OnPlayerUse` is.

use bevy::prelude::*;

use super::suit::{Suit, clear_line};
use crate::{
    core::{Health, Intent, SimClock, SimSet},
    map::{MapEntities, PlaySound, entities::FireEntityOutput},
    weapon::Armor,
};

const UNIT: f32 = 0.0254;
/// Health and armour are normalized: 1.0 is 100 points.
const POINTS: f32 = 100.0;
/// `sk_healthkit`, `sk_healthvial`, `sk_battery` (skill.cfg).
pub const KIT: f32 = 25.0;
pub const VIAL: f32 = 10.0;
pub const BATTERY: f32 = 15.0;
/// `sv_hl2mp_item_respawn_time`, seconds.
pub const ITEM_RESPAWN: f64 = 30.0;
/// An item's box (half size) and how far its touch box grows: sideways,
/// up (Source units).
const ITEM_HALF: f32 = 8.0;
const TOUCH_SIDE: f32 = 24.0;
const TOUCH_UP: f32 = 12.0;
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    HealthKit,
    HealthVial,
    Battery,
}

/// A map item (`item_healthkit`, `item_healthvial`, `item_battery`).
#[derive(Component, Clone, Debug)]
pub struct Pickup {
    pub item: Item,
    /// Taken: back at this time (seconds).
    pub back_at: Option<f64>,
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

/// The map's items and chargers came from this map (respawned with it).
#[derive(Component, Clone, Copy, Debug)]
pub struct MapItem;

pub struct PickupsPlugin;

impl Plugin for PickupsPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<PlaySound>().add_message::<FireEntityOutput>().add_systems(
            FixedUpdate,
            (spawn_items, chargers.before(SimSet::Movement), touch.after(SimSet::Movement))
                .chain()
                .run_if(crate::core::authoritative),
        );
    }
}

/// The map's items and chargers as entities, made again when the map's
/// entities change.
fn spawn_items(entities: Option<Res<MapEntities>>, old: Query<Entity, With<MapItem>>, mut commands: Commands) {
    let Some(map) = entities else { return };
    if !map.is_changed() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    for (i, e) in map.entities.iter().enumerate() {
        let class = e.classname().to_ascii_lowercase();
        let at = Transform::from_translation(crate::map::entities::entity_to_engine(e.origin(), map.scale));
        let flags: i64 = e.get("spawnflags").and_then(|v| v.trim().parse().ok()).unwrap_or(0);
        let item = match class.as_str() {
            "item_healthkit" => Some(Item::HealthKit),
            "item_healthvial" => Some(Item::HealthVial),
            "item_battery" => Some(Item::Battery),
            _ => None,
        };
        if let Some(item) = item {
            commands.spawn((Name::new(class.clone()), MapItem, at, Visibility::default(), Pickup { item, back_at: None }));
            continue;
        }
        let kind = match class.as_str() {
            "item_suitcharger" if flags & 8192 != 0 => ChargerKind::Citadel,
            "item_suitcharger" if flags & 16384 != 0 => ChargerKind::Kleiner,
            "item_suitcharger" => ChargerKind::Suit,
            "item_healthcharger" => ChargerKind::Health,
            _ => continue,
        };
        commands.spawn((Name::new(class.clone()), MapItem, at, Charger::new(kind, i)));
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

/// Items touched (suit.md "Pickups"), after the movement; taken ones come
/// back after `ITEM_RESPAWN`.
#[allow(clippy::type_complexity)]
fn touch(
    mut items: Query<(Entity, &Transform, &mut Pickup, &mut Visibility)>,
    mut players: Query<
        (Entity, &Transform, &crate::core::MovementState, &mut Health, &mut Armor, &Suit),
        Without<Pickup>,
    >,
    spatial: avian3d::prelude::SpatialQuery,
    clock: Res<SimClock>,
    mut play: MessageWriter<PlaySound>,
) {
    let now = clock.now;
    for (item, t, mut p, mut vis) in &mut items {
        if let Some(back) = p.back_at {
            if now + SAME >= back {
                p.back_at = None;
                *vis = Visibility::Inherited;
                sound(&mut play, "AlyxEMP.Charge", t.translation, item);
            }
            continue;
        }
        let half = Vec3::new(ITEM_HALF + TOUCH_SIDE, 0.0, ITEM_HALF + TOUCH_SIDE) * UNIT;
        let lo = t.translation - Vec3::new(half.x, 0.0, half.z);
        let hi = t.translation + Vec3::new(half.x, (2.0 * ITEM_HALF + TOUCH_UP) * UNIT, half.z);
        let centre = t.translation + Vec3::Y * ITEM_HALF * UNIT;
        for (who, pt, state, mut health, mut armor, suit) in &mut players {
            if health.current <= 0.0 {
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
            let (taken, entry) = match p.item {
                Item::HealthKit | Item::HealthVial => {
                    let give = if p.item == Item::HealthKit { KIT } else { VIAL };
                    let ok = (health.current * POINTS).round() < (health.max * POINTS).round();
                    if ok {
                        health.current = (health.current + give / POINTS).min(health.max);
                    }
                    (ok, if p.item == Item::HealthKit { "HealthKit.Touch" } else { "HealthVial.Touch" })
                }
                Item::Battery => {
                    let ok = suit.worn && (armor.amount * POINTS).round() < ARMOUR_CAP;
                    if ok {
                        armor.amount = (armor.amount + BATTERY / POINTS).min(ARMOUR_CAP / POINTS);
                    }
                    (ok, "ItemBattery.Touch")
                }
            };
            if taken {
                sound(&mut play, entry, pt.translation, who);
                p.back_at = Some(now + ITEM_RESPAWN);
                *vis = Visibility::Hidden;
                break;
            }
        }
    }
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
