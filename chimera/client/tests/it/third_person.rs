//! The over-the-shoulder camera's aim (`client::third_person`;
//! docs/plans/active/third-person-camera.md, M1) on the test kit: the
//! command's angles run from the eye to where the camera's crosshair ray
//! lands, so a shot lands where the crosshair points, near and far; round
//! a wall's edge the camera sees past, the shot stops on the wall and the
//! blocked-shot marker sits there; the camera never ends inside a wall.

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_client::client::third_person::{ShoulderSettings, aim_point, angles_of, blocked_point, camera_from_eye, fit, wanted_offsets};
use chimera_core::core::{CHARACTER_FILTER, Health};
use chimera_sim::testkit::{self, RIFLE};
use chimera_world::{greybox, harness::Sim};

/// The shooter at the greybox's first spawn looking along -Z (yaw 0), and
/// a target straight ahead of the right-shoulder camera, `ahead` metres
/// from the eye.
fn scene(ahead: f32) -> (Sim, Entity, Entity) {
    let mut sim = testkit::kit_sim();
    let shooter = testkit::with_player(&mut sim, None, RIFLE);
    let s = ShoulderSettings::default();
    let target = sim.spawn_character(greybox::SPAWNS[0] + Vec3::new(s.side, 0.0, -ahead), chimera_sim::movement::placeholder::ID);
    sim.seconds(1.0);
    (sim, shooter, target)
}

/// The eye and the camera (no walls behind it here) looking along -Z.
fn camera(sim: &Sim, shooter: Entity) -> (Vec3, Vec3) {
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    let s = ShoulderSettings::default();
    let (pivot, dist) = wanted_offsets(&s, 1.0, 0.0, Quat::IDENTITY);
    (eye, eye + camera_from_eye(pivot, dist, Quat::IDENTITY, 0.0))
}

/// Aim as the shoulder camera does, fire one round: (the marker, the
/// target's health lost).
fn aim_and_fire(sim: &mut Sim, shooter: Entity, target: Entity) -> (Option<Vec3>, f32) {
    let (eye, cam) = camera(sim, shooter);
    // The crosshair on the target's middle.
    let dir = Dir3::new(sim.position(target) - cam).unwrap();
    let (point, marker) = sim
        .app
        .world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            let point = aim_point(&spatial, cam, dir, eye, &[shooter], 200.0);
            (point, blocked_point(&spatial, eye, point, &[shooter]))
        })
        .unwrap();
    let angles = angles_of(point - eye);
    {
        let mut i = sim.intent(shooter);
        i.yaw = angles.x;
        i.pitch = angles.y;
    }
    let before = sim.app.world().get::<Health>(target).unwrap().current;
    sim.intent(shooter).fire = true;
    sim.ticks(1);
    sim.intent(shooter).fire = false;
    sim.ticks(2);
    let after = sim.app.world().get::<Health>(target).unwrap().current;
    (marker, before - after)
}

#[test]
fn the_shot_lands_where_the_crosshair_points_near_and_far() {
    for ahead in [4.0, 25.0] {
        let (mut sim, shooter, target) = scene(ahead);
        let (marker, lost) = aim_and_fire(&mut sim, shooter, target);
        assert!(marker.is_none(), "{ahead} m: the line is clear");
        assert!(lost > 0.0, "{ahead} m: the target is hit");
    }
}

#[test]
fn round_a_wall_s_edge_the_shot_hits_the_wall_and_the_marker_shows_it() {
    let (mut sim, shooter, target) = scene(10.0);
    let (eye, _) = camera(&sim, shooter);
    // A wall 3 m ahead covering the eye's line to the target (which runs
    // 0.18 m right of the eye there) but not the camera's (0.6 m right).
    let wall = sim
        .app
        .world_mut()
        .spawn((
            RigidBody::Static,
            Collider::cuboid(1.0, 4.0, 0.2),
            Transform::from_translation(eye + Vec3::new(-0.15, 0.0, -3.0)),
        ))
        .id();
    sim.ticks(2);
    let (marker, lost) = aim_and_fire(&mut sim, shooter, target);
    let marker = marker.expect("the marker shows");
    assert!((marker.z - (eye.z - 2.9)).abs() < 0.05, "on the wall's face: {marker}");
    assert_eq!(lost, 0.0, "the wall takes the shot");
    // Step out (the wall gone): the marker hides and the shot hits.
    sim.app.world_mut().despawn(wall);
    // (The gun ready again, its kick settled.)
    sim.seconds(1.0);
    let (marker, lost) = aim_and_fire(&mut sim, shooter, target);
    assert!(marker.is_none());
    assert!(lost > 0.0);
}

#[test]
fn backed_into_a_corner_the_camera_stays_out_of_the_walls() {
    let (mut sim, shooter, _) = scene(10.0);
    let (eye, _) = camera(&sim, shooter);
    // A wall 0.5 m behind and one 0.4 m to the right.
    for (at, size) in [
        (eye + Vec3::new(0.0, 0.0, 0.6), Vec3::new(4.0, 4.0, 0.2)),
        (eye + Vec3::new(0.5, 0.0, 0.0), Vec3::new(0.2, 4.0, 4.0)),
    ] {
        sim.app
            .world_mut()
            .spawn((RigidBody::Static, Collider::cuboid(size.x, size.y, size.z), Transform::from_translation(at)));
    }
    sim.ticks(2);
    let (cam, inside) = sim
        .app
        .world_mut()
        .run_system_once(move |spatial: SpatialQuery| {
            let s = ShoulderSettings::default();
            let (pivot, dist) = wanted_offsets(&s, 1.0, 0.0, Quat::IDENTITY);
            let filter = SpatialQueryFilter::from_excluded_entities([shooter]).with_mask(CHARACTER_FILTER);
            let (pivot, reach) = fit(&spatial, &filter, eye, pivot, dist, Quat::IDENTITY);
            let cam = eye + camera_from_eye(pivot, reach, Quat::IDENTITY, 0.0);
            let inside = spatial.shape_intersections(&Collider::sphere(0.15), cam, Quat::IDENTITY, &filter);
            (cam, inside)
        })
        .unwrap();
    assert!(inside.is_empty(), "camera at {cam} touches {inside:?}");
    assert!(cam.x < eye.x + 0.5 && cam.z < eye.z + 0.6, "between the walls: {cam}");
}
