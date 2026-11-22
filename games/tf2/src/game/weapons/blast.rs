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
//!   owner's hull counts 55 tall; a Heavy takes half of another's. The
//!   volume is TF2's hull for the victim's stance (`tf2_hull`: standing
//!   48 x 48 x 82, ducked 62 tall), whatever box the victim really has:
//!   another game's character, or one fitted into a smaller map's
//!   envelope (a CS:S map's 32 x 32 x 72 is 2.56 x less volume), is
//!   pushed as a TF2 player in its stance would be, not harder. A push
//!   upward above `LIFT` (250 units/s) takes him off the ground at his
//!   next move (the movement's own rule; `MovementState::on_ground` too,
//!   for another game's movement). The owner's own push sets
//!   `BlastJumping` (until he lands: `blast_jump_lands`) and his count of
//!   air ducks back to 0.
//! - **Bullets** push too (`bullet_push`, after the frame): one push per
//!   victim per shot from its summed damage at scale 6, from the shooter's
//!   centre lowered 10.
//! - **Where** (effects.md 13, `Placement`): 1 unit out along the
//!   normal of what its trace met (a rocket's touch: `touched`; a pipe's
//!   direct hit: a 64-unit line through it along its flight, `pipe_hit`;
//!   a fused pipe or detonated sticky: a line from 8 units above its
//!   centre to 24 below, `fused`, which snaps it onto a floor there and
//!   else leaves it in the air), the `Scorch` decal on the same trace's
//!   hit unless a player; a stuck sticky sends its surface's normal.
//! - an explosion's look and its sound (`Tf2Explosion`, drawn and played
//!   by each machine that draws when the drawn time reaches it) and its
//!   measure (`weapon::grenade::BlastDealt`: the centre, the radius, the
//!   damage at the centre and each victim's; the sandbox's blast rings).
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
/// TF2's ducked hull height (units, specs/tf2/classes.md).
pub const CROUCH_HEIGHT: f32 = 62.0;
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
/// A blast whose surface normal has every component under this is in the
/// air (effects.md 5 step 2).
pub const AIR_NORMAL: f32 = 0.05;
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

/// An explosion to draw: the server's, sent to every client (`looks`):
/// its centre, the surface normal it went off on (zero in the air), on a
/// player or not, and the gun (whose looks name the effects).
#[derive(Message, Clone, Copy, Debug)]
pub struct Tf2Explosion {
    pub at: Vec3,
    pub normal: Vec3,
    pub on_player: bool,
    pub weapon: &'static str,
}

/// A blast weapon's explosion sound (its rocket's, grenade's or sticky's
/// `sound`), on every machine (the rules are each machine's).
#[derive(SystemParam)]
pub struct BlastSounds<'w> {
    rockets: Option<Res<'w, super::rocket::RocketRules>>,
    grenades: Option<Res<'w, super::grenade::GrenadeRules>>,
    stickies: Option<Res<'w, super::sticky::StickyRules>>,
}

impl BlastSounds<'_> {
    pub fn of(&self, weapon: &str) -> Option<&str> {
        fn rocket<'a>(r: &'a super::rocket::RocketRules, weapon: &str) -> Option<&'a str> {
            r.get(weapon).and_then(|(p, _)| p.sound.as_deref())
        }
        self.rockets
            .as_deref()
            .and_then(|r| rocket(r, weapon))
            .or_else(|| self.grenades.as_deref().and_then(|g| rocket(&g.0, weapon)))
            .or_else(|| self.stickies.as_deref().and_then(|s| s.get(weapon)).and_then(|(_, b, _)| b.sound.as_deref()))
    }
}

/// The decal a blast leaves where it went off (effects.md 5).
pub const SCORCH: &str = "Scorch";

/// A blast's centre is this far out along the touched surface's normal
/// (units, effects.md 13).
pub const OUT: f32 = 1.0;
/// A pipe's direct hit: its line from this far behind its centre along
/// its flight to as far ahead (units, effects.md 13).
pub const PIPE_LINE: f32 = 32.0;
/// A fused pipe's or detonated sticky's line down: from this far above
/// its centre to `DOWN_BELOW` below (units, effects.md 13).
pub const DOWN_ABOVE: f32 = 8.0;
pub const DOWN_BELOW: f32 = 24.0;

