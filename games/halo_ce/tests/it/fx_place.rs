//! Where Halo's gun effects go (`game::fx::place`), without the disc: a
//! view model's marker follows its pose (a bone turned moves the muzzle
//! with it) and is re-projected by the two cameras' fields of view; on
//! `HaloSim` with the library of `fx.rs` and a stand-in view model, the
//! shooter's muzzle light and flash start at the view model's `primary
//! trigger` attachment (not at a fixed offset from the camera), everyone
//! else's at the held model's attachment (`HeldAttachment`), and the
//! bind-pose marker only with neither; the muzzle light follows its
//! shooter (`HaloLight::follow`).

use std::sync::Arc;

use bevy::prelude::*;
use chimera_core::core::{Intent, LocalPlayer, MovementState};
use chimera_sim::weapon::{aim_of, give};
use chimera_world::{
    greybox::{self, GreyboxMapPlugin},
    harness::Sim,
    map::{
        MapBone, ViewModels,
        view_model::{HeldAttachment, MapAttachment, MapViewModel},
    },
};
use halo_ce::{
    game::{
        body::skeleton,
        fx::{
            ActiveFx, HaloLight,
            place::{Frame, bind_pose_frame, camera, view_model_frames},
        },
        movement,
        weapons::ASSAULT_RIFLE,
    },
    harness::HaloSim,
};

use super::fx::library;

/// A one-bone view model rooted at the camera as Halo's are (its axes and
/// units), its muzzle 1 WU forward of the bone, 0.1 left, 0.2 down.
fn model(key: &str) -> MapViewModel {
    MapViewModel {
        key: key.into(),
        model: Default::default(),
        bones: vec![MapBone { name: "gun".into(), parent: None, position: Vec3::ZERO, rotation: Quat::IDENTITY }],
        root: skeleton::root(),
        animations: None,
        right_handed: true,
        allow_flipping: false,
        attachments: vec![MapAttachment {
            name: "primary trigger".into(),
            bone: 0,
            local: Transform::from_xyz(1.0, 0.1, -0.2),
        }],
        light_origin: Vec3::ZERO,
    }
}

#[test]
fn a_view_models_marker_follows_its_pose() {
    let eye = camera(Vec3::new(1.0, 2.0, 3.0), Quat::from_rotation_y(0.7));
    let m = model(ASSAULT_RIFLE);
    let at = |pose: &[(Quat, Vec3)], fovs| view_model_frames(eye, &m, pose, &Transform::IDENTITY, fovs, "PRIMARY trigger")[0];
    let rest = at(&[], None);
    let k = halo_ce::game::METERS_PER_UNIT;
    assert!(rest.at.abs_diff_eq(eye.at + eye.forward * k + eye.left() * 0.1 * k - eye.up * 0.2 * k, 1e-4), "{rest:?}");
    assert!(rest.forward.abs_diff_eq(eye.forward, 1e-5) && rest.up.abs_diff_eq(eye.up, 1e-5));
    // The bone turned a quarter left about Halo's up: the muzzle with it.
    let turned = at(&[(Quat::from_rotation_z(std::f32::consts::FRAC_PI_2), Vec3::ZERO)], None);
    assert!(turned.at.abs_diff_eq(eye.at + eye.left() * k - eye.forward * 0.1 * k - eye.up * 0.2 * k, 1e-4), "{turned:?}");
    assert!(turned.forward.abs_diff_eq(eye.left(), 1e-5));
    // Drawn at a narrower view model camera: right and up scaled out.
    let wide = at(&[], Some((90.0, 60.0)));
    let s = 45f32.to_radians().tan() / 30f32.to_radians().tan();
    let expected = eye.at + eye.forward * k + eye.left() * 0.1 * k * s - eye.up * 0.2 * k * s;
    assert!(wide.at.abs_diff_eq(expected, 1e-4), "{wide:?}");
    // No such marker: none.
    assert!(view_model_frames(eye, &m, &[], &Transform::IDENTITY, None, "vent").is_empty());
}

/// A Spartan holding the assault rifle with the library and, with
/// `views`, a stand-in view model.
fn armed(views: bool) -> (Sim, Entity) {
    let mut sim = Sim::new(HaloSim, GreyboxMapPlugin);
    sim.app.insert_resource(ActiveFx(Some(Arc::new(library()))));
    if views {
        sim.app.world_mut().insert_resource(ViewModels::of([model(ASSAULT_RIFLE)]));
    }
    let p = sim.spawn_character(greybox::SPAWNS[0], movement::ID);
    sim.ticks(2);
    give(sim.app.world_mut(), p, ASSAULT_RIFLE).unwrap();
    sim.seconds(2.0);
    (sim, p)
}

/// One shot: the muzzle light it asked for and the shooter's eye frame.
fn shot_light(sim: &mut Sim, p: Entity) -> (HaloLight, Frame) {
    let mut cursor = sim.app.world().resource::<Messages<HaloLight>>().get_cursor_current();
    sim.intent(p).fire = true;
    let mut light = None;
    let mut eye = None;
    for _ in 0..4 {
        let w = sim.app.world();
        let i = w.get::<Intent>(p).unwrap();
        let at = w.get::<Transform>(p).unwrap().translation + w.get::<MovementState>(p).unwrap().eye_offset;
        let frame = camera(at, aim_of(i.yaw, i.pitch));
        sim.ticks(1);
        if let Some(l) = cursor.read(sim.app.world().resource::<Messages<HaloLight>>()).next() {
            light = Some(l.clone());
            eye = Some(frame);
            break;
        }
    }
    sim.intent(p).fire = false;
    (light.expect("a muzzle light"), eye.unwrap())
}

