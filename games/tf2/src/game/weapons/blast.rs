//! TF2's explosions and knockback (specs/tf2/weapons.md 5.2-5.4,
//! weapons_more.md 4.3, 5.4, 6, and their corrections from the SDK),
//! shared by the rocket, the grenade and the stickybomb
//! (`BlastWorld::detonate`), and the bullets' push (`bullet_push`):
//!
//! - **Who**: every living character whose box's nearest point is within
//!   the radius of the centre, with a clear line from it to its centre (the
//!   world blocks; characters don't), except the owner (below). r is the
//!   smaller of the distances to its centre and to its feet (`reach`), 0
//!   for the player a projectile touched; the splash
//!   f(r) = 1 - (1 - `edge`) min(r, radius) / radius (`splash`: one
//!   reached by its box past the radius takes the edge's); the damage
//!   `damage` x f x the distance modifier between the owner's and its
//!   centre with the blast's ramp (`Blast::ramp`; None: the grenade's and
//!   an old sticky's flat 1).
//! - **The owner**, after everyone else: his own radius (`self_radius`:
//!   the rocket's 121, the Demoman's 146 even for an air-detonated
//!   sticky) and the launcher's base damage (`self_damage`: 90, 100, 120,
//!   even on a grenade's 60 % fuse), no distance modifier, x
//!   `self_air_scale` airborne (the Soldier's 0.6, the Demoman's 0.75) or
//!   `self_ground_scale` on the ground (1; the Demoman's 0.75).
//! - **The push** (5.4, `push`): min(1000, D x scale x (48 48 82) / the
//!   victim's hull volume) units/s from 10 units below the blast's centre
//!   to the victim's centre, D the damage before rounding; the scale
//!   `push_other` (6) for others, `push_self_air` / `push_self_ground`
//!   for the owner (the Soldier's 10 / 5, the Demoman's 9); a crouched
//!   owner's hull counts 55 tall; a Heavy takes half of another's. A push
//!   upward above `LIFT` (250 units/s) takes him off the ground at his
//!   next move (the movement's own rule; `MovementState::on_ground` too,
//!   for another game's movement). The owner's own push sets
//!   `BlastJumping` (until he lands: `blast_jump_lands`) and his count of
//!   air ducks back to 0.
//! - **Bullets** push too (`bullet_push`, after the frame): one push per
//!   victim per shot from its summed damage at scale 6, from the shooter's
//!   centre lowered 10.
//! - its sound there, heard by every client (`map::GameSound`), and an
//!   explosion's look (`Tf2Explosion`, drawn by the client).
//!
//! Each damage rounds to whole points, of kind blast, naming the gun.
//!
//! Known shortcuts: crits are off; "airborne" ignores water (a swimming
//! Soldier counts as airborne); a bullet push reads the summed damage
//! rounded to whole points (the delivery's), not the unrounded sum.

use avian3d::prelude::*;
use bevy::{ecs::system::SystemParam, prelude::*};
use serde::Deserialize;

use chimera_sim::weapon::{BulletHit, Inflicted};
use source_engine::source_movement::SourceMovement;

use super::{
    super::classes::{Classes, Tf2Class},
    damage::{HP, Tf2Gun, U, modifier, round_points},
};
use crate::{
    core::{Damage, DamageKind, FriendlyFire, Health, Hitgroup, Intent, MovementState, SOLID_LAYERS, Team, Velocity},
    map::{GameSound, PlaySound},
};

/// TF2's standing box (units: 48 x 48 x 82, specs/tf2/classes.md).
pub const STANDING: Vec3 = Vec3::new(48.0, 48.0, 82.0);
/// A crouched owner's hull height for his own blast's push (units).
pub const SELF_CROUCH_HEIGHT: f32 = 55.0;
/// The most speed one hit adds (units/s).
pub const FORCE_CAP: f32 = 1000.0;
/// The push comes from this far below the inflictor's centre (units).
pub const FORCE_LOWER: f32 = 10.0;
/// A Heavy takes this of another's push.
pub const HEAVY_KNOCKBACK: f32 = 0.5;
/// The bullets' push scale (`tf_damageforcescale_other`).
pub const BULLET_PUSH: f32 = 6.0;
/// An upward speed above this (units/s) lifts the victim off the ground.
pub const LIFT: f32 = 250.0;
/// The Heavy's class name (classes.toml).
pub const HEAVY: &str = "heavy";