/// Where a blast goes off (effects.md 13): its centre, the surface normal
/// sent with it (zero: in the air), what its trace hit (a player: the
/// player effect, no scorch) and the scorch's place on the same trace
/// (point, normal), if any.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Placement {
    pub centre: Vec3,
    pub normal: Vec3,
    pub target: Option<Entity>,
    pub scorch: Option<(Vec3, Vec3)>,
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

/// The hull (units) a push divides by (5.4's V): TF2's own for the
/// victim's stance, whatever its real box (module doc). `ducked`: its
/// hull is the ducked one; `own`: the owner's own blast (55 tall ducked).
pub fn tf2_hull(ducked: bool, own: bool) -> Vec3 {
    match (ducked, own) {
        (false, _) => STANDING,
        (true, true) => Vec3::new(STANDING.x, STANDING.y, SELF_CROUCH_HEIGHT),
        (true, false) => Vec3::new(STANDING.x, STANDING.y, CROUCH_HEIGHT),
    }
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
    /// Source movements' duck state (the hull, not the key: `ducked`).
    ducks: Query<'w, 's, &'static SourceMovement>,
    friendly_fire: Option<Res<'w, FriendlyFire>>,
    classes: Option<Res<'w, Classes>>,
    damage: MessageWriter<'w, Damage>,
    inflicted: MessageWriter<'w, Inflicted>,
    sounds: MessageWriter<'w, GameSound>,
    looks: MessageWriter<'w, Tf2Explosion>,
    decals: MessageWriter<'w, crate::map::PlaceDecal>,
    /// The blast as measured (`BlastDealt`: the sandbox's blast rings).
    dealt_out: MessageWriter<'w, chimera_sim::weapon::grenade::BlastDealt>,
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

    /// Whether `e`'s hull is ducked: a Source movement's ducked hull,
    /// another movement's crouch.
    fn ducked(&self, e: Entity, state: Option<&MovementState>) -> bool {
        match self.ducks.get(e) {
            Ok(m) => m.ducked,
            Err(_) => state.is_some_and(|s| s.crouching),
        }
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

    /// What a line from `from` along `dir` for `length` (m) meets first
    /// among solids (characters too): the point, the surface's normal and
    /// the body. A line starting inside a solid meets nothing there
    /// (effects.md 13: a sticky on a ceiling).
    fn line(&self, from: Vec3, dir: Vec3, length: f32) -> Option<(Vec3, Vec3, Entity)> {
        let dir = Dir3::new(dir).ok()?;
        let filter = SpatialQueryFilter::default().with_mask(SOLID_LAYERS);
        let hit = self.spatial.cast_ray(from, dir, length, true, &filter)?;
        if hit.distance <= 0.0 {
            return None;
        }
        let body = self.colliders.get(hit.entity).map_or(hit.entity, |o| o.body);
        Some((from + *dir * hit.distance, hit.normal, body))
    }

    /// A placement from what a trace met: `OUT` along its normal, the
    /// scorch there unless on a player.
    fn met(&self, (point, normal, body): (Vec3, Vec3, Entity), sent: Option<Vec3>) -> Placement {
        let player = self.chars.contains(body);
        Placement {
            centre: point + normal * OUT * U,
            normal: sent.unwrap_or(normal),
            target: Some(body),
            scorch: (!player).then_some((point, normal)),
        }
    }

    /// A rocket's touch at `point` on a surface of `normal`, `other` the
    /// body touched (None: the world): `OUT` off it, the scorch there
    /// unless on a player (effects.md 13).
    pub fn touched(&self, point: Vec3, normal: Vec3, other: Option<Entity>) -> Placement {
        let player = other.is_some_and(|o| self.chars.contains(o));
        Placement {
            centre: point + normal * OUT * U,
            normal,
            target: other,
            scorch: (!player).then_some((point, normal)),
        }
    }

    /// A pipe's direct hit (effects.md 13): its 64-unit line through its
    /// centre along its flight; the line missing, its centre in the air.
    pub fn pipe_hit(&self, centre: Vec3, velocity: Vec3) -> Placement {
        let dir = velocity.normalize_or_zero();
        let from = centre - dir * PIPE_LINE * U;
        match self.line(from, dir, 2.0 * PIPE_LINE * U) {
            Some(m) => self.met(m, None),
            None => Placement { centre, ..default() },
        }
    }

    /// A fused pipe or detonated sticky (effects.md 13): a line down from
    /// `DOWN_ABOVE` above its centre to `DOWN_BELOW` below snaps it onto
    /// a floor it finds; `stuck` the normal of the surface a sticky stuck
    /// to (sent instead of the floor's, or of none).
    pub fn fused(&self, centre: Vec3, stuck: Option<Vec3>) -> Placement {
        let from = centre + Vec3::Y * DOWN_ABOVE * U;
        match self.line(from, Vec3::NEG_Y, (DOWN_ABOVE + DOWN_BELOW) * U) {
            Some(m) => self.met(m, stuck),
            None => Placement { centre, normal: stuck.unwrap_or(Vec3::ZERO), ..default() },
        }
    }

    /// Explode `blast` of `weapon` by `owner` where `at` places it (m;
    /// its look and scorch too), `touched` the player hit directly (full
    /// splash).
    pub fn detonate(&mut self, blast: &Blast, at: &Placement, owner: Option<Entity>, weapon: &'static str, touched: Option<Entity>) {
        let (centre, normal) = (at.centre, at.normal);
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
            let ducked = self.ducked(e, state);
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
                own = Some(Dealt {
                    e,
                    points: p.self_damage.unwrap_or(p.damage) * f * scale,
                    centre: c,
                    scale: if airborne { p.push_self_air } else { p.push_self_ground },
                    // His own push counts a crouched hull 55 tall.
                    size: tf2_hull(ducked, true),
                    own: true,
                });
            } else {
                let m = match blast.ramp {
                    Some(k) => modifier(owner_at.map_or(0.0, |o| o.distance(c) / U), k),
                    None => 1.0,
                };
                dealt.push(Dealt { e, points: p.damage * blast.scale * m * f, centre: c, scale: p.push_other, size: tf2_hull(ducked, false), own: false });
            }
        }
        // The owner after everyone else.
        dealt.extend(own);
        let from = centre - Vec3::Y * FORCE_LOWER * U;
        let mut victims = Vec::new();
        for d in dealt {
            let amount = round_points(d.points * HP);
            if amount <= 0.0 {
                continue;
            }
            victims.push(chimera_sim::weapon::grenade::BlastVictim { victim: d.e, at: d.centre, distance_m: d.centre.distance(centre), damage: amount });
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
        self.dealt_out.write(chimera_sim::weapon::grenade::BlastDealt {
            attacker: owner,
            weapon: Some(weapon),
            at: centre,
            inner: 0.0,
            outer: blast.radius * U,
            edge_damage: p.edge * p.damage * HP,
            max_damage: p.damage * HP,
            victims,
        });
        // Its sound plays with its look on each machine that draws, when
        // the drawn time reaches it (effects.md 5 step 5, 11: `looks`).
        let on_player = at.target.is_some_and(|t| self.chars.contains(t));
        // Effects.md 5: a near-zero normal is in the air.
        let normal = if normal.abs().max_element() < AIR_NORMAL { Vec3::ZERO } else { normal };
        self.looks.write(Tf2Explosion { at: centre, normal, on_player, weapon });
        // The scorch on what its own trace met (effects.md 13), unless a
        // player.
        if let Some((point, n)) = at.scorch {
            self.decals.write(crate::map::PlaceDecal {
                target: None,
                group: crate::map::DecalGroup::Named(SCORCH.into()),
                point,
                normal: n,
                dir: -n,
                spin: true,
                round: false,
            });
        }
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
        let c = bounds(at, aabb, state).1;
        let size = tf2_hull(world.ducked(d.target, state), false);
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
