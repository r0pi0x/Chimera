//! Deliveries: hitscan shots (spec 4) and melee swings (spec 6.1), and the
//! damage they deal. Damage to one target within one firing call is summed
//! and applied once (spec 4.5). Bullets of weapons with `Penetration` go on
//! through walls and characters (measured M13); a target's `BulletTarget`
//! may give its own hitgroup multipliers, let a bullet go on inside it
//! from the box it passed (so it hits another of its boxes) and keep its
//! damage exact (CS:S's hostages, objectives.md Q8). The shape a ray
//! enters first decides the body region (`core::BodyRegion`: a body's
//! `Hitboxes`, boxes or spheres, whichever game made them; without any,
//! height bands on its collider), the gun's table (`DamageEffect::regions`)
//! or the target's own its multiplier, and armour takes its share through
//! the shared step (`HitArmor`, `hits::absorb`;
//! docs/DESIGN.md "Hits across games"). Each bullet hit on
//! something with health is also told as a `BulletHit` (the damage before
//! armour and truncation, whether armour covered it, a helmet). A
//! spawn-protected character (`core::SpawnProtected`) in its bullets
//! window isn't there for a shot or a swing: the trace passes through it
//! and hits what is behind (the wall, another character), on the server
//! and in a client's predicted and others' redrawn shots alike. In the
//! damage window alone a hit on it shows and deals nothing: its trace is
//! told with no `hit`, no push, no damage, a swing as a miss, and the
//! round ends there. A body its shooter holds (`map::held::Held`) isn't
//! there for the shooter's own shots either (gravity_gun.md 5).

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{
    lagcomp::Rewound,
    random::Rng,
    BulletTarget, DamageEffect, Hitscan, PassMaterials, Penetration, SpreadShape, Swing, WeaponEvent,
    WeaponEventKind,
};
use crate::{
    core::{
        BodyRegion, Damage, DamageKind, Damageable, FirstTimePredicted, Health, Hitboxes, Hitgroup, Intent, NetRole,
        SOLID_LAYERS,
    },
    map::{
        PlaySound, PropSurface,
        sound::{SoundBank, SurfaceGrid},
    },
};

/// How far past a boundary a bullet steps before looking again, m.
const STEP: f32 = 1e-3;
/// How far from a hit point a world triangle may be to give its surface, m.
const SURFACE_REACH: f32 = 0.3;

#[derive(bevy::ecs::query::QueryData)]
pub(super) struct Target {
    entity: Entity,
    transform: &'static Transform,
    aabb: Option<&'static ColliderAabb>,
    health: Option<&'static Health>,
    damageable: Option<&'static Damageable>,
    intent: Option<&'static Intent>,
    hitboxes: Option<&'static Hitboxes>,
    bullet: Option<&'static BulletTarget>,
}

/// The world access deliveries need.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct World<'w, 's> {
    spatial: SpatialQuery<'w, 's>,
    targets: Query<'w, 's, Target>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    damage: MessageWriter<'w, Damage>,
    /// A hit's full region beside its damage (`core::RegionHit`).
    regions: Option<ResMut<'w, Messages<crate::core::RegionHit>>>,
    armor: super::HitArmor<'w, 's>,
    props: Query<'w, 's, &'static PropSurface>,
    prop_entities: Query<'w, 's, (), With<crate::map::PropEntity>>,
    kinds: Query<'w, 's, &'static RigidBody>,
    bank: Option<Res<'w, SoundBank>>,
    grid: Option<Res<'w, SurfaceGrid>>,
    materials: Res<'w, PassMaterials>,
    pub events: MessageWriter<'w, WeaponEvent>,
    hits: MessageWriter<'w, super::BulletHit>,
    play: MessageWriter<'w, PlaySound>,
    first: Res<'w, FirstTimePredicted>,
    role: Option<Res<'w, NetRole>>,
    playback: Option<Res<'w, crate::core::Playback>>,
    impulses: MessageWriter<'w, super::impact::BulletImpulse>,
    /// Spawn-protected characters (`core::SpawnProtected`): passed
    /// through in the bullets window, a hit on them showing nothing.
    protected: Query<'w, 's, (Entity, &'static crate::core::SpawnProtected)>,
    /// Bodies players hold (`map::held`): a holder's shots pass its own.
    held: Query<'w, 's, (Entity, &'static crate::map::held::Held, Option<&'static avian3d::prelude::RigidBodyColliders>)>,
}

impl World<'_, '_> {
    /// Play a sound, the first time this command runs only.
    pub fn sound(&mut self, sound: PlaySound) {
        if self.first.0 {
            self.play.write(sound);
        }
    }

    /// Tell what a weapon did (marked a replay when the command runs again).
    pub fn event(&mut self, owner: Entity, weapon: Entity, kind: WeaponEventKind) {
        self.events.write(WeaponEvent {
            owner,
            weapon,
            kind,
            replay: !self.first.0,
        });
    }

