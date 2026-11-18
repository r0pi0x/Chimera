use bevy::prelude::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use super::*;

/// A shape on a character's body that shots test, in the character's
/// local space: feet at the origin, Y up, facing -Z (yaw 0), meters. An
/// oriented box (Source hitboxes: `half`, `rotation`) or a sphere (Combat
/// Arms' hit spheres: `radius`), with its body region, the hit priority
/// Combat Arms' server walk ranks shapes by and the body's own damage
/// factor for it (`scale`: Combat Arms' zone factor; guns whose damage
/// table says so use it). The body's game converts its own format into
/// these (docs/DESIGN.md "Hits across games"); every gun's rule works on
/// either shape.
#[derive(Clone, Copy, Debug, PartialEq, Reflect, serde::Serialize, serde::Deserialize)]
pub struct Hitbox {
    pub center: Vec3,
    pub half: Vec3,
    pub rotation: Quat,
    pub region: BodyRegion,
    /// A sphere of this radius (`half` and `rotation` unused); None: a box.
    pub radius: Option<f32>,
    pub priority: f32,
    pub scale: Option<f32>,
}

impl Hitbox {
    /// An oriented box of `region` (its region's default priority, no
    /// scale of its own).
    pub fn cuboid(center: Vec3, half: Vec3, rotation: Quat, region: BodyRegion) -> Self {
        Self {
            center,
            half,
            rotation,
            region,
            radius: None,
            priority: region.default_priority(),
            scale: None,
        }
    }

    /// A sphere of `region`.
    pub fn sphere(center: Vec3, radius: f32, region: BodyRegion) -> Self {
        Self {
            center,
            half: Vec3::splat(radius),
            rotation: Quat::IDENTITY,
            region,
            radius: Some(radius),
            priority: region.default_priority(),
            scale: None,
        }
    }

    /// Its Source hitgroup (`BodyRegion::hitgroup`).
    pub fn group(&self) -> Hitgroup {
        self.region.hitgroup()
    }

    /// Where a ray (local space) enters the shape, as a distance along
    /// `dir` (unit), if it does.
    pub fn ray_entry(&self, origin: Vec3, dir: Vec3) -> Option<f32> {
        self.ray_span(origin, dir).map(|(enter, _)| enter)
    }

    /// Where a ray (local space) enters and leaves the shape, as distances
    /// along `dir` (unit; the entry clamped at 0), if it crosses it.
    pub fn ray_span(&self, origin: Vec3, dir: Vec3) -> Option<(f32, f32)> {
        if let Some(r) = self.radius {
            if r <= 0.0 {
                return None;
            }
            let p = origin - self.center;
            let b = p.dot(dir);
            let c = p.length_squared() - r * r;
            let disc = b * b - c;
            if disc < 0.0 {
                return None;
            }
            let s = disc.sqrt();
            let (enter, leave) = (-b - s, -b + s);
            return (leave >= 0.0).then_some((enter.max(0.0), leave));
        }
        let (enter, leave) = self.line_span(origin, dir)?;
        (leave >= 0.0).then_some((enter.max(0.0), leave))
    }

    /// Where the whole line through `origin` along `dir` (unit; local
    /// space) enters and leaves a box, as signed distances along `dir`
    /// (behind `origin` negative), if it crosses it: a box's slabs (a
    /// sphere's callers use its radius).
    fn line_span(&self, origin: Vec3, dir: Vec3) -> Option<(f32, f32)> {
        let inv = self.rotation.inverse();
        let o = inv * (origin - self.center);
        let d = inv * dir;
        let (mut enter, mut leave) = (f32::MIN, f32::MAX);
        for i in 0..3 {
            if d[i].abs() < 1e-9 {
                if o[i].abs() > self.half[i] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((-self.half[i] - o[i]) / d[i], (self.half[i] - o[i]) / d[i]);
            enter = enter.max(a.min(b));
            leave = leave.min(a.max(b));
        }
        (enter <= leave).then_some((enter, leave))
    }

    /// The radius of a sphere around it: a sphere's own, a box's
    /// bounding sphere's (a blast's reach of it).
    pub fn reach(&self) -> f32 {
        self.radius.unwrap_or_else(|| self.half.length())
    }

    /// Whether the whole segment `start`..`end` passes the shape (Combat
    /// Arms' client test: a sphere when its closest point is within the
    /// radius; a box when the segment crosses it). A zero radius never.
    pub fn segment_passes(&self, start: Vec3, end: Vec3) -> bool {
        match self.radius {
            Some(r) => {
                if r <= 0.0 {
                    return false;
                }
                let seg = end - start;
                let len2 = seg.length_squared().max(1e-12);
                let t = ((self.center - start).dot(seg) / len2).clamp(0.0, 1.0);
                (start + seg * t).distance_squared(self.center) <= r * r
            }
            None => {
                let seg = end - start;
                let len = seg.length();
                len > 0.0 && self.ray_span(start, seg / len).is_some_and(|(enter, _)| enter <= len)
            }
        }
    }

    /// The axis-aligned box around the shape (in its own space: min, max).
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let extent = match self.radius {
            Some(r) => Vec3::splat(r.max(0.0)),
            None => {
                let m = Mat3::from_quat(self.rotation);
                Vec3::new(
                    m.row(0).abs().dot(self.half),
                    m.row(1).abs().dot(self.half),
                    m.row(2).abs().dot(self.half),
                )
            }
        };
        (self.center - extent, self.center + extent)
    }

