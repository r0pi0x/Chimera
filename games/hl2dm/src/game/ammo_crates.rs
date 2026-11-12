//! `item_ammo_crate` (specs/hl2dm/maps_entities.md 3): a solid crate a
//! player opens with `+use` (one use a press) or by hitting it while
//! holding the crowbar (`HL2Player.Use`), giving one opening's ammo of its
//! `AmmoType` (data/items.toml's `[[crate]]`: Pistol 300, ... type 5 a
//! frag first if missing) capped by the carry limits.
//!
//! Each use fires `OnUsed`. A crate not already opening, with nothing on
//! its lid, opens for its user (the receiver; `AmmoCrate.Open`) and
//! thinks every 7 ticks: the first think after an idle spell advances its
//! `Open` sequence (35 frames at 30 fps) by 0.2 s, later ones by the time
//! since the last; when the sequence passes the pickup event (cycle
//! 0.882353) the receiver gets the ammo (the ammo shown empty if anything
//! was given). Every use puts the close 1.5 s after it; the first think at
//! or after that starts `Close`, and when `Close` ends `AmmoCrate.Close`
//! plays and the crate is full again. A use while it closes re-opens it
//! for the new user. Server-side.
//!
//! The crate is drawn by its model (`CrateModel`: the map loader places it
//! by its `AmmoType`, solid, `maps::fixed_models`); the model's collision is
//! the crate's, its own box off. Without a model (a map made in code) the
//! crate is its box.
//!
//! Known shortcuts: the lid test is a player standing on a 64 x 32 x 32
//! box around the origin's base (the stock crates' size, roughly), not the
//! model's bounds; the `+use` reach is a ray from the eye, 80 units; the
//! lid's `Open` and `Close` aren't drawn (the model holds `idle`) and the
//! ammo bodygroup is state only (`AmmoCrate::shown_empty`).

use std::collections::BTreeMap;

use avian3d::prelude::{Collider, RigidBody, SpatialQuery, SpatialQueryFilter};
use bevy::prelude::*;

use super::ammo::{self, ItemData};
use crate::{
    core::{Damage, Damageable, Health, Intent, MovementState, RoundRestarts, SimClock},
    map::{MapEntities, PlaySound, entities::FireEntityOutput},
};
use chimera_sim::weapon::{Inventory, Weapon};

const UNIT: f32 = 0.0254;
/// The crate's half size (units) and the `+use` reach.
const HALF: Vec3 = Vec3::new(32.0, 16.0, 16.0);
const USE_REACH: f32 = 80.0;
/// `Open` and `Close`: 35 frames at 30 fps; the pickup event's cycle.
pub const SEQUENCE: f64 = 35.0 / 30.0 - 1.0 / 30.0;
pub const PICKUP_CYCLE: f64 = 0.882353;
/// Think period (ticks at 0.015 s: 0.1 s), the most one think advances
/// (s), the close delay (s).
pub const THINK_TICKS: u64 = 7;
const MAX_ADVANCE: f64 = 0.2;
pub const CLOSE_DELAY: f64 = 1.5;

/// Where a crate's lid is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Lid {
    Idle,
    Opening(f64),
    Closing(f64),
}

/// An ammo crate.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct AmmoCrate {
    pub kind: usize,
    pub map_index: usize,
    pub lid: Lid,
    pub receiver: Option<Entity>,
    pub close_at: f64,
    pub next_think: Option<u64>,
    /// When the sequence last advanced (s).
    pub advanced: f64,
    /// The ammo bodygroup shows the crate empty.
    pub shown_empty: bool,
}

/// A crate came from this map.
#[derive(Component)]
struct MapCrate;

/// The crate's drawn model: the prop node the map loader placed for its
/// entity (`maps::fixed_models`, solid by its model). Its collision is the
/// crate's: the crate's own box is off, `+use` and crowbar hits on the node
/// count as on the crate.
#[derive(Component, Clone, Copy, Debug)]
pub struct CrateModel(pub Entity);

/// Each crate takes its model's node (by map index) when either appears.
pub(super) fn link_models(
    appeared: Query<(), Or<(Added<AmmoCrate>, Added<crate::map::PropEntity>)>>,
    crates: Query<(Entity, &AmmoCrate), Without<CrateModel>>,
    nodes: Query<(Entity, &crate::map::PropEntity)>,
    mut commands: Commands,
) {
    if appeared.is_empty() || crates.is_empty() {
        return;
    }
    for (c, a) in &crates {
        let Some((node, _)) = nodes.iter().find(|(_, p)| p.0 == a.map_index) else { continue };
        commands.entity(c).insert((CrateModel(node), avian3d::prelude::ColliderDisabled));
        commands.entity(node).insert(Damageable);
    }
}