/// A blast's numbers (data units: units, points), as the hooks give them
/// (`tf2:rocket`, `tf2:grenade`; `tf2:sticky` builds one).
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BlastParams {
    pub damage: f32,
    pub radius: f32,
    /// The splash's factor at the radius.
    pub edge: f32,
    /// The owner's own blast: its radius and base damage (None: the
    /// blast's), x `self_air_scale` airborne, x `self_ground_scale` on the
    /// ground.
    #[serde(default)]
    pub self_radius: Option<f32>,
    #[serde(default)]
    pub self_damage: Option<f32>,
    pub self_air_scale: f32,
    #[serde(default = "one")]
    pub self_ground_scale: f32,
    /// A grenade's fuse explosion deals this share of `damage` to others
    /// (0.6; a direct hit the whole); the owner's is the base either way.
    #[serde(default)]
    pub fuse_scale: Option<f32>,
    /// Push scales: others', the owner's airborne and on the ground.
    pub push_other: f32,
    pub push_self_air: f32,
    pub push_self_ground: f32,
    /// The distance modifier applies (the gun's `tf2:gun` ramp); false:
    /// the grenade's flat damage.
    #[serde(default = "yes")]
    pub by_distance: bool,
    /// The explosion's sound entry.
    #[serde(default)]
    pub sound: Option<String>,
    /// A grenade's bounce off the world.
    #[serde(default)]
    pub bounce_sound: Option<String>,
}

fn yes() -> bool {
    true
}

fn one() -> f32 {
    1.0
}

/// One explosion: its numbers, the radius it reaches, its ramp (None: no
/// distance modifier), the share of its damage it deals to others (a
/// grenade's fuse: 0.6; the owner's is always the base).
#[derive(Clone, Debug)]
pub struct Blast<'a> {
    pub params: &'a BlastParams,
    pub radius: f32,
    pub ramp: Option<f32>,
    pub scale: f32,
}

/// An explosion to draw (its centre): the server's, sent to every client
/// (`looks`).
#[derive(Message, Clone, Copy, Debug)]
pub struct Tf2Explosion {
    pub at: Vec3,
}

/// Launched by his own blast, until he lands (weapons.md 5.4).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct BlastJumping;

/// The splash factor at `r` units of a blast of `radius` with `edge` at
/// its edge, clamped at the edge (one reached by its box past the radius
/// takes the edge's).
pub fn splash(r: f32, radius: f32, edge: f32) -> f32 {
    1.0 - (1.0 - edge) * r.clamp(0.0, radius) / radius.max(1e-6)
}

/// The push (units/s) of `points` of damage at `scale` on a victim whose
/// box is `size` (units): along `away` (unit), capped at `FORCE_CAP`.
pub fn push(points: f32, scale: f32, size: Vec3, away: Vec3) -> Vec3 {
    let volume = (size.x * size.y * size.z).max(1.0);
    away * (points * scale * (STANDING.x * STANDING.y * STANDING.z) / volume).min(FORCE_CAP)
}

/// How far a blast at `centre` is from a character whose box is
/// `min`..`max` (m): (the nearest point of its box, the nearer of its
/// centre and its feet), units.
pub fn reach(centre: Vec3, min: Vec3, max: Vec3) -> (f32, f32) {
    let mid = (min + max) / 2.0;
    let feet = Vec3::new(mid.x, min.y, mid.z);
    let nearest = centre.clamp(min, max).distance(centre) / U;
    (nearest, mid.distance(centre).min(feet.distance(centre)) / U)
}