    /// The power of `start` to the shape along the segment `start`..`end`
    /// (Combat Arms' client test takes the smallest of the shapes a
    /// segment passes): a sphere's |p|^2 - r^2, p its centre from `start`,
    /// which is the product of the distances from `start` to where any
    /// line through it enters and leaves the sphere; a box's is that
    /// product along the segment's own line (where it enters and leaves
    /// the box, negative from inside), not its bounding sphere's, which
    /// would favour a big box (another game's chest) over a small one
    /// the segment meets first (its head).
    pub fn power_along(&self, start: Vec3, end: Vec3) -> f32 {
        if let Some(r) = self.radius {
            return (self.center - start).length_squared() - r * r;
        }
        let seg = end - start;
        let len = seg.length();
        match (len > 0.0).then(|| self.line_span(start, seg / len)).flatten() {
            Some((enter, leave)) => enter * leave,
            None => f32::INFINITY,
        }
    }

    /// Whether the infinite line through `at` along the unit `dir` passes
    /// the shape (Combat Arms' server walk): a sphere within its radius of
    /// its centre, a box through the box itself (not its bounding
    /// sphere). A zero radius never.
    pub fn line_passes(&self, at: Vec3, dir: Vec3) -> bool {
        match self.radius {
            Some(r) => {
                let p = self.center - at;
                r > 0.0 && (p - dir * p.dot(dir)).length_squared() <= r * r
            }
            None => self.line_span(at, dir).is_some(),
        }
    }

    /// Between `self` and `other` (the same shape later) by `f`.
    pub fn lerp(&self, other: &Hitbox, f: f32) -> Hitbox {
        Hitbox {
            center: self.center.lerp(other.center, f),
            half: self.half.lerp(other.half, f),
            rotation: self.rotation.slerp(other.rotation, f),
            radius: match (self.radius, other.radius) {
                (Some(a), Some(b)) => Some(a + (b - a) * f),
                (r, _) => r,
            },
            ..*self
        }
    }
}

/// A character's hitboxes (see `Hitbox`).
#[derive(Component, Clone, Debug, Default, Reflect)]
#[reflect(Component)]
#[component(storage = "SparseSet")]
pub struct Hitboxes(pub Vec<Hitbox>);

/// A character's hitboxes made only when something asks (`later::Later`):
/// a game that poses its bodies on demand (Combat Arms) sets these each
/// tick instead of `Hitboxes`, from what the pose was made of then, and a
/// shot that tests the character makes them (docs/performance.md, "Poses
/// on demand"). Read both kinds through `HitShapes`; where both are on a
/// character, these are the ones that count (the game that set them owns
/// its body).
///
/// The second field, if the game can say it, is a box that holds every
/// shape whatever the pose turns out to be (`HitBound`, itself made when
/// first asked: it must cost far less than the shapes). A shot tests it
/// first (`HitShapes::may_meet`) and makes the shapes only when its ray
/// can reach them: a ray far from a body leaves it unposed
/// (docs/performance.md, "A broad phase for the hitscan").
#[derive(Component, Clone, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct LazyHitboxes(pub crate::later::Later<Hitboxes>, pub Option<crate::later::Later<HitBound>>);

/// An axis-aligned box in a character's hitbox space (its `Hitbox`es'
/// own: feet at the origin, turned by its look) that holds every shape of
/// its `LazyHitboxes` (`LazyHitboxes`' second field). A game declares it
/// conservatively: a shape outside it would be missed by every ray that
/// misses the box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitBound {
    pub min: Vec3,
    pub max: Vec3,
}

