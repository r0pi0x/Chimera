//! The over-the-shoulder camera's aim (`client::third_person`;
//! docs/plans/active/third-person-camera.md, M1) on the test kit: the
//! command's angles run from the eye to where the camera's crosshair ray
//! lands, so a shot lands where the crosshair points, near and far; round
//! a wall's edge the camera sees past, the shot stops on the wall and the
//! blocked-shot marker sits there; the camera never ends inside a wall.
//! M2: the plugin on the kit: the glide to and from the eye, the dimmed
//! crosshair and the target-ID ray while the marker shows, the recoil in
//! the camera's look, the distances scaled with the eye height, the host's
//! switch.

use avian3d::prelude::*;
use bevy::{ecs::system::RunSystemOnce, prelude::*};
use chimera_client::client::{
    FirstPersonCamera,
    aim::{AimRay, character_under, crosshair_ray},
    input::FreeLook,
    third_person::{
        CameraData, GameCamera, Shoulder, ShoulderSettings, ShoulderView, ThirdPersonPlugin, aim_point, angles_of, blocked_point,
        camera_from_eye, fit, follow_step, wanted_offsets,
    },
    view::CameraMode,
};
use chimera_core::core::{CHARACTER_FILTER, Health, LocalPlayer, NetRole, ThirdPersonRules};
use chimera_sim::testkit::{self, RIFLE};
use chimera_sim::SimBasePlugins;
use chimera_world::{greybox::{self, GreyboxMapPlugin}, harness::Sim};

/// The shooter at the greybox's first spawn looking along -Z (yaw 0), and
/// a target straight ahead of the right-shoulder camera, `ahead` metres
/// from the eye.
fn scene(ahead: f32) -> (Sim, Entity, Entity) {
    scene_with(ahead, |_| {})
}