    /// Whether this process decides outcomes (damage, pushes): not a
    /// network client, which only predicts its own shots
    /// (`core::authoritative`).
    pub fn authoritative(&self) -> bool {
        self.role.as_deref().is_none_or(|r| *r != NetRole::Client) && self.playback.is_none()
    }
}

/// Everything one firing call needs.
pub(super) struct Shot<'a, 'w, 's> {
    pub owner: Entity,
    pub weapon: Entity,
    pub eye: Vec3,
    /// The aim (`aim_of(yaw, pitch)`) and its angles.
    pub aim: Quat,
    pub yaw: f32,
    pub pitch: f32,
    pub seed: u32,
    /// The time of the shot (game seconds).
    pub now: f64,
    /// Characters moved back to where the shooter saw them (lag
    /// compensation, `lagcomp`): traced as these instead of where they
    /// are.
    pub rewound: Vec<Rewound>,
    /// Pass through every character (others' rounds traced again on a
    /// client, `Hitscan::redrawn_through_characters`).
    pub through_characters: bool,
    /// Tell what each trace hit (false: the `Shot` events say nothing
    /// was hit, so no impact is drawn; a round the server says hit
    /// nothing, `remote::shot_flags::IMPACTS`).
    pub show_hits: bool,
    pub w: &'a mut World<'w, 's>,
}

/// A shot's aim from its look angles (the same on every machine for the
/// same angles: a remote client draws others' shots from them).
pub fn aim_of(yaw: f32, pitch: f32) -> Quat {
    Quat::from_euler(EulerRot::YXZ, yaw, pitch, 0.0)
}

/// The shape a ray enters first among `boxes` (local space) within
/// `limit` (CS:S's rule, `Hitboxes::nearest`): where it enters and leaves
/// it, and its region.
fn nearest_box(boxes: &[crate::core::Hitbox], o: Vec3, d: Vec3, limit: f32) -> Option<(f32, f32, BodyRegion)> {
    boxes
        .iter()
        .filter_map(|b| b.ray_span(o, d).map(|(enter, leave)| (enter, leave, b.region)))
        .filter(|(t, ..)| *t < limit)
        .min_by(|a, b| a.0.total_cmp(&b.0))
}

/// What a ray hits of a character's volume: the nearest hitbox entered
/// (distance, where it leaves that box, and its group) or, without
/// hitboxes, its box.
fn volume_entry(r: &Rewound, from: Vec3, dir: Dir3, limit: f32) -> Option<(f32, Option<f32>, Option<BodyRegion>)> {
    match &r.hitboxes {
        Some(boxes) => {
            let (feet, turn) = r.frame();
            let (o, d) = (turn * (from - feet), turn * *dir);
            nearest_box(boxes, o, d, limit).map(|(enter, leave, group)| (enter, Some(leave), Some(group)))
        }
        None => {
            let b = r.aabb();
            let (enter, _) = aabb_span(&b, from, *dir)?;
            (enter < limit).then(|| (enter, None, Some(BodyRegion::from_hitgroup(hitgroup_at(&b, from + *dir * enter)))))
        }
    }
}

/// Where a ray enters and leaves a world box (distances along `dir`, the
/// entry clamped at 0).
fn aabb_span(b: &ColliderAabb, from: Vec3, dir: Vec3) -> Option<(f32, f32)> {
    let centre = (b.min + b.max) / 2.0;
    let half = (b.max - b.min) / 2.0;
    crate::core::Hitbox::cuboid(centre, half, Quat::IDENTITY, crate::core::BodyRegion::Generic).ray_span(from, dir)
}

/// One hit to apply.
struct Hit {
    /// The body hit, and its collider (props may have child colliders).
    entity: Entity,
    collider: Entity,
    point: Vec3,
    dir: Vec3,
    /// From the eye.
    distance: f32,
    normal: Vec3,
    /// The hitbox's region, when the target has hitboxes.
    group: Option<BodyRegion>,
    /// Where the ray leaves that hitbox, from the eye.
    box_exit: Option<f32>,
}