impl HitBound {
    /// Whether a ray (hitbox space, from `origin` along the about-unit
    /// `dir`) may enter a shape inside the box, grown by `grow` (a box
    /// trace's, as the shapes are), nearer than `limit`: false only when it
    /// surely can't, so a shot that passes the shapes over when this says
    /// no finds what testing them would have found. The box is widened by
    /// a little for rounding (a millimetre, more for a far origin); one
    /// that isn't finite never rules anything out.
    pub fn may_meet(&self, origin: Vec3, dir: Vec3, limit: f32, grow: f32) -> bool {
        if !(self.min.is_finite() && self.max.is_finite()) {
            return true;
        }
        let slack = 1e-3 + 1e-5 * origin.abs().max_element();
        let half = (self.max - self.min) / 2.0 + Vec3::splat(grow.max(0.0) + slack);
        Hitbox::cuboid((self.min + self.max) / 2.0, half.max(Vec3::ZERO), Quat::IDENTITY, BodyRegion::Generic)
            .ray_span(origin, dir)
            .is_some_and(|(enter, _)| enter < limit + slack)
    }

    /// The smallest box holding both.
    pub fn union(self, other: HitBound) -> HitBound {
        HitBound {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// Whether every shape of `boxes` is inside (tests).
    pub fn holds(&self, boxes: &Hitboxes) -> bool {
        boxes.0.iter().all(|b| {
            let (lo, hi) = b.bounds();
            lo.cmpge(self.min).all() && hi.cmple(self.max).all()
        })
    }
}

/// A character's hitboxes whichever way its game keeps them (`Hitboxes`
/// or `LazyHitboxes`), for the shots' queries.
#[derive(bevy::ecs::query::QueryData)]
pub struct HitShapes {
    pub eager: Option<&'static Hitboxes>,
    pub lazy: Option<&'static LazyHitboxes>,
}

impl<'w> HitShapesItem<'w, '_> {
    /// Its hitboxes (made now if they are a `LazyHitboxes` nobody asked
    /// for yet); None: it has none.
    pub fn get(&self) -> Option<&'w Hitboxes> {
        hit_shapes(self.eager, self.lazy)
    }

    /// Whether it has hitboxes (without making them).
    pub fn is_some(&self) -> bool {
        self.eager.is_some() || self.lazy.is_some()
    }

    /// Whether a ray (hitbox space) may enter one of its shapes, grown by
    /// `grow`, nearer than `limit`, as far as its game's bound
    /// (`HitBound::may_meet`) can tell without making them: always true
    /// for ready `Hitboxes`, for lazy ones already made and for a game
    /// that declares no bound.
    pub fn may_meet(&self, origin: Vec3, dir: Vec3, limit: f32, grow: f32) -> bool {
        match self.lazy {
            Some(LazyHitboxes(shapes, Some(bound))) if !shapes.is_made() => bound.get().may_meet(origin, dir, limit, grow),
            _ => true,
        }
    }
}

/// `HitShapes::get` from the two components.
pub fn hit_shapes<'a>(eager: Option<&'a Hitboxes>, lazy: Option<&'a LazyHitboxes>) -> Option<&'a Hitboxes> {
    lazy.map(|l| l.0.get()).or(eager)
}

/// A character's hitboxes in `world` either way (tests, tools).
pub fn hitboxes_in(world: &World, e: Entity) -> Option<&Hitboxes> {
    hit_shapes(world.get::<Hitboxes>(e), world.get::<LazyHitboxes>(e))
}

impl Hitboxes {
    /// The axis-aligned box around every shape (min, max; in their
    /// space), if there are any. A gun whose rule first tests a body's
    /// box (Combat Arms') widens that box by these, so shapes poking out
    /// of the character's hull (another game's taller model's head) still
    /// count, as in Source.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        self.0.iter().map(Hitbox::bounds).reduce(|(a0, a1), (b0, b1)| (a0.min(b0), a1.max(b1)))
    }

    /// The shape a ray (local space) enters first within `limit`: its
    /// index, where it enters and leaves it (CS:S's rule).
    pub fn nearest(&self, origin: Vec3, dir: Vec3, limit: f32) -> Option<(usize, f32, f32)> {
        self.0
            .iter()
            .enumerate()
            .filter_map(|(i, b)| b.ray_span(origin, dir).map(|(enter, leave)| (i, enter, leave)))
            .filter(|(_, t, _)| *t < limit)
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Of the shapes the whole segment passes, the one with the smallest
    /// `Hitbox::power_along` it (Combat Arms' client rule).
    pub fn by_power(&self, start: Vec3, end: Vec3) -> Option<usize> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, b)| b.segment_passes(start, end))
            .map(|(i, b)| (i, b.power_along(start, end)))
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i)
    }

    /// Of the shapes the line through `at` along `dir` passes, the one
    /// of the highest priority; on a tie the later one (Combat Arms'
    /// server walk).
    pub fn by_priority(&self, at: Vec3, dir: Vec3) -> Option<usize> {
        self.0
            .iter()
            .enumerate()
            .filter(|(_, b)| b.line_passes(at, dir))
            .fold(None, |best: Option<(usize, f32)>, (i, b)| match best {
                Some((_, p)) if p > b.priority => best,
                _ => Some((i, b.priority)),
            })
            .map(|(i, _)| i)
    }
}