/// A character's box size (units, any axes' order) and centre (m).
pub fn bounds(at: &Transform, aabb: Option<&ColliderAabb>, state: Option<&MovementState>) -> (Vec3, Vec3) {
    let (min, max) = boxed(at, aabb, state);
    ((max - min).abs() / U, aabb.map_or((min + max) / 2.0, |b| (b.min + b.max) / 2.0))
}

/// A character's box (m): its movement's hull where it has one, else its
/// collider's bounds, else the standing box about its origin.
pub fn boxed(at: &Transform, aabb: Option<&ColliderAabb>, state: Option<&MovementState>) -> (Vec3, Vec3) {
    match (state, aabb) {
        (Some(s), _) if s.hull_max != s.hull_min => {
            let (a, b) = (at.translation + s.hull_min, at.translation + s.hull_max);
            (a.min(b), a.max(b))
        }
        (_, Some(b)) => (b.min, b.max),
        _ => {
            let half = Vec3::new(STANDING.x, STANDING.z, STANDING.y) * U / 2.0;
            (at.translation - half, at.translation + half)
        }
    }
}

type Chars<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Transform,
        Option<&'static ColliderAabb>,
        &'static Health,
        Option<&'static Team>,
        Option<&'static MovementState>,
        &'static mut Velocity,
        Option<&'static Tf2Class>,
    ),
    With<Intent>,
>;

/// What an explosion reads and writes.
#[derive(SystemParam)]
pub struct BlastWorld<'w, 's> {
    pub chars: Chars<'w, 's>,
    spatial: SpatialQuery<'w, 's>,
    colliders: Query<'w, 's, &'static ColliderOf>,
    friendly_fire: Option<Res<'w, FriendlyFire>>,
    classes: Option<Res<'w, Classes>>,
    damage: MessageWriter<'w, Damage>,
    inflicted: MessageWriter<'w, Inflicted>,
    sounds: MessageWriter<'w, GameSound>,
    looks: MessageWriter<'w, Tf2Explosion>,
    commands: Commands<'w, 's>,
}

/// One victim's share of a blast: its damage before rounding, the push's
/// scale and the box it is pushed by (units).
struct Dealt {
    e: Entity,
    points: f32,
    centre: Vec3,
    scale: f32,
    size: Vec3,
    own: bool,
}

impl BlastWorld<'_, '_> {
    /// A character's centre (m), if it is one.
    pub fn centre(&self, e: Entity) -> Option<Vec3> {
        self.chars.get(e).ok().map(|(_, t, b, _, _, s, ..)| bounds(t, b, s).1)
    }

    /// Whether `a` and `b` are teammates (non-zero teams, friendly fire off).
    pub fn mates(&self, a: Entity, b: Entity) -> bool {
        let ff = self.friendly_fire.as_ref().is_some_and(|f| f.0 != 0);
        let team = |e: Entity| self.chars.get(e).ok().and_then(|c| c.4.copied()).filter(|t| t.0 != 0);
        !ff && a != b && team(a).is_some() && team(a) == team(b)
    }

    /// Whether `e` plays the Heavy.
    fn heavy(&self, e: Entity) -> bool {
        let (Some(table), Ok(c)) = (self.classes.as_ref(), self.chars.get(e)) else { return false };
        c.7.is_some_and(|c| c.class(table).name == HEAVY)
    }

    /// Whether a line from `centre` to `c` is clear (only characters
    /// between).
    fn clear(&self, centre: Vec3, c: Vec3) -> bool {
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let Ok(dir) = Dir3::new(c - centre) else { return true };
        self.spatial
            .cast_ray_predicate(centre, dir, c.distance(centre), true, &filter, &|hit_e| {
                let body = self.colliders.get(hit_e).map_or(hit_e, |o| o.body);
                !self.chars.contains(body)
            })
            .is_none()
    }

