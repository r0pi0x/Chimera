//! Bodies in water float (Source's physics floats objects in water
//! brushes by their density against the water's: public physics docs;
//! the numbers below are ours, tech-debt). Each physics step, before the
//! solver: a dynamic body whose box reaches into a water volume gets the
//! water's lift for the part of its collision hulls below the surface
//! (`submerged`: each convex piece cut at the surface; water density × that
//! volume × g, upward, at the centre of the wet part, so a tipped body
//! turns until that is under its mass centre), and the water's drag. A
//! body lighter than the water it would displace floats where its weight of
//! water is under the surface (de_aztec's 30 kg oil drums, 0.44 m³ of hull:
//! 7 % under, on their sides); a heavier one sinks slowly. Bobbing is damped
//! by its own rate (`HEAVE_DAMPING`), so a light body floating high comes to
//! rest in a second or two and sleeps; a sleeping body stays asleep unless
//! its lift is well over its weight (`WAKE_LIFT`). Shapes without hulls
//! (balls, capsules, meshes) use their box's wet share of their volume.
//! `float_line` is where a body's weight of water is under (for checks).
//! Engine space, SI units, the physics step's time.
//!
//! Known shortcuts (docs/tech-debt.md): Water and physics (`map::buoyancy`,
//! `map::controllers`; no spec, public physics docs): lift is water density ×
//! the collision hulls' volume under the surface × g (the box's wet share of
//! the colliders' volume for shapes without hulls), capped at 2 g, the
//! surface the top of the highest water brush the body's box reaches into,
//! with drag in water of 3/s vertically by the wet share but at least a
//! 0.3 damping ratio of the body's bobbing, 0.5/s sideways and 2/s angular;
//! a sleeping body wakes in water only at 25 % more lift than weight, so it
//! may rest a fraction of a unit deep where `settle_props` stopped it;
//! phys_motor (public
//! entity docs) drives the angular velocity about its axis toward its speed
//! over the spin-up time (inertia factor unused); constraints are avian joints
//! that break past their force or torque limit (avian's joint force against the
//! limit in pounds as a weight in the map's gravity, lb·in times units for
//! torque; spec 1.5 leaves the exact measure open), hinge and slider motors,
//! friction and limits, pulleys, springs, ragdoll limits and constraint systems
//! aren't made. Why it's OK for now: The maps' boats, lamps, flowers and
//! drums behave; Source's VPhysics numbers (how deep a prop floats, how fast
//! it settles: docs/backlog.md's open questions) would need measuring.

use std::{collections::HashMap, sync::Arc};

use avian3d::{
    parry::shape::SharedShape,
    prelude::{
        AngularVelocity, Collider, ColliderAabb, ColliderMassProperties, ColliderOf, ComputedAngularInertia,
        ComputedCenterOfMass, ComputedMass, Gravity, LinearVelocity, Physics, PhysicsSystems, Position, RigidBody,
        Rotation, Sleeping,
    },
};
use bevy::prelude::*;

use crate::core::MapWater;

/// On a dynamic body the water lifts (part of it under a surface): what a
/// Source player landing on it counts as a floating object (specs/hl2dm/
/// movement.md "Falling and landing"; set each physics step).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Floating;

/// Water's density, kg/m³.
pub const WATER_DENSITY: f32 = 1000.0;
/// Speed lost in water per second at full depth: vertical (the least:
/// see `HEAVE_DAMPING`), horizontal (boats glide), angular; the
/// horizontal and angular ones whenever a body is in the water at all.
pub const WATER_DRAG: f32 = 3.0;
pub const WATER_SIDE_DRAG: f32 = 0.5;
pub const WATER_ANGULAR_DRAG: f32 = 2.0;
/// The damping ratio of a floating body's bobbing (ours: a bob or two,
/// gone in a second or two). The vertical drag is at least `2 ·
/// HEAVE_DAMPING · ω`, ω the body's bobbing rate (√ of the lift gained
/// per metre deeper, per kg): a light body floating high has little of
/// itself under water, and a drag by its wet share alone hardly slowed it
/// (de_aztec's drums bobbed for ever).
pub const HEAVE_DAMPING: f32 = 0.3;
/// Most lift, in g.
pub const MAX_LIFT_G: f32 = 2.0;
/// A sleeping body in water is woken only when its lift is over its
/// weight by this share (it would rise): one floating at rest sleeps, also
/// where `prop_physics::settle_props` stopped it a little deep in its last
/// bob (it stops bodies that keep within an inch for half a second).
pub const WAKE_LIFT: f32 = 0.25;