/// `scene` with `prep` on the kit's app before it starts (plugins).
fn scene_with(ahead: f32, prep: impl FnOnce(&mut App)) -> (Sim, Entity, Entity) {
    let mut sim = Sim::with(SimBasePlugins, |app| {
        app.add_plugins(GreyboxMapPlugin);
        testkit::add_weapons(app);
        prep(app);
    });
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

/// The kit with the shoulder camera's plugin and a local player (the
/// scene's shooter) on it, the camera as a child at the right shoulder.
fn client_scene(ahead: f32) -> (Sim, Entity, Entity) {
    let (mut sim, shooter, target) = scene_with(ahead, |app| {
        app
            .add_plugins(ThirdPersonPlugin)
            .init_resource::<CameraMode>()
            .init_resource::<FreeLook>();
    });
    let (eye, cam) = camera(&sim, shooter);
    let at = sim.position(shooter);
    sim.app.world_mut().entity_mut(shooter).insert(LocalPlayer).with_child((
        FirstPersonCamera,
        Camera::default(),
        Transform::from_translation(cam - at),
    ));
    let _ = eye;
    (sim, shooter, target)
}

fn third_person(sim: &mut Sim, on: bool) {
    sim.app.world_mut().resource_mut::<CameraMode>().third_person = on;
}

#[test]
fn the_camera_glides_to_the_shoulder_and_back_to_the_eye() {
    let (mut sim, ..) = client_scene(10.0);
    assert!(!sim.app.world().resource::<Shoulder>().drawn(), "first person at the eye");
    third_person(&mut sim, true);
    sim.seconds(0.1);
    let s = sim.app.world().resource::<Shoulder>().clone();
    assert!(s.on && s.blend > 0.0 && s.blend < 1.0, "gliding out: {}", s.blend);
    sim.seconds(0.4);
    assert_eq!(sim.app.world().resource::<Shoulder>().blend, 1.0, "at its spot");
    third_person(&mut sim, false);
    sim.seconds(0.1);
    let s = sim.app.world().resource::<Shoulder>().clone();
    assert!(!s.on && s.drawn() && s.blend < 1.0 && s.blend > 0.0, "gliding back, input already the eye's: {}", s.blend);
    assert!(s.exit_look.is_some());
    sim.seconds(0.4);
    let s = sim.app.world().resource::<Shoulder>();
    assert!(!s.drawn() && s.look.is_none() && s.exit_look.is_none(), "at the eye again");
}

#[test]
fn a_glide_stands_between_the_eye_and_the_spot_and_the_recoil_shakes_the_camera() {
    let (mut sim, shooter, _) = client_scene(10.0);
    third_person(&mut sim, true);
    sim.seconds(0.6);
    let eye = sim.position(shooter) + sim.state(shooter).eye_offset;
    let origin = sim.position(shooter);
    let place = |sim: &mut Sim, kick: Vec3| {
        let eye_off = sim.state(shooter).eye_offset;
        sim.app
            .world_mut()
            .run_system_once(move |mut view: ShoulderView, spatial: SpatialQuery| {
                let filter = SpatialQueryFilter::from_excluded_entities([shooter]).with_mask(CHARACTER_FILTER);
                view.place(origin, eye_off, Quat::IDENTITY, Quat::IDENTITY, kick, Vec2::ZERO, &spatial, &filter)
            })
            .unwrap()
            .expect("drawn")
    };
    let (far, look) = place(&mut sim, Vec3::ZERO);
    assert!(far.distance(eye - origin) > 0.5, "off the eye, at its spot: {far}");
    // The recoil: pitched up 0.1 rad, the camera's look is.
    let (_, kicked) = place(&mut sim, Vec3::new(0.1, 0.0, 0.0));
    assert!((kicked.angle_between(look) - 0.1).abs() < 1e-3, "{}", kicked.angle_between(look));
    // Half way: between the eye and the spot.
    sim.app.world_mut().resource_mut::<Shoulder>().blend = 0.5;
    let (mid, _) = place(&mut sim, Vec3::ZERO);
    let (from, to) = (eye - origin, far);
    // Smoothstep(0.5) is a half.
    assert!((mid.distance(from) - 0.5 * to.distance(from)).abs() < 1e-3, "{mid} between {from} and {to}");
    sim.app.world_mut().resource_mut::<Shoulder>().blend = 0.0;
    sim.app.world_mut().resource_mut::<Shoulder>().on = false;
    sim.app.world_mut().resource_mut::<Shoulder>().exit_look = None;
    sim.app.world_mut().resource_mut::<Shoulder>().look = None;
    let none = sim
        .app
        .world_mut()
        .run_system_once(move |mut view: ShoulderView, spatial: SpatialQuery| {
            let filter = SpatialQueryFilter::from_excluded_entities([shooter]).with_mask(CHARACTER_FILTER);
            view.place(origin, Vec3::Y, Quat::IDENTITY, Quat::IDENTITY, Vec3::ZERO, Vec2::ZERO, &spatial, &filter)
        })
        .unwrap();
    assert!(none.is_none(), "first person: the camera is the eye's");
}

#[test]
fn the_distances_scale_with_the_eye_height() {
    let s = ShoulderSettings::default();
    let tall = s.scaled(1.25);
    assert!((tall.dist - s.dist * 1.25).abs() < 1e-6 && (tall.side - s.side * 1.25).abs() < 1e-6 && (tall.up - s.up * 1.25).abs() < 1e-6);
    assert_eq!(tall.swap_secs, s.swap_secs, "only the distances");
    let (mut sim, shooter, _) = client_scene(10.0);
    third_person(&mut sim, true);
    sim.seconds(0.3);
    let state = sim.app.world().resource::<Shoulder>().clone();
    let stand = chimera_client::client::third_person::eye_height(&sim.state(shooter));
    assert!(stand > 1.4 && stand < 1.9, "eye height above the feet: {stand}");
    assert!((state.stand_eye - stand).abs() < 1e-5);
    assert!((state.scale() - stand / chimera_client::client::third_person::REF_EYE).abs() < 1e-5);
}

#[test]
fn the_marker_dims_the_crosshair_and_the_target_id_follows_the_crosshair() {
    let (mut sim, shooter, target) = client_scene(10.0);
    third_person(&mut sim, true);
    sim.seconds(0.6);
    let (eye, _) = camera(&sim, shooter);
    // The eye's own ray misses the target (it stands at the shoulder's
    // line); the crosshair's ray finds it.
    let eye_ray = AimRay { eye, dir: Dir3::NEG_Z };
    let find = |sim: &mut Sim, ray: AimRay| {
        sim.app
            .world_mut()
            .run_system_once(move |spatial: SpatialQuery, parents: Query<&ChildOf>, who: Query<(), With<Health>>| {
                character_under(&spatial, ray, 100.0, shooter, |e| who.contains(e), &parents).map(|h| h.0)
            })
            .unwrap()
    };
    assert_eq!(find(&mut sim, eye_ray), None, "the eye's ray passes beside it");
    let state = sim.app.world().resource::<Shoulder>().clone();
    assert_eq!(state.dim, 0.0, "a clear line: the crosshair as it is");
    let ray = crosshair_ray(Some(&state), eye_ray);
    assert_eq!(find(&mut sim, ray), Some(target), "the crosshair's ray");
    // The off camera leaves the eye's ray alone.
    let off = Shoulder::default();
    assert_eq!(crosshair_ray(Some(&off), eye_ray), eye_ray);
    // A wall's edge: the marker shows, the crosshair dims, and the name
    // goes (the crosshair's ray still sees the target past the edge).
    sim.app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(1.0, 4.0, 0.2),
        Transform::from_translation(eye + Vec3::new(-0.15, 0.0, -3.0)),
    ));
    sim.seconds(0.6);
    let s = sim.app.world().resource::<Shoulder>().clone();
    assert!(s.blocked.is_some(), "the marker shows");
    assert!(s.dim > 0.99, "the crosshair is dimmed: {}", s.dim);
}