    /// Explode `blast` of `weapon` by `owner` at `centre` (m), `touched`
    /// the player hit directly (full splash).
    pub fn detonate(&mut self, blast: &Blast, centre: Vec3, owner: Option<Entity>, weapon: &'static str, touched: Option<Entity>) {
        let p = blast.params;
        let owner_at = owner.and_then(|o| self.centre(o));
        let mut dealt: Vec<Dealt> = Vec::new();
        let mut own: Option<Dealt> = None;
        for (e, at, aabb, health, _, state, ..) in &self.chars {
            if health.current <= 0.0 {
                continue;
            }
            let is_owner = Some(e) == owner;
            // Teammates aren't hurt with friendly fire off (the owner is).
            if !is_owner && owner.is_some_and(|o| self.mates(o, e)) {
                continue;
            }
            let (min, max) = boxed(at, aabb, state);
            let c = (min + max) / 2.0;
            let mut size = (max - min).abs() / U;
            let hit = touched == Some(e);
            let (nearest, r) = reach(centre, min, max);
            let radius = if is_owner { p.self_radius.unwrap_or(blast.radius) } else { blast.radius };
            if !hit && nearest > radius {
                continue;
            }
            if !hit && !self.clear(centre, c) {
                continue;
            }
            let f = if hit { 1.0 } else { splash(r, radius, p.edge) };
            let airborne = state.is_some_and(|s| !s.on_ground);
            if is_owner {
                let scale = if airborne { p.self_air_scale } else { p.self_ground_scale };
                // His own push counts a crouched hull 55 tall.
                if state.is_some_and(|s| s.crouching) {
                    size.y = SELF_CROUCH_HEIGHT;
                }
                own = Some(Dealt {
                    e,
                    points: p.self_damage.unwrap_or(p.damage) * f * scale,
                    centre: c,
                    scale: if airborne { p.push_self_air } else { p.push_self_ground },
                    size,
                    own: true,
                });
            } else {
                let m = match blast.ramp {
                    Some(k) => modifier(owner_at.map_or(0.0, |o| o.distance(c) / U), k),
                    None => 1.0,
                };
                dealt.push(Dealt { e, points: p.damage * blast.scale * m * f, centre: c, scale: p.push_other, size, own: false });
            }
        }
        // The owner after everyone else.
        dealt.extend(own);
        let from = centre - Vec3::Y * FORCE_LOWER * U;
        for d in dealt {
            let amount = round_points(d.points * HP);
            if amount <= 0.0 {
                continue;
            }
            let dir = (d.centre - from).normalize_or(Vec3::Y);
            self.damage.write(Damage {
                target: d.e,
                attacker: owner,
                amount,
                point: d.centre,
                dir,
                hitgroup: Hitgroup::Generic,
                kind: DamageKind::Blast,
                weapon: Some(weapon),
                force: Vec3::ZERO,
            });
            self.inflicted.write(Inflicted { target: d.e, amount, from: centre });
            let half = if !d.own && self.heavy(d.e) { HEAVY_KNOCKBACK } else { 1.0 };
            let dv = push(d.points, d.scale, d.size, dir) * half * U;
            self.shove(d.e, dv, d.own);
        }
        if let Some(s) = &p.sound {
            self.sounds.write(GameSound(PlaySound::at(s.clone(), centre)));
        }
        self.looks.write(Tf2Explosion { at: centre });
    }

    /// Add `dv` (m/s) to `e`'s velocity; `own`: his own blast (blast
    /// jumping, his air ducks counted again).
    fn shove(&mut self, e: Entity, dv: Vec3, own: bool) {
        let Ok((.., mut velocity, _)) = self.chars.get_mut(e) else { return };
        velocity.0 += dv;
        let lift = dv.y > LIFT * U;
        if !lift && !own {
            return;
        }
        self.commands.entity(e).queue(move |mut w: EntityWorldMut| {
            if lift && let Some(mut s) = w.get_mut::<MovementState>() {
                s.on_ground = false;
            }
            if own {
                if let Some(mut m) = w.get_mut::<SourceMovement>() {
                    m.air_ducks = 0;
                }
                w.insert(BlastJumping);
            }
        });
    }

    /// A direct hit's damage (a needle's), from `from`.
    pub fn hurt(&mut self, d: Damage, from: Vec3) {
        self.inflicted.write(Inflicted { target: d.target, amount: d.amount, from });
        self.damage.write(d);
    }