#[test]
fn the_shooters_effects_start_at_the_view_models_muzzle() {
    let (mut sim, p) = armed(true);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let (light, eye) = shot_light(&mut sim, p);
    let muzzle = view_model_frames(eye, &model(ASSAULT_RIFLE), &[], &Transform::IDENTITY, None, "primary trigger")[0];
    assert!(light.at.abs_diff_eq(muzzle.at, 1e-3), "{:?} at the view model's muzzle {:?}", light.at, muzzle.at);
    assert!(light.follow && light.owner == Some(p), "the muzzle light goes with its shooter");
    // Without a view model: the first-person model's bind-pose marker.
    let (mut sim, p) = armed(false);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    let (light, eye) = shot_light(&mut sim, p);
    let gun = library().guns["weapons\\assault rifle\\assault rifle"].clone();
    let bind = bind_pose_frame(eye, &gun, "primary trigger");
    assert!(light.at.abs_diff_eq(bind.at, 1e-3), "{:?} at the bind pose's {:?}", light.at, bind.at);
}

#[test]
fn everyone_else_sees_them_at_the_held_models_muzzle() {
    let (mut sim, p) = armed(true);
    // Someone else is the local player; the shooter's held model has its
    // muzzle attachment here.
    sim.app.world_mut().spawn(LocalPlayer);
    let held = Vec3::new(4.0, 1.5, -2.0);
    sim.app.world_mut().spawn((
        HeldAttachment { owner: p, index: 0, name: "primary trigger".into() },
        GlobalTransform::from(Transform::from_translation(held)),
    ));
    let (light, _) = shot_light(&mut sim, p);
    assert!(light.at.abs_diff_eq(held, 1e-4), "{:?} at the held model's muzzle", light.at);
}

/// The shown projectiles of `p`'s next shot, as spawned (a bullet may
/// hit the greybox's wall and go in the same update).
fn shown_of_a_shot(sim: &mut Sim, p: Entity) -> Vec<halo_ce::game::fx::shots::HaloShown> {
    use halo_ce::game::fx::shots::HaloShown;
    #[derive(Resource, Default)]
    struct Seen(Vec<HaloShown>);
    sim.app.init_resource::<Seen>();
    sim.app.add_observer(|add: On<Add<HaloShown>>, q: Query<&HaloShown>, mut seen: ResMut<Seen>| {
        if let Ok(s) = q.get(add.entity) {
            seen.0.push(s.clone());
        }
    });
    shot_light(sim, p);
    std::mem::take(&mut sim.app.world_mut().resource_mut::<Seen>().0).into_iter().filter(|s| s.owner == p).collect()
}

#[test]
fn everyone_else_sees_its_projectiles_leave_the_held_guns_muzzle() {
    use halo_ce::game::fx::shots::{DRAWN_BLEND, drawn_offset};
    let k = halo_ce::game::METERS_PER_UNIT;
    let (mut sim, p) = armed(true);
    sim.app.world_mut().spawn(LocalPlayer);
    // The held gun's muzzle in front of the body at the hand's height,
    // well below and beside the eye.
    let eye = sim.position(p) + sim.app.world().get::<MovementState>(p).unwrap().eye_offset;
    let held = eye + Vec3::new(0.25, -0.45, -0.5);
    sim.app.world_mut().spawn((
        chimera_world::map::view_model::HeldMuzzle { owner: p },
        GlobalTransform::from(Transform::from_translation(held)),
    ));
    let shown = shown_of_a_shot(&mut sim, p);
    assert!(!shown.is_empty(), "a shown projectile");
    for s in &shown {
        let (start, offset) = s.drawn;
        // Drawn from the muzzle; its flight (the hits) still from the eye.
        assert!((start * k + offset).abs_diff_eq(held, 1e-3), "drawn from {:?}, muzzle {held:?}", start * k + offset);
        assert!((start * k).distance(eye) < 0.3, "its flight from the eye: {:?} vs {eye:?}", start * k);
        // Meets its true line within DRAWN_BLEND metres.
        let dir = s.flight.velocity.normalize();
        assert_eq!(drawn_offset(s.drawn, start + dir * (DRAWN_BLEND / k + 0.01)), Vec3::ZERO);
        assert!(drawn_offset(s.drawn, start + dir * (DRAWN_BLEND / k * 0.5)).abs_diff_eq(offset * 0.5, 1e-4));
    }
    // The shooter's own (first person): from the eye as Halo draws it.
    let (mut sim, p) = armed(true);
    sim.app.world_mut().entity_mut(p).insert(LocalPlayer);
    sim.app.world_mut().spawn((
        chimera_world::map::view_model::HeldMuzzle { owner: p },
        GlobalTransform::from(Transform::from_translation(held)),
    ));
    let shown = shown_of_a_shot(&mut sim, p);
    assert!(!shown.is_empty() && shown.iter().all(|s| s.drawn.1 == Vec3::ZERO));
}