#[test]
fn a_server_that_forbids_third_person_keeps_the_client_in_first() {
    let (mut sim, ..) = client_scene(10.0);
    sim.app.insert_resource(NetRole::Client);
    third_person(&mut sim, true);
    sim.seconds(0.4);
    assert!(!sim.app.world().resource::<Shoulder>().drawn(), "allow_thirdperson 0: first person");
    assert!(sim.app.world().resource::<CameraMode>().held_off);
    sim.app.world_mut().resource_mut::<ThirdPersonRules>().allow = 1;
    sim.seconds(0.6);
    assert_eq!(sim.app.world().resource::<Shoulder>().blend, 1.0, "allowed: the shoulder camera");
}

/// Run console words on the sim's world.
fn console(sim: &mut Sim, words: &[&str]) {
    let words: Vec<String> = words.iter().map(|w| w.to_string()).collect();
    chimera_core::console::execute(sim.app.world_mut(), &words, 0);
}

/// Where the shoulder camera stands relative to the shooter's origin now.
fn camera_offset(sim: &mut Sim, shooter: Entity) -> Vec3 {
    let origin = sim.position(shooter);
    let eye_off = sim.state(shooter).eye_offset;
    sim.app
        .world_mut()
        .run_system_once(move |mut view: ShoulderView, spatial: SpatialQuery| {
            let filter = SpatialQueryFilter::from_excluded_entities([shooter]).with_mask(CHARACTER_FILTER);
            view.place(origin, eye_off, Quat::IDENTITY, Quat::IDENTITY, Vec3::ZERO, Vec2::ZERO, &spatial, &filter)
        })
        .unwrap()
        .expect("drawn")
        .0
}

#[test]
fn the_follow_spring_settles_without_overshoot() {
    let (mut pos, mut vel) = (Vec3::ZERO, Vec3::ZERO);
    let target = Vec3::X;
    let mut last = 0.0;
    for _ in 0..200 {
        (pos, vel) = follow_step(pos, vel, target, 0.1, 1.0 / 60.0);
        assert!(pos.x <= 1.0 + 1e-5 && pos.x >= last - 1e-6, "monotone, no overshoot: {}", pos.x);
        last = pos.x;
    }
    assert!((pos.x - 1.0).abs() < 1e-3 && vel.length() < 1e-2, "settled: {pos} {vel}");
    assert_eq!(follow_step(Vec3::ZERO, Vec3::ZERO, target, 0.0, 0.01).0, target, "0 is off");
}