impl Shot<'_, '_, '_> {
    fn filter(&self) -> SpatialQueryFilter {
        SpatialQueryFilter::from_excluded_entities(std::iter::once(self.owner).chain(self.passing())).with_mask(SOLID_LAYERS)
    }

    /// Spawn-protected characters attacks pass through now
    /// (`core::SpawnProtected::passes_attacks`).
    fn passing(&self) -> Vec<Entity> {
        self.w
            .protected
            .iter()
            .filter(|(_, p)| p.passes_attacks(self.now))
            .map(|(e, _)| e)
            .chain(crate::map::held::held_by(self.owner, self.w.held.iter()))
            .chain(
                self.w
                    .targets
                    .iter()
                    .filter(|t| self.through_characters && t.intent.is_some())
                    .map(|t| t.entity),
            )
            .collect()
    }

    /// Whether a hit on `e` shows and deals nothing now (spawn protection).
    fn hides(&self, e: Entity) -> bool {
        self.w.protected.get(e).is_ok_and(|(_, p)| p.hides_hits(self.now))
    }

    /// The body a collider belongs to (props may have child colliders).
    fn body(&self, collider: Entity) -> Entity {
        self.w.colliders.get(collider).map_or(collider, |c| c.body)
    }

    /// The first thing along the ray between `start` and `end` meters from
    /// the eye, leaving out `skip`. Characters with hitboxes are hit only
    /// where a hitbox is (shots pass through the rest of their hull, and
    /// hitboxes poking out of it still count, as in Source); the nearest
    /// box entered decides the hitgroup.
    fn trace(&self, dir: Dir3, start: f32, end: f32, skip: &[Entity]) -> Option<Hit> {
        let from = self.eye + *dir * start;
        let rewound: Vec<Entity> = self.rewound.iter().map(|r| r.entity).collect();
        let boxed: Vec<Entity> = self
            .w
            .targets
            .iter()
            .filter(|t| t.hitboxes.is_some() && t.entity != self.owner && !rewound.contains(&t.entity))
            .map(|t| t.entity)
            .collect();
        // The dead are left out entirely (their body is a ragdoll, which
        // bullets pass through, or hidden), and so are the spawn-protected
        // in their bullets window (`passing`).
        let dead = self
            .w
            .targets
            .iter()
            .filter(|t| t.intent.is_some() && t.health.is_some_and(|h| h.current <= 0.0))
            .map(|t| t.entity);
        let dead: Vec<Entity> = dead.chain(self.passing()).collect();
        let filter = SpatialQueryFilter::from_excluded_entities(
            std::iter::once(self.owner)
                .chain(boxed.iter().copied())
                .chain(rewound.iter().copied())
                .chain(dead.iter().copied())
                .chain(skip.iter().copied()),
        )
        .with_mask(SOLID_LAYERS);
        let boxed: Vec<Entity> = boxed
            .into_iter()
            .filter(|e| self.w.targets.get(*e).ok().and_then(|t| t.health).is_none_or(|h| h.current > 0.0))
            .filter(|e| !dead.contains(e))
            .collect();
        let mut best = self
            .w
            .spatial
            .cast_ray(from, dir, end - start, true, &filter)
            .map(|hit| Hit {
                entity: self.body(hit.entity),
                collider: hit.entity,
                point: from + *dir * hit.distance,
                dir: *dir,
                distance: start + hit.distance,
                normal: hit.normal,
                group: None,
                box_exit: None,
            });
        for e in boxed.into_iter().filter(|e| !skip.contains(e)) {
            let Some((o, d)) = self.local_ray(e, from, dir) else {
                continue;
            };
            let Some(boxes) = self.w.targets.get(e).ok().and_then(|t| t.hitboxes) else {
                continue;
            };
            let limit = best.as_ref().map_or(end, |b| b.distance) - start;
            if let Some((distance, leave, group)) = nearest_box(&boxes.0, o, d, limit) {
                best = Some(Hit {
                    entity: e,
                    collider: e,
                    point: from + *dir * distance,
                    dir: *dir,
                    distance: start + distance,
                    normal: -*dir,
                    group: Some(group),
                    box_exit: Some(start + leave),
                });
            }
        }
        // Characters where the shooter saw them (lag compensation).
        for r in self.rewound.iter().filter(|r| !skip.contains(&r.entity) && !dead.contains(&r.entity)) {
            let limit = best.as_ref().map_or(end, |b| b.distance) - start;
            if let Some((distance, leave, group)) = volume_entry(r, from, dir, limit) {
                best = Some(Hit {
                    entity: r.entity,
                    collider: r.entity,
                    point: from + *dir * distance,
                    dir: *dir,
                    distance: start + distance,
                    normal: -*dir,
                    group,
                    box_exit: leave.map(|l| start + l),
                });
            }
        }
        best
    }

    /// A ray in a character's hitbox frame: feet at the origin, facing -Z.
    fn local_ray(&self, character: Entity, origin: Vec3, dir: Dir3) -> Option<(Vec3, Vec3)> {
        let t = self.w.targets.get(character).ok()?;
        let feet = t
            .aabb
            .map_or(t.transform.translation, |b| t.transform.translation.with_y(b.min.y));
        let turn = t.intent.map_or(Quat::IDENTITY, |i| i.yaw_rotation()).inverse();
        Some((turn * (origin - feet), turn * *dir))
    }

    /// Whether what was hit is a character (bullets pass those by
    /// `PassMaterials::character`).
    fn is_character(&self, body: Entity) -> bool {
        self.w
            .targets
            .get(body)
            .is_ok_and(|t| t.intent.is_some() || t.hitboxes.is_some())
    }

    /// Where the bullet leaves a character it hit, as a distance from the
    /// eye: past its last hitbox along the ray, or out of its collider.
    fn character_exit(&self, hit: &Hit) -> Option<f32> {
        let dir = Dir3::new(hit.dir).ok()?;
        if let Some(r) = self.rewound.iter().find(|r| r.entity == hit.entity) {
            return match &r.hitboxes {
                Some(boxes) => {
                    let (feet, turn) = r.frame();
                    let (o, d) = (turn * (self.eye - feet), turn * *dir);
                    boxes.iter().filter_map(|b| b.ray_span(o, d)).map(|(_, out)| out).reduce(f32::max)
                }
                None => aabb_span(&r.aabb(), self.eye, *dir).map(|(_, out)| out),
            };
        }
        if let Some(boxes) = self.w.targets.get(hit.entity).ok().and_then(|t| t.hitboxes) {
            let (o, d) = self.local_ray(hit.entity, self.eye, dir)?;
            return boxes
                .0
                .iter()
                .filter_map(|b| b.ray_span(o, d))
                .map(|(_, out)| out)
                .reduce(f32::max);
        }
        let from = hit.point + *dir * STEP;
        self.w
            .spatial
            .cast_ray_predicate(from, dir, 10.0, false, &SpatialQueryFilter::default(), &|e| {
                self.body(e) == hit.entity
            })
            .map(|h| hit.distance + STEP + h.distance)
    }

    /// Where a bullet that entered something solid `entry` meters from the
    /// eye comes out of it, if that's within `limit` meters: it steps from
    /// boundary to boundary while inside any solid collider (adjacent
    /// brushes count as one wall). None also when there is nothing to pass
    /// (the point just past the entry isn't inside a solid: open surfaces
    /// such as displacements).
    fn solid_exit(&self, dir: Dir3, entry: f32, limit: f32) -> Option<f32> {
        let filter = SpatialQueryFilter::from_excluded_entities([self.owner]).with_mask(SOLID_LAYERS);
        let mut t = entry;
        for _ in 0..64 {
            let p = self.eye + *dir * (t + STEP);
            let mut inside = Vec::new();
            self.w.spatial.point_intersections_callback(p, &filter, |e| {
                if !self.is_character(self.body(e)) {
                    inside.push(e);
                }
                true
            });
            if inside.is_empty() {
                return (t > entry).then_some(t);
            }
            let left = entry + limit - t - STEP;
            if left <= 0.0 {
                return None;
            }
            let hit = self
                .w
                .spatial
                .cast_ray_predicate(p, dir, left, false, &filter, &|e| inside.contains(&e))?;
            t += STEP + hit.distance;
        }
        None
    }

    /// A map entity or a moving body (not the world or a static prop), for
    /// `PassMaterials::entity_twice_from`.
    fn is_entity(&self, body: Entity) -> bool {
        self.w.prop_entities.contains(body)
            || self
                .w
                .kinds
                .get(body)
                .is_ok_and(|b| !matches!(b, RigidBody::Static))
    }

    /// The material class of the surface hit (`MapSurface::game_material`):
    /// a prop's own surface property, else the nearest world triangle's.
    fn material_class(&self, hit: &Hit) -> Option<char> {
        let name = match self.w.props.get(hit.collider).or_else(|_| self.w.props.get(hit.entity)) {
            Ok(s) => s.0.clone(),
            Err(_) => self.w.grid.as_ref()?.nearest(hit.point, SURFACE_REACH)?.to_string(),
        };
        let sounds = &self.w.bank.as_ref()?.0;
        sounds
            .surface(&name)
            .or_else(|| sounds.surface("default"))
            .map(|s| s.game_material)
    }

    /// Fire every pellet of one shot.
    pub fn fire(&mut self, scan: &Hitscan, effect: &DamageEffect, pen: Option<&Penetration>) {
        let mut total: Vec<(Entity, f32, BodyRegion, Vec3, Vec3)> = Vec::new();
        for dir in pellet_dirs(self.aim, self.seed, scan.spread, scan.pellets) {
            // The bullet's damage so far (before hitgroups), where the
            // current segment starts and how far it may go, and what it
            // passed.
            let mut carried = effect.amount;
            let (mut start, mut end) = (0.0, scan.range);
            let mut skip: Vec<Entity> = Vec::new();
            let mut passed = 0;
            let mut budget = pen.map_or(0.0, |p| p.power);
            loop {
                let hit = self.trace(dir, start, end, &skip);
                let (from, to) = (
                    self.eye + *dir * start,
                    hit.as_ref().map_or(self.eye + *dir * end, |h| h.point),
                );
                // A spawn-protected character past its bullets window but
                // in its damage one (`core::SpawnProtected`; in the bullets
                // window `trace` passes it): the trace is told with no hit
                // (nothing to draw), it takes nothing, and the round ends
                // there.
                let protected = hit.as_ref().is_some_and(|h| self.hides(h.entity));
                let (entity, normal) = match protected || !self.show_hits {
                    true => (None, None),
                    false => (hit.as_ref().map(|h| h.entity), hit.as_ref().map(|h| h.normal)),
                };
                let kind = if passed == 0 {
                    WeaponEventKind::Shot {
                        from,
                        to,
                        hit: entity,
                        normal,
                    }
                } else {
                    WeaponEventKind::ShotContinued {
                        from,
                        to,
                        hit: entity,
                        normal,
                    }
                };
                self.w.event(self.owner, self.weapon, kind);
                let Some(hit) = hit else { break };
                if protected {
                    break;
                }
                self.push(hit.entity, hit.point, hit.dir * effect.impulse);
                // Falloff again at every hit, by the distance from the eye.
                carried *= falloff(effect.falloff, effect.falloff_step, hit.distance);
                if let Ok(t) = self.w.targets.get(hit.entity)
                    && (t.health.is_some() || t.damageable.is_some())
                {
                    // Plain objects (breakables) have no hitgroups.
                    let group = match (hit.group, t.health) {
                        (Some(g), _) => g,
                        (None, Some(_)) => t
                            .aabb
                            .map_or(BodyRegion::Generic, |b| BodyRegion::from_hitgroup(hitgroup_at(b, hit.point))),
                        (None, None) => BodyRegion::Generic,
                    };
                    // The gun's table, or the target's own (hostages).
                    let own = t.bullet.and_then(|b| b.hitgroups).map(|h| super::RegionScale::from_hitgroups(&h));
                    let amount = carried * own.as_ref().unwrap_or(&effect.regions).get(group);
                    match total.iter_mut().find(|t| t.0 == hit.entity) {
                        Some(t) => {
                            t.1 += amount;
                            t.2 = group;
                            t.3 = hit.point;
                        }
                        None => total.push((hit.entity, amount, group, hit.point, hit.dir)),
                    }
                }
                // Through it?
                let Some(pen) = pen else { break };
                if passed >= pen.objects || hit.distance >= pen.max_distance {
                    break;
                }
                let character = self.is_character(hit.entity);
                // Through only the box hit, and on inside the character
                // (`BulletTarget::hits_inside`), or out of it whole.
                let inside = character
                    && hit.box_exit.is_some()
                    && self
                        .w
                        .targets
                        .get(hit.entity)
                        .ok()
                        .and_then(|t| t.bullet)
                        .is_some_and(|b| b.hits_inside);
                let (exit, cost, factor) = if character {
                    let pass = self.w.materials.character;
                    let exit = if inside { hit.box_exit } else { self.character_exit(&hit) };
                    let Some(exit) = exit else { break };
                    (exit, pass.cost, pass.damage)
                } else {
                    let m = self.w.materials.get(self.material_class(&hit));
                    if m.scale <= 0.0 {
                        break;
                    }
                    let Some(exit) = self.solid_exit(dir, hit.distance, budget * m.scale) else {
                        break;
                    };
                    let thick = exit - hit.distance;
                    let twice = thick >= self.w.materials.entity_twice_from && self.is_entity(hit.entity);
                    (exit, thick / m.scale, if twice { m.damage * m.damage } else { m.damage })
                };
                if cost > budget {
                    break;
                }
                budget -= cost;
                carried *= factor;
                passed += 1;
                if character {
                    if !inside {
                        skip.push(hit.entity);
                    }
                    end = exit + (end - exit) * self.w.materials.character.range_scale;
                }
                start = exit.max(hit.distance) + STEP;
                if start >= end {
                    break;
                }
            }
        }
        for (target, amount, hitgroup, point, dir) in total {
            let exact = self.w.targets.get(target).ok().and_then(|t| t.bullet).is_some_and(|b| b.exact);
            self.apply(
                target,
                amount,
                effect.armor_ratio,
                if exact { 0.0 } else { effect.quantum },
                hitgroup,
                point,
                dir,
                DamageKind::Bullet,
                true,
                dir * effect.impulse,
            );
        }
    }

    /// Deal `raw` damage on `region`: armour (if the weapon's ratio
    /// applies and it covers the region, `HitArmor`) takes its share, then
    /// both are truncated.
    #[allow(clippy::too_many_arguments)]
    fn apply(
        &mut self,
        target: Entity,
        raw: f32,
        armor_ratio: Option<f32>,
        quantum: f32,
        region: BodyRegion,
        point: Vec3,
        dir: Vec3,
        kind: DamageKind,
        shot: bool,
        force: Vec3,
    ) {
        // Damage is dealt once, not again when a command is re-run, and
        // only by the server (a client's own shots show their effects;
        // the hits are the server's).
        if !self.w.first.0 || !self.w.authoritative() {
            return;
        }
        // Only characters' health is whole points: objects (props,
        // breakables) take the exact amount and truncate it after their
        // own multipliers (CS:S, measured 2026-10-17;
        // specs/source/prop_damage.md 4).
        let quantum = if self.w.targets.get(target).is_ok_and(|t| t.health.is_some()) {
            quantum
        } else {
            0.0
        };
        let mut damage = Damage {
            force,
            target,
            attacker: Some(self.owner),
            amount: quantize(raw, quantum),
            point,
            dir,
            hitgroup: region.hitgroup(),
            kind,
            weapon: None,
        };
        let hitgroup = damage.hitgroup;
        // Armour takes its share only if the hit lands (`HitArmor`).
        let super::ArmorHit { covered, helmet } = self.w.armor.apply_region(&mut damage, region, raw, armor_ratio, quantum);
        let amount = damage.amount;
        // A gun's bullet (a knife's hit deals bullet damage in CS:S too,
        // but isn't one).
        if shot && kind == DamageKind::Bullet && self.w.targets.get(target).is_ok_and(|t| t.health.is_some()) {
            self.w.hits.write(super::BulletHit {
                damage: damage.clone(),
                weapon: self.weapon,
                raw,
                covered,
                helmet,
            });
        }
        if let Some(regions) = self.w.regions.as_mut() {
            regions.write(crate::core::RegionHit { target, region });
        }
        self.w.damage.write(damage);
        self.w.event(self.owner, self.weapon, WeaponEventKind::Hit {
                target,
                amount,
                hitgroup,
            });
    }

    /// Give a dynamic body an impulse at a point.
    fn push(&mut self, body: Entity, at: Vec3, impulse: Vec3) {
        if impulse == Vec3::ZERO || !self.w.first.0 || !self.w.authoritative() {
            return;
        }
        // Applied after the frame, as every game's bullets are
        // (`impact::apply_bullet_impulses`, with `phys_pushscale`).
        if self.w.kinds.get(body).is_ok_and(|rb| rb.is_dynamic()) {
            self.w.impulses.write(super::impact::BulletImpulse { body, point: at, impulse });
        }
    }

    /// One swing (spec 6.1): a line, then a box when the line misses.
    /// Returns whether it hit anything.
    pub fn swing(&mut self, swing: &Swing, secondary: bool) -> bool {
        let forward = self.aim * Vec3::NEG_Z;
        let dir = Dir3::new(forward).unwrap_or(Dir3::NEG_Z);
        let mut hit = self.trace(dir, 0.0, swing.range, &[]);
        // The line's span (its end on a miss: the range's end).
        let line = (self.eye, hit.as_ref().map_or(self.eye + *dir * swing.range, |h| h.point));
        if hit.is_none()
            && let Some(half) = swing.hull
        {
            // Box swept to the range minus its corner distance, counting
            // only targets roughly in front.
            let reach = swing.hull_reach;
            let shape = Collider::cuboid(half * 2.0, half * 2.0, half * 2.0);
            let config = ShapeCastConfig::from_max_distance(reach);
            if let Some(h) = self
                .w
                .spatial
                .cast_shape(&shape, self.eye, Quat::IDENTITY, dir, &config, &self.filter())
            {
                let entity = self.body(h.entity);
                let origin = self.w.targets.get(entity).map_or(h.point1, |t| t.transform.translation);
                let to = (origin - self.eye).normalize_or_zero();
                if to.dot(forward) >= swing.facing_cos {
                    hit = Some(Hit {
                        entity,
                        collider: h.entity,
                        point: h.point1,
                        dir: forward,
                        distance: h.distance,
                        normal: -h.normal1,
                        group: None,
                        box_exit: None,
                    });
                }
            }
        }
        // A spawn-protected character in its bullets window isn't there
        // (`trace` and `filter` leave it out: the swing goes on to what is
        // behind); in its damage window alone it takes the swing as a miss:
        // nothing shows, nothing lands (`core::SpawnProtected`).
        let hit = hit.filter(|h| !self.hides(h.entity));
        self.w.event(self.owner, self.weapon, WeaponEventKind::Swing {
                hit: hit.is_some(),
                secondary,
                at: hit.as_ref().map(|h| (h.point, h.normal, h.entity)),
                line,
            });
        let Some(hit) = hit else {
            if let Some(s) = &swing.sound_miss {
                self.w.sound(PlaySound::at(s.clone(), self.eye));
            }
            return false;
        };
        let (alive, object, yaw, aabb, origin) = match self.w.targets.get(hit.entity) {
            Ok(t) => (
                t.health.is_some(),
                t.damageable.is_some(),
                t.intent.map(|i| i.yaw_rotation()),
                t.aabb.copied(),
                Some(t.transform.translation),
            ),
            Err(_) => (false, false, None, None, None),
        };
        let mut amount = swing.damage;
        if let (Some(back), Some(yaw), Some(origin)) = (swing.backstab, yaw, origin) {
            // Backstab: the target faces along the bearing from the
            // attacker to it (not the attacker's view; measured M11).
            let facing = (yaw * Vec3::NEG_Z).with_y(0.0).normalize_or_zero();
            let bearing = (origin - self.eye).with_y(0.0).normalize_or_zero();
            if facing.dot(bearing) >= BACKSTAB_COS {
                amount = back;
            }
        }
        self.push(hit.entity, hit.point, hit.dir * swing.force);
        let sound = if alive {
            &swing.sound_hit
        } else {
            &swing.sound_hit_world
        };
        if let Some(s) = sound {
            self.w.sound(PlaySound::at(s.clone(), hit.point));
        }
        if alive || object {
            let group = match aabb.filter(|_| swing.hitgroups && alive) {
                Some(b) => BodyRegion::from_hitgroup(hitgroup_at(&b, hit.point)),
                None => BodyRegion::Generic,
            };
            self.apply(
                hit.entity,
                amount,
                swing.armor_ratio,
                swing.quantum,
                group,
                hit.point,
                hit.dir,
                swing.kind,
                false,
                Vec3::ZERO,
            );
        }
        true
    }
}