pub(super) fn plugin(app: &mut App) {
    app.add_message::<Damage>();
}

fn spawn(world: &mut World, map: &MapEntities) {
    for (i, e) in map.entities.iter().enumerate() {
        if !e.classname().eq_ignore_ascii_case("item_ammo_crate") {
            continue;
        }
        let kind = e.get("AmmoType").or_else(|| e.get("ammotype")).and_then(|v| v.trim().parse::<usize>().ok()).unwrap_or(0);
        let origin = crate::map::entities::entity_to_engine(e.origin(), map.scale);
        let turn = crate::map::entities::rotation_to_engine(crate::map::entities::entity_rotation(e.angles()));
        let half = Vec3::new(HALF.x, HALF.z, HALF.y) * UNIT;
        world.spawn((
            Name::new("item_ammo_crate"),
            MapCrate,
            AmmoCrate {
                kind: kind.min(9),
                map_index: i,
                lid: Lid::Idle,
                receiver: None,
                close_at: f64::NEG_INFINITY,
                next_think: None,
                advanced: f64::NEG_INFINITY,
                shown_empty: false,
            },
            Transform::from_translation(origin + Vec3::Y * half.y).with_rotation(turn),
            RigidBody::Static,
            Collider::cuboid(2.0 * half.x, 2.0 * half.y, 2.0 * half.z),
            Damageable,
        ));
    }
}

/// The crates: made with the map, used (`+use` presses, crowbar hits),
/// thinking. What it finds is world work (the crates' state, the ammo),
/// queued in order.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn crates(
    map: Option<Res<MapEntities>>,
    restarts: Option<Res<RoundRestarts>>,
    mut seen: Local<(usize, u32)>,
    mut held: Local<BTreeMap<Entity, bool>>,
    mut hurt: MessageReader<Damage>,
    crates: Query<Entity, With<AmmoCrate>>,
    models: Query<(Entity, &CrateModel)>,
    users: Query<(Entity, &Transform, &Intent, &MovementState, &Health)>,
    carried: Query<&Inventory>,
    weapons: Query<&Weapon>,
    spatial: SpatialQuery,
    clock: Res<SimClock>,
    mut commands: Commands,
) {
    // Made again with a new map or a restart.
    if let Some(map) = map {
        let key = (std::sync::Arc::as_ptr(&map.entities) as usize, restarts.map_or(0, |r| r.0));
        if *seen != key {
            *seen = key;
            let map = map.clone();
            commands.queue(move |w: &mut World| {
                let old: Vec<Entity> = w.query_filtered::<Entity, With<MapCrate>>().iter(w).collect();
                for e in old {
                    w.despawn(e);
                }
                spawn(w, &map);
            });
        }
    }
    let hurt: Vec<Damage> = hurt.read().cloned().collect();
    if crates.is_empty() {
        return;
    }
    // A hit on a crate's model is on the crate.
    let crate_of = |e: Entity| {
        if crates.contains(e) {
            Some(e)
        } else {
            models.iter().find(|(_, m)| m.0 == e).map(|(c, _)| c)
        }
    };
    let mut uses: Vec<(Entity, Entity, bool)> = Vec::new();
    // `+use` presses that find a crate.
    for (who, t, i, s, h) in &users {
        let down = i.use_key && h.current > 0.0;
        let was = held.insert(who, down).unwrap_or(false);
        if !down || was {
            continue;
        }
        let dir = Quat::from_euler(EulerRot::YXZ, i.yaw, i.pitch, 0.0) * Vec3::NEG_Z;
        let filter = SpatialQueryFilter::from_excluded_entities([who]).with_mask(crate::core::SOLID_LAYERS);
        let Ok(dir) = Dir3::new(dir) else { continue };
        if let Some(hit) = spatial.cast_ray(t.translation + s.eye_offset, dir, USE_REACH * UNIT, true, &filter)
            && let Some(c) = crate_of(hit.entity)
        {
            uses.push((c, who, false));
        }
    }
    // Hits by a player holding the crowbar.
    for d in hurt {
        let Some(a) = d.attacker else { continue };
        let Some(target) = crate_of(d.target) else { continue };
        if uses.iter().any(|(c, w, _)| *c == target && *w == a) {
            continue;
        }
        let held = carried.get(a).ok().and_then(|i| i.active).and_then(|w| weapons.get(w).ok()).map(|w| w.id);
        if held == Some(super::weapons::CROWBAR) {
            uses.push((target, a, true));
        }
    }
    let (now, tick) = (clock.now, clock.tick);
    let all: Vec<Entity> = crates.iter().collect();
    commands.queue(move |w: &mut World| {
        for (c, who, crowbar) in uses {
            if crowbar {
                let at = w.get::<Transform>(who).map_or(Vec3::ZERO, |t| t.translation);
                sound(w, "HL2Player.Use", at, who);
            }
            use_crate(w, c, who, now, tick);
        }
        for c in all {
            think(w, c, now, tick);
        }
    });
}

