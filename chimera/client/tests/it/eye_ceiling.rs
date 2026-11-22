//! The first-person eye against ceilings (`client::view::below_ceiling`)
//! for every game's body: CS:S's box with its eye over the box's top,
//! Combat Arms' cylinder with its eye well under its top, and a Combat
//! Arms character fitted into CS:S's box on a CS:S map
//! (`core::envelope`). Walking up to a barrel, a crate or a wall leaves the
//! eye where it is from any side; a ceiling over the hull keeps the near
//! plane out of it (Combat Arms' camera dropped at its maps' green barrels:
//! the old probe, a square swept up from the origin, reached past a round
//! hull and took a barrel's top for a ceiling).

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_client::client::view::{CameraMode, Hull, camera_offset_for_hull};
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::GreyboxMapPlugin, harness::Sim};

/// CS:S's inch and Combat Arms' unit, metres.
const IN: f32 = 0.0254;
const CA: f32 = 0.0185;

/// A body: its hull (from its collider) and its eye above its origin.
struct Body {
    name: &'static str,
    collider: Collider,
    eye: f32,
}

fn bodies() -> [Body; 3] {
    [
        // CS:S: box 32 x 62, eye 64 above the feet (33 over the centre).
        Body {
            name: "CS:S",
            collider: Collider::cuboid(32.0 * IN, 62.0 * IN, 32.0 * IN),
            eye: 33.0 * IN,
        },
        // Combat Arms: cylinder radius 15, half-height 50, eye 0.66 of it
        // over the centre.
        Body {
            name: "Combat Arms",
            collider: Collider::cylinder(15.0 * CA, 100.0 * CA),
            eye: 33.0 * CA,
        },
        // Combat Arms fitted into CS:S's box (no wider, no taller): its eye
        // at CS:S's own, over the fitted top.
        Body {
            name: "Combat Arms on CS:S",
            collider: Collider::cylinder((15.0 * CA).min(16.0 * IN), 62.0 * IN),
            eye: 33.0 * IN,
        },
    ]
}

/// High over the greybox: nothing else in the way. The body's origin is
/// its hull's centre.
const ORIGIN: Vec3 = Vec3::new(0.0, 40.0, 12.0);

fn eye_with(sim: &mut Sim, body: &Body) -> f32 {
    let hull = Hull::of(&body.collider);
    let eye = Vec3::Y * body.eye;
    sim.ticks(2);
    sim.app
        .world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            camera_offset_for_hull(&CameraMode::default(), ORIGIN, eye, Quat::IDENTITY, &spatial, [], Some(hull))
        })
        .unwrap()
        .y
}

/// A barrel as a map's mesh: a 16-sided trimesh cylinder, `radius` and
/// `height` metres, its base at `base` (world).
fn barrel(radius: f32, height: f32, base: Vec3) -> (RigidBody, Collider, Transform) {
    let n = 16;
    let mut points = Vec::new();
    for y in [0.0, height] {
        for i in 0..n {
            let a = i as f32 / n as f32 * std::f32::consts::TAU;
            points.push(Vec3::new(radius * a.cos(), y, radius * a.sin()));
        }
    }
    points.push(Vec3::ZERO);
    points.push(Vec3::Y * height);
    let (bottom, top) = (2 * n as u32, 2 * n as u32 + 1);
    let mut tris = Vec::new();
    for i in 0..n as u32 {
        let j = (i + 1) % n as u32;
        tris.push([i, j, n as u32 + i]);
        tris.push([j, n as u32 + j, n as u32 + i]);
        tris.push([bottom, j, i]);
        tris.push([top, n as u32 + i, n as u32 + j]);
    }
    (RigidBody::Static, Collider::trimesh(points, tris), Transform::from_translation(base))
}

/// Up against a barrel, a crate and a wall, from the side and the
/// diagonals: the eye stays at eye height, whatever the obstacle's height
/// (under the hull's top, between it and the eye, over the character).
#[test]
fn obstacles_beside_the_hull_leave_the_eye() {
    for body in bodies() {
        let hull = Hull::of(&body.collider);
        let r = hull.half.x;
        let feet = ORIGIN.y - hull.top;
        for height in [0.6 * hull.top * 2.0, hull.top * 2.0 + 0.5 * (body.eye - hull.top).max(0.05), 3.0] {
            for angle in [0.0f32, 45.0, 30.0, 90.0, 135.0] {
                let dir = Vec3::new(angle.to_radians().cos(), 0.0, angle.to_radians().sin());
                // How far the hull reaches along `dir` (a box's corner on
                // the diagonals).
                let reach = if hull.round { r } else { r * (angle.to_radians().cos().abs() + angle.to_radians().sin().abs()) };
                let mut sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);
                // A round barrel touching the hull (radius 0.3 m).
                let b = sim.app.world_mut().spawn(barrel(0.3, height, Vec3::new(ORIGIN.x, feet, ORIGIN.z) + dir * (reach + 0.3 + 0.001))).id();
                let y = eye_with(&mut sim, &body);
                assert!((y - body.eye).abs() < 1e-4, "{}: barrel {height} m tall at {angle} degrees: eye {y}, wanted {}", body.name, body.eye);
                sim.app.world_mut().despawn(b);
                // A crate or wall face touching the hull's side or (for a
                // box) its corner, turned with the direction.
                sim.app.world_mut().spawn((
                    RigidBody::Static,
                    Collider::cuboid(0.8, height, 2.0),
                    Transform::from_translation(Vec3::new(ORIGIN.x, feet + height / 2.0, ORIGIN.z) + dir * (reach + 0.4 + 0.001))
                        .with_rotation(Quat::from_rotation_y(-angle.to_radians())),
                ));
                let y = eye_with(&mut sim, &body);
                assert!((y - body.eye).abs() < 1e-4, "{}: crate {height} m tall at {angle} degrees: eye {y}, wanted {}", body.name, body.eye);
            }
        }
    }
}

/// A ceiling over the hull (a duct as tall as the hull, a brush edge over
/// part of its top): the eye stays the near plane's clearance under it
/// (CS:S's eye over its box drops; an eye under its hull's top already is).
#[test]
fn a_ceiling_over_the_hull_keeps_the_eye_under_it() {
    for body in bodies() {
        let hull = Hull::of(&body.collider);
        // Slabs whose underside touches the hull's top, over its centre and
        // over just its edge.
        for edge in [-1.0, 0.5 * hull.half.x] {
            let mut sim = Sim::new(SimBasePlugins, GreyboxMapPlugin);
            let w = 3.0;
            sim.app.world_mut().spawn((
                RigidBody::Static,
                Collider::cuboid(w, 0.2, w),
                Transform::from_translation(ORIGIN + Vec3::new(edge + w / 2.0, hull.top + 0.1, 0.0)),
            ));
            let y = eye_with(&mut sim, &body);
            assert!(y <= body.eye + 1e-6, "{}: never raised: {y}", body.name);
            assert!(y <= hull.top - 0.1 + 1e-4, "{}: edge {edge}: eye {y} too close to the ceiling at {}", body.name, hull.top);
            if body.eye + 0.12 <= hull.top {
                assert!((y - body.eye).abs() < 1e-4, "{}: an eye under the hull's top stays: {y}", body.name);
            } else {
                assert!(y < body.eye, "{}: an eye over the hull's top drops: {y}", body.name);
            }
        }
    }
}