/// Each pellet's direction for a shot along `aim` with random seed `seed`
/// (spec 4.2: pellet `i` reseeds with `seed + 1 + i`). The inaccuracy
/// part of a `Disc` is drawn once per shot, shared by its pellets; each
/// pellet draws its own spread part (a shotgun's pattern keeps its shape
/// while the whole of it wanders). The same on every machine for the same
/// inputs: others' shots are drawn from them.
pub fn pellet_dirs(aim: Quat, seed: u32, spread: SpreadShape, pellets: u32) -> Vec<Dir3> {
    let mut shared: Option<Vec2> = None;
    (0..pellets.max(1))
        .map(|pellet| {
            let mut rng = Rng::new(seed.wrapping_add(1 + pellet));
            match spread {
                SpreadShape::Template(s) => {
                    let x = rng.range(-0.5, 0.5) + rng.range(-0.5, 0.5);
                    let y = rng.range(-0.5, 0.5) + rng.range(-0.5, 0.5);
                    spread_dir(aim, s, x, y)
                }
                SpreadShape::Cone(s) => {
                    // Source's generic rule, a player's bias: flatness 0.5,
                    // drawn again until inside the unit disc.
                    let (mut x, mut y);
                    let mut tries = 0;
                    loop {
                        x = rng.range(-1.0, 1.0) * 0.5 + rng.range(-1.0, 1.0) * 0.5;
                        y = rng.range(-1.0, 1.0) * 0.5 + rng.range(-1.0, 1.0) * 0.5;
                        tries += 1;
                        if x * x + y * y <= 1.0 || tries >= 64 {
                            break;
                        }
                    }
                    cone_dir(aim, s, x, y)
                }
                SpreadShape::Disc { inaccuracy, spread } => {
                    let mut disc = |r: f32| {
                        let angle = rng.range(0.0, std::f32::consts::TAU);
                        Vec2::from_angle(angle) * rng.range(0.0, 1.0) * r
                    };
                    let base = *shared.get_or_insert_with(|| disc(inaccuracy));
                    let offset = base + disc(spread);
                    spread_dir(aim, 1.0, offset.x, offset.y)
                }
            }
        })
        .collect()
}