fn sound(world: &mut World, entry: &str, at: Vec3, source: Entity) {
    world.write_message(PlaySound {
        entry: entry.into(),
        at: Some(at),
        volume: None,
        pitch: None,
        source: Some(source),
        channel: None,
    });
}

/// Something stands on the crate's lid.
fn lid_blocked(world: &mut World, c: Entity) -> bool {
    let Some(t) = world.get::<Transform>(c).copied() else { return false };
    let half = Vec3::new(HALF.x, HALF.z, HALF.y) * UNIT;
    let top = t.translation.y + half.y;
    world
        .query::<(&Transform, &MovementState, &Health)>()
        .iter(world)
        .any(|(pt, s, h)| {
            let feet = pt.translation + s.hull_min;
            h.current > 0.0
                && (feet.y - top).abs() < 2.0 * UNIT
                && (pt.translation.x - t.translation.x).abs() < half.x + 16.0 * UNIT
                && (pt.translation.z - t.translation.z).abs() < half.z + 16.0 * UNIT
        })
}

fn use_crate(world: &mut World, c: Entity, who: Entity, now: f64, tick: u64) {
    let Some(index) = world.get::<AmmoCrate>(c).map(|a| a.map_index) else { return };
    world.write_message(FireEntityOutput {
        map_index: index,
        output: "OnUsed".into(),
        activator: Some(who),
    });
    let opening = matches!(world.get::<AmmoCrate>(c).map(|a| a.lid), Some(Lid::Opening(_)));
    if !opening {
        if lid_blocked(world, c) {
            return;
        }
        let at = world.get::<Transform>(c).map_or(Vec3::ZERO, |t| t.translation);
        sound(world, "AmmoCrate.Open", at, c);
        let mut a = world.get_mut::<AmmoCrate>(c).unwrap();
        a.lid = Lid::Opening(0.0);
        a.receiver = Some(who);
        a.next_think = Some(tick + THINK_TICKS);
    }
    world.get_mut::<AmmoCrate>(c).unwrap().close_at = now + CLOSE_DELAY;
}

fn think(world: &mut World, c: Entity, now: f64, tick: u64) {
    let Some(a) = world.get::<AmmoCrate>(c).cloned() else { return };
    if a.next_think.is_none_or(|t| tick < t) {
        return;
    }
    let step = (now - a.advanced).min(MAX_ADVANCE) / SEQUENCE;
    let mut next = a.clone();
    next.advanced = now;
    next.next_think = Some(tick + THINK_TICKS);
    match a.lid {
        Lid::Opening(cycle) => {
            let cycle = (cycle + step).min(1.0);
            next.lid = Lid::Opening(cycle);
            if cycle >= PICKUP_CYCLE
                && let Some(r) = a.receiver
            {
                next.receiver = None;
                let kind = world.resource::<ItemData>().crates.get(a.kind).cloned();
                if let Some(k) = kind {
                    if let Some(w) = &k.weapon
                        && !carries(world, r, w)
                    {
                        chimera_sim::weapon::give(world, r, w);
                    }
                    if ammo::give(world, r, &k.ammo, k.count) > 0 {
                        next.shown_empty = true;
                    }
                }
            }
            if now + 1e-9 >= a.close_at {
                next.receiver = None;
                next.lid = Lid::Closing(0.0);
            }
        }
        Lid::Closing(cycle) => {
            let cycle = cycle + step;
            if cycle >= 1.0 - 1e-9 {
                let at = world.get::<Transform>(c).map_or(Vec3::ZERO, |t| t.translation);
                sound(world, "AmmoCrate.Close", at, c);
                next.lid = Lid::Idle;
                next.shown_empty = false;
                next.next_think = None;
            } else {
                next.lid = Lid::Closing(cycle);
            }
        }
        Lid::Idle => next.next_think = None,
    }
    *world.get_mut::<AmmoCrate>(c).unwrap() = next;
}

fn carries(world: &World, who: Entity, id: &str) -> bool {
    world
        .get::<Inventory>(who)
        .is_some_and(|i| i.weapons.iter().any(|w| world.get::<Weapon>(*w).is_some_and(|x| x.id == id)))
}