#[test]
fn the_camera_trails_the_character_a_little_and_settles() {
    let (mut sim, shooter, _) = client_scene(10.0);
    third_person(&mut sim, true);
    sim.seconds(1.0);
    let at_rest = camera_offset(&mut sim, shooter);
    // The character jumps 0.3 m sideways: the camera stays behind a moment.
    sim.app.world_mut().get_mut::<Transform>(shooter).unwrap().translation.x += 0.3;
    sim.ticks(1);
    let lagging = camera_offset(&mut sim, shooter);
    let lag = (lagging - at_rest).x;
    assert!(lag < -0.05 && lag > -0.3, "trails the move, but not all of it: {lag}");
    // (The camera is placed once a frame.)
    for _ in 0..200 {
        sim.ticks(1);
        camera_offset(&mut sim, shooter);
    }
    let settled = camera_offset(&mut sim, shooter);
    assert!(settled.distance(at_rest) < 0.01, "back on its spot: {settled} vs {at_rest}");
    // cam_smooth 0: none.
    console(&mut sim, &["chimera:cam_smooth", "0"]);
    sim.app.world_mut().get_mut::<Transform>(shooter).unwrap().translation.x += 0.3;
    sim.ticks(1);
    assert!(camera_offset(&mut sim, shooter).distance(at_rest) < 0.01, "no lag with cam_smooth 0");
}

#[test]
fn a_scope_takes_the_view_to_first_person_and_gives_it_back() {
    let (mut sim, shooter, _) = client_scene(10.0);
    third_person(&mut sim, true);
    sim.seconds(0.6);
    assert_eq!(sim.app.world().resource::<Shoulder>().blend, 1.0);
    let scope = chimera_sim::weapon::Zoomed { fov: 20.0, scope: true, sensitivity: None };
    let place = |sim: &mut Sim, z: Option<chimera_sim::weapon::Zoomed>| {
        let mut e = sim.app.world_mut().entity_mut(shooter);
        match z {
            Some(z) => e.insert(z),
            None => e.remove::<chimera_sim::weapon::Zoomed>(),
        };
    };
    place(&mut sim, Some(scope));
    sim.app.world_mut().run_system_once(|| {}).unwrap();
    sim.ticks(1);
    let s = sim.app.world().resource::<Shoulder>().clone();
    assert!(!s.on && s.blend == 0.0 && !s.drawn(), "scoped: first person at once ({} {})", s.on, s.blend);
    // A zoom without a scope keeps the camera (the sights are the gun's).
    place(&mut sim, None);
    sim.seconds(0.6);
    assert_eq!(sim.app.world().resource::<Shoulder>().blend, 1.0, "unscoped: back on the shoulder");
}

#[test]
fn the_cvars_set_the_camera_and_the_mode() {
    let (mut sim, ..) = client_scene(10.0);
    console(&mut sim, &["chimera:cam_fov", "100"]);
    console(&mut sim, &["chimera:cam_smooth", "0.2"]);
    let s = *sim.app.world().resource::<ShoulderSettings>();
    assert_eq!((s.fov, s.smooth), (100.0, 0.2));
    assert!(!sim.app.world().resource::<CameraMode>().third_person);
    console(&mut sim, &["chimera:thirdperson", "1"]);
    assert!(sim.app.world().resource::<CameraMode>().third_person);
    sim.seconds(0.6);
    assert!(sim.app.world().resource::<Shoulder>().on);
    console(&mut sim, &["chimera:thirdperson", "0"]);
    assert!(!sim.app.world().resource::<CameraMode>().third_person);
}

#[test]
fn the_data_file_gives_the_defaults_and_each_game_its_factors() {
    let data = CameraData::builtin();
    assert_eq!(ShoulderSettings::default(), data.camera);
    assert!(data.camera.dist > 1.0 && data.camera.smooth > 0.0 && data.camera.marker_size > 0.0);
    for game in ["cs_source", "combat_arms", "hl2dm", "tf2", "halo_ce"] {
        assert!(data.games.contains_key(game), "{game} has a table");
    }
    // A game's factors reach the distances the camera uses.
    let (mut sim, shooter, _) = client_scene(10.0);
    let mut d = CameraData::builtin();
    d.games.insert("tester".into(), GameCamera { dist: 2.0, side: 1.0, up: 1.0 });
    sim.app.insert_resource(d);
    sim.app.world_mut().entity_mut(shooter).insert(chimera_core::slots::GameIdentity("tester".into()));
    third_person(&mut sim, true);
    sim.seconds(0.6);
    let s = sim.app.world().resource::<Shoulder>().clone();
    assert_eq!(s.game.dist, 2.0);
    let base = ShoulderSettings::default();
    assert!((s.tuned(&base).dist - base.dist * s.scale() * 2.0).abs() < 1e-5);
}