/// Backstab facing test: within 36.87° (CS:S measured: 36° backstab, 37°
/// front).
const BACKSTAB_COS: f32 = 0.8;

/// A hit of `raw` damage on covering armour (CS:S, measured M7): health
/// takes `raw x ratio x 0.5` truncated to `quantum`, armour half the
/// rest, and what armour is left is truncated: a fraction of a point
/// costs a whole one (measured 2026-10-17, weapons.md M7: a knife slash
/// on 100 armour leaves 98, `player_hurt`'s dmg_armor says 1; an AK-47
/// chest hit's 4.04 leaves 95). When `available` armour can't take its
/// part, it is all used and health takes `raw - 2 x available` (measured
/// on C4 blasts, objectives.md Q3; bullets assumed alike: CS:S's player
/// damage code is shared). Returns (to health, armour taken).
pub fn armor_split(raw: f32, ratio: f32, available: f32, quantum: f32) -> (f32, f32) {
    let to_health = raw * ratio * 0.5;
    let to_armor = (raw - to_health) * 0.5;
    if to_armor > available {
        return (quantize(raw - available * 2.0, quantum), available);
    }
    let left = if quantum > 0.0 {
        quantize(available - to_armor, quantum)
    } else {
        available - to_armor
    };
    (quantize(to_health, quantum), available - left)
}