/// The fraction of a box (min, max) below a surface at height `surface`.
pub fn wet_fraction(min: Vec3, max: Vec3, surface: f32) -> f32 {
    let h = max.y - min.y;
    if h <= 1e-6 {
        return if min.y < surface { 1.0 } else { 0.0 };
    }
    ((surface - min.y) / h).clamp(0.0, 1.0)
}

/// Upward acceleration of a body of `mass` kg and `volume` m³ with
/// `wet` of it under water.
pub fn lift(mass: f32, volume: f32, wet: f32, g: f32) -> f32 {
    if mass <= 0.0 {
        return 0.0;
    }
    WATER_DENSITY * volume * wet * g / mass
}

/// A closed triangle mesh (consistently wound).
pub type Hull = (Vec<Vec3>, Vec<[u32; 3]>);

/// A collider shape's closed surfaces in its own space (a compound's
/// pieces placed), or None for a shape without one here (balls,
/// capsules, triangle meshes: the box estimate stands in).
pub fn shape_hulls(shape: &SharedShape) -> Option<Vec<Hull>> {
    if let Some(c) = shape.as_compound() {
        let mut out = Vec::new();
        for (pose, piece) in c.shapes() {
            for (points, tris) in shape_hulls(piece)? {
                out.push((points.into_iter().map(|p| pose.transform_point(p)).collect(), tris));
            }
        }
        return Some(out);
    }
    if let Some(p) = shape.as_convex_polyhedron() {
        return Some(vec![p.to_trimesh()]);
    }
    if let Some(b) = shape.as_cuboid() {
        return Some(vec![b.to_trimesh()]);
    }
    None
}

/// The volume (m³) and centre of the part of closed surfaces `hulls`
/// (placed by `place`) below the plane y = `surface`. Each triangle,
/// clipped to below the plane, adds the signed volume of its cone from a
/// point on the plane: the cut face lies in the plane and adds nothing.
pub fn submerged(hulls: &[Hull], place: impl Fn(Vec3) -> Vec3, surface: f32) -> (f32, Vec3) {
    let (mut volume, mut moment) = (0.0f32, Vec3::ZERO);
    for (points, tris) in hulls {
        let world: Vec<Vec3> = points.iter().map(|p| place(*p)).collect();
        if world.iter().all(|p| p.y >= surface) {
            continue;
        }
        let mid = world.iter().copied().sum::<Vec3>() / world.len().max(1) as f32;
        let r = Vec3::new(mid.x, surface, mid.z);
        let (mut v, mut m) = (0.0f32, Vec3::ZERO);
        let mut poly: Vec<Vec3> = Vec::with_capacity(4);
        for t in tris {
            let tri = t.map(|i| world[i as usize]);
            poly.clear();
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                let (da, db) = (a.y - surface, b.y - surface);
                if da <= 0.0 {
                    poly.push(a);
                }
                if (da < 0.0) != (db < 0.0) && (da - db).abs() > 1e-9 {
                    poly.push(a + (b - a) * (da / (da - db)));
                }
            }
            for k in 1..poly.len().saturating_sub(1) {
                let (a, b, c) = (poly[0] - r, poly[k] - r, poly[k + 1] - r);
                let dv = a.dot(b.cross(c)) / 6.0;
                v += dv;
                m += (r + (poly[0] + poly[k] + poly[k + 1] - 3.0 * r) / 4.0) * dv;
            }
        }
        if v.abs() > 1e-12 {
            // The sign is the winding's: the same for volume and moment.
            volume += v.abs();
            moment += m / v * v.abs();
        }
    }
    let centre = if volume > 1e-12 { moment / volume } else { Vec3::ZERO };
    (volume, centre)
}