    /// A sound at `at`, heard by every client.
    pub fn sound(&mut self, entry: &str, at: Vec3) {
        self.sounds.write(GameSound(PlaySound::at(entry.to_string(), at)));
    }
}

/// The blast-jumping state ends on landing.
pub fn blast_jump_lands(q: Query<(Entity, &MovementState), With<BlastJumping>>, mut commands: Commands) {
    for (e, s) in &q {
        if s.on_ground {
            commands.entity(e).remove::<BlastJumping>();
        }
    }
}

/// After the frame, the server's: each TF2 bullet hit pushes its target
/// (5.4): the shot's damage on it at scale 6 from the shooter's centre
/// lowered 10 (a Heavy half).
pub fn bullet_push(mut hits: MessageReader<BulletHit>, guns: Query<(), With<Tf2Gun>>, mut world: BlastWorld) {
    for h in hits.read() {
        let d = &h.damage;
        if !guns.contains(h.weapon) || d.amount <= 0.0 {
            continue;
        }
        let Some(shooter) = d.attacker.and_then(|a| world.centre(a)) else { continue };
        let Ok((_, at, aabb, _, _, state, ..)) = world.chars.get(d.target) else { continue };
        let (size, c) = bounds(at, aabb, state);
        let from = shooter - Vec3::Y * FORCE_LOWER * U;
        let dir = (c - from).normalize_or(Vec3::Y);
        let half = if world.heavy(d.target) { HEAVY_KNOCKBACK } else { 1.0 };
        let dv = push(d.amount / HP, BULLET_PUSH, size, dir) * half * U;
        world.shove(d.target, dv, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 5.2: linear to 50 % at 146 units, clamped there.
    #[test]
    fn the_splash_falls_to_half_at_the_edge() {
        assert_eq!(splash(0.0, 146.0, 0.5), 1.0);
        assert_eq!(splash(73.0, 146.0, 0.5), 0.75);
        assert_eq!(splash(146.0, 146.0, 0.5), 0.5);
        assert_eq!(splash(160.0, 146.0, 0.5), 0.5);
        // Test 21: the Soldier's own, 43 units from his feet: 74.01.
        assert!((90.0 * splash(43.0, 121.0, 0.5) - 74.01).abs() < 0.01);
    }

    /// 5.4's worked numbers: 90 at 512: 540; a crit's 270 capped at 1000;
    /// a crouched hull farther.
    #[test]
    fn the_push_is_capped_and_a_smaller_box_goes_farther() {
        assert!((push(90.0, 6.0, STANDING, Vec3::Z).z - 540.0).abs() < 1e-3);
        assert!((push(270.0, 6.0, STANDING, Vec3::Z).z - 1000.0).abs() < 1e-3);
        let own = |h: f32| push(53.8, 10.0, Vec3::new(48.0, 48.0, h), Vec3::Z).z;
        assert!((own(82.0) - 538.0).abs() < 0.5);
        assert!((own(55.0) - 802.1).abs() < 0.5, "{}", own(55.0));
        assert!((push(89.6, 5.0, STANDING, Vec3::Z).z - 448.0).abs() < 0.5);
    }

    /// 5.2: the nearer of the centre and the feet; the box's nearest point.
    #[test]
    fn reach_is_to_the_nearer_of_centre_and_feet() {
        let (min, max) = (Vec3::new(-24.0, 0.0, -24.0) * U, Vec3::new(24.0, 82.0, 24.0) * U);
        let (near, r) = reach(Vec3::new(43.0, 0.0, 0.0) * U, min, max);
        assert!((r - 43.0).abs() < 1e-3 && (near - 19.0).abs() < 1e-3, "{near} {r}");
        let (near, r) = reach(Vec3::new(160.0, 41.0, 0.0) * U, min, max);
        assert!((r - 160.0).abs() < 1e-3 && (near - 136.0).abs() < 1e-3, "{near} {r}");
    }
}