/// `amount` truncated to whole multiples of `quantum` (0: unchanged),
/// tolerant of float error just below a whole step.
pub fn quantize(amount: f32, quantum: f32) -> f32 {
    if quantum <= 0.0 {
        return amount;
    }
    (amount / quantum + 1e-4).floor() * quantum
}

/// Aim direction bent by spread factors `x`, `y` (spec 4.3, CS template):
/// normalize(forward + x·s·right + y·s·up).
pub fn spread_dir(aim: Quat, spread: f32, x: f32, y: f32) -> Dir3 {
    let f = aim * Vec3::NEG_Z;
    let r = aim * Vec3::X;
    let u = aim * Vec3::Y;
    Dir3::new(f + r * (x * spread) + u * (y * spread)).unwrap_or(Dir3::NEG_Z)
}

/// A `SpreadShape::Cone` pellet's direction: forward + x s right + y s up
/// of the aim, where right is level (forward x world up) and up follows
/// from them (Source's generic rule: no roll, a vertical aim's own axes).
pub fn cone_dir(aim: Quat, spread: f32, x: f32, y: f32) -> Dir3 {
    let f = aim * Vec3::NEG_Z;
    let (r, u) = match f.cross(Vec3::Y).try_normalize() {
        Some(r) => (r, r.cross(f).normalize()),
        None => (aim * Vec3::X, aim * Vec3::Y),
    };
    Dir3::new(f + r * x * spread + u * y * spread).unwrap_or(Dir3::new(f).unwrap_or(Dir3::NEG_Z))
}