/// The water height at which `volume` of `hulls` (placed) is under:
/// where a body displacing its own weight floats (bisection, to 0.1 mm).
pub fn float_line(hulls: &[Hull], place: impl Fn(Vec3) -> Vec3 + Copy, volume: f32) -> f32 {
    let ys = hulls.iter().flat_map(|(p, _)| p.iter().map(move |q| place(*q).y));
    let (mut lo, mut hi) = ys.fold((f32::MAX, f32::MIN), |(l, h), y| (l.min(y), h.max(y)));
    while hi - lo > 1e-4 {
        let mid = (lo + hi) * 0.5;
        if submerged(hulls, place, mid).0 < volume {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (lo + hi) * 0.5
}

/// Each collider's closed surfaces, made once from its shape.
#[derive(Default)]
struct HullCache(HashMap<Entity, Option<Arc<Vec<Hull>>>>);

/// One body's water: its colliders' surfaces (placed) or, without them,
/// its box's wet share of its volume.
struct Wet {
    min: Vec3,
    max: Vec3,
    /// Colliders' volume (density 1 mass), m³.
    volume: f32,
    /// (hulls, collider position, rotation) per collider with surfaces.
    pieces: Vec<(Arc<Vec<Hull>>, Vec3, Quat)>,
    /// A collider without surfaces: the box estimate for all.
    boxed: bool,
}

impl Wet {
    /// Volume under `surface` and its centre.
    fn under(&self, surface: f32) -> (f32, Vec3) {
        if self.boxed || self.pieces.is_empty() {
            let f = wet_fraction(self.min, self.max, surface);
            let centre = Vec3::new(
                (self.min.x + self.max.x) * 0.5,
                (self.min.y + surface.min(self.max.y)) * 0.5,
                (self.min.z + self.max.z) * 0.5,
            );
            return (self.volume * f, centre);
        }
        let (mut v, mut m) = (0.0, Vec3::ZERO);
        for (hulls, pos, rot) in &self.pieces {
            let (dv, c) = submerged(hulls, |p| *pos + *rot * p, surface);
            v += dv;
            m += c * dv;
        }
        (v, if v > 0.0 { m / v } else { Vec3::ZERO })
    }
}

#[allow(clippy::type_complexity)]
fn float_bodies(
    water: Option<Res<MapWater>>,
    gravity: Option<Res<Gravity>>,
    time: Res<Time<Physics>>,
    colliders: Query<(
        Entity,
        &ColliderOf,
        &ColliderAabb,
        &ColliderMassProperties,
        &Collider,
        &Position,
        &Rotation,
    )>,
    mut bodies: Query<(
        &RigidBody,
        &mut LinearVelocity,
        &mut AngularVelocity,
        &Position,
        &Rotation,
        &ComputedMass,
        Option<&ComputedCenterOfMass>,
        Option<&ComputedAngularInertia>,
        Has<Sleeping>,
    )>,
    mut cache: Local<HullCache>,
    floating: Query<Entity, With<Floating>>,
    mut commands: Commands,
) {
    let Some(water) = water.filter(|w| !w.0.is_empty()) else {
        return;
    };
    // The dynamic bodies looked at, and whether the water lifts each.
    let mut afloat: Vec<(Entity, bool)> = Vec::new();
    let g = gravity.map_or(9.81, |g| g.0.length());
    // The physics step's own (its gravity's): not the fixed tick's, which
    // differ when physics is slowed or a tick is long.
    let h = time.delta_secs();
    if h <= 0.0 {
        return;
    }
    // Each body's box, volume and surfaces, from its colliders.
    let mut wets: HashMap<Entity, Wet> = HashMap::new();
    for (e, of, aabb, props, collider, pos, rot) in &colliders {
        // Only dynamic bodies float: a map's static colliders (most of
        // them) are passed over here, not gathered and dropped below.
        if !bodies.get(of.body).is_ok_and(|b| b.0.is_dynamic()) {
            continue;
        }
        let w = wets.entry(of.body).or_insert(Wet {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
            volume: 0.0,
            pieces: Vec::new(),
            boxed: false,
        });
        w.min = w.min.min(aabb.min);
        w.max = w.max.max(aabb.max);
        w.volume += props.mass;
        let hulls = cache
            .0
            .entry(e)
            .or_insert_with(|| shape_hulls(collider.shape_scaled()).filter(|h| !h.is_empty()).map(Arc::new));
        match hulls {
            Some(hulls) => w.pieces.push((hulls.clone(), pos.0, rot.0)),
            None => w.boxed = true,
        }
    }
    if cache.0.len() > 4 * wets.len() + 64 {
        cache.0.retain(|e, _| colliders.contains(*e));
    }
    for (body, wet) in wets {
        let Ok((kind, mut v, mut w, pos, rot, mass, com, inertia, sleeping)) = bodies.get_mut(body) else {
            continue;
        };
        if !kind.is_dynamic() || wet.volume <= 0.0 {
            continue;
        }
        afloat.push((body, false));
        let (min, max) = (wet.min, wet.max);
        // The highest surface of the water volumes it reaches into.
        let mut surface = f32::MIN;
        for vol in &water.0 {
            let b = &vol.brush;
            if max.x < b.min.x || min.x > b.max.x || max.z < b.min.z || min.z > b.max.z || min.y > b.max.y {
                continue;
            }
            if min.y < b.max.y && b.max.y > surface {
                surface = b.max.y;
            }
        }
        if surface == f32::MIN {
            continue;
        }
        let (under, wet_centre) = wet.under(surface);
        if under <= 0.0 {
            continue;
        }
        let m = mass.value();
        if m <= 0.0 {
            continue;
        }
        if let Some(a) = afloat.last_mut() {
            a.1 = true;
        }
        let share = (under / wet.volume).clamp(0.0, 1.0);
        // At most twice gravity: a very light body comes up briskly
        // without shooting out (where it floats is unchanged: there the
        // lift is one g).
        let up = (WATER_DENSITY * under * g / m).min(MAX_LIFT_G * g);
        if sleeping {
            // At rest afloat (or on the bottom): let it sleep. Woken, it
            // floats from the next step (a push now would wait in its
            // velocity, the step still taking it as asleep: a kick).
            if up > g * (1.0 + WAKE_LIFT) {
                commands.entity(body).remove::<Sleeping>();
            }
            continue;
        }
        v.0.y += up * h;
        // Lift acts at the centre of the wet part: a tipped body turns
        // until that is under its mass centre.
        let centre = pos.0 + rot.0 * com.map_or(Vec3::ZERO, |c| c.0);
        let arm = Vec3::new(wet_centre.x - centre.x, 0.0, wet_centre.z - centre.z);
        let torque = arm.cross(Vec3::Y * up * m);
        if let Some(i) = inertia {
            w.0 += rot.0 * (i.inverse() * (rot.0.inverse() * torque)) * h;
        }
        // Bobbing's rate: the lift gained a centimetre deeper.
        let (deeper, _) = wet.under(surface + 0.01);
        let stiffness = (WATER_DENSITY * (deeper - under).max(0.0) / 0.01 * g / m).max(0.0);
        let heave = (WATER_DRAG * share).max(2.0 * HEAVE_DAMPING * stiffness.sqrt());
        v.0.y *= 1.0 / (1.0 + heave * h);
        let side = 1.0 / (1.0 + WATER_SIDE_DRAG * h);
        v.0.x *= side;
        v.0.z *= side;
        w.0 *= 1.0 / (1.0 + WATER_ANGULAR_DRAG * h);
    }
    for (body, lifted) in afloat {
        if lifted != floating.contains(body) {
            if lifted {
                commands.entity(body).try_insert(Floating);
            } else {
                commands.entity(body).try_remove::<Floating>();
            }
        }
    }
}

pub struct BuoyancyPlugin;

impl Plugin for BuoyancyPlugin {
    fn build(&self, app: &mut App) {
        // After avian's preparation (positions, collider shapes and mass
        // properties it reads), before the step.
        app.add_systems(
            FixedPostUpdate,
            float_bodies
                .after(PhysicsSystems::Prepare)
                .before(PhysicsSystems::StepSimulation),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floats_where_lift_meets_weight() {
        // A 0.5 m³ box of 350 kg (density 700): balanced when 70 % is
        // under water.
        let g = 9.81;
        assert!((lift(350.0, 0.5, 0.7, g) - g).abs() < 1e-4);
        assert!(lift(350.0, 0.5, 1.0, g) > g);
        // Steel (7800) can't float.
        assert!(lift(3900.0, 0.5, 1.0, g) < g);
        assert_eq!(wet_fraction(Vec3::ZERO, Vec3::ONE, 0.25), 0.25);
        assert_eq!(wet_fraction(Vec3::ZERO, Vec3::ONE, 2.0), 1.0);
        assert_eq!(wet_fraction(Vec3::ZERO, Vec3::ONE, -1.0), 0.0);
    }

    #[test]
    fn the_wet_part_of_a_hull_is_cut_at_the_surface() {
        let cube = shape_hulls(&SharedShape::cuboid(0.5, 0.5, 0.5)).unwrap();
        // A 1 m cube, centred at the origin, a quarter under water.
        let (v, c) = submerged(&cube, |p| p, -0.25);
        assert!((v - 0.25).abs() < 1e-4, "{v}");
        assert!((c - Vec3::new(0.0, -0.375, 0.0)).length() < 1e-4, "{c}");
        // Turned 45 degrees about z with the surface through its centre:
        // half under, the centre of that half below the middle.
        let r = Quat::from_rotation_z(std::f32::consts::FRAC_PI_4);
        let (v, c) = submerged(&cube, |p| r * p, 0.0);
        assert!((v - 0.5).abs() < 1e-4, "{v}");
        assert!(c.y < -0.2 && c.x.abs() < 1e-4, "{c}");
        // An edge down, a sliver under: the box estimate would say more.
        let (v, _) = submerged(&cube, |p| r * p, -0.6);
        let sliver = (0.7071 - 0.6f32).powi(2);
        assert!((v - sliver).abs() < 1e-3, "{v} against {sliver}");
        assert_eq!(submerged(&cube, |p| p + Vec3::Y * 2.0, 0.0).0, 0.0);
    }
}