/// Damage scale after `distance`: base^(distance / step).
pub fn falloff(base: f32, step: f32, distance: f32) -> f32 {
    if step <= 0.0 {
        return 1.0;
    }
    base.powf(distance / step)
}

/// Hitgroup from where on a body's bounds a hit landed, until bodies carry
/// real hitboxes: height bands from CS:S's player hitboxes (head and neck
/// above ~83 % of the height, chest to 63 %, stomach to 47 %, legs below),
/// arms where the hit is far off the body's axis at chest height.
pub fn hitgroup_at(b: &ColliderAabb, point: Vec3) -> Hitgroup {
    let h = (b.max.y - b.min.y).max(1e-3);
    let frac = (point.y - b.min.y) / h;
    let centre = (b.min + b.max) / 2.0;
    let half_w = ((b.max.x - b.min.x).min(b.max.z - b.min.z) / 2.0).max(1e-3);
    let off = Vec2::new(point.x - centre.x, point.z - centre.z).length() / half_w;
    match frac {
        f if f >= 0.83 => Hitgroup::Head,
        f if f >= 0.47 && off > 0.9 => Hitgroup::LeftArm,
        f if f >= 0.63 => Hitgroup::Chest,
        f if f >= 0.47 => Hitgroup::Stomach,
        _ => Hitgroup::LeftLeg,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn armour_that_runs_out_passes_the_rest_to_health() {
        // objectives.md Q3 (a C4 blast, ratio 1): 376 on 100 armour takes
        // 188 and 94; 447 on 100 armour takes all 100 and 247 health.
        assert_eq!(armor_split(376.0, 1.0, 100.0, 1.0), (188.0, 94.0));
        assert_eq!(armor_split(447.0, 1.0, 100.0, 1.0), (247.0, 100.0));
        // 6.5 armour damage costs 7 points (what is left is truncated).
        assert_eq!(armor_split(26.0, 1.0, 100.0, 1.0), (13.0, 7.0));
    }

    #[test]
    fn a_fraction_of_an_armour_point_costs_a_whole_one() {
        // Measured 2026-10-17 (weapons.md M7, probe server): the knife
        // (ratio 1.7) on 100 armour: slash 20 -> 17 health, 98 armour
        // left; stab 65 -> 55, 95 left; the 15 follow-up slash on 98 -> 12,
        // 96 left. A Glock chest hit 24.477 (1.05) -> 12, 94 left; an AK-47
        // chest hit 35.946 (1.55) -> 27, 95 left.
        for (raw, ratio, armour, health, left) in [
            (20.0, 1.7, 100.0, 17.0, 98.0),
            (65.0, 1.7, 100.0, 55.0, 95.0),
            (15.0, 1.7, 98.0, 12.0, 96.0),
            (65.0, 1.7, 10.0, 55.0, 5.0),
            (24.477, 1.05, 100.0, 12.0, 94.0),
            (35.946, 1.55, 100.0, 27.0, 95.0),
        ] {
            let (h, taken) = armor_split(raw, ratio, armour, 1.0);
            assert_eq!((h, armour - taken), (health, left), "{raw} x {ratio} on {armour}");
        }
    }

    #[test]
    fn falloff_matches_spec_t12() {
        // AK-47 at 1000 units with RangeModifier 0.98 per 500 units.
        assert!((36.0 * falloff(0.98, 500.0, 1000.0) - 34.5744).abs() < 1e-3);
    }

    #[test]
    fn spread_bends_right_for_positive_x() {
        // Spec T2: x = 0.5, s = 0.01 -> 0.2865 degrees to the right.
        let d = spread_dir(Quat::IDENTITY, 0.01, 0.5, 0.0);
        let angle = d.x.atan2(-d.z).to_degrees();
        assert!((angle - 0.2865).abs() < 1e-3, "{angle}");
    }

    #[test]
    fn hitgroups_by_height() {
        let b = ColliderAabb {
            min: Vec3::new(-0.4, 0.0, -0.4),
            max: Vec3::new(0.4, 1.8, 0.4),
        };
        assert_eq!(hitgroup_at(&b, Vec3::new(0.0, 1.7, 0.0)), Hitgroup::Head);
        assert_eq!(hitgroup_at(&b, Vec3::new(0.0, 1.3, 0.1)), Hitgroup::Chest);
        assert_eq!(hitgroup_at(&b, Vec3::new(0.0, 1.0, 0.1)), Hitgroup::Stomach);
        assert_eq!(hitgroup_at(&b, Vec3::new(0.0, 0.5, 0.1)), Hitgroup::LeftLeg);
        assert_eq!(hitgroup_at(&b, Vec3::new(0.39, 1.2, 0.0)), Hitgroup::LeftArm);
    }
}
